use super::*;
use terra_core::{FruitPhase, FruitState, PlayerUuid, TreeSpecies};
fn tree(id: u64) -> TreeRecord {
    TreeRecord {
        id,
        root: [8, 1, 8],
        species: TreeSpecies::Apple,
        seed: id,
        planted_by: Some(PlayerUuid::from_u128(1)),
        stage: TreeStage::Generated,
        growth_seconds: 1400.,
        bounds: None,
        wood: vec![[0, 0, 0], [0, 1, 0]],
        leaves: vec![[1, 3, 0], [-1, 3, 0]],
        fruit: Some(FruitState {
            phase: FruitPhase::Ripe,
            remaining: 8,
            picked: 0,
            cycle_day: 0,
            last_harvest_day: None,
            revision: 0,
        }),
        revision: 1,
    }
    .with_bounds()
}
fn meta(mut tree: TreeRecord) -> TreeRecord {
    tree.wood.clear();
    tree.leaves.clear();
    tree
}
fn patch(revision: u64, reset: bool, record: TreeUpdate) -> TreePatch {
    TreePatch {
        revision,
        reset,
        removals: vec![],
        records: vec![record],
    }
}
fn fixture() -> (ClientSession, EditJournal, ChunkStore) {
    let session = ClientSession {
        player: Some(PlayerId(1)),
        ..Default::default()
    };
    let mut chunks = ChunkStore::default();
    let position = terra_voxel::SectionPos::new(0, 0, 0);
    chunks.loaded.insert(
        position,
        terra_world::streaming::LoadedSection {
            data: terra_voxel::BlockSection::empty(position).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![],
        },
    );
    (session, EditJournal::default(), chunks)
}
#[test]
fn ordered_patches_preserve_geometry_on_metadata_and_apply_exact_removed_cells() {
    let (mut session, mut journal, mut chunks) = fixture();
    let original = tree(1);
    let first = patch(
        1,
        true,
        TreeUpdate {
            tree: original.clone(),
            geometry: TreeGeometryUpdate::Full,
        },
    );
    assert_eq!(
        apply(first.clone(), &mut session, &mut journal, &mut chunks),
        Ok(Some(true))
    );
    let section = terra_voxel::SectionPos::new(0, 0, 0);
    let epoch = journal.mesh_epoch(section);
    let session_cells = session.trees.get(1).unwrap().leaves.as_ptr();
    let mirror_cells = journal.gameplay().trees.get(1).unwrap().leaves.as_ptr();
    assert_eq!(
        apply(first.clone(), &mut session, &mut journal, &mut chunks),
        Ok(None)
    );
    let mut next = meta(original.clone());
    next.fruit.as_mut().unwrap().remaining = 7;
    next.fruit.as_mut().unwrap().picked = 1;
    next.fruit.as_mut().unwrap().revision = 1;
    next.revision += 1;
    let update = patch(
        2,
        false,
        TreeUpdate {
            tree: next.clone(),
            geometry: TreeGeometryUpdate::Unchanged,
        },
    );
    assert_eq!(
        apply(update.clone(), &mut session, &mut journal, &mut chunks),
        Ok(Some(false))
    );
    assert_eq!(session.trees.get(1).unwrap().leaves.as_ptr(), session_cells);
    assert_eq!(
        journal.gameplay().trees.get(1).unwrap().leaves.as_ptr(),
        mirror_cells
    );
    assert_eq!(journal.mesh_epoch(section), epoch);
    assert_eq!(
        apply(first, &mut session, &mut journal, &mut chunks),
        Ok(None)
    );
    let mut conflict = update;
    conflict.records[0].tree.revision += 1;
    assert!(apply(conflict, &mut session, &mut journal, &mut chunks).is_err());
    let mut expected = session.trees.get(1).unwrap().clone();
    let removed = expected.leaves.remove(0);
    expected.refresh_bounds();
    expected.revision += 1;
    let update = patch(
        3,
        false,
        TreeUpdate {
            tree: meta(expected.clone()),
            geometry: TreeGeometryUpdate::Removed {
                wood: vec![],
                leaves: vec![removed],
            },
        },
    );
    assert_eq!(
        apply(update, &mut session, &mut journal, &mut chunks),
        Ok(Some(true))
    );
    assert_eq!(session.trees.get(1), Some(&expected));
    assert_eq!(journal.gameplay().trees.get(1), Some(&expected));
    expected.wood.remove(0);
    expected.stage = TreeStage::GeneratedFelled;
    expected.fruit = None;
    expected.refresh_bounds();
    expected.revision += 1;
    let update = patch(
        4,
        false,
        TreeUpdate {
            tree: meta(expected.clone()),
            geometry: TreeGeometryUpdate::Removed {
                wood: vec![[0, 0, 0]],
                leaves: vec![],
            },
        },
    );
    assert_eq!(
        apply(update, &mut session, &mut journal, &mut chunks),
        Ok(Some(true))
    );
    assert_eq!(session.trees.get(1), Some(&expected));
    assert!(session.trees.get(1).unwrap().stage.is_generated());
}
#[test]
fn malformed_patch_does_not_partially_change_metadata_geometry_or_epochs() {
    let (mut session, mut journal, mut chunks) = fixture();
    apply(
        patch(
            1,
            true,
            TreeUpdate {
                tree: tree(1),
                geometry: TreeGeometryUpdate::Full,
            },
        ),
        &mut session,
        &mut journal,
        &mut chunks,
    )
    .unwrap();
    for mode in 0..6 {
        let before = session.trees.clone();
        let mirror = journal.gameplay().trees.clone();
        let epoch = journal.mesh_epoch(terra_voxel::SectionPos::new(0, 0, 0));
        let mut next = meta(tree(1));
        next.revision += 1;
        let mut update = patch(
            2,
            false,
            TreeUpdate {
                tree: next,
                geometry: TreeGeometryUpdate::Unchanged,
            },
        );
        match mode {
            0 => update.records.push(TreeUpdate {
                tree: meta(tree(2)),
                geometry: TreeGeometryUpdate::Unchanged,
            }),
            1 => {
                update.records[0].geometry = TreeGeometryUpdate::Removed {
                    wood: vec![[9, 9, 9]],
                    leaves: vec![],
                }
            }
            2 => update.records[0].tree.seed += 1,
            3 => update.records[0].tree.bounds = None,
            4 => update.revision = 3,
            _ => update.records.push(update.records[0].clone()),
        }
        assert!(
            apply(update, &mut session, &mut journal, &mut chunks).is_err(),
            "scenario {mode}"
        );
        assert_eq!(session.trees, before);
        assert_eq!(journal.gameplay().trees, mirror);
        assert_eq!(
            journal.mesh_epoch(terra_voxel::SectionPos::new(0, 0, 0)),
            epoch
        );
    }
}
#[test]
fn nonreset_partial_interest_keeps_previously_received_large_records() {
    let (mut session, mut journal, mut chunks) = fixture();
    for id in 1..=3 {
        let mut tree = tree(id);
        let mut cells = (0..20_000)
            .map(|n| {
                [
                    (n % 32) as i8 - 16,
                    (n / 1024) as i8,
                    (n / 32 % 32) as i8 - 16,
                ]
            })
            .collect::<Vec<_>>();
        tree.leaves = cells.split_off(4096);
        tree.wood = cells;
        assert_eq!((tree.wood.len(), tree.leaves.len()), (4096, 15_904));
        tree.refresh_bounds();
        assert!(tree.valid());
        apply(
            patch(
                id,
                id == 1,
                TreeUpdate {
                    tree,
                    geometry: TreeGeometryUpdate::Full,
                },
            ),
            &mut session,
            &mut journal,
            &mut chunks,
        )
        .unwrap();
        assert_eq!(session.trees.trees.len(), id as usize);
    }
    let update = TreePatch {
        revision: 4,
        reset: false,
        removals: vec![2],
        records: vec![],
    };
    apply(update, &mut session, &mut journal, &mut chunks).unwrap();
    assert_eq!(
        session.trees.trees.keys().copied().collect::<Vec<_>>(),
        vec![1, 3]
    );
}

#[test]
fn late_confirmed_section_import_preserves_full_tree_mirror_for_following_metadata_patch() {
    let (mut session, mut journal, mut chunks) = fixture();
    let original = tree(1);
    apply(
        patch(
            1,
            true,
            TreeUpdate {
                tree: original.clone(),
                geometry: TreeGeometryUpdate::Full,
            },
        ),
        &mut session,
        &mut journal,
        &mut chunks,
    )
    .unwrap();
    let mut authority = EditJournal::default();
    for (offset, material) in original
        .wood
        .iter()
        .map(|offset| (*offset, original.species.wood()))
        .chain(
            original
                .leaves
                .iter()
                .map(|offset| (*offset, original.species.leaves())),
        )
    {
        authority.record(original.global(offset), material).unwrap();
    }
    let section = terra_voxel::SectionPos::new(0, 0, 0);
    let delta = authority.section_delta(section);
    let session_cells = session.trees.get(1).unwrap().wood.as_ptr();
    let mirror_cells = journal.gameplay().trees.get(1).unwrap().wood.as_ptr();
    chunks
        .apply_confirmed_delta(&mut journal, delta.clone())
        .unwrap();
    assert_eq!(
        journal.gameplay().trees,
        session.trees,
        "section snapshots must not invent local tree removals"
    );
    let mut next = meta(original.clone());
    next.fruit.as_mut().unwrap().picked = 1;
    next.fruit.as_mut().unwrap().remaining = 7;
    next.fruit.as_mut().unwrap().revision += 1;
    next.revision += 1;
    let epoch = journal.mesh_epoch(section);
    assert_eq!(
        apply(
            patch(
                2,
                false,
                TreeUpdate {
                    tree: next,
                    geometry: TreeGeometryUpdate::Unchanged
                }
            ),
            &mut session,
            &mut journal,
            &mut chunks
        ),
        Ok(Some(false))
    );
    assert_eq!(journal.gameplay().trees, session.trees);
    assert_eq!(journal.mesh_epoch(section), epoch);
    assert_eq!(session.trees.get(1).unwrap().wood.as_ptr(), session_cells);
    assert_eq!(
        journal.gameplay().trees.get(1).unwrap().wood.as_ptr(),
        mirror_cells
    );
    chunks.apply_confirmed_delta(&mut journal, delta).unwrap();
    assert_eq!(
        journal.gameplay().trees,
        session.trees,
        "duplicate older section data must retain current fruit metadata and geometry"
    );
    authority.record(original.root, 0).unwrap();
    chunks
        .apply_confirmed_delta(&mut journal, authority.section_delta(section))
        .unwrap();
    assert_eq!(
        journal.gameplay().trees,
        session.trees,
        "only the ordered authoritative tree patch relinquishes replica cells"
    );
    let mut remaining = session.trees.get(1).unwrap().clone();
    remaining.wood.retain(|offset| *offset != [0, 0, 0]);
    remaining.stage = TreeStage::GeneratedFelled;
    remaining.fruit = None;
    remaining.revision += 1;
    remaining.refresh_bounds();
    apply(
        patch(
            3,
            false,
            TreeUpdate {
                tree: meta(remaining.clone()),
                geometry: TreeGeometryUpdate::Removed {
                    wood: vec![[0, 0, 0]],
                    leaves: vec![],
                },
            },
        ),
        &mut session,
        &mut journal,
        &mut chunks,
    )
    .unwrap();
    assert_eq!(session.trees.get(1), Some(&remaining));
    assert_eq!(journal.gameplay().trees, session.trees);
}
