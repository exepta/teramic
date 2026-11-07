//! Catalog emblems share the persisted glyph with cape cloth and the UI picker.
use super::ModelCell;
use crate::blocks::{PrimitiveTemplate, TemplateSet};
use terra_core::kingdom::{KingdomHeraldry, KingdomIcon};

pub(super) fn glyph(
    model: &ModelCell,
    heraldry: KingdomHeraldry,
    templates: &TemplateSet,
) -> Option<PrimitiveTemplate> {
    if !matches!(heraldry.icon, KingdomIcon::Catalog(_))
        || !matches!(
            model.material,
            terra_core::castle::BERGFRIED | terra_core::castle::ROYAL_WALL
        )
    {
        return None;
    }
    let mask = heraldry.icon.glyph_mask()?;
    // Reuse the authored white emblem material, retaining its cloth roughness.
    let material = *templates
        .castle_icon_materials
        .iter()
        .find(|(_, icon)| **icon == KingdomIcon::Castle)?
        .0;
    let center_y = if model.material == terra_core::castle::BERGFRIED {
        0.410
    } else {
        0.490
    };
    let mut primitive = PrimitiveTemplate {
        positions: Vec::new(),
        normals: Vec::new(),
        tangents: Vec::new(),
        uvs: Vec::new(),
        indices: Vec::new(),
        material,
    };
    for (y, row) in mask.iter().copied().enumerate() {
        let mut x = 0;
        while x < 32 {
            if row & (1 << x) == 0 {
                x += 1;
                continue;
            }
            let start = x;
            while x < 32 && row & (1 << x) != 0 {
                x += 1;
            }
            let left = 0.34 + start as f32 * 0.01;
            let right = 0.34 + x as f32 * 0.01;
            let top = center_y + 0.16 - y as f32 * 0.01;
            let bottom = top - 0.01;
            for side in 0..4 {
                let rotate = |[x, y, z]: [f32; 3]| match side {
                    0 => [x, y, z],
                    1 => [1.0 - z, y, x],
                    2 => [1.0 - x, y, 1.0 - z],
                    _ => [z, y, 1.0 - x],
                };
                let base = primitive.positions.len() as u32;
                // Outside the cloth's z=.0014 face; the original icons use .0002.
                primitive.positions.extend(
                    [
                        [left, bottom, 0.0002],
                        [left, top, 0.0002],
                        [right, top, 0.0002],
                        [right, bottom, 0.0002],
                    ]
                    .map(rotate),
                );
                let normal = [
                    [0.0, 0.0, -1.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0],
                    [-1.0, 0.0, 0.0],
                ][side];
                primitive.normals.extend([normal; 4]);
                primitive
                    .uvs
                    .extend([[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]);
                primitive
                    .indices
                    .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
    }
    Some(primitive)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_catalog_emblem_has_visible_banner_geometry_on_all_four_sides() {
        let mut templates = TemplateSet::default();
        templates
            .castle_icon_materials
            .insert(7, KingdomIcon::Castle);
        let model = ModelCell {
            cell: [0; 3],
            material: terra_core::castle::ROYAL_WALL,
            state: Default::default(),
            peer_wood: Default::default(),
            neighbors: [None; 6],
            neighbor_facing: [0; 6],
        };
        for icon in KingdomIcon::catalog().filter(|icon| matches!(icon, KingdomIcon::Catalog(_))) {
            let mesh = glyph(
                &model,
                KingdomHeraldry {
                    icon,
                    ..Default::default()
                },
                &templates,
            )
            .unwrap();
            assert!(!mesh.indices.is_empty(), "{}", icon.key());
            assert_eq!(mesh.normals.len(), mesh.positions.len());
            for normal in [
                [0.0, 0.0, -1.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                [-1.0, 0.0, 0.0],
            ] {
                assert!(mesh.normals.contains(&normal), "{}", icon.key());
            }
            for point in mesh.positions {
                assert!(point.into_iter().all(|v| (0.0..=1.0).contains(&v)));
            }
        }
    }
}
