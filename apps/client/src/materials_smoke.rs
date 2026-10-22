//! Offline fixture using the real journal, section mesher, materials and shaders.
use bevy::{app::AppExit, ecs::system::SystemParam, prelude::*};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_player::{ControllerMode, TerraCamera};
use terra_render::{RenderBlockPreviews, RenderStatus, RenderedSections};
use terra_voxel::{MaterialId, global_to_section, meshing::mesh_section};
use terra_world::{
    edits::{BlockObjectState, EditJournal},
    streaming::{
        BlockEditRequest, ChunkFocus, ChunkStore, GenerationStatus, WorldGeneration, WorldTerrain,
    },
};

const COMPLETE_STAGE: u8 = 11;

#[derive(Resource)]
struct MaterialsSmoke {
    started: Instant,
    since: Instant,
    stage: u8,
    base_y: i64,
    captured: bool,
    oriented: bool,
    last_report: Instant,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-materials") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(MaterialsSmoke {
            started: now,
            since: now,
            stage: 0,
            base_y: 0,
            captured: false,
            oriented: false,
            last_report: now,
        })
        .add_systems(PostStartup, setup)
        .add_systems(Update, run);
}

fn setup(
    mut state: ResMut<MaterialsSmoke>,
    terrain: Res<WorldTerrain>,
    mut focus: ResMut<ChunkFocus>,
    mut camera: Query<&mut Transform, With<TerraCamera>>,
    mut gizmos: ResMut<bevy::gizmos::config::GizmoConfigStore>,
    mut config: ResMut<terra_core::GameConfig>,
) {
    config.client.fov_degrees = 45.0;
    // Material review must not mistake the hovered-cell outline for a frame.
    gizmos
        .config_mut::<bevy::gizmos::config::DefaultGizmoConfigGroup>()
        .0
        .enabled = false;
    state.base_y = ((terrain.0.surface_height(16.0, 12.0) / 16.0).ceil() as i64 + 2) * 16 - 1;
    let y = state.base_y as f32;
    let center_z = (2.0 + platform_end_z() as f32) * 0.5;
    focus.0 = [14.0, f64::from(y + 3.0), f64::from(center_z)];
    for mut transform in &mut camera {
        *transform = Transform::from_xyz(25.0, y + 12.0, center_z + 20.0)
            .looking_at(Vec3::new(13.0, y + 0.7, center_z), Vec3::Y);
    }
}

#[derive(SystemParam)]
struct FixtureWorld<'w> {
    chunks: ResMut<'w, ChunkStore>,
    generation: Res<'w, WorldGeneration>,
    render: Res<'w, RenderStatus>,
    rendered: Res<'w, RenderedSections>,
    journal: ResMut<'w, EditJournal>,
    previews: Res<'w, RenderBlockPreviews>,
    images: Res<'w, Assets<Image>>,
}

fn sample_position(material: u16, y: i64) -> [i64; 3] {
    let index = i64::from(material - 1);
    [8 + index % 5 * 2, y, 8 + index / 5 * 2]
}

fn platform_end_z() -> i64 {
    (sample_position(MaterialId::LAST, 0)[2] + 2).max(54)
}

fn construction_facing(material: u16) -> Option<u8> {
    match material {
        74..=81 => Some([0, 4, 8, 12, 16, 20, 1, 3][usize::from(material - 74)]),
        82..=89 => Some([0, 4, 5, 6, 7, 8, 9, 0][usize::from(material - 82)]),
        98 => Some(1),
        _ => None,
    }
}

fn scene_cells(y: i64) -> Vec<([i64; 3], u16)> {
    let mut cells = Vec::new();
    for z in 2..=platform_end_z() {
        for x in 6..=19 {
            cells.push(([x, y - 1, z], MaterialId::SmoothStone as u16));
            for height in 0..=3 {
                cells.push(([x, y + height, z], 0));
            }
        }
    }
    for material in 1..=MaterialId::LAST {
        let position = sample_position(material, y);
        if let Some((_, value)) = cells.iter_mut().find(|(cell, _)| *cell == position) {
            *value = material;
        }
        if matches!(material, 54 | 55)
            && let Some((_, value)) = cells
                .iter_mut()
                .find(|(cell, _)| *cell == [position[0], position[1] + 1, position[2]])
        {
            *value = MaterialId::Stone as u16;
        }
    }
    for (i, state) in (1..=8).enumerate() {
        for (position, material) in [
            ([8 + i as i64, y - 1, 52], 1),
            ([8 + i as i64, y, 52], if state == 8 { 5 } else { 98 }),
        ] {
            if let Some((_, id)) = cells.iter_mut().find(|(p, _)| *p == position) {
                *id = material;
            }
        }
    }
    for x in 8..14 {
        if let Some((_, id)) = cells.iter_mut().find(|(p, _)| *p == [x, y, 49]) {
            *id = 82;
        }
    }
    for x in [15, 16] {
        for height in [0, 1] {
            if let Some((_, value)) = cells
                .iter_mut()
                .find(|(cell, _)| *cell == [x, y + height, 5])
            {
                *value = MaterialId::Glass as u16;
            }
        }
    }
    for x in [15, 16] {
        for height in [0, 1] {
            if let Some((_, value)) = cells
                .iter_mut()
                .find(|(cell, _)| *cell == [x, y + height, 3])
            {
                *value = if (x + height) % 2 == 0 {
                    MaterialId::BirchPlanks as u16
                } else {
                    MaterialId::SprucePlanks as u16
                };
            }
        }
    }
    // Rooted hanging segments must be placed after their actual ceiling support.
    cells.sort_by_key(|(_, material)| matches!(material, 54 | 55));
    cells
}

fn check_window(world: &FixtureWorld, y: i64) -> Result<(), String> {
    let mut geometry = HashMap::new();
    for x in [15, 16] {
        for height in [0, 1] {
            let position = [x, y + height, 5];
            let (owner, local) = global_to_section(position);
            // The renderer moves LoadedSection.mesh into its geometry cache.
            // Re-mesh this tiny fixture's current padded cells, after upload
            // epochs match, rather than incorrectly requiring the consumed mesh.
            let mesh = match geometry.entry(owner) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let origin = owner.origin().map_err(|error| error.to_string())?;
                    let mut padded = Vec::with_capacity(18 * 18 * 18);
                    for z in -1..=16 {
                        for y in -1..=16 {
                            for x in -1..=16 {
                                let global = [origin[0] + x, origin[1] + y, origin[2] + z];
                                padded.push(world.chunks.block_at(global).ok_or_else(|| {
                                    format!("Missing window padding at {global:?}")
                                })?);
                            }
                        }
                    }
                    entry.insert(mesh_section(&padded).map_err(|error| error.to_string())?)
                }
            };
            let Some(face) = mesh.faces.iter().find(|face| {
                face.minimum == local.map(|v| v as u32)
                    && face.side == 5
                    && face.material == MaterialId::Glass as u16
            }) else {
                return Err(format!("Missing window front face at {position:?}"));
            };
            if face.width != 1 || face.height != 1 || face.edge_mask.count_ones() != 2 {
                return Err(format!("Unexpected window frame at {position:?}: {face:?}"));
            }
        }
    }
    Ok(())
}

fn run(
    mut commands: Commands,
    mut state: ResMut<MaterialsSmoke>,
    mut world: FixtureWorld,
    mut edits: MessageWriter<BlockEditRequest>,
    mut exit: MessageWriter<AppExit>,
    mut camera: Query<&mut Transform, With<TerraCamera>>,
) {
    if state.stage == COMPLETE_STAGE {
        return;
    }
    if state.last_report.elapsed() > Duration::from_secs(10) {
        state.last_report = Instant::now();
        let stale: Vec<_> = (1..=MaterialId::LAST)
            .filter_map(|id| {
                let p = sample_position(id, state.base_y);
                let owner = global_to_section(p).0;
                let block = world.chunks.block_at(p);
                let rendered = world.rendered.sections.get(&owner).map(|s| s.mesh_epoch);
                let epoch = world.journal.mesh_epoch(owner);
                (block != Some(id) || rendered != Some(epoch))
                    .then_some((p, id, block, rendered, epoch))
            })
            .take(12)
            .collect();
        info!(stage = state.stage, generation = ?world.generation.status,
            generation_jobs = world.generation.pending_jobs,
            render_ready = world.render.ready, render_jobs = world.render.pending_jobs,
            preview_pending = world.previews.pending, stale = ?stale,
            "Materials fixture readiness");
    }
    if state.started.elapsed() > Duration::from_secs(120)
        || world.render.error.is_some()
        || world.previews.error.is_some()
    {
        error!(stage = state.stage, render_error = ?world.render.error,
            preview_error = ?world.previews.error, "Materials fixture failed or timed out");
        state.stage = COMPLETE_STAGE;
        exit.write(AppExit::error());
        return;
    }
    if world.generation.status != GenerationStatus::Ready || !world.render.ready {
        return;
    }
    // Each published preview has already passed the renderer's GPU pixel readback.
    if world.previews.pending != 0
        || (1..=MaterialId::LAST).any(|id| {
            world
                .previews
                .images
                .get(&id)
                .is_none_or(|handle| !world.images.contains(handle.id()))
        })
    {
        return;
    }
    let y = state.base_y;
    if state.stage == 0 {
        let cells = scene_cells(y);
        if cells
            .iter()
            .any(|(position, _)| world.chunks.block_at(*position).is_none())
        {
            return;
        }
        for (position, material) in cells {
            if let Some(expected_material) = world.chunks.block_at(position)
                && expected_material != material
            {
                edits.write(BlockEditRequest {
                    position,
                    expected_material,
                    material,
                });
            }
        }
        state.stage = 1;
        state.since = Instant::now();
        return;
    }
    if state.since.elapsed() < Duration::from_secs(3) {
        return;
    }
    let samples: Vec<_> =
        (1..=MaterialId::LAST)
            .map(|material| (sample_position(material, y), material))
            .chain([15, 16].into_iter().flat_map(|x| {
                [0, 1].map(move |height| ([x, y + height, 5], MaterialId::Glass as u16))
            }))
            .collect();
    if samples.iter().any(|(position, material)| {
        let owner = global_to_section(*position).0;
        world.chunks.block_at(*position) != Some(*material)
            || world
                .rendered
                .sections
                .get(&owner)
                .is_none_or(|section| section.mesh_epoch != world.journal.mesh_epoch(owner))
    }) {
        return;
    }
    if !state.oriented {
        for (position, material) in &samples {
            if let Some(facing) = construction_facing(*material)
                && let Err(error) = terra_world::streaming::commit_object_state(
                    &mut world.chunks,
                    &mut world.journal,
                    *position,
                    Some(BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )
            {
                error!(%error, "Construction fixture pose failed");
                state.stage = COMPLETE_STAGE;
                exit.write(AppExit::error());
                return;
            }
        }
        for (i, facing) in [0, 4, 5, 6, 7, 8].into_iter().enumerate() {
            if let Err(error) = terra_world::streaming::commit_object_state(
                &mut world.chunks,
                &mut world.journal,
                [8 + i as i64, y, 49],
                Some(BlockObjectState {
                    facing: terra_core::construction::merged_slab(82, facing, 85).unwrap_or(9),
                    ..Default::default()
                }),
            ) {
                error!(%error, "Mixed slab fixture failed");
                exit.write(AppExit::error());
                return;
            }
        }
        for layer in 1..=8 {
            let p = [7 + i64::from(layer), y, 52];
            if let Err(error) = terra_world::streaming::commit_object_state(
                &mut world.chunks,
                &mut world.journal,
                p,
                Some(BlockObjectState {
                    facing: layer,
                    ..Default::default()
                }),
            ) {
                error!(%error, "Snow fixture failed");
                exit.write(AppExit::error());
                return;
            }
            if let Err(error) = world.journal.set_surface_condition(
                [p[0], y - 1, p[2]],
                terra_world::environment::SurfaceCondition {
                    snow: f32::from(layer) / 8.0,
                    ..Default::default()
                },
            ) {
                error!(%error, "Snow support fixture failed");
                exit.write(AppExit::error());
                return;
            }
        }
        state.oriented = true;
        state.since = Instant::now();
        return;
    }
    if !state.captured
        && let Err(error) = check_window(&world, y)
    {
        error!(%error, "Materials fixture window validation failed");
        state.stage = COMPLETE_STAGE;
        exit.write(AppExit::error());
        return;
    }
    if !state.captured {
        let label = match state.stage {
            1 => "all-blocks",
            2 => "joined-glass",
            3 => "solid-water",
            4 => "mushroom-blocks",
            5 => "reference-terrain",
            6 => "reference-foliage",
            7 => "gravel-structure-stone",
            8 => "construction-orientations",
            9 => "snow-layers",
            _ => "mixed-slabs",
        };
        let path = std::env::var_os("TERRA_SCREENSHOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp/terra-materials.png"));
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("materials");
        let path = path.with_file_name(format!("{stem}-{label}.png"));
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(path));
        info!(
            label,
            blocks = MaterialId::BLOCK_COUNT,
            previews = world.previews.images.len(),
            "Materials fixture captured: registered blocks and joined glass across X=16"
        );
        state.captured = true;
    }
    if state.since.elapsed() < Duration::from_secs(4) {
        return;
    }
    state.stage += 1;
    state.captured = false;
    state.since = Instant::now();
    let y = y as f32;
    for mut transform in &mut camera {
        *transform = match state.stage {
            2 => Transform::from_xyz(18.5, y + 2.5, 9.0)
                .looking_at(Vec3::new(16.0, y + 1.0, 5.5), Vec3::Y),
            3 => Transform::from_xyz(17.6, y + 3.0, 16.6)
                .looking_at(Vec3::new(14.5, y + 0.5, 12.5), Vec3::Y),
            4 => Transform::from_xyz(20.0, y + 6.0, 23.0)
                .looking_at(Vec3::new(13.0, y + 0.5, 16.5), Vec3::Y),
            5 => Transform::from_xyz(12.0, y + 6.0, 0.0)
                .looking_at(Vec3::new(12.0, y + 0.5, 9.0), Vec3::Y),
            6 => Transform::from_xyz(15.5, y + 2.7, 13.0)
                .looking_at(Vec3::new(12.5, y + 0.5, 10.5), Vec3::Y),
            7 => Transform::from_xyz(15.0, y + 2.8, 28.0)
                .looking_at(Vec3::new(11.5, y + 0.5, 24.5), Vec3::Y),
            8 => Transform::from_xyz(22.0, y + 6.5, 29.0)
                .looking_at(Vec3::new(12.0, y + 0.65, 39.0), Vec3::Y),
            9 => Transform::from_xyz(11.5, y + 3.0, 46.0)
                .looking_at(Vec3::new(11.5, y + 0.3, 52.0), Vec3::Y),
            10 => Transform::from_xyz(11.0, y + 3.0, 44.0)
                .looking_at(Vec3::new(10.5, y + 0.45, 49.0), Vec3::Y),
            _ => *transform,
        };
    }
    if state.stage == COMPLETE_STAGE {
        info!(
            "Materials fixture passed: all blocks and GPU previews ready, connected glass has no internal frames, solid water and mushroom palette rendered"
        );
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_fixture_places_every_registered_id_on_its_platform() {
        let cells: HashMap<_, _> = scene_cells(63).into_iter().collect();
        for material in 1..=MaterialId::LAST {
            let position = sample_position(material, 63);
            assert_eq!(cells.get(&position), Some(&material));
            assert_eq!(
                cells.get(&[position[0], 62, position[2]]),
                Some(&(MaterialId::SmoothStone as u16))
            );
        }
    }
}
