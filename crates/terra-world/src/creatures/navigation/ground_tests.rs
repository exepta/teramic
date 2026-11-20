use super::*;
use crate::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::{
    WorldConfig,
    creatures::{CreatureBehavior, CreatureNeeds, villagers},
};
use terra_voxel::{BlockSection, SectionPos, global_to_section};

fn fixture() -> (TerraGenerator, ChunkStore, EditJournal) {
    let terrain = TerraGenerator::new(WorldConfig {
        water: false,
        trees: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for x in -1..=1 {
        let owner = SectionPos::new(x, 0, 0);
        let mut data = BlockSection::empty(owner).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 0, z], 3).unwrap();
            }
        }
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
    }
    (terrain, chunks, EditJournal::default())
}

fn animal(position: [f64; 3]) -> Creature {
    Creature {
        id: villagers::id([0, 0], 3).unwrap(),
        kind: Kind::Villager,
        home: [0, 0],
        position,
        velocity: [0.0; 3],
        yaw: 0.0,
        health: 20.0,
        behavior: CreatureBehavior::Wander,
        timer: 0.0,
        age: terra_core::creatures::breeding::GROWTH_SECONDS + 1.0,
        variant: 1,
        needs: CreatureNeeds::full(Kind::Villager),
    }
}

fn set(chunks: &mut ChunkStore, cell: [i64; 3], material: u16) {
    let (section, local) = global_to_section(cell);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
}

fn cached(
    a: &Creature,
    target: [f64; 3],
    points: impl IntoIterator<Item = [f64; 3]>,
) -> Navigation {
    let mut nav = Navigation::default();
    nav.routes.insert(
        a.id,
        Route {
            target,
            points: points.into_iter().collect(),
            expires: a.age + 100.0,
            ground_blocked: true,
            last_progress: a.position,
            progress_at: a.age,
            stuck_since: a.age,
            arrived: false,
        },
    );
    nav
}

fn follow(
    a: &mut Creature,
    h: &Habitat,
    target: [f64; 3],
    allowed: impl Fn([f64; 3]) -> bool,
) -> (bool, Navigation) {
    let mut nav = Navigation::default();
    let mut next_search = 0.0;
    for _ in 0..1000 {
        a.age += 0.05;
        if ground_arrived(a, h, target, 0.12) {
            return (true, nav);
        }
        let search = a.age >= next_search;
        if search {
            next_search = a.age + 0.4;
        }
        if let Some(point) =
            ground_waypoint_with_access(a, h, target, search, Some(&mut nav), &allowed)
        {
            let before = a.position;
            let length = (point[0] - before[0]).hypot(point[2] - before[2]);
            assert!(
                length > 0.00001,
                "a completed point must not trap locomotion in place"
            );
            let amount = length.min(0.08);
            let step = [
                before[0] + (point[0] - before[0]) / length * amount,
                before[1],
                before[2] + (point[2] - before[2]) / length * amount,
            ];
            let yaw = (before[0] - point[0]).atan2(before[2] - point[2]);
            let accepted = h
                .walk_step_sized(before, step, a.size(), yaw)
                .expect("cached steering must remain physically traversable");
            assert!((accepted[0] - before[0]).hypot(accepted[2] - before[2]) <= 0.080001);
            assert!(allowed(accepted));
            a.position = accepted;
        }
    }
    (ground_arrived(a, h, target, 0.12), nav)
}

#[test]
fn f49_editing_the_middle_of_a_cached_edge_invalidates_it_without_running_an_unbudgeted_search() {
    let (terrain, mut chunks, mut journal) = fixture();
    let a = animal([6.5, 1.0, 8.5]);
    let target = [10.5, 1.0, 8.5];
    let mut nav = cached(&a, target, [target]);
    for y in 1..=3 {
        set(&mut chunks, [8, y, 8], 3);
        journal.record([8, y, 8], 3).unwrap();
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(
        h.ground(target, a.kind).is_some(),
        "the endpoint itself did not become blocked"
    );
    testing::reset_path_calls();
    assert!(ground_waypoint(&a, &h, target, false, Some(&mut nav)).is_none());
    assert_eq!(testing::path_calls(), 0);
    assert_eq!(
        nav.ground_state(a.id, a.age),
        GroundRouteState::AwaitingSearch
    );
    let next = ground_waypoint(&a, &h, target, true, Some(&mut nav)).unwrap();
    assert!(ground::edge(&h, a.position, next, a.size(), &|_| true).is_some());
    assert_eq!(testing::path_calls(), 1);
    let mut a = a;
    let (arrived, _) = follow(&mut a, &h, target, |_| true);
    assert!(arrived, "detour stopped at {:?}", a.position);
}

#[test]
fn f49_off_center_start_physically_crosses_a_single_voxel_doorway_and_arrives_at_the_exact_goal() {
    let (terrain, mut chunks, journal) = fixture();
    for z in 0..16 {
        if z != 8 {
            for y in 1..=3 {
                set(&mut chunks, [8, y, z], 3);
            }
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal([5.1, 1.0, 7.1]);
    let target = [11.1, 1.0, 7.1];
    let (arrived, nav) = follow(&mut a, &h, target, |_| true);
    assert!(
        arrived,
        "narrow doorway was not reached: {:?}, {:?}",
        a.position,
        nav.inspection(&a)
    );
    assert!(distance(a.position, target) < 0.13_f64.powi(2));
}

#[test]
fn f49_same_xz_on_a_different_floor_is_not_arrival_and_a_real_stair_route_returns_to_the_upper_floor()
 {
    let (terrain, mut chunks, journal) = fixture();
    for x in 6..=10 {
        for z in 6..=10 {
            set(&mut chunks, [x, 3, z], 3);
        }
    }
    for (x, height) in [(4, 1), (5, 2), (6, 3)] {
        for y in 1..=height {
            set(&mut chunks, [x, y, 5], 3);
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal([8.5, 1.0, 8.5]);
    let target = [8.5, 4.0, 8.5];
    assert!(!ground_arrived(&a, &h, target, 0.2));
    // Demonstrate each actual stair transition independently, so planner budget
    // regressions cannot be hidden by an accidentally impassable test fixture.
    let stairs = [
        [3.5, 1.0, 5.5],
        [4.5, 2.0, 5.5],
        [5.5, 3.0, 5.5],
        [6.5, 4.0, 5.5],
        [6.5, 4.0, 6.5],
    ];
    for pair in stairs.windows(2) {
        assert!(
            ground::edge(&h, pair[0], pair[1], a.size(), &|_| true).is_some(),
            "physical stair edge {:?} -> {:?}",
            pair[0],
            pair[1]
        );
    }
    let route = ground::search(&h, a.position, target, a.size(), &|_| true);
    assert!(
        route.expanded <= NODE_BUDGET,
        "forward/reverse share one bounded search"
    );
    assert!(
        !route.points.is_empty(),
        "real stair must connect floors within the local budget"
    );
    let (arrived, nav) = follow(&mut a, &h, target, |_| true);
    assert!(
        arrived,
        "stairs stopped at {:?}: {:?}",
        a.position,
        nav.inspection(&a)
    );
    assert_eq!(a.position[1], 4.0);
}

#[test]
fn f49_access_predicate_is_checked_between_nodes_and_alternative_routes_stay_outside_the_claim() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut a = animal([5.5, 1.0, 8.5]);
    let target = [11.5, 1.0, 8.5];
    let allowed = |p: [f64; 3]| !(p[0] >= 7.0 && p[0] < 9.0 && p[2] < 10.0);
    let mut nav = cached(&a, target, [target]);
    assert!(ground_waypoint_with_access(&a, &h, target, false, Some(&mut nav), allowed).is_none());
    let (arrived, nav) = follow(&mut a, &h, target, allowed);
    assert!(
        arrived,
        "claim detour stopped: {:?}, {:?}",
        a.position,
        nav.inspection(&a)
    );
}

#[test]
fn f49_evicted_sections_are_barriers_even_with_saved_air_overrides_and_search_stays_bounded() {
    let (terrain, mut chunks, mut journal) = fixture();
    let a = animal([14.5, 1.0, 8.5]);
    let target = [17.5, 1.0, 8.5];
    let mut nav = cached(&a, target, [target]);
    journal.record([16, 1, 8], 0).unwrap();
    chunks.loaded.remove(&SectionPos::new(1, 0, 0));
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(ground_waypoint(&a, &h, target, false, Some(&mut nav)).is_none());
    let search = ground::search(&h, a.position, target, a.size(), &|_| true);
    assert!(search.expanded <= NODE_BUDGET);
    assert!(search.points.len() <= NODE_BUDGET);
    let mut previous = a.position;
    for point in search.points {
        assert!(point[0] < 16.0);
        assert!(ground::edge(&h, previous, point, a.size(), &|_| true).is_some());
        previous = point;
    }
}

#[test]
fn f49_small_goal_changes_drop_old_routes_and_blocked_retries_obey_the_search_gate() {
    let (terrain, chunks, journal) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let a = animal([6.5, 1.0, 8.5]);
    let old = [10.5, 1.0, 8.5];
    let target = [10.5, 1.0, 9.1];
    let mut nav = cached(&a, old, [[7.5, 1.0, 8.5]]);
    testing::reset_path_calls();
    assert!(ground_waypoint(&a, &h, target, false, Some(&mut nav)).is_none());
    assert_eq!(testing::path_calls(), 0);
    assert_eq!(nav.inspection(&a).destination, Some(target));
    assert!(ground_waypoint(&a, &h, target, true, Some(&mut nav)).is_some());
    assert_eq!(testing::path_calls(), 1);
    nav.invalidate(a.id);
    let forbidden = |_: [f64; 3]| false;
    assert!(ground_waypoint_with_access(&a, &h, target, true, Some(&mut nav), forbidden).is_none());
    let calls = testing::path_calls();
    for _ in 0..20 {
        assert!(
            ground_waypoint_with_access(&a, &h, target, true, Some(&mut nav), forbidden).is_none()
        );
    }
    assert_eq!(testing::path_calls(), calls);
    assert_eq!(nav.ground_state(a.id, a.age), GroundRouteState::Blocked);
}

#[test]
fn f49_valid_long_detours_do_not_request_search_every_tick_after_their_soft_expiry() {
    let (terrain, mut chunks, journal) = fixture();
    let a = animal([5.5, 1.0, 8.5]);
    let target = [11.5, 1.0, 8.5];
    let mut nav = cached(&a, target, [[7.5, 1.0, 8.5], [9.5, 1.0, 8.5], target]);
    nav.routes.get_mut(&a.id).unwrap().expires = a.age - 0.1;
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    testing::reset_path_calls();
    for _ in 0..20 {
        assert!(!nav.needs_search(a.id, a.age));
        assert_eq!(
            ground_waypoint(&a, &h, target, false, Some(&mut nav)),
            Some([7.5, 1.0, 8.5])
        );
    }
    assert_eq!(testing::path_calls(), 0);
    for y in 1..=3 {
        set(&mut chunks, [6, y, 8], 3);
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(ground_waypoint(&a, &h, target, false, Some(&mut nav)).is_none());
    assert!(
        nav.needs_search(a.id, a.age),
        "a newly invalid edge still requests a budgeted retry"
    );
    assert_eq!(testing::path_calls(), 0);
}
