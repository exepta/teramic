use super::*;

struct Triangle {
    points: [Vec3; 3],
    normal: Vec3,
    side: usize,
    tone: f32,
}

// Two species, two LODs, 64 exposure masks and four deterministic variants.
static NEEDLES: LazyLock<Vec<OnceLock<Vec<Triangle>>>> =
    LazyLock::new(|| (0..1024).map(|_| OnceLock::new()).collect());

fn triangle(
    triangles: &mut Vec<Triangle>,
    mut points: [Vec3; 3],
    center: Vec3,
    side: usize,
    tone: f32,
) {
    let mut normal = (points[1] - points[0]).cross(points[2] - points[0]);
    if normal.dot((points[0] + points[1] + points[2]) / 3.0 - center) < 0.0 {
        points.swap(1, 2);
        normal = -normal;
    }
    triangles.push(Triangle {
        points,
        normal: normal.normalize(),
        side,
        tone,
    });
}

fn quad(triangles: &mut Vec<Triangle>, points: [Vec3; 4], center: Vec3, side: usize, tone: f32) {
    triangle(
        triangles,
        [points[0], points[1], points[2]],
        center,
        side,
        tone,
    );
    triangle(
        triangles,
        [points[0], points[2], points[3]],
        center,
        side,
        tone,
    );
}

fn pyramid(triangles: &mut Vec<Triangle>, roots: [Vec3; 3], tip: Vec3, side: usize, tone: f32) {
    let center = (roots[0] + roots[1] + roots[2] + tip) * 0.25;
    for points in [
        [roots[0], roots[1], roots[2]],
        [roots[0], roots[1], tip],
        [roots[1], roots[2], tip],
        [roots[2], roots[0], tip],
    ] {
        triangle(triangles, points, center, side, tone);
    }
}

fn crown_core(exposure: usize, variant: usize, fir: bool) -> Vec<Triangle> {
    let mut low = if fir {
        Vec3::new(0.05, 0.07, 0.05)
    } else {
        Vec3::new(0.07, 0.09, 0.07)
    };
    let mut high = Vec3::ONE - low;
    for axis in 0..3 {
        if exposure & (1 << (axis * 2)) == 0 {
            low[axis] = 0.0;
        }
        if exposure & (1 << (axis * 2 + 1)) == 0 {
            high[axis] = 1.0;
        }
    }
    let bevel = Vec3::splat(if fir { 0.09 } else { 0.065 } + variant as f32 * 0.006);
    let inner_low = low + bevel;
    let inner_high = high - bevel;
    let center = (low + high) * 0.5;
    let bound = |axis: usize, positive: usize, inner: bool| {
        if inner {
            if positive == 0 {
                inner_low[axis]
            } else {
                inner_high[axis]
            }
        } else if positive == 0 {
            low[axis]
        } else {
            high[axis]
        }
    };
    let mut triangles = Vec::with_capacity(44);
    // A closed, bevelled crown body fills every viewing direction; only its
    // boundary with another leaf cell is culled, as for the other leaf meshes.
    for side in 0usize..6 {
        if exposure & (1 << side) == 0 {
            continue;
        }
        let axis = side / 2;
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        let mut points = [center; 4];
        for (point, (positive_u, positive_v)) in
            points.iter_mut().zip([(0, 0), (1, 0), (1, 1), (0, 1)])
        {
            point[axis] = bound(axis, side % 2, false);
            point[u] = bound(u, positive_u, true);
            point[v] = bound(v, positive_v, true);
        }
        quad(
            &mut triangles,
            points,
            center,
            side,
            ((side + variant) % 3) as f32 * 0.02 - 0.02,
        );
    }
    for a in 0..3 {
        for b in a + 1..3 {
            let c = 3 - a - b;
            for positive_a in 0..2 {
                for positive_b in 0..2 {
                    let sides = [a * 2 + positive_a, b * 2 + positive_b];
                    let Some(side) = sides.into_iter().find(|side| exposure & (1 << side) != 0)
                    else {
                        continue;
                    };
                    let mut points = [center; 4];
                    for (index, point) in points.iter_mut().enumerate() {
                        point[a] = bound(a, positive_a, index >= 2);
                        point[b] = bound(b, positive_b, index < 2);
                        point[c] = bound(c, usize::from(index == 1 || index == 2), true);
                    }
                    quad(&mut triangles, points, center, side, 0.0);
                }
            }
        }
    }
    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                let signs = [x, y, z];
                let Some(side) = (0..3)
                    .map(|axis| axis * 2 + signs[axis])
                    .find(|side| exposure & (1 << side) != 0)
                else {
                    continue;
                };
                let points = std::array::from_fn(|outer_axis| {
                    Vec3::from_array(std::array::from_fn(|axis| {
                        bound(axis, signs[axis], axis != outer_axis)
                    }))
                });
                triangle(&mut triangles, points, center, side, -0.01);
            }
        }
    }
    triangles
}

fn conifer(middle: bool, exposure: usize, variant: usize, fir: bool) -> Vec<Triangle> {
    let mut triangles = crown_core(exposure, variant, fir);
    for side in 0usize..6 {
        if exposure & (1 << side) == 0 {
            continue;
        }
        let axis = side / 2;
        let u_axis = (axis + 1) % 3;
        let v_axis = (axis + 2) % 3;
        let mut normal = Vec3::ZERO;
        normal[axis] = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
        let mut u = Vec3::ZERO;
        u[u_axis] = 1.0;
        let mut v = Vec3::ZERO;
        v[v_axis] = 1.0;
        for spray in 0..if middle { 1 } else { 3 } {
            let sign = if (spray + variant).is_multiple_of(2) {
                -1.0
            } else {
                1.0
            };
            let root = Vec3::splat(0.5)
                + normal * if fir { 0.28 } else { 0.25 }
                + u * if spray == 0 {
                    0.0
                } else {
                    sign * if fir { 0.22 } else { 0.25 }
                }
                + v * if spray == 0 { 0.04 } else { sign * 0.10 };
            let width = if fir { 0.14 } else { 0.11 };
            let depth = if fir { 0.10 } else { 0.08 };
            let roots = [
                root - u * width - v * depth,
                root + u * width - v * depth,
                root + v * if fir { 0.15 } else { 0.13 },
            ];
            let tip = root
                + normal * if fir { 0.21 } else { 0.24 }
                + u * sign * if fir { 0.04 } else { 0.08 }
                - v * if fir { 0.03 } else { 0.06 };
            // Short, thick tips intersect the solid core instead of floating as
            // disconnected needle fans. Both LODs retain the same crown volume.
            pyramid(
                &mut triangles,
                roots,
                tip,
                side,
                ((spray + side + variant) % 5) as f32 * 0.025 - 0.05,
            );
        }
    }
    triangles
}

fn template(index: usize) -> &'static [Triangle] {
    NEEDLES[index].get_or_init(|| {
        let fir = index / 512 != 0;
        let middle = !(index / 256).is_multiple_of(2);
        let exposure = (index / 4) % 64;
        let variant = index % 4;
        let mut triangles = conifer(middle, exposure, variant, fir);
        let mut area = 0.0;
        let mut tone = 0.0;
        for triangle in &triangles {
            let weight = (triangle.points[1] - triangle.points[0])
                .cross(triangle.points[2] - triangle.points[0])
                .length();
            area += weight;
            tone += triangle.tone * weight;
        }
        if area > 0.0 {
            for triangle in &mut triangles {
                triangle.tone -= tone / area;
            }
        }
        triangles
    })
}

pub(super) fn append(
    batches: &mut BTreeMap<usize, MeshBatch>,
    block: &BlockTemplate,
    global: [i64; 3],
    local: [u32; 3],
    exposure: u8,
    level: DetailLevel,
    species_material: (u16, bool),
) -> Result<(), String> {
    let (material, fir) = species_material;
    let index = usize::from(fir) * 512
        + usize::from(level == DetailLevel::Middle) * 256
        + usize::from(exposure) * 4
        + (variation(global, 0x004e_4545_444c) % 4) as usize;
    let starts = block
        .face_materials
        .map(|slot| batches.get(&slot).map_or(0, |batch| batch.positions.len()));
    let offset = Vec3::from_array(local.map(|coordinate| coordinate as f32));
    for triangle in template(index) {
        let slot = block.face_materials[triangle.side];
        let batch = batches.entry(slot).or_insert_with(|| MeshBatch {
            material: slot,
            ..default()
        });
        let base = u32::try_from(batch.positions.len()).map_err(|_| "Needle mesh overflow")?;
        base.checked_add(2).ok_or("Needle mesh overflow")?;
        let points = triangle.points.map(|point| (point + offset).to_array());
        batch.colors.resize(batch.positions.len(), [1.0; 4]);
        batch.positions.extend(points);
        batch.normals.extend([triangle.normal.to_array(); 3]);
        let shade = 1.0 + triangle.tone;
        batch.colors.extend([[shade, shade, shade, 1.0]; 3]);
        let axis = triangle.side / 2;
        batch
            .uvs
            .extend(points.map(|point| [point[(axis + 1) % 3], point[(axis + 2) % 3]]));
        batch.indices.extend([base, base + 1, base + 2]);
    }
    for (index, &slot) in block.face_materials.iter().enumerate() {
        if block.face_materials[..index].contains(&slot) {
            continue;
        }
        if let Some(batch) = batches.get_mut(&slot)
            && starts[index] < batch.positions.len()
        {
            batch.foliage_owners.push(FoliageOwner {
                vertices: starts[index]..batch.positions.len(),
                cell: local,
                material,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needle_templates_are_bounded_wound_outwards_and_keep_lod_color() {
        for index in 0..NEEDLES.len() {
            let triangles = template(index);
            let exposure = (index / 4) % 64;
            let middle = (index / 256) % 2 != 0;
            let maximum = if middle { 68 } else { 116 };
            assert!(triangles.len() <= maximum);
            let mut weighted_tone = 0.0;
            for triangle in triangles {
                assert_ne!(exposure & (1 << triangle.side), 0);
                assert!(triangle.points.iter().all(|point| point.is_finite()
                    && point.cmpge(Vec3::ZERO).all()
                    && point.cmple(Vec3::ONE).all()));
                let cross = (triangle.points[1] - triangle.points[0])
                    .cross(triangle.points[2] - triangle.points[0]);
                assert!(cross.dot(triangle.normal) > 0.0);
                assert!(triangle.normal.is_finite());
                weighted_tone += cross.length() * triangle.tone;
            }
            assert!(weighted_tone.abs() < 1e-5);
        }
        assert_ne!(template(63 * 4)[1].points, template(512 + 63 * 4)[1].points);
    }

    #[test]
    fn conifer_mesh_keeps_one_source_owner_per_material_slot_at_section_boundaries() {
        let block = BlockTemplate {
            surface: None,
            render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
            bevel_width: 0.0,
            faces: std::array::from_fn(|_| Vec::new()),
            face_materials: [7; 6],
            side_cap: None,
            details: Vec::new(),
            model: Vec::new(),
        };
        for level in [DetailLevel::Near, DetailLevel::Middle] {
            for fir in [false, true] {
                let mut batches = BTreeMap::new();
                append(
                    &mut batches,
                    &block,
                    [-17, 31, -1],
                    [15; 3],
                    63,
                    level,
                    (7, fir),
                )
                .unwrap();
                let batch = &batches[&7];
                assert_eq!(batch.foliage_owners.len(), 1);
                let owner = &batch.foliage_owners[0];
                assert_eq!(owner.cell, [15; 3]);
                assert_eq!(owner.material, 7);
                assert_eq!(owner.vertices, 0..batch.positions.len());
                assert_eq!(batch.positions.len(), batch.normals.len());
                assert_eq!(batch.positions.len(), batch.colors.len());
                assert_eq!(batch.positions.len(), batch.uvs.len());
                assert!(
                    batch
                        .positions
                        .iter()
                        .flatten()
                        .all(|v| (15.0..=16.0).contains(v))
                );
                assert_eq!(
                    batch.positions.len(),
                    if level == DetailLevel::Near { 348 } else { 204 }
                );
            }
        }
    }

    #[test]
    fn conifers_keep_distinct_cached_surfaces_and_cull_fully_hidden_cells() {
        assert_eq!(NEEDLES.len(), 1024);
        for middle in [false, true] {
            for exposure in 0..64 {
                for variant in 0..4 {
                    let index = usize::from(middle) * 256 + exposure * 4 + variant;
                    for species in [0, 512] {
                        let crown = template(index + species);
                        assert_eq!(crown.is_empty(), exposure == 0);
                        let distinct: std::collections::HashSet<_> = crown
                            .iter()
                            .map(|triangle| {
                                triangle
                                    .points
                                    .map(|point| point.to_array().map(f32::to_bits))
                            })
                            .collect();
                        assert_eq!(
                            distinct.len(),
                            crown.len(),
                            "Duplicated conifer surfaces at {}",
                            index + species
                        );
                        assert!(std::ptr::eq(crown, template(index + species)));
                    }
                }
            }
        }
    }

    fn signed_volume(triangles: &[Triangle]) -> f32 {
        triangles
            .iter()
            .map(|triangle| {
                triangle.points[0].dot(triangle.points[1].cross(triangle.points[2])) / 6.0
            })
            .sum()
    }

    fn assert_closed_connected(triangles: &[Triangle]) {
        let mut edges = BTreeMap::<_, Vec<_>>::new();
        for (index, triangle) in triangles.iter().enumerate() {
            let points = triangle
                .points
                .map(|point| point.to_array().map(f32::to_bits));
            for (a, b) in [
                (points[0], points[1]),
                (points[1], points[2]),
                (points[2], points[0]),
            ] {
                let (edge, forward) = if a < b {
                    ((a, b), true)
                } else {
                    ((b, a), false)
                };
                edges.entry(edge).or_default().push((index, forward));
            }
        }
        let mut neighbors = vec![Vec::new(); triangles.len()];
        for shared in edges.values() {
            assert_eq!(shared.len(), 2, "Open or non-manifold crown edge");
            assert_ne!(shared[0].1, shared[1].1, "Inconsistent crown winding");
            neighbors[shared[0].0].push(shared[1].0);
            neighbors[shared[1].0].push(shared[0].0);
        }
        let mut visited = vec![false; triangles.len()];
        let mut pending = vec![0];
        while let Some(index) = pending.pop() {
            if !visited[index] {
                visited[index] = true;
                pending.extend(neighbors[index].iter().copied());
            }
        }
        assert!(visited.into_iter().all(|connected| connected));
    }

    #[test]
    fn conifers_have_dense_closed_connected_crown_bodies_in_both_lods() {
        for fir in [false, true] {
            for variant in 0..4 {
                let core = crown_core(63, variant, fir);
                assert_eq!(core.len(), 44);
                assert_closed_connected(&core);
                let minimum_volume = if fir { 0.60 } else { 0.55 };
                assert!((minimum_volume..0.75).contains(&signed_volume(&core)));
                for triangle in &core {
                    assert!(triangle.normal.dot(triangle.points[0] - Vec3::splat(0.5)) > 0.0);
                    assert!(
                        core.iter().flat_map(|face| face.points).all(|point| {
                            triangle.normal.dot(point - triangle.points[0]) <= 1e-6
                        })
                    );
                }
                for middle in [false, true] {
                    let crown = template(
                        usize::from(fir) * 512 + usize::from(middle) * 256 + 63 * 4 + variant,
                    );
                    for (actual, expected) in crown.iter().zip(&core) {
                        assert_eq!(actual.points, expected.points);
                        assert_eq!(actual.normal, expected.normal);
                    }
                }
            }
        }
    }

    #[test]
    fn conifer_needle_tips_are_thick_closed_solids_rooted_inside_the_crown() {
        for fir in [false, true] {
            for variant in 0..4 {
                let core = crown_core(63, variant, fir);
                for middle in [false, true] {
                    let crown = template(
                        usize::from(fir) * 512 + usize::from(middle) * 256 + 63 * 4 + variant,
                    );
                    let tips = &crown[core.len()..];
                    assert_eq!(tips.len(), 6 * if middle { 1 } else { 3 } * 4);
                    for tip in tips.chunks_exact(4) {
                        assert_closed_connected(tip);
                        assert!(signed_volume(tip) > if fir { 0.002 } else { 0.0015 });
                        let roots = tip[0].points;
                        let minimum_depth = if fir { 0.05 } else { 0.04 };
                        assert!(roots.iter().all(|root| {
                            core.iter().all(|face| {
                                face.normal.dot(*root - face.points[0]) < -minimum_depth
                            })
                        }));
                        let point = tip[1]
                            .points
                            .into_iter()
                            .find(|point| !roots.contains(point))
                            .unwrap();
                        assert!(
                            core.iter()
                                .any(|face| { face.normal.dot(point - face.points[0]) > 0.03 })
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn fir_keeps_contiguous_owner_ranges_with_distinct_face_materials() {
        let block = BlockTemplate {
            surface: None,
            render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
            bevel_width: 0.0,
            faces: std::array::from_fn(|_| Vec::new()),
            face_materials: [7, 8, 9, 7, 8, 9],
            side_cap: None,
            details: Vec::new(),
            model: Vec::new(),
        };
        for level in [DetailLevel::Near, DetailLevel::Middle] {
            let mut batches = BTreeMap::new();
            for (index, local) in [[0; 3], [15; 3]].into_iter().enumerate() {
                append(
                    &mut batches,
                    &block,
                    [-17, 31, -1],
                    local,
                    63,
                    level,
                    (7, true),
                )
                .unwrap();
                for batch in batches.values() {
                    assert_eq!(batch.foliage_owners.len(), index + 1);
                    let owner = &batch.foliage_owners[index];
                    let start = if index == 0 {
                        0
                    } else {
                        batch.foliage_owners[0].vertices.end
                    };
                    assert_eq!(owner.vertices, start..batch.positions.len());
                    assert_eq!(owner.cell, local);
                    assert_eq!(owner.material, 7);
                    assert_eq!(batch.positions.len(), batch.normals.len());
                    assert_eq!(batch.positions.len(), batch.colors.len());
                    assert_eq!(batch.positions.len(), batch.uvs.len());
                }
            }
        }
    }
}
