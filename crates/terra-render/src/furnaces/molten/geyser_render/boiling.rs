use super::{Geometry, GeyserWorld, geyser};
use bevy::prelude::*;
use std::f32::consts::TAU;
use terra_world::water::FLUID_CAPACITY;

const SURFACE_LIFT: f32 = 0.04;

/// Samples only the connected pool above the vent and its immediate shoreline.
struct Pool {
    center: [i64; 2],
    heights: [[Option<f32>; 3]; 3],
}

impl Pool {
    fn around(world: &GeyserWorld, vent: [i64; 3]) -> Option<Self> {
        let [x, floor, z] = vent;
        let liquid = |p| {
            let raw = world.chunks.block_at(p)?;
            let material = world.journal.block(p).unwrap_or(raw);
            let fluid = world
                .chunks
                .fluid_cell_at(world.terrain, world.journal, p, material);
            (fluid.amount > 0).then_some(fluid)
        };
        let open = |p| geyser::open(world.chunks, world.journal, p);
        let mut top = None;
        for y in floor + 1..=floor + 24 {
            let Some(fluid) = liquid([x, y, z]) else {
                break;
            };
            top = Some((y, fluid));
            if fluid.amount < FLUID_CAPACITY {
                break;
            }
        }
        let (top, fluid) = top?;
        // A covered water column cannot show surface turbulence.
        if !open([x, top + 1, z]) || liquid([x, top + 1, z]).is_some() {
            return None;
        }
        let height = top as f32 + fluid.height() as f32;
        let mut heights = std::array::from_fn(|row| {
            std::array::from_fn(|col| {
                let x = x + col as i64 - 1;
                let z = z + row as i64 - 1;
                // Include a neighbouring shelf up to one water step lower.
                [top, top - 1].into_iter().find_map(|y| {
                    let fluid = liquid([x, y, z])?;
                    let surface = y as f32 + fluid.height() as f32;
                    ((surface - height).abs() <= 0.15
                        && open([x, y + 1, z])
                        && liquid([x, y + 1, z]).is_none())
                    .then_some(surface)
                })
            })
        });
        for (row, col) in [(0, 0), (0, 2), (2, 0), (2, 2)] {
            if heights[row][1].is_none() && heights[1][col].is_none() {
                heights[row][col] = None;
            }
        }
        Some(Self {
            center: [x, z],
            heights,
        })
    }

    fn at(&self, p: Vec2) -> Option<Vec3> {
        let [x, z] = self.center;
        let row = (p.y.floor() as i64 - z + 1) as usize;
        let col = (p.x.floor() as i64 - x + 1) as usize;
        let surface = *self.heights.get(row)?.get(col)?;
        Some(Vec3::new(p.x, surface? + SURFACE_LIFT, p.y))
    }
}

pub(super) fn append(
    vapor: &mut Geometry,
    world: &GeyserWorld,
    cell: [i64; 3],
    progress: f32,
    (right, up): (Vec3, Vec3),
) {
    let Some(pool) = Pool::around(world, cell) else {
        return;
    };
    let center = Vec2::new(cell[0] as f32 + 0.5, cell[2] as f32 + 0.5);
    let envelope = (progress * 3.).min(1.) * ((1. - progress) * 24.).min(1.);
    let tempo = progress * (2.4 + 1.8 * progress);

    // A compact ring remains visible even when a filled vent only has one water cell.
    for (i, growth) in [0.32, 0.95, 1.15].into_iter().enumerate() {
        let age = (tempo * 0.8 + i as f32 / 3.).fract();
        let radius = 0.12 + age * growth;
        let width = 0.038 + age * 0.025;
        let alpha = envelope * (age * 8.).min(1.) * (1. - age) * 0.78;
        ripple_ring(vapor, &pool, center, radius, width, alpha);
    }

    for i in 0..16 {
        let angle = i as f32 * 2.39996 + (tempo * 1.4 + i as f32).sin() * 0.09;
        let age = (tempo * (0.88 + (i % 5) as f32 * 0.11) + i as f32 * 0.617).fract();
        let distance = 0.19 + (i as f32 / 16.).sqrt() * (0.65 + 0.36 * progress);
        let offset = Vec2::from_angle(angle) * distance;
        let Some(surface) = pool.at(center + offset) else {
            continue;
        };
        let rise = (age * std::f32::consts::PI).sin();
        let radius = 0.055 + rise * 0.09;
        let top = surface + Vec3::Y * (0.02 + rise * (0.18 + 0.20 * progress));
        let cell = top.floor().as_i64vec3().to_array();
        if geyser::open(world.chunks, world.journal, cell) {
            vapor.wisp(
                top,
                radius,
                right,
                up,
                [0.63, 0.92, 0.95, 0.86 * rise * envelope],
            );
        }
    }
}

fn ripple_ring(
    vapor: &mut Geometry,
    pool: &Pool,
    center: Vec2,
    radius: f32,
    width: f32,
    alpha: f32,
) {
    const SEGMENTS: usize = 16;
    let spoke = |i| {
        let direction = Vec2::from_angle(i as f32 * TAU / SEGMENTS as f32);
        Some([
            pool.at(center + direction * (radius - width))?,
            pool.at(center + direction * radius)?,
            pool.at(center + direction * (radius + width))?,
        ])
    };
    let spokes: [_; SEGMENTS] = std::array::from_fn(spoke);
    for i in 0..SEGMENTS {
        let (Some(a), Some(b)) = (spokes[i], spokes[(i + 1) % SEGMENTS]) else {
            continue;
        };
        let base = vapor.positions.len() as u32;
        let middle = (a[1] + b[1]) * 0.5;
        if pool.at(middle.xz()).is_none() {
            continue;
        }
        vapor
            .positions
            .extend(a.into_iter().chain(b).map(|p| p.to_array()));
        vapor.normals.extend([[0., 1., 0.]; 6]);
        vapor.uvs.extend([[0.5, 0.5]; 6]);
        vapor
            .colors
            .extend([0., alpha, 0., 0., alpha, 0.].map(|a| [0.49, 0.87, 0.96, a]));
        vapor
            .indices
            .extend([0, 3, 4, 0, 4, 1, 1, 4, 5, 1, 5, 2].map(|i| i + base));
    }
}
