use super::*;
use terra_core::{FruitPhase, FruitState, PlayerUuid, TreeSpecies};
fn grown(id: u64, cells: usize) -> TreeRecord {
    let mut offsets = (0..cells)
        .map(|n| {
            [
                (n % 32) as i8 - 16,
                (n / 1024) as i8,
                (n / 32 % 32) as i8 - 16,
            ]
        })
        .collect::<Vec<_>>();
    let leaves = offsets.split_off(cells.min(4096));
    let wood = offsets;
    TreeRecord {
        id,
        root: [8, 1, 8],
        species: TreeSpecies::Apple,
        seed: id,
        planted_by: Some(PlayerUuid::from_u128(1)),
        stage: TreeStage::Generated,
        growth_seconds: 1400.,
        bounds: None,
        wood,
        leaves,
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
#[test]
fn large_interest_uses_bounded_ordered_partial_packets_without_acknowledging_unsent_trees() {
    let a = grown(1, 20_000);
    let b = grown(2, 20_000);
    assert!(a.valid() && b.valid());
    let mut peer = TreePeer::default();
    let visible = [&a, &b];
    let first = next_patch(&mut peer, &visible).unwrap();
    assert!(first.reset);
    assert_eq!(first.revision, 1);
    assert_eq!(first.records.len(), 1);
    assert_eq!(peer.confirmed.len(), 1);
    assert!(!peer.confirmed.contains_key(&2));
    let second = next_patch(&mut peer, &visible).unwrap();
    assert!(!second.reset);
    assert_eq!(second.revision, 2);
    assert_eq!(second.records.len(), 1);
    assert_eq!(second.records[0].tree.id, 2);
    assert_eq!(peer.confirmed.len(), 2);
    for patch in [first, second] {
        assert!(patch.records.iter().map(cells).sum::<usize>() <= MAX_TREE_PATCH_CELLS);
        assert!(
            postcard::to_allocvec(&ServerControl::TreeUpdates(patch))
                .unwrap()
                .len()
                <= MAX_TREE_PATCH_BYTES
        );
    }
    assert!(next_patch(&mut peer, &visible).is_none());
    let removed = next_patch(&mut peer, &[&b]).unwrap();
    assert_eq!(removed.removals, vec![1]);
    assert!(removed.records.is_empty());
    assert_eq!(peer.confirmed.len(), 1);
}
#[test]
fn percent_buckets_and_fruit_updates_send_metadata_without_copying_confirmed_geometry() {
    let mut sapling = grown(1, 1);
    sapling.stage = TreeStage::Sapling;
    sapling.wood.clear();
    sapling.fruit = None;
    sapling.growth_seconds = 18.9;
    sapling.species = TreeSpecies::Oak;
    sapling.refresh_bounds();
    let mut peer = TreePeer::default();
    let initial = next_patch(&mut peer, &[&sapling]).unwrap();
    assert_eq!(initial.records[0].tree.growth_seconds, 18.);
    sapling.growth_seconds = 26.99;
    sapling.revision += 1;
    assert!(next_patch(&mut peer, &[&sapling]).is_none());
    sapling.growth_seconds = 27.;
    let progress = next_patch(&mut peer, &[&sapling]).unwrap();
    assert_eq!(progress.records[0].tree.growth_seconds, 27.);
    assert!(matches!(
        progress.records[0].geometry,
        TreeGeometryUpdate::Unchanged
    ));
    assert_eq!(cells(&progress.records[0]), 0);
    let mut apple = grown(2, 20_000);
    next_patch(&mut peer, &[&sapling, &apple]).unwrap();
    let wood = peer.confirmed[&2].wood.as_ptr();
    let leaves = peer.confirmed[&2].leaves.as_ptr();
    let fruit = apple.fruit.as_mut().unwrap();
    fruit.picked = 1;
    fruit.remaining = 7;
    fruit.revision += 1;
    apple.revision += 1;
    let harvest = next_patch(&mut peer, &[&sapling, &apple]).unwrap();
    assert_eq!(harvest.records.len(), 1);
    assert_eq!(cells(&harvest.records[0]), 0);
    assert!(matches!(
        harvest.records[0].geometry,
        TreeGeometryUpdate::Unchanged
    ));
    assert_eq!(peer.confirmed[&2].wood.as_ptr(), wood);
    assert_eq!(peer.confirmed[&2].leaves.as_ptr(), leaves);
    assert!(postcard::to_allocvec(&harvest).unwrap().len() < 256);
}
#[test]
fn felling_sends_only_removed_offsets_including_generated_felled_rests() {
    let mut tree = grown(7, 20_000);
    let mut peer = TreePeer::default();
    next_patch(&mut peer, &[&tree]).unwrap();
    let removed_leaf = tree.leaves.remove(99);
    tree.refresh_bounds();
    tree.revision += 1;
    let patch = next_patch(&mut peer, &[&tree]).unwrap();
    assert_eq!(
        patch.records[0].geometry,
        TreeGeometryUpdate::Removed {
            wood: vec![],
            leaves: vec![removed_leaf]
        }
    );
    assert!(patch.records[0].tree.wood.is_empty() && patch.records[0].tree.leaves.is_empty());
    let removed_wood = std::mem::take(&mut tree.wood);
    tree.stage = TreeStage::GeneratedFelled;
    tree.fruit = None;
    tree.refresh_bounds();
    tree.revision += 1;
    let patch = next_patch(&mut peer, &[&tree]).unwrap();
    assert_eq!(
        patch.records[0].geometry,
        TreeGeometryUpdate::Removed {
            wood: removed_wood,
            leaves: vec![]
        }
    );
    assert_eq!(patch.records[0].tree.stage, TreeStage::GeneratedFelled);
    assert_eq!(peer.confirmed[&7], tree);
}
