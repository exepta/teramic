//! Reusable north-up surface tiles. One pixel per metre, no terrain resampling.
use crate::{PreparedColumn, SurfaceColumn};

#[cfg(feature = "bevy")]
mod live;
#[cfg(feature = "bevy")]
pub use live::{DetailTileIndex, DetailTileSnapshot, detail_revision, detail_revisions};

pub const TILE_SIZE: usize = 16;
pub const TILE_BYTES: usize = TILE_SIZE * TILE_SIZE * 4;

/// Surface relief and a walkable horizontal cave slice use independent caches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MapLayer {
    #[default]
    Surface,
    Cave {
        level: i16,
    },
}

/// A real loaded cell, before the presentation layer applies texture albedos.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapSample {
    pub material: u16,
    /// Upper face of the mapped block or water column, in world metres.
    pub height: i16,
    pub water_depth: u8,
    pub known: bool,
    pub canopy: bool,
    pub biome_weights: [f32; 8],
    pub shore: f32,
    pub snow: f32,
}

impl Default for MapSample {
    fn default() -> Self {
        Self {
            material: 0,
            height: 0,
            water_depth: 0,
            known: false,
            canopy: false,
            biome_weights: [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            shore: 0.0,
            snow: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DetailTile {
    pub coordinate: [i64; 2],
    pub layer: MapLayer,
    pub revision: u64,
    /// North-up, x + z * TILE_SIZE. Unknown space must remain undiscovered.
    pub cells: [MapSample; TILE_SIZE * TILE_SIZE],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapTile {
    pub coordinate: [i64; 2],
    pub pixels: [u8; TILE_BYTES],
    pub relief: Option<Box<MapRelief>>,
}

/// Compact exploration geometry survives eviction of expensive live samples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapRelief {
    pub heights: [i16; TILE_SIZE * TILE_SIZE],
    pub water: [u8; TILE_SIZE * TILE_SIZE / 8],
}

impl MapRelief {
    pub const UNKNOWN: i16 = i16::MIN;

    pub fn height(&self, index: usize) -> Option<i16> {
        self.heights
            .get(index)
            .copied()
            .filter(|v| *v != Self::UNKNOWN)
    }

    pub fn is_water(&self, index: usize) -> bool {
        self.water
            .get(index / 8)
            .is_some_and(|v| v & (1 << (index % 8)) != 0)
    }
}

impl MapTile {
    pub fn from_column(coordinate: [i64; 2], column: &PreparedColumn) -> Self {
        let mut tile = Self {
            coordinate,
            pixels: [0; TILE_BYTES],
            relief: Some(Box::new(MapRelief {
                heights: [MapRelief::UNKNOWN; TILE_SIZE * TILE_SIZE],
                water: [0; TILE_SIZE * TILE_SIZE / 8],
            })),
        };
        for z in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                let wx = coordinate[0] * 16 + x as i64;
                let wz = coordinate[1] * 16 + z as i64;
                if let Some(surface) = column.surface_at(wx, wz) {
                    let pixel = color(surface);
                    let offset = (z * TILE_SIZE + x) * 4;
                    tile.pixels[offset..offset + 4].copy_from_slice(&pixel);
                    if let Some(relief) = tile.relief.as_mut() {
                        let index = z * TILE_SIZE + x;
                        let water = surface.water_level.filter(|level| *level > surface.height);
                        relief.heights[index] = water.unwrap_or(surface.height).ceil() as i16;
                        if water.is_some() {
                            relief.water[index / 8] |= 1 << (index % 8);
                        }
                    }
                }
            }
        }
        tile
    }
}

fn color(surface: &SurfaceColumn) -> [u8; 4] {
    let weights = surface.biome.weights.values();
    let palette = [
        [114.0, 155.0, 116.0],
        [123.0, 162.0, 83.0],
        [67.0, 116.0, 73.0],
        [214.0, 189.0, 128.0],
        [133.0, 145.0, 133.0],
        [159.0, 119.0, 167.0],
        [183.0, 151.0, 104.0],
        [145.0, 207.0, 99.0],
    ];
    let mut rgb: [f64; 3] =
        std::array::from_fn(|i| weights.iter().zip(palette).map(|(w, c)| w * c[i]).sum());
    if surface
        .water_level
        .is_some_and(|level| level > surface.height)
    {
        let depth =
            (surface.water_level.unwrap_or(surface.height) - surface.height).clamp(0.0, 20.0);
        rgb = [56.0, 139.0, 172.0].map(|v| v * (1.0 - depth * 0.012));
        let mushroom = surface.biome.weights.mushroom_park;
        for i in 0..3 {
            rgb[i] += ([157.0, 137.0, 192.0][i] - rgb[i]) * mushroom;
        }
        for i in 0..3 {
            rgb[i] +=
                ([151.0, 118.0, 145.0][i] - rgb[i]) * surface.biome.weights.cherry_forest * 0.35;
        }
    } else {
        let snow = if surface.biome.aridity() <= 0.5 {
            surface.snow_cover
        } else {
            0.0
        };
        let relief =
            (0.93 + surface.height * 0.0008 - surface.slope.min(4.0) * 0.035).clamp(0.68, 1.12);
        for (v, sand) in rgb.iter_mut().zip([214.0, 189.0, 128.0]) {
            *v += (sand - *v) * surface.shore_cover;
            *v = (*v + (229.0 - *v) * snow) * relief;
        }
    }
    [rgb[0] as u8, rgb[1] as u8, rgb[2] as u8, 255]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_columns_produce_complete_stable_tiles_without_neighbor_seams() {
        let generator = crate::SectionGenerator::new(
            crate::TerraGenerator::new(terra_core::WorldConfig::default()).unwrap(),
        );
        let column = generator.prepare_column([-1, -1], true).unwrap();
        let a = MapTile::from_column([-1, -1], &column);
        assert_eq!(a, MapTile::from_column([-1, -1], &column));
        assert!(a.pixels.chunks_exact(4).all(|p| p[3] == 255));
        assert_eq!(&a.pixels[..4], &color(column.surface_at(-16, -16).unwrap()));
        assert_eq!(
            &a.pixels[TILE_BYTES - 4..],
            &color(column.surface_at(-1, -1).unwrap())
        );
        let mut shore = *column.surface_at(-1, -1).unwrap();
        shore.water_level = None;
        shore.snow_cover = 0.0;
        shore.shore_cover = 1.0;
        let sand = color(&shore);
        assert!(sand[0] > sand[1] && sand[1] > sand[2]);
        shore.biome.weights.desert = 1.0;
        shore.snow_cover = 1.0;
        assert_eq!(color(&shore), sand, "dry desert shores do not become snow");
    }

    #[test]
    fn cherry_exploration_tiles_keep_grass_and_subtle_pink_water_colors() {
        let generator = crate::SectionGenerator::new(
            crate::TerraGenerator::new(terra_core::WorldConfig::default()).unwrap(),
        );
        let column = generator.prepare_column([-1, -1], true).unwrap();
        let mut surface = *column.surface_at(-1, -1).unwrap();
        surface.biome.weights = crate::generation::biomes::BiomeWeights {
            ocean: 0.0,
            plains: 0.0,
            forest: 0.0,
            desert: 0.0,
            highland: 0.0,
            mushroom_park: 0.0,
            cliffted_desert: 0.0,
            cherry_forest: 1.0,
        };
        surface.shore_cover = 0.0;
        surface.snow_cover = 0.0;
        surface.water_level = None;
        let grass = color(&surface);
        assert!(grass[1] > 100 && grass[1] > grass[0] && grass[0] > grass[2]);
        surface.water_level = Some(surface.height + 2.0);
        let pink = color(&surface);
        surface.biome.weights.cherry_forest = 0.0;
        surface.biome.weights.forest = 1.0;
        let blue = color(&surface);
        assert!(
            pink[0] > blue[0] && pink[2] > pink[0],
            "subtle tint, not opaque pink"
        );
        assert_eq!(pink[3], 255);
    }
}
