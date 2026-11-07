use super::{TerraGenerator, biomes::BiomeSample, biomes::smoothstep};
use noise::NoiseFn;
use terra_voxel::MaterialId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceColumn {
    pub(crate) surface_shape: super::advanced::SurfaceShape,
    pub cave_biome: super::CaveBiomeColumn,
    pub cave: super::CaveStructure,
    pub seagrass_height: u8,
    pub ocean: super::OceanHabitat,
    pub water_level: Option<f64>,
    pub water_flow: [f32; 2],
    /// Actual floor and ceiling of a covered river, prepared once per x/z column.
    pub river_cavity: Option<[f64; 2]>,
    pub cave_cavity: Option<[f64; 2]>,
    pub(crate) cave_shape: super::caves::CaveShape,
    pub oasis_moisture: f64,
    pub height: f64,
    pub slope: f64,
    pub soil_depth: f64,
    pub biome: BiomeSample,
    pub snow_cover: f64,
    pub shore_cover: f64,
    pub surface_material: MaterialId,
    /// Material-only deposits; none of these fields change density or water levels.
    pub clay_depth: f64,
    pub sandstone_depth: f64,
    pub moss_entrance: bool,
    pub moss_daylight_floor: f64,
}

impl SurfaceColumn {
    pub fn biome_kind(&self, sea_level: f64) -> super::biomes::BiomeKind {
        let mut weights = self.biome.weights;
        let marine = self.water_level.is_some_and(|level| {
            (level - sea_level).abs() < 0.01
                && self.height < level
                && self.water_bed() < level
                && weights.ocean > 0.45
        });
        if marine {
            return super::biomes::BiomeKind::Ocean;
        }
        weights.ocean = 0.0;
        if weights.values().iter().all(|w| *w <= f64::EPSILON) {
            super::biomes::BiomeKind::Plains
        } else {
            weights.dominant()
        }
    }
    pub fn water_columns(self) -> crate::water::WaterColumns {
        crate::water::WaterColumns {
            intervals: [
                self.water_level.map(|level| {
                    let bed = self.water_bed();
                    crate::water::WaterColumn {
                        level,
                        bed,
                        depth: level - bed,
                    }
                }),
                self.cave.water,
                None,
            ],
        }
    }
    pub fn ice_bottom(self) -> Option<f64> {
        (self.ocean.ice_depth > 0).then(|| {
            self.water_level.unwrap_or(self.height).floor() + 1.0 - f64::from(self.ocean.ice_depth)
        })
    }
    /// Shared rounded fluid floor, also used by render-only exposure snapshots.
    pub fn water_bed(self) -> f64 {
        let floor = self
            .river_cavity
            .map_or(self.height, |cavity| self.height.min(cavity[0]));
        (floor - 0.5).floor() + 1.0
    }
}

impl TerraGenerator {
    fn surface_cover(
        &self,
        biome: BiomeSample,
        height: f64,
        level: f64,
        oasis: Option<super::OasisSample>,
        dry: bool,
    ) -> [f64; 2] {
        let snow_height = self.config().snow_height + (biome.temperature - 0.5) * 16.0;
        let snow = smoothstep(snow_height - 5.0, snow_height + 5.0, height);
        let coast = 1.0 - smoothstep(level, level + 3.0, height);
        let mut shore = 1.0 - (1.0 - coast) * (1.0 - biome.aridity());
        if let Some(oasis) = oasis.filter(|oasis| dry && oasis.distance > oasis.radius * 0.9) {
            // A continuous dry-grass belt, not independently randomized green cells.
            shore *= 1.0 - smoothstep(0.12, 0.65, oasis.moisture());
        }
        [snow, shore]
    }

    pub fn biome_at(&self, x: f64, z: f64) -> BiomeSample {
        let climate = self.climate_biome_at(x, z);
        if self.config().landscape_revision < 4 && climate.weights.highland == 0.0 {
            return climate;
        }
        self.surface_ecology(climate, self.height_with_biome(x, z, climate))
    }

    /// Visible surface identity: ocean requires a real marine water column.
    /// Climate weights stay intact for generation and smooth material blending.
    pub fn surface_biome(&self, column: &SurfaceColumn) -> super::biomes::BiomeKind {
        column.biome_kind(self.config().sea_level)
    }

    pub(super) fn climate_biome_at(&self, x: f64, z: f64) -> BiomeSample {
        if self.config().biomes {
            let biome = if self.config().landscape_revision >= 5 {
                self.advanced_biome(x, z)
            } else {
                let mut biome = self.climate.sample(x, z);
                if self.config().ecology_revision > 0 {
                    biome = self.climate.with_cherries(biome, x, z);
                }
                if self.config().geology_revision > 0 {
                    biome = self.climate.with_cliffs(biome, x, z);
                }
                biome
            };
            self.config()
                .biome_definitions
                .as_ref()
                .map_or(biome, |definitions| biome.with_distribution(definitions))
        } else {
            BiomeSample {
                temperature: 0.5,
                humidity: 0.75,
                weights: super::biomes::BiomeWeights {
                    ocean: 0.0,
                    plains: 0.0,
                    forest: 1.0,
                    desert: 0.0,
                    highland: 0.0,
                    mushroom_park: 0.0,
                    cliffted_desert: 0.0,
                    cherry_forest: 0.0,
                },
            }
        }
    }

    /// Cached per horizontal column by section jobs, never per voxel there.
    pub fn surface_column(&self, x: f64, z: f64) -> SurfaceColumn {
        let climate = self.climate_biome_at(x, z);
        let height = self.height_with_biome(x, z, climate);
        let biome = self.surface_ecology(climate, height);
        let river = self.river_at(x, z);
        let oasis = self.oasis_at(x, z).or_else(|| self.lake_at(x, z));
        let level = oasis
            .filter(|sample| sample.distance <= sample.radius)
            .map_or_else(
                || {
                    river.map_or(self.config().sea_level, |river| {
                        river.water_level(self.config().sea_level)
                    })
                },
                |sample| sample.level,
            );
        let river_cavity = river.and_then(|river| river.cavity());
        let cave_cavity = if self.config().bounded_world && (river.is_some() || oasis.is_some()) {
            None
        } else {
            self.cave_cavity_at(x, z, height, river_cavity)
        };
        let oasis_moisture = oasis.map_or(0.0, |sample| sample.moisture());
        let floor = river_cavity.map_or(height, |cavity| height.min(cavity[0]));
        let bed = (floor - 0.5).floor() + 1.0;
        let water_level = (self.config().water && bed < level).then_some(level);
        let mut ocean = self.ocean_with_biome(x, z, biome);
        if water_level.is_none()
            || (level - self.config().sea_level).abs() > 0.01
            || level - bed < 3.0
        {
            ocean.ice_depth = 0;
        } else {
            // Always leave a water passage between the seabed and the ice.
            ocean.ice_depth = ocean.ice_depth.min((level - bed - 1.0).max(0.0) as u8);
        }
        let neighbors = [
            self.surface_height(x + 1.0, z),
            self.surface_height(x - 1.0, z),
            self.surface_height(x, z + 1.0),
            self.surface_height(x, z - 1.0),
        ];
        let dx = (neighbors[0] - neighbors[1]) * 0.5;
        let dz = (neighbors[2] - neighbors[3]) * 0.5;
        let slope = dx.hypot(dz);
        let relief = height - neighbors.into_iter().fold(height, f64::min);
        // One-block grass steps retain dirt. Only genuine >=3m cliffs expose
        // shallow rock, and a coherent meadow mask also leaves green mountains.
        let rocky = smoothstep(
            -0.18,
            0.28,
            self.valleys.get([x * 0.004 + 4.71, z * 0.004 - 2.13]),
        );
        let exposure = smoothstep(3.0, 7.0, relief) * rocky;
        let [snow_cover, shore_cover] =
            self.surface_cover(biome, height, level, oasis, water_level.is_none());
        // Materials remain real, integral blocks. The renderer blends their top
        // colors across this contour instead of dithering material IDs per cell.
        let mut surface_material = if snow_cover >= 0.5 && biome.aridity() <= 0.5 {
            MaterialId::Snow
        } else if shore_cover >= 0.5 {
            MaterialId::Sand
        } else {
            MaterialId::Grass
        };
        let marine_clay = self.config().landscape_revision >= 4
            && self.config().ocean_revision > 0
            && ocean.weights.iter().copied().sum::<f32>() > 0.1
            && water_level.is_some_and(|water| (water - self.config().sea_level).abs() < 0.01);
        let clay_depth = if marine_clay {
            f64::from(ocean.clay_depth)
        } else if water_level.is_some()
            && (surface_material == MaterialId::Sand || river_cavity.is_some())
        {
            let patch = self.valleys.get([x * 0.061 + 73.4, z * 0.061 - 29.8])
                + self.hills.get([x * 0.19 - 11.7, z * 0.19 + 47.2]) * 0.24;
            if patch > 0.16 {
                1.0 + (patch - 0.16) * 3.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        let sandstone_depth = if biome.aridity() > 0.25 {
            6.0 + biome.weights.cliffted_desert * 14.0
                + self.hills.get([x * 0.035 + 17.9, z * 0.035 - 68.1]) * 3.0
        } else {
            0.0
        };
        // Existing river galleries have a genuine eroded mouth where roof_blend
        // ramps in. Moss belongs to occasional damp mouths, not all cliff faces.
        let moss_patch =
            biome.humidity > 0.45 && self.valleys.get([x * 0.075 - 32.7, z * 0.075 + 81.3]) > 0.08;
        let moss_entrance = moss_patch
            && (river.is_some_and(|r| r.roof_blend > 0.05 && r.roof_blend < 0.85)
                || cave_cavity.is_some_and(|cavity| cavity[1] >= height - 2.0));
        // Ordinary caves can meet daylight at steep terrain sides. Only these
        // rare damp columns need the wider exterior-height check, never each voxel.
        let moss_daylight_floor = if moss_patch && self.config().caves && slope > 1.0 {
            [[-4.0, 0.0], [4.0, 0.0], [0.0, -4.0], [0.0, 4.0]]
                .into_iter()
                .map(|[dx, dz]| self.surface_height(x + dx, z + dz))
                .fold(f64::INFINITY, f64::min)
        } else {
            f64::INFINITY
        };
        let profile_soil = if self.config().landscape_revision >= 4 {
            self.configured_soil_depth(biome)
        } else {
            biome.profile().soil_depth.max(5.0)
        };
        let mut soil_depth =
            (profile_soil + (1.0 - rocky) * 4.0) * (1.0 - exposure) + 4.5 * exposure;
        if let Some(river) = river.filter(|river| river.fall_erosion > 0.0) {
            let raw = self.base_height_with_biome(x, z, climate);
            let stripped = river.fall_erosion
                * smoothstep(0.75, 4.0, raw - height)
                * smoothstep(0.5, 1.65, slope);
            if stripped > 0.0 {
                soil_depth = soil_depth * (1.0 - stripped).powi(3) + 0.45 * stripped;
                if stripped > 0.55 {
                    surface_material = match surface_material {
                        MaterialId::Grass => MaterialId::Stone,
                        MaterialId::Sand => MaterialId::Sandstone,
                        material => material,
                    };
                }
            }
        }
        let mut column = SurfaceColumn {
            surface_shape: super::advanced::SurfaceShape {
                strength: if self.config().landscape_revision >= 5
                    && water_level.is_none()
                    && river.is_none()
                    && oasis.is_none()
                    && self.config().hill_amplitude + self.config().mountain_amplitude > 0.0
                {
                    self.advanced.sample(x, z).overhangs * smoothstep(0.25, 1.1, slope)
                } else {
                    0.0
                },
            },
            cave_biome: self.cave_biome_column_with_surface(x, z, height),
            cave: self.cave_structure_at(x, z),
            seagrass_height: 0,
            ocean,
            water_level,
            water_flow: river
                .filter(|river| river.distance <= river.width + 1.0)
                .map_or([0.0; 2], |river| river.flow),
            river_cavity,
            cave_cavity,
            cave_shape: self.cave_shape(x, z),
            oasis_moisture,
            height,
            slope,
            soil_depth,
            biome,
            snow_cover,
            shore_cover,
            surface_material,
            clay_depth,
            sandstone_depth,
            moss_entrance,
            moss_daylight_floor,
        };
        if self.config().cave_structure_revision > 0
            && let Some(level) = water_level
        {
            let ground = (height - 0.5).floor() as i64;
            let support = [x.floor() as i64, ground, z.floor() as i64];
            let material = self.column_material(column, height - ground as f64 - 0.5, -0.5);
            if let Some(prop) = self.aquatic_prop(
                support,
                terra_voxel::global_to_section(support).0,
                material as u16,
                column,
                level - ground as f64 - 1.0,
            ) && prop.kind == crate::props::PropKind::Seagrass
            {
                column.seagrass_height = ((prop.bounds()[1][1] - prop.position[1]).ceil() as u8)
                    .min((level - ground as f64 - 1.2).floor().max(0.0) as u8)
                    .min(8);
            }
        }
        column
    }

    pub fn forest_probability(&self, x: f64, z: f64) -> f64 {
        self.forest_probability_with_biome(x, z, self.biome_at(x, z))
    }

    pub(crate) fn forest_probability_with_biome(&self, x: f64, z: f64, biome: BiomeSample) -> f64 {
        let density = if self.config().landscape_revision >= 4 {
            self.configured_tree_density(biome)
        } else {
            biome.profile().forest_cover
        };
        (density * self.climate.forest_mask(x, z)).clamp(0.0, 1.0)
            * self.climate.ground_masks(x, z).1
    }

    pub(super) fn column_material(
        &self,
        column: SurfaceColumn,
        depth: f64,
        density: f64,
    ) -> MaterialId {
        if density > 0.0 {
            MaterialId::Air
        } else if self.config().landscape_revision >= 4
            && self.config().ocean_revision > 0
            && column.ocean.weights.iter().copied().sum::<f32>() > 0.1
            && column.clay_depth > 0.0
            && {
                let bed_depth = column.water_bed() - 0.5 - (column.height - depth);
                (-0.1..column.clay_depth).contains(&bed_depth)
            }
        {
            MaterialId::Clay
        } else if self.config().ocean_revision > 0
            && depth < 5.0
            && column.water_level.is_some()
            && let Some(material) = column.ocean.floor_material()
        {
            if depth < 3.5 {
                material
            } else {
                MaterialId::Stone
            }
        } else if column.clay_depth > 0.0 && {
            let bed_depth = column.water_bed() - 0.5 - (column.height - depth);
            (-0.1..column.clay_depth).contains(&bed_depth)
        } {
            MaterialId::Clay
        } else if density >= -1.25
            && depth > column.soil_depth
            && (column.moss_entrance
                || (depth >= 5.0 && column.height - depth >= column.moss_daylight_floor))
        {
            MaterialId::MossyStone
        } else if column.surface_material == MaterialId::Sand
            && column.sandstone_depth > 0.0
            && depth >= 1.5
            && depth < column.sandstone_depth
        {
            MaterialId::Sandstone
        } else if column.surface_material == MaterialId::Sand
            && column.biome.aridity() > 0.5
            && depth <= 1.5
        {
            column.surface_material
        } else if depth > column.soil_depth {
            MaterialId::Stone
        } else if column.surface_material == MaterialId::Sand && column.biome.aridity() > 0.05 {
            MaterialId::Sand
        } else if column.surface_material == MaterialId::Grass {
            let top_voxel = (column.height - 0.5).floor();
            if column.height - depth >= top_voxel {
                MaterialId::Grass
            } else {
                MaterialId::Dirt
            }
        } else if depth > 1.5 {
            MaterialId::Dirt
        } else {
            column.surface_material
        }
    }

    pub fn material_at(&self, height: f64, depth: f64, density: f64) -> MaterialId {
        if density > 0.0 {
            MaterialId::Air
        } else if depth > 4.0 {
            MaterialId::Stone
        } else if depth > 1.5 {
            MaterialId::Dirt
        } else {
            self.surface_material(height, self.config().snow_height)
        }
    }

    fn surface_material(&self, height: f64, snow_height: f64) -> MaterialId {
        if height <= self.config().sea_level + 1.5 {
            MaterialId::Sand
        } else if height >= snow_height {
            MaterialId::Snow
        } else {
            MaterialId::Grass
        }
    }
}
