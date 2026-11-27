use super::{
    Tree,
    geometry::{Crown, WoodSegment},
    random,
    species::TreeSpecies,
    tree::TreeForm,
};

impl Tree {
    /// Horizontally spreading acacia boughs hold several thin, overlapping pads.
    /// A baobab's thicker, tapering bole forks much higher into separate crowns.
    pub(super) fn savanna(origin: [i64; 3], seed: u64, species: TreeSpecies) -> Self {
        let baobab = species == TreeSpecies::Baobab;
        let height = if baobab {
            15. + random(seed, 1) * 5.
        } else {
            8. + random(seed, 1) * 5.
        };
        let bend = [random(seed, 3) * 2.6 - 1.3, random(seed, 4) * 2.6 - 1.3];
        let fork = [
            0.5 + bend[0],
            height * if baobab { 0.70 } else { 0.46 },
            0.5 + bend[1],
        ];
        let mut wood = vec![WoodSegment::new(
            [0.5, 0., 0.5],
            fork,
            if baobab { [2.35, 1.38] } else { [0.96, 0.77] },
        )];
        let mut crowns = Vec::new();
        let branches = 4 + (seed % 3) as usize;
        let phase = random(seed, 97) * std::f32::consts::TAU;
        for n in 0..branches {
            let channel = 10 + n as u64 * 7;
            let angle = phase
                + (n as f32 + random(seed, channel) * 0.34) * std::f32::consts::TAU
                    / branches as f32;
            let (dz, dx) = angle.sin_cos();
            let reach = if baobab { 3.0 } else { 3.6 } + random(seed, channel + 1) * 2.7;
            let tip = [
                fork[0] + dx * reach,
                height - 1.3 + random(seed, channel + 2) * 2.,
                fork[2] + dz * reach,
            ];
            let elbow = [
                fork[0] + dx * reach * 0.79,
                fork[1] + (tip[1] - fork[1]) * 0.46,
                fork[2] + dz * reach * 0.79,
            ];
            wood.push(WoodSegment::new(
                fork,
                elbow,
                [if baobab { 1.18 } else { 0.87 }, 0.82],
            ));
            wood.push(WoodSegment::new(elbow, tip, [0.82, 0.70]));
            let span = if baobab { 2.7 } else { 3.2 } + random(seed, channel + 3) * 1.35;
            crowns.push(Crown::new(
                tip,
                [
                    span,
                    if baobab { 2.3 } else { 1.55 },
                    span * (0.84 + random(seed, channel + 4) * 0.28),
                ],
            ));
            if baobab {
                wood.push(WoodSegment::new(
                    [0.5 + dx * 3.4, -0.6, 0.5 + dz * 3.4],
                    [0.5, 2.5, 0.5],
                    [0.73, 1.25],
                ));
            }
        }
        Self::from_shapes(origin, species, TreeForm::Spreading, wood, crowns)
    }
}
