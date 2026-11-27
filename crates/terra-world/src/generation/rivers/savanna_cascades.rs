//! Tributaries start in an existing shelf and run down its face into an open
//! river. Their reservoirs and free falls use the ordinary water simulation.
use super::{RiverSample, TerraGenerator};
use std::{collections::HashMap, sync::Mutex};

const TILE: i64 = 128;
const LENGTH: usize = 44;
const WIDTH: f64 = 2.4;

#[derive(Clone, Copy)]
pub(in crate::generation) struct Cascade {
    mouth: [i64; 2],
    uphill: [i64; 2],
    levels: [f64; LENGTH],
}

#[derive(Default)]
pub(in crate::generation) struct CascadeCache(Mutex<HashMap<[i64; 2], Option<Cascade>>>);

impl Cascade {
    fn sample(self, x: f64, z: f64) -> Option<RiverSample> {
        let dx = x - (self.mouth[0] as f64 + 0.5);
        let dz = z - (self.mouth[1] as f64 + 0.5);
        let [a, b] = self.uphill.map(|v| v as f64);
        let along = dx * a + dz * b;
        let across = (dz * a - dx * b).abs();
        let t = along.clamp(0., (LENGTH - 1) as f64);
        let distance = (along - t).hypot(across);
        if along < 0. || distance >= WIDTH + 3. {
            return None;
        }
        let index = t.floor() as usize;
        let level = self.levels[index];
        Some(RiverSample {
            level,
            distance,
            width: WIDTH,
            flow: [-a as f32, -b as f32],
            depth: 1.55,
            covered: false,
            cascade_height: 0.,
            roof_blend: 0.,
            roof_detail: 0.,
            bed_detail: 0.,
            island_height: None,
            bank_reach: 3.,
            bank_level: level,
            bank_erosion: 0.,
            fall_erosion: 0.,
            bounded_width: true,
        })
    }
}

impl TerraGenerator {
    pub(in crate::generation) fn savanna_cascade_at(
        &self,
        x: f64,
        z: f64,
        river: Option<RiverSample>,
        savanna: Option<f64>,
    ) -> Option<RiverSample> {
        if self.config().ecology_revision < 17
            || !self.config().rivers
            || !self.config().water
            || !x.is_finite()
            || !z.is_finite()
            || river.is_some_and(|r| r.distance < r.width && r.bed_height() + 0.5 < r.level)
            || savanna.unwrap_or_else(|| self.climate_biome_at(x, z).weights.savanna) < 0.42
        {
            return None;
        }
        let cell = [x, z].map(|v| (v.floor() as i64).div_euclid(TILE));
        let local = [x, z].map(|v| (v.floor() as i64).rem_euclid(TILE));
        // A tributary is owned by its receiving river tile. Check the adjacent
        // owner as well if its source could extend across this tile boundary.
        let owners = local.map(|n| {
            if n < LENGTH as i64 + 6 {
                -1
            } else if TILE - n < LENGTH as i64 + 6 {
                1
            } else {
                0
            }
        });
        let mut nearest: Option<RiverSample> = None;
        for dx in 0.min(owners[0])..=0.max(owners[0]) {
            for dz in 0.min(owners[1])..=0.max(owners[1]) {
                let cell = [cell[0] + dx, cell[1] + dz];
                let cached = self.cascades.0.lock().unwrap().get(&cell).copied();
                let cascade = cached.unwrap_or_else(|| {
                    let candidate = self.plan_savanna_cascade(cell);
                    let mut cache = self.cascades.0.lock().unwrap();
                    if cache.len() >= 4096 {
                        cache.clear()
                    }
                    *cache.entry(cell).or_insert(candidate)
                });
                if let Some(s) = cascade.and_then(|c| c.sample(x, z))
                    && nearest.is_none_or(|n| s.distance < n.distance)
                {
                    nearest = Some(s);
                }
            }
        }
        nearest
    }

    fn plan_savanna_cascade(&self, tile: [i64; 2]) -> Option<Cascade> {
        let origin = tile.map(|v| v * TILE);
        let mut best = None;
        let mut highest = 6.;
        for z in (2..TILE).step_by(7) {
            for x in (2..TILE).step_by(7) {
                let mouth = [origin[0] + x, origin[1] + z];
                let [px, pz] = mouth.map(|v| v as f64 + 0.5);
                let Some(river) = self.river_course_at(px, pz) else {
                    continue;
                };
                if river.distance > river.width * 0.78
                    || river.covered
                    || river.roof_blend > 0.
                    || self.climate_biome_at(px, pz).weights.savanna < 0.70
                {
                    continue;
                }
                for uphill in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                    let coord = |i, side| {
                        let x = px + (uphill[0] * i - uphill[1] * side) as f64;
                        let z = pz + (uphill[1] * i + uphill[0] * side) as f64;
                        [x, z]
                    };
                    let ground = |i, side| {
                        let [x, z] = coord(i, side);
                        let raw = self.watershed_height(x, z);
                        self.river_course_at(x, z).map_or(raw, |r| {
                            super::super::landforms::river_height(raw, r, self.config().sea_level)
                        })
                    };
                    if self.config().ecology_revision >= 21
                        && (0..LENGTH).step_by(4).any(|i| {
                            [-5, 0, 5].into_iter().any(|side| {
                                let [x, z] = coord(i as i64, side);
                                self.volcanic_watershed_at(x, z)
                            })
                        })
                    {
                        continue;
                    }
                    if ground(LENGTH as i64, 0) - river.level < highest + 2. {
                        continue;
                    }
                    let mut levels = [river.level; LENGTH];
                    let mut cap = f64::INFINITY;

                    for i in (1..LENGTH).rev() {
                        // A spring cuts a bed into its host rock. Its highest
                        // possible head is below both existing lateral banks.
                        let support = [-5, -4, -3, 0, 3, 4, 5]
                            .map(|side| ground(i as i64, side))
                            .into_iter()
                            .fold(f64::INFINITY, f64::min);
                        cap = cap.min((support - 1.25).floor());
                        levels[i] = cap.max(river.level);
                    }
                    let relief = levels[LENGTH - 1] - river.level;
                    if relief <= highest {
                        continue;
                    }
                    let ledge = levels
                        .windows(5)
                        .enumerate()
                        .filter(|(_, span)| {
                            span[4] - span[0] >= 3.
                                && span[4] - river.level >= crate::water::MIN_WATERFALL_DROP
                        })
                        .max_by(|(_, a), (_, b)| (a[4] - a[0]).total_cmp(&(b[4] - b[0])))
                        .map(|(index, _)| index + 4);
                    if let Some(lip) = ledge {
                        // Plunge-pool scour concentrates the fall at the real
                        // slope break. Both its tailwater and the head remain
                        // below the adjacent rock and on the receiving course.
                        let pool = levels[lip] - crate::water::MIN_WATERFALL_DROP;
                        for stage in &mut levels[..lip] {
                            *stage = stage.min(pool);
                        }
                        best = Some(Cascade {
                            mouth,
                            uphill,
                            levels,
                        });
                        highest = relief;
                    }
                }
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use terra_core::{BiomeDefinitions, WorldConfig, WorldSeed};

    fn reported() -> TerraGenerator {
        TerraGenerator::new(WorldConfig {
            seed: WorldSeed(16892963061413013603),
            ecology_revision: 17,
            rivers: true,
            biome_definitions: Some(BiomeDefinitions::default()),
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn savanna_stream_descends_a_supported_bank_into_the_river_in_the_reported_province() {
        let terrain = reported();
        let mut found = Vec::new();
        // Both river bends to the east of the photographed waterhole.
        for z in 4..=8 {
            for x in 18..=20 {
                let Some(source) = terrain.plan_savanna_cascade([x, z]) else {
                    continue;
                };
                let [mx, mz] = source.mouth.map(|v| v as f64 + 0.5);
                let river = terrain
                    .surface_column(mx, mz)
                    .water_level
                    .expect("wet receiving channel");
                assert!((river - source.levels[0]).abs() < 0.01);
                let mut prior = river;
                for i in 1..LENGTH {
                    let x = mx + source.uphill[0] as f64 * i as f64;
                    let z = mz + source.uphill[1] as f64 * i as f64;
                    let c = terrain.surface_column(x, z);
                    let head = c
                        .water_level
                        .expect("continuous surface stream to the river");
                    assert!(
                        head >= prior - 0.55 && c.water_bed() < head,
                        "{} / {}: {prior} -> {head}",
                        x,
                        z
                    );
                    for side in [-4., 4.] {
                        let nx = x - source.uphill[1] as f64 * side;
                        let nz = z + source.uphill[0] as f64 * side;
                        let bank = terrain.surface_column(nx, nz);
                        assert!(
                            bank.water_level.is_some() || bank.height + 1. >= head,
                            "hanging stream at {x}/{z}, head={head}, bank={}",
                            bank.height
                        );
                    }
                    prior = head;
                }
                assert!(prior - river >= 6.);
                assert!(
                    source
                        .levels
                        .windows(2)
                        .any(|s| s[1] - s[0] >= crate::water::MIN_WATERFALL_DROP)
                );
                found.push((source.mouth, source.uphill, river, prior));
            }
        }
        assert!(!found.is_empty());
        println!("Savanna falls (mouth, uphill, lower and upper levels): {found:?}");
    }

    #[test]
    fn reported_gully_has_a_daylit_soil_surface_and_waterhole_has_no_raised_plinth() {
        let terrain = reported();
        for z in 1076..1090 {
            let x = 1485.5;
            let z = f64::from(z) + 0.5;
            let c = terrain.surface_column(x, z);
            assert!(c.height > 59. && c.cave_cavity.is_none(), "{x}/{z}: {c:?}");
            let y = (c.height - 0.5).floor() + 0.5;
            assert!(terrain.sample_with_column([x, y, z], c).density <= 0.);
            assert!(
                terrain.sample_with_column([x, y + 1., z], c).density > 0.,
                "{x}/{z}: y={y}, c={c:?}"
            );
            assert_eq!(
                terrain.sample_with_column([x, y, z], c).material,
                c.surface_material as u16
            );
        }
        for z in 943..951 {
            for x in 2097..2105 {
                let (x, z) = (f64::from(x) + 0.5, f64::from(z) + 0.5);
                let c = terrain.surface_column(x, z);
                assert!(
                    c.water_level.is_none_or(
                        |head| head <= 61. || head <= terrain.watershed_height(x, z) - 0.5
                    ),
                    "elevated head {} above natural support {}",
                    c.water_level.unwrap_or(0.),
                    terrain.watershed_height(x, z)
                );
            }
        }
    }

    #[test]
    fn freshwater_cannot_form_a_suspended_wall_in_the_volcanic_valley() {
        let terrain = reported();
        let mut volcanic = 0;
        for x in (-5480..-5300).step_by(2) {
            for z in (-1046..-1015).step_by(2) {
                let x = f64::from(x) + 0.5;
                let z = f64::from(z) + 0.5;
                let c = terrain.surface_column(x, z);
                if c.biome.weights.ashlands < 0.8 {
                    continue;
                }
                volcanic += 1;
                let Some(level) = c.water_level else { continue };
                for [dx, dz] in [[1., 0.], [-1., 0.], [0., 1.], [0., -1.]] {
                    let bank = terrain.surface_column(x + dx, z + dz);
                    assert!(
                        bank.water_level.is_some() || bank.water_bed() + 1.0 >= level,
                        "perched head at {x}/{z}, {level} vs bank {}",
                        bank.height
                    );
                }
            }
        }
        assert!(volcanic > 1000);
    }
}
