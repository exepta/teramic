//! Fixed authoritative cells, real native meshes and timed GPU interpolation.
use super::*;
#[path = "materials_smoke_flow_stability.rs"]
mod stability;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use terra_world::{edits::LiquidState, streaming::WorldAccess};

#[derive(Resource)]
struct Inspection {
    started: Instant,
    since: Instant,
    last_report: Instant,
    base: i64,
    fluid: usize,
    shot: usize,
    seeded: bool,
    edited: bool,
    pending: bool,
    publication: u64,
    blood_origin: [i64; 2],
}

#[derive(Resource, Default)]
struct FixtureOrigin([i64; 2]);

pub(super) fn install(app: &mut App) {
    stability::install(app);
    app.init_resource::<FixtureOrigin>()
        .insert_resource(ControllerMode::Fly)
        .insert_resource(Inspection {
            started: Instant::now(),
            since: Instant::now(),
            last_report: Instant::now(),
            base: 0,
            fluid: 0,
            shot: 0,
            seeded: false,
            edited: false,
            pending: false,
            publication: 0,
            blood_origin: [-1280, 1152],
        })
        .add_systems(PostStartup, setup)
        .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}

fn pose(base: i64, origin: [i64; 2]) -> Transform {
    let offset = Vec3::new(origin[0] as f32, 0.0, origin[1] as f32);
    Transform::from_translation(Vec3::new(11.0, base as f32 + 3.2, 12.5) + offset)
        .looking_at(Vec3::new(8.0, base as f32 + 0.5, 8.5) + offset, Vec3::Y)
}

fn setup(
    mut state: ResMut<Inspection>,
    terrain: Res<WorldTerrain>,
    mut focus: ResMut<ChunkFocus>,
    mut cameras: Query<&mut Transform, With<TerraCamera>>,
    mut config: ResMut<terra_core::GameConfig>,
    mut gizmos: ResMut<bevy::gizmos::config::GizmoConfigStore>,
) {
    let origin = if stability::enabled() {
        stability::ORIGIN
    } else {
        [0; 2]
    };
    let ground = terrain
        .0
        .surface_height((origin[0] + 8) as f64, (origin[1] + 8) as f64);
    state.base = ((ground / 16.0).ceil() as i64 + 2) * 16 - 1;
    let p = pose(state.base, origin);
    for mut camera in &mut cameras {
        *camera = p;
    }
    focus.0 = p.translation.to_array().map(f64::from);
    // The fourth case uses actual Blood Forest optics, not a manually colored proxy.
    state.blood_origin = (-2048..=2048)
        .step_by(64)
        .flat_map(|z| (-2048..=2048).step_by(64).map(move |x| [x, z]))
        .find(|p| {
            terrain
                .0
                .biome_at((p[0] + 8) as f64, (p[1] + 8) as f64)
                .weights
                .blood_forest
                > 0.75
        })
        .expect("native flow fixture needs a real Blood Forest sample");
    config.client.fov_degrees = 35.0;
    config.client.water_wave_strength = if stability::enabled() { 1.0 } else { 0.0 };
    gizmos
        .config_mut::<bevy::gizmos::config::DefaultGizmoConfigGroup>()
        .0
        .enabled = false;
    gizmos.config_mut::<ChunkBorderGizmos>().0.enabled = false;
}

#[derive(SystemParam)]
struct View<'w, 's> {
    chunks: ResMut<'w, ChunkStore>,
    journal: ResMut<'w, EditJournal>,
    access: ResMut<'w, WorldAccess>,
    properties: Res<'w, terra_core::block_properties::BlockPropertiesCatalog>,
    render: Res<'w, RenderStatus>,
    water: Res<'w, terra_render::WaterStatus>,
    flow: Res<'w, terra_render::FlowInterpolationStatus>,
    time: Res<'w, Time>,
    frame: Res<'w, bevy::diagnostic::FrameCount>,
    weather: Res<'w, terra_core::WorldWeather>,
    origin: ResMut<'w, FixtureOrigin>,
    cameras: Query<
        'w,
        's,
        (
            &'static mut Transform,
            &'static Camera,
            &'static GlobalTransform,
        ),
        With<TerraCamera>,
    >,
    focus: ResMut<'w, ChunkFocus>,
}

fn at(world: &View, p: [i64; 3]) -> [i64; 3] {
    [p[0] + world.origin.0[0], p[1], p[2] + world.origin.0[1]]
}

fn block(world: &mut View, position: [i64; 3], material: u16) {
    let position = at(world, position);
    let expected_material = world.chunks.block_at(position).unwrap();
    terra_world::streaming::commit_block_edit(
        &mut world.chunks,
        &mut world.journal,
        &BlockEditRequest {
            position,
            expected_material,
            material,
        },
        &world.properties,
    )
    .unwrap();
    terra_world::streaming::commit_object_state(
        &mut world.chunks,
        &mut world.journal,
        position,
        None,
    )
    .unwrap();
}

fn cell(world: &mut View, fluid: usize, p: [i64; 3], amount: u8, source: bool) {
    let position = at(world, p);
    if matches!(fluid, 0 | 3) {
        world
            .journal
            .set_fluid(
                position,
                terra_world::water::FluidCell {
                    amount,
                    source: amount > 0 && source,
                    flow: if source || amount == 0 {
                        [0; 2]
                    } else {
                        [1, 0]
                    },
                },
            )
            .unwrap();
        return;
    }
    let material = if fluid == 1 {
        terra_core::bile::FLUID
    } else {
        terra_core::roads::WET_TAR
    };
    if amount == 0 {
        block(
            world,
            p,
            if fluid == 2 {
                terra_world::roads::PATH
            } else {
                0
            },
        );
        return;
    }
    block(world, p, material);
    let age = if fluid == 1 {
        terra_core::bile::FINE
    } else {
        0
    };
    let facing = if fluid == 1 {
        terra_core::bile::facing(amount, age)
    } else {
        terra_core::roads::tar_state(amount.div_ceil(8), 0)
    };
    let liquid_origin = at(world, [7, p[1], 8]);
    terra_world::streaming::commit_object_state(
        &mut world.chunks,
        &mut world.journal,
        position,
        Some(BlockObjectState {
            facing,
            liquid: Some(LiquidState {
                volume: amount,
                age,
                source: fluid == 1 && source,
                origin: liquid_origin,
            }),
            ..Default::default()
        }),
    )
    .unwrap();
}

fn scene(world: &mut View, base: i64, fluid: usize) -> bool {
    let mut cells = Vec::new();
    for x in 5..=12 {
        for z in 5..=12 {
            cells.push(([x, base - 1, z], 3));
            if x == 5 || z == 5 {
                for y in base..base + 4 {
                    cells.push(([x, y, z], 3));
                }
            }
        }
    }
    for x in 6..=10 {
        for z in 7..=10 {
            cells.push((
                [x, base, z],
                if fluid == 2 {
                    terra_world::roads::PATH
                } else {
                    0
                },
            ));
        }
    }
    // Real glass boundaries and a solid step; no proxy fluid geometry.
    for x in 5..=12 {
        for y in base..=base + 2 {
            cells.push(([x, y, 12], MaterialId::Glass as u16));
        }
    }
    for z in 5..=12 {
        for y in base..=base + 2 {
            cells.push(([12, y, z], MaterialId::Glass as u16));
        }
    }
    cells.push(([6, base, 7], MaterialId::Sandstone as u16));
    if cells
        .iter()
        .any(|(p, _)| world.chunks.block_at(at(world, *p)).is_none())
    {
        return false;
    }
    // The fixture sends discrete authoritative targets. Only the renderer advances
    // between screenshots; source recession and authority are tested separately.
    *world.access = WorldAccess::Replica { ready: true };
    for x in [7, 8] {
        let p = at(world, [x, base, 8]);
        world
            .journal
            .set_fluid(p, terra_world::water::FluidCell::default())
            .unwrap();
    }
    for (p, material) in cells {
        block(world, p, material);
    }
    cell(
        world,
        fluid,
        [7, base, 8],
        if matches!(fluid, 0 | 3) { 8 } else { 64 },
        true,
    );
    cell(world, fluid, [8, base, 8], 1, false);
    true
}

fn units(world: &View, fluid: usize, p: [i64; 3]) -> u8 {
    let p = at(world, p);
    if matches!(fluid, 0 | 3) {
        world.journal.fluid(p).map_or(0, |v| v.amount)
    } else {
        world
            .journal
            .object_state(p)
            .and_then(|v| v.liquid)
            .map_or(0, |v| v.volume)
    }
}

fn roi(world: &View, base: i64) -> Option<[u32; 4]> {
    let (_, camera, transform) = world.cameras.iter().next()?;
    let logical = camera.logical_viewport_size()?;
    let size = camera.physical_viewport_size()?.as_vec2();
    let scale = size / logical;
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for x in [8.0, 9.0] {
        for y in [0.0, 1.05] {
            for z in [8.0, 9.0] {
                let p = camera
                    .world_to_viewport(
                        transform,
                        Vec3::new(
                            x + world.origin.0[0] as f32,
                            base as f32 + y,
                            z + world.origin.0[1] as f32,
                        ),
                    )
                    .ok()?
                    * scale;
                min = min.min(p);
                max = max.max(p);
            }
        }
    }
    Some([
        (min.x - 5.).max(0.) as u32,
        (min.y - 5.).max(0.) as u32,
        (max.x + 5.).min(size.x) as u32,
        (max.y + 5.).min(size.y) as u32,
    ])
}

fn run(
    mut commands: Commands,
    mut state: ResMut<Inspection>,
    mut world: View,
    mut exit: MessageWriter<AppExit>,
    temporal: Option<ResMut<stability::Sequence>>,
) {
    if state.fluid >= 4 {
        return;
    }
    if state.started.elapsed() > Duration::from_secs(150) || world.render.error.is_some() {
        error!(fluid=state.fluid,shot=state.shot,seeded=state.seeded,ready=world.render.ready,pending_jobs=world.render.pending_jobs,base=state.base,error=?world.render.error,"Temporal fluid inspection failed");
        state.fluid = 4;
        exit.write(AppExit::error());
        return;
    }
    world.origin.0 = if temporal.is_some() {
        stability::ORIGIN
    } else if state.fluid == 3 {
        state.blood_origin
    } else {
        [0, 0]
    };
    let p = pose(state.base, world.origin.0);
    for (mut camera, _, _) in &mut world.cameras {
        *camera = p;
    }
    world.focus.0 = p.translation.to_array().map(f64::from);
    if state.last_report.elapsed() >= Duration::from_secs(10) {
        info!(fluid=state.fluid,shot=state.shot,seeded=state.seeded,ready=world.render.ready,pending_jobs=world.render.pending_jobs,water=world.water.ready,publication=world.flow.publications,base=state.base,cell=?world.chunks.block_at([8,state.base,8]),"Temporal fluid progress");
        state.last_report = Instant::now();
    }
    if state.pending || !world.render.ready {
        return;
    }
    if !state.seeded {
        if scene(&mut world, state.base, state.fluid) {
            if temporal.is_some() {
                stability::fill(&mut world, state.base);
            }
            state.seeded = true;
            state.since = Instant::now();
        }
        return;
    }
    if let Some(mut sequence) = temporal {
        if world.water.ready && state.since.elapsed() >= Duration::from_secs(3) {
            stability::capture(&mut commands, &mut sequence, &world, state.base);
        }
        return;
    }
    let big = [7, 56, 48, 7][state.fluid];
    if matches!(state.shot, 1 | 3) && !state.edited {
        state.publication = world.flow.publications;
        if state.shot == 1 {
            cell(&mut world, state.fluid, [8, state.base, 8], big, false);
        } else {
            // Keep the neighboring source visible: this checks partial cell
            // removal within a still-wet section, not only whole-mesh removal.
            cell(&mut world, state.fluid, [8, state.base, 8], 0, false);
        }
        state.edited = true;
        state.since = Instant::now();
        return;
    }
    if matches!(state.fluid, 0 | 3) && !world.water.ready {
        return;
    }
    let fraction =
        (world.time.elapsed_secs() - world.flow.last_started) / world.flow.last_duration.max(0.001);
    match state.shot {
        0 if state.since.elapsed() < Duration::from_millis(1400) => return,
        1 | 3 => {
            if world.flow.publications <= state.publication || fraction < 0.38 {
                return;
            }
            if fraction > 0.90 {
                error!(
                    "Missed intermediate GPU tween: fluid={} shot={} fraction={fraction}",
                    state.fluid, state.shot
                );
                state.fluid = 4;
                exit.write(AppExit::error());
                return;
            }
        }
        2 | 4 if fraction < 1.15 => return,
        _ => {}
    }
    let expected = match state.shot {
        0 => 1,
        1 | 2 => big,
        _ => 0,
    };
    assert_eq!(
        units(&world, state.fluid, [8, state.base, 8]),
        expected,
        "GPU interpolation must not rewrite authoritative targets"
    );
    let Some(roi) = roi(&world, state.base) else {
        return;
    };
    let name = ["water", "galle", "tar", "blood"][state.fluid];
    let dir = PathBuf::from(std::env::var_os("TERRA_CAPTURE_DIR").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let stem = format!("flow-{name}-{}", state.shot);
    std::fs::write(dir.join(format!("{stem}.json")),serde_json::to_vec_pretty(&serde_json::json!({
        "fluid":name,"stage":state.shot,"authoritative_units":expected,"tween_fraction":fraction,
        "publication":world.flow.publications,"duration":world.flow.last_duration,"roi":roi,"roi_units":"physical_pixels",
        "replica":!world.access.authoritative(),"simulation_seconds":world.time.elapsed_secs(),
        "wall_seconds":state.started.elapsed().as_secs_f64(),"origin":world.origin.0,
        "source_units":units(&world,state.fluid,[7,state.base,8]),
        "scope":"Discrete authoritative targets in an isolated replica; actual mesher, material and GPU interpolation. Source simulation is tested separately.",
    })).unwrap()).unwrap();
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(dir.join(format!("{stem}.png"))))
        .observe(|_:On<ScreenshotCaptured>,mut state:ResMut<Inspection>,mut exit:MessageWriter<AppExit>| {
            state.shot+=1;state.pending=false;state.edited=false;
            if state.shot==5 {state.fluid+=1;state.shot=0;state.seeded=false;}
            if state.fluid==4 {info!("Temporal fluid gallery passed: continuous fill and drain for Water, Galle, Tar, Blood");exit.write(AppExit::Success);}
        });
    state.pending = true;
}
