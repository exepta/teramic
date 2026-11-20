//! Real GPU particle motion, collision, free surfaces and source withdrawal.
use super::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use terra_render::{
    FluidCollider, FluidParticleSeed, FluidPath, ParticleFluidInput, ParticleFluidStatus,
};

#[derive(Resource)]
struct Inspection {
    start: Instant,
    base: i64,
    placed: bool,
    seeded: bool,
    shot: usize,
    request: u64,
    pending: bool,
    last_report: Instant,
    initial_y: [f32; 4],
    simulation_start: f32,
    motion_start: Instant,
    stress: bool,
}

pub(super) fn install(app: &mut App) {
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(Inspection {
            start: Instant::now(),
            base: 0,
            placed: false,
            seeded: false,
            shot: 0,
            request: 0,
            pending: false,
            last_report: Instant::now(),
            initial_y: [0.; 4],
            simulation_start: 0.,
            motion_start: Instant::now(),
            stress: std::env::var("TERRA_PARTICLE_STRESS").as_deref() == Ok("1"),
        })
        .add_systems(PostStartup, setup)
        .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}
fn pose(y: i64) -> Transform {
    Transform::from_xyz(20., y as f32 + 12., 23.)
        .looking_at(Vec3::new(8., y as f32 + 2., 8.), Vec3::Y)
}
fn setup(
    mut state: ResMut<Inspection>,
    terrain: Res<WorldTerrain>,
    mut focus: ResMut<ChunkFocus>,
    mut cameras: Query<&mut Transform, With<TerraCamera>>,
    mut config: ResMut<terra_core::GameConfig>,
) {
    state.base = ((terrain.0.surface_height(8., 8.) / 16.).ceil() as i64 + 2) * 16 - 1;
    let p = pose(state.base);
    for mut camera in &mut cameras {
        *camera = p;
    }
    focus.0 = p.translation.to_array().map(f64::from);
    config.client.fov_degrees = 48.;
}

#[derive(SystemParam)]
struct Scene<'w, 's> {
    chunks: ResMut<'w, ChunkStore>,
    journal: ResMut<'w, EditJournal>,
    properties: Res<'w, terra_core::block_properties::BlockPropertiesCatalog>,
    render: Res<'w, RenderStatus>,
    input: ResMut<'w, ParticleFluidInput>,
    gpu: Res<'w, ParticleFluidStatus>,
    focus: ResMut<'w, ChunkFocus>,
    cameras: Query<'w, 's, &'static mut Transform, With<TerraCamera>>,
}
fn place(scene: &mut Scene, p: [i64; 3], material: u16) -> bool {
    let Some(expected_material) = scene.chunks.block_at(p) else {
        return false;
    };
    terra_world::streaming::commit_block_edit(
        &mut scene.chunks,
        &mut scene.journal,
        &BlockEditRequest {
            position: p,
            expected_material,
            material,
        },
        &scene.properties,
    )
    .unwrap();
    true
}
fn floor(scene: &mut Scene, y: i64) -> bool {
    let positions: Vec<_> = (0..16)
        .flat_map(|x| (0..16).map(move |z| [x, y, z]))
        .collect();
    if positions
        .iter()
        .any(|p| scene.chunks.block_at(*p).is_none())
    {
        return false;
    }
    for p in positions {
        place(scene, p, 3);
    }
    // A real half slab and wall, plus a prepared tar patch visible in the game.
    place(scene, [5, y + 1, 3], 82);
    for z in 1..=6 {
        for h in 1..=3 {
            place(scene, [13, y + h, z], 3);
        }
    }
    for x in 9..=14 {
        for z in 9..=14 {
            place(scene, [x, y + 1, z], 102);
        }
    }
    true
}
fn seed(input: &mut ParticleFluidInput, y: i64, generation: u32) {
    input.enabled = true;
    input.origin = [0., y as f32, 0.];
    input.solids = vec![
        FluidCollider {
            minimum: [0., 0., 0.],
            maximum: [16., 1., 16.],
            blocked_kinds: 15,
        },
        FluidCollider {
            minimum: [5., 1., 3.],
            maximum: [6., 1.5, 4.],
            blocked_kinds: 15,
        },
        FluidCollider {
            minimum: [13., 1., 1.],
            maximum: [14., 4., 7.],
            blocked_kinds: 15,
        },
        FluidCollider {
            minimum: [9., 1., 9.],
            maximum: [15., 1.9375, 15.],
            blocked_kinds: 15,
        },
    ];
    input.paths = vec![FluidPath {
        minimum: [9., 1.9375, 9.],
        maximum: [15., 2., 15.],
    }];
    input.particles = Vec::new();
    for kind in 0..4 {
        for i in 0..64 {
            let x = (i % 4) as f32;
            let z = (i / 4 % 4) as f32;
            let layer = (i / 16) as f32;
            let position = match kind {
                0 => [
                    3. + (x + 0.5) * 0.25,
                    4. + (layer + 0.5) * 0.25,
                    3. + (z + 0.5) * 0.25,
                ],
                1 => [
                    10. + (x + 0.5) * 0.25,
                    4. + (layer + 0.5) * 0.25,
                    3. + (z + 0.5) * 0.25,
                ],
                2 => [
                    3. + (x + 0.5) * 0.25,
                    4. + (layer + 0.5) * 0.25,
                    10. + (z + 0.5) * 0.25,
                ],
                _ => [
                    10. + (i % 8) as f32 * 0.125 + 0.0625,
                    1.966,
                    10. + (i / 8) as f32 * 0.125 + 0.0625,
                ],
            };
            input.particles.push(FluidParticleSeed {
                position,
                velocity: [if kind == 3 { 0.8 } else { 1.6 }, 0., 0.],
                kind,
                generation: generation + i + kind * 64,
                flow_origin: if kind == 2 {
                    [3., 1., 10.]
                } else {
                    [10., 1., 10.]
                },
                flow_radius: if kind >= 2 { 4. } else { 0. },
            });
        }
    }
    input.target_mass = [1., 1., 1., 1. / 16.];
    input.revision += 1;
}

fn run(
    mut commands: Commands,
    mut state: ResMut<Inspection>,
    mut scene: Scene,
    mut exit: MessageWriter<AppExit>,
) {
    if state.shot >= 4 {
        return;
    }
    if state.start.elapsed() > Duration::from_secs(150)
        || scene.gpu.error.is_some()
        || scene.render.error.is_some()
    {
        error!(gpu=?scene.gpu.error,render=?scene.render.error,"Particle fluid inspection failed");
        state.shot = 4;
        exit.write(AppExit::error());
        return;
    }
    for mut camera in &mut scene.cameras {
        *camera = pose(state.base);
    }
    scene.focus.0 = pose(state.base).translation.to_array().map(f64::from);
    if state.last_report.elapsed() > Duration::from_secs(10) {
        info!(
            shot = state.shot,
            placed = state.placed,
            seeded = state.seeded,
            render = scene.render.ready,
            gpu = scene.gpu.gpu_ready,
            frame = scene.gpu.gpu_frames,
            request = state.request,
            "Particle fluid inspection progress"
        );
        state.last_report = Instant::now();
    }
    if state.pending || !scene.render.ready {
        return;
    }
    if !state.placed {
        if floor(&mut scene, state.base) {
            state.placed = true;
        }
        return;
    }
    if !scene.gpu.gpu_ready {
        return;
    }
    if !state.seeded {
        seed(&mut scene.input, state.base, 1);
        if state.stress {
            scene.input.particles = (0..8192)
                .map(|i| FluidParticleSeed {
                    position: [
                        4. + (i % 32) as f32 * 0.25 + 0.125,
                        4. + (i / 1024) as f32 * 0.25 + 0.125,
                        4. + (i / 32 % 32) as f32 * 0.25 + 0.125,
                    ],
                    velocity: [0.; 3],
                    kind: 0,
                    generation: i + 1,
                    flow_origin: [0.; 3],
                    flow_radius: 0.,
                })
                .collect();
            scene.input.target_mass = [128., 0., 0., 0.];
        }
        state.seeded = true;
        state.simulation_start = scene.gpu.simulated_seconds;
        state.motion_start = Instant::now();
        return;
    }
    if state.request == 0 {
        let elapsed = scene.gpu.simulated_seconds - state.simulation_start;
        let target = match state.shot {
            0 => 0.07,
            1 => 0.75,
            2 => 4.0,
            _ => 0.0,
        };
        if elapsed < target {
            return;
        }
        if state.shot == 3 {
            for p in &mut scene.input.particles {
                p.kind = u32::MAX;
            }
            scene.input.target_mass = [0.; 4];
            scene.input.revision += 1;
        }
        state.request = scene.gpu.gpu_frames + 2;
        scene.input.readback_after_frames = Some(state.request);
        return;
    }
    let Some(readback) = scene
        .gpu
        .readback
        .as_ref()
        .filter(|r| r.frame >= state.request)
    else {
        return;
    };
    let mut count = [0u32; 4];
    let mut sum = [[0f32; 3]; 4];
    let mut bounds_min = [[f32::MAX; 3]; 4];
    let mut bounds_max = [[f32::MIN; 3]; 4];
    for p in &readback.particles {
        assert!(
            p.iter().all(|v| v.is_finite()),
            "GPU solver emitted NaN/Inf"
        );
        if p[3] < 0. {
            continue;
        }
        let k = p[3] as usize;
        assert!(k < 4);
        count[k] += 1;
        for i in 0..3 {
            sum[k][i] += p[i];
            bounds_min[k][i] = bounds_min[k][i].min(p[i]);
            bounds_max[k][i] = bounds_max[k][i].max(p[i]);
            assert!((-0.001..=16.001).contains(&p[i]), "Particle escaped domain");
        }
        assert!(p[1] >= 0.99, "Particle penetrated the actual solid floor");
        if k == 3 {
            assert!(
                (1.936..=2.002).contains(&p[1]),
                "Tar left its thin prepared path coating: {p:?}"
            );
            assert!((9.0..=15.001).contains(&p[0]) && (9.0..=15.001).contains(&p[2]));
        }
    }
    let centroid: Vec<[f32; 3]> = (0..4)
        .map(|k| sum[k].map(|v| v / count[k].max(1) as f32))
        .collect();
    let dir = PathBuf::from(std::env::var_os("TERRA_CAPTURE_DIR").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let name = ["airborne", "impact", "settled", "drained"][state.shot];
    std::fs::write(dir.join(format!("pbf-{}-{name}.json",state.shot)),serde_json::to_vec_pretty(&serde_json::json!({
        "phase":name,"gpu_frame":readback.frame,"simulated_seconds":readback.simulated_seconds-state.simulation_start,"counts":count,"centroid":centroid,"minimum":bounds_min,"maximum":bounds_max,
        "real_elapsed_seconds":state.motion_start.elapsed().as_secs_f64(),"stress":state.stress,
        "positions":readback.particles,"mass":scene.input.target_mass,"source_revision":scene.input.revision,"gpu_ready":scene.gpu.gpu_ready,
    })).unwrap()).unwrap();
    if state.shot < 3 {
        assert_eq!(
            count,
            if state.stress {
                [8192, 0, 0, 0]
            } else {
                [64; 4]
            },
            "GPU particle mass changed without authority"
        );
        if state.shot == 0 {
            for (k, point) in centroid.iter().enumerate() {
                state.initial_y[k] = point[1];
            }
        }
        if state.shot == 2 {
            for (k, c) in centroid.iter().enumerate().take(3) {
                if count[k] == 0 {
                    continue;
                }
                assert!(
                    c[1] < state.initial_y[k] - 0.35,
                    "Fluid did not actually fall and settle: {centroid:?}"
                );
                assert!(
                    state.stress || (c[1] < 1.8 && bounds_max[k][1] < 2.5),
                    "Fluid retained unphysical airborne energy after four simulated seconds: kind={k}, centroid={c:?}, maximum={:?}",
                    bounds_max[k]
                );
            }
        }
    } else {
        assert_eq!(count, [0; 4], "Source withdrawal left GPU fluid alive");
    }
    let path = dir.join(format!("pbf-{}-{name}.png", state.shot));
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path)).observe(|_:On<ScreenshotCaptured>,mut state:ResMut<Inspection>,mut exit:MessageWriter<AppExit>|{
        state.shot+=1;state.request=0;state.pending=false;
        if state.shot==4 {info!("GPU particle fluid gallery passed: four fluids, real gravity/collision, stable mass and complete source withdrawal");exit.write(AppExit::Success);}
    });
    state.pending = true;
}
