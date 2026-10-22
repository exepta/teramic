use super::{PrimitiveTemplate, definitions::CubeSurface};
use bevy::math::Vec3;
use std::{collections::HashMap, sync::OnceLock};
mod underwater;

type Faces = [[[OnceLock<Vec<SurfacePatch>>; 64]; 12]; 6];

#[derive(Clone, Debug)]
pub(crate) struct SurfaceTemplate {
    style: CubeSurface,
    faces: OnceLock<Box<Faces>>,
}

#[derive(Clone, Debug)]
pub(crate) struct SurfacePatch {
    pub primitive: PrimitiveTemplate,
    pub shade: f32,
}

impl SurfaceTemplate {
    pub fn new(style: CubeSurface) -> Self {
        Self {
            style,
            faces: OnceLock::new(),
        }
    }

    pub fn face(
        &self,
        source: &[PrimitiveTemplate],
        side: usize,
        variant: usize,
        mask: u8,
        connect: impl Fn(Vec3) -> Vec3,
    ) -> &[SurfacePatch] {
        self.faces.get_or_init(|| {
            Box::new(std::array::from_fn(|_| {
                std::array::from_fn(|_| std::array::from_fn(|_| OnceLock::new()))
            }))
        })[side][variant.min(11)][usize::from(mask)]
        .get_or_init(|| build(source, self.style, side, variant, connect))
    }
}

fn build(
    source: &[PrimitiveTemplate],
    style: CubeSurface,
    side: usize,
    variant: usize,
    connect: impl Fn(Vec3) -> Vec3,
) -> Vec<SurfacePatch> {
    let axis = side / 2;
    let (u, v) = match axis {
        0 => (2, 1),
        1 => (0, 2),
        _ => (0, 1),
    };
    let plane = (side % 2) as f32;
    let mut outward = Vec3::ZERO;
    outward[axis] = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
    let mut result = Vec::new();
    for primitive in source {
        for (index, quad) in primitive.positions.chunks_exact(4).enumerate() {
            let original: [Vec3; 4] = std::array::from_fn(|i| Vec3::from_array(quad[i]));
            let points = original.map(&connect);
            let low = original
                .into_iter()
                .fold(Vec3::splat(f32::INFINITY), Vec3::min);
            let high = original
                .into_iter()
                .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
            // Keep bevel bands and the thin grass cap intact. Detail cannot move
            // a block outline or any shared material/section/LOD boundary.
            if original.iter().any(|p| (p[axis] - plane).abs() > 0.00001)
                || high[u] - low[u] < 0.35
                || high[v] - low[v] < 0.35
                || side == 2
            {
                let normal = Vec3::from_array(primitive.normals[index * 4]);
                push(&mut result, primitive.material, points, normal, 1.0, (u, v));
                continue;
            }
            let fine = matches!(
                style,
                CubeSurface::Turf
                    | CubeSurface::Sand
                    | CubeSurface::Snow
                    | CubeSurface::CoralRed
                    | CubeSurface::CoralBlue
                    | CubeSurface::CoralYellow
                    | CubeSurface::CoralPurple
            );
            let base_count = match variant / 4 {
                1 => {
                    if fine {
                        6
                    } else {
                        4
                    }
                }
                2 => {
                    if fine {
                        4
                    } else {
                        3
                    }
                }
                _ => {
                    if fine {
                        8
                    } else {
                        6
                    }
                }
            };
            // Splitting a grass face must not multiply its micro-grid density.
            let count = ((base_count as f32 * ((high[u] - low[u]) * (high[v] - low[v])).sqrt())
                .ceil() as usize)
                .clamp(4.min(base_count), base_count);
            let coordinate = |i: usize| match i {
                0 => 0.0,
                i if i == count => 1.0,
                i => 0.035 + (i - 1) as f32 * 0.93 / (count - 2) as f32,
            };
            let depth = |x: usize, y: usize| {
                if x == 0 || y == 0 || x == count - 1 || y == count - 1 {
                    return 0.0;
                }
                if let Some(depth) = underwater::depth(style, x, y, variant) {
                    return depth;
                }
                let seed = hash((x + variant % 2) / 2, (y + variant / 2) / 2, variant);
                let level = if count == 3 {
                    1 + (hash(x, y, variant) % 3) as usize
                } else if matches!(style, CubeSurface::Wood) {
                    (x + variant) % 4
                } else if matches!(style, CubeSurface::Mushroom) {
                    (hash(x, y / 2, variant + 17) % 4) as usize
                } else if matches!(
                    style,
                    CubeSurface::Rock | CubeSurface::Earth | CubeSurface::Snow
                ) {
                    // Varied connected clods rather than one almost-flat face.
                    1 + (seed % 3) as usize
                } else {
                    match seed % 10 {
                        0..=5 => 0,
                        6..=7 => 1,
                        8 => 2,
                        _ => 3,
                    }
                };
                let maximum = match style {
                    CubeSurface::Turf if side == 3 => 0.025,
                    CubeSurface::Turf => 0.035,
                    CubeSurface::Earth => 0.055,
                    CubeSurface::Rock => 0.075,
                    CubeSurface::Wood => 0.025,
                    CubeSurface::Mushroom => 0.06,
                    CubeSurface::Sand => 0.0045,
                    CubeSurface::Snow => 0.022,
                    CubeSurface::CoralRed
                    | CubeSurface::CoralBlue
                    | CubeSurface::CoralYellow
                    | CubeSurface::CoralPurple => 0.12,
                };
                level as f32 / 3.0 * maximum
            };
            let point = |x: usize, y: usize, depth: f32| {
                let a = coordinate(x);
                let b = coordinate(y);
                points[0]
                    .lerp(points[1], a)
                    .lerp(points[3].lerp(points[2], a), b)
                    - outward * depth
            };
            let shade = |x: usize, y: usize| {
                if depth(x, y) == 0.0 {
                    1.0
                } else if side == 3 {
                    [0.96, 0.98, 1.0, 1.02][((hash(
                        (x + variant % 2) / 2,
                        (y + variant / 2) / 2,
                        variant,
                    ) >> 8)
                        & 3) as usize]
                } else {
                    [0.90, 0.96, 1.02, 1.08][((hash(
                        (x + variant % 2) / 2,
                        (y + variant / 2) / 2,
                        variant,
                    ) >> 8)
                        & 3) as usize]
                }
            };
            let mut visited = [[false; 8]; 8];
            for y in 0..count {
                for x in 0..count {
                    let d = depth(x, y);
                    if !visited[y][x] {
                        let tone = shade(x, y);
                        let same = |xx: usize, yy: usize| {
                            !visited[yy][xx] && depth(xx, yy) == d && shade(xx, yy) == tone
                        };
                        let mut width = 1;
                        while x + width < count && same(x + width, y) {
                            width += 1;
                        }
                        let mut height = 1;
                        while y + height < count && (x..x + width).all(|xx| same(xx, y + height)) {
                            height += 1;
                        }
                        for row in &mut visited[y..y + height] {
                            row[x..x + width].fill(true);
                        }
                        push(
                            &mut result,
                            primitive.material,
                            [
                                point(x, y, d),
                                point(x + width, y, d),
                                point(x + width, y + height, d),
                                point(x, y + height, d),
                            ],
                            outward,
                            tone,
                            (u, v),
                        );
                    }
                }
            }
            // Join consecutive step walls as well as the horizontal plateaus.
            for x in 0..count - 1 {
                let mut y = 0;
                while y < count {
                    let d = depth(x, y);
                    let next = depth(x + 1, y);
                    if d == next {
                        y += 1;
                        continue;
                    }
                    let start = y;
                    y += 1;
                    while y < count && depth(x, y) == d && depth(x + 1, y) == next {
                        y += 1;
                    }
                    let mut normal = Vec3::ZERO;
                    normal[u] = (next - d).signum();
                    push(
                        &mut result,
                        primitive.material,
                        [
                            point(x + 1, start, d),
                            point(x + 1, y, d),
                            point(x + 1, y, next),
                            point(x + 1, start, next),
                        ],
                        normal,
                        0.96,
                        (u, v),
                    );
                }
            }
            for y in 0..count - 1 {
                let mut x = 0;
                while x < count {
                    let d = depth(x, y);
                    let next = depth(x, y + 1);
                    if d == next {
                        x += 1;
                        continue;
                    }
                    let start = x;
                    x += 1;
                    while x < count && depth(x, y) == d && depth(x, y + 1) == next {
                        x += 1;
                    }
                    let mut normal = Vec3::ZERO;
                    normal[v] = (next - d).signum();
                    push(
                        &mut result,
                        primitive.material,
                        [
                            point(start, y + 1, d),
                            point(x, y + 1, d),
                            point(x, y + 1, next),
                            point(start, y + 1, next),
                        ],
                        normal,
                        0.96,
                        (u, v),
                    );
                }
            }
        }
    }
    for patch in &mut result {
        index_vertices(&mut patch.primitive);
    }
    result
}

fn index_vertices(mesh: &mut PrimitiveTemplate) {
    let mut vertices = HashMap::with_capacity(mesh.positions.len());
    let mut positions = Vec::with_capacity(mesh.positions.len());
    let mut normals = Vec::with_capacity(mesh.normals.len());
    let mut uvs = Vec::with_capacity(mesh.uvs.len());
    let mut remap = Vec::with_capacity(mesh.positions.len());
    let bits = |value: f32| if value == 0.0 { 0 } else { value.to_bits() };
    for ((&position, &normal), &uv) in mesh.positions.iter().zip(&mesh.normals).zip(&mesh.uvs) {
        // Preserve every hard normal and texture seam; only exact attribute
        // matches can reuse an index. This cache work happens once on workers.
        let key = (position.map(bits), normal.map(bits), uv.map(bits));
        let index = *vertices.entry(key).or_insert_with(|| {
            let index = positions.len() as u32;
            positions.push(position);
            normals.push(normal);
            uvs.push(uv);
            index
        });
        remap.push(index);
    }
    for index in &mut mesh.indices {
        *index = remap[*index as usize];
    }
    mesh.positions = positions;
    mesh.normals = normals;
    mesh.uvs = uvs;
}

fn hash(x: usize, y: usize, variant: usize) -> u32 {
    let value = (x as u32).wrapping_mul(0x9e3779b9)
        ^ (y as u32).wrapping_mul(0x85ebca6b)
        ^ (variant as u32).wrapping_mul(0xc2b2ae35);
    (value ^ (value >> 16)).wrapping_mul(0x7feb352d)
}

fn push(
    result: &mut Vec<SurfacePatch>,
    material: usize,
    points: [Vec3; 4],
    outward: Vec3,
    shade: f32,
    (u, v): (usize, usize),
) {
    let index = result
        .iter()
        .position(|p| p.primitive.material == material && p.shade == shade)
        .unwrap_or_else(|| {
            result.push(SurfacePatch {
                primitive: PrimitiveTemplate {
                    positions: Vec::new(),
                    normals: Vec::new(),
                    uvs: Vec::new(),
                    indices: Vec::new(),
                    material,
                },
                shade,
            });
            result.len() - 1
        });
    let primitive = &mut result[index].primitive;
    let indices = [0, 1, 2, 0, 2, 3];
    for triangle in indices.chunks_exact(3) {
        let mut positions = [
            points[triangle[0]],
            points[triangle[1]],
            points[triangle[2]],
        ];
        let mut triangle_normal = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
        if triangle_normal.length_squared() < 1e-12 {
            continue;
        }
        if triangle_normal.dot(outward) < 0.0 {
            positions.swap(1, 2);
            triangle_normal = -triangle_normal;
        }
        let start = primitive.positions.len() as u32;
        primitive.positions.extend(positions.map(|p| p.to_array()));
        primitive
            .normals
            .extend([triangle_normal.normalize().to_array(); 3]);
        primitive.uvs.extend(positions.map(|p| [p[u], p[v]]));
        primitive.indices.extend([start, start + 1, start + 2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{SideCap, cube_faces};

    #[test]
    fn indexing_preserves_triangles_normals_and_texture_seams_exactly() {
        let mut patches = Vec::new();
        let points = [Vec3::ZERO, Vec3::X, Vec3::X + Vec3::Y, Vec3::Y];
        push(&mut patches, 0, points, Vec3::Z, 1.0, (0, 1));
        push(&mut patches, 0, points, -Vec3::Z, 1.0, (0, 1));
        let mesh = &mut patches[0].primitive;
        let triangles = |mesh: &PrimitiveTemplate| {
            mesh.indices
                .iter()
                .map(|&i| {
                    let i = i as usize;
                    (mesh.positions[i], mesh.normals[i], mesh.uvs[i])
                })
                .collect::<Vec<_>>()
        };
        let before = triangles(mesh);
        assert_eq!(mesh.positions.len(), 12);
        index_vertices(mesh);
        assert_eq!(
            mesh.positions.len(),
            8,
            "hard normal edge must not be smoothed"
        );
        assert_eq!(triangles(mesh), before);
        mesh.positions.push(mesh.positions[0]);
        mesh.normals.push(mesh.normals[0]);
        mesh.uvs.push([1.0, 1.0]);
        let index = mesh.positions.len() as u32 - 1;
        mesh.indices.push(index);
        index_vertices(mesh);
        assert_eq!(
            mesh.positions.len(),
            9,
            "distinct UVs must retain their own vertex"
        );
    }

    #[test]
    fn continuous_bark_ridges_use_one_wall_instead_of_a_wall_per_grid_cell() {
        let template = SurfaceTemplate::new(CubeSurface::Wood);
        let source = cube_faces(0.09, [0; 6], None);
        for variant in 0..4 {
            let patches = template.face(&source[3], 3, variant, 63, |p| p);
            let wall_triangles = patches
                .iter()
                .flat_map(|p| {
                    let mesh = &p.primitive;
                    mesh.indices.chunks_exact(3).filter(|triangle| {
                        let n = Vec3::from_array(mesh.normals[triangle[0] as usize]);
                        n.x.abs() > 0.999 && n.y.abs() < 0.001 && n.z.abs() < 0.001
                    })
                })
                .count();
            assert!(
                wall_triangles > 0 && wall_triangles <= 10,
                "unmerged bark walls: variant={variant}, triangles={wall_triangles}"
            );
        }
    }

    #[test]
    fn neighbor_flattening_keeps_a_valid_triangle_when_the_other_half_collapses() {
        let mut patches = Vec::new();
        push(
            &mut patches,
            0,
            [Vec3::ZERO, Vec3::ZERO, Vec3::new(0.0, 1.0, 1.0), Vec3::Z],
            Vec3::X,
            1.0,
            (2, 1),
        );
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].primitive.indices.len(), 3);
        assert!(
            patches[0]
                .primitive
                .normals
                .iter()
                .all(|&n| Vec3::from_array(n).dot(Vec3::X) > 0.99)
        );
    }

    #[test]
    fn every_style_and_variant_has_bounded_inward_detail_and_valid_front_faces() {
        for style in [
            CubeSurface::Turf,
            CubeSurface::Earth,
            CubeSurface::Rock,
            CubeSurface::Sand,
            CubeSurface::Snow,
            CubeSurface::Wood,
            CubeSurface::Mushroom,
            CubeSurface::CoralRed,
            CubeSurface::CoralBlue,
            CubeSurface::CoralYellow,
            CubeSurface::CoralPurple,
        ] {
            let template = SurfaceTemplate::new(style);
            let source = cube_faces(
                0.065,
                [0; 6],
                Some(SideCap {
                    material: 1,
                    height: 0.1,
                }),
            );
            for (side, face) in source.iter().enumerate() {
                for variant in 0..4 {
                    let patches = template.face(face, side, variant, 63, |p| p);
                    assert!(
                        patches.len() <= 12,
                        "coalesce geometry by material and tone"
                    );
                    assert!(
                        patches
                            .iter()
                            .map(|p| p.primitive.positions.len())
                            .sum::<usize>()
                            <= if matches!(style, CubeSurface::Mushroom) {
                                320
                            } else {
                                1200
                            },
                        "keep per-face micro geometry bounded"
                    );
                    let mut inset_points = 0;
                    for patch in patches {
                        let mesh = &patch.primitive;
                        assert!(mesh.material <= 1);
                        assert_eq!(mesh.normals.len(), mesh.positions.len());
                        assert_eq!(mesh.uvs.len(), mesh.positions.len());
                        for p in &mesh.positions {
                            assert!(
                                p.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
                                "never extrude the block"
                            );
                            if (0.2..0.8).contains(&p[(side / 2 + 1) % 3])
                                && (0.2..0.8).contains(&p[(side / 2 + 2) % 3])
                                && (p[side / 2] - (side % 2) as f32).abs() > 0.001
                            {
                                inset_points += 1;
                            }
                        }
                        for triangle in mesh.indices.chunks_exact(3) {
                            let [a, b, c] = triangle.try_into().expect("triangle");
                            let [a, b, c] = [a, b, c]
                                .map(|i: u32| Vec3::from_array(mesh.positions[i as usize]));
                            let normal = (b - a).cross(c - a).normalize();
                            assert!(normal.is_finite());
                            for &i in triangle {
                                assert!(
                                    normal.dot(Vec3::from_array(mesh.normals[i as usize])) > 0.999
                                );
                            }
                        }
                    }
                    if side != 2 {
                        assert!(inset_points > 0, "detail must not silently be flat");
                    }
                }
            }
        }
    }

    #[test]
    fn surface_cache_is_shared_and_preserves_grass_side_cap_materials() {
        let template = SurfaceTemplate::new(CubeSurface::Turf);
        let faces = cube_faces(
            0.065,
            [0; 6],
            Some(SideCap {
                material: 1,
                height: 0.1,
            }),
        );
        let first = template.face(&faces[0], 0, 1, 63, |p| p);
        assert!(std::ptr::eq(
            first,
            template.face(&faces[0], 0, 1, 63, |p| p)
        ));
        assert!(first.iter().any(|p| p.primitive.material == 1));
        let alternate = template.face(&faces[0], 0, 2, 63, |p| p);
        let first_geometry: Vec<_> = first
            .iter()
            .flat_map(|p| p.primitive.positions.iter())
            .collect();
        let alternate_geometry: Vec<_> = alternate
            .iter()
            .flat_map(|p| p.primitive.positions.iter())
            .collect();
        assert_ne!(first_geometry, alternate_geometry);
    }
}
