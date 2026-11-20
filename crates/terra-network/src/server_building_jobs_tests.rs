use super::*;
use crate::{protocol::building_jobs::BuildingAction, server::building_jobs};
use terra_core::{
    building_jobs::{
        BuildArea, BuildCell, BuildError, BuildPhase, BuildStatus, BuildStep, BuildingJob,
        HOUSE_TEMPLATE,
    },
    creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds, villagers},
    kingdom::{ClaimActor, ClaimKind, KingdomId},
    npc_jobs::{WorkArea, WorkKind, WorkOrder},
};

fn building_fixture() -> (App, Peer, KingdomId, u64) {
    let (mut app, peer, _) = fixture(ResourcePropKind::SmallStones, false);
    let keep = [12, 4, 12];
    let npc = villagers::id([0, 0], 25).unwrap();
    let kingdom = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.record(keep, terra_core::castle::BERGFRIED).unwrap();
        let kingdom = journal
            .gameplay_mut()
            .kingdoms
            .found(peer.uuid, keep, "Builder authority")
            .unwrap();
        journal.gameplay_mut().creatures.animals.insert(
            npc,
            Creature {
                id: npc,
                kind: CreatureKind::Villager,
                home: [0, 0],
                position: [5.5, 4.0, 6.5],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: 20.0,
                behavior: CreatureBehavior::Rest,
                timer: 0.0,
                age: 0.0,
                variant: villagers::UNEMPLOYED,
                needs: CreatureNeeds::full(CreatureKind::Villager),
            },
        );
        journal
            .gameplay_mut()
            .creatures
            .owners
            .insert(npc, peer.uuid);
        kingdom
    };
    (app, peer, kingdom, npc)
}

fn apply(app: &mut App, peer: &Peer, action: BuildingAction) -> Result<(), BuildError> {
    let mut state = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = state.get_mut(app.world_mut()).unwrap();
    building_jobs::apply(action, peer, &mut world)
}

fn seed_job(app: &mut App, peer: &Peer, kingdom: KingdomId, npc: u64) {
    apply(
        app,
        peer,
        BuildingAction::AssignCraftsman {
            kingdom,
            npc,
            enabled: true,
        },
    )
    .unwrap();
    let mut game = app.world_mut().resource_mut::<EditJournal>();
    let job = BuildingJob {
        id: 1,
        kingdom,
        owner: peer.uuid,
        origin: [10, 4, 10],
        facing: 0,
        template: HOUSE_TEMPLATE.into(),
        free_resources: false,
        free_from_step: 0,
        workers: vec![npc],
        steps: vec![BuildStep {
            phase: BuildPhase::Foundation,
            cells: vec![BuildCell {
                position: [10, 4, 10],
                material: 3,
                facing: 0,
                peer: None,
                secondary: false,
            }],
            costs: vec![ItemStack {
                item: ItemKind::Block(3),
                count: 1,
            }],
        }],
        next_step: 0,
        work_seconds: 0.0,
        status: BuildStatus::Queued,
        enabled: true,
        revision: 7,
    };
    game.gameplay_mut().buildings.next_id = 2;
    game.gameplay_mut().buildings.jobs.insert(1, job);
}

#[test]
fn creative_first_house_without_storage_or_roster_keeps_full_gameplay_persistable() {
    let (mut app, mut peer, kingdom, _) = building_fixture();
    peer.mode = Some(GameMode::Creative);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .player_stats
        .entry(peer.uuid)
        .or_default()
        .game_mode = Some(GameMode::Creative);
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal
            .gameplay_mut()
            .kingdoms
            .claim(
                kingdom,
                [0, 1],
                ClaimKind::ProtectedBuilding,
                ClaimActor::Player(peer.uuid),
            )
            .unwrap();
    }
    let section = terra_voxel::SectionPos { x: 0, y: 0, z: 1 };
    let mut data = terra_voxel::BlockSection::empty(section).unwrap();
    for x in 0..16 {
        for y in 0..=3 {
            for z in 0..16 {
                data.set_block([x, y, z], 3).unwrap();
            }
        }
    }
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        section,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    assert!(before.valid());
    assert!(before.buildings.settlements.is_empty());
    assert!(
        apply(
            &mut app,
            &peer,
            BuildingAction::QueueHouse {
                kingdom,
                origin: [8, 4, 40],
                facing: 0,
                template: "spruce-cottage-v1".into(),
            }
        )
        .is_err()
    );
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
    apply(
        &mut app,
        &peer,
        BuildingAction::QueueHouse {
            kingdom,
            origin: [8, 4, 24],
            facing: 0,
            template: "spruce-cottage-v1".into(),
        },
    )
    .unwrap();
    let game = app.world().resource::<EditJournal>().gameplay();
    assert!(
        game.valid(),
        "a successful command must be immediately saveable"
    );
    let site = &game.buildings.settlements[&kingdom];
    assert_eq!(site.owner, peer.uuid);
    assert_eq!(site.storage, None);
    assert!(site.craftsmen.is_empty());
    let job = &game.buildings.jobs[&1];
    assert_eq!(job.owner, site.owner);
    assert_eq!(job.kingdom, kingdom);
    assert!(job.free_resources);
    assert_eq!(job.free_from_step, 0);
    assert_eq!(game.inventories, before.inventories);
    let encoded = postcard::to_allocvec(game).unwrap();
    let restored: terra_core::GameplayState = postcard::from_bytes(&encoded).unwrap();
    assert_eq!(restored, *game);
    assert!(restored.valid());
}

#[test]
fn forged_template_ids_fail_atomically_and_a_retired_auto_template_can_still_be_disabled() {
    let (mut app, peer, kingdom, _) = building_fixture();
    let area = BuildArea::new([1, 4, 1], [14, 9, 14]).unwrap();
    for template in [
        "../village/residential.json",
        "missing-residential-v1",
        "",
        "House",
    ] {
        let before = app.world().resource::<EditJournal>().gameplay().clone();
        for action in [
            BuildingAction::QueueHouse {
                kingdom,
                origin: [8, 4, 8],
                facing: 0,
                template: template.into(),
            },
            BuildingAction::SetResidential {
                kingdom,
                area: Some(area),
                template: template.into(),
            },
        ] {
            assert_eq!(
                apply(&mut app, &peer, action),
                Err(BuildError::InvalidTemplate)
            );
            assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
        }
    }
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let mut site = terra_core::building_jobs::SettlementBuilding::new(peer.uuid);
        site.residential = Some(area);
        site.selected_template = "retired-residential-v1".into();
        journal
            .gameplay_mut()
            .buildings
            .settlements
            .insert(kingdom, site);
    }
    apply(
        &mut app,
        &peer,
        BuildingAction::SetResidential {
            kingdom,
            area: None,
            template: "retired-residential-v1".into(),
        },
    )
    .unwrap();
    let site = &app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .buildings
        .settlements[&kingdom];
    assert_eq!(site.residential, None);
    assert_eq!(site.selected_template, "retired-residential-v1");
}

#[test]
fn own_keep_can_choose_empty_town_storage_and_enroll_a_craftsman_without_a_work_area_or_material_minting()
 {
    let (mut app, peer, kingdom, npc) = building_fixture();
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    apply(&mut app, &peer, BuildingAction::Inspect { kingdom }).unwrap();
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
    let area = BuildArea::new([2, 4, 2], [4, 5, 4]).unwrap();
    apply(
        &mut app,
        &peer,
        BuildingAction::SetStorage { kingdom, area },
    )
    .unwrap();
    let sex = before.creatures.animals[&npc].sex();
    for _ in 0..2 {
        apply(
            &mut app,
            &peer,
            BuildingAction::AssignCraftsman {
                kingdom,
                npc,
                enabled: true,
            },
        )
        .unwrap();
    }
    let journal = app.world().resource::<EditJournal>();
    let game = journal.gameplay();
    assert_eq!(game.buildings.settlements[&kingdom].storage, Some(area));
    assert_eq!(game.buildings.settlements[&kingdom].craftsmen, [npc]);
    assert_eq!(game.creatures.animals[&npc].variant, villagers::CRAFTSMAN);
    assert_eq!(game.creatures.animals[&npc].sex(), sex);
    assert!(
        game.jobs.orders.is_empty(),
        "craftsmanship does not invent a voxel work area"
    );
    assert_eq!(game.inventories, before.inventories);
    assert_eq!(game.buildings.inventory(npc).unwrap().occupied_slots(), 0);
    assert_eq!(
        journal.block(area.min),
        None,
        "choosing storage never places a free chest"
    );
    assert!(game.valid());
}

#[test]
fn construction_storage_rejects_territory_foreign_and_unclaimed_chunks_without_partial_mutation() {
    let (mut app, peer, kingdom, _) = building_fixture();
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal
            .gameplay_mut()
            .kingdoms
            .claim(
                kingdom,
                [1, 0],
                ClaimKind::RealmTerritory,
                ClaimActor::Player(peer.uuid),
            )
            .unwrap();
        journal
            .gameplay_mut()
            .kingdoms
            .found(PlayerUuid::from_u128(9900), [40, 4, 8], "Foreign builder")
            .unwrap();
    }
    for area in [
        BuildArea::new([16, 4, 1], [18, 5, 3]).unwrap(),
        BuildArea::new([32, 4, 1], [34, 5, 3]).unwrap(),
        BuildArea::new([-16, 4, 1], [-14, 5, 3]).unwrap(),
        BuildArea::new([14, 4, 1], [17, 5, 3]).unwrap(),
    ] {
        let before = app.world().resource::<EditJournal>().gameplay().clone();
        assert_eq!(
            apply(
                &mut app,
                &peer,
                BuildingAction::SetStorage { kingdom, area }
            ),
            Err(BuildError::Protected)
        );
        assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
    }
}

#[test]
fn craftsman_assignment_rechecks_owner_life_and_existing_productive_job_before_any_mutation() {
    let (mut app, peer, kingdom, npc) = building_fixture();
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .creatures
        .owners
        .insert(npc, PlayerUuid::from_u128(9911));
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    assert_eq!(
        apply(
            &mut app,
            &peer,
            BuildingAction::AssignCraftsman {
                kingdom,
                npc,
                enabled: true
            }
        ),
        Err(BuildError::Protected)
    );
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let game = journal.gameplay_mut();
        game.creatures.owners.insert(npc, peer.uuid);
        game.jobs.next_id = 2;
        game.jobs.orders.insert(
            1,
            WorkOrder {
                id: 1,
                npc,
                owner: peer.uuid,
                kind: WorkKind::Lumberjack,
                area: WorkArea::new([2, 4, 2], [6, 5, 6]).unwrap(),
                storage: [8, 4, 8],
                priority: 5,
                enabled: true,
                revision: 1,
            },
        );
        game.jobs.workers.insert(npc, Default::default());
    }
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    assert_eq!(
        apply(
            &mut app,
            &peer,
            BuildingAction::AssignCraftsman {
                kingdom,
                npc,
                enabled: true
            }
        ),
        Err(BuildError::Busy)
    );
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.gameplay_mut().jobs.orders.clear();
        journal.gameplay_mut().jobs.workers.clear();
        journal
            .gameplay_mut()
            .creatures
            .animals
            .get_mut(&npc)
            .unwrap()
            .health = 0.0;
        journal
            .gameplay_mut()
            .creatures
            .animals
            .get_mut(&npc)
            .unwrap()
            .behavior = CreatureBehavior::Dead;
    }
    assert_eq!(
        apply(
            &mut app,
            &peer,
            BuildingAction::AssignCraftsman {
                kingdom,
                npc,
                enabled: true
            }
        ),
        Err(BuildError::MissingWorker)
    );
}

#[test]
fn stale_pause_and_cancel_never_change_house_or_carried_resources_and_live_cancel_releases_only_assignment()
 {
    let (mut app, peer, kingdom, npc) = building_fixture();
    seed_job(&mut app, &peer, kingdom, npc);
    let mut bag = Inventory::default();
    bag.insert(ItemKind::Block(3), 11);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .buildings
        .inventories
        .insert(npc, bag.clone());
    for action in [
        BuildingAction::SetEnabled {
            id: 1,
            revision: 6,
            enabled: false,
        },
        BuildingAction::Cancel { id: 1, revision: 6 },
        BuildingAction::AssignCraftsman {
            kingdom,
            npc,
            enabled: false,
        },
    ] {
        let before = app.world().resource::<EditJournal>().gameplay().clone();
        let expected = if matches!(action, BuildingAction::AssignCraftsman { .. }) {
            BuildError::Busy
        } else {
            BuildError::Stale
        };
        assert_eq!(apply(&mut app, &peer, action), Err(expected));
        assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
    }
    apply(
        &mut app,
        &peer,
        BuildingAction::SetEnabled {
            id: 1,
            revision: 7,
            enabled: false,
        },
    )
    .unwrap();
    let game = app.world().resource::<EditJournal>().gameplay();
    assert_eq!(game.buildings.jobs[&1].status, BuildStatus::Paused);
    assert_eq!(game.buildings.jobs[&1].revision, 8);
    assert_eq!(game.buildings.inventory(npc), Some(&bag));
    apply(
        &mut app,
        &peer,
        BuildingAction::Cancel { id: 1, revision: 8 },
    )
    .unwrap();
    let game = app.world().resource::<EditJournal>().gameplay();
    assert_eq!(game.buildings.jobs[&1].status, BuildStatus::Cancelled);
    assert!(game.buildings.jobs[&1].workers.is_empty());
    assert_eq!(game.buildings.inventory(npc), Some(&bag));
    assert_eq!(
        apply(
            &mut app,
            &peer,
            BuildingAction::AssignCraftsman {
                kingdom,
                npc,
                enabled: false
            }
        ),
        Err(BuildError::Busy)
    );
}

#[test]
fn a_craftsman_with_carried_materials_cannot_be_unenrolled_and_empty_unenroll_restores_civilian_role()
 {
    let (mut app, peer, kingdom, npc) = building_fixture();
    apply(
        &mut app,
        &peer,
        BuildingAction::AssignCraftsman {
            kingdom,
            npc,
            enabled: true,
        },
    )
    .unwrap();
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .buildings
        .inventories
        .get_mut(&npc)
        .unwrap()
        .insert(ItemKind::Block(6), 9);
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    assert_eq!(
        apply(
            &mut app,
            &peer,
            BuildingAction::AssignCraftsman {
                kingdom,
                npc,
                enabled: false
            }
        ),
        Err(BuildError::Busy)
    );
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
    *app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .buildings
        .inventories
        .get_mut(&npc)
        .unwrap() = Inventory::default();
    apply(
        &mut app,
        &peer,
        BuildingAction::AssignCraftsman {
            kingdom,
            npc,
            enabled: false,
        },
    )
    .unwrap();
    let game = app.world().resource::<EditJournal>().gameplay();
    assert!(
        !game.buildings.settlements[&kingdom]
            .craftsmen
            .contains(&npc)
    );
    assert!(game.buildings.inventory(npc).is_none());
    assert_eq!(game.creatures.animals[&npc].variant, villagers::UNEMPLOYED);
}

#[test]
fn building_snapshots_revoke_remote_access_after_leaving_own_territory_or_losing_authority() {
    let (mut app, mut peer, kingdom, npc) = building_fixture();
    seed_job(&mut app, &peer, kingdom, npc);
    let position = app
        .world()
        .get::<Transform>(peer.body)
        .unwrap()
        .translation
        .to_array()
        .map(f64::from);
    let config = app.world().resource::<GameConfig>();
    let game = app.world().resource::<EditJournal>().gameplay();
    let snapshot = building_jobs::snapshot(&peer, position, game, Some(kingdom), config);
    assert_eq!(snapshot.settlements[&kingdom].craftsmen, [npc]);
    assert!(snapshot.jobs.contains_key(&1) && snapshot.valid());
    assert!(
        building_jobs::snapshot(&peer, [512.0, 4.0, 512.0], game, Some(kingdom), config)
            .jobs
            .is_empty()
    );
    peer.permissions.build = false;
    assert!(
        building_jobs::snapshot(&peer, position, game, Some(kingdom), config)
            .settlements
            .is_empty()
    );
    peer.permissions.build = true;
    peer.uuid = PlayerUuid::from_u128(9922);
    assert!(
        building_jobs::snapshot(&peer, position, game, Some(kingdom), config)
            .jobs
            .is_empty()
    );
}

#[test]
fn a_removed_member_with_allied_npc_work_rights_cannot_keep_the_old_construction_subscription() {
    use terra_core::kingdom::{
        DiplomaticRelation, DiplomaticTarget, KingdomPermission, KingdomRole,
    };
    let (mut app, mut peer, kingdom, npc) = building_fixture();
    seed_job(&mut app, &peer, kingdom, npc);
    let founder = peer.uuid;
    let member = PlayerUuid::from_u128(9955);
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let registry = &mut journal.gameplay_mut().kingdoms;
        registry.join(member, kingdom).unwrap();
        registry
            .set_role(founder, kingdom, member, KingdomRole::Worker)
            .unwrap();
    }
    peer.uuid = member;
    let position = [11.5, 4.0, 12.5];
    {
        let game = app.world().resource::<EditJournal>().gameplay();
        let snapshot = building_jobs::snapshot(
            &peer,
            position,
            game,
            Some(kingdom),
            app.world().resource::<GameConfig>(),
        );
        assert!(snapshot.jobs.contains_key(&1));
    }
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let registry = &mut journal.gameplay_mut().kingdoms;
        registry.leave(member, kingdom).unwrap();
        let allied = registry
            .found(member, [40, 4, 8], "Former member ally")
            .unwrap();
        registry
            .set_relation(
                founder,
                kingdom,
                DiplomaticTarget::Player(allied),
                DiplomaticRelation::Allied,
            )
            .unwrap();
        registry
            .set_relation(
                member,
                allied,
                DiplomaticTarget::Player(kingdom),
                DiplomaticRelation::Allied,
            )
            .unwrap();
        assert!(
            registry.can(member, KingdomPermission::NpcWork, [12, 4, 12]),
            "the former shortcut really grants diplomacy access"
        );
    }
    let game = app.world().resource::<EditJournal>().gameplay();
    let snapshot = building_jobs::snapshot(
        &peer,
        position,
        game,
        Some(kingdom),
        app.world().resource::<GameConfig>(),
    );
    assert!(snapshot.settlements.is_empty() && snapshot.jobs.is_empty());
}

#[test]
fn spectator_or_unsynced_peers_lose_building_snapshots_including_the_server_mode_fallback() {
    let (mut app, mut peer, kingdom, npc) = building_fixture();
    seed_job(&mut app, &peer, kingdom, npc);
    let position = [11.5, 4.0, 12.5];
    let game = app.world().resource::<EditJournal>().gameplay();
    let mut config = app.world().resource::<GameConfig>().clone();
    peer.mode = Some(GameMode::Spectator);
    assert!(
        building_jobs::snapshot(&peer, position, game, Some(kingdom), &config)
            .jobs
            .is_empty()
    );
    peer.mode = None;
    config.server.game_mode = GameMode::Spectator;
    assert!(
        building_jobs::snapshot(&peer, position, game, Some(kingdom), &config)
            .settlements
            .is_empty()
    );
    config.server.game_mode = GameMode::Survival;
    peer.synced = false;
    assert!(
        building_jobs::snapshot(&peer, position, game, Some(kingdom), &config)
            .jobs
            .is_empty()
    );
}

#[test]
fn free_construction_policy_uses_only_server_permissions_and_authoritative_saved_modes() {
    let owner = terra_core::PlayerUuid::from_u128(241);
    let mut game = terra_core::GameplayState::default();
    let mut config = terra_core::GameConfig::default();
    config.server.game_mode = terra_core::GameMode::Survival;
    assert!(!building_jobs::creative_owner(owner, &game, &config));
    game.player_stats.entry(owner).or_default().game_mode = Some(terra_core::GameMode::Creative);
    assert_eq!(
        building_jobs::creative_owner(owner, &game, &config),
        config.server.permissions_for(owner).build
    );
    game.player_stats.get_mut(&owner).unwrap().game_mode = Some(terra_core::GameMode::Survival);
    config.server.game_mode = terra_core::GameMode::Creative;
    assert!(
        !building_jobs::creative_owner(owner, &game, &config),
        "saved Survival overrides a Creative server default"
    );
}
