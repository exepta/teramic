use super::{
    definitions::{BlockLibrary, BlockRenderKind, ModelLayout},
    test_static_scene,
};

#[test]
fn castle_window_uses_real_window_glass_and_four_separate_transparent_panes() {
    let models = terra_core::asset_directory().join("terra/blocks/models");
    let document = |name: &str| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(models.join(name)).unwrap()).unwrap()
    };
    let castle = document("castle_window.gltf");
    let window = document("oak_window.gltf");
    let materials = castle["materials"].as_array().unwrap();
    let (slot, pane) = materials
        .iter()
        .enumerate()
        .find(|(_, m)| m["name"] == "glass")
        .unwrap();
    let ordinary = window["materials"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "leaf_glass")
        .unwrap();
    assert_eq!(pane["alphaMode"], "BLEND");
    assert_eq!(pane["doubleSided"], true);
    assert_eq!(
        pane["pbrMetallicRoughness"],
        ordinary["pbrMetallicRoughness"]
    );
    let model = test_static_scene(
        "terra/blocks/models/castle_window.gltf",
        terra_core::castle::WINDOW,
    );
    let glass = model.iter().find(|p| p.material == slot).unwrap();
    assert_eq!(glass.positions.len(), 4 * 24);
    // None of the glass reaches into the wooden vertical/horizontal crossbars.
    for triangle in glass.indices.chunks_exact(3) {
        let center = std::array::from_fn::<_, 3, _>(|axis| {
            triangle
                .iter()
                .map(|&index| glass.positions[index as usize][axis])
                .sum::<f32>()
                / 3.0
        });
        assert!(center[0] <= 0.480001 || center[0] >= 0.519999);
        assert!(center[1] <= 0.480001 || center[1] >= 0.529999);
    }
}

#[test]
fn castle_models_load_through_the_real_scene_pipeline_with_stone_patterns_tangents_and_relief() {
    let directory = terra_core::asset_directory().join("terra/blocks");
    let library = BlockLibrary::load(&directory).unwrap();
    for material in terra_core::castle::BLOCKS {
        let definition = library
            .blocks
            .iter()
            .find(|block| block.id == material)
            .unwrap();
        assert_eq!(definition.render_kind, BlockRenderKind::CustomMesh);
        assert_eq!(definition.model_layout, ModelLayout::Scene);
        assert!(definition.materials.iter().any(|(name, material)| {
            (name.starts_with("stone_") || name.contains("_stone_"))
                && material.texture_pattern.is_some()
                && material.base_color[..3]
                    .iter()
                    .all(|channel| *channel >= 0.9 || definition.id == 190)
        }));
        assert_eq!(
            definition.collision_boxes.as_slice(),
            terra_core::castle::boxes(material)
        );
        let path: &'static str =
            Box::leak(format!("terra/blocks/{}", definition.model).into_boxed_str());
        let geometry = test_static_scene(path, material);
        assert!(
            geometry.len() >= 2,
            "authored stone/mortar palette missing: {material}"
        );
        assert!(
            geometry
                .iter()
                .map(|primitive| primitive.positions.len())
                .sum::<usize>()
                > 200
        );
        for primitive in &geometry {
            assert_eq!(primitive.positions.len(), primitive.normals.len());
            assert_eq!(primitive.positions.len(), primitive.tangents.len());
            assert_eq!(primitive.positions.len(), primitive.uvs.len());
            assert!(
                primitive
                    .positions
                    .iter()
                    .flatten()
                    .all(|value| (-0.00001..=1.00001).contains(value))
            );
            assert!(primitive.normals.iter().all(|normal| {
                (bevy::math::Vec3::from_array(*normal).length() - 1.0).abs() < 0.001
            }));
            assert!(
                primitive
                    .indices
                    .iter()
                    .all(|index| (*index as usize) < primitive.positions.len())
            );
        }
        let mut templates = super::TemplateSet::default();
        let document: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(directory.join(&definition.model)).unwrap(),
        )
        .unwrap();
        for (slot, material) in document["materials"].as_array().unwrap().iter().enumerate() {
            templates.register_castle_material(material["name"].as_str().unwrap(), slot);
        }
        if matches!(
            material,
            terra_core::castle::BERGFRIED | terra_core::castle::ROYAL_WALL
        ) {
            assert_eq!(
                templates.castle_icon_materials.len(),
                terra_core::kingdom::KingdomIcon::ALL.len()
            );
            assert_eq!(templates.castle_cloth_materials.len(), 1);
        }
        templates.blocks.insert(
            material,
            super::BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: geometry.clone(),
            },
        );
        for facing in 0..24 {
            let batches = crate::meshing::custom::model_geometry(
                material,
                terra_world::edits::BlockObjectState {
                    facing,
                    ..Default::default()
                },
                Default::default(),
                [None; 6],
                [0; 6],
                &templates,
            )
            .unwrap();
            let selected = if terra_core::castle::connections::is_connecting(material) {
                &geometry[..terra_core::castle::connections::MODULE_PRIMITIVES]
            } else {
                &geometry
            };
            for primitive in selected {
                if templates
                    .castle_icon_materials
                    .get(&primitive.material)
                    .is_some_and(|icon| {
                        *icon != terra_core::kingdom::KingdomHeraldry::default().icon
                    })
                {
                    assert!(
                        !batches
                            .iter()
                            .any(|batch| batch.material == primitive.material)
                    );
                    continue;
                }
                let batch = batches
                    .iter()
                    .find(|batch| batch.material == primitive.material)
                    .unwrap();
                assert_eq!(batch.positions.len(), primitive.positions.len());
                if templates
                    .castle_decor_materials
                    .contains(&primitive.material)
                {
                    continue;
                }
                for (source, rendered) in primitive.positions.iter().zip(&batch.positions) {
                    let expected = terra_core::castle::point(facing, source.map(f64::from));
                    assert!(
                        (0..3).all(
                            |axis| (expected[axis] - f64::from(rendered[axis])).abs() < 0.00001
                        ),
                        "castle mesh/collision rotation disagreement: {material}/{facing}"
                    );
                }
            }
        }
    }
}

#[test]
fn castle_crafting_keeps_the_requested_stone_and_crown_costs_and_covers_every_model() {
    let mut book = terra_core::crafting::RecipeBook::default();
    book.load_directory(&terra_core::asset_directory().join("crafting"))
        .unwrap();
    for material in terra_core::castle::BLOCKS {
        let id = terra_voxel::MaterialId::from_raw(material).unwrap();
        let slug = id.identifier().strip_prefix("teramic::").unwrap();
        let path = terra_core::asset_directory()
            .join("crafting")
            .join(format!("{slug}.json"));
        if terra_core::castle::retired_item(material) {
            assert!(
                !path.exists(),
                "automatic corners must not retain a second craftable item"
            );
            continue;
        }
        let recipe: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(recipe["output"]["item"]["block"], material);
        assert_eq!(recipe["output"]["count"], 1);
        if material == terra_core::castle::STONE_WALL
            || material == terra_core::castle::BERGFRIED_WALL
        {
            let expected = if material == terra_core::castle::STONE_WALL {
                3
            } else {
                terra_core::castle::STONE_WALL
            };
            let cells = recipe["pattern"]["cells"].as_array().unwrap();
            assert_eq!(cells.len(), 4);
            assert!(
                cells
                    .iter()
                    .all(|cell| cell["item"]["block"] == expected && cell["count"] == 1)
            );
        }
    }
}

#[test]
fn castle_arch_visible_soffit_is_a_full_depth_continuous_circle_with_smooth_normals() {
    for (material, file, right) in [
        (
            terra_core::castle::ARCH_LEFT,
            "terra/blocks/models/castle_arch_left.gltf",
            false,
        ),
        (
            terra_core::castle::ARCH_RIGHT,
            "terra/blocks/models/castle_arch_right.gltf",
            true,
        ),
    ] {
        let geometry = test_static_scene(file, material);
        let mut stations = std::collections::BTreeSet::new();
        for primitive in &geometry {
            for (&position, &normal) in primitive.positions.iter().zip(&primitive.normals) {
                if normal[1] >= -0.0001 || normal[2].abs() > 0.0001 || normal[0].abs() < 0.0001 {
                    continue;
                }
                let x = if right {
                    1.0 - position[0]
                } else {
                    position[0]
                };
                let radius = bevy::math::Vec2::new(x - 1.0, position[1] - 0.15);
                assert!(
                    (radius.length() - 0.75).abs() < 0.00001,
                    "visible stair step in arch {material}: {position:?}"
                );
                let direction = (-radius).normalize();
                let expected_x = if right { -direction.x } else { direction.x };
                assert!((normal[0] - expected_x).abs() < 0.00001);
                assert!((normal[1] - direction.y).abs() < 0.00001);
                assert!(
                    position[2] == 0.0 || position[2] == 1.0,
                    "arch must span the full block depth"
                );
                stations.insert((x * 1_000_000.0).round() as i32);
            }
        }
        assert!(
            stations.len() >= 30,
            "insufficient continuous curve resolution"
        );
    }
}
