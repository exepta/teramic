//! Short local render/HUD fixture, not a network or creature-AI encounter.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{
    MobHealthBars,
    creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds},
};
use terra_network::client::{ClientSession, CreatureTrack, ReplicatedCreatures};

#[derive(Resource)]
struct Inspection {
    start: Instant,
    since: Instant,
    captured: usize,
    pending: bool,
    wall: bool,
    failed: bool,
}

#[derive(Clone, Copy)]
struct FillRoi {
    bounds: Rect,
    hostile: bool,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-mob-health") {
        return;
    }
    app.init_resource::<ClientSession>()
        .init_resource::<ReplicatedCreatures>()
        .insert_resource(terra_player::ControllerMode::Fly)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Inspection {
            start: Instant::now(),
            since: Instant::now(),
            captured: 0,
            pending: false,
            wall: false,
            failed: false,
        })
        .add_systems(PostStartup, setup)
        // This local gallery has no server to acknowledge MovementInputs. Keep
        // the ready HUD session without running the network prediction motor.
        .add_systems(FixedFirst, |mut session: ResMut<ClientSession>| {
            session.teleporting = true;
        })
        .add_systems(FixedLast, |mut session: ResMut<ClientSession>| {
            session.teleporting = false;
        })
        .add_systems(
            Update,
            pin.after(terra_player::PlayerSet::Movement)
                .before(terra_player::PlayerSet::Targeting),
        )
        .add_systems(Last, run);
}

fn setup(world: &mut World) {
    let mut menu = world.resource_mut::<terra_ui::menu::MenuState>();
    menu.page = terra_ui::menu::MenuPage::Playing;
    menu.session_active = true;
    world.resource_mut::<ClientSession>().ready = true;
    world.resource_mut::<ClientSession>().allow_flight = true;
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
    world
        .resource_mut::<terra_core::GameplayConfig>()
        .minimap
        .enabled = false;
    for (id, kind, x) in [
        (1, CreatureKind::Cow, 8.0),
        (2, CreatureKind::Stalker, 10.5),
    ] {
        let creature = Creature {
            id,
            kind,
            home: [0, 1],
            position: [x, 32.05, 18.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: kind.health() * 0.5,
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        };
        world
            .resource_mut::<ReplicatedCreatures>()
            .creatures
            .insert(
                id,
                CreatureTrack {
                    previous: creature.clone(),
                    current: creature,
                    interval: 0.1,
                    received: Instant::now(),
                    playback: Default::default(),
                },
            );
    }
    let mesh = world
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(12.0, 0.2, 12.0));
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.27, 0.24),
            perceptual_roughness: 0.9,
            ..default()
        });
    world.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_xyz(8.0, 31.95, 18.0),
    ));
    world.spawn((
        PointLight {
            intensity: 1600.0,
            range: 15.0,
            ..default()
        },
        Transform::from_xyz(6.0, 38.0, 12.0),
    ));
}

fn pin(world: &mut World) {
    let stage = world.resource::<Inspection>().captured;
    if stage >= 5 {
        let mut replica = world.resource_mut::<ReplicatedCreatures>();
        let a = &mut replica.creatures.get_mut(&2).unwrap().current;
        a.kind = CreatureKind::DarkDragon;
        a.health = a.kind.health() * 0.5;
        replica.boss = (stage == 5).then_some(2);
    }
    let bounds = stage == 1;
    if world
        .resource::<terra_core::DebugSettings>()
        .creature_bounds
        != bounds
    {
        world
            .resource_mut::<terra_core::DebugSettings>()
            .creature_bounds = bounds;
    }
    let mode = match stage {
        0 => MobHealthBars::LookingAt,
        1 | 2 => MobHealthBars::InView,
        3 => MobHealthBars::Always,
        _ => MobHealthBars::None,
    };
    if world
        .resource::<terra_core::GameplayConfig>()
        .mob_health_bars
        != mode
    {
        world
            .resource_mut::<terra_core::GameplayConfig>()
            .mob_health_bars = mode;
    }
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        *camera = if stage >= 5 {
            Transform::from_xyz(-8.0, 39.0, -4.0).looking_at(Vec3::new(8.0, 36.2, 18.0), Vec3::Y)
        } else {
            Transform::from_xyz(8.0, 33.0, 11.0).looking_at(Vec3::new(8.0, 33.0, 18.0), Vec3::Y)
        };
    }
    world.resource_mut::<terra_world::streaming::ChunkFocus>().0 = [8.0, 33.0, 18.0];
    if stage >= 2 && !world.resource::<Inspection>().wall {
        if let Err(error) = build_wall(world) {
            error!(%error, "Native mob health collision wall commit failed");
            world.resource_mut::<Inspection>().failed = true;
            return;
        }
        world.resource_mut::<Inspection>().wall = true;
    }
}

fn build_wall(world: &mut World) -> Result<(), String> {
    use terra_world::streaming::{BlockEditRequest, ChunkStore, commit_block_edit};
    let properties = world
        .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .clone();
    world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
        let mut journal = world.resource_mut::<terra_world::edits::EditJournal>();
        for x in 9..=12 {
            for y in 32..=35 {
                let position = [x, y, 15];
                let expected_material = chunks
                    .block_at(position)
                    .ok_or_else(|| format!("unloaded wall at {position:?}"))?;
                if expected_material != 3
                    && !commit_block_edit(
                        &mut chunks,
                        &mut journal,
                        &BlockEditRequest {
                            position,
                            expected_material,
                            material: 3,
                        },
                        &properties,
                    )?
                {
                    return Err(format!("wall commit rejected at {position:?}"));
                }
                if chunks.block_at(position) != Some(3) || journal.block(position) != Some(3) {
                    return Err(format!("wall journal/loaded data disagree at {position:?}"));
                }
            }
        }
        Ok(())
    })
}

fn run(world: &mut World) {
    let inspection = world.resource::<Inspection>();
    let stage = inspection.captured;
    if inspection.failed || inspection.start.elapsed() > Duration::from_secs(60) {
        let detail = diagnostics(world);
        error!(stage, %detail, "Native mob health HUD fixture failed/timed out");
        world.write_message(AppExit::error());
        return;
    }
    if stage >= 7 {
        if inspection.since.elapsed() > Duration::from_millis(600) {
            info!(
                "Native mob health HUD passed: actual projected friendly/hostile bars, aim, wall occlusion, Always and None; boss encounter half-health and disengagement; seven nonblank captures"
            );
            world.write_message(AppExit::Success);
        }
        return;
    }
    if inspection.pending
        || inspection.start.elapsed() < Duration::from_secs(7)
        || inspection.since.elapsed() < Duration::from_secs(2)
        || !world.resource::<terra_render::RenderStatus>().ready
    {
        return;
    }
    let expected = match stage {
        0 | 2 => 1,
        1 | 3 => 2,
        _ => 0,
    };
    let hud = world.resource::<terra_ui::MobHealthHud>();
    if hud.bars.len() != expected || expected == 1 && hud.bars[0].id != "1" {
        return;
    }
    if expected > 0
        && !hud
            .bars
            .iter()
            .any(|b| b.id == "1" && b.fill.contains("#69cb86"))
        || expected == 2
            && !hud
                .bars
                .iter()
                .any(|b| b.id == "2" && b.fill.contains("#df625d"))
    {
        error!(stage, "Wrong friendly/hostile health-bar colors");
        world.write_message(AppExit::error());
        return;
    }
    if hud.boss.is_empty() == (stage == 5) {
        return;
    }
    let nodes: Vec<_> = world
        .query::<(
            &tilt_ui::ElementClasses,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(world)
        .filter(|(c, _, _)| c.classes.iter().any(|c| c == "mob-health"))
        .map(|(_, n, t)| (n.size(), t.translation))
        .collect();
    if nodes.len() != expected
        || nodes
            .iter()
            .any(|(size, position)| size.x < 100.0 || size.y < 20.0 || !position.is_finite())
    {
        return;
    }
    let mut fills: Vec<_> = world
        .query::<(
            &tilt_ui::ElementClasses,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
            &BackgroundColor,
        )>()
        .iter(world)
        .filter(|(c, _, _, _)| c.classes.iter().any(|c| c == "mob-health-fill"))
        .map(|(_, node, transform, background)| FillRoi {
            bounds: Rect::from_center_size(transform.translation, node.size()),
            hostile: background.0.to_srgba().red > background.0.to_srgba().green,
        })
        .collect();
    if fills.len() != expected
        || fills.iter().filter(|f| f.hostile).count() != usize::from(expected == 2)
        || fills
            .iter()
            .any(|f| f.bounds.size().x < 30.0 || f.bounds.size().y < 8.0)
    {
        return;
    }
    let top_bars: Vec<_> = world
        .query::<(
            &tilt_ui::ElementClasses,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(world)
        .filter(|(c, _, _)| c.classes.iter().any(|c| c == "dragon-boss-health"))
        .map(|(_, n, t)| Rect::from_center_size(t.translation, n.size()))
        .collect();
    if stage == 5 {
        let [bounds] = top_bars.as_slice() else {
            return;
        };
        if bounds.size().x < 40.0 || bounds.size().y < 8.0 {
            return;
        }
        fills.push(FillRoi {
            bounds: *bounds,
            hostile: true,
        });
    } else if !top_bars.is_empty() {
        return;
    }
    if world
        .query::<&terra_render::creatures::CreatureAvatar>()
        .iter(world)
        .count()
        != 2
    {
        return;
    }
    let directory = PathBuf::from(
        std::env::var("TERRA_CAPTURE_DIR").unwrap_or_else(|_| "/tmp/terra-mob-health".into()),
    );
    if std::fs::create_dir_all(&directory).is_err() {
        world.write_message(AppExit::error());
        return;
    }
    let name = [
        "looking-at",
        "in-view",
        "occluded",
        "always",
        "none",
        "boss-half",
        "boss-ended",
    ][stage];
    info!(
        stage,
        bars = expected,
        "Native mob health HUD data and actual UI geometry verified"
    );
    world.resource_mut::<Inspection>().pending = true;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(
            directory.join(format!("mob-health-{name}.png")),
        ))
        .observe(
            move |capture: On<ScreenshotCaptured>, mut inspection: ResMut<Inspection>| {
                let nonblank = capture.image.data.as_ref().is_some_and(|data| {
                    data.chunks_exact(4)
                        .step_by(16)
                        .filter(|p| p[..3].iter().any(|v| *v > 16))
                        .count()
                        > data.len() / 4 / 16 / 100
                });
                let pixels = validate_fill_pixels(&capture.image, &fills, stage);
                if let Err(error) = &pixels {
                    error!(stage, %error, "Native mob health bar pixels missing");
                }
                inspection.failed = !nonblank || pixels.is_err();
                inspection.pending = false;
                inspection.captured += 1;
                inspection.since = Instant::now();
            },
        );
}

fn validate_fill_pixels(image: &Image, fills: &[FillRoi], stage: usize) -> Result<(), String> {
    let image = image
        .clone()
        .try_into_dynamic()
        .map_err(|e| e.to_string())?
        .into_rgba8();
    for fill in fills {
        let min = fill.bounds.min.max(Vec2::ZERO).floor().as_uvec2();
        let max = fill
            .bounds
            .max
            .min(Vec2::new(image.width() as f32, image.height() as f32))
            .ceil()
            .as_uvec2();
        let mut matching = 0;
        let mut sampled = 0;
        for y in min.y..max.y {
            for x in min.x..max.x {
                let [r, g, b, _] = image.get_pixel(x, y).0.map(f32::from);
                matching += usize::from(if fill.hostile {
                    r > 90.0 && r > g * 1.3 && r > b * 1.3
                } else {
                    g > 70.0 && g > r * 1.15 && g > b * 1.15
                });
                sampled += 1;
            }
        }
        info!(stage, hostile=fill.hostile, matching, sampled, bounds=?fill.bounds, "Native mob health framebuffer ROI pixels");
        if matching < 30 || matching * 5 < sampled {
            return Err(format!(
                "expected {} bar pixels in {:?}, found {matching}/{sampled}",
                if fill.hostile { "red" } else { "green" },
                fill.bounds
            ));
        }
    }
    Ok(())
}

fn diagnostics(world: &mut World) -> String {
    let session = world.resource::<ClientSession>();
    let state = format!(
        "ready={}, teleporting={}, error={:?}, pending_inputs={}, menu={:?}, mode={:?}, hud={:?}, render_ready={}, render_error={:?}, loaded={}",
        session.ready,
        session.teleporting,
        session.error,
        session.pending_inputs,
        world.resource::<terra_ui::menu::MenuState>().page,
        world
            .resource::<terra_core::GameplayConfig>()
            .mob_health_bars,
        world.resource::<terra_ui::MobHealthHud>().bars,
        world.resource::<terra_render::RenderStatus>().ready,
        world.resource::<terra_render::RenderStatus>().error,
        world
            .resource::<terra_world::streaming::ChunkStore>()
            .loaded
            .len(),
    );
    let nodes: Vec<_> = world
        .query::<(&tilt_ui::ElementClasses, &ComputedNode)>()
        .iter(world)
        .filter(|(c, _)| c.classes.iter().any(|c| c == "mob-health"))
        .map(|(_, node)| node.size())
        .collect();
    let cameras: Vec<_> = world
        .query_filtered::<(&Camera, &Transform, &GlobalTransform), With<terra_player::TerraCamera>>(
        )
        .iter(world)
        .map(|(camera, local, global)| {
            (
                local.translation,
                global.translation(),
                Vec3::from(global.forward()),
                camera.logical_viewport_size(),
                camera.world_to_viewport(global, Vec3::new(8.0, 33.94, 18.0)),
            )
        })
        .collect();
    let avatars = world
        .query::<&terra_render::creatures::CreatureAvatar>()
        .iter(world)
        .count();
    format!("{state}, nodes={nodes:?}, cameras={cameras:?}, avatars={avatars}")
}
