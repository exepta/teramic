use super::*;

#[test]
fn player_placed_road_meshes_rise_in_look_direction_and_match_collision_after_real_proposal() {
    use crate::meshing::{builder::RenderOptions, custom};
    use terra_core::{ItemKind, block_properties::BlockPropertiesCatalog, roads};
    use terra_voxel::{BlockSection, SectionPos, meshing::BlockMesh};
    use terra_world::{
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection},
    };
    let library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    for (id, path) in [
        (roads::SLOPE, Some("terra/blocks/models/asphalt_slope.gltf")),
        (roads::ENTRY, Some("terra/blocks/models/asphalt_entry.gltf")),
        (roads::EXIT, Some("terra/blocks/models/asphalt_exit.gltf")),
        (roads::CURB, None),
    ] {
        let definition = library.blocks.iter().find(|b| b.id == id).unwrap();
        let model = path.map_or_else(
            || crate::blocks::cube::box_model(&definition.collision_boxes, 0.0, [0; 6]),
            |path| load_static_scene(path, id),
        );
        let mut templates = TemplateSet::default();
        templates.blocks.insert(
            3,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::Cube,
                bevel_width: 0.0,
                faces: crate::blocks::cube_faces(0.0, [1; 6], None),
                face_materials: [1; 6],
                side_cap: None,
                details: Vec::new(),
                model: Vec::new(),
            },
        );
        templates.blocks.insert(
            id,
            BlockTemplate {
                surface: None,
                render_kind: definition.render_kind,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model,
            },
        );
        for heading in [
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [-1.0, 0.0, 0.0],
        ] {
            let owner = SectionPos::new(-1, 0, -1);
            let position = [-8, 8, -8];
            let mut section = BlockSection::empty(owner).unwrap();
            section.set_block([8, 7, 8], 3).unwrap();
            let mut chunks = ChunkStore::default();
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data: section,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            let mut journal = EditJournal::default();
            let properties = BlockPropertiesCatalog::default();
            let direction = [heading[0] * 0.8, -0.6, heading[2] * 0.8];
            let point = [-7.5, 8.0, -7.5];
            let origin = std::array::from_fn(|i| point[i] - direction[i] * 3.0);
            let hit = terra_world::interaction_shapes::raycast(
                &chunks,
                &journal,
                &properties,
                origin,
                direction,
                6.0,
            )
            .unwrap();
            assert_eq!(hit.position, [-8, 7, -8]);
            let yaw = (-heading[0]).atan2(-heading[2]) as f32;
            let plan = terra_world::placement::propose(
                &chunks,
                &journal,
                hit,
                ItemKind::Block(id),
                (origin, direction),
                [yaw, (-0.6_f32).asin()],
            )
            .unwrap();
            let collision = terra_world::placement::boxes(&plan, &chunks, &journal, &properties);
            for edit in &plan.edits {
                terra_world::streaming::commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    edit,
                    &properties,
                )
                .unwrap();
            }
            for &(cell, state) in &plan.states {
                terra_world::streaming::commit_object_state(&mut chunks, &mut journal, cell, state)
                    .unwrap();
            }
            let snapshot =
                custom::capture(&chunks.loaded[&owner].data, &templates, &journal, |cell| {
                    chunks.block_at(cell)
                })
                .unwrap();
            let batches = custom::compose(
                &BlockMesh::default(),
                &snapshot,
                &templates,
                owner,
                &RenderOptions {
                    water_level: None,
                    detail_level: crate::meshing::DetailLevel::Near,
                    terrain: None,
                    exposure: None,
                    tints: Default::default(),
                    conditions: Default::default(),
                },
            )
            .unwrap();
            // The compositor also restores exposed support-stone surface next
            // to a partial road. Keep that real geometry in a separate material
            // batch so only the placed road is compared with its own collider.
            assert!(
                batches
                    .iter()
                    .any(|batch| batch.material == 1 && !batch.positions.is_empty())
            );
            let section_origin = owner.origin().unwrap();
            let points: Vec<[f64; 3]> = batches
                .iter()
                .filter(|batch| batch.material == 0)
                .flat_map(|batch| &batch.positions)
                .map(|p| std::array::from_fn(|i| f64::from(p[i]) + section_origin[i] as f64))
                .collect();
            assert!(!points.is_empty());
            assert!(
                points
                    .iter()
                    .all(|p| collision.iter().any(|b| (0..3)
                        .all(|i| p[i] >= b.minimum[i] - 1e-5 && p[i] <= b.maximum[i] + 1e-5))),
                "render/collision mismatch for {id} toward {heading:?}"
            );
            if id == roads::CURB {
                let axis = if heading[0] == 0.0 { 2 } else { 0 };
                let extent = |axis: usize| {
                    points
                        .iter()
                        .map(|p| p[axis])
                        .fold(f64::NEG_INFINITY, f64::max)
                        - points.iter().map(|p| p[axis]).fold(f64::INFINITY, f64::min)
                };
                assert!((extent(axis) - 1.0).abs() < 1e-5);
                assert!((extent(2 - axis) - 0.1875).abs() < 1e-5);
            } else {
                let end_height = |sign| {
                    points
                        .iter()
                        .filter(|p| {
                            ((p[0] - position[0] as f64 - 0.5) * heading[0]
                                + (p[2] - position[2] as f64 - 0.5) * heading[2])
                                * sign
                                > 0.49
                        })
                        .map(|p| p[1])
                        .fold(f64::NEG_INFINITY, f64::max)
                };
                assert!(
                    end_height(1.0) > end_height(-1.0) + 0.3,
                    "rendered ramp rises toward the player for {id}/{heading:?}"
                );
            }
        }
    }
}

#[test]
fn road_ramps_load_real_continuous_textured_meshes_with_shared_pickable_collision() {
    use terra_core::roads;
    let library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    for (id, path, low, high) in [
        (
            roads::SLOPE,
            "terra/blocks/models/asphalt_slope.gltf",
            0.0,
            1.0,
        ),
        (
            roads::ENTRY,
            "terra/blocks/models/asphalt_entry.gltf",
            0.125,
            0.5,
        ),
        (
            roads::EXIT,
            "terra/blocks/models/asphalt_exit.gltf",
            0.5,
            1.0,
        ),
    ] {
        let definition = library.blocks.iter().find(|b| b.id == id).unwrap();
        definition.validate().unwrap();
        assert_eq!(definition.collision_boxes, roads::boxes(id));
        assert_eq!(
            definition.textures["Road aggregate"],
            "textures/asphalt.png"
        );
        let model = load_static_scene(path, id);
        assert!(!model.is_empty());
        let mut top = 0;
        for primitive in &model {
            assert_eq!(primitive.positions.len(), primitive.uvs.len());
            for triangle in primitive.indices.chunks_exact(3) {
                let [a, b, c] = std::array::from_fn(|i| {
                    Vec3::from_array(primitive.positions[triangle[i] as usize])
                });
                let cross = (b - a).cross(c - a);
                assert!(cross.length() > 1e-6, "degenerate {id}");
            }
            for ((point, normal), uv) in primitive
                .positions
                .iter()
                .zip(&primitive.normals)
                .zip(&primitive.uvs)
            {
                assert!(
                    point
                        .iter()
                        .all(|p| p.is_finite() && (-1e-6..=1.000001).contains(p))
                );
                assert!(roads::boxes(id).iter().any(|b| {
                    (0..3).all(|i| {
                        f64::from(point[i]) >= b.minimum[i] - 1e-6
                            && f64::from(point[i]) <= b.maximum[i] + 1e-6
                    })
                }));
                if normal[1] > 0.5 {
                    top += 1;
                    assert!(
                        normal[2] < -0.1,
                        "road visual must keep a smooth inclined top"
                    );
                    assert!((point[1] - (low + (high - low) * point[2])).abs() < 1e-5);
                    assert_eq!(*uv, [point[0], point[2]]);
                }
            }
        }
        assert!(top >= 4);
    }
}
