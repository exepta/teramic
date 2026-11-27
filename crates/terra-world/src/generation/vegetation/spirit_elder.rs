//! Moonlit grove trees have bowed limbs, tiered parasol crowns and racemes.
use super::{
    DIRECTIONS, Tree,
    geometry::{Crown, WoodSegment},
    random,
    species::TreeSpecies,
    tree::TreeForm,
};
use terra_voxel::MaterialId;

impl Tree {
    pub(super) fn spirit_elder(
        origin: [i64; 3],
        seed: u64,
        roots: [f32; 4],
        species: TreeSpecies,
    ) -> Self {
        let height = 17.0 + 8.5 * random(seed, 472);
        let angle = random(seed, 473) * std::f32::consts::TAU;
        let trunk = |t: f32| {
            let curve = (t * std::f32::consts::TAU - 0.55).sin() + 0.55_f32.sin();
            [
                0.5 + angle.cos() * curve * 1.15,
                height * t,
                0.5 + angle.sin() * curve * 1.15,
            ]
        };
        let radius = |t: f32| 0.88 + 1.38 * (1.0 - t).powf(1.65);
        let mut wood = Vec::with_capacity(34);
        for i in 0..6 {
            let (a, b) = (i as f32 / 6.0, (i + 1) as f32 / 6.0);
            wood.push(WoodSegment::new(trunk(a), trunk(b), [radius(a), radius(b)]));
        }
        for (i, ground) in roots.into_iter().enumerate() {
            let [dx, dz] = DIRECTIONS[i * 2];
            let knee = [0.5 + 1.9 * dx, 1.5 + ground * 0.2, 0.5 + 1.9 * dz];
            wood.push(WoodSegment::new(trunk(0.16), knee, [1.32, 1.11]));
            wood.push(WoodSegment::new(
                knee,
                [0.5 + 4.0 * dx, ground + 0.2, 0.5 + 4.0 * dz],
                [1.11, 0.82],
            ));
        }
        let mut crowns = Vec::with_capacity(38);
        let mut pendants = Vec::with_capacity(18);
        let mut lamp_branches = Vec::with_capacity(2);
        for i in 0..8 {
            let ch = 500 + 8 * i;
            let level = 0.44 + i as f32 * 0.058;
            let a = angle + i as f32 * 2.39996;
            let (dz, dx) = a.sin_cos();
            let reach = (8.4 + random(seed, ch) * 1.25) * (1.0 - i as f32 * 0.025);
            let start = trunk(level);
            let elbow = [
                start[0] + dx * reach * 0.42 - dz * 0.8,
                start[1] + 0.55,
                start[2] + dz * reach * 0.42 + dx * 0.8,
            ];
            let tip = [start[0] + dx * reach, start[1] + 2.5, start[2] + dz * reach];
            if i == 1 || (i == 5 && seed & 1 == 0) {
                lamp_branches.push(elbow);
            }
            wood.push(WoodSegment::new(start, elbow, [1.34, 1.10]));
            wood.push(WoodSegment::new(elbow, tip, [1.10, 0.88]));
            let fork = [
                elbow[0] + dx * 1.65 - dz * 2.7,
                elbow[1] + 2.3,
                elbow[2] + dz * 1.65 + dx * 2.7,
            ];
            wood.push(WoodSegment::new(elbow, fork, [1.04, 0.86]));
            crowns.push(Crown::new(
                [fork[0], fork[1] + 0.6, fork[2]],
                [3.25, 1.72, 3.25],
            ));
            let r = 3.55 + random(seed, ch + 1) * 0.60;
            let center = [tip[0], tip[1] + 0.75, tip[2]];
            let cap = Crown::new(center, [r, 1.85, r]);
            for side in [-1.0, 1.0] {
                let x = (tip[0] + dx * 0.9 - dz * r * 0.59 * side).floor() as i32;
                let z = (tip[2] + dz * 0.9 + dx * r * 0.59 * side).floor() as i32;
                // The drawn chain starts directly below an actual occupied crown voxel.
                let Some(first_leaf) = (cap.bounds.min[1]..cap.bounds.max[1])
                    .find(|y| cap.contains([x as f32 + 0.5, *y as f32 + 0.5, z as f32 + 0.5]))
                else {
                    continue;
                };
                let channel = ch + if side > 0.0 { 3 } else { 5 };
                let length = 2 + (random(seed, channel) * 4.0) as i32;
                let material = if random(seed, channel + 1) < 0.57 {
                    MaterialId::WisteriaTassel
                } else {
                    MaterialId::JadeSpiritVine
                };
                pendants.push(Crown::pendant(x, z, first_leaf - 1, length, material));
            }
            crowns.push(cap);
        }
        let top = trunk(1.0);
        crowns.push(Crown::new([top[0], top[1], top[2]], [4.4, 2.0, 4.3]));
        // Real canopy always wins where another limb grows through a raceme.
        crowns.extend(pendants);
        let mut tree = Self::from_shapes(origin, species, TreeForm::Spreading, wood, crowns);
        // Place lanterns against a real branch underside, then include their
        // cells in the same crown ownership used by mining and section stamps.
        let mut lamps = tree.lantern.take().into_iter().collect::<Vec<_>>();
        for elbow in lamp_branches {
            let x = origin[0] + elbow[0].floor() as i64;
            let z = origin[2] + elbow[2].floor() as i64;
            let y = origin[1] + elbow[1].floor() as i64;
            if let Some(cell) = (y - 3..=y).map(|h| [x, h, z]).find(|p| {
                tree.wood_at(*p).is_none()
                    && tree.leaves_at(*p).is_none()
                    && tree.wood_at([x, p[1] + 1, z]).is_some()
            }) {
                lamps.push(cell);
            }
        }
        for cell in lamps {
            tree.crowns.push(Crown::pendant(
                (cell[0] - origin[0]) as i32,
                (cell[2] - origin[2]) as i32,
                (cell[1] - origin[1]) as i32,
                1,
                MaterialId::HangingBlossomLantern,
            ));
        }
        tree.leaves_material = if random(seed, 476) < 0.28 {
            MaterialId::MoonlitSpiritLeaves
        } else {
            MaterialId::TealSpiritLeaves
        };
        tree
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spirit_elders_stamp_anchored_garlands_across_section_edges() {
        use crate::generation::vegetation::{MAX_TREE_HEIGHT, MAX_TREE_REACH};
        let mut shades = std::collections::HashSet::new();
        for seed in 0..16 {
            let origin = [-17, 15, 16];
            let tree = Tree::spirit_elder(origin, seed, [0.0; 4], TreeSpecies::SpiritBlossom(1));
            shades.insert(tree.leaves_material as u16);
            let b = tree.bounds;
            assert!(b.max[1] <= MAX_TREE_HEIGHT as i32);
            for axis in [0, 2] {
                assert!(
                    b.min[axis] >= -MAX_TREE_REACH as i32 && b.max[axis] <= MAX_TREE_REACH as i32
                );
            }
            for id in [MaterialId::WisteriaTassel, MaterialId::JadeSpiritVine] {
                let mut count = 0;
                for c in &tree.crowns {
                    let min = c.bounds.min;
                    if c.bounds.max[0] != min[0] + 1 {
                        continue;
                    }
                    for ly in min[1]..c.bounds.max[1] {
                        let p = [
                            origin[0] + i64::from(min[0]),
                            origin[1] + i64::from(ly),
                            origin[2] + i64::from(min[2]),
                        ];
                        if tree.leaves_at(p) == Some(id) && tree.wood_at(p).is_none() {
                            let above = [p[0], p[1] + 1, p[2]];
                            assert!(
                                tree.leaves_at(above).is_some() || tree.wood_at(above).is_some()
                            );
                            let start = p.map(|v| v.div_euclid(16) * 16);
                            let local: [usize; 3] =
                                std::array::from_fn(|i| (p[i] - start[i]) as usize);
                            let mut blocks = [0; 4096];
                            tree.stamp_leaves(start, 16, &mut blocks);
                            assert_eq!(
                                blocks[local[0] + 16 * (local[1] + 16 * local[2])],
                                id as u16
                            );
                            count += 1;
                            break;
                        }
                    }
                }
                assert!(count > 0);
            }
        }
        assert_eq!(shades.len(), 2);
    }
}
