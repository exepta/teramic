//! Mounted models retain placed-world profiles without activating world machines.
use super::*;
use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};
use terra_core::{WoodSpecies, trains::TrainPart};
use terra_world::edits::BlockObjectState;

fn templates() -> TemplateSet {
    let mut templates = TemplateSet::default();
    let mut slots = BTreeMap::<String, usize>::new();
    for (id, path) in [
        (163, "terra/blocks/models/wood_pipe.gltf"),
        (165, "terra/blocks/models/iron_pipe.gltf"),
        (169, "terra/blocks/models/basin.gltf"),
        (171, "terra/blocks/models/fluid_infuser.gltf"),
        (177, "terra/blocks/models/castle_overgrown_wall.gltf"),
        (183, "terra/blocks/models/castle_arch_left.gltf"),
        (184, "terra/blocks/models/castle_arch_right.gltf"),
    ] {
        let source: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(terra_core::asset_directory().join(path)).unwrap(),
        )
        .unwrap();
        let mut model = crate::blocks::test_static_scene(path, id);
        for primitive in &mut model {
            let name = source["materials"][primitive.material]["name"]
                .as_str()
                .unwrap();
            let next = slots.len();
            let slot = *slots.entry(name.to_owned()).or_insert(next);
            primitive.material = slot;
            templates.register_castle_material(name, slot);
        }
        templates.blocks.insert(
            id,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model,
            },
        );
    }
    templates
}

fn part(material: u16, facing: u8) -> TrainPart {
    TrainPart {
        material,
        facing,
        offset: [0; 3],
        wood: WoodSpecies::Oak,
    }
}

fn placed(
    part: TrainPart,
    neighbors: [Option<(u16, u8)>; 6],
    templates: &TemplateSet,
    position: [i64; 3],
) -> Vec<MeshBatch> {
    let (section, local) = terra_voxel::global_to_section(position);
    let local = local.map(|value| value as i32);
    let snapshot = ModelSnapshot {
        standalone: false,
        source_position: None,
        heraldry: HashMap::new(),
        domains: Vec::new(),
        include_moving: true,
        blocks: vec![0; 4096],
        models: vec![ModelCell {
            cell: local,
            material: part.material,
            state: terra_world::edits::BlockObjectState {
                facing: part.facing,
                ..Default::default()
            },
            peer_wood: WoodSpecies::Oak,
            neighbors: neighbors.map(|neighbor| neighbor.map(|(material, _)| material)),
            neighbor_facing: neighbors.map(|neighbor| neighbor.map_or(0, |(_, facing)| facing)),
        }],
        border_models: Vec::new(),
        border_glass: HashSet::new(),
        bile_surface: None,
        water_contacts: HashMap::new(),
        water_plants: HashMap::new(),
        chimney_infill: BTreeMap::new(),
        pipe_surround: BTreeMap::new(),
        funnel_supports: HashSet::new(),
    };
    let mut batches = compose(
        &BlockMesh::default(),
        &snapshot,
        templates,
        section,
        &RenderOptions {
            water_level: None,
            detail_level: super::super::DetailLevel::Near,
            terrain: None,
            exposure: None,
            conditions: Default::default(),
            tints: Default::default(),
        },
    )
    .unwrap();
    for batch in &mut batches {
        for point in &mut batch.positions {
            for axis in 0..3 {
                point[axis] -= local[axis] as f32;
            }
        }
    }
    batches
}

fn equal(actual: &[MeshBatch], expected: &[MeshBatch]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.material, expected.material);
        assert_eq!(actual.indices, expected.indices);
        assert_eq!(actual.uvs, expected.uvs);
        assert_eq!(actual.colors, expected.colors);
        for (actual, expected) in [
            (&actual.positions, &expected.positions),
            (&actual.normals, &expected.normals),
        ] {
            assert_eq!(actual.len(), expected.len());
            for (actual, expected) in actual.iter().zip(expected) {
                for axis in 0..3 {
                    assert!(
                        (actual[axis] - expected[axis]).abs() < 0.00001,
                        "{actual:?} != {expected:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn connected_vehicle_pipes_and_infuser_fittings_match_the_placed_world_instead_of_item_previews() {
    let templates = templates();
    let neighbors = [Some((165, 0)), None, None, None, None, Some((165, 0))];
    for material in [163, 165, 171] {
        for facing in 0..4 {
            let part = part(material, facing);
            let source = [-17, 31, 18];
            let actual = vehicle_model_geometry(
                &part,
                None,
                WoodSpecies::Oak,
                neighbors,
                &templates,
                source,
            )
            .unwrap();
            equal(&actual, &placed(part, neighbors, &templates, source));
            if material == 163 && facing == 0 {
                let preview = model_geometry(
                    material,
                    BlockObjectState::default(),
                    WoodSpecies::Oak,
                    neighbors.map(|neighbor| neighbor.map(|(material, _)| material)),
                    [0; 6],
                    &templates,
                )
                .unwrap();
                assert_ne!(
                    actual, preview,
                    "the connected elbow must not use the isolated item profile"
                );
            }
        }
    }
}

#[test]
fn vehicle_basins_and_paired_arches_keep_open_seams_and_hide_shared_join_trim() {
    let templates = templates();
    for (part, neighbors) in [
        (part(169, 2), [None, Some((169, 1)), None, None, None, None]),
        (
            part(169, 10),
            [None, Some((169, 1)), None, Some((169, 4)), None, None],
        ),
        (part(183, 0), [None, Some((184, 0)), None, None, None, None]),
        (part(184, 0), [Some((183, 0)), None, None, None, None, None]),
    ] {
        let actual =
            vehicle_model_geometry(&part, None, WoodSpecies::Oak, neighbors, &templates, [0; 3])
                .unwrap();
        equal(&actual, &placed(part, neighbors, &templates, [0; 3]));
        let preview = model_geometry(
            part.material,
            BlockObjectState {
                facing: part.facing,
                ..Default::default()
            },
            WoodSpecies::Oak,
            neighbors.map(|neighbor| neighbor.map(|(material, _)| material)),
            neighbors.map(|neighbor| neighbor.map_or(0, |(_, facing)| facing)),
            &templates,
        )
        .unwrap();
        assert_ne!(
            actual, preview,
            "joined geometry must not restore the inventory preview's walls/trim"
        );
    }
}

#[test]
fn mounted_castle_decoration_keeps_its_original_world_seed_in_negative_and_positive_sections() {
    let templates = templates();
    assert!(!templates.castle_decor_materials.is_empty());
    let part = part(177, 0);
    let mut poses = std::collections::BTreeSet::new();
    for source in [
        [-17, 31, 18],
        [17, -2, -19],
        [32, 5, 16],
        [-31, 4, -17],
        [0, 2, 1],
        [0, 2, 2],
        [0, 2, 3],
        [0, 2, 4],
    ] {
        let actual =
            vehicle_model_geometry(&part, None, WoodSpecies::Oak, [None; 6], &templates, source)
                .unwrap();
        equal(&actual, &placed(part, [None; 6], &templates, source));
        let rotation = castle::detail_rotation_at(source);
        poses.insert(rotation.to_array().map(f32::to_bits));
    }
    assert!(
        poses.len() > 1,
        "fixture must exercise distinct existing decoration poses"
    );
}
