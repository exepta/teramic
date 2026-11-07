//! Keep contact shadows local when a large greedy quad borders a raised block.
use super::{MeshBatch, sample};
use std::collections::BTreeMap;

const MAX_QUADS: usize = 64;
const MAX_ERROR: f32 = 0.015;

#[derive(Clone, Copy, Debug)]
struct Rect {
    axes: [usize; 2],
    u: usize,
    v: usize,
    width: usize,
    height: usize,
}

fn interpolate<const N: usize>(corners: [[f32; N]; 4], u: f32, v: f32) -> [f32; N] {
    std::array::from_fn(|axis| {
        let [a, b, c, d] = corners.map(|point| point[axis]);
        if u >= v {
            a * (1.0 - u) + b * (u - v) + c * v
        } else {
            a * (1.0 - v) + c * u + d * (v - u)
        }
    })
}

fn inaccurate(rect: Rect, grid: &[f32; 289]) -> bool {
    let corners = [
        grid[rect.u + rect.v * 17],
        grid[rect.u + rect.width + rect.v * 17],
        grid[rect.u + rect.width + (rect.v + rect.height) * 17],
        grid[rect.u + (rect.v + rect.height) * 17],
    ]
    .map(|value| [value]);
    (0..=rect.height).any(|v| {
        (0..=rect.width).any(|u| {
            let linear = interpolate(
                corners,
                u as f32 / rect.width as f32,
                v as f32 / rect.height as f32,
            )[0];
            (linear - grid[rect.u + u + (rect.v + v) * 17]).abs() > MAX_ERROR
        })
    })
}

fn partition(rect: Rect, grid: &[f32; 289], quota: usize, result: &mut Vec<Rect>) {
    if quota == 1 || rect.width == 1 && rect.height == 1 || !inaccurate(rect, grid) {
        result.push(rect);
        return;
    }
    let mut a = rect;
    let mut b = rect;
    if rect.width >= rect.height {
        a.width /= 2;
        b.u += a.width;
        b.width -= a.width;
    } else {
        a.height /= 2;
        b.v += a.height;
        b.height -= a.height;
    }
    partition(a, grid, quota / 2, result);
    partition(b, grid, quota - quota / 2, result);
}

fn point(batch: &MeshBatch, start: usize, rect: Rect, u: usize, v: usize) -> [f32; 3] {
    let [u_axis, v_axis] = rect.axes;
    let mut point = batch.positions[start];
    point[u_axis] += u as f32;
    point[v_axis] += v as f32;
    point
}

fn quad(batch: &MeshBatch, start: usize) -> Option<(Rect, bool)> {
    let base = u32::try_from(start).ok()?;
    let index = &batch.indices[start / 4 * 6..][..6];
    let reverse = if index == [base, base + 1, base + 2, base, base + 2, base + 3] {
        false
    } else if index == [base, base + 2, base + 1, base, base + 3, base + 2] {
        true
    } else {
        return None;
    };
    let p = &batch.positions[start..][..4];
    let n = batch.normals[start];
    if batch.normals[start..start + 4]
        .iter()
        .any(|&normal| normal != n)
        || n.iter().filter(|&&value| value.abs() == 1.0).count() != 1
        || n.iter().any(|value| *value != 0.0 && value.abs() != 1.0)
        || p.iter()
            .flatten()
            .any(|value| !value.is_finite() || value.fract() != 0.0)
    {
        return None;
    }
    let axis = n.iter().position(|value| value.abs() == 1.0)?;
    // The voxel mesher uses Yzx for horizontal faces, whereas authored cube
    // quads may use Yxz. Preserve the actual vertex order in either case.
    let u = (0..3).find(|&axis| p[1][axis] != p[0][axis])?;
    let v = (0..3).find(|&axis| p[3][axis] != p[0][axis])?;
    if u == axis || v == axis || u == v {
        return None;
    }
    let width = p[1][u] - p[0][u];
    let height = p[3][v] - p[0][v];
    if !(1.0..=16.0).contains(&width)
        || !(1.0..=16.0).contains(&height)
        || p[1][v] != p[0][v]
        || p[3][u] != p[0][u]
        || p[2][u] != p[1][u]
        || p[2][v] != p[3][v]
        || p.iter().any(|point| point[axis] != p[0][axis])
    {
        return None;
    }
    Some((
        Rect {
            axes: [u, v],
            u: 0,
            v: 0,
            width: width as usize,
            height: height as usize,
        },
        reverse,
    ))
}

pub(super) fn refine(batch: &mut MeshBatch, bits: &[u64], cache: &mut [u8]) {
    let length = batch.positions.len();
    if length == 0
        || !length.is_multiple_of(4)
        || batch.indices.len() != length / 4 * 6
        || batch.normals.len() != length
        || [&batch.uvs, &batch.surface_water]
            .iter()
            .any(|values| !values.is_empty() && values.len() != length)
        || [&batch.colors, &batch.tangents]
            .iter()
            .any(|values| !values.is_empty() && values.len() != length)
        || !batch.rooted_wind.is_empty()
        || !batch.growth_anchors.is_empty()
        || !batch.growth_ranges.is_empty()
        || !batch.foliage_owners.is_empty()
        || !batch.compact_indices.is_empty()
        || !batch.packed_normals.is_empty()
        || !batch.packed_tangents.is_empty()
        || !batch.packed_colors.is_empty()
        || batch.staged.is_some()
    {
        return;
    }
    let mut plans = BTreeMap::new();
    for start in (0..length).step_by(4) {
        let Some((rect, _)) = quad(batch, start) else {
            return;
        };
        if rect.width == 1 && rect.height == 1 {
            continue;
        }
        let mut grid = [1.0; 289];
        for v in 0..=rect.height {
            for u in 0..=rect.width {
                grid[u + v * 17] = sample(
                    bits,
                    point(batch, start, rect, u, v),
                    batch.normals[start],
                    cache,
                );
            }
        }
        if !inaccurate(rect, &grid) {
            continue;
        }
        let mut parts = Vec::new();
        partition(rect, &grid, MAX_QUADS, &mut parts);
        if parts.len() > 1 {
            plans.insert(start, parts);
        }
    }
    if plans.is_empty() {
        return;
    }
    // Only independent integer cube quads reach this point. Authored relief,
    // collision geometry and material ownership never change here.
    let source = std::mem::take(batch);
    batch.material = source.material;
    batch.shadow_only = source.shadow_only;
    batch.shadow_sources = source.shadow_sources.clone();
    for start in (0..length).step_by(4) {
        let (whole, reverse) = quad(&source, start).unwrap();
        let single = [whole];
        for rect in plans.get(&start).map_or(single.as_slice(), Vec::as_slice) {
            let points = [
                (rect.u, rect.v),
                (rect.u + rect.width, rect.v),
                (rect.u + rect.width, rect.v + rect.height),
                (rect.u, rect.v + rect.height),
            ];
            let base = batch.positions.len() as u32;
            for (u, v) in points {
                batch.positions.push(point(&source, start, *rect, u, v));
                let (u, v) = (
                    u as f32 / whole.width as f32,
                    v as f32 / whole.height as f32,
                );
                batch.normals.push(source.normals[start]);
                if !source.uvs.is_empty() {
                    batch.uvs.push(interpolate(
                        std::array::from_fn(|i| source.uvs[start + i]),
                        u,
                        v,
                    ));
                }
                if !source.colors.is_empty() {
                    batch.colors.push(interpolate(
                        std::array::from_fn(|i| source.colors[start + i]),
                        u,
                        v,
                    ));
                }
                if !source.tangents.is_empty() {
                    batch.tangents.push(interpolate(
                        std::array::from_fn(|i| source.tangents[start + i]),
                        u,
                        v,
                    ));
                }
                if !source.surface_water.is_empty() {
                    batch.surface_water.push(interpolate(
                        std::array::from_fn(|i| source.surface_water[start + i]),
                        u,
                        v,
                    ));
                }
            }
            batch.indices.extend(
                if reverse {
                    [0, 2, 1, 0, 3, 2]
                } else {
                    [0, 1, 2, 0, 2, 3]
                }
                .map(|i| base + i),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{blocks::TemplateSet, meshing::builder::append_greedy};

    fn face() -> terra_voxel::meshing::BlockFace {
        let mut voxels = vec![0; terra_voxel::meshing::PADDED_VOLUME];
        for z in 0..18 {
            for x in 0..18 {
                voxels[x + 18 * (1 + 18 * z)] = 1;
            }
        }
        terra_voxel::meshing::mesh_section(&voxels)
            .unwrap()
            .faces
            .into_iter()
            .find(|face| face.side == 3 && face.width == 16 && face.height == 16)
            .expect("the real mesher emits one greedy grass top")
    }

    fn batch() -> MeshBatch {
        batch_from(face())
    }

    fn batch_from(face: terra_voxel::meshing::BlockFace) -> MeshBatch {
        let mut batches = BTreeMap::new();
        let template = crate::blocks::BlockTemplate {
            surface: None,
            render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
            bevel_width: 0.0,
            faces: crate::blocks::cube_faces(0.0, [4; 6], None),
            face_materials: [4; 6],
            side_cap: None,
            details: Vec::new(),
            model: Vec::new(),
        };
        append_greedy(&mut batches, face, &template).unwrap();
        let mut batch = batches.remove(&4).unwrap();
        batch.colors = batch
            .positions
            .iter()
            .map(|p| [0.6 + p[0] * 0.01, 0.7, 0.8 + p[2] * 0.005, 0.37])
            .collect();
        batch.tangents = vec![[1.0, 0.0, 0.0, -1.0]; 4];
        batch.surface_water = vec![[3.25, 1.5]; 4];
        batch
    }

    fn halo(origin: [i32; 3], cells: &[[i32; 3]]) -> Vec<u64> {
        let mut bits = vec![0; terra_voxel::meshing::PADDED_VOLUME.div_ceil(64)];
        for z in -1..=16 {
            for y in -1..=16 {
                for x in -1..=16 {
                    if cells.contains(&[x + origin[0], y + origin[1], z + origin[2]]) {
                        let i = ((x + 1) + 18 * ((y + 1) + 18 * (z + 1))) as usize;
                        bits[i / 64] |= 1 << (i % 64);
                    }
                }
            }
        }
        bits
    }

    fn shade_at(batch: &MeshBatch, point: [f32; 2]) -> [f32; 4] {
        for start in (0..batch.positions.len()).step_by(4) {
            let a = batch.positions[start];
            let c = batch.positions[start + 2];
            if (a[0]..=c[0]).contains(&point[0]) && (a[2]..=c[2]).contains(&point[1]) {
                let (rect, _) = quad(batch, start).unwrap();
                let position = [point[0], a[1], point[1]];
                let color = std::array::from_fn(|i| {
                    if batch.colors.is_empty() {
                        batch.packed_colors[start + i].map(|v| f32::from(v) / 255.0)
                    } else {
                        batch.colors[start + i]
                    }
                });
                return interpolate(
                    color,
                    (position[rect.axes[0]] - a[rect.axes[0]]) / rect.width as f32,
                    (position[rect.axes[1]] - a[rect.axes[1]]) / rect.height as f32,
                );
            }
        }
        panic!("point is outside the original face");
    }

    #[test]
    fn real_voxel_top_order_and_authored_order_keep_the_same_local_contact_shadow() {
        let voxel = face();
        assert_eq!((voxel.u_axis, voxel.v_axis), (2, 0));
        let mut authored = voxel;
        std::mem::swap(&mut authored.u_axis, &mut authored.v_axis);
        let bits = halo([0; 3], &[[-1, 1, 0]]);
        let mut fields = Vec::new();
        for face in [voxel, authored] {
            let mut batch = batch_from(face);
            super::super::apply(
                std::slice::from_mut(&mut batch),
                &bits,
                &TemplateSet::default(),
            );
            assert!(batch.positions.len() > 4 && batch.positions.len() <= MAX_QUADS * 4);
            assert!((shade_at(&batch, [8.0, 8.0])[1] - 0.7).abs() < 0.005);
            fields.push(
                (0..=16)
                    .flat_map(|z| (0..=16).map(move |x| [x as f32, z as f32]))
                    .map(|point| shade_at(&batch, point))
                    .collect::<Vec<_>>(),
            );
        }
        for (voxel, authored) in fields[0].iter().zip(&fields[1]) {
            for channel in 0..4 {
                assert!((voxel[channel] - authored[channel]).abs() < 0.005);
            }
        }
    }

    #[test]
    fn raised_corner_keeps_contact_local_instead_of_darkening_a_whole_greedy_face() {
        let source = batch();
        let bits = halo([0; 3], &[[-1, 1, 0]]);
        let old_center = interpolate(
            std::array::from_fn(|i| {
                [sample(
                    &bits,
                    source.positions[i],
                    source.normals[i],
                    &mut [],
                )]
            }),
            0.5,
            0.5,
        )[0];
        assert!(
            old_center < 0.95,
            "regression fixture reproduces broad false shadow"
        );
        assert_eq!(
            sample(&bits, [8.0, 1.0, 8.0], [0.0, 1.0, 0.0], &mut []),
            1.0
        );
        let mut batches = [source];
        super::super::apply(&mut batches, &bits, &TemplateSet::default());
        let refined = &batches[0];
        assert!(refined.positions.len() > 4 && refined.positions.len() <= MAX_QUADS * 4);
        let center = shade_at(refined, [8.0, 8.0]);
        assert!((center[0] - 0.68).abs() < 0.005 && (center[1] - 0.7).abs() < 0.005);
        assert!(shade_at(refined, [0.0, 0.0])[1] < 0.65);
        for (&position, &water) in refined.positions.iter().zip(&refined.surface_water) {
            assert_eq!(position[1], 1.0);
            assert!(
                position
                    .into_iter()
                    .all(|value| value.fract() == 0.0 && (0.0..=16.0).contains(&value))
            );
            assert_eq!(water, [3.25, 1.5]);
        }
        let area: f32 = refined
            .positions
            .chunks_exact(4)
            .map(|p| (p[2][0] - p[0][0]) * (p[2][2] - p[0][2]))
            .sum();
        assert_eq!(area, 256.0);
        assert!(refined.tangents.iter().all(|&t| t == [1.0, 0.0, 0.0, -1.0]));
        assert!(
            refined
                .indices
                .iter()
                .all(|&i| (i as usize) < refined.positions.len())
        );
    }

    #[test]
    fn prepacked_tangents_are_never_lost_by_greedy_refinement() {
        let packed = || {
            let mut batch = batch();
            batch.tangents.clear();
            batch.packed_tangents = vec![[127, 0, 0, -127]; 4];
            batch
        };
        let expected = packed();
        let mut actual = packed();
        refine(&mut actual, &halo([0; 3], &[[-1, 1, 0]]), &mut []);
        assert_eq!(actual, expected);
    }

    #[test]
    fn uniform_lighting_retains_greedy_geometry_and_dense_relief_is_never_rebuilt() {
        let mut actual = batch();
        let mut original = batch();
        refine(&mut actual, &halo([0; 3], &[]), &mut []);
        assert_eq!(actual, original);
        actual.positions[1][1] = 0.99;
        original.positions[1][1] = 0.99;
        refine(&mut actual, &halo([0; 3], &[[-1, 1, 0]]), &mut []);
        assert_eq!(actual, original);
    }

    #[test]
    fn adaptive_lighting_matches_at_negative_section_edges_and_keeps_a_hard_geometry_bound() {
        let occupied = [[-1, 49, -17]];
        let mut edges = Vec::new();
        for (origin, x) in [([-16, 48, -16], 16.0), ([0, 48, -16], 0.0)] {
            let mut batch = batch();
            batch.colors.fill([0.75, 0.8, 0.9, 0.37]);
            super::super::apply(
                std::slice::from_mut(&mut batch),
                &halo(origin, &occupied),
                &TemplateSet::default(),
            );
            assert!(batch.positions.len() <= MAX_QUADS * 4);
            edges.push(
                (0..=16)
                    .map(|z| shade_at(&batch, [x, z as f32]))
                    .collect::<Vec<_>>(),
            );
        }
        for (left, right) in edges[0].iter().zip(&edges[1]) {
            for axis in 0..4 {
                assert!((left[axis] - right[axis]).abs() < 0.005);
            }
        }
        let mut batch = batch();
        let cells: Vec<_> = (0..16).step_by(2).map(|z| [-1, 1, z]).collect();
        refine(&mut batch, &halo([0; 3], &cells), &mut []);
        assert!(batch.positions.len() <= MAX_QUADS * 4);
    }
}
