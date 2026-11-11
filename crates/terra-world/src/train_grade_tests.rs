//! Grade contacts and recovery retain real obstacle collision.
use super::*;
use crate::train_motion::tests::{fixture, set};
use terra_core::{
    Inventory, ItemKind, ItemStack,
    block_shapes::ShapeBox,
    rails::{CurveSpec, Piece},
    train_blueprint,
    trains::{TrainPart, VehicleKind},
};

#[test]
fn suspension_only_accepts_shallow_contact_with_loaded_rail_supports() {
    let (mut chunks, journal, _) = fixture();
    let properties = Default::default();
    let contact = ShapeBox {
        minimum: [0.2, -0.2, 4.2],
        maximum: [0.8, 0.7, 4.8],
        hinged: false,
    };
    assert_eq!(
        Clearance::new(&chunks, &journal, &properties).shape(contact, true),
        Ok(())
    );
    assert_eq!(
        Clearance::new(&chunks, &journal, &properties).shape(contact, false),
        Err(StopReason::Obstructed),
        "body and decoration cannot borrow the running-gear allowance"
    );
    let deep = ShapeBox {
        minimum: [0.2, -0.6, 4.2],
        ..contact
    };
    assert_eq!(
        Clearance::new(&chunks, &journal, &properties).shape(deep, true),
        Err(StopReason::Obstructed)
    );
    set(&mut chunks, [2, -1, 4], 3);
    let outside = ShapeBox {
        minimum: [2.2, -0.2, 4.2],
        maximum: [2.8, 0.7, 4.8],
        ..contact
    };
    assert_eq!(
        Clearance::new(&chunks, &journal, &properties).shape(outside, true),
        Err(StopReason::Obstructed),
        "ground beside the authored support footprint still blocks wheels"
    );
    set(&mut chunks, [0, 0, 4], 3);
    assert_eq!(
        Clearance::new(&chunks, &journal, &properties).shape(contact, true),
        Err(StopReason::Obstructed),
        "a wall replacing the rail cannot supply support through old metadata"
    );
}

fn descending_fixture() -> (ChunkStore, EditJournal, RouteNetwork, Vehicle) {
    let (mut chunks, mut journal, mut car) = fixture();
    for x in -4..=4 {
        for y in -4..=10 {
            for z in -4..=100 {
                if chunks.block_at([x, y, z]).is_none() {
                    set(&mut chunks, [x, y, z], 0);
                }
            }
        }
    }
    let grade = Piece::new_curve(
        [0, 0, 36],
        CurveSpec::new([[8, 2, 0], [8, 2, 24], [8, -14, 40], [8, -14, 64]]).unwrap(),
    )
    .unwrap();
    let pieces = std::iter::once(grade).chain(
        (40..=96)
            .step_by(4)
            .map(|z| Piece::new([0, -1, z], terra_core::rails::STRAIGHT, 0).unwrap()),
    );
    for piece in pieces {
        for support in piece.supports().iter() {
            set(
                &mut chunks,
                std::array::from_fn(|axis| piece.anchor[axis] + i64::from(support.support[axis])),
                3,
            );
        }
        for cell in rails::cells(piece) {
            set(&mut chunks, cell, piece.material);
            journal.record(cell, piece.material).unwrap();
            journal
                .set_object_state(cell, Some(rails::object_state(piece, cell)))
                .unwrap();
        }
    }
    car.parts =
        train_blueprint::building_template_with_width([0, 2, 4], 0, VehicleKind::Locomotive, 8, 3)
            .into_iter()
            .map(|part| TrainPart {
                offset: part.cell.map(|coordinate| coordinate as i16),
                material: part.material,
                facing: part.facing,
                wood: Default::default(),
            })
            .collect();
    car.cargo.clear();
    for index in car.cargo_indices().collect::<Vec<_>>() {
        car.cargo.insert(index, Inventory::default());
    }
    let engine = car
        .parts
        .iter()
        .position(|part| part.material == terra_core::train_parts::ENGINE)
        .unwrap() as u16;
    car.cargo.get_mut(&engine).unwrap().slots = vec![ItemStack {
        item: ItemKind::Block(18),
        count: 8,
    }];
    car.motion.throttle = 1;
    car.motion.brake = false;
    assert!(car.valid());
    let network = rails::network(&journal).unwrap();
    (chunks, journal, network, car)
}

#[test]
fn broad_locomotive_crosses_the_short_crest_and_can_reverse_up_it() {
    let (chunks, journal, network, mut car) = descending_fixture();
    let properties = Default::default();
    let mut pitched = false;
    for _ in 0..900 {
        let next = advance(&chunks, &journal, &network, &car, 1.0 / 60.0).unwrap();
        clear_swept(&chunks, &journal, &properties, &car, &next).unwrap();
        pitched |= next.motion.pitch > 0.04;
        car = next;
    }
    assert!(pitched);
    assert!(car.motion.translation[1] < -0.99);
    assert!(car.motion.wheel_distance_m > 40.0);
    car.motion.speed_mps = 0.0;
    car.motion.throttle = -1;
    car.motion.stop_reason = StopReason::Obstructed;
    for _ in 0..800 {
        let next = advance(&chunks, &journal, &network, &car, 1.0 / 60.0).unwrap();
        clear_swept(&chunks, &journal, &properties, &car, &next).unwrap();
        car = next;
    }
    assert!(car.motion.translation[1].abs() < 1e-6);
    assert!(car.motion.speed_mps < -3.9);
    assert_eq!(car.motion.stop_reason, StopReason::Ready);
}

#[test]
fn a_wall_above_the_lower_track_stops_the_body_but_allows_reversing_away() {
    let (mut chunks, mut journal, network, mut car) = descending_fixture();
    let properties = Default::default();
    set(&mut chunks, [0, 0, 60], 3);
    journal.record([0, 0, 60], 3).unwrap();
    let mut blocked = false;
    for _ in 0..900 {
        let next = advance(&chunks, &journal, &network, &car, 1.0 / 60.0).unwrap();
        match clear_swept(&chunks, &journal, &properties, &car, &next) {
            Ok(_) => car = next,
            Err(StopReason::Obstructed) => {
                blocked = true;
                break;
            }
            Err(error) => panic!("unexpected obstruction: {error:?}"),
        }
    }
    assert!(blocked);
    let stopped_distance = car.motion.wheel_distance_m;
    assert!(
        stopped_distance > 30.0,
        "the grade itself must remain clear"
    );
    car.motion.speed_mps = 0.0;
    car.motion.throttle = -1;
    car.motion.brake = false;
    car.motion.stop_reason = StopReason::Obstructed;
    for _ in 0..240 {
        let next = advance(&chunks, &journal, &network, &car, 1.0 / 60.0).unwrap();
        clear_swept(&chunks, &journal, &properties, &car, &next).unwrap();
        car = next;
    }
    assert!(car.motion.wheel_distance_m < stopped_distance - 8.0);
    assert_eq!(car.motion.stop_reason, StopReason::Ready);
}
