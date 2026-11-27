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

    /// Two open crown silhouettes share a bowed trunk and individually arched pinnae.
    /// Each frond is sampled in four short capsules, with its mesh aligned to the rachis.
    pub(super) fn oasis_palm(origin: [i64; 3], seed: u64, voxel_leaves: bool) -> Self {
        use terra_voxel::MaterialId as M;
        let tall = random(seed, 5) < 0.53;
        let height = if tall { 15.0 } else { 8.0 } + random(seed, 1) * 3.8;
        let lean = if tall { 3.6 } else { 1.8 } + random(seed, 2) * 1.8;
        let (dz, dx) = (random(seed, 3) * std::f32::consts::TAU).sin_cos();
        let trunk = |t: f32| {
            let curve = lean * (1.65 * t - 0.65 * t * t);
            [0.5 + dx * curve, height * t, 0.5 + dz * curve]
        };
        let mut wood = Vec::with_capacity(5);
        for step in 0..5 {
            let a = step as f32 / 5.0;
            let b = (step + 1) as f32 / 5.0;
            wood.push(WoodSegment::new(
                trunk(a),
                trunk(b),
                [1.02 - 0.15 * a, 1.02 - 0.15 * b],
            ));
        }
        let top = trunk(1.0);
        let arms = if tall { 8 } else { 6 };
        let axes = [
            M::PalmFrondEast,
            M::PalmFrondNorthEast,
            M::PalmFrondNorth,
            M::PalmFrondNorthWest,
        ];
        let mut crowns = Vec::with_capacity(arms * 4);
        let phase = random(seed, 11) * std::f32::consts::TAU;
        for i in 0..arms {
            let ch = 50 + i as u64 * 3;
            let angle = phase
                + (i as f32 + (random(seed, ch) - 0.5) * 0.22) * std::f32::consts::TAU
                    / (arms as f32);
            let (z, x) = angle.sin_cos();
            let material =
                axes[((angle / std::f32::consts::FRAC_PI_4).round() as i32).rem_euclid(4) as usize];
            let reach = if tall { 7.8 } else { 5.0 } + random(seed, ch + 1) * 1.9;
            let fall = if i % 2 == 0 { 3.8 } else { 1.8 };
            let mut last = top;
            for step in 1..=4 {
                let t = step as f32 / 4.0;
                let next = [
                    top[0] + x * reach * t,
                    top[1] + (std::f32::consts::PI * t).sin() * (1.7 + random(seed, ch + 2))
                        - fall * t * t,
                    top[2] + z * reach * t,
                ];
                crowns.push(Crown::palm_segment(
                    last,
                    next,
                    [0.90 - 0.18 * (t - 0.25), 0.90 - 0.18 * t],
                    (!voxel_leaves).then_some((material, [t - 0.25, t])),
                ));
                last = next;
            }
        }
        let mut palm =
            Self::from_shapes(origin, TreeSpecies::Palm, TreeForm::Spreading, wood, crowns);
        if !voxel_leaves {
            palm.leaves_material = M::PalmFrondEast;
        }
        palm
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

    pub(super) fn park_mushroom(
        origin: [i64; 3],
        seed: u64,
        roots: [f32; 4],
        mature_canopies: bool,
    ) -> Self {
        let variant = match seed % 100 {
            0..=45 => 1,  // scarlet fly agarics
            46..=72 => 5, // cream parasols
            73..=88 => 6, // chestnut boletes
            89..=97 => 7, // luminous lilac
            _ => 0,
        };
        let fallen = seed.is_multiple_of(13);
        let stature = random(seed, 6);
        let mature = mature_canopies && !fallen && random(seed, 40) < 0.44;
        let (height, radius, rise_ratio, foot, waist, collar) = if mature {
            // An upper canopy of broad parasols and taller domed fly agarics.
            // The largest rim and leaning stem fit the 16-cell section search halo.
            let parasol = variant == 5 || variant == 6;
            (
                20.0 + stature * 8.2,
                10.4 + random(seed, 41) * 2.6,
                if parasol { 0.30 } else { 0.43 },
                2.65,
                1.45 + stature * 0.45,
                2.65,
            )
        } else {
            (
                8.0 + stature * 12.0,
                if fallen { 4.0 } else { 4.2 + stature * 4.7 },
                0.40 + random(seed, 22) * 0.10,
                1.8,
                1.05 + 0.45 * stature,
                1.55,
            )
        };
        let side = if seed & 1 == 0 { 1.0 } else { -1.0 };
        let top = if fallen {
            [0.5 + 8.4 * side, 4.0, 0.5]
        } else {
            [
                0.5 + (random(seed, 9) - 0.5) * 2.8,
                height,
                0.5 + (random(seed, 10) - 0.5) * 2.5,
            ]
        };
        let base = [0.5, 0., 0.5];
        let bend = [
            0.5 + (top[0] - 0.5) * 0.35,
            top[1] * 0.45,
            0.5 + (top[2] - 0.5) * 0.68,
        ];
        let mut wood = vec![
            WoodSegment::new(base, bend, [foot, waist]),
            WoodSegment::new(bend, top, [waist, collar]),
        ];
        for (i, [dx, dz]) in [[1., 0.], [0., 1.], [-1., 0.], [0., -1.]]
            .into_iter()
            .enumerate()
        {
            let toe = [0.5 + dx * 3.6, roots[i] - 0.2, 0.5 + dz * 3.6];
            wood.push(WoodSegment::new([0.5, 2.4, 0.5], toe, [1.1, 0.82]));
        }
        let angle = if fallen {
            -1.17 * side
        } else {
            (random(seed, 24) - 0.5) * 0.15
        };
        let mut caps = vec![Crown::park_cap(
            top,
            radius,
            radius * rise_ratio,
            angle,
            random(seed, 23) * std::f32::consts::TAU,
        )];
        if !fallen && random(seed, 31) < 0.34 {
            let joint = [0.5 + side * 2.0, 2.0, 0.5];
            let young = [0.5 + side * 4.5, 4.7, 0.5 + random(seed, 32)];
            wood.push(WoodSegment::new(base, joint, [1.15, 0.95]));
            wood.push(WoodSegment::new(joint, young, [0.95, 0.9]));
            caps.push(Crown::park_cap(young, 2.8, 1.45, -side * 0.15, 1.8));
        }
        Self::from_shapes(
            origin,
            TreeSpecies::Mushroom(variant),
            TreeForm::Spreading,
            wood,
            caps,
        )
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
