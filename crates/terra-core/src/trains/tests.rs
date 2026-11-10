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
        name: String::new(),
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
        steam: Default::default(),
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
        for part in &mut vehicle.parts {
            if matches!(
                part.material,
                train_parts::CHASSIS | train_parts::BOGIE | train_parts::COUPLER
            ) {
                part.facing = facing;
            }
        }
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
fn opposite_bogie_yaw_keeps_the_historical_chassis_basis_for_saved_pitch() {
    let mut vehicle = car([8, 12, -7], VehicleKind::Locomotive);
    for part in &mut vehicle.parts {
        if matches!(part.material, train_parts::BOGIE | train_parts::COUPLER) {
            part.facing = 2;
        }
    }
    assert!(vehicle.valid());
    vehicle.motion.yaw = 0.35;
    vehicle.motion.pitch = -0.2;
    vehicle.motion.pivot = [0.5, 1.0, 5.5];
    vehicle.motion.translation = [1.25, 0.3, -2.0];
    assert_eq!(vehicle.build_yaw(), 0.0);
    let local = [0.8, 2.0, 3.5];
    let rotated = vehicle.motion.rotate(local, 0.0);
    let pivot = vehicle.motion.rotate(vehicle.motion.pivot, 0.0);
    let expected: [f64; 3] = std::array::from_fn(|i| {
        vehicle.anchor[i] as f64
            + if i == 1 { MODEL_Y_OFFSET } else { 0.0 }
            + vehicle.motion.translation[i]
            + vehicle.motion.pivot[i]
            - pivot[i]
            + rotated[i]
    });
    let actual = vehicle.world_point(local);
    for i in 0..3 {
        assert!((actual[i] - expected[i]).abs() < 1e-9);
    }
    assert_eq!(vehicle.view().build_yaw(), 0.0);
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

#[test]
fn cached_local_envelope_contains_all_rotated_shapes_and_observes_geometry_edits() {
    let mut vehicle = car([10, 20, -30], VehicleKind::Freight);
    for angle in [0.0, 0.35, 1.7, 3.0] {
        vehicle.motion.yaw = angle;
        vehicle.motion.pitch = 0.2;
        let bounds = vehicle.model_bounds();
        for index in 0..vehicle.parts.len() {
            for shape in vehicle.transformed_shapes(index) {
                for (axis, _) in shape.minimum.iter().enumerate() {
                    assert!(shape.minimum[axis] >= bounds[0][axis] - 1e-8);
                    assert!(shape.maximum[axis] <= bounds[1][axis] + 1e-8);
                }
            }
        }
        assert_eq!(bounds, vehicle.model_bounds());
    }
    vehicle.motion = Default::default();
    let before = vehicle.model_bounds();
    vehicle.parts[0].offset[0] += 20;
    assert!(vehicle.model_bounds()[1][0] > before[1][0] + 10.0);
}

#[test]
fn train_bounds_repeated_query_cost_is_measured_against_the_previous_shape_walk() {
    let mut vehicle = car([8123, 10, -64], VehicleKind::Locomotive);
    for index in 0..70 {
        vehicle.parts.push(TrainPart {
            offset: [index % 3, 3, index / 3],
            material: 219 + index as u16,
            facing: 0,
            wood: WoodSpecies::Oak,
        });
    }
    let _ = vehicle.model_bounds();
    let started = std::time::Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(vehicle.model_bounds());
    }
    let cached = started.elapsed();
    let started = std::time::Instant::now();
    for _ in 0..1000 {
        let transform = RigidTransform::new(vehicle.anchor, vehicle.motion, vehicle.build_yaw());
        let mut bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
        for (index, part) in vehicle.parts.iter().enumerate() {
            for shape in vehicle.part_shapes(index) {
                let shape = transform.bounds(part, shape);
                for (axis, _) in shape.minimum.iter().enumerate() {
                    bounds[0][axis] = bounds[0][axis].min(shape.minimum[axis]);
                    bounds[1][axis] = bounds[1][axis].max(shape.maximum[axis]);
                }
            }
        }
        std::hint::black_box(bounds);
    }
    eprintln!(
        "TRAIN_BOUNDS_1000_QUERIES cached_us={} previous_shape_walk_us={}",
        cached.as_micros(),
        started.elapsed().as_micros()
    );
}

#[test]
fn touching_couplers_use_fractional_world_positions_on_curves_and_slopes() {
    let owner = PlayerUuid::from_u128(1);
    for (yaw, pitch) in [(0.0, 0.0), (0.37, 0.0), (-0.8, 0.10), (2.1, -0.08)] {
        let mut registry = TrainRegistry::default();
        let a = registry
            .insert(car([0, 5, 0], VehicleKind::Locomotive))
            .unwrap();
        let b = registry
            .insert(car([0, 5, 9], VehicleKind::Tender))
            .unwrap();
        for id in [a, b] {
            let motion = &mut registry.vehicles.get_mut(&id).unwrap().motion;
            motion.yaw = yaw;
            motion.pitch = pitch;
        }
        let point = registry.vehicles[&a]
            .coupler_point(CouplerEnd::Front)
            .unwrap();
        let axis = registry.vehicles[&a]
            .coupler_direction(CouplerEnd::Front)
            .unwrap();
        let other = registry.vehicles[&b]
            .coupler_point(CouplerEnd::Rear)
            .unwrap();
        registry.vehicles.get_mut(&b).unwrap().motion.translation =
            std::array::from_fn(|i| point[i] + axis[i] * 0.9 - other[i]);
        assert!(
            registry.vehicles[&a]
                .coupling_distance(CouplerEnd::Front, &registry.vehicles[&b], CouplerEnd::Rear)
                .is_some()
        );
        assert!(
            registry.vehicles[&a]
                .coupling_distance(CouplerEnd::Rear, &registry.vehicles[&b], CouplerEnd::Rear)
                .is_none()
        );
        registry
            .couple(owner, a, CouplerEnd::Front, b, CouplerEnd::Rear)
            .unwrap();
        assert!(registry.valid());
    }
}

#[test]
fn saved_driving_wheel_profiles_follow_the_corrected_canonical_tread() {
    let part = TrainPart {
        material: 232,
        offset: [-1, -1, 0],
        facing: 0,
        wood: Default::default(),
    };
    let legacy = TrainBlockView {
        boxes: vec![crate::block_shapes::ShapeBox {
            minimum: [-0.088, 0.05, -0.35],
            maximum: [0.128, 1.75, 1.35],
            hinged: false,
        }],
        ..Default::default()
    };
    assert_eq!(part_shapes(&part, Some(&legacy)), part_shapes(&part, None));
    let wheel = part_shapes(&part, Some(&legacy));
    assert!(wheel.iter().all(|shape| shape.minimum[0] >= 0.3));
}
