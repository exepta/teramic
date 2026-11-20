use super::*;
use crate::{edits::BlockObjectState, streaming::commit_object_state};

fn doorway(chunks: &mut ChunkStore, journal: &mut EditJournal, material: u16) {
    for y in 1..=4 {
        for z in -4..=4 {
            set(chunks, [5, y, z], 3);
        }
    }
    for (position, peer, secondary) in [([5, 1, 0], [5, 2, 0], false), ([5, 2, 0], [5, 1, 0], true)]
    {
        set(chunks, position, material);
        journal.record(position, material).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    facing: 1,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
}

fn collector_drop(journal: &mut EditJournal, position: [f64; 3]) {
    assert!(journal.gameplay_mut().drop_stack(
        position,
        ItemStack {
            item: ItemKind::Block(3),
            count: 7
        },
        [0.0; 3],
        0.0
    ));
}

#[test]
fn collector_requests_a_real_authorized_door_then_walks_through_and_delivers() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Collector);
    doorway(&mut chunks, &mut journal, 65);
    collector_drop(&mut journal, [8.5, 1.2, 0.5]);
    let mut request = None;
    for _ in 0..120 {
        run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
        let requests = runtime.drain_door_requests().collect::<Vec<_>>();
        assert!(requests.len() <= WORKER_BUDGET);
        if let Some(first) = requests.first() {
            request = Some(*first);
            break;
        }
        assert!(journal.gameplay().creatures.animals[&npc].position[0] < 5.5);
    }
    let request = request.expect("worker must reach and request the closed wooden doorway");
    assert_eq!(request.npc, npc);
    assert_eq!(request.owner, PlayerUuid::from_u128(1));
    assert_eq!(request.position, [5, 1, 0]);
    assert_eq!(journal.object_state(request.position).unwrap().facing, 1);
    assert_eq!(journal.gameplay().drops.len(), 1);
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(3)),
        0
    );
    // Emulate the authoritative network transaction's confirmed paired poses.
    // Until these actual edits occur, the navigation has no geometry exemption.
    for cell in [[5, 1, 0], [5, 2, 0]] {
        let state = journal.object_state(cell).unwrap();
        commit_object_state(
            &mut chunks,
            &mut journal,
            cell,
            Some(BlockObjectState {
                facing: state.facing + 4,
                ..state
            }),
        )
        .unwrap();
    }
    let mut crossed = false;
    for _ in 0..450 {
        run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
        let creature = &journal.gameplay().creatures.animals[&npc];
        crossed |= creature.position[0] > 6.0;
        assert!(
            Habitat {
                chunks: &chunks,
                journal: &journal,
                terrain: &terrain
            }
            .clear(creature.position, creature.size(), false)
        );
    }
    assert!(crossed);
    assert!(journal.gameplay().drops.is_empty());
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(3)),
        7
    );
}

#[test]
fn closed_unoperable_or_incomplete_doors_never_allow_remote_work_or_open_requests() {
    for material in [65, 248] {
        let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Collector);
        doorway(&mut chunks, &mut journal, material);
        if material == 65 {
            journal.set_object_state([5, 2, 0], None).unwrap();
        }
        collector_drop(&mut journal, [8.5, 1.2, 0.5]);
        let mut waiting = false;
        for _ in 0..220 {
            run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
            assert!(runtime.drain_door_requests().next().is_none());
            let creature = &journal.gameplay().creatures.animals[&npc];
            assert!(
                creature.position[0] < 5.5,
                "passed closed material {material}"
            );
            let worker = &journal.gameplay().jobs.workers[&npc];
            assert_ne!(worker.status, WorkStatus::Working);
            waiting |= worker.status == WorkStatus::Waiting(WaitReason::NoPath);
        }
        assert!(
            waiting,
            "an unreachable doorway must expose a blocked status"
        );
        assert_eq!(journal.gameplay().drops.len(), 1);
        assert_eq!(
            journal
                .container_at([0, 1, 0])
                .unwrap()
                .count(ItemKind::Block(3)),
            0
        );
    }
}

#[test]
fn worker_cannot_collect_while_still_falling_but_resumes_after_physical_landing() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Collector);
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap()
        .position = [6.5, 2.0, 0.5];
    collector_drop(&mut journal, [7.5, 1.2, 0.5]);
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
    assert_eq!(journal.gameplay().drops.len(), 1);
    assert_ne!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Working
    );
    assert_eq!(
        journal.gameplay().jobs.workers[&npc]
            .inventory
            .count(ItemKind::Block(3)),
        0
    );
    assert!(journal.gameplay().creatures.animals[&npc].position[1] < 2.0);
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 300);
    assert!(journal.gameplay().drops.is_empty());
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(3)),
        7
    );
}

#[test]
fn collector_routes_around_foreign_claims_with_real_cargo_instead_of_crossing_and_rolling_back() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Collector);
    for x in -4..=36 {
        for z in -4..=4 {
            for y in -1..=4 {
                set(&mut chunks, [x, y, z], if y <= 0 { 3 } else { 0 });
            }
        }
    }
    set(&mut chunks, [0, 1, 0], 29);
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap()
        .position = [14.5, 1.0, 0.5];
    journal.gameplay_mut().jobs.orders.get_mut(&1).unwrap().area =
        WorkArea::new([32, 1, 0], [35, 2, 2]).unwrap();
    journal
        .gameplay_mut()
        .kingdoms
        .found(PlayerUuid::from_u128(2), [24, 1, 8], "Foreign")
        .unwrap();
    collector_drop(&mut journal, [34.5, 1.2, 0.5]);
    let mut detoured = false;
    for _ in 0..1400 {
        run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
        let creature = &journal.gameplay().creatures.animals[&npc];
        let cell = creature.position.map(|v| v.floor() as i64);
        assert!(crate::kingdom_access::can_use(
            &journal.gameplay().kingdoms,
            &chunks,
            PlayerUuid::from_u128(1),
            KingdomPermission::NpcWork,
            cell
        ));
        if (16.0..32.0).contains(&creature.position[0]) {
            assert!(creature.position[2] < 0.0);
            detoured = true;
        }
    }
    assert!(
        detoured,
        "must find the authorized strip south of the foreign claim"
    );
    assert!(journal.gameplay().drops.is_empty());
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(3)),
        7
    );
}

#[test]
fn unloaded_body_stops_work_without_changing_cargo_and_resumes_after_reload() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Collector);
    collector_drop(&mut journal, [7.5, 1.2, 0.5]);
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 4);
    let position = journal.gameplay().creatures.animals[&npc].position;
    let section = global_to_section(position.map(|v| v.floor() as i64)).0;
    let data = chunks.loaded.remove(&section).unwrap();
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 10);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::Unloaded)
    );
    assert_eq!(
        journal.gameplay().creatures.animals[&npc].position,
        position
    );
    assert_eq!(journal.gameplay().drops.len(), 1);
    chunks.loaded.insert(section, data);
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 350);
    assert!(journal.gameplay().drops.is_empty());
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(3)),
        7
    );
}

#[test]
fn collector_recovers_real_partial_terrace_contact_and_uses_the_open_stairs() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Collector);
    for x in 5..=8 {
        for z in -2..=2 {
            for y in 1..=5 {
                set(&mut chunks, [x, y, z], 3);
            }
        }
    }
    // The five real descending steps are north of the visually open terrace.
    // A direct five-block drop is not a legal walking edge.
    for x in 1..=4 {
        for y in 1..=x {
            set(&mut chunks, [x, y, -2], 3);
        }
    }
    let actor = journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap();
    actor.position = [4.67, 6.0, 0.5];
    actor.yaw = std::f32::consts::FRAC_PI_4;
    journal.gameplay_mut().jobs.orders.get_mut(&1).unwrap().area =
        WorkArea::new([1, 1, 0], [3, 2, 2]).unwrap();
    collector_drop(&mut journal, [2.5, 1.2, 0.5]);
    let mut reached_ledge_center = false;
    let mut used_stairs = false;
    for _ in 0..900 {
        let old = journal.gameplay().creatures.animals[&npc].position;
        run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
        let p = journal.gameplay().creatures.animals[&npc].position;
        assert!(
            (p[0] - old[0]).hypot(p[2] - old[2]) <= 0.14,
            "normal walking speed, no teleport"
        );
        reached_ledge_center |= p[0] > 5.1 && p[1] > 5.9;
        used_stairs |= p[2] < -1.0 && p[1] > 1.1 && p[1] < 6.0;
        if journal.gameplay().drops.is_empty()
            && journal
                .container_at([0, 1, 0])
                .unwrap()
                .count(ItemKind::Block(3))
                == 7
        {
            break;
        }
    }
    assert!(
        reached_ledge_center && used_stairs,
        "{:?} at {:?}",
        journal.gameplay().jobs.workers[&npc],
        journal.gameplay().creatures.animals[&npc].position
    );
    assert!(journal.gameplay().drops.is_empty());
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(3)),
        7
    );
}
#[test]
fn worker_physically_finishes_the_last_two_centimetres_of_a_supported_stance() {
    let (chunks, journal, terrain, _, npc) = fixture(WorkKind::Collector);
    let mut actor = journal.gameplay().creatures.animals[&npc].clone();
    actor.position = [2.48, 1.0, 0.5];
    let mut navigation = crate::creatures::navigation::Navigation::default();
    let h = Habitat {
        chunks: &chunks,
        journal: &journal,
        terrain: &terrain,
    };
    assert!(crate::creatures::work_step_with_access(
        &mut actor,
        &h,
        [2.5, 1.0, 0.5],
        0.1,
        &mut navigation,
        |_| true
    ));
    assert!((actor.position[0] - 2.5).abs() < 0.000001);
    assert_eq!(actor.position[1], 1.0);
    assert!((actor.age - 0.1).abs() < 0.000001);
}
#[test]
fn open_ground_high_target_uses_the_actual_eye_reach_instead_of_feet_height() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Collector);
    journal.gameplay_mut().jobs.orders.get_mut(&1).unwrap().area =
        WorkArea::new([5, 1, 0], [8, 5, 2]).unwrap();
    collector_drop(&mut journal, [7.5, 4.2, 0.5]);
    for _ in 0..400 {
        run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
        if journal.gameplay().drops.is_empty()
            && journal
                .container_at([0, 1, 0])
                .unwrap()
                .count(ItemKind::Block(3))
                == 7
        {
            break;
        }
    }
    assert!(
        journal.gameplay().drops.is_empty(),
        "{:?}",
        journal.gameplay().jobs.workers[&npc]
    );
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(3)),
        7
    );
}

#[test]
fn a_fresh_work_goal_searches_once_without_inheriting_another_goals_cooldown() {
    use crate::creatures::{navigation::GroundRouteState, work_step};
    let (mut chunks, journal, terrain, _, npc) = fixture(WorkKind::Collector);
    for z in -4..=4 {
        for y in 1..=4 {
            set(&mut chunks, [3, y, z], 3);
        }
    }
    let habitat = Habitat {
        chunks: &chunks,
        journal: &journal,
        terrain: &terrain,
    };
    let mut actor = journal.gameplay().creatures.animals[&npc].clone();
    actor.needs.search_in = 0.7;
    let mut navigation = Navigation::default();
    crate::creatures::navigation::testing::reset_path_calls();
    let before = actor.position;
    assert!(!work_step(
        &mut actor,
        &habitat,
        [4.5, 1.0, 0.5],
        0.1,
        &mut navigation
    ));
    assert_eq!(actor.position, before);
    assert_eq!(
        navigation.ground_state(npc, actor.age),
        GroundRouteState::Blocked
    );
    assert_eq!(crate::creatures::navigation::testing::path_calls(), 1);
    assert!(!work_step(
        &mut actor,
        &habitat,
        [4.5, 1.0, 0.5],
        0.1,
        &mut navigation
    ));
    assert_eq!(
        crate::creatures::navigation::testing::path_calls(),
        1,
        "a cached failure keeps its ordinary retry interval"
    );
}
