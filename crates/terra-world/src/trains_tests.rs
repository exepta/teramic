use super::*;
use crate::streaming::LoadedSection;
use terra_core::{ItemKind, ItemStack, WoodSpecies, rails::Piece};
use terra_voxel::{BlockSection, global_to_section};
fn block(chunks: &mut ChunkStore, cell: Cell, material: u16) {
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
            props: Vec::new(),
        })
        .data
        .set_block(local, material)
        .unwrap();
}
fn fixture(kind: VehicleKind) -> (ChunkStore, EditJournal, Cell, Cell) {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    for x in -3..=4 {
        for y in -2..=6 {
            for z in -3..=10 {
                block(&mut chunks, [x, y, z], 0);
            }
        }
    }
    for piece in [
        Piece::new([0, 0, 0], terra_core::rails::STRAIGHT, 0).unwrap(),
        Piece::new([0, 0, 4], terra_core::rails::STRAIGHT, 0).unwrap(),
    ] {
        for point in piece.supports().iter() {
            block(
                &mut chunks,
                std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis])),
                3,
            );
        }
        let plan = rails::plan(&chunks, &journal, piece.anchor, piece.material, 0).unwrap();
        rails::commit(&mut chunks, &mut journal, &plan).unwrap();
    }
    let mut parts = Vec::new();
    for z in 0..=6 {
        parts.push((
            [0, 2, z],
            if z == 2 {
                train_parts::CHASSIS
            } else {
                train_parts::FRAME
            },
        ));
    }
    parts.extend([
        ([0, 1, 1], train_parts::BOGIE),
        ([0, 1, 5], train_parts::BOGIE),
        ([0, 2, -1], train_parts::COUPLER),
        ([0, 2, 7], train_parts::COUPLER),
    ]);
    parts.push((
        [0, 3, 3],
        match kind {
            VehicleKind::Locomotive => train_parts::CAB,
            VehicleKind::Passenger => train_parts::SEAT,
            _ => train_parts::CARGO,
        },
    ));
    if kind == VehicleKind::Locomotive {
        parts.push(([0, 3, 4], train_parts::CARGO));
    }
    for (cell, material) in parts {
        block(&mut chunks, cell, material);
        journal.record(cell, material).unwrap();
        journal
            .set_object_state(
                cell,
                Some(BlockObjectState {
                    wood: WoodSpecies::Birch,
                    ..Default::default()
                }),
            )
            .unwrap();
        if material == train_parts::CARGO {
            journal
                .set_container(
                    cell,
                    Inventory {
                        slots: vec![ItemStack {
                            item: ItemKind::Block(3),
                            count: 42,
                        }],
                        ..Default::default()
                    },
                )
                .unwrap();
        }
    }
    let station = [3, 2, 2];
    block(&mut chunks, station, train_parts::STATION);
    journal.record(station, train_parts::STATION).unwrap();
    (chunks, journal, station, [0, 2, 2])
}
#[test]
fn assembly_and_dismantling_move_every_part_pose_and_private_cargo_exactly_once() {
    for kind in [
        VehicleKind::Locomotive,
        VehicleKind::Tender,
        VehicleKind::Passenger,
        VehicleKind::Freight,
    ] {
        let (mut chunks, mut journal, station, start) = fixture(kind);
        let owner = PlayerUuid::from_u128(1);
        let plan = plan_assembly(&chunks, &journal, owner, station, start, kind).unwrap();
        let source = plan.parts.clone();
        let id = commit(&mut chunks, &mut journal, &plan).unwrap();
        assert!(journal.gameplay().valid());
        assert!(journal.gameplay().trains.valid());
        assert!(
            source
                .iter()
                .all(|part| chunks.block_at(part.cell) == Some(0)
                    && journal.object_state(part.cell).is_none()
                    && journal.container_at(part.cell).is_none())
        );
        assert_eq!(
            commit(&mut chunks, &mut journal, &plan),
            Err(TrainError::Stale)
        );
        let vehicle = journal.gameplay().trains.vehicles.get(&id).unwrap();
        assert_eq!(vehicle.owner, owner);
        assert_eq!(vehicle.kind, kind);
        let view = vehicle.view();
        let [low, high] = view.bounds();
        assert!(!boxes_in(&journal, low.map(|v| v as f64), high.map(|v| v as f64)).is_empty());
        let restore = plan_dismantle(&chunks, &journal, owner, station, id).unwrap();
        commit(&mut chunks, &mut journal, &restore).unwrap();
        assert!(journal.gameplay().trains.vehicles.is_empty());
        assert!(journal.gameplay().trains.valid());
        for part in source {
            assert_eq!(chunks.block_at(part.cell), Some(part.expected));
            assert_eq!(journal.object_state(part.cell), part.state);
            if let Some(cargo) = part.cargo {
                assert_eq!(journal.container_at(part.cell), Some(&cargo));
            }
        }
    }
}
#[test]
fn stale_cargo_or_occupied_restore_never_publishes_partial_parts_or_inventory_ownership() {
    let (mut chunks, mut journal, station, start) = fixture(VehicleKind::Freight);
    let owner = PlayerUuid::from_u128(1);
    let stale = plan_assembly(
        &chunks,
        &journal,
        owner,
        station,
        start,
        VehicleKind::Freight,
    )
    .unwrap();
    let cargo = stale
        .parts
        .iter()
        .find(|part| part.cargo.is_some())
        .unwrap()
        .cell;
    journal
        .set_container(
            cargo,
            Inventory {
                slots: vec![ItemStack {
                    item: ItemKind::Block(3),
                    count: 43,
                }],
                ..Default::default()
            },
        )
        .unwrap();
    let before = journal.gameplay().clone();
    assert_eq!(
        commit(&mut chunks, &mut journal, &stale),
        Err(TrainError::Stale)
    );
    assert_eq!(journal.gameplay(), &before);
    assert!(
        stale
            .parts
            .iter()
            .all(|part| chunks.block_at(part.cell) == Some(part.expected))
    );
    let plan = plan_assembly(
        &chunks,
        &journal,
        owner,
        station,
        start,
        VehicleKind::Freight,
    )
    .unwrap();
    let id = commit(&mut chunks, &mut journal, &plan).unwrap();
    let restore = plan_dismantle(&chunks, &journal, owner, station, id).unwrap();
    let blocked = restore.parts.last().unwrap().cell;
    block(&mut chunks, blocked, 3);
    let before = journal.gameplay().clone();
    assert_eq!(
        commit(&mut chunks, &mut journal, &restore),
        Err(TrainError::Occupied)
    );
    assert_eq!(journal.gameplay(), &before);
    assert!(
        restore
            .parts
            .iter()
            .all(|part| part.cell == blocked || chunks.block_at(part.cell) == Some(0))
    );
}
#[test]
fn wrong_owner_and_missing_real_rail_support_are_rejected_before_assembly() {
    let (mut chunks, mut journal, station, start) = fixture(VehicleKind::Locomotive);
    let owner = PlayerUuid::from_u128(1);
    block(&mut chunks, [0, 0, 5], 0);
    assert!(matches!(
        plan_assembly(
            &chunks,
            &journal,
            owner,
            station,
            start,
            VehicleKind::Locomotive
        ),
        Err(TrainError::Unsupported)
    ));
    block(&mut chunks, [0, 0, 5], terra_core::rails::STRAIGHT);
    let plan = plan_assembly(
        &chunks,
        &journal,
        owner,
        station,
        start,
        VehicleKind::Locomotive,
    )
    .unwrap();
    let id = commit(&mut chunks, &mut journal, &plan).unwrap();
    assert!(matches!(
        plan_dismantle(&chunks, &journal, PlayerUuid::from_u128(2), station, id),
        Err(TrainError::NotOwner)
    ));
}
