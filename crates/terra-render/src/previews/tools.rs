use super::{sculpt::*, *};
use terra_core::ToolKind;

pub(super) fn batches(item: ItemKind, material: usize) -> Result<Vec<MeshBatch>, String> {
    let mut s = Sculpt::new(32, 2);
    let wood = [0.59, 0.27, 0.085];
    let grain = [0.38, 0.14, 0.038];
    let binding = [0.76, 0.60, 0.37];
    let wooden = matches!(
        item,
        ItemKind::Tool(
            ToolKind::WoodPickaxe | ToolKind::WoodAxe | ToolKind::WoodShovel | ToolKind::WoodSword
        )
    );
    let iron = item == ItemKind::Tool(ToolKind::IronPickaxe);
    let head = if wooden {
        [0.73, 0.37, 0.13]
    } else if iron {
        [0.61, 0.65, 0.66]
    } else {
        [0.49, 0.48, 0.52]
    };
    let edge = if wooden {
        [0.92, 0.54, 0.23]
    } else {
        [0.71, 0.69, 0.70]
    };
    let sword = matches!(
        item,
        ItemKind::Tool(ToolKind::WoodSword | ToolKind::StoneSword)
    );
    let length = if sword { 12 } else { 27 };
    for y in 2i16..=length {
        s.cuboid([14, y, 14], [17, y, 17], wood);
        s.cell([14, y, 17], grain);
        s.cell([17, y, 14], grain);
        if y % 5 > 1 {
            s.cell([16, y, 18], [0.66, 0.32, 0.10]);
        }
    }
    if item == ItemKind::Stick {
        return Ok(vec![s.mesh(material)?]);
    }
    match item {
        ItemKind::Tool(ToolKind::WoodPickaxe | ToolKind::StonePickaxe | ToolKind::IronPickaxe) => {
            for x in 2i16..30 {
                let dx = f32::from(x - 15) / 13.0;
                let top = 28 - (dx * dx * 6.0) as i16;
                let depth = if dx.abs() > 0.80 { 2 } else { 4 };
                for y in top - depth..=top {
                    let color = if y == top { edge } else { head };
                    s.cuboid([x, y, 14], [x, y, 18], color);
                }
            }
            if wooden || iron {
                plate(&mut s, [13, 24, 19], [18, 28, 19]);
            } else {
                // Crossed rawhide straps grip the curved stone head.
                for x in 10i16..22 {
                    let y = 22 + (x - 10) / 2;
                    s.cuboid([x, y, 19], [x, y + 1, 20], binding);
                }
            }
        }
        ItemKind::Tool(ToolKind::WoodAxe | ToolKind::StoneAxe) => {
            for x in 5i16..27 {
                let d = (x - 15).abs();
                let width = if wooden && x > 18 { 2 } else { 3 + d / 3 };
                for y in 25 - width..=25 + width {
                    let color = if x <= 6 || (!wooden && x >= 25) {
                        edge
                    } else {
                        head
                    };
                    s.cuboid([x, y, 14], [x, y, 18], color);
                }
            }
            if wooden {
                plate(&mut s, [17, 21, 19], [20, 28, 19]);
            } else {
                for i in 0i16..10 {
                    let x = 11 + i;
                    s.cuboid([x, 21 + i, 19], [x, 22 + i, 20], binding);
                    s.cuboid([x, 30 - i, 19], [x, 31 - i, 20], binding);
                }
            }
        }
        ItemKind::Tool(ToolKind::WoodShovel | ToolKind::StoneShovel) => {
            for y in 19i16..31 {
                let width = if y == 19 || y == 30 {
                    3
                } else if y == 29 {
                    4
                } else {
                    5
                };
                for x in 15 - width..=16 + width {
                    let color = if x == 15 - width || x == 16 + width || y == 30 {
                        edge
                    } else {
                        head
                    };
                    let bulge = if (13..=18).contains(&x) && (21..=28).contains(&y) {
                        20
                    } else {
                        19
                    };
                    s.cuboid([x, y, 15], [x, y, bulge], color);
                }
            }
            wrap(&mut s, 18, binding);
        }
        ItemKind::Tool(ToolKind::WoodSword | ToolKind::StoneSword) => {
            for y in 14i16..31 {
                let half = if y >= 29 { 30 - y } else { 3 };
                for x in 15 - half..=16 + half {
                    let color = if x == 15 - half || x == 16 + half {
                        edge
                    } else {
                        head
                    };
                    s.cuboid(
                        [x, y, 15],
                        [x, y, if (15..=16).contains(&x) { 19 } else { 18 }],
                        color,
                    );
                }
            }
            s.cuboid([9, 11, 14], [22, 13, 19], if wooden { wood } else { head });
            s.cuboid([8, 12, 14], [10, 14, 19], edge);
            s.cuboid([21, 10, 14], [23, 12, 19], edge);
            plate(&mut s, [13, 11, 20], [18, 13, 20]);
            s.cuboid([13, 1, 13], [18, 3, 18], if wooden { wood } else { head });
            wrap(&mut s, 7, binding);
        }
        _ => return Err("Not a tool or stick".into()),
    }
    wrap(&mut s, 4, binding);
    if !sword {
        wrap(&mut s, 17, binding);
    }
    let mut mesh = s.mesh(material)?;
    // Keep the grip centred while matching the slight handmade cant of the
    // reference. Held-item attachment continues to use the same +Y tool axis.
    let rotation = Quat::from_rotation_z(-0.10);
    for p in &mut mesh.positions {
        *p = (Vec3::splat(0.5) + rotation * (Vec3::from_array(*p) - Vec3::splat(0.5))).to_array();
    }
    for n in &mut mesh.normals {
        *n = (rotation * Vec3::from_array(*n)).to_array();
    }
    // The broad axe corners move outside the design cube when canted. Fit the
    // finished silhouette uniformly about the grip centre, not by clipping it.
    let radius = mesh
        .positions
        .iter()
        .map(|p| {
            (Vec3::from_array(*p) - Vec3::splat(0.5))
                .abs()
                .max_element()
        })
        .fold(0.5_f32, f32::max);
    if radius > 0.5 {
        let scale = 0.5 / radius;
        for p in &mut mesh.positions {
            *p = (Vec3::splat(0.5) + (Vec3::from_array(*p) - Vec3::splat(0.5)) * scale).to_array();
        }
    }
    Ok(vec![mesh])
}

fn wrap(s: &mut Sculpt, y: i16, color: [f32; 3]) {
    for x in 13i16..=18 {
        for z in 13i16..=19 {
            if (14..=17).contains(&x) && (14..=18).contains(&z) {
                continue;
            }
            s.cuboid([x, y, z], [x, y + 1, z], color);
        }
    }
    s.cuboid([15, y - 1, 20], [17, y + 1, 20], [0.58, 0.41, 0.22]);
}

fn plate(s: &mut Sculpt, min: [i16; 3], max: [i16; 3]) {
    s.cuboid(min, max, [0.32, 0.30, 0.31]);
    s.cell([min[0] + 1, min[1] + 1, max[2] + 1], [0.72, 0.66, 0.52]);
    s.cell([max[0] - 1, max[1] - 1, max[2] + 1], [0.72, 0.66, 0.52]);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_tools_share_one_material_and_remain_valid_three_dimensional_solids() {
        for item in ItemKind::EXTRA
            .into_iter()
            .filter(|i| matches!(i, ItemKind::Tool(_) | ItemKind::Stick))
        {
            let batches = batches(item, 42).unwrap();
            assert_eq!(batches.len(), 1);
            let mesh = &batches[0];
            assert_eq!(mesh.material, 42);
            assert!(
                mesh.positions.len() <= 12_000,
                "{item:?}: {}",
                mesh.positions.len()
            );
            assert_eq!(mesh.positions.len(), mesh.colors.len());
            let minimum = mesh
                .positions
                .iter()
                .fold(Vec3::splat(f32::INFINITY), |a, p| {
                    a.min(Vec3::from_array(*p))
                });
            let maximum = mesh
                .positions
                .iter()
                .fold(Vec3::splat(f32::NEG_INFINITY), |a, p| {
                    a.max(Vec3::from_array(*p))
                });
            assert!(
                mesh.positions
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite() && (-1e-6..=1.0 + 1e-6).contains(v)),
                "{item:?}: out-of-unit geometry; min={minimum:?}, max={maximum:?}"
            );
            assert!(
                mesh.indices
                    .iter()
                    .all(|&i| (i as usize) < mesh.positions.len())
            );
        }
    }
}
