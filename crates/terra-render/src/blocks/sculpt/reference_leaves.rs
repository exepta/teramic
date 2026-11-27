//! Blunt, notched oak sprays growing from the reference canopy.
use super::patch::FacePatch;
use crate::blocks::PrimitiveTemplate;
use bevy::math::{Vec2, Vec3};
use std::collections::HashMap;

// Blade half-width at successive positions along the midrib. The nearly
// vertical steps between the fingers form real concave corners. The stalk is
// continuous with the blade and has its own narrow segment at the base.
const THREE_LOBES: &[(f32, f32)] = &[
    (0.00, 0.10),
    (0.15, 0.10),
    (0.19, 0.40),
    (0.21, 0.90),
    (0.27, 1.00),
    (0.48, 1.00),
    (0.52, 0.77),
    (0.53, 0.37),
    (0.68, 0.37),
    (0.70, 0.55),
    (0.92, 0.55),
    (1.00, 0.39),
];
const FIVE_LOBES: &[(f32, f32)] = &[
    (0.00, 0.10),
    (0.14, 0.10),
    (0.18, 0.43),
    (0.20, 0.92),
    (0.25, 1.00),
    (0.37, 1.00),
    (0.39, 0.43),
    (0.49, 0.43),
    (0.51, 0.83),
    (0.69, 0.83),
    (0.72, 0.35),
    (0.80, 0.35),
    (0.82, 0.48),
    (0.95, 0.48),
    (1.00, 0.32),
];

// Contiguous patches of pigment in the reference leaf atlas, from deep green
// to sunny chartreuse. Each individual blade retains a coherent colour.
const PIGMENTS: [[f32; 2]; 8] = [
    [0.76, 0.29],
    [0.92, 0.84],
    [0.14, 0.53],
    [0.92, 0.53],
    [0.92, 0.37],
    [0.45, 0.29],
    [0.68, 0.29],
    [0.45, 0.76],
];

pub(super) fn face(side: usize, variant: usize) -> Vec<PrimitiveTemplate> {
    sprays(side, variant, false, None)
}

pub(super) fn distant_face(side: usize, variant: usize) -> Vec<PrimitiveTemplate> {
    sprays(side, variant, true, None)
}

pub(crate) struct ShadowCard {
    pub positions: [[f32; 3]; 6],
    pub stations: u16,
}

pub(crate) fn shadow_cards(side: usize, variant: usize, distant: bool) -> &'static [ShadowCard] {
    use std::sync::OnceLock;
    static CARDS: [[[OnceLock<Vec<ShadowCard>>; 6]; 4]; 2] =
        [const { [const { [const { OnceLock::new() }; 6] }; 4] }; 2];
    CARDS[usize::from(distant)][variant][side].get_or_init(|| {
        let mut cards = Vec::new();
        sprays(side, variant, distant, Some(&mut cards));
        cards
    })
}

fn sprays(
    side: usize,
    variant: usize,
    distant: bool,
    mut cards: Option<&mut Vec<ShadowCard>>,
) -> Vec<PrimitiveTemplate> {
    let mut patch = FacePatch::new(side, variant);
    let canopy_top = side == 3;
    // Roots are shared by the two or three blades in each spray. Offsetting
    // neighbouring rows leaves pockets through which the underlying foliage shows.
    let sprays: &[(f32, f32, f32, usize)] = if canopy_top {
        &[
            (0.17, 0.19, 0.86, 2),
            (0.52, 0.19, 1.60, 3),
            (0.79, 0.23, 0.46, 2),
            (0.20, 0.49, -0.48, 2),
            (0.71, 0.53, 2.38, 3),
            (0.28, 0.79, 2.54, 3),
            (0.62, 0.80, -0.68, 2),
            (0.87, 0.79, 2.05, 1),
        ]
    } else if side == 2 {
        &[
            (0.22, 0.22, 0.88, 2),
            (0.70, 0.36, 2.42, 2),
            (0.42, 0.75, -0.83, 2),
        ]
    } else {
        &[
            (0.16, 0.88, -1.23, 2),
            (0.42, 0.83, -1.92, 3),
            (0.76, 0.91, -1.05, 2),
            (0.86, 0.61, -2.11, 1),
            (0.24, 0.55, -1.65, 2),
            (0.58, 0.51, -1.26, 3),
            (0.19, 0.28, -1.20, 1),
            (0.77, 0.27, -1.80, 2),
        ]
    };
    let (u_axis, v_axis) = match side / 2 {
        0 => (2, 1),
        1 => (0, 2),
        _ => (0, 1),
    };
    for (i, &(u, v, angle, count)) in sprays.iter().enumerate() {
        let noise = |salt| variation(side, variant, i * 32 + salt);
        let root = Vec2::new(u, v) + Vec2::new(noise(1) - 0.5, noise(2) - 0.5) * 0.065;
        let mirror = (variant + side) % 2 != 0;
        let root = if mirror {
            Vec2::new(1.0 - root.x, root.y)
        } else {
            root
        };
        let card_start = cards.as_ref().map_or(0, |out| out.len());
        let start = patch.meshes.first().map_or(0, |m| m.positions.len());
        let (mut min, mut max) = (Vec2::ONE, Vec2::ZERO);
        for j in 0..count {
            let spread = if count == 1 {
                0.0
            } else {
                j as f32 / (count - 1) as f32 - 0.5
            };
            let angle = angle + spread * (1.45 + noise(3) * 0.40) + (noise(4 + j) - 0.5) * 0.45;
            let (sin, cos) = angle.sin_cos();
            let along = Vec2::new(if mirror { -cos } else { cos }, sin);
            let key = i * 5 + j;
            let length = 0.195 + 0.081 * noise(7 + j);
            let width = length * (0.32 + 0.09 * noise(10 + j));
            let height = if (key + variant + side) % 7 == 0 {
                0.151 + 0.013 * noise(13 + j)
            } else {
                0.038 + 0.101 * noise(13 + j)
            };
            let (blade_min, blade_max) = blade(
                &mut patch,
                root,
                along,
                length,
                width,
                height,
                key,
                distant,
                cards.as_deref_mut(),
            );
            min = min.min(blade_min);
            max = max.max(blade_max);
        }
        // Fit the whole spray as a unit so the roots still meet. UVs already
        // address independent pigment swatches and stay attached to each blade.
        let shift =
            (Vec2::splat(0.011) - min).max(Vec2::ZERO) - (max - Vec2::splat(0.989)).max(Vec2::ZERO);
        if let Some(out) = &mut cards {
            for p in out[card_start..]
                .iter_mut()
                .flat_map(|card| &mut card.positions)
            {
                p[u_axis] += shift.x;
                p[v_axis] += shift.y;
                if matches!(side, 1 | 4) {
                    p[u_axis] = 1.0 - p[u_axis];
                }
            }
        } else {
            for point in &mut patch.meshes[0].positions[start..] {
                point[u_axis] += shift.x;
                point[v_axis] += shift.y;
            }
        }
    }
    patch.meshes
}

fn variation(side: usize, variant: usize, key: usize) -> f32 {
    let mut bits = ((side * 4 + variant) as u32 + 1).wrapping_mul(0x85ebca77)
        ^ (key as u32).wrapping_mul(0x9e3779b9);
    bits ^= bits >> 16;
    bits = bits.wrapping_mul(0x7feb352d);
    ((bits ^ (bits >> 15)) & 0xffff) as f32 / 65535.0
}

#[allow(clippy::too_many_arguments)]
fn blade(
    patch: &mut FacePatch,
    root: Vec2,
    along: Vec2,
    length: f32,
    width: f32,
    height: f32,
    key: usize,
    distant: bool,
    cards: Option<&mut Vec<ShadowCard>>,
) -> (Vec2, Vec2) {
    let side = patch.side;
    let variant = patch.variant;
    let across = Vec2::new(-along.y, along.x);
    let profile = if (key + variant + side) % 3 == 0 {
        FIVE_LOBES
    } else {
        THREE_LOBES
    };
    let skew = variation(side, variant, key + 113) - 0.5;
    let point = |t: f32, w: f32, upper: bool| {
        // A slight shear staggers the lobes between the two sides. Leaning the
        // rim inwards leaves a small light-catching bevel along every edge.
        let wt = if w > 0.0 { w * (0.87 + skew * 0.2) } else { w };
        let edge = if upper { 0.93 } else { 1.0 };
        let p = root
            + along * (length * (t + (0.08 + 0.09 * skew) * wt))
            + across * (width * (wt * edge + 0.15 * t * skew));
        let h = height * t + 0.008 * t * (1.0 - t) - 0.008 * w.abs() * t;
        let thickness = 0.021 + 0.004 * t;
        [p.x, p.y, -(if upper { h + thickness } else { h })]
    };
    let begin = patch.meshes.first().map_or(0, |m| m.uvs.len());
    let rings: Vec<_> = profile
        .iter()
        .map(|&(t, w)| {
            [
                point(t, -w, false),
                point(t, w, false),
                point(t, -w, true),
                point(t, w, true),
            ]
            .map(Vec3::from_array)
        })
        .collect();
    // Use all authored stations for spray registration, including at the outer LOD.
    let mut min = Vec2::ONE;
    let mut max = Vec2::ZERO;
    for point in rings.iter().flatten() {
        min = min.min(point.truncate());
        max = max.max(point.truncate());
    }
    let stations: Vec<_> = if distant {
        simplify_stations(&rings)
    } else {
        (0..rings.len()).collect()
    };
    if let Some(cards) = cards {
        let positions = std::array::from_fn(|i| {
            let t = (i / 3) as f32;
            let w = (i % 3) as f32 - 1.;
            let wt = if w > 0. { w * (0.87 + skew * 0.2) } else { w };
            let p = root
                + along * (length * (t + (0.08 + 0.09 * skew) * wt))
                + across * (width * (wt * 0.965 + 0.15 * t * skew));
            let h = height * t + (0.021 + 0.004 * t) * 0.5 - 0.004 * t;
            let depth = if side % 2 == 0 { -h } else { 1. + h };
            match side / 2 {
                0 => [depth, p.y, p.x],
                1 => [p.x, depth, p.y],
                _ => [p.x, p.y, depth],
            }
        });
        let mask = stations.iter().fold(0u16, |mask, i| mask | (1u16 << i));
        cards.push(ShadowCard {
            positions,
            stations: mask | (u16::from(profile.len() == FIVE_LOBES.len()) << 15),
        });
        return (min, max);
    }
    for pair in stations.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let low = [rings[a][0], rings[b][0], rings[b][1], rings[a][1]].map(|p| p.to_array());
        let high = [rings[a][2], rings[b][2], rings[b][3], rings[a][3]].map(|p| p.to_array());
        patch.wound_polygon(side, high);
        patch.wound_polygon(side, [low[3], low[2], low[1], low[0]]);
        for (a, b) in [(0, 1), (2, 3)] {
            patch.wound_polygon(side, [low[a], low[b], high[b], high[a]]);
        }
        if a == 0 {
            patch.wound_polygon(side, [low[3], low[0], high[0], high[3]]);
        }
        if b == rings.len() - 1 {
            patch.wound_polygon(side, [low[1], low[2], high[2], high[1]]);
        }
    }
    let swatch = Vec2::from_array(PIGMENTS[(key * 3 + side + variant) % PIGMENTS.len()]);
    for uv in &mut patch.meshes[0].uvs[begin..] {
        *uv = (swatch + (Vec2::from_array(*uv) - root - along * length * 0.5) * 0.24).to_array();
    }
    (min, max)
}

// Each station carries both sides of the upper and lower outline. The stalk
// and the end cap remain pinned. Across a skipped station, all four rails
// must remain inside a 9 mm tube about the new edge in *three* dimensions.
const EDGE_ERROR: f32 = 0.009;
fn simplify_stations(rings: &[[Vec3; 4]]) -> Vec<usize> {
    let mut stations = vec![0, 1];
    let mut start = 1;
    while start + 1 < rings.len() {
        let end = (start + 2..rings.len())
            .rev()
            .find(|&end| {
                (start + 1..end).all(|mid| {
                    (0..4).all(|lane| {
                        edge_distance_squared(
                            rings[mid][lane],
                            rings[start][lane],
                            rings[end][lane],
                        ) <= EDGE_ERROR * EDGE_ERROR
                    })
                })
            })
            .unwrap_or(start + 1);
        stations.push(end);
        start = end;
    }
    stations
}

fn edge_distance_squared(p: Vec3, start: Vec3, end: Vec3) -> f32 {
    let edge = end - start;
    let offset = p - start;
    let along = (offset.dot(edge) / edge.length_squared()).clamp(0., 1.);
    (offset - edge * along).length_squared()
}

// At the outer edge of the modeled canopy, neighboring faces can share a lighting
// frame. Positions and UVs must match bit for bit. Keep sharp notches, cap/rim
// joins, UV seams and opposite normal-map handedness as separate vertex records.
pub(super) fn distant_facets(source: &PrimitiveTemplate) -> PrimitiveTemplate {
    const MIN_DOT: f32 = 0.965_926; // fifteen degrees to the first face
    let mut result = PrimitiveTemplate {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        tangents: Vec::new(),
        material: source.material,
        indices: Vec::with_capacity(source.indices.len()),
    };
    let mut buckets = HashMap::<_, Vec<usize>>::new();
    let mut sums = Vec::<(Vec3, Vec3)>::new();
    let mut remap = Vec::with_capacity(source.positions.len());
    for (i, &position) in source.positions.iter().enumerate() {
        let uv = source.uvs[i];
        let normal = Vec3::from_array(source.normals[i]);
        let [x, y, z, sign] = source.tangents[i];
        let tangent = Vec3::new(x, y, z);
        let bucket = buckets
            .entry((
                position.map(f32::to_bits),
                uv.map(f32::to_bits),
                sign.to_bits(),
            ))
            .or_default();
        let index = if let Some(&index) = bucket.iter().find(|&&index| {
            let [a, b, c, _] = result.tangents[index];
            normal.dot(Vec3::from_array(result.normals[index])) >= MIN_DOT
                && tangent.dot(Vec3::new(a, b, c)) >= MIN_DOT
        }) {
            sums[index].0 += normal;
            sums[index].1 += tangent;
            index
        } else {
            let index = result.positions.len();
            result.positions.push(position);
            result.uvs.push(uv);
            result.normals.push(normal.to_array());
            result.tangents.push(source.tangents[i]);
            sums.push((normal, tangent));
            bucket.push(index);
            index
        };
        remap.push(index as u32);
    }
    for (i, (normal, tangent)) in sums.into_iter().enumerate() {
        let normal = normal.normalize();
        result.normals[i] = normal.to_array();
        result.tangents[i] = crate::blocks::tangent_frame(tangent, normal, result.tangents[i][3]);
    }
    result
        .indices
        .extend(source.indices.iter().map(|&i| remap[i as usize]));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_triangle_shadow_profiles_bound_both_sides_of_every_real_windy_blade() {
        let mut original_diameter = 0.0_f32;
        let mut card_diameter = 0.0_f32;
        let mut max_contour = 0.0_f32;
        for distant in [false, true] {
            let (mut count, mut triangles) = (0, 0);
            for side in 0..6 {
                for variant in 0..4 {
                    let mesh = sprays(side, variant, distant, None).remove(0);
                    let mut start = 0;
                    for card in shadow_cards(side, variant, distant) {
                        count += 1;
                        let stations = card.stations & 0x7fff;
                        let n = stations.count_ones() as usize;
                        let num_triangles = 8 * (n - 1) + 4;
                        let end = start + 3 * num_triangles;
                        let p = card.positions.map(Vec3::from_array);
                        let profile = if card.stations & 0x8000 != 0 {
                            FIVE_LOBES
                        } else {
                            THREE_LOBES
                        };
                        let mut contour = Vec::new();
                        for (index, &(t, width)) in profile.iter().enumerate() {
                            if stations & (1 << index) != 0 {
                                let center = p[1].lerp(p[4], t);
                                contour.extend([
                                    center + (p[0] - p[1]) * width,
                                    center + (p[2] - p[1]) * width,
                                ]);
                            }
                        }
                        for tri in [[0, 3, 4], [0, 4, 1], [1, 4, 5], [1, 5, 2]] {
                            for k in 0..3 {
                                card_diameter =
                                    card_diameter.max(p[tri[k]].distance(p[tri[(k + 1) % 3]]));
                            }
                        }
                        let mut matched = vec![false; contour.len()];
                        for tri in mesh.indices[start..end].chunks_exact(3) {
                            let q = [tri[0], tri[1], tri[2]].map(|i| {
                                let mut q = Vec3::from_array(mesh.positions[i as usize]);
                                if matches!(side, 1 | 4) {
                                    let u = if side == 1 { 2 } else { 0 };
                                    q[u] = 1. - q[u];
                                }
                                q
                            });
                            for k in 0..3 {
                                original_diameter =
                                    original_diameter.max(q[k].distance(q[(k + 1) % 3]));
                                let mut nearest = f32::INFINITY;
                                for (i, c) in contour.iter().enumerate() {
                                    let dist = c.distance(q[k]);
                                    nearest = nearest.min(dist);
                                    matched[i] |= dist < 0.0175;
                                }
                                max_contour = max_contour.max(nearest);
                            }
                        }
                        assert!(matched.into_iter().all(|v| v));
                        start = end;
                        triangles += num_triangles;
                    }
                    assert_eq!(start, mesh.indices.len());
                }
            }
            assert_eq!(count, 352);
            assert!(count * 4 * 15 < triangles);
        }
        // Base error: half the 25 mm thickness, 4.5 mm midrib bow and the
        // lateral half-bevel (3.97 mm) form a <17.5 mm tube. The two halves
        // map each retained station linearly, so the whole PL panel stays in it.
        assert!(max_contour < 0.0175);
        // For wind x -> x + W(x), maximum wind 4 bounds the Lipschitz factor
        // by 1.15 and ||Hessian(W)|| by 0.145/m. For any barycentric blend on
        // a triangle of diameter d, the interpolation error is <= H*d*d/6.
        let wind_bound =
            1.15 * 0.0175 + 0.145 / 6.0 * (original_diameter.powi(2) + card_diameter.powi(2));
        assert!(
            wind_bound < crate::meshing::shadows::PROFILE_ERROR_METRES,
            "{wind_bound}"
        );
        eprintln!(
            "oak profile contour={max_contour:.6} m, conservative windy bound={wind_bound:.6} m"
        );
    }

    #[test]
    fn distant_oak_lighting_weld_preserves_contours_uv_seams_and_handedness() {
        let (mut near_vertices, mut distant_vertices) = (0, 0);
        for variant in 0..4 {
            for side in 0..6 {
                for mut near in distant_face(side, variant) {
                    crate::blocks::surface::index_vertices(&mut near);
                    let far = distant_facets(&near);
                    assert_eq!(near.material, far.material);
                    assert_eq!(near.indices.len(), far.indices.len());
                    near_vertices += near.positions.len();
                    distant_vertices += far.positions.len();
                    for (&a, &b) in near.indices.iter().zip(&far.indices) {
                        let (a, b) = (a as usize, b as usize);
                        assert_eq!(near.positions[a], far.positions[b]);
                        assert_eq!(near.uvs[a], far.uvs[b]);
                        assert_eq!(near.tangents[a][3], far.tangents[b][3]);
                        let n = Vec3::from_array(far.normals[b]);
                        let t = Vec3::from_slice(&far.tangents[b]);
                        assert!(n.is_normalized() && t.is_normalized() && n.dot(t).abs() < 0.001);
                        // The measured maximum is under eleven degrees even at the notches.
                        assert!(Vec3::from_array(near.normals[a]).dot(n) > 0.98);
                        assert!(Vec3::from_slice(&near.tangents[a]).dot(t) > 0.98);
                    }
                }
            }
        }
        assert!(distant_vertices * 10 < near_vertices * 9);
    }
    #[test]
    fn distant_oak_keeps_all_closed_blades_within_nine_mm_of_each_authored_outline() {
        use std::collections::HashSet;

        type Point = [u32; 3];
        fn shell(mesh: &PrimitiveTemplate) -> (HashSet<Point>, Vec<(Vec3, Vec3)>, usize) {
            let mut positions = HashSet::new();
            let mut edges = HashMap::new();
            for triangle in mesh.indices.chunks_exact(3) {
                let points = [triangle[0], triangle[1], triangle[2]].map(|i| {
                    // Canonicalize negative zero for geometric identity.
                    mesh.positions[i as usize].map(|v| if v == 0.0 { 0 } else { v.to_bits() })
                });
                positions.extend(points);
                for (a, b) in [
                    (points[0], points[1]),
                    (points[1], points[2]),
                    (points[2], points[0]),
                ] {
                    let edge = edges.entry((a.min(b), a.max(b))).or_insert((0, 0));
                    if a < b {
                        edge.0 += 1;
                    } else {
                        edge.1 += 1;
                    }
                }
            }
            assert!(
                edges.values().all(|&sides| sides == (1, 1)),
                "each solid has two oppositely wound triangles on every edge"
            );
            let triangles = mesh.indices.len() / 3;
            let blades = (positions.len() + triangles - edges.len()) / 2; // Euler characteristic of the closed, genus-zero blades
            let lines = edges
                .into_keys()
                .map(|(a, b)| {
                    (
                        Vec3::from_array(a.map(f32::from_bits)),
                        Vec3::from_array(b.map(f32::from_bits)),
                    )
                })
                .collect();
            (positions, lines, blades)
        }

        let (mut blades, mut near_tris, mut far_tris, mut near_vertices, mut far_vertices) =
            (0, 0, 0, 0, 0);
        for variant in 0..4 {
            for side in 0..6 {
                let mut near = face(side, variant).remove(0);
                let mut far = distant_face(side, variant).remove(0);
                crate::blocks::surface::index_vertices(&mut near);
                crate::blocks::surface::index_vertices(&mut far);
                let far = distant_facets(&far);
                let (original, _, original_blades) = shell(&near);
                let (retained, edges, retained_blades) = shell(&far);
                assert_eq!(original_blades, retained_blades);
                assert!(
                    retained.is_subset(&original),
                    "retained stations stay at their exact authored wind positions"
                );
                for p in original.difference(&retained) {
                    let point = Vec3::from_array(p.map(f32::from_bits));
                    assert!(
                        edges
                            .iter()
                            .any(|&(a, b)| edge_distance_squared(point, a, b)
                                <= (EDGE_ERROR + 0.000001).powi(2))
                    );
                }
                blades += original_blades;
                near_tris += near.indices.len() / 3;
                far_tris += far.indices.len() / 3;
                near_vertices += near.positions.len();
                far_vertices += far.positions.len();
            }
        }
        assert_eq!(blades, 352);
        assert!(far_tris * 10 < near_tris * 7 && far_vertices * 10 < near_vertices * 6);
    }
}
