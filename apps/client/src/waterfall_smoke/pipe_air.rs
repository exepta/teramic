//! Submerged pipe views beside a shore, including legacy dry overrides.
use super::*;
use terra_core::plumbing;
use terra_world::{edits::BlockObjectState, streaming::commit_object_state, water::FluidCell};

#[derive(Resource)]
struct Inspection {
    start: Instant,
    since: Instant,
    stage: usize,
    built: bool,
    pending: bool,
}
pub(super) fn install(app: &mut App) {
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Inspection {
            start: Instant::now(),
            since: Instant::now(),
            stage: 0,
            built: false,
            pending: false,
        })
        .add_systems(
            Update,
            run.after(terra_player::PlayerSet::Movement)
                .after(terra_world::streaming::StreamingSet::Dispatch),
        );
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Inspection>| {
        if state.stage == usize::MAX {
            return;
        }
        let result = (|| -> Result<bool, String> {
            if state.start.elapsed() > Duration::from_secs(75) {
                return Err("Submerged shore pipe timeout".into());
            }
            let x = 8.0 + state.stage.min(4) as f32 * 5.0;
            let y = if state.stage == 3 { 20.0 } else { 18.0 };
            let target = Vec3::new(x + 0.8, y + 0.5, 8.8);
            let view = Transform::from_translation(target + Vec3::new(2.2, -0.1, 3.0))
                .looking_at(target, Vec3::Y);
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                *camera = view;
            }
            for mut body in world
                .query_filtered::<&mut Transform, With<PlayerBody>>()
                .iter_mut(world)
            {
                body.translation = view.translation;
            }
            world.resource_mut::<ChunkFocus>().0 = [18.0, 20.0, 10.0];
            if !state.built {
                if ![8, 30].into_iter().all(|x| {
                    world
                        .resource::<ChunkStore>()
                        .block_at([x, 20, 8])
                        .is_some()
                }) || !world.resource::<RenderStatus>().ready
                {
                    return Ok(false);
                }
                *world.resource_mut::<WorldClock>() = WorldClock {
                    phase: 0.5,
                    paused: true,
                    ..Default::default()
                };
                for i in 0..5 {
                    let x = 8 + i * 5;
                    for a in -1..=2 {
                        for y in 17..=20 {
                            edit(world, [x + a, y, 7], 2)?;
                            edit(world, [x - 1, y, 8 + a], 2)?;
                        }
                    }
                    if i == 4 {
                        for a in 0..=2 {
                            for z in 8..=10 {
                                for y in 17..=20 {
                                    edit(world, [x + a, y, z], 14)?;
                                }
                            }
                        }
                    }
                    let y = if i == 3 { 20 } else { 18 };
                    let material = [
                        plumbing::WOOD_PIPE,
                        plumbing::STONE_PIPE,
                        plumbing::GLASS_PIPE,
                        plumbing::GLASS_PIPE,
                        plumbing::GLASS_PIPE,
                    ][i as usize];
                    for p in [[x, y, 8], [x + 1, y, 8], [x + 1, y, 9]] {
                        edit(world, p, material)?;
                        world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
                            commit_object_state(
                                &mut chunks,
                                &mut world.resource_mut::<EditJournal>(),
                                p,
                                Some(BlockObjectState {
                                    facing: 4,
                                    ..Default::default()
                                }),
                            )
                        })?;
                        if i == 2 {
                            world
                                .resource_mut::<EditJournal>()
                                .set_fluid(p, FluidCell::default())
                                .map_err(|e| e.to_string())?;
                        }
                    }
                }
                state.built = true;
                state.since = Instant::now();
                return Ok(false);
            }
            if state.stage == 5 {
                return Ok(captures_finished(world));
            }
            if state.pending || state.since.elapsed() < Duration::from_secs(3) {
                return Ok(false);
            }
            let p = [x as i64, y as i64, 8];
            let chunks = world.resource::<ChunkStore>();
            let journal = world.resource::<EditJournal>();
            if chunks.loaded.iter().any(|(owner, s)| {
                s.mesh_epoch != journal.mesh_epoch(*owner)
                    || s.water_epoch != journal.water_mesh_epoch(*owner)
            }) {
                return Ok(false);
            }
            let cell = chunks.fluid_cell_at(
                &world.resource::<WorldTerrain>().0,
                journal,
                p,
                chunks.block_at(p).unwrap_or(0),
            );
            info!(
                stage = state.stage,
                ?p,
                ?cell,
                "Submerged shore pipe occupancy"
            );
            let folder = std::env::var_os("TERRA_CAPTURE_DIR")
                .map(PathBuf::from)
                .ok_or("Set TERRA_CAPTURE_DIR")?;
            std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            let name = [
                "natural-wood",
                "natural-stone",
                "legacy-glass",
                "surface-glass",
                "solid-water-glass",
            ][state.stage];
            world
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(folder.join(format!("{name}.png"))))
                .observe(|_: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                    state.stage += 1;
                    state.pending = false;
                    state.since = Instant::now();
                });
            state.pending = true;
            Ok(false)
        })();
        match result {
            Ok(true) => {
                info!("Submerged shore pipe capture passed");
                world.write_message(AppExit::Success);
                state.stage = usize::MAX;
            }
            Ok(false) => {}
            Err(error) => {
                error!(%error,"Submerged shore pipe capture failed");
                world.write_message(AppExit::error());
                state.stage = usize::MAX;
            }
        }
    });
}
