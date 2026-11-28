//! Short perennial headwaters spill from savanna ledges into their waterholes.
//! Both reservoirs use normal source cells. Their open shared lip wakes the
//! same flowing-water scheduler that supplies river waterfalls.
use super::{Oasis, TerraGenerator, smoothstep};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Spring {
    lip: [f64; 2],
    outward: [f64; 2],
    rise: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SpringSample {
    along: f64,
    across: f64,
    rise: f64,
    downhill: [f32; 2],
}

impl Spring {
    pub(super) fn on_bank(terrain: &TerraGenerator, oasis: Oasis) -> Option<Self> {
        [[0., -1.], [1., 0.], [0., 1.], [-1., 0.]]
            .into_iter()
            .filter_map(|outward| {
                // The irregular shore is star-shaped about its centre, so one
                // directional sample gives its exact radius along this bank.
                let [cx, cz] = oasis.center;
                let ray = oasis.sample(cx + outward[0], cz + outward[1]).distance;
                let radius = oasis.radius / ray;
                let lip = [
                    (cx + outward[0] * radius).floor(),
                    (cz + outward[1] * radius).floor(),
                ];
                let ground = |along: f64, across: f64| {
                    terrain.watershed_height(
                        lip[0] + outward[0] * along - outward[1] * across,
                        lip[1] + outward[1] * along + outward[0] * across,
                    )
                };
                // A waterhole alone cannot manufacture a waterfall. Require a
                // rock shelf under the lip and a separate hillside behind it.
                let support = [0.5, 3.5, 6.5, 9.5]
                    .into_iter()
                    .flat_map(|along| [-3., 0., 3.].map(|across| ground(along, across)))
                    .fold(f64::INFINITY, f64::min);
                let rise = (support - oasis.level - 1.5).floor();
                (rise >= crate::water::MIN_WATERFALL_DROP && ground(7., 0.) - ground(-5., 0.) >= 3.)
                    .then_some(Self {
                        lip,
                        outward,
                        rise: rise.min(16.),
                    })
            })
            .max_by(|a, b| a.rise.total_cmp(&b.rise))
    }
    pub(super) fn sample(self, x: f64, z: f64) -> Option<SpringSample> {
        let dx = x - self.lip[0];
        let dz = z - self.lip[1];
        let [a, b] = self.outward;
        let along = dx * a + dz * b;
        let across = (dz * a - dx * b).abs();
        ((-7.0..15.).contains(&along) && across < 10.).then_some(SpringSample {
            along,
            across,
            rise: self.rise,
            downhill: [-a as f32, -b as f32],
        })
    }
}

impl SpringSample {
    fn stream(self) -> bool {
        self.along >= 0.
            && ((self.along <= 6. && self.across < 1.8)
                || (self.along - 6.).hypot(self.across) < 3.)
    }
    fn plunge(self) -> bool {
        self.along < 0. && self.across < 3.5
    }
    pub(super) fn water_level(self, level: f64) -> Option<f64> {
        if self.stream() {
            Some(level + self.rise)
        } else if self.plunge() {
            Some(level)
        } else {
            None
        }
    }
    pub(crate) fn flow(self) -> Option<[f32; 2]> {
        (self.stream() || self.plunge()).then_some(self.downhill)
    }
    pub(super) fn terrain(self, basin: f64, raw: f64, level: f64) -> f64 {
        if self.stream() {
            return raw.min(level + self.rise - 1.8);
        }
        if self.plunge() {
            return basin.min(level - 1.8);
        }
        if self.along < 0. {
            return basin;
        }
        let strength =
            (1. - smoothstep(3.5, 10., self.across)) * (1. - smoothstep(9., 15., self.along));
        // Preserve the natural bank around the cut. It can never rise above its parent hillside.
        basin + (raw - basin).max(0.) * strength
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SectionGenerator;
    use terra_core::{BiomeDefinitions, WorldConfig};

    #[test]
    fn savanna_cliff_springs_feed_a_real_downhill_outlet_into_the_lower_pool() {
        let terrain = TerraGenerator::new(WorldConfig {
            biome_definitions: Some(BiomeDefinitions::default()),
            ..Default::default()
        })
        .unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        // The outlet from this natural ledge drops eight metres into its basin.
        let region = [-19i64, -11];
        let [cx, cz] = region.map(|n| n * 384 + 192);
        terrain.oasis_at(cx as f64, cz as f64).expect("waterhole");
        let basin = terrain.oases.0.lock().unwrap()[&region].unwrap();
        let spring = basin.spring.expect("grounded hillside spring");
        let [x, z] =
            std::array::from_fn(|i| (spring.lip[i] + spring.outward[i] * 0.5).floor() as i64);
        let sample = |x: i64, z: i64| terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
        let upper = sample(x, z);
        let high = upper.water_level.unwrap();
        let [dx, dz] = spring.outward.map(|v| -(v as i64));
        let low = sample(x + dx, z + dz).water_level.unwrap();
        assert_eq!(low, basin.level);
        assert!(high - low >= crate::water::MIN_WATERFALL_DROP);
        let top = high as i64 - 1;
        let neighbors = |[a, b]: [i64; 2]| sample(x + a, z + b);
        let outlets =
            crate::water::natural_outlets([x, z], top.div_euclid(16), upper, neighbors, |p| {
                generator.block_at(p).unwrap()
            });
        assert!(
            outlets.contains(&[x + dx, top, z + dz]),
            "the simulated waterfall must have an open lip"
        );
        let p = [x + dx, top, z + dz];
        let section = terra_voxel::global_to_section(p).0;
        let prepared = generator
            .prepare_column([section.x, section.z], true)
            .unwrap();
        let destination = prepared.surface_at(p[0], p[2]).unwrap();
        let span = destination.falls[0].expect("prepared discharge");
        assert_eq!(i64::from(span.top), top);
        assert_eq!(i64::from(span.bottom), low.ceil() as i64);
        let mut journal = crate::edits::EditJournal::default();
        let checksum = journal.fluid_cells().count();
        for y in i64::from(span.bottom)..=top {
            let cell = crate::water::falls::sampled(
                destination.water_columns(),
                [p[0], y, p[2]],
                &journal,
            );
            assert!(cell.amount > 0 && !cell.source, "dry at {y}");
        }
        for sy in i64::from(span.bottom).div_euclid(16)..=top.div_euclid(16) {
            let owner = terra_voxel::SectionPos::new(section.x, sy, section.z);
            let result = generator
                .build_prepared(owner, 0, &Default::default(), &prepared)
                .unwrap();
            let mesh = result
                .water
                .expect("first published section contains the waterfall");
            let origin = owner.origin().unwrap();
            let floor = i64::from(span.bottom).max(origin[1]) as f32;
            let has_bottom = mesh.positions.iter().any(|v| {
                (v[1] + origin[1] as f32 - floor).abs() < 0.15
                    && (v[0] + origin[0] as f32 - (p[0] as f32 + 0.5)).abs() < 0.7
                    && (v[2] + origin[2] as f32 - (p[2] as f32 + 0.5)).abs() < 0.7
            });
            assert!(
                has_bottom,
                "first remote mesh misses lower end of {owner:?}: {floor}"
            );
        }
        assert_eq!(journal.fluid_cells().count(), checksum);
        journal
            .record([x, top, z], terra_voxel::MaterialId::Stone as u16)
            .unwrap();
        assert_eq!(
            crate::water::falls::sampled(
                destination.water_columns(),
                [p[0], i64::from(span.bottom), p[2]],
                &journal
            )
            .amount,
            0
        );
        println!("Savanna fall on seed 42042 at ({x}, {high}, {z}) → {low}, outlet {dx}, {dz}");
    }
}
