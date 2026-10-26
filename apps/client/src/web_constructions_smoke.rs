//! Short offline inspection of real WorldProps through the normal section prop jobs.
use bevy::{
    camera::{Exposure, Hdr, primitives::Aabb},
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{GameConfig, WorldClock};
use terra_render::{PropRenderStatus, WindProp};
use terra_voxel::{BlockSection, SectionPos};
use terra_world::{
    edits::EditJournal,
    props::{PropId, WorldProp, webs},
    streaming::{ChunkFocus, ChunkStore, LoadedSection, SectionPublished, StreamingSettings},
};

#[derive(Component)]
struct InspectionCamera;

#[derive(Resource)]
struct Gallery {
    started: Instant,
    last: Instant,
    pending: bool,
    captures: usize,
    failure: Option<String>,
    finish_frames: u8,
    props: [WorldProp; 3],
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-web-constructions") {
        return;
    }
    app.world_mut()
        .resource_mut::<GameConfig>()
        .client
        .vegetation_density = 1.0;
    let props = std::array::from_fn(|index| {
        webs::spanning(
            [index as i64 * 40, 43, 24],
            [1, 0, 0],
            [8, 13, 21][index],
            10 + index as u8,
            PropId(0x7765_6267_616c_0000 + index as u64),
        )
        .expect("bounded construction gallery")
    });
    app.insert_resource(terra_player::ControllerMode::Fly)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Gallery {
            started: Instant::now(),
            last: Instant::now(),
            pending: false,
            captures: 0,
            failure: None,
            finish_frames: 0,
            props,
        })
        .add_systems(PostStartup, setup)
        .add_systems(
            Update,
            pin.after(terra_player::PlayerSet::Movement)
                .before(terra_player::PlayerSet::Targeting),
        )
        .add_systems(Last, run);
}

fn setup(world: &mut World) {
    detach_gameplay_environment(world);
    {
        let mut settings = world.resource_mut::<StreamingSettings>();
        settings.radius = 8;
        settings.vertical_radius = 0;
        settings.max_jobs = 0;
        settings.meshes = false;
    }
    let clock = WorldClock {
        phase: 0.5,
        paused: true,
        ..Default::default()
    };
    *world.resource_mut::<WorldClock>() = clock;
    world.resource_mut::<EditJournal>().clock = Some(clock);
    world
        .resource_mut::<terra_core::WorldWeather>()
        .set_pattern(terra_core::WeatherPattern::Clear);
    let props = world.resource::<Gallery>().props;
    world.resource_mut::<ChunkFocus>().0 = [48.0, 40.0, 24.0];
    for x in [0, 2, 3, 5, 6] {
        let position = SectionPos::new(x, 2, 1);
        let mut data = BlockSection::empty(position).expect("bounded gallery section");
        for local_x in 0..16 {
            for z in 0..16 {
                data.set_block([local_x, 3, z], 3).unwrap();
                data.set_block([local_x, 15, z], 3).unwrap();
                if props
                    .iter()
                    .any(|prop| webs::support(prop)[0] == x * 16 + local_x as i64)
                {
                    for y in 4..15 {
                        data.set_block([local_x, y, z], 3).unwrap();
                    }
                }
            }
        }
        for prop in props {
            for anchor in webs::anchors(&prop).unwrap() {
                let (owner, local) = terra_voxel::global_to_section(anchor);
                if owner == position {
                    data.set_block(local, 3).unwrap();
                }
            }
        }
        world.resource_mut::<ChunkStore>().loaded.insert(
            position,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: props
                    .iter()
                    .copied()
                    .filter(|prop| prop.section == position)
                    .collect(),
            },
        );
        world.write_message(SectionPublished { position });
    }
    for prop in props {
        let fits = {
            let chunks = world.resource::<ChunkStore>();
            webs::fits(&prop, |p| chunks.block_at(p))
        };
        assert!(
            fits,
            "native fixture must use real clear strands and loaded rock anchors"
        );
        world
            .resource_mut::<EditJournal>()
            .place_prop(prop)
            .unwrap();
    }
    // Only the cave shell is fixture geometry. Every visible strand is composed
    // by the production WorldProp jobs and their ordinary shared material.
    let cube = world
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(1.0, 1.0, 1.0));
    let stone = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.11, 0.12),
            perceptual_roughness: 0.95,
            reflectance: 0.03,
            ..default()
        });
    for prop in props {
        let x = webs::support(&prop)[0] as f32;
        let span = prop.scale * webs::TEMPLATE_SPAN;
        let end = x + span + 1.024;
        for (center, size) in [
            ([x + (span + 2.0) * 0.5, 35.5, 24.5], [span + 2.0, 1.0, 9.0]),
            ([x + (span + 2.0) * 0.5, 47.5, 24.5], [span + 2.0, 1.0, 9.0]),
            ([x + 0.5, 41.5, 24.5], [1.0, 11.0, 9.0]),
            ([end + 0.5, 41.5, 24.5], [1.0, 11.0, 9.0]),
            (
                [x + (span + 2.0) * 0.5, 41.5, 28.5],
                [span + 2.0, 11.0, 1.0],
            ),
        ] {
            world.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(stone.clone()),
                Transform::from_translation(Vec3::from_array(center))
                    .with_scale(Vec3::from_array(size)),
            ));
        }
        world.spawn((
            PointLight {
                intensity: 85_000.0,
                range: 28.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(x + 1.0 + span * 0.5, 44.8, prop.position[2] as f32 - 3.0),
        ));
    }
    // The actual production materials remain unchanged. This camera is not a
    // TerraCamera, so world cave/daylight exposure cannot overwrite the study.
    world.spawn((
        InspectionCamera,
        Camera3d::default(),
        Hdr,
        Exposure { ev100: 6.0 },
        bevy::core_pipeline::tonemapping::Tonemapping::AcesFitted,
        AmbientLight {
            color: Color::WHITE,
            brightness: 120.0,
            ..default()
        },
        Transform::default(),
    ));
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
}

fn detach_gameplay_environment(world: &mut World) {
    let cameras: Vec<_> = world
        .query_filtered::<Entity, With<terra_player::TerraCamera>>()
        .iter(world)
        .collect();
    for entity in cameras {
        let mut camera = world.entity_mut(entity);
        if let Some(mut view) = camera.get_mut::<Camera>() {
            view.is_active = false;
        }
        // This runs in PostStartup, before Bevy's Update probe preparation.
        // An inactive atmospheric camera otherwise retains a generated probe
        // while no active atmospheric view can provide its uniform buffer.
        camera.remove::<(
            bevy::light::AtmosphereEnvironmentMapLight,
            bevy::light::GeneratedEnvironmentMapLight,
            bevy::light::EnvironmentMapLight,
            bevy::pbr::AtmosphereSettings,
        )>();
    }
}

fn pin(world: &mut World) {
    let gallery = world.resource::<Gallery>();
    let stage = gallery.captures.min(5);
    let prop = gallery.props[stage / 2];
    let span = prop.scale * webs::TEMPLATE_SPAN;
    let target = Vec3::new(
        prop.position[0] as f32 + span * 0.5,
        43.5 - prop.scale,
        prop.position[2] as f32,
    );
    let side = if stage.is_multiple_of(2) {
        0.0
    } else {
        span * 0.30
    };
    let view = Transform::from_translation(target + Vec3::new(side, 1.0, -span * 0.8 - 3.0))
        .looking_at(target, Vec3::Y);
    world.resource_mut::<ChunkFocus>().0 = [48.0, 40.0, 24.0];
    for mut transform in world
        .query_filtered::<&mut Transform, With<InspectionCamera>>()
        .iter_mut(world)
    {
        *transform = view;
    }
    for mut camera in world
        .query_filtered::<&mut Camera, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        camera.is_active = false;
    }
}

fn run(world: &mut World) {
    for mut light in world.query::<&mut DirectionalLight>().iter_mut(world) {
        light.illuminance = 0.0;
    }
    world.resource_scope(|world, mut state: Mut<Gallery>| {
        if let Err(error) = advance(world, &mut state) {
            error!(%error, captures=state.captures, "Native spanning silk gallery failed");
            world.write_message(AppExit::error());
        }
    });
}

fn advance(world: &mut World, state: &mut Gallery) -> Result<(), String> {
    if let Some(error) = &state.failure {
        return Err(error.clone());
    }
    if state.started.elapsed() > Duration::from_secs(30) {
        return Err("gallery timed out".into());
    }
    if state.captures == 6 {
        state.finish_frames += 1;
        if state.finish_frames >= 3 {
            info!(
                "Native spanning silk gallery passed: actual section-batched 7/12/20m WorldProps, isolated neutral views, real remote anchors and cross-section AABBs; visual fixture only"
            );
            world.write_message(AppExit::Success);
        }
        return Ok(());
    }
    if state.pending
        || state.started.elapsed() < Duration::from_secs(3)
        || state.last.elapsed() < Duration::from_millis(900)
    {
        return Ok(());
    }
    let status = world.resource::<PropRenderStatus>();
    if let Some(error) = &status.error {
        return Err(error.clone());
    }
    if !status.ready || status.pending_jobs != 0 || status.visible_props != 3 {
        return Ok(());
    }
    let batches: Vec<_> = world
        .query_filtered::<(&Aabb, &GlobalTransform), With<WindProp>>()
        .iter(world)
        .map(|(bounds, transform)| (*bounds, *transform))
        .collect();
    if batches.len() != 3 {
        return Err(format!(
            "expected three section batches, got {}",
            batches.len()
        ));
    }
    for prop in state.props {
        let span = prop.scale * webs::TEMPLATE_SPAN;
        if !batches.iter().any(|(bounds, transform)| {
            let min = transform.transform_point(Vec3::from(bounds.center - bounds.half_extents));
            let max = transform.transform_point(Vec3::from(bounds.center + bounds.half_extents));
            min.is_finite()
                && max.is_finite()
                && (min.x - prop.position[0] as f32).abs() < 0.1
                && max.x >= prop.position[0] as f32 + span - 0.04
                && min.z < 24.5
                && max.z > 24.5
        }) {
            return Err(format!(
                "actual section batch clipped construction {:?}",
                prop.id
            ));
        }
    }
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-web-constructions"));
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let stage = state.captures;
    state.pending = true;
    world.spawn(Screenshot::primary_window()).observe(save_to_disk(directory.join(format!("web-{}m-{}.png", [7, 12, 20][stage / 2], stage % 2))))
        .observe(move |capture: On<ScreenshotCaptured>, mut gallery: ResMut<Gallery>| {
            let result = capture.image.clone().try_into_dynamic().map_err(|error| error.to_string()).and_then(|image| {
                let image = image.into_rgba8();
                let mut silk = 0;
                let mut nonblank = 0;
                for y in image.height() / 8..image.height() * 7 / 8 {
                    for x in image.width() / 10..image.width() * 9 / 10 {
                        let [r, g, b, _] = image.get_pixel(x, y).0.map(f64::from);
                        nonblank += usize::from(r.max(g).max(b) > 25.0);
                        let peak = r.max(g).max(b);
                        let floor = r.min(g).min(b);
                        let dark = [x - 3, x + 3].into_iter().map(|side| {
                            let p = image.get_pixel(side, y).0;
                            f64::from(p[0].max(p[1]).max(p[2]))
                        }).fold(f64::INFINITY, f64::min);
                        silk += usize::from(peak > 90.0 && floor > peak * 0.45 && peak > dark + 25.0);
                    }
                }
                info!(stage, silk, nonblank, "Native spanning silk framebuffer pixels");
                if nonblank < 1_000 || silk < 100 { Err(format!("blank or absent silk, stage={stage} silk={silk} nonblank={nonblank}")) } else { Ok(()) }
            });
            if let Err(error) = result { gallery.failure = Some(error); }
            gallery.captures += 1;
            gallery.last = Instant::now();
            gallery.pending = false;
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_gallery_removes_probe_generators_before_disabling_the_original_atmosphere_view() {
        let mut world = World::new();
        let original = world
            .spawn((
                Camera3d::default(),
                terra_player::TerraCamera,
                bevy::light::AtmosphereEnvironmentMapLight::default(),
                bevy::pbr::AtmosphereSettings::default(),
            ))
            .id();
        let inspection = world.spawn((Camera3d::default(), InspectionCamera)).id();
        detach_gameplay_environment(&mut world);
        assert!(!world.get::<Camera>(original).unwrap().is_active);
        assert!(
            world
                .get::<bevy::light::AtmosphereEnvironmentMapLight>(original)
                .is_none()
        );
        assert!(
            world
                .get::<bevy::pbr::AtmosphereSettings>(original)
                .is_none()
        );
        assert!(world.get::<Camera>(inspection).unwrap().is_active);
    }
}
