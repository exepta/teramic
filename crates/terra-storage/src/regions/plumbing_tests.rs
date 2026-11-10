use super::*;
use terra_core::{brewing, campfire, plumbing as core};
use terra_world::{edits::BlockObjectState, plumbing::PlumbingState, water::FluidCell};

fn node(journal: &mut EditJournal, position: [i64; 3], material: u16, facing: u8) {
    journal.record(position, material).unwrap();
    journal
        .set_object_state(
            position,
            Some(BlockObjectState {
                facing,
                ..Default::default()
            }),
        )
        .unwrap();
}

fn attached_fire(journal: &mut EditJournal, position: [i64; 3]) {
    node(journal, position, brewing::MATERIAL, brewing::ATTACHED_FIRE);
    journal
        .commit_attached_campfire(position, campfire::AttachedCampfire::default())
        .unwrap();
}

fn fixture() -> EditJournal {
    let mut journal = EditJournal::default();
    let shape = core::BasinShape {
        origin: [-1, 15, -1],
        size: [2, 2, 2],
    };
    for (index, position) in shape.cells().enumerate() {
        node(&mut journal, position, core::BASIN, 0);
        journal
            .set_plumbing(
                position,
                PlumbingState {
                    train_reserve: None,
                    water: FluidCell {
                        amount: index as u8 + 1,
                        flow: [0, 0],
                        source: false,
                    },
                    basin: Some(shape),
                    ..Default::default()
                },
            )
            .unwrap();
    }
    for (index, material) in (core::WOOD_PIPE..=core::PUMP)
        .chain([core::GLASS_PIPE])
        .enumerate()
    {
        let position = [-10 - index as i64, 18, -1];
        let facing = if material == core::VALVE {
            8
        } else if material == core::FAUCET {
            5
        } else if material == core::GLASS_PIPE {
            4
        } else {
            0
        };
        node(&mut journal, position, material, facing);
        journal
            .set_plumbing(
                position,
                PlumbingState {
                    train_reserve: None,
                    water: FluidCell {
                        amount: index as u8,
                        flow: [-1, 1],
                        source: false,
                    },
                    pump_credit: if material == core::PUMP { 2.75 } else { 0. },
                    basin: None,
                    intake: None,
                    flowing: false,
                },
            )
            .unwrap();
    }
    attached_fire(&mut journal, [-8, 18, -1]);
    attached_fire(&mut journal, [512, 18, 0]);
    journal
}

#[test]
fn rg12_roundtrips_finite_water_credit_and_cross_region_basins_without_loaded_chunks() {
    let root = std::env::temp_dir().join(format!("terra-plumbing-{}", PlayerUuid::new_v4()));
    let world = WorldId(1660);
    let mut journal = fixture();
    let expected: BTreeMap<_, _> = journal
        .plumbing()
        .map(|(position, state)| (position, state.clone()))
        .collect();
    let revision = journal.gameplay().revision;
    for _ in 0..3 {
        save_edits(&root, world, &journal).unwrap();
        for owner in [[-1, -1], [-1, 0], [0, -1], [0, 0]] {
            let bytes = read(&root.join(region_name(owner))).unwrap();
            assert_eq!(bytes.get(12..16), Some(b"RG12".as_slice()));
            assert_eq!(read_creature_region(&bytes).unwrap().region.position, owner);
        }
        assert_eq!(
            read(&root.join(region_name([1, 0]))).unwrap().get(12..16),
            Some(b"RG11".as_slice()),
            "regions without plumbing retain the frozen prior format"
        );
        let loaded = load_edits(&root, world).unwrap();
        assert_eq!(
            loaded
                .plumbing()
                .map(|(position, state)| (position, state.clone()))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
        assert_eq!(loaded.gameplay().revision, revision);
        for position in [[-8, 18, -1], [512, 18, 0]] {
            assert_eq!(
                loaded.attached_campfire_at(position),
                journal.attached_campfire_at(position)
            );
        }
        assert_eq!(loaded.object_state([-13, 18, -1]).unwrap().facing, 8);
        assert_eq!(loaded.object_state([-14, 18, -1]).unwrap().facing, 5);
        assert_eq!(loaded.block([-16, 18, -1]), Some(core::GLASS_PIPE));
        assert_eq!(loaded.object_state([-16, 18, -1]).unwrap().facing, 4);
        assert_eq!(loaded.plumbing_at([-16, 18, -1]).unwrap().water.amount, 6);
        journal = loaded;
    }
    for position in expected.keys() {
        journal.record(*position, 0).unwrap();
    }
    assert!(journal.plumbing().next().is_none());
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert!(loaded.plumbing().next().is_none());
    assert!(loaded.has_attached_campfire([-8, 18, -1]));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rg12_rejects_duplicate_orphan_misplaced_nonfinite_and_invalid_water_or_basin_records() {
    let root = std::env::temp_dir().join(format!("terra-bad-plumbing-{}", PlayerUuid::new_v4()));
    let world = WorldId(1661);
    save_edits(&root, world, &fixture()).unwrap();
    let path = root.join(region_name([-1, -1]));
    let original = read(&path).unwrap();
    for mutation in 0..11 {
        let mut data: PlumbingRegion = decode(b"RG12", &original).unwrap();
        let pipe = data
            .plumbing
            .iter()
            .position(|(position, _)| *position == [-10, 18, -1])
            .unwrap();
        let basin = data
            .plumbing
            .iter()
            .position(|(_, state)| state.basin.is_some())
            .unwrap();
        match mutation {
            0 => data.plumbing.push(data.plumbing[pipe].clone()),
            1 => data.plumbing[pipe].0 = [-100, 18, -1],
            2 => data.plumbing[pipe].0 = [512, 18, -1],
            3 => data.plumbing[pipe].1.water.amount = 9,
            4 => {
                data.plumbing[pipe].1.water.amount = 1;
                data.plumbing[pipe].1.water.source = true;
            }
            5 => data.plumbing[pipe].1.water.flow = [2, 0],
            6 => data.plumbing[pipe].1.pump_credit = f32::NAN,
            7 => data.plumbing[pipe].1.pump_credit = 8.5,
            8 => data.plumbing[pipe].1.basin = data.plumbing[basin].1.basin,
            9 => data.plumbing[basin].1.basin.as_mut().unwrap().size = [6, 2, 2],
            _ => data.plumbing[basin].1.basin.as_mut().unwrap().origin = [-50, 15, -1],
        }
        std::fs::write(&path, encode(b"RG12", &data).unwrap()).unwrap();
        assert!(load_edits(&root, world).is_err(), "mutation {mutation}");
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rg12_enforces_global_node_capacity_across_individually_bounded_regions() {
    let root = std::env::temp_dir().join(format!("terra-plumbing-cap-{}", PlayerUuid::new_v4()));
    let world = WorldId(1662);
    let mut journal = EditJournal::default();
    for position in [[-1, 18, -1], [512, 18, 0]] {
        node(&mut journal, position, core::IRON_PIPE, 0);
        journal.set_plumbing(position, Default::default()).unwrap();
    }
    save_edits(&root, world, &journal).unwrap();
    for (owner, count) in [([-1, -1], 4096), ([1, 0], 4097)] {
        let path = root.join(region_name(owner));
        let mut data: PlumbingRegion = decode(b"RG12", &read(&path).unwrap()).unwrap();
        data.plumbing = (0..count)
            .map(|index| {
                (
                    [
                        owner[0] * 512 + index % 512,
                        18,
                        owner[1] * 512 + index / 512,
                    ],
                    PlumbingState::default(),
                )
            })
            .collect();
        std::fs::write(&path, encode(b"RG12", &data).unwrap()).unwrap();
    }
    assert_eq!(
        load_edits(&root, world).err().unwrap(),
        "Too many saved plumbing nodes"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rg11_without_plumbing_remains_readable_with_unchanged_inner_payload() {
    let root = std::env::temp_dir().join(format!("terra-plumbing-prior-{}", PlayerUuid::new_v4()));
    let world = WorldId(1663);
    save_edits(&root, world, &fixture()).unwrap();
    for owner in [[-1, -1], [-1, 0], [0, -1], [0, 0]] {
        let path = root.join(region_name(owner));
        let data: PlumbingRegion = decode(b"RG12", &read(&path).unwrap()).unwrap();
        std::fs::write(&path, encode(b"RG11", &data.prior).unwrap()).unwrap();
    }
    let loaded = load_edits(&root, world).unwrap();
    assert!(loaded.plumbing().next().is_none());
    assert_eq!(loaded.block([-1, 15, -1]), Some(core::BASIN));
    assert!(loaded.has_attached_campfire([-8, 18, -1]));
    assert!(loaded.has_attached_campfire([512, 18, 0]));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn confirmed_transfer_is_ephemeral_and_does_not_change_the_saved_water_format() {
    let idle = PlumbingState {
        water: FluidCell {
            amount: 3,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut moving = idle.clone();
    moving.flowing = true;
    let bytes = postcard::to_allocvec(&moving).unwrap();
    assert_eq!(bytes, postcard::to_allocvec(&idle).unwrap());
    let restored: PlumbingState = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(restored, idle);
    assert!(!restored.flowing);
}

#[test]
fn rg14_persists_dock_reserves_without_changing_the_frozen_plumbing_payload() {
    let root = std::env::temp_dir().join(format!("terra-train-reserve-{}", PlayerUuid::new_v4()));
    let world = WorldId(2261);
    let cell = [-17, 18, -1];
    let mut journal = fixture();
    node(&mut journal, cell, 217, 0);
    let state = PlumbingState {
        water: FluidCell {
            amount: 3,
            ..Default::default()
        },
        ..Default::default()
    };
    journal.set_plumbing(cell, state.clone()).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(region_name(region_at(cell)));
    let prior = read(&path).unwrap();
    let reserve = terra_core::train_power::SteamState {
        water: 1234.5,
        pressure_bar: 0.0,
    };
    journal
        .set_plumbing(
            cell,
            PlumbingState {
                train_reserve: Some(reserve),
                ..state
            },
        )
        .unwrap();
    save_edits(&root, world, &journal).unwrap();
    let bytes = read(&path).unwrap();
    assert_eq!(bytes.get(12..16), Some(b"RG14".as_slice()));
    let wrapped: TrainServiceRegion = decode(b"RG14", &bytes).unwrap();
    assert_eq!(
        wrapped.prior, prior,
        "new reserves must wrap the exact previous bytes"
    );
    assert_eq!(wrapped.reserves, vec![(cell, reserve)]);
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(
        loaded.plumbing_at(cell).unwrap().train_reserve,
        Some(reserve)
    );
    assert_eq!(loaded.plumbing_at(cell).unwrap().water.amount, 3);
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(read(&path).unwrap(), bytes);
    for failure in 0..5 {
        let mut bad: TrainServiceRegion = decode(b"RG14", &bytes).unwrap();
        match failure {
            0 => bad.reserves.push(bad.reserves[0]),
            1 => bad.reserves[0].0 = [-18, 18, -1],
            2 => bad.reserves[0].1.water = f64::NAN,
            3 => bad.reserves[0].1.pressure_bar = 1.0,
            4 => bad.reserves[0].0 = [600, 18, -1],
            _ => unreachable!(),
        };
        std::fs::write(&path, encode(b"RG14", &bad).unwrap()).unwrap();
        assert!(load_edits(&root, world).is_err(), "case {failure}");
    }
    std::fs::remove_dir_all(root).unwrap();
}
