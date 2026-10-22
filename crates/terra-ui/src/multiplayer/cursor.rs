//! Small cached pointer bitmaps, with the actual pointing tip at the shared position.
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use std::collections::{HashMap, HashSet};

pub(super) fn images(
    mut assets: ResMut<Assets<Image>>,
    mut cache: Local<HashMap<[u8; 3], Handle<Image>>>,
    mut nodes: Query<(&tilt_ui::ImageMetadata, &mut ImageNode)>,
) {
    let mut used = HashSet::new();
    for (metadata, mut node) in &mut nodes {
        let Some(hex) = metadata
            .alt
            .as_deref()
            .and_then(|s| s.strip_prefix("Shared cursor "))
        else {
            continue;
        };
        if hex.len() != 6 || !hex.is_ascii() {
            continue;
        }
        let Some(color) = (0..3)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let color = [color[0], color[1], color[2]];
        used.insert(color);
        let handle = cache
            .entry(color)
            .or_insert_with(|| assets.add(pointer(color)));
        if node.image != *handle {
            node.image = handle.clone();
        }
    }
    cache.retain(|color, handle| {
        if used.contains(color) {
            true
        } else {
            assets.remove(handle.id());
            false
        }
    });
}

fn inside(p: Vec2) -> bool {
    let polygon = [
        Vec2::new(1.0, 1.0),
        Vec2::new(1.0, 26.0),
        Vec2::new(7.0, 20.0),
        Vec2::new(12.0, 30.0),
        Vec2::new(17.0, 27.0),
        Vec2::new(12.0, 18.0),
        Vec2::new(23.0, 18.0),
    ];
    let mut hit = false;
    let mut previous = polygon[polygon.len() - 1];
    for current in polygon {
        if (current.y > p.y) != (previous.y > p.y)
            && p.x
                < (previous.x - current.x) * (p.y - current.y) / (previous.y - current.y)
                    + current.x
        {
            hit = !hit;
        }
        previous = current;
    }
    hit
}

fn pointer(color: [u8; 3]) -> Image {
    // Render at twice the CSS size for crisp edges on high-DPI displays.
    let mut data = Vec::with_capacity(48 * 64 * 4);
    for y in 0..64 {
        for x in 0..48 {
            let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * 0.5;
            let alpha = if inside(p) { 255 } else { 0 };
            let inner = [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y]
                .into_iter()
                .all(|d| inside(p + d));
            let rgb = if inner { color } else { [20, 28, 22] };
            data.extend_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
        }
    }
    Image::new(
        Extent3d {
            width: 48,
            height: 64,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_faces_upper_left_with_colored_interior_and_transparent_exterior() {
        let image = pointer([78, 190, 140]);
        let data = image.data.unwrap();
        assert_eq!(
            &data[(20 * 48 + 8) * 4..(20 * 48 + 8) * 4 + 4],
            &[78, 190, 140, 255]
        );
        assert_eq!(data[(60 * 48 + 46) * 4 + 3], 0);
        assert!(inside(Vec2::new(2.0, 3.0)));
        assert!(!inside(Vec2::new(20.0, 3.0)));
    }
}
