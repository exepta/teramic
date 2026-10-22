use super::*;
use terra_core::creatures::*;
use terra_world::edits::EditJournal;

#[test]
fn two_udp_clients_receive_every_new_aquatic_species_without_changing_stable_ids() {
    let (mut host, mut a, mut b) = pair();
    let p = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position
        .map(f64::from);
    let home = [(p[0] as i64).div_euclid(32), (p[2] as i64).div_euclid(32)];
    let mut expected = Vec::new();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        for (index, kind) in CreatureKind::ALL
            .into_iter()
            .filter(|kind| kind.is_aquatic() && *kind != CreatureKind::Fish)
            .enumerate()
        {
            let home = [home[0] + index as i64, home[1]];
            let id = creature_id(home, 0).unwrap();
            expected.push((id, kind));
            journal.gameplay_mut().creatures.explored.insert(home);
            journal.gameplay_mut().creatures.animals.insert(
                id,
                Creature {
                    id,
                    kind,
                    home,
                    position: [p[0] + 3.0 + index as f64 * 0.5, p[1], p[2] - 5.0],
                    velocity: [0.0; 3],
                    yaw: 0.0,
                    health: kind.health(),
                    behavior: CreatureBehavior::Swim,
                    timer: 2.0,
                    age: 0.0,
                    variant: 0,
                    needs: CreatureNeeds::full(kind),
                },
            );
        }
    }
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            let population = client.world().resource::<client::ReplicatedCreatures>();
            expected.iter().all(|(id, kind)| {
                population.creatures.get(id).is_some_and(|track| {
                    track.current.kind == *kind && track.current.id == *id && track.current.valid()
                })
            })
        })
    });
}

#[test]
fn two_udp_clients_share_creatures_damage_drops_and_late_join_tombstones() {
    let (mut host, mut a, mut b) = pair();
    let p = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let home = [(p[0] as i64).div_euclid(32), (p[2] as i64).div_euclid(32)];
    let id = creature_id(home, 0).unwrap();
    let other_id = creature_id(home, 1).unwrap();
    let animal = Creature {
        id,
        kind: CreatureKind::Cow,
        home,
        position: [
            f64::from(p[0]),
            f64::from(p[1]) - f64::from(terra_physics::character::PLAYER_HALF_HEIGHT),
            f64::from(p[2]) - 3.5,
        ],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: 2.0,
        behavior: CreatureBehavior::Rest,
        timer: 60.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::Cow),
    };
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        journal.gameplay_mut().creatures.explored.insert(home);
        journal
            .gameplay_mut()
            .creatures
            .animals
            .insert(id, animal.clone());
        let mut other = animal.clone();
        other.id = other_id;
        other.position[0] += 5.0;
        journal
            .gameplay_mut()
            .creatures
            .animals
            .insert(other_id, other);
    }
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|c| {
            c.world()
                .resource::<client::ReplicatedCreatures>()
                .creatures
                .contains_key(&id)
        })
    });
    // A remote identity cannot be hit by simply submitting its ID.
    a.world_mut().resource_mut::<client::ClientSession>().look = [std::f32::consts::PI, 0.0];
    a.world_mut()
        .resource_mut::<client::CreatureActions>()
        .0
        .push_back((id, false));
    for _ in 0..30 {
        host.update();
        a.update();
        b.update();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .animals[&id]
            .health,
        2.0
    );
    std::thread::sleep(Duration::from_millis(500));
    a.world_mut().resource_mut::<client::ClientSession>().look = [0.0, -0.1];
    a.world_mut()
        .resource_mut::<client::CreatureActions>()
        .0
        .push_back((id, false));
    wait(&mut host, &mut [&mut a, &mut b], |h, clients| {
        h.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .animals[&id]
            .health
            == 1.0
            && clients.iter().all(|c| {
                c.world()
                    .resource::<client::ReplicatedCreatures>()
                    .creatures
                    .get(&id)
                    .is_some_and(|t| t.current.health == 1.0)
            })
    });
    std::thread::sleep(Duration::from_millis(500));
    a.world_mut()
        .resource_mut::<client::CreatureActions>()
        .0
        .push_back((id, false));
    wait(&mut host, &mut [&mut a, &mut b], |h, clients| {
        h.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .animals[&id]
            .health
            == 0.0
            && clients.iter().all(|c| {
                !c.world()
                    .resource::<client::ReplicatedCreatures>()
                    .creatures
                    .contains_key(&id)
            })
    });
    assert!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .iter()
            .any(|d| d.stack.item == terra_core::ItemKind::Animal(AnimalProduct::Beef))
    );
    let token = host
        .world()
        .resource::<server::ServerRuntime>()
        .issue_token(203)
        .unwrap();
    let mut late = client(token);
    wait(&mut host, &mut [&mut a, &mut b, &mut late], |_, clients| {
        clients[2]
            .world()
            .resource::<client::ReplicatedCreatures>()
            .creatures
            .contains_key(&other_id)
    });
    assert!(
        !late
            .world()
            .resource::<client::ReplicatedCreatures>()
            .creatures
            .contains_key(&id)
    );
}

#[test]
fn full_creature_interest_budget_and_needs_reach_two_real_udp_clients() {
    let (mut host, mut a, mut b) = pair();
    let p = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let origin = [(p[0] as i64).div_euclid(32), (p[2] as i64).div_euclid(32)];
    let mut ids = std::collections::BTreeSet::new();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let population = &mut journal.gameplay_mut().creatures;
        *population = Default::default();
        for dx in -2..=2 {
            for dz in -2..=2 {
                population.explored.insert([origin[0] + dx, origin[1] + dz]);
            }
        }
        for i in 0..MAX_VISIBLE_CREATURES {
            let home = [origin[0] + i as i64, origin[1]];
            let id = creature_id(home, 0).unwrap();
            ids.insert(id);
            population.explored.insert(home);
            population.animals.insert(
                id,
                Creature {
                    id,
                    home,
                    kind: CreatureKind::Cow,
                    position: [
                        f64::from(p[0]) + (i % 16) as f64 * 1.25 - 10.0,
                        f64::from(p[1]) - f64::from(terra_physics::character::PLAYER_HALF_HEIGHT),
                        f64::from(p[2]) - (i / 16) as f64 * 1.25 - 7.0,
                    ],
                    velocity: [0.0; 3],
                    yaw: 0.0,
                    health: 10.0,
                    behavior: CreatureBehavior::Rest,
                    timer: 60.0,
                    age: 0.0,
                    variant: 0,
                    needs: CreatureNeeds {
                        food: 15.0,
                        water: 12.0,
                        search_in: 15.0,
                        goal: None,
                    },
                },
            );
        }
        population.ecology.take_nectar(59, p.map(f64::from), 40);
        population.ecology.nests.insert(
            60,
            terra_core::creatures::ecology::NestState {
                position: p.map(f64::from),
                nectar: 150,
                honey: 2,
                progress: 0.1,
            },
        );
        assert!(population.valid());
    }
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            let creatures = &client
                .world()
                .resource::<client::ReplicatedCreatures>()
                .creatures;
            let ecology = &client
                .world()
                .resource::<client::ReplicatedCreatures>()
                .ecology;
            ecology.nectar(59) == 60
                && ecology.nests.get(&60).is_some_and(|n| n.honey == 2)
                && creatures
                    .keys()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
                    == ids
        })
    });
    for client in [&a, &b] {
        let creatures = &client
            .world()
            .resource::<client::ReplicatedCreatures>()
            .creatures;
        assert_eq!(creatures.len(), 128);
        assert!(
            creatures
                .values()
                .all(|track| track.current.needs.food > 14.0
                    && track.current.needs.water > 11.0
                    && track.current.valid())
        );
    }
}

#[test]
fn resource_search_budget_eventually_serves_all_256_animals_with_two_clients() {
    let (mut host, mut a, mut b) = pair();
    let p = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let origin = [(p[0] as i64).div_euclid(32), (p[2] as i64).div_euclid(32)];
    let mut ids = std::collections::BTreeSet::new();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let population = &mut journal.gameplay_mut().creatures;
        *population = Default::default();
        for dx in -2..=2 {
            for dz in -2..=2 {
                population.explored.insert([origin[0] + dx, origin[1] + dz]);
            }
        }
        for i in 0..256 {
            let home = [origin[0] + i as i64, origin[1]];
            let id = creature_id(home, 0).unwrap();
            ids.insert(id);
            population.explored.insert(home);
            population.animals.insert(
                id,
                Creature {
                    id,
                    home,
                    kind: CreatureKind::Cow,
                    position: [
                        f64::from(p[0]) + (i % 16) as f64 - 8.0,
                        f64::from(p[1]) - f64::from(terra_physics::character::PLAYER_HALF_HEIGHT),
                        f64::from(p[2]) - (i / 16) as f64 - 7.0,
                    ],
                    velocity: [0.0; 3],
                    yaw: 0.0,
                    health: 10.0,
                    behavior: CreatureBehavior::Rest,
                    timer: 60.0,
                    age: 0.0,
                    variant: 0,
                    needs: CreatureNeeds::full(CreatureKind::Cow),
                },
            );
        }
    }
    let served = std::cell::RefCell::new(std::collections::BTreeSet::new());
    wait(&mut host, &mut [&mut a, &mut b], |host, _| {
        let mut served = served.borrow_mut();
        for animal in host
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .animals
            .values()
        {
            if animal.needs.search_in > 0.0 {
                served.insert(animal.id);
            }
        }
        *served == ids
    });
    assert_eq!(served.borrow().len(), 256);
}
