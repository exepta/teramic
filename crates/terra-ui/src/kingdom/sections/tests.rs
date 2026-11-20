use super::*;

#[test]
fn an_unbound_keep_does_not_invent_a_population_or_an_army() {
    let world = World::new();
    let session = ClientSession::default();
    let view = project(&world, &session, None);
    assert!(view.realm_stats.is_empty());
    assert!(view.worker_stats.is_empty());
    assert!(view.army.is_empty());
}

fn session() -> (ClientSession, KingdomId, PlayerUuid, KingdomId, PlayerUuid) {
    let mut session = ClientSession::default();
    let owner = PlayerUuid::from_u128(42);
    let other = PlayerUuid::from_u128(43);
    session.uuid = Some(owner);
    let own = session
        .kingdoms
        .found(owner, [0, 0, 0], "Own realm")
        .unwrap();
    let foreign = session
        .kingdoms
        .found(other, [512, 0, 512], "Foreign realm")
        .unwrap();
    (session, own, owner, foreign, other)
}

#[test]
fn realm_counts_only_include_active_settlements_in_the_displayed_realm() {
    let (mut session, own, _, _, _) = session();
    for (id, kind, parent, state) in [
        (
            100,
            SettlementKind::Village,
            Some(own),
            KingdomState::Active,
        ),
        (101, SettlementKind::City, Some(own), KingdomState::Active),
        (
            102,
            SettlementKind::Outpost,
            Some(own),
            KingdomState::Dissolved,
        ),
        (103, SettlementKind::Village, None, KingdomState::Active),
    ] {
        let mut child = session.kingdoms.get(own).unwrap().clone();
        child.id = KingdomId(id);
        child.kind = kind;
        child.parent = parent;
        child.state = state;
        session.kingdoms.kingdoms.push(child);
    }
    let view = project(&World::new(), &session, session.kingdoms.get(own));
    let values: Vec<_> = view
        .realm_stats
        .iter()
        .map(|stat| (stat.key.as_str(), stat.value.as_str()))
        .collect();
    assert_eq!(
        values,
        [("villages", "1"), ("cities", "1"), ("outposts", "0")]
    );
}

#[test]
fn opening_a_foreign_keep_does_not_relabel_the_previous_own_worker_roster() {
    let (mut session, own, _, foreign, _) = session();
    session
        .work
        .workers
        .push(terra_network::protocol::npc_work::WorkerView {
            npc: 7,
            name: "Own worker".into(),
            position: [0.0; 3],
            order: None,
            status: WorkStatus::Working,
            carried_slots: 1,
        });
    let own_view = project(&World::new(), &session, session.kingdoms.get(own));
    assert!(own_view.own_realm);
    assert_eq!(own_view.worker_count, "1");
    let foreign_view = project(&World::new(), &session, session.kingdoms.get(foreign));
    assert!(!foreign_view.own_realm);
    assert!(foreign_view.worker_stats.is_empty());
    assert!(foreign_view.professions.is_empty());
    assert!(foreign_view.army.is_empty());
}

#[test]
fn army_counts_only_loaded_living_soldiers_with_a_real_owner_in_the_own_realm() {
    use terra_core::creatures::{
        Creature, CreatureKind, CreatureNeeds,
        taming::{CompanionMode, CompanionView},
    };
    use terra_network::client::CreatureTrack;
    let (session, own, owner, _, foreign_owner) = session();
    let mut creatures = ReplicatedCreatures::default();
    for (id, role, health, bond) in [
        (1, 10, 80.0, Some(owner)),
        (2, 11, 75.0, Some(owner)),
        (3, 12, 70.0, Some(owner)),
        (4, 10, 80.0, Some(foreign_owner)),
        (5, 10, 80.0, None),
        (6, 10, 0.0, Some(owner)),
        (
            7,
            terra_core::creatures::villagers::UNEMPLOYED,
            80.0,
            Some(owner),
        ),
    ] {
        let current = Creature {
            id,
            kind: CreatureKind::Villager,
            home: [0, 0],
            position: [1.0, 2.0, 1.0],
            velocity: [1.0, 0.0, 0.0],
            yaw: 0.0,
            health,
            behavior: CreatureBehavior::Wander,
            timer: 0.0,
            age: 0.0,
            variant: role,
            needs: CreatureNeeds::default(),
        };
        creatures.creatures.insert(
            id,
            CreatureTrack {
                playback: Default::default(),
                previous: current.clone(),
                current,
                received: std::time::Instant::now(),
                interval: 0.1,
            },
        );
        if let Some(owner) = bond {
            creatures.bonds.insert(
                id,
                CompanionView {
                    id,
                    owner: Some(owner),
                    player: owner,
                    trust: 100,
                    mode: CompanionMode::Wait,
                },
            );
        }
    }
    let mut world = World::new();
    world.insert_resource(creatures);
    let view = project(&world, &session, session.kingdoms.get(own));
    assert_eq!(
        view.army
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        ["1", "2", "3"]
    );
    assert_eq!(
        view.army_stats
            .iter()
            .map(|stat| stat.value.as_str())
            .collect::<Vec<_>>(),
        ["1", "1", "1", "3"]
    );
    assert!(
        view.army
            .iter()
            .all(|row| row.status == "Im Baugebiet unterwegs")
    );
    assert!(
        view.army
            .iter()
            .all(|row| tilt_ui::tilt_ui_icons::parse_source(&row.icon).is_some())
    );
}

#[test]
fn every_projected_stat_uses_a_real_catalog_source_at_a_supported_resolution() {
    let (session, own, _, _, _) = session();
    let view = project(&World::new(), &session, session.kingdoms.get(own));
    for stat in view
        .realm_stats
        .iter()
        .chain(&view.worker_stats)
        .chain(&view.professions)
        .chain(&view.army_stats)
    {
        let (_, size) = tilt_ui::tilt_ui_icons::parse_source(&stat.icon)
            .unwrap_or_else(|| panic!("Invalid real icon source for {}: {}", stat.key, stat.icon));
        assert_eq!(size, tilt_ui::IconSize::Px32);
    }
}

#[test]
fn section_templates_use_real_static_icon_names_and_bind_dynamic_sources() {
    for html in [
        include_str!("../../../../../src-ui/components/kingdom-menu/kingdom-menu.component.html"),
        include_str!(
            "../../../../../src-ui/components/kingdom-general/kingdom-general.component.html"
        ),
        include_str!(
            "../../../../../src-ui/components/kingdom-members/kingdom-members.component.html"
        ),
        include_str!(
            "../../../../../src-ui/components/kingdom-workers/kingdom-workers.component.html"
        ),
        include_str!("../../../../../src-ui/components/kingdom-army/kingdom-army.component.html"),
    ] {
        assert!(
            !html.contains("[name]="),
            "Dynamic icon names are not materialized by TiltUI"
        );
        for icon in html.split("<icon ").skip(1) {
            let attributes = icon.split('>').next().unwrap();
            let name = attributes
                .split("name=\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .unwrap();
            let size = attributes
                .split("size=\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .unwrap();
            assert!(
                tilt_ui::tilt_ui_icons::parse_source(&format!("tilt-icon:{name}@{size}")).is_some(),
                "Unknown icon or unsupported source resolution: {name}@{size}"
            );
        }
    }
}
