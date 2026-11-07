use super::{Builder, Palette};
use bevy::prelude::*;

pub(super) fn build(builder: &mut Builder, palette: &Palette) -> Result<(), String> {
    builder.bevel_box(
        [0.5, 0.30, 0.5],
        [0.052, 0.60, 0.055],
        palette.body,
        Quat::IDENTITY,
    )?;
    builder.bevel_box(
        [0.5, 0.20, 0.5],
        [0.066, 0.055, 0.068],
        palette.fitting,
        Quat::IDENTITY,
    )?;
    for side in 0..12 {
        let point = |index: usize, back: bool| {
            let angle = index as f32 * std::f32::consts::TAU / 12.0;
            Vec3::new(
                0.5 + angle.cos() * 0.13,
                0.80 + angle.sin() * 0.20,
                if back { 0.525 } else { 0.55 },
            )
        };
        let a = point(side, false);
        let b = point(side + 1, false);
        builder.triangle([Vec3::new(0.5, 0.80, 0.405), a, b], palette.light)?;
        builder.triangle(
            [
                Vec3::new(0.5, 0.80, 0.38),
                point(side + 1, true),
                point(side, true),
            ],
            palette.body,
        )?;
        builder.quad(
            [a, point(side, true), point(side + 1, true), b],
            palette.edge,
        )?;
    }
    Ok(())
}
