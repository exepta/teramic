use super::*;
use terra_core::{
    GameplayState,
    creatures::{
        CreatureBehavior, CreatureKind, CreatureNeeds, bee_id, breeding, creature_id,
        ecology::{BeeState, NestState},
    },
};

fn animal(id: u64, kind: CreatureKind) -> Creature {
    Creature {
        id,
        kind,
        home: [0, 0],
        position: [3.0, 2.0, 3.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: kind.health(),
        behavior: CreatureBehavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(kind),
    }
}

fn colony() -> (GameplayState, u64) {
    let mut state = GameplayState::default();
    state.creatures.explored.insert([0, 0]);
    let parents = [bee_id([0, 0], 0).unwrap(), bee_id([0, 0], 1).unwrap()];
    for id in parents {
        state
            .creatures
            .animals
            .insert(id, animal(id, CreatureKind::Bee));
    }
    let child = breeding::next_birth_id(&state.creatures, 0).unwrap();
    state
        .creatures
        .animals
        .insert(child, animal(child, CreatureKind::Bee));
    let cow = creature_id([0, 0], 0).unwrap();
    state
        .creatures
        .animals
        .insert(cow, animal(cow, CreatureKind::Cow));
    state.animal_families.insert(
        child,
        breeding::AnimalFamily {
            parents: Some(parents),
            ..Default::default()
        },
    );
    state.next_animal_birth = breeding::birth_serial(child).unwrap();
    state.creatures.ecology.bees.insert(
        child,
        BeeState {
            nest: 7,
            ..Default::default()
        },
    );
    state.creatures.ecology.nests.insert(
        7,
        NestState {
            position: [3.0, 2.0, 3.0],
            nectar: 200,
            honey: 0,
            progress: 0.0,
        },
    );
    assert!(state.valid());
    (state, child)
}

fn packet(state: &GameplayState, tick: u64) -> CreatureStates {
    CreatureStates {
        tick,
        creatures: state.creatures.animals.values().cloned().collect(),
        ecology: state.creatures.ecology.clone(),
        intentions: Vec::new(),
        bonds: Vec::new(),
        families: state
            .animal_families
            .iter()
            .map(|(id, family)| breeding::FamilyView::new(&state.creatures.animals[id], *family))
            .collect(),
        boss: None,
        stalker_grip: None,
    }
}

#[test]
fn legitimate_wild_bee_offspring_does_not_invalidate_other_creatures_in_full_snapshots() {
    let (mut state, child) = colony();
    let cow = creature_id([0, 0], 0).unwrap();
    for tick in 1..=128 {
        state.creatures.animals.get_mut(&cow).unwrap().position[0] += 0.01;
        state.creatures.animals.get_mut(&child).unwrap().age += 0.1;
        let snapshot = packet(&state, tick);
        let ids = snapshot_ids(&snapshot).unwrap();
        assert!(ids.contains(&cow) && ids.contains(&child));
        assert_eq!(ids.len(), 4);
        assert!(snapshot.bonds.is_empty());
        assert!(state.valid());
    }
}

#[test]
fn unowned_non_bee_offspring_and_bees_without_real_colony_metadata_still_fail_closed() {
    let (mut state, child) = colony();
    let mut snapshot = packet(&state, 1);
    snapshot.ecology.bees.remove(&child);
    assert!(snapshot_ids(&snapshot).is_err());
    snapshot = packet(&state, 2);
    snapshot.families.clear();
    assert!(snapshot_ids(&snapshot).is_err());
    snapshot = packet(&state, 3);
    snapshot.ecology.bees.get_mut(&child).unwrap().nest = 0;
    assert!(snapshot_ids(&snapshot).is_err());
    state
        .creatures
        .animals
        .insert(child, animal(child, CreatureKind::Cow));
    snapshot = packet(&state, 4);
    assert!(snapshot_ids(&snapshot).is_err());
    let owner = terra_core::PlayerUuid::from_u128(159);
    snapshot
        .bonds
        .push(terra_core::creatures::taming::CompanionView {
            id: child,
            owner: Some(owner),
            player: owner,
            trust: 100,
            mode: terra_core::creatures::taming::CompanionMode::Wait,
        });
    assert!(snapshot_ids(&snapshot).is_ok());
}

#[test]
fn a_destroyed_nest_does_not_turn_its_still_valid_wild_offspring_into_an_invalid_snapshot() {
    let (mut state, child) = colony();
    state.creatures.ecology.nests.clear();
    assert!(state.valid());
    let snapshot = packet(&state, 1);
    assert!(snapshot.ecology.bees.contains_key(&child));
    assert!(snapshot_ids(&snapshot).is_ok());
}

#[test]
fn invalid_snapshot_diagnostics_are_bounded_and_warn_at_most_once_per_five_seconds() {
    let mut creatures = ReplicatedCreatures::default();
    let start = Instant::now();
    for tick in 1..=500 {
        creatures.reject_snapshot(
            tick,
            "invalid creature state",
            start + Duration::from_millis(tick),
        );
    }
    assert_eq!(creatures.rejected_snapshots, 500);
    assert_eq!(creatures.last_rejected_tick, 500);
    assert_eq!(
        creatures.last_snapshot_error,
        Some("invalid creature state")
    );
    assert_eq!(creatures.warning_at, Some(start + Duration::from_millis(1)));
    let next = start + Duration::from_secs(6);
    creatures.reject_snapshot(501, "invalid ecology state", next);
    assert_eq!(creatures.warning_at, Some(next));
    assert_eq!(creatures.last_snapshot_error, Some("invalid ecology state"));
    creatures.rejected_snapshots = u64::MAX;
    creatures.reject_snapshot(502, "invalid creature state", next);
    assert_eq!(creatures.rejected_snapshots, u64::MAX);
}

#[test]
fn replicated_boss_must_be_a_present_living_dragon() {
    let (mut state, _) = colony();
    let id = creature_id([0, 0], 0).unwrap();
    let mut snapshot = packet(&state, 1);
    snapshot.boss = Some(id);
    assert!(snapshot_ids(&snapshot).is_err());
    state
        .creatures
        .animals
        .insert(id, animal(id, CreatureKind::DarkDragon));
    let mut snapshot = packet(&state, 2);
    snapshot.boss = Some(id);
    assert!(snapshot_ids(&snapshot).is_ok());
    snapshot.boss = Some(u64::MAX);
    assert!(snapshot_ids(&snapshot).is_err());
    snapshot.boss = Some(id);
    snapshot
        .creatures
        .iter_mut()
        .find(|a| a.id == id)
        .unwrap()
        .health = 0.0;
    assert!(snapshot_ids(&snapshot).is_err());
}

#[test]
fn repeated_full_snapshots_leave_real_strides_and_attack_clocks_intact_for_every_species() {
    let start = Instant::now();
    for kind in CreatureKind::ALL {
        let mut actor = animal(1, kind);
        actor.behavior = CreatureBehavior::Wander;
        actor.velocity[0] = 2.0;
        let mut track = CreatureTrack {
            playback: Default::default(),
            current: actor.clone(),
            previous: actor.clone(),
            received: start,
            interval: 0.1,
        };
        let mut reference = track.clone();
        for tick in 0..=10 {
            let now = start + Duration::from_millis(tick * 100);
            let stamp = tick as f64 * 0.1;
            if tick % 2 == 0 {
                actor.age = stamp;
                actor.position[0] = 3.0 + stamp * 2.0;
                reference.apply(actor.clone(), stamp, now);
            } else {
                actor.health -= 0.1;
            }
            track.apply(actor.clone(), stamp, now);
            assert_eq!(track.current.health, actor.health);
            assert_eq!(track.received, reference.received, "{kind:?}");
            let between = now + Duration::from_millis(50);
            assert_eq!(
                track.position_at(between),
                reference.position_at(between),
                "{kind:?}"
            );
        }
        // A real same-age transition (e.g. an interaction between AI ticks)
        // must still reach the renderer immediately.
        actor.behavior = CreatureBehavior::Attack;
        let now = start + Duration::from_millis(1100);
        track.apply(actor, 1.1, now);
        assert_eq!(track.received, now);
        assert_eq!(track.current.behavior, CreatureBehavior::Attack);
    }
}

#[test]
fn stone_guardian_and_other_walkers_ease_each_stair_without_changing_authority() {
    use std::time::Duration;
    for kind in [
        CreatureKind::Cow,
        CreatureKind::Villager,
        CreatureKind::ThornBrute,
        CreatureKind::StoneGolem,
        CreatureKind::FireDragon,
        CreatureKind::LargeSpider,
    ] {
        let mut a = animal(creature_id([0, 0], 0).unwrap(), kind);
        a.behavior = if kind.is_spider() {
            CreatureBehavior::Crawl
        } else {
            CreatureBehavior::Hunt
        };
        a.velocity = [2., 0., 0.];
        assert!(ground_stride(&a));
        for dy in [-1., 1.] {
            let mut track = CreatureTrack {
                current: a.clone(),
                previous: a.clone(),
                received: Instant::now(),
                interval: 0.1,
                playback: Default::default(),
            };
            let start = track.received;
            track.apply(a.clone(), 0., start);
            let mut b = a.clone();
            b.position[0] += 0.2;
            b.position[1] += dy;
            track.apply(b.clone(), 0.1, start + Duration::from_millis(100));
            let mut last = a.position[1] as f32;
            let mut intermediate = 0;
            for i in 0..=80 {
                let y = track.position_at(start + Duration::from_millis(5 * i)).y;
                assert!((y - last).abs() < 0.15, "{kind:?}: {last} to {y}");
                if (y - a.position[1] as f32).abs() > 0.03
                    && (y - b.position[1] as f32).abs() > 0.03
                {
                    intermediate += 1;
                }
                last = y;
            }
            assert!(intermediate >= 10);
            assert!((last - b.position[1] as f32).abs() < 0.001);
            assert_eq!(track.current, b);
        }
    }
}
