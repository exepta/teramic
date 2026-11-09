use super::*;
use terra_core::rails::BuildChoice;

#[derive(Clone, Copy)]
enum Scenario {
    Straight,
    Ramp,
}

struct Fixture {
    app: App,
    peer: Peer,
    old: Piece,
    next: Piece,
    selected: terra_world::rails::ContextChoice,
    command: EditCommand,
}

fn fixture_context(scenario: Scenario) -> Fixture {
    let (mut app, peer, old, place) = rail_fixture(rails::STRAIGHT, [8, 4, 3]);
    assert_eq!(run(&mut app, &peer, place, false), EditOutcome::Applied);
    for x in 3..=12 {
        for z in -8..=7 {
            set_block(&mut app, [x, 3, z], 3);
            if app
                .world()
                .resource::<EditJournal>()
                .block([x, 4, z])
                .is_none()
            {
                set_block(&mut app, [x, 4, z], 0);
            }
            set_block(&mut app, [x, 5, z], 0);
        }
    }
    let (eye, choice) = match scenario {
        Scenario::Straight => (Vec3::new(8.5, 6.0, -2.5), BuildChoice::Straight),
        Scenario::Ramp => {
            set_block(&mut app, [8, 4, -1], 3);
            (Vec3::new(8.5, 7.0, -1.5), BuildChoice::Straight)
        }
    };
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    let look = match scenario {
        Scenario::Straight => target_look(&app, &peer, [8, 4, 0], old.material),
        Scenario::Ramp => aim(&app, &peer, [8.5, 5.0, -0.5]),
    };
    let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let (_, target) = terra_world::targeting::pick_world_for_placement(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        app.world()
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
        PropPose::Rest,
    );
    let Some(WorldTarget::Block(hit)) = target else {
        panic!("real contextual hit is required")
    };
    assert!(hit.distance <= 4.5);
    let selected = terra_world::rails::context_choice(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        hit,
        (
            eye.to_array().map(f64::from),
            direction.to_array().map(f64::from),
        ),
        look,
        terra_core::WoodSpecies::Oak,
        choice,
    );
    assert!(selected.valid, "{:?}", selected.error);
    assert_eq!(
        selected.replaced.is_some(),
        matches!(scenario, Scenario::Ramp)
    );
    let layout = selected.layout.as_ref().unwrap();
    let next = layout
        .states
        .iter()
        .find_map(|(cell, state)| {
            let state = (*state)?;
            if state.secondary {
                return None;
            }
            let material = layout
                .edits
                .iter()
                .find(|edit| edit.position == *cell)?
                .material;
            terra_world::rails::piece_from_state(*cell, material, state)
        })
        .unwrap();
    let cost = terra_world::rails::cost(layout)
        .unwrap()
        .saturating_sub(selected.replaced.map_or(0, terra_world::rails::piece_cost));
    let source = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let bag = journal
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap();
        let wanted = ItemStack {
            item: ItemKind::Block(rails::STRAIGHT),
            count: cost + 1,
        };
        if bag.slot(0) != Some(wanted) {
            assert!(bag.creative_set(0, wanted));
        }
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
            .revision(global_to_section(hit.position).0),
        look,
        source: Some(source),
        action: EditAction::BuildRail {
            position: hit.position,
            expected: app
                .world()
                .resource::<ChunkStore>()
                .block_at(hit.position)
                .unwrap(),
            choice,
            signature: terra_world::rails::context_signature(&selected),
        },
    };
    Fixture {
        app,
        peer,
        old,
        next,
        selected,
        command,
    }
}

#[test]
fn actual_context_extensions_and_same_wood_ramp_rewrites_charge_only_net_material() {
    for scenario in [Scenario::Straight, Scenario::Ramp] {
        let mut fixture = fixture_context(scenario);
        let replaced = fixture.selected.replaced;
        assert_eq!(
            run(&mut fixture.app, &fixture.peer, fixture.command, false),
            EditOutcome::Applied
        );
        let journal = fixture.app.world().resource::<EditJournal>();
        assert!(terra_world::rails::complete(journal, fixture.next));
        assert_eq!(
            journal.gameplay().inventories[&fixture.peer.uuid]
                .count(ItemKind::Block(rails::STRAIGHT)),
            1
        );
        assert!(
            journal.gameplay().drops.is_empty(),
            "a rewrite cannot refund or duplicate old track"
        );
        assert_eq!(
            terra_world::rails::network(journal).unwrap().len(),
            if replaced.is_some() { 1 } else { 2 }
        );
        if replaced.is_some() {
            assert_eq!(
                fixture.next.ports()[0].position,
                fixture.old.ports()[0].position
            );
            assert_eq!(
                fixture.next.ports()[0].tangent,
                fixture.old.ports()[0].tangent
            );
        } else {
            assert!(terra_world::rails::complete(journal, fixture.old));
        }
    }
}

#[test]
fn stale_context_signature_revision_protected_source_and_short_stock_roll_back_every_cell() {
    for failure in ["signature", "revision", "claim", "inventory", "choice"] {
        let mut fixture = fixture_context(Scenario::Straight);
        match failure {
            "signature" => {
                if let EditAction::BuildRail { signature, .. } = &mut fixture.command.action {
                    *signature ^= 1;
                }
            }
            "revision" => fixture.command.revision += 1,
            "claim" => {
                fixture
                    .app
                    .world_mut()
                    .resource_mut::<EditJournal>()
                    .gameplay_mut()
                    .kingdoms
                    .found(
                        PlayerUuid::from_u128(207_114),
                        [8, 10, 2],
                        "Protected context source",
                    )
                    .unwrap();
                let registry = &fixture
                    .app
                    .world()
                    .resource::<EditJournal>()
                    .gameplay()
                    .kingdoms;
                assert!(
                    terra_world::rails::cells(fixture.next)
                        .iter()
                        .all(|cell| registry.can(
                            fixture.peer.uuid,
                            terra_core::kingdom::KingdomPermission::Build,
                            *cell
                        ))
                );
            }
            "inventory" => {
                assert!(terra_world::rails::piece_cost(fixture.next) > 0);
                let mut journal = fixture.app.world_mut().resource_mut::<EditJournal>();
                let bag = journal
                    .gameplay_mut()
                    .inventories
                    .get_mut(&fixture.peer.uuid)
                    .unwrap();
                bag.slots.clear();
                bag.revision += 1;
                fixture.command.source.as_mut().unwrap().revision = bag.revision;
            }
            "choice" => {
                if let EditAction::BuildRail { choice, .. } = &mut fixture.command.action {
                    *choice = BuildChoice::Curve;
                }
            }
            _ => unreachable!(),
        }
        let before = snapshot(&fixture.app, fixture.next);
        assert_ne!(
            run(&mut fixture.app, &fixture.peer, fixture.command, false),
            EditOutcome::Applied,
            "{failure}"
        );
        assert_eq!(snapshot(&fixture.app, fixture.next), before, "{failure}");
        assert!(terra_world::rails::complete(
            fixture.app.world().resource::<EditJournal>(),
            fixture.old
        ));
    }
}

#[test]
fn aiming_a_lower_free_rail_builds_a_supported_ramp_to_bare_higher_ground_with_real_body_and_stock()
{
    for gap in 0..=4 {
        let (mut app, peer, old, place) = rail_fixture(rails::STRAIGHT, [8, 4, 3]);
        assert_eq!(run(&mut app, &peer, place, false), EditOutcome::Applied);
        for x in 4..=13 {
            for z in -10..=7 {
                set_block(&mut app, [x, 3, z], 3);
                if app
                    .world()
                    .resource::<EditJournal>()
                    .block([x, 4, z])
                    .is_none()
                {
                    set_block(&mut app, [x, 4, z], if z <= -1 - gap { 3 } else { 0 });
                }
                set_block(&mut app, [x, 5, z], 0);
            }
        }
        let position = old.ports()[1].position.position();
        let eye = Vec3::new(
            (position[0] - 2.3) as f32,
            (position[1] + 1.8) as f32,
            (-gap as f64 * 0.5 + 0.25) as f32,
        );
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
        let look = aim(
            &app,
            &peer,
            [position[0] - 1.0, position[1] + 0.18, position[2] + 0.2],
        );
        let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
        let (_, target) = terra_world::targeting::pick_world_for_placement(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            app.world()
                .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
            eye.to_array().map(f64::from),
            direction.to_array().map(f64::from),
            PropPose::Rest,
        );
        let Some(WorldTarget::Block(hit)) = target else {
            panic!("the real lower rail must be visible")
        };
        assert_eq!(
            terra_world::rails::owner_at(app.world().resource::<EditJournal>(), hit.position),
            Some(old)
        );
        let selected = terra_world::rails::context_choice(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            hit,
            (
                eye.to_array().map(f64::from),
                direction.to_array().map(f64::from),
            ),
            look,
            terra_core::WoodSpecies::Oak,
            BuildChoice::Automatic,
        );
        assert!(selected.valid, "gap {gap}: {:?}", selected.error);
        assert_eq!(selected.replaced, Some(old));
        let layout = selected.layout.as_ref().unwrap();
        let next = layout
            .states
            .iter()
            .find_map(|(cell, state)| {
                let state = (*state)?;
                (!state.secondary).then(|| {
                    terra_world::rails::piece_from_state(*cell, rails::STRAIGHT, state).unwrap()
                })
            })
            .unwrap();
        let net = terra_world::rails::cost(layout).unwrap() - terra_world::rails::piece_cost(old);
        assert!(net > 0);
        let source = {
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
                    count: net + 1
                }
            ));
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
                .revision(global_to_section(hit.position).0),
            look,
            source: Some(source),
            action: EditAction::BuildRail {
                position: hit.position,
                expected: old.material,
                choice: BuildChoice::Automatic,
                signature: terra_world::rails::context_signature(&selected),
            },
        };
        assert_eq!(
            run(&mut app, &peer, command, false),
            EditOutcome::Applied,
            "gap {gap}"
        );
        let journal = app.world().resource::<EditJournal>();
        assert!(terra_world::rails::complete(journal, next));
        assert_eq!(journal.rail_module_count(), 1);
        assert_eq!(
            journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(rails::STRAIGHT)),
            1
        );
        assert!(journal.gameplay().drops.is_empty());
        assert_eq!(next.ports()[0].position, old.ports()[0].position);
        assert_eq!(next.ports()[0].tangent, old.ports()[0].tangent);
        assert_eq!(
            app.world()
                .resource::<ChunkStore>()
                .block_at([8, 4, -1 - gap]),
            Some(3)
        );
    }
}

#[test]
fn a_signed_stretch_uses_the_real_long_ray_and_one_target_support_without_spending_on_stale_intent()
{
    let (mut app, peer, old, place) = rail_fixture(rails::STRAIGHT, [8, 4, 3]);
    assert_eq!(run(&mut app, &peer, place, false), EditOutcome::Applied);
    for x in 6..=10 {
        for z in -22..=7 {
            if app
                .world()
                .resource::<EditJournal>()
                .block([x, 4, z])
                .is_none()
            {
                set_block(&mut app, [x, 4, z], 0);
            }
        }
    }
    let target = [8, 3, -20];
    set_block(&mut app, target, 3);
    let eye =
        app.world().get::<Transform>(peer.body).unwrap().translation + Vec3::Y * PLAYER_EYE_OFFSET;
    let look = aim(&app, &peer, [8.5, 4.0, -19.5]);
    let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let (_, hit) = terra_world::targeting::pick_world_for_rails(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        app.world()
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
        PropPose::Rest,
    );
    let Some(WorldTarget::Block(hit)) = hit else {
        panic!("the actual distant support must be visible")
    };
    assert_eq!(hit.position, target);
    assert!(hit.distance > 4.5 && hit.distance <= rails::RAIL_BUILD_REACH);
    let start = rails::Endpoint {
        anchor: old.anchor,
        port: 1,
    };
    let plan = terra_world::rails::connection_aim(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        start,
        hit,
        (
            eye.to_array().map(f64::from),
            direction.to_array().map(f64::from),
        ),
        look,
        terra_core::WoodSpecies::Oak,
    )
    .unwrap();
    let endpoint = terra_world::rails::next_endpoint(&plan).unwrap();
    let state = plan
        .states
        .iter()
        .find(|(cell, _)| *cell == endpoint.anchor)
        .unwrap()
        .1
        .unwrap();
    let next =
        terra_world::rails::piece_from_state(endpoint.anchor, rails::STRAIGHT, state).unwrap();
    assert_eq!(
        next.supports()
            .iter()
            .filter(|point| app
                .world()
                .resource::<ChunkStore>()
                .block_at(std::array::from_fn(
                    |axis| next.anchor[axis] + i64::from(point.support[axis])
                ))
                == Some(3))
            .count(),
        1
    );
    let cost = terra_world::rails::cost(&plan).unwrap();
    let source = {
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
                count: cost + 1
            }
        ));
        InventorySource {
            slot: 0,
            revision: bag.revision,
        }
    };
    let signature = terra_world::rails::connection_signature(&plan, start);
    let command = EditCommand {
        request: 3,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(global_to_section(hit.position).0),
        look,
        source: Some(source),
        action: EditAction::StretchRail {
            start,
            position: hit.position,
            expected: 3,
            signature,
        },
    };
    let mut blocking_car = terra_core::trains::Vehicle {
        name: String::new(),
        id: 1,
        owner: peer.uuid,
        kind: terra_core::trains::VehicleKind::Freight,
        anchor: [8, 4, -6],
        parts: [
            ([0, 0, 0], 210),
            ([0, -1, 0], 211),
            ([0, -1, 1], 211),
            ([0, 0, 1], 212),
            ([0, 1, 0], 216),
            ([0, 0, -1], 213),
        ]
        .into_iter()
        .map(|(offset, material)| terra_core::trains::TrainPart {
            offset,
            material,
            facing: 0,
            wood: terra_core::WoodSpecies::Oak,
        })
        .collect(),
        cargo: Default::default(),
        blocks: Default::default(),
        payloads: Default::default(),
        couplings: [None; 2],
        revision: 1,

        motion: Default::default(),
        burn_seconds: 0.0,
    };
    blocking_car.cargo.insert(
        4,
        Inventory {
            slots: Vec::new(),
            ..Default::default()
        },
    );
    assert!(blocking_car.valid());
    let obstruction = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let id = journal.gameplay_mut().trains.insert(blocking_car).unwrap();
        let obstruction = terra_world::trains::raycast(
            &journal,
            eye.to_array().map(f64::from),
            direction.to_array().map(f64::from),
            hit.distance,
        )
        .unwrap();
        assert!(obstruction.distance > 4.5 && obstruction.distance < hit.distance);
        id
    };
    let blocked_before = snapshot(&app, next);
    assert_eq!(run(&mut app, &peer, command, false), EditOutcome::Occluded);
    assert_eq!(
        snapshot(&app, next),
        blocked_before,
        "occlusion cannot consume stock or publish rail metadata"
    );
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .vehicles
        .remove(&obstruction);
    let before = snapshot(&app, next);
    let mut stale = command;
    stale.request = 2;
    if let EditAction::StretchRail { signature, .. } = &mut stale.action {
        *signature ^= 1;
    }
    assert_eq!(run(&mut app, &peer, stale, false), EditOutcome::Conflict);
    assert_eq!(snapshot(&app, next), before);
    assert_eq!(run(&mut app, &peer, command, false), EditOutcome::Applied);
    let journal = app.world().resource::<EditJournal>();
    assert!(terra_world::rails::complete(journal, old));
    assert!(terra_world::rails::complete(journal, next));
    assert_eq!(
        journal.gameplay().inventories[&peer.uuid].count(ItemKind::Block(rails::STRAIGHT)),
        1
    );
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at(target),
        Some(3)
    );
}
