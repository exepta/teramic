use super::PrimitiveTemplate;
use bevy::math::{Vec2, Vec3};
use terra_core::street;

/// A quarter-disc deck with a continuous rim. Collision strips from the
/// authored profile approximate the same unit circle and never appear as seams.
pub(crate) fn arc(material: u16, facing: u8, slots: [usize; 6]) -> Vec<PrimitiveTemplate> {
    const SEGMENTS: usize = 32;
    let left = material == street::ARC_LEFT;
    let center = Vec2::new(if left { 1. } else { 0. }, 1.);
    let rotate = |p: Vec2| match facing % 4 {
        0 => p,
        1 => Vec2::new(p.y, 1. - p.x),
        2 => Vec2::ONE - p,
        _ => Vec2::new(1. - p.y, p.x),
    };
    let center = rotate(center);
    let points: Vec<_> = (0..=SEGMENTS)
        .map(|i| {
            let (s, c) = (i as f32 * std::f32::consts::FRAC_PI_2 / SEGMENTS as f32).sin_cos();
            rotate(
                if left {
                    Vec2::new(1. - s, 1. - c)
                } else {
                    Vec2::new(c, 1. - s)
                }
                .clamp(Vec2::ZERO, Vec2::ONE),
            )
        })
        .collect();
    let height = street::boxes(material)[0].maximum[1] as f32;
    let at = |p: Vec2, h: f32| [p.x, h, p.y];
    let mut model = Vec::new();
    for (h, normal, slot) in [(height, Vec3::Y, slots[3]), (0., Vec3::NEG_Y, slots[2])] {
        let cap: Vec<_> = std::iter::once(&center).chain(&points).copied().collect();
        let indices = (1..=SEGMENTS as u32)
            .flat_map(|i| if h > 0. { [0, i, i + 1] } else { [0, i + 1, i] })
            .collect();
        model.push(PrimitiveTemplate {
            positions: cap.iter().map(|&p| at(p, h)).collect(),
            uvs: cap.iter().map(|p| p.to_array()).collect(),
            normals: vec![normal.to_array(); cap.len()],
            tangents: Vec::new(),
            indices,
            material: slot,
        });
    }
    let mut rim = PrimitiveTemplate {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        tangents: Vec::new(),
        indices: Vec::new(),
        material: slots[0],
    };
    // Two straight sides meet the neighboring sidewalk; the rounded street
    // face shares its shading normals from one chord to the next.
    let mut distance = 0.;
    let perimeter = std::iter::once(center)
        .chain(points.iter().copied())
        .chain([center]);
    let outline: Vec<_> = perimeter.collect();
    for (i, pair) in outline.windows(2).enumerate() {
        let (a, b) = (pair[0], pair[1]);
        let edge = b - a;
        let straight = Vec2::new(-edge.y, edge.x).normalize();
        let n = |p| {
            if i == 0 || i == SEGMENTS + 1 {
                straight
            } else {
                p - center
            }
        };
        let end = distance + edge.length();
        let start = rim.positions.len() as u32;
        for (p, h, u) in [
            (a, 0., distance),
            (b, 0., end),
            (b, height, end),
            (a, height, distance),
        ] {
            rim.positions.push(at(p, h));
            let outward = n(p);
            rim.normals.push(at(outward, 0.));
            rim.tangents.push([outward.y, 0., -outward.x, 1.]);
            rim.uvs.push([u, h]);
        }
        rim.indices.extend([0, 1, 2, 0, 2, 3].map(|j| start + j));
        distance = end;
    }
    model.push(rim);
    model
}
