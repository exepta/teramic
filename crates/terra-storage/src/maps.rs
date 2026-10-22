//! Client exploration data is disposable cartography, never authoritative world edits.
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use terra_world::cartography::{MapRelief, MapTile, TILE_BYTES, TILE_SIZE};

const MAGIC: &[u8; 8] = b"TRMAP001";
const FILE_BYTES: usize = 8 + 16 + TILE_BYTES + 8;
const RELIEF_MAGIC: &[u8; 8] = b"TRMAP002";
const RELIEF_BYTES: usize = TILE_SIZE * TILE_SIZE * 2 + TILE_SIZE * TILE_SIZE / 8;

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
    let file = match fs::File::open(path(root, coordinate)) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = Vec::with_capacity(FILE_BYTES + RELIEF_BYTES);
    file.take((FILE_BYTES + RELIEF_BYTES) as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let has_relief = bytes.len() == FILE_BYTES + RELIEF_BYTES && &bytes[..8] == RELIEF_MAGIC;
    if !has_relief && (bytes.len() != FILE_BYTES || &bytes[..8] != MAGIC) {
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
    let relief = has_relief.then(|| {
        let mut heights = [MapRelief::UNKNOWN; TILE_SIZE * TILE_SIZE];
        let start = 24 + TILE_BYTES;
        for (i, height) in heights.iter_mut().enumerate() {
            *height = i16::from_le_bytes([bytes[start + i * 2], bytes[start + i * 2 + 1]]);
        }
        let mut water = [0; TILE_SIZE * TILE_SIZE / 8];
        water.copy_from_slice(&bytes[start + heights.len() * 2..bytes.len() - 8]);
        Box::new(MapRelief { heights, water })
    });
    Ok(Some(MapTile {
        coordinate,
        pixels,
        relief,
    }))
}

pub fn save(root: &Path, tile: &MapTile) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
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
    bytes.extend_from_slice(&checksum(&bytes).to_le_bytes());
    let destination = path(root, tile.coordinate);
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
}
