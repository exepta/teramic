//! The molten drainage network and its basalt banks share one elevation field.
//! Liquids have a level upper surface and a rounded, independently mineable bed.
use super::{
    TerraGenerator,
    biomes::{BiomeSample, smoothstep},
    seed::layer_seed,
};
use noise::NoiseFn;
use terra_voxel::MaterialId as M;

const CONE_SPACING: f64 = 416.;

#[derive(Debug, Clone, Copy)]
struct Cone {
    center: [f64; 2],
    radius: f64,
    rise: f64,
    active: bool,
    seed: u32,
}

struct Spillway {
    profile: f64,
    across: f64,
    axis: [f64; 2],
}

impl Cone {
    fn bearing(self, r: f64) -> (f64, f64) {
        let bearing = f64::from((self.seed >> 8) & 1023) * std::f64::consts::TAU / 1024.;
        (bearing + (r * 6.).sin() * 0.16).sin_cos()
    }

    /// One breached rim carries an active lake down the flank. Both the cut
    /// and its molten fill sample this same seeded channel, including at seams.
    fn spillway(self, x: f64, z: f64, r: f64) -> Option<Spillway> {
        if !self.active || !(0.055..1.).contains(&r) {
            return None;
        }
        let (sin, cos) = self.bearing(r);
        let dx = x - self.center[0];
        let dz = (z - self.center[1]) * 1.04;
        if dx * cos + dz * sin <= 0. {
            return None;
        }
        let across = (dx * sin - dz * cos).abs();
        let profile = (1. - smoothstep(1.4, 7.5, across)) * (1. - smoothstep(0.78, 1., r));
        // Follow the upper cone's talus. The outlet is a shallow notch
        // below the summit and gradually widens into its downstream bed.
        let axis = [
            self.center[0] + cos * r * self.radius,
            self.center[1] + sin * r * self.radius / 1.04,
        ];
        (across < 13.).then_some(Spillway {
            profile,
            across,
            axis,
        })
    }
    fn distance(self, x: f64, z: f64) -> f64 {
        (x - self.center[0]).hypot((z - self.center[1]) * 1.04) / self.radius
    }
}

impl TerraGenerator {
    fn volcano_descriptor(&self, x: f64, z: f64) -> Cone {
        let tile = [
            (x / CONE_SPACING).floor() as i64,
            (z / CONE_SPACING).floor() as i64,
        ];
        let key = self.config().seed.0
            ^ (tile[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
            ^ (tile[1] as u64).wrapping_mul(0x94d049bb133111eb);
        let seed = layer_seed(key, 8318);
        let center = [0, 1].map(|axis| {
            (tile[axis] as f64 + 0.34 + f64::from((seed >> (axis * 8)) & 255) / 800.) * CONE_SPACING
        });
        Cone {
            center,
            radius: 79. + f64::from((seed >> 16) & 31),
            rise: 57.
                + f64::from(seed >> 27)
                + if self.config().ecology_revision >= 20 {
                    9.
                } else {
                    0.
                },
            active: seed & (1 << 23) != 0,
            seed,
        }
    }

    /// The candidate fits inside its own tile. An eruption requires a real,
    /// strongly volcanic crater with a generated molten vent.
    pub fn eruptive_vent(&self, x: f64, z: f64) -> Option<crate::volcano::Vent> {
        if self.config().ecology_revision < 16 || self.config().landscape_revision < 9 {
            return None;
        }
        let cone = self.volcano_descriptor(x, z);
        if !cone.active || cone.distance(x, z) * cone.radius > 156. {
            return None;
        }
        let [cx, cz] = cone.center.map(|v| v.floor() + 0.5);
        let level = self.lava_surface(cx, cz)?;
        (level > self.lava_datum()).then_some(crate::volcano::Vent {
            origin: [cx, level, cz],
            seed: cone.seed,
        })
    }

    pub(super) fn with_ashlands(&self, mut biome: BiomeSample, x: f64, z: f64) -> BiomeSample {
        let scale = self.config().biome_scale.max(360.) * 0.65;
        let province = self.valleys.get([x / scale + 32.71, z / scale - 19.83]);
        let eligible = smoothstep(0.43, 0.60, biome.temperature)
            * (1.0 - smoothstep(0.68, 0.82, biome.humidity));
        let fraction = eligible * smoothstep(0.26, 0.54, province);
        let w = &mut biome.weights;
        // The province takes up its share of warm land only. Mountain and sea
        // weights continue to drive the same coastlines and cold/high buffers.
        for source in [
            &mut w.plains,
            &mut w.forest,
            &mut w.desert,
            &mut w.cliffted_desert,
        ] {
            let transferred = *source * fraction;
            *source -= transferred;
            w.ashlands += transferred;
        }
        biome
    }

    /// Coastal provinces share their climate weight with ocean even on dry land.
    /// Recover the volcanic fraction on the landward side and fade it out at sea.
    pub(super) fn volcanic_strength(&self, biome: BiomeSample) -> f64 {
        let w = biome.weights;
        if self.config().ecology_revision < 18 || w.ashlands == 0. {
            return w.ashlands;
        }
        let coast = (1. - smoothstep(0.48, 0.72, w.ocean)) * w.ocean;
        w.ashlands / (1. - coast)
    }

    /// The hot province includes its transitional basalt shore. Coastal sea
    /// water and explicitly enclosed mineral springs have independent sources.
    pub(super) fn volcanic_watershed(&self, climate: BiomeSample) -> bool {
        self.config().ecology_revision >= 21 && self.volcanic_strength(climate) > 0.36
    }

    pub(super) fn volcanic_watershed_at(&self, x: f64, z: f64) -> bool {
        self.config().ecology_revision >= 21
            && self.config().landscape_revision >= 9
            && self.volcanic_watershed(self.climate_biome_at(x, z))
    }

    pub(super) fn volcanic_basin(&self, x: f64, z: f64, radius: f64) -> bool {
        if self.config().ecology_revision < 21 || self.config().landscape_revision < 9 {
            return false;
        }
        self.volcanic_watershed_at(x, z)
            || (0..8).any(|index| {
                let (sin, cos) = (f64::from(index) * std::f64::consts::FRAC_PI_4).sin_cos();
                self.volcanic_watershed_at(x + radius * cos, z + radius * sin)
            })
    }

    fn lava_datum(&self) -> f64 {
        (self.config().sea_level + 18.).round()
    }

    fn volcanic_valley(&self, x: f64, z: f64) -> (f64, f64) {
        let warp = self.hills.get([x * 0.010 + 3.92, z * 0.010 - 2.35]);
        let channel = self
            .valleys
            .get([x / 138. + warp * 0.11 + 5.21, z / 96. - 7.34])
            .abs();
        let basin = self.mountains.get([x / 102. + 72.6, z / 112. - 11.4]);
        let channel_bank = smoothstep(0.018, 0.19, channel);
        let lake_bank = 1. - smoothstep(0.39, 0.59, basin);
        let banks = channel_bank.min(lake_bank);
        let escarpment = self.hills.get([x / 55. + 38.6, z / 79. - 8.3]) + warp * 0.18;
        let ledges = smoothstep(0.04, 0.13, escarpment) * 5.0
            + smoothstep(0.24, 0.34, escarpment) * 8.
            + smoothstep(0.42, 0.50, escarpment) * 11.;
        let datum = self.lava_datum();
        let bed = datum - 4. - smoothstep(0.52, 0.72, basin) * 4.;
        // Outcrops of prismatically fractured basalt step out of the ash at
        // the shoulder of the valley. Riverbeds keep a common hydraulic datum.
        let ribs = if self.config().ecology_revision >= 20 {
            let sill = self.hills.get([x / 29. + 68.6, z / 51. - 72.1]);
            4.0 * smoothstep(0.14, 0.31, sill) * smoothstep(0.0, 0.38, escarpment)
        } else {
            0.
        };
        let valley = bed + (datum + 4. + warp * 2. + ledges + ribs - bed) * banks;
        (valley, warp)
    }

    /// Every block across the same bend uses a common hydraulic head. The
    /// valley sampled on the channel axis carries the outlet over basalt shelves.
    fn spillway_level(&self, cone: Cone, r: f64, axis: [f64; 2]) -> f64 {
        let datum = self.lava_datum();
        let talus = (1. - smoothstep(0.16, 1., r)).powf(1.48);
        let (valley, _) = self.volcanic_valley(axis[0], axis[1]);
        let outlet = datum + (valley - 1.5 - datum).max(0.) * (1. - smoothstep(0.75, 0.98, r));
        (outlet + (datum + cone.rise - 9. - outlet) * talus).round()
    }

    pub(super) fn volcanic_height(&self, x: f64, z: f64, surrounding: f64, weight: f64) -> f64 {
        if weight <= 0.04 || self.config().mountain_amplitude + self.config().hill_amplitude == 0. {
            return surrounding;
        }
        let datum = self.lava_datum();
        let (valley, warp) = self.volcanic_valley(x, z);
        // An isolated cone has a broad talus foot, steep rim and open caldera.
        // The centre stays far enough from its tile edge for the silhouette to
        // reach zero before the neighbouring tile takes ownership.
        let cone = self.volcano_descriptor(x, z);
        let r = cone.distance(x, z);
        let mut volcanic = valley;
        if r < 1. {
            let summit = datum + cone.rise;
            let talus = (1. - smoothstep(0.16, 1., r)).powf(1.48);
            let buttresses = if self.config().ecology_revision >= 20 {
                // Long radial ridges border the breached caldera. Their relief
                // dies into the crater lip and the scree fan at the mountain foot.
                let azimuth = (z - cone.center[1]).atan2(x - cone.center[0]);
                let radial = (azimuth * 9. + warp * 0.9).cos();
                9. * smoothstep(0.10, 0.95, radial)
                    * smoothstep(0.15, 0.34, r)
                    * (1. - smoothstep(0.69, 1., r))
            } else {
                0.
            };
            let rim = valley + (summit - valley + warp * 5.) * talus + buttresses;
            let floor = if cone.active {
                datum + 14.
            } else {
                // A small residual pool amid terraced, exposed crater rock.
                datum + 8. + smoothstep(0.022, 0.093, r) * 6.
            };
            volcanic = floor + (rim - floor) * smoothstep(0.08, 0.165, r);
            if let Some(channel) = cone.spillway(x, z, r) {
                let head = self.spillway_level(cone, r, channel.axis);
                let bed = head - 2.;
                volcanic -= (volcanic - bed).max(0.) * channel.profile;
                // Build the outer basalt shoulders even when the valley to
                // either side is much lower than the channel on this flank.
                let shoulder = smoothstep(2.7, 5.3, channel.across)
                    * (1. - smoothstep(7.6, 13., channel.across))
                    * smoothstep(0.08, 0.15, r)
                    * (1. - smoothstep(0.92, 1., r));
                volcanic += (head + 4. - volcanic).max(0.) * shoulder;
                // The outer crust makes a levee. Give a dry cell enough
                // freeboard for the higher liquid face one step upstream.
                if volcanic >= head - 0.5 {
                    let lip = 3.0 * smoothstep(0.0, 0.28, channel.profile);
                    volcanic = volcanic.max(head + lip);
                }
            }
        }
        let height = surrounding + (volcanic - surrounding) * smoothstep(0.04, 0.92, weight);
        // Bring the basalt bed through the lava surface before the climate
        // boundary. Otherwise a deep pool ends in a suspended vertical wall.
        let bank = smoothstep(0.65, 0.73, weight) * (1. - smoothstep(0.75, 0.83, weight));
        height + (datum + 1.5 - height).max(0.) * bank
    }

    pub(super) fn volcanic_lava_level(
        &self,
        x: f64,
        z: f64,
        weight: f64,
        height: f64,
        river: Option<super::RiverSample>,
        lake: Option<super::OasisSample>,
    ) -> Option<f64> {
        if weight <= 0.74 {
            return None;
        }
        let cone = self.volcano_descriptor(x, z);
        let mut level = self.lava_datum();
        let r = cone.distance(x, z);
        if let Some(channel) = cone.spillway(x, z, r).filter(|s| s.profile > 0.) {
            level = self.spillway_level(cone, r, channel.axis);
        } else if r < 0.16 {
            level += if cone.active { cone.rise - 9. } else { 10. };
        }
        let bed = (height - 0.5).floor() + 1.;
        // Test the actual freshwater column. A carved bank outside the pool
        // used to subtract a four-block-wide dry strip from a neighbouring flow.
        let freshwater = lake.filter(|l| l.distance <= l.radius).map_or_else(
            || {
                river.map_or(self.config().sea_level, |r| {
                    r.water_level(self.config().sea_level)
                })
            },
            |l| l.level,
        );
        (bed < level
            && self.config().mountain_amplitude + self.config().hill_amplitude > 0.
            && (!self.config().water
                || bed >= freshwater && river.is_none_or(|r| r.cavity().is_none())))
        .then_some(level)
    }

    /// Natural molten surface in world coordinates. Player edits can override its voxels.
    pub fn lava_surface(&self, x: f64, z: f64) -> Option<f64> {
        let climate = self.climate_biome_at(x, z);
        let strength = self.volcanic_strength(climate);
        if strength <= 0.74 {
            return None;
        }
        self.volcanic_lava_level(
            x,
            z,
            strength,
            self.height_with_biome(x, z, climate),
            self.river_at(x, z),
            self.oasis_at(x, z).or_else(|| self.lake_at(x, z)),
        )
    }

    /// Tangent of the natural outflow. Level reservoirs retain local convection.
    pub fn lava_current(&self, x: f64, z: f64) -> [f32; 2] {
        let cone = self.volcano_descriptor(x, z);
        let r = cone.distance(x, z);
        let Some(channel) = cone.spillway(x, z, r) else {
            return [0.; 2];
        };
        let (sin, cos) = cone.bearing(r);
        let turn = r * 0.96 * (r * 6.).cos();
        [
            (channel.profile * (cos - turn * sin)) as f32,
            (channel.profile * (sin + turn * cos) / 1.04) as f32,
        ]
    }

    pub(super) fn volcanic_surface(
        &self,
        x: f64,
        z: f64,
        weight: f64,
        slope: f64,
        height: f64,
    ) -> Option<M> {
        if weight < 0.20 {
            return None;
        }
        let patch = self.hills.get([x / 16. + 2.71, z / 21. - 14.16]);
        if weight < 0.82 && smoothstep(0.20, 0.82, weight) <= 0.5 + 0.48 * patch {
            return None;
        }
        if self.config().ecology_revision >= 21 && weight > 0.64 {
            // Cooling bands mark short pahoehoe lobes across gravelly slopes.
            // The outer rinds blend into scoria fans near the hotter valley floor.
            let flow = self.valleys.get([x / 33. + 44.2, z / 44. - 30.6]);
            let lobe = (x * 0.19 + z * 0.071 + 6.8 * flow + 1.4 * patch).sin();
            if height < self.lava_datum() + 11. && flow > 0.06 && slope < 1.4 {
                if lobe > 0.74 {
                    return Some(M::VolcanicObsidian);
                }
                if lobe > 0.28 {
                    return Some(M::LavaCrust);
                }
            }
            if slope > 0.82 && ((height - self.lava_datum()) * 0.68 + flow * 3.1).sin() > 0.83 {
                return Some(M::CompactedAsh);
            }
        }
        if weight > 0.74 {
            let cone = self.volcano_descriptor(x, z);
            if !cone.active && cone.distance(x, z) < 0.15 {
                return Some(if patch > 0.27 {
                    M::EmberCrystalOre
                } else {
                    M::DarkVolcanicStone
                });
            }
        }
        Some(
            if self.config().ecology_revision >= 20
                && patch < -0.38
                && height <= self.lava_datum() + 3.
            {
                M::VolcanicObsidian
            } else if height <= self.lava_datum() + 1. {
                if patch > -0.1 {
                    M::LavaCrust
                } else {
                    M::DarkVolcanicStone
                }
            } else if slope > 1.65 {
                if patch > 0.48 {
                    M::EmberCrystalOre
                } else {
                    M::ColumnarBasalt
                }
            } else if slope > 0.86 {
                M::DarkVolcanicStone
            } else if patch < -0.44 {
                M::ScorchedDirt
            } else if patch > 0.42 {
                M::VolcanicGravel
            } else if patch > 0.05 {
                M::CompactedAsh
            } else {
                M::Ash
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SectionGenerator, TerrainGenerator};
    use terra_core::BiomeKind;
    use terra_core::WorldConfig;
    #[test]
    fn volcanic_watershed_excludes_preexisting_brooks_and_region_rivers() {
        // Both channels previously exposed fresh water above the volcanic substrate.
        for (rivers, [x, z]) in [(false, [4568.5, -6195.5]), (true, [-6702.5, -7599.5])] {
            let config = WorldConfig {
                rivers,
                ecology_revision: 20,
                ..Default::default()
            };
            let old = TerraGenerator::new(config.clone()).unwrap();
            let current = TerraGenerator::new(WorldConfig {
                ecology_revision: 21,
                ..config
            })
            .unwrap();
            assert!(old.water_column(x, z).is_some());
            let channel = old.river_at(x, z).unwrap();
            assert!(channel.distance < channel.width);
            assert!(current.volcanic_watershed_at(x, z));
            assert!(current.river_course_at(x, z).is_none());
            assert!(current.river_at(x, z).is_none());
            assert!(current.water_column(x, z).is_none());
        }
    }

    #[test]
    fn volcanic_cave_streams_dry_out_while_their_chambers_stay_open() {
        let config = WorldConfig::default();
        let old = TerraGenerator::new(WorldConfig {
            ecology_revision: 20,
            ..config.clone()
        })
        .unwrap();
        let now = TerraGenerator::new(config).unwrap();
        for [x, z] in [[5473.5, -6667.5], [5074.5, -6629.5]] {
            assert!(old.cave_structure_at(x, z).water.is_some(), "{x}, {z}");
            let chamber = now.cave_structure_at(x, z);
            assert!(chamber.active() && chamber.water.is_none(), "{x}, {z}");
        }
    }

    #[test]
    fn spatter_chimneys_glassy_spines_and_pressure_ridges_have_mineable_relief() {
        let config = WorldConfig::default();
        let now = SectionGenerator::new(TerraGenerator::new(config.clone()).unwrap());
        let old = SectionGenerator::new(
            TerraGenerator::new(WorldConfig {
                ecology_revision: 20,
                ..config
            })
            .unwrap(),
        );
        for (position, mineral) in [
            ([1999, 55, -4852], M::Fumarole),
            ([2162, 29, -4651], M::VolcanicObsidian),
            ([2006, 31, -4946], M::LavaCrust),
        ] {
            assert_eq!(
                now.block_at(position).unwrap(),
                mineral as u16,
                "{position:?}"
            );
            assert_ne!(
                old.block_at(position).unwrap(),
                mineral as u16,
                "{position:?}"
            );
            let [x, y, z] = position;
            assert!(
                M::from_raw(now.block_at([x, y - 1, z]).unwrap())
                    .unwrap()
                    .is_solid()
            );
        }
    }

    #[test]
    fn coastal_ashlands_keep_molten_channels_on_dry_land() {
        let terrain = TerraGenerator::new(WorldConfig {
            seed: terra_core::WorldSeed(18358273135247655527),
            ecology_revision: 18,
            biome_definitions: Some(Default::default()),
            ..Default::default()
        })
        .unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        // Around this saved world's first Ashlands landing, no sampled column
        // within 256 m cleared the old threshold of 0.74 total volcanic cover.
        for z in (1856..=2048).step_by(8) {
            for x in (704..=864).step_by(8) {
                let c = terrain.surface_column(f64::from(x) + 0.5, f64::from(z) + 0.5);
                if let Some(level) = c.lava_level
                    && c.biome.weights.ashlands < 0.74
                    && c.biome_kind(terrain.config().sea_level) == BiomeKind::Ashlands
                {
                    let top = level.ceil() as i64 - 1;
                    let p = [i64::from(x), top, i64::from(z)];
                    let (owner, local) = terra_voxel::global_to_section(p);
                    assert_eq!(
                        generator.generate(owner).unwrap().block(local).unwrap(),
                        M::Lava as u16
                    );
                    assert!(c.water_level.is_none());
                    return;
                }
            }
        }
        panic!("coastal volcanic drainage produced no molten voxels");
    }

    #[test]
    fn new_provinces_have_molten_waterways_with_real_floors_and_dry_shores() {
        let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
        let (x, z) = (-6000..=6000)
            .step_by(240)
            .flat_map(|z| (-6000..=6000).step_by(240).map(move |x| (x, z)))
            .find(|&(x, z)| {
                terrain
                    .biome_at(x as f64 + 0.5, z as f64 + 0.5)
                    .weights
                    .ashlands
                    > 0.95
            })
            .expect("reachable volcanic province");
        let section = SectionGenerator::new(terrain.clone());
        let mut wet = 0;
        let mut dry = 0;
        let mut deep = 0;
        for dz in (-80..=80).step_by(6) {
            for dx in (-80..=80).step_by(6) {
                let (x, z) = (x + dx, z + dz);
                let px = x as f64 + 0.5;
                let pz = z as f64 + 0.5;
                let c = terrain.surface_column(px, pz);
                if c.biome.weights.ashlands < 0.95 {
                    continue;
                }
                assert_eq!(terrain.surface_biome(&c), BiomeKind::Ashlands);
                let level = terrain.lava_surface(px, pz);
                assert_eq!(level, c.lava_level);
                if let Some(level) = level {
                    assert!(c.water_level.is_none(), "water and lava share {x},{z}");
                    wet += 1;
                    if c.height < level - 3. {
                        deep += 1;
                    }
                    let y = level as i64 - 1;
                    assert_eq!(
                        terrain.sample([px, y as f64 + 0.5, pz]).material,
                        M::Lava as u16
                    );
                    if wet == 1 {
                        assert_eq!(section.block_at([x, y, z]).unwrap(), M::Lava as u16);
                        assert_eq!(section.block_at([x, y + 1, z]).unwrap(), M::Air as u16);
                        assert_ne!(
                            section
                                .block_at([x, (c.height - 0.5).floor() as i64, z])
                                .unwrap(),
                            M::Lava as u16
                        );
                    }
                } else if c.water_level.is_none() {
                    dry += 1;
                }
            }
        }
        assert!(
            wet >= 10 && dry >= 90 && deep >= 3,
            "wet {wet}, dry {dry}, deep {deep} at {x},{z}"
        );
        let legacy = TerraGenerator::new(WorldConfig {
            ecology_revision: 15,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(legacy.biome_at(x as f64, z as f64).weights.ashlands, 0.);
        assert_eq!(legacy.lava_surface(x as f64, z as f64), None);
        for revision in [0, 1, 3, 5, 8] {
            let terrain = TerraGenerator::new(WorldConfig {
                landscape_revision: revision,
                ..Default::default()
            })
            .unwrap();
            for [x, z] in [[1024., 768.], [-31186., -96620.], [2104.5, -4879.5]] {
                assert_eq!(terrain.biome_at(x, z).weights.ashlands, 0.);
                assert_eq!(terrain.lava_surface(x, z), None);
            }
        }
    }
    #[test]
    fn reported_spillway_fills_its_banks_and_the_basin_meets_the_climate_boundary() {
        let terrain = TerraGenerator::new(WorldConfig {
            seed: terra_core::WorldSeed(5186794449079436280),
            ecology_revision: 16,
            ..Default::default()
        })
        .unwrap();
        for (min_x, max_x, min_z, max_z) in
            [(-2274, -2208, -4030, -3920), (-2110, -2090, -4144, -4135)]
        {
            let mut edges = 0;
            let mut gaps = Vec::new();
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    let c = terrain.surface_column(f64::from(x) + 0.5, f64::from(z) + 0.5);
                    let Some(level) = c.lava_level else { continue };
                    for (dx, dz) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
                        let bank = terrain
                            .surface_column(f64::from(x + dx) + 0.5, f64::from(z + dz) + 0.5);
                        if bank.lava_level.is_none() {
                            edges += 1;
                            if bank.height < level - 0.5 {
                                gaps.push((x + dx, z + dz, level, bank.height));
                            }
                        }
                    }
                }
            }
            assert!(edges > 0, "no shore near {min_x},{min_z}");
            assert!(
                gaps.is_empty(),
                "{} open faces near {min_x},{min_z}: {:?}",
                gaps.len(),
                gaps.iter().take(8).collect::<Vec<_>>()
            );
        }
    }
    #[test]
    fn reported_lava_lake_keeps_a_continuous_mineable_bed() {
        let terrain = TerraGenerator::new(WorldConfig {
            seed: terra_core::WorldSeed(5186794449079436280),
            ..Default::default()
        })
        .unwrap();
        let section = SectionGenerator::new(terrain.clone());
        let mut wet = 0;
        for x in (-1994..=-1944).step_by(4) {
            for z in (-4040..=-3988).step_by(4) {
                let p = [x as f64 + 0.5, z as f64 + 0.5];
                let column = terrain.surface_column(p[0], p[1]);
                let Some(level) = column.lava_level else {
                    continue;
                };
                wet += 1;
                let bed = (column.height - 0.5).floor() as i64;
                assert!(
                    column
                        .cave_cavity
                        .is_none_or(|c| c[1] <= column.height - 6.)
                );
                for y in bed - 4..=bed {
                    let sample = terrain.sample_with_column([p[0], y as f64 + 0.5, p[1]], column);
                    assert!(
                        sample.density <= 0. && M::from_raw(sample.material).unwrap().is_solid(),
                        "unsupported lake at {x}/{y}/{z}"
                    );
                }
                if x == -1962 && z == -4000 {
                    assert_eq!(
                        section.block_at([x, level as i64 - 1, z]).unwrap(),
                        M::Lava as u16
                    );
                    assert!(
                        M::from_raw(section.block_at([x, bed, z]).unwrap())
                            .unwrap()
                            .is_solid()
                    );
                }
            }
        }
        assert!(wet > 20);
        // The nearby active cone has a high rim. At least one trajectory
        // actually clears its solid rock and reaches the far valley.
        let vent = terrain
            .eruptive_vent(-2243.5, -4010.5)
            .expect("nearby active vent");
        let event = crate::volcano::Eruption {
            vent,
            cycle: 1,
            age: 0.,
            landed: 0,
        };
        let mut furthest: f64 = 0.;
        for i in 0..crate::volcano::EJECTA_COUNT {
            let bomb = event.fragment(i);
            for frame in 0..140 {
                let Some(p) = bomb.position(bomb.delay + frame as f32 * 0.05) else {
                    break;
                };
                if p[1] <= terrain.surface_height(p[0], p[2]) {
                    break;
                }
                furthest = furthest.max((p[0] - vent.origin[0]).hypot(p[2] - vent.origin[2]));
            }
        }
        assert!(furthest > 95., "rim blocked ejecta after {furthest} blocks");
    }
    #[test]
    fn volcanic_provinces_have_active_vents_and_smaller_ore_rich_dormant_basins() {
        let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
        let mut active = false;
        let mut dormant = false;
        for tz in -100..100 {
            for tx in -100..100 {
                let cone =
                    terrain.volcano_descriptor(tx as f64 * CONE_SPACING, tz as f64 * CONE_SPACING);
                let [x, z] = cone.center.map(|v| v.floor() + 0.5);
                if terrain.biome_at(x, z).weights.ashlands < 0.98 {
                    continue;
                }
                let Some(lava) = terrain.lava_surface(x, z) else {
                    continue;
                };
                if terrain.surface_height(x + cone.radius * 0.17, z)
                    < lava + if cone.active { 3. } else { 22. }
                {
                    continue;
                }
                if cone.active {
                    active |= terrain.eruptive_vent(x, z).is_some();
                } else if !dormant {
                    let edge = terrain.surface_column(x + cone.radius * 0.09, z);
                    let ore = (-10..=10).any(|dz| {
                        (-10..=10).any(|dx| {
                            let c = terrain.surface_column(x + f64::from(dx), z + f64::from(dz));
                            c.lava_level.is_none() && c.surface_material == M::EmberCrystalOre
                        })
                    });
                    dormant =
                        terrain.eruptive_vent(x, z).is_none() && edge.lava_level.is_none() && ore;
                }
                if active && dormant {
                    return;
                }
            }
        }
        panic!("active={active} dormant={dormant}");
    }
    #[test]
    fn volcanic_scenery_is_grounded_and_a_crater_has_a_molten_floor_and_a_high_rim() {
        let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        let mut kinds = std::collections::BTreeSet::new();
        for tz in -153..=-150 {
            for tx in 64..=67 {
                for (p, id) in terrain.surface_flora([tx, tz]) {
                    if !(624..=631).contains(&id) {
                        continue;
                    }
                    if kinds.insert(id) {
                        assert_eq!(generator.block_at(p).unwrap(), id);
                        if id == M::Fumarole as u16 {
                            let surface =
                                terrain.surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5);
                            assert_eq!(p[1], (surface.height - 0.5).floor() as i64);
                            assert_eq!(
                                generator.block_at([p[0], p[1] + 1, p[2]]).unwrap(),
                                M::Air as u16
                            );
                        }
                        let floor = [p[0], p[1] - 1, p[2]];
                        assert!(
                            M::from_raw(generator.block_at(floor).unwrap())
                                .unwrap()
                                .is_solid()
                        );
                        let (section, local) = terra_voxel::global_to_section(p);
                        assert_eq!(
                            generator.generate(section).unwrap().block(local).unwrap(),
                            id
                        );
                    }
                }
            }
        }
        assert_eq!(kinds, (624..=631).collect());
        let mut found_crater = false;
        for [x, z] in [
            [1867.5, -4800.5],
            [2278.5, -4719.5],
            [2261.5, -5189.5],
            [1877.5, -5173.5],
        ] {
            let c = terrain.surface_column(x, z);
            if c.biome.weights.ashlands < 0.95 {
                continue;
            }
            let peak = terrain.surface_height(x + 14., z);
            if c.lava_level.is_some() && peak > c.height + 32. {
                found_crater = true;
                break;
            }
        }
        assert!(found_crater);
    }
}
