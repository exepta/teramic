use super::*;
use crate::protocol::session::ClientControl;
use terra_core::creatures::{
    Creature, CreatureBehavior, CreatureKind, CreatureNeeds,
    taming::{AnimalBond, CompanionMode},
};
use terra_world::edits::EditJournal;

fn edit_cells(host: &mut App, cells: impl IntoIterator<Item = ([i64; 3], u16)>) {
    let properties = host
        .world()
        .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .clone();
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for (position, material) in cells {
                let before = chunks
                    .block_at(position)
                    .expect("fixture edits require loaded terrain");
                if before != material {
                    assert!(
                        terra_world::streaming::commit_block_edit(
                            &mut chunks,
                            &mut journal,
                            &terra_world::streaming::BlockEditRequest {
                                position,
                                expected_material: before,
                                material
                            },
                            &properties,
                        )
                        .unwrap()
                    );
                }
                assert_eq!(chunks.block_at(position), Some(material));
            }
        });
}

fn pump(host: &mut App, a: &mut App, b: &mut App) {
    for _ in 0..24 {
        host.update();
        a.update();
        b.update();
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn real_udp_pet_commands_validate_owner_and_loaded_los_then_replicate_follow_rest_and_none() {
    let (mut host, mut a, mut b) = pair();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .natural_mob_limit = Some(0);
    let owner = a.world().resource::<client::ClientSession>().uuid.unwrap();
    let start = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let id = (1 << 62) + 120;
    let position = [
        f64::from(start[0]),
        f64::from(start[1] - terra_physics::character::PLAYER_HALF_HEIGHT),
        f64::from(start[2] - 3.5),
    ];
    let floor = position[1].round() as i64 - 1;
    let x = position[0].floor() as i64;
    let z = position[2].floor() as i64;
    wait(&mut host, &mut [&mut a, &mut b], |h, _| {
        [[x - 2, floor, z - 1], [x + 2, floor + 4, z + 5]]
            .into_iter()
            .all(|p| h.world().resource::<ChunkStore>().block_at(p).is_some())
    });
    let mut ground = Vec::new();
    for dx in -2..=2 {
        for dz in -1..=5 {
            for dy in 0..=4 {
                ground.push(([x + dx, floor + dy, z + dz], if dy == 0 { 3 } else { 0 }));
            }
        }
    }
    edit_cells(&mut host, ground);
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let state = journal.gameplay_mut();
        state.creatures = Default::default();
        state.animal_bonds.clear();
        state.animal_families.clear();
        state.pet_orders.clear();
        state.spider_dens.clear();
        state.creatures.animals.insert(
            id,
            Creature {
                id,
                kind: CreatureKind::Cow,
                home: [x.div_euclid(32), z.div_euclid(32)],
                position,
                velocity: [0.0; 3],
                yaw: 0.0,
                health: CreatureKind::Cow.health(),
                behavior: CreatureBehavior::Rest,
                timer: 0.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(CreatureKind::Cow),
            },
        );
        state.creatures.owners.insert(id, owner);
        state.animal_bonds.insert(
            id,
            AnimalBond {
                mode: CompanionMode::Follow,
                ..AnimalBond::owned(owner)
            },
        );
        state.revision = state.revision.wrapping_add(1);
    }
    wait(&mut host, &mut [&mut a, &mut b], |_, c| {
        c.iter().all(|c| {
            c.world()
                .resource::<client::ReplicatedCreatures>()
                .bonds
                .get(&id)
                .is_some_and(|b| b.mode == CompanionMode::Wait)
        })
    });
    let original = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .creatures
        .animals[&id]
        .position;
    send(
        &mut b,
        ClientControl::PetCommand {
            id,
            mode: CompanionMode::Follow,
        },
    );
    pump(&mut host, &mut a, &mut b);
    assert!(
        !host
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .pet_orders
            .contains_key(&id)
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .animals[&id]
            .position,
        original
    );

    // A forged visible-pet identifier does not permit orders through solid terrain.
    let wall = [x, floor + 1, z + 2];
    let wall_cells: Vec<_> = (-1..=1)
        .flat_map(|dx| (0..=3).map(move |dy| [wall[0] + dx, wall[1] + dy, wall[2]]))
        .collect();
    edit_cells(&mut host, wall_cells.iter().map(|&p| (p, 3)));
    {
        let eye = start.map(f64::from);
        let eye = [
            eye[0],
            eye[1] + f64::from(terra_physics::character::PLAYER_EYE_OFFSET),
            eye[2],
        ];
        let target = [
            position[0],
            position[1] + CreatureKind::Cow.size()[1] * 0.5,
            position[2],
        ];
        let delta: [f64; 3] = std::array::from_fn(|i| target[i] - eye[i]);
        let length = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
        let (hit, _) = terra_world::targeting::pick_world_with_context(
            host.world().resource::<ChunkStore>(),
            host.world().resource::<EditJournal>(),
            host.world()
                .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
            eye,
            delta.map(|v| v / length),
            terra_world::props::PropPose::Rest,
        );
        assert_eq!(
            hit.expect("the committed wall must obstruct the real picker")
                .position[2],
            wall[2]
        );
    }
    std::thread::sleep(Duration::from_millis(460));
    send(
        &mut a,
        ClientControl::PetCommand {
            id,
            mode: CompanionMode::Follow,
        },
    );
    pump(&mut host, &mut a, &mut b);
    assert!(
        !host
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .pet_orders
            .contains_key(&id),
        "wall blocks owner commands too"
    );
    edit_cells(&mut host, wall_cells.into_iter().map(|p| (p, 0)));
    send(
        &mut a,
        ClientControl::PetCommand {
            id,
            mode: CompanionMode::Follow,
        },
    );
    wait(&mut host, &mut [&mut a, &mut b], |h, c| {
        h.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .animals[&id]
            .position[2]
            > original[2] + 0.3
            && c.iter().all(|c| {
                c.world()
                    .resource::<client::ReplicatedCreatures>()
                    .bonds
                    .get(&id)
                    .is_some_and(|b| b.mode == CompanionMode::Follow)
            })
    });
    std::thread::sleep(Duration::from_millis(460));
    a.world_mut()
        .resource_mut::<client::PetCommands>()
        .0
        .push_back((id, CompanionMode::Wait));
    wait(&mut host, &mut [&mut a, &mut b], |_, c| {
        c.iter().all(|c| {
            c.world()
                .resource::<client::ReplicatedCreatures>()
                .bonds
                .get(&id)
                .is_some_and(|b| b.mode == CompanionMode::Wait)
        })
    });
    let resting = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .creatures
        .animals[&id]
        .position;
    pump(&mut host, &mut a, &mut b);
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .creatures
            .animals[&id]
            .position,
        resting
    );
    std::thread::sleep(Duration::from_millis(460));
    send(
        &mut a,
        ClientControl::PetCommand {
            id,
            mode: CompanionMode::None,
        },
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, c| {
        c.iter().all(|c| {
            c.world()
                .resource::<client::ReplicatedCreatures>()
                .bonds
                .get(&id)
                .is_some_and(|b| b.mode == CompanionMode::None)
        })
    });
    let state = host.world().resource::<EditJournal>().gameplay();
    assert_eq!(state.pet_orders.get(&id), Some(&CompanionMode::None));
    assert_eq!(state.creatures.owners.get(&id), Some(&owner));
    assert!(state.valid());
}
