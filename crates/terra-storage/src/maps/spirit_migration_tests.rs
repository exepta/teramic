use super::*;

#[test]
fn eight_biome_cache_keeps_exploration_and_fractions_when_read_as_ten_biomes() {
    let root = std::env::temp_dir().join(format!(
        "terra-map-spirit-migration-{}",
        terra_core::PlayerUuid::new_v4()
    ));
    let coordinate = [-17, 23];
    let fractions: [f32; 10] = [0.10, 0.20, 0.30, 0.15, 0.10, 0.05, 0.05, 0.05, 0.375, 0.625];
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"TRMAP003");
    for value in coordinate {
        bytes.extend_from_slice(&i64::to_le_bytes(value));
    }
    let mut pixels = [0; TILE_BYTES];
    pixels[..4].copy_from_slice(&[32, 64, 128, 255]);
    bytes.extend_from_slice(&pixels);
    bytes.push(0);
    bytes.extend_from_slice(&917_u64.to_le_bytes());
    bytes.resize(bytes.len() + RELIEF_BYTES, 0);
    for index in 0..TILE_SIZE * TILE_SIZE {
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&(-12_i16).to_le_bytes());
        bytes.push(3);
        bytes.push(u8::from(index == 0));
        for fraction in fractions {
            bytes.extend_from_slice(&fraction.to_le_bytes());
        }
    }
    assert_eq!(bytes.len() + 8, LEGACY_SOURCE_BYTES);
    write(&root, coordinate, bytes).unwrap();
    let cached = load_cached(&root, coordinate).unwrap().unwrap();
    assert_eq!(cached.tile.pixels, pixels);
    let source = cached.source.as_ref().unwrap();
    assert_eq!(source.palette, 917);
    for (index, sample) in source.cells.iter().enumerate() {
        assert_eq!(sample.biome_weights[..8], fractions[..8]);
        assert_eq!(sample.biome_weights[8], 0.0);
        assert_eq!(sample.biome_weights[9], 0.0);
        assert_eq!(sample.shore, 0.375);
        assert_eq!(sample.snow, 0.625);
        assert_eq!(sample.height, -12);
        assert_eq!(sample.known, index == 0);
    }
    let mut migrated = cached;
    migrated.source.as_mut().unwrap().cells[0].biome_weights =
        [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.25, 0.75, 0.0];
    save_cached(&root, &migrated).unwrap();
    assert_eq!(
        &fs::read(path(&root, coordinate)).unwrap()[..8],
        b"TRMAP005"
    );
    assert_eq!(load_cached(&root, coordinate).unwrap(), Some(migrated));
    fs::remove_dir_all(root).unwrap();
}
