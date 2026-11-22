//! Three authenticated survival players observing the same authoritative encounter.
use super::*;
use terra_core::{
    GameMode, PlayerLocation, PlayerProfile,
    creatures::{
        Creature, CreatureBehavior, CreatureKind, CreatureNeeds, drops, hitboxes, spawning,
    },
};
use terra_world::edits::EditJournal;

fn hit_from_client(app: &mut App, id: u64) {
    let session = app.world().resource::<client::ClientSession>();
    let eye = Vec3::from_array(session.confirmed_state().unwrap().position)
        + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let replicas = app.world().resource::<client::ReplicatedCreatures>();
    let animal = &replicas.creatures[&id].current;
    let shape = hitboxes::for_creature(animal);
    let direction = shape
        .iter()
        .map(|part| Vec3::from_array(part.center.map(|v| v as f32)) - eye)
        .filter(|d| d.length_squared() > 0.01 && d.normalize().y.abs() < 0.99)
        .filter(|d| {
            shape
                .ray_distance(
                    eye.to_array().map(f64::from),
                    d.normalize().to_array().map(f64::from),
                    4.5,
                )
                .is_some()
        })
        .min_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
        .expect("an exposed leg in melee reach")
        .normalize();
    app.world_mut().resource_mut::<client::ClientSession>().look =
        [(-direction.x).atan2(-direction.z), direction.y.asin()];
    app.world_mut()
        .resource_mut::<client::CreatureActions>()
        .0
        .push_back((id, false));
}

#[test]
fn dragon_encounter_replication_tracks_only_participants_and_death_drops_once() {
    for kind in [
        CreatureKind::EarthDragon,
        CreatureKind::DarkDragon,
        CreatureKind::WaterDragon,
    ] {
        verify_encounter(kind);
    }
}

fn verify_encounter(kind: CreatureKind) {
    let mut host = server();
    {
        let mut config = host.world_mut().resource_mut::<GameConfig>();
        config.server.game_mode = GameMode::Survival;
        config.server.natural_mob_limit = Some(0);
    }
    let profiles = std::array::from_fn::<_, 3, _>(|_| PlayerProfile::default());
    let body_y = 32.0 + terra_physics::character::PLAYER_HALF_HEIGHT;
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        for x in 1..32 {
            for z in -3..20 {
                for y in 31..50 {
                    journal
                        .record([x, y, z], if y == 31 { 3 } else { 0 })
                        .unwrap();
                }
            }
        }
        for (profile, [x, z]) in profiles.iter().zip([[8.5, 6.5], [11.5, 10.0], [28.5, 9.5]]) {
            journal
                .update_player_location(
                    profile.uuid,
                    PlayerLocation {
                        position: [x, body_y, z],
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
    let [mut target, mut helper, mut observer] = profiles.map(|p| profile_client(address, p));
    let mut clients = [&mut target, &mut helper, &mut observer];
    wait(&mut host, &mut clients, |_, clients| {
        clients.iter().all(|app| {
            let s = app.world().resource::<client::ClientSession>();
            s.ready && s.confirmed_state().is_some_and(|p| p.grounded)
        })
    });
    std::thread::sleep(Duration::from_millis(460));
    let id = {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let population = &mut journal.gameplay_mut().creatures;
        population.animals.clear();
        let id = spawning::next_command_id(population).unwrap();
        population.animals.insert(
            id,
            Creature {
                id,
                kind,
                home: [0, 0],
                position: [8.5, 32., 8.5],
                velocity: [0.; 3],
                yaw: 0.,
                health: kind.health(),
                behavior: CreatureBehavior::Rest,
                timer: 0.,
                age: 80.,
                variant: 0,
                needs: CreatureNeeds::full(kind),
            },
        );
        id
    };
    wait(&mut host, &mut clients, |_, clients| {
        clients.iter().enumerate().all(|(i, app)| {
            let replicas = app.world().resource::<client::ReplicatedCreatures>();
            replicas.creatures.contains_key(&id) && replicas.boss == (i == 0).then_some(id)
        })
    });
    hit_from_client(clients[1], id);
    wait(&mut host, &mut clients, |_, clients| {
        clients.iter().enumerate().all(|(i, app)| {
            let r = app.world().resource::<client::ReplicatedCreatures>();
            r.boss == (i < 2).then_some(id) && r.creatures[&id].current.health < kind.health()
        })
    });
    // Resolve the real cast before the last melee blow; the distant observer
    // continues receiving the dragon but never joins or takes area damage.
    let initial_health = clients[0]
        .world()
        .resource::<client::ClientSession>()
        .stats
        .survival
        .health;
    wait(&mut host, &mut clients, |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .stats
            .survival
            .health
            < initial_health
    });
    assert_eq!(
        clients[2]
            .world()
            .resource::<client::ReplicatedCreatures>()
            .boss,
        None
    );
    assert_eq!(
        clients[2]
            .world()
            .resource::<client::ClientSession>()
            .stats
            .survival
            .health,
        initial_health
    );
    host.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&id)
        .unwrap()
        .health = 1.;
    hit_from_client(clients[0], id);
    wait(&mut host, &mut clients, |_, clients| {
        clients.iter().all(|app| {
            let replicas = app.world().resource::<client::ReplicatedCreatures>();
            replicas.boss.is_none()
                && replicas.creatures.get(&id).is_some_and(|t| {
                    t.current.health == 0. && t.current.behavior == CreatureBehavior::Dead
                })
        })
    });
    let expected: Vec<_> = drops::forest_loot(kind, 0, id)
        .into_iter()
        .flatten()
        .collect();
    let state = host.world().resource::<EditJournal>().gameplay();
    let actual: Vec<_> = state
        .drops
        .iter()
        .filter(|d| matches!(d.stack.item, terra_core::ItemKind::DragonLoot(_)))
        .map(|d| d.stack)
        .collect();
    assert_eq!(actual, expected);
    let next = state.next_drop;
    clients[1]
        .world_mut()
        .resource_mut::<client::CreatureActions>()
        .0
        .push_back((id, false));
    wait(&mut host, &mut clients, |_, clients| {
        clients.iter().all(|app| {
            let r = app.world().resource::<client::ReplicatedCreatures>();
            r.boss.is_none() && !r.creatures.contains_key(&id) && r.rejected_snapshots == 0
        })
    });
    assert_eq!(
        host.world().resource::<EditJournal>().gameplay().next_drop,
        next
    );
}
