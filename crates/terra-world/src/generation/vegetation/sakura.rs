//! Old cherries split near the roots. Each crooked scaffold carries a cushion
//! of flowers, with hanging racemes at the outer rim and space below the forks.
use super::{
    DIRECTIONS, Tree,
    geometry::{Crown, WoodSegment},
    random,
    species::TreeSpecies,
    tree::TreeForm,
};
use terra_voxel::MaterialId;

impl Tree {
    pub(super) fn sakura(
        origin: [i64; 3],
        seed: u64,
        roots: [f32; 4],
        species: TreeSpecies,
    ) -> Self {
        let age = random(seed, 820);
        let height = if age < 0.22 {
            5.4 + age * 10.0
        } else {
            9.0 + age.powi(2) * 15.5
        };
        let spread = (height * 0.49).min(9.8);
        let bark = 0.95 + age * 0.80;
        let mut wood = Vec::new();
        let mut crowns = Vec::new();
        let mut tassels = Vec::new();
        let twist = random(seed, 821) * std::f32::consts::TAU;
        let root = [0.5, 0.0, 0.5];
        let joint = [
            0.5 + twist.cos() * 0.6,
            height * 0.29,
            0.5 + twist.sin() * 0.6,
        ];
        wood.push(WoodSegment::new(root, joint, [bark, bark * 0.88]));
        for (i, bed) in roots.into_iter().enumerate() {
            let [dx, dz] = DIRECTIONS[i * 2];
            wood.push(WoodSegment::new(
                [0.5, 1.25, 0.5],
                [0.5 + dx * 2.7, bed + 0.25, 0.5 + dz * 2.7],
                [bark * 0.82, 0.89],
            ));
        }
        // The highest limb curls back over the fork. The other limbs splay outward.
        for i in 0..5 {
            let key = 830 + i * 5;
            let a = twist + i as f32 * 2.39996 + random(seed, key) * 0.42;
            let (dz, dx) = a.sin_cos();
            let radius = if i == 4 {
                spread * 0.27
            } else {
                spread * (0.76 + random(seed, key + 1) * 0.24)
            };
            let tier = if i == 4 {
                height
            } else {
                height * (0.58 + random(seed, key + 2) * 0.26)
            };
            let elbow = [
                joint[0] + dx * radius * 0.53 - dz * 0.55,
                tier * 0.72,
                joint[2] + dz * radius * 0.53 + dx * 0.55,
            ];
            let tip = [joint[0] + dx * radius, tier, joint[2] + dz * radius];
            wood.push(WoodSegment::new(
                joint,
                elbow,
                [bark.max(1.05) * 0.93, 1.02],
            ));
            wood.push(WoodSegment::new(elbow, tip, [1.04, 0.92]));
            let breadth = 1.7 + age * 1.85 + random(seed, key + 3) * 0.50;
            crowns.push(Crown::new(tip, [breadth, breadth * 0.65, breadth]));
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            let fork = [
                tip[0] - dz * breadth * 0.82 * side,
                tier - 1.1,
                tip[2] + dx * breadth * 0.82 * side,
            ];
            wood.push(WoodSegment::new(elbow, fork, [0.97, 0.91]));
            crowns.push(Crown::new(
                fork,
                [breadth * 0.79, breadth * 0.59, breadth * 0.79],
            ));
            let fringe = [
                fork[0] + dx * breadth * 0.52,
                fork[1] - 1.1,
                fork[2] + dz * breadth * 0.52,
            ];
            let fringe = Crown::new(fringe, [1.0, 1.25 + age * 0.75, 1.0]);
            if age > 0.32 && random(seed, key + 4) < 0.76 {
                let x = fork[0] + dx * breadth * 0.52;
                let z = fork[2] + dz * breadth * 0.52;
                let (x, z) = (x.floor() as i32, z.floor() as i32);
                if let Some(leaf) = (fringe.bounds.min[1]..fringe.bounds.max[1])
                    .find(|y| fringe.contains([x as f32 + 0.5, *y as f32 + 0.5, z as f32 + 0.5]))
                {
                    let length = (2 + (age * 4.0) as i32).min(leaf - 2);
                    if length > 0 {
                        tassels.push(Crown::pendant(
                            x,
                            z,
                            leaf - 1,
                            length,
                            MaterialId::WisteriaTassel,
                        ));
                    }
                }
            }
            crowns.push(fringe);
        }
        crowns.extend(tassels);
        let mut tree = Self::from_shapes(origin, species, TreeForm::Spreading, wood, crowns);
        tree.lantern = tree.branch_lantern();
        tree.ornament_material = MaterialId::Lantern;
        tree
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn young_and_old_cherries_fit_section_search_and_keep_flower_tips() {
        let mut heights = Vec::new();
        let mut garlanded = 0;
        for seed in 0..64 {
            let tree = Tree::with_species_revision(
                [0; 3],
                seed,
                [0.; 4],
                TreeSpecies::Cherry(seed % 3 == 0),
                15,
            );
            assert!(tree.bounds.min[0] >= -16 && tree.bounds.min[2] >= -16);
            assert!(tree.bounds.max[0] <= 16 && tree.bounds.max[2] <= 16);
            assert_eq!(tree.wood_at([0; 3]), Some(MaterialId::CherryLog));
            assert!((15..=20).contains(&tree.crowns.len()));
            garlanded += usize::from(tree.crowns.len() > 15);
            heights.push(tree.bounds.max[1]);
        }
        assert!(heights.iter().max().unwrap() - heights.iter().min().unwrap() >= 16);
        assert!(garlanded > 24);
    }
}
