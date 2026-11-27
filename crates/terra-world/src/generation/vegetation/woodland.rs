//! Mature oaks, small pole trees and broad orchard crowns share one branching skeleton.
use super::{
    DIRECTIONS, Tree,
    geometry::{Crown, WoodSegment},
    random,
    species::TreeSpecies,
    tree::TreeForm,
};

impl Tree {
    pub(super) fn woodland_broadleaf(
        origin: [i64; 3],
        seed: u64,
        roots: [f32; 4],
        species: TreeSpecies,
    ) -> Self {
        let apple = species == TreeSpecies::Apple;
        let maturity = random(seed, 33);
        let (height, reach, crown) = if apple {
            (
                10.0 + 8.5 * maturity,
                3.3 + 2.3 * maturity,
                2.35 + maturity * 0.95,
            )
        } else {
            (
                10.5 + 14.0 * maturity,
                3.6 + 4.1 * maturity,
                2.45 + maturity * 1.45,
            )
        };
        let (sin, cos) = (random(seed, 31) * std::f32::consts::TAU).sin_cos();
        let trunk = |t: f32| [0.5 + cos * 1.6 * t * t, height * t, 0.5 + sin * 1.6 * t * t];
        let mut wood = Vec::with_capacity(22);
        let mut crowns = Vec::with_capacity(16);
        let base = 1.02 + maturity * 0.89;
        for i in 0..3 {
            let a = i as f32 / 3.0;
            let b = (i + 1) as f32 / 3.0;
            wood.push(WoodSegment::new(
                trunk(a),
                trunk(b),
                [base * (1.0 - a) + 0.91 * a, base * (1.0 - b) + 0.91 * b],
            ));
        }
        for (i, ground) in roots.into_iter().enumerate() {
            let d = DIRECTIONS[i * 2];
            let foot = 2.0 + maturity * 2.0;
            wood.push(WoodSegment::new(
                trunk(0.13),
                [0.5 + d[0] * foot, ground + 0.2, 0.5 + d[1] * foot],
                [base * 0.94, 0.90],
            ));
        }
        for i in 0..7 {
            let angle = i as f32 * 2.39996 + random(seed, 31) * std::f32::consts::TAU;
            let (dz, dx) = angle.sin_cos();
            let level = i as f32 / 7.0;
            let start = trunk(if apple {
                0.28 + level * 0.53
            } else {
                0.40 + level * 0.48
            });
            let arm = reach * (1.04 - 0.36 * level);
            let elbow = [
                start[0] + dx * arm * 0.5,
                start[1] + 1.0,
                start[2] + dz * arm * 0.5,
            ];
            let tip = [
                start[0] + dx * arm,
                start[1] + height * 0.15,
                start[2] + dz * arm,
            ];
            wood.push(WoodSegment::new(start, elbow, [1.17, 1.0]));
            wood.push(WoodSegment::new(elbow, tip, [1.0, 0.91]));
            let r = crown * (0.9 + 0.18 * random(seed, i + 90));
            crowns.push(Crown::new(
                [tip[0], tip[1] + 0.6, tip[2]],
                [r, r * 0.83, r * 0.94],
            ));
            if i % 2 == 0 {
                crowns.push(Crown::new(
                    [elbow[0] - dz * 0.9, elbow[1] + 1.4, elbow[2] + dx * 0.9],
                    [r * 0.85, r * 0.72, r * 0.8],
                ));
            }
        }
        crowns.push(Crown::new(trunk(1.0), [crown, crown * 0.8, crown]));
        Self::from_shapes(origin, species, TreeForm::Spreading, wood, crowns)
    }

    pub(super) fn woodland_spruce(origin: [i64; 3], seed: u64) -> Self {
        let maturity = random(seed, 81);
        let height = 13.0 + 17.0 * maturity;
        let trunk = |t: f32| [0.5, height * t, 0.5];
        let mut wood = vec![WoodSegment::new(
            trunk(0.0),
            trunk(0.95),
            [1.05 + maturity * 0.52, 0.68],
        )];
        let mut crowns = Vec::with_capacity(8);
        for i in 0..8 {
            let level = 0.28 + i as f32 * 0.09;
            let width = (4.4 + maturity * 2.3) * (1.0 - level) + 0.65;
            let phase = random(seed, 50 + i) * std::f32::consts::TAU;
            let center = trunk(level);
            crowns.push(Crown::conifer_tier(
                center,
                [width, height * 0.13, width * 0.96],
                phase,
            ));
            if i % 2 == 0 {
                let (dz, dx) = phase.sin_cos();
                wood.push(WoodSegment::new(
                    trunk(level + 0.05),
                    [0.5 + dx * width * 0.67, center[1], 0.5 + dz * width * 0.67],
                    [0.99, 0.9],
                ));
            }
        }
        Self::from_shapes(origin, TreeSpecies::Spruce, TreeForm::Tiered, wood, crowns)
    }

    pub(super) fn tapered_conifer(origin: [i64; 3], seed: u64, species: TreeSpecies) -> Self {
        let maturity = random(seed, 81);
        let (height, width, trunk_radius) = if species == TreeSpecies::Fir {
            (21.0 + maturity * 10.0, 4.7 + maturity * 1.8, 0.92)
        } else {
            (
                13.0 + maturity * 17.0,
                4.4 + maturity * 2.6,
                1.05 + maturity * 0.52,
            )
        };
        let base = (height * 0.19).floor() + 0.5;
        let tip = height.floor() + 0.5;
        let phase = random(seed, 810) * std::f32::consts::TAU;
        let crown = Crown::conifer_tier([0.5, base, 0.5], [width, tip - base, width], phase);
        let mut wood = vec![WoodSegment::new(
            [0.5, 0.0, 0.5],
            [0.5, height * 0.65, 0.5],
            [trunk_radius, 0.48],
        )];
        // Keep the woody arms within the same envelope as the six sprays of needles.
        for tier in 0..3 {
            let t = tier as f32 * 0.20;
            let y = base + (tip - base) * t;
            for arm in 0..3 {
                let (dz, dx) =
                    (phase + (tier + arm * 2) as f32 * std::f32::consts::TAU / 6.).sin_cos();
                let reach = width * (1.0 - t) * 0.62;
                wood.push(WoodSegment::new(
                    [0.5, y + 0.5, 0.5],
                    [0.5 + dx * reach, y, 0.5 + dz * reach],
                    [0.84, 0.57],
                ));
            }
        }
        Self::from_shapes(origin, species, TreeForm::Tiered, wood, vec![crown])
    }
}

#[cfg(all(test, feature = "bevy"))]
mod tests {
    use super::super::planted_shape_with_ecology;
    use super::*;
    use std::collections::BTreeSet;
    use terra_core::TreeSpecies as Planted;

    #[test]
    fn forest_saplings_recreate_all_five_natural_crowns_within_streaming_bounds() {
        for (kind, species) in [
            (Planted::Oak, TreeSpecies::Green),
            (Planted::Birch, TreeSpecies::Birch),
            (Planted::Spruce, TreeSpecies::Spruce),
            (Planted::Apple, TreeSpecies::Apple),
            (Planted::Cherry, TreeSpecies::Cherry(false)),
        ] {
            let mut range = (i32::MAX, i32::MIN);
            for seed in [0, 4, 42, 107, 509, 1042, 42042] {
                let tree = Tree::with_species_revision([0; 3], seed, [0.; 4], species, 14);
                assert!(tree.bounds.min[0] >= -16 && tree.bounds.min[2] >= -16);
                assert!(
                    tree.bounds.max[0] <= 16
                        && tree.bounds.max[2] <= 16
                        && tree.bounds.max[1] <= 36
                );
                let height = tree.bounds.max[1];
                range = (range.0.min(height), range.1.max(height));
                let owned = tree.felling_shape().unwrap();
                let (wood, leaves) = planted_shape_with_ecology(kind, seed, 14).unwrap();
                for (planted, natural) in [(wood, owned.wood), (leaves, owned.leaves)] {
                    assert_eq!(
                        planted
                            .into_iter()
                            .map(|p| p.map(i64::from))
                            .collect::<BTreeSet<_>>(),
                        natural.into_iter().filter(|p| p[1] >= 0).collect()
                    );
                }
            }
            if matches!(kind, Planted::Oak | Planted::Apple | Planted::Spruce) {
                assert!(range.1 - range.0 >= 5, "{species:?}: {range:?}");
            }
        }
    }
}
