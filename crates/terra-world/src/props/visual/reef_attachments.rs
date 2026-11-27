//! Rock-hugging sea fans and hanging beaded kelp share the renderer and hit mesh.
use super::{Batches, PropDetail, PropKind, PropPalette, colored_voxel};

pub(super) fn append(
    b: &mut Batches,
    kind: PropKind,
    v: usize,
    lod: PropDetail,
    p: &PropPalette,
) -> Result<(), String> {
    let far = lod == PropDetail::Far;
    let near = lod == PropDetail::Near;
    if kind == PropKind::ReefFan {
        colored_voxel(
            b,
            p.crop,
            [0., 0., 0.045],
            [0.14, 0.13, 0.10],
            [0.12, 0.31, 0.26],
        )?;
        let blades = if far { 3 } else { 5 };
        let layers = if far {
            1
        } else if near {
            5
        } else {
            3
        };
        for i in 0..blades {
            let side = i as f32 / (blades - 1) as f32 * 2. - 1.;
            for j in 0..layers {
                let t = (j as f32 + 0.5) / layers as f32;
                let color = [0.15 + 0.12 * t, 0.49 + 0.32 * t, 0.38 + 0.32 * t];
                colored_voxel(
                    b,
                    p.crop,
                    [
                        side * t * 0.34,
                        t * (0.32 - side.abs() * 0.1),
                        0.09 + t * 0.26,
                    ],
                    [0.095, 0.44 / layers as f32, 0.35 / layers as f32],
                    color,
                )?;
            }
        }
    } else {
        let strands = if far { 2 } else { 3 };
        let segments = if far {
            1
        } else if near {
            5
        } else {
            3
        };
        for i in 0..strands {
            let angle = i as f32 * 2.4 + v as f32 * 0.5;
            let length = 0.53 + 0.065 * ((i + v) % 4) as f32;
            for j in 0..segments {
                let t = (j as f32 + 0.5) / segments as f32;
                let offset = 0.12 + 0.09 * t;
                let center = [angle.sin() * offset, -t * length, angle.cos() * offset];
                colored_voxel(
                    b,
                    p.crop,
                    center,
                    [0.055, length / segments as f32 + 0.006, 0.060],
                    [0.2, 0.52, 0.36],
                )?;
                if !far && j % 2 == 0 {
                    colored_voxel(
                        b,
                        p.crop,
                        [center[0] + 0.04, center[1], center[2]],
                        [0.105, 0.085, 0.092],
                        [0.56, 0.86, 0.71],
                    )?;
                }
            }
        }
    }
    Ok(())
}
