use super::*;
use terra_core::kingdom::{ClaimActor, ClaimKind, claim_chunk};

#[test]
fn remote_keep_commands_use_actual_member_territory_and_stop_after_leaving_it() {
    let mut host = server();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let profile = PlayerProfile::default();
    let mut client = profile_client(address, profile.clone());
    wait(&mut host, &mut [&mut client], |_, clients| {
        let session = clients[0].world().resource::<client::ClientSession>();
        session.ready && session.confirmed_state().is_some_and(|p| p.grounded)
    });
    let cell = client
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position
        .map(|v| f64::from(v).floor() as i64);
    let chunk = claim_chunk(cell);
    let keep = [cell[0] + 8 * 16, cell[1], cell[2]];
    let foreign = [keep[0] + 3 * 16, keep[1], keep[2]];
    let id = {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        for position in [keep, foreign] {
            journal
                .record(position, terra_core::castle::BERGFRIED)
                .unwrap();
        }
        let game = journal.gameplay_mut();
        let id = game
            .kingdoms
            .found(profile.uuid, keep, "Remote home")
            .unwrap();
        for x in (chunk[0]..chunk[0] + 8).rev() {
            game.kingdoms
                .claim(
                    id,
                    [x, chunk[1]],
                    ClaimKind::RealmTerritory,
                    ClaimActor::Player(profile.uuid),
                )
                .unwrap();
        }
        game.kingdoms
            .found(PlayerUuid::from_u128(999236), foreign, "Unrelated realm")
            .unwrap();
        game.revision += 1;
        id
    };
    assert!(
        host.world()
            .resource::<ChunkStore>()
            .block_at(keep)
            .is_none(),
        "remote keep stays unloaded"
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut client],
            0,
            KingdomAction::Inspect { position: keep }
        ),
        KingdomOutcome::Inspected { kingdom: Some(id) }
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut client],
            0,
            KingdomAction::Rename {
                id,
                name: "Remote renamed".into()
            }
        ),
        KingdomOutcome::Renamed { id }
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut client],
            0,
            KingdomAction::Inspect { position: foreign }
        ),
        KingdomOutcome::Rejected(KingdomError::OutOfReach)
    );
    assert!(matches!(
        request(
            &mut host,
            &mut [&mut client],
            0,
            KingdomAction::ReleaseChunk { id, chunk }
        ),
        KingdomOutcome::Released { .. }
    ));
    assert_eq!(
        request(
            &mut host,
            &mut [&mut client],
            0,
            KingdomAction::Inspect { position: keep }
        ),
        KingdomOutcome::Rejected(KingdomError::OutOfReach)
    );
}
