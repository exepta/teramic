use super::*;
use crate::protocol::{
    inventory::{InventoryAction, InventoryCommand, InventoryOutcome},
    session::{ClientControl, EditAction, EditOutcome},
};
use terra_core::kingdom::{
    BotKingdom, ClaimActor, ClaimKind, DiplomaticRelation, DiplomaticTarget, KingdomHeraldry,
    KingdomIcon, KingdomRole, SettlementKind, claim_chunk,
};

#[test]
fn kingdom_claims_bot_authority_protected_inventory_paid_growth_and_late_join_are_real_udp() {
    let (mut host, mut owner_client, mut visitor_client) = pair();
    let actor = owner_client
        .world()
        .resource::<client::ClientSession>()
        .uuid
        .unwrap();
    let visitor = visitor_client
        .world()
        .resource::<client::ClientSession>()
        .uuid
        .unwrap();
    let pose = owner_client
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(pose.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let keep = [
        eye.x.floor() as i64,
        eye.y.floor() as i64,
        eye.z.floor() as i64 - 3,
    ];
    edit(&mut host, keep, terra_core::castle::BERGFRIED).unwrap();
    aim(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        0,
        keep,
    );
    aim(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        1,
        keep,
    );
    let heraldry = KingdomHeraldry {
        color: [30, 120, 210],
        icon: KingdomIcon::Anchor,
    };
    let KingdomOutcome::Founded { id } = request(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        0,
        KingdomAction::FoundSettlement {
            position: keep,
            name: "Harbor".into(),
            kind: SettlementKind::Kingdom,
            heraldry,
        },
    ) else {
        panic!("founding failed")
    };
    let base = claim_chunk(keep);
    let territory = [base[0] + 1, base[1]];
    assert_eq!(
        owner_client
            .world()
            .resource::<client::ClientSession>()
            .kingdoms
            .claim_counts(id)
            .protected,
        1
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            1,
            KingdomAction::ClaimChunk {
                id,
                chunk: territory,
                kind: ClaimKind::RealmTerritory
            }
        ),
        KingdomOutcome::Rejected(KingdomError::NotMember)
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBotChunk {
                id,
                chunk: territory,
                kind: ClaimKind::RealmTerritory,
                bot: 999_999
            }
        ),
        KingdomOutcome::Rejected(KingdomError::InvalidBot)
    );
    let bot = 70001;
    let kind = terra_core::creatures::CreatureKind::Villager;
    let bot_position = [
        (territory[0] * 16) as f64 + 2.5,
        f64::from(pose.position[1]),
        (territory[1] * 16) as f64 + 8.5,
    ];
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let state = journal.gameplay_mut();
        state.creatures.animals.insert(
            bot,
            terra_core::creatures::Creature {
                id: bot,
                kind,
                home: [0, 0],
                position: bot_position,
                velocity: [0.0; 3],
                yaw: 0.0,
                health: kind.health(),
                behavior: terra_core::creatures::CreatureBehavior::Rest,
                timer: 20.0,
                age: 0.0,
                variant: 0,
                needs: terra_core::creatures::CreatureNeeds::full(kind),
            },
        );
        state.creatures.owners.insert(bot, actor);
    }
    aim(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        0,
        [keep[0] + 5, keep[1], keep[2] + 2],
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBotChunk {
                id,
                chunk: territory,
                kind: ClaimKind::RealmTerritory,
                bot
            }
        ),
        KingdomOutcome::Claimed {
            id,
            chunk: territory,
            kind: ClaimKind::RealmTerritory
        }
    );
    let registry = &visitor_client
        .world()
        .resource::<client::ClientSession>()
        .kingdoms;
    assert!(registry.can_build(visitor, [territory[0] * 16, 20, territory[1] * 16]));
    assert!(!registry.can_build(visitor, keep));
    assert_eq!(
        registry.claim_at(territory).unwrap().actor,
        ClaimActor::Bot {
            id: bot,
            authorized_by: actor
        }
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            1,
            KingdomAction::ReleaseBotChunk {
                id,
                chunk: territory,
                bot
            }
        ),
        KingdomOutcome::Rejected(KingdomError::InvalidBot)
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ReleaseBotChunk {
                id,
                chunk: territory,
                bot
            }
        ),
        KingdomOutcome::Released {
            id,
            chunk: territory
        }
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBotChunk {
                id,
                chunk: territory,
                kind: ClaimKind::RealmTerritory,
                bot
            }
        ),
        KingdomOutcome::Claimed {
            id,
            chunk: territory,
            kind: ClaimKind::RealmTerritory
        }
    );
    let chest = [keep[0] + 1, keep[1], keep[2]];
    edit(&mut host, chest, 29).unwrap();
    visitor_client
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .last_inventory = None;
    let revision = visitor_client
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .revision;
    send(
        &mut visitor_client,
        ClientControl::Inventory(InventoryCommand {
            revision,
            action: InventoryAction::OpenContainer { position: chest },
        }),
    );
    wait(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        |_, clients| {
            clients[1]
                .world()
                .resource::<client::ClientSession>()
                .last_inventory
                .is_some()
        },
    );
    assert_eq!(
        visitor_client
            .world()
            .resource::<client::ClientSession>()
            .last_inventory,
        Some(InventoryOutcome::Denied)
    );
    assert!(
        visitor_client
            .world()
            .resource::<client::ClientSession>()
            .container
            .is_none()
    );
    let command = visitor_client
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .edit_command(
            0,
            EditAction::Block {
                position: chest,
                expected: 29,
                material: 0,
            },
        )
        .unwrap();
    let nonce = command.request;
    send(&mut visitor_client, ClientControl::Edit(command));
    wait(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        |_, clients| {
            clients[1]
                .world()
                .resource::<client::ClientSession>()
                .last_edit
                .is_some_and(|(n, _)| n == nonce)
        },
    );
    assert_eq!(
        visitor_client
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .unwrap()
            .1,
        EditOutcome::Invalid
    );
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at(chest),
        Some(29)
    );
    aim(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        0,
        keep,
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::Upgrade { id }
        ),
        KingdomOutcome::Rejected(KingdomError::UpgradeMaterialsMissing)
    );
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let bag = journal.gameplay_mut().inventories.entry(actor).or_default();
        assert_eq!(
            bag.insert(
                terra_core::ItemKind::Block(terra_core::castle::STONE_WALL),
                8
            ),
            0
        );
        assert_eq!(
            bag.insert(
                terra_core::ItemKind::Mineral(terra_core::MineralItem::GoldIngot),
                1
            ),
            0
        );
    }
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::Upgrade { id }
        ),
        KingdomOutcome::Upgraded { id, level: 2 }
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            1,
            KingdomAction::Join { id }
        ),
        KingdomOutcome::Joined { id }
    );
    assert_eq!(
        visitor_client
            .world()
            .resource::<client::ClientSession>()
            .kingdoms
            .role_for(id, visitor),
        Some(KingdomRole::Visitor)
    );
    assert!(
        !visitor_client
            .world()
            .resource::<client::ClientSession>()
            .kingdoms
            .can_build(visitor, chest)
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            1,
            KingdomAction::SetRole {
                id,
                member: visitor,
                role: KingdomRole::Officer
            }
        ),
        KingdomOutcome::Rejected(KingdomError::PermissionDenied)
    );
    aim(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        0,
        keep,
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::SetRole {
                id,
                member: visitor,
                role: KingdomRole::Builder
            }
        ),
        KingdomOutcome::RoleChanged {
            id,
            member: visitor,
            role: KingdomRole::Builder
        }
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::SetRelation {
                id,
                target: DiplomaticTarget::Bot(BotKingdom::Helteria),
                relation: DiplomaticRelation::Allied
            }
        ),
        KingdomOutcome::RelationChanged {
            id,
            target: DiplomaticTarget::Bot(BotKingdom::Helteria),
            relation: DiplomaticRelation::Allied
        }
    );
    // A grant opens the real chest, then revocation invalidates the live
    // snapshot and rejects a forged gesture using its old revisions.
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let mut contents = terra_core::Inventory::default();
        contents.insert(terra_core::ItemKind::Block(3), 2);
        journal.set_container(chest, contents).unwrap();
    }
    aim(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        1,
        chest,
    );
    let inventory_revision = visitor_client
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .revision;
    send(
        &mut visitor_client,
        ClientControl::Inventory(InventoryCommand {
            revision: inventory_revision,
            action: InventoryAction::OpenContainer { position: chest },
        }),
    );
    wait(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        |_, clients| {
            clients[1]
                .world()
                .resource::<client::ClientSession>()
                .container
                .is_some()
        },
    );
    let snapshot = visitor_client
        .world()
        .resource::<client::ClientSession>()
        .container
        .clone()
        .unwrap();
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::SetRole {
                id,
                member: visitor,
                role: KingdomRole::Visitor
            }
        ),
        KingdomOutcome::RoleChanged {
            id,
            member: visitor,
            role: KingdomRole::Visitor
        }
    );
    wait(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        |_, clients| {
            clients[1]
                .world()
                .resource::<client::ClientSession>()
                .container
                .is_none()
        },
    );
    visitor_client
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .last_inventory = None;
    send(
        &mut visitor_client,
        ClientControl::Container(crate::protocol::inventory::ContainerCommand {
            inventory_revision,
            anchor: snapshot.anchor,
            view: snapshot.view,
            revisions: snapshot
                .halves
                .iter()
                .map(|h| h.inventory.revision)
                .collect(),
            reserved: [0; terra_core::INVENTORY_SLOTS],
            action: terra_core::containers::ContainerGesture::QuickMove {
                slot: terra_core::INVENTORY_SLOTS as u8,
            },
        }),
    );
    wait(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        |_, clients| {
            clients[1]
                .world()
                .resource::<client::ClientSession>()
                .last_inventory
                .is_some()
        },
    );
    assert_eq!(
        visitor_client
            .world()
            .resource::<client::ClientSession>()
            .last_inventory,
        Some(InventoryOutcome::ContainerUnavailable)
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .container_at(chest)
            .unwrap()
            .count(terra_core::ItemKind::Block(3)),
        2
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::SetRole {
                id,
                member: visitor,
                role: KingdomRole::Builder
            }
        ),
        KingdomOutcome::RoleChanged {
            id,
            member: visitor,
            role: KingdomRole::Builder
        }
    );
    aim(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        1,
        keep,
    );
    let player_a = owner_client
        .world()
        .resource::<client::ClientSession>()
        .player
        .unwrap();
    wait(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        |_, clients| {
            clients[1]
                .world()
                .resource::<client::ReplicatedPlayers>()
                .players
                .get(&player_a)
                .is_some_and(|track| track.current.kingdom_heraldry == Some(heraldry))
        },
    );
    assert!(
        visitor_client
            .world()
            .resource::<client::ClientSession>()
            .kingdoms
            .can_build(visitor, chest)
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            1,
            KingdomAction::Upgrade { id }
        ),
        KingdomOutcome::Rejected(KingdomError::PermissionDenied)
    );
    aim(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        1,
        chest,
    );
    let revision = host
        .world()
        .resource::<EditJournal>()
        .revision(terra_voxel::global_to_section(chest).0);
    let command = visitor_client
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .edit_command(
            revision,
            EditAction::Block {
                position: chest,
                expected: 29,
                material: 0,
            },
        )
        .unwrap();
    let nonce = command.request;
    send(&mut visitor_client, ClientControl::Edit(command));
    wait(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        |_, clients| {
            clients[1]
                .world()
                .resource::<client::ClientSession>()
                .last_edit
                .is_some_and(|(n, _)| n == nonce)
        },
    );
    assert_eq!(
        visitor_client
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .unwrap()
            .1,
        EditOutcome::Applied,
        "a granted Builder role must authorize real protected building edits"
    );
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut late = profile_client(address, PlayerProfile::default());
    wait(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client, &mut late],
        |host, clients| {
            clients[2].world().resource::<client::ClientSession>().ready
                && clients[2]
                    .world()
                    .resource::<client::ClientSession>()
                    .kingdoms
                    == host.world().resource::<EditJournal>().gameplay().kingdoms
        },
    );
    assert_eq!(
        late.world()
            .resource::<client::ClientSession>()
            .kingdoms
            .get(id)
            .unwrap()
            .level,
        2
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client, &mut late],
            0,
            KingdomAction::Dissolve { id }
        ),
        KingdomOutcome::Dissolved { id }
    );
    assert!(
        late.world()
            .resource::<client::ClientSession>()
            .kingdoms
            .claims
            .is_empty()
    );
}

#[test]
fn rectangle_apply_is_atomic_and_replicated_over_real_udp() {
    let (mut host, mut owner_client, mut visitor_client) = pair();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .kingdom_rules
        .realm_min_border_gap = 0;
    let pose = owner_client
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(pose.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let keep = [
        eye.x.floor() as i64,
        eye.y.floor() as i64,
        eye.z.floor() as i64 - 3,
    ];
    edit(&mut host, keep, terra_core::castle::BERGFRIED).unwrap();
    aim(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        0,
        keep,
    );
    let KingdomOutcome::Founded { id } = request(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        0,
        KingdomAction::Found {
            position: keep,
            name: "Batchwatch".into(),
        },
    ) else {
        panic!("founding failed")
    };
    let base = claim_chunk(keep);
    // Far cells precede their connecting border: order must not cause rejection.
    let claims = vec![
        ([base[0] + 2, base[1] + 1], ClaimKind::RealmTerritory),
        ([base[0] + 2, base[1]], ClaimKind::ProtectedBuilding),
        ([base[0] + 1, base[1] + 1], ClaimKind::RealmTerritory),
        ([base[0] + 1, base[1]], ClaimKind::ProtectedBuilding),
    ];
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBatch {
                id,
                claims: claims.clone()
            }
        ),
        KingdomOutcome::ClaimsApplied { id, count: 4 }
    );
    let before = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .kingdoms
        .clone();
    assert_eq!(
        visitor_client
            .world()
            .resource::<client::ClientSession>()
            .kingdoms,
        before
    );
    assert_eq!(before.claim_counts(id).protected, 3);
    assert_eq!(before.claim_counts(id).territory, 2);
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBatch {
                id,
                claims: vec![
                    ([base[0] + 3, base[1]], ClaimKind::ProtectedBuilding),
                    ([base[0] + 3, base[1] + 3], ClaimKind::RealmTerritory)
                ]
            }
        ),
        KingdomOutcome::Rejected(KingdomError::ClaimDisconnected)
    );
    assert_eq!(
        host.world().resource::<EditJournal>().gameplay().kingdoms,
        before,
        "A rejected selection must not leave partial ownership behind"
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            1,
            KingdomAction::ClaimBatch {
                id,
                claims: claims.clone()
            }
        ),
        KingdomOutcome::Rejected(KingdomError::NotMember)
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBatch { id, claims }
        ),
        KingdomOutcome::ClaimsApplied { id, count: 0 }
    );
    assert_eq!(
        host.world().resource::<EditJournal>().gameplay().kingdoms,
        before
    );

    // The map may extend connected territory while the founder remains at the
    // keep. A Build border and a Territory border belong to the same settlement.
    let extension = (3..=40)
        .rev()
        .map(|offset| ([base[0] + offset, base[1]], ClaimKind::RealmTerritory))
        .collect::<Vec<_>>();
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBatch {
                id,
                claims: extension
            }
        ),
        KingdomOutcome::ClaimsApplied { id, count: 38 }
    );
    let extended = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .kingdoms
        .clone();
    assert_eq!(extended.claim_counts(id).territory, 40);
    assert_eq!(
        visitor_client
            .world()
            .resource::<client::ClientSession>()
            .kingdoms,
        extended
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBatch {
                id,
                claims: vec![
                    ([base[0] + 41, base[1]], ClaimKind::ProtectedBuilding),
                    ([base[0] + 43, base[1]], ClaimKind::RealmTerritory),
                ],
            }
        ),
        KingdomOutcome::Rejected(KingdomError::ClaimDisconnected)
    );
    assert_eq!(
        host.world().resource::<EditJournal>().gameplay().kingdoms,
        extended,
        "a distant invalid batch must preserve every previously confirmed claim"
    );

    // A valid custom setting may request a buffer far larger than the bounded
    // historical-site query. Fail closed before generating its whole halo.
    let mut frozen = host
        .world()
        .resource::<terra_world::streaming::WorldTerrain>()
        .0
        .config()
        .clone();
    frozen.structures_revision = 8;
    frozen.structures.set_all(true);
    host.insert_resource(terra_world::streaming::WorldTerrain(
        terra_world::generation::TerraGenerator::new(frozen).unwrap(),
    ));
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .kingdom_rules
        .realm_min_border_gap = 4096;
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimChunk {
                id,
                chunk: [base[0] + 41, base[1]],
                kind: ClaimKind::RealmTerritory,
            }
        ),
        KingdomOutcome::Rejected(KingdomError::CapacityReached)
    );
    assert_eq!(
        host.world().resource::<EditJournal>().gameplay().kingdoms,
        extended,
        "an over-budget native query cannot commit a partial claim"
    );
}

#[test]
fn dependent_village_uses_outpost_territory_limits_and_family_roles_over_real_udp() {
    let (mut host, mut owner_client, mut visitor_client) = pair();
    let actor = owner_client
        .world()
        .resource::<client::ClientSession>()
        .uuid
        .unwrap();
    let visitor = visitor_client
        .world()
        .resource::<client::ClientSession>()
        .uuid
        .unwrap();
    let pose = owner_client
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(pose.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let keep = [
        eye.x.floor() as i64,
        eye.y.floor() as i64,
        eye.z.floor() as i64 - 3,
    ];
    edit(&mut host, keep, terra_core::castle::BERGFRIED).unwrap();
    let base = claim_chunk(keep);
    let rules = terra_core::kingdom::KingdomRules {
        realm_min_border_gap: 0,
        ..Default::default()
    };
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .kingdom_rules = rules;
    let mut registry = KingdomRegistry::default();
    let main = registry
        .found_settlement_with_rules(
            actor,
            "Founder",
            keep,
            "Mainwatch",
            SettlementKind::Kingdom,
            KingdomHeraldry::default(),
            &rules,
            &[],
        )
        .unwrap();
    let at = |offset: i64| [(base[0] + offset) * 16 + 8, keep[1], base[1] * 16 + 8];
    let outpost = registry
        .found_settlement_with_rules(
            actor,
            "Founder",
            at(40),
            "Farwatch",
            SettlementKind::Outpost,
            KingdomHeraldry::default(),
            &rules,
            &[],
        )
        .unwrap();
    registry
        .claim_batch_with_rules(
            outpost,
            &(41..=54)
                .map(|offset| ([base[0] + offset, base[1]], ClaimKind::RealmTerritory))
                .collect::<Vec<_>>(),
            ClaimActor::Player(actor),
            &rules,
            &[],
        )
        .unwrap();
    let village = registry
        .found_settlement_with_rules(
            actor,
            "Founder",
            at(42),
            "Orchard",
            SettlementKind::Village,
            KingdomHeraldry::default(),
            &rules,
            &[],
        )
        .unwrap();
    assert!(registry.valid());
    // A loaded save may contain a distant dependent settlement even while its
    // founder is at the main keep. All following mutations use real commands.
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let gameplay = journal.gameplay_mut();
        gameplay.kingdoms = registry;
        gameplay.revision += 1;
    }
    let chunk = |offset| [base[0] + offset, base[1]];
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            1,
            KingdomAction::ClaimChunk {
                id: village,
                chunk: chunk(43),
                kind: ClaimKind::ProtectedBuilding,
            }
        ),
        KingdomOutcome::Rejected(KingdomError::NotMember)
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBatch {
                id: village,
                claims: (43..=51)
                    .map(|offset| (chunk(offset), ClaimKind::ProtectedBuilding))
                    .collect(),
            }
        ),
        KingdomOutcome::ClaimsApplied {
            id: village,
            count: 9
        }
    );
    let before = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .kingdoms
        .clone();
    assert_eq!(before.claim_counts(village).protected, 10);
    assert_eq!(before.claim_counts(village).territory, 0);
    assert_eq!(before.claim_at(chunk(42)).unwrap().kingdom, village);
    assert!(
        before
            .claims
            .iter()
            .any(|claim| claim.kingdom == outpost && claim.chunk == chunk(42))
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBatch {
                id: village,
                claims: vec![
                    (chunk(52), ClaimKind::ProtectedBuilding),
                    (chunk(53), ClaimKind::ProtectedBuilding)
                ],
            }
        ),
        KingdomOutcome::Rejected(KingdomError::ClaimLimitReached)
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimChunk {
                id: village,
                chunk: chunk(55),
                kind: ClaimKind::RealmTerritory,
            }
        ),
        KingdomOutcome::Rejected(KingdomError::SettlementTerritoryForbidden)
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ReleaseChunk {
                id: outpost,
                chunk: chunk(42),
            }
        ),
        KingdomOutcome::Rejected(KingdomError::ParentTerritoryInUse)
    );
    assert_eq!(
        host.world().resource::<EditJournal>().gameplay().kingdoms,
        before
    );
    aim(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        1,
        keep,
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            1,
            KingdomAction::Join { id: main }
        ),
        KingdomOutcome::Joined { id: main }
    );
    aim(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        0,
        keep,
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::SetRole {
                id: main,
                member: visitor,
                role: KingdomRole::Builder,
            }
        ),
        KingdomOutcome::RoleChanged {
            id: main,
            member: visitor,
            role: KingdomRole::Builder
        }
    );
    let live = &visitor_client
        .world()
        .resource::<client::ClientSession>()
        .kingdoms;
    assert!(live.get(village).unwrap().members.contains(&visitor));
    assert_eq!(live.role_for(outpost, visitor), Some(KingdomRole::Builder));
    assert!(live.can_build(visitor, at(43)));
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut late = profile_client(address, PlayerProfile::default());
    wait(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client, &mut late],
        |host, clients| {
            let session = clients[2].world().resource::<client::ClientSession>();
            session.ready
                && session.kingdoms == host.world().resource::<EditJournal>().gameplay().kingdoms
        },
    );
    let confirmed = &late.world().resource::<client::ClientSession>().kingdoms;
    assert_eq!(confirmed.get(village).unwrap().parent, Some(main));
    assert_eq!(confirmed.get(outpost).unwrap().parent, Some(main));
    assert_eq!(confirmed.claim_at(chunk(43)).unwrap().kingdom, village);
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client, &mut late],
            0,
            KingdomAction::Dissolve { id: main }
        ),
        KingdomOutcome::Dissolved { id: main }
    );
    let dissolved = &late.world().resource::<client::ClientSession>().kingdoms;
    assert!(dissolved.claims.is_empty());
    for id in [main, outpost, village] {
        assert_eq!(dissolved.get(id).unwrap().state, KingdomState::Dissolved);
    }
}

#[test]
fn build_claims_require_their_own_keep_path_and_bridge_changes_are_atomic_over_udp() {
    let (mut host, mut owner_client, mut visitor_client) = pair();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .kingdom_rules
        .realm_min_border_gap = 0;
    let pose = owner_client
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(pose.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let keep = [
        eye.x.floor() as i64,
        eye.y.floor() as i64,
        eye.z.floor() as i64 - 3,
    ];
    edit(&mut host, keep, terra_core::castle::BERGFRIED).unwrap();
    aim(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        0,
        keep,
    );
    let KingdomOutcome::Founded { id } = request(
        &mut host,
        &mut [&mut owner_client, &mut visitor_client],
        0,
        KingdomAction::Found {
            position: keep,
            name: "Bridgewatch".into(),
        },
    ) else {
        panic!("founding failed")
    };
    let base = claim_chunk(keep);
    let chunk = |x, z| [base[0] + x, base[1] + z];
    let changes = (1..=3)
        .map(|x| (chunk(x, 0), ClaimKind::RealmTerritory))
        .collect();
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBatch {
                id,
                claims: changes
            }
        ),
        KingdomOutcome::ClaimsApplied { id, count: 3 }
    );
    let before = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .kingdoms
        .clone();
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimChunk {
                id,
                chunk: chunk(3, 0),
                kind: ClaimKind::ProtectedBuilding
            }
        ),
        KingdomOutcome::Rejected(KingdomError::ClaimDisconnected)
    );
    assert_eq!(
        host.world().resource::<EditJournal>().gameplay().kingdoms,
        before
    );
    // The farthest cell comes first. Validate the finished Build corridor.
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBatch {
                id,
                claims: (1..=3)
                    .rev()
                    .map(|x| (chunk(x, 0), ClaimKind::ProtectedBuilding))
                    .collect()
            }
        ),
        KingdomOutcome::ClaimsApplied { id, count: 3 }
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBatch {
                id,
                claims: (0..=3)
                    .map(|x| (chunk(x, 1), ClaimKind::RealmTerritory))
                    .collect()
            }
        ),
        KingdomOutcome::ClaimsApplied { id, count: 4 }
    );
    let before = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .kingdoms
        .clone();
    // A complete Territory bypass cannot stand in for a Build bridge.
    for action in [
        KingdomAction::ClaimChunk {
            id,
            chunk: chunk(1, 0),
            kind: ClaimKind::RealmTerritory,
        },
        KingdomAction::ReleaseChunk {
            id,
            chunk: chunk(1, 0),
        },
    ] {
        assert_eq!(
            request(
                &mut host,
                &mut [&mut owner_client, &mut visitor_client],
                0,
                action
            ),
            KingdomOutcome::Rejected(KingdomError::ClaimDisconnected)
        );
        assert_eq!(
            host.world().resource::<EditJournal>().gameplay().kingdoms,
            before
        );
    }
    // One batch may replace a bridge with another complete Build route.
    let reroute = std::iter::once((chunk(1, 0), ClaimKind::RealmTerritory))
        .chain(
            (0..=3)
                .rev()
                .map(|x| (chunk(x, 1), ClaimKind::ProtectedBuilding)),
        )
        .collect();
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBatch {
                id,
                claims: reroute
            }
        ),
        KingdomOutcome::ClaimsApplied { id, count: 5 }
    );
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .kingdom_rules
        .require_adjacent_claims = false;
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimChunk {
                id,
                chunk: chunk(2, 1),
                kind: ClaimKind::RealmTerritory
            }
        ),
        KingdomOutcome::Claimed {
            id,
            chunk: chunk(2, 1),
            kind: ClaimKind::RealmTerritory
        }
    );
    let historical = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .kingdoms
        .clone();
    assert!(
        historical.valid(),
        "the explicit opt-out remains persistent historical data"
    );
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .kingdom_rules
        .require_adjacent_claims = true;
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimChunk {
                id,
                chunk: chunk(4, 0),
                kind: ClaimKind::ProtectedBuilding
            }
        ),
        KingdomOutcome::Rejected(KingdomError::ClaimDisconnected)
    );
    assert_eq!(
        host.world().resource::<EditJournal>().gameplay().kingdoms,
        historical
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimChunk {
                id,
                chunk: chunk(2, 1),
                kind: ClaimKind::ProtectedBuilding
            }
        ),
        KingdomOutcome::Claimed {
            id,
            chunk: chunk(2, 1),
            kind: ClaimKind::ProtectedBuilding
        }
    );
    // No keep or actor radius: a valid Build path may extend for hundreds of meters.
    assert_eq!(
        request(
            &mut host,
            &mut [&mut owner_client, &mut visitor_client],
            0,
            KingdomAction::ClaimBatch {
                id,
                claims: (-20..0)
                    .map(|x| (chunk(x, 0), ClaimKind::ProtectedBuilding))
                    .collect()
            }
        ),
        KingdomOutcome::ClaimsApplied { id, count: 20 }
    );
    let confirmed = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .kingdoms
        .clone();
    assert!(confirmed.valid());
    assert_eq!(
        confirmed.claim_at(chunk(-20, 0)).unwrap().kind,
        ClaimKind::ProtectedBuilding
    );
    assert_eq!(
        visitor_client
            .world()
            .resource::<client::ClientSession>()
            .kingdoms,
        confirmed
    );
}
