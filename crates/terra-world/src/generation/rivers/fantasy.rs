//! Saved hydrology six: broad winding ribbons with connected downhill outlets.
use std::f64::consts::PI;

pub(super) fn meander(along: f64, fraction: f64, phase: f64, seed: u32) -> f64 {
    // A vanishing envelope preserves both real endpoints and their tangents.
    // No local random offsets: neighboring samples describe one smooth course.
    let envelope = (fraction.clamp(0.0, 1.0) * PI).sin().powi(2);
    let sweep = 68.0 + f64::from((seed >> 12) % 37);
    ((along * 0.0052 + phase).sin() * sweep + (along * 0.016 + phase * 1.73).sin() * 19.0)
        * envelope
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use terra_core::{WorldConfig, WorldSeed};

    #[test]
    fn sweeping_meanders_preserve_endpoints_and_have_continuous_tangents() {
        for seed in [7, 42042, 99173] {
            let length = 1600.0;
            let offset = |t| super::meander(t * length, t, 0.7, seed);
            assert_eq!(offset(0.0), 0.0);
            assert!(offset(1.0).abs() < 1e-10);
            assert!(offset(0.00001).abs() < 0.00001);
            assert!(offset(0.99999).abs() < 0.00001);
            assert!((0..100).any(|i| offset(i as f64 / 100.0).abs() > 55.0));
            for i in 0..100 {
                let t = i as f64 / 100.0;
                assert!((offset(t + 0.000001) - offset(t)).abs() < 0.002);
            }
        }
    }

    #[test]
    fn fantasy_rivers_remain_downhill_wet_and_connected_to_their_original_outlets() {
        let config = WorldConfig {
            seed: WorldSeed(42042),
            landscape_revision: 5,
            river_terrain_revision: 6,
            ecology_revision: 10,
            spirit_blossom_revision: 0,
            rivers: true,
            caves: false,
            trees: false,
            structures_revision: 0,
            ..Default::default()
        };
        let modern = TerraGenerator::new(config.clone()).unwrap();
        let old = TerraGenerator::new(WorldConfig {
            river_terrain_revision: 5,
            ..config
        })
        .unwrap();
        let mut checked = 0;
        let mut changed = 0;
        for z in -2..=2 {
            for x in -2..=2 {
                let at = [x as f64 * 2048.0 + 1024.0, z as f64 * 2048.0 + 1024.0];
                let (Some(a), Some(b)) = (
                    old.hydrology_region_at(at[0], at[1]),
                    modern.hydrology_region_at(at[0], at[1]),
                ) else {
                    continue;
                };
                assert_eq!([a.source[0], a.source[2]], [b.source[0], b.source[2]]);
                assert_eq!([a.mouth[0], a.mouth[2]], [b.mouth[0], b.mouth[2]]);
                assert!(b.reaches_sea || b.lake.is_some());
                assert!(b.course.windows(2).all(|p| p[1][1] <= p[0][1]));
                changed += usize::from(
                    a.course
                        .iter()
                        .zip(b.course)
                        .any(|(p, q)| (p[0] - q[0]).hypot(p[2] - q[2]) > 15.0),
                );
                for pair in b.course.windows(2) {
                    for step in 0..4 {
                        let t = step as f64 / 4.0;
                        let x = pair[0][0] + (pair[1][0] - pair[0][0]) * t;
                        let z = pair[0][2] + (pair[1][2] - pair[0][2]) * t;
                        let water = modern
                            .water_column(x, z)
                            .expect("connected fantasy channel must stay wet");
                        assert!(water.bed < water.level);
                        let river = modern.river_at(x, z).unwrap();
                        assert!(
                            (super::super::width::MIN_HALF_WIDTH
                                ..=super::super::width::MAX_HALF_WIDTH)
                                .contains(&river.width)
                        );
                    }
                }
                checked += 1;
                if checked == 3 {
                    break;
                }
            }
            if checked == 3 {
                break;
            }
        }
        assert!(
            checked >= 2 && changed >= 2,
            "missing real courses: {checked}/{changed}"
        );
    }
}
