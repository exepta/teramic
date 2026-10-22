//! Small attached relief, sharing the existing section/material batches.
use super::{
    DetailLevel,
    builder::{MeshBatch, variation},
};
use crate::blocks::{BlockTemplate, TemplateSet};
use bevy::prelude::*;
use std::collections::BTreeMap;

#[allow(clippy::too_many_arguments)]
pub(super) fn append(
    batches: &mut BTreeMap<usize, MeshBatch>,
    templates: &TemplateSet,
    block: &BlockTemplate,
    material_id: u16,
    global: [i64; 3],
    local: Vec3,
    side: usize,
    level: DetailLevel,
) -> Result<(), String> {
    if level == DetailLevel::Far {
        return Ok(());
    }
    let seed = variation(global, 0x004d_5553_4852 + side as u64);
    let axis = side / 2;
    let stem = material_id == 20;
    let probability = if stem || side == 2 {
        0.55
    } else {
        match material_id {
            22 => 0.46,
            21 | 25 => 0.30,
            24 => 0.22,
            _ => 0.12,
        }
    };
    let random = |shift: u32| ((seed >> shift) & 255_u64) as f32 / 255.0;
    if !stem && side == 2 && random(56) < 0.28 {
        append_fibres(batches, block.face_materials[side], local, seed, level)?;
    }
    if (stem && axis == 1) || random(0) >= probability {
        return Ok(());
    }
    let count = if level == DetailLevel::Middle {
        1
    } else if !stem && side != 2 && random(48) > 0.5 {
        3
    } else {
        2
    };
    for detail in 0..count {
        append_detail(
            batches,
            templates,
            block,
            material_id,
            local,
            side,
            variation(global, 0x004d_5553_4852 + side as u64 + detail * 37),
            detail,
        )?;
    }
    Ok(())
}

fn append_fibres(
    batches: &mut BTreeMap<usize, MeshBatch>,
    material: usize,
    local: Vec3,
    seed: u64,
    level: DetailLevel,
) -> Result<(), String> {
    let random = |shift: u32| ((seed >> shift) & 255_u64) as f32 / 255.0;
    let strands = if level == DetailLevel::Near { 2 } else { 1 };
    for strand in 0..strands {
        let length = 0.35 + random(16 + strand * 8).powf(1.35) * 2.15;
        let root_y = local.y + 0.025;
        let mut start =
            local + Vec3::new(0.23 + strand as f32 * 0.43, 0.025, 0.20 + random(32) * 0.55);
        // Each ribbon is one connected three-segment strip, not falling particles.
        for segment in 0..3 {
            let bend = (random(8 + segment * 8) - 0.5) * 0.10;
            let end = start + Vec3::new(bend, -length / 3.0, bend * 0.45);
            let direction = end - start;
            let first = batches
                .get(&material)
                .map_or(0, |batch| batch.positions.len());
            crate::props::add_box(
                batches,
                material,
                (start + end) * 0.5,
                Vec3::new(
                    0.065 - segment as f32 * 0.013,
                    direction.length() + 0.012,
                    0.023,
                ),
                Quat::from_rotation_arc(Vec3::Y, direction.normalize()),
            )?;
            if let Some(batch) = batches.get_mut(&material) {
                batch.colors.resize(batch.positions.len(), [1.0; 4]);
                batch.colors[first..].fill([1.08, 1.04, 0.94, 1.0]);
                batch.rooted_wind.resize(batch.positions.len(), [0.0; 2]);
                for (point, weight) in batch.positions[first..]
                    .iter()
                    .zip(&mut batch.rooted_wind[first..])
                {
                    let distance = ((root_y - point[1] - 0.035) / (length - 0.035)).clamp(0.0, 1.0);
                    *weight = [0.0, distance * distance];
                }
            }
            start = end;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_detail(
    batches: &mut BTreeMap<usize, MeshBatch>,
    templates: &TemplateSet,
    block: &BlockTemplate,
    material_id: u16,
    local: Vec3,
    side: usize,
    seed: u64,
    detail: u64,
) -> Result<(), String> {
    let axis = side / 2;
    let stem = material_id == 20;
    let random = |shift: u32| ((seed >> shift) & 255_u64) as f32 / 255.0;
    let sign = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
    let mut size = Vec3::splat(0.16 + random(8) * 0.24);
    let mut center = Vec3::splat(0.5);
    let u = (axis + 1) % 3;
    let v = (axis + 2) % 3;
    size[v] *= 0.60 + random(32) * 0.70;
    let material = if stem {
        size[1] = 0.70;
        size[if axis == 0 { 2 } else { 0 }] = 0.10;
        block.face_materials[side]
    } else if side == 2 {
        // Short lamella ridges catch light below the cap instead of a flat decal.
        size[0] = 0.09;
        size[2] = 0.70;
        block.face_materials[side]
    } else {
        templates
            .blocks
            .get(&26)
            .map_or(block.face_materials[side], |cap| cap.face_materials[3])
    };
    center[u] += (random(16) * 2.0 - 1.0) * (0.45 - size[u] * 0.5);
    center[v] += (random(24) * 2.0 - 1.0) * (0.45 - size[v] * 0.5);
    if stem {
        center[if axis == 0 { 2 } else { 0 }] = 0.25 + detail as f32 * 0.5;
    } else if side == 2 {
        center[0] = 0.25 + detail as f32 * 0.5;
    }
    let overlap = if stem || side == 2 { 0.035 } else { 0.085 };
    size[axis] = overlap + 0.012 + random(40) * 0.023;
    // The buried portion reaches below the deepest surface relief. Only a
    // thin, irregular 1.2-3.5cm scale protrudes, never a detached wooden peg.
    center[axis] = (side % 2) as f32 + sign * (size[axis] * 0.5 - overlap);
    let first = batches
        .get(&material)
        .map_or(0, |batch| batch.positions.len());
    crate::props::add_box(batches, material, local + center, size, Quat::IDENTITY)?;
    if let Some(batch) = batches.get_mut(&material) {
        batch.colors.resize(batch.positions.len(), [1.0; 4]);
        let shade = if stem {
            0.91
        } else if side == 2 {
            0.94
        } else {
            1.03
        };
        batch.colors[first..].fill([shade, shade, shade, 1.0]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::definitions::BlockRenderKind;

    #[test]
    fn fibre_lengths_vary_up_to_two_and_a_half_blocks_and_root_weights_stay_fixed() {
        let mut shortest = f32::INFINITY;
        let mut longest = 0.0_f32;
        for seed in 0..128 {
            let seed = variation([seed, 77, -11], 26);
            let mut batches = BTreeMap::new();
            append_fibres(&mut batches, 0, Vec3::ZERO, seed, DetailLevel::Near).unwrap();
            let batch = &batches[&0];
            assert_eq!(batch.positions.len(), 144);
            assert_eq!(batch.rooted_wind.len(), batch.positions.len());
            for strand in batch.positions.chunks_exact(72) {
                let minimum = strand
                    .iter()
                    .map(|point| point[1])
                    .fold(f32::INFINITY, f32::min);
                let length = 0.025 - minimum;
                assert!((0.35..=2.51).contains(&length));
                shortest = shortest.min(length);
                longest = longest.max(length);
            }
            assert!(batch.rooted_wind.iter().any(|weight| weight[1] > 0.95));
            for (point, weight) in batch.positions.iter().zip(&batch.rooted_wind) {
                assert!((0.0..=1.0).contains(&weight[1]));
                if point[1] >= 0.0 {
                    assert_eq!(weight[1], 0.0);
                }
                let sway = |time: f32| {
                    ((point[0] * 0.33 + point[2] * 0.27 + time * 0.85).sin() * 0.048
                        + (point[0] * 1.17 - point[2] * 0.91 + point[1] * 0.67 + time * 2.3).sin()
                            * 0.009)
                        * weight[1]
                };
                assert!(sway(0.3).abs() < crate::foliage::WIND_BOUNDS);
                if weight[1] == 0.0 {
                    assert_eq!(sway(0.3), 0.0);
                }
            }
            let mut middle = BTreeMap::new();
            append_fibres(&mut middle, 0, Vec3::ZERO, seed, DetailLevel::Middle).unwrap();
            assert_eq!(middle[&0].positions.len(), 72);
            assert_eq!(middle[&0].positions, batch.positions[..72]);
        }
        assert!(
            shortest < 0.5 && longest > 2.3,
            "range {shortest}..{longest}"
        );
    }

    #[test]
    fn scales_survive_coarse_surfaces_and_middle_lod_with_bounded_geometry() {
        use crate::blocks::{cube_faces, definitions::CubeSurface, surface::SurfaceTemplate};
        use crate::meshing::{
            DetailLevel,
            builder::{RenderOptions, compose_section_with_options},
        };
        use terra_voxel::{SectionPos, meshing::BlockFace};
        let make = |material| BlockTemplate {
            render_kind: BlockRenderKind::Cube,
            face_materials: [material; 6],
            side_cap: None,
            faces: cube_faces(0.035, [material; 6], None),
            model: Vec::new(),
            details: Vec::new(),
            bevel_width: 0.035,
            surface: Some(SurfaceTemplate::new(CubeSurface::Mushroom)),
        };
        let mut templates = TemplateSet::default();
        templates.blocks.insert(22, make(5));
        templates.blocks.insert(26, make(9));
        let face = BlockFace {
            edge_mask: 15,
            occupied_neighbor: false,
            minimum: [0; 3],
            width: 16,
            height: 1,
            u_axis: 0,
            v_axis: 2,
            side: 3,
            material: 22,
        };
        let options = RenderOptions {
            detail_level: DetailLevel::Near,
            water_level: None,
            terrain: None,
            exposure: None,
            conditions: Default::default(),
        };
        let near =
            compose_section_with_options(&[face], &templates, SectionPos::new(0, 0, 0), &options)
                .unwrap();
        assert!(near.iter().any(|batch| batch.material == 9));
        for block in templates.blocks.values_mut() {
            block.surface = None;
        }
        let coarse =
            compose_section_with_options(&[face], &templates, SectionPos::new(0, 0, 0), &options)
                .unwrap();
        assert!(
            coarse.iter().any(|batch| batch.material == 9),
            "mushroom details must survive the 48m micro-relief cutoff"
        );
        let middle = compose_section_with_options(
            &[face],
            &templates,
            SectionPos::default(),
            &RenderOptions {
                detail_level: DetailLevel::Middle,
                ..options.clone()
            },
        )
        .unwrap();
        assert!(middle.iter().any(|batch| batch.material == 9));
        assert!(
            middle
                .iter()
                .map(|batch| batch.positions.len())
                .sum::<usize>()
                < coarse
                    .iter()
                    .map(|batch| batch.positions.len())
                    .sum::<usize>()
        );
        let far = compose_section_with_options(
            &[face],
            &templates,
            SectionPos::default(),
            &RenderOptions {
                detail_level: DetailLevel::Far,
                ..options
            },
        )
        .unwrap();
        assert!(far.iter().all(|batch| batch.material == 5));
    }

    #[test]
    fn relief_is_deterministic_attached_and_bounded_with_valid_indices() {
        let block = BlockTemplate {
            render_kind: BlockRenderKind::Cube,
            faces: Default::default(),
            face_materials: [0; 6],
            side_cap: None,
            model: Vec::new(),
            details: Vec::new(),
            bevel_width: 0.035,
            surface: None,
        };
        let templates = TemplateSet::default();
        for material in 20..=26 {
            for side in 0..6 {
                for x in -4..4 {
                    let mut a = BTreeMap::new();
                    let mut b = BTreeMap::new();
                    append(
                        &mut a,
                        &templates,
                        &block,
                        material,
                        [x, 2, -17],
                        Vec3::ZERO,
                        side,
                        DetailLevel::Near,
                    )
                    .unwrap();
                    append(
                        &mut b,
                        &templates,
                        &block,
                        material,
                        [x, 2, -17],
                        Vec3::ZERO,
                        side,
                        DetailLevel::Near,
                    )
                    .unwrap();
                    assert_eq!(a, b);
                    for batch in a.values() {
                        let fibres = side == 2
                            && material != 20
                            && batch.positions.iter().any(|point| point[1] < -0.1);
                        if fibres {
                            assert!(batch.positions.len() <= 192);
                            assert_eq!(batch.positions.len() % 24, 0);
                        } else {
                            assert!(matches!(batch.positions.len(), 48 | 72));
                        }
                        assert_eq!(batch.positions.len(), batch.colors.len());
                        assert!(
                            batch
                                .indices
                                .iter()
                                .all(|&i| i < batch.positions.len() as u32)
                        );
                        for axis in 0..3 {
                            let minimum = batch
                                .positions
                                .iter()
                                .map(|p| p[axis])
                                .fold(f32::INFINITY, f32::min);
                            let maximum = batch
                                .positions
                                .iter()
                                .map(|p| p[axis])
                                .fold(f32::NEG_INFINITY, f32::max);
                            if axis == side / 2 {
                                let plane = (side % 2) as f32;
                                assert!(
                                    minimum < plane && maximum > plane,
                                    "relief must intersect its source face"
                                );
                                assert!(minimum >= if fibres { -2.51 } else { -0.086 });
                                assert!(maximum <= 1.086);
                            } else {
                                assert!(minimum >= 0.0 && maximum <= 1.0);
                            }
                        }
                    }
                }
            }
        }
    }
}
