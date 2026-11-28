use super::*;
use terra_core::creatures::*;
use terra_world::edits::EditJournal;

#[test]
fn udp_hen_hit_makes_the_rooster_attack_only_the_responsible_player() {
    use crate::client::CreatureActions;
    let (mut host, mut a, mut b) = pair();
    {
        let mut config = host.world_mut().resource_mut::<GameConfig>();
        config.server.game_mode = terra_core::GameMode::Survival;
        config.server.natural_mob_limit = Some(0);
    }
    let position = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let [x, y, z] = position.map(f64::from);
    let feet = y - f64::from(terra_physics::character::PLAYER_HALF_HEIGHT);
    let eye = y + f64::from(terra_physics::character::PLAYER_EYE_OFFSET);
    let terrain = host
        .world()
        .resource::<terra_world::streaming::WorldTerrain>();
    let chunks = host
        .world()
        .resource::<terra_world::streaming::ChunkStore>();
    let journal = host.world().resource::<EditJournal>();
    let habitat = terra_world::creatures::Habitat {
        terrain: &terrain.0,
        chunks,
        journal,
    };
    let mut population = CreaturePopulation::default();
    let mut spawn = |kind: CreatureKind, at| {
        let id = spawning::next_command_id(&population).unwrap();
        let position = habitat.ground(at, kind).expect("loaded flat test ground");
        population.animals.insert(
            id,
            Creature {
                id,
                kind,
                position,
                home: [
                    (position[0].floor() as i64).div_euclid(32),
                    (position[2].floor() as i64).div_euclid(32),
                ],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: kind.health(),
                behavior: CreatureBehavior::Rest,
                timer: 3.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(kind),
            },
        );
        id
    };
    let hen = spawn(CreatureKind::Hen, [x + 2.5, feet, z]);
    let rooster = spawn(CreatureKind::Rooster, [x + 0.3, feet, z + 2.3]);
    let at = population.animals[&hen].position;
    let ray = Vec3::new(
        (at[0] - x) as f32,
        (at[1] + 0.32 - eye) as f32,
        (at[2] - z) as f32,
    )
    .normalize();
    host.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .creatures = population;
    a.world_mut().resource_mut::<client::ClientSession>().look =
        [(-ray.x).atan2(-ray.z), ray.y.asin()];
    std::thread::sleep(Duration::from_millis(500));
    a.world_mut()
        .resource_mut::<CreatureActions>()
        .0
        .push_back((hen, false));
    wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
        let creatures = &host.world().resource::<EditJournal>().gameplay().creatures;
        let reported_health = |i: usize| {
            clients[i]
                .world()
                .resource::<client::ClientSession>()
                .stats
                .survival
                .health
        };
        creatures.animals[&hen].health < CreatureKind::Hen.health()
            && matches!(
                creatures.animals[&rooster].behavior,
                CreatureBehavior::Attack | CreatureBehavior::Hunt
            )
            && reported_health(0) < reported_health(1)
    });
    assert_eq!(
        b.world()
            .resource::<client::ClientSession>()
            .stats
            .survival
            .health,
        terra_core::SurvivalVitals::default().health
    );
}

#[test]
fn wild_bee_birth_snapshots_keep_two_udp_clients_updating_and_invalid_births_recover() {
    use lightyear::prelude::{MessageSender, MessageSystems};
    use protocol::session::{CreatureSnapshotChannel, CreatureStates};
    #[derive(Resource, Default)]
    struct FixtureSnapshot(Option<CreatureStates>);
    fn publish(
        mut fixture: ResMut<FixtureSnapshot>,
        mut senders: Query<&mut MessageSender<CreatureStates>>,
    ) {
        let Some(packet) = fixture.0.take() else {
            return;
        };
        let mut sent = 0;
        for mut sender in &mut senders {
            sender.send::<CreatureSnapshotChannel>(packet.clone());
            sent += 1;
        }
        assert_eq!(sent, 2);
    }
    fn send(host: &mut App, packet: &CreatureStates) {
        assert!(packet.creatures.iter().all(Creature::valid));
        assert!(packet.ecology.valid());
        assert!(packet.families.iter().all(|family| family.valid()));
        host.world_mut().resource_mut::<FixtureSnapshot>().0 = Some(packet.clone());
    }
    let (mut host, mut a, mut b) = pair();
    host.init_resource::<FixtureSnapshot>()
        .add_systems(PostUpdate, publish.before(MessageSystems::Send));
    let child = breeding::next_birth_id(&CreaturePopulation::default(), 0).unwrap();
    let parents = [bee_id([0, 0], 0).unwrap(), bee_id([0, 0], 1).unwrap()];
    let cow = creature_id([0, 0], 0).unwrap();
    let make = |id, kind: CreatureKind| Creature {
        id,
        kind,
        home: [0, 0],
        position: [3.0, 2.0, 3.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: kind.health(),
        behavior: CreatureBehavior::Rest,
        timer: 0.0,
        age: 1.0,
        variant: 0,
        needs: CreatureNeeds::full(kind),
    };
    let bee = make(child, CreatureKind::Bee);
    let mut ecology = ecology::Ecology::default();
    ecology.bees.insert(
        child,
        ecology::BeeState {
            nest: 7,
            ..Default::default()
        },
    );
    let family = breeding::FamilyView::new(
        &bee,
        breeding::AnimalFamily {
            parents: Some(parents),
            ..Default::default()
        },
    );
    // Keep ordinary, independently running server snapshots older than this
    // transport fixture so they cannot replace the exact regression payload.
    let base = host.world().resource::<server::ServerRuntime>().tick + 1_000_000;
    let mut packet = CreatureStates {
        tick: base,
        creatures: vec![bee, make(cow, CreatureKind::Cow)],
        ecology,
        intentions: Vec::new(),
        bonds: Vec::new(),
        families: vec![family],
        boss: None,
        stalker_grip: None,
    };
    // Animation age and calendar age intentionally disagree. Both UDP clients
    // must reconstruct the calendar record and use it for geometry after joining.
    let dragon_id = breeding::next_birth_id(&CreaturePopulation::default(), 1).unwrap();
    let mut dragon = make(dragon_id, CreatureKind::WaterDragon);
    let life = dragon_life::DragonLife {
        months: 4.5,
        training: 26.,
        ..Default::default()
    };
    dragon.needs.dragon = Some(life);
    packet
        .families
        .push(breeding::FamilyView::new(&dragon, Default::default()));
    let keeper = terra_core::PlayerUuid::from_u128(155);
    packet.bonds.push(taming::CompanionView {
        id: dragon_id,
        player: keeper,
        owner: Some(keeper),
        trust: 100,
        mode: taming::CompanionMode::Follow,
    });
    packet.creatures.push(dragon);
    let hen_id = poultry_id([0, 0], 1).unwrap();
    let rooster_id = poultry_id([0, 0], 0).unwrap();
    let mut hen = make(hen_id, CreatureKind::Hen);
    hen.behavior = CreatureBehavior::Graze;
    hen.variant = 5;
    let mut rooster = make(rooster_id, CreatureKind::Rooster);
    rooster.behavior = CreatureBehavior::Attack;
    rooster.variant = 3;
    packet.creatures.extend([hen.clone(), rooster.clone()]);
    for phase in 0..4 {
        packet.tick = base + phase;
        packet.creatures[1].position[0] = 3.0 + phase as f64 * 0.25;
        let expected = packet.creatures[1].position;
        send(&mut host, &packet);
        wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
            clients.iter().all(|client| {
                let creatures = client.world().resource::<client::ReplicatedCreatures>();
                creatures.creatures.contains_key(&child)
                    && creatures
                        .creatures
                        .get(&cow)
                        .is_some_and(|track| track.current.position == expected)
            })
        });
        for client in [&a, &b] {
            let replicas = &client
                .world()
                .resource::<client::ReplicatedCreatures>()
                .creatures;
            let dragon = &replicas[&dragon_id].current;
            assert_eq!(replicas[&hen_id].current, hen);
            assert_eq!(replicas[&rooster_id].current, rooster);
            assert_eq!(dragon.needs.dragon, Some(life));
            assert_eq!(dragon.dragon_stage(), dragon_life::DragonStage::Adolescent);
            assert!(client.world().resource::<client::ClientSession>().ready);
            assert_eq!(
                client
                    .world()
                    .resource::<client::ReplicatedCreatures>()
                    .rejected_snapshots,
                0
            );
        }
    }
    let mut invalid = packet.clone();
    invalid.tick += 1;
    invalid.creatures[0] = make(child, CreatureKind::Cow);
    invalid.families[0].growth = invalid.creatures[0].growth();
    send(&mut host, &invalid);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            let creatures = client.world().resource::<client::ReplicatedCreatures>();
            creatures.rejected_snapshots == 1
                && creatures.last_rejected_tick == invalid.tick
                && creatures.creatures[&child].current.kind == CreatureKind::Bee
        })
    });
    packet.tick = invalid.tick;
    packet.creatures[1].position[0] += 1.0;
    let expected = packet.creatures[1].position;
    send(&mut host, &packet);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            let creatures = client.world().resource::<client::ReplicatedCreatures>();
            creatures.creatures[&cow].current.position == expected
                && creatures.creatures[&child].current.kind == CreatureKind::Bee
                && creatures.rejected_snapshots == 1
        })
    });
}

#[test]
fn independent_authoritative_creature_and_player_snapshots_keep_both_udp_clients_advancing() {
    let (mut host, mut a, mut b) = pair();
    let p = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let id = spawning::next_command_id(&CreaturePopulation::default()).unwrap();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .natural_mob_limit = Some(0);
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let state = journal.gameplay_mut();
        state.creatures = CreaturePopulation::default();
        state.animal_bonds.clear();
        state.animal_families.clear();
        state.pet_orders.clear();
        let animal = Creature {
            id,
            kind: CreatureKind::Cow,
            home: [0, 0],
            position: [
                f64::from(p[0]),
                f64::from(p[1] - terra_physics::character::PLAYER_HALF_HEIGHT),
                f64::from(p[2]) - 3.5,
            ],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: CreatureKind::Cow.health(),
            behavior: CreatureBehavior::Rest,
            timer: 60.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(CreatureKind::Cow),
        };
        assert!(animal.valid());
        state.creatures.animals.insert(id, animal);
        state.revision = state.revision.wrapping_add(1);
    }
    for _ in 0..4 {
        let player_ticks =
            [&a, &b].map(|client| client.world().resource::<client::ReplicatedPlayers>().tick);
        let target_age = host
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .animals[&id]
            .age
            + 0.15;
        // No injected snapshots: the real 10 Hz creature tick and 20 Hz player
        // publisher must both survive their simultaneous sequenced transport.
        wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
            clients.iter().enumerate().all(|(index, client)| {
                client.world().resource::<client::ReplicatedPlayers>().tick > player_ticks[index]
                    && client
                        .world()
                        .resource::<client::ReplicatedCreatures>()
                        .creatures
                        .get(&id)
                        .is_some_and(|track| track.current.age >= target_age)
            })
        });
        for client in [&a, &b] {
            let creatures = client.world().resource::<client::ReplicatedCreatures>();
            assert!(creatures.creatures[&id].current.valid());
            assert_eq!(creatures.rejected_snapshots, 0);
            assert!(client.world().resource::<client::ClientSession>().ready);
        }
    }
}

#[test]
fn real_udp_villagers_and_high_slot_military_roles_replicate_greet_and_keep_identity_on_late_join()
{
    use terra_core::{CropItem, ItemKind};
    let (mut host, mut a, mut b) = pair();
    let owner = a.world().resource::<client::ClientSession>().uuid.unwrap();
    let player = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let home = [
        (f64::from(player[0]).floor() as i64).div_euclid(32),
        (f64::from(player[2]).floor() as i64).div_euclid(32),
    ];
    let food = ItemKind::Crop(CropItem::Wheat);
    let target_role = 7;
    let target = villagers::id(home, u16::from(target_role)).unwrap();
    let ids: Vec<_> = (0..villagers::ROLE_COUNT)
        .map(|role| {
            villagers::id(
                home,
                if role >= villagers::CIVILIAN_ROLE_COUNT {
                    128 + u16::from(role - villagers::CIVILIAN_ROLE_COUNT)
                } else {
                    u16::from(role)
                },
            )
            .unwrap()
        })
        .collect();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .natural_mob_limit = Some(0);
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let state = journal.gameplay_mut();
        state.creatures = CreaturePopulation::default();
        state.animal_bonds.clear();
        state.inventories.entry(owner).or_default().insert(food, 3);
        for role in 0..villagers::ROLE_COUNT {
            let id = ids[usize::from(role)];
            let x = if role == target_role {
                f64::from(player[0])
            } else {
                f64::from(player[0]) + 5.0 + f64::from(role)
            };
            let resident = Creature {
                id,
                kind: CreatureKind::Villager,
                home,
                position: [
                    x,
                    f64::from(player[1] - terra_physics::character::PLAYER_HALF_HEIGHT),
                    f64::from(player[2]) - 3.5,
                ],
                velocity: [0.0; 3],
                yaw: 1.25,
                health: CreatureKind::Villager.health(),
                behavior: CreatureBehavior::Rest,
                timer: 0.0,
                age: 0.0,
                variant: role,
                needs: CreatureNeeds::full(CreatureKind::Villager),
            };
            assert!(resident.valid());
            state.creatures.animals.insert(id, resident);
        }
        state.revision = state.revision.wrapping_add(1);
    }
    a.world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(0);
    a.world_mut().resource_mut::<client::ClientSession>().look = [0.0, -0.1];
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|c| {
            let residents = c.world().resource::<client::ReplicatedCreatures>();
            ids.iter().enumerate().all(|(role, id)| {
                residents.creatures.get(id).is_some_and(|track| {
                    track.current.valid()
                        && track.current.kind == CreatureKind::Villager
                        && track.current.variant == role as u8
                        && track.current.title().starts_with(villagers::name(*id))
                })
            })
        }) && clients[0]
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .count(food)
            == 3
    });
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .natural_count(),
        0,
        "residents must not consume the configurable natural mob budget"
    );
    for c in [&mut a, &mut b] {
        c.world_mut().resource_mut::<client::ClientChat>().clear();
    }
    // Real interactions start with the same 450 ms cooldown as session setup.
    std::thread::sleep(Duration::from_millis(500));
    a.world_mut()
        .resource_mut::<client::CreatureActions>()
        .0
        .push_back((target, true));
    let greeting = villagers::greeting(target, target_role, false);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientChat>()
            .entries
            .iter()
            .any(|entry| entry.text == greeting)
    });
    assert!(
        b.world()
            .resource::<client::ClientChat>()
            .entries
            .iter()
            .all(|entry| entry.text != greeting),
        "private greetings must not be broadcast to observers"
    );
    {
        let state = host.world().resource::<EditJournal>().gameplay();
        let resident = &state.creatures.animals[&target];
        assert_eq!(state.inventories[&owner].count(food), 3);
        assert!(state.creatures.owners.is_empty());
        assert!(state.animal_bonds.is_empty());
        assert_eq!(resident.health, CreatureKind::Villager.health());
        assert_eq!(resident.behavior, CreatureBehavior::Rest);
        assert!(
            resident.timer > 2.0,
            "conversation must pause the resident's route"
        );
        assert!(
            (resident.yaw.abs() - std::f32::consts::PI).abs() < 0.1,
            "resident should turn toward its visitor"
        );
        assert_eq!(state.creatures.natural_count(), 0);
    }
    let dead = ids[0];
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let state = journal.gameplay_mut();
        let resident = state.creatures.animals.get_mut(&dead).unwrap();
        resident.health = 0.0;
        resident.behavior = CreatureBehavior::Dead;
        state.revision = state.revision.wrapping_add(1);
    }
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|c| {
            !c.world()
                .resource::<client::ReplicatedCreatures>()
                .creatures
                .contains_key(&dead)
        })
    });
    let token = host
        .world()
        .resource::<server::ServerRuntime>()
        .issue_token(126_010)
        .unwrap();
    let mut late = client(token);
    wait(&mut host, &mut [&mut a, &mut b, &mut late], |_, clients| {
        let residents = clients[2].world().resource::<client::ReplicatedCreatures>();
        residents.creatures.get(&target).is_some_and(|track| {
            track.current.valid()
                && track.current.id == target
                && track.current.variant == target_role
                && track.current.title().starts_with(villagers::name(target))
        }) && !residents.creatures.contains_key(&dead)
    });
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .animals[&dead]
            .health,
        0.0,
        "late joins must keep the persistent death tombstone"
    );
}

#[test]
fn wild_cave_spider_commits_and_replicates_a_real_web_before_eating_nearby_food() {
    cave_spider_web_and_food(false);
}

#[test]
fn owned_cave_spider_keeps_autonomous_web_and_food_behavior_without_following() {
    cave_spider_web_and_food(true);
}

fn cave_spider_web_and_food(owned: bool) {
    use terra_core::{ItemKind, ItemStack};
    use terra_world::props::{PropId, PropKind};
    let (mut host, mut a, mut b) = pair();
    // Advance real server/physics/network ticks, not the cycle's private state.
    // This keeps the intentionally long nest wait a short integration test.
    for app in [&mut host, &mut a, &mut b] {
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
    }
    let owner = a.world().resource::<client::ClientSession>().uuid.unwrap();
    let player = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .natural_mob_limit = Some(0);
    let x = f64::from(player[0]).floor() as i64;
    let z = f64::from(player[2]).floor() as i64 - 3;
    let floor =
        f64::from(player[1] - terra_physics::character::PLAYER_HALF_HEIGHT).round() as i64 - 12;
    let corners: Vec<_> = [-4, 4]
        .into_iter()
        .flat_map(|dx| {
            [-4, 4].into_iter().flat_map(move |dz| {
                [0, 11]
                    .into_iter()
                    .map(move |dy| [x + dx, floor + dy, z + dz])
            })
        })
        .collect();
    wait(&mut host, &mut [&mut a, &mut b], |h, _| {
        corners
            .iter()
            .all(|p| h.world().resource::<ChunkStore>().block_at(*p).is_some())
    });
    let id = spawning::next_command_id(&CreaturePopulation::default()).unwrap();
    let origin = [x as f64 + 0.5, floor as f64 + 1.015, z as f64 + 0.5];
    let drop_id;
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        for dx in -4..=4 {
            for dz in -4..=4 {
                for dy in 0..=11 {
                    // A real large web needs tall wall anchors and a clear
                    // passage below its sagging strands; a four-metre room
                    // can only support the legacy decorative web.
                    let block = if matches!(dy, 0 | 10 | 11) { 3 } else { 0 };
                    journal.record([x + dx, floor + dy, z + dz], block).unwrap();
                }
            }
        }
        let state = journal.gameplay_mut();
        state.creatures = CreaturePopulation::default();
        state.animal_bonds.clear();
        state.spider_dens.clear();
        let mut needs = CreatureNeeds::full(CreatureKind::MediumSpider);
        needs.food = 35.0;
        state.creatures.animals.insert(
            id,
            Creature {
                id,
                kind: CreatureKind::MediumSpider,
                home: [0, 0],
                position: origin,
                velocity: [0.0; 3],
                yaw: 0.0,
                health: CreatureKind::MediumSpider.health(),
                behavior: CreatureBehavior::Rest,
                timer: 0.0,
                age: 0.0,
                variant: 0,
                needs,
            },
        );
        if owned {
            state.creatures.owners.insert(id, owner);
            state
                .animal_bonds
                .insert(id, terra_core::creatures::taming::AnimalBond::owned(owner));
        }
        assert!(state.drop_stack(
            [origin[0] + 2.5, origin[1] + 0.25, origin[2]],
            ItemStack {
                item: ItemKind::Animal(AnimalProduct::Beef),
                count: 1
            },
            [0.0; 3],
            0.0
        ));
        drop_id = state.next_drop;
        state.revision = state.revision.wrapping_add(1);
    }
    wait(&mut host, &mut [&mut a, &mut b], |h, _| {
        h.world()
            .resource::<EditJournal>()
            .gameplay()
            .spider_dens
            .contains_key(&id)
    });
    let (den, built_at) = {
        let journal = host.world().resource::<EditJournal>();
        let state = journal.gameplay();
        assert_eq!(state.creatures.owners.contains_key(&id), owned);
        assert_eq!(state.animal_bonds.contains_key(&id), owned);
        assert!(
            state.drops.iter().any(|drop| drop.id == drop_id),
            "food cannot be consumed before the first web commit"
        );
        assert!(state.creatures.animals[&id].needs.food <= 35.0);
        (state.spider_dens[&id], state.creatures.animals[&id].age)
    };
    let section = terra_voxel::global_to_section(den.anchor).0;
    {
        let journal = host.world().resource::<EditJournal>();
        let chunks = host.world().resource::<ChunkStore>();
        let prop = journal
            .section_delta(section)
            .placed_props
            .into_iter()
            .find(|prop| prop.id == PropId(den.web_id))
            .expect("the server must commit a real prop before starting the nest cycle");
        assert!(terra_world::props::webs::is_construction(&prop));
        assert!(terra_world::props::webs::fits(&prop, |p| {
            chunks
                .block_at(p)
                .map(|block| journal.block(p).unwrap_or(block))
        }));
    }
    let wait_seconds = terra_world::creatures::spiders::cycle::wait_seconds(id, den.web_id, 0);
    assert!((30.0..=90.0).contains(&wait_seconds));
    wait(&mut host, &mut [&mut a, &mut b], |host, _| {
        let state = host.world().resource::<EditJournal>().gameplay();
        assert!(
            state.drops.iter().any(|drop| drop.id == drop_id),
            "food must remain untouched during the completed nest's real wait"
        );
        assert!(state.creatures.animals[&id].needs.food <= 35.0);
        state.creatures.animals[&id].age >= built_at + wait_seconds - 0.25
    });
    wait(&mut host, &mut [&mut a, &mut b], |h, clients| {
        let state = h.world().resource::<EditJournal>().gameplay();
        state.creatures.animals[&id].needs.food > 45.0
            && !state.drops.iter().any(|drop| drop.id == drop_id)
            && clients.iter().all(|c| {
                c.world()
                    .resource::<EditJournal>()
                    .section_delta(section)
                    .placed_props
                    .iter()
                    .any(|prop| {
                        prop.id == PropId(den.web_id)
                            && prop.kind == PropKind::SpiderWeb
                            && terra_world::props::webs::is_construction(prop)
                            && !prop.removed
                    })
            })
    });
    let journal = host.world().resource::<EditJournal>();
    let state = journal.gameplay();
    assert!(
        state.creatures.animals[&id].position[0] > origin[0] + 0.1,
        "hungry spider must physically reach its food without an owner destination"
    );
    assert_eq!(state.creatures.animals[&id].needs.water, 0.0);
    assert_eq!(
        journal
            .section_delta(section)
            .placed_props
            .iter()
            .filter(|p| p.id == PropId(den.web_id))
            .count(),
        1,
        "one real web, no duplicate entities/props"
    );
}

#[test]
fn two_udp_clients_share_taming_but_cannot_enable_follow_even_with_a_legacy_saved_mode() {
    use terra_core::{
        CropItem, ItemKind,
        creatures::taming::{AnimalBond, CompanionMode},
    };
    let (mut host, mut a, mut b) = pair();
    let player = a.world().resource::<client::ClientSession>().uuid.unwrap();
    let stranger = b.world().resource::<client::ClientSession>().uuid.unwrap();
    let p = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let id = spawning::next_command_id(&CreaturePopulation::default()).unwrap();
    let food = ItemKind::Crop(CropItem::Wheat);
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .natural_mob_limit = Some(0);
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let state = journal.gameplay_mut();
        state.creatures = CreaturePopulation::default();
        state.creatures.animals.insert(
            id,
            Creature {
                id,
                kind: CreatureKind::Cow,
                home: [0, 0],
                position: [
                    f64::from(p[0]),
                    f64::from(p[1] - terra_physics::character::PLAYER_HALF_HEIGHT),
                    f64::from(p[2]) - 3.5,
                ],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: CreatureKind::Cow.health(),
                behavior: CreatureBehavior::Rest,
                timer: 60.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(CreatureKind::Cow),
            },
        );
        state.animal_bonds.insert(
            id,
            AnimalBond {
                player,
                trust: 75,
                mode: CompanionMode::Wait,
                last_fed_age: None,
            },
        );
        for uuid in [player, stranger] {
            state.inventories.entry(uuid).or_default().insert(food, 3);
        }
        state.revision = state.revision.wrapping_add(1);
    }
    for client in [&mut a, &mut b] {
        client
            .world_mut()
            .resource_mut::<client::SelectedInventorySlot>()
            .0 = Some(0);
        client
            .world_mut()
            .resource_mut::<client::ClientSession>()
            .look = [0.0, -0.1];
    }
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|c| {
            c.world()
                .resource::<client::ReplicatedCreatures>()
                .bonds
                .get(&id)
                .is_some_and(|bond| bond.trust == 75)
                && c.world()
                    .resource::<client::ClientSession>()
                    .inventory
                    .count(food)
                    == 3
        })
    });
    // Connection setup starts the same 450 ms action cooldown as real interactions.
    std::thread::sleep(Duration::from_millis(500));
    a.world_mut()
        .resource_mut::<client::CreatureActions>()
        .0
        .push_back((id, true));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|c| {
            c.world()
                .resource::<client::ReplicatedCreatures>()
                .bonds
                .get(&id)
                .is_some_and(|bond| {
                    bond.trust == 100
                        && bond.owner == Some(player)
                        && bond.mode == CompanionMode::Wait
                })
        }) && clients[0]
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .count(food)
            == 2
    });
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .natural_count(),
        0
    );
    b.world_mut()
        .resource_mut::<client::CreatureActions>()
        .0
        .push_back((id, true));
    for _ in 0..35 {
        host.update();
        a.update();
        b.update();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .inventories[&stranger]
            .count(food),
        3
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .animal_bonds[&id]
            .player,
        player
    );
    b.world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = None;
    for _ in 0..25 {
        host.update();
        a.update();
        b.update();
        std::thread::sleep(Duration::from_millis(2));
    }
    b.world_mut()
        .resource_mut::<client::CreatureActions>()
        .0
        .push_back((id, true));
    for _ in 0..35 {
        host.update();
        a.update();
        b.update();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .animal_bonds[&id]
            .mode,
        CompanionMode::Wait
    );
    // Old saves keep their original mode field, but it must never drive AI again.
    let before = {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let state = journal.gameplay_mut();
        state.animal_bonds.get_mut(&id).unwrap().mode = CompanionMode::Follow;
        state.revision = state.revision.wrapping_add(1);
        state.creatures.animals[&id].position
    };
    a.world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = None;
    for _ in 0..25 {
        host.update();
        a.update();
        b.update();
        std::thread::sleep(Duration::from_millis(2));
    }
    std::thread::sleep(Duration::from_millis(500));
    a.world_mut()
        .resource_mut::<client::CreatureActions>()
        .0
        .push_back((id, true));
    for _ in 0..120 {
        host.update();
        a.update();
        b.update();
        std::thread::sleep(Duration::from_millis(2));
    }
    for c in [&a, &b] {
        let bond = c.world().resource::<client::ReplicatedCreatures>().bonds[&id];
        assert_eq!(bond.owner, Some(player));
        assert_eq!(bond.mode, CompanionMode::Wait);
    }
    let state = host.world().resource::<EditJournal>().gameplay();
    assert_eq!(
        state.animal_bonds[&id].mode,
        CompanionMode::Follow,
        "empty-hand commands must not mutate saved ownership data"
    );
    assert_eq!(state.creatures.animals[&id].position[0], before[0]);
    assert_eq!(state.creatures.animals[&id].position[2], before[2]);
    assert_eq!(state.creatures.owners.get(&id), Some(&player));
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .inventories[&player]
            .count(food),
        2
    );
}

#[test]
fn creature_intentions_are_opt_in_per_udp_viewer_and_cleared_when_inspection_ends() {
    let (mut host, mut a, mut b) = pair();
    let p = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position
        .map(f64::from);
    let home = [(p[0] as i64).div_euclid(32), (p[2] as i64).div_euclid(32)];
    let id = creature_id(home, 0).unwrap();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        journal.gameplay_mut().creatures.explored.insert(home);
        journal.gameplay_mut().creatures.animals.insert(
            id,
            Creature {
                id,
                home,
                kind: CreatureKind::Cow,
                position: [p[0] + 2.0, p[1], p[2] - 4.0],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: CreatureKind::Cow.health(),
                behavior: CreatureBehavior::Rest,
                timer: 60.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(CreatureKind::Cow),
            },
        );
    }
    a.world_mut().resource_mut::<client::CreatureInspection>().0 = Some(id);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ReplicatedCreatures>()
            .intentions
            .contains_key(&id)
    });
    assert!(
        b.world()
            .resource::<client::ReplicatedCreatures>()
            .intentions
            .is_empty()
    );
    let view = &a
        .world()
        .resource::<client::ReplicatedCreatures>()
        .intentions[&id];
    assert!(view.valid() && view.path.len() <= 8);
    a.world_mut().resource_mut::<client::CreatureInspection>().0 = None;
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ReplicatedCreatures>()
                .intentions
                .is_empty()
        })
    });
}

#[test]
fn creature_snapshots_received_before_session_ready_are_consumed_before_lightyear_clears() {
    use lightyear::prelude::{MessageReceiver, MessageSystems, client::Client};
    #[derive(Resource, Default)]
    struct Delivered(usize);
    let (mut host, mut a, _) = pair();
    a.init_resource::<Delivered>()
        .add_systems(
            PreUpdate,
            (|receivers: Query<
                &MessageReceiver<protocol::session::CreatureStates>,
                With<Client>,
            >,
              mut delivered: ResMut<Delivered>| {
                delivered.0 += receivers
                    .iter()
                    .map(MessageReceiver::num_messages)
                    .sum::<usize>();
            })
            .after(MessageSystems::Receive),
        )
        .add_systems(
            PostUpdate,
            |receivers: Query<
                &MessageReceiver<protocol::session::CreatureStates>,
                With<Client>,
            >| {
                assert!(
                    receivers
                        .iter()
                        .all(|receiver| receiver.num_messages() == 0),
                    "Pre-ready creature packets must not reach Lightyear's Last cleanup unhandled"
                );
            },
        );
    a.world_mut().resource_mut::<client::ClientSession>().ready = false;
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0].world().resource::<Delivered>().0 > 0
    });
    assert!(
        a.world()
            .resource::<client::ReplicatedCreatures>()
            .creatures
            .is_empty()
    );
}

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
                        dragon: None,
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

#[test]
fn territorial_spider_replicates_locked_web_and_slows_only_the_struck_survivor() {
    use terra_core::{EffectKind, GameMode, PlayerLocation, PlayerProfile};
    let mut host = server();
    let profiles = [PlayerProfile::default(), PlayerProfile::default()];
    let victim = profiles[0].uuid;
    {
        let mut config = host.world_mut().resource_mut::<GameConfig>();
        config.server.game_mode = GameMode::Survival;
        config.server.natural_mob_limit = Some(0);
    }
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        for x in 1..32 {
            for z in 5..15 {
                for y in 31..38 {
                    journal
                        .record([x, y, z], if y == 31 { 3 } else { 0 })
                        .unwrap();
                }
            }
        }
        for (p, x) in profiles.iter().zip([13.5, 28.5]) {
            journal
                .update_player_location(
                    p.uuid,
                    PlayerLocation {
                        position: [x, 32. + terra_physics::character::PLAYER_HALF_HEIGHT, 10.5],
                        look: [0.; 2],
                        flying: false,
                    },
                )
                .unwrap();
        }
    }
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let [mut a, mut b] = profiles.map(|p| profile_client(address, p));
    let mut clients = [&mut a, &mut b];
    wait(&mut host, &mut clients, |_, clients| {
        clients.iter().all(|c| {
            c.world()
                .resource::<client::ClientSession>()
                .confirmed_state()
                .is_some_and(|s| s.grounded)
        })
    });
    let id = {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let pop = &mut journal.gameplay_mut().creatures;
        pop.animals.clear();
        let id = spawning::next_command_id(pop).unwrap();
        let kind = CreatureKind::LargeSpider;
        pop.animals.insert(
            id,
            Creature {
                id,
                kind,
                home: [0, 0],
                position: [3.5, 32.015, 10.5],
                velocity: [0.; 3],
                yaw: 0.,
                health: kind.health(),
                behavior: CreatureBehavior::Crawl,
                timer: 0.,
                age: 29. - (id % 11) as f64,
                variant: 0,
                needs: CreatureNeeds::full(kind),
            },
        );
        id
    };
    wait(&mut host, &mut clients, |_, clients| {
        clients.iter().all(|c| {
            c.world()
                .resource::<client::ReplicatedCreatures>()
                .creatures
                .get(&id)
                .is_some_and(|track| spiders::WebShot::of(&track.current).is_some())
        })
    });
    wait(&mut host, &mut clients, |host, clients| {
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .player_stats
            .get(&victim)
            .is_some_and(|s| s.survival.effects.contains(EffectKind::Slow))
            && clients[0]
                .world()
                .resource::<client::ClientSession>()
                .stats
                .survival
                .effects
                .get(EffectKind::Slow)
                .is_some_and(|e| e.level == 2)
    });
    assert!(
        !clients[1]
            .world()
            .resource::<client::ClientSession>()
            .stats
            .survival
            .effects
            .contains(EffectKind::Slow)
    );
}
