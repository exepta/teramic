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
    pub(crate) falls: [Option<crate::water::falls::Fall>; 3],
    pub water_level: Option<f64>,
    pub lava_level: Option<f64>,
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

fn open_ocean(water: Option<f64>, sea: f64, height: f64, bed: f64, weight: f64) -> bool {
    water.is_some_and(|level| {
        (level - sea).abs() < 0.01 && height < level && bed < level && weight > 0.45
    })
}

impl SurfaceColumn {
    pub fn biome_kind(&self, sea_level: f64) -> super::biomes::BiomeKind {
        let weights = self.biome.weights;
        if open_ocean(
            self.water_level,
            sea_level,
            self.height,
            self.water_bed(),
            weights.ocean,
        ) {
            return super::biomes::BiomeKind::Ocean;
        }
        weights.dominant_land()
    }
    pub fn water_columns(self) -> crate::water::WaterColumns {
        crate::water::WaterColumns {
            falls: self.falls,
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
        // A desert can lead a blend without owning half of its total weight.
        // Use the same land identity as the HUD: otherwise those whole desert
        // districts become meadows merely because several neighbours contribute.
        let mut land = biome.weights;
        land.ocean = 0.0;
        let desert = matches!(
            land.dominant(),
            super::biomes::BiomeKind::Desert | super::biomes::BiomeKind::ClifftedDesert
        );
        let sand = if self.desert_overhaul() {
            // Inland mixed grassland retains a real turf and soil column. True dunes
            // begin inside the desert, independently of the shore of a water body.
            smoothstep(0.64, 0.86, biome.aridity())
        } else if desert {
            biome.aridity().max(0.6)
        } else {
            biome.aridity()
        };
        let mut shore = 1.0 - (1.0 - coast) * (1.0 - sand);
        if let Some(oasis) = oasis.filter(|oasis| dry && oasis.distance > oasis.radius * 0.9) {
            // A continuous dry-grass belt, not independently randomized green cells.
            shore *= 1.0 - smoothstep(0.12, 0.65, oasis.moisture());
        }
        [snow, shore]
    }

    pub fn biome_at(&self, x: f64, z: f64) -> BiomeSample {
        let climate = self.climate_biome_at(x, z);
        let revision = self.config().landscape_revision;
        // Revision 4 gates every biome on elevation. Modern ranges and the
        // original mountain band only need to sample erosion inside their climate mask.
        if (revision != 4 && climate.weights.highland == 0.0)
            || (revision >= 5 && !self.highland_overhaul())
        {
            return climate;
        }
        self.surface_ecology(climate, self.height_with_biome(x, z, climate))
    }

    /// Visible surface identity: ocean requires a real marine water column.
    /// Climate weights stay intact for generation and smooth material blending.
    pub fn surface_biome(&self, column: &SurfaceColumn) -> super::biomes::BiomeKind {
        column.biome_kind(self.config().sea_level)
    }

    pub(crate) fn climate_biome_at(&self, x: f64, z: f64) -> BiomeSample {
        if self.config().biomes {
            if self.config().landscape_revision >= 5 {
                let node = self.advanced.sample(x, z);
                let biome = self.biome_from_node(node);
                if self.config().landscape_revision >= 9 {
                    return self.with_misty_coast(
                        x,
                        z,
                        self.regional_biome(x, z, biome),
                        node.height,
                    );
                }
                return self.apply_biome_overlays(x, z, biome);
            }
            let mut biome = self.climate.sample(x, z);
            if self.config().ecology_revision > 0 {
                biome = self.climate.with_cherries(biome, x, z);
            }
            if self.config().geology_revision > 0 {
                biome = self.climate.with_cliffs(biome, x, z);
            }
            self.apply_biome_overlays(x, z, biome)
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
                    spirit_blossom: 0.0,
                    blood_forest: 0.0,
                    ashlands: 0.0,
                    savanna: 0.0,
                    swamp: 0.0,
                    petrified_forest: 0.0,
                    crystal_highlands: 0.0,
                    misty_coast: 0.0,
                },
            }
        }
    }

    pub(super) fn apply_biome_overlays(&self, x: f64, z: f64, biome: BiomeSample) -> BiomeSample {
        let biome = if self.config().spirit_blossom_revision > 0 {
            self.climate.with_spirit_blossom(biome, x, z)
        } else {
            biome
        };
        let biome = if self.config().blood_forest_revision > 0 {
            self.climate.with_blood_forest(biome, x, z)
        } else {
            biome
        };
        let biome = if self.config().ecology_revision >= 16 && self.config().landscape_revision >= 9
        {
            self.with_ashlands(biome, x, z)
        } else {
            biome
        };
        let biome = if self.config().ecology_revision >= 17 && self.config().landscape_revision >= 9
        {
            self.with_savanna(biome, x, z)
        } else {
            biome
        };
        let biome = if self.config().ecology_revision >= 18 && self.config().landscape_revision >= 9
        {
            self.with_swamp(biome, x, z)
        } else {
            biome
        };
        let biome = if self.config().ecology_revision >= 19 && self.config().landscape_revision >= 9
        {
            self.with_petrified_forest(biome, x, z)
        } else {
            biome
        };
        let biome = if self.config().ecology_revision >= 21 && self.config().landscape_revision >= 9
        {
            self.with_crystal_highlands(biome, x, z)
        } else {
            biome
        };
        let biome = self
            .config()
            .biome_definitions
            .as_ref()
            .map_or(biome, |definitions| biome.with_distribution(definitions));
        if self.config().landscape_revision >= 8 {
            biome.with_thermal_buffers()
        } else {
            biome
        }
    }

    /// Cached per horizontal column by section jobs, never per voxel there.
    pub fn surface_column(&self, x: f64, z: f64) -> SurfaceColumn {
        self.surface_column_with_heights(x, z, |dx, dz| {
            self.surface_height(x + f64::from(dx), z + f64::from(dz))
        })
    }

    pub(super) fn surface_column_grid(&self, start: [i64; 2], size: usize) -> Vec<SurfaceColumn> {
        // Slope samples the four immediate neighbours. Moss also looks four metres
        // outward. Each distinct height in this job should only run worldgen once.
        const HALO: i32 = 4;
        let stride = size + 2 * HALO as usize;
        let mut heights = vec![f64::NAN; stride * stride];
        let mut columns = Vec::with_capacity(size * size);
        for z in 0..size {
            for x in 0..size {
                let wx = (start[0] + x as i64) as f64 + 0.5;
                let wz = (start[1] + z as i64) as f64 + 0.5;
                columns.push(self.surface_column_with_heights(wx, wz, |dx, dz| {
                    let ix = (x as i32 + HALO + dx) as usize;
                    let iz = (z as i32 + HALO + dz) as usize;
                    let height = &mut heights[ix + stride * iz];
                    if height.is_nan() {
                        *height = self.surface_height(wx + f64::from(dx), wz + f64::from(dz));
                    }
                    *height
                }));
            }
        }
        columns
    }

    fn surface_column_with_heights(
        &self,
        x: f64,
        z: f64,
        mut neighbor_height: impl FnMut(i32, i32) -> f64,
    ) -> SurfaceColumn {
        let climate = self.climate_biome_at(x, z);
        let waters = self.surface_waters(x, z, Some(climate.weights.savanna));
        let height =
            self.carved_surface_height(x, z, self.watershed_ground(x, z, climate), &waters);
        let biome = self.surface_ecology(climate, height);
        let river = self.surface_river(x, z, waters.course, waters.cascade);
        let oasis = waters.oasis.or(waters.lake);
        let volcanic = self.volcanic_strength(climate);
        let lava_level = self.volcanic_lava_level(x, z, volcanic, height, river, oasis);
        let level = oasis
            .and_then(|sample| sample.surface_water_level())
            .unwrap_or_else(|| {
                river.map_or(self.config().sea_level, |river| {
                    river.water_level(self.config().sea_level)
                })
            });
        let river_cavity = river
            .and_then(|river| river.cavity())
            .or_else(|| self.stack_opening(x, z, climate, height))
            .or_else(|| {
                (river.is_none() && oasis.is_none() && biome.weights.coastal_land_fraction() > 0.15)
                    .then(|| self.tidal_cavity(x, z, height))
                    .flatten()
            });
        let cave_cavity = if self.config().bounded_world && (river.is_some() || oasis.is_some()) {
            None
        } else {
            self.cave_cavity_at(x, z, height, river_cavity).or_else(|| {
                if river_cavity.is_some() || oasis.is_some() || river.is_some() {
                    return None;
                }
                self.fossil_gallery(x, z, height, climate.weights.petrified_forest)
            })
        };
        // Above the lava datum a dry ravine can run into a molten bank.
        // Taper its cut to zero on approach so it never opens a tall wall
        // exactly at the shoreline. Covered tunnels may continue underneath.
        let cave_cavity = cave_cavity.and_then(|[floor, ceiling]| {
            if lava_level.is_some() && ceiling > height - 6. {
                return None;
            }
            if volcanic > 0.04 && ceiling >= height {
                let bank = (height - (self.config().sea_level + 20.)).max(0.);
                let volcanic = super::biomes::smoothstep(0.04, 0.74, volcanic);
                let cut = height - floor;
                let depth = cut + (bank.min(cut) - cut) * volcanic;
                return (depth > 0.5).then_some([height - depth, ceiling]);
            }
            Some([floor, ceiling])
        });
        let oasis_moisture = oasis.map_or(0.0, |sample| sample.moisture());
        let floor = river_cavity.map_or(height, |cavity| height.min(cavity[0]));
        let bed = (floor - 0.5).floor() + 1.0;
        let water_level = (self.config().water && bed < level).then_some(level);
        // An inland lake or the dry roof of a sea arch may share a coastal climate.
        // Reef substrates and communities require an actual open marine column.
        let coastal_water = open_ocean(
            water_level,
            self.config().sea_level,
            height,
            bed,
            biome.weights.ocean,
        );
        let mut ocean = if self.marine_overhaul() && !coastal_water {
            super::OceanHabitat::default()
        } else {
            self.ocean_with_biome(x, z, biome)
        };
        if self.highland_overhaul()
            && biome.weights.highland > 0.5
            && water_level.is_some_and(|water| {
                water > super::highlands::SNOWLINE
                    && (water - self.config().sea_level).abs() > 0.01
                    && water - bed > 1.5
            })
            && river.is_none_or(|r| r.cavity().is_none())
            && self.hills.get([x / 22.0 + 17.5, z / 28.0 - 9.2]) > -0.12
        {
            ocean.ice_depth = 1;
        }
        let neighbors = [
            neighbor_height(1, 0),
            neighbor_height(-1, 0),
            neighbor_height(0, 1),
            neighbor_height(0, -1),
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
        // A river under a land bridge can have water below a dry, snowy roof.
        // Compare with the actual top voxel before choosing surface cover.
        let submerged_top = water_level.is_some_and(|water| (height - 0.5).floor() + 1.0 < water);
        let [mut snow_cover, shore_cover] =
            self.surface_cover(biome, height, level, oasis, water_level.is_none());
        if submerged_top {
            snow_cover = 0.0;
        }
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
            [[-4, 0], [4, 0], [0, -4], [0, 4]]
                .into_iter()
                .map(|[dx, dz]| neighbor_height(dx, dz))
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
        if self.config().spirit_blossom_revision > 0 && biome.weights.spirit_blossom > 0.5 {
            surface_material = if exposure > 0.5 {
                MaterialId::SpiritStone
            } else {
                MaterialId::SpiritBlossomGrass
            };
        }
        if self.config().ecology_revision >= 12
            && biome.weights.mushroom_park > 0.38
            && matches!(surface_material, MaterialId::Grass | MaterialId::Stone)
        {
            let beds = self.valleys.get([x * 0.056 + 13.6, z * 0.056 - 82.1]);
            if exposure > 0.43 {
                surface_material = MaterialId::MossyStone;
            } else if beds > 0.40 {
                surface_material = MaterialId::MyceliumTurf;
            } else if beds < -0.35 && (oasis_moisture > 0.18 || slope < 0.65) {
                surface_material = MaterialId::SporeSoil;
            }
        }
        if self.config().biomes
            && self.config().ecology_revision >= 14
            && biome.weights.forest > 0.5
            && water_level.is_none()
            && matches!(surface_material, MaterialId::Grass | MaterialId::Stone)
        {
            // Canopy spacing and ground props already use this same continuous clearance mask.
            let (litter, path) = self.climate.ground_masks(x, z);
            surface_material = if slope > 1.15 && litter > 0.55 {
                MaterialId::MossyStone
            } else if surface_material == MaterialId::Grass && slope < 0.9 && path < 0.16 {
                MaterialId::WoodlandTrail
            } else if surface_material == MaterialId::Grass && litter > 0.73 {
                MaterialId::WoodlandLitter
            } else {
                surface_material
            };
        }
        if self.config().ecology_revision >= 15
            && biome.weights.cherry_forest > 0.48
            && water_level.is_none()
            && matches!(surface_material, MaterialId::Grass | MaterialId::Stone)
        {
            let flowers = self.valleys.get([x / 21.0 - 4.7, z / 24.0 + 19.3]);
            surface_material = if slope > 1.05 || exposure > 0.53 {
                if flowers < -0.03 {
                    MaterialId::BlossomStone
                } else {
                    MaterialId::MossyBlossomStone
                }
            } else if flowers > 0.31 && surface_material == MaterialId::Grass {
                MaterialId::PetalTurf
            } else {
                surface_material
            };
        }
        if self.desert_overhaul()
            && biome.aridity() > 0.52
            && surface_material == MaterialId::Sand
            && water_level.is_none()
        {
            let bed = self.dune_sediment(x, z);
            surface_material = if slope > 1.05 {
                MaterialId::BandedSandstone
            } else if bed > 0.34 && oasis_moisture < 0.18 {
                MaterialId::RedDuneSand
            } else {
                MaterialId::DuneSand
            };
        }
        if self.highland_overhaul() && biome.weights.highland > 0.5 {
            surface_material = if submerged_top {
                if slope > 0.8 {
                    MaterialId::AlpineGranite
                } else {
                    MaterialId::Gravel
                }
            } else {
                self.highland_surface(x, z, height, slope)
            };
        }
        surface_material = self.plains_stone(
            [x, z],
            biome.weights.plains,
            slope,
            river.is_some() && (height - level).abs() < 5.5 && river_cavity.is_none(),
            surface_material,
        );
        if let Some(material) = self.blood_surface_material(
            [x, z],
            biome.weights.blood_forest,
            exposure,
            surface_material,
        ) {
            surface_material = material;
        }
        if self.marine_overhaul()
            && climate.weights.ocean > 0.70
            && height >= level - 1.0
            && height < level + 23.0
            && river.is_none()
            && oasis.is_none()
        {
            surface_material = if height < level + 1.2 {
                MaterialId::WetSand
            } else if slope < 0.85 && biome.temperature < 0.34 {
                MaterialId::Stone
            } else if slope < 0.65 && biome.temperature > 0.40 {
                MaterialId::MossyStone
            } else {
                MaterialId::SeaBasalt
            };
        }
        if let Some(volcanic) = self.volcanic_surface(x, z, volcanic, slope, height) {
            surface_material = volcanic;
            snow_cover = 0.;
        }
        if biome.weights.savanna > 0.48 {
            surface_material = self.savanna_ground(x, z, slope, oasis_moisture, submerged_top);
            snow_cover = 0.;
        }
        if biome.weights.swamp > 0.48 {
            surface_material = self.swamp_ground(x, z, submerged_top, slope);
            snow_cover = 0.;
        }
        if biome.weights.petrified_forest > 0.48 {
            surface_material = self.petrified_ground(x, z, slope);
            snow_cover = 0.;
        }
        if biome.weights.crystal_highlands > 0.42 {
            surface_material = if submerged_top {
                MaterialId::CrystalGravel
            } else {
                self.crystal_ground(x, z, slope, height)
            };
            snow_cover = if matches!(
                surface_material,
                MaterialId::Snow | MaterialId::SnowCoveredSod
            ) {
                1.
            } else {
                0.
            };
        }
        if biome.weights.coastal_land_fraction() > 0.49 {
            surface_material = self.coastal_ground(x, z, height, slope);
            snow_cover = 0.;
        }
        let mut column = SurfaceColumn {
            falls: [None; 3],
            surface_shape: super::advanced::SurfaceShape {
                strength: if self.config().landscape_revision >= 5
                    && water_level.is_none()
                    && volcanic <= 0.65
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
            cave: self.cave_structure_below(x, z, || height),
            seagrass_height: 0,
            ocean,
            water_level,
            lava_level,
            water_flow: oasis.and_then(|o| o.spring_flow()).unwrap_or_else(|| {
                river
                    .filter(|river| river.distance <= river.width + 1.0)
                    .map_or([0.0; 2], |river| river.flow)
            }),
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
        column.seagrass_height = self.seagrass_segments_height(column, [x, z]);
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
        } else if (774..=780).contains(&(column.surface_material as u16)) {
            if depth <= 1.
                || (depth < 2.6
                    && matches!(
                        column.surface_material,
                        MaterialId::BeachShingle | MaterialId::CoarseShoreSand
                    ))
            {
                column.surface_material
            } else if depth < 16. {
                MaterialId::CoastalGranite
            } else {
                MaterialId::Stone
            }
        } else if column.biome.weights.crystal_highlands > 0.42
            && (matches!(column.surface_material as u16, 739..=744)
                || column.surface_material == MaterialId::Snow)
        {
            if depth <= 1.0 {
                column.surface_material
            } else if depth < 39. {
                // Deformed bedding continues behind the cliff face into the mines.
                let bed = ((column.height - depth) + column.height * 0.27).floor() as i64;
                match (column.surface_material, bed.rem_euclid(19)) {
                    (MaterialId::CrystalOre, 3..=7) => MaterialId::CrystalOre,
                    (MaterialId::AuriciteOre, 8..=10) => MaterialId::AuriciteOre,
                    (_, 1..=2) => MaterialId::CrystalStone,
                    _ => MaterialId::HighlandCliffStone,
                }
            } else {
                MaterialId::Stone
            }
        } else if matches!(
            column.surface_material,
            MaterialId::DustyGround
                | MaterialId::CrackedStoneSoil
                | MaterialId::MineralEncrustedStone
                | MaterialId::FossiliferousStone
                | MaterialId::TurquoiseVeinedStone
                | MaterialId::RootBoundStone
        ) {
            if depth <= 1. {
                column.surface_material
            } else if self.config().ecology_revision >= 20
                && depth < column.soil_depth + 8.
                && (column.height - depth + column.height * 0.16)
                    .floor()
                    .rem_euclid(13.)
                    < 1.6
            {
                MaterialId::FossiliferousStone
            } else if depth <= column.soil_depth + 9. {
                MaterialId::CrackedStoneSoil
            } else {
                MaterialId::Stone
            }
        } else if matches!(column.surface_material as u16, 670..=677) {
            if column.height - depth >= (column.height - 0.5).floor() {
                column.surface_material
            } else if depth < column.soil_depth {
                MaterialId::Peat
            } else if depth < column.soil_depth + 7. {
                MaterialId::SwampClay
            } else {
                MaterialId::SwampStone
            }
        } else if matches!(column.surface_material as u16, 632..=635 | 638..=642) {
            if column.height - depth >= (column.height - 0.5).floor() {
                column.surface_material
            } else if depth < column.soil_depth
                && matches!(column.surface_material as u16, 632..=635)
            {
                MaterialId::DrySavannaSoil
            } else if depth < column.soil_depth + 8. {
                MaterialId::LayeredSavannaRock
            } else {
                // Horizontal beds cut across both escarpments and deep gullies.
                let bed = ((column.height - depth) / 3.0).floor() as i64;
                match bed.rem_euclid(17) {
                    3 => MaterialId::MeadowLimestone,
                    7..=8 => MaterialId::RedSandstone,
                    13 => MaterialId::PaleBasalt,
                    _ => MaterialId::SavannaRock,
                }
            }
        } else if matches!(column.surface_material as u16, 614..=620 | 623 | 730) {
            if depth < 1.5 {
                column.surface_material
            } else if depth < 3.8
                && matches!(
                    column.surface_material,
                    MaterialId::Ash | MaterialId::ScorchedDirt | MaterialId::VolcanicGravel
                )
            {
                MaterialId::CompactedAsh
            } else {
                MaterialId::DarkVolcanicStone
            }
        } else if self.config().blood_forest_revision > 0
            && matches!(
                column.surface_material,
                MaterialId::BloodDirt
                    | MaterialId::BlightedRootSoil
                    | MaterialId::CoagulatedMud
                    | MaterialId::BloodGrass
                    | MaterialId::BloodMoss
                    | MaterialId::CrimsonMycelium
                    | MaterialId::BloodStone
            )
        {
            if depth > column.soil_depth || column.surface_material == MaterialId::BloodStone {
                MaterialId::BloodStone
            } else if column.height - depth >= (column.height - 0.5).floor() {
                column.surface_material
            } else {
                MaterialId::BloodDirt
            }
        } else if self.config().spirit_blossom_revision > 0
            && column.biome.weights.spirit_blossom > 0.5
        {
            if depth > column.soil_depth || column.surface_material == MaterialId::SpiritStone {
                MaterialId::SpiritStone
            } else if column.height - depth >= (column.height - 0.5).floor() {
                MaterialId::SpiritBlossomGrass
            } else {
                MaterialId::SpiritBlossomDirt
            }
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
        } else if matches!(
            column.surface_material,
            MaterialId::DuneSand | MaterialId::RedDuneSand | MaterialId::BandedSandstone
        ) {
            if depth < 1.5 {
                column.surface_material
            } else if depth < column.sandstone_depth + 10.0 {
                MaterialId::BandedSandstone
            } else {
                MaterialId::Stone
            }
        } else if matches!(
            column.surface_material,
            MaterialId::Firn
                | MaterialId::IcySnow
                | MaterialId::SnowCoveredSod
                | MaterialId::AlpineGranite
        ) {
            if depth < 1.5 {
                column.surface_material
            } else if depth < 3.0 && column.surface_material == MaterialId::SnowCoveredSod {
                MaterialId::Dirt
            } else if depth < 8.0 {
                MaterialId::AlpineGranite
            } else {
                MaterialId::Stone
            }
        } else if matches!(
            column.surface_material,
            MaterialId::SeaBasalt | MaterialId::WetSand
        ) && depth < 8.
        {
            if depth < 1.6 {
                column.surface_material
            } else {
                MaterialId::SeaBasalt
            }
        } else if column.surface_material == MaterialId::MeadowLimestone && depth < 3.75 {
            MaterialId::MeadowLimestone
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
        } else if matches!(
            column.surface_material,
            MaterialId::Grass
                | MaterialId::WoodlandTrail
                | MaterialId::WoodlandLitter
                | MaterialId::PetalTurf
        ) {
            let top_voxel = (column.height - 0.5).floor();
            if column.height - depth >= top_voxel {
                column.surface_material
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
