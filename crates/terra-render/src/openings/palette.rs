//! Joined openings retain their authored frame, hardware and glass materials.
use crate::blocks::BlockAssets;
use bevy::prelude::*;

pub(crate) fn material_palette(
    id: u16,
    assets: &BlockAssets,
    materials: &Assets<StandardMaterial>,
) -> Option<[usize; 5]> {
    let templates = assets.templates.as_ref()?;
    let block = templates.blocks.get(&id)?;
    let material = |slot: usize| {
        assets
            .materials
            .get(slot)
            .and_then(|handle| materials.get(handle))
    };
    let mut fixed = Vec::new();
    for primitive in &block.model {
        if !templates.hinge_materials.contains(&primitive.material)
            && material(primitive.material)
                .is_some_and(|material| material.alpha_mode != AlphaMode::Blend)
            && !fixed.contains(&primitive.material)
        {
            fixed.push(primitive.material);
        }
    }
    let wood = fixed
        .iter()
        .copied()
        .filter(|&slot| material(slot).is_some_and(|material| material.metallic < 0.25))
        .collect::<Vec<_>>();
    // Painted metal cab frames have the same three authored color groups as
    // timber frames. They must not disappear because every group is metallic.
    let frame = if wood.is_empty() { &fixed } else { &wood };
    let base = *frame.first()?;
    let colors = [
        base,
        *frame.get(1).unwrap_or(&base),
        *frame.get(2).unwrap_or(&base),
    ];
    let hardware = block
        .model
        .iter()
        .find(|primitive| {
            !colors.contains(&primitive.material)
                && material(primitive.material).is_some_and(|material| {
                    material.alpha_mode != AlphaMode::Blend && material.metallic >= 0.25
                })
        })
        .map_or(base, |primitive| primitive.material);
    let glass = block
        .model
        .iter()
        .find(|primitive| {
            material(primitive.material)
                .is_some_and(|material| material.alpha_mode == AlphaMode::Blend)
        })
        .map_or(base, |primitive| primitive.material);
    Some([colors[0], colors[1], colors[2], hardware, glass])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{BlockTemplate, TemplateSet, definitions::BlockRenderKind};
    use std::sync::Arc;

    #[test]
    fn actual_painted_cab_openings_and_timber_openings_keep_all_joined_materials_without_gpu() {
        for (id, path) in [
            (65, "terra/blocks/models/oak_door.gltf"),
            (247, "terra/blocks/models/train_cab_window.gltf"),
            (248, "terra/blocks/models/train_cab_door.gltf"),
        ] {
            let source: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(terra_core::asset_directory().join(path)).unwrap(),
            )
            .unwrap();
            let mut assets = BlockAssets::default();
            let mut materials = Assets::<StandardMaterial>::default();
            let mut templates = TemplateSet::default();
            for (slot, entry) in source["materials"].as_array().unwrap().iter().enumerate() {
                let pbr = &entry["pbrMetallicRoughness"];
                assets.materials.push(materials.add(StandardMaterial {
                    metallic: pbr["metallicFactor"].as_f64().unwrap_or(0.0) as f32,
                    alpha_mode: if entry["alphaMode"].as_str() == Some("BLEND") {
                        AlphaMode::Blend
                    } else {
                        AlphaMode::Opaque
                    },
                    ..default()
                }));
                if entry["name"].as_str().unwrap().starts_with("leaf_") {
                    templates.hinge_materials.insert(slot);
                }
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
                    model: crate::blocks::test_static_scene(path, id),
                },
            );
            assets.templates = Some(Arc::new(templates));
            let palette = material_palette(id, &assets, &materials).unwrap();
            let window = terra_world::objects::is_window(id);
            // Door glTFs declare the shared glass material but contain no glass
            // primitives. Their solid leaves must not acquire a transparent pane.
            assert_eq!(palette, [0, 1, 2, 3, if window { 10 } else { 0 }]);
            let mut meshes = Assets::<Mesh>::default();
            let parts =
                crate::openings::windows::parts(id, [2, 2], &assets, &mut meshes, &materials)
                    .expect("joined painted-metal openings must have visible native geometry");
            assert_eq!(parts.iter().any(|part| part.glass), window);
            for part in parts.iter().filter(|part| part.glass) {
                assert_eq!(part.material, assets.materials[10]);
            }
            assert!(parts.iter().any(|part| part.moving));
            assert!(parts.iter().any(|part| !part.moving));
        }
    }
}
