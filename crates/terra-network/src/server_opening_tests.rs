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
    for material in 65..=70 {
        for facing in 0..4 {
            let mut app = app();
            let position = [-16, 15, -16];
            let upper = [-16, 16, -16];
            let door = material <= 67;
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
