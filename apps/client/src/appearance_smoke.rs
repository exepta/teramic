use bevy::{app::AppExit, asset::AssetId, ecs::system::SystemParam, prelude::*};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{GameConfig, WeatherKind};
use terra_player::{ControllerMode, TerraCamera};
use terra_render::{PropRenderStatus, RenderStatus, RenderedSections};
use terra_voxel::SectionPos;
use terra_world::{
    edits::EditJournal,
    environment::SurfaceCondition,
    streaming::{ChunkFocus, ChunkStore, GenerationStatus, WorldGeneration, WorldTerrain},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Dry,
    Rain,
    Snow,
    Charred,
    Wet,
    Away,
    Reloaded,
    Reset,
    Complete,
}
impl Stage {
    fn label(self) -> &'static str {
        match self {
            Self::Dry => "dry",
            Self::Rain => "rain",
            Self::Snow => "snow",
            Self::Charred => "charred",
            Self::Wet => "local-wet",
            Self::Away => "away",
            Self::Reloaded => "reloaded-wet",
            Self::Reset => "reset",
            Self::Complete => "complete",
        }
    }
}

struct Fixture {
    cells: Vec<[i64; 3]>,
    owner: SectionPos,
    camera: Transform,
    handles: Vec<AssetId<Mesh>>,
    material: AssetId<StandardMaterial>,
    neighbor: (SectionPos, Vec<AssetId<Mesh>>),
    mesh_epoch: u64,
    prop_epoch: u64,
    expected_appearance: u64,
}

#[derive(Resource)]
struct AppearanceSmoke {
    started: Instant,
    stage: Stage,
    ready_since: Option<Instant>,
    captured_at: Option<Instant>,
    fixture: Option<Fixture>,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-appearance") {
        return;
    }
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(terra_render::WeatherPreview {
            kind: WeatherKind::Clear,
            intensity: 1.0,
        })
        .insert_resource(AppearanceSmoke {
            started: Instant::now(),
            stage: Stage::Dry,
            ready_since: None,
            captured_at: None,
            fixture: None,
        })
        .add_systems(PostStartup, setup_camera)
        .add_systems(
            PostUpdate,
            run.before(bevy::transform::TransformSystems::Propagate),
        );
}

fn setup_camera(
    mut cameras: Query<&mut Transform, With<TerraCamera>>,
    mut focus: ResMut<ChunkFocus>,
    terrain: Res<WorldTerrain>,
    mut config: ResMut<GameConfig>,
) {
    let target = Vec3::new(
        -256.0,
        terrain.0.surface_height(-256.0, 1536.0) as f32,
        1536.0,
    );
    let eye = target + Vec3::new(8.0, 11.0, 14.0);
    for mut camera in &mut cameras {
        *camera = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
    focus.0 = eye.to_array().map(f64::from);
    config.client.weather = WeatherKind::Clear;
    config.client.weather_intensity = 1.0;
}

#[derive(SystemParam)]
struct AppearanceWorld<'w, 's> {
    chunks: Res<'w, ChunkStore>,
    generation: Res<'w, WorldGeneration>,
    rendered: Res<'w, RenderedSections>,
    renderer: Res<'w, RenderStatus>,
    props: Res<'w, PropRenderStatus>,
    journal: ResMut<'w, EditJournal>,
    materials: Res<'w, Assets<StandardMaterial>>,
    server: Res<'w, AssetServer>,
    children: Query<'w, 's, &'static Children>,
    material_handles: Query<'w, 's, &'static MeshMaterial3d<StandardMaterial>>,
}

#[derive(SystemParam)]
struct AppearanceView<'w, 's> {
    config: ResMut<'w, GameConfig>,
    preview: ResMut<'w, terra_render::WeatherPreview>,
    focus: ResMut<'w, ChunkFocus>,
    cameras: Query<'w, 's, &'static mut Transform, With<TerraCamera>>,
}

fn run(
    mut commands: Commands,
    mut smoke: ResMut<AppearanceSmoke>,
    mut world: AppearanceWorld,
    mut view: AppearanceView,
    mut exit: MessageWriter<AppExit>,
) {
    if smoke.stage == Stage::Complete {
        return;
    }
    if let Some(error) = world
        .renderer
        .error
        .as_deref()
        .or(world.props.error.as_deref())
    {
        fail(&mut smoke, &mut exit, error);
        return;
    }
    if smoke.started.elapsed() > Duration::from_secs(60) {
        fail(
            &mut smoke,
            &mut exit,
            "Appearance smoke exceeded 60 seconds",
        );
        return;
    }
    if smoke.stage == Stage::Away {
        let Some(fixture) = smoke.fixture.as_ref() else {
            return;
        };
        if world.chunks.loaded.contains_key(&fixture.owner)
            || world.rendered.sections.contains_key(&fixture.owner)
        {
            return;
        }
        tracing::info!(owner=?fixture.owner, "Appearance smoke observed actual section eviction");
        set_camera(&mut view, fixture.camera);
        smoke.stage = Stage::Reloaded;
        return;
    }
    if world.generation.status != GenerationStatus::Ready
        || !world.renderer.ready
        || !world.props.ready
    {
        smoke.ready_since = None;
        return;
    }
    if smoke.fixture.is_none() {
        let Some(camera) = view.cameras.iter().next().copied() else {
            return;
        };
        let Some(fixture) = find_fixture(&world, camera.translation) else {
            fail(
                &mut smoke,
                &mut exit,
                "No visible grass fixture or grass material found",
            );
            return;
        };
        set_camera(&mut view, fixture.camera);
        tracing::info!(owner=?fixture.owner, cells=fixture.cells.len(), material=?fixture.material,
            "Appearance smoke selected natural grass patch");
        smoke.fixture = Some(fixture);
        return;
    }
    let Some(fixture) = smoke.fixture.as_ref() else {
        return;
    };
    let Some(rendered) = world.rendered.sections.get(&fixture.owner) else {
        return;
    };
    if rendered.appearance_epoch != fixture.expected_appearance {
        return;
    }
    if smoke.ready_since.get_or_insert_with(Instant::now).elapsed() < Duration::from_millis(350) {
        return;
    }
    if smoke.captured_at.is_none() {
        let Some(fixture) = smoke.fixture.as_ref() else {
            return;
        };
        if let Err(error) = validate(smoke.stage, fixture, &world) {
            fail(&mut smoke, &mut exit, &error);
            return;
        }
        capture(&mut commands, smoke.stage.label());
        tracing::info!(stage=smoke.stage.label(), owner=?fixture.owner,
            appearance_epoch=fixture.expected_appearance, mesh_epoch=fixture.mesh_epoch,
            mesh_handles=?mesh_ids(&world, fixture.owner), material=?fixture.material,
            "Appearance smoke stage verified");
        smoke.captured_at = Some(Instant::now());
        return;
    }
    if smoke
        .captured_at
        .is_some_and(|instant| instant.elapsed() < Duration::from_millis(350))
    {
        return;
    }
    if let Err(error) = advance(&mut smoke, &mut world, &mut view) {
        fail(&mut smoke, &mut exit, &error);
        return;
    }
    if smoke.stage == Stage::Complete {
        tracing::info!(
            elapsed_seconds = smoke.started.elapsed().as_secs_f64(),
            "Appearance smoke passed: dry/rain/snow, local charred/wet/reset, actual eviction/reload"
        );
        exit.write(AppExit::Success);
    }
}

fn find_fixture(world: &AppearanceWorld, eye: Vec3) -> Option<Fixture> {
    let mut candidates = Vec::new();
    for &owner in world.chunks.loaded.keys() {
        if !world.rendered.sections.contains_key(&owner) {
            continue;
        }
        let Ok(origin) = owner.origin() else {
            continue;
        };
        if Vec3::from_array(origin.map(|v| v as f32)).distance(eye) > 42.0 {
            continue;
        }
        let mut cells = Vec::new();
        for z in 3..13 {
            for x in 3..13 {
                for y in 0..16 {
                    let p = [origin[0] + x, origin[1] + y, origin[2] + z];
                    if world.chunks.block_at(p) == Some(1)
                        && (1..=9)
                            .all(|dy| world.chunks.block_at([p[0], p[1] + dy, p[2]]) == Some(0))
                    {
                        cells.push(p);
                    }
                }
            }
        }
        if cells.len() >= 16 {
            candidates.push((owner, cells));
        }
    }
    candidates
        .sort_by_key(|(owner, cells)| (std::cmp::Reverse(cells.len()), owner.y, owner.z, owner.x));
    let (owner, cells) = candidates.into_iter().next()?;
    let rendered = world.rendered.sections.get(&owner)?;
    let material = world
        .children
        .get(rendered.entity)
        .ok()?
        .iter()
        .find_map(|entity| {
            let handle = world.material_handles.get(entity).ok()?;
            let material = world.materials.get(&handle.0)?;
            let path = world
                .server
                .get_path(material.base_color_texture.as_ref()?.id())?;
            path.path()
                .file_name()?
                .to_str()?
                .starts_with("grass-")
                .then_some(handle.0.id())
        })?;
    let target = cells
        .iter()
        .map(|p| Vec3::from_array(p.map(|v| v as f32)) + Vec3::splat(0.5))
        .sum::<Vec3>()
        / cells.len() as f32;
    let camera = Transform::from_translation(target + Vec3::new(7.0, 12.0, 13.0))
        .looking_at(target, Vec3::Y);
    let neighbor = world
        .rendered
        .sections
        .iter()
        .filter(|(position, section)| **position != owner && !section.meshes.is_empty())
        .min_by_key(|(position, _)| {
            (position.x - owner.x).abs()
                + (position.y - owner.y).abs()
                + (position.z - owner.z).abs()
        })
        .map(|(&position, _)| (position, mesh_ids(world, position)))?;
    Some(Fixture {
        cells,
        owner,
        camera,
        handles: mesh_ids(world, owner),
        material,
        neighbor,
        mesh_epoch: world.journal.mesh_epoch(owner),
        prop_epoch: world.journal.prop_epoch(owner),
        expected_appearance: world.journal.appearance_epoch(owner),
    })
}

fn mesh_ids(world: &AppearanceWorld, owner: SectionPos) -> Vec<AssetId<Mesh>> {
    world
        .rendered
        .sections
        .get(&owner)
        .map(|section| section.meshes.iter().map(Handle::id).collect())
        .unwrap_or_default()
}

fn validate(stage: Stage, fixture: &Fixture, world: &AppearanceWorld) -> Result<(), String> {
    if world.journal.mesh_epoch(fixture.owner) != fixture.mesh_epoch
        || world.journal.prop_epoch(fixture.owner) != fixture.prop_epoch
    {
        return Err("Appearance update changed logical terrain/prop revision".to_owned());
    }
    if world.journal.appearance_epoch(fixture.owner) != fixture.expected_appearance {
        return Err("Unexpected appearance epoch".to_owned());
    }
    let handles = mesh_ids(world, fixture.owner);
    if handles.is_empty() {
        return Err("Appearance owner has no uploaded meshes".to_owned());
    }
    if matches!(stage, Stage::Dry | Stage::Rain | Stage::Snow) && handles != fixture.handles {
        return Err("Global weather unnecessarily rebuilt terrain geometry".to_owned());
    }
    if matches!(
        stage,
        Stage::Charred | Stage::Wet | Stage::Reloaded | Stage::Reset
    ) && handles == fixture.handles
    {
        return Err("Local surface state did not rebuild owner mesh".to_owned());
    }
    if stage != Stage::Reloaded && mesh_ids(world, fixture.neighbor.0) != fixture.neighbor.1 {
        return Err("Appearance-only update rebuilt an unrelated neighbor".to_owned());
    }
    let material = world
        .materials
        .get(fixture.material)
        .ok_or("Grass material handle was replaced")?;
    let expected = match stage {
        Stage::Rain => [0.87, 0.92, 0.95],
        Stage::Snow => [1.02, 1.03, 1.05],
        _ => [1.0; 3],
    };
    let actual = material.base_color.to_linear().to_f32_array();
    if (0..3).any(|i| (actual[i] - expected[i]).abs() > 0.001) {
        return Err(format!("Weather material tint mismatch: {actual:?}"));
    }
    let roughness = if stage == Stage::Rain { 0.67 } else { 0.95 };
    if (material.perceptual_roughness - roughness).abs() > 0.001 {
        return Err("Weather roughness mismatch".to_owned());
    }
    let condition = match stage {
        Stage::Charred => SurfaceCondition {
            wetness: 0.0,
            charred: 1.0,
            ..Default::default()
        },
        Stage::Wet | Stage::Reloaded => SurfaceCondition {
            wetness: 1.0,
            charred: 0.0,
            ..Default::default()
        },
        _ => SurfaceCondition::default(),
    };
    if fixture.cells.iter().any(|&p| {
        world.chunks.block_at(p) != Some(1) || world.journal.surface_condition(p) != condition
    }) {
        return Err("Local surface condition or grass ID changed unexpectedly".to_owned());
    }
    Ok(())
}

fn advance(
    smoke: &mut AppearanceSmoke,
    world: &mut AppearanceWorld,
    view: &mut AppearanceView,
) -> Result<(), String> {
    let Some(fixture) = smoke.fixture.as_mut() else {
        return Ok(());
    };
    fixture.handles = mesh_ids(world, fixture.owner);
    if smoke.stage == Stage::Reloaded {
        fixture.neighbor.1 = mesh_ids(world, fixture.neighbor.0);
    }
    smoke.stage = match smoke.stage {
        Stage::Dry => {
            view.preview.kind = WeatherKind::Rain;
            Stage::Rain
        }
        Stage::Rain => {
            view.preview.kind = WeatherKind::Snow;
            Stage::Snow
        }
        Stage::Snow => {
            view.preview.kind = WeatherKind::Clear;
            set_condition(
                fixture,
                world,
                SurfaceCondition {
                    wetness: 0.0,
                    charred: 1.0,
                    ..Default::default()
                },
            )?;
            Stage::Charred
        }
        Stage::Charred => {
            set_condition(
                fixture,
                world,
                SurfaceCondition {
                    wetness: 1.0,
                    charred: 0.0,
                    ..Default::default()
                },
            )?;
            Stage::Wet
        }
        Stage::Wet => {
            let mut camera = fixture.camera;
            camera.translation.x += (f32::from(view.config.client.view_distance) + 4.0) * 16.0;
            set_camera(view, camera);
            Stage::Away
        }
        Stage::Reloaded => {
            set_condition(fixture, world, SurfaceCondition::default())?;
            Stage::Reset
        }
        Stage::Reset => Stage::Complete,
        other => other,
    };
    smoke.ready_since = None;
    smoke.captured_at = None;
    Ok(())
}

fn set_condition(
    fixture: &mut Fixture,
    world: &mut AppearanceWorld,
    condition: SurfaceCondition,
) -> Result<(), String> {
    for &cell in &fixture.cells {
        world
            .journal
            .set_surface_condition(cell, condition)
            .map_err(|error| error.to_string())?;
    }
    fixture.expected_appearance = world.journal.appearance_epoch(fixture.owner);
    Ok(())
}

fn set_camera(view: &mut AppearanceView, transform: Transform) {
    for mut camera in &mut view.cameras {
        *camera = transform;
    }
    view.focus.0 = transform.translation.to_array().map(f64::from);
}

fn capture(commands: &mut Commands, label: &str) {
    let path = std::env::var_os("TERRA_SCREENSHOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/terra-appearance.png"));
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let path = path.with_file_name(format!("{stem}-{label}.png"));
    commands
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(path));
}

fn fail(smoke: &mut AppearanceSmoke, exit: &mut MessageWriter<AppExit>, error: &str) {
    tracing::error!(stage=smoke.stage.label(), %error, "Appearance smoke failed");
    smoke.stage = Stage::Complete;
    exit.write(AppExit::error());
}
