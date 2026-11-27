use super::{OasisSample, RiverSample, TerraGenerator, biomes::BiomeSample, biomes::smoothstep};
use noise::NoiseFn;

pub(super) struct SurfaceWaters {
    pub(super) course: Option<RiverSample>,
    pub(super) cascade: Option<RiverSample>,
    pub(super) oasis: Option<OasisSample>,
    pub(super) lake: Option<OasisSample>,
}

impl TerraGenerator {
    pub(super) fn surface_waters(&self, x: f64, z: f64, savanna: Option<f64>) -> SurfaceWaters {
        let course = self.river_course_at(x, z);
        SurfaceWaters {
            course,
            cascade: self.savanna_cascade_at(x, z, course, savanna),
            oasis: self.oasis_at(x, z),
            lake: self.lake_at(x, z),
        }
    }

    pub fn surface_height(&self, x: f64, z: f64) -> f64 {
        if self.config().landscape_revision >= 5 && self.config().ecology_revision < 16 {
            return self.carved_surface_height(
                x,
                z,
                self.terrain_base_height(x, z),
                &self.surface_waters(x, z, None),
            );
        }
        self.height_with_biome(x, z, self.climate_biome_at(x, z))
    }

    pub(super) fn height_with_biome(&self, x: f64, z: f64, biome: BiomeSample) -> f64 {
        self.carved_surface_height(
            x,
            z,
            self.watershed_ground(x, z, biome),
            &self.surface_waters(x, z, Some(biome.weights.savanna)),
        )
    }

    /// The banks that constrain a reservoir are the actual pre-carving terrain.
    /// A hydraulic head fitted to the original hills can sit far above a basalt valley.
    pub(super) fn watershed_ground(&self, x: f64, z: f64, biome: BiomeSample) -> f64 {
        let raw = self.base_height_with_biome(x, z, biome);
        let raw = self.coastal_relief(x, z, raw, biome.weights);
        let raw = self.crystal_height(x, z, raw, biome.weights.crystal_highlands);
        let raw = self.petrified_height(x, z, raw, biome.weights.petrified_forest);
        let raw = self.swamp_height(x, z, raw, biome.weights.swamp);
        let volcanic = self.volcanic_strength(biome);
        let height = self.volcanic_height(
            x,
            z,
            self.savanna_height(x, z, raw, biome.weights.savanna),
            volcanic,
        );
        self.gully_height(x, z, height, volcanic)
    }

    pub(super) fn watershed_height(&self, x: f64, z: f64) -> f64 {
        self.watershed_ground(x, z, self.climate_biome_at(x, z))
    }

    pub(super) fn carved_surface_height(
        &self,
        x: f64,
        z: f64,
        raw: f64,
        waters: &SurfaceWaters,
    ) -> f64 {
        let river = waters.course;
        let height = river.map_or(raw, |river| {
            let mut height = river_height(raw, river, self.config().sea_level);
            if let Some(lake) = waters.lake.filter(|_| self.connected_hydrology()) {
                let wet_channel = river.distance < river.width
                    && (river.bed_height() - 0.5).floor() + 1.0 < river.level;
                if !wet_channel {
                    // An outlet may descend below its lake. Its exterior eroded
                    // apron must not cut holes in the lake's higher dry shore.
                    let protect =
                        1.0 - smoothstep(lake.radius + 6.0, lake.radius + 18.0, lake.distance);
                    // Retain enough bank to contain the lake. Restoring the
                    // entire pre-carving hillside created sheer walls alongside
                    // the last wet voxel of channels entering a high basin.
                    let rim = if self.config().river_terrain_revision >= 5 {
                        raw.min(lake.level.ceil() + 0.5)
                    } else {
                        raw
                    };
                    height += (rim - height).max(0.0) * protect;
                }
            }
            let Some(cavity) = river.cavity() else {
                return height;
            };
            if height >= cavity[1] && !self.river_roof_supported(x, z, height, cavity) {
                height.min(cavity[0])
            } else {
                height
            }
        });
        let height = waters.cascade.map_or(height, |stream| {
            river_height(height, stream, self.config().sea_level)
        });
        let height = waters.oasis.map_or(height, |oasis| {
            if self.desert_overhaul() {
                oasis.carve_desert(height)
            } else {
                oasis.carve(height)
            }
        });
        let lake = waters.lake;
        let height = lake.map_or(height, |lake| lake.carve(height));
        self.spirit_hill_relief(x, z, height, river, lake)
    }

    fn river_roof_supported(&self, x: f64, z: f64, height: f64, cavity: [f64; 2]) -> bool {
        let span = RoofSpan::new(height, Some(cavity), self.config().caves);
        if span.bottom > span.top {
            return false;
        }
        // A smooth exterior height and an independently carved arch can intersect
        // in detached roof islands. Keep only voxel roof spans with an overlapping
        // face-connected path to a bank. This runs per column, never per voxel.
        let cross = if self.connected_hydrology() {
            self.river_at(x, z).map_or([1.0, 0.0], |river| {
                // Stay on face-neighbour voxels even for diagonal courses.
                if river.flow[0].abs() > river.flow[1].abs() {
                    [0.0, 1.0]
                } else {
                    [1.0, 0.0]
                }
            })
        } else {
            [1.0, 0.0]
        };
        [-1.0, 1.0].into_iter().any(|direction| {
            let mut previous = span;
            // Covers both banks even at the widest fork/terminal pool.
            for step in 1..=64 {
                let sx = x + direction * f64::from(step) * cross[0];
                let sz = z + direction * f64::from(step) * cross[1];
                let raw = self.watershed_height(sx, sz);
                let river = self.river_at(sx, sz);
                let h = river.map_or(raw, |r| river_height(raw, r, self.config().sea_level));
                let next =
                    RoofSpan::new(h, river.and_then(RiverSample::cavity), self.config().caves);
                if previous.bottom.max(next.protected_bottom) <= previous.top.min(next.floor) {
                    return true;
                }
                if previous.bottom.max(next.bottom) > previous.top.min(next.top) {
                    return false;
                }
                previous = next;
            }
            false
        })
    }

    pub(super) fn base_height_with_biome(&self, x: f64, z: f64, biome: BiomeSample) -> f64 {
        let config = self.config();
        if config.landscape_revision >= 5 {
            if config.hill_amplitude == 0.0 && config.mountain_amplitude == 0.0 {
                return config.base_height;
            }
            return self.regional_base_height(x, z, self.advanced.height(x, z), biome);
        }
        if config.landscape_revision >= 4 {
            if config.hill_amplitude == 0.0 && config.mountain_amplitude == 0.0 {
                return config.base_height;
            }
            return self.configured_height(x, z, biome);
        }
        let profile = biome.profile();
        let hills = self.hills.get([x * 0.008 + 0.37, z * 0.008 + 0.71]);
        let detail = self.hills.get([x * 0.033 + 1.17, z * 0.033 + 2.31]);
        let mountains = (self.mountains.get([x * 0.0018 + 0.53, z * 0.0018 + 0.89]) + 0.35)
            .max(0.0)
            .powi(2);
        // A winding low-frequency ridge field carves broad continuous valleys.
        let warp = self.hills.get([x * 0.002 + 3.17, z * 0.002 + 1.23]) * 28.0;
        let valley_noise = self
            .valleys
            .get([(x + warp) * 0.0028 + 0.19, z * 0.0028 + 0.47]);
        let valley = 1.0 - smoothstep(0.03, 0.30, valley_noise.abs());
        let raw = config.base_height
            + profile.elevation
            + smoothstep(0.16, 0.68, biome.weights.highland) * config.mountain_amplitude * 1.65
            + hills * config.hill_amplitude * profile.hills
            + mountains * config.mountain_amplitude * profile.mountains
            - valley * config.hill_amplitude * 0.95;
        if config.hill_amplitude == 0.0 && config.mountain_amplitude == 0.0 {
            return config.base_height;
        }
        let step = config.terrace_height * profile.terrace_scale;
        let phase = hills * config.hill_amplitude * 0.12;
        let terraced = terrace(raw, step, phase);
        let strength =
            config.terrace_strength * (1.0 - biome.weights.ocean * 0.9 - biome.aridity() * 0.85);
        let dune =
            (x * 0.018 + detail * 2.0).sin() * config.hill_amplitude * 0.22 * biome.aridity();
        let seabed = smoothstep(0.58, 0.92, biome.weights.ocean)
            * (self.valleys.get([x * 0.012 + 8.13, z * 0.012 - 3.71]) * 4.0
                + self.hills.get([x * 0.045 - 2.47, z * 0.045 + 5.39]) * 1.4);
        let cliffs = if biome.weights.cliffted_desert > 0.0 {
            smoothstep(
                -0.035,
                0.035,
                self.valleys.get([x * 0.008 + 13.9, z * 0.008 - 21.3]),
            ) * config.mountain_amplitude
                * 0.45
                * biome.weights.cliffted_desert
        } else {
            0.0
        };
        let height = raw
            + (terraced - raw) * strength
            + detail * config.hill_amplitude * 0.13
            + dune
            + seabed
            + cliffs;
        let height = super::spirit_blossom::terraced_height(height, biome.weights.spirit_blossom);
        self.ocean_relief(x, z, biome, self.landscape_relief(x, z, biome, height))
    }
}

pub(super) fn river_height(raw: f64, river: RiverSample, sea_level: f64) -> f64 {
    if river.covered || raw < sea_level - 1.0 {
        return raw;
    }
    let channel = if river.distance < river.width {
        river.bed_height()
    } else {
        let reach = (14.0 + (raw - river.bank_level).max(0.0) * 1.8)
            .clamp(14.0, 64.0)
            .min(river.bank_reach);
        let blend = smoothstep(river.width, river.width + reach, river.distance);
        // Wet edge cells still have a full-height bank. Beyond the shore, the
        // waterfall's sharp level change gives way to the continuous eroded valley.
        let shore = 1.0 - smoothstep(river.width + 1.5, river.width + 10.0, river.distance);
        let bank = river.bank_level + (river.level - river.bank_level) * shore;
        let detail = river.bank_erosion * (blend * (1.0 - blend) * 4.0);
        bank + 0.45 + (raw - bank - 0.45) * blend + detail
    };
    // Carve a valley into existing land; never raise artificial retaining walls.
    let channel = river
        .island_height
        .map_or(channel, |height| channel.min(river.level + height));
    let open = raw.min(channel);
    open + (raw - open) * river.roof_blend
}

#[derive(Clone, Copy)]
struct RoofSpan {
    bottom: i64,
    top: i64,
    floor: i64,
    protected_bottom: i64,
}

impl RoofSpan {
    fn new(height: f64, cavity: Option<[f64; 2]>, caves: bool) -> Self {
        // The general cave field cannot carve the top five metres. Restrict
        // support to that guaranteed solid band when caves are enabled.
        let protected_bottom = if caves {
            (height - 5.0 - 0.5).ceil() as i64
        } else {
            i64::MIN
        };
        Self {
            // The explicit river roof excludes the independent cave field.
            bottom: cavity.map_or(protected_bottom, |c| (c[1] - 0.5).ceil() as i64),
            top: (height - 0.5).floor() as i64,
            floor: (cavity.map_or(height, |c| height.min(c[0])) - 0.5).floor() as i64,
            protected_bottom,
        }
    }
}

/// Continuous shelf-to-cliff curve; floor's jump cancels at each period boundary.
pub(super) fn terrace(height: f64, step: f64, phase: f64) -> f64 {
    let level = (height - phase) / step;
    (level.floor() + smoothstep(0.34, 0.66, level - level.floor())) * step + phase
}
