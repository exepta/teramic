use super::*;
use std::collections::BTreeMap;
use terra_core::{
    WoodSpecies,
    kingdom::KingdomPermission,
    rails::{CurveSpec, Endpoint, FixedPoint, Port},
};
use terra_world::{
    placement::Layout,
    props::{PropId, WorldProp},
};

struct ConnectionFixture {
    app: App,
    peer: Peer,
    old: Piece,
    curve: Piece,
    command: EditCommand,
    item: ItemKind,
}
fn connection_fixture(wood: WoodSpecies) -> ConnectionFixture {
    connection_fixture_at(wood, 0)
}
fn connection_fixture_at(wood: WoodSpecies, z_offset: i64) -> ConnectionFixture {
    let (mut app, peer, old, place) = rail_fixture(rails::STRAIGHT, [8, 4, 10 + z_offset]);
    assert_eq!(run(&mut app, &peer, place, false), EditOutcome::Applied);
    let start = Endpoint {
        anchor: old.anchor,
        port: 1,
    };
    let end = Port {
        index: 0,
        position: FixedPoint([4 * 16 + 8, 4 * 16 + 2, (-6 + z_offset) * 16]),
        tangent: [0, 0, 1],
        connectable: true,
    };
    let anchor = [8, 4, 6 + z_offset];
    let curve = Piece::new_curve(
        anchor,
        CurveSpec::between(old.ports()[1], end, anchor).unwrap(),
    )
    .unwrap();
    for cell in terra_world::rails::cells(curve) {
        set_block(&mut app, cell, 0);
    }
    for support in curve.supports().iter() {
        set_block(
            &mut app,
            std::array::from_fn(|axis| anchor[axis] + i64::from(support.support[axis])),
            3,
        );
    }
    let clicked = [4, 3, -6 + z_offset];
    set_block(&mut app, clicked, 3);
    let eye = Vec3::new(4.5, 6.0, -2.5 + z_offset as f32);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    let look = aim(&app, &peer, [4.5, 4.0, -5.5 + z_offset as f64]);
    let item = ItemKind::for_wood(rails::STRAIGHT, wood);
    let source = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let bag = journal
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap();
        assert!(bag.creative_set(0, ItemStack { item, count: 8 }));
        InventorySource {
            slot: 0,
            revision: bag.revision,
        }
    };
    let command = EditCommand {
        request: 2,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(global_to_section(clicked).0),
        look,
        source: Some(source),
        action: EditAction::ConnectRail {
            start,
            position: clicked,
            expected: 3,
        },
    };
    let fixture = ConnectionFixture {
        app,
        peer,
        old,
        curve,
        command,
        item,
    };
    let plan = connection_plan(&fixture);
    assert_eq!(
        terra_world::rails::cost(&plan),
        Some(terra_world::rails::piece_cost(curve))
    );
    assert!(terra_world::rails::piece_cost(curve) > 1);
    fixture
}
fn connection_plan(fixture: &ConnectionFixture) -> Layout {
    let EditAction::ConnectRail {
        start, position, ..
    } = fixture.command.action
    else {
        unreachable!()
    };
    let eye = fixture
        .app
        .world()
        .get::<Transform>(fixture.peer.body)
        .unwrap()
        .translation
        + Vec3::Y * PLAYER_EYE_OFFSET;
    let direction = Quat::from_euler(
        EulerRot::YXZ,
        fixture.command.look[0],
        fixture.command.look[1],
        0.0,
    ) * Vec3::NEG_Z;
    let (block, _) = terra_world::targeting::pick_world_for_placement(
        fixture.app.world().resource::<ChunkStore>(),
        fixture.app.world().resource::<EditJournal>(),
        fixture
            .app
            .world()
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
        PropPose::Rest,
    );
    let hit = block.expect("actual destination floor ray");
    assert_eq!(hit.position, position);
    assert!(hit.distance <= 4.5);
    terra_world::rails::connection(
        fixture.app.world().resource::<ChunkStore>(),
        fixture.app.world().resource::<EditJournal>(),
        start,
        hit,
        fixture.command.look,
        fixture.item.wood_species().unwrap_or_default(),
    )
    .unwrap()
}
#[derive(Debug, PartialEq)]
struct RpcSnapshot {
    state: Snapshot,
    props: BTreeMap<SectionPos, Vec<WorldProp>>,
}
fn rpc_snapshot(fixture: &ConnectionFixture) -> RpcSnapshot {
    RpcSnapshot {
        state: snapshot(&fixture.app, fixture.curve),
        props: fixture
            .app
            .world()
            .resource::<ChunkStore>()
            .loaded
            .iter()
            .map(|(section, loaded)| (*section, loaded.props.clone()))
            .collect(),
    }
}
fn visible_secondary_command(fixture: &ConnectionFixture) -> EditCommand {
    let app = &fixture.app;
    let peer = &fixture.peer;
    let eye =
        app.world().get::<Transform>(peer.body).unwrap().translation + Vec3::Y * PLAYER_EYE_OFFSET;
    let journal = app.world().resource::<EditJournal>();
    for cell in terra_world::rails::cells(fixture.curve)
        .into_iter()
        .filter(|cell| *cell != fixture.curve.anchor)
    {
        for bounds in terra_world::rails::cell_boxes(
            cell,
            fixture.curve.material,
            journal.object_state(cell).unwrap(),
        )
        .iter()
        {
            let target = std::array::from_fn(|axis| {
                cell[axis] as f64 + (bounds.minimum[axis] + bounds.maximum[axis]) * 0.5
            });
            let look = aim(app, peer, target);
            let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
            let (_, selected) = terra_world::targeting::pick_world_for_mining(
                app.world().resource::<ChunkStore>(),
                journal,
                app.world()
                    .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
                eye.to_array().map(f64::from),
                direction.to_array().map(f64::from),
                PropPose::Rest,
            );
            if matches!(selected,Some(WorldTarget::Block(hit)) if hit.position==cell && hit.distance<=4.5)
            {
                return EditCommand {
                    request: 3,
                    revision: journal.revision(global_to_section(cell).0),
                    look,
                    source: None,
                    action: EditAction::Block {
                        position: cell,
                        expected: fixture.curve.material,
                        material: 0,
                    },
                };
            }
        }
    }
    panic!("a real secondary curve slice must be visible from the end click")
}
fn add_prop(
    fixture: &mut ConnectionFixture,
    id: u64,
    kind: PropKind,
    saved: bool,
    distance: f64,
) -> WorldProp {
    let sample = fixture.curve.paths()[0].sample_distance(distance).position;
    let prop = WorldProp {
        id: PropId(id),
        kind,
        position: [sample[0], 4.0, sample[2]],
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section: global_to_section([sample[0].floor() as i64, 3, sample[2].floor() as i64]).0,
        removed: false,
    };
    assert!(prop.valid());
    fixture
        .app
        .world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&prop.section)
        .unwrap()
        .props
        .push(prop);
    if saved {
        assert!(
            fixture
                .app
                .world_mut()
                .resource_mut::<EditJournal>()
                .place_prop(prop)
                .unwrap()
        );
    }
    prop
}

fn commit_existing_module(fixture: &mut ConnectionFixture, piece: Piece) {
    for cell in terra_world::rails::cells(piece) {
        set_block(&mut fixture.app, cell, 0);
    }
    for support in piece.supports().iter() {
        set_block(
            &mut fixture.app,
            std::array::from_fn(|axis| piece.anchor[axis] + i64::from(support.support[axis])),
            3,
        );
    }
    let plan = if let Some(curve) = piece.curve {
        let mut layout = Layout {
            edits: Vec::new(),
            states: Vec::new(),
            belts: Vec::new(),
        };
        for cell in terra_world::rails::cells(piece) {
            layout.edits.push(BlockEditRequest {
                position: cell,
                expected_material: 0,
                material: piece.material,
            });
            let state = terra_world::rails::object_state_with_wood(piece, cell, WoodSpecies::Oak);
            assert_eq!(state.rail_curve, Some(curve));
            layout.states.push((cell, Some(state)));
        }
        layout
    } else {
        terra_world::rails::plan(
            fixture.app.world().resource::<ChunkStore>(),
            fixture.app.world().resource::<EditJournal>(),
            piece.anchor,
            piece.material,
            piece.encoded_facing(),
        )
        .unwrap()
    };
    let mut system =
        SystemState::<(ResMut<ChunkStore>, ResMut<EditJournal>)>::new(fixture.app.world_mut());
    let (mut chunks, mut journal) = system.get_mut(fixture.app.world_mut()).unwrap();
    assert!(terra_world::rails::commit(&mut chunks, &mut journal, &plan).unwrap());
    assert!(terra_world::rails::complete(&journal, piece));
}

fn aim_existing_destination(
    fixture: &mut ConnectionFixture,
    target: Piece,
    eye: [f32; 3],
    preferred_cell: [i64; 3],
) {
    fixture
        .app
        .world_mut()
        .get_mut::<Transform>(fixture.peer.body)
        .unwrap()
        .translation = Vec3::from_array(eye) - Vec3::Y * PLAYER_EYE_OFFSET;
    let mut cells = terra_world::rails::cells(target);
    cells.sort_by_key(|cell| {
        (0..3)
            .map(|axis| cell[axis].abs_diff(preferred_cell[axis]))
            .sum::<u64>()
    });
    for cell in cells {
        let journal = fixture.app.world().resource::<EditJournal>();
        for shape in terra_world::rails::cell_boxes(
            cell,
            target.material,
            journal.object_state(cell).unwrap(),
        )
        .iter()
        {
            let point = std::array::from_fn(|axis| {
                cell[axis] as f64 + (shape.minimum[axis] + shape.maximum[axis]) * 0.5
            });
            let look = aim(&fixture.app, &fixture.peer, point);
            if look[1].abs() > 1.501 {
                continue;
            }
            let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
            let (_, selected) = terra_world::targeting::pick_world_for_placement(
                fixture.app.world().resource::<ChunkStore>(),
                journal,
                fixture
                    .app
                    .world()
                    .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
                eye.map(f64::from),
                direction.to_array().map(f64::from),
                PropPose::Rest,
            );
            let Some(WorldTarget::Block(hit)) = selected else {
                continue;
            };
            if hit.distance > 4.5
                || terra_world::rails::owner_at(journal, hit.position) != Some(target)
            {
                continue;
            }
            let EditAction::ConnectRail { start, .. } = fixture.command.action else {
                unreachable!()
            };
            let Ok(plan) = terra_world::rails::connection(
                fixture.app.world().resource::<ChunkStore>(),
                journal,
                start,
                hit,
                look,
                fixture.item.wood_species().unwrap_or_default(),
            ) else {
                continue;
            };
            if plan.states.iter().all(|(cell, state)| {
                *state
                    == Some(terra_world::rails::object_state_with_wood(
                        fixture.curve,
                        *cell,
                        fixture.item.wood_species().unwrap_or_default(),
                    ))
            }) {
                fixture.command.look = look;
                fixture.command.revision = journal.revision(global_to_section(hit.position).0);
                fixture.command.action = EditAction::ConnectRail {
                    start,
                    position: hit.position,
                    expected: target.material,
                };
                return;
            }
        }
    }
    panic!("the existing destination must have a real reachable hit and valid connection")
}

#[test]
fn common_wood_track_consumes_curve_length_cost_and_secondary_break_returns_the_same_species_and_quantity()
 {
    for wood in [
        WoodSpecies::Oak,
        WoodSpecies::Birch,
        WoodSpecies::Spruce,
        WoodSpecies::Cherry,
    ] {
        let mut fixture = connection_fixture(wood);
        let cost = terra_world::rails::piece_cost(fixture.curve);
        assert_eq!(
            run(&mut fixture.app, &fixture.peer, fixture.command, false),
            EditOutcome::Applied
        );
        let journal = fixture.app.world().resource::<EditJournal>();
        assert!(terra_world::rails::complete(journal, fixture.curve));
        assert!(terra_world::rails::cells(fixture.curve).iter().all(|cell| {
            journal
                .object_state(*cell)
                .is_some_and(|state| state.wood == wood && state.rail_curve == fixture.curve.curve)
        }));
        assert_eq!(
            journal.gameplay().inventories[&fixture.peer.uuid].count(fixture.item),
            8 - cost
        );
        let remove = visible_secondary_command(&fixture);
        assert_eq!(
            run(&mut fixture.app, &fixture.peer, remove, true),
            EditOutcome::Applied
        );
        let journal = fixture.app.world().resource::<EditJournal>();
        assert!(terra_world::rails::complete(journal, fixture.old));
        assert!(
            terra_world::rails::cells(fixture.curve)
                .iter()
                .all(
                    |cell| journal.block(*cell) == Some(0) && journal.object_state(*cell).is_none()
                )
        );
        assert_eq!(journal.gameplay().drops.len(), 1);
        assert_eq!(
            journal.gameplay().drops[0].stack,
            ItemStack {
                item: fixture.item,
                count: cost
            }
        );
    }
}

#[test]
fn stale_or_underfunded_sources_and_foreign_curve_cells_cannot_publish_partial_connections() {
    for failure in ["stale_source", "missing_material", "foreign_claim"] {
        let mut fixture = connection_fixture(WoodSpecies::Spruce);
        match failure {
            "stale_source" => fixture.command.source.as_mut().unwrap().revision += 1,
            "missing_material" => {
                let cost = terra_world::rails::piece_cost(fixture.curve);
                let mut journal = fixture.app.world_mut().resource_mut::<EditJournal>();
                let bag = journal
                    .gameplay_mut()
                    .inventories
                    .get_mut(&fixture.peer.uuid)
                    .unwrap();
                assert!(bag.creative_set(
                    0,
                    ItemStack {
                        item: fixture.item,
                        count: cost - 1
                    }
                ));
                fixture.command.source.as_mut().unwrap().revision = bag.revision;
            }
            "foreign_claim" => {
                fixture
                    .app
                    .world_mut()
                    .resource_mut::<EditJournal>()
                    .gameplay_mut()
                    .kingdoms
                    .found(
                        PlayerUuid::from_u128(205_620),
                        [8, 10, 4],
                        "Foreign curve land",
                    )
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let before = rpc_snapshot(&fixture);
        assert_ne!(
            run(&mut fixture.app, &fixture.peer, fixture.command, false),
            EditOutcome::Applied,
            "{failure}"
        );
        assert_eq!(rpc_snapshot(&fixture), before, "{failure}");
        assert!(terra_world::rails::complete(
            fixture.app.world().resource::<EditJournal>(),
            fixture.old
        ));
    }
}

#[test]
fn a_foreign_source_at_a_claim_boundary_cannot_be_connected_from_unclaimed_land() {
    let mut fixture = connection_fixture_at(WoodSpecies::Spruce, -7);
    fixture
        .app
        .world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .kingdoms
        .found(
            PlayerUuid::from_u128(205_625),
            [8, 10, 2],
            "Protected source track",
        )
        .unwrap();
    let plan = connection_plan(&fixture);
    let registry = &fixture
        .app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .kingdoms;
    assert!(plan.edits.iter().all(|edit| registry.can(
        fixture.peer.uuid,
        KingdomPermission::Build,
        edit.position
    )));
    assert!(
        terra_world::rails::cells(fixture.old)
            .iter()
            .all(|cell| { !registry.can(fixture.peer.uuid, KingdomPermission::Build, *cell) })
    );
    let before = rpc_snapshot(&fixture);
    assert_eq!(
        run(&mut fixture.app, &fixture.peer, fixture.command, false),
        EditOutcome::Invalid
    );
    assert_eq!(rpc_snapshot(&fixture), before);
}

#[test]
fn snapping_each_common_special_mode_cannot_occupy_a_foreign_modules_free_port() {
    for material in [
        rails::SWITCH_LEFT,
        rails::SWITCH_RIGHT,
        rails::CROSSING,
        rails::BUFFER,
    ] {
        let (mut app, peer, old, initial) = rail_fixture(rails::STRAIGHT, [8, 4, 3]);
        assert_eq!(run(&mut app, &peer, initial, false), EditOutcome::Applied);
        let special = Piece::new([8, 4, -1], material, 2).unwrap();
        for cell in terra_world::rails::cells(special) {
            set_block(&mut app, cell, 0);
        }
        for point in special.supports().iter() {
            set_block(
                &mut app,
                std::array::from_fn(|axis| special.anchor[axis] + i64::from(point.support[axis])),
                3,
            );
        }
        let eye = Vec3::new(8.5, 6.8, -2.5);
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
        let mut selected = None;
        for cell in terra_world::rails::cells(old) {
            let journal = app.world().resource::<EditJournal>();
            for bounds in terra_world::rails::cell_boxes(
                cell,
                old.material,
                journal.object_state(cell).unwrap(),
            )
            .iter()
            {
                let point = std::array::from_fn(|axis| {
                    cell[axis] as f64 + (bounds.minimum[axis] + bounds.maximum[axis]) * 0.5
                });
                let look = aim(&app, &peer, point);
                let direction =
                    Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
                let (_, target) = terra_world::targeting::pick_world_for_placement(
                    app.world().resource::<ChunkStore>(),
                    journal,
                    app.world()
                        .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
                    eye.to_array().map(f64::from),
                    direction.to_array().map(f64::from),
                    PropPose::Rest,
                );
                let Some(WorldTarget::Block(hit)) = target else {
                    continue;
                };
                if hit.distance > 4.5
                    || terra_world::rails::owner_at(journal, hit.position) != Some(old)
                {
                    continue;
                }
                if let Ok(plan) = terra_world::rails::placement(
                    app.world().resource::<ChunkStore>(),
                    journal,
                    hit,
                    material,
                    (
                        eye.to_array().map(f64::from),
                        direction.to_array().map(f64::from),
                    ),
                    look,
                ) && let Some(proposed) = plan.states.iter().find_map(|(cell, state)| {
                    let state = (*state)?;
                    if state.secondary {
                        return None;
                    }
                    terra_world::rails::piece_from_state(*cell, material, state)
                }) {
                    selected = Some((look, proposed));
                    break;
                }
            }
            if selected.is_some() {
                break;
            }
        }
        let (look, special) =
            selected.expect("a real rail hit must snap the special mode to its free port");
        let command = {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            let bag = journal
                .gameplay_mut()
                .inventories
                .get_mut(&peer.uuid)
                .unwrap();
            assert!(bag.creative_set(
                0,
                ItemStack {
                    item: ItemKind::Block(rails::STRAIGHT),
                    count: terra_world::rails::piece_cost(special) + 1,
                }
            ));
            let source = InventorySource {
                slot: 0,
                revision: bag.revision,
            };
            EditCommand {
                request: 2,
                revision: journal.revision(global_to_section(special.anchor).0),
                look,
                source: Some(source),
                action: EditAction::Block {
                    position: special.anchor,
                    expected: 0,
                    material,
                },
            }
        };
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        {
            let world = system.get_mut(app.world_mut()).unwrap();
            assert_eq!(
                check(command, &peer, &world),
                EditOutcome::Applied,
                "mode {material}"
            );
        }
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .kingdoms
            .found(
                PlayerUuid::from_u128(205_627),
                [8, 10, 2],
                "Protected snapped module",
            )
            .unwrap();
        let registry = &app.world().resource::<EditJournal>().gameplay().kingdoms;
        assert!(
            terra_world::rails::cells(special)
                .iter()
                .all(|cell| { registry.can(peer.uuid, KingdomPermission::Build, *cell) })
        );
        assert!(
            terra_world::rails::cells(old)
                .iter()
                .all(|cell| { !registry.can(peer.uuid, KingdomPermission::Build, *cell) })
        );
        let before = snapshot(&app, special);
        assert_eq!(
            run(&mut app, &peer, command, false),
            EditOutcome::Invalid,
            "mode {material}"
        );
        assert_eq!(snapshot(&app, special), before);
        assert!(terra_world::rails::complete(
            app.world().resource::<EditJournal>(),
            old
        ));
    }
}

#[test]
fn a_foreign_destination_cannot_be_connected_when_every_new_curve_cell_is_unclaimed() {
    let mut fixture = connection_fixture_at(WoodSpecies::Cherry, 22);
    let target = Piece::new([4, 4, 15], rails::STRAIGHT, 2).unwrap();
    commit_existing_module(&mut fixture, target);
    aim_existing_destination(&mut fixture, target, [4.5, 6.0, 19.5], [4, 4, 15]);
    fixture
        .app
        .world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .kingdoms
        .found(
            PlayerUuid::from_u128(205_626),
            [4, 10, 10],
            "Protected destination track",
        )
        .unwrap();
    let plan = connection_plan(&fixture);
    let registry = &fixture
        .app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .kingdoms;
    assert!(plan.edits.iter().all(|edit| registry.can(
        fixture.peer.uuid,
        KingdomPermission::Build,
        edit.position
    )));
    assert!(
        terra_world::rails::cells(fixture.old)
            .iter()
            .all(|cell| registry.can(fixture.peer.uuid, KingdomPermission::Build, *cell))
    );
    assert!(
        terra_world::rails::cells(target)
            .iter()
            .all(|cell| !registry.can(fixture.peer.uuid, KingdomPermission::Build, *cell))
    );
    let before = rpc_snapshot(&fixture);
    assert_eq!(
        run(&mut fixture.app, &fixture.peer, fixture.command, false),
        EditOutcome::Invalid
    );
    assert_eq!(rpc_snapshot(&fixture), before);
    assert!(terra_world::rails::complete(
        fixture.app.world().resource::<EditJournal>(),
        target
    ));
}

#[test]
fn clicking_a_long_destination_middle_does_not_reach_its_distant_free_port() {
    let mut fixture = connection_fixture_at(WoodSpecies::Oak, 22);
    let target = Piece::new_curve(
        [4, 4, 15],
        CurveSpec::new([[8, 2, 16], [8, 2, -160], [8, 2, -320], [8, 2, -496]]).unwrap(),
    )
    .unwrap();
    commit_existing_module(&mut fixture, target);
    let eye = [6.5, 6.0, 0.5];
    aim_existing_destination(&mut fixture, target, eye, [4, 4, 0]);
    let plan = connection_plan(&fixture);
    let destination = terra_world::rails::connection_target(&plan)
        .unwrap()
        .position
        .position();
    assert!(
        (0..3)
            .map(|axis| (destination[axis] - f64::from(eye[axis])).powi(2))
            .sum::<f64>()
            > 4.5 * 4.5
    );
    let before = rpc_snapshot(&fixture);
    assert_eq!(
        run(&mut fixture.app, &fixture.peer, fixture.command, false),
        EditOutcome::OutOfReach
    );
    assert_eq!(rpc_snapshot(&fixture), before);
    assert!(terra_world::rails::complete(
        fixture.app.world().resource::<EditJournal>(),
        target
    ));
}

#[test]
fn connection_clears_generated_and_saved_props_only_after_drop_capacity_is_reserved() {
    for full_drops in [false, true] {
        let mut fixture = connection_fixture(WoodSpecies::Birch);
        let generated = add_prop(&mut fixture, 205_621, PropKind::SmallStones, false, 4.5);
        let saved = add_prop(&mut fixture, 205_622, PropKind::Grass, true, 7.5);
        let plan = connection_plan(&fixture);
        let cleared = terra_world::rails::clearance(
            &plan,
            fixture.app.world().resource::<ChunkStore>(),
            fixture.app.world().resource::<EditJournal>(),
        )
        .unwrap();
        assert_eq!(cleared.len(), 2);
        assert!(cleared.iter().any(|entry| entry.prop.id == generated.id
            && entry.source == terra_world::rails::PropSource::Generated));
        assert!(cleared.iter().any(|entry| entry.prop.id == saved.id
            && entry.source == terra_world::rails::PropSource::Saved));
        let expected_drops: Vec<_> = cleared
            .iter()
            .flat_map(|entry| {
                resource_stacks_for_prop(&entry.prop, fixture.app.world().resource::<EditJournal>())
            })
            .collect();
        assert!(
            !expected_drops.is_empty(),
            "clearance genuinely needs a drop reservation"
        );
        if full_drops {
            let mut journal = fixture.app.world_mut().resource_mut::<EditJournal>();
            for _ in 0..terra_core::MAX_WORLD_DROPS {
                assert!(
                    journal
                        .gameplay_mut()
                        .drop_one([100.0; 3], ItemKind::Block(3))
                );
            }
        }
        let before = rpc_snapshot(&fixture);
        let outcome = run(&mut fixture.app, &fixture.peer, fixture.command, false);
        if full_drops {
            assert_eq!(outcome, EditOutcome::RateLimited);
            assert_eq!(rpc_snapshot(&fixture), before);
        } else {
            assert_eq!(outcome, EditOutcome::Applied);
            let journal = fixture.app.world().resource::<EditJournal>();
            assert!(terra_world::rails::complete(journal, fixture.curve));
            for prop in [generated, saved] {
                assert!(journal.prop_removed(prop.section, prop.id));
                assert!(
                    !fixture.app.world().resource::<ChunkStore>().loaded[&prop.section]
                        .props
                        .iter()
                        .any(|live| live.id == prop.id && !live.removed)
                );
            }
            assert!(
                journal
                    .placed_props(saved.section)
                    .all(|prop| prop.id != saved.id)
            );
            assert_eq!(
                journal
                    .gameplay()
                    .drops
                    .iter()
                    .map(|drop| (drop.position, drop.stack))
                    .collect::<Vec<_>>(),
                expected_drops
            );
        }
    }
}

#[test]
fn clearance_checks_foreign_prop_support_outside_the_entire_rail_footprint() {
    let (mut app, peer, piece, command) = rail_fixture(rails::STRAIGHT, [14, 4, 10]);
    let prop = WorldProp {
        id: PropId(205_623),
        kind: PropKind::Grass,
        position: [16.1, 4.0, 10.5],
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section: global_to_section([16, 3, 10]).0,
        removed: false,
    };
    set_block(&mut app, [16, 3, 10], 3);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&prop.section)
        .unwrap()
        .props
        .push(prop);
    let plan = terra_world::rails::plan(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        piece.anchor,
        piece.material,
        piece.encoded_facing(),
    )
    .unwrap();
    let clearance = terra_world::rails::clearance(
        &plan,
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
    )
    .unwrap();
    assert_eq!(clearance.len(), 1);
    let support = terra_world::props::placement::support(&prop);
    assert!(clearance[0].rights_cells.contains(&support));
    assert!(!terra_world::rails::cells(piece).contains(&support));
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .kingdoms
        .found(
            PlayerUuid::from_u128(205_624),
            [16, 8, 10],
            "Foreign prop soil",
        )
        .unwrap();
    let registry = &app.world().resource::<EditJournal>().gameplay().kingdoms;
    assert!(
        terra_world::rails::cells(piece)
            .iter()
            .all(|cell| registry.can(peer.uuid, KingdomPermission::Build, *cell))
    );
    assert!(!registry.can(peer.uuid, KingdomPermission::Break, support));
    let before = snapshot(&app, piece);
    assert_eq!(run(&mut app, &peer, command, false), EditOutcome::Invalid);
    assert_eq!(snapshot(&app, piece), before);
    assert!(
        !app.world()
            .resource::<EditJournal>()
            .prop_removed(prop.section, prop.id)
    );
    assert!(
        app.world().resource::<ChunkStore>().loaded[&prop.section]
            .props
            .iter()
            .any(|live| live.id == prop.id && !live.removed)
    );
}
