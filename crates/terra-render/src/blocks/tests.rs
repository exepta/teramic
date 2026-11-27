use super::definitions::{BlockDefinition, BlockLibrary, BlockRenderKind};

fn library() -> BlockLibrary {
    BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks"))
        .expect("shipped JSON block definitions")
}

#[test]
fn all_leaf_blocks_bind_separate_complete_sprites_without_changing_preview_faces() {
    let library = library();
    for id in terra_voxel::MaterialId::FOLIAGE {
        let block = library.blocks.iter().find(|b| b.id == id).unwrap();
        let cluster = block.foliage_cluster.as_ref().unwrap();
        let faces = block.face_materials.as_ref().unwrap();
        assert!(!faces.contains(&cluster.material));
        assert!(block.textures[&cluster.material].ends_with("_cluster.png"));
        assert_eq!(block.materials[&cluster.material].base_color, [1.0; 4]);
        assert_eq!(cluster.needles.is_some(), matches!(id, 7 | 585));
        assert!(block.validate().is_ok());
        let mut missing = block.clone();
        missing.foliage_cluster.as_mut().unwrap().material = "missing sprite".into();
        assert!(missing.validate().is_err());
        let mut nonleaf = block.clone();
        nonleaf.id = 3;
        assert!(
            nonleaf.validate().is_err(),
            "ordinary solid cubes cannot acquire leaf spray geometry"
        );
    }
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
        let bevel = match block.id {
            6 | 10 | 16 | 17 | 18 | 36 | 134 | 135 | 313 | 314 | 404 | 405 => Some(0.025),
            727..=729 => Some(0.018),
            708 | 709 => Some(0.035), // Weathered and carved ruin masonry.
            terra_core::bile::BLOCK | terra_core::street::STRAW => Some(0.022),
            id if terra_core::street::is_shape(id) => Some(0.015),
            _ => None,
        };
        if let Some(width) = bevel {
            assert_eq!(block.bevel_width, width);
            assert_eq!(
                block.surface.is_some(),
                matches!(
                    block.id,
                    terra_core::bile::BLOCK | terra_core::street::STRAW
                )
            );
            assert!(block.materials.values().all(|m| m.normal_texture.is_some()));
            for face in super::cube_faces(block.bevel_width, [0; 6], None) {
                for mesh in face {
                    assert!(mesh.positions.len() <= 36);
                    assert!(
                        mesh.positions
                            .iter()
                            .flatten()
                            .all(|v| (0.0..=1.0).contains(v))
                    );
                    assert!(
                        mesh.normals
                            .iter()
                            .all(|n| (Vec3::from_array(*n).length() - 1.0).abs() < 1e-5)
                    );
                }
            }
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
fn frosted_ice_writes_depth_and_occludes_the_water_below() {
    let definitions = library();
    let ice = definitions.blocks.iter().find(|d| d.id == 49).unwrap();
    assert_eq!(ice.render_kind, BlockRenderKind::Cube);
    assert!(ice.model.is_empty() && ice.light.is_none());
    assert!(ice.frame_material.is_none() && ice.collision_boxes.is_empty());
    assert!(terra_voxel::MaterialId::Ice.is_solid());
    assert!(terra_voxel::MaterialId::Ice.is_opaque());
    assert_eq!(terra_voxel::MaterialId::Ice.identifier(), "teramic::ice");
    let material = ice.materials.values().next().unwrap();
    for id in [49, 81, 89, 97, 477, 478, 479] {
        let ice = definitions
            .blocks
            .iter()
            .find(|block| block.id == id)
            .unwrap();
        assert!(
            ice.materials.values().all(|face| face.base_color[3] == 1.0),
            "{}",
            ice.name
        );
    }
    assert!(material.roughness >= 0.3);
    assert_eq!(material.emission, [0.0; 3]);
    assert_eq!(
        material.texture_pattern,
        Some(super::definitions::TexturePattern::Ice)
    );
}

#[test]
fn regional_plants_are_orientable_noncolliding_scenes_and_cactus_uses_three_faces() {
    let definitions = library();
    for id in terra_core::biome_flora::FIRST_PROP..=terra_core::biome_flora::LAST_PROP {
        let block = definitions.blocks.iter().find(|d| d.id == id).unwrap();
        assert_eq!(block.render_kind, BlockRenderKind::CustomMesh);
        assert_eq!(block.model_layout, super::definitions::ModelLayout::Scene);
        assert!(!block.model.is_empty());
        assert!(block.gameplay.struct_block);
        assert!(block.collision_boxes.is_empty() && block.light.is_none());
        assert!(!terra_voxel::MaterialId::from_raw(id).unwrap().is_solid());
    }
    let cactus = definitions
        .blocks
        .iter()
        .find(|d| d.id == terra_core::biome_flora::CACTUS)
        .unwrap();
    assert_eq!(cactus.render_kind, BlockRenderKind::Cube);
    assert!(cactus.model.is_empty() && cactus.light.is_none());
    assert_eq!(cactus.materials.len(), 3);
    assert_eq!(cactus.textures.len(), 3);
    let faces = cactus.face_materials.as_ref().unwrap();
    assert_eq!(faces[0], faces[1]);
    assert_eq!(faces[0], faces[4]);
    assert_eq!(faces[0], faces[5]);
    assert_ne!(faces[2], faces[3]);
    assert_ne!(faces[0], faces[3]);
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
fn spirit_and_blood_surfaces_cut_inward_and_preserve_every_shared_cube_corner() {
    use bevy::math::Vec3;
    let definitions = library();
    for id in [
        306, 307, 308, 313, 314, 315, 400, 401, 402, 403, 404, 405, 407, 408, 409,
    ] {
        let block = definitions
            .blocks
            .iter()
            .find(|block| block.id == id)
            .unwrap();
        assert_eq!(block.render_kind, BlockRenderKind::Cube);
        if [313, 314, 404, 405].contains(&id) {
            assert_eq!(block.bevel_width, 0.025);
            assert!(block.surface.is_none());
            assert!(block.materials.values().all(|m| m.normal_texture.is_some()));
            continue;
        }
        assert_eq!(block.bevel_width, 0.0);
        assert!(block.model.is_empty());
        let template = super::surface::SurfaceTemplate::new(block.surface.unwrap());
        let faces = super::cube_faces(0.0, [0; 6], None);
        let mut inward_vertices = 0;
        for (side, source) in faces.iter().enumerate() {
            let patches = template.face(source, side, 0, 0, |p| p);
            let points: Vec<_> = patches
                .iter()
                .flat_map(|p| &p.primitive.positions)
                .collect();
            for point in &points {
                assert!(point.iter().all(|v| (0.0..=1.0).contains(v)), "block {id}");
                let plane = (side % 2) as f32;
                inward_vertices += usize::from((point[side / 2] - plane).abs() > 0.00001);
            }
            // The contour stays exactly on the shared voxel face: relief cannot
            // expose cracks where two cubes or independently streamed chunks meet.
            for primitive in source {
                for &corner in &primitive.positions {
                    if corner.iter().all(|v| *v == 0.0 || *v == 1.0) {
                        assert!(
                            points
                                .iter()
                                .any(|p| Vec3::from_array(**p) == Vec3::from_array(corner)),
                            "block {id}, side {side}"
                        );
                    }
                }
            }
        }
        assert!(inward_vertices > 0, "block {id} must have visible relief");
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
        if matches!(
            block.id,
            1 | 28..=29 | 40 | 59..=73 | 99..=100 | 139..=172 | 316..=323 | 344..=396 | 412..=421
        ) || (459..=463).contains(&block.id)
            || matches!(block.id, 467 | 468 | 483..=487 | 489..=491 | 496..=503 | 508..=511 | 526 | 545..=548 | 575..=579)
            || terra_core::castle::is_castle(block.id)
            || terra_core::creatures::dragon_eggs::kind(block.id).is_some()
            || matches!(
                terra_voxel::MaterialId::from_raw(block.id),
                Some(terra_voxel::MaterialId::AmberGeode | terra_voxel::MaterialId::TideGeode)
            )
            || terra_core::biome_flora::is_prop(block.id)
            || (770..=773).contains(&block.id)
            || (terra_voxel::MaterialId::VioletTorch as u16
                ..=terra_voxel::MaterialId::TempleCrystalCluster as u16)
                .contains(&block.id)
            || terra_core::lantern::is_part(block.id)
            || terra_core::rails::is_rail(block.id)
            || terra_core::train_parts::is_train(block.id)
            || matches!(block.id, terra_core::roads::SLOPE..=terra_core::roads::EXIT)
            || terra_core::profession_stations::is_station(block.id)
            || block.id == terra_voxel::MaterialId::RunePillar as u16
            || block.id == terra_voxel::MaterialId::NestStraw as u16
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
                || block.id == terra_core::bile::FLUID
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
    assert_eq!(grass.model, "models/fiverr_grass_reference_base.glb");
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
fn shipped_cubes_have_standalone_textures_without_runtime_glb_dependencies() {
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
    let spirit_textures = BTreeSet::from([
        "textures/spirit_blossom_grass.png",
        "textures/spirit_blossom_grass_bottom.png",
        "textures/spirit_blossom_grass_side.png",
        "textures/spirit_blossom_dirt.png",
        "textures/spirit_stone.png",
        "textures/blossom_petal_block.png",
        "textures/spirit_blossom_log.png",
        "textures/spirit_blossom_log_end.png",
        "textures/spirit_blossom_planks.png",
        "textures/glowing_lantern_stone.png",
    ]);
    let foliage_textures: BTreeSet<String> = [
        "autumn",
        "birch",
        "cherry_pink",
        "cherry_white",
        "apple",
        "spirit_pink",
        "spirit_pearl",
        "spirit_lavender",
        "spirit_teal",
        "spirit_moonlit",
        "blood",
        "snowy_spruce_leaves",
        "winter_birch_branches",
        "acacia",
        "baobab",
        "willow_leaves",
    ]
    .into_iter()
    .flat_map(|style| {
        ["side", "top", "cluster"].map(move |face| format!("textures/foliage/{style}_{face}.png"))
    })
    .chain(["oak", "spruce"].map(|style| format!("textures/foliage/{style}_cluster.png")))
    .collect();
    let cemetery_textures: BTreeSet<String> = (324..=343)
        .chain([397, 398, 399])
        .map(|id| {
            let slug = terra_voxel::MaterialId::from_raw(id)
                .unwrap()
                .identifier()
                .strip_prefix("teramic::")
                .unwrap();
            format!("textures/{slug}.png")
        })
        .collect();
    let blood_textures: BTreeSet<String> = [
        "blood_grass",
        "blood_grass_bottom",
        "blood_grass_side",
        "blood_dirt",
        "blood_stone",
        "veined_stone",
        "bloodwood_planks",
        "bloodwood_log",
        "bloodwood_log_end",
        "crimson_mycelium",
        "crimson_mycelium_bottom",
        "crimson_mycelium_side",
        "blood_moss",
        "blood_moss_bottom",
        "blood_moss_side",
        "blood_crystal_block",
        "blood_fungal_block",
        "blood_glow_mushroom_block",
        "blighted_root_soil",
        "coagulated_mud",
        "bloodstained_bricks",
        "carved_blood_stone",
    ]
    .into_iter()
    .map(|name| format!("textures/{name}.png"))
    .collect();
    let park_textures = [
        "stem_side",
        "scarlet_top",
        "cream_top",
        "gill_side",
        "chestnut_top",
        "chestnut_side",
        "amethyst_top",
        "amethyst_side",
        "mycelium_top",
        "mycelium_side",
        "spore_top",
        "spore_side",
        "shingles_top",
        "shingles_side",
        "timber_top",
        "timber_side",
    ]
    .map(|name| format!("textures/mushroom_park/{name}.png"));
    let salt_textures: BTreeSet<String> = [
        "base", "deep", "crust", "compact", "wet", "dust", "layer", "clear",
    ]
    .into_iter()
    .flat_map(|swatch| {
        ["top", "side"].map(move |face| format!("textures/salt_cave/{swatch}_{face}.png"))
    })
    .chain(["textures/salt_cave/accent_side.png".to_owned()])
    .collect();
    let karst_textures: BTreeSet<String> = ["wet", "calcite", "dripstone", "moss", "mud"]
        .into_iter()
        .flat_map(|tile| {
            ["side", "top"].map(move |side| format!("textures/dripstone_cave/{tile}_{side}.png"))
        })
        .collect();
    let living_textures = [
        "rock",
        "overgrown",
        "roots",
        "turf",
        "moss",
        "glow",
        "clear",
        "packed",
        "rime",
        "frozen",
        "dust",
        "azure",
    ]
    .into_iter()
    .flat_map(|swatch| {
        ["top", "side"].map(move |face| format!("textures/living_caves/{swatch}_{face}.png"))
    })
    .collect::<BTreeSet<_>>();
    let savanna_textures: BTreeSet<String> = (632..=647)
        .filter(|&id| !matches!(id, 644 | 647))
        .flat_map(|id| {
            let slug = terra_voxel::MaterialId::from_raw(id)
                .unwrap()
                .identifier()
                .trim_start_matches("teramic::");
            let faces: &[&str] = match id {
                632 | 633 => &["side", "top"],
                643 | 646 => &["side", "end"],
                _ => &["side"],
            };
            faces
                .iter()
                .map(move |face| format!("textures/savanna/{slug}_{face}.png"))
        })
        .collect();
    let swamp_textures: BTreeSet<String> = (670..=681)
        .flat_map(|id| {
            let slug = terra_voxel::MaterialId::from_raw(id)
                .unwrap()
                .identifier()
                .trim_start_matches("teramic::");
            let faces: &[&str] = match id {
                670 => &["side", "top"],
                679 | 680 => &["side", "end"],
                _ => &["side"],
            };
            faces
                .iter()
                .map(move |face| format!("textures/swamp/{slug}_{face}.png"))
        })
        .collect();
    let relic_textures: Vec<String> = [
        "fossiliferous_stone",
        "turquoise_veined_stone",
        "root_bound_stone",
        "temple_architrave",
        "scoria_bricks",
        "pyric_rune_stone",
        "volcanic_obsidian",
        "geyser_vent",
    ]
    .map(|slug| format!("textures/relics/{slug}.png"))
    .to_vec();
    let crystal_textures: Vec<String> = (739..=749)
        .chain([759])
        .map(|id| {
            let slug = terra_voxel::MaterialId::from_raw(id)
                .unwrap()
                .identifier()
                .trim_start_matches("teramic::");
            format!("textures/crystal_highlands/{slug}.png")
        })
        .collect();
    let coastal_textures: Vec<_> = (774..=784)
        .map(|id| {
            let slug = terra_voxel::MaterialId::from_raw(id)
                .unwrap()
                .identifier()
                .trim_start_matches("teramic::");
            format!("textures/misty_coast/{slug}.png")
        })
        .collect();
    let petrified_textures: Vec<String> = [
        "dusty_ground",
        "dusty_ground_side",
        "cracked_stone_soil",
        "petrified_log_side",
        "petrified_log_end",
        "petrified_planks",
        "mineral_encrusted_stone",
        "weathered_ruin_stone",
        "carved_temple_stone",
        "petrified_sapwood",
        "petrified_heartwood",
    ]
    .map(|slug| format!("textures/petrified_forest/{slug}.png"))
    .into();
    let reference_textures: BTreeSet<String> =
        ["oak", "birch", "spruce", "blood", "ghost", "sakura"]
            .into_iter()
            .flat_map(|species| {
                ["bark", "end", "planks"]
                    .map(move |face| format!("textures/block_palette/{species}_{face}.png"))
            })
            .chain(
                ["stone", "sand", "gravel", "cobble"]
                    .map(|name| format!("textures/block_palette/{name}.png")),
            )
            .collect();
    assert_eq!(
        textures
            .iter()
            .map(|path| path.as_str())
            .collect::<BTreeSet<_>>(),
        spirit_textures
            .iter()
            .copied()
            .chain(cemetery_textures.iter().map(String::as_str))
            .chain(blood_textures.iter().map(String::as_str))
            .chain(park_textures.iter().map(String::as_str))
            .chain(savanna_textures.iter().map(String::as_str))
            .chain(swamp_textures.iter().map(String::as_str))
            .chain(petrified_textures.iter().map(String::as_str))
            .chain(relic_textures.iter().map(String::as_str))
            .chain(crystal_textures.iter().map(String::as_str))
            .chain(coastal_textures.iter().map(String::as_str))
            .chain([
                "textures/ashlands/ash_side.png",
                "textures/ashlands/charred_log_end.png",
                "textures/ashlands/charred_log_side.png",
                "textures/ashlands/charred_planks_side.png",
                "textures/ashlands/columnar_basalt_end.png",
                "textures/ashlands/columnar_basalt_side.png",
                "textures/ashlands/compacted_ash_side.png",
                "textures/ashlands/dark_volcanic_stone_side.png",
                "textures/ashlands/ember_crystal_ore_side.png",
                "textures/ashlands/fumarole_top.png",
                "textures/ashlands/lava_crust_side.png",
                "textures/ashlands/lava_side.png",
                "textures/ashlands/scorched_dirt_side.png",
                "textures/ashlands/volcanic_gravel_side.png",
                "textures/sakura/petal_turf_side.png",
                "textures/sakura/petal_turf_top.png",
                "textures/sakura/petal_turf_bottom.png",
                "textures/sakura/blossom_stone_side.png",
                "textures/sakura/blossom_stone_top.png",
                "textures/sakura/mossy_blossom_stone_side.png",
                "textures/sakura/mossy_blossom_stone_top.png",
                "textures/ocean/wet_sand.png",
                "textures/ocean/marine_limestone.png",
                "textures/ocean/reef_stone.png",
                "textures/ocean/sea_basalt.png",
                "textures/highlands/firn_side.png",
                "textures/highlands/firn_top.png",
                "textures/highlands/icy_snow_side.png",
                "textures/highlands/icy_snow_top.png",
                "textures/highlands/alpine_granite_side.png",
                "textures/highlands/alpine_granite_top.png",
                "textures/highlands/snow_covered_sod_side.png",
                "textures/highlands/snow_covered_sod_top.png",
                "textures/plains/bound_hay_bale_end.png",
                "textures/plains/bound_hay_bale_surface.png",
                "textures/plains/meadow_limestone_surface.png",
                "textures/plains/blue_slate_shingles_surface.png",
                "textures/forest/woodland_trail_surface.png",
                "textures/forest/woodland_trail_side.png",
                "textures/forest/woodland_litter_surface.png",
                "textures/forest/woodland_litter_side.png",
                "textures/desert/dune_sand_top.png",
                "textures/desert/dune_sand_side.png",
                "textures/desert/red_dune_sand_top.png",
                "textures/desert/red_dune_sand_side.png",
                "textures/desert/banded_sandstone_top.png",
                "textures/desert/banded_sandstone_side.png",
                "textures/desert/carved_sunstone_surface.png",
                "textures/desert/oasis_mosaic_surface.png",
            ])
            .chain(foliage_textures.iter().map(String::as_str))
            .chain(reference_textures.iter().map(String::as_str))
            .chain(salt_textures.iter().map(String::as_str))
            .chain(karst_textures.iter().map(String::as_str))
            .chain(living_textures.iter().map(String::as_str))
            .chain([
                "textures/cactus_side.png",
                "textures/cactus_top.png",
                "textures/cactus_bottom.png"
            ])
            .chain([
                "textures/wood.png",
                "textures/stone-strata-hd.png",
                "textures/reference_base/snow.png",
                "textures/dirt.png", // Soil beneath snow-covered sod.
                "textures/sculpted/leaves.png",
                "textures/asphalt.png",
                "textures/street/bile.png",
                "textures/street/straw.png",
            ])
            .filter(|path| !matches!(
                *path,
                "textures/wood.png"
                    | "textures/spirit_blossom_log.png"
                    | "textures/spirit_blossom_log_end.png"
                    | "textures/spirit_blossom_planks.png"
                    | "textures/bloodwood_log.png"
                    | "textures/bloodwood_log_end.png"
                    | "textures/bloodwood_planks.png"
            ))
            .collect::<BTreeSet<_>>()
    );
    for (id, texture) in [(3, "stone"), (4, "sand"), (13, "cobble"), (42, "gravel")] {
        let block = definitions.blocks.iter().find(|b| b.id == id).unwrap();
        let face = &block.face_materials.as_ref().unwrap()[0];
        assert!(block.materials[face].texture_pattern.is_none());
        assert_eq!(
            block.textures[face],
            format!("textures/block_palette/{texture}.png")
        );
        assert!(block.materials[face].normal_texture.is_some());
    }
    for (id, pattern) in [
        (19, TexturePattern::Clay),
        (43, TexturePattern::StructureStone),
    ] {
        let block = definitions.blocks.iter().find(|b| b.id == id).unwrap();
        assert!(
            block
                .materials
                .values()
                .any(|m| m.texture_pattern == Some(pattern))
        );
        assert!(block.textures.is_empty());
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
    let stone_face = &stone.face_materials.as_ref().unwrap()[0];
    for ore in definitions
        .blocks
        .iter()
        .filter(|block| (30..=35).contains(&block.id))
    {
        let host = &ore.face_materials.as_ref().unwrap()[0];
        if ore.id == terra_voxel::MaterialId::SaltOre as u16 {
            assert_eq!(ore.textures[host], "textures/salt_cave/accent_side.png");
            assert_eq!(
                ore.materials[host].normal_texture.as_deref(),
                Some("textures/salt_cave/accent_side_normal.png")
            );
        } else {
            assert_eq!(ore.materials[host], stone.materials[stone_face]);
            assert_eq!(ore.textures[host], stone.textures[stone_face]);
        }
    }
    for texture in textures {
        let png = std::fs::read(directory.join(texture)).expect("standalone PNG");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let width = u32::from_be_bytes(png[16..20].try_into().expect("width"));
        let height = u32::from_be_bytes(png[20..24].try_into().expect("height"));
        assert_eq!(width, height, "standalone square texture: {texture}");
        if foliage_textures.contains(texture.as_str()) {
            assert_eq!(width, 512, "authored foliage texture resolution: {texture}");
        } else if texture.as_str() == "textures/sculpted/leaves.png" {
            assert_eq!(width, 384, "sculpted canopy reference tile");
        } else if spirit_textures.contains(texture.as_str())
            || blood_textures.contains(texture.as_str())
            || park_textures.contains(texture)
            || texture.starts_with("textures/ocean/")
            || texture.starts_with("textures/highlands/")
            || texture.starts_with("textures/ashlands/")
            || texture.starts_with("textures/savanna/")
            || texture.starts_with("textures/swamp/")
            || texture.starts_with("textures/petrified_forest/")
            || texture.starts_with("textures/relics/")
            || texture.starts_with("textures/crystal_highlands/")
            || texture.starts_with("textures/sakura/")
        {
            assert_eq!(width, 256, "authored biome texture resolution: {texture}");
        } else if cemetery_textures.contains(texture.as_str())
            || salt_textures.contains(texture.as_str())
            || karst_textures.contains(texture.as_str())
            || living_textures.contains(texture.as_str())
        {
            assert_eq!(
                width, 128,
                "authored mineral / masonry texture resolution: {texture}"
            );
        } else {
            assert!(
                (256..=2048).contains(&width),
                "texture resolution: {texture}"
            );
        }
    }
    for block in definitions
        .blocks
        .iter()
        .filter(|block| block.render_kind == BlockRenderKind::Cube)
    {
        assert!(block.model.is_empty(), "cube {} loads no GLB", block.id);
        if (306..=315).contains(&block.id)
            || (324..=343).contains(&block.id)
            || matches!(block.id, 397..=399)
        {
            // Portable authoring references exist, but the renderer still uses
            // shared closed cube geometry and the explicit per-face textures.
            let slug = terra_voxel::MaterialId::from_raw(block.id)
                .unwrap()
                .identifier()
                .strip_prefix("teramic::")
                .unwrap();
            let reference = std::fs::read(directory.join(format!("models/{slug}.glb")))
                .expect("Spirit cube authoring reference");
            assert_eq!(&reference[..4], b"glTF");
            assert_eq!(u32::from_le_bytes(reference[4..8].try_into().unwrap()), 2);
        }
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
    assert_eq!(pattern(4), None);
    assert!(block(4).textures.values().any(|p| p.ends_with("/sand.png")));
    assert_eq!(pattern(11), Some(TexturePattern::Sandstone));
    assert_eq!(pattern(12), Some(TexturePattern::SmoothStone));
    assert_eq!(pattern(13), None);
    assert!(
        block(13)
            .textures
            .values()
            .any(|p| p.ends_with("/cobble.png"))
    );
    assert_eq!(pattern(19), Some(TexturePattern::Clay));
    let glass = block(15);
    let pane = &glass.materials[&glass.face_materials.as_ref().unwrap()[0]];
    assert_eq!(pane.base_color[3], 0.1);
    let frame = &glass.materials[glass.frame_material.as_ref().unwrap()];
    assert_eq!(frame.base_color[3], 1.0);
    assert_ne!(block(16).textures, block(17).textures);
    assert_ne!(block(17).textures, block(18).textures);
    for id in [16, 17, 18] {
        assert!(
            block(id)
                .materials
                .values()
                .all(|m| m.base_color == [1.0; 4])
        );
    }
}

#[test]
fn mushroom_caps_have_speckled_colors_and_shared_pale_undersides() {
    use super::definitions::TexturePattern;
    let library = library();
    let stem = library.blocks.iter().find(|block| block.id == 20).unwrap();
    assert_eq!(
        stem.textures["Mushroom stem"],
        "textures/mushroom_park/stem_side.png"
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
        match block.id {
            22 => assert_eq!(
                block.textures[&faces[3]],
                "textures/mushroom_park/scarlet_top.png"
            ),
            26 => assert_eq!(
                block.textures[&faces[3]],
                "textures/mushroom_park/cream_top.png"
            ),
            _ => assert_eq!(block.materials[&faces[3]].texture_pattern, Some(color)),
        }
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
    let mut custom: serde_json::Value =
        serde_json::from_str(include_str!("../../../../assets/terra/blocks/stone.json")).unwrap();
    custom["id"] = 60000.into();
    custom["name"] = "Custom stone".into();
    std::fs::write(directory.0.join("custom.json"), custom.to_string()).unwrap();
    let loaded = BlockLibrary::load(&directory.0).expect("discovery");
    assert_eq!(
        loaded.blocks.iter().map(|b| b.id).collect::<Vec<_>>(),
        vec![1, 3, 60000]
    );
    assert_eq!(loaded.by_id(60000).unwrap().name, "Custom stone");
    assert!(loaded.by_id(59999).is_none());
    std::fs::write(directory.0.join("malformed.json"), "{ not JSON }").expect("bad JSON");
    assert!(
        BlockLibrary::load(&directory.0)
            .expect_err("invalid JSON")
            .contains("malformed.json")
    );
    assert!(BlockLibrary::load(&directory.0.join("missing")).is_err());
}

#[test]
fn fog_block_is_solid_mineable_and_spirit_stone_does_not_self_illuminate() {
    let library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    let fog = library
        .blocks
        .iter()
        .find(|b| b.id == terra_core::fog_blocks::MATERIAL)
        .unwrap();
    assert_eq!(fog.name, "Fog Block");
    assert!(terra_voxel::MaterialId::FogBlock.is_solid());
    assert!(terra_voxel::MaterialId::FogBlock.is_opaque());
    assert!(terra_core::ItemKind::Block(430).valid());
    for id in [308, 430] {
        let block = library.blocks.iter().find(|b| b.id == id).unwrap();
        assert!(
            block.light.is_none(),
            "ordinary stone must not create a cave light"
        );
        for material in block.materials.values() {
            assert_eq!(material.emission, [0.0; 3]);
        }
    }
}
