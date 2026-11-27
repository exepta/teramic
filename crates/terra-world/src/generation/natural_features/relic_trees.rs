//! Twisting, leafless crowns and buttress roots made of independently mineable
//! cells. The short segments share endpoints, including where a limb forks.
use super::{Cell, Habitat, TILE_SIZE, Tile};
use std::collections::BTreeMap;
use terra_voxel::MaterialId as M;

pub(super) struct TreeForm {
    pub root: Cell,
    pub height: f64,
    pub radius: f64,
    pub wood: M,
    pub seed: u32,
}

impl TreeForm {
    fn sweep(
        &self,
        cells: &mut BTreeMap<Cell, u16>,
        start: [f64; 3],
        end: [f64; 3],
        thickness: [f64; 2],
    ) {
        let delta: [f64; 3] = std::array::from_fn(|i| end[i] - start[i]);
        let length = delta.iter().map(|v| v * v).sum::<f64>().max(0.01);
        let radius = thickness[0].max(thickness[1]);
        let first: [i64; 3] =
            std::array::from_fn(|i| (start[i].min(end[i]) - radius).floor() as i64);
        let last: [i64; 3] = std::array::from_fn(|i| (start[i].max(end[i]) + radius).ceil() as i64);
        let owner = [
            self.root[0].div_euclid(TILE_SIZE),
            self.root[2].div_euclid(TILE_SIZE),
        ];
        for x in first[0]..=last[0] {
            for z in first[2]..=last[2] {
                if [x.div_euclid(TILE_SIZE), z.div_euclid(TILE_SIZE)] != owner {
                    continue;
                }
                for y in first[1]..=last[1] {
                    let p = [x, y, z];
                    let v: [f64; 3] = std::array::from_fn(|i| p[i] as f64 - start[i]);
                    let t = (v.iter().zip(delta).map(|(a, b)| a * b).sum::<f64>() / length)
                        .clamp(0., 1.);
                    let r = (thickness[0] + (thickness[1] - thickness[0]) * t).max(0.88);
                    let d = v
                        .iter()
                        .zip(delta)
                        .map(|(a, b)| (a - b * t).powi(2))
                        .sum::<f64>();
                    if d <= r * r {
                        let mineral_band = (y - self.root[1] + (x - self.root[0]).div_euclid(3)
                            - (z - self.root[2]).div_euclid(4))
                        .rem_euclid(15);
                        let material = if self.wood == M::PetrifiedLog && mineral_band <= 1 {
                            M::PetrifiedSapwood
                        } else {
                            self.wood
                        };
                        cells.insert(p, material as u16);
                    }
                }
            }
        }
    }

    fn point(&self, dx: f64, dy: f64, dz: f64) -> [f64; 3] {
        [
            self.root[0] as f64 + dx,
            self.root[1] as f64 + dy,
            self.root[2] as f64 + dz,
        ]
    }

    pub(super) fn grow(&self, habitat: &mut Habitat<'_>, cells: &mut BTreeMap<Cell, u16>) {
        let angle = f64::from(self.seed % 360).to_radians();
        let (sa, ca) = angle.sin_cos();
        let h = self.height;
        let r = self.radius;
        let foot = self.point(0., 0., 0.);
        let collar = self.point(-ca * 0.7, h * 0.31, -sa * 0.7);
        let fork = self.point(ca * 1.55, h * 0.52, sa * 1.55);
        let crown = self.point(ca * 2.1 - sa * 1.7, h * 0.79, sa * 2.1 + ca * 1.7);
        self.sweep(cells, foot, collar, [r * 1.12, r * 0.83]);
        self.sweep(cells, collar, fork, [r * 0.83, r * 0.68]);
        self.sweep(cells, fork, crown, [r * 0.68, r * 0.52]);
        self.sweep(
            cells,
            crown,
            self.point(ca * 2.8 - sa, h, sa * 2.8 + ca),
            [r * 0.52, 0.82],
        );
        for i in 0..6 {
            let a = angle + f64::from(i) * 2.39996;
            let (s, c) = a.sin_cos();
            let spread = h * (0.32 + f64::from((self.seed.rotate_left(i * 3) >> 4) % 10) * 0.012);
            let start = if i.is_multiple_of(3) {
                collar
            } else if i % 3 == 1 {
                fork
            } else {
                crown
            };
            let elbow = [
                start[0] + c * spread * 0.42 - s * 0.7,
                start[1] + h * 0.115,
                start[2] + s * spread * 0.42 + c * 0.7,
            ];
            let wrist = [
                start[0] + c * spread,
                start[1] + h * 0.18,
                start[2] + s * spread,
            ];
            self.sweep(cells, start, elbow, [r * 0.58, r * 0.43]);
            self.sweep(cells, elbow, wrist, [r * 0.43, 0.93]);
            self.sweep(
                cells,
                wrist,
                [
                    wrist[0] + c * 1.3 - s,
                    wrist[1] + h * 0.16,
                    wrist[2] + s * 1.3 + c,
                ],
                [1.03, 0.72],
            );
            self.sweep(
                cells,
                elbow,
                [
                    elbow[0] + c * 1.4 + s * 2.4,
                    elbow[1] + h * 0.19,
                    elbow[2] + s * 1.4 - c * 2.4,
                ],
                [1.12, 0.70],
            );
            let span = (r * 2.7 + f64::from(i % 3)).min(9.);
            let toe: Tile = [
                self.root[0] + (c * span).round() as i64,
                self.root[2] + (s * span).round() as i64,
            ];
            let col = habitat.column(toe);
            if col.lava_level.is_some() || col.water_level.is_some() {
                continue;
            }
            let floor = (col.height - 0.5).floor();
            let knee = self.point(
                c * span * 0.50 - s * 0.55,
                h * 0.12,
                s * span * 0.50 + c * 0.55,
            );
            self.sweep(
                cells,
                self.point(c * r * 0.46, h * 0.26, s * r * 0.46),
                knee,
                [r * 0.60, r * 0.44],
            );
            self.sweep(
                cells,
                knee,
                [toe[0] as f64, floor + 0.55, toe[1] as f64],
                [r * 0.44, 0.78],
            );
        }
    }
}
