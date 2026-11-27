use super::*;
use terra_core::creatures::{
    Creature, CreatureKind, CreatureNeeds, CreaturePopulation, creature_id,
};

#[test]
fn stopped_fish_request_only_expired_budgeted_detours() {
    let mut fish = Creature {
        id: creature_id([0, 0], 0).unwrap(),
        kind: CreatureKind::Fish,
        home: [0, 0],
        position: [8.0, 5.0, 8.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: CreatureKind::Fish.health(),
        behavior: CreatureBehavior::Swim,
        timer: 3.0,
        age: 10.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::Fish),
    };
    fish.needs.search_in = 0.0;
    assert!(search_due(&fish, 0.1, false));
    fish.velocity[1] = -0.2;
    assert!(
        !search_due(&fish, 0.1, false),
        "unblocked diving needs no graph search"
    );
    fish.velocity = [0.0; 3];
    fish.needs.search_in = 2.0;
    assert!(!search_due(&fish, 0.1, false));
    fish.needs.search_in = 0.0;
    fish.behavior = CreatureBehavior::Stranded;
    assert!(!search_due(&fish, 0.1, false));
    fish.behavior = CreatureBehavior::Swim;
    let eligible = (0..24)
        .filter(|&id| {
            fish.id = id;
            search_due(&fish, 0.1, false)
        })
        .collect();
    assert_eq!(grant_searches(eligible, &mut None).len(), 8);
}

#[test]
fn snapshot_ecology_prioritizes_every_visible_birth_bee_without_exceeding_its_budget() {
    use terra_core::creatures::{
        bee_id, breeding,
        ecology::{BeeState, NestState},
    };
    let mut population = CreaturePopulation::default();
    for (id, position) in [(7, [0.0; 3]), (8, [95.0, 2.0, 0.0])] {
        population.ecology.nests.insert(
            id,
            NestState {
                position,
                nectar: 200,
                honey: 0,
                progress: 0.0,
            },
        );
    }
    for x in 0..200 {
        let home = [x, 0];
        let id = bee_id(home, 0).unwrap();
        population.explored.insert(home);
        population.animals.insert(
            id,
            Creature {
                id,
                kind: CreatureKind::Bee,
                home,
                position: [2.0; 3],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: CreatureKind::Bee.health(),
                behavior: CreatureBehavior::Fly,
                timer: 0.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(CreatureKind::Bee),
            },
        );
        population.ecology.bees.insert(
            id,
            BeeState {
                nest: 7,
                ..Default::default()
            },
        );
    }
    let mut visible = Vec::new();
    for _ in 0..MAX_VISIBLE_CREATURES {
        let id = breeding::next_birth_id(&population, 0).unwrap();
        let animal = Creature {
            id,
            kind: CreatureKind::Bee,
            home: [0, 0],
            position: [95.0, 2.0, 0.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: CreatureKind::Bee.health(),
            behavior: CreatureBehavior::Fly,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(CreatureKind::Bee),
        };
        population.animals.insert(id, animal.clone());
        population.ecology.bees.insert(
            id,
            BeeState {
                nest: 8,
                ..Default::default()
            },
        );
        visible.push(animal);
    }
    assert!(population.valid());
    let legacy = population.ecology.nearby([0.0; 3]);
    assert_eq!(legacy.bees.len(), MAX_VISIBLE_CREATURES);
    assert!(
        visible
            .iter()
            .all(|animal| !legacy.bees.contains_key(&animal.id))
    );
    for count in [1, 64, MAX_VISIBLE_CREATURES] {
        let snapshot = snapshot_ecology(&population, &visible[..count], [0.0; 3]);
        assert_eq!(snapshot.bees.len(), MAX_VISIBLE_CREATURES);
        assert!(
            visible[..count]
                .iter()
                .all(|animal| snapshot.bees.get(&animal.id)
                    == population.ecology.bees.get(&animal.id))
        );
        assert!(snapshot.valid());
        assert!(snapshot.nests.len() <= 32 && snapshot.flowers.len() <= 128);
    }
    population.ecology.nests.remove(&8);
    let snapshot = snapshot_ecology(&population, &visible, [0.0; 3]);
    assert!(snapshot.valid());
    assert!(
        visible
            .iter()
            .all(|animal| snapshot.bees.contains_key(&animal.id))
    );
}

#[test]
fn pet_commands_preserve_a_horses_ballistic_ascent_instead_of_resetting_it_to_rest() {
    let mut horse = Creature {
        id: creature_id([0, 0], 0).unwrap(),
        kind: CreatureKind::Horse,
        home: [0, 0],
        position: [8.0, 4.0, 8.0],
        velocity: [1.0, 5.0, 1.0],
        yaw: 0.0,
        health: CreatureKind::Horse.health(),
        behavior: CreatureBehavior::Leap,
        timer: 1.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::Horse),
    };
    pet_command_pose(&mut horse);
    assert_eq!(horse.behavior, CreatureBehavior::Leap);
    assert_eq!(horse.velocity, [0.0, 5.0, 0.0]);
    assert_eq!(horse.timer, 1.0);
}

#[test]
fn feeding_preserves_spider_surface_and_silk_attachments() {
    use terra_core::creatures::{NeedGoal, NeedKind, spiders};
    for kind in [
        CreatureKind::SmallSpider,
        CreatureKind::MediumSpider,
        CreatureKind::LargeSpider,
    ] {
        for behavior in [
            CreatureBehavior::Climb,
            CreatureBehavior::Ceiling,
            CreatureBehavior::Descend,
        ] {
            let mut animal = Creature {
                id: terra_core::creatures::spider_id([0, 0], 0).unwrap(),
                kind,
                home: [0, 0],
                position: [8.0, -30.0, 8.0],
                velocity: [0.0, -0.55, 0.0],
                yaw: std::f32::consts::FRAC_PI_2,
                health: kind.health(),
                behavior,
                timer: 2.0,
                age: 1.0,
                variant: 0,
                needs: CreatureNeeds::full(kind),
            };
            let anchor = NeedGoal {
                kind: NeedKind::Food,
                position: [8, -25, 8],
            };
            animal.needs.goal = Some(anchor);
            let normal = spiders::surface_normal(&animal);
            let position = animal.position;
            let center = spiders::visual_center(&animal);
            companion_feeding_pose(&mut animal);
            assert_eq!(animal.behavior, behavior);
            assert_eq!(spiders::surface_normal(&animal), normal);
            assert_eq!(spiders::visual_center(&animal), center);
            assert_eq!(animal.position, position);
            assert_eq!(animal.needs.goal, Some(anchor));
            assert!(animal.valid());
        }
    }
    let kind = CreatureKind::Cow;
    let mut animal = Creature {
        id: creature_id([0, 0], 0).unwrap(),
        kind,
        home: [0, 0],
        position: [8.0, 3.0, 8.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: kind.health(),
        behavior: CreatureBehavior::Wander,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(kind),
    };
    animal.needs.goal = Some(NeedGoal {
        kind: NeedKind::Food,
        position: [10, 3, 8],
    });
    companion_feeding_pose(&mut animal);
    assert_eq!(animal.behavior, CreatureBehavior::Graze);
    assert_eq!(animal.timer, 4.0);
    assert!(animal.needs.goal.is_some());
}

#[test]
fn removed_spider_dens_advance_one_persistent_id_without_reviving_or_scanning_tombstones() {
    use terra_core::creatures::taming::SpiderDen;
    use terra_world::{
        creatures::spiders::WebRequest,
        props::{PropId, webs},
    };
    let mut journal = EditJournal::default();
    let request = WebRequest {
        separation: 0,
        anchor: [-16, -12, 15],
        face: [0, 1, 0],
        variant: 5,
        id: 1 << 60,
    };
    let mut prior = None;
    let mut ids = std::collections::BTreeSet::new();
    for _ in 0..32 {
        let next = den_rebuild_request(request, prior, &journal);
        assert_eq!(
            next,
            den_rebuild_request(request, prior, &journal),
            "failed retries keep one identity"
        );
        assert!(ids.insert(next.id));
        let prop = webs::attached(next.anchor, next.face, next.variant, PropId(next.id)).unwrap();
        assert!(journal.place_prop(prop).unwrap());
        assert!(!journal.place_prop(prop).unwrap(), "no duplicate live web");
        assert!(journal.remove_prop(prop.section, prop.id).unwrap());
        assert!(
            !journal.place_prop(prop).unwrap(),
            "removed player edits stay removed"
        );
        prior = Some(SpiderDen {
            anchor: next.anchor,
            web_id: next.id,
        });
    }
    assert_eq!(journal.removed_props(), 32);
}

#[test]
fn authoritative_spanning_webs_require_loaded_anchors_deduplicate_and_preserve_the_section_cap() {
    use terra_world::{creatures::spiders::WebRequest, streaming::LoadedSection};
    let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
        water: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for x in 0..=1 {
        let position = terra_voxel::SectionPos::new(x, 0, 0);
        let mut data = terra_voxel::BlockSection::empty(position).unwrap();
        for y in 0..16 {
            for z in 0..16 {
                data.set_block([if x == 0 { 0 } else { 5 }, y, z], 3)
                    .unwrap();
            }
        }
        chunks.loaded.insert(
            position,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
    }
    let mut journal = EditJournal::default();
    let request = WebRequest {
        anchor: [0, 11, 4],
        face: [1, 0, 0],
        separation: 21,
        variant: 10,
        id: (1 << 60) + 118,
    };
    let far = terra_voxel::SectionPos::new(1, 0, 0);
    let removed = chunks.loaded.remove(&far).unwrap();
    assert!(spin_web(&mut chunks, &mut journal, &terrain, request).is_none());
    chunks.loaded.insert(far, removed);
    for (index, z) in [4, 7, 10, 13].into_iter().enumerate() {
        let web = WebRequest {
            anchor: [0, 11, z],
            id: request.id + index as u64,
            ..request
        };
        let den = spin_web(&mut chunks, &mut journal, &terrain, web).unwrap();
        assert_eq!(den.anchor, web.anchor);
        assert!(constructed_den(&chunks, &journal, den));
        assert_eq!(
            spin_web(&mut chunks, &mut journal, &terrain, web),
            Some(den)
        );
    }
    let owner = terra_voxel::SectionPos::default();
    assert_eq!(chunks.loaded[&owner].props.len(), 4);
    assert!(
        chunks.loaded[&owner]
            .props
            .iter()
            .all(terra_world::props::webs::is_construction)
    );
    let web = WebRequest {
        anchor: [0, 11, 15],
        id: request.id + 5,
        ..request
    };
    spin_web(&mut chunks, &mut journal, &terrain, web);
    assert_eq!(chunks.loaded[&owner].props.len(), 4);
    assert_eq!(journal.placed_props(owner).count(), 4);
    let existing = chunks.loaded[&owner].props[0];
    let den = terra_core::creatures::taming::SpiderDen {
        anchor: terra_world::props::webs::support(&existing),
        web_id: existing.id.0,
    };
    journal.remove_prop(owner, existing.id).unwrap();
    assert!(
        !constructed_den(&chunks, &journal, den),
        "a retired large web cannot start the wait/hunt cycle"
    );
}

#[test]
fn a_small_decorative_web_never_counts_as_a_completed_spider_construction() {
    use terra_world::{
        props::{PropId, webs},
        streaming::LoadedSection,
    };
    let prop = webs::attached([7, 5, 7], [1, 0, 0], 0, PropId(121)).unwrap();
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        prop.section,
        LoadedSection {
            data: terra_voxel::BlockSection::empty(prop.section).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![prop],
        },
    );
    let den = terra_core::creatures::taming::SpiderDen {
        anchor: [7, 5, 7],
        web_id: prop.id.0,
    };
    assert!(!constructed_den(&chunks, &EditJournal::default(), den));
}

#[test]
fn every_jelly_stings_on_body_contact_not_from_a_distance_or_after_death() {
    for kind in CreatureKind::ALL.into_iter().filter(|kind| kind.is_jelly()) {
        let mut jelly = Creature {
            id: creature_id([0, 0], 0).unwrap(),
            kind,
            home: [0, 0],
            position: [8.0, 3.0, 8.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: kind.health(),
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        };
        assert!(jelly_contact(&jelly, [8.0, 3.4, 8.0]), "{kind:?}");
        assert!(!jelly_contact(&jelly, [9.5, 3.4, 8.0]));
        assert!(!jelly_contact(&jelly, [8.0, 6.0, 8.0]));
        assert!(!jelly_contact(&jelly, [8.0, 0.0, 8.0]));
        jelly.health = 0.0;
        assert!(!jelly_contact(&jelly, [8.0, 3.4, 8.0]));
        jelly.health = kind.health();
        jelly.kind = CreatureKind::Fish;
        assert!(!jelly_contact(&jelly, [8.0, 3.4, 8.0]));
    }
}

#[test]
fn natural_habitats_keep_sixty_percent_without_changing_spawn_identities() {
    for seed in [0, 1, 42042, u64::MAX] {
        let mut enabled = 0;
        let mut total = 0;
        for x in -64..64 {
            for z in -64..64 {
                let home = [x, z];
                let id = creature_id(home, 0).unwrap();
                let accepted = natural_habitat_enabled(home, seed);
                assert_eq!(accepted, natural_habitat_enabled(home, seed));
                assert_eq!(Some(id), creature_id(home, 0));
                enabled += usize::from(accepted);
                total += 1;
            }
        }
        let ratio = enabled as f64 / total as f64;
        assert!((0.58..=0.62).contains(&ratio), "seed={seed}: {ratio}");
    }
    assert!(!natural_habitat_enabled([31_251, 0], 42042));
    assert!(!natural_habitat_enabled([0, -31_251], 42042));
}

#[test]
fn active_bears_only_request_the_shared_search_budget_for_real_pending_detours() {
    let mut bear = Creature {
        id: creature_id([0, 0], 0).unwrap(),
        kind: CreatureKind::Bear,
        home: [0, 0],
        position: [8.0, 3.0, 8.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: 24.0,
        behavior: CreatureBehavior::Hunt,
        timer: 0.0,
        age: 30.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::Bear),
    };
    for behavior in [CreatureBehavior::Hunt, CreatureBehavior::Attack] {
        bear.behavior = behavior;
        assert!(!search_due(&bear, 0.1, false));
        assert!(search_due(&bear, 0.1, true));
    }
    bear.needs.goal = Some(terra_core::creatures::NeedGoal {
        kind: terra_core::creatures::NeedKind::Tree,
        position: [8, 3, 9],
    });
    bear.behavior = CreatureBehavior::Climb;
    assert!(!search_due(&bear, 0.1, true));
    bear.behavior = CreatureBehavior::Wander;
    assert!(search_due(&bear, 0.1, true));
    assert!(!search_due(&bear, 0.1, false));
    bear.needs.goal = None;
    bear.behavior = CreatureBehavior::Rest;
    assert!(search_due(&bear, 0.1, false));
    bear.kind = CreatureKind::Cow;
    assert!(search_due(&bear, 0.1, false));
}

#[test]
fn spider_combat_only_requests_a_grant_for_a_real_blocked_surface_route_and_never_silk() {
    let kind = CreatureKind::MediumSpider;
    let mut animal = Creature {
        id: terra_core::creatures::spider_id([0, 0], 1).unwrap(),
        kind,
        home: [0, 0],
        position: [8.0, 3.0, 8.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: kind.health(),
        behavior: CreatureBehavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(kind),
    };
    for behavior in [
        CreatureBehavior::Hunt,
        CreatureBehavior::Attack,
        CreatureBehavior::Descend,
    ] {
        animal.behavior = behavior;
        assert!(!search_due(&animal, 0.1, false));
        assert_eq!(
            search_due(&animal, 0.1, true),
            behavior != CreatureBehavior::Descend
        );
    }
    animal.needs.search_in = 1.0;
    for behavior in [CreatureBehavior::Climb, CreatureBehavior::Ceiling] {
        animal.behavior = behavior;
        assert!(!search_due(&animal, 0.1, false));
    }
    animal.behavior = CreatureBehavior::Crawl;
    animal.needs.goal = Some(terra_core::creatures::NeedGoal {
        kind: terra_core::creatures::NeedKind::Shelter,
        position: [10, 3, 8],
    });
    animal.needs.search_in = 0.0;
    assert!(search_due(&animal, 0.1, false));
    animal.needs.search_in = 10.0;
    assert!(!search_due(&animal, 0.1, false));
    assert!(search_due(&animal, 0.1, true));
}

#[test]
fn land_detours_request_a_budget_even_with_an_existing_food_or_water_goal() {
    for kind in [CreatureKind::Cow, CreatureKind::Sheep, CreatureKind::Pig] {
        let mut creature = Creature {
            id: creature_id([0, 0], 0).unwrap(),
            kind,
            home: [0, 0],
            position: [8.0, 3.0, 8.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: kind.health(),
            behavior: CreatureBehavior::SeekFood,
            timer: 0.0,
            age: 300.0,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        };
        creature.needs.search_in = 10.0;
        for (behavior, need) in [
            (
                CreatureBehavior::SeekFood,
                terra_core::creatures::NeedKind::Food,
            ),
            (
                CreatureBehavior::SeekWater,
                terra_core::creatures::NeedKind::Water,
            ),
        ] {
            creature.behavior = behavior;
            creature.needs.goal = Some(terra_core::creatures::NeedGoal {
                kind: need,
                position: [8, 3, 4],
            });
            assert!(!search_due(&creature, 0.1, false));
            assert!(search_due(&creature, 0.1, true));
        }
        creature.behavior = CreatureBehavior::Sleep;
        assert!(!search_due(&creature, 0.1, true));
        creature.needs.goal = None;
        creature.needs.search_in = 0.0;
        assert!(!search_due(&creature, 0.1, true));
        creature.behavior = CreatureBehavior::Flee;
        assert!(!search_due(&creature, 0.1, true));
        creature.kind = CreatureKind::Fish;
        creature.behavior = CreatureBehavior::Swim;
        // Free swimmers need no search; stationary fish now share the budget.
        creature.velocity = [0.2, 0.0, 0.0];
        assert!(!search_due(&creature, 0.1, true));
    }
}

#[test]
fn repeated_pending_navigation_shares_eight_searches_fairly_across_large_herds() {
    for count in [128_u64, 137, 257] {
        let eligible: Vec<_> = (1..=count).rev().collect();
        let mut cursor = None;
        let mut granted = std::collections::HashSet::new();
        for _ in 0..count.div_ceil(8) {
            let searches = grant_searches(eligible.clone(), &mut cursor);
            assert_eq!(searches.len(), 8);
            assert_eq!(
                searches
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                8
            );
            granted.extend(searches);
        }
        assert_eq!(granted.len(), count as usize);
    }
}

#[test]
fn fair_navigation_grants_follow_changed_loaded_candidates_and_wrap_at_the_maximum_id() {
    let mut cursor = None;
    assert_eq!(
        grant_searches((1..=24).collect(), &mut cursor),
        (1..=8).collect::<Vec<_>>()
    );
    let eligible: Vec<_> = (1..=24)
        .filter(|id| !(9..=12).contains(id))
        .chain(100..=108)
        .chain([13, 13, 14])
        .collect();
    assert_eq!(
        grant_searches(eligible.clone(), &mut cursor),
        (13..=20).collect::<Vec<_>>()
    );
    assert_eq!(
        grant_searches(vec![u64::MAX, 2, u64::MAX - 1, 1], &mut Some(u64::MAX - 1)),
        vec![u64::MAX, 1, 2, u64::MAX - 1]
    );
    cursor = Some(u64::MAX);
    assert_eq!(
        grant_searches(eligible, &mut cursor),
        (1..=8).collect::<Vec<_>>()
    );
    assert_eq!(cursor, Some(8));
    assert!(grant_searches(Vec::new(), &mut cursor).is_empty());
    assert_eq!(cursor, Some(8));
}

#[test]
fn caps_are_local_and_exclude_owned_dead_and_far_away_animals() {
    let mut population = CreaturePopulation::default();
    for (home, x) in [([0, 0], 10.0), ([20, 0], 650.0)] {
        let id = creature_id(home, 0).unwrap();
        population.animals.insert(
            id,
            Creature {
                id,
                kind: CreatureKind::Cow,
                home,
                position: [x, 0.0, 0.0],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: 10.0,
                behavior: CreatureBehavior::Rest,
                timer: 0.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(CreatureKind::Cow),
            },
        );
    }
    assert_eq!(natural_near(&population, [0.0; 3]), 1);
    assert_eq!(natural_near(&population, [650.0, 0.0, 0.0]), 1);
    let id = creature_id([0, 0], 0).unwrap();
    population
        .owners
        .insert(id, terra_core::PlayerUuid::new_v4());
    assert_eq!(natural_near(&population, [0.0; 3]), 0);
    population
        .animals
        .get_mut(&creature_id([20, 0], 0).unwrap())
        .unwrap()
        .health = 0.0;
    assert_eq!(natural_near(&population, [650.0, 0.0, 0.0]), 0);
}

#[test]
fn visible_worm_anatomy_has_an_interest_margin_and_started_cycles_finish_beyond_it() {
    let mut chunks = ChunkStore::default();
    let mut worm = Creature {
        id: creature_id([0, 0], 0).unwrap(),
        kind: CreatureKind::SandWorm,
        home: [0, 0],
        position: [164.0, 1.0, 0.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: 42.0,
        behavior: CreatureBehavior::Leap,
        timer: 2.0,
        age: 1.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::SandWorm),
    };
    assert!(
        in_interest(&worm, [0.0, 1.0, 0.0]),
        "visible rings must not freeze when their anchor crosses 150 m"
    );
    worm.position[0] = 230.0;
    assert!(!in_interest(&worm, [0.0, 1.0, 0.0]));
    for phase in [
        CreatureBehavior::Tremor,
        CreatureBehavior::Leap,
        CreatureBehavior::Emerge,
        CreatureBehavior::Attack,
        CreatureBehavior::Spit,
        CreatureBehavior::Retreat,
    ] {
        worm.behavior = phase;
        assert!(
            needs_distant_tick(&worm, &chunks),
            "{phase:?} finishes despite lost interest"
        );
    }
    worm.behavior = CreatureBehavior::Burrowed;
    assert!(!needs_distant_tick(&worm, &chunks));
    worm.kind = CreatureKind::LavaWorm;
    worm.behavior = CreatureBehavior::Swim;
    assert!(!needs_distant_tick(&worm, &chunks));
    let (section, local) = terra_voxel::global_to_section([230, 0, 0]);
    let mut data = terra_voxel::BlockSection::empty(section).unwrap();
    data.set_block(local, terra_voxel::MaterialId::Lava as u16)
        .unwrap();
    chunks.loaded.insert(
        section,
        terra_world::streaming::LoadedSection {
            data,
            props: vec![],
            mesh: None,
            mesh_epoch: 0,
            water: None,
            water_epoch: 0,
        },
    );
    assert!(needs_distant_tick(&worm, &chunks));
}

#[test]
fn a_molten_worm_hit_ignites_only_a_vulnerable_living_player_for_five_seconds() {
    use terra_core::{EffectKind, GameMode};
    for (mode, protection, burns) in [
        (GameMode::Survival, 0., true),
        (GameMode::Creative, 0., false),
        (GameMode::Survival, 1., false),
    ] {
        let mut peer = super::super::survival::tests::resting_peer(85.);
        peer.survival.protection = protection;
        molten_worm_hit(&mut peer, 6., mode);
        assert_eq!(
            peer.vitals
                .effects
                .timed(EffectKind::OnFire)
                .map(|a| a.remaining_seconds),
            burns.then_some(5.)
        );
        if burns {
            let health = peer.vitals.health;
            for _ in 0..20 {
                peer.vitals.advance_life(false, 0.25);
            }
            assert!(!peer.vitals.effects.contains(EffectKind::OnFire));
            assert!(peer.vitals.health < health);
        }
    }
}
