use super::*;
use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};

fn templates() -> TemplateSet {
    let mut templates = TemplateSet::default();
    for (id, slot) in [(103, 0), (104, 0), (105, 0), (85, 1), (82, 2)] {
        templates.blocks.insert(
            id,
            BlockTemplate {
                surface: None,
                render_kind: if id == 103 {
                    BlockRenderKind::Cube
                } else {
                    BlockRenderKind::Boxes
                },
                bevel_width: 0.0,
                faces: crate::blocks::cube_faces(0.0, [slot; 6], None),
                face_materials: [slot; 6],
                side_cap: None,
                details: Vec::new(),
                model: if id == 103 {
                    Vec::new()
                } else {
                    crate::blocks::cube::box_model(
                        terra_core::block_shapes::builtin_boxes(id),
                        0.0,
                        [slot; 6],
                    )
                },
            },
        );
    }
    templates.wood_materials.insert(2);
    templates
}

fn compose_shape(
    material: u16,
    facing: u8,
    level: super::super::DetailLevel,
    tint: Option<[u8; 3]>,
) -> Vec<MeshBatch> {
    let snapshot = ModelSnapshot {
        standalone: true,
        source_position: None,
        heraldry: HashMap::new(),
        domains: Vec::new(),
        include_moving: true,
        blocks: vec![0; 4096],
        models: vec![ModelCell {
            cell: [15, 4, 0],
            material,
            state: terra_world::edits::BlockObjectState {
                facing,
                wood: terra_core::WoodSpecies::Birch,
                ..Default::default()
            },
            peer_wood: Default::default(),
            neighbors: [None; 6],
            neighbor_facing: [0; 6],
        }],
        border_models: Vec::new(),
        border_glass: HashSet::new(),
        water_contacts: HashMap::new(),
        chimney_infill: BTreeMap::new(),
        pipe_surround: BTreeMap::new(),
        funnel_supports: HashSet::new(),
    };
    let options = RenderOptions {
        water_level: None,
        detail_level: level,
        terrain: None,
        exposure: None,
        conditions: Default::default(),
        tints: std::sync::Arc::new(tint.map(|rgb| ([15, 4, 0], rgb)).into_iter().collect()),
    };
    compose(
        &BlockMesh::default(),
        &snapshot,
        &templates(),
        SectionPos::new(-2, -1, -1),
        &options,
    )
    .unwrap()
}

#[test]
fn wool_stairs_and_all_slab_poses_keep_exact_geometry_and_linear_rgb_in_every_lod() {
    let rgb = [71, 93, 184];
    for level in [
        super::super::DetailLevel::Near,
        super::super::DetailLevel::Middle,
        super::super::DetailLevel::Far,
    ] {
        for (material, facings) in [
            (104, (0..24).collect::<Vec<_>>()),
            (105, vec![0, 4, 5, 6, 7, 8, 9]),
        ] {
            for facing in facings {
                let plain = compose_shape(material, facing, level, None);
                let tinted = compose_shape(material, facing, level, Some(rgb));
                assert_eq!(plain.len(), tinted.len());
                for (plain, tinted) in plain.iter().zip(&tinted) {
                    assert_eq!(plain.material, tinted.material);
                    assert_eq!(plain.positions, tinted.positions);
                    assert_eq!(plain.normals, tinted.normals);
                    assert_eq!(plain.uvs, tinted.uvs);
                    assert_eq!(plain.indices, tinted.indices);
                    assert_eq!(
                        tinted.colors,
                        vec![crate::dye_tint::rgb(rgb); tinted.positions.len()]
                    );
                }
            }
        }
    }
}

#[test]
fn mixed_slab_rgb_only_changes_wool_in_either_primary_order_and_leaves_stone_and_wood_unchanged() {
    for other in [82, 85] {
        for (primary, secondary) in [(105, other), (other, 105)] {
            for half in [0, 4, 5, 6, 7, 8] {
                let facing =
                    terra_core::construction::merged_slab(primary, half, secondary).unwrap();
                let level = super::super::DetailLevel::Near;
                let plain = compose_shape(primary, facing, level, None);
                let tinted = compose_shape(primary, facing, level, Some([71, 93, 184]));
                assert_eq!(plain.len(), 2);
                assert_eq!(plain.len(), tinted.len());
                for (plain, tinted) in plain.iter().zip(&tinted) {
                    assert_eq!(plain.positions, tinted.positions);
                    assert_eq!(plain.normals, tinted.normals);
                    assert_eq!(plain.uvs, tinted.uvs);
                    assert_eq!(plain.indices, tinted.indices);
                    if tinted.material == 0 {
                        assert_eq!(
                            tinted.colors,
                            vec![crate::dye_tint::rgb([71, 93, 184]); tinted.positions.len()]
                        );
                    } else {
                        assert_eq!(plain.colors, tinted.colors, "never tint the non-wool half");
                    }
                }
            }
        }
    }
}

#[test]
fn rgb_wool_shape_item_icons_use_the_same_linear_tint_without_new_materials() {
    let options = RenderOptions {
        water_level: None,
        detail_level: super::super::DetailLevel::Near,
        terrain: None,
        exposure: None,
        conditions: Default::default(),
        tints: Default::default(),
    };
    for material in [104, 105] {
        let item = terra_core::ItemKind::TintedBlock {
            material,
            wood: terra_core::WoodSpecies::Oak,
            color: [71, 93, 184],
        };
        let tinted = crate::previews::tints::batches(item, &templates(), &options).unwrap();
        assert_eq!(tinted.len(), 1);
        assert_eq!(tinted[0].material, 0);
        assert_eq!(
            tinted[0].colors,
            vec![crate::dye_tint::rgb([71, 93, 184]); tinted[0].positions.len()]
        );
    }
}
