//! Cached rigid-part voxel sculptures. Authoring primitives never become entities.
//! Painting happens once per species/variant; only the greedy outer surface is uploaded.
use bevy::prelude::*;
use std::collections::BTreeMap;

pub(in crate::creatures::model) mod surface;
#[cfg(test)]
mod tests;

const PART_RESOLUTION: u16 = 24;
const MAX_AXIS_CELLS: i32 = 256;

#[derive(Clone, Copy)]
enum Volume {
    Box {
        center: Vec3,
        half: Vec3,
        inverse: Quat,
    },
    Rounded {
        center: Vec3,
        half: Vec3,
        radius: f32,
    },
    Ellipsoid {
        center: Vec3,
        radius: Vec3,
    },
    Triangle([Vec3; 3]),
}

struct Stroke {
    shape: Volume,
    low: Vec3,
    high: Vec3,
    tint: surface::Tint,
}

#[derive(Default)]
pub(in crate::creatures::model) struct Builder {
    strokes: Vec<Stroke>,
    cell_size: Option<f32>,
    resolution: Option<u16>,
}

impl Builder {
    /// Articulated thin appendages need fewer steps than a broad torso; retain
    /// their exact silhouette bounds without resampling every band at 24 cells.
    pub(in crate::creatures::model) fn with_resolution(resolution: u16) -> Self {
        Self {
            resolution: Some(resolution.max(4)),
            ..default()
        }
    }

    pub(in crate::creatures::model) fn with_cell_size(cell_size: f32) -> Self {
        Self {
            cell_size: Some(cell_size),
            ..default()
        }
    }

    fn push(&mut self, shape: Volume, low: Vec3, high: Vec3, tint: [f32; 3]) -> Result<(), String> {
        if !low.is_finite() || !high.is_finite() || tint.iter().any(|v| !v.is_finite()) {
            return Err("Non-finite creature sculpture".into());
        }
        self.strokes.push(Stroke {
            shape,
            low,
            high,
            tint: tint.map(f32::to_bits),
        });
        Ok(())
    }

    pub(in crate::creatures::model) fn voxel(
        &mut self,
        center: [f32; 3],
        size: [f32; 3],
        tint: [f32; 3],
    ) -> Result<(), String> {
        self.rotated(center, size, tint, Quat::IDENTITY)
    }

    pub(in crate::creatures::model) fn rotated(
        &mut self,
        center: [f32; 3],
        size: [f32; 3],
        tint: [f32; 3],
        rotation: Quat,
    ) -> Result<(), String> {
        let center = Vec3::from_array(center);
        let half = Vec3::from_array(size) * 0.5;
        if half.min_element() <= 0.0 || !rotation.is_finite() {
            return Err("Invalid creature sculpture box".into());
        }
        let extent = Mat3::from_quat(rotation).abs() * half;
        self.push(
            Volume::Box {
                center,
                half,
                inverse: rotation.inverse(),
            },
            center - extent,
            center + extent,
            tint,
        )
    }

    pub(in crate::creatures::model) fn rounded(
        &mut self,
        center: [f32; 3],
        size: [f32; 3],
        radius: f32,
        tint: [f32; 3],
    ) -> Result<(), String> {
        let center = Vec3::from_array(center);
        let half = Vec3::from_array(size) * 0.5;
        if half.min_element() <= 0.0 || !radius.is_finite() {
            return Err("Invalid creature sculpture outline".into());
        }
        self.push(
            Volume::Rounded {
                center,
                half,
                radius: radius.clamp(0.0, half.min_element()),
            },
            center - half,
            center + half,
            tint,
        )
    }

    pub(in crate::creatures::model) fn ellipsoid(
        &mut self,
        center: Vec3,
        radius: Vec3,
        tint: [f32; 3],
    ) -> Result<(), String> {
        if radius.min_element() <= 0.0 {
            return Err("Invalid creature sculpture radii".into());
        }
        self.push(
            Volume::Ellipsoid { center, radius },
            center - radius,
            center + radius,
            tint,
        )
    }

    pub(in crate::creatures::model) fn triangle(
        &mut self,
        points: [Vec3; 3],
        tint: [f32; 3],
    ) -> Result<(), String> {
        if (points[1] - points[0])
            .cross(points[2] - points[0])
            .length_squared()
            <= 1e-18
        {
            return Ok(());
        }
        let low = points
            .into_iter()
            .fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let high = points
            .into_iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        self.push(Volume::Triangle(points), low, high, tint)
    }

    pub(in crate::creatures::model) fn quad(
        &mut self,
        p: [Vec3; 4],
        tint: [f32; 3],
    ) -> Result<(), String> {
        self.triangle([p[0], p[1], p[2]], tint)?;
        self.triangle([p[0], p[2], p[3]], tint)
    }

    pub(in crate::creatures::model) fn finish(self) -> Result<Mesh, String> {
        self.finish_painted(|_, tint| tint)
    }

    /// Apply coat patterns to the occupied sculpture, never floating surface tiles.
    pub(in crate::creatures::model) fn finish_painted(
        self,
        paint: impl Fn(Vec3, [f32; 3]) -> [f32; 3],
    ) -> Result<Mesh, String> {
        if self.strokes.is_empty() {
            return Err("Empty creature sculpture".into());
        }
        let low = self
            .strokes
            .iter()
            .fold(Vec3::splat(f32::INFINITY), |p, s| p.min(s.low));
        let high = self
            .strokes
            .iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), |p, s| p.max(s.high));
        let step = self
            .cell_size
            .unwrap_or(
                (high - low).max_element() / f32::from(self.resolution.unwrap_or(PART_RESOLUTION)),
            )
            .max(0.001);
        if !step.is_finite() || ((high - low) / step).max_element() > MAX_AXIS_CELLS as f32 {
            return Err("Creature sculpture exceeds its fixed grid budget".into());
        }
        // Paper-thin wing faces still become a closed voxel layer. Full bodies
        // retain their exact authored outer bounds, including the paw ground plane.
        let maximum = high.max(low + Vec3::splat(step * 0.05));
        let dimensions = ((maximum - low) / step).ceil().as_ivec3().max(IVec3::ONE);
        let mut cells = BTreeMap::new();
        for stroke in self.strokes {
            let from = ((stroke.low - low) / step)
                .floor()
                .as_ivec3()
                .max(IVec3::ZERO)
                .min(dimensions - IVec3::ONE);
            let to = (((stroke.high - low) / step)
                .ceil()
                .as_ivec3()
                .max(from + IVec3::ONE))
            .min(dimensions);
            for x in from.x..to.x {
                for y in from.y..to.y {
                    for z in from.z..to.z {
                        let p = low
                            + (Vec3::new(x as f32, y as f32, z as f32) + Vec3::splat(0.5)) * step;
                        if stroke.shape.intersects(p, step * 0.5) {
                            cells.insert([x as i16, y as i16, z as i16], stroke.tint);
                        }
                    }
                }
            }
        }
        for (cell, tint) in &mut cells {
            let center = low + (Vec3::from_array(cell.map(f32::from)) + Vec3::splat(0.5)) * step;
            *tint = paint(center, tint.map(f32::from_bits)).map(f32::to_bits);
        }
        surface::mesh(&cells, low, step, maximum)
    }
}

impl Volume {
    fn intersects(&self, p: Vec3, h: f32) -> bool {
        match *self {
            Self::Box {
                center,
                half,
                inverse,
            } => {
                let margin = Mat3::from_quat(inverse).abs() * Vec3::splat(h);
                (inverse * (p - center)).abs().cmple(half + margin).all()
            }
            Self::Rounded {
                center,
                half,
                radius,
            } => {
                let q = ((p - center).abs() - Vec3::splat(h)).max(Vec3::ZERO);
                (q - (half - Vec3::splat(radius)))
                    .max(Vec3::ZERO)
                    .length_squared()
                    <= radius * radius
            }
            Self::Ellipsoid { center, radius } => {
                (((p - center).abs() - Vec3::splat(h)).max(Vec3::ZERO) / radius).length_squared()
                    <= 1.0
            }
            Self::Triangle(points) => triangle_cell(points.map(|v| v - p), h),
        }
    }
}

fn triangle_cell(points: [Vec3; 3], half: f32) -> bool {
    let separated = |axis: Vec3| {
        let projections = points.map(|p| p.dot(axis));
        let radius = half * axis.abs().element_sum();
        projections.into_iter().all(|v| v > radius + 1e-7)
            || projections.into_iter().all(|v| v < -radius - 1e-7)
    };
    let edges = [
        points[1] - points[0],
        points[2] - points[1],
        points[0] - points[2],
    ];
    ![Vec3::X, Vec3::Y, Vec3::Z].into_iter().any(separated)
        && !separated(edges[0].cross(edges[1]))
        && !edges.into_iter().any(|edge| {
            [Vec3::X, Vec3::Y, Vec3::Z]
                .into_iter()
                .any(|axis| separated(edge.cross(axis)))
        })
}
