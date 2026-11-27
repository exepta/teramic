//! Persistent, bounded GPU state. CPU uploads authority changes, never particle motion.
use super::*;
use bevy::{
    core_pipeline::schedule::camera_driver,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_resource::{binding_types::*, *},
        renderer::{RenderContext, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue},
        texture::GpuImage,
    },
};
use std::{
    borrow::Cow,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

const HEADS: usize = 32 * 32 * 32;
const FIELD: usize = 128 * 128 * 128;
const FIXED_DT: f32 = 1.0 / 120.0;
const MAX_SUBSTEPS: u32 = 4;

/// An idle solver accrues no debt. A long render stall discards excess debt
/// rather than launching a burst of pressure corrections on the next frame.
#[derive(Default)]
struct PhysicsClock {
    remainder: f64,
    steps: u64,
}
impl PhysicsClock {
    fn advance(&mut self, elapsed: f32, active: bool) -> u32 {
        if !active {
            self.remainder = 0.0;
            return 0;
        }
        let dt = f64::from(FIXED_DT);
        self.remainder =
            (self.remainder + f64::from(elapsed.max(0.0))).min(dt * f64::from(MAX_SUBSTEPS));
        let count = (self.remainder / dt).floor() as u32;
        self.remainder -= f64::from(count) * dt;
        self.steps += u64::from(count);
        count
    }
    fn seconds(&self) -> f32 {
        (self.steps as f64 * f64::from(FIXED_DT)) as f32
    }
}
#[derive(Resource, Clone, ExtractResource)]
#[extract_app(RenderApp)]
struct Frame {
    input: Arc<ParticleFluidInput>,
    dt: f32,
}
#[derive(Resource, Clone, Default, ExtractResource)]
#[extract_app(RenderApp)]
struct Shared(Arc<Mutex<ParticleFluidStatus>>);
#[derive(Clone, ShaderType)]
struct Params {
    shift_dt: Vec4,
    counts: UVec4,
    h: Vec4,
    volume: Vec4,
    frame: Vec4,
}

pub(super) fn install(app: &mut App) {
    app.insert_resource(Frame {
        input: Arc::new(ParticleFluidInput::default()),
        dt: 1. / 60.,
    })
    .init_resource::<Shared>()
    .add_systems(PostUpdate, publish)
    .add_systems(PreUpdate, receive_status);
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .add_systems(RenderStartup, initialize)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareBindGroups))
            .add_systems(RenderGraph, simulate.before(camera_driver))
            .add_systems(RenderGraph, map_readback.in_set(RenderGraphSystems::Finish));
        app.add_plugins((
            ExtractResourcePlugin::<Frame>::default(),
            ExtractResourcePlugin::<Shared>::default(),
        ));
    }
}
fn publish(input: Res<ParticleFluidInput>, time: Res<Time>, mut frame: ResMut<Frame>) {
    static DISABLE: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        std::env::args().any(|a| a.starts_with("--smoke-test"))
            && std::env::var("TERRA_PARTICLE_FLUIDS_DISABLE").is_ok_and(|v| v == "1")
    });
    if input.is_changed() || frame.input.revision != input.revision {
        let mut copy = input.clone();
        if *DISABLE {
            copy.enabled = false;
        }
        frame.input = Arc::new(copy);
    }
    frame.dt = time.delta_secs().clamp(0.0, 0.1);
}
fn receive_status(shared: Res<Shared>, mut status: ResMut<ParticleFluidStatus>) {
    if let Ok(report) = shared.0.lock() {
        *status = report.clone();
    }
}
#[derive(Resource)]
struct Gpu {
    solver_layout: BindGroupLayoutDescriptor,
    field_layout: BindGroupLayoutDescriptor,
    solver: Vec<CachedComputePipelineId>,
    field: Vec<CachedComputePipelineId>,
    params: UniformBuffer<Params>,
    seed: Buffer,
    state: Buffer,
    predicted: Buffer,
    scratch: Buffer,
    grid: Buffer,
    shapes: Buffer,
    density: Buffer,
    tar: Buffer,
    bind: Option<(BindGroup, BindGroup)>,
    images: Option<(bevy::asset::AssetId<Image>, bevy::asset::AssetId<Image>)>,
    uploaded_input: Option<(u64, [f32; 3])>,
    origin: Vec3,
    initialized: bool,
    frames: u64,
    clock: PhysicsClock,
    active: u32,
    had_particles: bool,
    field_elapsed: f32,
    rebase_field: bool,
    staging: Buffer,
    readback_token: Option<u64>,
    pending_map: Option<(u64, f32)>,
    busy: Arc<AtomicBool>,
}
fn buffer(device: &RenderDevice, label: &'static str, size: usize) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: size as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}
fn initialize(
    mut commands: Commands,
    device: Res<RenderDevice>,
    server: Res<AssetServer>,
    cache: Res<PipelineCache>,
) {
    let solver_layout = BindGroupLayoutDescriptor::new(
        "particle PBF solver",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<Params>(false),
                storage_buffer_read_only_sized(false, None),
                storage_buffer_sized(false, None),
                storage_buffer_sized(false, None),
                storage_buffer_sized(false, None),
                storage_buffer_sized(false, None),
                storage_buffer_read_only_sized(false, None),
            ),
        ),
    );
    let field_layout = BindGroupLayoutDescriptor::new(
        "particle fluid reconstruction",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<Params>(false),
                storage_buffer_read_only_sized(false, None),
                storage_buffer_sized(false, None),
                storage_buffer_sized(false, None),
                texture_storage_3d(TextureFormat::Rgba16Float, StorageTextureAccess::WriteOnly),
                texture_storage_2d_array(
                    TextureFormat::Rgba16Float,
                    StorageTextureAccess::WriteOnly,
                ),
            ),
        ),
    );
    let solver_shader = server.load("shaders/terra_particle_solver.wesl");
    let solver = [
        "reconcile",
        "predict",
        "clear_grid",
        "build_grid",
        "density",
        "correction",
        "apply_correction",
        "velocity",
        "commit",
    ]
    .into_iter()
    .map(|name| {
        cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some(Cow::Borrowed(name)),
            layout: vec![solver_layout.clone()],
            shader: solver_shader.clone(),
            entry_point: Some(Cow::Borrowed(name)),
            ..default()
        })
    })
    .collect();
    let field_shader = server.load("shaders/terra_particle_field.wesl");
    let field = ["clear", "splat", "pack"]
        .into_iter()
        .map(|name| {
            cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some(Cow::Borrowed(name)),
                layout: vec![field_layout.clone()],
                shader: field_shader.clone(),
                entry_point: Some(Cow::Borrowed(name)),
                ..default()
            })
        })
        .collect();
    let params = UniformBuffer::from(Params {
        shift_dt: Vec4::ZERO,
        counts: UVec4::ZERO,
        h: Vec4::splat(0.475),
        volume: Vec4::ZERO,
        frame: Vec4::ZERO,
    });
    commands.insert_resource(Gpu {
        solver_layout,
        field_layout,
        solver,
        field,
        params,
        seed: buffer(&device, "authority particle seeds", MAX_PARTICLES * 64),
        state: buffer(&device, "persistent PBF particles", MAX_PARTICLES * 64),
        predicted: buffer(&device, "PBF predicted positions", MAX_PARTICLES * 16),
        scratch: buffer(
            &device,
            "PBF density/pressure/viscosity",
            MAX_PARTICLES * 64,
        ),
        grid: buffer(
            &device,
            "PBF linked spatial grid",
            (HEADS + MAX_PARTICLES) * 4,
        ),
        shapes: buffer(
            &device,
            "fluid collision and path constraints",
            (MAX_COLLIDERS + MAX_PATHS) * 48,
        ),
        density: buffer(
            &device,
            "fluid packed reconstruction density",
            (FIELD + 4096) * 4,
        ),
        tar: buffer(&device, "thin tar reconstruction", 128 * 128 * 16 * 16),
        bind: None,
        images: None,
        uploaded_input: None,
        origin: Vec3::ZERO,
        initialized: false,
        frames: 0,
        clock: PhysicsClock::default(),
        active: 0,
        had_particles: false,
        field_elapsed: 1.0,
        rebase_field: true,
        staging: device.create_buffer(&BufferDescriptor {
            label: Some("one-shot fluid diagnostics"),
            size: (MAX_PARTICLES * 64) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        }),
        readback_token: None,
        pending_map: None,
        busy: Arc::new(AtomicBool::new(false)),
    });
}
fn encoded_seed(input: &ParticleFluidInput) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(MAX_PARTICLES * 64);
    for i in 0..MAX_PARTICLES {
        let seed = input.particles.get(i).copied().unwrap_or_default();
        for v in seed
            .position
            .into_iter()
            .chain([1.])
            .chain(seed.velocity)
            .chain([0.])
            .chain(seed.flow_origin)
            .chain([seed.flow_radius])
        {
            bytes.extend(v.to_le_bytes());
        }
        for v in [seed.generation, seed.kind, 0, 0] {
            bytes.extend(v.to_le_bytes());
        }
    }
    bytes
}
fn encoded_shapes(input: &ParticleFluidInput) -> Vec<u8> {
    let mut bytes = Vec::with_capacity((input.solids.len() + input.paths.len()) * 48);
    for (lo, hi, mask) in input
        .solids
        .iter()
        .map(|s| (s.minimum, s.maximum, s.blocked_kinds))
        .chain(input.paths.iter().map(|s| (s.minimum, s.maximum, 0)))
    {
        for v in lo.into_iter().chain([0.]).chain(hi).chain([0.]) {
            bytes.extend(v.to_le_bytes());
        }
        for v in [mask, 0, 0, 0] {
            bytes.extend(v.to_le_bytes());
        }
    }
    bytes
}
fn encoded_legality(input: &ParticleFluidInput) -> Vec<u8> {
    let mut mask = vec![0u32; 4096];
    for path in &input.paths {
        let y = path.minimum[1].floor().clamp(0.0, 15.0) as usize;
        for z in
            path.minimum[2].floor().max(0.0) as usize..path.maximum[2].ceil().min(16.0) as usize
        {
            for x in
                path.minimum[0].floor().max(0.0) as usize..path.maximum[0].ceil().min(16.0) as usize
            {
                mask[x + 16 * (y + 16 * z)] |= 1;
            }
        }
    }
    let mut origins = [
        std::collections::HashSet::new(),
        std::collections::HashSet::new(),
    ];
    for seed in &input.particles {
        if seed.kind == GALLE || seed.kind == TAR {
            origins[(seed.kind - GALLE) as usize].insert((
                seed.flow_origin[0].floor() as i32,
                seed.flow_origin[2].floor() as i32,
                seed.flow_radius.floor() as i32,
            ));
        }
    }
    for z in 0..16 {
        for x in 0..16 {
            for (kind, sources) in origins.iter().enumerate() {
                if sources
                    .iter()
                    .any(|&(a, b, r)| r <= 0 || (x as i32 - a).abs() + (z as i32 - b).abs() <= r)
                {
                    for y in 0..16 {
                        mask[x + 16 * (y + 16 * z)] |= 2 << kind;
                    }
                }
            }
        }
    }
    mask.into_iter().flat_map(u32::to_le_bytes).collect()
}

fn input_error(input: &ParticleFluidInput) -> Option<String> {
    if input.particles.len() > MAX_PARTICLES
        || input.solids.len() > MAX_COLLIDERS
        || input.paths.len() > MAX_PATHS
    {
        return Some("Particle fluid input exceeds its fixed GPU budget".into());
    }
    if input
        .origin
        .into_iter()
        .chain(input.target_mass)
        .any(|v| !v.is_finite())
        || input.target_mass.iter().any(|v| *v < 0.)
    {
        return Some("Nonfinite particle fluid domain/mass".into());
    }
    for s in &input.particles {
        if s.position
            .into_iter()
            .chain(s.velocity)
            .chain(s.flow_origin)
            .chain([s.flow_radius])
            .any(|v| !v.is_finite())
            || (s.kind > 3 && s.kind != INACTIVE)
        {
            return Some("Invalid fluid particle seed".into());
        }
    }
    for (lo, hi) in input
        .solids
        .iter()
        .map(|s| (s.minimum, s.maximum))
        .chain(input.paths.iter().map(|s| (s.minimum, s.maximum)))
    {
        if (0..3).any(|i| !lo[i].is_finite() || !hi[i].is_finite() || lo[i] >= hi[i]) {
            return Some("Invalid fluid collision/path bounds".into());
        }
    }
    None
}
// The buffers can already contain the current input while the solver is idle
// or waiting for pipelines. Its last simulated origin is tracked separately.
fn input_upload_changed(
    uploaded: &mut Option<(u64, [f32; 3])>,
    input: &ParticleFluidInput,
) -> bool {
    let key = (input.revision, input.origin);
    uploaded.replace(key) != Some(key)
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    mut gpu: ResMut<Gpu>,
    frame: Res<Frame>,
    images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    cache: Res<PipelineCache>,
    shared: Res<Shared>,
) {
    let input = &frame.input;
    if let Some(error) = input_error(input) {
        if let Ok(mut report) = shared.0.lock() {
            report.error = Some(error);
            report.gpu_ready = false;
        }
        gpu.bind = None;
        return;
    }
    let mut counts = [0u32; 4];
    for seed in &input.particles {
        if seed.kind < 4 {
            counts[seed.kind as usize] += 1;
        }
    }
    gpu.active = if input.enabled {
        counts.iter().sum()
    } else {
        0
    };
    if let Ok(mut report) = shared.0.lock() {
        report.enabled = input.enabled;
        report.active_particles = gpu.active;
        report.requested_particles = input.particles.iter().filter(|s| s.kind < 4).count() as u32;
        report.error = None;
    }
    let origin = Vec3::from_array(input.origin);
    if input_upload_changed(&mut gpu.uploaded_input, input) {
        queue.write_buffer(&gpu.seed, 0, &encoded_seed(input));
        let shapes = encoded_shapes(input);
        if !shapes.is_empty() {
            queue.write_buffer(&gpu.shapes, 0, &shapes);
        }
        queue.write_buffer(&gpu.density, (FIELD * 4) as u64, &encoded_legality(input));
    }
    let mut h = [0.475f32; 4];
    let mut volume = [0f32; 4];
    for kind in 0..4 {
        if counts[kind] > 0 {
            volume[kind] = input.target_mass[kind] / counts[kind] as f32;
            let normalized = volume[kind] / particle_volume(kind as u32);
            h[kind] = (0.475 * normalized.max(0.125).cbrt()).clamp(0.24, 2.0);
        }
    }
    let cell = h.iter().copied().fold(0.5f32, f32::max);
    let shift = if gpu.initialized {
        gpu.origin - origin
    } else {
        Vec3::ZERO
    };
    let frames = gpu.frames as f32;
    gpu.rebase_field |= shift.length_squared() > 0.0;
    gpu.params.set(Params {
        shift_dt: shift.extend(FIXED_DT),
        counts: UVec4::new(
            input.particles.len() as u32,
            input.solids.len() as u32,
            input.paths.len() as u32,
            0,
        ),
        h: Vec4::from_array(h),
        volume: Vec4::from_array(volume),
        frame: Vec4::new(frames, cell, f32::from(input.enabled), 0.),
    });
    gpu.params.write_buffer(&device, &queue);
    let (Some(field), Some(tar)) = (
        images.get(super::surface::FIELD_IMAGE.id()),
        images.get(super::surface::TAR_IMAGE.id()),
    ) else {
        return;
    };
    let image_ids = (
        super::surface::FIELD_IMAGE.id(),
        super::surface::TAR_IMAGE.id(),
    );
    if gpu.bind.is_none() || gpu.images != Some(image_ids) {
        let solver = device.create_bind_group(
            "PBF solver",
            &cache.get_bind_group_layout(&gpu.solver_layout),
            &BindGroupEntries::sequential((
                &gpu.params,
                gpu.seed.as_entire_binding(),
                gpu.state.as_entire_binding(),
                gpu.predicted.as_entire_binding(),
                gpu.scratch.as_entire_binding(),
                gpu.grid.as_entire_binding(),
                gpu.shapes.as_entire_binding(),
            )),
        );
        let reconstruction = device.create_bind_group(
            "PBF surface field",
            &cache.get_bind_group_layout(&gpu.field_layout),
            &BindGroupEntries::sequential((
                &gpu.params,
                gpu.state.as_entire_binding(),
                gpu.density.as_entire_binding(),
                gpu.tar.as_entire_binding(),
                &field.texture_view,
                &tar.texture_view,
            )),
        );
        gpu.bind = Some((solver, reconstruction));
        gpu.images = Some(image_ids);
    }
}
fn simulate(
    mut context: RenderContext,
    mut gpu: ResMut<Gpu>,
    cache: Res<PipelineCache>,
    frame: Res<Frame>,
    shared: Res<Shared>,
) {
    for pipeline in gpu.solver.iter().chain(&gpu.field) {
        match cache.get_compute_pipeline_state(*pipeline) {
            CachedPipelineState::Ok(_) => {}
            CachedPipelineState::Err(bevy::shader::ShaderCacheError::ShaderNotLoaded(_))
            | CachedPipelineState::Err(
                bevy::shader::ShaderCacheError::ShaderImportNotYetAvailable,
            ) => return,
            CachedPipelineState::Err(error) => {
                if let Ok(mut report) = shared.0.lock() {
                    report.error = Some(format!("Fluid GPU pipeline: {error}"));
                    report.gpu_ready = false;
                }
                return;
            }
            _ => return,
        }
    }
    if gpu.bind.is_none() {
        return;
    }
    if let Ok(mut report) = shared.0.lock() {
        report.gpu_ready = true;
    }
    let pending_readback = frame
        .input
        .readback_after_frames
        .is_some_and(|token| gpu.readback_token != Some(token));
    if (gpu.active == 0 || !frame.input.enabled) && !gpu.had_particles && !pending_readback {
        gpu.clock.advance(0.0, false);
        return;
    }
    let active = gpu.active > 0 && frame.input.enabled;
    let substeps = gpu.clock.advance(frame.dt, active);
    gpu.field_elapsed += frame.dt;
    let reconstruct = gpu.field_elapsed >= 1.0 / 30.0
        || gpu.rebase_field
        || !gpu.initialized
        || (gpu.active == 0 && gpu.had_particles);
    let Some((solver, field)) = &gpu.bind else {
        return;
    };
    let count = (frame.input.particles.len() as u32).div_ceil(64);
    {
        let mut pass = context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor::default());
        pass.set_bind_group(0, solver, &[]);
        {
            let mut run = |index: usize, groups: u32| {
                if let Some(pipeline) = cache.get_compute_pipeline(gpu.solver[index]) {
                    pass.set_pipeline(pipeline);
                    pass.dispatch_workgroups(groups, 1, 1);
                }
            };
            run(0, 128);
            for _ in 0..substeps {
                run(1, count);
                for _ in 0..8 {
                    run(2, (HEADS as u32).div_ceil(64));
                    run(3, count);
                    run(4, count);
                    run(5, count);
                    run(6, count);
                }
                run(7, count);
                run(8, count);
            }
        }
        if reconstruct {
            pass.set_bind_group(0, field, &[]);
            pass.set_pipeline(cache.get_compute_pipeline(gpu.field[0]).unwrap());
            pass.dispatch_workgroups((FIELD as u32).div_ceil(64), 1, 1);
            if gpu.active > 0 {
                pass.set_pipeline(cache.get_compute_pipeline(gpu.field[1]).unwrap());
                pass.dispatch_workgroups(count, 1, 1);
            }
            pass.set_pipeline(cache.get_compute_pipeline(gpu.field[2]).unwrap());
            pass.dispatch_workgroups(32, 32, 32);
        }
    }
    if reconstruct {
        gpu.field_elapsed = 0.0;
        gpu.rebase_field = false;
    }
    gpu.origin = Vec3::from_array(frame.input.origin);
    gpu.had_particles = gpu.active > 0;
    gpu.initialized = true;
    gpu.frames += 1;
    if let Ok(mut report) = shared.0.lock() {
        report.gpu_ready = true;
        report.gpu_frames = gpu.frames;
        report.simulated_seconds = gpu.clock.seconds();
    }
    if let Some(token) = frame.input.readback_after_frames
        && gpu.frames >= token
        && gpu.readback_token != Some(token)
        && !gpu.busy.swap(true, Ordering::AcqRel)
    {
        context.command_encoder().copy_buffer_to_buffer(
            &gpu.state,
            0,
            &gpu.staging,
            0,
            (MAX_PARTICLES * 64) as u64,
        );
        gpu.pending_map = Some((gpu.frames, gpu.clock.seconds()));
        gpu.readback_token = Some(token);
    }
}
fn map_readback(mut gpu: ResMut<Gpu>, shared: Res<Shared>) {
    let Some((frame, simulated_seconds)) = gpu.pending_map.take() else {
        return;
    };
    let buffer = gpu.staging.clone();
    let callback_buffer = buffer.clone();
    let report = shared.0.clone();
    let busy = gpu.busy.clone();
    buffer.slice(..).map_async(MapMode::Read, move |result| {
        if result.is_ok() {
            if let Ok(data) = callback_buffer.slice(..).get_mapped_range() {
                let particles = data
                    .chunks_exact(64)
                    .map(|p| {
                        let get =
                            |offset| f32::from_le_bytes(p[offset..offset + 4].try_into().unwrap());
                        let kind = u32::from_le_bytes(p[52..56].try_into().unwrap());
                        let active = u32::from_le_bytes(p[56..60].try_into().unwrap());
                        [
                            get(0),
                            get(4),
                            get(8),
                            if active == 0 { -1. } else { kind as f32 },
                        ]
                    })
                    .collect();
                let velocities = data
                    .chunks_exact(64)
                    .map(|p| {
                        std::array::from_fn(|axis| {
                            f32::from_le_bytes(p[16 + axis * 4..20 + axis * 4].try_into().unwrap())
                        })
                    })
                    .collect();
                if let Ok(mut status) = report.lock() {
                    status.readback = Some(ParticleFluidReadback {
                        frame,
                        simulated_seconds,
                        particles,
                        velocities,
                    });
                }
                drop(data);
            }
            callback_buffer.unmap();
        } else if let Ok(mut status) = report.lock() {
            status.error = Some("Fluid diagnostic readback failed".into());
        }
        busy.store(false, Ordering::Release);
    });
}

#[cfg(test)]
mod tests {
    use super::{
        FIXED_DT, FluidParticleSeed, MAX_SUBSTEPS, ParticleFluidInput, PhysicsClock, WATER,
        input_upload_changed,
    };

    #[test]
    fn authority_uploads_follow_input_revisions_and_domain_rebases_during_idle_frames() {
        let mut uploaded = None;
        let mut input = ParticleFluidInput::default();
        assert!(input_upload_changed(&mut uploaded, &input));
        for _ in 0..120 {
            assert!(!input_upload_changed(&mut uploaded, &input));
        }

        input.particles.push(FluidParticleSeed {
            kind: WATER,
            ..Default::default()
        });
        input.revision += 1;
        assert!(input_upload_changed(&mut uploaded, &input));
        input.particles.clear();
        input.revision += 1;
        assert!(input_upload_changed(&mut uploaded, &input));
        assert!(!input_upload_changed(&mut uploaded, &input));

        input.origin[0] += 16.0;
        assert!(input_upload_changed(&mut uploaded, &input));
        input.readback_after_frames = Some(1);
        assert!(!input_upload_changed(&mut uploaded, &input));
    }

    #[test]
    fn fixed_physics_time_is_independent_of_render_frame_rate() {
        for fps in [30, 60, 144, 240, 1000] {
            let mut clock = PhysicsClock::default();
            for _ in 0..fps * 4 {
                assert!(clock.advance(1.0 / fps as f32, true) <= MAX_SUBSTEPS);
            }
            // f32 frame periods may leave one unconsumed fractional step.
            assert!(
                (clock.seconds() - 4.0).abs() <= FIXED_DT * 1.1,
                "{fps} render FPS integrated {} physical seconds",
                clock.seconds()
            );
        }
    }

    #[test]
    fn idle_and_zero_elapsed_never_advance_or_accumulate_catch_up_debt() {
        let mut clock = PhysicsClock::default();
        assert_eq!(clock.advance(0.0, true), 0);
        assert_eq!(clock.advance(50.0, false), 0);
        assert_eq!(clock.advance(FIXED_DT * 0.4, true), 0);
        assert_eq!(clock.advance(50.0, false), 0);
        assert_eq!(clock.advance(FIXED_DT * 0.7, true), 0);
        assert_eq!(clock.advance(FIXED_DT * 0.4, true), 1);
        assert_eq!(clock.steps, 1);
    }

    #[test]
    fn render_stalls_are_bounded_without_a_later_pressure_burst() {
        let mut clock = PhysicsClock::default();
        assert_eq!(clock.advance(30.0, true), MAX_SUBSTEPS);
        assert_eq!(clock.advance(0.0, true), 0);
        assert!((clock.seconds() - MAX_SUBSTEPS as f32 * FIXED_DT).abs() < 1e-6);
    }
}
