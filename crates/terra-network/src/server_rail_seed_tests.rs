use super::*;
use terra_core::WoodSpecies;

fn seed_fixture(wood: WoodSpecies, side: usize) -> (App, Peer, Piece, EditCommand) {
    let anchor = [15, 4, 10];
    let (mut app, peer, _, _) = rail_fixture(rails::STRAIGHT, anchor);
    for x in 11..=19 {
        for z in 6..=14 {
            set_block(&mut app, [x, 3, z], 3);
            for y in 4..=5 {
                set_block(&mut app, [x, y, z], 0);
            }
        }
    }
    let offset = [[0.0, 3.0], [3.0, 0.0], [0.0, -3.0], [-3.0, 0.0]][side];
    let eye = Vec3::new(15.5 + offset[0], 6.0, 10.5 + offset[1]);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    let look = aim(&app, &peer, [15.5, 4.0, 10.5]);
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
        panic!("real seed floor target")
    };
    assert_eq!(hit.position, [15, 3, 10]);
    let plan = terra_world::rails::seed(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        hit,
        (
            eye.to_array().map(f64::from),
            direction.to_array().map(f64::from),
        ),
        look,
        wood,
    )
    .unwrap();
    let end = terra_world::rails::next_endpoint(&plan).unwrap();
    let state = plan
        .states
        .iter()
        .find_map(|(cell, state)| (*cell == end.anchor).then_some(*state))
        .flatten()
        .unwrap();
    let piece = terra_world::rails::piece_from_state(end.anchor, rails::STRAIGHT, state).unwrap();
    let source = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let bag = journal
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap();
        let wanted = ItemStack {
            item: ItemKind::for_wood(rails::STRAIGHT, wood),
            count: 2,
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
        request: 1,
        source: Some(source),
        look,
        revision: app
            .world()
            .resource::<EditJournal>()
            .revision(global_to_section(hit.position).0),
        action: EditAction::SeedRail {
            position: hit.position,
            expected: 3,
            signature: terra_world::rails::seed_signature(&plan),
        },
    };
    (app, peer, piece, command)
}

#[test]
fn survival_seed_rpc_builds_one_metre_in_all_woods_and_headings_and_returns_one_item() {
    for wood in [
        WoodSpecies::Oak,
        WoodSpecies::Spruce,
        WoodSpecies::Birch,
        WoodSpecies::Cherry,
    ] {
        for side in 0..4 {
            let (mut app, peer, piece, command) = seed_fixture(wood, side);
            assert!(piece.curve.is_some());
            assert!((piece.paths()[0].length() - 1.0).abs() < 1e-6);
            assert_eq!(terra_world::rails::piece_cost(piece), 1);
            assert_eq!(run(&mut app, &peer, command, false), EditOutcome::Applied);
            let journal = app.world().resource::<EditJournal>();
            assert!(terra_world::rails::complete(journal, piece));
            assert!(terra_world::rails::cells(piece).iter().all(|cell| {
                journal
                    .object_state(*cell)
                    .is_some_and(|state| state.wood == wood)
            }));
            assert_eq!(
                journal.gameplay().inventories[&peer.uuid]
                    .count(ItemKind::for_wood(rails::STRAIGHT, wood)),
                1
            );
            let secondary = terra_world::rails::cells(piece)
                .into_iter()
                .find(|cell| *cell != piece.anchor)
                .unwrap();
            app.world_mut()
                .get_mut::<Transform>(peer.body)
                .unwrap()
                .translation = Vec3::new(15.5, 7.0, 10.5) - Vec3::Y * PLAYER_EYE_OFFSET;
            let remove = removal(&app, &peer, piece, secondary);
            assert_eq!(run(&mut app, &peer, remove, true), EditOutcome::Applied);
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(journal.gameplay().drops.len(), 1);
            let dropped = &journal.gameplay().drops[0];
            assert_eq!(
                dropped.stack,
                ItemStack {
                    item: ItemKind::for_wood(rails::STRAIGHT, wood),
                    count: 1
                }
            );
            assert!(
                terra_world::rails::cells(piece).iter().all(|cell| app
                    .world()
                    .resource::<ChunkStore>()
                    .block_at(*cell)
                    == Some(0))
            );
        }
    }
}

#[test]
fn seed_rpc_stale_intent_stock_or_foreign_secondary_claim_never_partially_mutates() {
    for failure in 0..5 {
        let (mut app, peer, piece, mut command) = seed_fixture(WoodSpecies::Spruce, 0);
        match failure {
            0 => {
                if let EditAction::SeedRail { signature, .. } = &mut command.action {
                    *signature ^= 1;
                }
            }
            1 => command.source.as_mut().unwrap().revision += 1,
            2 => {
                let mut journal = app.world_mut().resource_mut::<EditJournal>();
                let bag = journal
                    .gameplay_mut()
                    .inventories
                    .get_mut(&peer.uuid)
                    .unwrap();
                assert!(bag.creative_set(
                    0,
                    ItemStack {
                        item: ItemKind::for_wood(rails::SWITCH_LEFT, WoodSpecies::Spruce),
                        count: 2
                    }
                ));
                command.source.as_mut().unwrap().revision = bag.revision;
            }
            3 => {
                let mut journal = app.world_mut().resource_mut::<EditJournal>();
                journal
                    .gameplay_mut()
                    .kingdoms
                    .found(
                        terra_core::PlayerUuid::from_u128(209_101),
                        [16, 5, 10],
                        "Foreign seed edge",
                    )
                    .unwrap();
            }
            4 => {
                let source = command.source.unwrap();
                let mut journal = app.world_mut().resource_mut::<EditJournal>();
                let bag = journal
                    .gameplay_mut()
                    .inventories
                    .get_mut(&peer.uuid)
                    .unwrap();
                let item = bag.slot(usize::from(source.slot)).unwrap().item;
                while bag.take_from(usize::from(source.slot), item) {}
                command.source.as_mut().unwrap().revision = bag.revision;
            }
            _ => unreachable!(),
        }
        let before = snapshot(&app, piece);
        assert_ne!(
            run(&mut app, &peer, command, false),
            EditOutcome::Applied,
            "failure {failure}"
        );
        assert_eq!(snapshot(&app, piece), before, "failure {failure}");
    }
}
