//! Twisting, buttressed bloodwoods for the current Blood Forest.
use super::{
    DIRECTIONS, Tree,
    geometry::{Crown, WoodSegment},
    random,
    species::TreeSpecies,
    tree::TreeForm,
};
use std::f32::consts::TAU;

impl Tree {
    pub(super) fn ancient_bloodwood(origin: [i64; 3], seed: u64, roots: [f32; 4]) -> Self {
        let height = 18.0 + random(seed, 330) * 10.0;
        let yaw = TAU * random(seed, 331);
        let twist = if seed & 1 == 0 { 1.0 } else { -1.0 };
        let lean = 1.2 + random(seed, 332) * 1.8;
        let trunk = |t: f32| {
            let angle = yaw + twist * t * 4.7;
            let bend = lean * (t * 3.0).sin();
            [
                0.5 + angle.cos() * bend,
                height * t,
                0.5 + angle.sin() * bend,
            ]
        };
        let thickness = |t: f32| 0.90 + 1.75 * (1.0 - t).powi(2);
        let mut wood = Vec::with_capacity(50);
        let mut crowns = Vec::with_capacity(18);
        for i in 0..7 {
            let (a, b) = (i as f32 / 7., (i + 1) as f32 / 7.);
            wood.push(WoodSegment::new(
                trunk(a),
                trunk(b),
                [thickness(a), thickness(b)],
            ));
        }
        // Endpoints coincide with the four terrain probes used at planting.
        for (i, ground) in roots.into_iter().enumerate() {
            let [dx, dz] = DIRECTIONS[i * 2];
            let elbow = [
                0.5 + dx * 2.2 - dz * twist,
                1.6,
                0.5 + dz * 2.2 + dx * twist,
            ];
            let toe = [0.5 + dx * 4., ground, 0.5 + dz * 4.];
            wood.push(WoodSegment::new(trunk(0.17), elbow, [1.48, 1.18]));
            wood.push(WoodSegment::new(elbow, toe, [1.18, 0.86]));
        }
        for i in 0..8 {
            let t = 0.34 + i as f32 * 0.066;
            let a = yaw + i as f32 * 2.40;
            let (dz, dx) = a.sin_cos();
            let base = trunk(t);
            let reach = (7.9 - 2.8 * t) * (0.83 + 0.24 * random(seed, 340 + i));
            let elbow = [
                base[0] + dx * reach * 0.51 - dz * twist,
                base[1] + 1.2,
                base[2] + dz * reach * 0.51 + dx * twist,
            ];
            let tip = [base[0] + dx * reach, base[1] + 2.6, base[2] + dz * reach];
            let fork = [elbow[0] - dz * 2.3, elbow[1] + 2.8, elbow[2] + dx * 2.3];
            wood.push(WoodSegment::new(base, elbow, [1.38, 1.02]));
            wood.push(WoodSegment::new(elbow, tip, [1.02, 0.86]));
            wood.push(WoodSegment::new(elbow, fork, [0.98, 0.84]));
            let r = 3.0 + random(seed, 360 + i) * 0.9;
            crowns.push(Crown::new(
                [tip[0], tip[1] + 0.45, tip[2]],
                [r, 2.15, r * 0.9],
            ));
            crowns.push(Crown::new(
                [fork[0], fork[1] + 0.48, fork[2]],
                [r * 0.8, 1.70, r * 0.8],
            ));
        }
        crowns.push(Crown::new(trunk(1.), [3.5, 2.3, 3.0]));
        let mut tree = Self::from_shapes(
            origin,
            TreeSpecies::Bloodwood,
            TreeForm::Spreading,
            wood,
            crowns,
        );
        if seed.is_multiple_of(5) {
            tree.ornament_material = terra_voxel::MaterialId::HangingBloodLantern;
        }
        tree
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::vegetation::{MAX_TREE_HEIGHT, MAX_TREE_REACH};

    #[test]
    fn old_and_new_woods_have_distinct_silhouettes_and_fit_streaming_bounds() {
        for seed in 0..32 {
            let make = |rev| {
                Tree::with_species_revision(
                    [-33, 40, -17],
                    seed,
                    [0., 1., -2., 2.],
                    TreeSpecies::Bloodwood,
                    rev,
                )
            };
            let older = make(11);
            let tree = make(12);
            assert_ne!(older, tree);
            assert_eq!(tree, make(12));
            for axis in [0, 2] {
                assert!(i64::from(tree.bounds.min[axis]) >= -MAX_TREE_REACH);
                assert!(i64::from(tree.bounds.max[axis]) <= MAX_TREE_REACH);
            }
            assert!(i64::from(tree.bounds.max[1]) <= MAX_TREE_HEIGHT);
            for [dx, dy, dz] in [[4, 0, 0], [0, 1, 4], [-4, -2, 0], [0, 2, -4]] {
                assert!(tree.wood_at([-33 + dx, 40 + dy, -17 + dz]).is_some());
            }
        }
    }
}
