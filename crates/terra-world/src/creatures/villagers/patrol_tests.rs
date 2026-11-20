use super::*;
use crate::{
    TerraGenerator,
    edits::{BlockObjectState, EditJournal},
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::{
    WorldConfig,
    kingdom::{ClaimActor, KingdomId},
};
use terra_voxel::{BlockSection, SectionPos, global_to_section};

fn fixture() -> (
    TerraGenerator,
    ChunkStore,
    EditJournal,
    Residents,
    Creature,
    PlayerUuid,
    KingdomId,
) {
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
    for x in -1..=1 {
        for z in -1..=1 {
            let section = SectionPos::new(x, 0, z);
            let mut data = BlockSection::empty(section).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    data.set_block([x, 2, z], 3).unwrap();
                }
            }
            chunks.loaded.insert(
                section,
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
    }
    let owner = PlayerUuid::from_u128(303);
    let mut journal = EditJournal::default();
    let mut next = journal.gameplay().kingdoms.clone();
    let realm = next.found(owner, [8, 3, 8], "Patrol Kingdom").unwrap();
    let positions =
        std::array::from_fn(|i| [1.5 + (i % 6) as f64 * 2.0, 3.0, 1.5 + (i / 6) as f64 * 4.0]);
    journal
        .gameplay_mut()
        .commit_founding_with_starters(next, realm, positions)
        .unwrap();
    let id = journal.gameplay().kingdom_starters.grants[&realm].ids[0];
    let mut soldier = journal.gameplay().creatures.animals[&id].clone();
    soldier.position = [8.5, 3.0, 8.5];
    (
        terrain,
        chunks,
        journal,
        Residents::default(),
        soldier,
        owner,
        realm,
    )
}

fn step(
    residents: &Residents,
    soldier: &mut Creature,
    h: &Habitat,
    nav: &mut navigation::Navigation,
    owner: PlayerUuid,
    phase: f64,
) -> Option<[i64; 3]> {
    residents.step_as(soldier, h, phase, 0.1, true, nav, Some(owner))
}

#[test]
fn all_founder_guard_roles_patrol_without_a_resident_site_or_bed_even_at_night() {
    for role in [10, 11, 12] {
        for phase in [0.0, 0.5] {
            let (terrain, chunks, journal, residents, mut soldier, owner, _) = fixture();
            soldier.variant = role;
            let h = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            let mut nav = navigation::Navigation::default();
            let mut travelled = 0.0;
            let mut goals = std::collections::BTreeSet::new();
            for _ in 0..240 {
                let before = soldier.position;
                step(&residents, &mut soldier, &h, &mut nav, owner, phase);
                travelled += distance(before, soldier.position).sqrt();
                if let Some(goal) = soldier.needs.goal {
                    goals.insert(goal.position);
                }
                assert!(
                    Area::new(&soldier, &h, owner)
                        .unwrap()
                        .owns(soldier.position)
                );
                assert!(h.clear_rotated(
                    soldier.position,
                    soldier.kind.size(),
                    false,
                    f64::from(soldier.yaw)
                ));
                assert_ne!(soldier.behavior, Behavior::Sleep);
                assert!(soldier.valid());
            }
            assert!(
                travelled > 10.0,
                "role{role}, phase{phase}: walked{travelled}"
            );
            assert!(
                goals.len() >= 2,
                "the garrison must choose changing sentry destinations"
            );
        }
    }
}

#[test]
fn patrol_uses_build_land_only_and_revalidates_released_or_foreign_claims() {
    let (terrain, chunks, mut journal, residents, mut soldier, owner, realm) = fixture();
    journal
        .gameplay_mut()
        .kingdoms
        .claim(
            realm,
            [1, 0],
            ClaimKind::RealmTerritory,
            ClaimActor::Player(owner),
        )
        .unwrap();
    {
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let area = Area::new(&soldier, &h, owner).unwrap();
        assert!(
            !area.owns([24.5, 3.0, 8.5]),
            "territory is not a Build patrol route"
        );
    }
    journal
        .gameplay_mut()
        .kingdoms
        .claim(
            realm,
            [1, 0],
            ClaimKind::ProtectedBuilding,
            ClaimActor::Player(owner),
        )
        .unwrap();
    soldier.needs.goal = Some(NeedGoal {
        kind: NeedKind::Food,
        position: [24, 3, 8],
    });
    let mut nav = navigation::Navigation::default();
    for _ in 0..20 {
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        step(&residents, &mut soldier, &h, &mut nav, owner, 0.5);
    }
    assert!(soldier.position[0] > 9.5 && soldier.position[0] < 15.0);
    journal
        .gameplay_mut()
        .kingdoms
        .release(owner, realm, [1, 0])
        .unwrap();
    let other = PlayerUuid::from_u128(404);
    journal
        .gameplay_mut()
        .kingdoms
        .found(other, [24, 3, 8], "Foreign Build")
        .unwrap();
    for _ in 0..200 {
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        step(&residents, &mut soldier, &h, &mut nav, owner, 0.5);
        assert!(soldier.position[0] + 0.3 < 16.0);
    }
}

#[test]
fn explicit_commands_unloaded_ground_and_lost_realm_stop_idle_patrol() {
    let (terrain, mut chunks, mut journal, residents, mut soldier, owner, realm) = fixture();
    journal.gameplay_mut().pet_orders.insert(
        soldier.id,
        terra_core::creatures::taming::CompanionMode::Wait,
    );
    let initial = soldier.position;
    let mut nav = navigation::Navigation::default();
    for _ in 0..20 {
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        step(&residents, &mut soldier, &h, &mut nav, owner, 0.5);
    }
    assert_eq!(soldier.position, initial);
    assert!(soldier.needs.goal.is_none());
    journal.gameplay_mut().pet_orders.remove(&soldier.id);
    let loaded = chunks.loaded.remove(&SectionPos::default()).unwrap();
    {
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        step(&residents, &mut soldier, &h, &mut nav, owner, 0.5);
        assert_eq!(soldier.position, initial);
    }
    chunks.loaded.insert(SectionPos::default(), loaded);
    journal
        .gameplay_mut()
        .kingdoms
        .dissolve(owner, realm)
        .unwrap();
    for _ in 0..20 {
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        step(&residents, &mut soldier, &h, &mut nav, owner, 0.5);
    }
    assert_eq!(soldier.position, initial);
    assert_eq!(soldier.behavior, Behavior::Rest);
}

#[test]
fn all_idle_civilian_roles_wander_in_build_land_but_keep_their_night_rest() {
    for role in villagers::CIVILIAN_ROLES {
        let (terrain, chunks, journal, residents, mut civilian, owner, _) = fixture();
        civilian.variant = role;
        let initial = civilian.position;
        let h = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut nav = navigation::Navigation::default();
        let mut travelled = 0.0;
        for _ in 0..120 {
            let previous = civilian.position;
            step(&residents, &mut civilian, &h, &mut nav, owner, 0.5);
            travelled += distance(previous, civilian.position).sqrt();
            assert!(
                Area::new(&civilian, &h, owner)
                    .unwrap()
                    .owns(civilian.position)
            );
        }
        assert!(travelled > 4.0, "role{role} from{initial:?}");
        let night = civilian.position;
        for _ in 0..40 {
            step(&residents, &mut civilian, &h, &mut nav, owner, 0.0);
        }
        assert_eq!(civilian.position, night);
        assert_eq!(civilian.behavior, Behavior::Rest);
    }
}

#[test]
fn a_worker_without_targets_may_wander_but_real_missing_prerequisites_keep_job_priority() {
    use terra_core::npc_jobs::{
        WaitReason, WorkArea, WorkKind, WorkOrder, WorkStatus, WorkerState,
    };
    let (terrain, chunks, mut journal, residents, mut civilian, owner, _) = fixture();
    civilian.variant = villagers::UNEMPLOYED;
    let npc = civilian.id;
    journal.gameplay_mut().jobs.next_id = 2;
    journal.gameplay_mut().jobs.orders.insert(
        1,
        WorkOrder {
            id: 1,
            npc,
            owner,
            kind: WorkKind::Lumberjack,
            area: WorkArea::new([1, 3, 1], [14, 4, 14]).unwrap(),
            storage: [8, 3, 8],
            priority: 5,
            enabled: true,
            revision: 0,
        },
    );
    journal.gameplay_mut().jobs.workers.insert(
        npc,
        WorkerState {
            status: WorkStatus::Waiting(WaitReason::NoTargets),
            ..Default::default()
        },
    );
    assert!(!crate::npc_work::controls_movement(
        journal.gameplay(),
        npc,
        0.5
    ));
    let mut nav = navigation::Navigation::default();
    let before = civilian.position;
    let mut travelled = 0.0;
    let mut furthest = 0.0_f64;
    for _ in 0..100 {
        let previous = civilian.position;
        step(
            &residents,
            &mut civilian,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            &mut nav,
            owner,
            0.5,
        );
        travelled += distance(previous, civilian.position).sqrt();
        furthest = furthest.max(distance(civilian.position, before));
        assert!(
            Area::new(
                &civilian,
                &Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal,
                },
                owner,
            )
            .unwrap()
            .owns(civilian.position)
        );
    }
    assert!(
        travelled > 4.0,
        "a targetless worker must really walk: {travelled}"
    );
    assert!(
        furthest > 1.0,
        "a targetless worker must leave its idle spot"
    );
    for reason in [
        WaitReason::MissingStation,
        WaitReason::MissingTool,
        WaitReason::MissingMaterial,
    ] {
        journal
            .gameplay_mut()
            .jobs
            .workers
            .get_mut(&npc)
            .unwrap()
            .status = WorkStatus::Waiting(reason);
        assert!(crate::npc_work::controls_movement(
            journal.gameplay(),
            npc,
            0.5
        ));
        let before = civilian.position;
        for _ in 0..20 {
            step(
                &residents,
                &mut civilian,
                &Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal,
                },
                &mut nav,
                owner,
                0.5,
            );
        }
        assert_eq!(civilian.position, before);
    }
}

#[test]
fn a_patrol_requests_only_a_loaded_authorized_wooden_door_and_keeps_real_collision() {
    let (terrain, mut chunks, mut journal, residents, mut soldier, owner, _) = fixture();
    for y in 3..=6 {
        for z in 0..16 {
            // The keep is at [8,3,8]; a legitimate doorway must not replace it.
            let cell = [9, y, z];
            let (section, local) = global_to_section(cell);
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block(local, 3)
                .unwrap();
        }
    }
    for (cell, peer, secondary) in [([9, 3, 8], [9, 4, 8], false), ([9, 4, 8], [9, 3, 8], true)] {
        let (section, local) = global_to_section(cell);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 65)
            .unwrap();
        journal.record(cell, 65).unwrap();
        journal
            .set_object_state(
                cell,
                Some(BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    facing: 1,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    soldier.position = [7.5, 3.0, 8.5];
    soldier.needs.goal = Some(NeedGoal {
        kind: NeedKind::Food,
        position: [12, 3, 8],
    });
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut nav = navigation::Navigation::default();
    let request = step(&residents, &mut soldier, &h, &mut nav, owner, 0.5);
    assert_eq!(request, Some([9, 3, 8]));
    assert!(soldier.position[0] < 9.0);
    assert_eq!(
        journal.object_state([9, 3, 8]).unwrap().facing,
        1,
        "only the server transaction opens it"
    );
}

#[test]
fn crowd_separation_cannot_push_an_idle_guard_over_a_build_boundary() {
    let (terrain, chunks, journal, _, mut soldier, owner, realm) = fixture();
    let ids = journal.gameplay().kingdom_starters.grants[&realm].ids;
    soldier.id = ids[1];
    soldier.position = [15.68, 3.0, 8.5];
    let mut other = soldier.clone();
    other.id = ids[0];
    let mut population = journal.gameplay().creatures.clone();
    population.animals.insert(soldier.id, soldier.clone());
    population.animals.insert(other.id, other.clone());
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    navigation::separate(&mut population, &h, &[soldier.id, other.id], 0.1);
    assert_eq!(population.animals[&soldier.id].position, soldier.position);
    assert!(population.animals[&other.id].position[0] < other.position[0]);
    for id in [soldier.id, other.id] {
        let resident = &population.animals[&id];
        assert!(
            Area::new(resident, &h, owner)
                .unwrap()
                .owns(resident.position)
        );
    }
}

#[test]
fn an_edited_block_recovers_a_guard_inside_build_land_when_the_shortest_exit_is_outside() {
    let (terrain, mut chunks, mut journal, residents, mut soldier, owner, _) = fixture();
    soldier.position = [15.68, 3.0, 8.5];
    soldier.yaw = 0.0;
    let obstruction = [15, 3, 8];
    let (section, local) = global_to_section(obstruction);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 3)
        .unwrap();
    journal.record(obstruction, 3).unwrap();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(!h.clear_rotated(
        soldier.position,
        soldier.size(),
        false,
        f64::from(soldier.yaw)
    ));
    let mut unconstrained = soldier.clone();
    support::settle(&mut unconstrained, &h, 0.1);
    assert!(
        unconstrained.position[0] > 16.0,
        "the fixture must expose the shorter outward recovery"
    );
    let initial = soldier.position;
    let mut nav = navigation::Navigation::default();
    step(&residents, &mut soldier, &h, &mut nav, owner, 0.5);
    assert!(
        Area::new(&soldier, &h, owner)
            .unwrap()
            .owns(soldier.position)
    );
    assert!(h.clear_rotated(
        soldier.position,
        soldier.size(),
        false,
        f64::from(soldier.yaw)
    ));
    assert!(distance(initial, soldier.position) <= 1.001_f64.powi(2));
    assert!(soldier.position[0] + 0.3 < 16.0);
}
