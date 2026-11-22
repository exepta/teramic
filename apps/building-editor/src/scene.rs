use crate::document::{Document, MAXIMUM, MINIMUM, PLATFORM_Y};
use crate::workspace::{PLATFORM_MATERIAL, WorkspacePlatform};
use bevy::prelude::*;
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, SectionRemoved, WorldGeneration},
};

pub fn journal(document: &Document, platform: WorkspacePlatform) -> Result<EditJournal, String> {
    let mut journal = EditJournal::default();
    for z in -platform.half_size()..=platform.half_size() {
        for x in -platform.half_size()..=platform.half_size() {
            journal
                .record(
                    [
                        x + document.anchor[0],
                        PLATFORM_Y + document.anchor[1],
                        z + document.anchor[2],
                    ],
                    PLATFORM_MATERIAL,
                )
                .map_err(|e| e.to_string())?;
        }
    }
    for (&p, cell) in &document.cells {
        if !document.contains(p) {
            return Err(format!("Authored voxel outside the building bounds: {p:?}"));
        }
        journal
            .record(p, cell.material)
            .map_err(|e| e.to_string())?;
    }
    for (&p, cell) in &document.cells {
        if cell.state != Default::default() {
            journal.set_object_state(p, Some(cell.state))?;
        }
    }
    Ok(journal)
}
fn replace_terrain(world: &mut World, journal: EditJournal) {
    let old: Vec<_> = world
        .resource::<ChunkStore>()
        .loaded
        .keys()
        .copied()
        .collect();
    world.insert_resource(ChunkStore::default());
    world.insert_resource(journal);
    world.insert_resource(WorldGeneration::default());
    for position in old {
        world.write_message(SectionRemoved { position });
    }
}

/// Resizing changes the workspace, never the authored document or its camera.
pub fn resize_workspace(world: &mut World, size: u16) -> Result<(), String> {
    let platform = WorkspacePlatform::new(size)?;
    if world.resource::<WorkspacePlatform>() == &platform {
        return Ok(());
    }
    let journal = journal(world.resource::<Document>(), platform)?;
    replace_terrain(world, journal);
    world.insert_resource(platform);
    if let Some(mut config) = world.get_resource_mut::<terra_core::GameConfig>() {
        config.client.view_distance = platform.view_distance();
        config.client.vertical_view_distance = platform.view_distance();
    }
    if let Some(mut graphics) = world.get_resource_mut::<terra_core::GraphicsConfig>() {
        graphics.render_distance = platform.view_distance();
    }
    Ok(())
}

pub fn install_document(world: &mut World, document: Document) -> Result<(), String> {
    let platform = *world.resource::<WorkspacePlatform>();
    let journal = journal(&document, platform)?;
    replace_terrain(world, journal);
    {
        let mut editor = world.resource_mut::<crate::editor::Editor>();
        editor.name = document.name.clone();
        editor.filename = document
            .path
            .as_ref()
            .and_then(|path| path.file_stem())
            .and_then(|name| name.to_str())
            .filter(|name| terra_core::building_jobs::template::valid_template_id(name))
            .unwrap_or(&document.id)
            .to_owned();
        editor.pending_path = None;
        if let Some(parent) = document.path.as_ref().and_then(|path| path.parent()) {
            editor.directory = parent.into();
        }
    }
    let fov = world
        .get_resource::<terra_core::GameConfig>()
        .map_or(45.0_f32, |config| config.client.fov_degrees)
        .to_radians();
    let aspect = world
        .query::<&Window>()
        .iter(world)
        .next()
        .map_or(1.6, |window| window.width() / window.height().max(1.0));
    let transform = initial_camera(&document, fov, aspect);
    world.resource_mut::<terra_world::streaming::ChunkFocus>().0 =
        transform.translation.to_array().map(f64::from);
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        *camera = transform;
    }
    world.insert_resource(document);
    Ok(())
}
fn load_argument() -> Result<Option<std::path::PathBuf>, String> {
    let mut args = std::env::args_os().skip(1);
    let mut path = None;
    while let Some(argument) = args.next() {
        if argument == "--load" {
            if path.is_some() {
                return Err("--load can only be provided once".into());
            }
            path = Some(args.next().ok_or("--load requires a template path")?.into());
        }
    }
    Ok(path)
}

fn load_document(world: &mut World, path: std::path::PathBuf) -> Result<(), String> {
    let template = terra_core::building_jobs::template::BuildingTemplate::load(&path)?;
    install_document(world, Document::from_template(template, Some(path))?)
}

pub fn seed(world: &mut World) {
    let result = load_argument().and_then(|path| match path {
        Some(path) => load_document(world, path),
        None => install_document(world, Document::default()),
    });
    if let Err(error) = result {
        world.resource_mut::<crate::editor::Editor>().error = error;
    }
}

pub(crate) fn verify_package() -> Result<(), String> {
    let path = load_argument()?.ok_or("--verify-package requires --load PATH.json")?;
    let mut world = World::new();
    world.init_resource::<WorkspacePlatform>();
    world.init_resource::<ChunkStore>();
    world.init_resource::<crate::editor::Editor>();
    world.insert_resource(terra_world::streaming::ChunkFocus([0.0; 3]));
    load_document(&mut world, path)?;
    let document = world.resource::<Document>();
    let journal = world.resource::<EditJournal>();
    if document.cells.is_empty() {
        return Err("The installed template contains no authored cells".into());
    }
    for (&position, cell) in &document.cells {
        if journal.block(position) != Some(cell.material)
            || journal.object_state(position).unwrap_or_default() != cell.state
        {
            return Err(format!(
                "Installed template differs from the workspace at {position:?}"
            ));
        }
    }
    println!(
        "Package editor verified ({}: {} cells)",
        document.id,
        document.cells.len()
    );
    Ok(())
}
pub(super) fn document_bounds(document: &Document) -> (Vec3, Vec3) {
    let anchor = Vec3::from_array(document.anchor.map(|v| v as f32));
    let mut min = anchor + Vec3::new(-4.0, PLATFORM_Y as f32, -4.0);
    let mut max = anchor + Vec3::new(5.0, PLATFORM_Y as f32 + 1.0, 5.0);
    for position in document.cells.keys() {
        let position = Vec3::from_array(position.map(|v| v as f32));
        min = min.min(position);
        max = max.max(position + Vec3::ONE);
    }
    (min, max)
}
pub(super) fn initial_camera(document: &Document, fov: f32, aspect: f32) -> Transform {
    let (min, max) = document_bounds(document);
    let center = (min + max) * 0.5;
    let back = Vec3::new(10.0, 7.0, 13.0).normalize();
    let orientation = Transform::from_translation(center + back).looking_at(center, Vec3::Y);
    let right = *orientation.right();
    let up = *orientation.up();
    let tan_y = (fov * 0.5).tan();
    let tan_x = tan_y * aspect.max(0.1);
    let mut distance = 2.0_f32;
    // Fit every AABB corner into the real perspective frustum, leaving the
    // upper quarter for the editor HUD and a small margin around the building.
    for corner in 0..8 {
        let p = Vec3::new(
            if corner & 1 == 0 { min.x } else { max.x },
            if corner & 2 == 0 { min.y } else { max.y },
            if corner & 4 == 0 { min.z } else { max.z },
        ) - center;
        let z = p.dot(back);
        distance = distance
            .max(z + 2.0)
            .max(z + p.dot(right).abs() / (tan_x * 0.88))
            .max((p.dot(up) / tan_y + 0.55 * z) / (0.55 + 0.18))
            .max((-p.dot(up) / tan_y + 0.90 * z) / (0.90 - 0.18));
    }
    let target = center + up * (0.18 * distance * tan_y);
    Transform::from_translation(target + back * distance).looking_at(target, Vec3::Y)
}
pub fn camera(
    document: Res<Document>,
    config: Res<terra_core::GameConfig>,
    windows: Query<&Window>,
    mut cameras: Query<&mut Transform, With<terra_player::TerraCamera>>,
) {
    let aspect = windows
        .iter()
        .next()
        .map_or(1.6, |window| window.width() / window.height().max(1.0));
    for mut camera in &mut cameras {
        *camera = initial_camera(&document, config.client.fov_degrees.to_radians(), aspect);
    }
}
pub fn outline(
    mut gizmos: Gizmos,
    document: Res<Document>,
    mut cameras: Query<&mut Transform, With<terra_player::TerraCamera>>,
) {
    let min = Vec3::from_array(std::array::from_fn(|axis| {
        (MINIMUM[axis] + document.anchor[axis]) as f32
    }));
    let max = Vec3::from_array(std::array::from_fn(|axis| {
        (MAXIMUM[axis] + document.anchor[axis] + 1) as f32
    }));
    gizmos.cube(
        Transform::from_translation((min + max) * 0.5).with_scale(max - min),
        Color::srgba(0.79, 0.66, 0.32, 0.58),
    );
    let anchor = Vec3::from_array(document.anchor.map(|v| v as f32)) + Vec3::splat(0.5);
    gizmos.line(anchor, anchor + Vec3::X * 1.5, Color::srgb(1.0, 0.25, 0.2));
    gizmos.line(anchor, anchor + Vec3::Y * 1.5, Color::srgb(0.3, 1.0, 0.4));
    gizmos.line(anchor, anchor + Vec3::Z * 1.5, Color::srgb(0.3, 0.6, 1.0));
    for mut camera in &mut cameras {
        camera.translation = camera.translation.clamp(
            Vec3::from_array(document.anchor.map(|v| v as f32)) - Vec3::splat(128.0),
            Vec3::from_array(document.anchor.map(|v| v as f32)) + Vec3::splat(128.0),
        );
    }
}
