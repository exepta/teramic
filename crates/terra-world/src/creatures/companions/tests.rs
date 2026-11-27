use super::*;
use crate::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::{
    PlayerUuid, WorldConfig,
    creatures::{CreatureNeeds, spawning},
};
use terra_voxel::{BlockSection, SectionPos};

fn fixture() -> (TerraGenerator, ChunkStore, EditJournal, Creature) {
    let terrain = TerraGenerator::new(WorldConfig {
        water: false,
        trees: false,
        caves: false,
        rivers: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    let owner = SectionPos::new(-2, 0, -2);
    let mut data = BlockSection::empty(owner).unwrap();
    for x in 0..16 {
        for z in 0..16 {
            data.set_block([x, 2, z], 3).unwrap();
        }
    }
    chunks.loaded.insert(
        owner,
        LoadedSection {
            data,
            mesh: None,
            water: None,
            mesh_epoch: 0,
            water_epoch: 0,
            props: vec![],
        },
    );
    let animal = Creature {
        id: spawning::next_command_id(&Default::default()).unwrap(),
        kind: Kind::Cow,
        home: [-1, -1],
        position: [-28.0, 3.0, -24.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: Kind::Cow.health(),
        behavior: Behavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(Kind::Cow),
    };
    (terrain, chunks, EditJournal::default(), animal)
}

#[test]
fn explicit_follow_moves_and_stops_near_owner_while_rest_and_legacy_follow_do_not() {
    let (terrain, chunks, mut journal, mut animal) = fixture();
    let destination = [-20.0, 3.0, -24.0];
    let start = animal.position;
    let mut nav = navigation::Navigation::default();
    for _ in 0..15 {
        step(
            &mut animal,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            Some(destination),
            CompanionMode::Follow,
            0.1,
            true,
            Some(&mut nav),
        );
    }
    assert_eq!(
        animal.position, start,
        "legacy caller/bond values do not authorize following"
    );
    let owner = PlayerUuid::from_u128(120);
    journal
        .gameplay_mut()
        .creatures
        .animals
        .insert(animal.id, animal.clone());
    journal
        .gameplay_mut()
        .creatures
        .owners
        .insert(animal.id, owner);
    assert!(terra_core::creatures::taming::command(
        journal.gameplay_mut(),
        animal.id,
        owner,
        CompanionMode::Follow
    ));
    for _ in 0..150 {
        step(
            &mut animal,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            Some(destination),
            CompanionMode::Follow,
            0.1,
            true,
            Some(&mut nav),
        );
    }
    let gap = (animal.position[0] - destination[0]).hypot(animal.position[2] - destination[2]);
    assert!(
        (1.74..=2.05).contains(&gap),
        "no overshoot into owner: {gap}"
    );
    assert!(animal.position[0] > start[0] + 4.0);
    let rest = animal.position;
    for _ in 0..30 {
        step(
            &mut animal,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            Some([-27.0, 3.0, -20.0]),
            CompanionMode::Wait,
            0.1,
            true,
            Some(&mut nav),
        );
    }
    assert_eq!(animal.position, rest);
    assert!(animal.valid());
}

#[test]
fn following_never_crosses_unknown_sections_walls_or_teleports_to_a_distant_owner() {
    let (terrain, mut chunks, mut journal, mut animal) = fixture();
    journal
        .gameplay_mut()
        .pet_orders
        .insert(animal.id, CompanionMode::Follow);
    for y in 3..=7 {
        for z in 0..16 {
            chunks
                .loaded
                .get_mut(&SectionPos::new(-2, 0, -2))
                .unwrap()
                .data
                .set_block([7, y, z], 3)
                .unwrap();
        }
    }
    let mut nav = navigation::Navigation::default();
    for _ in 0..150 {
        step(
            &mut animal,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            Some([-20.0, 3.0, -24.0]),
            CompanionMode::Follow,
            0.1,
            true,
            Some(&mut nav),
        );
    }
    assert!(
        animal.position[0] < -25.0 + 0.01,
        "solid wall must remain impassable: {:?}",
        animal.position
    );
    let before = animal.position;
    step(
        &mut animal,
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        },
        Some([300.0, 3.0, -24.0]),
        CompanionMode::Follow,
        0.1,
        true,
        Some(&mut nav),
    );
    assert_eq!(animal.position, before);
    step(
        &mut animal,
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        },
        None,
        CompanionMode::Follow,
        0.1,
        true,
        Some(&mut nav),
    );
    assert_eq!(animal.position, before);
}

#[test]
fn rest_commands_finish_a_real_horse_jump_without_cancelling_positive_vertical_velocity() {
    let (terrain, chunks, journal, mut horse) = fixture();
    horse.kind = Kind::Horse;
    horse.health = Kind::Horse.health();
    horse.needs = CreatureNeeds::full(Kind::Horse);
    horse.position[1] += 0.5;
    horse.behavior = Behavior::Leap;
    horse.velocity = [0.0, 5.0, 0.0];
    horse.timer = 1.0;
    let before = horse.position;
    step(
        &mut horse,
        &Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        },
        None,
        CompanionMode::Wait,
        0.1,
        false,
        None,
    );
    assert!(horse.position[1] > before[1] + 0.35);
    assert!(horse.velocity[1] > 3.5);
    assert_eq!(horse.behavior, Behavior::Leap);
}

#[test]
fn companion_uses_stairs_when_owner_stands_directly_overhead() {
    let (terrain, chunks, mut journal, mut pet) = fixture();
    pet.position = [-23.5, 3.0, -23.5];
    let owner = [-23.5, 6.0, -23.5];
    journal
        .gameplay_mut()
        .pet_orders
        .insert(pet.id, CompanionMode::Follow);
    // A stone balcony above the pet. Three steps beside the balcony can only
    // be reached by initially moving away from the owner's horizontal position.
    for x in -24..=-21 {
        for z in -24..=-21 {
            journal.record([x, 5, z], 3).unwrap();
        }
    }
    for (x, top) in [(-28, 3), (-27, 3), (-26, 4), (-25, 4)] {
        for z in -27..=-23 {
            for y in 3..=top {
                journal.record([x, y, z], 3).unwrap();
            }
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut nav = navigation::Navigation::default();
    for tick in 0..700 {
        let before = pet.position;
        step(
            &mut pet,
            &h,
            Some(owner),
            CompanionMode::Follow,
            0.1,
            tick % 6 == 0,
            Some(&mut nav),
        );
        assert!((pet.position[0] - before[0]).hypot(pet.position[2] - before[2]) <= 0.101);
        assert!((pet.position[1] - before[1]).abs() <= 1.001);
        assert!(h.clear_rotated(pet.position, pet.size(), false, f64::from(pet.yaw)));
        if pet.position[1] == owner[1] && pet.behavior == Behavior::Rest {
            break;
        }
    }
    assert_eq!(pet.position[1], owner[1], "pet ended at {:?}", pet.position);
    let gap = (pet.position[0] - owner[0]).hypot(pet.position[2] - owner[2]);
    assert!((0.9..=2.0).contains(&gap));
    assert_eq!(pet.behavior, Behavior::Rest);
}
