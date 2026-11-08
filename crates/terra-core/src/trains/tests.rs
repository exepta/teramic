use super::*;
use crate::{ItemKind, ItemStack};
fn car(anchor: Cell, kind: VehicleKind) -> Vehicle {
    let mut parts = Vec::new();
    let mut add = |offset, material| {
        parts.push(TrainPart {
            offset,
            material,
            facing: 0,
            wood: WoodSpecies::Birch,
        })
    };
    for z in 0..=6 {
        add(
            [0, 1, z],
            if z == 2 {
                train_parts::CHASSIS
            } else {
                train_parts::FRAME
            },
        );
    }
    add([0, 0, 1], train_parts::BOGIE);
    add([0, 0, 5], train_parts::BOGIE);
    add([0, 1, -1], train_parts::COUPLER);
    add([0, 1, 7], train_parts::COUPLER);
    add(
        [0, 2, 3],
        match kind {
            VehicleKind::Locomotive => train_parts::CAB,
            VehicleKind::Passenger => train_parts::SEAT,
            _ => train_parts::CARGO,
        },
    );
    let cargo = parts
        .iter()
        .enumerate()
        .filter(|(_, part)| part.material == train_parts::CARGO)
        .map(|(index, _)| (index as u16, Inventory::default()))
        .collect();
    Vehicle {
        id: 1,
        owner: PlayerUuid::from_u128(1),
        kind,
        anchor,
        parts,
        cargo,
        blocks: Default::default(),
        payloads: Default::default(),
        couplings: [None, None],
        revision: 0,

        motion: Default::default(),
        burn_seconds: 0.0,
    }
}
#[test]
fn every_vehicle_kind_has_real_parts_mass_seats_and_private_owned_cargo() {
    for kind in [
        VehicleKind::Locomotive,
        VehicleKind::Tender,
        VehicleKind::Passenger,
        VehicleKind::Freight,
    ] {
        let mut registry = TrainRegistry::default();
        let id = registry.insert(car([-17, 16, -17], kind)).unwrap();
        assert!(registry.valid());
        let vehicle = &registry.vehicles[&id];
        assert!(vehicle.mass_kg() > 1000);
        assert_eq!(
            vehicle.cargo_indices().count(),
            usize::from(matches!(kind, VehicleKind::Tender | VehicleKind::Freight))
        );
        assert_eq!(
            (0..vehicle.parts.len())
                .filter(|index| vehicle.seat_pose(*index).is_some())
                .count(),
            usize::from(matches!(
                kind,
                VehicleKind::Locomotive | VehicleKind::Passenger
            ))
        );
        let encoded = serde_json::to_string(&registry).unwrap();
        assert_eq!(
            serde_json::from_str::<TrainRegistry>(&encoded).unwrap(),
            registry
        );
        let public = serde_json::to_value(registry.snapshot(Vec::new())).unwrap();
        assert!(public["vehicles"][0].get("cargo").is_none());
    }
    let mut freight = car([0, 0, 0], VehicleKind::Freight);
    let empty = freight.mass_kg();
    freight
        .cargo
        .values_mut()
        .next()
        .unwrap()
        .slots
        .push(ItemStack {
            item: ItemKind::Block(3),
            count: 19,
        });
    assert_eq!(freight.mass_kg(), empty + 38);
}
#[test]
fn coupling_is_symmetric_aligned_owner_checked_and_atomic_on_revision_overflow() {
    let owner = PlayerUuid::from_u128(1);
    let mut registry = TrainRegistry::default();
    let a = registry
        .insert(car([0, 0, 0], VehicleKind::Locomotive))
        .unwrap();
    let b = registry
        .insert(car([0, 0, 9], VehicleKind::Tender))
        .unwrap();
    let untouched = registry.clone();
    assert_eq!(
        registry.couple(
            PlayerUuid::from_u128(2),
            a,
            CouplerEnd::Front,
            b,
            CouplerEnd::Rear
        ),
        Err(TrainError::NotOwner)
    );
    assert_eq!(registry, untouched);
    registry
        .couple(owner, a, CouplerEnd::Front, b, CouplerEnd::Rear)
        .unwrap();
    assert!(registry.valid());
    assert_eq!(registry.consist(a).unwrap(), vec![a, b]);
    assert_eq!(registry.remove(owner, a), Err(TrainError::Coupled));
    registry.uncouple(owner, a, CouplerEnd::Front).unwrap();
    assert_eq!(registry.consist(a).unwrap(), vec![a]);
    registry.revision = u64::MAX;
    let before = registry.clone();
    assert_eq!(
        registry.couple(owner, a, CouplerEnd::Front, b, CouplerEnd::Rear),
        Err(TrainError::Overflow)
    );
    assert_eq!(registry, before);
}
#[test]
fn disconnected_parts_duplicate_cells_and_sideways_couplers_cannot_be_imported() {
    let mut malformed = car([0, 0, 0], VehicleKind::Locomotive);
    malformed.parts.push(TrainPart {
        offset: [20, 0, 0],
        material: train_parts::FRAME,
        facing: 0,
        wood: WoodSpecies::Oak,
    });
    assert!(!malformed.valid());
    let mut registry = TrainRegistry::default();
    let a = registry
        .insert(car([0, 0, 0], VehicleKind::Locomotive))
        .unwrap();
    let b = registry
        .insert(car([1, 0, 0], VehicleKind::Passenger))
        .unwrap();
    let before = registry.clone();
    assert_eq!(
        registry.couple(
            PlayerUuid::from_u128(1),
            a,
            CouplerEnd::Front,
            b,
            CouplerEnd::Rear
        ),
        Err(TrainError::OutOfReach)
    );
    assert_eq!(registry, before);
    let mut duplicate = car([0, 0, 0], VehicleKind::Freight);
    duplicate.parts.push(duplicate.parts[0]);
    assert!(!duplicate.valid());
}

#[test]
fn moving_the_rigid_model_releases_its_old_build_pad_but_keeps_its_current_cells_reserved() {
    let mut registry = TrainRegistry::default();
    let first = registry
        .insert(car([0, 4, 0], VehicleKind::Locomotive))
        .unwrap();
    registry
        .vehicles
        .get_mut(&first)
        .unwrap()
        .motion
        .translation = [20.0, 0.0, 0.0];
    let second = registry
        .insert(car([0, 4, 0], VehicleKind::Freight))
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(registry.vehicles[&first].anchor, [0, 4, 0]);
    assert_eq!(
        registry.insert(car([20, 4, 0], VehicleKind::Freight)),
        Err(TrainError::Occupied)
    );
    assert!(registry.valid());
}

#[test]
fn analytic_rigid_bounds_match_independently_rotated_corners_at_every_build_axis() {
    for facing in 0..4 {
        let mut vehicle = car([9, 12, -7], VehicleKind::Locomotive);
        vehicle
            .parts
            .iter_mut()
            .find(|part| part.material == train_parts::CHASSIS)
            .unwrap()
            .facing = facing;
        vehicle.motion.yaw = 0.63;
        vehicle.motion.pitch = -0.27;
        vehicle.motion.pivot = [0.5, 0.875, 5.5];
        vehicle.motion.translation = [3.2, -0.4, 8.6];
        let index = 2;
        let shape = crate::block_shapes::ShapeBox {
            minimum: [-0.3, 0.1, 0.2],
            maximum: [1.2, 0.8, 0.9],
            hinged: false,
        };
        let transformed = vehicle.world_shape(index, shape);
        let rotated_pivot = vehicle
            .motion
            .rotate(vehicle.motion.pivot, vehicle.build_yaw());
        let origin: [f64; 3] = std::array::from_fn(|i| {
            vehicle.anchor[i] as f64
                + if i == 1 { MODEL_Y_OFFSET } else { 0.0 }
                + vehicle.motion.translation[i]
                + vehicle.motion.pivot[i]
                - rotated_pivot[i]
        });
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for corner in 0..8 {
            let local = std::array::from_fn(|i| {
                f64::from(vehicle.parts[index].offset[i])
                    + if corner & (1 << i) == 0 {
                        shape.minimum[i]
                    } else {
                        shape.maximum[i]
                    }
            });
            let rotated = vehicle.motion.rotate(local, vehicle.build_yaw());
            let point = std::array::from_fn::<_, 3, _>(|i| origin[i] + rotated[i]);
            let restored = vehicle.local_point(point);
            for i in 0..3 {
                low[i] = low[i].min(point[i]);
                high[i] = high[i].max(point[i]);
                assert!((restored[i] - local[i]).abs() < 1e-9);
            }
        }
        for i in 0..3 {
            assert!((transformed.minimum[i] - low[i]).abs() < 1e-9);
            assert!((transformed.maximum[i] - high[i]).abs() < 1e-9);
        }
    }
}

#[test]
fn parked_model_origin_and_seat_feet_share_exact_authored_wheel_and_cushion_offsets() {
    for kind in [VehicleKind::Locomotive, VehicleKind::Passenger] {
        let vehicle = car([-17, 16, -17], kind);
        let origin = vehicle.model_origin();
        assert_eq!(origin, [-17.0, 16.0 + MODEL_Y_OFFSET, -17.0]);
        let [low, high] = vehicle.model_bounds();
        assert!((0..3).all(|axis| low[axis] < high[axis]));
        for (index, part) in vehicle.parts.iter().enumerate() {
            if let Some((feet, _)) = vehicle.seat_pose(index) {
                let cell = vehicle.part_cell(index).unwrap();
                assert!((feet[1] + 0.015 - (cell[1] as f64 + MODEL_Y_OFFSET + 0.5)).abs() < 1e-10);
                assert!(matches!(
                    part.material,
                    train_parts::CAB | train_parts::SEAT
                ));
            }
        }
        assert_eq!(vehicle.view().model_bounds(), vehicle.model_bounds());
        assert_eq!(vehicle.view().model_origin(), origin);
    }
}

#[test]
fn arbitrary_block_views_preserve_full_facing_and_reject_invalid_collision_or_unbounded_payloads() {
    let mut vehicle = car([0, 8, 0], VehicleKind::Locomotive);
    let index = vehicle.parts.len() as u16;
    vehicle.parts.push(TrainPart {
        offset: [1, 1, 2],
        material: 105,
        facing: crate::construction::LAST_MIXED_SLAB,
        wood: WoodSpecies::Cherry,
    });
    vehicle.blocks.insert(
        index,
        TrainBlockView {
            boxes: vec![crate::block_shapes::ShapeBox {
                minimum: [0.0; 3],
                maximum: [1.0; 3],
                hinged: false,
            }],
            tint: Some([31, 92, 211]),
            heraldry: Some(crate::kingdom::KingdomHeraldry::default()),
            ..Default::default()
        },
    );
    vehicle
        .payloads
        .insert(index, b"private machine state".to_vec());
    assert!(vehicle.valid());
    assert!(vehicle.view().valid());
    assert_eq!(
        vehicle.view().parts[index as usize].facing,
        crate::construction::LAST_MIXED_SLAB
    );
    assert_eq!(vehicle.part_shapes(index as usize)[0].maximum, [1.0; 3]);
    let public = serde_json::to_value(vehicle.view()).unwrap();
    assert!(public.get("payloads").is_none());
    assert!(public.get("cargo").is_none());
    vehicle.blocks.get_mut(&index).unwrap().boxes[0].minimum[0] = f64::NAN;
    assert!(!vehicle.valid());
    vehicle.blocks.get_mut(&index).unwrap().boxes[0].minimum[0] = 0.0;
    vehicle
        .payloads
        .insert(index, vec![0; MAX_PAYLOAD_BYTES + 1]);
    assert!(!vehicle.valid());
}

#[test]
fn aggregate_collision_and_private_payload_limits_are_atomic_for_registry_and_snapshot() {
    let shape = crate::block_shapes::ShapeBox {
        minimum: [0.0; 3],
        maximum: [1.0; 3],
        hinged: false,
    };
    let mut dense = car([0, 8, 0], VehicleKind::Locomotive);
    for index in 0..dense.parts.len() {
        dense.blocks.insert(
            index as u16,
            TrainBlockView {
                boxes: vec![shape; 128],
                ..Default::default()
            },
        );
    }
    assert!(dense.valid());
    let count = shape_count(&dense.parts, &dense.blocks);
    let capacity = MAX_TOTAL_SHAPES / count;
    let mut registry = TrainRegistry::default();
    for index in 0..capacity {
        let mut car = dense.clone();
        car.anchor[0] = index as i64 * 100;
        registry.insert(car).unwrap();
    }
    let before = registry.clone();
    dense.anchor[0] = 100_000;
    assert_eq!(registry.insert(dense.clone()), Err(TrainError::Limit));
    assert_eq!(registry, before);
    let mut excessive = registry.snapshot(Vec::new());
    let mut view = dense.view();
    view.id = 999;
    excessive.vehicles.push(view);
    assert!(!excessive.valid());

    let mut packed = car([0, 8, 0], VehicleKind::Locomotive);
    for z in 0..=6 {
        packed.parts.push(TrainPart {
            offset: [1, 1, z],
            material: train_parts::FRAME,
            facing: 0,
            wood: WoodSpecies::Birch,
        });
    }
    for index in 0..16 {
        packed.payloads.insert(index, vec![0; MAX_PAYLOAD_BYTES]);
    }
    assert!(packed.valid());
    let mut registry = TrainRegistry::default();
    for index in 0..8 {
        let mut car = packed.clone();
        car.anchor[0] = index * 100;
        registry.insert(car).unwrap();
    }
    let before = registry.clone();
    packed.anchor[0] = 100_000;
    assert_eq!(registry.insert(packed), Err(TrainError::Limit));
    assert_eq!(registry, before);
}
