//! Convert a stopped rigid train into editable, cardinal world blocks atomically.
use crate::{
    edits::{BlockObjectState, EditJournal},
    streaming::ChunkStore,
    trains::{AssemblyPlan, PartEdit},
};
use terra_core::{
    PlayerUuid,
    kingdom::KingdomPermission,
    trains::{Cell, TrainError, Vehicle},
};

pub(crate) fn turn(mut vector: Cell, turns: u8) -> Cell {
    for _ in 0..turns % 4 {
        vector = [vector[2], vector[1], -vector[0]];
    }
    vector
}
/// Preserve encoded material state while applying a world Y quarter-turn.
pub(crate) fn facing(material: u16, value: u8, turns: u8) -> Result<u8, TrainError> {
    use terra_core::{construction, kinetic};
    if turns.is_multiple_of(4) {
        return Ok(value);
    }
    if matches!(material, 5 | 98 | 131 | 194) {
        return Ok(value);
    }
    if construction::is_stair(material) || terra_core::castle::is_castle(material) {
        let basis = construction::stair_basis(value)
            .map(|axis| turn(axis.map(i64::from), turns).map(|v| v as i8));
        return (0..24)
            .find(|pose| construction::stair_basis(*pose) == basis)
            .ok_or(TrainError::Invalid);
    }
    if construction::is_slab(material) {
        if value == construction::DOUBLE_SLAB {
            return Ok(value);
        }
        let halves = construction::slab_halves(material, value);
        let first = halves.map_or(value, |halves| halves[0].1);
        let shape = construction::slab_bounds(first).rotated_y(turns);
        let next = [0, 4, 5, 6, 7, 8]
            .into_iter()
            .find(|pose| construction::slab_bounds(*pose) == shape)
            .ok_or(TrainError::Invalid)?;
        return if let Some(halves) = halves {
            construction::merged_slab(material, next, halves[1].0).ok_or(TrainError::Invalid)
        } else {
            Ok(next)
        };
    }
    if material == terra_core::lantern::LANTERN || material == 40 {
        if (4..=7).contains(&value) {
            let normal = terra_core::block_shapes::torch_normal(value).map(|v| v as i64);
            let next = turn(normal, turns).map(|v| v as i32);
            return if material == terra_core::lantern::LANTERN {
                terra_core::lantern::facing(material, next).ok_or(TrainError::Invalid)
            } else {
                (4..8)
                    .find(|pose| {
                        terra_core::block_shapes::torch_normal(*pose).map(|v| v as i32) == next
                    })
                    .ok_or(TrainError::Invalid)
            };
        }
        return Ok(value);
    }
    if material == 60 && value >= 8 {
        let ports = crate::objects::chimney_ports(material, value).ok_or(TrainError::Invalid)?;
        return crate::objects::chimney_pose(ports.map(|port| turn(port, turns)))
            .map(|pose| pose + if value >= 40 { 32 } else { 0 })
            .ok_or(TrainError::Invalid);
    }
    if terra_core::plumbing::is_pipe(material)
        || material == 166
        || kinetic::is_rotary(material)
        || matches!(material, 144..=146)
    {
        let axis = value & 7;
        if axis >= 4 {
            return Ok(value);
        }
    }
    Ok((value & !3) | ((value + turns) & 3))
}
/// One exact affine integer transform is shared by block cells and frozen peers.
pub(crate) fn coordinate(source: Cell, target: Cell, position: Cell, turns: u8) -> Cell {
    let delta = turn(std::array::from_fn(|i| position[i] - source[i]), turns);
    std::array::from_fn(|i| target[i] + delta[i])
}
pub fn baked_vehicle(vehicle: &Vehicle) -> Result<Vehicle, TrainError> {
    let turns = vehicle.disassembly_turns().ok_or(TrainError::Busy)?;
    let cells = vehicle.disassembly_cells().ok_or(TrainError::Busy)?;
    let source = vehicle.part_cell(0).ok_or(TrainError::Invalid)?;
    let target = cells[0];
    let anchor = *cells.iter().min().ok_or(TrainError::Invalid)?;
    let mut baked = vehicle.clone();
    baked.anchor = anchor;
    let first_engine = vehicle.parts.iter().position(|part| part.material == 218);
    for (index, (part, cell)) in baked.parts.iter_mut().zip(cells).enumerate() {
        part.offset = std::array::from_fn(|i| (cell[i] - anchor[i]) as i16);
        let had_pose = vehicle
            .payloads
            .get(&(index as u16))
            .map(|payload| EditJournal::train_payload_pose(payload).map(|pose| pose.is_some()))
            .transpose()?
            .unwrap_or(true);
        if had_pose {
            part.facing = facing(part.material, part.facing, turns)?;
        }
        if let Some(view) = baked.blocks.get_mut(&(index as u16)) {
            view.boxes = view
                .boxes
                .iter()
                .map(|shape| shape.rotated_y(turns))
                .collect();
            view.peer = view
                .peer
                .map(|peer| turn(peer.map(i64::from), turns).map(|v| v as i16));
        }
        if let Some(payload) = baked.payloads.get_mut(&(index as u16)) {
            *payload = EditJournal::rebase_train_payload(
                payload,
                part.material,
                part.facing,
                source,
                target,
                turns,
                (Some(index) == first_engine).then_some(vehicle.burn_seconds),
            )?;
        }
    }
    baked.motion = Default::default();
    baked.burn_seconds = 0.0;
    if !baked.valid() {
        return Err(TrainError::Invalid);
    }
    Ok(baked)
}
pub fn prepare(
    chunks: &ChunkStore,
    journal: &EditJournal,
    actor: PlayerUuid,
    station: Cell,
    id: u64,
) -> Result<AssemblyPlan, TrainError> {
    let allowed = |cell, permission| {
        crate::kingdom_access::can_use(
            &journal.gameplay().kingdoms,
            chunks,
            actor,
            permission,
            cell,
        )
    };
    if actor.is_nil() || chunks.block_at(station) != Some(217) {
        return Err(TrainError::Missing);
    }
    if !allowed(station, KingdomPermission::Inventories) {
        return Err(TrainError::Protected);
    }
    let original = journal
        .gameplay()
        .trains
        .vehicles
        .get(&id)
        .ok_or(TrainError::Missing)?;
    if original.owner != actor {
        return Err(TrainError::NotOwner);
    }
    if original.couplings.iter().any(Option::is_some) {
        return Err(TrainError::Coupled);
    }
    let vehicle = baked_vehicle(original)?;
    let mut edits = Vec::new();
    for (index, part) in vehicle.parts.iter().enumerate() {
        let cell = vehicle.part_cell(index).ok_or(TrainError::Invalid)?;
        if (0..3).any(|axis| cell[axis].abs_diff(station[axis]) > 64) {
            return Err(TrainError::OutOfReach);
        }
        if !matches!(chunks.block_at(cell), Some(0 | 27)) {
            return Err(TrainError::Occupied);
        }
        if !allowed(cell, KingdomPermission::Build)
            || !allowed(cell, KingdomPermission::Inventories)
        {
            return Err(TrainError::Protected);
        }
        if part.material == terra_core::train_parts::BOGIE
            && !crate::trains::bogie_supported(chunks, journal, cell)
        {
            return Err(TrainError::Unsupported);
        }
        let low = cell.map(|v| v as f64);
        let high = cell.map(|v| v as f64 + 1.0);
        if journal.gameplay().trains.vehicles.values().any(|other| {
            other.id != id
                && (0..other.parts.len()).any(|index| {
                    other
                        .transformed_shapes(index)
                        .iter()
                        .any(|shape| shape.intersects(low, high))
                })
        }) {
            return Err(TrainError::Occupied);
        }
        edits.push(PartEdit {
            cell,
            expected: chunks.block_at(cell).unwrap(),
            state: if let Some(payload) = vehicle.payloads.get(&(index as u16)) {
                EditJournal::train_payload_pose(payload)?
            } else {
                Some(BlockObjectState {
                    facing: part.facing,
                    wood: part.wood,
                    ..Default::default()
                })
            },
            cargo: vehicle.cargo.get(&(index as u16)).cloned(),
        });
    }
    Ok(AssemblyPlan {
        station,
        start: vehicle.anchor,
        actor,
        vehicle,
        assembling: false,
        registry_revision: journal.gameplay().trains.revision,
        parts: edits,
        properties: Default::default(),
        source_motion: Some(original.motion),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use terra_core::{
        Inventory, WoodSpecies, train_parts,
        trains::{TrainBlockView, TrainPart, VehicleKind},
    };
    fn car() -> Vehicle {
        let mut parts = (0..=6)
            .map(|z| TrainPart {
                offset: [0, 1, z],
                material: if z == 2 { 210 } else { 212 },
                facing: 0,
                wood: WoodSpecies::Birch,
            })
            .collect::<Vec<_>>();
        for (offset, material) in [
            ([0, 0, 1], 211),
            ([0, 0, 5], 211),
            ([0, 1, -1], 213),
            ([0, 1, 7], 213),
            ([0, 2, 3], 214),
            ([0, 2, 4], 218),
            ([1, 1, 3], 105),
        ] {
            parts.push(TrainPart {
                offset,
                material,
                facing: if material == 105 { 5 } else { 0 },
                wood: WoodSpecies::Cherry,
            });
        }
        let engine = parts.iter().position(|p| p.material == 218).unwrap() as u16;
        let slab = parts.iter().position(|p| p.material == 105).unwrap() as u16;
        let mut vehicle = Vehicle {
            id: 1,
            owner: PlayerUuid::from_u128(1),
            kind: VehicleKind::Locomotive,
            anchor: [20, 6, 20],
            parts,
            cargo: BTreeMap::from([(engine, Inventory::default())]),
            blocks: BTreeMap::from([(
                slab,
                TrainBlockView {
                    boxes: vec![terra_core::construction::slab_bounds(5)],
                    tint: Some([31, 92, 211]),
                    ..Default::default()
                },
            )]),
            payloads: Default::default(),
            couplings: [None; 2],
            revision: 7,
            motion: Default::default(),
            burn_seconds: 1.75,
        };
        let mut journal = EditJournal::default();
        for index in 0..vehicle.parts.len() {
            let cell = vehicle.part_cell(index).unwrap();
            let p = vehicle.parts[index];
            journal.record(cell, p.material).unwrap();
            journal
                .set_object_state(
                    cell,
                    Some(BlockObjectState {
                        facing: p.facing,
                        wood: p.wood,
                        ..Default::default()
                    }),
                )
                .unwrap();
            if p.material == 105 {
                journal.set_tint(cell, Some([31, 92, 211])).unwrap();
            }
            vehicle
                .payloads
                .insert(index as u16, journal.train_payload(cell).unwrap());
        }
        assert!(vehicle.valid());
        vehicle
    }
    #[test]
    fn a_stopped_rotated_train_bakes_shapes_inventory_and_partial_fuel_for_editing() {
        let mut original = car();
        original.motion.translation = [8.2, 0.0, 3.2];
        original.motion.yaw = std::f64::consts::FRAC_PI_2;
        let expected = original.disassembly_cells().unwrap();
        let baked = baked_vehicle(&original).unwrap();
        assert_eq!(baked.motion, Default::default());
        assert_eq!(baked.burn_seconds, 0.0);
        assert_eq!(baked.cargo, original.cargo);
        for (index, cell) in expected.into_iter().enumerate() {
            assert_eq!(baked.part_cell(index), Some(cell));
        }
        let slab = baked.parts.iter().position(|p| p.material == 105).unwrap();
        assert_eq!(baked.parts[slab].facing, 8);
        assert_eq!(baked.blocks[&(slab as u16)].tint, Some([31, 92, 211]));
        let engine = baked.parts.iter().position(|p| p.material == 218).unwrap();
        let state: serde_json::Value =
            serde_json::from_slice(&baked.payloads[&(engine as u16)]).unwrap();
        assert_eq!(state["furnace"]["burn_remaining"], 1.75);
    }
    #[test]
    fn every_stair_orientation_and_mixed_slab_rotates_instead_of_losing_material_state() {
        for pose in 0..24 {
            let mut next = pose;
            for _ in 0..4 {
                next = facing(74, next, 1).unwrap();
            }
            assert_eq!(next, pose);
        }
        for material in terra_core::construction::SLAB_MATERIALS {
            for pose in [0, 4, 5, 6, 7, 8] {
                let original = terra_core::construction::merged_slab(material, pose, 105).unwrap();
                let mut next = original;
                for _ in 0..4 {
                    next = facing(material, next, 1).unwrap();
                }
                assert_eq!(next, original);
            }
        }
        assert_eq!(facing(train_parts::CAB, 2, 1).unwrap(), 3);
    }
    #[test]
    fn moving_or_sloped_trains_cannot_enter_edit_mode() {
        let mut vehicle = car();
        vehicle.motion.speed_mps = 0.1;
        assert!(matches!(baked_vehicle(&vehicle), Err(TrainError::Busy)));
        vehicle.motion.speed_mps = 0.0;
        vehicle.motion.pitch = 0.1;
        assert!(matches!(baked_vehicle(&vehicle), Err(TrainError::Busy)));
        vehicle.motion.pitch = 0.0;
        vehicle.motion.yaw = 0.3;
        assert!(matches!(baked_vehicle(&vehicle), Err(TrainError::Busy)));
    }
}
