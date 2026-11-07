use super::{AXES, Group, Pixels, sample};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::VecDeque;

const MARGIN: f32 = 0.04;
const GRID: usize = 32;
const ROOT: f32 = 0.002;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Settings {
    pub(super) enabled: bool,
    pub(super) max_grass_per_face: usize,
    pub(super) max_stones_per_face: usize,
    pub(super) grass_depth: f32,
    pub(super) stone_depth: f32,
    pub(super) top_relief: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_grass_per_face: 10,
            max_stones_per_face: 6,
            grass_depth: 0.028,
            stone_depth: 0.025,
            top_relief: 0.009,
        }
    }
}

pub(super) fn validate(settings: &Settings) -> Result<(), String> {
    if settings.max_grass_per_face > 12 || settings.max_stones_per_face > 8 {
        return Err("Relief allows at most 12 grass lips and 8 stones per face".into());
    }
    for (name, value, maximum) in [
        ("grass_depth", settings.grass_depth, 0.035),
        ("stone_depth", settings.stone_depth, 0.035),
        ("top_relief", settings.top_relief, 0.012),
    ] {
        if !value.is_finite() || !(0.0..=maximum).contains(&value) {
            return Err(format!(
                "Relief {name} must be finite and within 0..={maximum}"
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Grass,
    Stone,
}

impl Kind {
    fn matches(self, rgba: [f64; 4]) -> bool {
        let [r, g, b, a] = rgba;
        if a < 0.95 {
            return false;
        }
        match self {
            Self::Grass => g > 0.22 && g > r * 1.08 && g > b * 1.3,
            Self::Stone => {
                r.max(g).max(b) > 0.30
                    && r.max(g).max(b) - r.min(g).min(b) < 0.14
                    && (r - g).abs() < 0.065
                    && b > g * 0.88
                    && !Self::Grass.matches(rgba)
            }
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Grass => "grass",
            Self::Stone => "stone",
        }
    }
}

#[derive(Clone, Copy)]
struct Patch {
    minimum: [f32; 2],
    maximum: [f32; 2],
    kind: Kind,
}

fn uv(fit: [[f64; 3]; 2], q: [f32; 2]) -> [f32; 2] {
    fit.map(|row| (row[0] * f64::from(q[0]) + row[1] * f64::from(q[1]) + row[2]) as f32)
}

fn color(image: &Pixels, fit: [[f64; 3]; 2], q: [f32; 2]) -> [f64; 4] {
    sample(image, uv(fit, q).map(f64::from), false)
}

fn patch_matches(image: &Pixels, fit: [[f64; 3]; 2], patch: Patch) -> bool {
    (0..3).all(|y| {
        (0..3).all(|x| {
            let q = [x, y].map(|axis_step| axis_step as f32 * 0.5);
            let q = std::array::from_fn(|axis| {
                patch.minimum[axis] + (patch.maximum[axis] - patch.minimum[axis]) * q[axis]
            });
            patch.kind.matches(color(image, fit, q))
        })
    })
}

fn grass_patches(image: &Pixels, fit: [[f64; 3]; 2], maximum: usize) -> Vec<Patch> {
    const LIP_TOP: f32 = 0.875;
    let mut patches = Vec::new();
    for column in 0..12 {
        let center = MARGIN + (column as f32 + 0.5) * (1.0 - MARGIN * 2.0) / 12.0;
        let half = 0.029;
        let mut bottom = 0.0_f32;
        let mut attached = true;
        for x in [center - half, center, center + half] {
            if !Kind::Grass.matches(color(image, fit, [x, 1.0 - MARGIN])) {
                attached = false;
                break;
            }
            let mut edge = 1.0 - MARGIN;
            while edge > 0.48 && Kind::Grass.matches(color(image, fit, [x, edge - 1.0 / 128.0])) {
                edge -= 1.0 / 128.0;
            }
            bottom = bottom.max(edge + 0.006);
        }
        if !attached || bottom >= LIP_TOP - 0.02 {
            continue;
        }
        let patch = Patch {
            minimum: [center - half, bottom],
            maximum: [center + half, LIP_TOP],
            kind: Kind::Grass,
        };
        if patch_matches(image, fit, patch) {
            patches.push(patch);
        }
    }
    patches.sort_by(|a, b| {
        let a_height = a.maximum[1] - a.minimum[1];
        let b_height = b.maximum[1] - b.minimum[1];
        b_height
            .total_cmp(&a_height)
            .then_with(|| a.minimum[0].total_cmp(&b.minimum[0]))
    });
    patches.truncate(maximum);
    patches.sort_by(|a, b| a.minimum[0].total_cmp(&b.minimum[0]));
    patches
}

fn stone_patches(image: &Pixels, fit: [[f64; 3]; 2], maximum: usize) -> Vec<Patch> {
    let mut mask = [false; GRID * GRID];
    for y in 0..GRID {
        for x in 0..GRID {
            let q = [
                (x as f32 + 0.5) / GRID as f32,
                (y as f32 + 0.5) / GRID as f32,
            ];
            mask[y * GRID + x] = q.into_iter().all(|v| (MARGIN..=1.0 - MARGIN).contains(&v))
                && Kind::Stone.matches(color(image, fit, q));
        }
    }
    let mut candidates = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] {
            continue;
        }
        mask[start] = false;
        let mut queue = VecDeque::from([start]);
        let mut component = Vec::new();
        while let Some(index) = queue.pop_front() {
            component.push(index);
            let [x, y] = [index % GRID, index / GRID];
            for (nx, ny) in [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ] {
                if nx < GRID && ny < GRID && mask[ny * GRID + nx] {
                    mask[ny * GRID + nx] = false;
                    queue.push_back(ny * GRID + nx);
                }
            }
        }
        if component.len() < 2 {
            continue;
        }
        let centroid = component.iter().fold([0.0_f32; 2], |mut sum, &i| {
            sum[0] += (i % GRID) as f32 + 0.5;
            sum[1] += (i / GRID) as f32 + 0.5;
            sum
        });
        let centroid = centroid.map(|v| v / component.len() as f32);
        component.sort_by(|&a, &b| {
            let distance = |i: usize| {
                ((i % GRID) as f32 + 0.5 - centroid[0]).powi(2)
                    + ((i / GRID) as f32 + 0.5 - centroid[1]).powi(2)
            };
            distance(a).total_cmp(&distance(b)).then(a.cmp(&b))
        });
        let mut selected = None;
        for &i in &component {
            let center = [
                ((i % GRID) as f32 + 0.5) / GRID as f32,
                ((i / GRID) as f32 + 0.5) / GRID as f32,
            ];
            let mut patch = Patch {
                minimum: center.map(|v| v - 0.013),
                maximum: center.map(|v| v + 0.013),
                kind: Kind::Stone,
            };
            if patch.minimum.into_iter().any(|v| v < MARGIN)
                || patch.maximum.into_iter().any(|v| v > 1.0 - MARGIN)
                || !patch_matches(image, fit, patch)
            {
                continue;
            }
            for edge in 0..4 {
                for _ in 0..5 {
                    let mut grown = patch;
                    if edge < 2 {
                        grown.minimum[edge] -= 0.009;
                    } else {
                        grown.maximum[edge - 2] += 0.009;
                    }
                    if grown.minimum.into_iter().any(|v| v < MARGIN)
                        || grown.maximum.into_iter().any(|v| v > 1.0 - MARGIN)
                        || !patch_matches(image, fit, grown)
                    {
                        break;
                    }
                    patch = grown;
                }
            }
            selected = Some(patch);
            break;
        }
        if let Some(patch) = selected {
            candidates.push((component.len(), patch));
        }
    }
    candidates.sort_by(|(a_area, a), (b_area, b)| {
        b_area
            .cmp(a_area)
            .then_with(|| a.minimum[1].total_cmp(&b.minimum[1]))
            .then_with(|| a.minimum[0].total_cmp(&b.minimum[0]))
    });
    candidates.truncate(maximum);
    candidates.into_iter().map(|(_, patch)| patch).collect()
}

fn edge_weight(q: [f32; 2]) -> f32 {
    let edge = q.into_iter().map(|v| v.min(1.0 - v)).fold(1.0, f32::min);
    // Keep the first inner row of the source's 8x8 lattice pristine as well.
    // Otherwise interpolation dents a joined face even inside the join band.
    let t = ((edge - 0.13) / 0.10).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn recompute_normals(group: &mut Group) {
    let mut normals = vec![[0.0_f32; 3]; group.positions.len()];
    for triangle in group.indices.chunks_exact(3) {
        let points = [triangle[0], triangle[1], triangle[2]]
            .map(|i| bevy::math::Vec3::from_array(group.positions[i as usize]));
        let normal = (points[1] - points[0]).cross(points[2] - points[0]);
        for &index in triangle {
            let old = bevy::math::Vec3::from_array(normals[index as usize]);
            normals[index as usize] = (old + normal).to_array();
        }
    }
    for (destination, calculated) in group.normals.iter_mut().zip(normals) {
        if let Some(normal) = bevy::math::Vec3::from_array(calculated).try_normalize() {
            *destination = normal.to_array();
        }
    }
    group.tangents.clear();
}

fn surface(group: &Group, original_indices: usize, side: usize, q: [f32; 2]) -> Option<f32> {
    let [u, v] = AXES[side];
    let axis = side / 2;
    let sign = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
    for triangle in group.indices[..original_indices].chunks_exact(3) {
        let p = [triangle[0], triangle[1], triangle[2]].map(|i| group.positions[i as usize]);
        let denominator =
            (p[1][v] - p[2][v]) * (p[0][u] - p[2][u]) + (p[2][u] - p[1][u]) * (p[0][v] - p[2][v]);
        if denominator.abs() < 1e-10 {
            continue;
        }
        let a = ((p[1][v] - p[2][v]) * (q[0] - p[2][u]) + (p[2][u] - p[1][u]) * (q[1] - p[2][v]))
            / denominator;
        let b = ((p[2][v] - p[0][v]) * (q[0] - p[2][u]) + (p[0][u] - p[2][u]) * (q[1] - p[2][v]))
            / denominator;
        let c = 1.0 - a - b;
        if [a, b, c].into_iter().all(|weight| weight >= -1e-5) {
            return Some(sign * (a * p[0][axis] + b * p[1][axis] + c * p[2][axis]));
        }
    }
    None
}

fn point(side: usize, q: [f32; 2], distance: f32) -> [f32; 3] {
    let mut p = [0.0; 3];
    p[side / 2] = distance * if side.is_multiple_of(2) { -1.0 } else { 1.0 };
    p[AXES[side][0]] = q[0];
    p[AXES[side][1]] = q[1];
    p
}

fn quad(group: &mut Group, mut points: [[f32; 3]; 4], mut uvs: [[f32; 2]; 4], normal: [f32; 3]) {
    let vector = |p| bevy::math::Vec3::from_array(p);
    if (vector(points[1]) - vector(points[0]))
        .cross(vector(points[2]) - vector(points[0]))
        .dot(vector(normal))
        < 0.0
    {
        points.swap(1, 3);
        uvs.swap(1, 3);
    }
    let base = group.positions.len() as u32;
    group.positions.extend(points);
    group.normals.extend([normal; 4]);
    group.uvs.extend(uvs);
    group
        .indices
        .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn append_feature(
    group: &mut Group,
    original_indices: usize,
    side: usize,
    fit: [[f64; 3]; 2],
    patch: Patch,
    depth: f32,
) -> Option<Value> {
    let [u0, v0] = patch.minimum;
    let [u1, v1] = patch.maximum;
    let corners = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
    let mut low = f32::INFINITY;
    let mut high = f32::NEG_INFINITY;
    for q in corners
        .into_iter()
        .chain([[(u0 + u1) * 0.5, (v0 + v1) * 0.5]])
    {
        let distance = surface(group, original_indices, side, q)?;
        low = low.min(distance);
        high = high.max(distance);
    }
    let boundary = if side.is_multiple_of(2) { 0.0 } else { 1.0 };
    let front = (low + depth).min(boundary);
    if depth <= ROOT || front <= high + ROOT {
        return None;
    }
    let root = low - ROOT;
    let outward = {
        let mut n = [0.0; 3];
        n[side / 2] = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
        n
    };
    let front_points = corners.map(|q| point(side, q, front));
    let root_points = corners.map(|q| point(side, q, root));
    let uvs = corners.map(|q| uv(fit, q));
    quad(group, front_points, uvs, outward);
    for edge in 0..4 {
        let next = (edge + 1) % 4;
        let mut normal = [0.0; 3];
        let (axis, sign) = match edge {
            0 => (AXES[side][1], -1.0),
            1 => (AXES[side][0], 1.0),
            2 => (AXES[side][1], 1.0),
            _ => (AXES[side][0], -1.0),
        };
        normal[axis] = sign;
        // A wall uses the colored patch as a non-degenerate UV rectangle, not
        // the zero-area projection of its depth edge onto the original atlas.
        quad(
            group,
            [
                root_points[edge],
                root_points[next],
                front_points[next],
                front_points[edge],
            ],
            uvs,
            normal,
        );
    }
    Some(
        json!({"kind":patch.kind.name(),"minimum":patch.minimum,"maximum":patch.maximum,
        "root":root,"front":front,"support_minimum":low,"support_maximum":high}),
    )
}

pub(super) fn apply(
    groups: &mut [Group; 6],
    fits: &[[[f64; 3]; 2]; 6],
    image: &Pixels,
    settings: &Settings,
) -> Result<Value, String> {
    validate(settings)?;
    if !settings.enabled {
        return Ok(json!({"enabled":false,"added_vertices":0,"added_triangles":0}));
    }
    let expected_bytes = (image.width as usize)
        .checked_mul(image.height as usize)
        .and_then(|pixels| pixels.checked_mul(4));
    if image.width == 0 || image.height == 0 || expected_bytes != Some(image.data.len()) {
        return Err("Relief requires a non-empty RGBA image".into());
    }
    for fit in fits {
        if fit.iter().flatten().any(|v| !v.is_finite())
            || (fit[0][0] * fit[1][1] - fit[0][1] * fit[1][0]).abs() < 1e-12
        {
            return Err("Relief requires finite, non-degenerate face UV mappings".into());
        }
    }
    let mut faces = Vec::new();
    let mut added_vertices = 0;
    let mut added_triangles = 0;
    for side in [0, 1, 3, 4, 5] {
        let group = &mut groups[side];
        let before_vertices = group.positions.len();
        let before_indices = group.indices.len();
        let fit = fits[side];
        let mut displaced = 0;
        for position in &mut group.positions {
            let q = AXES[side].map(|axis| position[axis]);
            let weight = edge_weight(q);
            let amount = if side == 3 {
                let rgba = color(image, fit, q);
                if !Kind::Grass.matches(rgba) {
                    0.0
                } else {
                    let shade = (0.55 * rgba[0] + 0.45 * rgba[1]) as f32;
                    settings.top_relief * (1.0 - shade).clamp(0.2, 1.0)
                }
            } else {
                settings.grass_depth.max(settings.stone_depth) + 0.006
            } * weight;
            if amount > 0.0 {
                let sign = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
                position[side / 2] -= sign * amount;
                displaced += 1;
            }
        }
        if displaced > 0 {
            recompute_normals(group);
        }
        let mut features = Vec::new();
        if side != 3 {
            let grass = grass_patches(image, fit, settings.max_grass_per_face);
            let stones = stone_patches(image, fit, settings.max_stones_per_face);
            for mut patch in grass.into_iter().chain(stones) {
                let depth = match patch.kind {
                    Kind::Grass => settings.grass_depth,
                    Kind::Stone => settings.stone_depth,
                };
                loop {
                    if let Some(feature) =
                        append_feature(group, before_indices, side, fit, patch, depth)
                    {
                        features.push(feature);
                        break;
                    }
                    if patch.kind == Kind::Stone {
                        let size: [f32; 2] =
                            std::array::from_fn(|axis| patch.maximum[axis] - patch.minimum[axis]);
                        if size.into_iter().any(|v| v <= 0.03) {
                            break;
                        }
                        for (axis, extent) in size.into_iter().enumerate() {
                            patch.minimum[axis] += extent * 0.12;
                            patch.maximum[axis] -= extent * 0.12;
                        }
                        continue;
                    }
                    if patch.maximum[1] - patch.minimum[1] <= 0.028 {
                        break;
                    }
                    // Stop below the untouched rim instead of floating above
                    // it or flattening the neighboring block's shared seam.
                    patch.maximum[1] -= 0.02;
                }
            }
        }
        if group
            .positions
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || group.normals.iter().flatten().any(|v| !v.is_finite())
            || group.uvs.iter().flatten().any(|v| !v.is_finite())
            || group
                .indices
                .iter()
                .any(|&i| i as usize >= group.positions.len())
        {
            return Err(format!(
                "Relief face {side} violates the finite unit-cell mesh contract"
            ));
        }
        let vertices = group.positions.len() - before_vertices;
        let triangles = (group.indices.len() - before_indices) / 3;
        added_vertices += vertices;
        added_triangles += triangles;
        faces.push(
            json!({"side":side,"displaced_vertices":displaced,"added_vertices":vertices,
            "added_triangles":triangles,"features":features}),
        );
    }
    Ok(
        json!({"enabled":true,"added_vertices":added_vertices,"added_triangles":added_triangles,
        "faces":faces,"normal_extent":"strict unit cell; recessed original sides and rooted prisms"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(rgba: [u8; 4]) -> Pixels {
        Pixels {
            width: 64,
            height: 64,
            data: rgba.repeat(64 * 64),
        }
    }

    fn groups() -> [Group; 6] {
        std::array::from_fn(|side| {
            let mut group = Group::default();
            let distance = if side.is_multiple_of(2) { 0.0 } else { 1.0 };
            for y in 0..8 {
                for x in 0..8 {
                    let corners = [[x, y], [x + 1, y], [x + 1, y + 1], [x, y + 1]]
                        .map(|q| q.map(|v| v as f32 / 8.0));
                    let mut normal = [0.0; 3];
                    normal[side / 2] = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
                    quad(
                        &mut group,
                        corners.map(|q| point(side, q, distance)),
                        corners,
                        normal,
                    );
                }
            }
            group
        })
    }

    fn fits() -> [[[f64; 3]; 2]; 6] {
        [[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]; 6]
    }

    #[test]
    fn masks_do_not_turn_soil_into_grass_or_stones() {
        assert!(Kind::Grass.matches([0.35, 0.60, 0.15, 1.0]));
        assert!(Kind::Stone.matches([0.52, 0.53, 0.56, 1.0]));
        for rgba in [[0.55, 0.35, 0.16, 1.0], [0.30, 0.23, 0.14, 1.0]] {
            assert!(!Kind::Grass.matches(rgba));
            assert!(!Kind::Stone.matches(rgba));
        }
    }

    #[test]
    fn relief_is_deterministic_bounded_and_rooted_in_the_original_side() {
        let mut a = groups();
        let mut b = groups();
        let image = image([80, 160, 32, 255]);
        let report = apply(&mut a, &fits(), &image, &Settings::default()).unwrap();
        assert_eq!(
            report,
            apply(&mut b, &fits(), &image, &Settings::default()).unwrap()
        );
        assert!(report["added_triangles"].as_u64().unwrap() > 0);
        for side in 0..6 {
            assert_eq!(a[side].positions, b[side].positions);
            assert_eq!(a[side].indices, b[side].indices);
            assert!(
                a[side]
                    .positions
                    .iter()
                    .flatten()
                    .all(|v| (0.0..=1.0).contains(v))
            );
            assert_eq!(a[side].positions.len(), a[side].normals.len());
            assert_eq!(a[side].positions.len(), a[side].uvs.len());
        }
        for face in report["faces"].as_array().unwrap() {
            let features = face["features"].as_array().unwrap();
            assert!(features.len() <= 10);
            if face["side"] != 3 {
                assert!(
                    !features.is_empty(),
                    "each green side needs visible grass relief"
                );
            }
            for feature in features {
                let root = feature["root"].as_f64().unwrap();
                let low = feature["support_minimum"].as_f64().unwrap();
                let high = feature["support_maximum"].as_f64().unwrap();
                let front = feature["front"].as_f64().unwrap();
                assert!(root < low && front > high);
                assert!(front - root <= 0.035);
            }
        }
    }

    #[test]
    fn feature_quads_have_non_degenerate_geometry_and_uvs() {
        let mut groups = groups();
        apply(
            &mut groups,
            &fits(),
            &image([132, 136, 143, 255]),
            &Settings::default(),
        )
        .unwrap();
        for group in groups {
            assert!(group.tangents.is_empty());
            for triangle in group.indices.chunks_exact(3) {
                let p = [triangle[0], triangle[1], triangle[2]]
                    .map(|i| bevy::math::Vec3::from_array(group.positions[i as usize]));
                let q = [triangle[0], triangle[1], triangle[2]]
                    .map(|i| bevy::math::Vec2::from_array(group.uvs[i as usize]));
                let cross = (p[1] - p[0]).cross(p[2] - p[0]);
                assert!(cross.length_squared() > 1e-12);
                let normal = bevy::math::Vec3::from_array(group.normals[triangle[0] as usize]);
                assert!(cross.dot(normal) > 0.0);
                assert!((q[1] - q[0]).perp_dot(q[2] - q[0]).abs() > 1e-8);
            }
        }
    }

    #[test]
    fn unchanged_border_and_no_extra_top_triangles_preserve_join_contract() {
        let original = groups();
        let mut changed = groups();
        apply(
            &mut changed,
            &fits(),
            &image([80, 160, 32, 255]),
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(original[3].indices.len(), changed[3].indices.len());
        assert_eq!(original[2].positions, changed[2].positions);
        for side in [0, 1, 3, 4, 5] {
            for (before, after) in original[side]
                .positions
                .iter()
                .zip(&changed[side].positions)
            {
                let q = AXES[side].map(|axis| before[axis]);
                if edge_weight(q) == 0.0 {
                    assert_eq!(before, after);
                }
            }
        }
    }

    #[test]
    fn soil_is_not_decorated_and_feature_limits_are_validated() {
        let mut groups = groups();
        let report = apply(
            &mut groups,
            &fits(),
            &image([145, 90, 42, 255]),
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(report["added_vertices"], 0);
        assert!(
            validate(&Settings {
                max_grass_per_face: 13,
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate(&Settings {
                max_stones_per_face: 9,
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate(&Settings {
                stone_depth: 0.036,
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate(&Settings {
                top_relief: f32::NAN,
                ..Default::default()
            })
            .is_err()
        );
    }
}
