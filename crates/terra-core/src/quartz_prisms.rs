//! Large quartz outcrops share one terraced, quarter-block silhouette across
//! mineable cells. Both draw distances use the same cached slice boundaries.
use bevy_math::{IVec3, Vec2, Vec3};
use std::sync::LazyLock;

pub const fn is_quartz(id: u16) -> bool {
    matches!(id, 770..=773)
}
pub const fn diameter(id: u16) -> u8 {
    if id <= 771 { 5 } else { 3 }
}
pub const fn head(id: u16) -> u8 {
    if id <= 771 { 6 } else { 3 }
}
pub fn code(id: u16, x: u8, z: u8, layer: u8) -> u8 {
    let d = diameter(id);
    4 + x + z * d + layer * d * d
}

/// A fracture face becomes visible when its particular adjoining slice is mined.
pub fn joins(id: u16, face: u8, side: usize, neighbor: u8) -> bool {
    if face < 4 || neighbor < 4 {
        return false;
    }
    let d = i16::from(diameter(id));
    let current = i16::from(face - 4);
    let [x, z, l] = [current % d, (current / d) % d, current / (d * d)];
    let n = i16::from(neighbor - 4);
    let [nx, nz, nl] = [n % d, (n / d) % d, n / (d * d)];
    match side {
        0 => x - 1 == nx && z == nz && l == nl,
        1 => x + 1 == nx && z == nz && l == nl,
        2 => x == nx && z == nz && (l - 1 == nl || l == 0 && nl == 0),
        3 => x == nx && z == nz && (l + 1 == nl || l == 0 && nl == 0),
        4 => z - 1 == nz && x == nx && l == nl,
        5 => z + 1 == nz && x == nx && l == nl,
        _ => false,
    }
}

#[derive(Debug)]
pub struct Face {
    pub vertices: [Vec3; 4],
    pub normal: Vec3,
    /// Original cell boundary; omitted when the adjacent crystal continues.
    pub boundary: Option<usize>,
    pub facet: usize,
}
#[derive(Debug)]
pub struct Slice {
    pub faces: Vec<Face>,
    pub bounds: [Vec3; 2],
}

static LARGE: LazyLock<Vec<Option<Slice>>> = LazyLock::new(|| cache(770));
static LANCE: LazyLock<Vec<Option<Slice>>> = LazyLock::new(|| cache(772));
pub fn slice(id: u16, facing: u8) -> Option<&'static Slice> {
    if !is_quartz(id) || facing < 4 {
        return None;
    }
    let cache = if diameter(id) == 5 { &LARGE } else { &LANCE };
    cache.get(usize::from(facing - 4)).and_then(Option::as_ref)
}

const STEPS: usize = 4;

fn cache(id: u16) -> Vec<Option<Slice>> {
    let diameter = usize::from(diameter(id));
    let width = diameter * STEPS;
    let head = usize::from(head(id));
    let shear = if diameter == 5 {
        Vec2::new(1.18, 0.53)
    } else {
        Vec2::new(-0.91, -0.44)
    };
    let center = Vec2::splat(diameter as f32 * 0.5);
    let radius = center.x - 0.06;
    let direction = |i: f32| Vec2::from_angle((i + 0.22) * std::f32::consts::TAU / 6.);
    let normals = std::array::from_fn::<_, 6, _>(|i| direction(i as f32));
    let extent = (0..6)
        .map(|i| direction(i as f32 + 0.5).abs())
        .fold(Vec2::ZERO, Vec2::max);
    let half = 0.5 / STEPS as f32;
    let mut heights = vec![0; width * width];
    // Intersect each square with the six prism planes and the two grid axes.
    // Conservative overlap preserves even the thin corner slices in old worlds.
    for y in 0..(head + 1) * STEPS {
        let taper = y.saturating_sub(STEPS) as f32 / (head * STEPS) as f32;
        let r = radius * (1. - taper);
        for z in 0..width {
            for x in 0..width {
                let point = Vec2::new(x as f32 + 0.5, z as f32 + 0.5) / STEPS as f32;
                let delta = point - (center + shear * taper);
                if (delta.abs() - Vec2::splat(half)).cmplt(extent * r).all()
                    && normals
                        .iter()
                        .all(|n| n.dot(delta) - half * n.abs().element_sum() < r * 0.866_025_4)
                {
                    heights[x + z * width] = y as i32 + 1;
                }
            }
        }
    }
    let occupied = |p: IVec3| {
        (0..width as i32).contains(&p.x)
            && (0..width as i32).contains(&p.z)
            && p.y.max(0) < heights[p.x as usize + p.z as usize * width]
    };
    let mut slices = Vec::new();
    for layer in 0..=head {
        for z in 0..diameter {
            for x in 0..diameter {
                let origin = IVec3::new(x as i32, layer as i32, z as i32) * STEPS as i32;
                slices.push(build(origin, &occupied));
            }
        }
    }
    slices
}

fn build(origin: IVec3, occupied: &impl Fn(IVec3) -> bool) -> Option<Slice> {
    let mut faces = Vec::new();
    let mut bounds = [Vec3::ONE, Vec3::ZERO];
    crate::voxel_surface::visit_cell_faces::<STEPS>(
        |local| occupied(origin + local),
        |vertices, normal, side, cut| {
            for &p in &vertices {
                bounds[0] = bounds[0].min(p);
                bounds[1] = bounds[1].max(p);
            }
            faces.push(Face {
                vertices,
                normal,
                boundary: cut.then_some(side),
                facet: side,
            });
        },
    );
    (!faces.is_empty()).then_some(Slice { faces, bounds })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepped_crowns_retain_every_persisted_quartz_slice() {
        // Rows correspond to the shaft and each successive layer of the crown.
        // Bit x + z * diameter is set for each slice stored by the original generator.
        for (id, rows) in [
            (
                770,
                &[
                    0x01efffef, 0x01efffef, 0x01efffce, 0x00cf7bc0, 0x000f7980, 0x00067000,
                    0x00042000,
                ][..],
            ),
            (772, &[0x1ff, 0x1ff, 0xff, 0x1b][..]),
        ] {
            let width = diameter(id);
            for (layer, expected) in rows.iter().enumerate() {
                let mut mask = 0;
                for z in 0..width {
                    for x in 0..width {
                        if slice(id, code(id, x, z, layer as u8)).is_some() {
                            mask |= 1 << (x + z * width);
                        }
                    }
                }
                assert_eq!(mask, *expected, "layer {layer} of material {id}");
            }
        }
    }
}
