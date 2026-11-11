//! Exact, bounded forward excavation shared by rail extensions and the moving attachment.
use super::*;
use crate::props::{PropId, PropKind, WorldProp};
use terra_core::block_shapes::ShapeBox;
use terra_voxel::SectionPos;

pub const WIDTH: f64 = 5.0;
pub const HEIGHT: i64 = 9;
const MAX_CELLS: usize = 8192;
const FRONT_REACH: f64 = 2.0;

type Before = (Cell, u16, Option<BlockObjectState>, Option<[u8; 3]>);

pub struct ClearancePlan {
    pub(super) layout: Layout,
    pub(super) cleared: Vec<PropClearance>,
    pub(super) drops: Vec<([f64; 3], ItemStack)>,
    cells: BTreeSet<Cell>,
    before: Vec<Before>,
}

impl ClearancePlan {
    pub fn layout(&self) -> &Layout {
        &self.layout
    }
    pub fn cleared(&self) -> &[PropClearance] {
        &self.cleared
    }
    pub fn drops(&self) -> &[([f64; 3], ItemStack)] {
        &self.drops
    }
    pub fn is_empty(&self) -> bool {
        self.layout.edits.is_empty() && self.cleared.is_empty()
    }
    pub fn commit(self, chunks: &mut ChunkStore, journal: &mut EditJournal) -> Result<(), String> {
        let current = collect(chunks, journal, self.cells.clone())?;
        if self.before != current.before
            || self.cleared != current.cleared
            || self.drops != current.drops
            || !same_edits(&self.layout.edits, &current.layout.edits)
        {
            return Err("Räumbereich wurde verändert".into());
        }
        if !current.is_empty() {
            journal.commit_rail_builder_changes(chunks, &current.layout, current.cleared)?;
        }
        Ok(())
    }
}

fn same_edits(a: &[BlockEditRequest], b: &[BlockEditRequest]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.position == b.position
                && a.material == b.material
                && a.expected_material == b.expected_material
        })
}

/// Cell coverage of a configured horizontal sweep. Positive-volume overlap
/// includes edge voxels on bends without extending cardinal routes by another block.
pub(super) fn columns(
    origin: [f64; 3],
    forward: [f64; 3],
    length: f64,
    width: u8,
) -> Result<BTreeSet<[i64; 2]>, String> {
    if origin.iter().chain(&forward).any(|v| !v.is_finite())
        || origin.iter().any(|v| v.abs() > 990_000.0)
        || !(0.0..=8.0).contains(&length)
        || !(terra_core::train_motion::RailClearance { width, height: 0 }).valid()
    {
        return Err("Ungültiger Räumbereich".into());
    }
    let width = f64::from(width);
    let magnitude = forward[0].hypot(forward[2]);
    if magnitude < 0.5 {
        return Err("Ungültige Räumrichtung".into());
    }
    let f = [forward[0] / magnitude, forward[2] / magnitude];
    let side = [-f[1], f[0]];
    let center = [
        origin[0] + f[0] * length * 0.5,
        origin[2] + f[1] * length * 0.5,
    ];
    let ext = [
        f[0].abs() * length * 0.5 + side[0].abs() * width * 0.5,
        f[1].abs() * length * 0.5 + side[1].abs() * width * 0.5,
    ];
    let radius = (f[0].abs() + f[1].abs()) * 0.5;
    let mut columns = BTreeSet::new();
    for x in (center[0] - ext[0]).floor() as i64..(center[0] + ext[0]).ceil() as i64 {
        for z in (center[1] - ext[1]).floor() as i64..(center[1] + ext[1]).ceil() as i64 {
            let d = [x as f64 + 0.5 - center[0], z as f64 + 0.5 - center[1]];
            if (d[0] * f[0] + d[1] * f[1]).abs() < length * 0.5 + radius - 1e-7
                && (d[0] * side[0] + d[1] * side[1]).abs() < width * 0.5 + radius - 1e-7
            {
                columns.insert([x, z]);
            }
        }
    }
    Ok(columns)
}

pub(super) fn cells(columns: &BTreeSet<[i64; 2]>, base_y: i64, height: u8) -> BTreeSet<Cell> {
    columns
        .iter()
        .flat_map(|[x, z]| (base_y..base_y + i64::from(height)).map(move |y| [*x, y, *z]))
        .collect()
}

/// Sweep two metres in front of the attachment, even on already existing track.
pub fn plan_clearance(
    chunks: &ChunkStore,
    journal: &EditJournal,
    car: &Vehicle,
) -> Result<ClearancePlan, String> {
    collect(chunks, journal, front_cells(car)?)
}

pub(super) fn front_cells(car: &Vehicle) -> Result<BTreeSet<Cell>, String> {
    let (origin, forward, base_y) = front_pose(car)?;
    let size = dimensions(car)?;
    Ok(cells(
        &columns(origin, forward, FRONT_REACH, size.width)?,
        base_y,
        size.height,
    ))
}

/// Distant optional construction must not block clearing along existing track.
/// The port height does not shorten the attachment's horizontal working reach.
pub fn extension_in_reach(car: &Vehicle, port: Port) -> Result<bool, String> {
    let (origin, forward, _) = front_pose(car)?;
    let magnitude = forward[0].hypot(forward[2]);
    if magnitude < 0.5 || !magnitude.is_finite() {
        return Err("Ungültige Räumrichtung".into());
    }
    let position = port.position.position();
    let distance = ((position[0] - origin[0]) * forward[0]
        + (position[2] - origin[2]) * forward[2])
        / magnitude;
    Ok(distance <= FRONT_REACH + 1e-7)
}

fn front_pose(car: &Vehicle) -> Result<([f64; 3], [f64; 3], i64), String> {
    let module = car
        .parts
        .iter()
        .find(|part| part.material == terra_core::train_parts::RAIL_BUILDER)
        .ok_or("Freiräumer fehlt")?;
    let bogie = car
        .parts
        .iter()
        .find(|part| part.material == terra_core::train_parts::BOGIE)
        .ok_or("Drehgestell fehlt")?;
    let angle = f64::from(module.facing % 4) * std::f64::consts::FRAC_PI_2;
    let forward = car.world_vector([-angle.sin(), 0.0, -angle.cos()]);
    let center = car.world_point(module.offset.map(|v| f64::from(v) + 0.5));
    let origin = std::array::from_fn(|a| center[a] + forward[a] * 0.5);
    let contact = car.world_point([
        f64::from(module.offset[0]) + 0.5,
        f64::from(bogie.offset[1]) - 1.0 + terra_core::rails::DECK_HEIGHT
            - terra_core::trains::MODEL_Y_OFFSET,
        f64::from(module.offset[2]) + 0.5,
    ]);
    let base_y = (contact[1] - terra_core::rails::DECK_HEIGHT + 1e-7).floor() as i64;
    Ok((origin, forward, base_y))
}

pub(super) fn dimensions(car: &Vehicle) -> Result<terra_core::train_motion::RailClearance, String> {
    let mut size = car.motion.rail_clearance;
    size.height = car.rail_clearance_height();
    if !size.valid() || size.height == 0 {
        return Err("Räumhöhe liegt außerhalb von 3–32 Blöcken".into());
    }
    Ok(size)
}

fn protected(
    material: u16,
    state: Option<BlockObjectState>,
    journal: &EditJournal,
    cell: Cell,
) -> bool {
    matches!(material, 14 | 27 | 29 | 41 | 59 | 99 | 131)
        || terra_core::kinetic::is_part(material)
        || terra_core::plumbing::is_part(material)
        || terra_core::logistics::is_port(material)
        || terra_core::train_parts::is_train(material)
        || terra_core::profession_stations::is_station(material)
        || material == terra_core::brewing::MATERIAL
        || material == terra_core::campfire::MATERIAL
        || journal.container_at(cell).is_some()
        || journal.furnace_at(cell).is_some()
        || journal.plumbing_at(cell).is_some()
        || journal.has_attached_campfire(cell)
        || journal.gameplay().kingdoms.active_at(cell).is_some()
        || state.is_some_and(|state| state.peer.is_some())
}

pub(super) fn block_drops(journal: &EditJournal, cell: Cell, material: u16) -> Vec<ItemStack> {
    if matches!(material, 30 | 38) {
        return vec![];
    }
    let state = journal.object_state(cell).unwrap_or_default();
    let mut stacks =
        if let Some(halves) = terra_core::construction::slab_halves(material, state.facing) {
            halves
                .into_iter()
                .map(|(half, _)| ItemStack {
                    item: ItemKind::for_wood(half, state.wood),
                    count: 1,
                })
                .collect()
        } else {
            let item = if material == 1 {
                ItemKind::Block(2)
            } else if material == 51 {
                ItemKind::Mineral(terra_core::OreKind::Salt.resource())
            } else if let Some(ore) = terra_core::OreKind::from_material(material) {
                ItemKind::Mineral(ore.resource())
            } else {
                ItemKind::for_wood(terra_core::castle::item_material(material), state.wood)
            };
            vec![ItemStack { item, count: 1 }]
        };
    if material == 98 {
        for stack in &mut stacks {
            stack.count = u16::from(terra_core::construction::snow_layers(state.facing));
        }
    }
    if let Some(rgb) = journal.tint_at(cell) {
        for stack in &mut stacks {
            if let Some(item) =
                terra_core::tints::recolored(stack.item, terra_core::beds::DyeColor::Rgb(rgb))
            {
                stack.item = item;
            }
        }
    }
    stacks
}

pub(super) fn collect(
    chunks: &ChunkStore,
    journal: &EditJournal,
    cells: BTreeSet<Cell>,
) -> Result<ClearancePlan, String> {
    if cells.len() > MAX_CELLS {
        return Err("Räumbereich überschreitet sein Zellbudget".into());
    }
    let mut edits = Vec::new();
    let mut before = Vec::new();
    let mut drops = Vec::new();
    let mut bounds = Vec::new();
    let sweep_bounds = cells.iter().fold(None::<[[f64; 3]; 2]>, |bounds, cell| {
        Some(bounds.map_or(
            [cell.map(|v| v as f64), cell.map(|v| v as f64 + 1.0)],
            |mut bounds| {
                for axis in 0..3 {
                    bounds[0][axis] = bounds[0][axis].min(cell[axis] as f64);
                    bounds[1][axis] = bounds[1][axis].max(cell[axis] as f64 + 1.0);
                }
                bounds
            },
        ))
    });
    let vehicles: Vec<_> = journal
        .gameplay()
        .trains
        .vehicles
        .values()
        .filter_map(|car| {
            let bounds = car.model_bounds();
            sweep_bounds
                .filter(|sweep| {
                    (0..3).all(|axis| {
                        bounds[0][axis] < sweep[1][axis] && bounds[1][axis] > sweep[0][axis]
                    })
                })
                .map(|_| {
                    (
                        bounds,
                        car.parts
                            .iter()
                            .enumerate()
                            .flat_map(|(index, _)| car.transformed_shapes(index))
                            .collect::<Vec<_>>(),
                    )
                })
        })
        .collect();
    for &cell in &cells {
        let material = chunks
            .block_at(cell)
            .ok_or("Räumbereich noch nicht geladen")?;
        let state = journal.object_state(cell);
        before.push((cell, material, state, journal.tint_at(cell)));
        if terra_core::rails::is_rail(material) {
            continue;
        }
        let low = cell.map(|v| v as f64 + 1e-6);
        let high = cell.map(|v| v as f64 + 1.0 - 1e-6);
        if vehicles.iter().any(|(bounds, shapes)| {
            (0..3).all(|axis| bounds[0][axis] < high[axis] && bounds[1][axis] > low[axis])
                && shapes.iter().any(|shape| shape.intersects(low, high))
        }) {
            continue;
        }
        bounds.push(ShapeBox {
            minimum: low,
            maximum: high,
            hinged: false,
        });
        if protected(material, state, journal, cell)
            || journal.fluid(cell).is_some_and(|fluid| fluid.amount > 0)
        {
            return Err("Geschützter Block, Maschine oder Wasser im Räumbereich".into());
        }
        if material == 0 {
            continue;
        }
        if terra_voxel::MaterialId::from_raw(material).is_none() {
            return Err("Unbekannter Block im Räumbereich".into());
        }
        for stack in block_drops(journal, cell, material) {
            drops.push((cell.map(|v| v as f64 + 0.5), stack));
        }
        if let Some(species) = crate::saplings::leaf_drop(chunks, journal, cell, material) {
            drops.push((
                cell.map(|value| value as f64 + 0.5),
                ItemStack {
                    item: ItemKind::Prop(species.sapling()),
                    count: 1,
                },
            ));
        }
        edits.push(BlockEditRequest {
            position: cell,
            expected_material: material,
            material: 0,
        });
    }
    let cleared = props(chunks, journal, &bounds)?;
    let states = edits.iter().map(|edit| (edit.position, None)).collect();
    Ok(ClearancePlan {
        layout: Layout {
            edits,
            states,
            belts: vec![],
        },
        cleared,
        drops,
        cells,
        before,
    })
}

fn props(
    chunks: &ChunkStore,
    journal: &EditJournal,
    bounds: &[ShapeBox],
) -> Result<Vec<PropClearance>, String> {
    let mut sections = BTreeSet::new();
    for shape in bounds {
        let owner = global_to_section(shape.minimum.map(|v| v.floor() as i64)).0;
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    sections.insert(owner.offset([x, y, z]).map_err(|e| e.to_string())?);
                }
            }
        }
    }
    let mut found = BTreeMap::<(SectionPos, PropId), WorldProp>::new();
    for owner in sections {
        for prop in chunks
            .loaded
            .get(&owner)
            .into_iter()
            .flat_map(|section| section.props.iter())
            .chain(journal.placed_props(owner))
        {
            if prop.removed || journal.prop_removed(prop.section, prop.id) {
                continue;
            }
            let occupied = prop.placement_bounds();
            if !bounds
                .iter()
                .any(|shape| shape.intersects(occupied[0], occupied[1]))
            {
                continue;
            }
            if matches!(prop.kind, PropKind::BeeNest | PropKind::AppleFruit) {
                return Err("Geschütztes Nest oder Frucht im Räumbereich".into());
            }
            found.insert((prop.section, prop.id), *prop);
            if found.len() > MAX_CLEARED_PROPS {
                return Err("Zu viele Pflanzen im Räumbereich".into());
            }
        }
    }
    found
        .into_values()
        .map(|prop| {
            let occupied = prop.placement_bounds();
            let low = occupied[0].map(|v| v.floor() as i64);
            let high = occupied[1].map(|v| (v - 1e-7).floor() as i64);
            let mut rights = BTreeSet::from([crate::props::placement::support(&prop)]);
            for x in low[0]..=high[0] {
                for y in low[1]..=high[1] {
                    for z in low[2]..=high[2] {
                        rights.insert([x, y, z]);
                        if rights.len() > 4096 {
                            return Err("Pflanze überschreitet den Rechtebereich".into());
                        }
                    }
                }
            }
            let source = if journal.placed_props(prop.section).any(|p| p.id == prop.id) {
                PropSource::Saved
            } else {
                PropSource::Generated
            };
            Ok(PropClearance {
                prop,
                source,
                rights_cells: rights.into_iter().collect(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::BlockSection;

    #[test]
    fn extension_reach_follows_exact_horizontal_attachment_pose_not_height_or_cell_rounding() {
        use terra_core::{rails::FixedPoint, trains::TrainPart};
        for facing in 0..4 {
            for yaw in [0.0, std::f64::consts::FRAC_PI_4] {
                let mut car = Vehicle {
                    name: String::new(),
                    id: 1,
                    owner: terra_core::PlayerUuid::from_u128(1),
                    kind: VehicleKind::Locomotive,
                    anchor: [-32, 10, -32],
                    parts: vec![
                        TrainPart {
                            offset: [0, 0, 0],
                            material: terra_core::train_parts::BOGIE,
                            facing: 0,
                            wood: WoodSpecies::Oak,
                        },
                        TrainPart {
                            offset: [0, 1, 2],
                            material: terra_core::train_parts::RAIL_BUILDER,
                            facing,
                            wood: WoodSpecies::Oak,
                        },
                    ],
                    cargo: Default::default(),
                    blocks: Default::default(),
                    payloads: Default::default(),
                    couplings: [None; 2],
                    revision: 0,
                    motion: terra_core::train_motion::TrainMotion {
                        yaw,
                        translation: [0.125, 0.0, -0.25],
                        ..Default::default()
                    },
                    burn_seconds: 0.0,
                    steam: Default::default(),
                };
                let (origin, forward, _) = front_pose(&car).unwrap();
                let mut port = Port {
                    index: 0,
                    position: FixedPoint(std::array::from_fn(|axis| {
                        ((origin[axis] + forward[axis] * (FRONT_REACH + 0.25)) * 16.0).round()
                            as i64
                    })),
                    tangent: [0; 3],
                    connectable: true,
                };
                assert!(!extension_in_reach(&car, port).unwrap());
                car.motion.translation[0] += forward[0] * 0.5;
                car.motion.translation[2] += forward[2] * 0.5;
                assert!(extension_in_reach(&car, port).unwrap());
                port.position.0[1] += 30 * 16;
                assert!(extension_in_reach(&car, port).unwrap());
            }
        }
    }

    #[test]
    fn configured_sweeps_use_exact_width_and_height_in_every_direction() {
        for width in [3, 5, 11, 15] {
            for height in [3, 9, 23, 32] {
                for [dx, dz] in [[0.0, 1.0], [1.0, 0.0], [0.0, -1.0], [-1.0, 0.0]] {
                    let origin = [0.5 + dx * 0.5, 10.0, 0.5 + dz * 0.5];
                    let swept = columns(origin, [dx, 0.0, dz], 2.0, width).unwrap();
                    let volume = cells(&swept, 10, height);
                    assert_eq!(volume.len(), usize::from(width) * usize::from(height) * 2);
                    assert!(
                        volume
                            .iter()
                            .all(|cell| (10..10 + i64::from(height)).contains(&cell[1]))
                    );
                }
            }
        }
    }

    fn set(chunks: &mut ChunkStore, cell: Cell, material: u16) {
        let (owner, local) = global_to_section(cell);
        chunks
            .loaded
            .entry(owner)
            .or_insert_with(|| LoadedSection {
                data: BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            })
            .data
            .set_block(local, material)
            .unwrap();
    }

    #[test]
    fn cardinal_sweeps_clear_exactly_five_by_nine_without_touching_ground_or_roof() {
        for [dx, dz] in [[0.0, 1.0], [1.0, 0.0], [0.0, -1.0], [-1.0, 0.0]] {
            let origin = [0.5 + dx * 0.5, 10.0, 0.5 + dz * 0.5];
            let columns = columns(origin, [dx, 0.0, dz], 2.0, 5).unwrap();
            assert_eq!(columns.len(), 10);
            let volume = cells(&columns, 10, 9);
            assert_eq!(volume.len(), 90);
            let mut chunks = ChunkStore::default();
            for x in -4..=4 {
                for y in 9..=19 {
                    for z in -4..=4 {
                        set(&mut chunks, [x, y, z], 3);
                    }
                }
            }
            let mut journal = EditJournal::default();
            let prepared = collect(&chunks, &journal, volume.clone()).unwrap();
            assert_eq!(prepared.layout.edits.len(), 90);
            assert_eq!(prepared.drops.len(), 90);
            let epoch = journal.rail_epoch();
            prepared.commit(&mut chunks, &mut journal).unwrap();
            assert_eq!(journal.rail_epoch(), epoch);
            for x in -4..=4 {
                for y in 9..=19 {
                    for z in -4..=4 {
                        let cell = [x, y, z];
                        assert_eq!(
                            chunks.block_at(cell),
                            Some(if volume.contains(&cell) { 0 } else { 3 }),
                            "{cell:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn existing_rails_survive_and_an_unchanged_sweep_becomes_empty() {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let piece = Piece::seed([0, 10, 0], 0).unwrap();
        for cell in super::super::super::cells(piece) {
            set(&mut chunks, cell, piece.material);
            journal.record(cell, piece.material).unwrap();
            journal
                .set_object_state(cell, Some(super::super::super::object_state(piece, cell)))
                .unwrap();
        }
        let volume = cells(
            &columns([0.5, 10.0, 0.0], [0.0, 0.0, 1.0], 4.0, 5).unwrap(),
            10,
            9,
        );
        for &cell in &volume {
            if !chunks
                .block_at(cell)
                .is_some_and(terra_core::rails::is_rail)
            {
                set(&mut chunks, cell, 3);
            }
        }
        let epoch = journal.rail_epoch();
        collect(&chunks, &journal, volume.clone())
            .unwrap()
            .commit(&mut chunks, &mut journal)
            .unwrap();
        assert_eq!(journal.rail_epoch(), epoch);
        assert!(super::super::super::complete(&journal, piece));
        assert!(collect(&chunks, &journal, volume).unwrap().is_empty());
    }

    #[test]
    fn metadata_and_exact_tree_ownership_are_preserved_in_drops_and_survivors() {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let root = [0, 10, 0];
        let plank = [1, 10, 0];
        for cell in [root, [0, 11, 0]] {
            set(&mut chunks, cell, 6);
        }
        set(&mut chunks, plank, 28);
        journal.record(plank, 28).unwrap();
        journal
            .set_object_state(
                plank,
                Some(BlockObjectState {
                    wood: WoodSpecies::Cherry,
                    ..Default::default()
                }),
            )
            .unwrap();
        journal.gameplay_mut().trees.insert(
            terra_core::TreeRecord {
                id: 1,
                root,
                species: terra_core::TreeSpecies::Oak,
                seed: 7,
                planted_by: None,
                stage: terra_core::TreeStage::Mature,
                growth_seconds: 900.0,
                bounds: None,
                wood: vec![[0, 0, 0], [0, 1, 0]],
                leaves: vec![],
                fruit: None,
                revision: 1,
            }
            .with_bounds(),
        );
        let prepared = collect(&chunks, &journal, BTreeSet::from([root, plank])).unwrap();
        assert!(
            prepared
                .drops
                .iter()
                .any(|(_, stack)| stack.item == ItemKind::for_wood(28, WoodSpecies::Cherry))
        );
        prepared.commit(&mut chunks, &mut journal).unwrap();
        assert!(journal.gameplay().trees.owner(root, 6).is_none());
        assert!(journal.gameplay().trees.owner([0, 11, 0], 6).is_some());
        assert_eq!(chunks.block_at([0, 11, 0]), Some(6));
        assert!(journal.gameplay().trees.valid());
    }

    #[test]
    fn protected_obstacles_and_stale_plans_never_publish_partial_excavation() {
        for material in [14, 29, 41, 59, 99] {
            let mut chunks = ChunkStore::default();
            let journal = EditJournal::default();
            set(&mut chunks, [0, 10, 0], 3);
            set(&mut chunks, [0, 11, 0], material);
            assert!(collect(&chunks, &journal, BTreeSet::from([[0, 10, 0], [0, 11, 0]])).is_err());
            assert_eq!(chunks.block_at([0, 10, 0]), Some(3));
        }
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        set(&mut chunks, [0, 10, 0], 3);
        set(&mut chunks, [0, 11, 0], 0);
        let prepared =
            collect(&chunks, &journal, BTreeSet::from([[0, 10, 0], [0, 11, 0]])).unwrap();
        set(&mut chunks, [0, 11, 0], 3);
        assert!(prepared.commit(&mut chunks, &mut journal).is_err());
        assert_eq!(chunks.block_at([0, 10, 0]), Some(3));
    }
}
