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

#[test]
fn arbitrary_blocks_keep_full_shapes_colors_chests_and_paused_machine_state_atomically() {
    let (mut chunks, mut journal, station, start) = fixture(VehicleKind::Locomotive);
    let owner = PlayerUuid::from_u128(1);
    let additions = [
        ([-1, 2, 0], 103, None),
        (
            [-1, 2, 1],
            74,
            Some(BlockObjectState {
                facing: 3,
                ..Default::default()
            }),
        ),
        (
            [-1, 2, 2],
            29,
            Some(BlockObjectState {
                facing: 2,
                wood: terra_core::WoodSpecies::Cherry,
                ..Default::default()
            }),
        ),
        (
            [-1, 2, 3],
            59,
            Some(BlockObjectState {
                facing: 1,
                peer: Some([-1, 3, 3]),
                ..Default::default()
            }),
        ),
        (
            [-1, 3, 3],
            59,
            Some(BlockObjectState {
                facing: 1,
                peer: Some([-1, 2, 3]),
                secondary: true,
                ..Default::default()
            }),
        ),
        ([-1, 2, 4], 163, Some(BlockObjectState::default())),
        (
            [-1, 2, 5],
            65,
            Some(BlockObjectState {
                peer: Some([-1, 3, 5]),
                ..Default::default()
            }),
        ),
        (
            [-1, 3, 5],
            65,
            Some(BlockObjectState {
                peer: Some([-1, 2, 5]),
                secondary: true,
                ..Default::default()
            }),
        ),
    ];
    for (cell, material, pose) in additions {
        block(&mut chunks, cell, material);
        journal.record(cell, material).unwrap();
        journal.set_object_state(cell, pose).unwrap();
    }
    // A neighboring natural surface is not a player-built train part.
    block(&mut chunks, [-2, 2, 0], 3);
    journal.set_tint([-1, 2, 0], Some([31, 92, 211])).unwrap();
    journal
        .set_container(
            [-1, 2, 2],
            Inventory {
                slots: vec![ItemStack {
                    item: ItemKind::Block(3),
                    count: 17,
                }],
                revision: 8,
                ..Default::default()
            },
        )
        .unwrap();
    let furnace = terra_core::furnace::FurnaceState {
        burn_remaining: 12.0,
        burn_total: 30.0,
        progress_seconds: 0.75,
        completed: 18,
        ..Default::default()
    };
    journal.set_furnace([-1, 2, 3], furnace.clone()).unwrap();
    journal
        .set_plumbing(
            [-1, 2, 4],
            crate::plumbing::PlumbingState {
                water: crate::water::FluidCell {
                    amount: 4,
                    ..Default::default()
                },
                pump_credit: 0.5,
                ..Default::default()
            },
        )
        .unwrap();
    let captured: Vec<_> = additions
        .iter()
        .map(|(cell, _, _)| (*cell, journal.train_payload(*cell).unwrap()))
        .collect();
    let plan = plan_assembly(
        &chunks,
        &journal,
        owner,
        station,
        start,
        VehicleKind::Locomotive,
    )
    .unwrap();
    assert_eq!(plan.parts.len(), 13 + additions.len());
    assert!(!plan.parts.iter().any(|part| part.cell == [-2, 2, 0]));
    let stair = plan
        .parts
        .iter()
        .position(|part| part.cell == [-1, 2, 1])
        .unwrap();
    assert!(plan.vehicle.part_shapes(stair).len() > 1);
    let chest = plan
        .parts
        .iter()
        .position(|part| part.cell == [-1, 2, 2])
        .unwrap() as u16;
    assert_eq!(plan.vehicle.cargo[&chest].slots[0].count, 17);
    let private_before = serde_json::to_value(&plan.vehicle).unwrap();
    let public = serde_json::to_value(plan.vehicle.view()).unwrap();
    assert!(private_before.get("payloads").is_some());
    assert!(public.get("payloads").is_none());
    assert!(public.get("cargo").is_none());
    let id = commit(&mut chunks, &mut journal, &plan).unwrap();
    assert!(journal.furnace_at([-1, 2, 3]).is_none());
    assert!(journal.plumbing_at([-1, 2, 4]).is_none());
    assert!(journal.container_at([-1, 2, 2]).is_none());
    assert!(journal.tint_at([-1, 2, 0]).is_none());
    let restore = plan_dismantle(&chunks, &journal, owner, station, id).unwrap();
    commit(&mut chunks, &mut journal, &restore).unwrap();
    for (cell, material, pose) in additions {
        assert_eq!(chunks.block_at(cell), Some(material));
        assert_eq!(journal.object_state(cell), pose);
    }
    for (cell, payload) in captured {
        assert_eq!(journal.train_payload(cell).unwrap(), payload);
    }
    assert_eq!(journal.container_at([-1, 2, 2]).unwrap().slots[0].count, 17);
    assert_eq!(journal.furnace_at([-1, 2, 3]), Some(&furnace));
    assert_eq!(chunks.block_at([-2, 2, 0]), Some(3));
}

#[test]
fn arbitrary_train_assembly_rejects_external_multiblock_links_and_stale_machine_changes() {
    let (mut chunks, mut journal, station, start) = fixture(VehicleKind::Locomotive);
    let owner = PlayerUuid::from_u128(1);
    let cell = [-1, 2, 3];
    for p in [cell, [-1, 3, 3]] {
        block(&mut chunks, p, 59);
        journal.record(p, 59).unwrap();
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    facing: 0,
                    peer: Some(if p == cell { [-1, 3, 3] } else { cell }),
                    secondary: p != cell,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    journal
        .set_furnace(cell, terra_core::furnace::FurnaceState::default())
        .unwrap();
    let plan = plan_assembly(
        &chunks,
        &journal,
        owner,
        station,
        start,
        VehicleKind::Locomotive,
    )
    .unwrap();
    let changed = terra_core::furnace::FurnaceState {
        completed: 1,
        ..Default::default()
    };
    journal.set_furnace(cell, changed.clone()).unwrap();
    assert_eq!(
        commit(&mut chunks, &mut journal, &plan),
        Err(TrainError::Stale)
    );
    assert_eq!(journal.furnace_at(cell), Some(&changed));
    assert!(journal.gameplay().trains.vehicles.is_empty());
    // A block belonging to an unloaded/unrecorded other half cannot be stolen.
    journal.record([-1, 3, 3], 0).unwrap();
    block(&mut chunks, [-1, 3, 3], 0);
    assert!(matches!(
        plan_assembly(
            &chunks,
            &journal,
            owner,
            station,
            start,
            VehicleKind::Locomotive
        ),
        Err(TrainError::Incomplete)
    ));
    assert_eq!(chunks.block_at(cell), Some(59));
}

#[test]
fn mounted_plumbing_cannot_overfill_world_capacity_when_dismantled() {
    let (mut chunks, mut journal, station, start) = fixture(VehicleKind::Locomotive);
    let owner = PlayerUuid::from_u128(1);
    let pipe = [-1, 2, 2];
    block(&mut chunks, pipe, 163);
    journal.record(pipe, 163).unwrap();
    journal
        .set_object_state(pipe, Some(BlockObjectState::default()))
        .unwrap();
    journal
        .set_plumbing(pipe, crate::plumbing::PlumbingState::default())
        .unwrap();
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
    for index in 0..crate::plumbing::MAX_NODES {
        let cell = [1000 + index as i64, 2, 0];
        journal.record(cell, 163).unwrap();
        journal
            .set_object_state(cell, Some(BlockObjectState::default()))
            .unwrap();
        journal
            .set_plumbing(cell, crate::plumbing::PlumbingState::default())
            .unwrap();
    }
    let before = journal.gameplay().trains.clone();
    let restore = plan_dismantle(&chunks, &journal, owner, station, id).unwrap();
    assert_eq!(
        commit(&mut chunks, &mut journal, &restore),
        Err(TrainError::Limit)
    );
    assert_eq!(journal.gameplay().trains, before);
    assert_eq!(journal.plumbing().count(), crate::plumbing::MAX_NODES);
    assert!(
        restore
            .parts
            .iter()
            .all(|part| chunks.block_at(part.cell) == Some(0))
    );
}

#[test]
fn an_elevated_train_part_can_start_the_same_complete_assembly() {
    let (mut chunks, mut journal, station, _) = fixture(VehicleKind::Locomotive);
    for y in 4..=5 {
        let cell = [0, y, 3];
        block(&mut chunks, cell, train_parts::FRAME);
        journal.record(cell, train_parts::FRAME).unwrap();
        journal
            .set_object_state(cell, Some(BlockObjectState::default()))
            .unwrap();
    }
    let plan = plan_assembly(
        &chunks,
        &journal,
        PlayerUuid::from_u128(1),
        station,
        [0, 5, 3],
        VehicleKind::Locomotive,
    )
    .unwrap();
    assert_eq!(plan.parts.len(), 15);
}

#[test]
fn live_crops_and_attached_world_props_are_rejected_without_orphaning_their_state() {
    for crop in [false, true] {
        let (mut chunks, mut journal, station, start) = fixture(VehicleKind::Locomotive);
        let owner = PlayerUuid::from_u128(1);
        let support = [-1, 2, 2];
        let section = global_to_section(support).0;
        block(&mut chunks, support, terra_core::farming::FARMLAND);
        journal
            .record(support, terra_core::farming::FARMLAND)
            .unwrap();
        assert!(
            plan_assembly(
                &chunks,
                &journal,
                owner,
                station,
                start,
                VehicleKind::Locomotive
            )
            .is_ok()
        );
        let prop = crate::props::WorldProp {
            id: crate::farming::crop_id(support, terra_core::CropKind::Wheat),
            kind: if crop {
                crate::props::PropKind::Wheat
            } else {
                crate::props::PropKind::Flower
            },
            position: [
                support[0] as f64 + 0.5,
                support[1] as f64 + 1.0,
                support[2] as f64 + 0.5,
            ],
            rotation: 0.0,
            scale: 1.0,
            variant: 0,
            section,
            removed: false,
        };
        assert!(journal.place_prop(prop).unwrap());
        if crop {
            journal
                .set_crop(
                    support,
                    terra_core::farming::CropState::new(terra_core::CropKind::Wheat, prop.id.0),
                )
                .unwrap();
        } else {
            chunks.loaded.get_mut(&section).unwrap().props.push(prop);
        }
        let before = journal.section_delta(section);
        let crop_before = journal.crop_at(support).copied();
        assert!(matches!(
            plan_assembly(
                &chunks,
                &journal,
                owner,
                station,
                start,
                VehicleKind::Locomotive
            ),
            Err(TrainError::AttachedProps)
        ));
        assert_eq!(journal.section_delta(section), before);
        assert_eq!(journal.crop_at(support).copied(), crop_before);
        assert_eq!(
            journal.placed_props(section).copied().collect::<Vec<_>>(),
            vec![prop]
        );
        assert_eq!(
            chunks.block_at(support),
            Some(terra_core::farming::FARMLAND)
        );
        assert!(journal.gameplay().trains.vehicles.is_empty());
    }
}

#[test]
fn recorded_world_water_is_not_collected_with_the_vehicle_body() {
    let (mut chunks, mut journal, station, start) = fixture(VehicleKind::Locomotive);
    let owner = PlayerUuid::from_u128(1);
    let water = [-1, 2, 2];
    let material = terra_voxel::MaterialId::FluidWater as u16;
    block(&mut chunks, water, material);
    journal.record(water, material).unwrap();
    let fluid = journal.fluid(water);
    let plan = plan_assembly(
        &chunks,
        &journal,
        owner,
        station,
        start,
        VehicleKind::Locomotive,
    )
    .unwrap();
    assert_eq!(plan.parts.len(), 13);
    assert!(!plan.parts.iter().any(|part| part.cell == water));
    let id = commit(&mut chunks, &mut journal, &plan).unwrap();
    assert_eq!(chunks.block_at(water), Some(material));
    assert_eq!(journal.fluid(water), fluid);
    let restore = plan_dismantle(&chunks, &journal, owner, station, id).unwrap();
    commit(&mut chunks, &mut journal, &restore).unwrap();
    assert_eq!(chunks.block_at(water), Some(material));
    assert_eq!(journal.fluid(water), fluid);
}
