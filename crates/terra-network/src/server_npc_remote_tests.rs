//! The construction camera sends the ordinary InspectNpc request; authority is
//! derived from current server membership, real ownership and claimed land.
use super::*;
use terra_core::kingdom::{ClaimActor, ClaimKind, KingdomId, KingdomRole};

fn remote_fixture(member: bool) -> (App, Peer, u64, KingdomId, PlayerUuid) {
    let (mut app, peer, npc, storage) = worker_fixture();
    apply(&mut app, &peer, assign(npc, storage)).unwrap();
    let founder = if member {
        PlayerUuid::new_v4()
    } else {
        peer.uuid
    };
    let kingdom = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let game = journal.gameplay_mut();
        let kingdom = game
            .kingdoms
            .found(founder, [8, 4, 8], "Remote build workers")
            .unwrap();
        if member {
            game.kingdoms.join(peer.uuid, kingdom).unwrap();
            game.kingdoms
                .set_role(founder, kingdom, peer.uuid, KingdomRole::Worker)
                .unwrap();
        }
        for x in 1..=2 {
            game.kingdoms
                .claim(
                    kingdom,
                    [x, 0],
                    ClaimKind::ProtectedBuilding,
                    ClaimActor::Player(founder),
                )
                .unwrap();
        }
        game.creatures.animals.get_mut(&npc).unwrap().position = [40.5, 4.0, 6.5];
        game.jobs
            .workers
            .get_mut(&npc)
            .unwrap()
            .insert(ItemKind::Block(3), 19);
        kingdom
    };
    (app, peer, npc, kingdom, founder)
}
fn actor(app: &App, peer: &Peer) -> [f64; 3] {
    app.world()
        .get::<Transform>(peer.body)
        .unwrap()
        .translation
        .to_array()
        .map(f64::from)
}

#[test]
fn ordinary_inspect_npc_request_opens_an_owned_far_build_claim_worker_without_moving_the_avatar() {
    for member in [false, true] {
        let (mut app, peer, npc, _, _) = remote_fixture(member);
        let position = actor(&app, &peer);
        let before = app.world().resource::<EditJournal>().gameplay().clone();
        assert_eq!(
            apply(&mut app, &peer, WorkAction::InspectNpc { npc }),
            Ok(())
        );
        let snapshot = private_snapshot(&app, &peer, Some(npc), position);
        assert!(snapshot.valid());
        assert_eq!(
            snapshot.detail.unwrap().inventory.count(ItemKind::Block(3)),
            19
        );
        assert_eq!(actor(&app, &peer), position);
        assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
    }
}

#[test]
fn a_live_remote_build_worker_subscription_loses_private_inventory_when_membership_or_capabilities_are_revoked()
 {
    for leave in [false, true] {
        let (mut app, peer, npc, kingdom, founder) = remote_fixture(true);
        let position = actor(&app, &peer);
        assert!(
            private_snapshot(&app, &peer, Some(npc), position)
                .detail
                .is_some()
        );
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            if leave {
                journal
                    .gameplay_mut()
                    .kingdoms
                    .leave(peer.uuid, kingdom)
                    .unwrap();
            } else {
                journal
                    .gameplay_mut()
                    .kingdoms
                    .set_role(founder, kingdom, peer.uuid, KingdomRole::Visitor)
                    .unwrap();
            }
        }
        assert!(
            private_snapshot(&app, &peer, Some(npc), position)
                .detail
                .is_none()
        );
        assert_eq!(
            apply(&mut app, &peer, WorkAction::InspectNpc { npc }),
            Err(WorkError::Protected)
        );
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .gameplay()
                .jobs
                .workers[&npc]
                .inventory
                .count(ItemKind::Block(3)),
            19
        );
    }
}

#[test]
fn remote_build_camera_cannot_disclose_foreign_neutral_soldier_or_outside_realm_inventory() {
    for denial in 0..4 {
        let (mut app, peer, npc, _, _) = remote_fixture(false);
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            let game = journal.gameplay_mut();
            match denial {
                0 => {
                    game.creatures.owners.insert(npc, PlayerUuid::new_v4());
                }
                1 => {
                    game.creatures.owners.remove(&npc);
                }
                2 => {
                    game.creatures.animals.get_mut(&npc).unwrap().variant =
                        terra_core::creatures::soldiers::SoldierLoadout::Sword.role();
                }
                _ => {
                    game.creatures.animals.get_mut(&npc).unwrap().position = [5000.5, 4.0, 6.5];
                }
            }
        }
        let position = actor(&app, &peer);
        let before = app.world().resource::<EditJournal>().gameplay().clone();
        assert!(
            private_snapshot(&app, &peer, Some(npc), position)
                .detail
                .is_none()
        );
        assert!(apply(&mut app, &peer, WorkAction::InspectNpc { npc }).is_err());
        assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
    }
}
