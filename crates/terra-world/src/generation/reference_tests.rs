use super::*;
use terra_core::WorldConfig;

#[test]
fn version_five_reference_regions_match_the_seeded_landscape() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 0,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        terrain.version(),
        5,
        "update reference fixtures when changing generator versions"
    );
    // Genuine highland ranges now rise above Y80; ecology follows that altitude
    // instead of labelling the old low foothills as Highland.
    for ([x, z], height, park_influence) in [
        ([0.0, 0.0], 107.676959142, true),
        ([512.0, -512.0], 112.549834159, true),
        ([-256.0, 256.0], 121.332939037, true),
        ([1024.0, 768.0], 118.130066651, false),
    ] {
        assert_eq!(
            terrain.biome_at(x, z).weights.mushroom_park > 0.0,
            park_influence
        );
        assert!(
            (terrain.surface_height(x, z) - height).abs() < 1e-7,
            "{x}, {z}"
        );
    }
}
