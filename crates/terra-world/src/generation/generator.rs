use super::{SurfaceColumn, TerrainGenerator, biomes::ClimateSampler, seed::layer_seed};
use noise::{NoiseFn, Perlin};
use terra_core::{Config, ConfigError, GENERATOR_VERSION, WorldConfig};
use terra_voxel::VoxelSample;

#[derive(Clone)]
pub struct TerraGenerator {
    pub(super) advanced: std::sync::Arc<super::advanced::AdvancedSampler>,
    pub(super) biome_regions: std::sync::Arc<super::biomes::RegionCache>,
    pub(super) rivers: std::sync::Arc<super::rivers::RiverCache>,
    pub(super) cave_portals: std::sync::Arc<super::caves::CaveCache>,
    pub(super) cave_biomes: std::sync::Arc<super::cave_biomes::CaveBiomeCache>,
    pub(super) connector_water: std::sync::Arc<super::connector_water::ConnectorWaterCache>,
    pub(super) oases: std::sync::Arc<super::oases::OasisCache>,
    pub(super) lakes: std::sync::Arc<super::lakes::LakeCache>,
    pub(super) landscape: super::landscape::LandscapeSampler,
    pub(super) planted_appearances: std::sync::Arc<super::vegetation::PlantedAppearanceSource>,
    pub(super) tree_appearances: std::sync::Arc<super::vegetation::TreeAppearanceCache>,
    pub(super) structures: std::sync::Arc<super::structures::StructureCache>,
    pub(super) natural_features: std::sync::Arc<super::natural_features::NaturalCache>,
    config: WorldConfig,
    pub(super) hills: Perlin,
    pub(super) mountains: Perlin,
    pub(super) valleys: Perlin,
    pub(in crate::generation) climate: ClimateSampler,
    caves_a: Perlin,
    caves_b: Perlin,
}
impl TerraGenerator {
    pub fn new(config: WorldConfig) -> Result<Self, ConfigError> {
        config.validate()?;
        let seed = config.seed.0;
        let climate = ClimateSampler::new(seed, config.biome_scale, config.ocean_scale)
            .larger_regions(config.landscape_revision >= 8);
        Ok(Self {
            advanced: std::sync::Arc::new(super::advanced::AdvancedSampler::new(&config)),
            biome_regions: Default::default(),
            rivers: Default::default(),
            cave_portals: Default::default(),
            cave_biomes: Default::default(),
            connector_water: Default::default(),
            oases: Default::default(),
            lakes: Default::default(),
            landscape: super::landscape::LandscapeSampler::new(seed),
            tree_appearances: Default::default(),
            planted_appearances: Default::default(),
            structures: Default::default(),
            natural_features: Default::default(),
            config,
            hills: Perlin::new(layer_seed(seed, 1)),
            mountains: Perlin::new(layer_seed(seed, 2)),
            valleys: Perlin::new(layer_seed(seed, 3)),
            caves_a: Perlin::new(layer_seed(seed, 4)),
            caves_b: Perlin::new(layer_seed(seed, 5)),
            climate,
        })
    }
    pub fn config(&self) -> &WorldConfig {
        &self.config
    }
    /// Calendar policy changes retain every seed/noise/cache and terrain contract.
    pub fn set_world_rules(&mut self, rules: terra_core::WorldRuleSettings) {
        rules.apply(&mut self.config);
    }
    pub(crate) fn sample_with_column(
        &self,
        position: [f64; 3],
        column: SurfaceColumn,
    ) -> VoxelSample {
        let [_, y, _] = position;
        if self.config.bounded_world {
            if !(-128.0..357.0).contains(&y) {
                return VoxelSample::new(1.0, terra_voxel::MaterialId::Air);
            }
            if y < -126.0 {
                return VoxelSample::new(-1.0, terra_voxel::MaterialId::Bedrock);
            }
        }
        let height = column.height;
        // The exposed bed and banks of a desert ravine collect surface sand.
        // A salt hall passing underneath must not colour that open-air ground.
        let depth = if self.config.ravine_revision >= 2
            && column.surface_material == terra_voxel::MaterialId::Sand
            && let Some([floor, ceiling]) = column.cave_cavity
            && ceiling >= height + 2.0
            && y >= floor - 2.0
        {
            floor - y
        } else {
            height - y
        };
        let (mut density, formation) = self.cave_density_with_column(position, column);
        let mut material = self.column_material(column, depth, density);
        // Spirit terrain can move portals into cave-ecology decoration sites,
        // including legacy landscape profiles without connected hydrology.
        // Keep their walking volume clear while retaining old saved-world output.
        let reserved_air = (self.connected_hydrology() || self.config.spirit_blossom_revision > 0)
            && column
                .river_cavity
                .or(column.cave_cavity)
                .is_some_and(|[floor, ceiling]| y > floor && y < ceiling);
        if !reserved_air {
            if self.config.cave_structure_revision >= 3 {
                // A shallow chamber ceiling is clipped by the terrain roof. Its
                // uncut geometric outline must not paint the ground above it.
                if self.config.caves
                    && (depth > 5.0 || depth >= 1.0 && column.cave.salt_window_wall(y))
                    && let Some(decoration) = self.cave_ecology_material(position, column, density)
                {
                    material = decoration;
                    density = density.min(-0.5);
                }
            } else {
                self.legacy_cave_material(
                    position,
                    column,
                    depth,
                    formation,
                    (&mut density, &mut material),
                );
            }
        }
        let ground = (column.height - 0.5).floor();
        if density > 0.0
            && y.floor() > ground
            && y.floor() <= ground + f64::from(column.seagrass_height)
        {
            material = terra_voxel::MaterialId::Seagrass;
            density = -0.5;
        }
        if density > 0.0
            && let Some(decoration) = self
                .blood_decoration(position, column)
                .or_else(|| self.spirit_decoration(position, column))
        {
            material = decoration;
            density = -0.5;
        }
        VoxelSample::new(density as f32, material)
    }

    #[inline]
    pub(super) fn cave_density_with_column(
        &self,
        [x, y, z]: [f64; 3],
        column: SurfaceColumn,
    ) -> (f64, bool) {
        let height = column.height;
        let depth = height - y;
        // Implicit signed density field, not an exact Euclidean distance after CSG/noise.
        let mut density = y - height;
        if self.config.landscape_revision >= 5 {
            density = self.advanced.density([x, y, z], column, density);
        }
        if let Some([floor, ceiling]) = column.river_cavity {
            density = density.max((y - floor).min(ceiling - y));
        }
        if let Some([floor, ceiling]) = column.cave_cavity {
            density = density.max((y - floor).min(ceiling - y));
        }
        // Before half the roof depth, the clipped cave field cannot exceed -depth.
        // Starting at that intersection keeps the CSG density continuous.
        const ROOF_DEPTH: f64 = 5.0;
        if self.config.caves && depth > ROOF_DEPTH * 0.5 {
            let mut shape = column.cave_shape;
            if self.config.cave_structure_revision >= 3
                && self.config.cave_biomes.connectors.inherit_biomes
            {
                let w = column.cave_biome.weights_at(y);
                shape.tunnel_width *= f64::from(1.0 - w[1] * 0.10 + w[2] * 0.15 + w[3] * 0.08);
            }
            let tunnel = self.tunnel_density_with_shape([x, y, z], &shape);
            // The river arch already has an explicit supported roof. A second
            // independent cavity above it could cut off its underside as islands.
            // Caves below the bed and outside that corridor retain their field.
            let tunnel = column
                .river_cavity
                .or(column.cave_cavity)
                .map_or(tunnel, |cavity| tunnel.min(cavity[1] - y));
            let tunnel = column
                .river_cavity
                .or(column.cave_cavity)
                .map_or(tunnel, |cavity| {
                    // A continuous two-metre floor band protects river beds and
                    // walkable descents while deeper cave chambers remain connected.
                    tunnel.min((y - cavity[0]).max(cavity[0] - 2.0 - y))
                });
            // A roof margin prevents caves from cutting through the exterior heightfield.
            density = density.max(tunnel.min(depth - ROOF_DEPTH));
        }
        density = column.cave.carve(y, density, height);
        let formation_density =
            if self.config.geology_revision >= 2 && self.config.caves && depth > 6.0 {
                column.cave_shape.formation_density(y)
            } else {
                f64::INFINITY
            };
        let formation = density > 0.0 && formation_density <= 0.0;
        density = density.min(formation_density);
        if self.connected_hydrology()
            && let Some([floor, ceiling]) = column.river_cavity.or(column.cave_cavity)
        {
            // Chambers and mineral formations run after connector carving.
            // Reserve its air and support bands after that CSG so a chamber
            // wall cannot close a passage or remove its two-metre solid floor.
            let roof_band = (ceiling - y).max(y - ceiling - 3.0);
            let clipped = density.min(roof_band).min((y - floor).max(floor - 2.0 - y));
            let corridor = (y - floor).min(ceiling - y);
            density = (y - height).max(clipped).max(corridor);
        }
        if self.config.landscape_revision >= 4
            && column.biome.weights.ocean > 0.45
            && column.water_level.is_some_and(|level| {
                (level - self.config.sea_level).abs() < 0.01 && level - height > 3.0
            })
        {
            // Apply after every chamber/connector CSG, otherwise a later carve can
            // reopen the seabed. Deeper caves remain intact below this solid roof.
            density = (y - height).max(density.min(depth - 6.0));
        }
        if self.config.bounded_world && y < -120.0 {
            density = density.min(-0.5);
        }
        (density, formation)
    }

    fn legacy_cave_material(
        &self,
        position: [f64; 3],
        column: SurfaceColumn,
        depth: f64,
        formation: bool,
        (density_out, material_out): (&mut f64, &mut terra_voxel::MaterialId),
    ) {
        let [x, y, z] = position;
        let mut density = *density_out;
        let mut material = *material_out;
        if self.config.geology_revision > 0
            && self.config.caves
            && (material == terra_voxel::MaterialId::Stone || formation)
            && depth > 5.0
            && (density > -1.35 || formation)
            && density <= 0.0
            && column.cave_shape.kind == super::CaveKind::Dripstone
            && (column.cave_shape.chamber_density(y) > -2.0 || formation)
        {
            material = if column.cave_shape.salt
                && self.config.ores.enabled
                && self.config.ores.salt.frequency > 0.0
                && (self.config.ores.salt.min_depth..=self.config.ores.salt.max_depth)
                    .contains(&depth)
            {
                if formation {
                    terra_voxel::MaterialId::SaltFormation
                } else {
                    terra_voxel::MaterialId::SaltOre
                }
            } else if formation {
                terra_voxel::MaterialId::DripstoneFormation
            } else {
                terra_voxel::MaterialId::Dripstone
            };
        }
        // Low-frequency 3D patches, not independent per-voxel random material swaps.
        if self.config.geology_revision >= 4
            && self.config.caves
            && depth > 6.0
            && (-2.0..=0.0).contains(&density)
            && matches!(
                material,
                terra_voxel::MaterialId::Stone | terra_voxel::MaterialId::Dripstone
            )
            && self
                .valleys
                .get([x * 0.065 + 137.0, y * 0.09, z * 0.065 - 81.0])
                > 0.32
        {
            material = terra_voxel::MaterialId::Dirt;
        }
        if let Some(decoration) = column.cave.material(position, density) {
            material = decoration;
            density = density.min(-0.5);
        }
        if self.config.cave_structure_revision >= 2
            && self.config.caves
            && density > 0.0
            && depth > 8.0
            && y > -115.0
            && !column.cave.active()
            && column.river_cavity.is_none()
            && column.cave_cavity.is_none()
            && column.water_level.is_none_or(|water| y > water)
        {
            // Only selected columns do extra probes. Test the original field,
            // not edited blocks, so clearing a tunnel never spawns new plants.
            let ticket = layer_seed(
                self.config.seed.0
                    ^ (x.floor() as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
                    ^ (z.floor() as i64 as u64).wrapping_mul(0xbf58476d1ce4e5b9),
                285,
            );
            let field = |dy| self.tunnel_density_with_shape([x, y + dy, z], &column.cave_shape);
            let decoration = if ticket.is_multiple_of(97) && field(-1.0) <= 0.0 {
                Some(terra_voxel::MaterialId::CaveMushroom)
            } else if ticket.is_multiple_of(71)
                && depth > 12.0
                && (field(1.0) <= 0.0 || (field(1.0) > 0.0 && field(2.0) <= 0.0))
            {
                Some(terra_voxel::MaterialId::CaveRoot)
            } else {
                None
            };
            if let Some(decoration) = decoration {
                material = decoration;
                density = -0.5;
            }
        }
        *density_out = density;
        *material_out = material;
    }

    pub(super) fn tunnel_density(&self, [x, y, z]: [f64; 3]) -> f64 {
        self.tunnel_density_with_shape([x, y, z], &self.cave_shape(x, z))
    }

    pub(super) fn tunnel_density_with_shape(
        &self,
        [x, y, z]: [f64; 3],
        shape: &super::caves::CaveShape,
    ) -> f64 {
        let a = self
            .caves_a
            .get([x * 0.035 + 0.13, y * 0.045 + 0.41, z * 0.035 + 0.67]);
        let b = self
            .caves_b
            .get([x * 0.037 + 0.79, y * 0.040 + 0.23, z * 0.037 + 0.59]);
        let a = a * (1.0 + shape.fissure * 1.2);
        let b = b * (1.0 - shape.fissure * 0.50);
        let relief = if self.config.geology_revision == 0 {
            1.4
        } else {
            match shape.kind {
                super::CaveKind::Cheese => 5.0,
                super::CaveKind::Dripstone => 3.0,
                super::CaveKind::Spaghetti => 1.4,
            }
        };
        ((shape.tunnel_width - (a * a + b * b).sqrt()) * 20.0)
            .min(shape.connector_limit)
            .max(shape.chamber_density(y) + (a + b) * relief)
    }
}

impl TerrainGenerator for TerraGenerator {
    fn version(&self) -> u32 {
        GENERATOR_VERSION
    }
    fn sample(&self, position: [f64; 3]) -> VoxelSample {
        if position.iter().all(|v| v.is_finite())
            && let Some(material) = self.structure_block(position.map(|v| v.floor() as i64))
        {
            return VoxelSample {
                density: if material == 0 { 1.0 } else { -1.0 },
                material,
            };
        }
        if position.iter().all(|v| v.is_finite())
            && let Some(material) = self.natural_material_at(position.map(|v| v.floor() as i64))
        {
            return VoxelSample {
                density: -1.0,
                material,
            };
        }
        let column = self.surface_column(position[0], position[2]);
        let mut sample = self.sample_with_column(position, column);
        if self.ore_host(sample.material)
            && sample.density <= 0.0
            && self.config.ores.enabled
            && let Some(ore) = self.ore_at(position.map(|value| value.floor() as i64), column)
        {
            sample.material = ore.material();
        }
        sample
    }
}
