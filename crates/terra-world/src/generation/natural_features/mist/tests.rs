use super::*;
use crate::SectionGenerator;
use terra_core::{Config, WorldConfig};

fn terrain(revision: u8) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        ecology_revision: revision,
        structures_revision: 0,
        trees: false,
        caves: false,
        water: false,
        rivers: false,
        base_height: 48.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        terrace_strength: 0.0,
        sea_level: -64.0,
        snow_height: 300.0,
        ..Default::default()
    })
    .unwrap()
}

fn habitat(terrain: &TerraGenerator, tile: Tile) -> Habitat<'_> {
    let mut column = terrain.surface_column(0.5, 0.5);
    column.height = 48.0;
    column.slope = 0.0;
    column.water_level = None;
    column.cave_cavity = None;
    column.river_cavity = None;
    column.surface_material = MaterialId::SpiritBlossomGrass;
    column.biome.weights.spirit_blossom = 1.0;
    let columns = (tile[1] * 32..tile[1] * 32 + 32)
        .flat_map(|z| (tile[0] * 32..tile[0] * 32 + 32).map(move |x| ([x, z], column)))
        .collect();
    Habitat {
        terrain,
        trees: Vec::new(),
        sites: Vec::new(),
        columns,
    }
}

#[test]
fn sparse_spirit_sources_are_buried_or_exposed_and_keep_separation_across_negative_tiles() {
    let terrain = terrain(8);
    let mut sources = Vec::new();
    let mut exposed = 0;
    for z in -6..6 {
        for x in -6..6 {
            let tile = [x, z];
            let mut cells = BTreeMap::new();
            build(&mut habitat(&terrain, tile), tile, &mut cells);
            assert!(cells.len() <= 1);
            for (cell, material) in cells {
                assert_eq!(material, MaterialId::FogBlock as u16);
                assert!((46..=47).contains(&cell[1]));
                assert_eq!([cell[0].div_euclid(32), cell[2].div_euclid(32)], tile);
                exposed += usize::from(cell[1] == 47);
                sources.push(cell);
            }
        }
    }
    assert!(
        (15..70).contains(&sources.len()),
        "sparse sources: {}",
        sources.len()
    );
    assert!(exposed > 0 && exposed < sources.len() / 2);
    for (i, a) in sources.iter().enumerate() {
        for b in &sources[i + 1..] {
            assert!((a[0] - b[0]).pow(2) + (a[2] - b[2]).pow(2) >= 17 * 17);
        }
    }
}

#[test]
fn old_ecology_and_unsuitable_or_occupied_ground_never_receive_fog_stones() {
    for revision in 0..=7 {
        let t = terrain(revision);
        let reloaded: WorldConfig =
            serde_json::from_slice(&serde_json::to_vec(t.config()).unwrap()).unwrap();
        assert_eq!(reloaded.ecology_revision, revision);
        for tile in [[-1, -1], [0, 0], [1, 1]] {
            let mut cells = BTreeMap::new();
            build(&mut habitat(&t, tile), tile, &mut cells);
            assert!(cells.is_empty());
        }
    }
    let t = terrain(8);
    for z in -3..3 {
        for x in -3..3 {
            for reason in 0..5 {
                let tile = [x, z];
                let mut h = habitat(&t, tile);
                for c in h.columns.values_mut() {
                    match reason {
                        0 => c.biome.weights.spirit_blossom = 0.4,
                        1 => c.surface_material = MaterialId::Grass,
                        2 => c.water_level = Some(48.5),
                        3 => c.slope = 1.4,
                        _ => c.river_cavity = Some([40.0, 54.0]),
                    }
                }
                let mut cells = BTreeMap::new();
                build(&mut h, tile, &mut cells);
                assert!(cells.is_empty(), "reason {reason}");
            }
            let tile = [x, z];
            let mut h = habitat(&t, tile);
            let mut cells = BTreeMap::new();
            build(&mut h, tile, &mut cells);
            if let Some((&source, _)) = cells.first_key_value() {
                let mut occupied = BTreeMap::from([(source, MaterialId::Stone as u16)]);
                build(&mut h, tile, &mut occupied);
                assert_eq!(
                    occupied,
                    BTreeMap::from([(source, MaterialId::Stone as u16)])
                );
            }
        }
    }
}

#[test]
fn actual_negative_spirit_sources_match_sections_queries_and_reload_without_changing_height() {
    let mut config =
        WorldConfig::from_toml(include_str!("../../../../../../configs/world.toml")).unwrap();
    config.ecology_revision = 8;
    config.biome_definitions = Some(terra_core::BiomeDefinitions::default());
    let t = TerraGenerator::new(config.clone()).unwrap();
    let a = SectionGenerator::new(t.clone());
    let mut sources = Vec::new();
    let anchor = [-2872_i64, -4045_i64].map(|v| v.div_euclid(32));
    for z in -4..=4 {
        for x in -4..=4 {
            let tile = [anchor[0] + x, anchor[1] + z];
            sources.extend(t.spirit_mist_sources(tile));
        }
    }
    assert!(
        !sources.is_empty(),
        "known negative Spirit Forest must contain natural fog stones"
    );
    let reloaded =
        TerraGenerator::new(serde_json::from_slice(&serde_json::to_vec(&config).unwrap()).unwrap())
            .unwrap();
    let b = SectionGenerator::new(reloaded.clone());
    let old = TerraGenerator::new(WorldConfig {
        ecology_revision: 7,
        ..config
    })
    .unwrap();
    for p in sources.iter().rev().take(6) {
        let (section, local) = terra_voxel::global_to_section(*p);
        assert_eq!(a.block_at(*p).unwrap(), MaterialId::FogBlock as u16);
        assert_eq!(
            a.generate(section).unwrap().block(local).unwrap(),
            MaterialId::FogBlock as u16
        );
        assert_eq!(
            b.generate(section).unwrap().block(local).unwrap(),
            MaterialId::FogBlock as u16
        );
        // A mined source stays removed when its section is regenerated.
        let edits = HashMap::from([(*p, 0)]);
        let changed = a.build(section, 1, &edits, false).unwrap();
        assert_eq!(changed.data.block(local).unwrap(), 0);
        let restored = b.build(section, 1, &edits, true).unwrap();
        assert_eq!(restored.data.block(local).unwrap(), 0);
        let tile = [p[0].div_euclid(32), p[2].div_euclid(32)];
        assert_eq!(
            t.spirit_mist_sources(tile),
            reloaded.spirit_mist_sources(tile)
        );
        assert!(old.spirit_mist_sources(tile).is_empty());
        assert_eq!(
            t.surface_height(p[0] as f64 + 0.5, p[2] as f64 + 0.5),
            old.surface_height(p[0] as f64 + 0.5, p[2] as f64 + 0.5)
        );
    }
    println!("natural_mist_sources seed=42042 sources={sources:?}");
}
