//! Layered natural and worked surfaces sampled once by the texture worker.
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
            // A lanceolate leaf has a broad shoulder and pointed ends, not a
            // cross. Coarse serrations remain legible after mip filtering.
            let half_width = leaf_width(dy);
            let inside = dx.abs() < half_width;
            if inside && result.is_none_or(|previous| seed > previous.seed) {
                result = Some(Chip {
                    seed,
                    edge: (half_width - dx.abs()).max(0.0),
                    x: dx,
                    y: dy,
                });
            }
        }
    }
    result
}

fn leaf_width(y: f32) -> f32 {
    let along = (y / 0.57).abs();
    if along >= 1.0 {
        return 0.0;
    }
    let serration = ((y * 36.0).floor().rem_euclid(2.0) - 0.5) * 0.014;
    (0.34 * (1.0 - along * along) + serration).max(0.0)
}

// Warped, jittered mineral patches have no shared horizontal mortar courses.
// Both warp and seed cells repeat exactly at the 64-unit texture boundary.
fn natural_rock(x: f32, y: f32) -> (u32, f32) {
    let tau = std::f32::consts::TAU;
    let x = x + (y * tau / 64.0).sin() * 2.1;
    let y = y + (x * tau / 64.0).cos() * 1.6;
    let (cx, cy) = ((x / 16.0).floor() as i32, (y / 16.0).floor() as i32);
    let mut nearest = f32::MAX;
    let mut second = f32::MAX;
    let mut selected = 0;
    let mut relief = 0.0;
    for row in cy - 1..=cy + 1 {
        for column in cx - 1..=cx + 1 {
            let seed = hash(column.rem_euclid(4) as u32 + 523, row.rem_euclid(4) as u32);
            let jitter = |shift: u32| 0.22 + ((seed >> shift) & 255) as f32 / 255.0 * 0.56;
            let dx = x / 16.0 - column as f32 - jitter(0);
            let dy = y / 16.0 - row as f32 - jitter(8);
            let distance = dx * dx + dy * dy;
            if distance < nearest {
                second = nearest;
                nearest = distance;
                selected = seed;
                relief = -dx * 0.035 - dy * 0.045;
            } else if distance < second {
                second = distance;
            }
        }
    }
    let edge = (second.sqrt() - nearest.sqrt()).max(0.0);
    let crack = (1.0 - edge / 0.035).clamp(0.0, 1.0) * 0.13;
    let rim = (1.0 - (edge - 0.035).abs() / 0.025).clamp(0.0, 1.0) * 0.045;
    (
        selected,
        0.91 + (selected % 9) as f32 * 0.009 + relief - crack + rim,
    )
}

fn blossom(x: f32, y: f32) -> Option<Chip> {
    let (cx, cy) = ((x / 8.0).floor() as i32, (y / 8.0).floor() as i32);
    let mut result: Option<Chip> = None;
    for row in cy - 1..=cy + 1 {
        for column in cx - 1..=cx + 1 {
            let seed = hash(column.rem_euclid(8) as u32 + 631, row.rem_euclid(8) as u32);
            let jitter = |shift: u32| (((seed >> shift) & 15) as f32 / 15.0 - 0.5) * 0.36;
            let dx = x / 8.0 - column as f32 - 0.5 - jitter(0);
            let dy = y / 8.0 - row as f32 - 0.5 - jitter(8);
            let radius = dx.hypot(dy);
            if radius > 0.53 {
                continue;
            }
            let angle = dy.atan2(dx) + (seed % 16) as f32 * 0.4;
            let rim = 0.39 + (angle * 5.0).cos() * 0.10;
            if radius < rim && result.is_none_or(|previous| seed > previous.seed) {
                result = Some(Chip {
                    seed,
                    edge: rim - radius,
                    x: dx,
                    y: dy,
                });
            }
        }
    }
    result
}

fn bark(pattern: TexturePattern, x: f32, y: f32, grain: f32) -> ([f32; 3], f32) {
    let tau = std::f32::consts::TAU;
    let warp = (y * tau / 64.0).sin() * 0.28 + (y * tau / 16.0).sin() * 0.07;
    let groove = (x / 5.333_333 + warp).rem_euclid(1.0);
    let lane = ((x / 5.333_333 + warp).floor() as i32).rem_euclid(12) as u32;
    let relief = (groove * std::f32::consts::PI).sin() * 0.105
        - (1.0 - groove / 0.11).clamp(0.0, 1.0) * 0.13;
    if pattern == TexturePattern::SpruceBark {
        let scales = hash(lane + 719, (y / 8.0) as u32) % 13;
        return (
            [111.0, 86.0, 60.0],
            0.83 + relief + scales as f32 * 0.007 + grain * 0.025,
        );
    }
    let row = (y / 8.0).floor() as u32;
    let offset = (hash(row, 719) % 64) as f32;
    let along = (x + offset).rem_euclid(64.0);
    let scar_y = (y + (along * tau / 64.0).sin() * 0.55).rem_euclid(8.0);
    let length = 7.0 + (hash(row, 757) % 20) as f32;
    let scar = along < length && scar_y < 0.65 + along / length * 0.65;
    let birch = pattern == TexturePattern::BirchBark;
    let color = if scar {
        if birch {
            [91.0, 89.0, 77.0]
        } else {
            [172.0, 166.0, 159.0]
        }
    } else if birch {
        [230.0, 228.0, 210.0]
    } else {
        // Cherry bark remains neutral: existing species tint supplies its color.
        [230.0, 225.0, 224.0]
    };
    (color, 0.93 + relief * 0.27 + grain * 0.018)
}

pub(super) fn sample(pattern: TexturePattern, px: u32, py: u32) -> Option<[f32; 3]> {
    let (px, py) = (px % SIZE, py % SIZE);
    let x = px as f32 / 8.0;
    let y = py as f32 / 8.0;
    let grain = (hash(px / 2, py / 2) % 19) as f32 / 18.0 - 0.5;
    let (base, shade) = match pattern {
        TexturePattern::Stone | TexturePattern::MossyStone => {
            let (seed, relief) = natural_rock(x, y);
            let moss = pattern == TexturePattern::MossyStone
                && chip(x, y, 547, 1.5).is_some_and(|p| p.edge > 0.12 && p.seed % 3 != 0);
            let base = if moss {
                [121.0, 137.0, 79.0]
            } else {
                [165.0, 160.0, 151.0]
            };
            let fleck = chip(x, y, 563, 0.29)
                .filter(|p| p.seed % 4 == 0)
                .map_or(0.0, |p| 0.06 - p.y * 0.04);
            (
                base,
                relief + fleck + (seed % 3) as f32 * 0.005 + grain * 0.022,
            )
        }
        TexturePattern::StructureStone => {
            let p = plate(x, y, 19);
            ([189.0, 179.0, 163.0], p.shade(2.0) + grain * 0.035)
        }
        TexturePattern::SpruceBark | TexturePattern::BirchBark | TexturePattern::CherryBark => {
            bark(pattern, x, y, grain)
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
            return Some(super::super::sand::color(px, py));
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
        TexturePattern::BlossomCanopy => {
            let flower = blossom(x, y);
            let heart = flower.is_some_and(|p| p.x.hypot(p.y) < 0.065);
            let base = if heart { [255.0; 3] } else { [242.0; 3] };
            (
                base,
                flower.map_or(0.83, |p| {
                    0.98 - p.y * 0.075 + (p.seed % 5) as f32 * 0.008
                        - (1.0 - p.edge / 0.045).clamp(0.0, 1.0) * 0.065
                }) + grain * 0.008,
            )
        }
        TexturePattern::Canopy | TexturePattern::BirchCanopy => {
            let leaf = leaf(x, y);
            let light_leaf =
                pattern == TexturePattern::BirchCanopy && leaf.is_some_and(|p| p.seed % 7 < 2);
            let base = if light_leaf {
                [249.0, 247.0, 220.0]
            } else {
                [222.0, 228.0, 198.0]
            };
            (
                base,
                leaf.map_or(0.81, |p| {
                    let midrib = (1.0 - p.x.abs() / 0.018).clamp(0.0, 1.0) * 0.065;
                    let branch = (p.y - p.x.abs() * 0.78).rem_euclid(0.16);
                    let vein = (1.0 - branch.min(0.16 - branch) / 0.012).clamp(0.0, 1.0) * 0.038;
                    let edge = (1.0 - p.edge / 0.035).clamp(0.0, 1.0) * 0.075;
                    0.93 - p.y * 0.065
                        + (p.seed % 7) as f32 * 0.009
                        + if p.x < 0.0 { 0.028 } else { -0.018 }
                        + midrib
                        + vein
                        - edge
                }) + grain * 0.009,
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
            TexturePattern::BlossomCanopy,
            TexturePattern::SpruceBark,
            TexturePattern::BirchBark,
            TexturePattern::CherryBark,
        ] {
            for y in [0, 13, 255, 511] {
                for x in [0, 77, 256, 511] {
                    assert_eq!(sample(pattern, x, y), sample(pattern, x + SIZE, y + SIZE));
                    let rgb = sample(pattern, x, y).unwrap();
                    assert!(
                        rgb.into_iter()
                            .all(|v| v.is_finite() && (0.0..=255.0).contains(&v))
                    );
                    if matches!(
                        pattern,
                        TexturePattern::Turf | TexturePattern::BlossomCanopy
                    ) {
                        assert_eq!(rgb[0], rgb[1]);
                        assert_eq!(rgb[1], rgb[2]);
                    }
                }
            }
        }
    }

    #[test]
    fn rock_patches_never_form_a_horizontal_mortar_course() {
        for row in 1..256 {
            let y = row as f32 * 0.25;
            let changed = (0..256)
                .filter(|&x| {
                    natural_rock(x as f32 * 0.25, y).0 != natural_rock(x as f32 * 0.25, y - 0.25).0
                })
                .count();
            assert!(changed < 64, "full-width rock seam at row {row}: {changed}");
        }
    }

    #[test]
    fn leaves_have_pointed_tips_and_blossoms_have_their_own_petals() {
        assert!(leaf_width(0.0) > leaf_width(0.54) * 5.0);
        assert_eq!(leaf_width(0.58), 0.0);
        let different = (0..64)
            .flat_map(|y| (0..64).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                sample(TexturePattern::Canopy, x * 8, y * 8)
                    != sample(TexturePattern::BlossomCanopy, x * 8, y * 8)
            })
            .count();
        assert!(different > 3000);
    }
}
