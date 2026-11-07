use super::definitions::{BlockDefinition, BlockLibrary, BlockRenderKind};

fn library() -> BlockLibrary {
    BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks"))
        .expect("shipped JSON block definitions")
}

#[test]
fn shipped_blocks_have_square_edges_without_changing_material_identity() {
    use bevy::math::Vec3;
    let definitions = library();
    let mut checked = 0;
    for block in &definitions.blocks {
        if block.id == 1 {
            assert_eq!(block.render_kind, BlockRenderKind::CustomMesh);
            assert_eq!(
                block.bevel_width, 0.02,
                "authored join band, not a new cube bevel"
            );
            assert!(block.coarse_faces.is_some());
            continue;
        }
        assert_eq!(
            block.bevel_width, 0.0,
            "{} still has rounded edges",
            block.id
        );
        if block.render_kind != BlockRenderKind::Cube {
            continue;
        }
        checked += 1;
        let faces = super::cube_faces(block.bevel_width, [0, 1, 2, 3, 4, 5], None);
        let mut corners = std::collections::BTreeSet::new();
        for (side, patches) in faces.iter().enumerate() {
            assert_eq!(patches.len(), 1, "square face needs only one quad");
            for patch in patches {
                assert_eq!(patch.material, side);
                assert_eq!(patch.positions.len(), 4);
                for (&position, &normal) in patch.positions.iter().zip(&patch.normals) {
                    assert!(
                        position
                            .into_iter()
                            .all(|value| value == 0.0 || value == 1.0)
                    );
                    let normal = Vec3::from_array(normal).abs();
                    assert_eq!(normal.element_sum(), 1.0);
                    assert_eq!(normal.max_element(), 1.0);
                    corners.insert(position.map(|value| value as u8));
                }
            }
        }
        assert_eq!(corners.len(), 8, "cube must retain all eight exact corners");
    }
    assert!(checked >= 30);
}

#[test]
fn ice_is_one_shared_translucent_solid_cube_without_a_glb_or_light() {
    let definitions = library();
    let ice = definitions.blocks.iter().find(|d| d.id == 49).unwrap();
    assert_eq!(ice.render_kind, BlockRenderKind::Cube);
    assert!(ice.model.is_empty() && ice.light.is_none());
    assert!(ice.frame_material.is_none() && ice.collision_boxes.is_empty());
    assert!(terra_voxel::MaterialId::Ice.is_solid());
    assert!(!terra_voxel::MaterialId::Ice.is_opaque());
    assert_eq!(terra_voxel::MaterialId::Ice.identifier(), "teramic::ice");
    let material = ice.materials.values().next().unwrap();
    assert!((0.8..1.0).contains(&material.base_color[3]));
    assert!(material.roughness >= 0.3);
    assert_eq!(material.emission, [0.0; 3]);
    assert_eq!(
        material.texture_pattern,
        Some(super::definitions::TexturePattern::Ice)
    );
}

#[test]
fn underwater_blocks_use_the_closed_cube_registry_without_models_or_extra_entities() {
    let definitions = library();
    for id in 44..=48 {
        let block = definitions
            .blocks
            .iter()
            .find(|block| block.id == id)
            .unwrap();
        assert_eq!(block.render_kind, BlockRenderKind::Cube);
        assert!(block.model.is_empty() && block.collision_boxes.is_empty());
        assert!(block.surface.is_some());
        assert_eq!(block.materials.len(), 1);
        assert!(block.gameplay.valid() && block.gameplay.can_harvest(None));
        assert!(!block.description.is_empty());
        assert!(block.light.is_none());
        let material = block.materials.values().next().unwrap();
        assert!(material.roughness >= 0.9 && material.reflectance <= 0.12);
        assert_eq!(material.emission, [0.0; 3]);
        let faces = super::cube_faces(block.bevel_width, [0; 6], None);
        let template = super::surface::SurfaceTemplate::new(block.surface.unwrap());
        let mut bounds = [
            bevy::math::Vec3::splat(f32::INFINITY),
            bevy::math::Vec3::splat(f32::NEG_INFINITY),
        ];
        for (side, source) in faces.iter().enumerate() {
            for patch in template.face(source, side, 0, 0, |p| p) {
                for &point in &patch.primitive.positions {
                    assert!(point.iter().all(|v| (0.0..=1.0).contains(v)));
                    let point = bevy::math::Vec3::from_array(point);
                    bounds[0] = bounds[0].min(point);
                    bounds[1] = bounds[1].max(point);
                }
            }
        }
        assert_eq!(bounds, [bevy::math::Vec3::ZERO, bevy::math::Vec3::ONE]);
    }
}

#[test]
fn definitions_reject_duplicate_air_and_unsafe_paths() {
    let valid = library();
    let mut invalid = valid.clone();
    invalid.blocks[0].id = 0;
    assert!(invalid.validate().is_err());
    invalid = valid.clone();
    invalid.blocks[0].id = invalid.blocks[1].id;
    assert!(invalid.validate().is_err());
    invalid = valid.clone();
    invalid.blocks[0].model = "../outside.glb".into();
    assert!(invalid.validate().is_err());
    invalid = valid.clone();
    invalid.blocks[0].bevel_width = 0.5;
    assert!(invalid.validate().is_err());
    invalid = valid.clone();
    invalid.blocks[0]
        .textures
        .insert("Bad".into(), "../outside.png".into());
    assert!(invalid.validate().is_err());
    invalid = valid;
    invalid
        .blocks
        .iter_mut()
        .find(|block| !block.details.is_empty())
        .unwrap()
        .details[0]
        .probability = f32::NAN;
    assert!(invalid.validate().is_err());
}

#[test]
fn shipped_ids_and_face_materials_stay_explicit_without_cube_models() {
    let definitions = library();
    assert_eq!(
        definitions.blocks.iter().map(|b| b.id).collect::<Vec<_>>(),
        (1..=terra_voxel::MaterialId::LAST).collect::<Vec<_>>()
    );
    for block in &definitions.blocks {
        if matches!(block.id, 1 | 28..=29 | 40 | 59..=73 | 99..=100 | 139..=172)
            || terra_core::castle::is_castle(block.id)
            || terra_core::lantern::is_part(block.id)
            || terra_core::rails::is_rail(block.id)
            || block.id == terra_voxel::MaterialId::RailSwitchStand as u16
        {
            assert_eq!(block.render_kind, BlockRenderKind::CustomMesh);
            assert!(!block.model.is_empty());
            block.validate().expect("static assembly definition");
            if block.id == terra_core::crusher::MATERIAL {
                assert!(block.gameplay.struct_block);
                assert_eq!(block.model, "models/crusher.gltf");
                assert!(!block.collision_boxes.is_empty());
                assert!(
                    block.collision_boxes.iter().all(|bounds| {
                        (0..3).all(|axis| {
                            bounds.minimum[axis].is_finite()
                                && bounds.maximum[axis].is_finite()
                                && bounds.minimum[axis] >= 0.0
                                && bounds.maximum[axis] <= 1.0
                                && bounds.minimum[axis] < bounds.maximum[axis]
                        })
                    }),
                    "the compact crusher collision must fit one complete cell"
                );
            }
            continue;
        }
        assert_eq!(
            block.render_kind,
            if terra_core::construction::is_shape(block.id)
                || matches!(block.id, 98 | 102)
                || terra_core::textiles::is_carpet(block.id)
                || block.id == terra_core::flower_pot::MATERIAL
            {
                BlockRenderKind::Boxes
            } else if (50..=56).contains(&block.id) {
                BlockRenderKind::Segment
            } else {
                BlockRenderKind::Cube
            }
        );
        assert!(
            block.model.is_empty(),
            "Cube definitions must not reference GLBs"
        );
        block.validate().expect("cube geometry requires no GLB");
    }
    let grass = &definitions.blocks[0];
    assert_eq!(grass.model, "models/fiverr_grass_faces.glb");
    assert_eq!(
        grass.faces,
        ["West", "East", "Down", "Up", "North", "South"]
    );
    assert_eq!(
        grass.coarse_faces.as_ref().unwrap(),
        &[
            "WestLod", "EastLod", "DownLod", "UpLod", "NorthLod", "SouthLod"
        ]
    );
    assert!(
        grass.face_materials.is_none() && grass.materials.is_empty() && grass.side_cap.is_none()
    );
    let snow_faces = definitions.blocks[4]
        .face_materials
        .as_ref()
        .expect("snow faces");
    assert!(snow_faces.iter().all(|material| material == "Soft snow"));
    assert!(definitions.blocks[4].side_cap.is_none());
}

#[test]
fn textile_and_pot_definitions_keep_their_explicit_shapes_palette_and_standard_material_pipeline() {
    use super::definitions::{CubeSurface, TexturePattern};
    use terra_core::{beds::DyeColor, textiles};

    let definitions = library();
    let block = |id| {
        definitions
            .blocks
            .iter()
            .find(|block| block.id == id)
            .unwrap()
    };
    for color in DyeColor::ALL {
        let carpet = block(textiles::dyed(textiles::WHITE_CARPET, color).unwrap());
        assert_eq!(carpet.render_kind, BlockRenderKind::Boxes);
        assert!(carpet.gameplay.struct_block && carpet.model.is_empty());
        assert!(carpet.light.is_none());
        assert_eq!(carpet.collision_boxes.len(), 1);
        assert_eq!(carpet.collision_boxes[0].minimum, [0.0; 3]);
        assert_eq!(carpet.collision_boxes[0].maximum, [1.0, 0.125, 1.0]);
        assert_eq!(carpet.materials["Woven wool"].base_color, color.tint());
        let wool = block(textiles::dyed(textiles::WHITE_WOOL, color).unwrap());
        assert_eq!(wool.render_kind, BlockRenderKind::Cube);
        assert!(matches!(wool.surface, Some(CubeSurface::Wool)));
        assert!(wool.collision_boxes.is_empty() && wool.model.is_empty());
        assert!(wool.light.is_none());
        assert_eq!(
            wool.materials["Woven wool"].base_color,
            if color == DyeColor::White {
                [1.0; 4]
            } else {
                color.tint()
            }
        );
        for textile in [carpet, wool] {
            assert_eq!(textile.materials.len(), 1);
            assert_eq!(
                textile.face_materials.as_ref().unwrap(),
                &std::array::from_fn(|_| "Woven wool".to_owned())
            );
            let material = &textile.materials["Woven wool"];
            assert_eq!(material.texture_pattern, Some(TexturePattern::Wool));
            assert_eq!(material.roughness, 1.0);
            assert_eq!(material.reflectance, 0.08);
            assert_eq!(material.emission, [0.0; 3]);
        }
    }
    let pot = block(terra_core::flower_pot::MATERIAL);
    assert_eq!(pot.render_kind, BlockRenderKind::Boxes);
    assert!(pot.gameplay.struct_block && pot.model.is_empty());
    assert!(pot.light.is_none());
    assert_eq!(pot.materials.len(), 1);
    assert_eq!(
        pot.materials["Fired clay"].base_color,
        [0.54, 0.25, 0.15, 1.0]
    );
    assert_eq!(
        pot.face_materials.as_ref().unwrap(),
        &std::array::from_fn(|_| "Fired clay".to_owned())
    );
    assert_eq!(pot.collision_boxes.len(), 10);
    for bounds in &pot.collision_boxes {
        assert!(bounds.valid());
        assert!(bounds.minimum.into_iter().all(|v| v >= 0.0));
        assert!(bounds.maximum.into_iter().all(|v| v <= 1.0));
        assert!(bounds.maximum[1] <= 0.4);
        assert!(
            !(0..3).all(|axis| {
                let point = [0.5, 0.35, 0.5][axis];
                bounds.minimum[axis] <= point && bounds.maximum[axis] >= point
            }),
            "the flower pot must keep its opening clear"
        );
    }
}

#[test]
fn legacy_cube_rim_width_reuses_the_same_closed_cube_vertex_budget() {
    use super::{SideCap, cube_faces};
    let build = |height| {
        cube_faces(
            0.0,
            [0; 6],
            Some(SideCap {
                material: 1,
                height,
            }),
        )
    };
    let old = build(0.1);
    let updated = build(0.18);
    let count = |faces: &[Vec<super::PrimitiveTemplate>; 6]| {
        faces
            .iter()
            .flatten()
            .map(|p| p.positions.len())
            .sum::<usize>()
    };
    assert_eq!(count(&old), count(&updated));
}

#[test]
fn original_schema_v1_models_remain_custom_meshes() {
    let legacy: BlockDefinition = serde_json::from_str(
        r#"{
        "schema_version": 1, "id": 12, "name": "Legacy authored model",
        "model": "model.glb", "bevel_width": 0.0,
        "textures": { "Surface": "textures/stone.png" }
    }"#,
    )
    .expect("legacy schema");
    assert_eq!(legacy.render_kind, BlockRenderKind::CustomMesh);
    legacy.validate().expect("legacy definition remains valid");
    let mut missing_model = legacy;
    missing_model.model.clear();
    assert!(missing_model.validate().is_err());
    let mut cube = library()
        .blocks
        .into_iter()
        .find(|block| block.render_kind == BlockRenderKind::Cube)
        .unwrap();
    cube.face_materials.as_mut().expect("cube faces")[3] = "Missing cap".into();
    assert!(cube.validate().is_err());
}

#[test]
fn every_ore_uses_closed_cube_host_and_a_valid_separate_shared_crystal_material() {
    let library = library();
    for ore in terra_core::OreKind::ALL {
        let definition = library
            .blocks
            .iter()
            .find(|block| block.id == ore.material())
            .unwrap();
        assert_eq!(definition.render_kind, BlockRenderKind::Cube);
        assert!(definition.model.is_empty());
        assert_eq!(
            definition.face_materials.as_ref().unwrap(),
            &std::array::from_fn(|_| "Host rock".to_owned())
        );
        let crystal = definition.crystal_material.as_ref().unwrap();
        assert_eq!(definition.materials[crystal].base_color, ore.color());
        let mut invalid = definition.clone();
        invalid.crystal_material = Some("missing".into());
        assert!(invalid.validate().is_err());
    }
}

#[test]
fn procedural_cube_geometry_is_closed_finite_and_outward_facing() {
    use bevy::math::Vec3;
    use std::collections::BTreeMap;
    for bevel in [0.0, 0.045, 0.065, 0.095, 0.22, 0.25] {
        for cap in [
            None,
            Some(super::SideCap {
                material: 8,
                height: 0.1,
            }),
        ] {
            let faces = super::cube::cube_faces(bevel, [0, 1, 2, 3, 4, 5], cap);
            let mut edges = BTreeMap::new();
            for (side, primitives) in faces.iter().enumerate() {
                for primitive in primitives {
                    assert_eq!(primitive.positions.len(), primitive.normals.len());
                    assert_eq!(primitive.positions.len(), primitive.uvs.len());
                    for triangle in primitive.indices.chunks_exact(3) {
                        let points = [triangle[0], triangle[1], triangle[2]].map(|index| {
                            let point = primitive.positions[index as usize];
                            assert!(
                                point
                                    .iter()
                                    .all(|p| p.is_finite() && (0.0..=1.0).contains(p))
                            );
                            Vec3::from_array(point)
                        });
                        let normal = (points[1] - points[0]).cross(points[2] - points[0]);
                        assert!(normal.length_squared() > 1e-12);
                        assert!(normal[side / 2] * if side % 2 == 0 { -1.0 } else { 1.0 } > 0.0);
                        let points =
                            points.map(|p| p.to_array().map(|v| (v * 1_000_000.0).round() as i32));
                        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                            let mut edge = [points[a], points[b]];
                            edge.sort();
                            *edges.entry(edge).or_insert(0) += 1;
                        }
                    }
                }
                if side == 3 {
                    assert!(primitives.iter().all(|p| p.material == 3));
                }
            }
            assert!(
                edges.values().all(|&uses| uses == 2),
                "closed cube: bevel={bevel}, cap={cap:?}"
            );
        }
    }
}

#[test]
fn shipped_cubes_have_shared_standalone_textures_and_no_glb_files() {
    use super::definitions::TexturePattern;
    use std::collections::BTreeSet;
    let directory = terra_core::asset_directory().join("terra/blocks");
    let definitions = library();
    let textures: BTreeSet<_> = definitions
        .blocks
        .iter()
        .filter(|block| block.render_kind == BlockRenderKind::Cube)
        .flat_map(|block| block.textures.values())
        .collect();
    assert_eq!(textures.len(), 2, "remaining standalone cube textures");
    assert_eq!(
        textures
            .iter()
            .map(|path| path.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["textures/wood.png", "textures/stone-strata-hd.png"])
    );
    for (id, pattern) in [
        (3, TexturePattern::Stone),
        (4, TexturePattern::Sand),
        (7, TexturePattern::Canopy),
        (8, TexturePattern::Canopy),
        (9, TexturePattern::BirchCanopy),
        (13, TexturePattern::MossyStone),
        (19, TexturePattern::Clay),
        (42, TexturePattern::Gravel),
        (43, TexturePattern::StructureStone),
    ] {
        let block = definitions
            .blocks
            .iter()
            .find(|block| block.id == id)
            .unwrap();
        assert!(
            block
                .materials
                .values()
                .any(|material| material.texture_pattern == Some(pattern)),
            "block {id} must reference its shared procedural {pattern:?} texture"
        );
        assert!(
            block.textures.is_empty(),
            "block {id} must not load stale PNGs"
        );
    }
    let dirt = definitions
        .blocks
        .iter()
        .find(|block| block.id == 2)
        .unwrap();
    assert!(dirt.model.is_empty() && dirt.textures.is_empty());
    assert!(dirt.materials.values().all(|material| {
        material.coarse_face == Some(super::definitions::CoarseFaceMaterial { block: 1, side: 2 })
            && material.texture_pattern.is_none()
    }));
    let stone = definitions
        .blocks
        .iter()
        .find(|block| block.id == 3)
        .unwrap();
    let host_pattern = stone.materials[&stone.face_materials.as_ref().unwrap()[0]].texture_pattern;
    assert_eq!(host_pattern, Some(TexturePattern::Stone));
    for ore in definitions
        .blocks
        .iter()
        .filter(|block| (30..=35).contains(&block.id))
    {
        assert_eq!(
            ore.materials[&ore.face_materials.as_ref().unwrap()[0]].texture_pattern,
            host_pattern
        );
        assert!(
            ore.textures.is_empty(),
            "ore host rock uses the shared stone pattern"
        );
    }
    for texture in textures {
        let png = std::fs::read(directory.join(texture)).expect("standalone PNG");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let width = u32::from_be_bytes(png[16..20].try_into().expect("width"));
        let height = u32::from_be_bytes(png[20..24].try_into().expect("height"));
        assert_eq!(width, height, "standalone square texture: {texture}");
        assert!(
            (256..=2048).contains(&width),
            "texture resolution: {texture}"
        );
    }
    for name in [
        "grass",
        "dirt",
        "stone",
        "sand",
        "snow",
        "wood",
        "leaves",
        "autumn-leaves",
        "silver-leaves",
        "weathered-wood",
    ] {
        assert!(
            !directory.join(format!("{name}.glb")).exists(),
            "obsolete cube geometry must not be regenerated: {name}"
        );
    }
    assert!(directory.join("fixtures/pedestal.gltf").is_file());
}

#[test]
fn new_materials_are_distinct_and_glass_has_only_a_transparent_pane() {
    use super::definitions::TexturePattern;
    let library = library();
    let block = |id| library.blocks.iter().find(|b| b.id == id).unwrap();
    let pattern = |id| block(id).materials.values().find_map(|m| m.texture_pattern);
    assert_eq!(pattern(4), Some(TexturePattern::Sand));
    assert_eq!(pattern(11), Some(TexturePattern::Sandstone));
    assert_eq!(pattern(12), Some(TexturePattern::SmoothStone));
    assert_eq!(pattern(13), Some(TexturePattern::MossyStone));
    assert_eq!(pattern(19), Some(TexturePattern::Clay));
    let glass = block(15);
    let pane = &glass.materials[&glass.face_materials.as_ref().unwrap()[0]];
    assert_eq!(pane.base_color[3], 0.1);
    let frame = &glass.materials[glass.frame_material.as_ref().unwrap()];
    assert_eq!(frame.base_color[3], 1.0);
    assert_ne!(
        block(16).materials.values().next().unwrap().base_color,
        block(17).materials.values().next().unwrap().base_color
    );
    assert_ne!(
        block(17).materials.values().next().unwrap().base_color,
        block(18).materials.values().next().unwrap().base_color
    );
}

#[test]
fn mushroom_caps_have_speckled_colors_and_shared_pale_undersides() {
    use super::definitions::TexturePattern;
    let library = library();
    let stem = library.blocks.iter().find(|block| block.id == 20).unwrap();
    assert_eq!(
        stem.materials["Mushroom stem"].texture_pattern,
        Some(TexturePattern::MushroomStem)
    );
    let colors = [
        TexturePattern::PurpleMushroomCap,
        TexturePattern::RedMushroomCap,
        TexturePattern::YellowMushroomCap,
        TexturePattern::GreenMushroomCap,
        TexturePattern::BlueMushroomCap,
        TexturePattern::WhiteMushroomCap,
    ];
    for (index, color) in colors.into_iter().enumerate() {
        let block = library
            .blocks
            .iter()
            .find(|block| block.id == 21 + index as u16)
            .unwrap();
        let faces = block.face_materials.as_ref().unwrap();
        assert_eq!(block.materials[&faces[3]].texture_pattern, Some(color));
        assert_eq!(
            block.materials[&faces[2]].texture_pattern,
            Some(TexturePattern::MushroomGills)
        );
        for side in [0, 1, 4, 5] {
            assert_eq!(faces[side], faces[3]);
        }
        assert!(block.model.is_empty());
    }
}

#[test]
fn json_directory_discovery_is_stable_and_reports_invalid_files() {
    struct TemporaryDirectory(std::path::PathBuf);
    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let directory = TemporaryDirectory(
        std::env::temp_dir().join(format!("terra-json-definition-test-{}", std::process::id())),
    );
    std::fs::create_dir(&directory.0).expect("unique test directory");
    assert!(BlockLibrary::load(&directory.0).is_err());
    std::fs::write(
        directory.0.join("a.json"),
        include_str!("../../../../assets/terra/blocks/stone.json"),
    )
    .expect("stone JSON");
    std::fs::write(
        directory.0.join("z.json"),
        include_str!("../../../../assets/terra/blocks/grass.json"),
    )
    .expect("grass JSON");
    let loaded = BlockLibrary::load(&directory.0).expect("discovery");
    assert_eq!(
        loaded.blocks.iter().map(|b| b.id).collect::<Vec<_>>(),
        vec![1, 3]
    );
    std::fs::write(directory.0.join("malformed.json"), "{ not JSON }").expect("bad JSON");
    assert!(
        BlockLibrary::load(&directory.0)
            .expect_err("invalid JSON")
            .contains("malformed.json")
    );
    assert!(BlockLibrary::load(&directory.0.join("missing")).is_err());
}
