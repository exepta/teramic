use super::{PropDetail, PropMesh, PropPalette, shapes::raw_template};
use crate::props::PropKind;
use bevy_math::Vec3;
use std::sync::OnceLock;

struct Sizing {
    center: Vec3,
    scale: Vec3,
    bounds: [[f32; 3]; 2],
    heights: [f32; 3],
}

// Shared template measurements are cached, never rebuilt for individual props.
static SIZING: [[OnceLock<Result<Sizing, String>>; 256]; PropKind::ALL.len()] =
    [const { [const { OnceLock::new() }; 256] }; PropKind::ALL.len()];

fn sizing(kind: PropKind, variant: u8) -> Result<&'static Sizing, String> {
    SIZING[kind as usize][usize::from(variant)]
        .get_or_init(|| {
            let mut minimum = Vec3::splat(f32::INFINITY);
            let mut maximum = Vec3::splat(f32::NEG_INFINITY);
            let mut heights = [0.0_f32; 3];
            for (index, detail) in [PropDetail::Near, PropDetail::Middle, PropDetail::Far]
                .into_iter()
                .enumerate()
            {
                for mesh in raw_template(kind, variant, detail, &PropPalette::default())? {
                    for point in mesh.positions {
                        let point = Vec3::from_array(point);
                        minimum = minimum.min(point);
                        maximum = maximum.max(point);
                        heights[index] = heights[index].max(point.y);
                    }
                }
            }
            if !minimum.is_finite()
                || !maximum.is_finite()
                || heights.iter().any(|height| *height <= 0.0)
            {
                return Err("Prop template has no finite rooted geometry".to_owned());
            }
            let maximum_visual_scale = match kind {
                PropKind::Flower => 0.95,
                PropKind::Mushroom => 0.55,
                _ => 1.3,
            };
            let width = (maximum.x - minimum.x).max(maximum.z - minimum.z);
            if width <= 0.0 {
                return Err("Prop template has no horizontal extent".to_owned());
            }
            let horizontal_scale = if kind == PropKind::BeeNest || kind.aquatic() {
                1.0
            } else {
                0.5 / maximum_visual_scale / width
            };
            let vertical_scale = if matches!(kind, PropKind::Flower | PropKind::Mushroom) {
                1.0 / maximum.y
            } else {
                1.0
            };
            let scale = Vec3::new(horizontal_scale, vertical_scale, horizontal_scale);
            let center = (minimum + maximum) * Vec3::new(0.5, 0.0, 0.5);
            Ok(Sizing {
                center,
                scale,
                bounds: [
                    ((minimum - center) * scale).to_array(),
                    ((maximum - center) * scale).to_array(),
                ],
                heights,
            })
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub(super) fn normalize(
    kind: PropKind,
    variant: u8,
    detail: PropDetail,
    meshes: &mut [PropMesh],
) -> Result<(), String> {
    let sizing = sizing(kind, variant)?;
    let mut scale = sizing.scale;
    if matches!(kind, PropKind::Flower | PropKind::Mushroom) {
        scale.y = 1.0
            / sizing.heights[match detail {
                PropDetail::Near => 0,
                PropDetail::Middle => 1,
                PropDetail::Far => 2,
            }];
    }
    for mesh in meshes {
        for point in &mut mesh.positions {
            *point = ((Vec3::from_array(*point) - sizing.center) * scale).to_array();
        }
        for normal in &mut mesh.normals {
            *normal = (Vec3::from_array(*normal) / scale)
                .normalize_or_zero()
                .to_array();
        }
    }
    Ok(())
}

pub(crate) fn bounds(kind: PropKind, variant: u8) -> Option<[[f32; 3]; 2]> {
    sizing(kind, variant).ok().map(|sizing| sizing.bounds)
}
