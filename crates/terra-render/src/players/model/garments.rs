//! Bounded garment surfaces shared by the editor, portrait and world avatars.
use super::*;

/// Small continuous relief changes surface normals without adding mesh vertices.
pub(super) fn relief(material: ClothingMaterial, point: Vec3) -> f32 {
    let across = point.x + point.z;
    match material {
        ClothingMaterial::Linen => (across * 180.0).sin() * (point.y * 200.0).sin() * 0.0008,
        ClothingMaterial::Canvas => (across * 100.0).sin() * (point.y * 130.0).sin() * 0.0015,
        ClothingMaterial::Leather => (across * 67.0 + point.y * 79.0).sin() * 0.00035,
        ClothingMaterial::Wool => ((across * 140.0).sin() + (point.y * 160.0).sin()) * 0.0018,
    }
}

pub(super) fn shade(material: ClothingMaterial, column: u32, row: u32, hash: u32) -> f32 {
    match material {
        ClothingMaterial::Linen => 0.965 + ((column + row) % 2) as f32 * 0.065,
        ClothingMaterial::Canvas => 0.94 + ((column / 2 + row / 2) % 2) as f32 * 0.10,
        ClothingMaterial::Leather => 0.985 + (hash & 3) as f32 * 0.008,
        ClothingMaterial::Wool => 0.92 + (row % 3) as f32 * 0.065,
    }
}

/// A single closed flared cape, with continuous folds rather than separate planks.
pub(super) fn cape(builder: &mut Builder, appearance: CharacterAppearance) -> Result<(), String> {
    const COLUMNS: u32 = 24;
    const ROWS: u32 = 24;
    let base = appearance.cape_color.map(|v| f32::from(v) / 255.0);
    let thickness = if appearance.cape_material == ClothingMaterial::Wool {
        0.025
    } else {
        0.014
    };
    let point = |column: u32, row: u32, back: bool| {
        let across = column as f32 / COLUMNS as f32 * 2.0 - 1.0;
        let down = row as f32 / ROWS as f32;
        let width = 0.45 + 0.23 * down;
        let fold = (across * std::f32::consts::PI * 4.0).cos() * (0.008 + 0.013 * down);
        let mut point = Vec3::new(
            across * width * 0.5,
            0.29 - 1.04 * down,
            0.19 + 0.10 * down
                + fold
                + if back {
                    thickness * 0.5
                } else {
                    -thickness * 0.5
                },
        );
        point.z += relief(appearance.cape_material, point);
        point
    };
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let trim = row == ROWS - 1 || column == 0 || column == COLUMNS - 1;
            let color = if trim {
                [0.78, 0.55, 0.23]
            } else {
                base.map(|channel| {
                    (channel
                        * shade(
                            appearance.cape_material,
                            column,
                            row,
                            noise(row * 73 + column * 17),
                        ))
                    .min(1.0)
                })
            };
            for back in [false, true] {
                let corners = [
                    point(column, row, back),
                    point(column, row + 1, back),
                    point(column + 1, row + 1, back),
                    point(column + 1, row, back),
                ];
                builder.quad(
                    if back {
                        corners
                    } else {
                        [corners[3], corners[2], corners[1], corners[0]]
                    },
                    color,
                )?;
            }
        }
        for column in [0, COLUMNS] {
            let corners = [
                point(column, row, false),
                point(column, row, true),
                point(column, row + 1, true),
                point(column, row + 1, false),
            ];
            builder.quad(
                if column == 0 {
                    [corners[3], corners[2], corners[1], corners[0]]
                } else {
                    corners
                },
                base,
            )?;
        }
    }
    for column in 0..COLUMNS {
        for row in [0, ROWS] {
            let corners = [
                point(column, row, false),
                point(column + 1, row, false),
                point(column + 1, row, true),
                point(column, row, true),
            ];
            builder.quad(
                if row == 0 {
                    [corners[3], corners[2], corners[1], corners[0]]
                } else {
                    corners
                },
                base,
            )?;
        }
    }
    // The yoke, shoulder mantle and clasp physically meet the cape's top edge.
    builder.bevel_box(
        [0.0, 0.285, 0.185],
        [0.47, 0.075, 0.06],
        base,
        Quat::IDENTITY,
    )?;
    for side in [-1.0, 1.0] {
        builder.bevel_box(
            [side * 0.185, 0.31, 0.005],
            [0.17, 0.10, 0.37],
            base,
            Quat::IDENTITY,
        )?;
        builder.rotated(
            [side * 0.09, 0.285, -0.18],
            [0.18, 0.035, 0.025],
            base,
            Quat::from_rotation_z(side * 0.13),
        )?;
    }
    builder.bevel_box(
        [0.0, 0.285, -0.20],
        [0.065, 0.08, 0.022],
        [0.85, 0.60, 0.19],
        Quat::IDENTITY,
    )?;
    Ok(())
}
