use super::{
    Tree,
    geometry::{Crown, WoodSegment},
    random,
    species::TreeSpecies,
    tree::TreeForm,
};
use terra_voxel::MaterialId;

impl Tree {
    /// Buttress roots emerge from the shallows, a crooked bole forks into spreading
    /// boughs, and drooping curtains are anchored to the lower edge of each crown.
    pub(super) fn willow(origin: [i64; 3], seed: u64) -> Self {
        let old = seed.is_multiple_of(5);
        let height = if old { 15.5 } else { 9. } + random(seed, 61) * 4.;
        let phase = random(seed, 64) * std::f32::consts::TAU;
        let (dz, dx) = phase.sin_cos();
        let knee = [0.5 + dx * 1.8, height * 0.28, 0.5 + dz * 1.8];
        let fork = [0.5 - dz * 1.4, height * 0.59, 0.5 + dx * 1.4];
        let girth = if old { 2.05 } else { 1.17 };
        let mut wood = vec![
            WoodSegment::new([0.5, -0.5, 0.5], knee, [girth, girth * 0.87]),
            WoodSegment::new(knee, fork, [girth * 0.87, girth * 0.72]),
        ];
        let mut crowns = Vec::new();
        let branches = if old { 6 } else { 4 };
        for n in 0..branches {
            let key = 80 + n * 6;
            let angle = phase
                + (n as f32 + random(seed, key) * 0.4) * std::f32::consts::TAU / branches as f32;
            let (sz, sx) = angle.sin_cos();
            let reach = (if old { 5.1 } else { 3.5 }) + random(seed, key + 1) * 1.8;
            let root_elbow = [0.5 + sx * 2.0, 1.5, 0.5 + sz * 2.0];
            wood.push(WoodSegment::new(knee, root_elbow, [girth * 0.73, 0.88]));
            wood.push(WoodSegment::new(
                root_elbow,
                [0.5 + sx * 3.5, -1.2, 0.5 + sz * 3.5],
                [0.88, 0.52],
            ));
            let elbow = [
                fork[0] + sx * reach * 0.50,
                height * 0.84,
                fork[2] + sz * reach * 0.50,
            ];
            let tip = [
                fork[0] + sx * reach,
                height - random(seed, key + 2) * 2.,
                fork[2] + sz * reach,
            ];
            wood.push(WoodSegment::new(fork, elbow, [1.0, 0.78]));
            wood.push(WoodSegment::new(elbow, tip, [0.8, 0.60]));
            let span = 2.4 + random(seed, key + 3) * 1.1;
            crowns.push(Crown::new(tip, [span, 2.0, span]));
            for strand in 0..5 {
                let a = angle + (strand as f32 - 2.) * 0.58;
                let (pz, px) = a.sin_cos();
                let x = (tip[0] + px * span * 0.78).floor() as i32;
                let z = (tip[2] + pz * span * 0.78).floor() as i32;
                let length = 2 + (random(seed, key + 19 + strand) * height * 0.40) as i32;
                crowns.push(Crown::pendant(
                    x,
                    z,
                    (tip[1] - 0.6).floor() as i32,
                    length,
                    MaterialId::HangingMoss,
                ));
            }
        }
        let mut tree = Self::from_shapes(
            origin,
            TreeSpecies::Willow,
            TreeForm::Spreading,
            wood,
            crowns,
        );
        if old {
            tree.wood_material = MaterialId::MossyWillowLog;
        }
        tree
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn willow_curtains_attach_to_the_canopy_and_hang_in_contiguous_runs() {
        for seed in [0, 1, 13] {
            let tree = Tree::willow([0; 3], seed);
            let mut curtains = 0;
            for x in -12..=12 {
                for z in -12..=12 {
                    let leaf = |y| tree.leaves_at([x, y, z]);
                    for y in 1..25 {
                        if leaf(y) == Some(MaterialId::HangingMoss)
                            && leaf(y + 1) == Some(MaterialId::HangingMoss)
                            && leaf(y + 2) == Some(MaterialId::WillowLeaves)
                        {
                            curtains += 1;
                        }
                    }
                }
            }
            assert!(
                curtains >= 3,
                "each willow needs distinct anchored curtains, found {curtains} for {seed}"
            );
        }
    }
}
