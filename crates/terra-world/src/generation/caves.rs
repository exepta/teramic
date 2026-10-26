use super::{TerraGenerator, seed::layer_seed};
use noise::NoiseFn;
use std::{collections::HashMap, sync::Mutex};

const SPACING: f64 = 128.0;
const LENGTH: f64 = 48.0;

#[cfg(test)]
#[path = "caves/connectors_tests.rs"]
mod connectors_tests;

#[cfg(test)]
mod ravine_tests {
    use super::*;
    use terra_core::{Config, WorldConfig};
    #[test]
    fn broad_ravines_have_organic_bounded_width_and_preserve_old_save_contracts() {
        assert_eq!(WorldConfig::from_toml("").unwrap().ravine_revision, 0);
        assert_eq!(WorldConfig::default().ravine_revision, 1);
        assert!(WorldConfig::from_toml("ravine_revision=2").is_err());
        let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
        let seed = layer_seed(terrain.config().seed.0, 0x726176);
        let phase = (seed % 628) as f64 * 0.01;
        let mut widths = Vec::new();
        for z in (-60..60).step_by(10) {
            let z = f64::from(z);
            let center = ((seed >> 12) % 800) as f64 - 400.0
                + (z * 0.027 + phase).sin() * 14.0
                + (z * 0.071 + phase).sin() * 3.0;
            let width = (-24..=24)
                .filter(|&x| {
                    terrain
                        .ravine_depth(center + f64::from(x) * 0.25, z)
                        .is_some()
                })
                .count() as f64
                * 0.25;
            assert!((2.75..=11.25).contains(&width), "{width}");
            widths.push(width);
            assert!(terrain.ravine_depth(center, z).unwrap() > 1.0);
        }
        assert!(widths.windows(2).any(|w| (w[0] - w[1]).abs() > 1.0));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaveKind {
    Cheese,
    Dripstone,
    Spaghetti,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CaveRegion {
    pub ecology: super::CaveBiome,
    pub surface: f64,
    pub climate: super::biomes::BiomeSample,
    pub kind: CaveKind,
    pub salt: bool,
    pub center: [f64; 3],
    pub radius: f64,
    pub height: f64,
    pub presence: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Formation {
    center: [f64; 2],
    roots: [f64; 2],
    lengths: [f64; 2],
    radius: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FormationColumn {
    distance: f64,
    growth: Formation,
}

/// Horizontal ecology of the existing cave density, prepared once per column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CaveShape {
    pub tunnel_width: f64,
    pub connector_limit: f64,
    pub fissure: f64,
    pub kind: CaveKind,
    pub salt: bool,
    chamber_guard: f64,
    chamber: Option<[f64; 3]>,
    formation: Option<FormationColumn>,
}

impl CaveShape {
    pub(super) fn formation_density(&self, y: f64) -> f64 {
        let Some(column) = &self.formation else {
            return f64::INFINITY;
        };
        (0..2)
            .map(|side| {
                let growth = &column.growth;
                let height = (y - growth.roots[side]) * if side == 0 { 1.0 } else { -1.0 };
                let t = (height / growth.lengths[side]).clamp(0.0, 1.0);
                let radius = growth.radius * (1.0 - t).powf(0.72);
                (column.distance - radius)
                    .max(-height)
                    .max(height - growth.lengths[side])
            })
            .fold(f64::INFINITY, f64::min)
    }
    pub(super) fn chamber_density(&self, y: f64) -> f64 {
        self.chamber
            .map_or(f64::NEG_INFINITY, |[radial, center_y, height]| {
                (1.0 - radial - ((y - center_y) / height).powi(2)) * height
                    - super::biomes::smoothstep(1.1, 1.6, radial) * self.chamber_guard
            })
    }
}

#[cfg(test)]
mod formation_tests {
    use super::*;
    use crate::{SectionGenerator, TerrainGenerator};
    use terra_core::WorldConfig;
    use terra_voxel::global_to_section;

    #[test]
    fn salt_clearance_matches_picking_after_cross_section_edits_and_reload() {
        use crate::edits::EditJournal;
        let config = WorldConfig {
            cave_structure_revision: 0,
            base_height: -64.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            caves: false,
            trees: false,
            water: false,
            biomes: false,
            ..Default::default()
        };
        let generator = SectionGenerator::new(TerraGenerator::new(config.clone()).unwrap());
        let (root, length) = (-48..48)
            .find_map(|x| {
                let p = [x, 16, -16];
                let h = crate::ore_shapes::salt_growth_height(p, 2, |_| Some(0));
                (h > 8.0).then_some((p, h))
            })
            .unwrap();
        let (section, local) = global_to_section(root);
        let mut journal = EditJournal::default();
        journal.record(root, 30).unwrap();
        let build = |journal: &EditJournal, generator: &SectionGenerator| {
            generator
                .build(
                    section,
                    journal.revision(section),
                    &journal.snapshot(section).unwrap(),
                    true,
                )
                .unwrap()
                .mesh
                .unwrap()
                .mineral_growth
                .into_iter()
                .find(|&(cell, side, _)| cell.map(usize::from) == local && side == 2)
                .map(|(_, _, h)| h)
        };
        assert_eq!(build(&journal, &generator), Some(length));
        let blocker = [root[0], root[1] - 2, root[2]];
        let affected = journal.record(blocker, 3).unwrap();
        assert!(
            !affected.contains(&section),
            "clearance must not invalidate remote collision data"
        );
        assert_eq!(build(&journal, &generator), None);
        let mut reloaded = EditJournal::default();
        for section in journal.changed_sections() {
            reloaded
                .import_delta(journal.section_delta(section))
                .unwrap();
        }
        assert_eq!(
            build(
                &reloaded,
                &SectionGenerator::new(TerraGenerator::new(config).unwrap())
            ),
            None
        );
        journal.record(blocker, 0).unwrap();
        assert_eq!(build(&journal, &generator), Some(length));
    }

    #[test]
    fn new_salt_caves_keep_legacy_bodies_only_in_saved_geology_two() {
        let old = TerraGenerator::new(WorldConfig {
            geology_revision: 2,
            cave_structure_revision: 2,
            ..Default::default()
        })
        .unwrap();
        let new = TerraGenerator::new(WorldConfig {
            cave_structure_revision: 2,
            ..Default::default()
        })
        .unwrap();
        let mut checked = 0;
        for z in -4..=4 {
            for x in -4..=4 {
                let px = 16800.0 + f64::from(x) * 12.0 + 6.5;
                let pz = -17952.0 + f64::from(z) * 12.0 + 6.5;
                let previous = old.cave_shape(px, pz);
                if previous.salt && previous.formation.is_some() {
                    checked += 1;
                    assert!(new.cave_shape(px, pz).formation.is_none());
                }
            }
        }
        assert!(checked > 0);
    }

    #[test]
    fn tall_growth_has_real_floor_and_ceiling_support_and_survives_section_boundaries() {
        let terrain = TerraGenerator::new(WorldConfig {
            geology_revision: 2,
            cave_structure_revision: 2,
            ..Default::default()
        })
        .unwrap();
        let old = TerraGenerator::new(WorldConfig {
            geology_revision: 1,
            cave_structure_revision: 2,
            ..Default::default()
        })
        .unwrap();
        let mut formations = Vec::new();
        for z in -4..=4 {
            for x in -4..=4 {
                let px = f64::from(x) * 12.0 + 6.5;
                let pz = f64::from(z) * 12.0 + 6.5;
                if let Some(column) = terrain.cave_shape(px, pz).formation {
                    formations.push(column.growth);
                }
            }
        }
        // The origin may be a non-dripstone region; use the known native salt
        // chamber too, without injecting any blocks into the terrain.
        for z in -4..=4 {
            for x in -4..=4 {
                let px = 16800.0 + f64::from(x) * 12.0 + 6.5;
                let pz = -17952.0 + f64::from(z) * 12.0 + 6.5;
                let region =
                    terrain.cave_region([(px / 192.0).floor() as i64, (pz / 192.0).floor() as i64]);
                if let Some(formation) = terrain.cave_formation(px, pz, region) {
                    formations.push(formation);
                }
            }
        }
        assert!(!formations.is_empty());
        let generator = SectionGenerator::new(terrain.clone());
        let mut tall = 0;
        for formation in formations.iter().take(20) {
            for side in 0..2 {
                let direction = if side == 0 { 1.0 } else { -1.0 };
                let root = [
                    formation.center[0],
                    formation.roots[side],
                    formation.center[1],
                ];
                assert!(old.sample(root).density <= 0.0, "unsupported root {root:?}");
                let mut air_replaced = 0;
                for step in 0..formation.lengths[side] as i32 {
                    let point = [
                        root[0],
                        root[1] + direction * (f64::from(step) + 0.5),
                        root[2],
                    ];
                    let sample = terrain.sample(point);
                    assert!(sample.density <= 0.0, "gap in formation {point:?}");
                    if old.sample(point).density > 0.0 {
                        air_replaced += 1;
                        assert!(matches!(
                            terra_voxel::MaterialId::from_raw(sample.material),
                            Some(
                                terra_voxel::MaterialId::DripstoneFormation
                                    | terra_voxel::MaterialId::SaltFormation
                            )
                        ));
                    }
                    let global = point.map(|value| value.floor() as i64);
                    let (section, local) = global_to_section(global);
                    let generated = generator.generate(section).unwrap();
                    assert_eq!(
                        generated.block(local).unwrap(),
                        generator.block_at(global).unwrap()
                    );
                }
                tall = tall.max(air_replaced);
            }
        }
        println!(
            "formations={} longest_new_solid_run={tall}",
            formations.len()
        );
        assert!(tall >= 5);
        terrain.cave_portals.2.lock().unwrap().clear();
        let fresh = TerraGenerator::new(WorldConfig {
            geology_revision: 2,
            cave_structure_revision: 2,
            ..Default::default()
        })
        .unwrap();
        let p = [16806.5, -45.5, -17945.5];
        assert_eq!(terrain.sample(p), fresh.sample(p));
    }
}

#[derive(Default)]
pub(super) struct CaveCache(
    Mutex<HashMap<[i64; 2], Option<Portal>>>,
    Mutex<HashMap<[i64; 2], CaveRegion>>,
    Mutex<HashMap<[i64; 2], Option<Formation>>>,
);

#[derive(Clone, Copy, Debug)]
struct Portal {
    mouth: [f64; 3],
    direction: [f64; 2],
    end_floor: f64,
    phase: f64,
}

impl Portal {
    fn coordinates(self, x: f64, z: f64) -> [f64; 2] {
        let dx = x - self.mouth[0];
        let dz = z - self.mouth[2];
        [
            dx * self.direction[0] + dz * self.direction[1],
            dz * self.direction[0] - dx * self.direction[1],
        ]
    }

    fn interval(self, x: f64, z: f64) -> Option<[f64; 2]> {
        let [along, across] = self.coordinates(x, z);
        if !(-6.0..LENGTH + 10.0).contains(&along) {
            return None;
        }
        let t = along.clamp(0.0, LENGTH) / LENGTH;
        let bend = (t * std::f64::consts::PI).sin() * 1.1;
        let room = (-((along - LENGTH + 2.0) / 10.0).powi(2)).exp();
        let cap = if along < 0.0 {
            (1.0 - (along / 6.0).powi(2)).sqrt()
        } else if along > LENGTH {
            (1.0 - ((along - LENGTH) / 10.0).powi(2)).sqrt()
        } else {
            1.0
        };
        let width = (2.8 + (along * 0.21 + self.phase).sin() * 0.45 + room * 6.5) * cap;
        if width <= 0.0 || !width.is_finite() {
            return None;
        }
        let lateral = (across - bend) / width;
        if !lateral.is_finite() || lateral.abs() >= 1.0 {
            return None;
        }
        let arch = (1.0 - lateral * lateral).sqrt();
        let floor = self.mouth[1]
            + (self.end_floor - self.mouth[1]) * t
            + (1.0 - arch) * 1.2
            + (along * 0.31 + across * 0.27 + self.phase).sin() * 0.16;
        let height = (4.8 + room * 3.6 + (along * 0.29 + self.phase).sin() * 0.65) * arch;
        Some([floor, floor + height])
    }
}

impl TerraGenerator {
    pub(super) fn cave_region(&self, cell: [i64; 2]) -> CaveRegion {
        if let Some(region) = self
            .cave_portals
            .1
            .lock()
            .ok()
            .and_then(|cache| cache.get(&cell).copied())
        {
            return region;
        }
        let seed = layer_seed(
            self.config().seed.0
                ^ (cell[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (cell[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            194,
        );
        let cx = cell[0] as f64 * 192.0 + 96.0;
        let cz = cell[1] as f64 * 192.0 + 96.0;
        let biome = self.climate_biome_at(cx, cz);
        // A separate deterministic ticket: ten percent of eligible cave regions,
        // not ten percent of blocks and not a new roll on every chunk reload.
        let ecology = if self.config().cave_structure_revision >= 3 {
            self.cave_biome_at([cx, -40.0, cz]).primary
        } else {
            super::CaveBiome::Normal
        };
        let salt = if self.config().cave_structure_revision >= 3 {
            ecology == super::CaveBiome::Salt
        } else {
            biome.weights.cliffted_desert > 0.5
                && layer_seed(self.config().seed.0 ^ u64::from(seed), 195).is_multiple_of(10)
        };
        let kind = if salt {
            CaveKind::Dripstone
        } else {
            match if self.config().cave_structure_revision >= 2 {
                seed % 5
            } else {
                seed % 3
            } {
                0 => CaveKind::Cheese,
                1 => CaveKind::Dripstone,
                3 => CaveKind::Cheese,
                4 => CaveKind::Dripstone,
                _ => CaveKind::Spaghetti,
            }
        };
        let radius = match kind {
            CaveKind::Cheese => 32.0 + f64::from((seed >> 8) % 17),
            CaveKind::Dripstone => 45.0 + f64::from((seed >> 8) % 23),
            CaveKind::Spaghetti => 0.0,
        };
        let radius = if self.config().cave_structure_revision >= 3 && radius > 0.0 {
            if seed.is_multiple_of(19) {
                90.0
            } else {
                26.0 + f64::from((seed >> 7) % 47)
            }
        } else if self.config().cave_structure_revision >= 2 && radius > 0.0 {
            (radius * 1.28).min(78.0)
        } else {
            radius
        };
        let surface = self.base_height_with_biome(cx, cz, biome);
        let height = if self.config().cave_structure_revision >= 3 {
            10.0 + radius * (0.18 + f64::from((seed >> 18) % 50) * 0.01)
        } else if salt {
            19.0 + f64::from((seed >> 16) % 6)
        } else {
            match kind {
                CaveKind::Cheese => radius * 0.65,
                _ => 13.0 + f64::from((seed >> 16) % 7),
            }
        };
        let jitter = if self.config().cave_structure_revision >= 3 {
            (88.0 - radius).max(0.0)
        } else {
            0.0
        };
        let center = [
            cx + (f64::from((seed >> 4) % 101) / 50.0 - 1.0) * jitter,
            (surface - height - 14.0).min(-12.0) - f64::from((seed >> 22) % 36),
            cz + (f64::from((seed >> 12) % 101) / 50.0 - 1.0) * jitter,
        ];
        let region = CaveRegion {
            ecology,
            surface,
            climate: biome,
            kind,
            salt,
            radius,
            height,
            center,
            presence: self.connector_presence(center[0], center[2]),
        };
        if let Ok(mut cache) = self.cave_portals.1.lock() {
            if cache.len() >= 2048 {
                cache.clear();
            }
            cache.insert(cell, region);
        }
        region
    }

    pub fn cave_kind_at(&self, x: f64, z: f64) -> CaveKind {
        self.cave_shape(x, z).kind
    }

    pub fn salt_chamber_center(&self, x: f64, z: f64) -> Option<[f64; 3]> {
        if self.config().geology_revision == 0 {
            return None;
        }
        let region = self.cave_region([(x / 192.0).floor() as i64, (z / 192.0).floor() as i64]);
        (region.salt && region.presence > 0.0).then_some(region.center)
    }

    pub fn cave_chamber_center(&self, x: f64, z: f64) -> Option<[f64; 3]> {
        if self.config().geology_revision == 0 || !self.config().caves {
            return None;
        }
        let region = self.cave_region([(x / 192.0).floor() as i64, (z / 192.0).floor() as i64]);
        (region.kind != CaveKind::Spaghetti && region.presence > 0.0).then(|| {
            if self.config().cave_structure_revision >= 2 {
                self.cave_structure_at(x, z).center
            } else {
                region.center
            }
        })
    }

    pub(super) fn cave_shape(&self, x: f64, z: f64) -> CaveShape {
        let cell = [(x / 192.0).floor() as i64, (z / 192.0).floor() as i64];
        if self.config().geology_revision > 0 {
            let region = self.cave_region(cell);
            let mut shape = self.native_cave_shape(x, z, region);
            if self.config().caves
                && self.config().geology_revision >= 2
                && region.kind == CaveKind::Dripstone
                && self.config().geology_revision < 4
                && !(region.salt && self.config().geology_revision >= 3)
            {
                shape.formation = self.cave_formation(x, z, region).and_then(|growth| {
                    let distance = (x - growth.center[0]).hypot(z - growth.center[1]);
                    (distance <= growth.radius).then_some(FormationColumn { distance, growth })
                });
            }
            return shape;
        }
        let variety = self.valleys.get([x * 0.0031 + 9.17, z * 0.0031 - 4.13]);
        let seed = layer_seed(
            self.config().seed.0
                ^ (cell[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (cell[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            184,
        );
        let chamber = (seed % 7 <= 1).then(|| {
            let giant = seed.is_multiple_of(7);
            let radius = if giant { 37.0 } else { 21.0 } + f64::from((seed >> 8) % 7);
            let cx = cell[0] as f64 * 192.0 + 96.0 + f64::from((seed >> 12) % 17) - 8.0;
            let cz = cell[1] as f64 * 192.0 + 96.0 + f64::from((seed >> 17) % 17) - 8.0;
            let radial = ((x - cx) / radius).powi(2) + ((z - cz) / (radius * 0.82)).powi(2);
            [radial, -24.0 - f64::from((seed >> 22) % 48), radius * 0.60]
        });
        CaveShape {
            tunnel_width: 0.105 + super::biomes::smoothstep(-0.35, 0.4, variety) * 0.09,
            connector_limit: f64::INFINITY,
            fissure: super::biomes::smoothstep(0.12, 0.48, variety),
            kind: if chamber.is_some() {
                CaveKind::Cheese
            } else {
                CaveKind::Spaghetti
            },
            salt: false,
            chamber_guard: 0.0,
            chamber,
            formation: None,
        }
    }

    fn native_cave_shape(&self, x: f64, z: f64, region: CaveRegion) -> CaveShape {
        let variety = self.valleys.get([x * 0.0031 + 9.17, z * 0.0031 - 4.13]);
        let radial = (((x - region.center[0]) / region.radius.max(1.0)).powi(2)
            + ((z - region.center[2]) / (region.radius.max(1.0) * 0.88)).powi(2))
            * (1.0 + self.hills.get([x * 0.026 + 37.1, z * 0.026 - 19.3]) * 0.18);
        let tunnel_width = if self.config().cave_structure_revision >= 3 {
            // Moderately thin the existing network. Never suppress entire
            // hundreds-of-metre regions with a negative width.
            (0.11 + super::biomes::smoothstep(-0.35, 0.4, variety) * 0.07)
                * self.config().cave_biomes.connectors.density
        } else if self.config().cave_structure_revision >= 2 {
            // Broad closed rock bands interrupt the noise network, without
            // discontinuities at region or chunk boundaries.
            -0.055 + super::biomes::smoothstep(-0.18, 0.34, variety) * 0.205
        } else {
            0.095 + super::biomes::smoothstep(-0.35, 0.4, variety) * 0.08
        };
        // Widen the existing implicit connector field without another tunnel
        // network or new per-voxel work. Saved revisions retain their exact field.
        let tunnel_width = if self.config().cave_structure_revision >= 6 {
            tunnel_width * 1.4
        } else {
            tunnel_width
        };
        let tunnel_width = if self.config().cave_structure_revision >= 8 {
            tunnel_width * 1.12
        } else {
            tunnel_width
        };
        CaveShape {
            tunnel_width,
            connector_limit: self.connector_presence(x, z),
            fissure: super::biomes::smoothstep(0.12, 0.48, variety),
            kind: region.kind,
            salt: region.salt
                && radial < 1.2
                && self.climate_biome_at(x, z).weights.cliffted_desert > 0.5,
            chamber_guard: 64.0,
            chamber: (region.kind != CaveKind::Spaghetti
                && self.config().cave_structure_revision < 3)
                .then_some([radial, region.center[1], region.height]),
            formation: None,
        }
    }

    // Prepared with the horizontal column, not for every voxel. A signed cap
    // groups the existing wide network without narrowing its active passages.
    fn connector_presence(&self, x: f64, z: f64) -> f64 {
        if self.config().cave_structure_revision < 7 || self.config().geology_revision == 0 {
            return f64::INFINITY;
        }
        let local = self.hills.get([x / 56.0 + 71.3, z / 56.0 - 28.9]);
        let regional = self.valleys.get([x / 128.0 - 19.7, z / 128.0 + 55.9]);
        (local * 0.80 + regional * 0.20 + 0.06) * 24.0
    }

    fn cave_formation(&self, x: f64, z: f64, region: CaveRegion) -> Option<Formation> {
        let cell = [(x / 12.0).floor() as i64, (z / 12.0).floor() as i64];
        if let Some(cached) = self
            .cave_portals
            .2
            .lock()
            .ok()
            .and_then(|cache| cache.get(&cell).copied())
        {
            return cached;
        }
        let seed = layer_seed(
            self.config().seed.0
                ^ (cell[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (cell[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            196,
        );
        let candidate = (|| {
            if seed % 5 > 2 {
                return None;
            }
            let center = [
                cell[0] as f64 * 12.0 + 4.5 + f64::from((seed >> 3) % 4),
                cell[1] as f64 * 12.0 + 4.5 + f64::from((seed >> 5) % 4),
            ];
            let shape = self.native_cave_shape(center[0], center[1], region);
            let [radial, center_y, _] = shape.chamber?;
            if radial > 0.78 {
                return None;
            }
            let cy = center_y.floor() + 0.5;
            let support = |direction: f64| {
                (1..=96).find_map(|step| {
                    let y = cy + f64::from(step) * direction;
                    (self.tunnel_density_with_shape([center[0], y, center[1]], &shape) < -1.0)
                        .then_some(y)
                })
            };
            let floor = support(-1.0)?;
            let ceiling = support(1.0)?;
            let span = ceiling - floor;
            if span < 10.0 {
                return None;
            }
            Some(Formation {
                center,
                roots: [floor - 2.5, ceiling + 2.5],
                lengths: [
                    (6.0 + f64::from((seed >> 10) % 10)).min(span * 0.53),
                    (5.0 + f64::from((seed >> 17) % 11)).min(span * 0.53),
                ],
                radius: 1.1 + f64::from((seed >> 23) % 17) * 0.10,
            })
        })();
        if let Ok(mut cache) = self.cave_portals.2.lock() {
            if cache.len() >= 8192 {
                cache.clear();
            }
            cache.insert(cell, candidate);
        }
        candidate
    }

    /// Entrance geometry is prepared once per horizontal column. The shared
    /// bounded cache contains only immutable descriptors, never world edits.
    pub(super) fn cave_cavity_at(
        &self,
        x: f64,
        z: f64,
        height: f64,
        river: Option<[f64; 2]>,
    ) -> Option<[f64; 2]> {
        if !self.config().caves
            || river.is_some()
            || !x.is_finite()
            || !z.is_finite()
            || !height.is_finite()
        {
            return None;
        }
        let mut ravine = None;
        if self.config().bounded_world && height > self.config().sea_level + 8.0 {
            if self.config().ravine_revision > 0 {
                if let Some(depth) = self.ravine_depth(x, z) {
                    ravine = Some([(height - depth).max(-119.0), height + 2.0]);
                }
            } else {
                let ridge = self.valleys.get([x * 0.006 + 41.0, z * 0.006 - 19.0]);
                let cross = self.hills.get([x * 0.014 - 17.0, z * 0.014 + 63.0]);
                let width = (0.025 - cross.abs()).max(0.0);
                if ridge > 0.28 && width > 0.0 {
                    let depth = 8.0 + super::biomes::smoothstep(0.0, 0.025, width) * 26.0;
                    return Some([(height - depth).max(-119.0), height + 2.0]);
                }
            }
        }
        let cell = [
            ((x + SPACING * 0.5) / SPACING).floor() as i64,
            ((z + SPACING * 0.5) / SPACING).floor() as i64,
        ];
        // Every descriptor fits inside its grid cell, including its chamber.
        if (x - cell[0] as f64 * SPACING).abs() > 49.0
            || (z - cell[1] as f64 * SPACING).abs() > 49.0
        {
            return ravine;
        }
        let cached = self
            .cave_portals
            .0
            .lock()
            .ok()
            .and_then(|cache| cache.get(&cell).copied());
        let Some(portal) = cached.unwrap_or_else(|| {
            let portal = self.build_portal(cell);
            if let Ok(mut cache) = self.cave_portals.0.lock() {
                if cache.len() >= 1024 {
                    cache.clear();
                }
                *cache.entry(cell).or_insert(portal)
            } else {
                portal
            }
        }) else {
            return ravine;
        };
        // A walkable entrance owns its corridor. A ravine must not replace its
        // floor midway with a sudden 10..30 m step.
        let Some(mut interval) = portal.interval(x, z) else {
            return ravine.and_then(|cavity| {
                let [along, across] = portal.coordinates(x, z);
                let separation = (along - along.clamp(0.0, LENGTH)).hypot(across);
                let blend = super::biomes::smoothstep(10.0, 20.0, separation);
                let depth = (height - cavity[0]) * blend;
                (depth > 0.5).then_some([height - depth, cavity[1]])
            });
        };
        if interval[0] >= height {
            return None;
        }
        if height - interval[1] < 3.0 || !self.cave_roof_supported(portal, x, z, height, interval) {
            interval[1] = interval[1].max(height + 1.0);
        }
        Some(interval)
    }

    /// Meandering channels with a 3..11 m opening, rounded shoulders and
    /// varying depth. All samples are column-local and deterministic.
    fn ravine_depth(&self, x: f64, z: f64) -> Option<f64> {
        let cell = ((z + 96.0) / 192.0).floor() as i64;
        let seed = layer_seed(self.config().seed.0, cell as u64 ^ 0x726176);
        let center = cell as f64 * 192.0;
        let along = z - center;
        let phase = (seed % 628) as f64 * 0.01;
        let center_x = ((seed >> 12) % 800) as f64 - 400.0
            + (z * 0.027 + phase).sin() * 14.0
            + (z * 0.071 + phase).sin() * 3.0;
        // Repeat the rare descriptor along X, without depending on chunk borders.
        let across = (x - center_x + 384.0).rem_euclid(768.0) - 384.0;
        let envelope = super::biomes::smoothstep(90.0, 68.0, along.abs());
        if envelope <= 0.0 {
            return None;
        }
        let width =
            (7.0 + (z * 0.043 + phase).sin() * 2.6 + (z * 0.12).sin() * 1.2).clamp(3.0, 11.0);
        let lateral = across.abs() / (width * 0.5);
        if lateral >= 1.0 {
            return None;
        }
        let shoulder = (1.0 - lateral * lateral).sqrt();
        let depth = (10.0 + (z * 0.035 + phase).sin() * 6.0 + (z * 0.083).cos() * 2.0)
            * shoulder
            * envelope;
        (depth > 0.5).then_some(depth)
    }

    fn build_portal(&self, cell: [i64; 2]) -> Option<Portal> {
        let seed = layer_seed(
            self.config().seed.0
                ^ (cell[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (cell[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            183,
        );
        if !self.config().bounded_world && seed.is_multiple_of(4) {
            return None;
        }
        let anchor = [
            cell[0] as f64 * SPACING + f64::from(seed % 13) - 6.0,
            cell[1] as f64 * SPACING + f64::from((seed >> 8) % 13) - 6.0,
        ];
        let phase = f64::from(seed % 1024) / 1024.0 * std::f64::consts::TAU;
        let mut best = None;
        let mut best_rise = if self.config().bounded_world {
            1.6
        } else {
            3.0
        };
        for index in 0..8 {
            let angle = f64::from(index) * std::f64::consts::FRAC_PI_4;
            let direction = [angle.cos(), angle.sin()];
            let mouth_x = anchor[0] - direction[0] * 24.0;
            let mouth_z = anchor[1] - direction[1] * 24.0;
            let inner_x = anchor[0] + direction[0] * 24.0;
            let inner_z = anchor[1] + direction[1] * 24.0;
            let mouth_height = self.surface_height(mouth_x, mouth_z);
            let inner_height = self.surface_height(inner_x, inner_z);
            let rise = inner_height - mouth_height;
            if rise <= best_rise || mouth_height <= self.config().sea_level + 4.0 {
                continue;
            }
            // A dry entrance cannot cut through a supported river arch or bank.
            if (0..=8).any(|i| {
                let d = f64::from(i) * 6.0;
                let x = mouth_x + direction[0] * d;
                let z = mouth_z + direction[1] * d;
                self.river_at(x, z).is_some() || self.oasis_at(x, z).is_some()
            }) {
                continue;
            }
            let connection = (8..=22).find_map(|drop| {
                let y = mouth_height - f64::from(drop);
                (self.tunnel_density([inner_x, y, inner_z]) > 0.2).then_some(y)
            });
            let Some(connection) = connection else {
                continue;
            };
            best_rise = rise;
            best = Some(Portal {
                mouth: [mouth_x, mouth_height - 0.75, mouth_z],
                direction,
                end_floor: connection - 1.8,
                phase,
            });
        }
        best
    }

    fn cave_roof_supported(
        &self,
        portal: Portal,
        x: f64,
        z: f64,
        height: f64,
        interval: [f64; 2],
    ) -> bool {
        let bottom = (interval[1] - 0.5).ceil() as i64;
        let top = (height - 0.5).floor() as i64;
        [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
            .into_iter()
            .any(|(dx, dz)| {
                let mut previous = [bottom, top];
                // Face-connected paths keep roofs attached even at section edges.
                for step in 1..=24 {
                    let sx = x + dx * f64::from(step);
                    let sz = z + dz * f64::from(step);
                    let ground = self.surface_height(sx, sz);
                    let cavity = portal.interval(sx, sz);
                    let next_top = (ground - 0.5).floor() as i64;
                    let floor = cavity.map_or(ground, |c| c[0]);
                    let protected = (ground - 5.0 - 0.5).ceil() as i64;
                    if previous[0].max(protected) <= previous[1].min((floor - 0.5).floor() as i64) {
                        return true;
                    }
                    let Some(cavity) = cavity else {
                        return false;
                    };
                    if ground - cavity[1] < 3.0 {
                        return false;
                    }
                    let next_bottom = (cavity[1] - 0.5).ceil() as i64;
                    if previous[0].max(next_bottom) > previous[1].min(next_top) {
                        return false;
                    }
                    previous = [next_bottom, next_top];
                }
                false
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SectionGenerator, TerrainGenerator};
    use std::collections::VecDeque;
    use terra_core::WorldConfig;

    #[test]
    fn deep_caves_include_large_chambers_and_distinct_tunnel_profiles() {
        let terrain = terrain();
        let mut narrow = 0;
        let mut fissures = 0;
        let mut giant = None;
        for z in -12..=12 {
            for x in -12..=12 {
                let [x, z] = [f64::from(x) * 192.0 + 96.0, f64::from(z) * 192.0 + 96.0];
                let shape = terrain.cave_shape(x, z);
                narrow += usize::from(shape.tunnel_width < 0.13);
                fissures += usize::from(shape.fissure > 0.7);
                if shape
                    .chamber
                    .is_some_and(|[radial, _, height]| radial < 0.1 && height >= 24.0)
                {
                    giant = Some((x, z, shape));
                }
            }
        }
        assert!(narrow > 20 && fissures > 20);
        let (x, z, shape) = giant.expect("rare giant chambers must be present");
        let [_, y, _] = shape.chamber.unwrap();
        let width = (-48..=48)
            .filter(|dx| terrain.sample([x + f64::from(*dx), y, z]).density > 0.0)
            .count();
        let height = (-36..=36)
            .filter(|dy| terrain.sample([x, y + f64::from(*dy), z]).density > 0.0)
            .count();
        assert!(
            width >= 60 && height >= 36,
            "giant chamber dimensions {width} x {height}"
        );
        let column = terrain.surface_column(x, z);
        assert!(
            terrain
                .sample_with_column([x, column.height - 1.0, z], column)
                .density
                < 0.0
        );
    }

    #[test]
    fn cave_profiles_are_continuous_at_negative_cell_boundaries_and_preserve_water_beds() {
        let terrain = terrain();
        for x in [-384.0, -192.0, 0.0, 192.0] {
            for z in [-285.5, -191.5, 17.5, 92.5] {
                for y in [-100.0, -48.0, -8.0] {
                    let a = terrain.tunnel_density([x - 1e-5, y, z]);
                    let b = terrain.tunnel_density([x + 1e-5, y, z]);
                    assert!((a - b).abs() < 0.001, "cave cell seam {x},{y},{z}");
                }
            }
        }
        let mut beds = 0;
        for z in (-8192..=8192).step_by(512) {
            for x in (-8192..=8192).step_by(512) {
                let [x, z] = [f64::from(x) + 0.5, f64::from(z) + 0.5];
                let column = terrain.surface_column(x, z);
                if column.water_level.is_some() && column.cave_cavity.is_none() {
                    let bed = column.water_bed() - 0.5;
                    assert!(terrain.sample_with_column([x, bed, z], column).density <= 0.0);
                    beds += 1;
                }
            }
        }
        assert!(beds > 10);
    }

    #[test]
    fn giant_chambers_cannot_cut_explicit_river_or_portal_floors_and_roofs() {
        let terrain = terrain();
        let [x, y, z] = [98.0, -25.0, -92.0];
        assert!(terrain.tunnel_density([x, y, z]) > 15.0);
        let base = terrain.surface_column(x, z);
        for river in [true, false] {
            let column = crate::SurfaceColumn {
                height: y + 16.0,
                river_cavity: river.then_some([y, y + 8.0]),
                cave_cavity: (!river).then_some([y, y + 8.0]),
                ..base
            };
            for offset in [-1.9, -1.0, -0.1, 8.1, 9.0, 11.0] {
                assert!(
                    terrain
                        .sample_with_column([x, y + offset, z], column)
                        .density
                        <= 0.0
                );
            }
            for offset in [0.1, 1.0, 4.0, 7.9] {
                assert!(
                    terrain
                        .sample_with_column([x, y + offset, z], column)
                        .density
                        > 0.0
                );
            }
        }
    }

    fn terrain() -> TerraGenerator {
        seeded_terrain(42042)
    }

    fn seeded_terrain(seed: u64) -> TerraGenerator {
        TerraGenerator::new(WorldConfig {
            trees: false,
            geology_revision: 0,
            seed: terra_core::WorldSeed(seed),
            ..Default::default()
        })
        .unwrap()
    }

    fn fixtures(terrain: &TerraGenerator) -> Vec<([i64; 2], Portal)> {
        (-32..=32)
            .flat_map(|z| (-32..=32).map(move |x| [x, z]))
            .filter_map(|cell| terrain.build_portal(cell).map(|portal| (cell, portal)))
            .take(3)
            .collect()
    }

    #[test]
    fn entrances_have_walkable_floors_and_connect_to_variable_chambers_and_deep_caves() {
        for seed in [42042, 7, 98173] {
            check_walkable_entrances(&seeded_terrain(seed));
        }
    }

    fn check_walkable_entrances(terrain: &TerraGenerator) {
        let fixtures = fixtures(terrain);
        assert_eq!(
            fixtures.len(),
            3,
            "seed {} needs real cave entrances",
            terrain.config().seed.0
        );
        let mut covered = 0;
        for (_, portal) in fixtures {
            let mut previous = None::<f64>;
            for step in 1..=LENGTH as i32 {
                let x = (portal.mouth[0] + portal.direction[0] * f64::from(step)).floor() + 0.5;
                let z = (portal.mouth[2] + portal.direction[1] * f64::from(step)).floor() + 0.5;
                let column = terrain.surface_column(x, z);
                let [floor, ceiling] = column.cave_cavity.expect("continuous entrance corridor");
                for boundary in [floor, floor - 2.0, ceiling] {
                    let below = terrain
                        .cave_density_with_column([x, boundary - 0.001, z], column)
                        .0;
                    let above = terrain
                        .cave_density_with_column([x, boundary + 0.001, z], column)
                        .0;
                    assert!((above - below).abs() < 0.01, "discontinuous portal density");
                }
                let ground = (floor - 0.5).floor();
                assert!(
                    terrain
                        .sample_with_column([x, ground + 0.5, z], column)
                        .density
                        <= 0.0
                );
                for y in [ground + 1.5, ground + 2.5] {
                    assert!(
                        terrain.sample_with_column([x, y, z], column).density > 0.0,
                        "blocked walkable portal at {x},{y},{z}"
                    );
                }
                if let Some(last) = previous {
                    assert!((ground - last).abs() <= 1.0, "entrance requires a >1m step");
                }
                previous = Some(ground);
                covered += usize::from(ceiling + 3.0 <= column.height);
            }
            let end = [
                portal.mouth[0] + portal.direction[0] * LENGTH,
                portal.end_floor + 1.8,
                portal.mouth[2] + portal.direction[1] * LENGTH,
            ];
            assert!(terrain.tunnel_density(end) > 0.2);
            assert!(terrain.sample(end).density > 0.0);
            let point = |along, lateral| {
                [
                    portal.mouth[0] + portal.direction[0] * along - portal.direction[1] * lateral,
                    portal.mouth[2] + portal.direction[1] * along + portal.direction[0] * lateral,
                ]
            };
            let narrow = point(12.0, 5.0);
            let chamber = point(LENGTH - 2.0, 5.0);
            assert!(portal.interval(narrow[0], narrow[1]).is_none());
            assert!(portal.interval(chamber[0], chamber[1]).is_some());
            eprintln!(
                "cave entrance fixture: seed={} x={:.1} y={:.1} z={:.1} chamber_floor={:.1}",
                terrain.config().seed.0,
                portal.mouth[0],
                portal.mouth[1],
                portal.mouth[2],
                portal.end_floor
            );
        }
        assert!(
            covered >= 60,
            "entrances must lead underground, not become open trenches"
        );
    }

    #[test]
    fn cave_roof_voxels_are_face_connected_to_the_surrounding_terrain() {
        for seed in [42042, 7, 98173] {
            check_supported_roofs(&seeded_terrain(seed));
        }
    }

    fn check_supported_roofs(terrain: &TerraGenerator) {
        let fixtures = fixtures(terrain);
        assert_eq!(
            fixtures.len(),
            3,
            "seed {} needs roof fixtures",
            terrain.config().seed.0
        );
        for (cell, _) in fixtures {
            const SIDE: usize = 88;
            let origin = [cell[0] * 128 - 44, cell[1] * 128 - 44];
            let columns: Vec<_> = (0..SIDE)
                .flat_map(|z| (0..SIDE).map(move |x| [x, z]))
                .map(|[x, z]| {
                    terrain.surface_column(
                        (origin[0] + x as i64) as f64 + 0.5,
                        (origin[1] + z as i64) as f64 + 0.5,
                    )
                })
                .collect();
            let bottom = columns
                .iter()
                .map(|c| c.cave_cavity.map_or(c.height, |v| v[0]).floor() as i64)
                .min()
                .unwrap()
                - 2;
            let top = columns
                .iter()
                .map(|c| c.height.ceil() as i64)
                .max()
                .unwrap()
                + 1;
            let height = (top - bottom + 1) as usize;
            let index = |x, y, z| (y * SIDE + z) * SIDE + x;
            let mut solid = vec![false; SIDE * SIDE * height];
            let mut connected = solid.clone();
            let mut queue = VecDeque::new();
            for y in 0..height {
                for z in 0..SIDE {
                    for x in 0..SIDE {
                        let p = [
                            (origin[0] + x as i64) as f64 + 0.5,
                            (bottom + y as i64) as f64 + 0.5,
                            (origin[1] + z as i64) as f64 + 0.5,
                        ];
                        let column = columns[z * SIDE + x];
                        let id = index(x, y, z);
                        solid[id] = terrain.sample_with_column(p, column).density <= 0.0;
                        if solid[id] && p[1] <= column.cave_cavity.map_or(column.height, |c| c[0]) {
                            connected[id] = true;
                            queue.push_back([x, y, z]);
                        }
                    }
                }
            }
            while let Some([x, y, z]) = queue.pop_front() {
                for [dx, dy, dz] in [
                    [-1, 0, 0],
                    [1, 0, 0],
                    [0, -1, 0],
                    [0, 1, 0],
                    [0, 0, -1],
                    [0, 0, 1],
                ] {
                    let (Some(nx), Some(ny), Some(nz)) = (
                        x.checked_add_signed(dx),
                        y.checked_add_signed(dy),
                        z.checked_add_signed(dz),
                    ) else {
                        continue;
                    };
                    if nx >= SIDE || ny >= height || nz >= SIDE {
                        continue;
                    }
                    let id = index(nx, ny, nz);
                    if solid[id] && !connected[id] {
                        connected[id] = true;
                        queue.push_back([nx, ny, nz]);
                    }
                }
            }
            let mut roofs = 0;
            for z in 0..SIDE {
                for x in 0..SIDE {
                    let Some([_, ceiling]) = columns[z * SIDE + x].cave_cavity else {
                        continue;
                    };
                    for y in 0..height {
                        let id = index(x, y, z);
                        if solid[id] && (bottom + y as i64) as f64 + 0.5 > ceiling {
                            assert!(
                                connected[id],
                                "detached cave roof seed={} at {},{},{}",
                                terrain.config().seed.0,
                                origin[0] + x as i64,
                                bottom + y as i64,
                                origin[1] + z as i64
                            );
                            roofs += 1;
                        }
                    }
                }
            }
            assert!(roofs > 100, "fixture must retain a real cave roof");
        }
    }

    #[test]
    fn closed_portal_tips_never_produce_nan_cavity_intervals() {
        let portal = Portal {
            mouth: [-64.0, 20.0, -32.0],
            direction: [1.0, 0.0],
            end_floor: 8.0,
            phase: 0.2,
        };
        for along in [-6.0, LENGTH + 10.0] {
            assert!(
                portal
                    .interval(portal.mouth[0] + along, portal.mouth[2])
                    .is_none()
            );
            for delta in [-1e-8, 0.0, 1e-8] {
                for across in [-1e-8, 0.0, 1e-8] {
                    if let Some([floor, ceiling]) =
                        portal.interval(portal.mouth[0] + along + delta, portal.mouth[2] + across)
                    {
                        assert!(floor.is_finite() && ceiling.is_finite());
                        assert!(ceiling > floor);
                    }
                }
            }
        }
        let terrain = terrain();
        for bad in [f64::NAN, f64::NEG_INFINITY, f64::INFINITY] {
            assert!(terrain.cave_cavity_at(bad, 0.0, 20.0, None).is_none());
            assert!(terrain.cave_cavity_at(0.0, bad, 20.0, None).is_none());
            assert!(terrain.cave_cavity_at(0.0, 0.0, bad, None).is_none());
        }
    }

    #[test]
    fn entrance_sections_match_direct_samples_and_cache_rebuilds_at_negative_coordinates() {
        let terrain = terrain();
        let (_, portal) = fixtures(&terrain)[0];
        assert!(portal.mouth[0] < 0.0 || portal.mouth[2] < 0.0);
        let point = [
            (portal.mouth[0] + portal.direction[0] * 20.0).floor() as i64,
            ((portal.mouth[1] + portal.end_floor) * 0.5).floor() as i64,
            (portal.mouth[2] + portal.direction[1] * 20.0).floor() as i64,
        ];
        let generator = SectionGenerator::new(terrain.clone());
        let owner = terra_voxel::global_to_section(point).0;
        for offset in [[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1]] {
            let position = owner.offset(offset).unwrap();
            let section = generator
                .build(position, 0, &Default::default(), true)
                .unwrap();
            let headless = generator.generate(position).unwrap();
            assert_eq!(section.data, headless);
            let origin = position.origin().unwrap();
            for a in 0..16 {
                for b in 0..16 {
                    let local = [0, a, b];
                    let global = std::array::from_fn(|i| origin[i] + local[i] as i64);
                    assert_eq!(
                        section.data.block(local).unwrap(),
                        generator.block_at(global).unwrap()
                    );
                }
            }
        }
        let before = terrain.surface_column(point[0] as f64 + 0.5, point[2] as f64 + 0.5);
        terrain.cave_portals.0.lock().unwrap().clear();
        assert_eq!(
            before,
            terrain.surface_column(point[0] as f64 + 0.5, point[2] as f64 + 0.5)
        );
        let disabled = TerraGenerator::new(WorldConfig {
            caves: false,
            ..Default::default()
        })
        .unwrap();
        assert!(
            disabled
                .surface_column(portal.mouth[0], portal.mouth[2])
                .cave_cavity
                .is_none()
        );
    }
}
