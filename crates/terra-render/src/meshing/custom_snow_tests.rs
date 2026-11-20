use super::*;
use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};

#[test]
fn dry_player_snow_plates_keep_cutout_uv1_under_a_roof_at_all_seven_physical_heights() {
    let mut templates = TemplateSet::default();
    templates.blocks.insert(
        98,
        BlockTemplate {
            surface: None,
            render_kind: BlockRenderKind::CustomMesh,
            bevel_width: 0.0,
            faces: std::array::from_fn(|_| Vec::new()),
            face_materials: [0; 6],
            side_cap: None,
            details: Vec::new(),
            model: crate::blocks::cube::box_model(
                &[terra_core::construction::snow_bounds(1)],
                0.0,
                [0; 6],
            ),
        },
    );
    let exposure = std::sync::Arc::new(crate::weather::surfaces::exposure::Exposure {
        natural: None,
        roofs: [64; 256],
        snow_roofs: [64; 256],
        coast: [0.0; 256],
        coast_vertices: [0.0; 289],
        water: [f32::NEG_INFINITY; 256],
    });
    for layer in 1..=7 {
        let snapshot = ModelSnapshot {
            standalone: false,
            source_position: None,
            heraldry: HashMap::new(),
            domains: Vec::new(),
            include_moving: false,
            blocks: vec![0; 4096],
            models: vec![ModelCell {
                cell: [15, 5, 15],
                material: 98,
                state: terra_world::edits::BlockObjectState {
                    facing: terra_core::construction::manual_snow_facing(layer).unwrap(),
                    ..Default::default()
                },
                peer_wood: Default::default(),
                neighbors: [None; 6],
                neighbor_facing: [0; 6],
            }],
            border_models: Vec::new(),
            border_glass: HashSet::new(),
            water_contacts: HashMap::new(),
            water_plants: HashMap::new(),
            chimney_infill: BTreeMap::new(),
            pipe_surround: BTreeMap::new(),
            funnel_supports: HashSet::new(),
        };
        let options = RenderOptions {
            water_level: None,
            detail_level: super::super::DetailLevel::Near,
            terrain: None,
            exposure: Some(exposure.clone()),
            tints: Default::default(),
            conditions: Default::default(),
        };
        let batches = compose(
            &BlockMesh::default(),
            &snapshot,
            &templates,
            SectionPos::new(-1, 0, -1),
            &options,
        )
        .unwrap();
        assert!(!batches.is_empty());
        for batch in batches {
            assert_eq!(batch.surface_water.len(), batch.positions.len());
            assert!(
                batch
                    .surface_water
                    .iter()
                    .all(|payload| *payload == [0.0; 2])
            );
            let top = batch
                .positions
                .iter()
                .map(|p| p[1])
                .fold(f32::NEG_INFINITY, f32::max);
            assert_eq!(top, 5.0 + f32::from(layer) / 8.0);
        }
    }
}

#[test]
fn saved_snow_coats_actual_stair_slab_path_and_fence_meshes_without_extra_geometry_or_batches() {
    for (material, facing) in (77..=78)
        .flat_map(|material| (0..24).map(move |facing| (material, facing)))
        .chain(
            (85..=86)
                .flat_map(|material| [0, 4, 5, 6, 7, 8, 9].map(move |facing| (material, facing))),
        )
        .chain([(94, 0), (102, 0), (138, 0)])
    {
        let boxes = if material == 102 {
            vec![terra_core::block_shapes::ShapeBox {
                minimum: [0.0; 3],
                maximum: [1.0, 0.9375, 1.0],
                hinged: false,
            }]
        } else {
            terra_core::construction::placed_boxes(material, facing, [None; 6])
        };
        let mut templates = TemplateSet::default();
        templates.blocks.insert(
            material,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: crate::blocks::cube::box_model(&boxes, 0.0, [0; 6]),
            },
        );
        let model = ModelCell {
            cell: [15, 5, 15],
            material,
            state: terra_world::edits::BlockObjectState {
                facing,
                ..Default::default()
            },
            peer_wood: Default::default(),
            neighbors: [None; 6],
            neighbor_facing: [0; 6],
        };
        let snapshot = ModelSnapshot {
            standalone: false,
            source_position: None,
            heraldry: HashMap::new(),
            domains: Vec::new(),
            include_moving: false,
            blocks: vec![0; 4096],
            models: vec![model],
            border_models: Vec::new(),
            border_glass: HashSet::new(),
            water_contacts: HashMap::new(),
            water_plants: HashMap::new(),
            chimney_infill: BTreeMap::new(),
            pipe_surround: BTreeMap::new(),
            funnel_supports: HashSet::new(),
        };
        let mut options = RenderOptions {
            water_level: None,
            detail_level: super::super::DetailLevel::Near,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: Default::default(),
        };
        let section = SectionPos::new(-1, 0, -1);
        let dry = compose(
            &BlockMesh::default(),
            &snapshot,
            &templates,
            section,
            &options,
        )
        .unwrap();
        options.conditions.insert(
            [15, 5, 15],
            terra_world::environment::SurfaceCondition {
                snow: 0.75,
                wetness: 0.4,
                ..Default::default()
            },
        );
        let snow = compose(
            &BlockMesh::default(),
            &snapshot,
            &templates,
            section,
            &options,
        )
        .unwrap();
        assert!(!snow.is_empty(), "material {material} facing {facing}");
        assert_eq!(snow.len(), dry.len());
        for (before, after) in dry.iter().zip(&snow) {
            assert_eq!(after.material, before.material);
            assert_eq!(
                after.positions, before.positions,
                "no horizontal cap above the real profile"
            );
            assert_eq!(after.normals, before.normals);
            assert_eq!(after.indices, before.indices);
            assert_eq!(after.uvs, before.uvs);
            assert_eq!(after.surface_water.len(), after.positions.len());
            let expected_snow = if material == 102 { 0.25 } else { 1.0 };
            assert!(after.surface_water.iter().all(|payload| *payload
                == [
                    crate::weather::storage::snow::encode_surface(0.4, expected_snow),
                    0.0
                ]));
            assert!(after.normals.iter().any(|normal| normal[1] > 0.99));
            assert!(after.normals.iter().any(|normal| normal[1].abs() < 0.01));
        }
    }
}
