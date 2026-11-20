//! Curved plated voxel predator: all rigid parts are sculpted once and cached.
use super::*;
const SAND: [f32; 3] = [0.71, 0.48, 0.25];
const CREAM: [f32; 3] = [0.83, 0.72, 0.51];
const PLATE: [f32; 3] = [0.54, 0.29, 0.14];
const DUNE: [f32; 3] = [0.918, 0.839, 0.710];
const IVORY: [f32; 3] = [0.94, 0.87, 0.69];

use crate::players::model::Builder as Facets;
use std::f32::consts::{FRAC_PI_2, TAU};

fn radial(angle: f32) -> Vec3 {
    Vec3::new(angle.cos(), angle.sin(), 0.0)
}

fn tint(color: [f32; 3], brightness: f32) -> [f32; 3] {
    color.map(|v| (v * brightness).min(1.0))
}

/// A segment is a single continuous cuticle. Longitudinal grooves divide the
/// broad forward armor into plates; the narrow folded skin behind it remains
/// exposed when the next segment rolls through a bend.
fn segment(index: u8) -> Result<(Part, Vec3, Mesh), String> {
    use terra_core::creatures::{CreatureBehavior, sandworm};
    let sides: usize = match index {
        0 | 1 => 9,
        2 | 3 => 12,
        _ => 18,
    };
    let radius = sandworm::segment_radius(usize::from(index));
    let mut mesh = Facets::default();
    // Axial position, radius, offset of the scalloped plate rim, groove depth.
    let profile = [
        (-0.80, 0.82, 0.00, 0.00),
        (-0.58, 0.97, 0.08, 0.03),
        (-0.26, 1.05, 0.12, 0.055),
        (0.10, 1.09, 0.16, 0.075),
        (0.19, 1.04, 0.16, 0.06),
        (0.28, 0.88, 0.14, 0.02),
        (0.42, 0.93, 0.08, 0.00),
        (0.54, 0.86, 0.04, 0.00),
        (0.80, 0.80, 0.00, 0.00),
    ];
    let phase = f32::from(index) * 2.399_963;
    let ring = |side: usize, (depth, r, rim, groove): (f32, f32, f32, f32)| {
        let a = side as f32 * TAU / sides as f32 + phase;
        let seam = if side.is_multiple_of(3) { 1.0 } else { 0.0 };
        let z = depth + rim * (0.31 * (a * 3.0 + phase).sin() - seam * 0.80);
        let taper = if index == 0 { (0.84 - z).min(1.0) } else { 1.0 };
        let r = radius * taper * (r - seam * groove);
        Vec3::new(a.cos() * 0.84 * r, a.sin() * 0.75 * r, z)
    };
    for side in 0..sides {
        let next = (side + 1) % sides;
        let a = (side as f32 + 0.5) * TAU / sides as f32 + phase;
        let ventral = ((-a.sin() - 0.18) / 0.82).clamp(0.0, 1.0);
        let plate =
            Vec3::from_array([0.73, 0.54, 0.38]).lerp(Vec3::from_array(CREAM), ventral * 0.8);
        let wear = 0.99 + 0.09 * ((side / 3) as f32 * 1.77 + phase).cos();
        for (band, pair) in profile.windows(2).enumerate() {
            let color = match band {
                0 => tint(PLATE, 0.97),
                1..=4 => tint(plate.to_array(), wear * [0.86, 1.0, 1.10, 0.82][band - 1]),
                5 => [0.45, 0.30, 0.20],
                6 => [0.57, 0.40, 0.28],
                _ => [0.43, 0.28, 0.19],
            };
            let face = [
                ring(side, pair[0]),
                ring(next, pair[0]),
                ring(next, pair[1]),
                ring(side, pair[1]),
            ];
            if side % 3 == 1 && band == 2 {
                // A broad low keel is part of each shield, over the suture
                // beneath it. Its edges use precisely the skin vertices.
                let crest = face[0].lerp(face[2], 0.42) + radial(a) * (0.055 * radius);
                mesh.faceted_quad(face, crest, color)?;
            } else {
                mesh.quad(face, color)?;
            }
        }
        mesh.triangle(
            [
                Vec3::Z * -0.84,
                ring(next, profile[0]),
                ring(side, profile[0]),
            ],
            PLATE,
        )?;
        mesh.triangle(
            [
                Vec3::Z * 0.84,
                ring(side, profile[8]),
                ring(next, profile[8]),
            ],
            PLATE,
        )?;
    }
    let center = sandworm::spine_pose(CreatureBehavior::Attack, 0.0, usize::from(index)).center;
    Ok((
        Part::WormSegment(index),
        Vec3::from_array(center),
        mesh.finish()?,
    ))
}

fn head_origin() -> Vec3 {
    use terra_core::creatures::{CreatureBehavior, sandworm};
    Vec3::from_array(
        sandworm::spine_pose(CreatureBehavior::Attack, 0.0, sandworm::HEAD_SEGMENT).center,
    )
}

const GUM: [f32; 3] = [0.66, 0.28, 0.21];

fn head() -> Result<(Part, Vec3, Mesh), String> {
    let mut mesh = Facets::default();
    // Follow the inside of the pharynx out to the lip, then return down the
    // outside to the neck. The mouth has depth even between the moving jaws.
    let profile = [
        (0.36, 0.18, [0.18, 0.054, 0.042]),
        (0.30, 0.28, [0.35, 0.10, 0.10]),
        (0.17, 0.38, [0.45, 0.16, 0.12]),
        (0.02, 0.43, [0.33, 0.095, 0.073]),
        (-0.14, 0.54, [0.57, 0.20, 0.15]),
        (-0.32, 0.59, [0.54, 0.18, 0.12]),
        (-0.52, 0.69, GUM),
        (-0.45, 0.77, [0.62, 0.39, 0.28]),
        (-0.13, 0.82, [0.65, 0.45, 0.31]),
        (0.19, 0.86, SAND),
        (0.33, 0.84, [0.67, 0.45, 0.31]),
        (0.43, 0.76, [0.51, 0.33, 0.23]),
        (0.66, 0.78, SAND),
        (0.89, 0.68, PLATE),
    ];
    const SIDES: usize = 36;
    let point = |side: usize, band: usize| {
        let a = side as f32 * TAU / SIDES as f32;
        let (z, radius, _): (f32, f32, _) = profile[band];
        let pleat = if band < 7 { 0.037 } else { 0.022 };
        let scallop = (3.0 * (a - FRAC_PI_2)).cos();
        let lip = if (5..=7).contains(&band) {
            0.065 * scallop
        } else {
            0.045 * scallop * (1.0 - z.abs())
        };
        radial(a) * (radius + pleat * (9.0 * a + 3.0 * z).cos()) + Vec3::Z * (z - lip)
    };
    for i in 0..SIDES {
        let n = (i + 1) % SIDES;
        mesh.triangle(
            [Vec3::Z * 0.38, point(n, 0), point(i, 0)],
            [0.14, 0.045, 0.038],
        )?;
        for (band, (_, _, color)) in profile[..profile.len() - 1].iter().enumerate() {
            let shade = if band < 6 {
                [1.08, 1.02, 0.89, 0.86][i % 4]
            } else {
                1.0
            };
            mesh.quad(
                [
                    point(i, band),
                    point(n, band),
                    point(n, band + 1),
                    point(i, band + 1),
                ],
                tint(*color, shade),
            )?;
        }
    }
    Ok((Part::Head, head_origin(), mesh.finish()?))
}

/// A broad root, a forward-projecting bend and one inward-pointing tip.
/// Five-sided cross sections retain the faceted ivory silhouette at any zoom.
fn fang(
    mesh: &mut Facets,
    angle: f32,
    root: Vec3,
    length: f32,
    width: f32,
    color: [f32; 3],
) -> Result<(), String> {
    let inward = -radial(angle);
    let tangent = radial(angle + FRAC_PI_2);
    let ring = |stage: usize| {
        let (advance, depth, size) =
            [(0.0, 0.0, 1.0), (0.21, -0.38, 0.84), (0.61, -0.47, 0.52)][stage];
        let center = root + (inward * advance + Vec3::Z * depth) * length;
        std::array::from_fn::<_, 5, _>(|i| {
            let a = i as f32 * TAU / 5.0;
            center + (tangent * a.cos() + Vec3::Z * a.sin() * 0.68) * (width * size)
        })
    };
    let base = ring(0);
    for i in 1..4 {
        mesh.triangle([base[0], base[i], base[i + 1]], tint(color, 0.71))?;
    }
    for stage in 0..2 {
        let a = ring(stage);
        let b = ring(stage + 1);
        for i in 0..5 {
            let n = (i + 1) % 5;
            mesh.quad(
                [a[n], a[i], b[i], b[n]],
                tint(color, if stage == 0 { 0.85 } else { 1.0 }),
            )?;
        }
    }
    let last = ring(2);
    let tip = root + (inward - Vec3::Z * 0.32) * length;
    for i in 0..5 {
        mesh.triangle([last[(i + 1) % 5], last[i], tip], color)?;
    }
    Ok(())
}

fn jaw() -> Result<(Part, Vec3, Mesh), String> {
    let mut mesh = Facets::default();
    for i in 0..9 {
        let a = (i as f32 + 0.4) * TAU / 9.0 + (i as f32 * 3.1).sin() * 0.035;
        fang(
            &mut mesh,
            a,
            radial(a) * 0.54 - Vec3::Z * 0.15,
            0.16 + 0.04 * (i as f32 * 1.9).cos(),
            0.042,
            [0.86, 0.66, 0.47],
        )?;
    }
    Ok((Part::Jaw, head_origin(), mesh.finish()?))
}

pub(in crate::creatures) fn mandible_hinge(index: u8) -> (Vec3, Vec3) {
    let angle = FRAC_PI_2 + f32::from(index) * TAU / 3.0;
    (
        radial(angle) * 0.78 - Vec3::Z * 0.12,
        radial(angle + FRAC_PI_2),
    )
}

/// Three muscular lobes hold the primary fangs. Each one tapers into the
/// neck at its hinge and folds forward near its centre, leaving a scalloped
/// lip in profile and exposing the smaller teeth deeper inside the throat.
fn mandible(index: u8) -> Result<(Part, Vec3, Mesh), String> {
    let mut mesh = Facets::default();
    let center_angle = FRAC_PI_2 + f32::from(index) * TAU / 3.0;
    let lobe = |u: f32| (1.0 - u * u) * (1.0 + 0.12 * u);
    let lip = |u: f32| -0.36 - 0.36 * lobe(u);
    let profile = |u: f32| {
        let r = radial(center_angle + u * 0.97);
        let pad = lobe(u);
        let z = Vec3::Z * lip(u);
        // Two low ribs feed from the muscular hinge into the tough lip.
        let rib = (u * 6.6 + 0.36).cos() * pad;
        [
            r * (0.83 - 0.036 * pad) + Vec3::Z * (0.17 + 0.33 * pad),
            r * (0.81 + 0.09 * pad + rib * 0.039) + Vec3::Z * (0.02 + 0.04 * pad),
            r * (0.78 + 0.12 * pad + rib * 0.051) + z * (0.57 + rib * 0.07),
            r * (0.75 + 0.055 * pad) + z,
            r * (0.69 - 0.025 * pad) + z - Vec3::Z * 0.025,
            r * 0.64 + z + Vec3::Z * 0.11,
        ]
    };
    for i in 0..12 {
        let a = profile(i as f32 / 6.0 - 1.0);
        let b = profile((i + 1) as f32 / 6.0 - 1.0);
        for (j, color) in [
            PLATE,
            SAND,
            [0.76, 0.57, 0.39],
            [0.67, 0.39, 0.29],
            GUM,
            PLATE,
        ]
        .into_iter()
        .enumerate()
        {
            let n = (j + 1) % a.len();
            let face = [a[n], b[n], b[j], a[j]];
            let u = (i as f32 + 0.5) / 6.0 - 1.0;
            if j == 1 || j == 2 {
                let relief = (u * 6.6 + 0.36).cos() * lobe(u);
                let ridge = face[0].lerp(face[2], 0.48)
                    + radial(center_angle + u * 0.97) * (0.017 + 0.018 * relief);
                mesh.faceted_quad(face, ridge, tint(color, 0.98 + 0.12 * relief))?;
            } else {
                mesh.quad(face, tint(color, 0.97 + 0.035 * (i as f32 * 1.57).cos()))?;
            }
        }
        if i == 0 || i == 11 {
            let end = if i == 0 { a } else { b };
            for j in 1..end.len() - 1 {
                let (j, k) = if i == 0 { (j + 1, j) } else { (j, j + 1) };
                mesh.triangle([end[0], end[j], end[k]], PLATE)?;
            }
        }
    }
    let fangs: &[(f32, f32)] = match index {
        0 => &[
            (-0.85, 0.15),
            (-0.45, 0.29),
            (0.05, 0.40),
            (0.55, 0.22),
            (0.87, 0.14),
        ],
        1 => &[(-0.85, 0.21), (-0.30, 0.36), (0.25, 0.28), (0.78, 0.18)],
        _ => &[(-0.80, 0.15), (-0.33, 0.27), (0.18, 0.37), (0.73, 0.22)],
    };
    for &(u, length) in fangs {
        let a = center_angle + u * 0.97;
        fang(
            &mut mesh,
            a,
            radial(a) * 0.68 + Vec3::Z * lip(u),
            length,
            0.028 + 0.13 * length,
            IVORY,
        )?;
    }
    Ok((Part::WormMandible(index), head_origin(), mesh.finish()?))
}

fn spit(index: u8) -> Result<(Part, Vec3, Mesh), String> {
    let mut b = Builder::with_cell_size(0.045);
    let size = 0.12 - f32::from(index) * 0.025;
    b.ellipsoid(
        Vec3::ZERO,
        Vec3::new(size, size * 0.84, size * 1.55),
        [0.68, 0.78, 0.055],
    )?;
    b.ellipsoid(
        Vec3::new(-size * 0.25, size * 0.35, -size * 0.30),
        Vec3::splat(size * 0.34),
        [0.94, 0.92, 0.28],
    )?;
    Ok((Part::WormSpit(index), Vec3::ZERO, b.finish()?))
}

fn mound() -> Result<(Part, Vec3, Mesh), String> {
    let mut b = Builder::with_cell_size(0.07);
    b.ellipsoid(
        Vec3::new(0.0, -0.14, 0.07),
        Vec3::new(0.96, 0.20, 0.83),
        DUNE,
    )?;
    for i in 0..7 {
        let angle = i as f32 * 2.399_963_1;
        let r = 0.74 + (i % 3) as f32 * 0.09;
        b.ellipsoid(
            Vec3::new(angle.sin() * r, -0.12, angle.cos() * r),
            Vec3::new(0.27, 0.15, 0.24),
            DUNE,
        )?;
    }
    Ok((Part::WormMound, Vec3::ZERO, b.finish()?))
}

fn dust() -> Result<(Part, Vec3, Mesh), String> {
    // A fixed physical cell size prevents 10 cm grains being inflated into
    // half-metre blocks when the entire particle ring is voxelized at once.
    let mut b = Builder::with_cell_size(0.055);
    for i in 0..18 {
        let angle = i as f32 * 2.399_963_1;
        let radius = 1.09 + (i % 5) as f32 * 0.12;
        b.voxel(
            [
                angle.sin() * radius,
                0.04 + (i % 4) as f32 * 0.10,
                angle.cos() * radius * 0.86,
            ],
            [
                0.055 + (i % 3) as f32 * 0.025,
                0.065 + (i % 2) as f32 * 0.025,
                0.065,
            ],
            DUNE.map(|channel| channel * if i % 2 == 0 { 1.0 } else { 0.94 }),
        )?;
    }
    Ok((Part::WormDust, Vec3::ZERO, b.finish()?))
}

/// A shallow annular ridge sunk into the dune. Three cached copies travel
/// outward like displaced sand, without any chunk edits or per-frame remeshing.
fn ripple(index: u8) -> (Part, Vec3, Mesh) {
    use bevy::{
        asset::RenderAssetUsages,
        mesh::{Indices, PrimitiveTopology},
    };
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    const STEPS: usize = 4;
    let sector = f32::from(index % 8) * std::f32::consts::FRAC_PI_4;
    let profile = [
        (-0.23, -0.055),
        (-0.085, 0.02),
        (0.0, 0.15),
        (0.11, 0.015),
        (0.24, -0.055),
    ];
    for step in 0..=STEPS {
        let angle = (step as f32 / STEPS as f32 - 0.5) * std::f32::consts::FRAC_PI_4;
        for (edge, (radial, height)) in profile.iter().enumerate() {
            let radius = 1.20 + radial + ((angle + sector) * 5.0).sin() * 0.018;
            positions.push([angle.sin() * radius, *height, angle.cos() * radius - 1.20]);
            let slope = [-0.52, -1.0, 0.0, 0.81, 0.54][edge];
            normals.push(
                Vec3::new(angle.sin() * slope, 1.0, angle.cos() * slope)
                    .normalize()
                    .to_array(),
            );
            let tint = 0.98 + ((step * 7 + edge * 3) % 9) as f32 * 0.004;
            colors.push(
                Color::srgb(DUNE[0] * tint, DUNE[1] * tint, DUNE[2] * tint)
                    .to_linear()
                    .to_f32_array(),
            );
            if edge < 4 && step < STEPS {
                let a = (step * 5 + edge) as u32;
                let b = ((step + 1) * 5 + edge) as u32;
                indices.extend([a, b + 1, b, a, a + 1, b + 1]);
            }
        }
    }
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0_f32; 2]; (STEPS + 1) * 5])
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices));
    (Part::WormRipple(index), Vec3::ZERO, mesh)
}

pub(super) fn parts() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut out = (0..terra_core::creatures::sandworm::BODY_SEGMENTS as u8)
        .map(segment)
        .collect::<Result<Vec<_>, _>>()?;
    out.extend([head()?, jaw()?, mound()?, dust()?]);
    out.extend((0..3).map(mandible).collect::<Result<Vec<_>, _>>()?);
    out.extend((0..3).map(spit).collect::<Result<Vec<_>, _>>()?);
    out.extend((0..24).map(ripple));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;
    fn points(mesh: &Mesh) -> &Vec<[f32; 3]> {
        let Some(VertexAttributeValues::Float32x3(points)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!()
        };
        points
    }
    #[test]
    fn cached_voxel_worm_has_a_long_tapered_body_open_maw_and_cached_bile_droplets() {
        let parts = parts().unwrap();
        assert_eq!(parts.len(), 58);
        let origins: Vec<_> = parts
            .iter()
            .filter_map(|(p, origin, _)| matches!(p, Part::WormSegment(_)).then_some(*origin))
            .collect();
        assert_eq!(
            origins.len(),
            terra_core::creatures::sandworm::BODY_SEGMENTS
        );
        assert!(origins[0].z - origins[23].z > 20.0);
        assert!(origins[23].y - origins[18].y > 1.5);
        let widths: Vec<_> = parts
            .iter()
            .filter(|(part, _, _)| matches!(part, Part::WormSegment(_)))
            .map(|(_, _, mesh)| {
                let extent = points(mesh).iter().fold(Vec3::ZERO, |extent, p| {
                    extent.max(Vec3::from_array(*p).abs())
                });
                extent.x * 2.0
            })
            .collect();
        assert!(
            widths[0] < widths[5] * 0.20,
            "tail must taper to a clean tip"
        );
        assert_eq!(
            parts
                .iter()
                .filter(|(part, _, _)| matches!(part, Part::WormMandible(_)))
                .count(),
            3
        );
        assert_eq!(
            parts
                .iter()
                .filter(|(part, _, _)| matches!(part, Part::WormSpit(_)))
                .count(),
            3
        );
        let vertices: usize = parts.iter().map(|(_, _, m)| m.count_vertices()).sum();
        assert!(vertices < 24000, "worm vertices {vertices}");
        let (_, offset, head) = parts
            .iter()
            .find(|(p, _, _)| matches!(p, Part::Head))
            .unwrap();
        assert!(points(head).iter().any(|p| p[2] < -0.50));
        assert!(
            points(head)
                .iter()
                .all(|p| (Vec3::from_array(*p) + *offset).y < 7.9)
        );
    }
    #[test]
    fn warning_grains_stay_small_and_the_mound_has_no_square_platform_corners() {
        let parts = parts().unwrap();
        let (_, _, dust) = parts
            .iter()
            .find(|(p, _, _)| matches!(p, Part::WormDust))
            .unwrap();
        let positions = points(dust);
        for triangle in dust
            .indices()
            .unwrap()
            .iter()
            .collect::<Vec<_>>()
            .chunks_exact(3)
        {
            let [a, b, c] =
                [triangle[0], triangle[1], triangle[2]].map(|i| Vec3::from_array(positions[i]));
            assert!((a - b).abs().max_element().max((b - c).abs().max_element()) < 0.25);
        }
        let (_, _, mound) = parts
            .iter()
            .find(|(p, _, _)| matches!(p, Part::WormMound))
            .unwrap();
        assert!(points(mound).iter().any(|p| p[1] < -0.25));
        assert!(
            points(mound)
                .iter()
                .all(|p| !(p[0].abs() > 1.05 && p[2].abs() > 1.00))
        );
    }

    #[test]
    fn radial_maw_keeps_a_real_front_opening_and_a_recessed_throat() {
        let (_, _, head) = head().unwrap();
        let positions = points(&head);
        assert!(
            positions
                .iter()
                .filter(|p| p[2] < -0.35)
                .all(|p| p[0].hypot(p[1]) > 0.42),
            "the wide central aperture must not be blocked by teeth or a snout cube"
        );
        // Greedy quads do not require a vertex at their centre. Ray-test the
        // actual faces instead of inferring an opening from vertex positions.
        let indices: Vec<_> = head.indices().unwrap().iter().collect();
        let first_at = |origin: Vec3| {
            let mut first = (f32::INFINITY, 0usize);
            for tri in indices.chunks_exact(3) {
                let [a, b, c] = std::array::from_fn(|i| Vec3::from_array(positions[tri[i]]));
                let direction = Vec3::Z;
                let e1 = b - a;
                let e2 = c - a;
                let p = direction.cross(e2);
                let det = e1.dot(p);
                if det.abs() < 1e-7 {
                    continue;
                }
                let t = origin - a;
                let u = t.dot(p) / det;
                let q = t.cross(e1);
                let v = direction.dot(q) / det;
                let distance = e2.dot(q) / det;
                if u >= -1e-5
                    && v >= -1e-5
                    && u + v <= 1.00001
                    && distance > 0.0
                    && distance < first.0
                {
                    first = (distance, tri[0]);
                }
            }
            first
        };
        let centre = first_at(Vec3::new(0.0, 0.0, -2.0));
        assert!(
            (2.25..2.42).contains(&centre.0),
            "centre ray must reach the recessed throat, got {}",
            centre.0
        );
        let Some(VertexAttributeValues::Float32x4(colors)) = head.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("worm colors")
        };
        assert!(
            colors[centre.1][..3].iter().all(|channel| *channel < 0.12),
            "the centre must show a dark throat rather than an occluding armor plate"
        );
        // Angular gaps may reveal the warm armor lip or recessed brown inner
        // row. Only an ivory hit in the front slice would indicate that the
        // outer fangs have merged into the old white gear shape.
        for i in 0..12 {
            let gap = (i as f32 + 0.75) * std::f32::consts::TAU / 12.0;
            let origin = Vec3::new(gap.cos() * 0.54, gap.sin() * 0.54, -2.0);
            let hit = first_at(origin);
            let color = colors[hit.1];
            let ivory = color[0] > 0.1 && color[1] > color[0] * 0.75 && color[2] > color[0] * 0.35;
            assert!(
                hit.0 > 1.49 || !ivory,
                "front ivory fangs must stay separated: gap={i}, depth={}, color={color:?}",
                hit.0
            );
        }
    }
    #[test]
    fn continuous_scalloped_chitin_girdles_preserve_the_tapered_seal() {
        let (_, _, ring) = segment(12).unwrap();
        let (_, _, neck) = segment(23).unwrap();
        assert!(
            ring.count_vertices() < 1_300,
            "body armor must fit the per-ring mesh budget"
        );
        assert!(points(&ring).iter().any(|p| p[0].abs() > 0.85));
        assert!(
            points(&neck)
                .iter()
                .all(|p| p[2] >= -0.841 && p[2] <= 0.841)
        );
        let neck_radius = points(&neck).iter().map(|p| p[0].abs()).fold(0.0, f32::max);
        assert!(
            (0.79..0.89).contains(&neck_radius),
            "neck should carry the width of the maw, got {neck_radius}"
        );
    }

    #[test]
    fn the_cuticle_is_one_closed_shell_even_between_the_scalloped_plates() {
        use std::collections::HashMap;
        let (_, _, mesh) = segment(21).unwrap();
        let vertices = points(&mesh);
        let mut edges = HashMap::new();
        let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
        for triangle in indices.chunks_exact(3) {
            for side in 0..3 {
                let a = vertices[triangle[side]].map(f32::to_bits);
                let b = vertices[triangle[(side + 1) % 3]].map(f32::to_bits);
                *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
        assert!(edges.values().all(|&adjacent| adjacent == 2));
    }

    #[test]
    fn actual_animated_body_surfaces_stay_behind_the_recessed_throat() {
        use terra_core::creatures::{CreatureBehavior as B, sandworm as pose};
        let segments: Vec<_> = (0..pose::BODY_SEGMENTS)
            .map(|i| segment(i as u8).unwrap().2)
            .collect();
        for behavior in [B::Leap, B::Emerge, B::Attack, B::Spit, B::Retreat] {
            for progress in (0..=32)
                .map(|i| i as f32 / 32.0)
                .chain([pose::BITE_PHASE, pose::SPIT_PHASE])
            {
                let head = pose::spine_pose(behavior, progress, pose::HEAD_SEGMENT);
                if !pose::anatomy_visible(behavior, head, pose::HEAD_SEGMENT) {
                    continue;
                }
                let inverse = Quat::from_rotation_x(-head.pitch);
                for (i, mesh) in segments.iter().enumerate() {
                    for clock in [0.0, 1.7, 4.3] {
                        let ring = pose::spine_pose_at(behavior, progress, clock, i);
                        if !pose::anatomy_visible(behavior, ring, i) {
                            continue;
                        }
                        let pulse = (clock * 3.0 - i as f32 * 0.65).sin();
                        let scale =
                            Vec3::new(1.0 + pulse * 0.045, 1.0 + pulse * 0.055, 1.0 - pulse * 0.05);
                        let rotation = Quat::from_rotation_x(ring.pitch);
                        for p in points(mesh) {
                            let local = inverse
                                * (Vec3::from_array(ring.center)
                                    + rotation * (Vec3::from_array(*p) * scale)
                                    - Vec3::from_array(head.center));
                            assert!(
                                local.x.hypot(local.y) > 0.50 || local.z > pose::THROAT_BACKPLANE,
                                "body mesh reaches mouth: {behavior:?}, phase={progress}, ring={i}, point={local:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}
