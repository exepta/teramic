//! Faceted reference avatars. Ten shared meshes articulate shoulders, elbows, hips and knees.
use super::AvatarPart;
use crate::meshing::builder::MeshBatch;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::collections::BTreeMap;
use terra_core::{CharacterAppearance, ClothingMaterial, PlayerGender};
mod garments;
#[path = "model/hair.rs"]
mod hair;

fn skin(appearance: CharacterAppearance) -> [f32; 3] {
    appearance.skin_color.map(|c| f32::from(c) / 255.0)
}
fn skin_light(appearance: CharacterAppearance) -> [f32; 3] {
    skin(appearance).map(|c| (c + 0.065).min(1.0))
}
const CORD: [f32; 3] = [0.72, 0.62, 0.44];
const NECK_HEIGHT: f32 = 0.049;
pub(super) const EYE_CENTER: f32 = 0.027;
pub(super) const EYE_HALF_HEIGHT: f32 = 0.047;
pub(crate) const ELBOW: f32 = -0.265;
pub(super) const KNEE: f32 = -0.31;
const FACET_SIZE: f32 = 0.020;

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
        // The cap is a planar convex octagon. A boundary fan needs six
        // triangles instead of eight and does not change its surface at all.
        for row in [0, 3] {
            for side in 1..7 {
                let next = side + 1;
                let (a, b) = if row == 0 { (side, next) } else { (next, side) };
                self.triangle([rings[row][0], rings[row][a], rings[row][b]], color)?;
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

    /// A shallow indexed surface facet shares its five vertices. Averaged
    /// edge normals preserve a soft carved surface without duplicate triangles.
    pub(crate) fn faceted_quad(
        &mut self,
        points: [Vec3; 4],
        center: Vec3,
        color: [f32; 3],
    ) -> Result<(), String> {
        let batch = self.0.entry(0).or_default();
        let index = u32::try_from(batch.positions.len()).map_err(|_| "Avatar vertex limit")?;
        let normals: [Vec3; 4] = std::array::from_fn(|i| {
            (points[(i + 1) % 4] - points[i])
                .cross(center - points[i])
                .normalize_or_zero()
        });
        batch.positions.extend(points.map(|p| p.to_array()));
        batch.positions.push(center.to_array());
        for i in 0..4 {
            batch.normals.push(
                (normals[i] + normals[(i + 3) % 4])
                    .normalize_or_zero()
                    .to_array(),
            );
        }
        batch.normals.push(
            normals
                .into_iter()
                .sum::<Vec3>()
                .normalize_or_zero()
                .to_array(),
        );
        batch
            .uvs
            .extend([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.5, 0.5]]);
        batch
            .colors
            .extend([Color::srgb_from_array(color).to_linear().to_f32_array(); 5]);
        for i in 0..4 {
            batch
                .indices
                .extend([index + i, index + (i + 1) % 4, index + 4]);
        }
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
        self.tapered_surface(rings, center, color, None)
    }

    fn garment(
        &mut self,
        rings: &[(f32, f32, f32)],
        center: Vec3,
        color: [f32; 3],
        material: ClothingMaterial,
    ) -> Result<(), String> {
        self.tapered_surface(rings, center, color, Some(material))
    }

    fn tapered_surface(
        &mut self,
        rings: &[(f32, f32, f32)],
        center: Vec3,
        color: [f32; 3],
        material: Option<ClothingMaterial>,
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
                                let envelope = (u * std::f32::consts::PI).sin()
                                    * (v * std::f32::consts::PI).sin();
                                let position = low[side]
                                    .lerp(low[next], u)
                                    .lerp(high[side].lerp(high[next], u), v);
                                // Every ring/corner keeps its shared boundary exactly closed.
                                let relief = envelope
                                    * material.map_or(0.002, |material| {
                                        garments::relief(material, position)
                                    });
                                position + normal * relief
                            };
                            let h = noise(row * 137 + column * 73 + side as u32 * 31);
                            self.quad(
                                [point(y, a), point(x, a), point(x, b), point(y, b)],
                                color.map(|c| {
                                    (c * material
                                        .map_or(0.98 + (h & 7) as f32 * 0.005, |material| {
                                            garments::shade(material, column, row, h)
                                        }))
                                    .min(1.0)
                                }),
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
    parts_with_appearance(gender, CharacterAppearance::default())
}

pub(super) fn parts_with_appearance(
    gender: PlayerGender,
    appearance: CharacterAppearance,
) -> Result<Vec<(AvatarPart, Vec3, Mesh)>, String> {
    let female = gender == PlayerGender::Female;
    let shoulder = if female { 0.235 } else { 0.283 };
    Ok(vec![
        (AvatarPart::Body, Vec3::ZERO, body(female, appearance)?),
        (
            AvatarPart::Head,
            Vec3::new(0.0, 0.549, 0.0),
            head(female, appearance)?,
        ),
        (
            AvatarPart::Arm(-1),
            Vec3::new(-shoulder, 0.305, 0.0),
            arm(-1.0, female, appearance)?,
        ),
        (
            AvatarPart::Arm(1),
            Vec3::new(shoulder, 0.305, 0.0),
            arm(1.0, female, appearance)?,
        ),
        (
            AvatarPart::Forearm(-1),
            Vec3::new(0.0, ELBOW, 0.0),
            forearm(-1.0, female, appearance)?,
        ),
        (
            AvatarPart::Forearm(1),
            Vec3::new(0.0, ELBOW, 0.0),
            forearm(1.0, female, appearance)?,
        ),
        (
            AvatarPart::Leg(-1),
            Vec3::new(-0.115, -0.23, 0.0),
            leg(-1.0, female, appearance)?,
        ),
        (
            AvatarPart::Leg(1),
            Vec3::new(0.115, -0.23, 0.0),
            leg(1.0, female, appearance)?,
        ),
        (
            AvatarPart::Calf(-1),
            Vec3::new(0.0, KNEE, 0.0),
            calf(-1.0, female, appearance)?,
        ),
        (
            AvatarPart::Calf(1),
            Vec3::new(0.0, KNEE, 0.0),
            calf(1.0, female, appearance)?,
        ),
    ])
}

pub(crate) fn head(female: bool, appearance: CharacterAppearance) -> Result<Mesh, String> {
    let mut builder = Builder::default();
    let width = if female { 0.345 } else { 0.36 };
    builder.tapered(
        &[
            (-0.17, width * 0.57, 0.21),
            (-0.148, width * 0.64, 0.245),
            (-0.120, width * 0.77, 0.28),
            (-0.060, width * 0.96, 0.319),
            (0.018, width, 0.324),
            (0.125, width * 0.96, 0.315),
            (0.181, width * 0.80, 0.27),
        ],
        Vec3::ZERO,
        skin(appearance),
    )?;
    for sign in [-1.0, 1.0] {
        builder.tapered(
            &[
                (-0.084, 0.038, 0.043),
                (-0.057, 0.061, 0.065),
                (0.014, 0.055, 0.058),
                (0.031, 0.035, 0.046),
            ],
            Vec3::new(sign * (width * 0.5 + 0.018), 0.0, 0.0),
            skin_light(appearance),
        )?;
        builder.voxel(
            [sign * (width * 0.5 + 0.039), -0.027, -0.013],
            [0.012, 0.041, 0.027],
            skin(appearance).map(|channel| channel * 0.76),
        )?;
        builder.face_plate(
            Vec3::new(sign * 0.085, EYE_CENTER, -0.166),
            Vec3::new(0.087, 0.083, 0.012),
            0.011,
            [0.95, 0.92, 0.83],
        )?;
        builder.face_plate(
            Vec3::new(sign * 0.079, EYE_CENTER - 0.001, -0.174),
            Vec3::new(0.054, 0.071, 0.011),
            0.007,
            if sign > 0.0 {
                appearance.resolved_right_eye_color()
            } else {
                appearance.eye_color
            }
            .map(|c| f32::from(c) / 255.0),
        )?;
        builder.face_plate(
            Vec3::new(sign * 0.077, EYE_CENTER + 0.003, -0.181),
            Vec3::new(0.032, 0.060, 0.007),
            0.004,
            [0.15, 0.10, 0.065],
        )?;
        builder.voxel(
            [sign * 0.082 - 0.010, EYE_CENTER + 0.023, -0.187],
            [0.013, 0.016, 0.005],
            [1.0, 0.97, 0.84],
        )?;
        builder.voxel(
            [sign * 0.077 + 0.009, EYE_CENTER - 0.014, -0.185],
            [0.005, 0.006, 0.005],
            [0.71, 0.52, 0.30],
        )?;
        builder.rotated(
            [sign * 0.085, 0.073, -0.174],
            [0.088, if female { 0.013 } else { 0.010 }, 0.016],
            [0.23, 0.13, 0.075],
            Quat::from_rotation_z(sign * 0.05),
        )?;
        builder.rotated(
            [sign * 0.085, 0.110, -0.168],
            [0.088, if female { 0.018 } else { 0.025 }, 0.018],
            appearance
                .hair_color
                .map(|c| (f32::from(c) / 255.0 * 0.68).max(0.055)),
            Quat::from_rotation_z(-sign * 0.065),
        )?;
        if female {
            builder.voxel(
                [sign * 0.131, 0.073, -0.174],
                [0.016, 0.009, 0.016],
                [0.23, 0.13, 0.075],
            )?;
        }
    }
    builder.bevel_box(
        [0.0, -0.030, -0.173],
        [0.028, 0.035, 0.025],
        skin_light(appearance),
        Quat::IDENTITY,
    )?;
    builder.bevel_box(
        [0.0, -0.041, -0.186],
        [0.036, 0.018, 0.012],
        skin(appearance).map(|channel| channel * 0.90),
        Quat::IDENTITY,
    )?;
    builder.voxel(
        [0.0, -0.106, -0.158],
        [0.068, 0.006, 0.009],
        [0.45, 0.24, 0.135],
    )?;
    for sign in [-1.0, 1.0] {
        builder.bevel_box(
            [sign * 0.124, -0.061, -0.155],
            [0.043, 0.014, 0.008],
            [
                skin(appearance)[0],
                skin(appearance)[1] * 0.94,
                skin(appearance)[2] * 0.92,
            ],
            Quat::IDENTITY,
        )?;
        builder.rotated(
            [sign * 0.041, -0.101, -0.158],
            [0.019, 0.006, 0.008],
            [0.45, 0.24, 0.135],
            Quat::from_rotation_z(sign * 0.35),
        )?;
    }
    hair::build(&mut builder, female, appearance)?;
    builder.finish()
}

pub(super) fn eyelids() -> Result<Mesh, String> {
    eyelids_for(CharacterAppearance::default())
}
pub(super) fn eyelids_for(appearance: CharacterAppearance) -> Result<Mesh, String> {
    let mut b = Builder::default();
    for side in [-1.0, 1.0] {
        b.voxel(
            [side * 0.085, 0.0, -0.192],
            [0.086, EYE_HALF_HEIGHT * 2.0, 0.012],
            skin(appearance),
        )?;
    }
    b.finish()
}

fn body(female: bool, appearance: CharacterAppearance) -> Result<Mesh, String> {
    let cloth = appearance.shirt_color.map(|c| f32::from(c) / 255.0);
    let trousers = appearance.pants_color.map(|c| f32::from(c) / 255.0);
    let cuff = trousers.map(|c| (c * 1.24 + 0.06).min(1.0));
    let mut builder = Builder::default();
    let waist = if female { 0.29 } else { 0.35 };
    let chest = if female { 0.35 } else { 0.43 };
    // The neck remains short; shoulders and head meet it without a floating gap.
    builder.voxel(
        [0.0, 0.33 + NECK_HEIGHT * 0.5, 0.0],
        [0.17, NECK_HEIGHT, 0.19],
        skin(appearance),
    )?;
    // Only the neckline is exposed; the opaque shirt replaces the hidden torso shell.
    builder.tapered(
        &[(0.195, chest, 0.272), (0.33, chest * 0.94, 0.22)],
        Vec3::ZERO,
        skin(appearance),
    )?;
    upper_torso_contours(&mut builder, appearance)?;
    builder.garment(
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
        cloth,
        appearance.shirt_material,
    )?;
    neckline(&mut builder, chest, female, cloth)?;
    for side in [-1.0, 1.0] {
        builder.bevel_box(
            [side * (chest * 0.5 - 0.038), 0.277, -0.002],
            [0.076, 0.142, 0.244],
            cloth,
            Quat::IDENTITY,
        )?;
    }
    // Raised seam bands follow the cloth silhouette without floating off the shoulders.
    for side in [-1.0, 1.0] {
        builder.rotated(
            [side * (chest * 0.5 - 0.038), 0.277, -0.127],
            [0.014, 0.136, 0.010],
            cloth.map(|channel| (channel * 0.86).min(1.0)),
            Quat::from_rotation_z(side * 0.08),
        )?;
    }
    builder.bevel_box(
        [0.0, -0.065, 0.0],
        [waist + 0.025, 0.038, 0.272],
        cloth.map(|c| (c * 1.06).min(1.0)),
        Quat::IDENTITY,
    )?;
    builder.garment(
        &[
            (-0.285, if female { 0.40 } else { 0.39 }, 0.245),
            (-0.19, if female { 0.43 } else { 0.40 }, 0.29),
            (-0.15, 0.39, 0.28),
            (-0.09, waist + 0.030, 0.26),
        ],
        Vec3::ZERO,
        trousers,
        appearance.pants_material,
    )?;
    builder.voxel(
        [0.0, -0.12, 0.0],
        [waist + 0.040, 0.045, 0.283],
        trousers.map(|c| c * 0.8),
    )?;
    for x in -3..=3 {
        builder.voxel(
            [x as f32 * waist / 7.0, -0.119, -0.148],
            [0.008, 0.045, 0.008],
            cuff,
        )?;
    }
    builder.voxel([0.0, -0.12, -0.158], [0.041, 0.038, 0.028], CORD)?;
    builder.rotated(
        [-0.018, -0.178, -0.159],
        [0.025, 0.091, 0.020],
        CORD,
        Quat::from_rotation_z(-0.12),
    )?;
    builder.rotated(
        [0.027, -0.159, -0.160],
        [0.025, 0.065, 0.020],
        CORD,
        Quat::from_rotation_z(0.34),
    )?;
    if appearance.tunic {
        builder.garment(
            &[
                (-0.27, waist + 0.10, 0.33),
                (-0.08, waist + 0.05, 0.30),
                (0.20, chest + 0.03, 0.33),
                (0.32, chest, 0.25),
            ],
            Vec3::ZERO,
            cloth,
            appearance.shirt_material,
        )?;
        builder.bevel_box(
            [0.0, -0.08, -0.005],
            [waist + 0.08, 0.045, 0.34],
            [0.25, 0.16, 0.10],
            Quat::IDENTITY,
        )?;
        builder.voxel(
            [0.0, -0.08, -0.18],
            [0.065, 0.052, 0.018],
            [0.83, 0.57, 0.16],
        )?;
    }
    if appearance.cape {
        garments::cape(&mut builder, appearance)?;
    }
    builder.finish()
}

/// Low relief meets the existing torso surface at its perimeter, without separate floating parts.
fn upper_torso_contours(
    builder: &mut Builder,
    appearance: CharacterAppearance,
) -> Result<(), String> {
    let depth = |y: f32| 0.5 * (0.272 + (0.22 - 0.272) * ((y - 0.195) / (0.33 - 0.195)));
    for side in [-1.0, 1.0] {
        for (back, center_y, half_width, half_height, relief) in [
            (false, 0.270, 0.044, 0.015, 0.0045),
            (true, 0.263, 0.034, 0.038, 0.0035),
        ] {
            let outward = if back { 1.0 } else { -1.0 };
            let center_x = side * 0.067;
            let surface = |x, y| Vec3::new(x, y, outward * (depth(y) + 0.0004));
            let corners = [
                surface(center_x - half_width, center_y - half_height),
                surface(center_x - half_width, center_y + half_height),
                surface(center_x + half_width, center_y + half_height),
                surface(center_x + half_width, center_y - half_height),
            ];
            builder.faceted_quad(
                if back {
                    [corners[3], corners[2], corners[1], corners[0]]
                } else {
                    corners
                },
                Vec3::new(center_x, center_y, outward * (depth(center_y) + relief)),
                skin(appearance).map(|channel| (channel * 1.018).min(1.0)),
            )?;
        }
    }
    Ok(())
}

fn neckline(b: &mut Builder, chest: f32, female: bool, cloth: [f32; 3]) -> Result<(), String> {
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
        b.quad([low_c, low_a, a, c], cloth)?;
        b.quad(
            [a, a + Vec3::Z * 0.012, c + Vec3::Z * 0.012, c],
            cloth.map(|c| (c * 1.06).min(1.0)),
        )?;
        let inner_a = a - Vec3::Y * 0.012 - Vec3::Z * 0.002;
        let inner_c = c - Vec3::Y * 0.012 - Vec3::Z * 0.002;
        b.quad(
            [inner_c, inner_a, a - Vec3::Z * 0.002, c - Vec3::Z * 0.002],
            cloth.map(|c| (c * 1.06).min(1.0)),
        )?;
    }
    b.bevel_box(
        [0.0, 0.261, 0.126],
        [chest - 0.04, 0.129, 0.018],
        cloth,
        Quat::IDENTITY,
    )?;
    Ok(())
}

fn arm(sign: f32, female: bool, appearance: CharacterAppearance) -> Result<Mesh, String> {
    let mut b = Builder::default();
    let width = if female { 0.128 } else { 0.160 };
    b.tapered(
        &[
            (ELBOW - 0.010, width * 0.76, width * 0.82),
            (-0.20, width * 0.90, width * 1.02),
            (-0.11, width * 1.04, width * 1.10),
            (-0.045, width * 1.13, width * 1.14),
            (0.010, width * 0.98, width * 1.04),
            (0.045, width * 0.63, width * 0.77),
            (0.060, width * 0.27, width * 0.42),
        ],
        Vec3::ZERO,
        skin_light(appearance),
    )?;
    // A shallow deltoid facet gives the bare shoulder a rounded, articulated outline.
    b.tapered(
        &[
            (-0.09, width * 0.34, width * 0.65),
            (-0.035, width * 0.54, width * 0.88),
            (0.012, width * 0.46, width * 0.80),
            (0.040, width * 0.18, width * 0.44),
        ],
        Vec3::new(sign * width * 0.30, 0.0, 0.0),
        skin_light(appearance),
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
        skin(appearance),
    )?;
    b.finish()
}

fn forearm(sign: f32, female: bool, appearance: CharacterAppearance) -> Result<Mesh, String> {
    let mut builder = Builder::default();
    let width = if female { 0.128 } else { 0.160 };
    builder.tapered(
        &[
            (-0.48 - ELBOW, width * 0.65, width * 0.77),
            (-0.40 - ELBOW, width * 0.83, width * 0.95),
            (-0.325 - ELBOW, width * 0.96, width * 1.02),
            (0.012, width * 0.80, width * 0.82),
        ],
        Vec3::ZERO,
        skin(appearance),
    )?;
    builder.tapered(
        &[
            (-0.555 - ELBOW, width * 0.75, 0.085),
            (-0.477 - ELBOW, width * 0.84, 0.095),
        ],
        Vec3::new(sign * 0.008, 0.0, -0.016),
        skin(appearance),
    )?;
    // Hands face the same way as the body: knuckles/nails to the front (-Z),
    // palm padding and relaxed finger curl to the rear (+Z). Thumb stays medial.
    builder.bevel_box(
        [sign * 0.008, -0.515 - ELBOW, 0.032],
        [width * 0.64, 0.061, 0.014],
        skin_light(appearance),
        Quat::IDENTITY,
    )?;
    for finger in 0..4 {
        builder.bevel_box(
            [
                sign * 0.008 + (finger as f32 - 1.5) * width * 0.18,
                -0.569 - ELBOW
                    + if finger == 0 || finger == 3 {
                        0.006
                    } else {
                        0.0
                    },
                -0.002,
            ],
            [width * 0.17, 0.044, 0.053],
            skin(appearance),
            Quat::from_rotation_x(-0.20),
        )?;
        builder.bevel_box(
            [
                sign * 0.008 + (finger as f32 - 1.5) * width * 0.18,
                -0.584 - ELBOW
                    + if finger == 0 || finger == 3 {
                        0.006
                    } else {
                        0.0
                    },
                0.025,
            ],
            [width * 0.17, 0.025, 0.030],
            skin_light(appearance),
            Quat::from_rotation_x(-0.85),
        )?;
        builder.bevel_box(
            [
                sign * 0.008 + (finger as f32 - 1.5) * width * 0.18,
                -0.575 - ELBOW
                    + if finger == 0 || finger == 3 {
                        0.006
                    } else {
                        0.0
                    },
                -0.026,
            ],
            [width * 0.115, 0.016, 0.005],
            skin(appearance).map(|channel| (channel * 0.85 + 0.12).min(1.0)),
            Quat::from_rotation_x(-0.20),
        )?;
    }
    builder.bevel_box(
        [-sign * width * 0.45, -0.515 - ELBOW, 0.009],
        [0.035, 0.067, 0.042],
        skin_light(appearance),
        Quat::from_rotation_z(-sign * 0.28),
    )?;
    builder.bevel_box(
        [-sign * width * 0.40, -0.542 - ELBOW, 0.032],
        [0.034, 0.031, 0.032],
        skin_light(appearance),
        Quat::from_rotation_y(-sign * 0.30),
    )?;
    builder.finish()
}

fn leg(sign: f32, female: bool, appearance: CharacterAppearance) -> Result<Mesh, String> {
    let trousers = appearance.pants_color.map(|c| f32::from(c) / 255.0);
    let mut b = Builder::default();
    let width = if female { 0.177 } else { 0.202 };
    b.garment(
        &[
            (KNEE - 0.012, width * 0.85, 0.195),
            (-0.25, width * 0.91, 0.215),
            (-0.14, width, 0.236),
            (-0.045, width * 1.05, 0.25),
            (0.015, width * 1.01, 0.245),
        ],
        Vec3::ZERO,
        trousers,
        appearance.pants_material,
    )?;
    b.garment(
        &[
            (-0.105, width * 0.25, 0.050),
            (-0.070, width * 0.67, 0.145),
            (-0.050, width * 0.80, 0.175),
            (-0.035, width * 0.88, 0.190),
            (0.0, width * 0.94, 0.205),
            (0.005, width * 0.93, 0.211),
            (0.025, width * 0.88, 0.195),
        ],
        Vec3::new(0.0, KNEE, -0.008),
        trousers,
        appearance.pants_material,
    )?;
    b.bevel_box(
        [sign * width * 0.45, -0.115, 0.02],
        [0.027, 0.135, 0.14],
        trousers.map(|c| c * 0.94),
        Quat::IDENTITY,
    )?;
    b.finish()
}

fn calf(sign: f32, female: bool, appearance: CharacterAppearance) -> Result<Mesh, String> {
    let trousers = appearance.pants_color.map(|c| f32::from(c) / 255.0);
    let mut b = Builder::default();
    let width = if female { 0.177 } else { 0.202 };
    b.garment(
        &[
            (-0.435 - KNEE, width * 0.94, 0.202),
            (-0.40 - KNEE, width * 1.03, 0.222),
            (-0.36 - KNEE, width * 0.95, 0.212),
            (0.012, width * 0.89, 0.208),
        ],
        Vec3::ZERO,
        trousers,
        appearance.pants_material,
    )?;
    b.bevel_box(
        [0.0, -0.426 - KNEE, 0.0],
        [width + 0.021, 0.056, 0.237],
        trousers.map(|c| (c * 1.24 + 0.06).min(1.0)),
        Quat::IDENTITY,
    )?;
    b.voxel(
        [0.0, -0.401 - KNEE, -0.124],
        [width + 0.024, 0.010, 0.012],
        trousers.map(|c| (c * 1.36 + 0.08).min(1.0)),
    )?;
    if appearance.boots {
        let leather = appearance.boots_color.map(|c| f32::from(c) / 255.0);
        b.garment(
            &[
                (-0.657 - KNEE, width * 1.04, 0.31),
                (-0.61 - KNEE, width * 1.04, 0.31),
                (-0.57 - KNEE, width * 0.88, 0.22),
                (-0.44 - KNEE, width * 0.95, 0.22),
            ],
            Vec3::new(0.0, 0.0, -0.035),
            leather,
            appearance.boots_material,
        )?;
        b.bevel_box(
            [0.0, -0.64 - KNEE, -0.035],
            [width * 1.08, 0.032, 0.32],
            leather.map(|c| c * 0.6),
            Quat::IDENTITY,
        )?;
        b.bevel_box(
            [0.0, -0.46 - KNEE, -0.02],
            [width * 1.04, 0.045, 0.26],
            leather.map(|c| (c * 1.4).min(1.0)),
            Quat::IDENTITY,
        )?;
        // Fitted instep straps and a side buckle read clearly in the reference pose.
        for y in [-0.54, -0.49] {
            b.bevel_box(
                [0.0, y - KNEE, -0.145],
                [width * 0.89, 0.022, 0.018],
                leather.map(|channel| channel * 0.72),
                Quat::IDENTITY,
            )?;
        }
        b.bevel_box(
            [sign * width * 0.44, -0.49 - KNEE, -0.143],
            [0.032, 0.033, 0.020],
            [0.70, 0.52, 0.28],
            Quat::IDENTITY,
        )?;
        return b.finish();
    }
    b.tapered(
        &[
            (-0.604 - KNEE, width * 0.51, 0.110),
            (-0.58 - KNEE, width * 0.56, 0.123),
            (-0.54 - KNEE, width * 0.71, 0.155),
            (-0.50 - KNEE, width * 0.73, 0.163),
            (-0.445 - KNEE, width * 0.77, 0.16),
        ],
        Vec3::new(0.0, 0.0, 0.007),
        skin(appearance),
    )?;
    // A shallow shin crest joins the tapered ankle and rounded rear calf volume.
    b.faceted_quad(
        [
            Vec3::new(-width * 0.14, -0.58 - KNEE, -0.0549),
            Vec3::new(-width * 0.14, -0.50 - KNEE, -0.0749),
            Vec3::new(width * 0.14, -0.50 - KNEE, -0.0749),
            Vec3::new(width * 0.14, -0.58 - KNEE, -0.0549),
        ],
        Vec3::new(0.0, -0.54 - KNEE, -0.0755),
        skin(appearance).map(|channel| (channel * 1.018).min(1.0)),
    )?;
    b.tapered(
        &[
            (-0.657 - KNEE, width * 0.94, 0.24),
            (-0.613 - KNEE, width * 0.94, 0.24),
            (-0.577 - KNEE, width * 0.65, 0.155),
        ],
        Vec3::new(0.0, 0.0, -0.042),
        skin(appearance),
    )?;
    for toe in 0..4 {
        let x = (toe as f32 - 1.5) * width * 0.23;
        b.bevel_box(
            [x, -0.633 - KNEE, -0.161 - (toe as f32 * sign * 0.003)],
            [width * 0.22, 0.047, 0.052],
            skin_light(appearance),
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
    fn body_landmarks_include_integrated_clavicles_shoulder_blades_kneecaps_and_shins() {
        let look = CharacterAppearance::default();
        for female in [false, true] {
            let body = body(female, look).unwrap();
            let Some(VertexAttributeValues::Float32x3(torso)) =
                body.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("positions")
            };
            for side in [-1.0, 1.0] {
                for (y, outward, relief) in [(0.270, -1.0, 0.0045), (0.263, 1.0, 0.0035)] {
                    let depth = 0.5 * (0.272 + (0.22 - 0.272) * ((y - 0.195) / (0.33 - 0.195)));
                    let landmark = Vec3::new(side * 0.067, y, outward * (depth + relief));
                    assert!(
                        torso
                            .iter()
                            .any(|point| Vec3::from_array(*point).abs_diff_eq(landmark, 1e-6))
                    );
                }
                let leg = leg(side, female, look).unwrap();
                let Some(VertexAttributeValues::Float32x3(knee)) =
                    leg.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("positions")
                };
                assert!(
                    knee.iter()
                        .any(|point| (point[1] - (KNEE + 0.005)).abs() < 1e-6 && point[2] < -0.113)
                );
                let calf = calf(side, female, look).unwrap();
                let Some(VertexAttributeValues::Float32x3(calf)) =
                    calf.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("positions")
                };
                assert!(
                    calf.iter()
                        .any(|point| (point[1] - (-0.54 - KNEE)).abs() < 1e-6 && point[2] > 0.084)
                );
                assert!(calf.iter().any(|point| {
                    Vec3::from_array(*point)
                        .abs_diff_eq(Vec3::new(0.0, -0.54 - KNEE, -0.0755), 1e-6)
                }));
            }
        }
    }

    #[test]
    fn garment_materials_change_real_surfaces_without_recoloring_heads_or_adding_parts() {
        let mut variants = Vec::new();
        for material in ClothingMaterial::ALL {
            let appearance = CharacterAppearance {
                shirt_material: material,
                pants_material: material,
                boots_material: material,
                cape_material: material,
                tunic: true,
                boots: true,
                cape: true,
                ..Default::default()
            };
            let parts = parts_with_appearance(PlayerGender::Female, appearance).unwrap();
            assert_eq!(parts.len(), 10);
            for (_, _, mesh) in &parts {
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
                assert!(points.iter().flatten().all(|value| value.is_finite()));
                assert!(
                    normals
                        .iter()
                        .all(|normal| (Vec3::from_array(*normal).length() - 1.0).abs() < 1e-5)
                );
                assert!(
                    mesh.indices()
                        .unwrap()
                        .iter()
                        .all(|index| index < points.len())
                );
            }
            variants.push(parts);
        }
        for pair in variants.windows(2) {
            for (left, right) in pair[0].iter().zip(&pair[1]) {
                if left.0 == AvatarPart::Head
                    || matches!(left.0, AvatarPart::Arm(_) | AvatarPart::Forearm(_))
                {
                    assert_eq!(
                        left.2.attribute(Mesh::ATTRIBUTE_POSITION),
                        right.2.attribute(Mesh::ATTRIBUTE_POSITION)
                    );
                    assert_eq!(
                        left.2.attribute(Mesh::ATTRIBUTE_COLOR),
                        right.2.attribute(Mesh::ATTRIBUTE_COLOR)
                    );
                } else {
                    assert_ne!(
                        left.2.attribute(Mesh::ATTRIBUTE_POSITION),
                        right.2.attribute(Mesh::ATTRIBUTE_POSITION)
                    );
                    assert_ne!(
                        left.2.attribute(Mesh::ATTRIBUTE_COLOR),
                        right.2.attribute(Mesh::ATTRIBUTE_COLOR)
                    );
                }
            }
        }
    }

    #[test]
    fn right_eye_tint_changes_only_the_right_iris_and_hands_have_nails_in_front() {
        let look = CharacterAppearance::default();
        let original = head(false, look).unwrap();
        let changed = head(
            false,
            CharacterAppearance {
                right_eye_color: Some([20, 140, 225]),
                ..look
            },
        )
        .unwrap();
        let Some(VertexAttributeValues::Float32x3(positions)) =
            original.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions")
        };
        let Some(VertexAttributeValues::Float32x4(before)) =
            original.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("colors")
        };
        let Some(VertexAttributeValues::Float32x4(after)) =
            changed.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("colors")
        };
        let changed_points: Vec<_> = positions
            .iter()
            .zip(before.iter().zip(after))
            .filter(|(_, (a, b))| a != b)
            .map(|(p, _)| p)
            .collect();
        assert!(changed_points.len() > 20);
        assert!(
            changed_points
                .iter()
                .all(|p| p[0] > 0.0 && p[2] < -0.16 && (p[1] - EYE_CENTER).abs() < 0.05)
        );
        for sign in [-1.0, 1.0] {
            let hand = forearm(sign, false, look).unwrap();
            let Some(VertexAttributeValues::Float32x3(positions)) =
                hand.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("positions")
            };
            let Some(VertexAttributeValues::Float32x4(colors)) =
                hand.attribute(Mesh::ATTRIBUTE_COLOR)
            else {
                panic!("colors")
            };
            let nail_color =
                Color::srgb_from_array(skin(look).map(|channel| (channel * 0.85 + 0.12).min(1.0)))
                    .to_linear()
                    .to_f32_array();
            let nails: Vec<_> = positions
                .iter()
                .zip(colors)
                .filter(|(_, color)| **color == nail_color)
                .map(|(p, _)| p)
                .collect();
            assert!(nails.len() > 20);
            assert!(nails.iter().all(|p| p[2] < -0.01 && p[1] < -0.28));
            let fingertips: Vec<_> = positions.iter().filter(|p| p[1] < -0.32).collect();
            assert!(!fingertips.is_empty());
            assert!(fingertips.iter().map(|p| p[2]).sum::<f32>() > 0.0);
        }
    }

    #[test]
    fn rounded_joint_surfaces_extend_into_the_flexion_gap_not_just_flat_end_caps() {
        for female in [false, true] {
            for (mesh, pivot, depth) in [
                (
                    arm(1.0, female, CharacterAppearance::default()).unwrap(),
                    ELBOW,
                    0.030,
                ),
                (
                    leg(1.0, female, CharacterAppearance::default()).unwrap(),
                    KNEE,
                    0.085,
                ),
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
    fn all_twenty_six_hairstyles_are_distinct_and_keep_skin_when_clothes_are_recolored() {
        let mut fingerprints = std::collections::BTreeSet::new();
        for gender in [PlayerGender::Male, PlayerGender::Female] {
            for hairstyle in 0..CharacterAppearance::hairstyle_count(gender) {
                let appearance = CharacterAppearance {
                    hairstyle,
                    ..default()
                };
                let meshes = parts_with_appearance(gender, appearance).unwrap();
                let (_, _, head) = meshes
                    .iter()
                    .find(|(part, _, _)| *part == AvatarPart::Head)
                    .unwrap();
                let Some(VertexAttributeValues::Float32x3(points)) =
                    head.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("head vertices")
                };
                let fingerprint = points
                    .iter()
                    .flatten()
                    .fold(0xcbf29ce484222325_u64, |hash, v| {
                        hash.wrapping_mul(0x100000001b3) ^ u64::from(v.to_bits())
                    });
                assert!(
                    fingerprints.insert(fingerprint),
                    "Repeated hairstyle: {gender:?} {hairstyle}"
                );
                assert!(
                    points
                        .iter()
                        .all(|point| Vec3::from_array(*point).is_finite())
                );
                let triangles = meshes
                    .iter()
                    .map(|(_, _, mesh)| mesh.indices().unwrap().len() / 3)
                    .sum::<usize>();
                assert!(
                    triangles < 50_000,
                    "{gender:?} hairstyle {hairstyle}: {triangles} triangles"
                );
            }
            let black = CharacterAppearance {
                hair_color: [4, 4, 4],
                shirt_color: [20, 70, 210],
                pants_color: [185, 30, 55],
                ..default()
            };
            let original = parts(gender).unwrap();
            let recolored = parts_with_appearance(gender, black).unwrap();
            for ((part, _, a), (_, _, b)) in original.iter().zip(&recolored) {
                if matches!(part, AvatarPart::Arm(_) | AvatarPart::Forearm(_)) {
                    assert_eq!(
                        a.attribute(Mesh::ATTRIBUTE_COLOR),
                        b.attribute(Mesh::ATTRIBUTE_COLOR)
                    );
                } else {
                    assert_ne!(
                        a.attribute(Mesh::ATTRIBUTE_COLOR),
                        b.attribute(Mesh::ATTRIBUTE_COLOR)
                    );
                }
            }
        }
        assert_eq!(fingerprints.len(), 26);
    }

    #[test]
    fn short_neck_connects_head_and_torso_without_gap() {
        assert!((NECK_HEIGHT / 0.14 - 0.35).abs() < 1e-6);
        assert!((0.549_f32 - 0.17 - (0.33 + NECK_HEIGHT)).abs() < 1e-6);
    }
}
