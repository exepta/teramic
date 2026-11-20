//! Opt-in original glTF/PBR gallery, separate from terrain model registration.
use bevy::{
    asset::{
        LoadState, RenderAssetUsages,
        io::{AssetSourceBuilder, file::FileAssetReader},
    },
    gltf::{Gltf, GltfLoaderSettings, GltfMesh, GltfNode},
    mesh::VertexAttributeValues,
    prelude::*,
    render::{
        render_resource::Face,
        view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    },
};
use std::{
    collections::HashSet,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use terra_core::{ClientConfig, WorldClock, WorldConfig};
use terra_render::{RenderStatus, RenderedSections};
use terra_voxel::{MaterialId, SectionPos};
use terra_world::{
    edits::EditJournal,
    streaming::{BlockEditRequest, ChunkFocus, ChunkStore, StreamingSet, commit_block_edit},
};

const FLAG: &str = "--smoke-test-fiverr-grass";
const FILE: &str = "VoxelGrassBlock glb file.glb";
const ASSET: &str = "fiverr://VoxelGrassBlock glb file.glb";
const VIEWS: [&str; 4] = ["front", "side", "top", "coarse"];
const LEFT: Vec3 = Vec3::new(4.0, 32.0, 8.0);
const RIGHT: Vec3 = Vec3::new(8.0, 32.0, 8.0);
const FOV: f32 = 55.0;

#[derive(Component)]
struct OriginalGrass;

#[derive(Resource)]
struct Fixture {
    started: Instant,
    root: Handle<Gltf>,
    imported: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
    edited: bool,
    view: usize,
    settled: Option<Instant>,
    pending: bool,
    failure: Option<String>,
    finish_frames: u8,
}

pub(crate) fn requested() -> bool {
    std::env::args().any(|argument| argument == FLAG)
}

pub(crate) fn configure(world: &mut WorldConfig, client: &mut ClientConfig) {
    *world = WorldConfig {
        base_height: 32.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        biomes: false,
        trees: false,
        caves: false,
        water: false,
        rivers: false,
        structures_revision: 0,
        ecology_revision: 0,
        natural_mob_limit: 0,
        ..default()
    };
    client.view_distance = 2;
    client.vertical_view_distance = 1;
    client.fov_degrees = FOV;
    client.weather = terra_core::WeatherKind::Clear;
    client.weather_intensity = 0.0;
    client.fog_distance = 0.0;
    client.chunk_animation = terra_core::ChunkAnimation::None;
}

fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../FIVERR")
}

pub(crate) fn register_source(app: &mut App) -> Result<(), String> {
    let root = source_root()
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let path = root.join(FILE);
    let mut file = std::fs::File::open(&path)
        .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
    let mut header = [0; 12];
    file.read_exact(&mut header)
        .map_err(|error| error.to_string())?;
    if &header[..4] != b"glTF"
        || u32::from_le_bytes(header[4..8].try_into().unwrap()) != 2
        || u64::from(u32::from_le_bytes(header[8..12].try_into().unwrap()))
            != file.metadata().map_err(|error| error.to_string())?.len()
    {
        return Err(format!("{} is not a complete GLB 2 file", path.display()));
    }
    app.register_asset_source(
        "fiverr",
        AssetSourceBuilder::new(move || Box::new(FileAssetReader::new(root.clone()))),
    );
    Ok(())
}

pub(crate) fn install(app: &mut App) {
    if !requested() {
        return;
    }
    let root = app
        .world()
        .resource::<AssetServer>()
        .load_builder()
        .with_settings(|settings: &mut GltfLoaderSettings| {
            settings.load_meshes = RenderAssetUsages::default();
        })
        .load(ASSET);
    app.insert_resource(Fixture {
        started: Instant::now(),
        root,
        imported: None,
        edited: false,
        view: 0,
        settled: None,
        pending: false,
        failure: None,
        finish_frames: 0,
    })
    .insert_resource(terra_player::ControllerMode::Fly)
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PostStartup, setup)
    .add_systems(
        Update,
        pin.after(terra_player::PlayerSet::Movement)
            .before(StreamingSet::Interest),
    )
    .add_systems(Last, run);
}

fn setup(world: &mut World) {
    let mut clock = world.resource_mut::<WorldClock>();
    clock.phase = 0.42;
    clock.paused = true;
    let clock = *clock;
    world.resource_mut::<EditJournal>().clock = Some(clock);
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
    world
        .resource_mut::<terra_core::WorldWeather>()
        .set_pattern(terra_core::WeatherPattern::Clear);
    info!(asset = ASSET, root = %source_root().display(), "Opt-in Fiverr grass preview: original shared mesh/material, four instances only; terrain registry unchanged");
}

fn view_transform(index: usize) -> Transform {
    let target = Vec3::new(7.0, 32.45, 9.0);
    match index {
        0 | 3 => Transform::from_xyz(7.0, 35.3, 15.4).looking_at(target, Vec3::Y),
        1 => Transform::from_xyz(13.0, 35.5, 13.2).looking_at(target, Vec3::Y),
        _ => Transform::from_xyz(7.0, 40.8, 9.0).looking_at(target, Vec3::NEG_Z),
    }
}

type FixtureView = Or<(
    With<terra_player::TerraCamera>,
    With<terra_player::PlayerBody>,
)>;

fn pin(
    state: Res<Fixture>,
    mut config: ResMut<terra_core::GameConfig>,
    mut focus: ResMut<ChunkFocus>,
    mut transforms: Query<&mut Transform, FixtureView>,
) {
    let detailed = state.view < 3;
    if config.client.blocks.micro_geometry != detailed {
        config.client.blocks.micro_geometry = detailed;
    }
    let view = view_transform(state.view.min(VIEWS.len() - 1));
    focus.0 = [7.0, 34.0, 9.0];
    for mut transform in &mut transforms {
        *transform = view;
    }
}

fn validate_mesh(mesh: &Mesh) -> Result<(), String> {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return Err("Original grass lacks positions".into());
    };
    let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else {
        return Err("Original grass lacks UV0".into());
    };
    let Some(VertexAttributeValues::Float32x4(tangents)) = mesh.attribute(Mesh::ATTRIBUTE_TANGENT)
    else {
        return Err("glTF normal-map import did not retain generated tangents".into());
    };
    if positions.len() != 1494
        || uvs.len() != positions.len()
        || tangents.len() != positions.len()
        || mesh.indices().is_none_or(|indices| indices.len() != 2304)
        || positions
            .iter()
            .flatten()
            .any(|p| !p.is_finite() || p.abs() > 0.50001)
        || uvs
            .iter()
            .flatten()
            .any(|uv| !uv.is_finite() || !(0.0..=1.0).contains(uv))
        || tangents.iter().flatten().any(|t| !t.is_finite())
    {
        return Err("Original grass geometry/UV/tangent contract changed".into());
    }
    for axis in 0..3 {
        let min = positions
            .iter()
            .map(|p| p[axis])
            .fold(f32::INFINITY, f32::min);
        let max = positions
            .iter()
            .map(|p| p[axis])
            .fold(f32::NEG_INFINITY, f32::max);
        if (min + 0.5).abs() > 0.00001 || (max - 0.5).abs() > 0.00001 {
            return Err("Original grass is no longer a centered one-metre block".into());
        }
    }
    Ok(())
}

fn validate_material(material: &StandardMaterial, images: &Assets<Image>) -> Result<(), String> {
    let textures = [
        material.base_color_texture.as_ref(),
        material.normal_map_texture.as_ref(),
        material.metallic_roughness_texture.as_ref(),
        material.occlusion_texture.as_ref(),
    ];
    if material.alpha_mode != AlphaMode::Opaque || material.cull_mode != Some(Face::Back) {
        return Err("Original grass must retain opaque, backface-culled PBR".into());
    }
    let mut unique = HashSet::new();
    for handle in textures {
        let handle = handle.ok_or("Original grass lost a PBR texture binding")?;
        let image = images
            .get(handle)
            .ok_or("Original grass PBR texture unavailable")?;
        if image.texture_descriptor.size.width != 1024
            || image.texture_descriptor.size.height != 1024
        {
            return Err("Original grass PBR texture dimensions changed".into());
        }
        unique.insert(handle.id());
    }
    if unique.len() != 3 {
        return Err("Expected shared ORM plus original base-color and normal textures".into());
    }
    Ok(())
}

fn import(world: &mut World, state: &mut Fixture) -> Result<bool, String> {
    let server = world.resource::<AssetServer>();
    if let Some(LoadState::Failed(error)) = server.get_load_state(state.root.id()) {
        return Err(error.to_string());
    }
    if !server.is_loaded_with_dependencies(state.root.id()) {
        return Ok(false);
    }
    let gltfs = world.resource::<Assets<Gltf>>();
    let gltf = gltfs
        .get(&state.root)
        .ok_or("Loaded grass glTF unavailable")?;
    if gltf.meshes.len() != 1 || gltf.nodes.len() != 1 || gltf.materials.len() != 1 {
        return Err("Original grass must have one static mesh, node and material".into());
    }
    let node = world
        .resource::<Assets<GltfNode>>()
        .get(&gltf.nodes[0])
        .ok_or("Grass node unavailable")?;
    if node.transform != Transform::IDENTITY || node.skin.is_some() || !node.children.is_empty() {
        return Err("Original grass node must retain its unskinned identity transform".into());
    }
    let gltf_meshes = world.resource::<Assets<GltfMesh>>();
    let mesh = gltf_meshes
        .get(&gltf.meshes[0])
        .ok_or("Grass mesh unavailable")?;
    if mesh.primitives.len() != 1 {
        return Err("Expected one original grass primitive".into());
    }
    let mesh = mesh.primitives[0].mesh.clone();
    validate_mesh(
        world
            .resource::<Assets<Mesh>>()
            .get(&mesh)
            .ok_or("Grass CPU mesh unavailable")?,
    )?;
    // Reuse the subasset already produced by Bevy's PBR extension; do not reload its root.
    let material = server
        .get_handle::<StandardMaterial>(format!("{ASSET}#Material0/std"))
        .ok_or("Loaded original grass PBR material unavailable")?;
    validate_material(
        world
            .resource::<Assets<StandardMaterial>>()
            .get(&material)
            .ok_or("Grass PBR material unavailable")?,
        world.resource::<Assets<Image>>(),
    )?;
    for z in 0..2 {
        for x in 0..2 {
            world.spawn((
                OriginalGrass,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(LEFT + Vec3::new(x as f32 + 0.5, 0.5, z as f32 + 0.5)),
            ));
        }
    }
    info!(
        vertices = 1494,
        triangles = 768,
        instances = 4,
        unique_meshes = 1,
        unique_materials = 1,
        textures = 3,
        "Original Fiverr glTF loaded with full UV/PBR/tangent data; original cube has no neighbour-face elimination"
    );
    state.imported = Some((mesh, material));
    Ok(true)
}

fn edit_comparison(world: &mut World) -> Result<bool, String> {
    let mut edits = Vec::new();
    for z in 6..12 {
        for x in 2..12 {
            edits.push((
                [x, 31, z],
                if (8..12).contains(&x) && (8..10).contains(&z) {
                    MaterialId::Dirt as u16
                } else {
                    MaterialId::Stone as u16
                },
            ));
        }
    }
    for z in 8..10 {
        for x in 8..10 {
            edits.push(([x, 32, z], MaterialId::Grass as u16));
        }
    }
    if edits
        .iter()
        .any(|(p, _)| world.resource::<ChunkStore>().block_at(*p).is_none())
    {
        return Ok(false);
    }
    let properties = world
        .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .clone();
    world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
        let mut journal = world.resource_mut::<EditJournal>();
        for (position, material) in edits {
            let expected_material = chunks
                .block_at(position)
                .ok_or("Comparison section unloaded")?;
            if expected_material != material
                && !commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &BlockEditRequest {
                        position,
                        expected_material,
                        material,
                    },
                    &properties,
                )?
            {
                return Err(format!(
                    "Real comparison block edit rejected at {position:?}"
                ));
            }
        }
        for z in 8..10 {
            for x in 8..10 {
                let p = [x, 32, z];
                if chunks.block_at(p) != Some(MaterialId::Grass as u16)
                    || journal.block(p) != Some(MaterialId::Grass as u16)
                {
                    return Err(
                        "Comparison grass was not placed through the real world edit path".into(),
                    );
                }
            }
        }
        Ok(true)
    })
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Fixture>| {
        if let Err(error) = advance(world, &mut state) {
            error!(%error, "Native Fiverr grass fixture failed");
            world.write_message(AppExit::error());
        }
    });
}

fn advance(world: &mut World, state: &mut Fixture) -> Result<(), String> {
    if let Some(error) = &state.failure {
        return Err(error.clone());
    }
    if state.started.elapsed() > Duration::from_secs(45) {
        return Err("Original grass preview timed out".into());
    }
    if state.pending {
        return Ok(());
    }
    if state.view == VIEWS.len() {
        state.finish_frames += 1;
        if state.finish_frames == 3 {
            info!(
                "Native Fiverr grass fixture passed: four textured GPU captures including coarse PBR, shared original meshes, neighbouring grass and matching soil world edits"
            );
            world.write_message(AppExit::Success);
        }
        return Ok(());
    }
    if state.imported.is_none() && !import(world, state)? {
        return Ok(());
    }
    if !state.edited {
        state.edited = edit_comparison(world)?;
        if !state.edited {
            return Ok(());
        }
    }
    if let Some(error) = &world.resource::<RenderStatus>().error {
        return Err(error.clone());
    }
    let journal = world.resource::<EditJournal>();
    let rendered = world.resource::<RenderedSections>();
    if [SectionPos::new(0, 1, 0), SectionPos::new(0, 2, 0)]
        .into_iter()
        .any(|p| {
            rendered.sections.get(&p).is_none_or(|section| {
                section.mesh_epoch != journal.mesh_epoch(p)
                    || section.meshes.is_empty()
                    || section.micro_detail != (state.view < 3)
            })
        })
    {
        state.settled = None;
        return Ok(());
    }
    let (mesh, material) = state.imported.as_ref().unwrap();
    let mut models =
        world.query_filtered::<(&Mesh3d, &MeshMaterial3d<StandardMaterial>), With<OriginalGrass>>();
    if models.iter(world).count() != 4
        || models
            .iter(world)
            .any(|(m, p)| m.0 != *mesh || p.0 != *material)
    {
        return Err(
            "Original grass fixture stopped sharing one mesh/material across four instances".into(),
        );
    }
    if state.settled.get_or_insert_with(Instant::now).elapsed() < Duration::from_millis(1200) {
        return Ok(());
    }
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-fiverr-grass"));
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let view = state.view;
    state.pending = true;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(
            directory.join(format!("fiverr-grass-{}.png", VIEWS[view])),
        ))
        .observe(
            move |capture: On<ScreenshotCaptured>, mut state: ResMut<Fixture>| {
                match inspect_pixels(&capture.image, view) {
                    Ok((original, current)) => info!(
                        view = VIEWS[view],
                        original_green_pixels = original,
                        current_green_pixels = current,
                        "Grass comparison GPU pixels measured"
                    ),
                    Err(error) => state.failure = Some(error),
                }
                state.pending = false;
                state.view += 1;
                state.settled = None;
            },
        );
    Ok(())
}

fn pixel_bounds(low: Vec3, view: usize, width: u32, height: u32) -> Result<[u32; 4], String> {
    let inverse = view_transform(view).compute_affine().inverse();
    let scale = (FOV.to_radians() * 0.5).tan();
    let aspect = width as f32 / height as f32;
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for z in [0.0, 2.0] {
        for y in [0.0, 1.0] {
            for x in [0.0, 2.0] {
                let p = inverse.transform_point3(low + Vec3::new(x, y, z));
                if p.z >= -0.1 {
                    return Err("Grass comparison is behind its fixture camera".into());
                }
                let ndc = Vec2::new(p.x / (-p.z * scale * aspect), -p.y / (-p.z * scale));
                let pixel = (ndc + Vec2::ONE) * Vec2::new(width as f32, height as f32) * 0.5;
                min = min.min(pixel);
                max = max.max(pixel);
            }
        }
    }
    if min.cmplt(Vec2::ZERO).any() || max.cmpgt(Vec2::new(width as f32, height as f32)).any() {
        return Err("Grass comparison was clipped by the capture viewport".into());
    }
    Ok([
        min.x.floor() as u32,
        min.y.floor() as u32,
        max.x.ceil() as u32,
        max.y.ceil() as u32,
    ])
}

fn inspect_pixels(image: &Image, view: usize) -> Result<(usize, usize), String> {
    let rgba = image
        .clone()
        .try_into_dynamic()
        .map_err(|error| error.to_string())?
        .into_rgba8();
    let (width, height) = rgba.dimensions();
    if width < 640 || height < 360 {
        return Err("Grass comparison capture must be at least 640x360".into());
    }
    let mut counts = [0; 2];
    for (index, low) in [LEFT, RIGHT].into_iter().enumerate() {
        let [x0, y0, x1, y1] = pixel_bounds(low, view, width, height)?;
        let mut darkest = 1.0_f32;
        let mut brightest = 0.0_f32;
        for y in y0..y1 {
            for x in x0..x1 {
                let pixel = rgba.get_pixel(x, y).0;
                let [r, g, b] = [pixel[0], pixel[1], pixel[2]].map(f32::from);
                counts[index] += usize::from(g > 35.0 && g > r * 1.10 && g > b * 1.20);
                let light = (r * 0.2126 + g * 0.7152 + b * 0.0722) / 255.0;
                darkest = darkest.min(light);
                brightest = brightest.max(light);
            }
        }
        if counts[index] < ((x1 - x0) * (y1 - y0)) as usize / 40 || brightest - darkest < 0.06 {
            return Err(format!(
                "Grass capture {} lacks textured green geometry in patch {index}",
                VIEWS[view]
            ));
        }
    }
    Ok((counts[0], counts[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_config_is_fresh_flat_and_uses_the_main_terrain_contract() {
        let mut world = WorldConfig {
            landscape_revision: 5,
            biome_definitions: Some(terra_core::BiomeDefinitions::default()),
            ..Default::default()
        };
        let mut client = ClientConfig::default();
        configure(&mut world, &mut client);
        assert_eq!(world.landscape_revision, 9);
        assert_eq!(world.sea_level, 8.0);
        assert!(world.biome_definitions.is_none());
        assert_eq!(world.hill_amplitude + world.mountain_amplitude, 0.0);
        assert!(!world.trees && !world.caves && !world.water && !world.rivers);
        assert_eq!(world.structures_revision, 0);
    }

    #[test]
    fn all_capture_views_frame_both_one_metre_join_patches() {
        for view in 0..VIEWS.len() {
            for (width, height) in [(1280, 720), (1280, 800), (640, 480)] {
                for low in [LEFT, RIGHT] {
                    let [x0, y0, x1, y1] = pixel_bounds(low, view, width, height).unwrap();
                    assert!(x1 > x0 + 20 && y1 > y0 + 20);
                }
            }
        }
    }

    #[test]
    fn original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images() {
        let mut app = App::new();
        register_source(&mut app).unwrap();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::world_serialization::WorldSerializationPlugin,
            bevy::mesh::MeshPlugin,
            bevy::image::ImagePlugin::default(),
            bevy::gltf::GltfPlugin::default(),
        ))
        .init_asset::<StandardMaterial>();
        app.finish();
        app.cleanup();
        let root: Handle<Gltf> = app.world().resource::<AssetServer>().load(ASSET);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            app.update();
            let server = app.world().resource::<AssetServer>();
            assert!(
                !matches!(server.get_load_state(root.id()), Some(LoadState::Failed(_))),
                "{:?}",
                server.get_load_state(root.id())
            );
            if server.is_loaded_with_dependencies(root.id()) {
                break;
            }
            assert!(Instant::now() < deadline, "named-source GLB load timed out");
            std::thread::sleep(Duration::from_millis(1));
        }
        let world = app.world();
        assert!(
            world
                .resource::<AssetServer>()
                .get_source("fiverr")
                .unwrap()
                .writer()
                .is_err()
        );
        let gltf = world.resource::<Assets<Gltf>>().get(&root).unwrap();
        let meshes = world.resource::<Assets<GltfMesh>>();
        let primitive = &meshes.get(&gltf.meshes[0]).unwrap().primitives[0];
        validate_mesh(
            world
                .resource::<Assets<Mesh>>()
                .get(&primitive.mesh)
                .unwrap(),
        )
        .unwrap();
        let material = world
            .resource::<Assets<bevy::gltf::GltfMaterial>>()
            .get(primitive.material.as_ref().unwrap())
            .unwrap();
        let standard = bevy::pbr::gltf::standard_material_from_gltf_material(material);
        validate_material(&standard, world.resource::<Assets<Image>>()).unwrap();
    }
}
