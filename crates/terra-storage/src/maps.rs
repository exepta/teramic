//! Client exploration data, never authoritative world edits.
//!
//! TRMAP001/002 contain only RGBA and optional height/water data. Their explored
//! pixels cannot be recolored reliably and must not be invalidated on a palette
//! change. They retain their original colors until those cells are streamed again.
//! TRMAP003 additionally stores exact per-cell raster inputs and a stable palette
//! fingerprint. Unknown-source legacy pixels can coexist with recolorable cells;
//! the fingerprint describes only cells whose source is known, not the whole tile.
//! TRMAP004 adds Spirit Blossom; TRMAP005 adds Blood Forest; TRMAP006 adds Ashlands; TRMAP007 adds Savanna; TRMAP008 adds Swamp. TRMAP009 adds Petrified Forest. TRMAP010 adds Crystal Highlands. TRMAP011 adds Misty Coast. Older sources retain
//! their original stride and receive zero weights only for the missing biomes.
//! The writer canonicalizes only f64 endpoint roundoff; stored fractions stay [0, 1].
//! No map migration modifies the independent waypoint book or exploration mask.
#[cfg(test)]
mod blood_migration_tests;
pub mod domains;
#[cfg(test)]
mod spirit_migration_tests;

use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use terra_world::cartography::{MapRelief, MapSample, MapTile, TILE_BYTES, TILE_SIZE};

const MAGIC: &[u8; 8] = b"TRMAP001";
const FILE_BYTES: usize = 8 + 16 + TILE_BYTES + 8;
const RELIEF_MAGIC: &[u8; 8] = b"TRMAP002";
const RELIEF_BYTES: usize = TILE_SIZE * TILE_SIZE * 2 + TILE_SIZE * TILE_SIZE / 8;
const LEGACY_SOURCE_MAGIC: &[u8; 8] = b"TRMAP003";
const SPIRIT_SOURCE_MAGIC: &[u8; 8] = b"TRMAP004";
const BLOOD_SOURCE_MAGIC: &[u8; 8] = b"TRMAP005";
const ASH_SOURCE_MAGIC: &[u8; 8] = b"TRMAP006";
const SAVANNA_SOURCE_MAGIC: &[u8; 8] = b"TRMAP007";
const SWAMP_SOURCE_MAGIC: &[u8; 8] = b"TRMAP008";
const PETRIFIED_SOURCE_MAGIC: &[u8; 8] = b"TRMAP009";
const CRYSTAL_SOURCE_MAGIC: &[u8; 8] = b"TRMAP010";
const SOURCE_MAGIC: &[u8; 8] = b"TRMAP011";
const LEGACY_SAMPLE_BYTES: usize = 2 + 2 + 1 + 1 + 8 * 4 + 4 + 4;
const SPIRIT_SAMPLE_BYTES: usize = LEGACY_SAMPLE_BYTES + 4;
const BLOOD_SAMPLE_BYTES: usize = SPIRIT_SAMPLE_BYTES + 4;
const ASH_SAMPLE_BYTES: usize = BLOOD_SAMPLE_BYTES + 4;
const SAVANNA_SAMPLE_BYTES: usize = ASH_SAMPLE_BYTES + 4;
const SWAMP_SAMPLE_BYTES: usize = SAVANNA_SAMPLE_BYTES + 4;
const PETRIFIED_SAMPLE_BYTES: usize = SWAMP_SAMPLE_BYTES + 4;
const CRYSTAL_SAMPLE_BYTES: usize = PETRIFIED_SAMPLE_BYTES + 4;
const SAMPLE_BYTES: usize = CRYSTAL_SAMPLE_BYTES + 4;
const LEGACY_SOURCE_BYTES: usize =
    FILE_BYTES + 1 + 8 + RELIEF_BYTES + TILE_SIZE * TILE_SIZE * LEGACY_SAMPLE_BYTES;
const SPIRIT_SOURCE_BYTES: usize =
    FILE_BYTES + 1 + 8 + RELIEF_BYTES + TILE_SIZE * TILE_SIZE * SPIRIT_SAMPLE_BYTES;
const BLOOD_SOURCE_BYTES: usize =
    FILE_BYTES + 1 + 8 + RELIEF_BYTES + TILE_SIZE * TILE_SIZE * BLOOD_SAMPLE_BYTES;
const ASH_SOURCE_BYTES: usize =
    FILE_BYTES + 1 + 8 + RELIEF_BYTES + TILE_SIZE * TILE_SIZE * ASH_SAMPLE_BYTES;
const SAVANNA_SOURCE_BYTES: usize =
    FILE_BYTES + 1 + 8 + RELIEF_BYTES + TILE_SIZE * TILE_SIZE * SAVANNA_SAMPLE_BYTES;
const SWAMP_SOURCE_BYTES: usize =
    FILE_BYTES + 1 + 8 + RELIEF_BYTES + TILE_SIZE * TILE_SIZE * SWAMP_SAMPLE_BYTES;
const PETRIFIED_SOURCE_BYTES: usize =
    FILE_BYTES + 1 + 8 + RELIEF_BYTES + TILE_SIZE * TILE_SIZE * PETRIFIED_SAMPLE_BYTES;
const CRYSTAL_SOURCE_BYTES: usize =
    FILE_BYTES + 1 + 8 + RELIEF_BYTES + TILE_SIZE * TILE_SIZE * CRYSTAL_SAMPLE_BYTES;
const SOURCE_BYTES: usize =
    FILE_BYTES + 1 + 8 + RELIEF_BYTES + TILE_SIZE * TILE_SIZE * SAMPLE_BYTES;
// Legacy climate mixes and redistributes fractions in f64 before storing f32.
// Subtraction at saturated endpoints can leave a few f64 ulps below zero.
const FRACTION_ROUNDOFF: f64 = 64.0 * f64::EPSILON;

/// Color-only legacy pixels remain explored even when their albedo is unknown.
/// New samples let the presentation layer recolor known cells without loading
/// or regenerating terrain. A source's palette identifies only its known cells.
#[derive(Clone, Debug, PartialEq)]
pub struct CachedMapTile {
    pub tile: MapTile,
    pub source: Option<Box<MapRasterSource>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapRasterSource {
    pub palette: u64,
    pub cells: [MapSample; TILE_SIZE * TILE_SIZE],
}

impl From<MapTile> for CachedMapTile {
    fn from(tile: MapTile) -> Self {
        Self { tile, source: None }
    }
}

impl CachedMapTile {
    /// False for legacy RGBA cells, including partially refreshed legacy tiles.
    pub fn fully_recolorable(&self) -> bool {
        self.source.as_ref().is_some_and(|source| {
            self.tile
                .pixels
                .chunks_exact(4)
                .zip(source.cells.iter())
                .all(|(pixel, sample)| pixel[3] == 0 || sample.known)
        })
    }
}

/// Discover explored files without issuing IO for every unknown cell in a wide map.
pub fn coordinates(root: &Path) -> Result<Vec<[i64; 2]>, String> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    let mut coordinates = Vec::new();
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().is_none_or(|extension| extension != "map") {
            continue;
        }
        let Some((x, z)) = path
            .file_stem()
            .and_then(|name| name.to_str())
            .and_then(|name| name.split_once('_'))
        else {
            continue;
        };
        if let (Ok(x), Ok(z)) = (x.parse::<i64>(), z.parse::<i64>()) {
            if x.unsigned_abs() > 1_000_000_000 || z.unsigned_abs() > 1_000_000_000 {
                continue;
            }
            coordinates.push([x, z]);
            if coordinates.len() > 65536 {
                return Err("Exploration index exceeds 65536 tiles".into());
            }
        }
    }
    coordinates.sort_unstable();
    Ok(coordinates)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapIdentity {
    pub world: terra_core::WorldId,
    pub seed: terra_core::WorldSeed,
    pub generator: u32,
    pub player: terra_core::PlayerUuid,
}

impl MapIdentity {
    pub fn directory(&self, root: &Path) -> PathBuf {
        root.join(format!(
            "{}-{}-v{}",
            self.world.0, self.seed.0, self.generator
        ))
        .join("dimensions")
        .join("overworld")
        .join(self.player.to_string())
    }
}

fn path(root: &Path, coordinate: [i64; 2]) -> PathBuf {
    root.join(format!("{}_{}.map", coordinate[0], coordinate[1]))
}
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}
pub fn load(root: &Path, coordinate: [i64; 2]) -> Result<Option<MapTile>, String> {
    load_cached(root, coordinate).map(|cached| cached.map(|cached| cached.tile))
}

pub fn load_cached(root: &Path, coordinate: [i64; 2]) -> Result<Option<CachedMapTile>, String> {
    let file = match fs::File::open(path(root, coordinate)) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = Vec::with_capacity(SOURCE_BYTES);
    file.take(SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let stored_biomes = if bytes.len() == LEGACY_SOURCE_BYTES && &bytes[..8] == LEGACY_SOURCE_MAGIC
    {
        Some(8)
    } else if bytes.len() == SPIRIT_SOURCE_BYTES && &bytes[..8] == SPIRIT_SOURCE_MAGIC {
        Some(9)
    } else if bytes.len() == BLOOD_SOURCE_BYTES && &bytes[..8] == BLOOD_SOURCE_MAGIC {
        Some(10)
    } else if bytes.len() == ASH_SOURCE_BYTES && &bytes[..8] == ASH_SOURCE_MAGIC {
        Some(11)
    } else if bytes.len() == SAVANNA_SOURCE_BYTES && &bytes[..8] == SAVANNA_SOURCE_MAGIC {
        Some(12)
    } else if bytes.len() == SWAMP_SOURCE_BYTES && &bytes[..8] == SWAMP_SOURCE_MAGIC {
        Some(13)
    } else if bytes.len() == PETRIFIED_SOURCE_BYTES && &bytes[..8] == PETRIFIED_SOURCE_MAGIC {
        Some(14)
    } else if bytes.len() == CRYSTAL_SOURCE_BYTES && &bytes[..8] == CRYSTAL_SOURCE_MAGIC {
        Some(15)
    } else if bytes.len() == SOURCE_BYTES && &bytes[..8] == SOURCE_MAGIC {
        Some(16)
    } else {
        None
    };
    let has_source = stored_biomes.is_some();
    let legacy_relief = bytes.len() == FILE_BYTES + RELIEF_BYTES && &bytes[..8] == RELIEF_MAGIC;
    let legacy_color = bytes.len() == FILE_BYTES && &bytes[..8] == MAGIC;
    if !has_source && !legacy_relief && !legacy_color {
        return Err("Invalid map tile header or size".into());
    }
    let integer = |offset| {
        let mut value = [0; 8];
        value.copy_from_slice(&bytes[offset..offset + 8]);
        value
    };
    if [
        i64::from_le_bytes(integer(8)),
        i64::from_le_bytes(integer(16)),
    ] != coordinate
        || u64::from_le_bytes(integer(bytes.len() - 8)) != checksum(&bytes[..bytes.len() - 8])
    {
        return Err("Map tile coordinate or checksum mismatch".into());
    }
    let mut pixels = [0; TILE_BYTES];
    pixels.copy_from_slice(&bytes[24..24 + TILE_BYTES]);
    let metadata = 24 + TILE_BYTES;
    if has_source && bytes[metadata] > 1 {
        return Err("Invalid map relief flags".into());
    }
    let relief_start = metadata + if has_source { 9 } else { 0 };
    let relief = (legacy_relief || has_source && bytes[metadata] == 1).then(|| {
        let mut heights = [MapRelief::UNKNOWN; TILE_SIZE * TILE_SIZE];
        let start = relief_start;
        for (i, height) in heights.iter_mut().enumerate() {
            *height = i16::from_le_bytes([bytes[start + i * 2], bytes[start + i * 2 + 1]]);
        }
        let mut water = [0; TILE_SIZE * TILE_SIZE / 8];
        let start = start + heights.len() * 2;
        let end = start + water.len();
        water.copy_from_slice(&bytes[start..end]);
        Box::new(MapRelief { heights, water })
    });
    let source = if let Some(biomes) = stored_biomes {
        let mut cells = [MapSample::default(); TILE_SIZE * TILE_SIZE];
        let start = relief_start + RELIEF_BYTES;
        let stride = 6 + (biomes + 2) * 4;
        for (index, (sample, bytes)) in cells
            .iter_mut()
            .zip(bytes[start..bytes.len() - 8].chunks_exact(stride))
            .enumerate()
        {
            if bytes[5] > 3 {
                return Err("Invalid map sample flags".into());
            }
            let number = |offset| f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
            let mut fractions = std::array::from_fn(|axis| {
                if axis < 16 {
                    if axis < biomes {
                        number(6 + axis * 4)
                    } else {
                        0.0
                    }
                } else {
                    number(6 + (biomes + axis - 16) * 4)
                }
            });
            check_fractions(&mut fractions, coordinate, index, false)?;
            *sample = MapSample {
                material: u16::from_le_bytes(bytes[..2].try_into().unwrap()),
                height: i16::from_le_bytes(bytes[2..4].try_into().unwrap()),
                water_depth: bytes[4],
                known: bytes[5] & 1 != 0,
                canopy: bytes[5] & 2 != 0,
                biome_weights: std::array::from_fn(|axis| fractions[axis]),
                shore: fractions[16],
                snow: fractions[17],
            };
        }
        Some(Box::new(MapRasterSource {
            palette: u64::from_le_bytes(integer(metadata + 1)),
            cells,
        }))
    } else {
        None
    };
    Ok(Some(CachedMapTile {
        tile: MapTile {
            coordinate,
            pixels,
            relief,
        },
        source,
    }))
}

pub fn save(root: &Path, tile: &MapTile) -> Result<(), String> {
    let mut bytes = Vec::with_capacity(FILE_BYTES);
    bytes.extend_from_slice(if tile.relief.is_some() {
        RELIEF_MAGIC
    } else {
        MAGIC
    });
    for v in tile.coordinate {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes.extend_from_slice(&tile.pixels);
    if let Some(relief) = tile.relief.as_ref() {
        for height in relief.heights {
            bytes.extend_from_slice(&height.to_le_bytes());
        }
        bytes.extend_from_slice(&relief.water);
    }
    write(root, tile.coordinate, bytes)
}

pub fn save_cached(root: &Path, cached: &CachedMapTile) -> Result<(), String> {
    let Some(source) = cached.source.as_ref() else {
        return save(root, &cached.tile);
    };
    let mut bytes = Vec::with_capacity(SOURCE_BYTES);
    bytes.extend_from_slice(SOURCE_MAGIC);
    for value in cached.tile.coordinate {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&cached.tile.pixels);
    bytes.push(u8::from(cached.tile.relief.is_some()));
    bytes.extend_from_slice(&source.palette.to_le_bytes());
    if let Some(relief) = cached.tile.relief.as_ref() {
        for height in relief.heights {
            bytes.extend_from_slice(&height.to_le_bytes());
        }
        bytes.extend_from_slice(&relief.water);
    } else {
        bytes.resize(bytes.len() + RELIEF_BYTES, 0);
    }
    for (index, sample) in source.cells.iter().enumerate() {
        let mut fractions = std::array::from_fn(|axis| match axis {
            0..16 => sample.biome_weights[axis],
            16 => sample.shore,
            _ => sample.snow,
        });
        check_fractions(&mut fractions, cached.tile.coordinate, index, true)?;
        bytes.extend_from_slice(&sample.material.to_le_bytes());
        bytes.extend_from_slice(&sample.height.to_le_bytes());
        bytes.push(sample.water_depth);
        bytes.push(u8::from(sample.known) | u8::from(sample.canopy) << 1);
        for value in fractions {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    write(root, cached.tile.coordinate, bytes)
}

fn check_fractions(
    fractions: &mut [f32; 18],
    coordinate: [i64; 2],
    sample: usize,
    writing: bool,
) -> Result<(), String> {
    let tolerance = if writing { FRACTION_ROUNDOFF } else { 0.0 };
    for (field, value) in fractions.iter_mut().enumerate() {
        if !value.is_finite() || !(-tolerance..=1.0 + tolerance).contains(&f64::from(*value)) {
            let name = match field {
                0..16 => format!("biome_weights[{field}]"),
                16 => "shore".into(),
                _ => "snow".into(),
            };
            return Err(format!(
                "Invalid map sample {name}={value:?} at tile {coordinate:?}, cell {sample}"
            ));
        }
        if writing {
            *value = value.clamp(0.0, 1.0);
        }
    }
    Ok(())
}

fn write(root: &Path, coordinate: [i64; 2], mut bytes: Vec<u8>) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    bytes.extend_from_slice(&checksum(&bytes).to_le_bytes());
    let destination = path(root, coordinate);
    // Per-process temporary names also allow independent local test clients.
    let temporary = destination.with_extension(format!("{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = fs::File::create(&temporary).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temporary, destination).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tiles_roundtrip_with_identity_isolation_and_corruption_detection() {
        let temporary = std::env::temp_dir().join(format!(
            "terra-map-test-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let key = MapIdentity {
            world: terra_core::WorldId(12),
            seed: terra_core::WorldSeed(42),
            generator: 5,
            player: terra_core::PlayerUuid::new_v4(),
        };
        let root = key.directory(&temporary);
        let tile = MapTile {
            coordinate: [-3, 6],
            pixels: [117; TILE_BYTES],
            relief: None,
        };
        assert_eq!(load(&root, tile.coordinate).unwrap(), None);
        save(&root, &tile).unwrap();
        assert_eq!(coordinates(&root).unwrap(), vec![[-3, 6]]);
        assert_eq!(load(&root, tile.coordinate).unwrap(), Some(tile.clone()));
        let other = MapIdentity {
            world: terra_core::WorldId(13),
            ..key.clone()
        };
        assert!(
            load(&other.directory(&temporary), tile.coordinate)
                .unwrap()
                .is_none()
        );
        let other = MapIdentity {
            player: terra_core::PlayerUuid::new_v4(),
            ..key
        };
        assert!(
            load(&other.directory(&temporary), tile.coordinate)
                .unwrap()
                .is_none()
        );
        let file = path(&root, tile.coordinate);
        let mut bytes = fs::read(&file).unwrap();
        bytes[40] ^= 1;
        fs::write(&file, bytes).unwrap();
        assert!(load(&root, tile.coordinate).is_err());
        fs::remove_dir_all(temporary).unwrap();
    }

    #[test]
    fn old_color_tiles_and_new_height_water_tiles_share_the_same_exploration_directory() {
        let root =
            std::env::temp_dir().join(format!("terra-relief-{}", terra_core::PlayerUuid::new_v4()));
        let mut tile = MapTile {
            coordinate: [-12, -2],
            pixels: [125; TILE_BYTES],
            relief: None,
        };
        save(&root, &tile).unwrap();
        assert_eq!(
            fs::read(path(&root, tile.coordinate)).unwrap().len(),
            FILE_BYTES
        );
        assert_eq!(load(&root, tile.coordinate).unwrap().unwrap(), tile);
        let mut relief = MapRelief {
            heights: [MapRelief::UNKNOWN; 256],
            water: [0; 32],
        };
        relief.heights[0] = -120;
        relief.heights[255] = 356;
        relief.water[0] = 1;
        tile.relief = Some(Box::new(relief));
        save(&root, &tile).unwrap();
        assert_eq!(
            fs::read(path(&root, tile.coordinate)).unwrap().len(),
            FILE_BYTES + RELIEF_BYTES
        );
        let loaded = load(&root, tile.coordinate).unwrap().unwrap();
        assert_eq!(loaded, tile);
        let relief = loaded.relief.unwrap();
        assert_eq!(relief.height(0), Some(-120));
        assert_eq!(relief.height(1), None);
        assert_eq!(relief.height(255), Some(356));
        assert!(relief.is_water(0));
        assert!(!relief.is_water(1));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sample_cache_preserves_legacy_pixels_and_unrelated_exploration_files() {
        let root = std::env::temp_dir().join(format!(
            "terra-map-source-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let mut tile = MapTile {
            coordinate: [-3, 19],
            pixels: [0; TILE_BYTES],
            relief: None,
        };
        tile.pixels[..8].copy_from_slice(&[10, 20, 30, 255, 40, 50, 60, 255]);
        save(&root, &tile).unwrap();
        fs::write(root.join("waypoints.toml"), b"untouched waypoint data").unwrap();
        let legacy = load_cached(&root, tile.coordinate).unwrap().unwrap();
        assert!(legacy.source.is_none());
        assert!(!legacy.fully_recolorable());
        let mut cells = [MapSample::default(); TILE_SIZE * TILE_SIZE];
        cells[0] = MapSample {
            material: 1,
            height: -42,
            known: true,
            canopy: true,
            water_depth: 3,
            biome_weights: [
                0.0, 0.25, 0.75, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ],
            shore: 0.125,
            snow: 0.5,
        };
        let mut cached = CachedMapTile {
            tile,
            source: Some(Box::new(MapRasterSource {
                palette: 12345,
                cells,
            })),
        };
        assert!(
            !cached.fully_recolorable(),
            "the second legacy pixel has no source"
        );
        save_cached(&root, &cached).unwrap();
        assert_eq!(
            fs::metadata(path(&root, cached.tile.coordinate))
                .unwrap()
                .len(),
            SOURCE_BYTES as u64
        );
        assert_eq!(
            load_cached(&root, cached.tile.coordinate).unwrap(),
            Some(cached.clone())
        );
        assert_eq!(
            load(&root, cached.tile.coordinate).unwrap(),
            Some(cached.tile.clone())
        );
        assert_eq!(coordinates(&root).unwrap(), vec![[-3, 19]]);
        assert_eq!(
            fs::read(root.join("waypoints.toml")).unwrap(),
            b"untouched waypoint data"
        );
        cached.source.as_mut().unwrap().cells[1].known = true;
        assert!(cached.fully_recolorable());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sample_cache_rejects_oversized_truncated_and_nonfinite_sources() {
        let root = std::env::temp_dir().join(format!(
            "terra-map-source-limits-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let mut cached = CachedMapTile {
            tile: MapTile {
                coordinate: [1, -1],
                pixels: [0; TILE_BYTES],
                relief: None,
            },
            source: Some(Box::new(MapRasterSource {
                palette: 1,
                cells: [MapSample::default(); TILE_SIZE * TILE_SIZE],
            })),
        };
        save_cached(&root, &cached).unwrap();
        let file = path(&root, cached.tile.coordinate);
        let original = fs::read(&file).unwrap();
        for length in [0, 7, SOURCE_BYTES - 1, SOURCE_BYTES + 1, SOURCE_BYTES * 2] {
            let mut bytes = original.clone();
            bytes.resize(length, 0);
            fs::write(&file, bytes).unwrap();
            assert!(load_cached(&root, cached.tile.coordinate).is_err());
        }
        let mut bytes = original;
        let sample_start = 24 + TILE_BYTES + 9 + RELIEF_BYTES;
        bytes[sample_start + 6..sample_start + 10].copy_from_slice(&f32::NAN.to_le_bytes());
        let end = bytes.len() - 8;
        let check = checksum(&bytes[..end]);
        bytes[end..].copy_from_slice(&check.to_le_bytes());
        fs::write(&file, bytes).unwrap();
        assert!(load_cached(&root, cached.tile.coordinate).is_err());
        for value in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -0.001,
            1.001,
            -1e-12,
            1.0 + f32::EPSILON,
        ] {
            cached.source.as_mut().unwrap().cells[0].snow = value;
            let error = save_cached(&root, &cached).unwrap_err();
            assert!(error.contains("snow="));
            assert!(error.contains("tile [1, -1], cell 0"));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_saturated_climate_roundoff_is_canonicalized_only_by_the_writer() {
        let ocean = 0.047_f64;
        let highland = 0.7624;
        let desert = 0.059086;
        let mushroom = 0.03550878;
        let forest = 1.0 - ocean - highland - desert - mushroom;
        let plains = 1.0 - ocean - highland - forest - desert - mushroom;
        assert!(plains < 0.0 && plains > -FRACTION_ROUNDOFF);
        let root = std::env::temp_dir().join(format!(
            "terra-map-roundoff-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let mut sample = MapSample {
            known: true,
            material: 1,
            biome_weights: [
                ocean, plains, forest, desert, highland, mushroom, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0,
            ]
            .map(|value| value as f32),
            shore: -f64::EPSILON as f32,
            ..Default::default()
        };
        let cached = CachedMapTile {
            tile: MapTile {
                coordinate: [-17, 31],
                pixels: [125; TILE_BYTES],
                relief: None,
            },
            source: Some(Box::new(MapRasterSource {
                palette: 12,
                cells: [sample; 256],
            })),
        };
        save_cached(&root, &cached).unwrap();
        let loaded = load_cached(&root, cached.tile.coordinate).unwrap().unwrap();
        sample.biome_weights[1] = 0.0;
        sample.shore = 0.0;
        assert!(
            loaded
                .source
                .as_ref()
                .unwrap()
                .cells
                .iter()
                .all(|cell| *cell == sample)
        );
        assert_eq!(loaded.tile, cached.tile);
        // The old writer never emitted negative fractions. V3 input remains
        // strict even for checksum-valid endpoint-sized negative corruption.
        let file = path(&root, cached.tile.coordinate);
        let mut bytes = fs::read(&file).unwrap();
        let plains_offset = 24 + TILE_BYTES + 9 + RELIEF_BYTES + 10;
        bytes[plains_offset..plains_offset + 4].copy_from_slice(&(plains as f32).to_le_bytes());
        let end = bytes.len() - 8;
        let check = checksum(&bytes[..end]);
        bytes[end..].copy_from_slice(&check.to_le_bytes());
        fs::write(file, bytes).unwrap();
        assert!(load_cached(&root, cached.tile.coordinate).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn actual_legacy_generator_samples_keep_their_finite_unit_fraction_cache_contract() {
        let generator = terra_world::SectionGenerator::new(
            terra_world::TerraGenerator::new(terra_core::WorldConfig {
                seed: terra_core::WorldSeed(42042),
                landscape_revision: 1,
                geology_revision: 4,
                ecology_revision: 2,
                ..Default::default()
            })
            .unwrap(),
        );
        let coordinate = [-231, -488];
        let column = generator.prepare_column(coordinate, true).unwrap();
        let cells = std::array::from_fn(|index| {
            let surface = column
                .surface_at(
                    coordinate[0] * 16 + (index % 16) as i64,
                    coordinate[1] * 16 + (index / 16) as i64,
                )
                .unwrap();
            MapSample {
                known: true,
                material: surface.surface_material as u16,
                biome_weights: surface.biome.weights.values().map(|value| value as f32),
                shore: surface.shore_cover as f32,
                snow: if surface.biome.aridity() <= 0.5 {
                    surface.snow_cover as f32
                } else {
                    0.0
                },
                ..Default::default()
            }
        });
        let cached = CachedMapTile {
            tile: MapTile {
                coordinate,
                pixels: [125; TILE_BYTES],
                relief: None,
            },
            source: Some(Box::new(MapRasterSource { palette: 12, cells })),
        };
        let root = std::env::temp_dir().join(format!(
            "terra-map-generator-fractions-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        save_cached(&root, &cached).unwrap();
        let loaded = load_cached(&root, coordinate).unwrap().unwrap();
        for (actual, mut expected) in loaded.source.unwrap().cells.into_iter().zip(cells) {
            expected.biome_weights = expected.biome_weights.map(|value| value.clamp(0.0, 1.0));
            expected.shore = expected.shore.clamp(0.0, 1.0);
            expected.snow = expected.snow.clamp(0.0, 1.0);
            assert_eq!(actual, expected);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
