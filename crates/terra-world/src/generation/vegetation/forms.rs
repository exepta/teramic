use super::{
    DIRECTIONS, Tree,
    geometry::{Crown, WoodSegment},
    random,
    species::TreeSpecies,
    tree::TreeForm,
};

impl Tree {
    pub(super) fn apple(origin: [i64; 3], seed: u64, connected_branches: bool) -> Self {
        let height = 7.0 + random(seed, 1) * 3.0;
        let trunk_top = [0.5, height * 0.68, 0.5];
        let mut wood = vec![WoodSegment::new([0.5, 0.0, 0.5], trunk_top, [0.68, 0.48])];
        let mut crowns = vec![Crown::new([0.5, height * 0.85, 0.5], [2.7, 2.1, 2.7])];
        for (index, direction) in DIRECTIONS.into_iter().step_by(2).enumerate() {
            let reach = 2.5 + random(seed, 100 + index as u64) * 1.1;
            let branch = [
                0.5 + direction[0] * reach,
                height * 0.50 + 0.7 * (index % 2) as f32,
                0.5 + direction[1] * reach,
            ];
            wood.push(WoodSegment::new(
                [0.5, height * 0.38, 0.5],
                branch,
                // Thin diagonal capsules can rasterize to disconnected tips.
                // New ecology keeps the authored axes while giving every branch
                // the same face-connected minimum radius as broad native trees.
                if connected_branches {
                    [0.9; 2]
                } else {
                    [0.58, 0.48]
                },
            ));
            crowns.push(Crown::new(
                [branch[0], branch[1] + 1.4, branch[2]],
                [2.35, 1.9, 2.35],
            ));
        }
        Self::from_shapes(
            origin,
            TreeSpecies::Apple,
            TreeForm::Spreading,
            wood,
            crowns,
        )
    }

    pub(super) fn palm(origin: [i64; 3], seed: u64) -> Self {
        let height = 8.0 + random(seed, 1) * 6.0;
        let direction = DIRECTIONS[(seed % 8) as usize];
        let lean = 1.0 + random(seed, 2) * 2.2;
        let trunk = |t: f32| {
            [
                0.5 + direction[0] * lean * t * t,
                height * t,
                0.5 + direction[1] * lean * t * t,
            ]
        };
        let mut wood = Vec::with_capacity(5);
        for step in 0..5 {
            wood.push(WoodSegment::new(
                trunk(step as f32 / 5.0),
                trunk((step + 1) as f32 / 5.0),
                [0.96, 0.90],
            ));
        }
        let top = trunk(1.0);
        let mut crowns = vec![Crown::new(top, [1.45, 1.3, 1.45])];
        // Shared tapered capsules keep bowed fronds face-connected even when
        // a tip lands diagonally on a section or voxel boundary.
        for (index, direction) in DIRECTIONS.into_iter().enumerate() {
            let length = 4.0 + random(seed, 70 + index as u64) * 2.0;
            let mut previous = top;
            for step in 1..=6 {
                let t = step as f32 / 6.0;
                let next = [
                    top[0] + direction[0] * length * t,
                    top[1] + (t * std::f32::consts::PI).sin() * 1.5 - t * 2.4,
                    top[2] + direction[1] * length * t,
                ];
                crowns.push(Crown::frond(
                    previous,
                    next,
                    [1.15 - (t - 1.0 / 6.0) * 0.24, 1.15 - t * 0.24],
                ));
                previous = next;
            }
        }
        Self::from_shapes(origin, TreeSpecies::Palm, TreeForm::Spreading, wood, crowns)
    }

    pub(super) fn spruce(origin: [i64; 3], seed: u64) -> Self {
        let (low, high) = TreeSpecies::Spruce.height_range();
        let height = low + random(seed, 1) * (high - low);
        let trunk = |t: f32| [0.5, height * t, 0.5];
        // Centered, sub-half-metre radii rasterize to one vertical block column.
        // Wood stops inside the crown; the last metres are foliage only.
        let wood = vec![WoodSegment::new(trunk(0.0), trunk(0.87), [0.48; 2])];
        let mut crowns = Vec::with_capacity(12);
        for tier in 0..11 {
            let t = 0.25 + tier as f32 * 0.067;
            let width = (6.0 * (1.0 - t) + 0.55) * (0.94 + random(seed, 40 + tier) * 0.12);
            let center = trunk(t);
            crowns.push(Crown::new(
                center,
                [width, 2.0 + height * 0.015, width * 0.95],
            ));
        }
        crowns.push(Crown::new(trunk(0.98), [1.3, 1.8, 1.3]));
        Self::from_shapes(origin, TreeSpecies::Spruce, TreeForm::Tiered, wood, crowns)
    }

    pub(super) fn fir(origin: [i64; 3], seed: u64) -> Self {
        let (low, high) = TreeSpecies::Fir.height_range();
        let height = low + random(seed, 1) * (high - low);
        let trunk = |t: f32| [0.5, height * t, 0.5];
        let wood = vec![WoodSegment::new(trunk(0.0), trunk(0.94), [0.48; 2])];
        let mut crowns = Vec::with_capacity(11);
        for tier in 0..10 {
            let t = 0.16 + tier as f32 * 0.076;
            let width = (5.8 * (1.0 - t) + 0.35) * (0.92 + random(seed, 90 + tier) * 0.16);
            crowns.push(Crown::conifer_tier(
                trunk(t),
                [width, height * 0.13 + 0.6, width * 0.92],
                random(seed, 110 + tier) * std::f32::consts::TAU,
            ));
        }
        crowns.push(Crown::new(trunk(0.98), [0.85, 1.55, 0.85]));
        Self::from_shapes(origin, TreeSpecies::Fir, TreeForm::Tiered, wood, crowns)
    }

    pub(super) fn mushroom(origin: [i64; 3], seed: u64, variant: u8) -> Self {
        let variant = variant % 6;
        let height = (6.0 + random(seed, 1) * 7.0)
            * match variant {
                2 => 1.2,
                3 => 0.78,
                _ => 1.0,
            };
        let radius = (3.8 + random(seed, 2) * 2.1)
            * match variant {
                2 | 3 => 1.08,
                _ => 1.0,
            };
        let lean = [(random(seed, 3) - 0.5) * 2.0, (random(seed, 4) - 0.5) * 2.0];
        let top = [0.5 + lean[0], height, 0.5 + lean[1]];
        let middle = [0.5 + lean[0] * 0.25, height * 0.5, 0.5 + lean[1] * 0.25];
        let thickness = if variant == 3 { 1.35 } else { 1.0 };
        let mut wood = vec![
            WoodSegment::new([0.5, 0.0, 0.5], middle, [1.45 * thickness, 1.0 * thickness]),
            WoodSegment::new(middle, top, [1.0 * thickness, 1.3]),
        ];
        let rise = match variant {
            0 => 4.0 + random(seed, 6) * 1.2,
            1 => 2.9 + random(seed, 6) * 1.0,
            2 => 1.8 + random(seed, 6) * 0.6,
            3 => 2.4 + random(seed, 6) * 0.8,
            4 => 3.4 + random(seed, 6) * 0.8,
            _ => 3.0 + random(seed, 6) * 1.0,
        };
        let mut crowns = vec![Crown::mushroom_cap(
            top,
            [radius, rise, radius * (0.90 + random(seed, 5) * 0.18)],
            variant,
            random(seed, 7) * std::f32::consts::TAU,
        )];
        // Elbowed, rising arms support offset caps. Both segments overlap the
        // stalk and each other, including after rasterization at block centers.
        let arms = if random(seed, 8) < 0.24 {
            2
        } else if random(seed, 8) < 0.68 {
            1
        } else {
            0
        };
        for arm in 0..arms {
            let direction = DIRECTIONS[(((seed >> 8) as usize) + arm * 3) % 8];
            let reach = radius * (0.83 + random(seed, 10 + arm as u64) * 0.20);
            let attachment = [middle[0], height * (0.33 + arm as f32 * 0.14), middle[2]];
            let elbow = [
                attachment[0] + direction[0] * reach * 0.72,
                attachment[1] + 0.65,
                attachment[2] + direction[1] * reach * 0.72,
            ];
            let young = [
                attachment[0] + direction[0] * reach,
                height * (0.63 + arm as f32 * 0.12),
                attachment[2] + direction[1] * reach,
            ];
            wood.push(WoodSegment::new(attachment, elbow, [1.05, 0.96]));
            wood.push(WoodSegment::new(elbow, young, [0.96, 1.0]));
            let scale = 0.48 + random(seed, 12 + arm as u64) * 0.16;
            crowns.push(Crown::mushroom_cap(
                young,
                [radius * scale, rise * (scale + 0.08), radius * scale * 0.9],
                variant,
                random(seed, 14 + arm as u64) * std::f32::consts::TAU,
            ));
        }
        Self::from_shapes(
            origin,
            TreeSpecies::Mushroom(variant),
            TreeForm::Spreading,
            wood,
            crowns,
        )
    }
}
