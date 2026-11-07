//! Exact RGB metadata for existing dyeable objects, without a color-sized block registry.
use crate::{ItemKind, WoodSpecies, beds::DyeColor};

pub const fn supports(material: u16) -> bool {
    matches!(material, 103..=106 | 148)
}

/// Mixed slabs retain their original primary material ID and encode the other half in facing.
pub fn cell_dyeable(material: u16, facing: u8) -> bool {
    dyeable(material)
        || crate::construction::slab_halves(material, facing)
            .is_some_and(|halves| halves.iter().any(|half| half.0 == 105))
}

pub fn dyeable(material: u16) -> bool {
    supports(material) || crate::textiles::base(material).is_some() || crate::beds::is_bed(material)
}

pub fn color(item: ItemKind) -> Option<[u8; 3]> {
    match item {
        ItemKind::TintedBlock { color, .. }
        | ItemKind::DyedBed {
            color: DyeColor::Rgb(color),
            ..
        }
        | ItemKind::Dye(DyeColor::Rgb(color)) => Some(color),
        ItemKind::Cape(cape) | ItemKind::HeraldicCape { cape, .. } => match cape.color {
            DyeColor::Rgb(rgb) => Some(rgb),
            _ => None,
        },
        _ => None,
    }
}

pub fn canonical_item(item: ItemKind) -> Option<ItemKind> {
    match item {
        ItemKind::TintedBlock { material, wood, .. } => Some(ItemKind::for_wood(material, wood)),
        ItemKind::DyedBed {
            material,
            color: DyeColor::Rgb(_),
            ..
        } => Some(ItemKind::DyedBed {
            material,
            color: DyeColor::White,
        }),
        _ => None,
    }
}

pub fn recolored(item: ItemKind, color: DyeColor) -> Option<ItemKind> {
    let material = item.block_material()?;
    if crate::beds::is_bed(material) {
        return Some(ItemKind::DyedBed { material, color });
    }
    let base = crate::textiles::base(material).unwrap_or(material);
    if !supports(base) {
        return None;
    }
    let wood = item.wood_species().unwrap_or(WoodSpecies::Oak);
    if let DyeColor::Rgb(color) = color {
        Some(ItemKind::TintedBlock {
            material: base,
            wood,
            color,
        })
    } else if base == 148 {
        // Packed legacy poses use linear multipliers; RGB item metadata uses sRGB bytes.
        let [r, g, b, _] = color.tint();
        Some(ItemKind::TintedBlock {
            material: base,
            wood,
            color: [r, g, b].map(|v| {
                let srgb = if v <= 0.003_130_8 {
                    v * 12.92
                } else {
                    1.055 * v.powf(1.0 / 2.4) - 0.055
                };
                (srgb * 255.0).round() as u8
            }),
        })
    } else if matches!(base, 104 | 105) {
        // Shapes have no palette-sized material family. Keep the same exact RGB item contract.
        let rgb = color.tint().map(|v| v.clamp(0.0, 1.0));
        Some(ItemKind::TintedBlock {
            material: base,
            wood: WoodSpecies::Oak,
            color: [rgb[0], rgb[1], rgb[2]].map(|v| {
                let srgb = if v <= 0.003_130_8 {
                    v * 12.92
                } else {
                    1.055 * v.powf(1.0 / 2.4) - 0.055
                };
                (srgb * 255.0).round() as u8
            }),
        })
    } else {
        Some(ItemKind::Block(crate::textiles::dyed(base, color)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn converting_legacy_belt_dye_to_rgb_keeps_its_linear_surface_color() {
        for dye in DyeColor::ALL {
            let rgb = color(recolored(ItemKind::Block(148), dye).unwrap()).unwrap();
            for (byte, linear) in rgb.into_iter().zip(dye.tint()) {
                let srgb = f32::from(byte) / 255.0;
                let decoded = if srgb <= 0.04045 {
                    srgb / 12.92
                } else {
                    ((srgb + 0.055) / 1.055).powf(2.4)
                };
                assert!(
                    (decoded - linear).abs() < 0.0045,
                    "{dye:?}: {decoded} vs {linear}"
                );
            }
        }
    }
    #[test]
    fn rgb_identity_is_exact_bounded_to_supported_fabrics_and_preserves_belt_wood() {
        for rgb in [[0, 0, 0], [255, 255, 255], [71, 93, 184]] {
            for base in [103, 104, 105, 106, 148, 71, 72, 73] {
                let source = ItemKind::for_wood(base, WoodSpecies::Cherry);
                let item = recolored(source, DyeColor::Rgb(rgb)).unwrap();
                assert!(item.valid());
                assert_eq!(color(item), Some(rgb));
                assert_eq!(canonical_item(item).unwrap().block_material(), Some(base));
                assert!(item.identifier().ends_with(&DyeColor::Rgb(rgb).code()));
                if base == 148 {
                    assert_eq!(item.wood_species(), Some(WoodSpecies::Cherry));
                }
            }
        }
        assert!(recolored(ItemKind::Block(3), DyeColor::Rgb([1, 2, 3])).is_none());
        assert!(
            !ItemKind::TintedBlock {
                material: 103,
                wood: WoodSpecies::Cherry,
                color: [1, 2, 3]
            }
            .valid()
        );
    }
}
