use super::*;
use crate::protocol::kingdom::{KingdomAction, KingdomOutcome};
use terra_core::{
    PlayerProfile, PlayerUuid,
    kingdom::{KingdomError, KingdomId, KingdomRecord, KingdomRegistry, KingdomState},
};
use terra_world::{
    edits::EditJournal,
    streaming::{BlockEditRequest, commit_block_edit},
};

#[path = "kingdom_claim_tests.rs"]
mod claim_tests;

fn request(
    host: &mut App,
    clients: &mut [&mut App],
    actor: usize,
    action: KingdomAction,
) -> KingdomOutcome {
    let before = clients[actor]
        .world()
        .resource::<client::ClientSession>()
        .kingdom_response;
    let command = clients[actor]
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .kingdom_request(action)
        .unwrap();
    let expected_request = command.0.request;
    clients[actor].world_mut().write_message(command);
    wait(host, clients, |host, clients| {
        clients[actor]
            .world()
            .resource::<client::ClientSession>()
            .kingdom_response
            > before
            && clients.iter().all(|app| {
                app.world().resource::<client::ClientSession>().kingdoms
                    == host.world().resource::<EditJournal>().gameplay().kingdoms
            })
    });
    let session = clients[actor].world().resource::<client::ClientSession>();
    assert_eq!(
        session.last_kingdom_result.as_ref().unwrap().request,
        expected_request
    );
    assert!(
        !session.edits_pending(),
        "kingdom nonces must not block edits"
    );
    session
        .last_kingdom_result
        .as_ref()
        .unwrap()
        .outcome
        .clone()
}

fn edit(host: &mut App, position: [i64; 3], material: u16) -> Result<bool, String> {
    let properties = host
        .world()
        .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .clone();
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            let before = chunks.block_at(position).unwrap();
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position,
                    expected_material: before,
                    material,
                },
                &properties,
            )
        })
}

fn aim(host: &mut App, clients: &mut [&mut App], actor: usize, position: [i64; 3]) {
    use lightyear::prelude::MessageSender;
    use protocol::session::{InputChannel, MovementInput};
    let state = clients[actor]
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(state.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let direction = (Vec3::from_array(position.map(|v| v as f32 + 0.5)) - eye).normalize();
    let look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
    clients[actor]
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .look = look;
    let world = clients[actor].world_mut();
    world
        .query_filtered::<&mut MessageSender<MovementInput>, With<lightyear::prelude::client::Client>>()
        .single_mut(world)
        .unwrap()
        .send::<InputChannel>(MovementInput {
            generation: state.generation,
            sequence: state.ack + 1,
            direction: [0.0; 3],
            look,
            jump: false,
            sprint: false,
            auto_climb: false,
            flight_speed: None,
        });
    wait(host, clients, |_, clients| {
        clients[actor]
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .is_some_and(|current| current.ack > state.ack)
    });
}

#[test]
fn kingdom_udp_lifecycle_checks_authority_replicates_members_and_protects_the_keep() {
    let mut host = server();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let alice = PlayerProfile {
        name: "Alice".into(),
        ..Default::default()
    };
    let bob = PlayerProfile {
        name: "Bob".into(),
        ..Default::default()
    };
    let mut alice_client = profile_client(address, alice.clone());
    let mut bob_client = profile_client(address, bob.clone());
    wait(
        &mut host,
        &mut [&mut alice_client, &mut bob_client],
        |_, clients| {
            clients.iter().all(|app| {
                let session = app.world().resource::<client::ClientSession>();
                session.ready
                    && session
                        .confirmed_state()
                        .is_some_and(|state| state.grounded)
            })
        },
    );
    let state = alice_client
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(state.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let position = [
        eye.x.floor() as i64,
        eye.y.floor() as i64,
        eye.z.floor() as i64 - 3,
    ];
    assert!(edit(&mut host, position, terra_core::castle::BERGFRIED).unwrap());
    aim(
        &mut host,
        &mut [&mut alice_client, &mut bob_client],
        0,
        position,
    );
    aim(
        &mut host,
        &mut [&mut alice_client, &mut bob_client],
        1,
        position,
    );
    let remote = [500_000, 20, 0];
    host.world_mut()
        .resource_mut::<EditJournal>()
        .record(remote, terra_core::castle::BERGFRIED)
        .unwrap();
    // Closing/reopening a modal can enqueue the replacement Inspect before
    // the earlier request is sent. Both IDs must already be stable here and
    // survive the transport unchanged so the UI can ignore the old response.
    let (old, replacement, receipts) = {
        let mut session = alice_client
            .world_mut()
            .resource_mut::<client::ClientSession>();
        let old = session
            .kingdom_request(KingdomAction::Inspect { position })
            .unwrap();
        let replacement = session
            .kingdom_request(KingdomAction::Inspect { position: remote })
            .unwrap();
        assert!(replacement.0.request > old.0.request);
        assert!(!session.edits_pending());
        (old, replacement, session.kingdom_response)
    };
    let replacement_id = replacement.0.request;
    alice_client.world_mut().write_message(old);
    alice_client.world_mut().write_message(replacement);
    wait(
        &mut host,
        &mut [&mut alice_client, &mut bob_client],
        |_, clients| {
            clients[0]
                .world()
                .resource::<client::ClientSession>()
                .kingdom_response
                == receipts + 2
        },
    );
    let session = alice_client.world().resource::<client::ClientSession>();
    assert_eq!(
        session.last_kingdom_result.as_ref().unwrap().request,
        replacement_id
    );
    assert_eq!(
        session.last_kingdom_result.as_ref().unwrap().outcome,
        KingdomOutcome::Rejected(KingdomError::OutOfReach)
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut alice_client, &mut bob_client],
            0,
            KingdomAction::Found {
                position: remote,
                name: "Remote forgery".into(),
            },
        ),
        KingdomOutcome::Rejected(KingdomError::OutOfReach),
    );
    let obstruction = [position[0], position[1], position[2] + 1];
    let original = host
        .world()
        .resource::<ChunkStore>()
        .block_at(obstruction)
        .unwrap();
    assert!(edit(&mut host, obstruction, 3).unwrap());
    assert_eq!(
        request(
            &mut host,
            &mut [&mut alice_client, &mut bob_client],
            0,
            KingdomAction::Inspect { position },
        ),
        KingdomOutcome::Rejected(KingdomError::Occluded),
    );
    assert!(edit(&mut host, obstruction, original).unwrap());
    assert_eq!(
        request(
            &mut host,
            &mut [&mut alice_client, &mut bob_client],
            0,
            KingdomAction::Inspect { position }
        ),
        KingdomOutcome::Inspected { kingdom: None },
    );
    let KingdomOutcome::Founded { id } = request(
        &mut host,
        &mut [&mut alice_client, &mut bob_client],
        0,
        KingdomAction::Found {
            position,
            name: "  Northwatch  ".into(),
        },
    ) else {
        panic!("founding must succeed");
    };
    let saved_revision = host.world().resource::<EditJournal>().gameplay().revision;
    let founded = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .kingdoms
        .clone();
    assert!(
        saved_revision > 0,
        "dedicated autosave must see the mutation"
    );
    let record = alice_client
        .world()
        .resource::<client::ClientSession>()
        .kingdoms
        .get(id)
        .unwrap();
    assert_eq!(record.name, "Northwatch");
    assert_eq!(record.founder, alice.uuid);
    assert_eq!(record.founder_name, "Alice");
    assert_eq!(record.member_names, vec![(alice.uuid, "Alice".into())]);
    for action in [
        KingdomAction::Rename {
            id,
            name: "Stolen".into(),
        },
        KingdomAction::Dissolve { id },
        KingdomAction::Kick {
            id,
            member: alice.uuid,
        },
    ] {
        assert_eq!(
            request(
                &mut host,
                &mut [&mut alice_client, &mut bob_client],
                1,
                action.clone()
            ),
            KingdomOutcome::Rejected(if matches!(action, KingdomAction::Dissolve { .. }) {
                KingdomError::FounderOnly
            } else {
                KingdomError::PermissionDenied
            }),
        );
    }
    assert_eq!(
        host.world().resource::<EditJournal>().gameplay().kingdoms,
        founded
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut alice_client, &mut bob_client],
            1,
            KingdomAction::Join { id }
        ),
        KingdomOutcome::Joined { id },
    );
    assert_eq!(
        bob_client
            .world()
            .resource::<client::ClientSession>()
            .kingdoms
            .get(id)
            .unwrap()
            .member_names,
        vec![(alice.uuid, "Alice".into()), (bob.uuid, "Bob".into())],
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut alice_client, &mut bob_client],
            0,
            KingdomAction::Leave { id }
        ),
        KingdomOutcome::Rejected(KingdomError::FounderMustDissolve),
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut alice_client, &mut bob_client],
            1,
            KingdomAction::Leave { id }
        ),
        KingdomOutcome::Left { id },
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut alice_client, &mut bob_client],
            1,
            KingdomAction::Join { id }
        ),
        KingdomOutcome::Joined { id },
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut alice_client, &mut bob_client],
            0,
            KingdomAction::Kick {
                id,
                member: bob.uuid
            }
        ),
        KingdomOutcome::Kicked {
            id,
            member: bob.uuid
        },
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut alice_client, &mut bob_client],
            0,
            KingdomAction::Rename {
                id,
                name: "Westwatch".into()
            }
        ),
        KingdomOutcome::Renamed { id },
    );
    // Late joins receive the complete persistent roster, including offline-safe names.
    let mut late = profile_client(address, PlayerProfile::default());
    wait(
        &mut host,
        &mut [&mut alice_client, &mut bob_client, &mut late],
        |host, clients| {
            let session = clients[2].world().resource::<client::ClientSession>();
            session.ready
                && session.kingdoms == host.world().resource::<EditJournal>().gameplay().kingdoms
        },
    );
    let owner = terra_voxel::global_to_section(position).0;
    let revision = alice_client
        .world()
        .resource::<EditJournal>()
        .revision(owner);
    let command = alice_client
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .edit_command(
            revision,
            protocol::session::EditAction::Block {
                position,
                expected: terra_core::castle::BERGFRIED,
                material: 0,
            },
        )
        .unwrap();
    let nonce = command.request;
    send(
        &mut alice_client,
        protocol::session::ClientControl::Edit(command),
    );
    wait(
        &mut host,
        &mut [&mut alice_client, &mut bob_client, &mut late],
        |_, clients| {
            clients[0]
                .world()
                .resource::<client::ClientSession>()
                .last_edit
                .is_some_and(|(request, _)| request == nonce)
        },
    );
    assert_eq!(
        alice_client
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .unwrap()
            .1,
        protocol::session::EditOutcome::Invalid,
    );
    let before = host.world().resource::<EditJournal>().clone();
    assert!(
        edit(&mut host, position, 0)
            .unwrap_err()
            .contains("active structure")
    );
    assert_eq!(
        host.world().resource::<EditJournal>().block(position),
        before.block(position)
    );
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at(position),
        Some(terra_core::castle::BERGFRIED)
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut alice_client, &mut bob_client, &mut late],
            0,
            KingdomAction::Dissolve { id }
        ),
        KingdomOutcome::Dissolved { id },
    );
    assert!(edit(&mut host, position, 0).unwrap());
    assert!(edit(&mut host, position, terra_core::castle::BERGFRIED).unwrap());
    let KingdomOutcome::Founded { id: next } = request(
        &mut host,
        &mut [&mut alice_client, &mut bob_client, &mut late],
        0,
        KingdomAction::Found {
            position,
            name: "Westwatch".into(),
        },
    ) else {
        panic!("dissolving must release the founder, name and keep");
    };
    assert!(next.0 > id.0);
    assert_eq!(
        late.world()
            .resource::<client::ClientSession>()
            .kingdoms
            .get(id)
            .unwrap()
            .state,
        KingdomState::Dissolved
    );
}

fn maximum_registry() -> KingdomRegistry {
    use terra_core::kingdom::{MAX_KINGDOM_MEMBERS, MAX_KINGDOMS};
    let member_name = "\u{10000}".repeat(32);
    let kingdoms: Vec<_> = (0..MAX_KINGDOMS)
        .map(|index| {
            let members: Vec<_> = (0..MAX_KINGDOM_MEMBERS)
                .map(|member| {
                    PlayerUuid::from_u128(
                        u128::MAX - (index * MAX_KINGDOM_MEMBERS + member) as u128,
                    )
                })
                .collect();
            let name = format!(
                "{}{}",
                "\u{10000}".repeat(31),
                char::from_u32(0x10100 + index as u32).unwrap()
            );
            KingdomRecord {
                kind: Default::default(),
                level: 1,
                heraldry: Default::default(),
                parent: None,
                id: KingdomId(u64::MAX - 1 - index as u64),
                name,
                founder: members[0],
                founder_name: member_name.clone(),
                member_names: members
                    .iter()
                    .map(|id| (*id, member_name.clone()))
                    .collect(),
                members,
                location: [990_000 - index as i64, -990_000, 990_000],
                state: KingdomState::Active,
            }
        })
        .collect();
    let roles = kingdoms
        .iter()
        .flat_map(|k| {
            k.members
                .iter()
                .skip(1)
                .map(|member| terra_core::kingdom::MemberRole {
                    kingdom: k.id,
                    member: *member,
                    role: terra_core::kingdom::KingdomRole::Builder,
                })
        })
        .collect();
    KingdomRegistry {
        roles,
        relations: Vec::new(),
        claims: Vec::new(),
        next_id: u64::MAX,
        revision: u64::MAX,
        kingdoms,
    }
}

#[test]
fn kingdom_maximum_snapshot_roundtrips_and_reliably_reassembles_over_udp_on_late_join() {
    let expected = maximum_registry();
    assert!(expected.valid());
    let wire = protocol::session::ServerControl::Kingdoms(expected.clone());
    let encoded = postcard::to_allocvec(&wire).unwrap();
    assert!(
        encoded.len() > 2 * 1024 * 1024,
        "the test must exceed a one-MiB cap"
    );
    let protocol::session::ServerControl::Kingdoms(decoded) =
        postcard::from_bytes(&encoded).unwrap()
    else {
        panic!("incorrect control variant");
    };
    assert_eq!(decoded, expected);
    eprintln!("maximum valid kingdom control: {} bytes", encoded.len());
    let mut host = server();
    host.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .kingdoms = expected.clone();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut joined = profile_client(address, PlayerProfile::default());
    wait(&mut host, &mut [&mut joined], |_, clients| {
        let session = clients[0].world().resource::<client::ClientSession>();
        session.ready && session.kingdoms == expected
    });
    assert!(
        joined
            .world()
            .resource::<client::ClientSession>()
            .kingdoms
            .valid()
    );
}
