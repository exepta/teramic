use super::*;
use crate::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::{GameplayState, ItemKind, ItemStack, WorldConfig, creatures::AnimalProduct};
use terra_voxel::{BlockSection, SectionPos};

fn animal() -> Creature {
    Creature {
        id: spider_id([0, 0], 0).unwrap(),
        kind: Kind::MediumSpider,
        home: [0, 0],
        position: [6.5, 3.015, 7.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: Kind::MediumSpider.health(),
        behavior: Behavior::Crawl,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(Kind::MediumSpider),
    }
}
fn fixture() -> (TerraGenerator, ChunkStore, EditJournal) {
    let terrain = TerraGenerator::new(WorldConfig {
        water: false,
        trees: false,
        caves: false,
        base_height: 15.0,
        ..Default::default()
    })
    .unwrap();
    let position = SectionPos::default();
    let mut data = BlockSection::empty(position).unwrap();
    for x in 0..16 {
        for z in 0..16 {
            data.set_block([x, 2, z], 3).unwrap();
            data.set_block([x, 12, z], 3).unwrap();
        }
    }
    for y in 3..=12 {
        for z in 0..16 {
            for x in [2, 13] {
                data.set_block([x, y, z], 3).unwrap();
            }
        }
    }
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        position,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![],
        },
    );
    (terrain, chunks, EditJournal::default())
}

#[test]
fn completed_nest_waits_thirty_to_ninety_real_seconds_then_hunts_returns_and_repeats() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal();
    a.position = [4.5, 7.0, 7.5];
    a = attach_with_depth(&h, &a, [2, 7, 7], [1, 0, 0], true).unwrap();
    let web = den_request(&a, &h).expect("real anchored web request");
    assert!(web.separation >= 8);
    assert!(crate::props::webs::is_construction(&web.prop().unwrap()));
    assert!(den_fits(web, &h));
    let mut cycles = Cycles::default();
    let mut nav = super::super::super::navigation::Navigation::default();
    let at_rest = a.position;
    let wait = wait_seconds(a.id, web.id, 0);
    while a.age + 0.00001 < wait {
        assert_eq!(cycles.phase(&a, web.id, web.anchor, 0.25), Phase::Wait);
        step_den(
            &mut a,
            &h,
            web.anchor,
            Phase::Wait,
            0.25,
            false,
            (&mut nav, true),
        );
        assert_eq!(a.position, at_rest);
        assert!(a.valid() && clear(&h, &a));
    }
    assert!((MIN_WAIT..=MAX_WAIT + 0.25).contains(&a.age));
    assert_eq!(cycles.phase(&a, web.id, web.anchor, 0.25), Phase::Hunt);
    assert!(cycles.wants_hunt(&a));
    a.position = [8.5, 3.015, 7.5];
    a.behavior = Behavior::Crawl;
    a.yaw = 0.0;
    for _ in 0..20 {
        step_den(
            &mut a,
            &h,
            web.anchor,
            Phase::Hunt,
            0.1,
            true,
            (&mut nav, true),
        );
        assert!(a.valid() && clear(&h, &a));
    }
    assert!(
        distance(a.position, [8.5, 3.015, 7.5]) > 0.01,
        "an empty hunting area still produces real bounded patrol motion"
    );
    while cycles.0[&a.id].clock < cycles.0[&a.id].until {
        assert_eq!(cycles.phase(&a, web.id, web.anchor, 0.25), Phase::Hunt);
    }
    assert_eq!(cycles.phase(&a, web.id, web.anchor, 0.25), Phase::Return);
    while cycles.0[&a.id].clock < cycles.0[&a.id].until {
        assert_eq!(cycles.phase(&a, web.id, web.anchor, 0.25), Phase::Return);
    }
    assert_eq!(cycles.phase(&a, web.id, web.anchor, 0.25), Phase::Wait);
    assert!(
        (MIN_WAIT - 0.25..=MAX_WAIT).contains(&(cycles.0[&a.id].until - cycles.0[&a.id].clock))
    );
    assert_eq!(cycles.0[&a.id].round, 1);
    cycles.forget(a.id);
    assert!(
        !cycles.wants_hunt(&a),
        "explicit pet commands remove, rather than run, the autonomous cycle"
    );
}

#[test]
fn wait_periods_are_deterministic_varied_and_never_sampled_again_each_tick() {
    let a = animal();
    let mut durations = std::collections::BTreeSet::new();
    for id in 1..=256 {
        for round in 0..3 {
            let seconds = wait_seconds(id, 99, round);
            assert!((MIN_WAIT..=MAX_WAIT).contains(&seconds));
            assert_eq!(seconds, wait_seconds(id, 99, round));
            durations.insert((seconds * 1000.0) as u64);
        }
    }
    assert!(durations.len() > 200);
    let mut cycles = Cycles::default();
    let home = [6, 3, 7];
    assert_eq!(cycles.phase(&a, 99, home, 0.1), Phase::Wait);
    let until = cycles.0[&a.id].until;
    for _ in 0..100 {
        assert_eq!(cycles.phase(&a, 99, home, 0.1), Phase::Wait);
    }
    assert_eq!(cycles.0[&a.id].until, until);
    while cycles.0[&a.id].clock < until {
        assert_eq!(cycles.phase(&a, 99, home, 0.1), Phase::Wait);
    }
    assert_eq!(
        a.age, 0.0,
        "growth-constrained biological age remains unchanged"
    );
    assert_eq!(cycles.phase(&a, 99, home, 0.1), Phase::Hunt);
    let mut population = CreaturePopulation::default();
    population.animals.insert(a.id, a.clone());
    cycles.retain(&population);
    assert_eq!(cycles.0.len(), 1);
    population.animals.get_mut(&a.id).unwrap().health = 0.0;
    cycles.retain(&population);
    assert!(cycles.0.is_empty());
}

#[test]
fn scheduled_hunt_reaches_real_food_even_when_not_starving_and_waiting_still_falls() {
    let (terrain, chunks, mut journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal();
    let mut gameplay = GameplayState::default();
    let center = terra_core::creatures::spiders::visual_center(&a);
    assert!(gameplay.drop_stack(
        [center[0] + 0.5, center[1], center[2]],
        ItemStack {
            item: ItemKind::Animal(AnimalProduct::Beef),
            count: 1
        },
        [0.0; 3],
        0.0
    ));
    let food = super::super::super::spider_hunting::FoodIndex::new(
        std::iter::empty(),
        gameplay.drops.iter(),
    );
    let mut nav = super::super::super::navigation::Navigation::default();
    assert!(a.needs.food > 40.0);
    assert!(matches!(
        super::super::super::spider_hunting::cycle_step(
            &mut a,
            &h,
            (&food, true),
            [2, 7, 7],
            0.1,
            true,
            &mut nav
        ),
        Some(Some(_))
    ));
    for x in 0..16 {
        for z in 0..16 {
            journal.record([x, 2, z], 0).unwrap();
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let before = a.position[1];
    step_den(
        &mut a,
        &h,
        [2, 7, 7],
        Phase::Wait,
        0.1,
        false,
        (&mut nav, true),
    );
    assert!(a.position[1] < before && a.velocity[1] < 0.0);
    assert!(a.valid());
}
