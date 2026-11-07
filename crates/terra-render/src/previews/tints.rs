use super::*;

pub(crate) fn batches(
    item: ItemKind,
    templates: &TemplateSet,
    options: &RenderOptions,
) -> Result<Vec<MeshBatch>, String> {
    if let ItemKind::DyedBed { material, color } = item {
        return dyed_bed_batches(material, color, templates, options);
    }
    let ItemKind::TintedBlock {
        material, color, ..
    } = item
    else {
        return Err("Not an RGB block".into());
    };
    let canonical = terra_core::tints::canonical_item(item).ok_or("No canonical RGB block")?;
    let mut batches = if material == 148 {
        wooden_block_batches(canonical, templates, options)?
    } else {
        block_batches(material, templates, options)?
    };
    let tint = crate::dye_tint::rgb(color);
    for batch in &mut batches {
        if match material {
            148 => templates
                .conveyor_ribbon_materials
                .contains(&batch.material),
            104 | 105 => templates.untinted_wool_material(batch.material),
            _ => true,
        } {
            multiply(batch, tint);
        }
    }
    Ok(batches)
}

fn multiply(batch: &mut MeshBatch, tint: [f32; 4]) {
    batch.colors.resize(batch.positions.len(), [1.0; 4]);
    for color in &mut batch.colors {
        for channel in 0..3 {
            color[channel] *= tint[channel];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dye_sculpt_already_decodes_srgb_once_and_agrees_with_direct_pbr_tints() {
        let rgb = [71, 93, 184];
        let [r, g, b, _] = terra_core::beds::DyeColor::Rgb(rgb).tint();
        let expected = crate::dye_tint::rgb(rgb);
        let mut neutral = MeshBatch::default();
        sculpt::quad(
            &mut neutral,
            [Vec3::ZERO, Vec3::X, Vec3::X + Vec3::Y, Vec3::Y],
            Vec3::Z,
            [r, g, b],
        )
        .unwrap();
        for actual in &neutral.colors {
            for (a, b) in actual.iter().zip(expected) {
                assert!((*a - b).abs() < 2e-7);
            }
        }
        let batches = dye_batches(
            terra_core::beds::DyeColor::Rgb(rgb),
            &TemplateSet::default(),
        )
        .unwrap();
        let mut powder = 0;
        for (position, color) in batches[0].positions.iter().zip(&batches[0].colors) {
            if position[1] > 0.35 {
                powder += 1;
                for channel in 0..3 {
                    assert!(
                        (0.85..=1.15).contains(&(color[channel] / expected[channel])),
                        "powder decodes only once while retaining authored grain"
                    );
                }
                assert_eq!(color[3], 1.0);
            }
        }
        assert!(powder > 100);
    }

    #[test]
    fn fabric_tint_keeps_vertex_shading_and_special_alpha_metadata() {
        let mut batch = MeshBatch {
            positions: vec![[0.0; 3]; 3],
            colors: vec![[0.8, 0.7, 0.6, 4.0], [0.4, 0.3, 0.2, 6.0], [1.0; 4]],
            ..default()
        };
        multiply(&mut batch, [0.25, 0.5, 0.75, 1.0]);
        for (actual, expected) in batch.colors[0].into_iter().zip([0.2, 0.35, 0.45, 4.0]) {
            assert!((actual - expected).abs() < 1e-7);
        }
        assert_eq!(batch.colors[1], [0.1, 0.15, 0.15, 6.0]);
        assert_eq!(batch.colors[2], [0.25, 0.5, 0.75, 1.0]);
    }

    #[test]
    fn rgb_belt_and_bed_icons_preserve_independent_wood_and_metal() {
        use crate::blocks::{BlockTemplate, PrimitiveTemplate, definitions::BlockRenderKind};
        let primitive = |material| PrimitiveTemplate {
            material,
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            tangents: Vec::new(),
            uvs: vec![[0.0; 2]; 3],
            indices: vec![0, 1, 2],
        };
        let mut templates = TemplateSet::default();
        templates.wood_materials.insert(0);
        templates.blanket_materials.insert(1);
        templates.conveyor_ribbon_materials.insert(1);
        for material in [71, 148] {
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
                    model: (0..3).map(primitive).collect(),
                },
            );
        }
        let options = RenderOptions {
            water_level: None,
            detail_level: DetailLevel::Near,
            terrain: None,
            exposure: None,
            tints: default(),
            conditions: default(),
        };
        let color = terra_core::beds::DyeColor::Rgb([71, 93, 184]);
        for item in [
            ItemKind::TintedBlock {
                material: 148,
                wood: terra_core::WoodSpecies::Cherry,
                color: [71, 93, 184],
            },
            ItemKind::DyedBed {
                material: 71,
                color,
            },
        ] {
            let actual = batches(item, &templates, &options).unwrap();
            let canonical = terra_core::tints::canonical_item(item).unwrap();
            let before = if item.block_material() == Some(148) {
                wooden_block_batches(canonical, &templates, &options).unwrap()
            } else {
                block_batches(71, &templates, &options).unwrap()
            };
            assert_eq!(actual.len(), before.len());
            for (actual, before) in actual.iter().zip(&before) {
                assert_eq!(actual.positions, before.positions);
                assert_eq!(actual.normals, before.normals);
                assert_eq!(actual.uvs, before.uvs);
                assert_eq!(actual.indices, before.indices);
                if actual.material == 1 {
                    assert!(
                        actual
                            .colors
                            .iter()
                            .all(|&value| value == crate::dye_tint::color(color))
                    );
                } else {
                    assert_eq!(actual.colors, before.colors, "never tint wood or metal");
                }
            }
        }
    }
}
