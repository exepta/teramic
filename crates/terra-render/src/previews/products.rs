use super::{sculpt::*, *};
use crate::blocks::ItemMaterials;
use terra_core::creatures::AnimalProduct as Product;

pub(super) fn batches(item: Product, material: ItemMaterials) -> Result<Vec<MeshBatch>, String> {
    let mut sculpt = Sculpt::new(32, 2);
    match item {
        Product::Wool => wool(&mut sculpt),
        Product::Leather => leather(&mut sculpt),
        Product::Honey => honey(&mut sculpt),
        Product::Fish | Product::CookedFish | Product::Clownfish | Product::CookedClownfish => {
            fish(&mut sculpt, item);
        }
        _ => meat(&mut sculpt, item),
    }
    let mut batches = vec![sculpt.mesh(material.opaque)?];
    if item == Product::Honey {
        batches.push(jar(material.glass)?);
    }
    Ok(batches)
}

fn meat(s: &mut Sculpt, item: Product) {
    let cooked = matches!(
        item,
        Product::CookedBeef | Product::CookedPork | Product::CookedMutton
    );
    let bone = matches!(item, Product::Mutton | Product::CookedMutton);
    let pork = matches!(item, Product::Pork | Product::CookedPork);
    let inside = |x: i16, z: i16| {
        let center = if bone { 14.0 } else { 15.5 };
        let dx = (f32::from(x) - center) / if bone { 10.0 } else { 11.5 };
        let dz = (f32::from(z) - if bone { 19.0 } else { 15.0 }) / if bone { 9.0 } else { 12.5 };
        let ellipse = dx * dx + dz * dz < 1.0;
        // A broad kidney cut for beef; pork keeps its oval fat rim.
        ellipse && (pork || bone || (x - 7).pow(2) + (z - 7).pow(2) >= 21)
    };
    for x in 3..29 {
        for z in 2..29 {
            if !inside(x, z) {
                continue;
            }
            let rim = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .into_iter()
                .any(|(dx, dz)| !inside(x + dx * 2, z + dz * 2));
            let marble = !cooked && !pork && ((x + z / 2 - 14).abs() <= 1 || x - z / 3 == 8);
            let edge = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .into_iter()
                .any(|(dx, dz)| !inside(x + dx, z + dz));
            let top = if edge {
                14
            } else if rim {
                16
            } else {
                17
            };
            let bottom = if edge { 13 } else { 11 };
            for y in bottom..=top {
                let crust = cooked && y == top && (x + z / 2).rem_euclid(6) <= 1;
                let color = if crust {
                    [0.28, 0.095, 0.035]
                } else if (rim && y >= top - 1) || marble {
                    if cooked {
                        [0.78, 0.40, 0.13]
                    } else {
                        [1.0, 0.76, 0.61]
                    }
                } else if cooked {
                    [0.69, 0.27, 0.085]
                } else if pork {
                    [0.96, 0.39, 0.36]
                } else {
                    [0.83, 0.16, 0.13]
                };
                let side = if y < 14 { 0.83 } else { 1.0 };
                s.cell(
                    [x, y, z],
                    shade(color.map(|v| v * side), [x / 2, y, z / 2], 0.08),
                );
            }
        }
    }
    if bone {
        for z in 3..14 {
            let x = 19 + (12 - z) / 4;
            s.cuboid([x, 12, z], [x + 2, 15, z], [0.95, 0.84, 0.65]);
        }
        s.cuboid([20, 11, 2], [25, 16, 4], [1.0, 0.91, 0.74]);
        s.cuboid([22, 12, 2], [23, 15, 4], [0.86, 0.70, 0.49]);
    }
}

fn leather(s: &mut Sculpt) {
    for x in 3i16..29 {
        for z in 3i16..29 {
            let dx = (x - 15).abs();
            let dz = (z - 15).abs();
            let center = dx <= 6 && dz <= 12;
            let legs = (7..=11).contains(&dz) && dx <= 11 + i16::from(dz == 8);
            if !center && !legs {
                continue;
            }
            let fold = (x + z / 3).rem_euclid(7) == 0;
            let height = 12 + (dx / 4) + i16::from(fold);
            for y in height..=height + 1 {
                let color = if fold {
                    [0.55, 0.23, 0.065]
                } else {
                    [0.75, 0.34, 0.11]
                };
                s.cell([x, y, z], shade(color, [x / 2, y, z / 2], 0.10));
            }
        }
    }
}

fn wool(s: &mut Sculpt) {
    for x in 6i16..26 {
        for y in 6i16..26 {
            for z in 6i16..26 {
                if (x - 15).abs() + (y - 15).abs() + (z - 15).abs() <= 25 {
                    s.cell([x, y, z], [0.93, 0.87, 0.74]);
                }
            }
        }
    }
    for x in 0i16..5 {
        for y in 0i16..5 {
            for z in 0i16..5 {
                if x != 0 && x != 4 && y != 0 && y != 4 && z != 0 && z != 4 {
                    continue;
                }
                if [x, y, z].iter().all(|v| *v == 0 || *v == 4) {
                    continue;
                }
                let p = [6 + x * 4, 6 + y * 4, 6 + z * 4];
                let variation = (x * 7 + y * 3 + z * 11) % 3;
                let min = std::array::from_fn(|i| p[i] - i16::from(p[i] == 6) * (1 + variation));
                let max =
                    std::array::from_fn(|i| p[i] + 3 + i16::from(p[i] == 22) * (1 + variation));
                s.cuboid(min, max, shade([0.99, 0.96, 0.86], [x, y, z], 0.04));
            }
        }
    }
}

fn fish(s: &mut Sculpt, item: Product) {
    let clown = matches!(item, Product::Clownfish | Product::CookedClownfish);
    let cooked = matches!(item, Product::CookedFish | Product::CookedClownfish);
    for x in 2i16..27 {
        for y in 8i16..25 {
            for z in 10i16..22 {
                let dx = (f32::from(x) - 13.0) / 11.0;
                let dy = (f32::from(y) - 16.0) / if clown { 7.0 } else { 6.0 };
                let dz = (f32::from(z) - 15.5) / 5.4;
                if dx * dx + dy * dy + dz * dz >= 1.0 {
                    continue;
                }
                let stripe = matches!(x, 7..=8 | 15..=16 | 23..=24);
                let border = matches!(x, 6 | 9 | 14 | 17 | 22 | 25);
                let color = if clown && stripe {
                    if cooked {
                        [0.88, 0.68, 0.38]
                    } else {
                        [0.97, 0.94, 0.85]
                    }
                } else if clown && border {
                    [0.27, 0.13, 0.07]
                } else if y < 14 {
                    if cooked {
                        [0.88, 0.62, 0.32]
                    } else {
                        [0.89, 0.82, 0.68]
                    }
                } else if cooked {
                    if (x + y / 2) % 7 <= 1 {
                        [0.42, 0.17, 0.045]
                    } else {
                        [0.77, 0.35, 0.09]
                    }
                } else if clown {
                    [0.96, 0.32, 0.035]
                } else if y > 18 {
                    [0.28, 0.40, 0.47]
                } else {
                    [0.49, 0.60, 0.65]
                };
                s.cell([x, y, z], shade(color, [x, y, z], 0.06));
            }
        }
    }
    let fin = if cooked {
        [0.53, 0.22, 0.055]
    } else if clown {
        [0.94, 0.30, 0.025]
    } else {
        [0.20, 0.38, 0.48]
    };
    for x in 25i16..30 {
        let width = x - 24;
        for y in 16 - width..=16 + width {
            s.cuboid([x, y, 15], [x, y, 17], fin);
        }
    }
    for x in 10i16..21 {
        let height = if x < 14 { 25 } else { 24 };
        s.cuboid([x, 21, 15], [x, height, 16], fin);
    }
    for z in [9, 20] {
        s.cuboid([12, 9, z], [16, 12, z + 2], fin);
    }
    for z in [11, 20] {
        s.cuboid([5, 16, z], [6, 18, z], [0.025, 0.022, 0.018]);
        s.cell([5, 18, z], [0.98, 0.96, 0.89]);
    }
}

fn honey(s: &mut Sculpt) {
    for y in 6i16..29 {
        let radius = match y {
            6..=7 => 8.0,
            8..=18 => 9.5,
            19..=20 => 8.5,
            21..=24 => 6.0,
            _ => 7.5,
        };
        for x in 5i16..27 {
            for z in 5i16..27 {
                let dx = (f32::from(x) - 15.5).abs();
                let dz = (f32::from(z) - 15.5).abs();
                if dx.max(dz) > radius || dx + dz > radius * 1.42 {
                    continue;
                }
                let color = match y {
                    6..=8 => [1.0, 0.76, 0.12],
                    9..=18 => [0.98, 0.58, 0.018],
                    19..=22 => [1.0, 0.70, 0.06],
                    23..=24 => [0.62, 0.42, 0.21],
                    _ => [0.76, 0.48, 0.21],
                };
                s.cell([x, y, z], shade(color, [x / 2, y / 2, z / 2], 0.045));
            }
        }
    }
    s.cuboid([10, 16, 7], [11, 22, 8], [1.0, 0.75, 0.09]);
    s.cuboid([6, 9, 11], [6, 17, 12], [1.0, 0.90, 0.53]);
    s.cuboid([22, 20, 10], [23, 24, 11], [0.77, 0.51, 0.20]);
    s.cuboid([24, 18, 11], [25, 21, 12], [0.70, 0.43, 0.17]);
}

fn jar(material: usize) -> Result<MeshBatch, String> {
    let mut mesh = MeshBatch {
        material,
        ..default()
    };
    // One faceted glass shell with a narrower shoulder; alpha blending uses the
    // existing material pass and does not add refraction/fullscreen passes.
    let rings = [
        (0.16, 0.28),
        (0.22, 0.34),
        (0.57, 0.34),
        (0.70, 0.225),
        (0.77, 0.225),
    ];
    for ring in rings.windows(2) {
        for i in 0..8 {
            let a = (i as f32 + 0.5) * std::f32::consts::FRAC_PI_4;
            let b = a + std::f32::consts::FRAC_PI_4;
            let point = |angle: f32, level: (f32, f32)| {
                Vec3::new(
                    0.5 + angle.cos() * level.1,
                    level.0,
                    0.5 + angle.sin() * level.1,
                )
            };
            let points = [
                point(a, ring[0]),
                point(a, ring[1]),
                point(b, ring[1]),
                point(b, ring[0]),
            ];
            let normal = (points[1] - points[0])
                .cross(points[2] - points[0])
                .normalize();
            quad(&mut mesh, points, normal, [1.0, 0.97, 0.85])?;
        }
    }
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_products_have_bounded_indexed_solids_and_at_most_one_glass_batch() {
        for item in Product::ALL {
            let batches = batches(
                item,
                ItemMaterials {
                    opaque: 0,
                    glass: 1,
                    ..default()
                },
            )
            .unwrap();
            assert_eq!(batches.len(), if item == Product::Honey { 2 } else { 1 });
            let vertices: usize = batches.iter().map(|b| b.positions.len()).sum();
            assert!(
                vertices > 0 && vertices <= 24_000,
                "{item:?}: {vertices} vertices"
            );
            for b in batches {
                assert_eq!(b.positions.len(), b.colors.len());
                assert_eq!(b.positions.len(), b.normals.len());
                assert!(b.indices.iter().all(|&i| (i as usize) < b.positions.len()));
                assert!(
                    b.positions
                        .iter()
                        .flatten()
                        .all(|v| v.is_finite() && (-0.01..=1.01).contains(v))
                );
            }
        }
    }
}
