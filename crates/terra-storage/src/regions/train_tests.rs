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
        id: 1,
        owner: PlayerUuid::from_u128(17),
        kind,
        anchor,
        parts,
        cargo,
        couplings: [None, None],
        revision: 1,
    }
}
#[test]
fn id13_preserves_the_parked_consist_exact_parts_owner_mass_and_private_cargo_across_real_save_reload()
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
    assert_eq!(&read(&root.join(INDEX)).unwrap()[12..16], b"ID13");
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
fn a_frozen_id12_index_loads_with_no_invented_train_and_empty_id13_keeps_retired_ids() {
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
    assert_eq!(&read(&root.join(INDEX)).unwrap()[12..16], b"ID13");
    fs::remove_dir_all(root).unwrap();
}
