//! Material relief attached to the outside of a complete one-metre cell.
//! Each face template contains only the additional details; negative local
//! depth denotes height above its solid supporting face.
use super::patch::FacePatch;
use crate::blocks::{PrimitiveTemplate, definitions::CubeSurface};
use bevy::math::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum NaturalForm {
    Turf,
    ReferenceTurf,
    Earth,
    Farmland,
    Rock,
    ReferenceRock,
    Gravel,
    Asphalt,
    Petals,
    Vent,
    Sand,
    Snow,
    Ice,
    Log,
    Leaves,
    Mushroom,
    Gills,
    Coral,
    Cactus,
}

pub(super) fn classify(name: &str, surface: Option<CubeSurface>) -> Option<NaturalForm> {
    use NaturalForm::*;
    let name = name.to_ascii_lowercase().replace(['_', '-'], " ");
    let has = |words: &[&str]| {
        words
            .iter()
            .any(|word| name.split_whitespace().any(|n| n == *word))
    };
    if name == "blossom petal block" {
        return Some(Petals);
    }
    if has(&["fumarole", "vent"]) {
        return Some(Vent);
    }
    if name == "asphalt" {
        return Some(Asphalt);
    }
    if has(&["cactus"]) {
        return Some(Cactus);
    }
    if has(&["gills"]) {
        return Some(Gills);
    }
    if has(&["fungal", "mushroom"]) {
        return Some(if has(&["stem"]) { Gills } else { Mushroom });
    }
    if has(&["coral"]) {
        return Some(Coral);
    }
    if has(&["foliage", "leaves", "blossoms", "boughs", "branches"]) {
        return Some(Leaves);
    }
    if has(&["log", "bark", "sapwood", "heartwood"]) || name == "weathered wood" {
        return Some(Log);
    }
    if has(&["gravel", "schotter", "shingle"]) {
        return Some(Gravel);
    }
    if has(&["ice"]) || name == "solid water" {
        return Some(Ice);
    }
    if has(&["sand"]) {
        return Some(Sand);
    }
    if has(&["firn", "snow"]) && !has(&["stone", "sod"]) {
        return Some(Snow);
    }
    if has(&["grass", "turf", "sod"]) || name == "cave moss" || name == "glow moss" {
        return Some(Turf);
    }
    if has(&[
        "dirt", "loam", "peat", "soil", "mud", "clay", "ash", "litter", "trail", "ground",
    ]) {
        return Some(Earth);
    }
    if has(&[
        "stone",
        "sandstone",
        "limestone",
        "granite",
        "basalt",
        "rock",
        "obsidian",
    ]) {
        return Some(Rock);
    }
    Some(match surface? {
        CubeSurface::Turf => Turf,
        CubeSurface::Earth => Earth,
        CubeSurface::Farmland => Farmland,
        CubeSurface::Rock => Rock,
        CubeSurface::Sand => Sand,
        CubeSurface::Snow => Snow,
        CubeSurface::Mushroom => Mushroom,
        CubeSurface::CoralRed
        | CubeSurface::CoralBlue
        | CubeSurface::CoralYellow
        | CubeSurface::CoralPurple => Coral,
        _ => return None,
    })
}

pub(super) fn face(form: NaturalForm, side: usize, variant: usize) -> Vec<PrimitiveTemplate> {
    use NaturalForm::*;
    let mut mesh = FacePatch::new(side, variant);
    match form {
        Turf | ReferenceTurf if side == 3 => crust::<6, 5>(&mut mesh, form),
        ReferenceTurf => crust::<12, 10>(&mut mesh, form),
        Earth | Turf => crust::<8, 6>(&mut mesh, form),
        Farmland if side == 3 => ridges(&mut mesh, Ridge::Furrows),
        Farmland => crust::<8, 6>(&mut mesh, Earth),
        ReferenceRock => reference_rock(&mut mesh),
        Rock | Ice => masonry(
            &mut mesh,
            if form == Ice {
                Relief::Ice
            } else {
                Relief::Rock
            },
        ),
        Gravel => pebbles(&mut mesh),
        Asphalt => road_surface(&mut mesh),
        Sand if side == 3 => ridges(&mut mesh, Ridge::Dunes),
        Sand => packed_sand(&mut mesh),
        Snow => ridges(&mut mesh, Ridge::Drifts),
        Log if side / 2 == 1 => end_grain(&mut mesh),
        Log => masonry(&mut mesh, Relief::Bark),
        Leaves => leaf_clusters(&mut mesh),
        Petals => leaf_clusters(&mut mesh),
        Vent if side == 3 => vent(&mut mesh),
        Vent => masonry(&mut mesh, Relief::Rock),
        Coral => coral_cups(&mut mesh),
        Mushroom if side != 2 => scales(&mut mesh),
        Gills | Mushroom | Cactus if side / 2 == 1 => radiating_gills(&mut mesh),
        Gills | Cactus | Mushroom => ridges(&mut mesh, Ridge::Ribs),
    }
    mesh.meshes
}

type Point = [f32; 3];

impl FacePatch {
    fn noise(&self, key: usize) -> f32 {
        let mut h = (self.variant as u32 + 1).wrapping_mul(0x9e3779b9)
            ^ (self.side as u32 + 1).wrapping_mul(0x85ebca77)
            ^ (key as u32).wrapping_mul(0xc2b2ae3d);
        h ^= h >> 16;
        h = h.wrapping_mul(0x7feb352d);
        h ^= h >> 15;
        (h & 0xffff) as f32 / 65535.0
    }

    fn surface_vertex_count(&self) -> usize {
        self.meshes
            .iter()
            .find(|p| p.material == self.side)
            .map_or(0, |p| p.uvs.len())
    }

    // Each small solid samples a patch of the original material. Projecting
    // the whole stone pile or canopy over one stone/leaf would paint extra
    // outlines across its cap. Uniform UV scale preserves the tangent frame.
    fn local_texture(&mut self, start: usize, origin: Vec2, sample: [f32; 2], scale: f32) {
        let piece = self
            .meshes
            .iter_mut()
            .find(|p| p.material == self.side)
            .unwrap();
        for uv in &mut piece.uvs[start..] {
            *uv = (Vec2::from_array(sample) + (Vec2::from_array(*uv) - origin) * scale).to_array();
        }
    }

    fn band<const N: usize>(&mut self, role: usize, outer: [Point; N], inner: [Point; N]) {
        for i in 0..N {
            let next = (i + 1) % N;
            self.surface_polygon(role, [outer[i], outer[next], inner[next], inner[i]]);
        }
    }

    /// One clipped platelet. A single tapered edge joins its raised face to the
    /// backing. Irregular footprint and bevel widths produce chipped corners.
    fn plate<const N: usize>(
        &mut self,
        role: usize,
        outline: [Vec2; N],
        low: f32,
        high: f32,
        bevel: f32,
    ) {
        let middle = outline.iter().copied().sum::<Vec2>() / N as f32;
        let lower = outline.map(|v| [v.x, v.y, low]);
        let top = outline.map(|p| {
            let v = p.lerp(middle, bevel);
            [v.x, v.y, high]
        });
        self.band(role, lower, top);
        self.surface_polygon(role, top);
    }
}

#[derive(Clone, Copy)]
enum Relief {
    Rock,
    Bark,
    Ice,
}

/// Outline landmarks in the 384 px rock reference, in the same +V direction
/// as the face UV. The upper and lower partial slabs continue through the
/// texture border; their chipped edges follow the actual fractures.
fn reference_rock(mesh: &mut FacePatch) {
    type Pixel = [u16; 2];
    let plates: &[(Pixel, &[Pixel])] = &[
        (
            [57, 156],
            &[
                [3, 35],
                [51, 34],
                [100, 29],
                [106, 38],
                [105, 239],
                [99, 252],
                [78, 259],
                [3, 265],
            ],
        ),
        (
            [157, 13],
            &[
                [82, 3],
                [215, 3],
                [209, 18],
                [195, 23],
                [134, 26],
                [92, 24],
                [85, 19],
            ],
        ),
        (
            [191, 104],
            &[
                [126, 34],
                [141, 28],
                [201, 26],
                [215, 17],
                [219, 3],
                [297, 3],
                [297, 129],
                [284, 135],
                [245, 138],
                [210, 144],
                [205, 153],
                [203, 198],
                [133, 200],
                [128, 189],
            ],
        ),
        (
            [345, 79],
            &[
                [304, 3],
                [381, 3],
                [381, 147],
                [322, 150],
                [310, 142],
                [304, 132],
                [303, 66],
            ],
        ),
        (
            [284, 209],
            &[
                [208, 151],
                [220, 143],
                [295, 136],
                [301, 150],
                [313, 162],
                [381, 158],
                [381, 262],
                [323, 267],
                [310, 274],
                [218, 274],
                [209, 265],
            ],
        ),
        (
            [156, 290],
            &[
                [128, 205],
                [200, 203],
                [202, 266],
                [198, 381],
                [88, 381],
                [87, 272],
                [110, 268],
                [122, 258],
            ],
        ),
        (
            [46, 330],
            &[
                [3, 271],
                [37, 270],
                [76, 268],
                [78, 280],
                [78, 353],
                [75, 381],
                [3, 381],
            ],
        ),
        (
            [253, 329],
            &[
                [205, 282],
                [215, 278],
                [308, 277],
                [311, 286],
                [312, 328],
                [304, 343],
                [299, 381],
                [204, 381],
            ],
        ),
        (
            [349, 330],
            &[
                [318, 277],
                [330, 270],
                [381, 267],
                [381, 381],
                [306, 381],
                [308, 349],
                [317, 340],
            ],
        ),
    ];
    let uv = |[x, y]: Pixel| Vec2::new(f32::from(x), f32::from(y)) / 384.0;
    for (i, &(center, contour)) in plates.iter().enumerate() {
        let center = uv(center);
        let mut outline = Vec::with_capacity(contour.len() + 1);
        let chip = (mesh.variant * 3 + i + mesh.side) % contour.len();
        for (j, &pixel) in contour.iter().enumerate() {
            let q = uv(pixel);
            if j == chip && q.min_element() > 0.018 && q.max_element() < 0.982 {
                let before = uv(contour[(j + contour.len() - 1) % contour.len()]);
                let after = uv(contour[(j + 1) % contour.len()]);
                let cut = 0.009 + mesh.noise(5 + i) * 0.011;
                for toward in [before, after] {
                    let delta = toward - q;
                    outline.push(q + delta.normalize() * cut.min(delta.length() * 0.34));
                }
            } else {
                outline.push(q);
            }
        }
        let h = 0.021 + mesh.noise(32 + i) * 0.044;
        let tilt = Vec2::new(mesh.noise(54 + i) - 0.50, mesh.noise(68 + i) - 0.50) * 0.032;
        let bevel = 0.004 + mesh.noise(83 + i) * 0.006;
        let cap: Vec<_> = outline
            .iter()
            .map(|&q| {
                let q = q + (center - q).normalize() * bevel.min(q.distance(center) * 0.12);
                [q.x, q.y, -h - tilt.dot(q - center)]
            })
            .collect();
        for (j, &p) in outline.iter().enumerate() {
            let k = (j + 1) % outline.len();
            let q = outline[k];
            mesh.wound_polygon(
                mesh.side,
                [[p.x, p.y, 0.0], [q.x, q.y, 0.0], cap[k], cap[j]],
            );
            mesh.surface_polygon(mesh.side, [cap[j], cap[k], [center.x, center.y, -h]]);
        }
    }
}

fn masonry(mesh: &mut FacePatch, relief: Relief) {
    let (rows, depth, gap, bevel) = match relief {
        Relief::Rock => (2, 0.085, 0.006, 0.08),
        Relief::Bark => (4, 0.081, 0.006, 0.14),
        Relief::Ice => (2, 0.045, 0.005, 0.08),
    };
    let mut bottom = 0.035;
    for row in 0..rows {
        let top = if row + 1 == rows {
            0.965
        } else {
            0.035 + 0.93 * (row as f32 + 1.0 + 0.34 * (mesh.noise(row) - 0.5)) / rows as f32
        };
        let columns = match relief {
            Relief::Bark => 1 + (mesh.noise(4 + row) > 0.42) as usize,
            Relief::Ice => 2,
            Relief::Rock if row == mesh.variant % 2 => 3,
            _ => 2 + (mesh.noise(4 + row) > 0.47) as usize,
        };
        let mut left = 0.035;
        for column in 0..columns {
            let key = 12 + (row * 3 + column) * 8;
            let right = if column + 1 == columns {
                0.965
            } else {
                0.035
                    + 0.93 * (column as f32 + 1.0 + (mesh.noise(key) - 0.5) * 0.76) / columns as f32
            };
            let x = left + gap;
            let y = bottom + gap;
            let w = right - left - gap * 2.0;
            let h = top - bottom - gap * 2.0;
            let clip = 0.08 + 0.27 * mesh.noise(key + 1);
            let footprint = [
                [0.0, clip],
                [clip * 0.73, 0.0],
                [1.0, 0.058],
                [0.97, 0.98 - clip * 0.28],
                [0.03, 0.97],
            ]
            .map(|[a, b]| {
                let p = Vec2::new(x + w * a, y + h * b);
                if matches!(relief, Relief::Bark) {
                    Vec2::new(p.y, p.x)
                } else {
                    p
                }
            });
            mesh.plate(
                mesh.side,
                footprint,
                0.0,
                -depth * (0.27 + mesh.noise(key + 2) * 0.72),
                bevel,
            );
            left = right;
        }
        bottom = top;
    }
}

// Adjacent pixels share each step wall. A field of 48 small sod/soil blocks
// therefore needs only visible caps and transitions between unlike heights.
fn crust<const NX: usize, const NY: usize>(mesh: &mut FacePatch, form: NaturalForm) {
    let side = mesh.side;
    let turf = matches!(form, NaturalForm::Turf | NaturalForm::ReferenceTurf);
    let fringe = turf && side / 2 != 1;
    // Boundary of the hanging sods in grass_side.png, measured in face UVs.
    // The physical lips rise at the same places as the photographed grass tips.
    let fringe_height = [
        0.672, 0.698, 0.661, 0.760, 0.818, 0.737, 0.688, 0.625, 0.659, 0.745, 0.740, 0.750,
    ];
    let tiled = |i: usize, count: usize, noise: f32| {
        let jitter = if i == 0 || i == count {
            0.0
        } else {
            0.43 * (noise - 0.5)
        };
        0.005 + (i as f32 + jitter) * 0.99 / count as f32
    };
    let xs: Vec<_> = (0..=NX)
        .map(|i| tiled(i, NX, mesh.noise(100 + i)))
        .collect();
    let ys: Vec<_> = (0..=NY)
        .map(|j| tiled(j, NY, mesh.noise(120 + j)))
        .collect();
    let layers: [[(usize, f32); NX]; NY] = std::array::from_fn(|j| {
        std::array::from_fn(|i| {
            let high_sod = if form == NaturalForm::ReferenceTurf {
                (j as f32 + 0.5) / NY as f32 >= fringe_height[i]
            } else {
                j >= 3 + (mesh.noise(48 + i) * 2.9) as usize
            };
            let green = turf && (side == 3 || fringe && high_sod);
            let roughness = (mesh.noise(150 + j * NX + i) * 3.99).floor();
            let height = if green && side == 3 {
                // Closely cropped sod grows across the walk plane. Its thin
                // steps catch grazing light; the thick roots hang on the bank.
                0.004 + roughness * 0.0025
            } else if green {
                0.030
                    + roughness * 0.011
                    + if fringe {
                        0.025 * (j as f32 / NY as f32)
                    } else {
                        0.0
                    }
            } else {
                0.012 + roughness * 0.010
            };
            (if fringe && green { 6 } else { side }, height)
        })
    });
    for j in 0..NY {
        for i in 0..NX {
            let (slot, h) = layers[j][i];
            let neighbours = [
                j.checked_sub(1).map_or(0.0, |y| layers[y][i].1),
                layers[j].get(i + 1).map_or(0.0, |l| l.1),
                layers.get(j + 1).map_or(0.0, |r| r[i].1),
                i.checked_sub(1).map_or(0.0, |x| layers[j][x].1),
            ];
            let bevel = neighbours.map(|n| {
                if h > n {
                    (h - n).min(0.007) * 0.72
                } else {
                    0.0
                }
            });
            let (l, r, b, t) = (xs[i], xs[i + 1], ys[j], ys[j + 1]);
            let corners = [[l, b], [r, b], [r, t], [l, t]];
            let top = [
                [l + bevel[3], b + bevel[0], -h],
                [r - bevel[1], b + bevel[0], -h],
                [r - bevel[1], t - bevel[2], -h],
                [l + bevel[3], t - bevel[2], -h],
            ];
            mesh.surface_polygon(slot, top);
            for (edge, base) in neighbours.into_iter().enumerate() {
                if h <= base {
                    continue;
                }
                let k = (edge + 1) % 4;
                let [a, b] = [corners[edge], corners[k]].map(|[u, v]| [u, v, -base]);
                mesh.wound_polygon(slot, [a, b, top[k], top[edge]]);
            }
        }
    }
    if fringe {
        turf_inclusions(mesh);
    }
}

fn turf_inclusions(mesh: &mut FacePatch) {
    let s = mesh.side;
    if (mesh.variant + s).is_multiple_of(2) {
        let cx = 0.22 + 0.55 * mesh.noise(44);
        for (x, y, r) in [(cx, 0.30, 0.078), (1.0 - cx, 0.16, 0.042)]
            .into_iter()
            .take(1 + usize::from(mesh.noise(45) > 0.49))
        {
            let stone = [
                [-1.0, -0.40],
                [-0.53, 0.76],
                [0.52, 1.0],
                [1.0, -0.45],
                [0.05, -0.85],
            ]
            .map(|[u, v]| Vec2::new(x + u * r, y + v * r * 0.85));
            mesh.plate(7, stone, -0.009, -0.064, 0.30);
        }
        // A small exposed fork runs from the sod into the upper soil bed.
        let x = 0.40 + 0.18 * mesh.noise(51);
        let joint = Vec2::new(x - 0.032, 0.53);
        let crown = Vec2::new(x + 0.036, 0.73);
        let tip = Vec2::new(x + 0.045, 0.35);
        let branch = Vec2::new(x - 0.13, 0.44);
        for (start, end, w0, w1, d0, d1) in [
            (crown, joint, 0.021, 0.015, -0.055, -0.066),
            (joint, tip, 0.015, 0.004, -0.066, -0.008),
            (joint, branch, 0.010, 0.003, -0.066, -0.008),
        ] {
            let a = [start.x - w0, start.y, 0.0];
            let b = [start.x, start.y, d0];
            let c = [start.x + w0, start.y, 0.0];
            let d = [end.x - w1, end.y, 0.0];
            let e = [end.x, end.y, d1];
            let f = [end.x + w1, end.y, 0.0];
            mesh.surface_polygon(s, [a, b, e, d]);
            mesh.surface_polygon(s, [b, c, f, e]);
        }
    }
}

fn pebbles(mesh: &mut FacePatch) {
    // A short course of broad cobbles interlocks with two finer courses.
    // The shallow separate shoulder keeps every stone's crown blunt.
    let courses = if mesh.variant % 2 == 0 {
        [3, 4, 2, 3]
    } else {
        [4, 2, 3, 3]
    };
    let samples = [
        [0.44, 0.64],
        [0.55, 0.57],
        [0.50, 0.13],
        [0.69, 0.78],
        [0.36, 0.35],
        [0.13, 0.68],
    ];
    for (row, columns) in courses.into_iter().enumerate() {
        for col in 0..columns {
            let k = (row * 4 + col) * 9;
            let w = 0.94 / columns as f32;
            let c = Vec2::new(
                0.03 + (col as f32 + 0.5) * w + (mesh.noise(k) - 0.5) * 0.035,
                0.146 + row as f32 * 0.235 + (mesh.noise(k + 1) - 0.5) * 0.038,
            );
            let angle = (mesh.noise(k + 2) - 0.5) * 1.09;
            let (sin, cos) = angle.sin_cos();
            let rx = w * (0.44 + 0.105 * mesh.noise(k + 3));
            let ry = 0.105 + 0.026 * mesh.noise(k + 4);
            let footprint = [
                [-1.0, -0.40],
                [-0.66, -0.86],
                [0.61, -0.88],
                [0.99, -0.49],
                [0.96, 0.50],
                [0.47, 0.92],
                [-0.76, 0.83],
                [-0.99, 0.38],
            ];
            let p = footprint.map(|[x, y]| {
                let (x, y) = (x * rx, y * ry);
                (c + Vec2::new(x * cos - y * sin, x * sin + y * cos))
                    .clamp(Vec2::splat(0.003), Vec2::splat(0.997))
            });
            let h = 0.032 + 0.042 * mesh.noise(k + 5);
            let bevel = 0.092 + 0.085 * mesh.noise(k + 6);
            let ring = |scale: f32, height: f32| {
                p.map(|v| {
                    let q = c.lerp(v, scale);
                    [q.x, q.y, -height]
                })
            };
            let lower = ring(0.93, 0.0);
            let shoulder = ring(1.0, h * 0.65);
            let cap = ring(1.0 - bevel, h);
            let start = mesh.surface_vertex_count();
            for j in 0..p.len() {
                let next = (j + 1) % p.len();
                mesh.wound_polygon(
                    mesh.side,
                    [lower[j], lower[next], shoulder[next], shoulder[j]],
                );
            }
            mesh.band(mesh.side, shoulder, cap);
            mesh.surface_polygon(mesh.side, cap);
            mesh.local_texture(
                start,
                c,
                samples[(col + row * 5 + mesh.variant + mesh.side) % samples.len()],
                0.19,
            );
        }
    }
}

/// Weathered bound asphalt stays continuous up to each joint, with just one
/// narrow, tapering fracture across its face.
fn road_surface(mesh: &mut FacePatch) {
    let offset = 0.27 + 0.43 * mesh.noise(3);
    let rotate = mesh.variant % 2 != 0;
    let point = |u, v, d| if rotate { [v, u, d] } else { [u, v, d] };
    let mut previous: Option<[Point; 5]> = None;
    for (i, u) in [0.012, 0.24, 0.57, 0.79, 0.988].into_iter().enumerate() {
        let v = offset + (mesh.noise(11 + i) - 0.5) * 0.32;
        let w = 0.006 + 0.006 * mesh.noise(21 + i);
        let h = if i == 0 || i == 4 {
            0.0005
        } else {
            0.009 + 0.007 * mesh.noise(31 + i)
        };
        let row = [
            point(u, v - 2.6 * w, 0.0),
            point(u, v - w, -h),
            point(u, v, -0.001),
            point(u, v + w, -h * 0.73),
            point(u, v + 2.3 * w, 0.0),
        ];
        if let Some(last) = previous {
            for j in 0..4 {
                mesh.surface_polygon(mesh.side, [last[j], row[j], row[j + 1], last[j + 1]]);
            }
        }
        previous = Some(row);
    }
}

/// A cut through packed grains has scattered shallow facets. Each small flake
/// tapers into the intact wall, leaving room between the exposed patches.
fn packed_sand(mesh: &mut FacePatch) {
    for (i, [u, v]) in [[0.19, 0.30], [0.61, 0.18], [0.80, 0.61], [0.36, 0.77]]
        .into_iter()
        .enumerate()
    {
        let k = 31 + i * 7;
        let center = Vec2::new(
            u + (mesh.noise(k) - 0.5) * 0.16,
            v + (mesh.noise(k + 1) - 0.5) * 0.14,
        );
        let width = 0.05 + mesh.noise(k + 2) * 0.055;
        let height = 0.04 + mesh.noise(k + 3) * 0.044;
        let outline = [
            [-1.0, -0.2],
            [-0.36, -0.93],
            [0.7, -0.65],
            [1.0, 0.41],
            [-0.45, 0.80],
        ]
        .map(|[x, y]| center + Vec2::new(x * width, y * height));
        mesh.plate(
            mesh.side,
            outline,
            0.0,
            -0.002 - mesh.noise(k + 4) * 0.005,
            0.48,
        );
    }
}

enum Ridge {
    Dunes,
    Drifts,
    Furrows,
    Ribs,
}

fn ridges(mesh: &mut FacePatch, kind: Ridge) {
    let (profile, margin, depth): (&[(f32, f32)], f32, f32) = match kind {
        Ridge::Dunes => (
            &[
                (0.0, 0.35),
                (0.16, 0.16),
                (0.26, 0.84),
                (0.50, 0.17),
                (0.60, 0.90),
                (0.85, 0.15),
                (1.0, 0.48),
            ],
            0.044,
            0.063,
        ),
        Ridge::Drifts => (
            &[
                (0.0, 0.42),
                (0.26, 0.14),
                (0.33, 0.78),
                (0.67, 0.18),
                (0.74, 0.92),
                (1.0, 0.30),
            ],
            0.055,
            0.086,
        ),
        Ridge::Ribs | Ridge::Furrows => (
            &[
                (0.0, 0.82),
                (0.12, 0.10),
                (0.24, 0.95),
                (0.38, 0.08),
                (0.52, 0.93),
                (0.66, 0.12),
                (0.79, 0.96),
                (0.89, 0.16),
                (1.0, 0.80),
            ],
            0.032,
            0.15,
        ),
    };
    // The strips connect directly. Their closed perimeter ramps back to the
    // grid boundary, even at a world chunk seam.
    // Sand/snow footprint cut-outs in both shader passes accept six
    // millimetres of height difference at the logical walk plane.
    let depth = if mesh.side == 3 && matches!(kind, Ridge::Dunes | Ridge::Drifts) {
        0.004
    } else {
        depth
    };
    let side = mesh.side;
    let bend = (mesh.noise(8) - 0.5)
        * if matches!(kind, Ridge::Furrows) {
            0.038
        } else {
            0.18
        };
    let rotate =
        matches!(kind, Ridge::Ribs) || (mesh.variant % 2 != 0 && !matches!(kind, Ridge::Furrows));
    let make = |u: f32, v: f32, d: f32| if rotate { [v, u, d] } else { [u, v, d] };
    let mut last = None;
    for (index, &(v, d)) in profile.iter().enumerate() {
        let y = margin + v * (1.0 - 2.0 * margin);
        let weight = (1.0 - (v * 2.0 - 1.0).abs()) * bend;
        let h = -d * depth * (0.8 + 0.2 * mesh.noise(index));
        let line = [
            make(margin, y - weight * 0.40, h),
            make(0.53, y + weight, h * 0.80),
            make(1.0 - margin, y + weight * 0.15, h),
        ];
        if let Some(prev) = last {
            let prev: [Point; 3] = prev;
            for i in 0..2 {
                mesh.surface_polygon(side, [prev[i], prev[i + 1], line[i + 1]]);
                mesh.surface_polygon(side, [prev[i], line[i + 1], line[i]]);
            }
            let v0 = profile[index - 1].0;
            mesh.surface_polygon(
                side,
                [make(0.0, v0, 0.0), prev[0], line[0], make(0.0, v, 0.0)],
            );
            mesh.surface_polygon(
                side,
                [prev[2], make(1.0, v0, 0.0), make(1.0, v, 0.0), line[2]],
            );
        }
        if index == 0 || index + 1 == profile.len() {
            // Match the strip's central height on the narrow end caps as well.
            for (i, (lo, hi)) in [(0.0, 0.53), (0.53, 1.0)].into_iter().enumerate() {
                mesh.surface_polygon(
                    side,
                    [make(lo, v, 0.0), make(hi, v, 0.0), line[i + 1], line[i]],
                );
            }
        }
        last = Some(line);
    }
}

fn end_grain(mesh: &mut FacePatch) {
    let side = mesh.side;
    let outer = [
        [0.0, 0.15],
        [0.13, 0.0],
        [0.81, 0.0],
        [1.0, 0.13],
        [1.0, 0.85],
        [0.85, 1.0],
        [0.14, 1.0],
        [0.0, 0.81],
    ];
    let mut previous = outer.map(|[u, v]| [u, v, 0.0]);
    let center = Vec2::new(0.43 + 0.11 * mesh.noise(0), 0.44 + 0.11 * mesh.noise(1));
    // The end grain rises between eccentric grooves. Their lowest lines
    // meet the solid cap; the outer corners are provided by the trunk itself.
    for (scale, depth) in [
        (0.82, -0.033),
        (0.77, -0.004),
        (0.72, -0.028),
        (0.48, -0.026),
        (0.43, -0.004),
        (0.38, -0.029),
        (0.17, -0.028),
        (0.12, -0.004),
    ] {
        let ring = outer.map(|[u, v]| {
            let p = center.lerp(Vec2::new(u, v), scale);
            [p.x, p.y, depth]
        });
        mesh.band(side, previous, ring);
        previous = ring;
    }
    mesh.surface_polygon(side, previous);
}

fn leaf_clusters(mesh: &mut FacePatch) {
    // Each stem bears one or two broad folded blades. The two staggered
    // rows overlap at different heights and leave small windows onto the crown.
    let sprays = [
        ([0.14, 0.28], -0.41, true),
        ([0.47, 0.22], 0.70, false),
        ([0.78, 0.32], 2.03, true),
        ([0.23, 0.69], -1.80, true),
        ([0.58, 0.73], 0.91, true),
        ([0.84, 0.77], -1.24, false),
    ];
    for (i, ([x, y], angle, pair)) in sprays.into_iter().enumerate() {
        let root = Vec2::new(x, y);
        let rotation = angle + 0.60 * (mesh.noise(7 + i) - 0.5);
        leaf_blade(mesh, root, rotation, i, 1.0);
        if pair {
            let spread = if i % 2 == 0 { 1.48 } else { -1.41 };
            leaf_blade(mesh, root, rotation + spread, i + 8, 0.82);
        }
    }
}

fn leaf_blade(mesh: &mut FacePatch, root: Vec2, angle: f32, key: usize, scale: f32) {
    // Along the midrib, width of the lower lobe, width of the upper lobe.
    // The first two outlines have notches; the third has a broad simple tip.
    let outline: &[[f32; 3]] = match (key + mesh.variant) % 3 {
        0 => &[
            [0., 0.24, 0.27],
            [0.22, 0.84, 0.93],
            [0.38, 0.93, 0.89],
            [0.44, 0.64, 0.54],
            [0.66, 0.87, 0.78],
            [0.77, 0.50, 0.59],
            [1., 0.31, 0.34],
        ],
        1 => &[
            [0., 0.26, 0.32],
            [0.17, 0.76, 0.96],
            [0.40, 0.91, 0.91],
            [0.63, 0.62, 0.78],
            [0.76, 0.64, 0.57],
            [1., 0.32, 0.28],
        ],
        _ => &[
            [0., 0.26, 0.26],
            [0.24, 0.77, 0.78],
            [0.55, 0.92, 0.86],
            [0.81, 0.74, 0.77],
            [1., 0.39, 0.33],
        ],
    };
    let (sin, cos) = angle.sin_cos();
    let dir = Vec2::new(cos, sin);
    let across = Vec2::new(-sin, cos);
    let length = (0.306 + mesh.noise(16 + key) * 0.065) * scale;
    let width = (0.129 + mesh.noise(32 + key) * 0.042) * scale;
    let tip_height = (0.053 + mesh.noise(48 + key) * 0.040) * scale;
    let skew = mesh.noise(64 + key) * 0.20 - 0.10;
    let base = root - dir * length * 0.21;
    let plan = |t: f32, w: f32| base + dir * (t * length) + across * ((w + t * skew) * width);
    let mut low = Vec2::ONE;
    let mut high = Vec2::ZERO;
    for &[t, l, r] in outline {
        for p in [plan(t, -l), plan(t, r)] {
            low = low.min(p);
            high = high.max(p);
        }
    }
    // Translate a whole peripheral blade. Clamping its individual corners
    // would crush the shape of the lobes into narrow triangles.
    let shift =
        (Vec2::splat(0.004) - low).max(Vec2::ZERO) - (high - Vec2::splat(0.996)).max(Vec2::ZERO);
    let point = |t: f32, w: f32, thickness: f32| {
        let p = plan(t, w) + shift;
        // A shallow ridge along the entire blade, with a gently rolled side.
        // Every triangle on each side of the midrib belongs to the same plane.
        let h = (0.017 + tip_height * t - 0.012 * w.abs() + 0.004 * w - thickness).max(0.0);
        [p.x, p.y, -h]
    };
    let foot_plan = [
        [0., -0.18],
        [0.47, -0.63],
        [1., -0.21],
        [1., 0.21],
        [0.47, 0.63],
        [0., 0.18],
    ];
    let foot = foot_plan.map(|[t, w]| point(t, w, 0.024));
    let middle = point(0.49, 0.0, 0.0);
    let mut top = Vec::with_capacity(2 * outline.len() + 2);
    let lower = |t| {
        if t < 0.23 {
            0
        } else if t < 0.74 {
            1
        } else {
            2
        }
    };
    for &[t, l, _] in outline {
        top.push((point(t, -l, 0.0), lower(t)));
    }
    top.push((point(1.0, 0.0, 0.0), 2));
    for &[t, _, r] in outline.iter().rev() {
        top.push((point(t, r, 0.0), 5 - lower(t)));
    }
    top.push((point(0.0, 0.0, 0.0), 5));
    let start = mesh.surface_vertex_count();
    for i in 0..top.len() {
        let (a, lo) = top[i];
        let (b, hi) = top[(i + 1) % top.len()];
        mesh.surface_polygon(mesh.side, [a, b, middle]);
        if lo == hi {
            mesh.wound_polygon(mesh.side, [foot[lo], b, a]);
        } else {
            mesh.wound_polygon(mesh.side, [foot[lo], foot[hi], b, a]);
        }
    }
    mesh.wound_polygon(
        mesh.side,
        [foot[5], foot[4], foot[3], foot[2], foot[1], foot[0]],
    );
    let samples = [
        [0.45, 0.23],
        [0.50, 0.82],
        [0.87, 0.42],
        [0.26, 0.55],
        [0.15, 0.73],
        [0.75, 0.15],
    ];
    let center = plan(0.5, 0.0) + shift;
    mesh.local_texture(
        start,
        center,
        samples[(key + mesh.variant + mesh.side) % samples.len()],
        0.29,
    );
}

fn radiating_gills(mesh: &mut FacePatch) {
    let c = Vec2::new(0.44 + 0.12 * mesh.noise(0), 0.45 + 0.12 * mesh.noise(1));
    let rim: [Vec2; 16] = std::array::from_fn(|i| {
        let q = (i % 4) as f32 / 4.0;
        match i / 4 {
            0 => Vec2::new(q, 0.0),
            1 => Vec2::new(1.0, q),
            2 => Vec2::new(1.0 - q, 1.0),
            _ => Vec2::new(0.0, 1.0 - q),
        }
    });
    let ring = |i: usize, r: f32, depth| {
        let p = c.lerp(rim[i], r);
        [p.x, p.y, depth]
    };
    let h = |i: usize| if i % 2 == 0 { -0.064 } else { -0.009 };
    for i in 0..16 {
        let j = (i + 1) % 16;
        let a = ring(i, 0.90, h(i));
        let b = ring(j, 0.90, h(j));
        let ai = ring(i, 0.14, -0.026);
        let bi = ring(j, 0.14, -0.026);
        mesh.surface_polygon(mesh.side, [ring(i, 1.0, 0.0), ring(j, 1.0, 0.0), b]);
        mesh.surface_polygon(mesh.side, [ring(i, 1.0, 0.0), b, a]);
        mesh.surface_polygon(mesh.side, [a, b, bi]);
        mesh.surface_polygon(mesh.side, [a, bi, ai]);
        mesh.surface_polygon(mesh.side, [ai, bi, [c.x, c.y, -0.023]]);
    }
}

fn scales(mesh: &mut FacePatch) {
    for (i, [x, y, r]) in [
        [0.28, 0.27, 0.18],
        [0.75, 0.27, 0.13],
        [0.54, 0.48, 0.095],
        [0.76, 0.76, 0.165],
        [0.25, 0.70, 0.12],
    ]
    .into_iter()
    .enumerate()
    {
        let cx = x + (mesh.noise(i) - 0.5) * 0.08;
        let p = [
            [-0.91, -0.46],
            [0.12, -1.0],
            [0.86, -0.46],
            [0.90, 0.42],
            [0.0, 0.94],
            [-0.77, 0.57],
        ]
        .map(|[u, v]| Vec2::new(cx + u * r, y + v * r));
        mesh.plate(mesh.side, p, 0.0, -0.029 - mesh.noise(8 + i) * 0.032, 0.36);
    }
}

fn vent(mesh: &mut FacePatch) {
    let center = Vec2::new(0.41 + 0.17 * mesh.noise(5), 0.41 + 0.17 * mesh.noise(13));
    let perimeter = [
        [0., 0.],
        [0.5, 0.],
        [1., 0.],
        [1., 0.5],
        [1., 1.],
        [0.5, 1.],
        [0., 1.],
        [0., 0.5],
    ];
    let ring = |radius: f32, depth: f32| {
        std::array::from_fn::<_, 8, _>(|i| {
            let edge = Vec2::from_array(perimeter[i]);
            let q = center.lerp(edge, radius * (0.91 + 0.09 * mesh.noise(i)));
            [q.x, q.y, depth]
        })
    };
    let outer = ring(0.79, 0.0);
    let lip = ring(0.54, -0.074);
    let mouth = ring(0.36, -0.058);
    let throat = ring(0.22, 0.0);
    mesh.band(mesh.side, outer, lip);
    mesh.band(mesh.side, lip, mouth);
    mesh.band(mesh.side, mouth, throat);
}

fn coral_cups(mesh: &mut FacePatch) {
    // Unequal flared corallites have solid rims around recessed calices.
    for (i, [x, y, r]) in [
        [0.29, 0.27, 0.21],
        [0.69, 0.29, 0.16],
        [0.29, 0.72, 0.17],
        [0.70, 0.69, 0.20],
    ]
    .into_iter()
    .enumerate()
    {
        let center = Vec2::new(
            x + (mesh.noise(i) - 0.5) * 0.06,
            y + (mesh.noise(i + 6) - 0.5) * 0.05,
        );
        let size = r * (0.90 + 0.1 * mesh.noise(i + 12));
        let points = [
            [-0.93, -0.42],
            [-0.11, -1.0],
            [0.90, -0.43],
            [0.79, 0.64],
            [-0.48, 0.93],
        ];
        let ring = |scale: f32, depth: f32| {
            points.map(|[u, v]| {
                let p = center + Vec2::new(u, v) * scale * size;
                [p.x, p.y, depth]
            })
        };
        let h = -0.061 - 0.038 * mesh.noise(i + 18);
        mesh.band(mesh.side, ring(1.0, 0.0), ring(0.82, h));
        mesh.band(mesh.side, ring(0.82, h), ring(0.58, h + 0.005));
        mesh.band(mesh.side, ring(0.58, h + 0.005), ring(0.31, -0.006));
        mesh.surface_polygon(mesh.side, ring(0.31, -0.006));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec3;

    #[test]
    fn nature_details_are_outside_the_supporting_square_and_never_cross_its_sides() {
        use NaturalForm::*;
        for form in [
            Turf,
            Earth,
            Farmland,
            Rock,
            ReferenceRock,
            ReferenceTurf,
            Gravel,
            Asphalt,
            Sand,
            Snow,
            Ice,
            Log,
            Leaves,
            Petals,
            Vent,
            Mushroom,
            Gills,
            Coral,
            Cactus,
        ] {
            for variant in 0..4 {
                for side in 0..6 {
                    let axis = side / 2;
                    let height = |p: [f32; 3]| {
                        if side % 2 == 0 {
                            -p[axis]
                        } else {
                            p[axis] - 1.0
                        }
                    };
                    let parts = face(form, side, variant);
                    assert!(!parts.is_empty(), "{form:?} has no decoration");
                    for part in parts {
                        for &p in &part.positions {
                            assert!((-1e-6..=0.15).contains(&height(p)), "{form:?} {side} {p:?}");
                            assert!(
                                (0..3)
                                    .filter(|&i| i != axis)
                                    .all(|i| (0.0..=1.0).contains(&p[i]))
                            );
                        }
                        for tri in part.indices.chunks_exact(3) {
                            let [a, b, c] =
                                [tri[0], tri[1], tri[2]].map(|i| part.positions[i as usize]);
                            assert!(
                                [a, b, c].into_iter().any(|p| height(p) > 1e-6),
                                "coplanar cap on {form:?}"
                            );
                            let normal = (Vec3::from_array(b) - Vec3::from_array(a))
                                .cross(Vec3::from_array(c) - Vec3::from_array(a))
                                .normalize();
                            assert!(
                                normal.dot(Vec3::from_array(part.normals[tri[0] as usize])) > 0.999
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn reference_stone_keeps_the_painted_breaks_and_uv_projection_in_all_variants() {
        let mut profiles = std::collections::HashSet::new();
        for variant in 0..4 {
            let pieces = face(NaturalForm::ReferenceRock, 3, variant);
            let mut heights = Vec::new();
            for piece in &pieces {
                for (i, &[x, y, z]) in piece.positions.iter().enumerate() {
                    // The cap carries the full picture. The wall UVs unwrap
                    // down the steep bevel to keep the edge from stretching.
                    if piece.normals[i][1] > 0.99 {
                        assert_eq!(piece.uvs[i], [x, z]);
                    }
                    heights.push(y.to_bits());
                }
            }
            assert!(profiles.insert(heights));
            let covered = |x: f32, y: f32| {
                let p = Vec2::new(x, y) / 384.0;
                pieces.iter().any(|piece| {
                    piece.indices.chunks_exact(3).any(|tri| {
                        let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| {
                            let [x, _, z] = piece.positions[i as usize];
                            Vec2::new(x, z)
                        });
                        let area = (b - a).perp_dot(c - a);
                        if area.abs() < 1e-10 {
                            return false;
                        }
                        let u = (p - a).perp_dot(c - a) / area;
                        let v = (b - a).perp_dot(p - a) / area;
                        u >= -1e-6 && v >= -1e-6 && u + v <= 1.000001
                    })
                })
            };
            for (x, y) in [
                (55., 150.),
                (157., 13.),
                (190., 100.),
                (345., 80.),
                (289., 210.),
                (156., 291.),
                (46., 330.),
                (253., 330.),
                (349., 330.),
            ] {
                assert!(covered(x, y), "missing reference plate at {x},{y}");
            }
            for (x, y) in [(115., 150.), (300., 93.), (275., 276.), (81., 326.)] {
                assert!(
                    !covered(x, y),
                    "geometry bridged a reference fracture at {x},{y}"
                );
            }
        }
    }

    #[test]
    fn raised_surfaces_keep_sand_and_snow_tracks_inside_the_shader_cutout() {
        for form in [NaturalForm::Sand, NaturalForm::Snow] {
            for variant in 0..4 {
                let lid = face(form, 3, variant);
                assert!(
                    lid.iter()
                        .flat_map(|p| &p.positions)
                        .all(|p| p[1] - 1.0 < 0.006)
                );
                assert!(lid.iter().flat_map(|p| &p.normals).all(|n| n[1] > 0.98));
            }
        }
    }

    #[test]
    fn turf_caps_are_low_and_exposed_sand_banks_have_local_facets() {
        for variant in 0..4 {
            for form in [NaturalForm::Turf, NaturalForm::ReferenceTurf] {
                let cap = face(form, 3, variant);
                let height = cap
                    .iter()
                    .flat_map(|p| &p.positions)
                    .map(|p| p[1] - 1.0)
                    .fold(0., f32::max);
                assert!((0.005..0.012).contains(&height));
            }
            for side in [0, 1, 2, 4, 5] {
                let axis = side / 2;
                for piece in face(NaturalForm::Sand, side, variant) {
                    for triangle in piece.indices.chunks_exact(3) {
                        let points = [triangle[0], triangle[1], triangle[2]]
                            .map(|i| Vec3::from_array(piece.positions[i as usize]));
                        let depth = |p: Vec3| {
                            if side % 2 == 0 {
                                -p[axis]
                            } else {
                                p[axis] - 1.
                            }
                        };
                        for i in 0..3 {
                            assert!((-1e-6..0.008).contains(&depth(points[i])));
                            assert!(points[i].distance(points[(i + 1) % 3]) < 0.22);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn the_section_mesher_covers_each_visible_grid_square_at_every_lod() {
        use super::super::{ReferencePalette, register};
        use crate::blocks::{
            BlockTemplate, TemplateSet, cube_faces,
            definitions::{BlockLibrary, BlockRenderKind},
        };
        use crate::meshing::{
            DetailLevel,
            builder::{RenderOptions, compose_section_with_options},
        };
        use terra_voxel::{SectionPos, meshing::BlockFace};

        let directory = terra_core::asset_directory().join("terra/blocks");
        let mut library = BlockLibrary::load(&directory).unwrap();
        library.blocks.retain(|b| matches!(b.id, 1 | 3 | 7 | 42));
        let mut templates = TemplateSet::default();
        for def in &library.blocks {
            let materials = [def.id as usize; 6];
            templates.blocks.insert(
                def.id,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::Cube,
                    bevel_width: 0.0,
                    faces: cube_faces(0.0, materials, None),
                    face_materials: materials,
                    side_cap: None,
                    details: Vec::new(),
                    model: Vec::new(),
                },
            );
        }
        let palette = ReferencePalette {
            grass_top: Some(90),
            grass_side: Some(91),
            soil: Some(92),
            rock: Some(93),
            gravel: Some(94),
            leaves: Some(95),
            ..Default::default()
        };
        register(&library, &mut templates, &palette);
        for def in &library.blocks {
            for side in 0..6 {
                let axis = side / 2;
                let (u_axis, v_axis) = match axis {
                    0 => (2, 1),
                    1 => (0, 2),
                    _ => (0, 1),
                };
                let face = BlockFace {
                    minimum: [0; 3],
                    width: 1,
                    height: 1,
                    side,
                    u_axis,
                    v_axis,
                    material: def.id,
                    edge_mask: 15,
                    occupied_neighbor: false,
                };
                for detail_level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
                    let options = RenderOptions {
                        detail_level,
                        water_level: None,
                        terrain: None,
                        exposure: None,
                        conditions: Default::default(),
                        tints: Default::default(),
                    };
                    for occupied_neighbor in [false, true] {
                        let batches = compose_section_with_options(
                            &[BlockFace {
                                occupied_neighbor,
                                ..face
                            }],
                            &templates,
                            SectionPos::new(0, 0, 0),
                            &options,
                        )
                        .unwrap();
                        let plane = (side % 2) as f32;
                        let mut area = 0.0;
                        for batch in &batches {
                            for tri in batch.indices.chunks_exact(3) {
                                let [a, b, c] = [tri[0], tri[1], tri[2]]
                                    .map(|i| Vec3::from_array(batch.positions[i as usize]));
                                if [a, b, c].iter().all(|p| p[axis] == plane) {
                                    let signed = (b - a).cross(c - a)[axis] * (plane * 2.0 - 1.0);
                                    assert!(signed >= 0.0, "inside-out solid surface");
                                    area += signed * 0.5;
                                }
                            }
                        }
                        assert!(
                            (area - 1.0).abs() < 1e-5,
                            "{} side {side}, {detail_level:?}, occupied {occupied_neighbor}: {area} m²",
                            def.name
                        );
                        if occupied_neighbor {
                            assert_eq!(
                                batches
                                    .iter()
                                    .map(|batch| batch.indices.len())
                                    .sum::<usize>(),
                                6,
                                "{} side {side}, {detail_level:?}: relief intersects the neighbouring cell",
                                def.name
                            );
                        }
                    }
                }
            }
        }
    }
}
