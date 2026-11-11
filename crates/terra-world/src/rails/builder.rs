//! Bounded, transactional earthworks for a locomotive-mounted rail builder.
use super::*;
use crate::{
    placement::Layout,
    streaming::{BlockEditRequest, ChunkStore, LoadedSection},
};
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{
    ItemKind, ItemStack, WoodSpecies,
    rails::{CurveSpec, Port},
    trains::{TrainError, TrainRegistry, Vehicle, VehicleKind},
};
use terra_voxel::global_to_section;

mod clearing;
pub use clearing::{ClearancePlan, HEIGHT, WIDTH, plan_clearance};

pub const SPEED: f64 = 3.0;

/// Work trains contain one locomotive and at most one freight wagon, no tenders.
pub fn validate_consist(registry: &TrainRegistry, car: &Vehicle) -> Result<(), TrainError> {
    if !car.motion.rail_clearance.valid()
        || car.rail_clearance_height() > terra_core::train_motion::RailClearance::MAX_HEIGHT
    {
        return Err(TrainError::ClearanceTooLarge);
    }
    let mut attachments = car
        .parts
        .iter()
        .filter(|part| part.material == terra_core::train_parts::RAIL_BUILDER);
    let attachment = attachments.next().ok_or(TrainError::Incomplete)?;
    let axis = f64::from(attachment.facing % 4) * std::f64::consts::FRAC_PI_2;
    if car.kind != VehicleKind::Locomotive
        || attachments.next().is_some()
        || (axis - car.build_yaw()).cos().abs() < 0.5
    {
        return Err(TrainError::Incomplete);
    }
    let group = registry.consist(car.id)?;
    if group.len() > 2 {
        return Err(TrainError::Limit);
    }
    for id in group.into_iter().filter(|id| *id != car.id) {
        let wagon = registry.vehicles.get(&id).ok_or(TrainError::Missing)?;
        if wagon.owner != car.owner {
            return Err(TrainError::NotOwner);
        }
        let [low, high] = wagon.bounds();
        if wagon.kind != VehicleKind::Freight || (high[0] - low[0]).max(high[2] - low[2]) > 15 {
            return Err(TrainError::Limit);
        }
    }
    Ok(())
}

pub struct BuilderPlan {
    layout: Layout,
    cleared: Vec<PropClearance>,
    ballast: u16,
    tracks: u16,
    drops: Vec<([f64; 3], ItemStack)>,
    rail: Layout,
    port: Port,
    wood: WoodSpecies,
    extra_clearance: BTreeSet<Cell>,
    size: terra_core::train_motion::RailClearance,
}

fn earth(material: u16) -> bool {
    matches!(material, 1..=5 | 11..=13 | 19 | 206)
}

/// Extend an actual free port by four metres, preserving its exact horizontal tangent.
pub fn plan(
    chunks: &ChunkStore,
    journal: &EditJournal,
    port: Port,
    wood: WoodSpecies,
) -> Result<BuilderPlan, String> {
    plan_with_clearance(
        chunks,
        journal,
        port,
        wood,
        BTreeSet::new(),
        terra_core::train_motion::RailClearance {
            width: WIDTH as u8,
            height: HEIGHT as u8,
        },
    )
}

/// Plan the moving attachment and the next track segment as one transaction.
pub fn plan_for_vehicle(
    chunks: &ChunkStore,
    journal: &EditJournal,
    port: Port,
    wood: WoodSpecies,
    car: &Vehicle,
) -> Result<BuilderPlan, String> {
    plan_with_clearance(
        chunks,
        journal,
        port,
        wood,
        clearing::front_cells(car)?,
        clearing::dimensions(car)?,
    )
}

fn plan_with_clearance(
    chunks: &ChunkStore,
    journal: &EditJournal,
    port: Port,
    wood: WoodSpecies,
    extra_clearance: BTreeSet<Cell>,
    size: terra_core::train_motion::RailClearance,
) -> Result<BuilderPlan, String> {
    if !port.connectable || port.tangent[1] != 0 {
        return Err("Gleisleger benötigt einen ebenen Anschluss".into());
    }
    let tangent = port.tangent.map(i32::from);
    let magnitude = tangent[0].abs().max(tangent[2].abs());
    if magnitude == 0 || magnitude > 16 {
        return Err("Ungültige Gleisrichtung".into());
    }
    let scale = (64 / magnitude).max(1);
    let anchor = std::array::from_fn(|axis| {
        let coordinate = port.position.0[axis];
        // The canonical owner must lie inside the new footprint, including
        // extensions that leave an integer boundary in the negative direction.
        (coordinate - i64::from(axis != 1 && tangent[axis] < 0 && coordinate % 16 == 0))
            .div_euclid(16)
    });
    let start =
        std::array::from_fn::<_, 3, _>(|axis| (port.position.0[axis] - anchor[axis] * 16) as i32);
    let end = std::array::from_fn::<_, 3, _>(|axis| start[axis] + tangent[axis] * scale);
    let points = [
        start,
        std::array::from_fn(|a| start[a] + tangent[a] * (scale / 3).max(1)),
        std::array::from_fn(|a| end[a] - tangent[a] * (scale / 3).max(1)),
        end,
    ];
    let piece = Piece::new_curve(
        anchor,
        CurveSpec::new(points).ok_or("Ungültige Gleisverlängerung")?,
    )
    .ok_or("Ungültiges Gleis")?;
    if !piece.ports().iter().any(|p| p.connects(port)) {
        return Err("Gleisanschluss passt nicht".into());
    }
    let cells = cells(piece);
    let floor_y = anchor[1] - 1;
    let columns: BTreeSet<_> = cells.iter().map(|p| [p[0], p[2]]).collect();
    let length = (f64::from(end[0] - start[0])).hypot(f64::from(end[2] - start[2])) / 16.0;
    let sweep_columns = clearing::columns(
        port.position.position(),
        tangent.map(f64::from),
        length,
        size.width,
    )?;
    let mut clearance_cells = clearing::cells(&sweep_columns, anchor[1], size.height);
    clearance_cells.extend(extra_clearance.iter().copied());
    // A pitched vehicle can sweep through the extension's support level.
    // The ballast transaction owns these cells and accounts for their drops once.
    clearance_cells.retain(|cell| cell[1] != floor_y || !columns.contains(&[cell[0], cell[2]]));
    let clearance = clearing::collect(chunks, journal, clearance_cells)?;
    let mut edits: BTreeMap<_, _> = clearance
        .layout
        .edits
        .iter()
        .map(|edit| (edit.position, *edit))
        .collect();
    let mut drops = clearance.drops;
    let cleared = clearance.cleared;
    let mut ballast = 0;
    for [x, z] in columns {
        let cell = [x, floor_y, z];
        let old = chunks
            .block_at(cell)
            .ok_or("Arbeitsbereich noch nicht geladen")?;
        if journal.object_state(cell).is_some() || journal.fluid(cell).is_some_and(|f| f.amount > 0)
        {
            return Err("Gebautes Objekt oder Wasser unter dem Gleisbett".into());
        }
        if old == 206 {
            continue;
        }
        if old != 0 && !earth(old) {
            return Err("Geschützter Boden unter dem Gleisbett".into());
        }
        if old == 0 && !chunks.block_at([x, floor_y - 1, z]).is_some_and(earth) {
            return Err("Boden liegt mehr als einen Block tiefer".into());
        }
        ballast += 1;
        if old != 0 {
            for stack in clearing::block_drops(journal, cell, old) {
                drops.push((
                    [x as f64 + 0.5, floor_y as f64 + 1.5, z as f64 + 0.5],
                    stack,
                ));
            }
        }
        edits.insert(
            cell,
            BlockEditRequest {
                position: cell,
                expected_material: old,
                material: 206,
            },
        );
    }
    // Project only the handful of affected sections; never clone the whole world.
    let mut projected = ChunkStore::default();
    let owners: BTreeSet<_> = edits
        .keys()
        .chain(cells.iter())
        .map(|p| global_to_section(*p).0)
        .collect();
    for owner in owners {
        let loaded = chunks
            .loaded
            .get(&owner)
            .ok_or("Arbeitsbereich noch nicht geladen")?;
        projected.loaded.insert(
            owner,
            LoadedSection {
                data: loaded.data.clone(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: loaded.props.clone(),
            },
        );
    }
    for edit in edits.values() {
        let (owner, local) = global_to_section(edit.position);
        projected
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(local, edit.material)
            .map_err(|e| e.to_string())?;
    }
    let rail = super::placement::candidate_piece(&projected, piece, wood)?;
    let cleared_cells = edits
        .values()
        .filter(|edit| edit.material == 0)
        .map(|edit| edit.position)
        .collect();
    super::placement::environment_after_clearance(&rail, &projected, journal, &cleared_cells)?;
    for edit in &rail.edits {
        let mut edit = *edit;
        edit.expected_material = chunks
            .block_at(edit.position)
            .ok_or("Arbeitsbereich noch nicht geladen")?;
        edits.insert(edit.position, edit);
    }
    let states = edits
        .keys()
        .map(|cell| {
            (
                *cell,
                rail.states
                    .iter()
                    .find(|(p, _)| p == cell)
                    .and_then(|(_, s)| *s),
            )
        })
        .collect();
    Ok(BuilderPlan {
        layout: Layout {
            edits: edits.into_values().collect(),
            states,
            belts: vec![],
        },
        cleared,
        ballast,
        tracks: piece_cost(piece),
        drops,
        rail,
        port,
        wood,
        extra_clearance,
        size,
    })
}

impl BuilderPlan {
    pub fn layout(&self) -> &Layout {
        &self.layout
    }
    pub fn cleared(&self) -> &[PropClearance] {
        &self.cleared
    }
    pub fn ballast(&self) -> u16 {
        self.ballast
    }
    pub fn tracks(&self) -> u16 {
        self.tracks
    }
    pub fn drops(&self) -> &[([f64; 3], ItemStack)] {
        &self.drops
    }
    /// Recheck terrain, metadata, props and supporting floor before publishing the transaction.
    pub fn commit(self, chunks: &mut ChunkStore, journal: &mut EditJournal) -> Result<(), String> {
        let current = plan_with_clearance(
            chunks,
            journal,
            self.port,
            self.wood,
            self.extra_clearance.clone(),
            self.size,
        )?;
        if self.rail.states != current.rail.states {
            return Err("Gleis wurde verändert".into());
        }
        if current.ballast != self.ballast
            || current.tracks != self.tracks
            || current.drops != self.drops
            || current.cleared != self.cleared
            || current.layout.states != self.layout.states
            || current.layout.edits.len() != self.layout.edits.len()
            || current
                .layout
                .edits
                .iter()
                .zip(&self.layout.edits)
                .any(|(a, b)| {
                    a.position != b.position
                        || a.expected_material != b.expected_material
                        || a.material != b.material
                })
        {
            return Err("Arbeitsbereich wurde verändert".into());
        }
        journal.commit_rail_builder_changes(chunks, &current.layout, current.cleared)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::BlockSection;
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
    fn terrain() -> ChunkStore {
        let mut chunks = ChunkStore::default();
        for x in -12..=12 {
            for z in -12..=12 {
                for y in 6..=22 {
                    set(&mut chunks, [x, y, z], if y < 10 { 3 } else { 0 });
                }
            }
        }
        chunks
    }
    #[test]
    fn extension_clears_configured_width_and_height_and_preserves_its_ceiling() {
        let mut chunks = terrain();
        let mut journal = EditJournal::default();
        let port = Piece::seed([0, 10, 0], 0).unwrap().ports()[1];
        let columns = clearing::columns(
            port.position.position(),
            port.tangent.map(f64::from),
            4.0,
            7,
        )
        .unwrap();
        let [x, z] = *columns.iter().max_by_key(|column| column[0]).unwrap();
        let inside = [x, 21, z];
        let ceiling = [x, 22, z];
        set(&mut chunks, inside, 3);
        set(&mut chunks, ceiling, 3);
        let narrow = plan_with_clearance(
            &chunks,
            &journal,
            port,
            WoodSpecies::Oak,
            BTreeSet::new(),
            terra_core::train_motion::RailClearance {
                width: 3,
                height: 4,
            },
        )
        .unwrap();
        assert!(
            !narrow
                .layout
                .edits
                .iter()
                .any(|edit| edit.position == inside)
        );
        let expanded = plan_with_clearance(
            &chunks,
            &journal,
            port,
            WoodSpecies::Oak,
            BTreeSet::new(),
            terra_core::train_motion::RailClearance {
                width: 7,
                height: 12,
            },
        )
        .unwrap();
        assert!(
            expanded
                .layout
                .edits
                .iter()
                .any(|edit| edit.position == inside && edit.material == 0)
        );
        expanded.commit(&mut chunks, &mut journal).unwrap();
        assert_eq!(chunks.block_at(inside), Some(0));
        assert_eq!(chunks.block_at(ceiling), Some(3));
    }

    #[test]
    fn builder_extends_real_ports_in_all_directions_and_keeps_connected_wood() {
        for facing in 0..4 {
            let mut chunks = terrain();
            let mut journal = EditJournal::default();
            let seed = super::super::placement::candidate_piece(
                &chunks,
                Piece::seed([0, 10, 0], facing).unwrap(),
                WoodSpecies::Oak,
            )
            .unwrap();
            let first = super::super::placement::proposed_piece(&seed).unwrap();
            super::super::commit(&mut chunks, &mut journal, &seed).unwrap();
            for port in first.ports() {
                let prepared = plan(&chunks, &journal, port, WoodSpecies::Cherry).unwrap();
                assert!(prepared.ballast >= 9);
                assert_eq!(prepared.tracks, 1);
                let next = super::super::placement::proposed_piece(&prepared.rail).unwrap();
                assert!(next.ports().iter().any(|p| p.connects(port)));
                let rail_cells = super::super::cells(next);
                prepared.commit(&mut chunks, &mut journal).unwrap();
                assert!(super::super::complete(&journal, next));
                assert!(
                    rail_cells.iter().all(
                        |cell| journal.object_state(*cell).unwrap().wood == WoodSpecies::Cherry
                    )
                );
                assert!(super::super::complete(&journal, first));
            }
        }
    }
    #[test]
    fn earthworks_fill_one_block_clear_nine_blocks_and_reject_large_gaps_or_machines() {
        let mut chunks = terrain();
        let journal = EditJournal::default();
        let port = Piece::seed([0, 10, 0], 0).unwrap().ports()[1];
        let initial = plan(&chunks, &journal, port, WoodSpecies::Oak).unwrap();
        let floor = initial
            .layout
            .edits
            .iter()
            .find(|e| e.material == 206)
            .unwrap()
            .position;
        set(&mut chunks, floor, 0);
        assert!(plan(&chunks, &journal, port, WoodSpecies::Oak).is_ok());
        set(&mut chunks, [floor[0], floor[1] - 1, floor[2]], 0);
        assert!(plan(&chunks, &journal, port, WoodSpecies::Oak).is_err());
        set(&mut chunks, floor, 3);
        set(&mut chunks, [floor[0], floor[1] + 1, floor[2]], 3);
        assert!(plan(&chunks, &journal, port, WoodSpecies::Oak).is_ok());
        set(&mut chunks, [floor[0], floor[1] + 2, floor[2]], 3);
        assert!(plan(&chunks, &journal, port, WoodSpecies::Oak).is_ok());
        set(&mut chunks, [floor[0], floor[1] + 2, floor[2]], 0);
        set(&mut chunks, [floor[0], floor[1] + 1, floor[2]], 29);
        assert!(plan(&chunks, &journal, port, WoodSpecies::Oak).is_err());
    }
    #[test]
    fn front_overlap_preserves_ballast_and_does_not_duplicate_floor_drops() {
        for material in [3, 206] {
            let mut chunks = terrain();
            let mut journal = EditJournal::default();
            let port = Piece::seed([0, 10, 0], 0).unwrap().ports()[1];
            let floor = plan(&chunks, &journal, port, WoodSpecies::Oak)
                .unwrap()
                .layout
                .edits
                .iter()
                .find(|edit| edit.material == 206)
                .unwrap()
                .position;
            set(&mut chunks, floor, material);
            let without_overlap = plan(&chunks, &journal, port, WoodSpecies::Oak).unwrap();
            let prepared = plan_with_clearance(
                &chunks,
                &journal,
                port,
                WoodSpecies::Oak,
                BTreeSet::from([floor]),
                terra_core::train_motion::RailClearance {
                    width: 5,
                    height: 9,
                },
            )
            .unwrap();
            assert_eq!(prepared.drops, without_overlap.drops);
            assert_eq!(prepared.ballast, without_overlap.ballast);
            let position = [
                floor[0] as f64 + 0.5,
                floor[1] as f64 + 1.5,
                floor[2] as f64 + 0.5,
            ];
            let floor_drops: Vec<_> = prepared
                .drops
                .iter()
                .filter(|(p, _)| *p == position)
                .collect();
            assert_eq!(floor_drops.len(), usize::from(material != 206));
            if material != 206 {
                assert_eq!(
                    floor_drops[0].1,
                    ItemStack {
                        item: ItemKind::Block(3),
                        count: 1
                    }
                );
            }
            assert!(
                !prepared
                    .drops
                    .iter()
                    .any(|(_, stack)| stack.item == ItemKind::Block(206))
            );
            prepared.commit(&mut chunks, &mut journal).unwrap();
            assert_eq!(chunks.block_at(floor), Some(206));
        }
    }

    #[test]
    fn front_and_extension_are_revalidated_as_one_atomic_excavation() {
        let mut chunks = terrain();
        let mut journal = EditJournal::default();
        let port = Piece::seed([0, 10, 0], 0).unwrap().ports()[1];
        let front = [8, 12, 8];
        set(&mut chunks, front, 3);
        let prepared = plan_with_clearance(
            &chunks,
            &journal,
            port,
            WoodSpecies::Oak,
            BTreeSet::from([front]),
            terra_core::train_motion::RailClearance {
                width: 5,
                height: 9,
            },
        )
        .unwrap();
        assert!(
            prepared
                .layout
                .edits
                .iter()
                .any(|edit| edit.position == front && edit.material == 0)
        );
        let expected = prepared.layout.edits.clone();
        set(&mut chunks, front, 29);
        let epoch = journal.rail_epoch();
        assert!(prepared.commit(&mut chunks, &mut journal).is_err());
        assert_eq!(journal.rail_epoch(), epoch);
        assert!(
            expected
                .iter()
                .filter(|edit| edit.position != front)
                .all(|edit| chunks.block_at(edit.position) == Some(edit.expected_material))
        );
    }

    #[test]
    fn stale_plan_changes_no_other_cell_or_rail_epoch() {
        let mut chunks = terrain();
        let mut journal = EditJournal::default();
        let port = Piece::seed([0, 10, 0], 0).unwrap().ports()[1];
        let prepared = plan(&chunks, &journal, port, WoodSpecies::Oak).unwrap();
        let cells = prepared.layout.edits.clone();
        let changed = cells.last().unwrap().position;
        set(&mut chunks, changed, 29);
        let epoch = journal.rail_epoch();
        assert!(prepared.commit(&mut chunks, &mut journal).is_err());
        assert_eq!(journal.rail_epoch(), epoch);
        assert!(
            cells
                .iter()
                .filter(|edit| edit.position != changed)
                .all(|edit| chunks.block_at(edit.position) == Some(edit.expected_material))
        );
    }
}
