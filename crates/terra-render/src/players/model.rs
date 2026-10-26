//! Faceted reference avatars. Ten shared meshes articulate shoulders, elbows, hips and knees.
use super::AvatarPart;
use crate::meshing::builder::MeshBatch;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::collections::BTreeMap;
use terra_core::PlayerGender;

const SKIN: [f32; 3] = [0.94, 0.65, 0.40];
const SKIN_LIGHT: [f32; 3] = [0.98, 0.71, 0.46];
const CLOTH: [f32; 3] = [0.83, 0.78, 0.65];
const TROUSERS: [f32; 3] = [0.36, 0.28, 0.19];
const CUFF: [f32; 3] = [0.51, 0.42, 0.29];
const CORD: [f32; 3] = [0.72, 0.62, 0.44];
const NECK_HEIGHT: f32 = 0.049;
pub(super) const EYE_CENTER: f32 = 0.027;
pub(super) const EYE_HALF_HEIGHT: f32 = 0.039;
pub(crate) const ELBOW: f32 = -0.265;
pub(super) const KNEE: f32 = -0.31;
const FACET_SIZE: f32 = 0.019;

#[derive(Default)]
pub(crate) struct Builder(BTreeMap<usize, MeshBatch>);

impl Builder {
    fn face_plate(
        &mut self,
        position: Vec3,
        size: Vec3,
        corner: f32,
        color: [f32; 3],
    ) -> Result<(), String> {
        let half = size * 0.5;
        let ring = |z| {
            [
                (-half.x + corner, -half.y),
                (half.x - corner, -half.y),
                (half.x, -half.y + corner),
                (half.x, half.y - corner),
                (half.x - corner, half.y),
                (-half.x + corner, half.y),
                (-half.x, half.y - corner),
                (-half.x, -half.y + corner),
            ]
            .map(|(x, y)| position + Vec3::new(x, y, z))
        };
        let front = ring(-half.z);
        let back = ring(half.z);
        for side in 0..8 {
            let next = (side + 1) % 8;
            self.triangle(
                [position - Vec3::Z * half.z, front[next], front[side]],
                color,
            )?;
            self.quad([front[side], front[next], back[next], back[side]], color)?;
        }
        Ok(())
    }
    pub(crate) fn bevel_box(
        &mut self,
        position: [f32; 3],
        size: [f32; 3],
        color: [f32; 3],
        rotation: Quat,
    ) -> Result<(), String> {
        let half = Vec3::from_array(size) * 0.5;
        let bevel = half.min_element() * 0.18;
        let ring = |y: f32, inset: f32| {
            let x = half.x - inset;
            let z = half.z - inset;
            [
                (-x + bevel, -z),
                (x - bevel, -z),
                (x, -z + bevel),
                (x, z - bevel),
                (x - bevel, z),
                (-x + bevel, z),
                (-x, z - bevel),
                (-x, -z + bevel),
            ]
            .map(|(x, z)| Vec3::from_array(position) + rotation * Vec3::new(x, y, z))
        };
        let rings = [
            ring(-half.y, bevel),
            ring(-half.y + bevel, 0.0),
            ring(half.y - bevel, 0.0),
            ring(half.y, bevel),
        ];
        for (row, pair) in rings.windows(2).enumerate() {
            for side in 0..8 {
                let next = (side + 1) % 8;
                self.quad(
                    [pair[0][next], pair[0][side], pair[1][side], pair[1][next]],
                    color.map(|c| (c * if row == 1 { 1.0 } else { 1.025 }).min(1.0)),
                )?;
            }
        }
        for (row, sign) in [(0, -1.0), (3, 1.0)] {
            let center = Vec3::from_array(position) + rotation * Vec3::Y * half.y * sign;
            for side in 0..8 {
                let next = (side + 1) % 8;
                let (a, b) = if row == 0 { (side, next) } else { (next, side) };
                self.triangle([center, rings[row][a], rings[row][b]], color)?;
            }
        }
        Ok(())
    }

    pub(crate) fn voxel(
        &mut self,
        position: [f32; 3],
        size: [f32; 3],
        color: [f32; 3],
    ) -> Result<(), String> {
        self.rotated(position, size, color, Quat::IDENTITY)
    }

    pub(crate) fn rotated(
        &mut self,
        position: [f32; 3],
        size: [f32; 3],
        color: [f32; 3],
        rotation: Quat,
    ) -> Result<(), String> {
        let start = self.0.get(&0).map_or(0, |batch| batch.positions.len());
        crate::props::add_box(
            &mut self.0,
            0,
            Vec3::from_array(position),
            Vec3::from_array(size),
            rotation,
        )?;
        if let Some(batch) = self.0.get_mut(&0) {
            batch.colors.resize(batch.positions.len(), [1.0; 4]);
            batch.colors[start..].fill(Color::srgb_from_array(color).to_linear().to_f32_array());
        }
        Ok(())
    }

    pub(crate) fn quad(&mut self, points: [Vec3; 4], color: [f32; 3]) -> Result<(), String> {
        let batch = self.0.entry(0).or_default();
        let index = u32::try_from(batch.positions.len()).map_err(|_| "Avatar vertex limit")?;
        let normal = (points[1] - points[0])
            .cross(points[2] - points[0])
            .normalize_or_zero();
        let tint = Color::srgb_from_array(color).to_linear().to_f32_array();
        batch.positions.extend(points.map(|point| point.to_array()));
        batch.normals.extend([normal.to_array(); 4]);
        batch
            .uvs
            .extend([[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
        batch.colors.extend([tint; 4]);
        batch
            .indices
            .extend([index, index + 1, index + 2, index, index + 2, index + 3]);
        Ok(())
    }

    pub(crate) fn triangle(&mut self, points: [Vec3; 3], color: [f32; 3]) -> Result<(), String> {
        let batch = self.0.entry(0).or_default();
        let index = u32::try_from(batch.positions.len()).map_err(|_| "Avatar vertex limit")?;
        let normal = (points[1] - points[0])
            .cross(points[2] - points[0])
            .normalize_or_zero();
        batch.positions.extend(points.map(|point| point.to_array()));
        batch.normals.extend([normal.to_array(); 3]);
        batch.uvs.extend([[0.5, 0.5], [0.0, 0.0], [1.0, 0.0]]);
        batch
            .colors
            .extend([Color::srgb_from_array(color).to_linear().to_f32_array(); 3]);
        batch.indices.extend([index, index + 1, index + 2]);
        Ok(())
    }

    /// Eight-sided rings soften the silhouette without smoothing the voxel facets.
    fn tapered(
        &mut self,
        rings: &[(f32, f32, f32)],
        center: Vec3,
        color: [f32; 3],
    ) -> Result<(), String> {
        let ring = |(y, width, depth): (f32, f32, f32)| {
            let x = width * 0.5;
            let z = depth * 0.5;
            let bevel = x.min(z) * 0.23;
            [
                (-x + bevel, -z),
                (x - bevel, -z),
                (x, -z + bevel),
                (x, z - bevel),
                (x - bevel, z),
                (-x + bevel, z),
                (-x, z - bevel),
                (-x, -z + bevel),
            ]
            .map(|(x, z)| center + Vec3::new(x, y, z))
        };
        for pair in rings.windows(2) {
            let low = ring(pair[0]);
            let high = ring(pair[1]);
            for side in 0..8 {
                let next = (side + 1) % 8;
                {
                    let rows = ((pair[1].0 - pair[0].0) / FACET_SIZE).ceil().max(1.0) as u32;
                    let columns = ((low[next] - low[side]).length() / FACET_SIZE)
                        .ceil()
                        .max(1.0) as u32;
                    let normal = (low[side] - low[next])
                        .cross(high[side] - low[next])
                        .normalize_or_zero();
                    for row in 0..rows {
                        let a = row as f32 / rows as f32;
                        let b = (row + 1) as f32 / rows as f32;
                        for column in 0..columns {
                            let x = column as f32 / columns as f32;
                            let y = (column + 1) as f32 / columns as f32;
                            let point = |u: f32, v: f32| {
                                let skin_relief = (u * std::f32::consts::PI).sin()
                                    * (v * std::f32::consts::PI).sin()
                                    * 0.002;
                                low[side]
                                    .lerp(low[next], u)
                                    .lerp(high[side].lerp(high[next], u), v)
                                    + normal * skin_relief
                            };
                            let h = noise(row * 137 + column * 73 + side as u32 * 31);
                            self.quad(
                                [point(y, a), point(x, a), point(x, b), point(y, b)],
                                color.map(|c| c * (0.98 + (h & 7) as f32 * 0.005)),
                            )?;
                        }
                    }
                }
            }
        }
        for (entry, top) in [
            rings.first().map(|r| (*r, false)),
            rings.last().map(|r| (*r, true)),
        ]
        .into_iter()
        .flatten()
        {
            let p = ring(entry);
            let center = center + Vec3::Y * entry.0;
            for side in 0..8 {
                let next = (side + 1) % 8;
                let pair = if top {
                    [p[next], p[side]]
                } else {
                    [p[side], p[next]]
                };
                self.triangle([center, pair[0], pair[1]], color)?;
            }
        }
        Ok(())
    }

    pub(crate) fn finish_batch(mut self) -> Result<MeshBatch, String> {
        self.0
            .remove(&0)
            .ok_or_else(|| "Empty avatar body part".into())
    }

    pub(crate) fn finish(self) -> Result<Mesh, String> {
        let batch = self.finish_batch()?;
        Ok(Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, batch.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, batch.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, batch.colors)
        .with_inserted_indices(Indices::U32(batch.indices)))
    }
}

fn noise(mut value: u32) -> u32 {
    value = (value ^ (value >> 16)).wrapping_mul(0x7feb_352d);
    value = (value ^ (value >> 15)).wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

pub(super) fn parts(gender: PlayerGender) -> Result<Vec<(AvatarPart, Vec3, Mesh)>, String> {
    let female = gender == PlayerGender::Female;
    let shoulder = if female { 0.235 } else { 0.283 };
    Ok(vec![
        (AvatarPart::Body, Vec3::ZERO, body(female)?),
        (AvatarPart::Head, Vec3::new(0.0, 0.549, 0.0), head(female)?),
        (
            AvatarPart::Arm(-1),
            Vec3::new(-shoulder, 0.305, 0.0),
            arm(-1.0, female)?,
        ),
        (
            AvatarPart::Arm(1),
            Vec3::new(shoulder, 0.305, 0.0),
            arm(1.0, female)?,
        ),
        (
            AvatarPart::Forearm(-1),
            Vec3::new(0.0, ELBOW, 0.0),
            forearm(-1.0, female)?,
        ),
        (
            AvatarPart::Forearm(1),
            Vec3::new(0.0, ELBOW, 0.0),
            forearm(1.0, female)?,
        ),
        (
            AvatarPart::Leg(-1),
            Vec3::new(-0.115, -0.23, 0.0),
            leg(-1.0, female)?,
        ),
        (
            AvatarPart::Leg(1),
            Vec3::new(0.115, -0.23, 0.0),
            leg(1.0, female)?,
        ),
        (
            AvatarPart::Calf(-1),
            Vec3::new(0.0, KNEE, 0.0),
            calf(-1.0, female)?,
        ),
        (
            AvatarPart::Calf(1),
            Vec3::new(0.0, KNEE, 0.0),
            calf(1.0, female)?,
        ),
    ])
}

fn head(female: bool) -> Result<Mesh, String> {
    let mut b = Builder::default();
    let width = if female { 0.345 } else { 0.36 };
    b.tapered(
        &[
            (-0.17, width * 0.57, 0.21),
            (-0.148, width * 0.78, 0.27),
            (-0.090, width * 0.94, 0.315),
            (0.018, width, 0.324),
            (0.125, width * 0.96, 0.315),
            (0.181, width * 0.80, 0.27),
        ],
        Vec3::ZERO,
        SKIN,
    )?;
    for sign in [-1.0, 1.0] {
        b.tapered(
            &[
                (-0.084, 0.038, 0.043),
                (-0.057, 0.061, 0.065),
                (0.014, 0.055, 0.058),
                (0.031, 0.035, 0.046),
            ],
            Vec3::new(sign * (width * 0.5 + 0.018), 0.0, 0.0),
            SKIN_LIGHT,
        )?;
        b.voxel(
            [sign * (width * 0.5 + 0.039), -0.027, -0.013],
            [0.012, 0.041, 0.027],
            [0.73, 0.43, 0.26],
        )?;
        b.face_plate(
            Vec3::new(sign * 0.085, EYE_CENTER, -0.166),
            Vec3::new(0.074, 0.069, 0.012),
            0.011,
            [0.95, 0.92, 0.83],
        )?;
        b.face_plate(
            Vec3::new(sign * 0.079, EYE_CENTER - 0.001, -0.174),
            Vec3::new(0.041, 0.058, 0.011),
            0.007,
            [0.32, 0.20, 0.12],
        )?;
        b.face_plate(
            Vec3::new(sign * 0.077, EYE_CENTER + 0.003, -0.181),
            Vec3::new(0.022, 0.049, 0.007),
            0.004,
            [0.15, 0.10, 0.065],
        )?;
        b.voxel(
            [sign * 0.082 - 0.008, EYE_CENTER + 0.019, -0.187],
            [0.010, 0.013, 0.005],
            [1.0, 0.97, 0.84],
        )?;
        b.voxel(
            [sign * 0.077 + 0.009, EYE_CENTER - 0.014, -0.185],
            [0.005, 0.006, 0.005],
            [0.71, 0.52, 0.30],
        )?;
        b.rotated(
            [sign * 0.085, 0.066, -0.174],
            [0.073, if female { 0.013 } else { 0.010 }, 0.016],
            [0.23, 0.13, 0.075],
            Quat::from_rotation_z(sign * 0.05),
        )?;
        b.rotated(
            [sign * 0.085, 0.099, -0.168],
            [0.075, if female { 0.018 } else { 0.025 }, 0.018],
            if female {
                [0.36, 0.18, 0.085]
            } else {
                [0.25, 0.14, 0.075]
            },
            Quat::from_rotation_z(-sign * 0.065),
        )?;
        if female {
            b.voxel(
                [sign * 0.122, 0.066, -0.174],
                [0.016, 0.009, 0.016],
                [0.23, 0.13, 0.075],
            )?;
        }
    }
    b.bevel_box(
        [0.0, -0.030, -0.173],
        [0.028, 0.035, 0.025],
        SKIN_LIGHT,
        Quat::IDENTITY,
    )?;
    b.bevel_box(
        [0.0, -0.041, -0.186],
        [0.036, 0.018, 0.012],
        [0.86, 0.58, 0.35],
        Quat::IDENTITY,
    )?;
    b.voxel(
        [0.0, -0.106, -0.158],
        [0.068, 0.006, 0.009],
        [0.45, 0.24, 0.135],
    )?;
    for sign in [-1.0, 1.0] {
        b.rotated(
            [sign * 0.041, -0.101, -0.158],
            [0.019, 0.006, 0.008],
            [0.45, 0.24, 0.135],
            Quat::from_rotation_z(sign * 0.35),
        )?;
    }
    hair(&mut b, female)?;
    b.finish()
}

fn hair(b: &mut Builder, female: bool) -> Result<(), String> {
    let base = if female {
        [0.49, 0.255, 0.115]
    } else {
        [0.35, 0.205, 0.115]
    };
    // A staggered ellipsoidal shell avoids latitudinal hair rings and hidden core cubes.
    let occupied = |x: i32, y: i32, z: i32| {
        let p = Vec3::new(x as f32 * 0.047, y as f32 * 0.047, z as f32 * 0.047);
        (p / Vec3::new(0.204, 0.220, 0.191)).length_squared() <= 1.0
            && !(p.z < -0.100 && p.y < 0.085)
            && !(p.x.abs() > 0.165 && p.y < -0.015 && p.z < 0.055)
    };
    let neighbors = [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ];
    for y in -4_i32..=4 {
        for x in -4_i32..=4 {
            for z in -4_i32..=4 {
                if !occupied(x, y, z)
                    || neighbors
                        .iter()
                        .all(|d| occupied(x + d[0], y + d[1], z + d[2]))
                {
                    continue;
                }
                let h = noise(((x + 5) * 317 + (y + 5) * 673 + (z + 5) * 137) as u32);
                let jitter = |shift| (((h >> shift) & 15_u32) as f32 - 7.5) * 0.0015;
                b.bevel_box(
                    [
                        x as f32 * 0.047 + jitter(0),
                        0.048 + y as f32 * 0.047 + jitter(4),
                        0.035 + z as f32 * 0.047 + jitter(8),
                    ],
                    [0.058 + jitter(12), 0.061 + jitter(16), 0.060 + jitter(20)],
                    base.map(|c| c * (0.87 + (h & 15) as f32 * 0.016)),
                    Quat::from_euler(
                        EulerRot::XYZ,
                        jitter(6) * 9.0,
                        jitter(10) * 12.0,
                        jitter(14) * 10.0,
                    ),
                )?;
            }
        }
    }
    for x in -3_i32..=3 {
        let drop = if female {
            (x + 3) as f32 * 0.010
        } else {
            (noise((x + 4) as u32) & 7) as f32 * 0.006
        };
        b.bevel_box(
            [x as f32 * 0.045, 0.178 - drop, -0.158],
            [0.055, 0.071, 0.065],
            base.map(|c| c * (1.0 + x.rem_euclid(3) as f32 * 0.06)),
            Quat::from_rotation_z(if female { 0.22 } else { -x as f32 * 0.035 }),
        )?;
    }
    for side in [-1.0, 1.0] {
        let steps = if female { 7 } else { 2 };
        for step in 0..steps {
            let curl = (step as f32 * 1.7 + side).sin();
            b.bevel_box(
                [
                    side * (0.170 + curl * 0.017),
                    0.09 - step as f32 * 0.047,
                    -0.124,
                ],
                [if step > 3 { 0.028 } else { 0.047 }, 0.061, 0.047],
                base.map(|c| c * 1.07),
                Quat::from_rotation_z(side * curl * 0.2),
            )?;
        }
    }
    if female {
        for y in -2_i32..=2 {
            for x in -2_i32..=2 {
                for z in -2_i32..=2 {
                    let p = Vec3::new(x as f32, y as f32, z as f32);
                    if p.length_squared() > 5.5 || p.length_squared() < 2.0 {
                        continue;
                    }
                    let h = noise(((x + 3) * 67 + (y + 3) * 173 + (z + 3) * 37) as u32);
                    b.bevel_box(
                        [
                            -0.030 + x as f32 * 0.038,
                            0.235 + y as f32 * 0.038,
                            0.171 + z as f32 * 0.038,
                        ],
                        [0.052, 0.055, 0.053],
                        base.map(|c| c * (0.93 + (h & 7) as f32 * 0.021)),
                        Quat::from_euler(
                            EulerRot::XYZ,
                            y as f32 * 0.09,
                            z as f32 * 0.10,
                            x as f32 * 0.08,
                        ),
                    )?;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn eyelids() -> Result<Mesh, String> {
    let mut b = Builder::default();
    for side in [-1.0, 1.0] {
        b.voxel(
            [side * 0.085, 0.0, -0.192],
            [0.071, EYE_HALF_HEIGHT * 2.0, 0.012],
            SKIN,
        )?;
    }
    b.finish()
}

fn body(female: bool) -> Result<Mesh, String> {
    let mut b = Builder::default();
    let waist = if female { 0.29 } else { 0.39 };
    let chest = if female { 0.35 } else { 0.43 };
    // The neck remains short; shoulders and head meet it without a floating gap.
    b.voxel(
        [0.0, 0.33 + NECK_HEIGHT * 0.5, 0.0],
        [0.145, NECK_HEIGHT, 0.16],
        SKIN,
    )?;
    // Only the neckline is exposed; the opaque shirt replaces the hidden torso shell.
    b.tapered(
        &[(0.195, chest, 0.272), (0.33, chest * 0.94, 0.22)],
        Vec3::ZERO,
        SKIN,
    )?;
    b.tapered(
        &[
            (-0.084, waist + 0.016, if female { 0.242 } else { 0.263 }),
            (
                0.015,
                waist * 0.96 + 0.008,
                if female { 0.237 } else { 0.261 },
            ),
            (0.10, waist + 0.016, if female { 0.255 } else { 0.27 }),
            (0.16, chest + 0.004, if female { 0.310 } else { 0.28 }),
            (0.205, chest + 0.014, if female { 0.307 } else { 0.28 }),
        ],
        Vec3::ZERO,
        CLOTH,
    )?;
    neckline(&mut b, chest, female)?;
    for side in [-1.0, 1.0] {
        b.bevel_box(
            [side * (chest * 0.5 - 0.038), 0.277, -0.002],
            [0.076, 0.142, 0.244],
            CLOTH,
            Quat::IDENTITY,
        )?;
    }
    b.bevel_box(
        [0.0, -0.065, 0.0],
        [waist + 0.025, 0.038, 0.272],
        [0.87, 0.82, 0.70],
        Quat::IDENTITY,
    )?;
    b.tapered(
        &[
            (-0.285, if female { 0.40 } else { 0.39 }, 0.245),
            (-0.19, if female { 0.43 } else { 0.40 }, 0.29),
            (-0.15, 0.39, 0.28),
            (-0.09, waist + 0.030, 0.26),
        ],
        Vec3::ZERO,
        TROUSERS,
    )?;
    b.voxel(
        [0.0, -0.12, 0.0],
        [waist + 0.040, 0.045, 0.283],
        [0.31, 0.23, 0.15],
    )?;
    for x in -3..=3 {
        b.voxel(
            [x as f32 * waist / 7.0, -0.119, -0.148],
            [0.008, 0.045, 0.008],
            CUFF,
        )?;
    }
    b.voxel([0.0, -0.12, -0.158], [0.041, 0.038, 0.028], CORD)?;
    b.rotated(
        [-0.018, -0.178, -0.159],
        [0.025, 0.091, 0.020],
        CORD,
        Quat::from_rotation_z(-0.12),
    )?;
    b.rotated(
        [0.027, -0.159, -0.160],
        [0.025, 0.065, 0.020],
        CORD,
        Quat::from_rotation_z(0.34),
    )?;
    b.finish()
}

fn neckline(b: &mut Builder, chest: f32, female: bool) -> Result<(), String> {
    // A continuous U-neck ribbon replaces disconnected blocks along the shirt opening.
    let profile = [
        (-1.0, 0.325),
        (-0.72, 0.325),
        (-0.58, 0.275),
        (-0.32, 0.24),
        (0.0, 0.218),
        (0.32, 0.24),
        (0.58, 0.275),
        (0.72, 0.325),
        (1.0, 0.325),
    ];
    let front = |x: f32, y: f32| {
        Vec3::new(
            x * (chest * 0.5 - 0.015),
            y,
            -(if female { 0.154 } else { 0.141 }) + (y - 0.205) * if female { 0.25 } else { 0.14 },
        )
    };
    for pair in profile.windows(2) {
        let a = front(pair[0].0, pair[0].1);
        let c = front(pair[1].0, pair[1].1);
        let low_a = front(pair[0].0, 0.201);
        let low_c = front(pair[1].0, 0.201);
        b.quad([low_c, low_a, a, c], CLOTH)?;
        b.quad(
            [a, a + Vec3::Z * 0.012, c + Vec3::Z * 0.012, c],
            [0.88, 0.83, 0.71],
        )?;
        let inner_a = a - Vec3::Y * 0.012 - Vec3::Z * 0.002;
        let inner_c = c - Vec3::Y * 0.012 - Vec3::Z * 0.002;
        b.quad(
            [inner_c, inner_a, a - Vec3::Z * 0.002, c - Vec3::Z * 0.002],
            [0.88, 0.83, 0.71],
        )?;
    }
    b.bevel_box(
        [0.0, 0.261, 0.126],
        [chest - 0.04, 0.129, 0.018],
        CLOTH,
        Quat::IDENTITY,
    )?;
    Ok(())
}

fn arm(sign: f32, female: bool) -> Result<Mesh, String> {
    let mut b = Builder::default();
    let width = if female { 0.115 } else { 0.145 };
    b.tapered(
        &[
            (ELBOW - 0.010, width * 0.76, width * 0.82),
            (-0.20, width * 0.90, width * 1.02),
            (-0.10, width * 1.10, width * 1.12),
            (0.015, width * 1.06, width * 1.15),
            (0.045, width * 0.71, width * 0.85),
        ],
        Vec3::ZERO,
        SKIN_LIGHT,
    )?;
    // A shallow deltoid facet gives the bare shoulder a rounded, articulated outline.
    b.tapered(
        &[
            (-0.06, width * 0.48, width * 0.85),
            (0.016, width * 0.62, width * 0.96),
            (0.042, width * 0.40, width * 0.68),
        ],
        Vec3::new(sign * width * 0.30, 0.0, 0.0),
        SKIN_LIGHT,
    )?;
    // Rounded overlap closes the hinge silhouette while the forearm rotates around its pivot.
    b.tapered(
        &[
            (-0.045, width * 0.46, width * 0.49),
            (-0.018, width * 0.81, width * 0.83),
            (0.018, width * 0.81, width * 0.83),
            (0.045, width * 0.46, width * 0.49),
        ],
        Vec3::new(0.0, ELBOW, 0.0),
        SKIN,
    )?;
    b.finish()
}

fn forearm(sign: f32, female: bool) -> Result<Mesh, String> {
    let mut b = Builder::default();
    let width = if female { 0.115 } else { 0.145 };
    b.tapered(
        &[
            (-0.48 - ELBOW, width * 0.65, width * 0.77),
            (-0.40 - ELBOW, width * 0.83, width * 0.95),
            (-0.325 - ELBOW, width * 0.96, width * 1.02),
            (0.012, width * 0.80, width * 0.82),
        ],
        Vec3::ZERO,
        SKIN,
    )?;
    b.tapered(
        &[
            (-0.555 - ELBOW, width * 0.75, 0.085),
            (-0.477 - ELBOW, width * 0.84, 0.095),
        ],
        Vec3::new(sign * 0.008, 0.0, -0.016),
        SKIN_LIGHT,
    )?;
    for finger in 0..4 {
        b.bevel_box(
            [
                sign * 0.008 + (finger as f32 - 1.5) * width * 0.18,
                -0.569 - ELBOW
                    + if finger == 0 || finger == 3 {
                        0.006
                    } else {
                        0.0
                    },
                -0.030,
            ],
            [width * 0.17, 0.044, 0.053],
            SKIN,
            Quat::from_rotation_x(0.20),
        )?;
        b.bevel_box(
            [
                sign * 0.008 + (finger as f32 - 1.5) * width * 0.18,
                -0.584 - ELBOW
                    + if finger == 0 || finger == 3 {
                        0.006
                    } else {
                        0.0
                    },
                -0.057,
            ],
            [width * 0.17, 0.025, 0.030],
            SKIN_LIGHT,
            Quat::from_rotation_x(0.85),
        )?;
    }
    b.bevel_box(
        [-sign * width * 0.45, -0.515 - ELBOW, -0.041],
        [0.035, 0.067, 0.042],
        SKIN_LIGHT,
        Quat::from_rotation_z(-sign * 0.28),
    )?;
    b.bevel_box(
        [-sign * width * 0.40, -0.542 - ELBOW, -0.064],
        [0.034, 0.031, 0.032],
        SKIN_LIGHT,
        Quat::from_rotation_y(sign * 0.30),
    )?;
    b.finish()
}

fn leg(sign: f32, female: bool) -> Result<Mesh, String> {
    let mut b = Builder::default();
    let width = if female { 0.177 } else { 0.202 };
    b.tapered(
        &[
            (KNEE - 0.012, width * 0.85, 0.195),
            (-0.25, width * 0.91, 0.215),
            (-0.14, width, 0.236),
            (-0.045, width * 1.05, 0.25),
            (0.015, width * 1.01, 0.245),
        ],
        Vec3::ZERO,
        TROUSERS,
    )?;
    b.tapered(
        &[
            (-0.105, width * 0.25, 0.050),
            (-0.070, width * 0.67, 0.145),
            (-0.035, width * 0.88, 0.190),
            (0.0, width * 0.94, 0.205),
            (0.025, width * 0.88, 0.195),
        ],
        Vec3::new(0.0, KNEE, 0.0),
        TROUSERS,
    )?;
    b.bevel_box(
        [sign * width * 0.45, -0.115, 0.02],
        [0.027, 0.135, 0.14],
        TROUSERS.map(|c| c * 0.94),
        Quat::IDENTITY,
    )?;
    b.finish()
}

fn calf(sign: f32, female: bool) -> Result<Mesh, String> {
    let mut b = Builder::default();
    let width = if female { 0.177 } else { 0.202 };
    b.tapered(
        &[
            (-0.435 - KNEE, width * 0.94, 0.202),
            (-0.40 - KNEE, width * 1.03, 0.222),
            (-0.36 - KNEE, width * 0.95, 0.212),
            (0.012, width * 0.89, 0.208),
        ],
        Vec3::ZERO,
        TROUSERS,
    )?;
    b.bevel_box(
        [0.0, -0.426 - KNEE, 0.0],
        [width + 0.021, 0.056, 0.237],
        CUFF,
        Quat::IDENTITY,
    )?;
    b.voxel(
        [0.0, -0.401 - KNEE, -0.124],
        [width + 0.024, 0.010, 0.012],
        [0.59, 0.49, 0.34],
    )?;
    b.tapered(
        &[
            (-0.604 - KNEE, width * 0.60, 0.126),
            (-0.51 - KNEE, width * 0.67, 0.142),
            (-0.48 - KNEE, width * 0.70, 0.147),
            (-0.445 - KNEE, width * 0.77, 0.16),
        ],
        Vec3::ZERO,
        SKIN_LIGHT,
    )?;
    b.tapered(
        &[
            (-0.657 - KNEE, width * 0.94, 0.24),
            (-0.613 - KNEE, width * 0.94, 0.24),
            (-0.577 - KNEE, width * 0.65, 0.155),
        ],
        Vec3::new(0.0, 0.0, -0.042),
        SKIN,
    )?;
    for toe in 0..4 {
        let x = (toe as f32 - 1.5) * width * 0.23;
        b.bevel_box(
            [x, -0.633 - KNEE, -0.161 - (toe as f32 * sign * 0.003)],
            [width * 0.22, 0.047, 0.052],
            SKIN_LIGHT,
            Quat::IDENTITY,
        )?;
    }
    b.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn rounded_joint_surfaces_extend_into_the_flexion_gap_not_just_flat_end_caps() {
        for female in [false, true] {
            for (mesh, pivot, depth) in [
                (arm(1.0, female).unwrap(), ELBOW, 0.030),
                (leg(1.0, female).unwrap(), KNEE, 0.085),
            ] {
                let Some(VertexAttributeValues::Float32x3(points)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("positions")
                };
                assert!(
                    points
                        .iter()
                        .any(|p| p[1] < pivot - 0.020 && p[1] > pivot - 0.055 && p[2] < -depth),
                    "The rounded upper-limb shell must fill the exposed front of a bent joint"
                );
            }
        }
    }

    #[test]
    fn reference_variants_share_ten_articulated_parts_with_real_triangle_budget() {
        let mut counts = Vec::new();
        for gender in [PlayerGender::Male, PlayerGender::Female] {
            let parts = parts(gender).unwrap();
            assert_eq!(parts.len(), 10);
            let mut vertices = 0;
            let mut triangles = 0;
            let mut colors = std::collections::HashSet::new();
            for (part, origin, mesh) in parts {
                let origin = origin
                    + match part {
                        AvatarPart::Forearm(side) => Vec3::new(
                            f32::from(side)
                                * if gender == PlayerGender::Female {
                                    0.235
                                } else {
                                    0.283
                                },
                            0.305,
                            0.0,
                        ),
                        AvatarPart::Calf(side) => Vec3::new(f32::from(side) * 0.115, -0.23, 0.0),
                        _ => Vec3::ZERO,
                    };
                let Some(VertexAttributeValues::Float32x3(points)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("positions")
                };
                let Some(VertexAttributeValues::Float32x3(normals)) =
                    mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                else {
                    panic!("normals")
                };
                let Some(VertexAttributeValues::Float32x4(tints)) =
                    mesh.attribute(Mesh::ATTRIBUTE_COLOR)
                else {
                    panic!("colors")
                };
                vertices += points.len();
                assert_eq!(points.len(), normals.len());
                assert_eq!(points.len(), tints.len());
                for point in points {
                    let p = Vec3::from_array(*point) + origin;
                    assert!(p.is_finite());
                    assert!(
                        p.x.abs() < 0.6 && p.z.abs() < 0.4 && p.y > -0.91 && p.y < 1.0,
                        "{gender:?}: {p:?}"
                    );
                }
                for normal in normals {
                    assert!((Vec3::from_array(*normal).length() - 1.0).abs() < 1e-5);
                }
                for color in tints {
                    assert!(
                        color
                            .iter()
                            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                    );
                    colors.insert(color.map(f32::to_bits));
                }
                let indices = mesh.indices().unwrap();
                triangles += indices.len() / 3;
                assert_eq!(indices.len() % 3, 0);
                assert!(indices.iter().all(|index| index < points.len()));
            }
            assert!(
                (20_000..=40_000).contains(&triangles),
                "{gender:?} must have 20k-40k triangles, not vertices: {triangles} triangles / {vertices} vertices"
            );
            assert!(colors.len() > 24);
            counts.push(vertices);
        }
        assert_ne!(counts[0], counts[1]);
    }

    #[test]
    fn short_neck_connects_head_and_torso_without_gap() {
        assert!((NECK_HEIGHT / 0.14 - 0.35).abs() < 1e-6);
        assert!((0.549_f32 - 0.17 - (0.33 + NECK_HEIGHT)).abs() < 1e-6);
    }
}
