//! Ecology-seven birches: broad rooted feet, a visible slender waist and a
//! branching upper crown. The ecology-six upright silhouette stays untouched.
use super::{
    DIRECTIONS, Tree,
    geometry::{Crown, WoodSegment},
    random,
    species::TreeSpecies,
    tree::TreeForm,
};

impl Tree {
    pub(super) fn crowned_birch(origin: [i64; 3], seed: u64, roots: [f32; 4]) -> Self {
        let height = 12.5 + random(seed, 1) * 3.5;
        let bend = [(random(seed, 2) - 0.5) * 1.4, (random(seed, 3) - 0.5) * 1.4];
        let trunk = |t: f32| [0.5 + bend[0] * t * t, height * t, 0.5 + bend[1] * t * t];
        let mut wood = Vec::with_capacity(20);
        let mut crowns = Vec::with_capacity(7);
        // The root flare tapers by three metres. The bare middle remains slim;
        // it is not filled with leaves from bottom to top like a column.
        for (a, b, radii) in [
            (0.0, 0.18, [1.65, 1.05]),
            (0.18, 0.48, [1.05, 0.90]),
            (0.48, 0.72, [0.90, 0.90]),
            (0.72, 1.0, [0.90, 0.90]),
        ] {
            wood.push(WoodSegment::new(trunk(a), trunk(b), radii));
        }
        for (i, ground) in roots.into_iter().enumerate() {
            let d = DIRECTIONS[i * 2];
            wood.push(WoodSegment::new(
                [0.5, 1.1, 0.5],
                [0.5 + d[0] * 3.0, ground + 0.25, 0.5 + d[1] * 3.0],
                [1.1, 0.9],
            ));
        }
        for i in 0..6 {
            let d = DIRECTIONS[((seed >> 8) as usize + i * 3) % 8];
            let level = 0.67 + i as f32 * 0.035;
            let reach = 3.2 + random(seed, 180 + i as u64) * 1.1;
            let start = trunk(level);
            let end = [
                start[0] + d[0] * reach,
                start[1] + 1.8,
                start[2] + d[1] * reach,
            ];
            wood.push(WoodSegment::new(start, end, [0.94, 0.90]));
            let radius = 2.4 + random(seed, 190 + i as u64) * 0.4;
            crowns.push(Crown::new(
                [end[0], end[1] + 0.5, end[2]],
                [radius, 2.05, radius * 0.92],
            ));
        }
        let top = trunk(1.0);
        crowns.push(Crown::new([top[0], top[1] + 0.2, top[2]], [3.1, 2.15, 2.9]));
        Self::from_shapes(
            origin,
            TreeSpecies::Birch,
            TreeForm::Spreading,
            wood,
            crowns,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn birches_have_root_flare_slender_waist_and_wide_connected_upper_crown() {
        for seed in [17, 42, 731, 42042] {
            let origin = [-17, 12, -1];
            let tree = Tree::with_species_revision(
                origin,
                seed,
                [-1.0, 1.0, 0.0, -2.0],
                TreeSpecies::Birch,
                7,
            );
            assert_eq!(
                tree,
                Tree::with_species_revision(
                    origin,
                    seed,
                    [-1.0, 1.0, 0.0, -2.0],
                    TreeSpecies::Birch,
                    7
                )
            );
            let mut wood = HashSet::new();
            let mut extents = [[i32::MAX, i32::MIN]; 3];
            let mut waist_leaves = 0;
            for y in tree.bounds.min[1]..tree.bounds.max[1] {
                for z in tree.bounds.min[2]..tree.bounds.max[2] {
                    for x in tree.bounds.min[0]..tree.bounds.max[0] {
                        let p = [
                            origin[0] + i64::from(x),
                            origin[1] + i64::from(y),
                            origin[2] + i64::from(z),
                        ];
                        let timber = tree.wood_at(p).is_some();
                        let occupied = timber || tree.leaves_at(p).is_some();
                        if timber {
                            wood.insert(p);
                        }
                        let band = if y <= 2 {
                            0
                        } else if (4..=6).contains(&y) {
                            1
                        } else if y >= 9 {
                            2
                        } else {
                            continue;
                        };
                        if occupied {
                            extents[band][0] = extents[band][0].min(x);
                            extents[band][1] = extents[band][1].max(x);
                            if band == 1 && !timber {
                                waist_leaves += 1;
                            }
                        }
                    }
                }
            }
            let widths = extents.map(|[min, max]| max - min + 1);
            assert!(widths[0] >= widths[1] * 2, "root flare {seed}: {widths:?}");
            assert!(widths[2] >= widths[1] * 3, "upper crown {seed}: {widths:?}");
            assert_eq!(waist_leaves, 0, "the middle trunk must stay visible");
            assert!(
                tree.bounds.max[0] <= 16 && tree.bounds.max[2] <= 16 && tree.bounds.max[1] <= 36
            );
            let mut seen = HashSet::from([origin]);
            let mut queue = vec![origin];
            while let Some(p) = queue.pop() {
                for d in [
                    [1, 0, 0],
                    [-1, 0, 0],
                    [0, 1, 0],
                    [0, -1, 0],
                    [0, 0, 1],
                    [0, 0, -1],
                ] {
                    let next = std::array::from_fn(|i| p[i] + d[i]);
                    if wood.contains(&next) && seen.insert(next) {
                        queue.push(next);
                    }
                }
            }
            assert_eq!(
                seen.len(),
                wood.len(),
                "all roots/branches must remain connected"
            );
        }
    }

    #[test]
    fn revision_seven_preserves_old_birches_and_every_other_species() {
        let origin = [-1, 24, -33];
        for seed in [17, 42, 731] {
            let old = Tree::with_species(origin, seed, [0.; 4], TreeSpecies::Birch);
            for revision in 0..=5 {
                assert_eq!(
                    Tree::with_species_revision(
                        origin,
                        seed,
                        [0.; 4],
                        TreeSpecies::Birch,
                        revision
                    ),
                    old
                );
            }
            let six = Tree::grove(origin, seed, [0.; 4], TreeSpecies::Birch);
            assert_eq!(
                Tree::with_species_revision(origin, seed, [0.; 4], TreeSpecies::Birch, 6),
                six
            );
            assert_ne!(
                Tree::with_species_revision(origin, seed, [0.; 4], TreeSpecies::Birch, 7),
                six
            );
            for species in [
                TreeSpecies::Green,
                TreeSpecies::Autumn,
                TreeSpecies::Cherry(false),
                TreeSpecies::SpiritBlossom(1),
                TreeSpecies::Bloodwood,
                TreeSpecies::Spruce,
                TreeSpecies::Fir,
                TreeSpecies::Palm,
                TreeSpecies::Apple,
            ] {
                assert_eq!(
                    Tree::with_species_revision(origin, seed, [0.; 4], species, 6),
                    Tree::with_species_revision(origin, seed, [0.; 4], species, 7)
                );
            }
        }
    }
}
