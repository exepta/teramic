use super::*;

#[test]
fn profession_stations_load_real_glb_scenes_with_distinct_geometry_and_matching_collision() {
    let directory = terra_core::asset_directory().join("terra/blocks");
    let library = BlockLibrary::load(&directory).unwrap();
    let mut fingerprints = std::collections::BTreeSet::new();
    for (id, path) in [
        (297, "terra/blocks/models/lumberjack_station.glb"),
        (298, "terra/blocks/models/smith_station.glb"),
        (299, "terra/blocks/models/market_station.glb"),
        (300, "terra/blocks/models/army_station.glb"),
        (301, "terra/blocks/models/railway_station.glb"),
        (302, "terra/blocks/models/mine_station.glb"),
        (303, "terra/blocks/models/butcher_station.glb"),
        (304, "terra/blocks/models/bishop_station.glb"),
        (305, "terra/blocks/models/farm_station.glb"),
    ] {
        let definition = library.blocks.iter().find(|block| block.id == id).unwrap();
        definition.validate().unwrap();
        assert_eq!(definition.render_kind, BlockRenderKind::CustomMesh);
        assert_eq!(definition.model_layout, ModelLayout::Scene);
        assert_eq!(
            definition.collision_boxes.as_slice(),
            terra_core::profession_stations::boxes(id)
        );
        let model = load_static_scene(path, id);
        let vertices: usize = model
            .iter()
            .map(|primitive| primitive.positions.len())
            .sum();
        let triangles: usize = model
            .iter()
            .map(|primitive| primitive.indices.len() / 3)
            .sum();
        assert!(
            vertices > 400 && triangles > 150,
            "{id}: authored scene geometry"
        );
        assert!(
            fingerprints.insert((vertices, triangles)),
            "{id}: distinct profession silhouette"
        );
        for primitive in &model {
            assert_eq!(primitive.positions.len(), primitive.normals.len());
            assert_eq!(primitive.positions.len(), primitive.uvs.len());
            for point in &primitive.positions {
                assert!(
                    point
                        .iter()
                        .all(|axis| axis.is_finite() && (-0.001..=1.001).contains(axis)),
                    "{id}: {point:?}"
                );
            }
            for normal in &primitive.normals {
                assert!(
                    (Vec3::from_array(*normal).length() - 1.).abs() < 0.001,
                    "{id}: {normal:?}"
                );
            }
            for triangle in primitive.indices.chunks_exact(3) {
                let [a, b, c] = std::array::from_fn(|i| {
                    Vec3::from_array(primitive.positions[triangle[i] as usize])
                });
                assert!(
                    (b - a).cross(c - a).length_squared() > 1e-14,
                    "{id}: degenerate triangle"
                );
            }
        }
    }
}
