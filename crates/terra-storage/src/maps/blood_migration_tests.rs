use super::*;

fn root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "terra-map-blood-migration-{}",
        terra_core::PlayerUuid::new_v4()
    ))
}

/// Encode the documented historical layout directly, independently of the
/// current writer, so an accidental stride change cannot change the fixture.
fn version_four(coordinate: [i64; 2], pixels: &[u8; TILE_BYTES], relief: &MapRelief) -> Vec<u8> {
    let mut bytes = b"TRMAP004".to_vec();
    for value in coordinate {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(pixels);
    bytes.push(1);
    bytes.extend_from_slice(&917_u64.to_le_bytes());
    for height in relief.heights {
        bytes.extend_from_slice(&height.to_le_bytes());
    }
    bytes.extend_from_slice(&relief.water);
    for index in 0..TILE_SIZE * TILE_SIZE {
        bytes.extend_from_slice(&300_u16.to_le_bytes());
        bytes.extend_from_slice(&(-12_i16).to_le_bytes());
        bytes.push(3);
        bytes.push((index % 4) as u8);
        let fractions: [f32; 11] = [
            0.05, 0.05, 0.05, 0.05, 0.05, 0.05, 0.05, 0.05, 0.6, 0.375, 0.625,
        ];
        for value in fractions {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    assert_eq!(bytes.len() + 8, SPIRIT_SOURCE_BYTES);
    bytes
}

#[test]
fn version_four_keeps_shore_snow_and_exploration_separate_from_new_blood_fraction() {
    let root = root();
    let coordinate = [-24, 7];
    let pixels = std::array::from_fn(|i| {
        if i % 4 == 3 {
            if (i / 4) % 3 == 0 { 255 } else { 0 }
        } else {
            (i % 251) as u8
        }
    });
    let relief = MapRelief {
        heights: std::array::from_fn(|i| i as i16 - 120),
        water: [0x55; TILE_SIZE * TILE_SIZE / 8],
    };
    write(
        &root,
        coordinate,
        version_four(coordinate, &pixels, &relief),
    )
    .unwrap();
    fs::write(root.join("waypoints.toml"), b"preserve all waypoints").unwrap();
    let legacy_tile = MapTile {
        coordinate: [3, -2],
        pixels: [17; TILE_BYTES],
        relief: None,
    };
    save(&root, &legacy_tile).unwrap();
    let untouched = fs::read(path(&root, legacy_tile.coordinate)).unwrap();
    let cached = load_cached(&root, coordinate).unwrap().unwrap();
    assert_eq!(cached.tile.pixels, pixels);
    assert_eq!(cached.tile.relief.as_deref(), Some(&relief));
    let source = cached.source.as_ref().unwrap();
    assert_eq!(source.palette, 917);
    for (index, sample) in source.cells.iter().enumerate() {
        assert_eq!(sample.biome_weights[..8], [0.05; 8]);
        assert_eq!(sample.biome_weights[8], 0.6);
        assert_eq!(sample.biome_weights[9], 0.0);
        assert_eq!((sample.shore, sample.snow), (0.375, 0.625));
        assert_eq!(sample.known, index % 2 == 1);
        assert_eq!(sample.canopy, index % 4 >= 2);
        assert_eq!(
            (sample.material, sample.height, sample.water_depth),
            (300, -12, 3)
        );
    }
    save_cached(&root, &cached).unwrap();
    let bytes = fs::read(path(&root, coordinate)).unwrap();
    assert_eq!(&bytes[..8], b"TRMAP005");
    assert_eq!(bytes.len(), SOURCE_BYTES);
    assert_eq!(load_cached(&root, coordinate).unwrap(), Some(cached));
    assert_eq!(
        fs::read(root.join("waypoints.toml")).unwrap(),
        b"preserve all waypoints"
    );
    assert_eq!(
        fs::read(path(&root, legacy_tile.coordinate)).unwrap(),
        untouched
    );
    assert_eq!(
        coordinates(&root).unwrap(),
        vec![coordinate, legacy_tile.coordinate]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn version_five_roundtrips_blood_forest_spirit_shore_and_snow_without_aliasing() {
    let root = root();
    let sample = MapSample {
        material: 400,
        height: 43,
        water_depth: 9,
        known: true,
        canopy: true,
        biome_weights: [0.0, 0.15, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.25, 0.6],
        shore: 0.125,
        snow: 0.875,
    };
    let mut cached = CachedMapTile {
        tile: MapTile {
            coordinate: [-1, -1],
            pixels: [123; TILE_BYTES],
            relief: None,
        },
        source: Some(Box::new(MapRasterSource {
            palette: 312,
            cells: [sample; TILE_SIZE * TILE_SIZE],
        })),
    };
    save_cached(&root, &cached).unwrap();
    assert_eq!(load_cached(&root, [-1, -1]).unwrap(), Some(cached.clone()));
    let file = path(&root, [-1, -1]);
    let original = fs::read(&file).unwrap();
    assert_eq!(&original[..8], b"TRMAP005");
    for invalid in [f32::NAN, f32::INFINITY, -0.01, 1.01] {
        cached.source.as_mut().unwrap().cells[0].biome_weights[9] = invalid;
        assert!(
            save_cached(&root, &cached)
                .unwrap_err()
                .contains("biome_weights[9]")
        );
        assert_eq!(
            fs::read(&file).unwrap(),
            original,
            "invalid source must not replace explored map data"
        );
    }
    let mut mislabeled = original;
    mislabeled[..8].copy_from_slice(b"TRMAP004");
    let end = mislabeled.len() - 8;
    let check = checksum(&mislabeled[..end]);
    mislabeled[end..].copy_from_slice(&check.to_le_bytes());
    fs::write(&file, mislabeled).unwrap();
    assert!(
        load_cached(&root, [-1, -1])
            .unwrap_err()
            .contains("header or size")
    );
    fs::remove_dir_all(root).unwrap();
}
