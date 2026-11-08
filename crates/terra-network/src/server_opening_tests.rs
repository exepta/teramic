// Included in server_furniture::tests to exercise the existing authoritative
// transaction helpers, without creating a second placement implementation.
fn opening_toggle(
    app: &mut App,
    position: [i64; 3],
    material: u16,
    facing: u8,
) -> Result<bool, String> {
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    toggle(position, material, facing, &mut world)
}

fn resident_opening_toggle(
    app: &mut App,
    position: [i64; 3],
    material: u16,
    facing: u8,
    players: &[[f64; 3]],
) -> Result<bool, String> {
    let mut system = SystemState::<(
        ResMut<ChunkStore>,
        ResMut<EditJournal>,
        Res<terra_core::block_properties::BlockPropertiesCatalog>,
    )>::new(app.world_mut());
    let (mut chunks, mut journal, properties) = system.get_mut(app.world_mut()).unwrap();
    toggle_with_players(
        position,
        material,
        facing,
        &mut chunks,
        &mut journal,
        &properties,
        players,
    )
}

fn resident_door_tick(
    app: &mut App,
    doors: &mut super::super::creatures::ResidentDoors,
    now: f32,
    requests: std::collections::BTreeSet<[i64; 3]>,
    population: &terra_core::creatures::CreaturePopulation,
    players: &[[f64; 3]],
) {
    let mut system = SystemState::<(
        ResMut<ChunkStore>,
        ResMut<EditJournal>,
        Res<terra_core::block_properties::BlockPropertiesCatalog>,
    )>::new(app.world_mut());
    let (mut chunks, mut journal, properties) = system.get_mut(app.world_mut()).unwrap();
    doors.update(
        now,
        &requests,
        super::super::creatures::ResidentDoorWorld {
            chunks: &mut chunks,
            journal: &mut journal,
            properties: &properties,
            population,
            players,
        },
    );
}

#[test]
fn resident_doors_close_both_halves_after_passage_and_wait_for_players_and_creatures() {
    use terra_core::creatures::{Creature, CreatureKind, CreatureNeeds, CreaturePopulation};
    for facing in 0_u8..4 {
        let mut app = app();
        let anchor = [-16, 15, 8];
        opening_floor(&mut app, anchor);
        edit(
            &mut app,
            anchor,
            0,
            65,
            f32::from(facing) * std::f32::consts::FRAC_PI_2,
        )
        .unwrap();
        let initial = app
            .world()
            .resource::<EditJournal>()
            .object_state(anchor)
            .unwrap();
        let peer = initial.peer.unwrap();
        let mut doors = super::super::creatures::ResidentDoors::default();
        let mut population = CreaturePopulation::default();
        resident_door_tick(&mut app, &mut doors, 0.0, [anchor].into(), &population, &[]);
        for cell in [anchor, peer] {
            assert_eq!(
                app.world()
                    .resource::<EditJournal>()
                    .object_state(cell)
                    .unwrap()
                    .facing,
                initial.facing ^ 4
            );
        }
        resident_door_tick(&mut app, &mut doors, 2.9, [].into(), &population, &[]);
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .object_state(anchor)
                .unwrap()
                .facing,
            initial.facing ^ 4
        );
        let position = [
            anchor[0] as f64 + 0.5,
            anchor[1] as f64,
            anchor[2] as f64 + 0.5,
        ];
        resident_door_tick(
            &mut app,
            &mut doors,
            3.1,
            [].into(),
            &population,
            &[position],
        );
        population.animals.insert(
            1,
            Creature {
                id: 1,
                kind: CreatureKind::Villager,
                home: [0; 2],
                position,
                velocity: [0.0; 3],
                yaw: 0.0,
                health: CreatureKind::Villager.health(),
                behavior: terra_core::creatures::CreatureBehavior::Rest,
                timer: 0.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(CreatureKind::Villager),
            },
        );
        resident_door_tick(&mut app, &mut doors, 6.2, [].into(), &population, &[]);
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .object_state(anchor)
                .unwrap()
                .facing,
            initial.facing ^ 4
        );
        population.animals.get_mut(&1).unwrap().health = 0.0;
        resident_door_tick(&mut app, &mut doors, 9.3, [].into(), &population, &[]);
        for cell in [anchor, peer] {
            assert_eq!(
                app.world()
                    .resource::<EditJournal>()
                    .object_state(cell)
                    .unwrap()
                    .facing,
                initial.facing
            );
        }
    }
}

#[test]
fn resident_door_controller_preserves_manual_openings_and_player_closing() {
    let mut app = app();
    let anchor = [4, 15, 8];
    opening_floor(&mut app, anchor);
    edit(&mut app, anchor, 0, 65, 0.0).unwrap();
    let initial = app
        .world()
        .resource::<EditJournal>()
        .object_state(anchor)
        .unwrap();
    let mut doors = super::super::creatures::ResidentDoors::default();
    let population = terra_core::creatures::CreaturePopulation::default();
    assert!(resident_opening_toggle(&mut app, anchor, 65, initial.facing, &[]).unwrap());
    resident_door_tick(&mut app, &mut doors, 20.0, [].into(), &population, &[]);
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .object_state(anchor)
            .unwrap()
            .facing,
        initial.facing ^ 4
    );
    assert!(resident_opening_toggle(&mut app, anchor, 65, initial.facing ^ 4, &[]).unwrap());
    resident_door_tick(
        &mut app,
        &mut doors,
        21.0,
        [anchor].into(),
        &population,
        &[],
    );
    assert!(resident_opening_toggle(&mut app, anchor, 65, initial.facing ^ 4, &[]).unwrap());
    resident_door_tick(&mut app, &mut doors, 30.0, [].into(), &population, &[]);
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .object_state(anchor)
            .unwrap()
            .facing,
        initial.facing
    );
}

#[test]
fn resident_auto_close_waits_for_unloaded_halves_and_retries_atomically() {
    for unload_anchor in [false, true] {
        let mut app = app();
        let anchor = [4, 15, 8];
        opening_floor(&mut app, anchor);
        edit(&mut app, anchor, 0, 65, 0.0).unwrap();
        let initial = app
            .world()
            .resource::<EditJournal>()
            .object_state(anchor)
            .unwrap();
        let peer = initial.peer.unwrap();
        let mut doors = super::super::creatures::ResidentDoors::default();
        let population = terra_core::creatures::CreaturePopulation::default();
        resident_door_tick(&mut app, &mut doors, 0.0, [anchor].into(), &population, &[]);
        let peer_section = terra_voxel::global_to_section(peer).0;
        let unloaded = app
            .world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .remove(&peer_section)
            .unwrap();
        let anchor_section = terra_voxel::global_to_section(anchor).0;
        let unloaded_anchor = unload_anchor.then(|| {
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .remove(&anchor_section)
                .unwrap()
        });
        let states = [anchor, peer].map(|p| {
            app.world()
                .resource::<EditJournal>()
                .object_state(p)
                .unwrap()
        });
        let revisions = [anchor, peer].map(|p| {
            app.world()
                .resource::<EditJournal>()
                .revision(terra_voxel::global_to_section(p).0)
        });
        resident_door_tick(&mut app, &mut doors, 10.0, [].into(), &population, &[]);
        for (i, p) in [anchor, peer].into_iter().enumerate() {
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(journal.object_state(p), Some(states[i]));
            assert_eq!(
                journal.revision(terra_voxel::global_to_section(p).0),
                revisions[i]
            );
        }
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(peer_section, unloaded);
        if let Some(section) = unloaded_anchor {
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .insert(anchor_section, section);
        }
        resident_door_tick(&mut app, &mut doors, 11.0, [].into(), &population, &[]);
        for p in [anchor, peer] {
            assert_eq!(
                app.world()
                    .resource::<EditJournal>()
                    .object_state(p)
                    .unwrap()
                    .facing,
                initial.facing
            );
        }
    }
}

#[test]
fn resident_opening_keeps_both_halves_unchanged_when_a_player_blocks_the_hinged_leaf() {
    for facing in 0_u8..4 {
        let mut app = app();
        let anchor = [-16, 15, 8];
        opening_floor(&mut app, anchor);
        edit(
            &mut app,
            anchor,
            0,
            65,
            f32::from(facing) * std::f32::consts::FRAC_PI_2,
        )
        .unwrap();
        let journal = app.world().resource::<EditJournal>();
        let state = journal.object_state(anchor).unwrap();
        let peer = state.peer.unwrap();
        let paired = journal.object_state(peer).unwrap();
        let revisions =
            [anchor, peer].map(|p| journal.revision(terra_voxel::global_to_section(p).0));
        let destination = BlockObjectState {
            facing: state.facing ^ 4,
            ..state
        };
        let properties = app
            .world()
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>();
        let bounds = properties.shape(65).iter().find(|b| b.hinged).unwrap();
        let leaf = terra_world::objects::opening_box(*bounds, anchor, destination);
        let occupant = std::array::from_fn(|axis| (leaf.minimum[axis] + leaf.maximum[axis]) * 0.5);
        assert!(resident_opening_toggle(&mut app, anchor, 65, state.facing, &[occupant]).is_err());
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.object_state(anchor), Some(state));
        assert_eq!(journal.object_state(peer), Some(paired));
        assert_eq!(
            [anchor, peer].map(|p| journal.revision(terra_voxel::global_to_section(p).0)),
            revisions
        );
        assert!(resident_opening_toggle(&mut app, anchor, 65, state.facing, &[]).unwrap());
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(
            journal.object_state(anchor).unwrap().facing,
            state.facing ^ 4
        );
        assert_eq!(
            journal.object_state(peer).unwrap().facing,
            paired.facing ^ 4
        );
    }
}

#[test]
fn resident_opening_rejects_a_missing_partner_without_opening_the_loaded_half() {
    let mut app = app();
    let anchor = [4, 15, 8];
    opening_floor(&mut app, anchor);
    edit(&mut app, anchor, 0, 65, 0.0).unwrap();
    let journal = app.world().resource::<EditJournal>();
    let state = journal.object_state(anchor).unwrap();
    let peer = state.peer.unwrap();
    let paired = journal.object_state(peer).unwrap();
    let peer_section = terra_voxel::global_to_section(peer).0;
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .remove(&peer_section);
    let journal = app.world().resource::<EditJournal>();
    let revisions = [anchor, peer].map(|p| journal.revision(terra_voxel::global_to_section(p).0));
    assert!(resident_opening_toggle(&mut app, anchor, 65, state.facing, &[]).is_err());
    let journal = app.world().resource::<EditJournal>();
    assert_eq!(journal.object_state(anchor), Some(state));
    assert_eq!(journal.object_state(peer), Some(paired));
    assert_eq!(
        [anchor, peer].map(|p| journal.revision(terra_voxel::global_to_section(p).0)),
        revisions
    );
}

fn opening_floor(app: &mut App, position: [i64; 3]) {
    for cell in [
        [position[0], position[1] - 1, position[2]],
        position,
        [position[0], position[1] + 1, position[2]],
    ] {
        let section = terra_voxel::global_to_section(cell).0;
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .entry(section)
            .or_insert_with(|| terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(section).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            });
    }
    let (owner, local) =
        terra_voxel::global_to_section([position[0], position[1] - 1, position[2]]);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, 3)
        .unwrap();
}

fn opening_cell(app: &mut App, position: [i64; 3], material: u16) {
    let (owner, local) = terra_voxel::global_to_section(position);
    let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
    chunks
        .loaded
        .entry(owner)
        .or_insert_with(|| terra_world::streaming::LoadedSection {
            data: terra_voxel::BlockSection::empty(owner).unwrap(),
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

fn remote_extended_opening_fixture(app: &mut App, material: u16) {
    assert!(edit(app, [24, 4, 8], 0, material, 0.0).unwrap());
    assert!(
        app.world()
            .resource::<EditJournal>()
            .has_extended_kinetics()
    );
}

#[test]
fn remote_extended_parts_do_not_reject_doors_or_connected_windows_at_negative_borders() {
    for extended in [147, 149, 152] {
        for material in (65..=70).chain([139, 140, 247, 248]) {
            for facing in 0_u8..4 {
                let mut app = app();
                remote_extended_opening_fixture(&mut app, extended);
                let position = [-16, 15, -16];
                opening_floor(&mut app, position);
                let yaw = f32::from(facing) * std::f32::consts::FRAC_PI_2;
                assert!(edit(&mut app, position, 0, material, yaw).unwrap());
                let state = app
                    .world()
                    .resource::<EditJournal>()
                    .object_state(position)
                    .unwrap();
                assert_eq!(state.facing, facing);
                if terra_world::objects::is_door(material) {
                    let upper = [position[0], position[1] + 1, position[2]];
                    let paired = app
                        .world()
                        .resource::<EditJournal>()
                        .object_state(upper)
                        .unwrap();
                    assert!(paired.secondary);
                    assert_eq!(paired.facing, facing);
                    assert_eq!(paired.peer, Some(position));
                } else {
                    let group = terra_world::objects::windows::Group {
                        anchor: position,
                        material,
                        facing,
                        width: 2,
                        height: 2,
                    };
                    for cell in group.cells().filter(|p| *p != position) {
                        if cell[1] == position[1] {
                            opening_floor(&mut app, cell);
                        } else {
                            opening_cell(&mut app, cell, 0);
                        }
                        assert!(
                            edit(&mut app, cell, 0, material, yaw).unwrap(),
                            "material{material} facing{facing} extended{extended} at{cell:?}"
                        );
                    }
                    let chunks = app.world().resource::<ChunkStore>();
                    let journal = app.world().resource::<EditJournal>();
                    let connected =
                        terra_world::objects::windows::group(position, material, facing, |p| {
                            Some((chunks.block_at(p)?, journal.object_state(p)?.facing))
                        });
                    assert_eq!((connected.width, connected.height), (2, 2));
                }
                assert!(opening_toggle(&mut app, position, material, facing).unwrap());
                assert!(opening_toggle(&mut app, position, material, facing + 4).unwrap());
            }
        }
    }
}

#[test]
fn remote_extended_parts_preserve_wall_window_and_snow_floor_door_placement() {
    for material in (65..=70).chain([139, 140, 247, 248]) {
        for floor in [1, 2, 3, 5, 98] {
            let mut app = app();
            remote_extended_opening_fixture(&mut app, 147);
            let position = [3, 4, 8];
            opening_floor(&mut app, position);
            opening_cell(&mut app, [3, 3, 8], floor);
            opening_cell(&mut app, [3, 2, 8], 3);
            if terra_world::objects::is_window(material) {
                opening_cell(&mut app, [3, 3, 8], 0);
                opening_cell(&mut app, [2, 4, 8], floor);
            }
            assert!(edit(&mut app, position, 0, material, 0.0).unwrap());
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(position),
                Some(material)
            );
            assert!(
                app.world()
                    .resource::<EditJournal>()
                    .object_state(position)
                    .is_some()
            );
        }
    }
}

#[test]
fn connected_window_ownership_does_not_ignore_a_foreign_overhanging_gear() {
    let mut app = app();
    let first = [2, 1, 8];
    let second = [3, 1, 8];
    assert!(edit(&mut app, first, 0, 68, 0.0).unwrap());
    assert!(edit(&mut app, [4, 1, 8], 0, 149, std::f32::consts::FRAC_PI_2).unwrap());
    let owner = terra_voxel::global_to_section(first).0;
    let before = app.world().resource::<EditJournal>().section_delta(owner);
    assert!(edit(&mut app, second, 0, 68, 0.0).is_err());
    assert_eq!(
        app.world().resource::<EditJournal>().section_delta(owner),
        before
    );
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at(second),
        Some(0)
    );
}

#[test]
fn authoritative_creative_commands_place_doors_and_connected_windows_with_remote_machinery() {
    for material in (65..=70).chain([139, 140, 247, 248]) {
        let mut app = app();
        remote_extended_opening_fixture(&mut app, 147);
        let position = [2, 1, 8];
        assert_eq!(
            creative_placement(
                &mut app,
                position,
                material,
                terra_core::ItemKind::Block(material)
            ),
            EditOutcome::Applied,
            "first opening {material}"
        );
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(position),
            Some(material)
        );
        let state = app
            .world()
            .resource::<EditJournal>()
            .object_state(position)
            .expect("Authoritative placement must create renderable opening metadata");
        assert!(state.valid_for(position, material));
        assert!(!state.secondary);
        if terra_world::objects::is_door(material) {
            let upper = [position[0], position[1] + 1, position[2]];
            assert_eq!(state.peer, Some(upper));
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(upper),
                Some(material)
            );
            assert_eq!(
                app.world().resource::<EditJournal>().object_state(upper),
                Some(BlockObjectState {
                    peer: Some(position),
                    secondary: true,
                    ..state
                })
            );
        }
        if terra_world::objects::is_window(material) {
            let second = [3, 1, 8];
            assert_eq!(
                creative_placement(
                    &mut app,
                    second,
                    material,
                    terra_core::ItemKind::Block(material)
                ),
                EditOutcome::Applied,
                "connected window {material}"
            );
            let chunks = app.world().resource::<ChunkStore>();
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(journal.object_state(second), Some(state));
            let group =
                terra_world::objects::windows::group(position, material, state.facing, |p| {
                    Some((chunks.block_at(p)?, journal.object_state(p)?.facing))
                });
            assert_eq!((group.width, group.height), (2, 1));
        }
    }
}

#[test]
fn connected_window_toggle_is_atomic_across_sections_all_species_shapes_and_yaws() {
    for material in 68..=70 {
        for facing in 0..4 {
            for (width, height) in [(2, 1), (1, 2), (2, 2)] {
                let mut app = app();
                let group = terra_world::objects::windows::Group {
                    anchor: [-16, 15, -16],
                    material,
                    facing,
                    width,
                    height,
                };
                for p in group.cells() {
                    let owner = terra_voxel::global_to_section(p).0;
                    app.world_mut()
                        .resource_mut::<ChunkStore>()
                        .loaded
                        .entry(owner)
                        .or_insert_with(|| terra_world::streaming::LoadedSection {
                            data: terra_voxel::BlockSection::empty(owner).unwrap(),
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: 0,
                            water: None,
                            props: Vec::new(),
                        });
                }
                for p in group.cells().filter(|p| p[1] == group.anchor[1]) {
                    opening_floor(&mut app, p);
                }
                for p in group.cells() {
                    assert!(
                        edit(
                            &mut app,
                            p,
                            0,
                            material,
                            f32::from(facing) * std::f32::consts::FRAC_PI_2
                        )
                        .unwrap()
                    );
                }
                let target = group.cells().last().unwrap();
                assert!(opening_toggle(&mut app, target, material, facing).unwrap());
                for p in group.cells() {
                    assert_eq!(
                        app.world()
                            .resource::<EditJournal>()
                            .object_state(p)
                            .unwrap()
                            .facing,
                        facing + 4
                    );
                }
                assert!(
                    !opening_toggle(&mut app, target, material, facing).unwrap(),
                    "stale command cannot reverse whole group"
                );
                let removed_owner = terra_voxel::global_to_section(group.anchor).0;
                let removed = app
                    .world_mut()
                    .resource_mut::<ChunkStore>()
                    .loaded
                    .remove(&removed_owner)
                    .unwrap();
                assert!(opening_toggle(&mut app, target, material, facing + 4).is_err());
                for p in group.cells() {
                    assert_eq!(
                        app.world()
                            .resource::<EditJournal>()
                            .object_state(p)
                            .unwrap()
                            .facing,
                        facing + 4
                    );
                }
                app.world_mut()
                    .resource_mut::<ChunkStore>()
                    .loaded
                    .insert(removed_owner, removed);
                assert!(opening_toggle(&mut app, target, material, facing + 4).unwrap());
                for p in group.cells() {
                    assert_eq!(
                        app.world()
                            .resource::<EditJournal>()
                            .object_state(p)
                            .unwrap()
                            .facing,
                        facing
                    );
                }
                assert!(edit(&mut app, target, material, 0, 0.0).unwrap());
                for p in group.cells().filter(|p| *p != target) {
                    assert_eq!(
                        app.world().resource::<ChunkStore>().block_at(p),
                        Some(material)
                    );
                }
            }
        }
    }
}

#[test]
fn connected_window_never_rotates_either_leaf_into_a_player() {
    let mut app = app();
    for x in 2..=3 {
        assert!(edit(&mut app, [x, 1, 8], 0, 68, 0.0).unwrap());
    }
    let body = app
        .world_mut()
        .spawn((
            ServerCharacter {
                link: Entity::PLACEHOLDER,
            },
            Transform::from_xyz(3.85, 1.8, 9.1),
        ))
        .id();
    assert!(opening_toggle(&mut app, [2, 1, 8], 68, 0).is_err());
    for x in 2..=3 {
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .object_state([x, 1, 8])
                .unwrap()
                .facing,
            0
        );
    }
    app.world_mut().despawn(body);
    assert!(opening_toggle(&mut app, [3, 1, 8], 68, 0).unwrap());
    let body = app
        .world_mut()
        .spawn((
            ServerCharacter {
                link: Entity::PLACEHOLDER,
            },
            Transform::from_xyz(3.45, 1.8, 8.5),
        ))
        .id();
    assert!(opening_toggle(&mut app, [2, 1, 8], 68, 4).is_err());
    for x in 2..=3 {
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .object_state([x, 1, 8])
                .unwrap()
                .facing,
            4
        );
    }
    app.world_mut().despawn(body);
    assert!(opening_toggle(&mut app, [2, 1, 8], 68, 4).unwrap());
}

#[test]
fn opening_species_place_face_toggle_and_break_across_negative_section_boundaries() {
    for material in (65..=70).chain([247, 248]) {
        for facing in 0..4 {
            let mut app = app();
            let position = [-16, 15, -16];
            let upper = [-16, 16, -16];
            let door = terra_world::objects::is_door(material);
            opening_floor(&mut app, position);
            let yaw = f32::from(facing) * std::f32::consts::FRAC_PI_2;
            assert!(edit(&mut app, position, 0, material, yaw).unwrap());
            let journal = app.world().resource::<EditJournal>();
            let state = journal.object_state(position).unwrap();
            assert_eq!(state.facing, facing);
            assert_eq!(state.peer, door.then_some(upper));
            assert!(!state.secondary);
            if door {
                let state = journal.object_state(upper).unwrap();
                assert_eq!(state.peer, Some(position));
                assert!(state.secondary);
            }
            let target = if door { upper } else { position };
            assert!(opening_toggle(&mut app, target, material, facing).unwrap());
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(journal.object_state(position).unwrap().facing, facing + 4);
            if door {
                assert_eq!(journal.object_state(upper).unwrap().facing, facing + 4);
            }
            // A delayed duplicate client command must not close the door again.
            let revision = journal.revision(terra_voxel::global_to_section(position).0);
            assert!(!opening_toggle(&mut app, target, material, facing).unwrap());
            assert_eq!(
                app.world()
                    .resource::<EditJournal>()
                    .revision(terra_voxel::global_to_section(position).0),
                revision
            );
            assert!(opening_toggle(&mut app, position, material, facing + 4).unwrap());
            assert_eq!(
                app.world()
                    .resource::<EditJournal>()
                    .object_state(target)
                    .unwrap()
                    .facing,
                facing
            );
            assert!(edit(&mut app, target, material, 0, yaw).unwrap());
            for cell in [position, upper] {
                assert_eq!(app.world().resource::<ChunkStore>().block_at(cell), Some(0));
                assert!(
                    app.world()
                        .resource::<EditJournal>()
                        .object_state(cell)
                        .is_none()
                );
            }
        }
    }
}

#[test]
fn occupied_closing_sweep_and_blocked_upper_cell_are_rejected_atomically() {
    for material in 65..=70 {
        let mut app = app();
        let position = [2, 1, 8];
        assert!(edit(&mut app, position, 0, material, 0.0).unwrap());
        assert!(opening_toggle(&mut app, position, material, 0).unwrap());
        let section = terra_voxel::global_to_section(position).0;
        let before = app.world().resource::<EditJournal>().section_delta(section);
        let player = app
            .world_mut()
            .spawn((
                ServerCharacter {
                    link: Entity::PLACEHOLDER,
                },
                Transform::from_xyz(2.5, 1.6, 8.5),
            ))
            .id();
        assert!(opening_toggle(&mut app, position, material, 4).is_err());
        assert_eq!(
            app.world().resource::<EditJournal>().section_delta(section),
            before
        );
        app.world_mut().despawn(player);
        assert!(opening_toggle(&mut app, position, material, 4).unwrap());
        let before = app.world().resource::<EditJournal>().section_delta(section);
        let player = app
            .world_mut()
            .spawn((
                ServerCharacter {
                    link: Entity::PLACEHOLDER,
                },
                Transform::from_xyz(2.48, 1.9, 9.0),
            ))
            .id();
        assert!(
            opening_toggle(&mut app, position, material, 0).is_err(),
            "opening must not insert its leaf into a player"
        );
        assert_eq!(
            app.world().resource::<EditJournal>().section_delta(section),
            before
        );
        app.world_mut().despawn(player);
        assert!(opening_toggle(&mut app, position, material, 0).unwrap());
    }
    for material in 65..=67 {
        let mut app = app();
        let position = [2, 1, 8];
        let (section, local) = terra_voxel::global_to_section([2, 2, 8]);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 3)
            .unwrap();
        let before = app.world().resource::<EditJournal>().section_delta(section);
        assert!(edit(&mut app, position, 0, material, 0.0).is_err());
        assert_eq!(
            app.world().resource::<EditJournal>().section_delta(section),
            before
        );
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(position),
            Some(0)
        );
    }
}

#[test]
fn doors_respect_upper_world_limit_without_rejecting_single_cell_windows() {
    for material in 65..=70 {
        for y in [355, 356] {
            let mut app = app();
            app.world_mut()
                .resource_mut::<GameConfig>()
                .world
                .bounded_world = true;
            let position = [-1, y, 8];
            opening_floor(&mut app, position);
            let result = edit(&mut app, position, 0, material, 0.0);
            if material <= 67 && y == 356 {
                assert!(result.is_err());
                assert_eq!(
                    app.world().resource::<ChunkStore>().block_at(position),
                    Some(0)
                );
                assert!(
                    app.world()
                        .resource::<EditJournal>()
                        .object_state(position)
                        .is_none()
                );
            } else {
                assert!(result.unwrap());
            }
        }
    }
}

fn pipe_edit(app: &mut App, position: [i64; 3], look: [f32; 2]) -> Result<bool, String> {
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    let command = EditCommand {
        request: 0,
        revision: 0,
        look,
        source: None,
        action: EditAction::Block {
            position,
            expected: 0,
            material: 60,
        },
    };
    let plan = plan(command, &world)?.ok_or("Missing pipe plan")?;
    commit(plan, &mut world)
}

fn pipe_ports(app: &App, position: [i64; 3]) -> [[i64; 3]; 2] {
    let state = app
        .world()
        .resource::<EditJournal>()
        .object_state(position)
        .unwrap();
    assert!((8..23).contains(&state.facing));
    terra_world::objects::chimney_ports(60, state.facing).unwrap()
}

#[test]
fn one_chimney_item_orients_straights_and_adapts_connected_ends_into_elbows() {
    let mut app = app();
    assert!(edit(&mut app, [2, 1, 8], 0, 59, 0.0).unwrap());
    let first = [2, 3, 8];
    let second = [3, 3, 8];
    assert!(pipe_edit(&mut app, first, [0.0, std::f32::consts::FRAC_PI_2]).unwrap());
    assert_eq!(pipe_ports(&app, first), [[0, -1, 0], [0, 1, 0]]);
    assert!(pipe_edit(&mut app, second, [-std::f32::consts::FRAC_PI_2, 0.0]).unwrap());
    let first_ports = pipe_ports(&app, first);
    assert!(first_ports.contains(&[0, -1, 0]));
    assert!(first_ports.contains(&[1, 0, 0]));
    let second_ports = pipe_ports(&app, second);
    assert!(second_ports.contains(&[-1, 0, 0]));
    assert!(second_ports.contains(&[1, 0, 0]));
    // Extend from the new end upward: the upstream elbow remains unchanged.
    assert!(pipe_edit(&mut app, [3, 4, 8], [0.0, std::f32::consts::FRAC_PI_2]).unwrap());
    assert_eq!(pipe_ports(&app, first), first_ports);
    let second_ports = pipe_ports(&app, second);
    assert!(second_ports.contains(&[-1, 0, 0]));
    assert!(second_ports.contains(&[0, 1, 0]));
}

#[test]
fn inclined_look_places_sloped_chimney_with_matching_outlet() {
    let mut app = app();
    let furnace = [2, 1, 8];
    assert!(edit(&mut app, furnace, 0, 59, 0.0).unwrap());
    let pipe = [2, 3, 8];
    assert!(pipe_edit(&mut app, pipe, [-std::f32::consts::FRAC_PI_2, 0.45]).unwrap());
    let journal = app.world().resource::<EditJournal>();
    let facing = journal.object_state(pipe).unwrap().facing;
    assert!((40..55).contains(&facing));
    let ports = terra_world::objects::chimney_ports(60, facing).unwrap();
    assert!(ports.contains(&[0, -1, 0]));
    assert!(ports.contains(&[1, 0, 0]));
    assert_eq!(
        terra_world::furnaces::chimney_outlet(
            furnace,
            app.world().resource::<ChunkStore>(),
            journal
        ),
        Some(([3, 3, 8], [1, 0, 0]))
    );
}

#[test]
fn a_third_chimney_connection_is_rejected_without_changing_existing_ports() {
    let mut app = app();
    let neighbors = [[8, 3, 8], [7, 4, 8], [8, 4, 7]];
    for (position, look) in neighbors.into_iter().zip([
        [0.0, std::f32::consts::FRAC_PI_2],
        [-std::f32::consts::FRAC_PI_2, 0.0],
        [std::f32::consts::PI, 0.0],
    ]) {
        assert!(pipe_edit(&mut app, position, look).unwrap());
    }
    let before = neighbors.map(|p| pipe_ports(&app, p));
    assert!(pipe_edit(&mut app, [8, 4, 8], [0.0; 2]).is_err());
    assert_eq!(neighbors.map(|p| pipe_ports(&app, p)), before);
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([8, 4, 8]),
        Some(0)
    );
    assert!(
        app.world()
            .resource::<EditJournal>()
            .object_state([8, 4, 8])
            .is_none()
    );
}

fn double_door_fixture(
    app: &mut App,
    material: u16,
    facing: u8,
) -> terra_world::objects::doors::Group {
    let layout = terra_world::objects::windows::Group {
        anchor: [-1, 15, 8],
        material,
        facing,
        width: 2,
        height: 2,
    };
    for cell in layout.cells().take(2) {
        opening_floor(app, cell);
        assert!(
            edit(
                app,
                cell,
                0,
                material,
                f32::from(facing) * std::f32::consts::FRAC_PI_2
            )
            .unwrap()
        );
    }
    let journal = app.world().resource::<EditJournal>();
    let chunks = app.world().resource::<ChunkStore>();
    let group = terra_world::objects::doors::group(
        layout.anchor,
        material,
        journal.object_state(layout.anchor).unwrap(),
        |p| Some((chunks.block_at(p)?, journal.object_state(p)?)),
    )
    .unwrap();
    assert_eq!(group.width, 2);
    group
}

#[test]
fn double_doors_toggle_from_any_of_four_cells_and_restore_existing_replica_states() {
    for material in [65, 66, 67, 139] {
        for facing in 0..4 {
            let mut app = app();
            let group = double_door_fixture(&mut app, material, facing);
            for clicked in group.cells() {
                assert!(opening_toggle(&mut app, clicked, material, facing).unwrap());
                let journal = app.world().resource::<EditJournal>();
                assert!(
                    group
                        .cells()
                        .all(|p| journal.object_state(p).unwrap().facing == facing + 4)
                );
                let mut replica = EditJournal::default();
                let mut replica_chunks = ChunkStore::default();
                let sections: std::collections::BTreeSet<_> = group
                    .cells()
                    .map(|p| terra_voxel::global_to_section(p).0)
                    .collect();
                for section in sections {
                    replica_chunks
                        .apply_confirmed_delta(&mut replica, journal.section_delta(section))
                        .unwrap();
                }
                for p in group.cells() {
                    let actual = terra_world::objects::doors::group(
                        p,
                        material,
                        replica.object_state(p).unwrap(),
                        |q| Some((replica.block(q)?, replica.object_state(q)?)),
                    )
                    .unwrap();
                    assert_eq!(actual, group.with_open(true));
                }
                assert!(opening_toggle(&mut app, clicked, material, facing + 4).unwrap());
            }
        }
    }
}

#[test]
fn double_door_second_leaf_occupant_missing_half_or_overflow_leaves_all_four_unchanged() {
    for fault in ["player", "unloaded", "overflow"] {
        let mut app = app();
        let group = double_door_fixture(&mut app, 65, 0);
        let cells = group.cells().collect::<Vec<_>>();
        if fault == "unloaded" {
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .remove(&terra_voxel::global_to_section(cells[3]).0);
        } else if fault == "overflow" {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            let mut delta = journal.section_delta(terra_voxel::global_to_section(cells[3]).0);
            delta.revision = u64::MAX;
            journal.import_delta(delta).unwrap();
        }
        let before = cells
            .iter()
            .map(|p| app.world().resource::<EditJournal>().object_state(*p))
            .collect::<Vec<_>>();
        let revisions = cells
            .iter()
            .map(|p| {
                app.world()
                    .resource::<EditJournal>()
                    .revision(terra_voxel::global_to_section(*p).0)
            })
            .collect::<Vec<_>>();
        let occupant = group.with_open(true).world_box(
            terra_world::objects::doors::pieces()
                .into_iter()
                .find(|p| p.solid && p.leaf == Some(1))
                .unwrap(),
        );
        let players = if fault == "player" {
            vec![std::array::from_fn(|a| {
                (occupant.minimum[a] + occupant.maximum[a]) * 0.5
            })]
        } else {
            Vec::new()
        };
        assert!(
            resident_opening_toggle(&mut app, group.anchor, 65, 0, &players).is_err(),
            "fault {fault}"
        );
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(
            cells
                .iter()
                .map(|p| journal.object_state(*p))
                .collect::<Vec<_>>(),
            before
        );
        assert_eq!(
            cells
                .iter()
                .map(|p| journal.revision(terra_voxel::global_to_section(*p).0))
                .collect::<Vec<_>>(),
            revisions
        );
    }
}

#[test]
fn mining_one_double_door_leaf_removes_only_its_vertical_pair_and_preflights_staged_revision() {
    let mut app = app();
    let group = double_door_fixture(&mut app, 65, 0);
    let cells = group.cells().collect::<Vec<_>>();
    assert!(edit(&mut app, cells[3], 65, 0, 0.0).unwrap());
    for p in [cells[1], cells[3]] {
        assert_eq!(app.world().resource::<ChunkStore>().block_at(p), Some(0));
    }
    for p in [cells[0], cells[2]] {
        assert_eq!(app.world().resource::<ChunkStore>().block_at(p), Some(65));
    }
    let staged = {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let world = system.get_mut(app.world_mut()).unwrap();
        plan(
            EditCommand {
                request: 0,
                revision: 0,
                look: [0.0; 2],
                source: None,
                action: EditAction::Block {
                    position: cells[0],
                    expected: 65,
                    material: 0,
                },
            },
            &world,
        )
        .unwrap()
        .unwrap()
    };
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let mut delta = journal.section_delta(terra_voxel::global_to_section(cells[2]).0);
        delta.revision = u64::MAX;
        journal.import_delta(delta).unwrap();
    }
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    assert!(commit(staged, &mut world).is_err());
    for p in [cells[0], cells[2]] {
        assert_eq!(world.chunks.block_at(p), Some(65));
    }
}

#[test]
fn residents_manage_both_double_door_leaves_and_preserve_any_manual_override() {
    let mut app = app();
    let group = double_door_fixture(&mut app, 65, 0);
    let cells = group.cells().collect::<Vec<_>>();
    let mut doors = super::super::creatures::ResidentDoors::default();
    let population = terra_core::creatures::CreaturePopulation::default();
    resident_door_tick(
        &mut app,
        &mut doors,
        0.0,
        [cells[3]].into(),
        &population,
        &[],
    );
    assert!(cells.iter().all(|p| {
        app.world()
            .resource::<EditJournal>()
            .object_state(*p)
            .unwrap()
            .facing
            == 4
    }));
    let second_center = [
        cells[1][0] as f64 + 2.0,
        cells[1][1] as f64 + 1.0,
        cells[1][2] as f64 + 0.5,
    ];
    resident_door_tick(
        &mut app,
        &mut doors,
        4.0,
        Default::default(),
        &population,
        &[second_center],
    );
    assert!(cells.iter().all(|p| {
        app.world()
            .resource::<EditJournal>()
            .object_state(*p)
            .unwrap()
            .facing
            == 4
    }));
    resident_door_tick(
        &mut app,
        &mut doors,
        8.0,
        Default::default(),
        &population,
        &[],
    );
    assert!(cells.iter().all(|p| {
        app.world()
            .resource::<EditJournal>()
            .object_state(*p)
            .unwrap()
            .facing
            == 0
    }));
    resident_door_tick(
        &mut app,
        &mut doors,
        9.0,
        [cells[1]].into(),
        &population,
        &[],
    );
    for p in [cells[1], cells[3]] {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let state = journal.object_state(p).unwrap();
        journal
            .set_object_state(p, Some(BlockObjectState { facing: 0, ..state }))
            .unwrap();
    }
    resident_door_tick(
        &mut app,
        &mut doors,
        13.0,
        Default::default(),
        &population,
        &[],
    );
    for p in [cells[0], cells[2]] {
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .object_state(p)
                .unwrap()
                .facing,
            4
        );
    }
    for p in [cells[1], cells[3]] {
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .object_state(p)
                .unwrap()
                .facing,
            0
        );
    }
}

#[test]
fn double_door_toggle_requires_inventory_permission_on_the_other_leaf_chunk() {
    use terra_core::kingdom::KingdomRole;
    let mut app = app();
    let group = double_door_fixture(&mut app, 65, 0);
    let actor = terra_core::PlayerUuid::from_u128(991);
    let owner = terra_core::PlayerUuid::from_u128(992);
    let realm = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let registry = &mut journal.gameplay_mut().kingdoms;
        let id = registry.found(owner, [16, 15, 8], "Other leaf").unwrap();
        registry
            .claim(
                id,
                [0, 0],
                terra_core::kingdom::ClaimKind::ProtectedBuilding,
                terra_core::kingdom::ClaimActor::Player(owner),
            )
            .unwrap();
        id
    };
    let now = Instant::now();
    let peer = Peer {
        farming: default(),
        crank: default(),
        chest_cursor: None,
        creature_action_at: now,
        container: None,
        held_slot: None,
        mode: Some(terra_core::GameMode::Creative),
        generation: 0,
        chat: default(),
        uuid: actor,
        gender: default(),
        mining: None,
        consumption: None,
        gameplay_sent: None,
        permissions: terra_core::PlayerPermissions {
            build: true,
            game_mode: false,
            flight: false,
            environment: false,
        },
        id: PlayerId(1),
        transport_id: 1,
        body: Entity::PLACEHOLDER,
        accepted: true,
        synced: true,
        view: 1,
        known: default(),
        inputs: default(),
        received: 0,
        ack: 0,
        look: [0.0; 2],
        last_input: now,
        joined: now,
        last_request: 0,
        edits_left: 20.0,
        rate_time: now,
        spawned: true,
        restored_spawn: false,
        restore_location: None,
        last_movement: default(),
        vitals: default(),
        equipment: default(),
        survival: default(),
        stats_sent: None,
    };
    let action = EditAction::ToggleObject {
        position: group.anchor,
        expected: 65,
        facing: 0,
        closing: false,
    };
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let world = system.get_mut(app.world_mut()).unwrap();
        assert!(!super::super::kingdoms::edit_allowed(action, &peer, &world));
        assert!(
            group
                .cells()
                .all(|p| world.journal.object_state(p).unwrap().facing == 0)
        );
    }
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let registry = &mut journal.gameplay_mut().kingdoms;
        registry.join(actor, realm).unwrap();
        registry
            .set_role(owner, realm, actor, KingdomRole::Worker)
            .unwrap();
    }
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    assert!(super::super::kingdoms::edit_allowed(action, &peer, &world));
    assert!(!super::super::kingdoms::permission_allowed(
        actor,
        terra_core::kingdom::KingdomPermission::Build,
        [0, 15, 8],
        &world
    ));
    assert!(toggle(group.anchor, 65, 0, &mut world).unwrap());
    assert!(
        group
            .cells()
            .all(|p| world.journal.object_state(p).unwrap().facing == 4)
    );
}
