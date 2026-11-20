//! Small eyes retain their own pixel grid instead of being repainted by the
//! much coarser head sculpture. Open eyes and lids share an exact footprint.
use super::voxel::surface;
use bevy::prelude::*;
use std::collections::BTreeMap;

pub(super) const PUPIL: [f32; 3] = [0.035, 0.028, 0.022];
pub(super) const WHITE: [f32; 3] = [0.93, 0.90, 0.79];
const GLINT: [f32; 3] = [1.0, 0.98, 0.93];
const PIXELS: i16 = 12;

#[derive(Clone, Copy)]
pub(super) struct Eye {
    /// Center of the forward surface in the head's local coordinate space.
    pub center: Vec3,
    pub width: f32,
    /// Forward eyes face -Z; horse eyes face sideways, ±X.
    pub side: f32,
    pub skin: [f32; 3],
}

impl Eye {
    pub fn mesh(self, closed: bool) -> Result<Mesh, String> {
        let step = self.width / f32::from(PIXELS);
        let mut cells = BTreeMap::new();
        for x in 0..PIXELS {
            for y in 0..PIXELS {
                if (x == 0 || x == PIXELS - 1) && !(4..8).contains(&y)
                    || (x == 1 || x == PIXELS - 2) && !(2..10).contains(&y)
                    || (y == 0 || y == PIXELS - 1) && !(4..8).contains(&x)
                {
                    continue;
                }
                let color = if closed {
                    if y == 5 && (2..10).contains(&x) {
                        PUPIL
                    } else {
                        self.skin
                    }
                } else if (x == 2 || x == 9) && (5..7).contains(&y) {
                    // Only a small warm corner catches light. Broad white
                    // frames around a tiny pupil made peaceful animals stare.
                    WHITE
                } else if x == 4 && y == 8 {
                    GLINT
                } else if (2..10).contains(&x) && (2..10).contains(&y) {
                    PUPIL
                } else {
                    self.skin.map(|v| v * 0.55)
                };
                // The lid starts a full pixel in front of the eye. Thus no
                // coarse head rounding or coincident depth can leak a pupil.
                let z = if closed { -2 } else { 0 };
                cells.insert([x, y, z], color.map(f32::to_bits));
            }
        }
        let mut mesh = surface::mesh(
            &cells,
            Vec3::new(-self.width * 0.5, -self.width * 0.5, 0.0),
            step,
            Vec3::splat(self.width),
        )?;
        // Exact component permutation avoids a floating rotation introducing
        // tiny off-axis normals into these static voxel faces.
        use bevy::mesh::VertexAttributeValues;
        if let Some(VertexAttributeValues::Float32x3(points)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            for point in points {
                let [x, y, z] = *point;
                let local = if self.side == 0.0 {
                    Vec3::new(x, y, z)
                } else {
                    Vec3::new(-self.side * z, y, self.side * x)
                };
                *point = (self.center + local).to_array();
            }
        }
        if let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL)
        {
            for normal in normals {
                let [x, y, z] = *normal;
                if self.side != 0.0 {
                    *normal = [-self.side * z, y, self.side * x];
                }
            }
        }
        Ok(mesh)
    }
}

pub(super) fn attach(mut head: Mesh, eyes: &[Eye]) -> Result<(Mesh, Mesh), String> {
    let mut lids: Option<Mesh> = None;
    for &eye in eyes {
        let eye = outside_surface(&head, eye);
        head.merge(&eye.mesh(false)?)
            .map_err(|e| format!("Eye mesh: {e}"))?;
        let lid = eye.mesh(true)?;
        if let Some(mesh) = &mut lids {
            mesh.merge(&lid).map_err(|e| format!("Eyelid mesh: {e}"))?;
        } else {
            lids = Some(lid);
        }
    }
    Ok((head, lids.ok_or("Missing creature eyes")?))
}

/// Conservative body voxels can extend beyond their authoring strokes by a
/// grid cell. Place the eye against the emitted surface, not the guessed
/// stroke boundary. Lids receive the same correction and preserve their gap.
fn outside_surface(head: &Mesh, mut eye: Eye) -> Eye {
    use bevy::mesh::VertexAttributeValues;
    let Some(VertexAttributeValues::Float32x3(points)) = head.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return eye;
    };
    let Some(VertexAttributeValues::Float32x3(normals)) = head.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        return eye;
    };
    let axis = if eye.side == 0.0 { 2 } else { 0 };
    let sign = if eye.side == 0.0 { -1.0 } else { eye.side };
    let tangent = if axis == 2 { 0 } else { 2 };
    let radius = eye.width * 0.5;
    let original = eye.center[axis] * sign;
    let mut front = original - eye.width / f32::from(PIXELS);
    for (quad, normals) in points.chunks_exact(4).zip(normals.chunks_exact(4)) {
        if normals[0][axis] * sign < 0.99 {
            continue;
        }
        let low = quad
            .iter()
            .map(|p| Vec3::from_array(*p))
            .fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let high = quad
            .iter()
            .map(|p| Vec3::from_array(*p))
            .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        if high.y <= eye.center.y - radius
            || low.y >= eye.center.y + radius
            || high[tangent] <= eye.center[tangent] - radius
            || low[tangent] >= eye.center[tangent] + radius
        {
            continue;
        }
        front = front.max(quad[0][axis] * sign);
    }
    eye.center[axis] = sign * original.max(front + eye.width / f32::from(PIXELS));
    eye
}

#[cfg(test)]
mod tests;
