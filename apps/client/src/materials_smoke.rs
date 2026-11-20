//! Offline fixture using the real journal, section mesher, materials and shaders.
use bevy::{app::AppExit, ecs::system::SystemParam, prelude::*};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_player::{ControllerMode, TerraCamera};
use terra_render::{ChunkBorderGizmos, RenderBlockPreviews, RenderStatus, RenderedSections};
use terra_voxel::{MaterialId, global_to_section, meshing::mesh_section};
use terra_world::{
    edits::{BlockObjectState, EditJournal},
    streaming::{
        BlockEditRequest, ChunkFocus, ChunkStore, GenerationStatus, WorldGeneration, WorldTerrain,
    },
};

#[path = "materials_smoke_bodies.rs"]
mod bodies;
#[path = "materials_smoke_castle.rs"]
mod castle;
#[path = "materials_smoke_flow.rs"]
mod flow;
#[path = "fog_block_smoke.rs"]
mod fog_block;
#[path = "materials_smoke_foliage.rs"]
mod foliage;
#[path = "materials_smoke_particles.rs"]
mod particles;
#[path = "materials_smoke_reference.rs"]
mod reference_palette;
#[path = "materials_smoke_roads.rs"]
mod roads;
#[path = "materials_smoke_tar.rs"]
mod tar;

const COMPLETE_STAGE: u8 = 16;
const SETUP_TIMEOUT: Duration = Duration::from_secs(120);
const CAPTURE_INTERVAL_SECONDS: u64 = 4;
const CAPTURE_TIMEOUT: Duration =
    Duration::from_secs((COMPLETE_STAGE as u64 - 1) * CAPTURE_INTERVAL_SECONDS + 30);

fn timed_out(setup_elapsed: Duration, capture_elapsed: Option<Duration>) -> bool {
    match capture_elapsed {
        Some(elapsed) => elapsed > CAPTURE_TIMEOUT,
        None => setup_elapsed > SETUP_TIMEOUT,
    }
}

#[derive(Resource)]
struct MaterialsSmoke {
    castle: bool,
    started: Instant,
    capture_started: Option<Instant>,
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
    if std::env::var_os("TERRA_FLOW_INSPECT").is_some() {
        flow::install(app);
        return;
    }
    if std::env::var_os("TERRA_WATER_BODIES_INSPECT").is_some() {
        bodies::install(app);
        return;
    }
    if std::env::var_os("TERRA_PARTICLE_INSPECT").is_some() {
        particles::install(app);
        return;
    }
    if std::env::var_os("TERRA_FOG_BLOCK_INSPECT").is_some() {
        fog_block::install(app);
        return;
    }
    if std::env::var_os("TERRA_FOLIAGE_INSPECT").is_some() {
        foliage::install(app);
        return;
    }
    if std::env::var_os("TERRA_ROADS_INSPECT").is_some() {
        roads::install(app);
        return;
    }
    if std::env::var_os("TERRA_REFERENCE_PALETTE_INSPECT").is_some() {
        reference_palette::install(app);
        return;
    }
    if std::env::var_os("TERRA_TAR_INSPECT").is_some() {
        tar::install(app);
        return;
    }
    let now = Instant::now();
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(MaterialsSmoke {
            castle: std::env::var("TERRA_MATERIALS_CASTLE").is_ok_and(|value| value == "1"),
            started: now,
            capture_started: None,
            since: now,
            stage: 0,
            base_y: 0,
            captured: false,
            oriented: false,
            last_report: now,
        })
        .add_systems(PostStartup, setup)
        .add_systems(Update, (run, show_borders).chain());
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
    gizmos.config_mut::<ChunkBorderGizmos>().0.enabled = false;
    state.base_y = ((terrain.0.surface_height(16.0, 12.0) / 16.0).ceil() as i64 + 2) * 16 - 1;
    let y = state.base_y as f32;
    let center_z = (2.0 + platform_end_z() as f32) * 0.5;
    focus.0 = [14.0, f64::from(y + 3.0), f64::from(center_z)];
    for mut transform in &mut camera {
        *transform = if state.castle {
            castle::camera(y, 1)
        } else {
            overview_camera(y)
        };
    }
    if state.castle {
        config.client.fov_degrees = 50.0;
        focus.0 = [13.0, f64::from(y + 3.0), 14.0];
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
    winter_shape_row_z() + 2
}

fn mixed_slab_row_z() -> i64 {
    sample_position(MaterialId::LAST, 0)[2] + 3
}

fn snow_row_z() -> i64 {
    mixed_slab_row_z() + 3
}

fn pot_row_z() -> i64 {
    snow_row_z() + 3
}

fn winter_shape_row_z() -> i64 {
    pot_row_z() + 4
}

fn winter_shape_samples(y: i64) -> [([i64; 3], u16, u8); 7] {
    let materials = [78, 74, 86, 82, 90, 93, MaterialId::Path as u16];
    std::array::from_fn(|i| ([7 + i as i64 * 2, y, winter_shape_row_z()], materials[i], 0))
}

fn winter_shape_camera(y: f32) -> Transform {
    Transform::from_xyz(21.5, y + 4.5, winter_shape_row_z() as f32 - 9.5).looking_at(
        Vec3::new(13.5, y + 0.45, winter_shape_row_z() as f32 + 0.5),
        Vec3::Y,
    )
}

fn potted_samples(y: i64) -> [([i64; 3], u8); 4] {
    use terra_core::{ItemKind, ResourcePropKind, beds::DyeColor};
    let plants = [
        ItemKind::Flower(DyeColor::White),
        ItemKind::Flower(DyeColor::Red),
        ItemKind::Flower(DyeColor::Blue),
        ItemKind::Prop(ResourcePropKind::Mushroom),
    ];
    std::array::from_fn(|i| {
        (
            [8 + i as i64 * 2, y + 1, pot_row_z()],
            terra_core::flower_pot::state(plants[i]).unwrap(),
        )
    })
}

fn textile_samples(y: i64) -> [([i64; 3], u16); 8] {
    use terra_core::{beds::DyeColor, textiles};
    let colors = [
        DyeColor::White,
        DyeColor::Red,
        DyeColor::Blue,
        DyeColor::White,
    ];
    std::array::from_fn(|i| {
        let carpet = i >= colors.len();
        let index = i % colors.len();
        let base = if carpet {
            textiles::WHITE_CARPET
        } else {
            textiles::WHITE_WOOL
        };
        (
            [8 + index as i64 * 2, y, pot_row_z() - i64::from(carpet)],
            textiles::dyed(base, colors[index]).unwrap(),
        )
    })
}

fn pot_camera(y: f32) -> Transform {
    Transform::from_xyz(18.0, y + 6.0, pot_row_z() as f32 + 10.0)
        .looking_at(Vec3::new(11.5, y + 1.0, pot_row_z() as f32), Vec3::Y)
}

fn overview_camera(y: f32) -> Transform {
    let center_z = (3.0 + platform_end_z() as f32) * 0.5;
    let half_depth = (platform_end_z() as f32 - 1.0) * 0.5;
    let radius = Vec3::new(7.0, 2.5, half_depth).length();
    let distance = radius / (45.0_f32.to_radians() * 0.5).sin() * 1.05;
    let target = Vec3::new(13.0, y + 1.5, center_z);
    Transform::from_translation(target + Vec3::new(0.35, 0.8, 0.65).normalize() * distance)
        .looking_at(target, Vec3::Y)
}

fn construction_facing(material: u16) -> Option<u8> {
    match material {
        74..=81 => Some([0, 4, 8, 12, 16, 20, 1, 3][usize::from(material - 74)]),
        82..=89 => Some([0, 4, 5, 6, 7, 8, 9, 0][usize::from(material - 82)]),
        98 => Some(1),
        104 => Some(0),
        105 => Some(0),
        _ => None,
    }
}

fn opening_states(y: i64) -> Vec<([i64; 3], u16, BlockObjectState)> {
    let mut result = Vec::new();
    for material in (1..=MaterialId::LAST).filter(|&id| terra_world::objects::is_opening(id)) {
        let position = sample_position(material, y);
        if terra_world::objects::is_door(material) {
            let upper = [position[0], y + 1, position[2]];
            for (position, peer, secondary) in [(position, upper, false), (upper, position, true)] {
                result.push((
                    position,
                    material,
                    BlockObjectState {
                        peer: Some(peer),
                        secondary,
                        ..default()
                    },
                ));
            }
        } else {
            result.push((position, material, BlockObjectState::default()));
        }
    }
    result
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
    for (position, material) in opening_states(y)
        .into_iter()
        .map(|(p, id, _)| (p, id))
        .chain(textile_samples(y))
        .chain(potted_samples(y).map(|(position, _)| (position, terra_core::flower_pot::MATERIAL)))
        .chain(winter_shape_samples(y).map(|(position, material, _)| (position, material)))
    {
        if let Some((_, id)) = cells.iter_mut().find(|(p, _)| *p == position) {
            *id = material;
        }
    }
    for (i, state) in (1..=8).enumerate() {
        for (position, material) in [
            ([8 + i as i64, y - 1, snow_row_z()], 1),
            (
                [8 + i as i64, y, snow_row_z()],
                if state == 8 { 5 } else { 98 },
            ),
        ] {
            if let Some((_, id)) = cells.iter_mut().find(|(p, _)| *p == position) {
                *id = material;
            }
        }
    }
    for x in 8..14 {
        if let Some((_, id)) = cells
            .iter_mut()
            .find(|(p, _)| *p == [x, y, mixed_slab_row_z()])
        {
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

fn check_winter_shapes(chunks: &ChunkStore, journal: &EditJournal, y: i64) -> Result<(), String> {
    for (position, material, facing) in winter_shape_samples(y) {
        if chunks.block_at(position) != Some(material)
            || chunks.block_at([position[0], y - 1, position[2]])
                != Some(MaterialId::SmoothStone as u16)
        {
            return Err(format!(
                "Winter shape is not on its real platform at {position:?}"
            ));
        }
        if journal.surface_condition(position).snow <= 0.0 {
            return Err(format!(
                "Winter shape lost its saved snowfall at {position:?}"
            ));
        }
        if terra_core::construction::is_shape(material) {
            if journal
                .object_state(position)
                .is_none_or(|state| state.facing != facing || !state.valid_for(position, material))
            {
                return Err(format!("Winter shape has an invalid pose at {position:?}"));
            }
            let bounds = terra_core::construction::placed_boxes(material, facing, [None; 6]);
            if bounds.is_empty()
                || bounds.iter().any(|bounds| {
                    bounds.minimum.iter().any(|value| !value.is_finite())
                        || bounds.maximum.iter().any(|value| !value.is_finite())
                        || bounds.minimum[1] < 0.0
                        || bounds.maximum[1] > 1.5
                })
            {
                return Err(format!(
                    "Winter shape has invalid authored geometry at {position:?}"
                ));
            }
        }
        for height in 1..=3 {
            let above = [position[0], y + height, position[2]];
            if chunks.block_at(above) != Some(0) {
                return Err(format!(
                    "Winter shape has an unexpected cap or roof at {above:?}"
                ));
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
    if state.castle {
        castle::run(
            &mut commands,
            &mut state,
            &mut world,
            &mut edits,
            &mut exit,
            &mut camera,
        );
        return;
    }
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
    if timed_out(
        state.started.elapsed(),
        state.capture_started.map(|start| start.elapsed()),
    ) || world.render.error.is_some()
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
    let samples: Vec<_> = (1..=MaterialId::LAST)
        .map(|material| (sample_position(material, y), material))
        .chain(
            [15, 16].into_iter().flat_map(|x| {
                [0, 1].map(move |height| ([x, y + height, 5], MaterialId::Glass as u16))
            }),
        )
        .chain(textile_samples(y))
        .chain(potted_samples(y).map(|(position, _)| (position, terra_core::flower_pot::MATERIAL)))
        .chain(winter_shape_samples(y).map(|(position, material, _)| (position, material)))
        .collect();
    // Upper door cells are occupied, but their shared model belongs to the
    // lower anchor; an otherwise empty upper section need not own terrain mesh.
    if opening_states(y)
        .iter()
        .any(|(position, material, _)| world.chunks.block_at(*position) != Some(*material))
    {
        return;
    }
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
        for (position, _, object) in opening_states(y) {
            if let Err(error) = terra_world::streaming::commit_object_state(
                &mut world.chunks,
                &mut world.journal,
                position,
                Some(object),
            ) {
                error!(%error, "Opening fixture state failed");
                state.stage = COMPLETE_STAGE;
                exit.write(AppExit::error());
                return;
            }
        }
        for (position, material) in &samples {
            if position[2] == winter_shape_row_z() {
                continue;
            }
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
                [8 + i as i64, y, mixed_slab_row_z()],
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
            let p = [7 + i64::from(layer), y, snow_row_z()];
            if let Err(error) = terra_world::streaming::commit_object_state(
                &mut world.chunks,
                &mut world.journal,
                p,
                terra_core::construction::manual_snow_facing(layer).map(|facing| {
                    BlockObjectState {
                        facing,
                        ..Default::default()
                    }
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
        for (position, facing) in potted_samples(y) {
            if let Err(error) = terra_world::streaming::commit_object_state(
                &mut world.chunks,
                &mut world.journal,
                position,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            ) {
                error!(%error, "Potted flower fixture failed");
                state.stage = COMPLETE_STAGE;
                exit.write(AppExit::error());
                return;
            }
        }
        for (position, material, facing) in winter_shape_samples(y) {
            if terra_core::construction::is_shape(material)
                && let Err(error) = terra_world::streaming::commit_object_state(
                    &mut world.chunks,
                    &mut world.journal,
                    position,
                    Some(BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )
            {
                error!(%error, "Winter shape fixture pose failed");
                state.stage = COMPLETE_STAGE;
                exit.write(AppExit::error());
                return;
            }
            if let Err(error) = world.journal.set_surface_condition(
                position,
                terra_world::environment::SurfaceCondition {
                    snow: 0.875,
                    ..Default::default()
                },
            ) {
                error!(%error, "Winter shape fixture snowfall failed");
                state.stage = COMPLETE_STAGE;
                exit.write(AppExit::error());
                return;
            }
        }
        state.oriented = true;
        let now = Instant::now();
        // Initial GPU previews have their own bounded setup budget; they must
        // not consume the time reserved for the complete screenshot sequence.
        state.capture_started = Some(now);
        state.since = now;
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
    if potted_samples(y).into_iter().any(|(position, facing)| {
        world.journal.object_state(position).is_none_or(|state| {
            state.facing != facing || !state.valid_for(position, terra_core::flower_pot::MATERIAL)
        })
    }) {
        error!("Potted flower fixture lost its real journal contents");
        state.stage = COMPLETE_STAGE;
        exit.write(AppExit::error());
        return;
    }
    if !state.captured {
        if state.stage == 15
            && let Err(error) = check_winter_shapes(&world.chunks, &world.journal, y)
        {
            error!(%error, "Winter shape fixture validation failed");
            state.stage = COMPLETE_STAGE;
            exit.write(AppExit::error());
            return;
        }
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
            10 => "mixed-slabs",
            11 => "village-building-blocks",
            13 => "textiles-flower-pots",
            14 => "cherry-wood",
            15 => "snow-shapes",
            _ => "chunk-borders",
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
    if state.since.elapsed() < Duration::from_secs(CAPTURE_INTERVAL_SECONDS) {
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
            9 => Transform::from_xyz(11.5, y + 3.0, snow_row_z() as f32 - 6.0)
                .looking_at(Vec3::new(11.5, y + 0.3, snow_row_z() as f32), Vec3::Y),
            10 => Transform::from_xyz(11.0, y + 3.0, mixed_slab_row_z() as f32 - 5.0).looking_at(
                Vec3::new(10.5, y + 0.45, mixed_slab_row_z() as f32),
                Vec3::Y,
            ),
            11 => Transform::from_xyz(21.0, y + 3.0, 56.0)
                .looking_at(Vec3::new(13.5, y + 0.45, 48.0), Vec3::Y),
            12 => Transform::from_xyz(35.0, y + 16.0, 60.0)
                .looking_at(Vec3::new(16.0, y + 0.5, 43.0), Vec3::Y),
            13 => pot_camera(y),
            14 => {
                let p = sample_position(137, state.base_y).map(|v| v as f32);
                Transform::from_xyz(p[0] + 8.0, y + 4.0, p[2] + 9.0)
                    .looking_at(Vec3::new(p[0], y + 0.7, p[2] - 1.0), Vec3::Y)
            }
            15 => winter_shape_camera(y),
            _ => *transform,
        };
    }
    if state.stage == COMPLETE_STAGE {
        info!(
            "Materials fixture passed: all blocks and GPU previews ready, connected glass, solid water, textiles, filled pots and surface-only winter shapes rendered"
        );
        exit.write(AppExit::Success);
    }
}

fn show_borders(
    state: Res<MaterialsSmoke>,
    mut debug: ResMut<terra_core::DebugSettings>,
    mut gizmos: ResMut<bevy::gizmos::config::GizmoConfigStore>,
) {
    let visible = state.stage == 12;
    debug.chunk_borders = visible;
    gizmos.config_mut::<ChunkBorderGizmos>().0.enabled = visible;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_preview_setup_does_not_consume_the_bounded_full_capture_sequence() {
        let setup = SETUP_TIMEOUT - Duration::from_secs(1);
        assert!(!timed_out(setup, None));
        for stage in 1..COMPLETE_STAGE {
            let capture_elapsed = Duration::from_secs(u64::from(stage) * CAPTURE_INTERVAL_SECONDS);
            assert!(!timed_out(setup + capture_elapsed, Some(capture_elapsed)));
        }
        assert!(timed_out(SETUP_TIMEOUT + Duration::from_secs(1), None));
        assert!(!timed_out(setup + CAPTURE_TIMEOUT, Some(CAPTURE_TIMEOUT)));
        let expired = CAPTURE_TIMEOUT + Duration::from_secs(1);
        assert!(timed_out(setup + expired, Some(expired)));
    }

    #[test]
    fn winter_shapes_follow_the_pots_with_real_saved_snow_and_no_occupied_cap_cells() {
        use terra_world::streaming::LoadedSection;
        let y = 63;
        let mut chunks = ChunkStore::default();
        for (position, material) in scene_cells(y) {
            let (section, local) = global_to_section(position);
            chunks
                .loaded
                .entry(section)
                .or_insert_with(|| LoadedSection {
                    data: terra_voxel::BlockSection::empty(section).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                })
                .data
                .set_block(local, material)
                .unwrap();
        }
        let mut journal = EditJournal::default();
        assert!(winter_shape_row_z() > pot_row_z());
        assert!(winter_shape_row_z() < platform_end_z());
        let camera = winter_shape_camera(y as f32);
        let forward = camera.rotation * Vec3::NEG_Z;
        let right = camera.rotation * Vec3::X;
        let up = camera.rotation * Vec3::Y;
        assert!(camera.translation.z < winter_shape_row_z() as f32 && forward.z > 0.0);
        for (position, material, facing) in winter_shape_samples(y) {
            assert!(!terra_world::weather::supports_snow_layers(material));
            journal.record(position, material).unwrap();
            if terra_core::construction::is_shape(material) {
                journal
                    .set_object_state(
                        position,
                        Some(BlockObjectState {
                            facing,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
            }
            journal
                .set_surface_condition(
                    position,
                    terra_world::environment::SurfaceCondition {
                        snow: 0.875,
                        ..Default::default()
                    },
                )
                .unwrap();
            let relative = Vec3::from_array(position.map(|v| v as f32)) + Vec3::splat(0.5)
                - camera.translation;
            let depth = relative.dot(forward);
            assert!(depth > 0.0);
            assert!(relative.dot(right).abs() / depth < (45.0_f32.to_radians() * 0.5).tan() * 1.6);
            assert!(relative.dot(up).abs() / depth < (45.0_f32.to_radians() * 0.5).tan());
        }
        check_winter_shapes(&chunks, &journal, y).unwrap();
        let above = [7, y + 1, winter_shape_row_z()];
        let (section, local) = global_to_section(above);
        for forbidden in [98, 5] {
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block(local, forbidden)
                .unwrap();
            assert!(check_winter_shapes(&chunks, &journal, y).is_err());
        }
    }

    #[test]
    fn chunk_border_capture_enables_only_its_dedicated_group_and_cleans_up_afterward() {
        use bevy::{
            ecs::system::RunSystemOnce,
            gizmos::{
                AppGizmoBuilder, GizmoAsset,
                config::{DefaultGizmoConfigGroup, GizmoConfigStore},
            },
        };
        let mut app = App::new();
        app.init_resource::<Assets<GizmoAsset>>()
            .init_gizmo_group::<DefaultGizmoConfigGroup>()
            .init_gizmo_group::<ChunkBorderGizmos>()
            .init_resource::<terra_core::DebugSettings>()
            .insert_resource(MaterialsSmoke {
                castle: false,
                started: Instant::now(),
                capture_started: None,
                since: Instant::now(),
                stage: 11,
                base_y: 0,
                captured: false,
                oriented: false,
                last_report: Instant::now(),
            });
        app.world_mut()
            .resource_mut::<GizmoConfigStore>()
            .config_mut::<DefaultGizmoConfigGroup>()
            .0
            .enabled = false;
        for stage in [11, 12, 13, COMPLETE_STAGE] {
            app.world_mut().resource_mut::<MaterialsSmoke>().stage = stage;
            app.world_mut().run_system_once(show_borders).unwrap();
            let configs = app.world().resource::<GizmoConfigStore>();
            assert!(!configs.config::<DefaultGizmoConfigGroup>().0.enabled);
            assert_eq!(configs.config::<ChunkBorderGizmos>().0.enabled, stage == 12);
            assert_eq!(
                app.world()
                    .resource::<terra_core::DebugSettings>()
                    .chunk_borders,
                stage == 12
            );
        }
    }

    #[test]
    fn material_gallery_uses_real_opening_states_and_both_door_halves() {
        let y = 63;
        let cells: HashMap<_, _> = scene_cells(y).into_iter().collect();
        let mut journal = EditJournal::default();
        for (position, material, _) in opening_states(y) {
            assert_eq!(cells.get(&position), Some(&material));
            journal.record(position, material).unwrap();
        }
        for (position, material, object) in opening_states(y) {
            assert!(object.valid_for(position, material));
            journal.set_object_state(position, Some(object)).unwrap();
        }
        for material in [139, 140] {
            let position = sample_position(material, y);
            let object = journal.object_state(position).unwrap();
            if terra_world::objects::is_door(material) {
                let peer = object.peer.unwrap();
                assert_eq!(journal.object_state(peer).unwrap().peer, Some(position));
                assert!(journal.object_state(peer).unwrap().secondary);
            }
        }
    }

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

    #[test]
    fn specialty_rows_follow_the_registry_without_overwriting_materials_and_stay_on_the_platform() {
        let y = 63;
        let cells: HashMap<_, _> = scene_cells(y).into_iter().collect();
        let last_row = sample_position(MaterialId::LAST, y)[2];
        assert!(mixed_slab_row_z() > last_row);
        assert!(snow_row_z() > mixed_slab_row_z());
        assert!(platform_end_z() > snow_row_z());
        for x in 8..14 {
            assert_eq!(cells.get(&[x, y, mixed_slab_row_z()]), Some(&82));
            assert_eq!(
                cells.get(&[x, y - 1, mixed_slab_row_z()]),
                Some(&(MaterialId::SmoothStone as u16))
            );
        }
        for state in 1..=8 {
            let x = 7 + state;
            assert_eq!(
                cells.get(&[x, y, snow_row_z()]),
                Some(&(if state == 8 { 5 } else { 98 }))
            );
            assert_eq!(cells.get(&[x, y - 1, snow_row_z()]), Some(&1));
        }
    }

    #[test]
    fn filled_pots_use_real_registered_blocks_supported_textiles_and_exact_journal_contents() {
        use terra_core::{ItemKind, ResourcePropKind, beds::DyeColor};
        use terra_voxel::BlockSection;
        use terra_world::streaming::LoadedSection;

        let y = 63;
        let mut chunks = ChunkStore::default();
        for (position, material) in scene_cells(y) {
            let (owner, local) = global_to_section(position);
            chunks
                .loaded
                .entry(owner)
                .or_insert_with(|| LoadedSection {
                    data: BlockSection::empty(owner).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                })
                .data
                .set_block(local, material)
                .unwrap();
        }
        let expected = [
            ItemKind::Flower(DyeColor::White),
            ItemKind::Flower(DyeColor::Red),
            ItemKind::Flower(DyeColor::Blue),
            ItemKind::Prop(ResourcePropKind::Mushroom),
        ];
        let mut journal = EditJournal::default();
        for ((position, facing), item) in potted_samples(y).into_iter().zip(expected) {
            assert!(position[2] > snow_row_z() && position[2] < platform_end_z());
            assert_eq!(
                chunks.block_at(position),
                Some(terra_core::flower_pot::MATERIAL)
            );
            assert!(terra_core::beds::supports_bed_floor(
                chunks.block_at([position[0], y, position[2]]).unwrap()
            ));
            assert_eq!(
                chunks.block_at([position[0], position[1] + 1, position[2]]),
                Some(0)
            );
            let state = BlockObjectState {
                facing,
                ..Default::default()
            };
            assert!(state.valid_for(position, terra_core::flower_pot::MATERIAL));
            journal
                .record(position, terra_core::flower_pot::MATERIAL)
                .unwrap();
            terra_world::streaming::commit_object_state(
                &mut chunks,
                &mut journal,
                position,
                Some(state),
            )
            .unwrap();
            assert_eq!(journal.object_state(position), Some(state));
            assert_eq!(terra_core::flower_pot::contents(state.facing), Some(item));
            assert_eq!(
                journal.block(position),
                Some(terra_core::flower_pot::MATERIAL)
            );
        }
        for (position, material) in textile_samples(y) {
            assert_eq!(chunks.block_at(position), Some(material));
            if terra_core::textiles::is_carpet(material) {
                assert_eq!(
                    chunks.block_at([position[0], y - 1, position[2]]),
                    Some(MaterialId::SmoothStone as u16)
                );
            }
        }
        assert!(
            chunks
                .loaded
                .values()
                .all(|section| section.props.is_empty())
        );
    }

    #[test]
    fn one_extra_detail_capture_frames_all_four_pots_and_their_carpet_samples() {
        let y = 63.0;
        let view = pot_camera(y).to_matrix().inverse();
        let projection = bevy::math::proj::perspective(45.0_f32.to_radians(), 1.0, 0.1, 1000.0);
        for x in [8.0, 15.0] {
            for z in [pot_row_z() as f32 - 1.0, pot_row_z() as f32 + 1.0] {
                for height in [0.0, 2.0] {
                    let clip = projection * view * Vec3::new(x, y + height, z).extend(1.0);
                    assert!(clip.w > 0.0);
                    let ndc = clip.truncate() / clip.w;
                    assert!(ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0, "{ndc:?}");
                }
            }
        }
        assert_eq!(COMPLETE_STAGE, 16);
    }

    #[test]
    fn overview_fits_the_complete_expanding_platform_at_square_or_wider_aspects() {
        let y = 63.0;
        let view = overview_camera(y).to_matrix().inverse();
        let projection = bevy::math::proj::perspective(45.0_f32.to_radians(), 1.0, 0.1, 1000.0);
        for x in [6.0, 20.0] {
            for z in [2.0, platform_end_z() as f32 + 1.0] {
                for height in [-1.0, 4.0] {
                    let clip = projection * view * Vec3::new(x, y + height, z).extend(1.0);
                    assert!(clip.w > 0.0);
                    let ndc = clip.truncate() / clip.w;
                    assert!(ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0, "{ndc:?}");
                }
            }
        }
    }
}
