//! Opt-in real-GPU views of natural caves; the inspection light adds no game content.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_world::{
    TerrainGenerator,
    generation::CaveKind,
    streaming::{ChunkFocus, GenerationStatus, WorldGeneration, WorldTerrain},
};

struct View {
    name: &'static str,
    camera: Transform,
    light: Vec3,
    intensity: f32,
}
#[derive(Resource)]
struct Inspection {
    started: Instant,
    entered: Instant,
    views: Vec<View>,
    stage: usize,
    captures: usize,
    requested: bool,
}
#[derive(Component)]
struct CaveLight;

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-geology") {
        return;
    }
    app.insert_resource(terra_player::ControllerMode::Fly)
        .add_systems(PostStartup, setup)
        .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}
fn setup(world: &mut World) {
    if std::env::var("TERRA_CAVE_MSAA").is_ok_and(|mode| mode == "off") {
        let cameras: Vec<_> = world
            .query_filtered::<Entity, With<terra_player::TerraCamera>>()
            .iter(world)
            .collect();
        for entity in cameras {
            world.entity_mut(entity).insert(Msaa::Off);
        }
    }
    let terrain = world.resource::<WorldTerrain>().0.clone();
    let mut cheese = None;
    let mut dripstone = None;
    let mut spaghetti = None;
    for z in -3..=3 {
        for x in -3..=3 {
            let px = f64::from(x) * 192.0 + 96.0;
            let pz = f64::from(z) * 192.0 + 96.0;
            match terrain.cave_kind_at(px, pz) {
                CaveKind::Cheese => {
                    cheese = cheese.or_else(|| terrain.cave_chamber_center(px, pz));
                }
                CaveKind::Dripstone => {
                    dripstone = dripstone.or_else(|| terrain.cave_chamber_center(px, pz));
                }
                CaveKind::Spaghetti => {
                    spaghetti.get_or_insert([px, pz]);
                }
            }
        }
    }
    let salt = (-96..=96).find_map(|z| {
        (-96..=96).find_map(|x| {
            terrain.salt_chamber_center(f64::from(x) * 192.0 + 96.0, f64::from(z) * 192.0 + 96.0)
        })
    });
    let (Some(cheese), Some(dripstone), Some(salt), Some(spaghetti)) =
        (cheese, dripstone, salt, spaghetti)
    else {
        error!("Natural geology fixtures missing");
        world.write_message(AppExit::error());
        return;
    };
    let chamber = |name, center: [f64; 3]| {
        let center = Vec3::from_array(center.map(|v| v as f32));
        let eye = center + Vec3::new(-14.0, 3.0, 18.0);
        View {
            name,
            camera: Transform::from_translation(eye)
                .looking_at(center + Vec3::new(22.0, -10.0, -18.0), Vec3::Y),
            light: center + Vec3::Y * 5.0,
            intensity: 8_000_000.0,
        }
    };
    let tunnel = (-80..=0).step_by(4).find_map(|y| {
        (-40..=40).step_by(4).find_map(|dz| {
            (-40..=40).step_by(4).find_map(|dx| {
                let p = [
                    spaghetti[0] + f64::from(dx) + 0.5,
                    f64::from(y) + 0.5,
                    spaghetti[1] + f64::from(dz) + 0.5,
                ];
                (terrain.sample(p).density > 1.2).then_some(p)
            })
        })
    });
    let Some(tunnel) = tunnel else {
        error!("Natural spaghetti tunnel missing");
        world.write_message(AppExit::error());
        return;
    };
    let eye = Vec3::from_array(tunnel.map(|v| v as f32));
    let direction = [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Z,
        Vec3::NEG_Z,
        Vec3::new(1.0, 0.0, 1.0).normalize(),
        Vec3::new(-1.0, 0.0, 1.0).normalize(),
    ]
    .into_iter()
    .max_by(|a, b| {
        let density = |d: Vec3| {
            terrain
                .sample((eye + d * 4.0).to_array().map(f64::from))
                .density
        };
        density(*a).total_cmp(&density(*b))
    })
    .unwrap_or(Vec3::Z);
    let surface = Vec3::new(
        salt[0] as f32,
        terrain.surface_height(salt[0], salt[2]) as f32,
        salt[2] as f32,
    );
    let views = vec![
        chamber("cheese", cheese),
        chamber("dripstone", dripstone),
        View {
            name: "spaghetti",
            camera: Transform::from_translation(eye).looking_at(eye + direction * 6.0, Vec3::Y),
            light: eye,
            intensity: 80_000.0,
        },
        chamber("salt", salt),
        View {
            name: "salt-ceiling",
            camera: Transform::from_xyz(
                salt[0] as f32 - 8.0,
                salt[1] as f32 + 5.0,
                salt[2] as f32 + 12.0,
            )
            .looking_at(
                Vec3::new(
                    salt[0] as f32 + 16.0,
                    salt[1] as f32 + 25.0,
                    salt[2] as f32 - 14.0,
                ),
                Vec3::Y,
            ),
            light: Vec3::from_array(salt.map(|v| v as f32)) + Vec3::Y * 12.0,
            intensity: 8_000_000.0,
        },
        View {
            name: "cliffted-desert",
            camera: Transform::from_translation(surface + Vec3::new(-55.0, 38.0, 70.0))
                .looking_at(surface, Vec3::Y),
            light: surface,
            intensity: 0.0,
        },
    ];
    for view in &views {
        info!(scene=view.name,eye=?view.camera.translation,"Natural geology inspection location");
    }
    world.spawn((
        CaveLight,
        PointLight {
            range: 160.0,
            color: Color::srgb(1.0, 0.96, 0.90),
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::default(),
    ));
    let now = Instant::now();
    world.insert_resource(Inspection {
        started: now,
        entered: now,
        views,
        stage: 0,
        captures: 0,
        requested: false,
    });
}
fn run(world: &mut World) {
    if !world.contains_resource::<Inspection>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<Inspection>| {
        if state.stage >= state.views.len() {
            return;
        }
        if state.started.elapsed() > Duration::from_secs(300)
            || world.resource::<terra_render::RenderStatus>().error.is_some()
        {
            error!(stage = state.stage, "Natural geology inspection failed");
            state.stage = state.views.len();
            world.write_message(AppExit::error());
            return;
        }
        let view = &state.views[state.stage];
        for mut camera in world
            .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
            .iter_mut(world)
        {
            *camera = view.camera;
        }
        world.resource_mut::<ChunkFocus>().0 = view.camera.translation.to_array().map(f64::from);
        for (mut transform, mut light) in world
            .query_filtered::<(&mut Transform, &mut PointLight), With<CaveLight>>()
            .iter_mut(world)
        {
            transform.translation = view.light;
            light.intensity = if std::env::var_os("TERRA_CAVE_NATURAL").is_some() { 0.0 } else { view.intensity };
        }
        let ready = world.resource::<WorldGeneration>().status == GenerationStatus::Ready
            && world.resource::<terra_render::RenderStatus>().ready
            && world.resource::<terra_render::PropRenderStatus>().ready;
        if !ready || state.entered.elapsed() < Duration::from_secs(2) {
            return;
        }
        if !state.requested {
            if std::env::var_os("TERRA_CAVE_NATURAL").is_some() && state.stage < 5 {
                let blend = world.resource::<terra_render::CaveState>().blend;
                if blend < 0.75 { return; }
                info!(scene = view.name, blend, "Natural cave shader active without inspection light");
            }
            let directory = std::env::var_os("TERRA_CAPTURE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/tmp/terra-geology"));
            if let Err(error) = std::fs::create_dir_all(&directory) {
                error!(%error, "Cannot create capture directory");
                world.write_message(AppExit::error());
                state.stage = state.views.len();
                return;
            }
            world
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(directory.join(format!("geology-{}.png", view.name))))
                .observe(|_: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| state.captures += 1);
            state.requested = true;
        } else if state.captures > state.stage {
            state.stage += 1;
            state.requested = false;
            state.entered = Instant::now();
            if state.stage == state.views.len() {
                info!("Native geology inspection passed: three natural cave types, salt chamber and adjacent desert terrain");
                world.write_message(AppExit::Success);
            }
        }
    });
}
