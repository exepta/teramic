use super::*;
use crate::streaming::LoadedSection;
use terra_voxel::{BlockSection, SectionPos};

const CENTER: [i64; 3] = [0, 2, 0];

fn fixture() -> (ChunkStore, EditJournal) {
    let mut chunks = ChunkStore::default();
    for z in -1..=1 {
        for x in -1..=1 {
            let p = SectionPos::new(x, 0, z);
            chunks.loaded.insert(
                p,
                LoadedSection {
                    data: BlockSection::empty(p).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
    }
    (chunks, EditJournal::default())
}

fn block(chunks: &mut ChunkStore, p: [i64; 3], material: u16) {
    let (section, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
}

fn path_square(chunks: &mut ChunkStore) {
    for z in -6..=6 {
        for x in -6..=6 {
            block(chunks, [x, 2, z], PATH);
        }
    }
}

fn mass(journal: &EditJournal) -> u32 {
    journal
        .object_states()
        .filter(|(_, id, _)| roads::is_tar(*id))
        .map(|(_, _, state)| {
            u32::from(
                state
                    .liquid
                    .map_or(roads::tar_amount(state.facing) * 8, |s| s.volume),
            )
        })
        .sum()
}

fn coatings(journal: &EditJournal) -> Vec<([i64; 3], u16, BlockObjectState)> {
    let mut cells: Vec<_> = journal
        .object_states()
        .filter(|(_, id, _)| roads::is_tar(*id))
        .collect();
    cells.sort_unstable_by_key(|(p, _, _)| *p);
    cells
}

#[test]
fn only_requested_soils_make_paths_and_only_clean_paths_accept_a_bucket() {
    let (mut chunks, mut journal) = fixture();
    for material in [1, 2, 4, 42, 306, 307, 400, 401] {
        block(&mut chunks, CENTER, material);
        assert!(!pour(&mut chunks, &mut journal, CENTER).unwrap());
        assert!(
            make_path(&mut chunks, &mut journal, CENTER).unwrap(),
            "{material}"
        );
        assert_eq!(chunks.block_at(CENTER), Some(PATH));
        assert!(pour(&mut chunks, &mut journal, CENTER).unwrap());
        assert!(!pour(&mut chunks, &mut journal, CENTER).unwrap());
        assert_eq!(
            journal.object_state(CENTER).unwrap().facing,
            roads::tar_state(8, 0)
        );
    }
    for material in [0, 3, 5, 12, 27, 50, roads::ASPHALT, roads::DRIED_TAR] {
        block(&mut chunks, CENTER, material);
        assert!(!make_path(&mut chunks, &mut journal, CENTER).unwrap());
        assert!(!pour(&mut chunks, &mut journal, CENTER).unwrap());
    }
    block(&mut chunks, CENTER, PATH);
    block(&mut chunks, [0, 3, 0], 3);
    assert!(!pour(&mut chunks, &mut journal, CENTER).unwrap());
    block(&mut chunks, [0, 3, 0], 0);
    block(&mut chunks, CENTER, 1);
    block(&mut chunks, [0, 3, 0], 3);
    assert!(!make_path(&mut chunks, &mut journal, CENTER).unwrap());
}

#[test]
fn tar_sets_in_the_open_corner_of_a_curved_sidewalk() {
    use terra_core::street;
    let (mut chunks, mut journal) = fixture();
    let arc = [0, 3, 0];
    block(&mut chunks, arc, street::ARC_LEFT);
    block(&mut chunks, CENTER, 4);
    let properties = BlockPropertiesCatalog::default();
    let downward = [0., -1., 0.];
    assert_eq!(
        target(
            &chunks,
            &journal,
            &properties,
            [0.06, 5., 0.06],
            downward,
            false
        ),
        Some(CENTER)
    );
    assert_eq!(
        target(
            &chunks,
            &journal,
            &properties,
            [0.9, 5., 0.9],
            downward,
            false
        ),
        None
    );
    assert!(make_path(&mut chunks, &mut journal, CENTER).unwrap());
    assert_eq!(
        target(
            &chunks,
            &journal,
            &properties,
            [0.06, 5., 0.06],
            downward,
            true
        ),
        Some(CENTER)
    );
    let open = [-1, 2, 0];
    let closed = [1, 2, 0];
    for p in [open, closed] {
        block(&mut chunks, p, PATH);
    }
    assert!(pour(&mut chunks, &mut journal, open).unwrap());
    for _ in 0..31 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    assert_eq!(chunks.block_at(CENTER), Some(roads::DRIED_TAR));
    assert_eq!(chunks.block_at(closed), Some(PATH));
    assert_eq!(chunks.block_at(arc), Some(street::ARC_LEFT));
    assert_eq!(mass(&journal), 64);
}

#[test]
fn placing_a_curve_aligns_both_open_edges_with_the_road() {
    use terra_core::street;
    let (mut chunks, journal) = fixture();
    block(&mut chunks, CENTER, 4);
    block(&mut chunks, [0, 2, -1], PATH);
    block(&mut chunks, [1, 2, 0], roads::DRIED_TAR);
    let hit = terra_voxel::blocks::BlockHit {
        position: CENTER,
        normal: [0, 1, 0],
        distance: 2.,
    };
    for id in [street::ARC_LEFT, street::ARC_RIGHT] {
        let facing = crate::targeting::placement_facing(
            &chunks,
            &journal,
            hit,
            id,
            ([0.1, 5., 0.1], [-0.1, -0.9, 0.1]),
        );
        assert!(street::arc_opens_to(id, facing, [0, 0, -1]));
        assert!(street::arc_opens_to(id, facing, [1, 0, 0]));
    }
}

#[test]
fn a_bucket_spreads_slowly_conserves_fine_units_and_drives_the_actual_drying_material() {
    let (mut chunks, mut journal) = fixture();
    path_square(&mut chunks);
    assert!(pour(&mut chunks, &mut journal, CENTER).unwrap());
    assert_eq!(mass(&journal), 64);
    let cells = wet_tar_cells(&chunks, &journal);
    let step = advance_from(&mut chunks, &mut journal, &mut 0, false, cells).unwrap();
    assert!(step.flowed >= 4 && step.flowed <= 5);
    assert_eq!(mass(&journal), 64);
    assert!(
        coatings(&journal)
            .iter()
            .filter(|(p, _, _)| *p != CENTER)
            .all(|(_, _, s)| s.liquid.unwrap().volume <= 2),
        "the first frontier is a thin paid film, never a full neighboring block"
    );
    advance(&mut chunks, &mut journal).unwrap();
    for _ in 1..31 {
        advance(&mut chunks, &mut journal).unwrap();
        assert_eq!(
            mass(&journal),
            64,
            "flow and hardening must preserve finite volume"
        );
    }
    let coatings: Vec<_> = journal
        .object_states()
        .filter(|(_, id, _)| roads::is_tar(*id))
        .collect();
    assert!(
        coatings.len() == 41,
        "one bucket reaches exactly its four-cell diamond"
    );
    assert!(coatings.iter().all(|(_, id, _)| *id == roads::DRIED_TAR));
    let fine: Vec<_> = coatings
        .iter()
        .map(|(_, _, s)| s.liquid.unwrap().volume)
        .collect();
    assert!(
        fine.iter().max().unwrap() - fine.iter().min().unwrap() <= 1,
        "connected film must be level to one fine volume unit"
    );
    assert!(
        coatings
            .iter()
            .all(|(p, _, s)| p[0].abs_diff(s.liquid.unwrap().origin[0])
                + p[2].abs_diff(s.liquid.unwrap().origin[2])
                <= 4)
    );
    let volumes: Vec<_> = coatings
        .iter()
        .map(|(p, _, s)| {
            let liquid = s.liquid.unwrap();
            assert_eq!(liquid.origin, CENTER);
            assert!(p[0].abs_diff(CENTER[0]) + p[2].abs_diff(CENTER[2]) <= roads::TAR_RANGE);
            liquid.volume
        })
        .collect();
    assert!(
        volumes.iter().max().unwrap() - volumes.iter().min().unwrap() <= 1,
        "connected film levels to within one fine unit"
    );
    assert!(
        coatings
            .iter()
            .all(|(_, _, state)| (1..=8).contains(&roads::tar_amount(state.facing)))
    );
    assert_eq!(
        advance(&mut chunks, &mut journal).unwrap(),
        TarStep::default()
    );
}

#[test]
fn tar_never_leaks_onto_soils_air_dry_road_or_covered_paths() {
    let (mut chunks, mut journal) = fixture();
    for (i, material) in [1, 4, 42, 0].into_iter().enumerate() {
        let d = DIRECTIONS[i];
        block(&mut chunks, [d[0], 2, d[2]], material);
    }
    block(&mut chunks, CENTER, PATH);
    pour(&mut chunks, &mut journal, CENTER).unwrap();
    for _ in 0..5 {
        assert_eq!(advance(&mut chunks, &mut journal).unwrap().flowed, 0);
    }
    for d in DIRECTIONS {
        let p = [d[0], 2, d[2]];
        block(&mut chunks, p, PATH);
        block(&mut chunks, [p[0], 3, p[2]], 3);
    }
    for _ in 0..5 {
        assert_eq!(advance(&mut chunks, &mut journal).unwrap().flowed, 0);
    }
    for d in DIRECTIONS {
        let p = [d[0], 2, d[2]];
        block(&mut chunks, [p[0], 3, p[2]], 0);
        set_coating(&mut chunks, &mut journal, p, roads::DRIED_TAR, 1, 0).unwrap();
    }
    for _ in 0..5 {
        assert_eq!(advance(&mut chunks, &mut journal).unwrap().flowed, 0);
    }
    // Four manually installed dry films retain their own volume and cannot
    // accept or become a conduit for the newly poured bucket.
    assert_eq!(mass(&journal), 96);
    assert_eq!(
        roads::tar_amount(journal.object_state(CENTER).unwrap().facing),
        8
    );
}

#[test]
fn a_covered_liquid_film_still_cures_without_flowing_and_incoming_tar_cannot_reset_its_age() {
    let (mut chunks, mut journal) = fixture();
    path_square(&mut chunks);
    pour(&mut chunks, &mut journal, CENTER).unwrap();
    block(&mut chunks, [0, 3, 0], 3);
    for _ in 0..31 {
        assert_eq!(advance(&mut chunks, &mut journal).unwrap().flowed, 0);
    }
    assert_eq!(mass(&journal), 64);
    assert_eq!(chunks.block_at(CENTER), Some(roads::DRIED_TAR));
    assert_eq!(
        journal
            .object_states()
            .filter(|(_, id, _)| roads::is_tar(*id))
            .count(),
        1
    );

    let (mut chunks, mut journal) = fixture();
    block(&mut chunks, CENTER, PATH);
    block(&mut chunks, [1, 2, 0], PATH);
    pour(&mut chunks, &mut journal, CENTER).unwrap();
    set_coating(&mut chunks, &mut journal, [1, 2, 0], roads::WET_TAR, 1, 29).unwrap();
    assert_eq!(advance(&mut chunks, &mut journal).unwrap().flowed, 0);
    assert!(roads::tar_stage(journal.object_state([1, 2, 0]).unwrap().facing) >= 30);
    advance(&mut chunks, &mut journal).unwrap();
    assert_eq!(chunks.block_at([1, 2, 0]), Some(roads::DRIED_TAR));
    assert_eq!(mass(&journal), 72);
}

#[test]
fn unload_freezes_tar_and_serialized_replica_resumes_the_same_stage_across_section_seams() {
    let (mut chunks, mut journal) = fixture();
    let center = [-1, 2, -1];
    for x in -3..=2 {
        for z in -3..=2 {
            block(&mut chunks, [x, 2, z], PATH);
        }
    }
    pour(&mut chunks, &mut journal, center).unwrap();
    for _ in 0..9 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    let saved: Vec<_> = journal
        .changed_sections()
        .map(|s| serde_json::to_string(&journal.section_delta(s)).unwrap())
        .collect();
    let before = coatings(&journal);
    assert!(before.iter().any(|(p, _, _)| p[0] >= 0));
    assert!(before.iter().any(|(p, _, _)| p[2] >= 0));
    let loaded = std::mem::take(&mut chunks.loaded);
    for _ in 0..100 {
        assert_eq!(advance(&mut chunks, &mut journal).unwrap().visited, 0);
    }
    assert_eq!(coatings(&journal), before);
    chunks.loaded = loaded;
    let (mut replica_chunks, mut replica) = fixture();
    for serialized in saved {
        replica_chunks
            .apply_confirmed_delta(&mut replica, serde_json::from_str(&serialized).unwrap())
            .unwrap();
    }
    assert_eq!(coatings(&replica), before);
    assert_eq!(mass(&replica), 64);
    for _ in 9..31 {
        advance(&mut replica_chunks, &mut replica).unwrap();
    }
    assert_eq!(mass(&replica), 64);
    assert!(
        replica
            .object_states()
            .filter(|(_, id, _)| roads::is_tar(*id))
            .all(|(_, id, _)| id == roads::DRIED_TAR)
    );
}

#[test]
fn packed_tar_stages_validate_all_amounts_and_reject_attachments_or_dry_ages() {
    for stage in 0..=31 {
        for amount in 1..=8 {
            let state = BlockObjectState {
                facing: roads::tar_state(amount, stage),
                ..Default::default()
            };
            assert!(state.valid_for(CENTER, roads::WET_TAR));
            assert_eq!(state.valid_for(CENTER, roads::DRIED_TAR), stage == 0);
            assert!(
                !BlockObjectState {
                    peer: Some([1, 2, 0]),
                    ..state
                }
                .valid_for(CENTER, roads::WET_TAR)
            );
            assert!(
                !BlockObjectState {
                    secondary: true,
                    ..state
                }
                .valid_for(CENTER, roads::WET_TAR)
            );
        }
    }
    assert!(!crate::wrench::supported(roads::WET_TAR));
    assert!(!crate::wrench::supported(roads::DRIED_TAR));
    assert!(!crate::props::placement::ground_support(roads::WET_TAR));
    assert!(!crate::props::placement::ground_support(roads::DRIED_TAR));
}

#[test]
fn preparation_and_pouring_require_the_visible_top_face() {
    let (mut chunks, mut journal) = fixture();
    let properties = BlockPropertiesCatalog::default();
    block(&mut chunks, CENTER, 1);
    let origin = [0.5, 6.0, 0.5];
    assert_eq!(
        target(
            &chunks,
            &journal,
            &properties,
            origin,
            [0.0, -1.0, 0.0],
            false
        ),
        Some(CENTER)
    );
    assert!(make_path(&mut chunks, &mut journal, CENTER).unwrap());
    assert_eq!(
        target(
            &chunks,
            &journal,
            &properties,
            origin,
            [0.0, -1.0, 0.0],
            true
        ),
        Some(CENTER)
    );
    assert_eq!(
        target(
            &chunks,
            &journal,
            &properties,
            [-2.0, 2.5, 0.5],
            [1.0, 0.0, 0.0],
            true
        ),
        None
    );
    assert!(pour(&mut chunks, &mut journal, CENTER).unwrap());
    assert_eq!(
        target(
            &chunks,
            &journal,
            &properties,
            origin,
            [0.0, -1.0, 0.0],
            true
        ),
        None
    );
}

#[test]
fn actual_collision_and_picking_stay_flush_for_every_tar_volume_and_drying_stage() {
    let (mut chunks, mut journal) = fixture();
    let properties = BlockPropertiesCatalog::default();
    for p in [CENTER, [-1, 2, -1]] {
        for material in [roads::WET_TAR, roads::DRIED_TAR] {
            for amount in 1..=8 {
                let ages: &[u8] = if material == roads::WET_TAR {
                    &[0, 17, 31]
                } else {
                    &[0]
                };
                for &age in ages {
                    set_coating(&mut chunks, &mut journal, p, material, amount, age).unwrap();
                    let expected = p[1] as f64 + 1.0;
                    let mut boxes = Vec::new();
                    assert!(crate::interaction_shapes::visit_collision_boxes(
                        &chunks,
                        &journal,
                        &properties,
                        p,
                        |bounds| boxes.push(bounds),
                    ));
                    assert!(!boxes.is_empty());
                    let top = boxes
                        .iter()
                        .map(|bounds| bounds.maximum[1])
                        .fold(f64::NEG_INFINITY, f64::max);
                    assert!(
                        (top - expected).abs() < 1e-9,
                        "{material},amount={amount},age={age},p={p:?}: {top} != {expected}"
                    );
                    let origin = [p[0] as f64 + 0.5, p[1] as f64 + 3.0, p[2] as f64 + 0.5];
                    let hit = crate::interaction_shapes::raycast(
                        &chunks,
                        &journal,
                        &properties,
                        origin,
                        [0.0, -1.0, 0.0],
                        4.0,
                    )
                    .unwrap();
                    assert_eq!(hit.position, p);
                    assert_eq!(hit.normal, [0, 1, 0]);
                    assert!((origin[1] - hit.distance - expected).abs() < 1e-9);
                }
            }
        }
    }
}

#[test]
fn replicas_never_dry_or_spread_tar_without_an_authoritative_update() {
    let (mut chunks, mut journal) = fixture();
    path_square(&mut chunks);
    pour(&mut chunks, &mut journal, CENTER).unwrap();
    let section = global_to_section(CENTER).0;
    let before = journal.section_delta(section);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(chunks)
        .insert_resource(journal)
        .insert_resource(WorldAccess::Replica { ready: true })
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(100),
        ))
        .add_systems(Update, tick);
    for _ in 0..50 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<EditJournal>().section_delta(section),
        before
    );
    *app.world_mut().resource_mut::<WorldAccess>() = WorldAccess::Server;
    app.update();
    let journal = app.world().resource::<EditJournal>();
    assert_eq!(mass(journal), 64);
    assert_ne!(journal.section_delta(section), before);
}
