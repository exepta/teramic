//! Ordinary wall placement and real, separately saved round-pipe surface fixtures.
use super::*;
use terra_network::protocol::inventory::{InventoryAction, InventoryCommand, InventoryOutcome};
use terra_world::streaming::ChunkStore;

const TORCH: u16 = 40;
const TORCH_RESERVE: usize = 9;
const TORCH_SLOT: usize = 8;
const GALLERY_X: [i64; 4] = [34, 40, 46, 52];
const NORMALS: [[i32; 3]; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
const MATERIALS: [u16; 4] = [
    plumbing::WOOD_PIPE,
    plumbing::STONE_PIPE,
    plumbing::IRON_PIPE,
    plumbing::GLASS_PIPE,
];
const NAMES: [&str; 4] = ["wood", "stone", "iron", "glass"];
const WET: [([i64; 3], u16, u8); 6] = [
    ([34, 19, 19], plumbing::WOOD_PIPE, 4),
    ([37, 19, 19], plumbing::STONE_PIPE, 4),
    ([40, 19, 19], plumbing::IRON_PIPE, 4),
    ([43, 19, 19], plumbing::GLASS_PIPE, 4),
    ([46, 19, 19], plumbing::VALVE, 4),
    ([49, 19, 19], plumbing::PUMP, 0),
];
pub(super) fn initial_glass_cells() -> [[i64; 3]; 4] {
    let [a, b, c] = gallery(3);
    [a, b, c, WET[3].0]
}

const MOVE_PENDING: u8 = 0;
const TORCH_READY: u8 = 1;
const TORCH_CLICK: u8 = 2;
const TORCH_ACK: u8 = 3;
const GALLERY: u8 = 4;
const GLASS_REVERSE: u8 = 5;
const WATER_FRONT: u8 = 6;
const WATER_OBLIQUE: u8 = 7;

#[derive(Default, Debug)]
pub(super) struct Inspection {
    phase: u8,
    index: usize,
    since: Option<Instant>,
    pulsed: bool,
    acknowledged: bool,
    water_wait_diagnosed: bool,
    previous_request: Option<u64>,
    inventory: Option<terra_core::Inventory>,
    pub(super) torches: [bool; 4],
    pub(super) galleries: [bool; 4],
    pub(super) water_samples: u8,
}

fn wall(index: usize) -> [i64; 3] {
    [GALLERY_X[index], 18, 0]
}
fn torch(index: usize) -> [i64; 3] {
    std::array::from_fn(|axis| wall(index)[axis] + i64::from(NORMALS[index][axis]))
}
fn wall_view(index: usize) -> Vec3 {
    let wall = wall(index);
    Vec3::new(
        wall[0] as f32 + 0.5 + NORMALS[index][0] as f32 * 3.,
        17.91,
        wall[2] as f32 + 0.5 + NORMALS[index][2] as f32 * 3.,
    )
}
fn gallery(index: usize) -> [[i64; 3]; 3] {
    let x = GALLERY_X[index];
    let sign = if index.is_multiple_of(2) { 1 } else { -1 };
    [[x, 19, 8], [x + sign, 19, 8], [x, 20, 8]]
}
fn gallery_view(index: usize) -> Vec3 {
    Vec3::new(GALLERY_X[index] as f32 + 3.5, 20.91, 12.5)
}
fn reverse_view() -> Vec3 {
    Vec3::new(49.5, 20.91, 4.5)
}
fn water_view(reverse: bool) -> Vec3 {
    if reverse {
        Vec3::new(53.5, 21.91, 23.5)
    } else {
        Vec3::new(42.5, 22.91, 31.5)
    }
}
fn travel(world: &mut World, position: Vec3) {
    teleport(
        world,
        &format!("/tp {} {} {}", position.x, position.y, position.z),
    );
}

// The valve's square physical envelope is wider than its round visual tube.
const WATER_PROBE_LOCAL: [f64; 3] = [0.95, 0.8, 0.95];
fn water_probe(cell: [i64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| cell[axis] as f64 + WATER_PROBE_LOCAL[axis])
}
fn verify_probe_clear(
    journal: &EditJournal,
    chunks: &ChunkStore,
    cell: [i64; 3],
    material: u16,
    facing: u8,
) -> Result<(), String> {
    // FluidSpace::base uses these same local physical boxes. The entire
    // X/Z column must be free so no upper fixture can interrupt level20.
    let boxes = terra_world::plumbing::plumbing_boxes(
        journal,
        cell,
        material,
        BlockObjectState {
            facing,
            ..default()
        },
        |p| chunks.block_at(p),
    );
    if boxes.iter().any(|bounds| {
        [0, 2].into_iter().all(|axis| {
            WATER_PROBE_LOCAL[axis] >= bounds.minimum[axis]
                && WATER_PROBE_LOCAL[axis] < bounds.maximum[axis]
        })
    }) {
        return Err(format!(
            "Water showcase probe crosses a physical hull: {cell:?}, material={material}, facing={facing}"
        ));
    }
    Ok(())
}

pub(super) fn prepare(journal: &mut EditJournal) -> Result<(), String> {
    // Keep the gallery independent of all existing machines and their ledgers.
    for x in 30..=56 {
        for z in -4..=24 {
            for y in 16..=24 {
                journal
                    .record([x, y, z], if y == 16 { 3 } else { 0 })
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    // Exercise the real clearance path that a remote large gear enables.
    let gear = [30, 18, 4];
    journal.record(gear, 149).map_err(|e| e.to_string())?;
    journal.set_object_state(gear, Some(BlockObjectState::default()))?;
    for index in 0..4 {
        journal.record(wall(index), 3).map_err(|e| e.to_string())?;
        let x = GALLERY_X[index];
        journal
            .record([x + 3, 19, 12], 3)
            .map_err(|e| e.to_string())?;
        for cell in gallery(index) {
            journal
                .record(cell, MATERIALS[index])
                .map_err(|e| e.to_string())?;
            journal.set_object_state(
                cell,
                Some(BlockObjectState {
                    facing: 4,
                    ..default()
                }),
            )?;
            journal.set_plumbing(cell, Default::default())?;
        }
    }
    for cell in [[49, 19, 4], [42, 21, 31], [53, 20, 23]] {
        journal.record(cell, 3).map_err(|e| e.to_string())?;
        for y in cell[1] + 1..=cell[1] + 3 {
            journal
                .record([cell[0], y, cell[2]], 0)
                .map_err(|e| e.to_string())?;
        }
    }
    // An explicit three-cell-deep source pool. There is no drain or powered pump.
    for x in 31..=55 {
        for z in 16..=22 {
            for y in 16..=20 {
                let wall = x == 31 || x == 55 || z == 16 || z == 22;
                let material = if y == 16 || wall {
                    3
                } else if y <= 19 {
                    27
                } else {
                    0
                };
                journal
                    .record([x, y, z], material)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    for (cell, material, facing) in WET {
        // Record water first: these are genuine source-preserving waterlogged parts.
        journal.record(cell, 27).map_err(|e| e.to_string())?;
        journal.record(cell, material).map_err(|e| e.to_string())?;
        journal.set_object_state(
            cell,
            Some(BlockObjectState {
                facing,
                ..default()
            }),
        )?;
        journal
            .set_fluid(
                cell,
                terra_world::water::FluidCell {
                    amount: 8,
                    source: true,
                    ..default()
                },
            )
            .map_err(|e| e.to_string())?;
        journal.set_plumbing(cell, Default::default())?;
    }
    // Fail before launching if an observer would hit another face or exceed reach.
    let chunks = fixture_chunks(journal)?;
    for (cell, material, facing) in WET {
        verify_probe_clear(journal, &chunks, cell, material, facing)?;
    }
    for (index, &normal) in NORMALS.iter().enumerate() {
        let eye = wall_view(index) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
        let target = Vec3::from_array(wall(index).map(|v| v as f32 + 0.5));
        let (_, hit) = terra_world::targeting::pick_world_with_context(
            &chunks,
            journal,
            &Default::default(),
            eye.to_array().map(f64::from),
            (target - eye).normalize().to_array().map(f64::from),
            terra_world::props::PropPose::Rest,
        );
        if !matches!(hit, Some(terra_world::targeting::WorldTarget::Block(hit)) if hit.position == wall(index) && hit.normal == normal && hit.distance < 6.)
        {
            return Err(format!(
                "Wall-torch observer {index} cannot see its exact support: {hit:?}"
            ));
        }
    }
    Ok(())
}

pub(super) fn prepare_inventory(bag: &mut terra_core::Inventory) -> Result<(), String> {
    if bag.slot(TORCH_RESERVE).is_some()
        || !bag.creative_set(
            TORCH_RESERVE,
            ItemStack {
                item: ItemKind::Block(TORCH),
                count: 4,
            },
        )
    {
        return Err("Wall-torch smoke needs its own four real reserved items".into());
    }
    Ok(())
}

fn phase(state: &mut Inspection, next: u8) {
    state.phase = next;
    state.since = Some(Instant::now());
    state.water_wait_diagnosed = false;
    info!(
        phase = next,
        index = state.index,
        "Hydraulics native surface stage"
    );
}

pub(super) fn begin(world: &mut World, state: &mut Inspection) -> Result<(), String> {
    let session = world.resource::<ClientSession>();
    if session.inventory.slot(TORCH_SLOT).is_some()
        || session.inventory.slot(TORCH_RESERVE)
            != Some(ItemStack {
                item: ItemKind::Block(TORCH),
                count: 4,
            })
    {
        return Err(
            "Surface smoke lost its four reserved torches or two consumed valve slots".into(),
        );
    }
    let revision = session.inventory.revision;
    state.inventory = Some(session.inventory.clone());
    world.write_message(terra_network::client::InventoryRequest(InventoryCommand {
        revision,
        action: InventoryAction::Move {
            from: TORCH_RESERVE as u8,
            to: TORCH_SLOT as u8,
            count: 4,
        },
    }));
    phase(state, MOVE_PENDING);
    Ok(())
}

pub(super) fn target(state: &Inspection) -> Vec3 {
    match state.phase {
        MOVE_PENDING | TORCH_READY | TORCH_CLICK => {
            Vec3::from_array(wall(state.index).map(|v| v as f32 + 0.5))
        }
        TORCH_ACK if state.acknowledged => {
            let p = torch(state.index);
            let facing = terra_world::torches::facing(NORMALS[state.index]).unwrap();
            let local = terra_core::block_shapes::torch_point(facing, [0.5, 0.45, 0.5]);
            Vec3::from_array(std::array::from_fn(|axis| {
                p[axis] as f32 + local[axis] as f32
            }))
        }
        TORCH_ACK => Vec3::from_array(wall(state.index).map(|v| v as f32 + 0.5)),
        GALLERY => Vec3::new(GALLERY_X[state.index] as f32 + 0.5, 19.65, 8.5),
        GLASS_REVERSE => Vec3::new(52.5, 19.65, 8.5),
        _ => Vec3::new(42., 19.85, 19.5),
    }
}

/// A single normal RMB edge, after body, target, preview and inventory were verified.
pub(super) fn place_edge(world: &mut World) -> bool {
    let state = &world.resource::<super::Inspection>().surfaces;
    let eligible = state.phase == TORCH_CLICK
        && !state.pulsed
        && !world.resource::<terra_player::PlayerInputBlock>().0
        && world
            .resource::<terra_player::CameraInputState>()
            .capture_ready();
    if eligible {
        world.resource_mut::<super::Inspection>().surfaces.pulsed = true;
    }
    eligible
}

fn settled(world: &World, desired: Vec3) -> bool {
    let session = world.resource::<ClientSession>();
    !session.teleporting
        && !world.resource::<terra_player::PlayerInputBlock>().0
        && session
            .confirmed_state()
            .is_some_and(|body| Vec3::from_array(body.position).distance(desired) < 0.3)
}

fn verify_gallery(world: &World, index: usize) -> Result<bool, String> {
    let chunks = world.resource::<ChunkStore>();
    let journal = world.resource::<EditJournal>();
    let cells = gallery(index);
    for cell in cells {
        if chunks.block_at(cell) != Some(MATERIALS[index]) {
            return Ok(false);
        }
    }
    let mask =
        terra_world::plumbing::connection_mask(journal, cells[0], MATERIALS[index], 4, |p| {
            chunks.block_at(p)
        });
    let expected = (1 << if index.is_multiple_of(2) { 1 } else { 0 }) | (1 << 3);
    if mask != expected {
        return Err(format!(
            "Gallery {} lost its actual elbow: {mask:#08b}",
            NAMES[index]
        ));
    }
    if index == 3 {
        let render = world.resource::<terra_render::GlassPipeRenderStatus>();
        for cell in cells {
            let Some(node) = render.nodes.iter().find(|node| node.position == cell) else {
                return Ok(false);
            };
            if node.amount != 0 || node.flowing || node.vertices != 0 {
                return Err(format!("Dry gallery glass invented liquid: {node:?}"));
            }
        }
    }
    Ok(true)
}

fn verify_water(world: &mut World, diagnose: bool) -> Result<bool, String> {
    use bevy::camera::primitives::Aabb;
    // Signed fields now publish reconstructed meshes. Require the current
    // source field and its actual uploaded geometry at this section's origin.
    let water_meshes: Vec<_> = world
        .query::<(
            &Name,
            &Mesh3d,
            &Transform,
            Option<&Aabb>,
            Option<&Visibility>,
        )>()
        .iter(world)
        .filter(|(name, _, _, _, _)| {
            name.as_str().starts_with("Reconstructed fluid 0 ")
                || name.as_str().starts_with("Natural water body ")
        })
        .map(|(name, mesh, transform, bounds, visibility)| {
            (
                name.as_str().to_owned(),
                mesh.0.id(),
                transform.translation,
                bounds.copied(),
                visibility.copied(),
            )
        })
        .collect();
    let chunks = world.resource::<ChunkStore>();
    let journal = world.resource::<EditJournal>();
    let terrain = &world.resource::<terra_world::streaming::WorldTerrain>().0;
    let rendered = world.resource::<terra_render::RenderedSections>();
    let terrain_status = world.resource::<terra_render::RenderStatus>();
    let water_status = world.resource::<terra_render::WaterStatus>();
    let far = world.resource::<terra_render::particle_fluids::FarFluidSections>();
    let mut all_ready = true;
    let mut details = Vec::new();
    for (cell, material, facing) in WET {
        if chunks.block_at(cell) != Some(material) {
            if diagnose {
                info!(?cell, expected=material, actual=?chunks.block_at(cell), "Surface wait: fixture block not yet loaded");
            }
            return Ok(false);
        }
        let water = chunks.fluid_cell_at(terrain, journal, cell, material);
        if water.amount != 8
            || !water.source
            || journal
                .object_state(cell)
                .is_none_or(|state| state.facing != facing)
        {
            return Err(format!(
                "Waterlogged showcase changed its source or orientation: {cell:?} {water:?}"
            ));
        }
        verify_probe_clear(journal, chunks, cell, material, facing)?;
        let probe = water_probe(cell);
        if chunks
            .water_column_at_point(terrain, journal, probe)
            .is_none_or(|column| (column.level - 20.).abs() > 1e-6)
        {
            return Err(format!(
                "Water no longer meets the real round fixture at {cell:?}"
            ));
        }
        if material == plumbing::GLASS_PIPE {
            let render = world.resource::<terra_render::GlassPipeRenderStatus>();
            let Some(node) = render.nodes.iter().find(|node| node.position == cell) else {
                if diagnose {
                    info!(
                        ?cell,
                        sequence = render.plumbing_sequence,
                        "Surface wait: waterlogged glass render node absent"
                    );
                }
                return Ok(false);
            };
            if node.amount != 0 || node.flowing || node.vertices != 0 {
                return Err(format!(
                    "Waterlogged gallery glass invented internal liquid: {node:?}"
                ));
            }
        }
        let (owner, _) = terra_voxel::global_to_section(cell);
        let section = chunks.loaded.get(&owner);
        let expected_water_epoch = journal.water_mesh_epoch(owner);
        let expected_mesh_epoch = journal.mesh_epoch(owner);
        let section_ready = terrain_status.section_ready(owner, section, None);
        let terrain_ready = rendered.sections.get(&owner).is_some_and(|render| {
            render.mesh_epoch == expected_mesh_epoch
                && !render.meshes.is_empty()
                && world
                    .get::<terra_render::TerrainSection>(render.entity)
                    .is_some_and(|root| root.position == owner)
        });
        let published = section.is_some_and(|section| {
            section.water_epoch == expected_water_epoch && section.mesh_epoch == expected_mesh_epoch
        });
        let origin = Vec3::from_array(owner.origin().map_err(|e| e.to_string())?.map(|v| v as f32));
        let field = far.sections.get(&owner);
        let body = field.is_some_and(|field| field.published_water.is_some());
        let field_ready = field.is_some_and(|field| {
            field
                .published_water
                .as_ref()
                .is_some_and(|p| !p.indices.is_empty())
                || field.fields[0].iter().any(|&value| value > 0.)
        });
        let prefix = if body {
            "Natural water body"
        } else {
            "Reconstructed fluid 0"
        };
        let expected_name = format!("{prefix} {},{},{}", owner.x, owner.y, owner.z);
        let water_entity =
            water_meshes
                .iter()
                .find(|(name, _, translation, bounds, visibility)| {
                    name == &expected_name
                        && translation.abs_diff_eq(origin, 1e-5)
                        && *visibility != Some(Visibility::Hidden)
                        && bounds.is_some_and(|bounds| {
                            bounds.half_extents.x > 0.
                                && bounds.half_extents.z > 0.
                                && origin.y + bounds.center.y - bounds.half_extents.y <= 20.
                                && origin.y + bounds.center.y + bounds.half_extents.y >= 20.
                        })
                });
        // WaterStatus.ready compares every uploaded cache epoch against these
        // FarFluidSections, so an earlier field mesh cannot satisfy readiness.
        all_ready &=
            section_ready && terrain_ready && published && field_ready && water_entity.is_some();
        if diagnose {
            details.push(format!(
                "cell={cell:?} owner={owner:?} terrain_status_ready={section_ready} terrain_entity_ready={terrain_ready} water_publication_current={published} section_epochs={:?} expected_epochs=({expected_mesh_epoch},{expected_water_epoch}) field_ready={field_ready} field_epoch={:?} water_entity={water_entity:?}",
                section.map(|s| (s.mesh_epoch, s.water_epoch, s.water.as_ref().map(|p| p.indices.len()))),
                field.map(|f| f.epoch),
            ));
        }
    }
    let ready = all_ready
        && water_status.ready
        && water_status.visible_tiles > 0
        && water_status.triangles > 0;
    if diagnose && !ready {
        info!(all_ready, water_ready=water_status.ready, water_initial_ready=water_status.initial_ready,
            water_tiles=water_status.visible_tiles, water_triangles=water_status.triangles,
            conditions=?details, uploaded_water_entities=?water_meshes,
            "Surface wait: exact terrain publication and reconstructed fluid epochs/meshes");
    }
    Ok(ready)
}

pub(super) fn step(world: &mut World, state: &mut super::Inspection) -> Result<bool, String> {
    let session = world.resource::<ClientSession>();
    if let Some(error) = &session.error {
        return Err(error.clone());
    }
    if !session.ready {
        return Ok(false);
    }
    let elapsed = state
        .surfaces
        .since
        .map_or(0., |since| since.elapsed().as_secs_f32());
    if elapsed > 8. {
        return Err(format!(
            "Surface smoke stalled: {:?}; body={:?}; edit={:?}; inventory={:?}; target={:?}; preview={:?}",
            state.surfaces,
            session.confirmed_state(),
            session.last_edit,
            session.last_inventory,
            world.resource::<terra_player::TargetBlock>().0,
            world.resource::<terra_player::interaction::PlacementPreview>()
        ));
    }
    match state.surfaces.phase {
        MOVE_PENDING => {
            let before = state
                .surfaces
                .inventory
                .as_ref()
                .ok_or("Missing torch reserve checkpoint")?;
            if session.inventory.revision <= before.revision {
                return Ok(false);
            }
            if session.last_inventory != Some(InventoryOutcome::Applied)
                || session.inventory.slot(TORCH_RESERVE).is_some()
                || session.inventory.slot(TORCH_SLOT)
                    != Some(ItemStack {
                        item: ItemKind::Block(TORCH),
                        count: 4,
                    })
            {
                return Err(
                    "Ordinary torch inventory move failed its server acknowledgement".into(),
                );
            }
            let mut expected = before.clone();
            if !expected.move_slot(TORCH_RESERVE, TORCH_SLOT, 4) || session.inventory != expected {
                return Err(
                    "Torch move changed inventory outside the exact reserved-stack transfer".into(),
                );
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(TORCH_SLOT as u8));
            travel(world, wall_view(0));
            phase(&mut state.surfaces, TORCH_READY);
        }
        TORCH_READY => {
            let index = state.surfaces.index;
            if elapsed < 0.5
                || !settled(world, wall_view(index))
                || world.resource::<terra_player::interaction::ActiveItem>().0
                    != Some(ItemKind::Block(TORCH))
            {
                return Ok(false);
            }
            let preview = world.resource::<terra_player::interaction::PlacementPreview>();
            if world
                .resource::<terra_player::TargetBlock>()
                .0
                .is_none_or(|hit| hit.position != wall(index) || hit.normal != NORMALS[index])
                || preview.position != Some(torch(index))
                || preview.material != TORCH
                || !preview.valid
                || preview.plan.as_ref().is_none_or(|plan| {
                    !plan.states.iter().any(|(cell, pose)| {
                        *cell == torch(index)
                            && pose.is_some_and(|pose| {
                                Some(pose.facing) == terra_world::torches::facing(NORMALS[index])
                            })
                    })
                })
            {
                return Ok(false);
            }
            if session.inventory.count(ItemKind::Block(TORCH)) != (4 - index) as u16
                || world.resource::<ChunkStore>().block_at(torch(index)) != Some(0)
            {
                return Err("Wall-torch preview lost its exact real item/air cell".into());
            }
            state.surfaces.previous_request = session.last_edit.map(|(id, _)| id);
            state.surfaces.inventory = Some(session.inventory.clone());
            state.surfaces.pulsed = false;
            state.surfaces.acknowledged = false;
            phase(&mut state.surfaces, TORCH_CLICK);
        }
        TORCH_CLICK if state.surfaces.pulsed => phase(&mut state.surfaces, TORCH_ACK),
        TORCH_ACK => {
            let index = state.surfaces.index;
            let applied = session.last_edit.is_some_and(|(id, result)| {
                Some(id) != state.surfaces.previous_request
                    && result == terra_network::protocol::session::EditOutcome::Applied
            });
            let before = state
                .surfaces
                .inventory
                .as_ref()
                .ok_or("Missing torch placement checkpoint")?;
            if !applied
                || session.inventory.revision <= before.revision
                || world.resource::<ChunkStore>().block_at(torch(index)) != Some(TORCH)
            {
                return Ok(false);
            }
            let mut expected = before.clone();
            if !expected.take_from(TORCH_SLOT, ItemKind::Block(TORCH))
                || session.inventory != expected
            {
                return Err(
                    "Wall-torch placement changed inventory beyond exactly one real torch".into(),
                );
            }
            let facing = terra_world::torches::facing(NORMALS[index]).unwrap();
            let journal = world.resource::<EditJournal>();
            let Some(pose) = journal.object_state(torch(index)) else {
                return Ok(false);
            };
            if pose.facing != facing
                || terra_world::torches::support(torch(index), pose.facing) != Some(wall(index))
            {
                return Err(format!(
                    "Confirmed wall torch has wrong attachment: index={index}, {pose:?}"
                ));
            }
            if !state.surfaces.acknowledged {
                state.surfaces.acknowledged = true;
                state.surfaces.since = Some(Instant::now());
                return Ok(false);
            }
            if elapsed < 0.3 {
                return Ok(false);
            }
            if !capture(
                world,
                state,
                &format!("wall-torch-facing-{facing}"),
                25 + index as u8,
            )? {
                return Ok(false);
            }
            state.surfaces.torches[index] = true;
            info!(index, facing, position=?torch(index), "Native wall torch placed by ordinary RMB and consumed exactly one real item");
            state.surfaces.index += 1;
            if state.surfaces.index < 4 {
                travel(world, wall_view(state.surfaces.index));
                phase(&mut state.surfaces, TORCH_READY);
            } else {
                world.write_message(terra_ui::InventoryAction::Hotbar(6));
                state.surfaces.index = 0;
                travel(world, gallery_view(0));
                phase(&mut state.surfaces, GALLERY);
            }
        }
        GALLERY => {
            let index = state.surfaces.index;
            if elapsed < 0.6
                || !settled(world, gallery_view(index))
                || !verify_gallery(world, index)?
            {
                return Ok(false);
            }
            if !capture(
                world,
                state,
                &format!("pipe-bend-{}", NAMES[index]),
                29 + index as u8,
            )? {
                return Ok(false);
            }
            state.surfaces.galleries[index] = true;
            state.surfaces.index += 1;
            if state.surfaces.index < 4 {
                travel(world, gallery_view(state.surfaces.index));
                phase(&mut state.surfaces, GALLERY);
            } else {
                travel(world, reverse_view());
                phase(&mut state.surfaces, GLASS_REVERSE);
            }
        }
        GLASS_REVERSE => {
            if elapsed < 0.6 || !settled(world, reverse_view()) || !verify_gallery(world, 3)? {
                return Ok(false);
            }
            if !capture(world, state, "pipe-bend-glass-reverse", 33)? {
                return Ok(false);
            }
            travel(world, water_view(false));
            phase(&mut state.surfaces, WATER_FRONT);
        }
        WATER_FRONT | WATER_OBLIQUE => {
            let reverse = state.surfaces.phase == WATER_OBLIQUE;
            if elapsed < 0.8 {
                return Ok(false);
            }
            let body_settled = settled(world, water_view(reverse));
            let diagnose = elapsed > 1.5 && !state.surfaces.water_wait_diagnosed;
            let ready = verify_water(world, diagnose)?;
            if !body_settled || !ready {
                if diagnose {
                    state.surfaces.water_wait_diagnosed = true;
                    info!(phase=state.surfaces.phase, elapsed, body_settled, ready,
                        desired=?water_view(reverse), actual=?world.resource::<ClientSession>().confirmed_state(),
                        "Surface wait: observer and mesh readiness (once per view)");
                }
                return Ok(false);
            }
            if !capture(
                world,
                state,
                if reverse {
                    "waterlogged-round-parts-oblique"
                } else {
                    "waterlogged-round-parts-front"
                },
                if reverse { 35 } else { 34 },
            )? {
                return Ok(false);
            }
            state.surfaces.water_samples += 1;
            if reverse {
                if !state.surfaces.torches.into_iter().all(|placed| placed)
                    || !state.surfaces.galleries.into_iter().all(|seen| seen)
                    || state.surfaces.water_samples != 2
                {
                    return Err(
                        "Surface smoke omitted a required real placement or gallery view".into(),
                    );
                }
                return Ok(true);
            }
            travel(world, water_view(true));
            phase(&mut state.surfaces, WATER_OBLIQUE);
        }
        _ => {}
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_fixture_keeps_four_legal_wall_faces_dry_bends_and_external_sources_separate() {
        let mut journal = EditJournal::default();
        prepare(&mut journal).unwrap(); // Includes real world-ray preflight for all four faces.
        let mut glass = 0;
        for index in 0..4 {
            let facing = terra_world::torches::facing(NORMALS[index]).unwrap();
            assert_eq!(facing, 4 + index as u8);
            assert_eq!(
                terra_world::torches::support(torch(index), facing),
                Some(wall(index))
            );
            assert_eq!(journal.block(torch(index)), Some(0));
            for cell in gallery(index) {
                assert_eq!(journal.block(cell), Some(MATERIALS[index]));
                assert_eq!(journal.plumbing_at(cell).unwrap().water.amount, 0);
                glass += usize::from(MATERIALS[index] == plumbing::GLASS_PIPE);
            }
        }
        for (cell, material, facing) in WET {
            assert_eq!(journal.block(cell), Some(material));
            assert_eq!(journal.object_state(cell).unwrap().facing, facing);
            let water = journal.fluid(cell).unwrap();
            assert_eq!(water.amount, 8);
            assert!(water.source);
            assert_eq!(journal.plumbing_at(cell).unwrap().water.amount, 0);
            glass += usize::from(material == plumbing::GLASS_PIPE);
        }
        assert_eq!(glass, initial_glass_cells().len());
        let mut bag = terra_core::Inventory::default();
        prepare_inventory(&mut bag).unwrap();
        assert!(
            bag.slot(3).is_none(),
            "the old filled-bottle hotbar remains available"
        );
        assert!(
            bag.slot(TORCH_SLOT).is_none(),
            "the real valve items own this slot until consumed"
        );
        assert_eq!(
            bag.slot(TORCH_RESERVE),
            Some(ItemStack {
                item: ItemKind::Block(TORCH),
                count: 4
            })
        );
    }
}
