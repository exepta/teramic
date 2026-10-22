use super::*;
use crate::{
    TerraGenerator,
    edits::EditJournal,
    props::{PropId, WorldProp},
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::{
    WorldConfig,
    creatures::{CreatureNeeds, bee_id, creature_id},
};
use terra_voxel::{SectionPos, blocks::BlockSection};
fn fixture() -> (TerraGenerator, ChunkStore, EditJournal) {
    let terrain = TerraGenerator::new(WorldConfig {
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        caves: false,
        trees: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let owner = SectionPos::default();
    let mut data = BlockSection::empty(owner).unwrap();
    for x in 0..16 {
        for z in 0..16 {
            data.set_block([x, 0, z], 3).unwrap();
        }
    }
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        owner,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    let mut journal = EditJournal::default();
    // Explicitly dry test space, independent of the unchanged procedural hydrology.
    for x in 0..16 {
        for y in 1..16 {
            for z in 0..16 {
                journal.set_fluid([x, y, z], Default::default()).unwrap();
            }
        }
    }
    (terrain, chunks, journal)
}
fn animal(kind: Kind, p: [f64; 3]) -> Creature {
    Creature {
        id: if kind == Kind::Bee {
            bee_id([0, 0], 0).unwrap()
        } else {
            creature_id([0, 0], 0).unwrap()
        },
        kind,
        home: [0, 0],
        position: p,
        velocity: [0.0; 3],
        yaw: 0.0,
        health: kind.health(),
        behavior: Behavior::Wander,
        timer: 1.0,
        age: 4.0,
        variant: 0,
        needs: CreatureNeeds::full(kind),
    }
}
#[test]
fn flight_detours_around_a_trunk_without_landing_or_entering_solids() {
    let (terrain, mut chunks, journal) = fixture();
    for y in 1..=5 {
        for z in 4..=6 {
            chunks
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block([5, y, z], 6)
                .unwrap();
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut bee = animal(Kind::Bee, [3.5, 2.0, 5.5]);
    let mut navigation = super::super::navigation::Navigation::default();
    for _ in 0..200 {
        bee.age += 0.1;
        super::super::navigation::fly_cached(
            &mut bee,
            &h,
            [8.5, 2.0, 5.5],
            0.1,
            Some(&mut navigation),
        );
        assert!(h.clear(bee.position, bee.kind.size(), false));
        assert_ne!(bee.behavior, Behavior::Land);
    }
    assert!(
        distance(bee.position, [8.5, 2.0, 5.5]) < 0.1,
        "{:?}",
        bee.position
    );
    assert!(
        super::super::navigation::waypoint(&h, [18.5, 2.0, 5.5], [20.5, 2.0, 5.5], Kind::Bee)
            .is_none()
    );
}
#[test]
fn coincident_bees_separate_and_sleep_times_are_stable_but_different() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut pop = terra_core::creatures::CreaturePopulation::default();
    let ids: Vec<_> = (0..4).map(|i| bee_id([0, 0], i).unwrap()).collect();
    for &id in &ids {
        let mut a = animal(Kind::Bee, [5.5, 2.0, 5.5]);
        a.id = id;
        pop.animals.insert(id, a);
    }
    for _ in 0..20 {
        super::super::navigation::separate(&mut pop, &h, &ids, 0.1);
    }
    assert!(distance(pop.animals[&ids[0]].position, pop.animals[&ids[3]].position) > 0.01);
    let sleeping = (1..100).filter(|&id| sleep_at(id, 20.0 / 24.0)).count();
    assert!((20..80).contains(&sleeping));
    assert!((1..100).all(|id| sleep_at(id, 0.0)));
    assert!((1..100).all(|id| !sleep_at(id, 0.5)));
}
#[test]
fn bees_reserve_separate_flowers_and_hide_inside_for_daytime_delivery() {
    let (terrain, mut chunks, journal) = fixture();
    let owner = SectionPos::default();
    for i in 0..2 {
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .props
            .push(WorldProp {
                id: PropId(100 + i),
                kind: PropKind::Flower,
                position: [5.5 + i as f64 * 2.0, 1.0, 5.5],
                rotation: 0.0,
                scale: 0.7,
                variant: 0,
                section: owner,
                removed: false,
            });
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut eco = Ecology::default();
    eco.nests.insert(
        12,
        NestState {
            position: [4.5, 2.0, 5.5],
            nectar: 0,
            honey: 0,
            progress: 0.0,
        },
    );
    let mut a = animal(Kind::Bee, [3.5, 2.0, 5.5]);
    a.timer = 0.0;
    let mut b = a.clone();
    b.id = bee_id([0, 0], 1).unwrap();
    step(&mut a, &mut eco, &h, &[], 0.1, true, false);
    step(&mut b, &mut eco, &h, &[], 0.1, true, false);
    assert!(eco.bees[&a.id].flower.is_some());
    assert_ne!(eco.bees[&a.id].flower, eco.bees[&b.id].flower);
    a.position = [4.5, 2.25, 5.85];
    eco.bees.get_mut(&a.id).unwrap().carried = 50;
    step(&mut a, &mut eco, &h, &[], 0.1, true, false);
    assert_eq!(a.behavior, Behavior::InNest);
    assert_eq!(eco.nests[&12].nectar, 50);
    for _ in 0..35 {
        step(&mut a, &mut eco, &h, &[], 0.1, true, false);
    }
    assert!(!eco.bees[&a.id].inside);
}
#[test]
fn mammals_sleep_through_passing_players_and_wake_by_day() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut eco = Ecology::default();
    for kind in [Kind::Cow, Kind::Sheep, Kind::Pig] {
        let mut a = animal(kind, [5.5, 1.0, 5.5]);
        step(&mut a, &mut eco, &h, &[], 0.1, true, true);
        assert_eq!(a.behavior, Behavior::Sleep);
        assert_eq!(a.velocity, [0.0; 3]);
        step(&mut a, &mut eco, &h, &[], 0.1, true, false);
        assert_ne!(a.behavior, Behavior::Sleep);
        step(&mut a, &mut eco, &h, &[[5.5, 1.0, 6.5]], 0.1, true, true);
        assert_eq!(a.behavior, Behavior::Sleep);
    }
}
#[test]
fn actual_flower_is_depleted_once_then_bee_returns_to_nest_and_emerges_by_day() {
    let (terrain, mut chunks, journal) = fixture();
    let owner = SectionPos::default();
    let flower = WorldProp {
        id: PropId(123),
        kind: PropKind::Flower,
        position: [5.5, 1.0, 5.5],
        rotation: 0.0,
        scale: 0.7,
        variant: 0,
        section: owner,
        removed: false,
    };
    chunks.loaded.get_mut(&owner).unwrap().props.push(flower);
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut eco = Ecology::default();
    eco.nests.insert(
        12,
        NestState {
            position: [7.5, 2.0, 5.5],
            nectar: 0,
            honey: 0,
            progress: 0.0,
        },
    );
    let mut a = animal(
        Kind::Bee,
        [5.5, 1.0 + f64::from(flower.visual_scale()) * 0.85, 5.5],
    );
    eco.bees.insert(
        a.id,
        BeeState {
            nest: 12,
            ..Default::default()
        },
    );
    a.behavior = Behavior::Nectar;
    a.timer = 0.05;
    step(&mut a, &mut eco, &h, &[], 0.1, false, false);
    assert_eq!(eco.nectar(123), 75);
    assert_eq!(eco.bees[&a.id].carried, 25);
    step(&mut a, &mut eco, &h, &[], 0.1, false, false);
    assert_eq!(eco.nectar(123), 75);
    for _ in 0..120 {
        step(&mut a, &mut eco, &h, &[], 0.1, true, true);
    }
    assert_eq!(a.behavior, Behavior::InNest);
    assert_eq!(eco.inside(12), 1);
    assert_eq!(eco.nests[&12].nectar, 25);
    step(&mut a, &mut eco, &h, &[], 0.1, true, false);
    assert_eq!(a.behavior, Behavior::Fly);
    assert_eq!(eco.inside(12), 0);
    assert!(!chunks.loaded[&owner].props[0].removed);
}
#[test]
fn fish_jump_is_ballistic_and_reenters_actual_liquid() {
    let (terrain, chunks, mut journal) = fixture();
    for x in 1..15 {
        for z in 1..15 {
            journal
                .set_fluid(
                    [x, 1, z],
                    crate::water::FluidCell {
                        amount: 8,
                        ..Default::default()
                    },
                )
                .unwrap();
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal(Kind::Fish, [7.5, 1.65, 7.5]);
    a.age = (40..100_000)
        .find(|t| random(a.id, *t).is_multiple_of(240))
        .unwrap() as f64
        / 10.0;
    assert!(fish_leap(&mut a, &h, 0.05));
    assert_eq!(a.behavior, Behavior::Leap);
    let mut left = false;
    for _ in 0..80 {
        left |= !h.wet(a.position);
        if a.behavior != Behavior::Leap {
            break;
        }
        fish_leap(&mut a, &h, 0.05);
    }
    assert!(left);
    assert_eq!(a.behavior, Behavior::Swim);
    assert!(h.wet(a.position));
    assert!(a.valid());
}
#[test]
fn unloaded_cells_suspend_even_swimming_and_nest_ai() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut eco = Ecology::default();
    for kind in [Kind::Bee, Kind::Cow, Kind::Fish] {
        let mut a = animal(kind, [500.0, 1.0, 500.0]);
        let before = a.clone();
        step(&mut a, &mut eco, &h, &[], 0.1, true, true);
        assert_eq!(a, before);
    }
}
