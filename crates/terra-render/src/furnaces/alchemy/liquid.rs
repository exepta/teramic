use super::*;

const SIDES: usize = 16;
const RINGS: usize = 4;
const BUBBLES: usize = 6;

fn wave(point: Vec2, radius: f32, time: f32, heated: bool) -> f32 {
    if !heated {
        return 0.0;
    }
    let edge = (1.0 - point.length_squared() / radius.powi(2)).max(0.0);
    edge * ((point.x * 31.0 + time * 4.7).sin() * 0.0025
        + (point.y * 26.0 - time * 3.9).cos() * 0.002)
}

fn vertex(target: &mut Geometry, point: Vec3, normal: Vec3, color: [f32; 4]) {
    target.positions.push(point.to_array());
    target.normals.push(normal.to_array());
    target.colors.push(color);
    target.uvs.push([0.5, 0.5]);
}

pub(super) fn append(
    target: &mut Geometry,
    anchor: Vec3,
    water: u8,
    color: [f32; 4],
    time: f32,
    heated: bool,
) {
    if water == 0 {
        return;
    }
    let height = liquid_height(water);
    let radius = liquid_radius(height);
    let origin = anchor + Vec3::new(0.5, height, 0.5);
    let base = target.positions.len() as u32;
    vertex(
        target,
        origin + Vec3::Y * wave(Vec2::ZERO, radius, time, heated),
        Vec3::Y,
        color,
    );
    for ring in 1..=RINGS {
        for side in 0..SIDES {
            let angle = side as f32 * std::f32::consts::TAU / SIDES as f32;
            let point = Vec2::from_angle(angle) * (radius * ring as f32 / RINGS as f32);
            let epsilon = 0.0005;
            let slope_x = (wave(point + Vec2::X * epsilon, radius, time, heated)
                - wave(point - Vec2::X * epsilon, radius, time, heated))
                / (2.0 * epsilon);
            let slope_z = (wave(point + Vec2::Y * epsilon, radius, time, heated)
                - wave(point - Vec2::Y * epsilon, radius, time, heated))
                / (2.0 * epsilon);
            let mut tint = color;
            if heated {
                let brightness = 1.0 + (slope_x + slope_z) * 0.65;
                for channel in &mut tint[..3] {
                    *channel *= brightness;
                }
            }
            vertex(
                target,
                origin + Vec3::new(point.x, wave(point, radius, time, heated), point.y),
                Vec3::new(-slope_x, 1.0, -slope_z).normalize(),
                tint,
            );
        }
    }
    let index = |ring: usize, side: usize| base + 1 + ((ring - 1) * SIDES + side % SIDES) as u32;
    for side in 0..SIDES {
        target
            .indices
            .extend([base, index(1, side + 1), index(1, side)]);
        for ring in 1..RINGS {
            let [a, b, c, d] = [
                index(ring, side),
                index(ring, side + 1),
                index(ring + 1, side + 1),
                index(ring + 1, side),
            ];
            target.indices.extend([a, b, c, a, c, d]);
        }
    }
    if !heated {
        return;
    }
    // Six staggered caps grow out of the surface, flatten and burst into rings.
    // Their complete geometry stays in the existing shared flame/liquid mesh.
    for bubble in 0..BUBBLES {
        let phase = (time * (0.65 + bubble as f32 * 0.035) + bubble as f32 * 0.173).fract();
        let angle = bubble as f32 * 2.4;
        let distance = radius * (0.25 + (bubble % 3) as f32 * 0.18);
        let point = Vec2::from_angle(angle) * distance;
        let center = origin + Vec3::new(point.x, wave(point, radius, time, true) + 0.001, point.y);
        let size = 0.006 + phase * 0.024;
        let rise = if phase < 0.72 {
            (phase / 0.72).sqrt() * 0.024 + 0.002
        } else {
            ((1.0 - phase) / 0.28) * 0.026
        };
        let mut highlight = color;
        let pop = phase > 0.82;
        for channel in &mut highlight[..3] {
            *channel = *channel * 0.72 + if pop { 0.31 } else { 0.20 };
        }
        let base = target.positions.len() as u32;
        vertex(target, center + Vec3::Y * rise, Vec3::Y, highlight);
        for ring in 1..=2 {
            let fraction = ring as f32 / 2.0;
            for side in 0..8 {
                let angle = side as f32 * std::f32::consts::TAU / 8.0;
                let outward = Vec2::from_angle(angle);
                let y = rise * (1.0 - fraction.powi(2)).sqrt();
                let normal =
                    Vec3::new(outward.x * fraction, y / rise, outward.y * fraction).normalize();
                let tint = if ring == 1 { highlight } else { color };
                vertex(
                    target,
                    center + Vec3::new(outward.x * size * fraction, y, outward.y * size * fraction),
                    normal,
                    tint,
                );
            }
        }
        for side in 0..8 {
            let next = (side + 1) % 8;
            let [a, b, c, d] = [
                base + 1 + side,
                base + 1 + next,
                base + 9 + next,
                base + 9 + side,
            ];
            target.indices.extend([base, b, a, a, b, c, a, c, d]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(water: u8, heated: bool, time: f32) -> Geometry {
        let mut target = Geometry::default();
        append(
            &mut target,
            Vec3::ZERO,
            water,
            [0.3, 0.1, 0.5, 1.0],
            time,
            heated,
        );
        target
    }

    #[test]
    fn heat_animates_real_bubble_caps_without_a_recipe_or_more_meshes() {
        for water in 1..=3 {
            let cold = geometry(water, false, 0.0);
            let later = geometry(water, false, 20.0);
            assert_eq!(cold.positions, later.positions);
            assert_eq!(cold.colors, later.colors);
            assert_eq!(cold.normals, later.normals);
            assert_eq!(cold.positions.len(), 1 + RINGS * SIDES);
            let boiling = geometry(water, true, 0.0);
            let next = geometry(water, true, 0.1);
            assert_ne!(boiling.positions, next.positions);
            assert_eq!(boiling.positions.len(), cold.positions.len() + BUBBLES * 17);
            assert!(
                boiling
                    .positions
                    .iter()
                    .any(|point| point[1] > liquid_height(water) + 0.01)
            );
        }
        assert!(geometry(0, true, 1.0).positions.is_empty());
    }

    #[test]
    fn bubbling_is_bounded_inside_every_fill_level_and_leaves_a_fixed_wall_seam() {
        for water in 1..=3 {
            let height = liquid_height(water);
            let radius = liquid_radius(height);
            for time in [0.0, 0.25, 0.6, 0.9, 1.4, 20.0] {
                let geometry = geometry(water, true, time);
                assert!(geometry.positions.len() <= 167);
                assert!(geometry.indices.len() <= 768);
                assert_eq!(geometry.positions.len(), geometry.colors.len());
                assert_eq!(geometry.positions.len(), geometry.normals.len());
                for point in &geometry.positions {
                    assert!(point.iter().all(|value| value.is_finite()));
                    assert!((height - 0.005..height + 0.033).contains(&point[1]));
                    assert!(Vec2::new(point[0] - 0.5, point[2] - 0.5).length() <= radius + 1e-6);
                }
                for point in &geometry.positions[1 + (RINGS - 1) * SIDES..=RINGS * SIDES] {
                    assert!((point[1] - height).abs() < 1e-6);
                }
                for triangle in geometry.indices.chunks_exact(3) {
                    let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                        .map(|index| Vec3::from_array(geometry.positions[index as usize]));
                    assert!((b - a).cross(c - a).y > 0.0);
                }
            }
        }
    }
}
