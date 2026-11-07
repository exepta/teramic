use super::*;

fn reference(
    owners: &HashSet<SectionPos>,
    changes: &[SectionPos],
    radius: [i64; 3],
) -> HashSet<SectionPos> {
    owners
        .iter()
        .copied()
        .filter(|owner| {
            changes.iter().any(|changed| {
                owner.x.abs_diff(changed.x) <= radius[0] as u64
                    && owner.y.abs_diff(changed.y) <= radius[1] as u64
                    && owner.z.abs_diff(changed.z) <= radius[2] as u64
            })
        })
        .collect()
}

#[test]
fn sparse_neighbors_match_reference_for_negative_coordinates_edges_duplicates_and_both_domains() {
    for radius in [[2, 1, 2], [1, 1, 1]] {
        for base in [SectionPos::new(-17, -8, -1), SectionPos::new(15, 22, 16)] {
            let changes = vec![
                base,
                base,
                SectionPos::new(base.x + 10, base.y - 10, base.z),
            ];
            let owners: HashSet<_> = (-3..=3)
                .flat_map(|z| {
                    (-2..=2).flat_map(move |y| {
                        (-3..=3).map(move |x| SectionPos::new(base.x + x, base.y + y, base.z + z))
                    })
                })
                .collect();
            assert_eq!(
                neighbor_owners(&owners, changes.iter().copied(), radius).positions,
                reference(&owners, &changes, radius),
            );
            for owner in &owners {
                let sparse = HashSet::from([*owner]);
                assert_eq!(
                    neighbor_owners(&sparse, changes.iter().copied(), radius).positions,
                    reference(&sparse, &changes, radius),
                    "radius={radius:?}, owner={owner:?}"
                );
            }
        }
    }
}

#[test]
fn sparse_neighbors_bound_membership_probes_by_owner_count_on_mass_layer_unload() {
    let owners = HashSet::from([SectionPos::new(-3, -2, -1), SectionPos::new(9, 5, -4)]);
    let changes: Vec<_> = (0..1606).map(|x| SectionPos::new(x, -8, 12)).collect();
    for (radius, volume) in [([2, 1, 2], 75), ([1, 1, 1], 27)] {
        let result = neighbor_owners(&owners, changes.iter().copied(), radius);
        assert_eq!(result.positions, reference(&owners, &changes, radius));
        assert!(result.probes <= owners.len() * volume);
        assert!(result.probes * 100 < changes.len() * volume);
    }
}

#[test]
fn sparse_web_invalidation_keeps_other_waiters_and_deduplicates_affected_owners() {
    let near = SectionPos::new(-2, -3, 4);
    let far = SectionPos::new(50, 6, 80);
    let prior = SectionPos::new(-90, -9, -90);
    let mut queue = PropQueue::default();
    queue.web_owners.extend([near, far]);
    queue.changed.insert(prior);
    queue.waiting_support.insert(prior);
    let changes: Vec<_> = std::iter::repeat_n(SectionPos::new(0, -2, 2), 100).collect();
    queue.invalidate_web_neighbors(changes.into_iter());
    assert_eq!(queue.changed, HashSet::from([prior, near]));
    assert_eq!(queue.waiting_support, HashSet::from([prior, near]));
    assert_eq!(queue.web_owners, HashSet::from([near, far]));
    assert!(
        neighbor_owners(&HashSet::new(), [near].into_iter(), [2, 1, 2])
            .positions
            .is_empty()
    );
    assert!(
        neighbor_owners(&queue.web_owners, [].into_iter(), [1, 1, 1])
            .positions
            .is_empty()
    );
}
