//! Recessed sockets surround separate opaque, red pupil meshes.
use super::*;

pub(super) fn meshes(armored: bool) -> Result<(Mesh, Mesh), String> {
    let mut skull = Builder::with_cell_size(0.01);
    // The lower face is hollow in front of the rear cranium. A solid box here
    // would hide the pupils as soon as the head turns.
    skull.rounded([0.0, 0.115, 0.015], [0.42, 0.24, 0.35], 0.05, IVORY)?;
    // Uneven cranial plates and thin sutures read as bone rather than a white
    // cube. All details belong to the same solid occupancy as the skull.
    for side in [-1.0, 1.0] {
        skull.rounded(
            [side * 0.105, 0.205, 0.02],
            [0.20, 0.085, 0.31],
            0.018,
            IVORY,
        )?;
        skull.voxel([side * 0.115, 0.221, 0.182], [0.012, 0.035, 0.012], BONE)?;
        skull.voxel([side * 0.198, 0.113, -0.074], [0.018, 0.105, 0.020], BONE)?;
    }
    skull.voxel([0.0, 0.248, 0.03], [0.012, 0.008, 0.19], BONE)?;
    skull.voxel([0.025, 0.248, 0.105], [0.055, 0.008, 0.012], BONE)?;
    skull.rounded([0.0, -0.035, 0.07], [0.36, 0.18, 0.22], 0.025, BONE)?;
    skull.voxel([0.0, 0.028, -0.15], [0.35, 0.055, 0.075], IVORY)?;
    for side in [-1.0, 1.0] {
        skull.voxel([side * 0.085, -0.045, -0.065], [0.12, 0.10, 0.025], DARK)?;
        skull.voxel([side * 0.165, -0.04, -0.115], [0.095, 0.12, 0.10], IVORY)?;
        skull.voxel([side * 0.145, -0.12, -0.105], [0.045, 0.09, 0.11], BONE)?;
    }
    skull.voxel([0.0, -0.025, -0.145], [0.04, 0.11, 0.055], BONE)?;
    skull.voxel([0.0, -0.083, -0.154], [0.035, 0.028, 0.02], DARK)?;
    skull.voxel([0.0, -0.17, -0.105], [0.31, 0.045, 0.13], IVORY)?;
    for tooth in -2..=2 {
        skull.voxel(
            [tooth as f32 * 0.055, -0.13, -0.159],
            [0.035, 0.045, 0.037],
            IVORY,
        )?;
    }
    if armored {
        skull.rounded([0.0, 0.18, 0.02], [0.48, 0.21, 0.41], 0.035, STEEL)?;
        skull.voxel([0.0, 0.064, -0.177], [0.44, 0.04, 0.06], EDGE)?;
        skull.voxel([0.0, 0.19, 0.015], [0.055, 0.22, 0.44], EDGE)?;
        skull.rounded([0.0, 0.28, 0.05], [0.10, 0.09, 0.31], 0.018, STEEL)?;
        skull.voxel([0.0, 0.326, 0.05], [0.045, 0.018, 0.27], EDGE)?;
        skull.voxel([0.0, -0.01, -0.195], [0.043, 0.16, 0.035], STEEL)?;
        for side in [-1.0, 1.0] {
            skull.voxel([side * 0.22, -0.015, 0.035], [0.05, 0.20, 0.29], STEEL)?;
            skull.voxel([side * 0.22, -0.105, 0.035], [0.065, 0.035, 0.30], EDGE)?;
            skull.voxel([side * 0.183, 0.139, -0.191], [0.025, 0.025, 0.018], EDGE)?;
            skull.voxel([side * 0.225, 0.13, 0.10], [0.025, 0.025, 0.03], EDGE)?;
            // An open ear vent and a stepped cheek guard preserve the sockets.
            skull.voxel([side * 0.249, 0.01, 0.066], [0.006, 0.075, 0.025], DARK)?;
        }
    }
    // An independent detail grid preserves both pupils without increasing the
    // resolution of the whole skeleton or painting red onto the bone material.
    let mut pupils = Builder::with_cell_size(0.004);
    for side in [-1.0, 1.0] {
        pupils.voxel(
            [side * 0.085, -0.045, -0.105],
            [0.036, 0.044, 0.016],
            [0.95, 0.008, 0.012],
        )?;
        pupils.voxel(
            [side * 0.085, -0.041, -0.114],
            [0.012, 0.016, 0.004],
            [1.0, 0.05, 0.04],
        )?;
    }
    Ok((skull.finish()?, pupils.finish()?))
}
