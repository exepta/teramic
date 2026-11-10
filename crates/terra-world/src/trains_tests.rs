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

#[test]
fn moving_vehicle_picking_and_reservations_use_the_current_rigid_pose() {
    let (mut chunks, mut journal, station, start) = fixture(VehicleKind::Locomotive);
    let owner = PlayerUuid::from_u128(77);
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
    let car = journal.gameplay_mut().trains.vehicles.get_mut(&id).unwrap();
    car.motion.translation = [80.0, 0.0, 32.0];
    car.motion.yaw = 0.73;
    let index = car
        .parts
        .iter()
        .position(|part| part.material == train_parts::CHASSIS)
        .unwrap();
    let part = car.parts[index];
    let target = car.world_point(part.offset.map(|v| f64::from(v) + 0.5));
    let offset = car.world_vector([-4.0, 0.0, 0.0]);
    let origin = std::array::from_fn(|axis| target[axis] + offset[axis]);
    let direction = car.world_vector([1.0, 0.0, 0.0]);
    let view = car.view();
    let hit = raycast(&journal, origin, direction, 8.0).unwrap();
    assert_eq!((hit.vehicle, hit.part), (id, index as u16));
    assert!((hit.distance - 3.5).abs() < 0.01);
    assert_eq!(
        raycast_views(&[view], origin, direction, 8.0).unwrap().part,
        hit.part
    );
    assert!(reserved(&journal, target.map(|v| v.floor() as i64)));
    assert!(
        !reserved(&journal, [0, 2, 2]),
        "the old construction pad is free after travel"
    );
}

#[test]
fn new_assemblies_require_three_to_fifteen_meter_axle_spacing() {
    for spacing in [2, 3, 4, 15, 16] {
        let (mut chunks, mut journal, station, start) = fixture(VehicleKind::Locomotive);
        for x in -3..=4 {
            for y in -2..=6 {
                for z in 11..=22 {
                    block(&mut chunks, [x, y, z], 0);
                }
            }
        }
        for z in [8, 12, 16, 20] {
            let piece =
                terra_core::rails::Piece::new([0, 0, z], terra_core::rails::STRAIGHT, 0).unwrap();
            for point in piece.supports().iter() {
                block(
                    &mut chunks,
                    std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis])),
                    3,
                );
            }
            let rail = rails::plan(&chunks, &journal, piece.anchor, piece.material, 0).unwrap();
            rails::commit(&mut chunks, &mut journal, &rail).unwrap();
        }
        block(&mut chunks, [0, 1, 5], 0);
        journal.record([0, 1, 5], 0).unwrap();
        let rear = [0, 1, 1 + spacing];
        block(&mut chunks, rear, train_parts::BOGIE);
        journal.record(rear, train_parts::BOGIE).unwrap();
        journal
            .set_object_state(rear, Some(BlockObjectState::default()))
            .unwrap();
        for z in 7..=spacing + 2 {
            let cell = [0, 2, z];
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
            start,
            VehicleKind::Locomotive,
        );
        if (3..=15).contains(&spacing) {
            assert!(plan.is_ok(), "spacing={spacing}: {plan:?}");
        } else {
            assert_eq!(plan.unwrap_err(), TrainError::Wheelbase);
        }
    }
}

#[test]
fn real_guides_assemble_on_drawn_and_native_rails_despite_a_cube_chassis_hidden_yaw() {
    use terra_core::{rails::CurveSpec, train_blueprint};
    for facing in 0..4 {
        for drawn in [false, true] {
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            let origin = [-17, 16, -17];
            let forward = [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]][usize::from(facing)];
            for x in -12..=12 {
                for y in -2..=6 {
                    for z in -12..=12 {
                        block(
                            &mut chunks,
                            [origin[0] + x, origin[1] + y, origin[2] + z],
                            0,
                        );
                    }
                }
            }
            let pieces = if drawn {
                let points =
                    [[8, 2, 0], [8, 2, 64], [8, 2, 128], [8, 2, 192]].map(
                        |[x, y, z]| match facing {
                            0 => [x, y, z],
                            1 => [z, y, 16 - x],
                            2 => [16 - x, y, 16 - z],
                            _ => [16 - z, y, x],
                        },
                    );
                vec![Piece::new_curve(origin, CurveSpec::new(points).unwrap()).unwrap()]
            } else {
                (0..3)
                    .map(|step| {
                        Piece::new(
                            std::array::from_fn(|i| origin[i] + forward[i] * step * 4),
                            terra_core::rails::STRAIGHT,
                            facing,
                        )
                        .unwrap()
                    })
                    .collect()
            };
            for piece in pieces {
                for cell in rails::cells(piece) {
                    block(&mut chunks, cell, piece.material);
                    journal.record(cell, piece.material).unwrap();
                    journal
                        .set_object_state(cell, Some(rails::object_state(piece, cell)))
                        .unwrap();
                }
                for support in piece.supports().iter() {
                    block(
                        &mut chunks,
                        std::array::from_fn(|i| piece.anchor[i] + i64::from(support.support[i])),
                        3,
                    );
                }
            }
            let start = [
                origin[0] + forward[0] * 2,
                origin[1] + 2,
                origin[2] + forward[2] * 2,
            ];
            let station = [
                start[0] + forward[2] * 3,
                start[1],
                start[2] - forward[0] * 3,
            ];
            block(&mut chunks, station, train_parts::STATION);
            journal.record(station, train_parts::STATION).unwrap();
            let guide = train_blueprint::template(start, facing, VehicleKind::Locomotive);
            for part in &guide {
                block(&mut chunks, part.cell, part.material);
                journal.record(part.cell, part.material).unwrap();
                let pose = BlockObjectState {
                    facing: if part.material == train_parts::CHASSIS {
                        (facing + 1) % 4
                    } else {
                        part.facing
                    },
                    ..Default::default()
                };
                journal.set_object_state(part.cell, Some(pose)).unwrap();
                if part.material == 218 {
                    journal
                        .set_container(
                            part.cell,
                            Inventory {
                                slots: vec![ItemStack {
                                    item: ItemKind::Block(16),
                                    count: 1,
                                }],
                                ..Default::default()
                            },
                        )
                        .unwrap();
                }
                if part.material == train_parts::BOGIE {
                    assert!(
                        bogie_supported_with_facing(&chunks, &journal, part.cell, part.facing),
                        "facing={facing}, drawn={drawn}, bogie={part:?}"
                    );
                    assert!(!bogie_supported_with_facing(
                        &chunks,
                        &journal,
                        part.cell,
                        (part.facing + 1) % 4
                    ));
                }
            }
            let actor = PlayerUuid::from_u128(119);
            let plan = plan_assembly(
                &chunks,
                &journal,
                actor,
                station,
                start,
                VehicleKind::Locomotive,
            )
            .unwrap_or_else(|error| panic!("facing={facing}, drawn={drawn}: {error:?}"));
            let chassis = plan
                .vehicle
                .parts
                .iter()
                .position(|part| part.material == train_parts::CHASSIS)
                .unwrap();
            assert_eq!(plan.vehicle.parts[chassis].facing, (facing + 1) % 4);
            assert!(
                (plan.vehicle.build_yaw() - f64::from(facing) * std::f64::consts::FRAC_PI_2).abs()
                    < 1e-10
            );
            assert_eq!(
                plan.parts[chassis].state.unwrap().facing,
                (facing + 1) % 4,
                "removal still compares the original placed pose"
            );
            let id = commit(&mut chunks, &mut journal, &plan).unwrap();
            let car = &journal.gameplay().trains.vehicles[&id];
            let mut powered = car.clone();
            powered.motion.throttle = 1;
            powered.motion.brake = false;
            let moving = crate::train_motion::advance(
                &chunks,
                &journal,
                &rails::network(&journal).unwrap(),
                &powered,
                1.0 / 60.0,
            )
            .unwrap();
            assert!(moving.motion.speed_mps > 0.0);
            assert!(
                crate::train_motion::clear_swept(
                    &chunks,
                    &journal,
                    &Default::default(),
                    car,
                    &moving
                )
                .is_ok()
            );
            let edit = plan_dismantle(&chunks, &journal, actor, station, id).unwrap();
            commit(&mut chunks, &mut journal, &edit).unwrap();
            assert_eq!(
                journal.object_state(start).unwrap().facing,
                (facing + 1) % 4
            );
        }
    }
}

#[test]
fn saved_mixed_yaw_train_on_a_long_drawn_rail_assembles_without_reorienting_any_source_pose() {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    for x in 65..=74 {
        for y in 22..=30 {
            for z in -9..=24 {
                block(&mut chunks, [x, y, z], 0);
            }
        }
    }
    let curve =
        terra_core::rails::CurveSpec::new([[8, 2, 16], [8, 2, -163], [8, 2, -253], [8, 2, -432]])
            .unwrap();
    let piece = Piece::new_curve([69, 24, 20], curve).unwrap();
    for cell in rails::cells(piece) {
        block(&mut chunks, cell, piece.material);
        journal.record(cell, piece.material).unwrap();
        journal
            .set_object_state(cell, Some(rails::object_state(piece, cell)))
            .unwrap();
    }
    for support in piece.supports().iter() {
        block(
            &mut chunks,
            std::array::from_fn(|i| piece.anchor[i] + i64::from(support.support[i])),
            3,
        );
    }
    let station = [66, 24, 8];
    block(&mut chunks, station, 217);
    journal.record(station, 217).unwrap();
    let placed = [
        ([69, 25, 7], 211, 0),
        ([69, 25, 11], 211, 0),
        ([69, 26, 5], 213, 2),
        ([69, 26, 6], 212, 2),
        ([69, 26, 7], 212, 3),
        ([69, 26, 8], 210, 3),
        ([69, 26, 9], 212, 2),
        ([69, 26, 10], 212, 3),
        ([69, 26, 11], 212, 3),
        ([69, 26, 12], 212, 3),
        ([69, 26, 13], 213, 0),
        ([69, 27, 9], 214, 2),
        ([69, 27, 10], 218, 3),
    ];
    for (cell, material, facing) in placed {
        block(&mut chunks, cell, material);
        journal.record(cell, material).unwrap();
        journal
            .set_object_state(
                cell,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
        if material == 218 {
            journal
                .set_container(
                    cell,
                    Inventory {
                        slots: vec![ItemStack {
                            item: ItemKind::Block(16),
                            count: 1,
                        }],
                        ..Default::default()
                    },
                )
                .unwrap();
        }
    }
    for cell in [[69, 25, 7], [69, 25, 11]] {
        assert!(bogie_supported_with_facing(&chunks, &journal, cell, 0));
    }
    let payloads = placed
        .iter()
        .map(|(cell, _, _)| (*cell, journal.train_payload(*cell).unwrap()))
        .collect::<Vec<_>>();
    let actor = PlayerUuid::from_u128(119_028);
    let plan = plan_assembly(
        &chunks,
        &journal,
        actor,
        station,
        [69, 26, 8],
        VehicleKind::Locomotive,
    )
    .unwrap();
    assert_eq!(
        plan.vehicle
            .parts
            .iter()
            .find(|part| part.material == 210)
            .unwrap()
            .facing,
        3
    );
    assert_eq!(plan.vehicle.build_yaw(), 0.0);
    let id = commit(&mut chunks, &mut journal, &plan).unwrap();
    let restore = plan_dismantle(&chunks, &journal, actor, station, id).unwrap();
    commit(&mut chunks, &mut journal, &restore).unwrap();
    for (cell, material, facing) in placed {
        assert_eq!(chunks.block_at(cell), Some(material));
        assert_eq!(journal.object_state(cell).unwrap().facing, facing);
    }
    for (cell, payload) in payloads {
        assert_eq!(journal.train_payload(cell).unwrap(), payload);
    }
    assert_eq!(
        journal
            .container_at([69, 27, 10])
            .unwrap()
            .count(ItemKind::Block(16)),
        1
    );
}

#[test]
fn automatic_build_is_atomic_consumes_only_missing_parts_and_preserves_wood() {
    let (mut chunks, mut journal, station, chassis) = fixture(VehicleKind::Freight);
    let actor = PlayerUuid::from_u128(219);
    let guide = terra_core::train_blueprint::building_template_with_width(
        chassis,
        0,
        VehicleKind::Freight,
        4,
        3,
    );
    for part in &guide {
        block(&mut chunks, part.cell, 0);
        journal.record(part.cell, 0).unwrap();
    }
    let request = BuildRequest {
        actor,
        station,
        chassis,
        facing: 0,
        kind: VehicleKind::Freight,
        wheelbase: 4,
        width: 3,
        creative: false,
    };
    let before = journal.gameplay().clone();
    assert!(matches!(
        plan_blueprint(&chunks, &journal, request),
        Err(TrainError::MissingMaterials)
    ));
    assert_eq!(journal.gameplay(), &before);
    assert!(
        guide
            .iter()
            .all(|part| chunks.block_at(part.cell) == Some(0))
    );
    let mut bag = Inventory::default();
    for part in &guide {
        assert_eq!(
            bag.insert(ItemKind::for_wood(part.material, WoodSpecies::Cherry), 1),
            0
        );
    }
    journal
        .gameplay_mut()
        .inventories
        .insert(actor, bag.clone());
    let plan = plan_blueprint(&chunks, &journal, request).unwrap();
    let blocked = guide.last().unwrap().cell;
    block(&mut chunks, blocked, 3);
    journal.record(blocked, 3).unwrap();
    assert!(commit_blueprint(&mut chunks, &mut journal, &plan).is_err());
    assert_eq!(journal.gameplay().inventories[&actor], bag);
    block(&mut chunks, blocked, 0);
    journal.record(blocked, 0).unwrap();
    let plan = plan_blueprint(&chunks, &journal, request).unwrap();
    commit_blueprint(&mut chunks, &mut journal, &plan).unwrap();
    for part in &guide {
        assert_eq!(chunks.block_at(part.cell), Some(part.material));
        if ItemKind::supports_wood(part.material) {
            assert_eq!(
                journal.object_state(part.cell).unwrap().wood,
                WoodSpecies::Cherry
            );
        }
    }
    assert!(
        journal.gameplay().inventories[&actor]
            .slots
            .iter()
            .all(|s| s.count == 0)
    );
    let bag = journal.gameplay().inventories[&actor].clone();
    let again = plan_blueprint(&chunks, &journal, request).unwrap();
    assert!(again.cells.is_empty());
    commit_blueprint(&mut chunks, &mut journal, &again).unwrap();
    assert_eq!(journal.gameplay().inventories[&actor], bag);
    let assembly = plan_assembly(
        &chunks,
        &journal,
        actor,
        station,
        chassis,
        VehicleKind::Freight,
    )
    .unwrap();
    assert!(assembly.vehicle.valid());
}
#[test]
fn creative_build_needs_no_materials_and_rejects_missing_rails() {
    let (mut chunks, mut journal, station, chassis) = fixture(VehicleKind::Freight);
    let actor = PlayerUuid::from_u128(220);
    let guide = terra_core::train_blueprint::building_template_with_width(
        chassis,
        0,
        VehicleKind::Freight,
        4,
        3,
    );
    for part in &guide {
        block(&mut chunks, part.cell, 0);
        journal.record(part.cell, 0).unwrap();
    }
    let request = BuildRequest {
        actor,
        station,
        chassis,
        facing: 0,
        kind: VehicleKind::Freight,
        wheelbase: 4,
        width: 3,
        creative: true,
    };
    let plan = plan_blueprint(&chunks, &journal, request).unwrap();
    commit_blueprint(&mut chunks, &mut journal, &plan).unwrap();
    assert!(
        guide
            .iter()
            .all(|part| chunks.block_at(part.cell) == Some(part.material))
    );
    assert!(
        journal.gameplay().inventories[&actor]
            .slots
            .iter()
            .all(|stack| stack.count == 0)
    );
    block(&mut chunks, [0, 0, 1], 0);
    assert!(matches!(
        plan_blueprint(&chunks, &journal, request),
        Err(TrainError::Unsupported)
    ));
}
#[test]
fn wide_boilers_keep_placed_visual_pose_collision_and_roundtrip_metadata() {
    let (mut chunks, mut journal, _, _) = fixture(VehicleKind::Freight);
    let cell = [0, 3, 2];
    for x in -1..=1 {
        block(&mut chunks, [x, 2, 2], 212);
        journal.record([x, 2, 2], 212).unwrap();
    }
    block(&mut chunks, cell, 0);
    journal.record(cell, 0).unwrap();
    let plan = crate::placement::layout(&chunks, &journal, cell, 0, 219, [0.0; 2], None).unwrap();
    let state = plan
        .states
        .iter()
        .find(|(p, _)| *p == cell)
        .unwrap()
        .1
        .unwrap();
    assert_eq!(state.facing, 4);
    block(&mut chunks, cell, 219);
    journal.record(cell, 219).unwrap();
    journal.set_object_state(cell, Some(state)).unwrap();
    assert!(crate::interaction_shapes::space_intersects(
        &chunks,
        &journal,
        &Default::default(),
        [1, 5, 2],
        [1.3, 5.0, 2.3],
        [1.6, 5.8, 2.7]
    ));
    let mut loaded = EditJournal::default();
    for section in journal.changed_sections() {
        loaded.import_delta(journal.section_delta(section)).unwrap();
    }
    assert_eq!(loaded.object_state(cell), Some(state));
}

#[test]
fn wide_auto_build_checks_the_boiler_overhang_before_spending_or_publishing() {
    let (mut chunks, mut journal, station, chassis) = fixture(VehicleKind::Locomotive);
    let actor = PlayerUuid::from_u128(220);
    let guide = terra_core::train_blueprint::building_template_with_width(
        chassis,
        0,
        VehicleKind::Locomotive,
        4,
        3,
    );
    for part in &guide {
        block(&mut chunks, part.cell, 0);
        journal.record(part.cell, 0).unwrap();
    }
    let request = BuildRequest {
        actor,
        station,
        chassis,
        facing: 0,
        kind: VehicleKind::Locomotive,
        wheelbase: 4,
        width: 3,
        creative: true,
    };
    let plan = plan_blueprint(&chunks, &journal, request).unwrap();
    let boiler = guide
        .iter()
        .find(|part| terra_core::train_boiler::is_boiler(part.material))
        .unwrap();
    let obstruction = [boiler.cell[0] + 1, boiler.cell[1] + 1, boiler.cell[2]];
    block(&mut chunks, obstruction, 3);
    let before = journal.edited_blocks();
    assert!(matches!(
        plan_blueprint(&chunks, &journal, request),
        Err(TrainError::Occupied)
    ));
    assert!(commit_blueprint(&mut chunks, &mut journal, &plan).is_err());
    assert_eq!(journal.edited_blocks(), before);
    assert!(!journal.gameplay().inventories.contains_key(&actor));
}

#[test]
fn automatically_built_widths_assemble_the_whole_body_including_boiler_top_fittings() {
    for width in [1, 3] {
        let (mut chunks, mut journal, station, chassis) = fixture(VehicleKind::Locomotive);
        let actor = PlayerUuid::from_u128(220);
        let guide = terra_core::train_blueprint::building_template_with_width(
            chassis,
            0,
            VehicleKind::Locomotive,
            4,
            width,
        );
        // Retire the fixture skeleton, including its previous cargo where the
        // new fuel motor will be constructed.
        for x in -1..=1 {
            for y in 2..=6 {
                for z in -1..=7 {
                    block(&mut chunks, [x, y, z], 0);
                    journal.record([x, y, z], 0).unwrap();
                }
            }
        }
        let request = BuildRequest {
            actor,
            station,
            chassis,
            facing: 0,
            kind: VehicleKind::Locomotive,
            wheelbase: 4,
            width,
            creative: true,
        };
        let build = plan_blueprint(&chunks, &journal, request).unwrap();
        commit_blueprint(&mut chunks, &mut journal, &build).unwrap();
        let assembly = plan_assembly(
            &chunks,
            &journal,
            actor,
            station,
            chassis,
            VehicleKind::Locomotive,
        )
        .unwrap();
        assert_eq!(
            assembly.vehicle.parts.len(),
            guide.len(),
            "every visual fitting must join width {width}"
        );
        assert!(
            assembly
                .vehicle
                .parts
                .iter()
                .any(|part| part.material == 224)
        );
        assert!(
            assembly
                .vehicle
                .parts
                .iter()
                .any(|part| part.material == 225)
        );
        assert!(assembly.vehicle.valid());
        let id = commit(&mut chunks, &mut journal, &assembly).unwrap();
        assert_eq!(
            journal.gameplay().trains.vehicles[&id].parts.len(),
            guide.len()
        );
        assert!(
            guide
                .iter()
                .all(|part| chunks.block_at(part.cell) == Some(0))
        );
        let restore = plan_dismantle(&chunks, &journal, actor, station, id).unwrap();
        commit(&mut chunks, &mut journal, &restore).unwrap();
        assert!(guide.iter().all(|part| {
            terra_core::train_blueprint::matches(
                *part,
                chunks.block_at(part.cell),
                journal
                    .object_state(part.cell)
                    .map_or(0, |state| state.facing),
            )
        }));
    }
}

#[test]
fn finite_dock_water_survives_dismantling_and_reassembly_without_duplication() {
    let (mut chunks, mut journal, station, start) = fixture(VehicleKind::Locomotive);
    let actor = PlayerUuid::from_u128(1);
    let tank = [1, 3, 4];
    block(&mut chunks, tank, 254);
    journal.record(tank, 254).unwrap();
    journal
        .set_object_state(tank, Some(BlockObjectState::default()))
        .unwrap();
    journal
        .set_object_state(station, Some(BlockObjectState::default()))
        .unwrap();
    let plan = plan_assembly(
        &chunks,
        &journal,
        actor,
        station,
        start,
        VehicleKind::Locomotive,
    )
    .unwrap();
    let id = commit(&mut chunks, &mut journal, &plan).unwrap();
    journal
        .set_plumbing(
            station,
            crate::plumbing::PlumbingState {
                water: crate::water::FluidCell {
                    amount: 4,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
    let mut runtime = crate::train_services::Runtime::default();
    crate::train_services::advance(&mut runtime, &chunks, &mut journal, 0.2).unwrap();
    assert_eq!(journal.gameplay().trains.vehicles[&id].steam.water, 125.0);
    assert_eq!(journal.plumbing_at(station).unwrap().water.amount, 3);
    let plan = plan_dismantle(&chunks, &journal, actor, station, id).unwrap();
    commit(&mut chunks, &mut journal, &plan).unwrap();
    let dock = journal.plumbing_at(station).unwrap();
    assert_eq!(dock.train_reserve.unwrap().water, 125.0);
    assert_eq!(dock.train_reserve.unwrap().pressure_bar, 0.0);
    let plan = plan_assembly(
        &chunks,
        &journal,
        actor,
        station,
        start,
        VehicleKind::Locomotive,
    )
    .unwrap();
    let rebuilt = commit(&mut chunks, &mut journal, &plan).unwrap();
    assert_eq!(
        journal.gameplay().trains.vehicles[&rebuilt].steam.water,
        125.0
    );
    assert!(
        journal
            .plumbing_at(station)
            .unwrap()
            .train_reserve
            .is_none()
    );
    assert_eq!(journal.plumbing_at(station).unwrap().water.amount, 3);
}

#[test]
fn train_service_never_draws_water_while_moving_unloaded_protected_or_at_revision_overflow() {
    for failure in ["moving", "unloaded", "protected", "overflow"] {
        let (mut chunks, mut journal, station, start) = fixture(VehicleKind::Locomotive);
        let actor = PlayerUuid::from_u128(1);
        let tank = [1, 3, 4];
        block(&mut chunks, tank, 254);
        journal.record(tank, 254).unwrap();
        journal
            .set_object_state(tank, Some(BlockObjectState::default()))
            .unwrap();
        journal
            .set_object_state(station, Some(BlockObjectState::default()))
            .unwrap();
        let plan = plan_assembly(
            &chunks,
            &journal,
            actor,
            station,
            start,
            VehicleKind::Locomotive,
        )
        .unwrap();
        let id = commit(&mut chunks, &mut journal, &plan).unwrap();
        journal
            .set_plumbing(
                station,
                crate::plumbing::PlumbingState {
                    water: crate::water::FluidCell {
                        amount: 4,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();
        match failure {
            "moving" => {
                let car = journal.gameplay_mut().trains.vehicles.get_mut(&id).unwrap();
                car.motion.speed_mps = 1.0;
                car.motion.brake = false;
            }
            "unloaded" => {
                chunks
                    .loaded
                    .remove(&terra_voxel::SectionPos::new(-1, 0, 0));
            }
            "protected" => {
                journal
                    .gameplay_mut()
                    .kingdoms
                    .found(PlayerUuid::from_u128(999), station, "Foreign service area")
                    .unwrap();
            }
            "overflow" => journal.gameplay_mut().revision = u64::MAX - 1,
            _ => unreachable!(),
        }
        let before = journal.plumbing_at(station).unwrap().clone();
        let water = journal.gameplay().trains.vehicles[&id].steam.water;
        crate::train_services::advance(&mut Default::default(), &chunks, &mut journal, 0.2)
            .unwrap();
        assert_eq!(journal.plumbing_at(station), Some(&before), "{failure}");
        assert_eq!(
            journal.gameplay().trains.vehicles[&id].steam.water,
            water,
            "{failure}"
        );
    }
}

#[test]
fn a_parked_steam_boiler_supplies_one_real_shaft_and_spends_pressure_and_water_under_load() {
    let (mut chunks, mut journal, station, start) = fixture(VehicleKind::Locomotive);
    let actor = PlayerUuid::from_u128(1);
    journal
        .set_object_state(station, Some(BlockObjectState::default()))
        .unwrap();
    let mut extra = (0..6).map(|z| ([1, 4, z], 219)).collect::<Vec<_>>();
    extra.extend([
        ([1, 3, 3], 218),
        ([1, 4, 6], 229),
        ([2, 4, 6], 230),
        ([1, 3, 4], 254),
    ]);
    for (cell, material) in extra {
        block(&mut chunks, cell, material);
        journal.record(cell, material).unwrap();
        journal
            .set_object_state(cell, Some(BlockObjectState::default()))
            .unwrap();
    }
    let plan = plan_assembly(
        &chunks,
        &journal,
        actor,
        station,
        start,
        VehicleKind::Locomotive,
    )
    .unwrap();
    let id = commit(&mut chunks, &mut journal, &plan).unwrap();
    let car = journal.gameplay_mut().trains.vehicles.get_mut(&id).unwrap();
    car.steam = terra_core::train_power::SteamState {
        water: 1000.0,
        pressure_bar: 9.0,
    };
    for (cell, facing) in [([4, 2, 2], 0), ([3, 2, 3], 1)] {
        block(&mut chunks, cell, 144);
        journal.record(cell, 144).unwrap();
        journal
            .set_object_state(
                cell,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let drives = crate::train_services::mechanical_drives(&chunks, &journal);
    assert_eq!(
        drives.len(),
        1,
        "several shafts must not multiply one boiler's capacity"
    );
    let (&shaft, drive) = drives.first_key_value().unwrap();
    assert_eq!(drive.vehicle, id);
    journal.replace_kinetic_states(std::collections::BTreeMap::from([(
        shaft,
        terra_core::kinetic::KineticState {
            rpm: 40.0,
            stress: 32.0,
            capacity: 128.0,
            available_capacity: 96.0,
            overloaded: false,
        },
    )]));
    let before = journal.gameplay().trains.vehicles[&id].steam;
    crate::train_services::advance(&mut Default::default(), &chunks, &mut journal, 0.2).unwrap();
    let after = journal.gameplay().trains.vehicles[&id].steam;
    assert!(after.water < before.water && after.pressure_bar < before.pressure_bar);
    journal
        .gameplay_mut()
        .trains
        .vehicles
        .get_mut(&id)
        .unwrap()
        .steam
        .water = 0.0;
    assert!(crate::train_services::mechanical_drives(&chunks, &journal).is_empty());
}
