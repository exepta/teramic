//! Layered, handworked surfaces sampled once when the shared library is loaded.
//! All relief stays in the existing closed cube mesh; no extra streaming geometry.
use super::{TexturePattern, hash};

const SIZE: u32 = 512;

#[derive(Clone, Copy)]
struct Plate {
    seed: u32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl Plate {
    fn edge(self) -> f32 {
        self.left.min(self.top).min(self.right).min(self.bottom)
    }

    fn shade(self, bevel: f32) -> f32 {
        let edge = self.edge();
        let crease = 0.67 + (edge / 0.7).clamp(0.0, 1.0) * 0.33;
        let lit = (1.0 - self.left.min(self.top) / bevel).clamp(0.0, 1.0) * 0.12;
        let shaded = (1.0 - self.right.min(self.bottom) / bevel).clamp(0.0, 1.0) * 0.10;
        crease * (0.92 + (self.seed % 13) as f32 * 0.008 + lit - shaded)
    }
}

fn interval(value: f32, edges: [f32; 5]) -> (u32, f32, f32) {
    let index = edges[1..]
        .iter()
        .position(|&edge| value < edge)
        .unwrap_or(3);
    (index as u32, value - edges[index], edges[index + 1] - value)
}

fn plate(x: f32, y: f32, salt: u32) -> Plate {
    let (row, local_y, far_y) = interval(y, [0.0, 13.0, 33.0, 48.0, 64.0]);
    let offset = (hash(row, salt) % 4) as f32 * 4.0;
    let xx = (x + offset).rem_euclid(64.0);
    let (column, local_x, far_x) = interval(xx, [0.0, 18.0, 38.0, 49.0, 64.0]);
    let seed = hash(column + salt, row);
    // Small square chips interrupt a panel edge, never the physical cube shell.
    let chip_x = if hash(column, (y / 2.0) as u32) % 13 < 2 {
        0.7
    } else {
        0.0
    };
    let chip_y = if hash((xx / 2.0) as u32, row) % 17 < 2 {
        0.8
    } else {
        0.0
    };
    Plate {
        seed,
        left: (local_x - chip_x).max(0.0),
        right: (far_x - chip_x).max(0.0),
        top: (local_y - chip_y).max(0.0),
        bottom: (far_y - chip_y).max(0.0),
    }
}

#[derive(Clone, Copy)]
struct Chip {
    seed: u32,
    edge: f32,
    x: f32,
    y: f32,
}

// Jittered, differently sized octagonal chips keep soil and gravel from reading
// as masonry. Neighbor cells wrap, including when a chip crosses a texture edge.
fn chip(x: f32, y: f32, salt: u32, scale: f32) -> Option<Chip> {
    let cell_x = (x / 8.0).floor() as i32;
    let cell_y = (y / 8.0).floor() as i32;
    let mut nearest = 1.0;
    let mut result = None;
    for cy in cell_y - 1..=cell_y + 1 {
        for cx in cell_x - 1..=cell_x + 1 {
            let seed = hash(cx.rem_euclid(8) as u32 + salt, cy.rem_euclid(8) as u32);
            let jitter = |shift: u32| (((seed >> shift) & 15_u32) as f32 / 15.0 - 0.5) * 0.45;
            let size = |shift: u32| scale * (0.3 + ((seed >> shift) & 7_u32) as f32 / 7.0 * 0.23);
            let dx = (x / 8.0 - cx as f32 - 0.5 - jitter(8)) / size(16);
            let dy = (y / 8.0 - cy as f32 - 0.5 - jitter(12)) / size(20);
            let radius = dx.abs().max(dy.abs()).max((dx.abs() + dy.abs()) * 0.67);
            if radius < nearest {
                nearest = radius;
                result = Some(Chip {
                    seed,
                    edge: 1.0 - radius,
                    x: dx,
                    y: dy,
                });
            }
        }
    }
    result
}

fn leaf(x: f32, y: f32) -> Option<Chip> {
    let cell_x = (x / 8.0).floor() as i32;
    let cell_y = (y / 8.0).floor() as i32;
    let mut result: Option<Chip> = None;
    for cy in cell_y - 1..=cell_y + 1 {
        for cx in cell_x - 1..=cell_x + 1 {
            let seed = hash(cx.rem_euclid(8) as u32 + 179, cy.rem_euclid(8) as u32);
            let jitter = |shift: u32| (((seed >> shift) & 15_u32) as f32 / 15.0 - 0.5) * 0.4;
            let size = 0.8 + ((seed >> 20) & 7) as f32 / 7.0 * 0.5;
            let mut dx = (x / 8.0 - cx as f32 - 0.5 - jitter(8)) / size;
            let mut dy = (y / 8.0 - cy as f32 - 0.5 - jitter(12)) / size;
            if seed & 1 == 0 {
                std::mem::swap(&mut dx, &mut dy);
            }
            if seed & 2 == 0 {
                (dx, dy) = (
                    (dx + dy) * std::f32::consts::FRAC_1_SQRT_2,
                    (dy - dx) * std::f32::consts::FRAC_1_SQRT_2,
                );
            }
            let (a, b) = (dx.abs(), dy.abs());
            let inside = (a < 0.22 && b < 0.47)
                || (a < 0.43 && b < 0.17)
                || (a < 0.34 && dy > -0.30 && dy < 0.04);
            if inside && result.is_none_or(|previous| seed > previous.seed) {
                result = Some(Chip {
                    seed,
                    edge: 0.0,
                    x: dx,
                    y: dy,
                });
            }
        }
    }
    result
}

pub(super) fn sample(pattern: TexturePattern, px: u32, py: u32) -> Option<[f32; 3]> {
    let (px, py) = (px % SIZE, py % SIZE);
    let x = px as f32 / 8.0;
    let y = py as f32 / 8.0;
    let grain = (hash(px / 2, py / 2) % 19) as f32 / 18.0 - 0.5;
    let (base, shade) = match pattern {
        TexturePattern::Stone | TexturePattern::MossyStone | TexturePattern::StructureStone => {
            let p = plate(x, y, 19);
            let vein_x = 5.0 + ((y / 3.0) as u32 % 3) as f32 * 0.65;
            let vein = p.seed.is_multiple_of(3) && (p.left - vein_x).abs() < 0.23 && p.top > 5.0;
            let moss = pattern == TexturePattern::MossyStone
                && (hash((x / 8.0) as u32, (y / 8.0) as u32) % 7 < 2);
            let color = if moss {
                [114.0, 130.0, 73.0]
            } else if pattern == TexturePattern::StructureStone {
                [189.0, 179.0, 163.0]
            } else {
                [159.0, 153.0, 144.0]
            };
            (
                color,
                p.shade(2.0) + grain * 0.035 - if vein { 0.19 } else { 0.0 },
            )
        }
        TexturePattern::Earth => {
            if let Some(p) = chip(x, y, 57, 0.55).filter(|p| p.seed % 5 == 0) {
                (
                    [137.0, 126.0, 105.0],
                    0.91 - p.y * 0.09 - p.x * 0.04 + grain * 0.04,
                )
            } else {
                let relief =
                    chip(x, y, 73, 1.1).map_or(-0.035, |p| p.edge.min(0.25) * 0.18 - p.y * 0.03);
                ([161.0, 108.0, 68.0], 0.93 + relief + grain * 0.10)
            }
        }
        TexturePattern::Farmland | TexturePattern::FarmlandSide => {
            // Large quiet clods and fine warm chips remain legible through mips;
            // rootlets are sparse and only appear on the vertical soil texture.
            let cell_x = px / 24;
            let cell_y = py / 20;
            let seed = hash(cell_x, cell_y);
            let edge_x = px % 24;
            let edge_y = py % 20;
            let seam = edge_x < 2 || edge_y < 2;
            let pebble =
                seed.is_multiple_of(17) && edge_x > 6 && edge_x < 18 && edge_y > 5 && edge_y < 15;
            let root_lane = (cell_y / 2 + seed % 3) % 4;
            let root = pattern == TexturePattern::FarmlandSide
                && hash(cell_x / 2, cell_y / 4).is_multiple_of(11)
                && ((edge_x < 4 && edge_y < 15) || (edge_y > 9 && edge_y < 13 && edge_x < 12))
                && root_lane < 3;
            let base = if root {
                if seed.is_multiple_of(3) {
                    [106.0, 118.0, 40.0]
                } else {
                    [135.0, 112.0, 56.0]
                }
            } else if pebble {
                [126.0, 117.0, 104.0]
            } else {
                match seed % 5 {
                    0 => [123.0, 76.0, 42.0],
                    1 => [153.0, 95.0, 48.0],
                    2 => [112.0, 68.0, 42.0],
                    3 => [145.0, 88.0, 46.0],
                    _ => [133.0, 81.0, 43.0],
                }
            };
            let highlight = if edge_y > 2 && edge_y < 5 { 0.13 } else { 0.0 };
            (
                base,
                0.91 + highlight + grain * 0.055 - if seam { 0.21 } else { 0.0 },
            )
        }
        TexturePattern::Sand => {
            let pore = hash((x * 2.0) as u32, (y * 2.0) as u32).is_multiple_of(43);
            (
                [230.0, 204.0, 148.0],
                0.98 + grain * 0.09 - if pore { 0.09 } else { 0.0 },
            )
        }
        TexturePattern::Clay => {
            let p = plate(x, y, 113);
            ([183.0, 103.0, 64.0], p.shade(1.3) + grain * 0.025)
        }
        TexturePattern::Gravel => {
            if let Some(p) = chip(x, y, 137, 1.0) {
                let tint = (p.seed % 7) as f32 - 3.0;
                (
                    [169.0 + tint * 4.0, 157.0 + tint * 3.0, 140.0 + tint * 2.0],
                    0.93 - p.y * 0.12 - p.x * 0.06 - (1.0 - p.edge / 0.16).clamp(0.0, 1.0) * 0.08
                        + grain * 0.045,
                )
            } else {
                ([144.0, 131.0, 116.0], 0.94 + grain * 0.12)
            }
        }
        TexturePattern::Turf => {
            let clump =
                chip(x, y, 151, 1.2).map_or(-0.025, |p| p.edge.min(0.25) * 0.12 - p.y * 0.02);
            let stalk = hash((x / 2.0) as u32, (y / 4.0) as u32) % 11 < 2
                && x.rem_euclid(2.0) < 0.45
                && y.rem_euclid(4.0) < 2.5;
            // Neutral art is tinted by the biome and weather vertex colors.
            (
                [227.0; 3],
                0.94 + clump + grain * 0.035 - if stalk { 0.06 } else { 0.0 },
            )
        }
        TexturePattern::Canopy | TexturePattern::BirchCanopy | TexturePattern::BlossomCanopy => {
            let birch = pattern == TexturePattern::BirchCanopy;
            let leaf = leaf(x, y);
            let light_leaf = birch && leaf.is_some_and(|p| p.seed % 7 < 2);
            let base = if pattern == TexturePattern::BlossomCanopy {
                [238.0; 3]
            } else if light_leaf {
                [249.0, 247.0, 220.0]
            } else {
                [222.0, 228.0, 198.0]
            };
            (
                base,
                leaf.map_or(0.76, |p| {
                    0.92 - p.y * 0.10 + (p.seed % 9) as f32 * 0.012
                        - if p.x.abs() < 0.025 && p.y.abs() < 0.38 {
                            0.035
                        } else {
                            0.0
                        }
                }) + grain * 0.012,
            )
        }
        _ => return None,
    };
    Some(base.map(|v| (v * shade).clamp(0.0, 255.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_patterns_repeat_and_keep_turf_neutral() {
        for pattern in [
            TexturePattern::Stone,
            TexturePattern::Earth,
            TexturePattern::Turf,
            TexturePattern::Sand,
            TexturePattern::Clay,
            TexturePattern::Gravel,
            TexturePattern::StructureStone,
            TexturePattern::Canopy,
            TexturePattern::BirchCanopy,
        ] {
            for y in [0, 13, 255, 511] {
                for x in [0, 77, 256, 511] {
                    assert_eq!(sample(pattern, x, y), sample(pattern, x + SIZE, y + SIZE));
                    let rgb = sample(pattern, x, y).unwrap();
                    assert!(
                        rgb.into_iter()
                            .all(|v| v.is_finite() && (0.0..=255.0).contains(&v))
                    );
                    if pattern == TexturePattern::Turf {
                        assert_eq!(rgb[0], rgb[1]);
                        assert_eq!(rgb[1], rgb[2]);
                    }
                }
            }
        }
    }
}
