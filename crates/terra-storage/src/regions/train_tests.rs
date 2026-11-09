use super::*;
use terra_core::{
    ItemKind, ItemStack, WoodSpecies, train_parts,
    trains::{CouplerEnd, TrainPart, TrainRegistry, Vehicle, VehicleKind},
};
fn temporary() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "teramic-train-save-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}
fn car(anchor: [i64; 3], kind: VehicleKind) -> Vehicle {
    let mut parts = Vec::new();
    for z in 0..=6 {
        parts.push(TrainPart {
            offset: [0, 1, z],
            material: if z == 2 {
                train_parts::CHASSIS
            } else {
                train_parts::FRAME
            },
            facing: 0,
            wood: WoodSpecies::Cherry,
        });
    }
    for (offset, material) in [
        ([0, 0, 1], train_parts::BOGIE),
        ([0, 0, 5], train_parts::BOGIE),
        ([0, 1, -1], train_parts::COUPLER),
        ([0, 1, 7], train_parts::COUPLER),
        (
            [0, 2, 3],
            if kind == VehicleKind::Locomotive {
                train_parts::CAB
            } else {
                train_parts::CARGO
            },
        ),
    ] {
        parts.push(TrainPart {
            offset,
            material,
            facing: 0,
            wood: WoodSpecies::Cherry,
        });
    }
    let cargo = parts
        .iter()
        .enumerate()
        .filter(|(_, part)| part.material == train_parts::CARGO)
        .map(|(index, _)| {
            (
                index as u16,
                Inventory {
                    slots: vec![ItemStack {
                        item: ItemKind::Block(3),
                        count: 47,
                    }],
                    revision: 9,
                    ..Default::default()
                },
            )
        })
        .collect();
    Vehicle {
        name: String::new(),
        id: 1,
        owner: PlayerUuid::from_u128(17),
        kind,
        anchor,
        parts,
        cargo,
        blocks: Default::default(),
        payloads: Default::default(),
        couplings: [None, None],
        revision: 1,

        motion: Default::default(),
        burn_seconds: 0.0,
    }
}
#[test]
fn id14_preserves_the_parked_consist_exact_parts_owner_mass_and_private_cargo_across_real_save_reload()
 {
    let root = temporary();
    let world = WorldId(21001);
    let mut journal = EditJournal::default();
    let registry = &mut journal.gameplay_mut().trains;
    let a = registry
        .insert(car([-17, 16, -17], VehicleKind::Locomotive))
        .unwrap();
    let b = registry
        .insert(car([-17, 16, -8], VehicleKind::Tender))
        .unwrap();
    registry
        .couple(
            PlayerUuid::from_u128(17),
            a,
            CouplerEnd::Front,
            b,
            CouplerEnd::Rear,
        )
        .unwrap();
    let expected = registry.clone();
    assert!(journal.gameplay().valid());
    save_edits(&root, world, &journal).unwrap();
    assert_eq!(&read(&root.join(INDEX)).unwrap()[12..16], b"ID16");
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().trains, expected);
    assert_eq!(loaded.gameplay().trains.consist(a).unwrap(), vec![a, b]);
    assert_eq!(
        loaded.gameplay().trains.vehicles[&b].mass_kg(),
        expected.vehicles[&b].mass_kg()
    );
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(
        load_edits(&root, world).unwrap().gameplay().trains,
        expected
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn a_frozen_id12_index_loads_with_no_invented_train_and_empty_id14_keeps_retired_ids() {
    let root = temporary();
    let world = WorldId(21002);
    let mut journal = EditJournal::default();
    let owner = PlayerUuid::from_u128(17);
    let kingdom = journal
        .gameplay_mut()
        .kingdoms
        .found(owner, [0, 16, 0], "Legacy")
        .unwrap();
    journal
        .record([0, 16, 0], terra_core::castle::BERGFRIED)
        .unwrap();
    save_edits(&root, world, &journal).unwrap();
    let bytes = read(&root.join(INDEX)).unwrap();
    assert_eq!(&bytes[12..16], b"ID12");
    let old: TreeIndex = decode(b"ID12", &bytes).unwrap();
    let frozen = encode(b"ID12", &old).unwrap();
    assert_eq!(frozen, bytes);
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().trains, TrainRegistry::default());
    assert!(loaded.gameplay().kingdoms.get(kingdom).is_some());
    let id = loaded
        .gameplay_mut()
        .trains
        .insert(car([-33, 16, -17], VehicleKind::Locomotive))
        .unwrap();
    loaded.gameplay_mut().trains.remove(owner, id).unwrap();
    let retired = loaded.gameplay().trains.clone();
    save_edits(&root, world, &loaded).unwrap();
    let mut again = load_edits(&root, world).unwrap();
    assert_eq!(again.gameplay().trains, retired);
    assert!(
        again
            .gameplay_mut()
            .trains
            .insert(car([-33, 16, -17], VehicleKind::Locomotive))
            .unwrap()
            > id
    );
    assert_eq!(&read(&root.join(INDEX)).unwrap()[12..16], b"ID16");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn id14_roundtrip_preserves_arbitrary_block_appearance_and_private_machine_payloads() {
    let root = temporary();
    let world = WorldId(21201);
    let mut vehicle = car([8, 12, 8], VehicleKind::Locomotive);
    let index = vehicle.parts.len() as u16;
    vehicle.parts.push(TrainPart {
        offset: [1, 1, 2],
        material: 105,
        facing: 75,
        wood: WoodSpecies::Cherry,
    });
    vehicle.blocks.insert(
        index,
        terra_core::trains::TrainBlockView {
            boxes: vec![terra_core::block_shapes::ShapeBox {
                minimum: [0.0; 3],
                maximum: [1.0; 3],
                hinged: false,
            }],
            tint: Some([31, 92, 211]),
            heraldry: Some(terra_core::kingdom::KingdomHeraldry::default()),
            ..Default::default()
        },
    );
    vehicle.payloads.insert(
        index,
        b"frozen private state with progress and inventories".to_vec(),
    );
    let mut journal = EditJournal::default();
    let id = journal.gameplay_mut().trains.insert(vehicle).unwrap();
    let expected = journal.gameplay().trains.clone();
    save_edits(&root, world, &journal).unwrap();
    assert_eq!(&read(&root.join(INDEX)).unwrap()[12..16], b"ID16");
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().trains, expected);
    assert_eq!(
        loaded.gameplay().trains.vehicles[&id].blocks[&index].tint,
        Some([31, 92, 211])
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn frozen_id13_vehicle_registry_loads_without_reinterpreting_new_fields() {
    let root = temporary();
    let world = WorldId(21202);
    let mut journal = EditJournal::default();
    journal
        .gameplay_mut()
        .trains
        .insert(car([8, 12, 8], VehicleKind::Locomotive))
        .unwrap();
    let expected = journal.gameplay().trains.clone();
    save_edits(&root, world, &journal).unwrap();
    let current = read_train_index(&read(&root.join(INDEX)).unwrap()).unwrap();
    let old = train_legacy::TrainIndex13 {
        prior: current.prior,
        trains: train_legacy::Registry13 {
            next_id: current.trains.next_id,
            revision: current.trains.revision,
            vehicles: current
                .trains
                .vehicles
                .into_iter()
                .map(|(id, car)| {
                    (
                        id,
                        train_legacy::Vehicle13 {
                            id: car.id,
                            owner: car.owner,
                            kind: car.kind,
                            anchor: car.anchor,
                            parts: car.parts,
                            cargo: car.cargo,
                            couplings: car.couplings,
                            revision: car.revision,
                        },
                    )
                })
                .collect(),
        },
    };
    fs::write(root.join(INDEX), encode(b"ID13", &old).unwrap()).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().trains, expected);
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(&read(&root.join(INDEX)).unwrap()[12..16], b"ID16");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn frozen_id14_keeps_machine_payloads_and_defaults_only_the_new_drive_state() {
    let root = temporary();
    let world = WorldId(21203);
    let mut vehicle = car([8, 12, 8], VehicleKind::Locomotive);
    vehicle
        .payloads
        .insert(0, b"private machine progress".to_vec());
    vehicle.blocks.insert(
        0,
        terra_core::trains::TrainBlockView {
            boxes: vec![terra_core::block_shapes::ShapeBox {
                minimum: [0.0, 0.08, 0.0],
                maximum: [1.0, 0.18, 1.0],
                hinged: false,
            }],
            tint: Some([40, 90, 150]),
            ..Default::default()
        },
    );
    let mut journal = EditJournal::default();
    journal.gameplay_mut().trains.insert(vehicle).unwrap();
    let mut expected = journal.gameplay().trains.clone();
    let expected_car = expected.vehicles.values_mut().next().unwrap();
    expected_car.blocks.get_mut(&0).unwrap().boxes = train_parts::boxes(train_parts::FRAME, 0);
    save_edits(&root, world, &journal).unwrap();
    let current = read_train_index(&read(&root.join(INDEX)).unwrap()).unwrap();
    let frozen = train_legacy::TrainIndex14 {
        prior: current.prior,
        trains: train_legacy::Registry14 {
            next_id: current.trains.next_id,
            revision: current.trains.revision,
            vehicles: current
                .trains
                .vehicles
                .into_iter()
                .map(|(id, car)| {
                    (
                        id,
                        train_legacy::Vehicle14 {
                            id: car.id,
                            owner: car.owner,
                            kind: car.kind,
                            anchor: car.anchor,
                            parts: car.parts,
                            cargo: car.cargo,
                            blocks: car.blocks,
                            payloads: car.payloads,
                            couplings: car.couplings,
                            revision: car.revision,
                        },
                    )
                })
                .collect(),
        },
    };
    fs::write(root.join(INDEX), encode(b"ID14", &frozen).unwrap()).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().trains, expected);
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(&read(&root.join(INDEX)).unwrap()[12..16], b"ID16");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn id15_preserves_current_pose_rail_cursor_and_partial_fuel_without_moving_the_build_anchor() {
    use terra_core::train_motion::{RailCursor, TrainMotion};
    let root = temporary();
    let world = WorldId(21204);
    let mut vehicle = car([8, 12, 8], VehicleKind::Locomotive);
    let engine = vehicle.parts.len() as u16;
    vehicle.parts.push(TrainPart {
        offset: [1, 1, 3],
        material: terra_core::locomotive_parts::ENGINE,
        facing: 0,
        wood: WoodSpecies::Oak,
    });
    vehicle.cargo.insert(
        engine,
        Inventory {
            slots: vec![ItemStack {
                item: ItemKind::Block(16),
                count: 2,
            }],
            ..Default::default()
        },
    );
    vehicle.motion = TrainMotion {
        translation: [24.25, 1.5, -12.75],
        pivot: [0.5, 1.0, 5.5],
        yaw: 0.6,
        pitch: -0.1,
        speed_mps: 4.2,
        throttle: 2,
        brake: false,
        wheel_distance_m: 31.125,
        fuel_available: true,
        cursor: Some(RailCursor {
            anchor: [32, 13, -8],
            path: 0,
            distance: 2.25,
            forward: true,
        }),
        ..Default::default()
    };
    vehicle.burn_seconds = 3.75;
    let expected_anchor = vehicle.anchor;
    let mut journal = EditJournal::default();
    let id = journal.gameplay_mut().trains.insert(vehicle).unwrap();
    let expected = journal.gameplay().trains.clone();
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().trains, expected);
    assert_eq!(
        loaded.gameplay().trains.vehicles[&id].anchor,
        expected_anchor
    );
    assert_eq!(loaded.gameplay().trains.vehicles[&id].burn_seconds, 3.75);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn frozen_id15_motion_and_fuel_survive_name_upgrade_and_new_names_roundtrip() {
    let root = temporary();
    let world = WorldId(21900);
    let mut journal = EditJournal::default();
    let id = journal
        .gameplay_mut()
        .trains
        .insert(car([8, 12, 8], VehicleKind::Locomotive))
        .unwrap();
    save_edits(&root, world, &journal).unwrap();
    let current = read_train_index(&read(&root.join(INDEX)).unwrap()).unwrap();
    let old = train_legacy::TrainIndex15 {
        prior: current.prior,
        trains: train_legacy::Registry15 {
            next_id: current.trains.next_id,
            revision: current.trains.revision,
            vehicles: current
                .trains
                .vehicles
                .into_iter()
                .map(|(id, car)| {
                    (
                        id,
                        train_legacy::Vehicle15 {
                            id: car.id,
                            owner: car.owner,
                            kind: car.kind,
                            anchor: car.anchor,
                            parts: car.parts,
                            cargo: car.cargo,
                            blocks: car.blocks,
                            payloads: car.payloads,
                            motion: car.motion,
                            burn_seconds: car.burn_seconds,
                            couplings: car.couplings,
                            revision: car.revision,
                        },
                    )
                })
                .collect(),
        },
    };
    fs::write(root.join(INDEX), encode(b"ID15", &old).unwrap()).unwrap();
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().trains, journal.gameplay().trains);
    loaded
        .gameplay_mut()
        .trains
        .vehicles
        .get_mut(&id)
        .unwrap()
        .name = "TERAMIC EXPRESS".into();
    save_edits(&root, world, &loaded).unwrap();
    let again = load_edits(&root, world).unwrap();
    assert_eq!(again.gameplay().trains, loaded.gameplay().trains);
    assert_eq!(&read(&root.join(INDEX)).unwrap()[12..16], b"ID16");
    fs::remove_dir_all(root).unwrap();
}
