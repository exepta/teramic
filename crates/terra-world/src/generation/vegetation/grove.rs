//! Ecology-six native silhouettes. Oak crowns spread from low forks, birches
//! are narrow and upright, flowering trees form layered fans, and Bloodwood
//! has an elevated, asymmetric crown. Legacy constructors remain unchanged.
use super::{
    DIRECTIONS, Tree,
    geometry::{Crown, WoodSegment},
    random,
    species::TreeSpecies,
    tree::TreeForm,
};

impl Tree {
    pub(super) fn grove(
        origin: [i64; 3],
        seed: u64,
        roots: [f32; 4],
        species: TreeSpecies,
    ) -> Self {
        let (height, reach, crown_radius, crown_rise, branch_count, fork_height) = match species {
            TreeSpecies::Birch => (16.0 + random(seed, 1) * 6.0, 3.0, 2.15, 3.2, 5, 0.53),
            TreeSpecies::Cherry(_) => (10.0 + random(seed, 1) * 5.0, 6.1, 3.1, 1.7, 7, 0.40),
            TreeSpecies::SpiritBlossom(_) => {
                (15.0 + random(seed, 1) * 6.0, 6.0, 3.15, 2.0, 7, 0.48)
            }
            TreeSpecies::Bloodwood => (16.0 + random(seed, 1) * 7.0, 5.8, 3.0, 2.7, 7, 0.54),
            _ => (11.0 + random(seed, 1) * 7.0, 5.8, 3.3, 2.6, 6, 0.35),
        };
        let narrow = species == TreeSpecies::Birch;
        let flowering = matches!(
            species,
            TreeSpecies::Cherry(_) | TreeSpecies::SpiritBlossom(_)
        );
        let bend = [(random(seed, 2) - 0.5) * 2.6, (random(seed, 3) - 0.5) * 2.6];
        let trunk = |t: f32| [0.5 + bend[0] * t * t, height * t, 0.5 + bend[1] * t * t];
        let mut wood = Vec::with_capacity(28);
        let mut crowns = Vec::with_capacity(16);
        // Thick connecting capsules keep the voxel skeleton face-connected.
        let thickness = if narrow {
            0.92
        } else {
            1.25 + random(seed, 4) * 0.3
        };
        for i in 0..5 {
            let a = i as f32 / 5.0;
            let b = (i + 1) as f32 / 5.0;
            wood.push(WoodSegment::new(
                trunk(a),
                trunk(b),
                [
                    thickness + (0.92 - thickness) * a,
                    thickness + (0.92 - thickness) * b,
                ],
            ));
        }
        if !narrow {
            for (i, &ground) in roots.iter().enumerate() {
                let dir = DIRECTIONS[i * 2];
                wood.push(WoodSegment::new(
                    [0.5, 1.0, 0.5],
                    [0.5 + dir[0] * 3.0, ground + 0.25, 0.5 + dir[1] * 3.0],
                    [1.05, 0.9],
                ));
            }
        }
        for i in 0..branch_count {
            let channel = 180 + i as u64 * 5;
            let dir = DIRECTIONS[(((seed >> 8) as usize) + i * 3) % 8];
            let level = fork_height + i as f32 / branch_count as f32 * (0.88 - fork_height);
            let start = trunk(level);
            let length =
                reach * (0.75 + random(seed, channel) * 0.25) * (1.0 - (level - fork_height) * 0.3);
            let rise = if narrow {
                2.8
            } else if flowering {
                1.2
            } else {
                2.0
            };
            let elbow = [
                start[0] + dir[0] * length * 0.6,
                start[1] + rise * 0.3,
                start[2] + dir[1] * length * 0.6,
            ];
            let end = [
                start[0] + dir[0] * length,
                start[1] + rise,
                start[2] + dir[1] * length,
            ];
            wood.push(WoodSegment::new(start, elbow, [0.98, 0.92]));
            wood.push(WoodSegment::new(elbow, end, [0.92, 0.9]));
            let r = crown_radius * (0.85 + random(seed, channel + 1) * 0.25);
            crowns.push(Crown::new(
                [end[0], end[1] + 0.7, end[2]],
                [r, crown_rise, r * 0.91],
            ));
            // Offset smaller lobes produce scalloped, connected silhouettes.
            if !narrow && i.is_multiple_of(2) {
                crowns.push(Crown::new(
                    [
                        elbow[0] - dir[1] * 1.1,
                        elbow[1] + 1.2,
                        elbow[2] + dir[0] * 1.1,
                    ],
                    [r * 0.86, crown_rise * 0.9, r * 0.86],
                ));
            }
        }
        let top = trunk(1.0);
        crowns.push(Crown::new(
            [top[0], top[1] + 0.2, top[2]],
            [crown_radius * 0.86, crown_rise, crown_radius * 0.86],
        ));
        Self::from_shapes(
            origin,
            species,
            if narrow {
                TreeForm::Upright
            } else {
                TreeForm::Spreading
            },
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
    fn native_groves_keep_legacy_shapes_but_have_distinct_bounded_connected_skeletons() {
        let mut birch_width = 0;
        let mut oak_width = 0;
        for seed in [17, 42, 731] {
            for species in [
                TreeSpecies::Green,
                TreeSpecies::Autumn,
                TreeSpecies::Birch,
                TreeSpecies::Cherry(false),
                TreeSpecies::SpiritBlossom(2),
                TreeSpecies::Bloodwood,
            ] {
                let origin = [-1, 32, -17];
                let legacy = Tree::with_species(origin, seed, [0.0; 4], species);
                for revision in 0..=5 {
                    assert_eq!(
                        Tree::with_species_revision(origin, seed, [0.0; 4], species, revision),
                        legacy
                    );
                }
                let tree = Tree::with_species_revision(origin, seed, [0.0; 4], species, 6);
                assert_ne!(tree, legacy);
                assert_eq!(
                    tree,
                    Tree::with_species_revision(origin, seed, [0.0; 4], species, 6)
                );
                assert!(tree.bounds.min[0] >= -16 && tree.bounds.min[2] >= -16);
                assert!(
                    tree.bounds.max[0] <= 16
                        && tree.bounds.max[2] <= 16
                        && tree.bounds.max[1] <= 36
                );
                let width = tree.bounds.max[0] - tree.bounds.min[0];
                if species == TreeSpecies::Birch {
                    birch_width += width;
                }
                if species == TreeSpecies::Green {
                    oak_width += width;
                }
                let mut wood = HashSet::new();
                for z in tree.bounds.min[2]..tree.bounds.max[2] {
                    for y in tree.bounds.min[1]..tree.bounds.max[1] {
                        for x in tree.bounds.min[0]..tree.bounds.max[0] {
                            let cell = [
                                origin[0] + i64::from(x),
                                origin[1] + i64::from(y),
                                origin[2] + i64::from(z),
                            ];
                            if tree.wood_at(cell).is_some() {
                                wood.insert(cell);
                            }
                        }
                    }
                }
                assert!(wood.contains(&origin));
                let mut seen = HashSet::from([origin]);
                let mut queue = vec![origin];
                while let Some(cell) = queue.pop() {
                    for d in [
                        [1, 0, 0],
                        [-1, 0, 0],
                        [0, 1, 0],
                        [0, -1, 0],
                        [0, 0, 1],
                        [0, 0, -1],
                    ] {
                        let next = std::array::from_fn(|i| cell[i] + d[i]);
                        if wood.contains(&next) && seen.insert(next) {
                            queue.push(next);
                        }
                    }
                }
                assert_eq!(seen.len(), wood.len(), "disconnected {species:?}");
            }
        }
        assert!(
            birch_width * 4 < oak_width * 3,
            "birches stay visibly narrower than spreading oaks"
        );
    }
}
