use super::*;
use crate::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
use terra_voxel::{BlockSection, MaterialId, SectionPos};

fn deep_pool() -> (TerraGenerator, ChunkStore, EditJournal) {
    let (old, mut chunks, journal) = tests::fixture();
    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        sea_level: 45.0,
        ..old.config().clone()
    })
    .unwrap();
    for y in 1..=2 {
        for z in -1..=1 {
            for x in -1..=1 {
                let pos = SectionPos::new(x, y, z);
                chunks.loaded.insert(
                    pos,
                    LoadedSection {
                        data: BlockSection::empty(pos).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: vec![],
                    },
                );
            }
        }
    }
    (terrain, chunks, journal)
}

#[test]
fn authoritative_aquatics_including_fish_leave_the_surface_and_keep_the_whole_body_wet() {
    let (terrain, chunks, journal) = deep_pool();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    for kind in Kind::ALL.into_iter().filter(|kind| kind.is_aquatic()) {
        let mut a = tests::animal(kind, 0);
        a.position[1] = 45.0 - a.size()[1] - 0.15;
        let start = a.position[1];
        let mut lowest = start;
        for _ in 0..300 {
            // The production dispatch is intentional: Fish uses its existing
            // locomotion/strand path while the other species use aquatics::step.
            crate::creatures::step(&mut a, &h, &[], 0.1);
            lowest = lowest.min(a.position[1]);
            assert!(
                h.clear_rotated(a.position, a.size(), true, f64::from(a.yaw)),
                "{kind:?}: {a:?}"
            );
            assert!(a.valid(), "{kind:?}: {a:?}");
        }
        assert!(
            start - lowest > 3.0,
            "{kind:?} stayed at the surface: {start} -> {lowest}"
        );
        assert!(a.position[1] >= 3.0, "no animal may enter the floor");
    }
}

#[test]
fn depth_course_and_submerged_motion_survive_population_save_reload() {
    let (terrain, chunks, journal) = deep_pool();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    for kind in [Kind::Fish, Kind::Squid, Kind::WhaleShark, Kind::MoonJelly] {
        let mut a = tests::animal(kind, 0);
        a.position[1] = 22.0;
        a.age = 71.25;
        let mut population = CreaturePopulation::default();
        population.explored.insert(a.home);
        population.animals.insert(a.id, a.clone());
        let bytes = serde_json::to_vec(&population).unwrap();
        let mut restored: CreaturePopulation = serde_json::from_slice(&bytes).unwrap();
        assert!(restored.valid());
        let b = restored.animals.get_mut(&a.id).unwrap();
        assert_eq!(depth_goal(&a, &h), depth_goal(b, &h));
        for _ in 0..60 {
            crate::creatures::step(&mut a, &h, &[], 0.1);
            crate::creatures::step(b, &h, &[], 0.1);
            assert_eq!(a.position, b.position);
            assert_eq!(a.velocity, b.velocity);
            assert_eq!(a.behavior, b.behavior);
            assert!(h.clear_rotated(b.position, b.size(), true, f64::from(b.yaw)));
        }
        assert!(
            b.position[1] < 35.0,
            "reload must not snap a deep swimmer to the surface"
        );
    }
}

#[test]
fn edited_floors_roofs_and_unloaded_depths_bound_actual_water_courses() {
    let (terrain, mut chunks, mut journal) = deep_pool();
    let mut a = tests::animal(Kind::Squid, 0);
    a.position[1] = 22.0;
    // Use the real authority transaction: a journal-only edit is not an
    // installed world change, because connected-water queries read ChunkStore.
    let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
    for x in 0..16 {
        for z in 0..16 {
            for y in [18, 28] {
                assert!(
                    crate::streaming::commit_block_edit(
                        &mut chunks,
                        &mut journal,
                        &crate::streaming::BlockEditRequest {
                            position: [x, y, z],
                            expected_material: 0,
                            material: MaterialId::Stone as u16,
                        },
                        &properties,
                    )
                    .unwrap()
                );
            }
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let column = chunks
        .water_column_at_point(&terrain, &journal, center(&a))
        .unwrap();
    assert_eq!(column.bed, 19.0);
    assert_eq!(column.level, 28.0);
    for age in [0., 35., 70., 105.] {
        a.age = age;
        let y = depth_goal(&a, &h).unwrap();
        assert!(y >= 19.2 && y + a.size()[1] < 28.0);
    }
    assert!(
        !swim_step(&a, &h, [8., 17., 8.], a.yaw),
        "cannot tunnel through the edited floor"
    );
    assert!(
        !swim_step(&a, &h, [8., 30., 8.], a.yaw),
        "cannot tunnel through the edited roof"
    );
    // A separate intact reservoir isolates the unloaded-section condition from
    // the previously installed player floor and roof.
    let (terrain, mut chunks, journal) = deep_pool();
    for z in -1..=1 {
        for x in -1..=1 {
            chunks.loaded.remove(&SectionPos::new(x, 0, z));
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert_eq!(
        chunks
            .water_column_at_point(&terrain, &journal, center(&a))
            .unwrap()
            .bed,
        16.0
    );
    assert!(depth_goal(&a, &h).unwrap() >= 16.2);
    assert!(
        !swim_step(&a, &h, [8., 12., 8.], a.yaw),
        "unknown depth is a barrier, not generated on demand"
    );
}

#[test]
fn rotated_long_bodies_and_partial_surface_cells_are_checked_during_the_sweep() {
    let (terrain, chunks, mut journal) = tests::fixture();
    let mut a = tests::animal(Kind::WhaleShark, 0);
    a.position = [8., 5., 8.];
    let half_width = a.size()[0] * 0.5;
    let half_length = a.size()[2] * 0.5;
    assert!(half_length > half_width);
    let obstruction_x = (a.position[0] + half_length - 0.05).floor() as i64;
    journal
        .record([obstruction_x, 5, 8], MaterialId::Stone as u16)
        .unwrap();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    // Choose a non-overlapping initial offset, then rotate the full long body.
    a.position[0] = obstruction_x as f64 - half_width - 0.05;
    assert!(h.clear_rotated(a.position, a.size(), true, 0.0));
    assert!(!swim_step(&a, &h, a.position, std::f32::consts::FRAC_PI_2));
    journal = EditJournal::default();
    for x in 0..16 {
        for z in 0..16 {
            journal
                .set_fluid(
                    [x, 12, z],
                    crate::water::FluidCell {
                        amount: 2,
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
    a.position = [8., 10., 8.];
    let target = [8., 12.3 - a.size()[1], 8.];
    assert!(
        !swim_step(&a, &h, target, 0.0),
        "even the top of a partial cell cannot leave the body in air"
    );
}

#[test]
fn dry_corners_outside_a_rotated_body_do_not_block_diagonal_swimming() {
    let (terrain, chunks, mut journal) = tests::fixture();
    let position = [8.0, 5.0, 8.0];
    let size = [1.0, 1.0, 5.0];
    let yaw = std::f64::consts::FRAC_PI_4;
    // This cell intersects the enclosing AABB but not the actual diagonal OBB.
    // Its dry fluid and stone must both be ignored; a central stone must not.
    journal.record([5, 5, 5], MaterialId::Stone as u16).unwrap();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(h.clear_rotated(position, size, true, yaw));
    journal.record([8, 5, 8], MaterialId::Stone as u16).unwrap();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(!h.clear_rotated(position, size, true, yaw));
}

#[test]
fn fish_detours_obey_the_navigation_search_grant() {
    let (terrain, chunks, mut journal) = tests::fixture();
    for x in 0..16 {
        for y in 3..13 {
            journal.record([x, y, 7], MaterialId::Stone as u16).unwrap();
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut fish = tests::animal(Kind::Fish, 0);
    fish.position = [8.5, 5.0, 8.4];
    fish.timer = 10.0;
    assert!(h.clear_rotated(fish.position, fish.size(), true, 0.0));
    for allowed in [false, true] {
        let mut a = fish.clone();
        crate::creatures::navigation::testing::reset_path_calls();
        crate::creatures::step_budgeted(&mut a, &h, &[], 0.25, allowed);
        assert_eq!(
            crate::creatures::navigation::testing::path_calls(),
            usize::from(allowed)
        );
        assert!(h.clear_rotated(a.position, a.size(), true, f64::from(a.yaw)));
        if allowed {
            assert_eq!(a.needs.search_in, 2.0);
        }
    }
}

#[test]
fn timer_turns_sweep_from_the_previous_fish_orientation() {
    let (terrain, chunks, mut journal) = tests::fixture();
    journal.record([8, 5, 8], MaterialId::Stone as u16).unwrap();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut fish = tests::animal(Kind::Fish, 0);
    fish.position = [7.76, 5.0, 7.76];
    fish.timer = 0.0;
    assert!(h.clear_rotated(fish.position, fish.size(), true, 0.0));
    assert!(h.clear_rotated(
        fish.position,
        fish.size(),
        true,
        std::f64::consts::FRAC_PI_2
    ));
    assert!(!h.clear_rotated(
        fish.position,
        fish.size(),
        true,
        std::f64::consts::FRAC_PI_4
    ));
    let dt = 0.1_f32;
    let (age, desired) = (0..1000)
        .find_map(|i| {
            let age = f64::from(i) * 0.1;
            let seed = random(fish.id, ((age + f64::from(dt)) * 10.0) as u64);
            let yaw = (seed % 628) as f32 * 0.01;
            ((1.5..=1.65).contains(&yaw)
                && h.clear_rotated(fish.position, fish.size(), true, f64::from(yaw)))
            .then_some((age, yaw))
        })
        .expect("bounded deterministic timer turn near ninety degrees");
    assert!(!swim_step(&fish, &h, fish.position, desired));
    fish.age = age;
    crate::creatures::step_budgeted(&mut fish, &h, &[], dt, false);
    assert_eq!(
        fish.yaw, 0.0,
        "a clear final angle cannot bypass a colliding intermediate angle"
    );
    assert!(
        fish.position[2] < 7.76,
        "the safe old heading remains usable"
    );
    assert!(h.clear_rotated(fish.position, fish.size(), true, f64::from(fish.yaw)));
}
