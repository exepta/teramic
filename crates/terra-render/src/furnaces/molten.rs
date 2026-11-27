//! Shared, bounded discovery for visible heat, bubbling and the local soundscape.
use super::Geometry;
use bevy::prelude::*;
use std::collections::BTreeMap;
use terra_network::client::ReplicatedPlayers;
use terra_player::TerraCamera;
use terra_world::{
    edits::EditJournal,
    lava::{self, FUMAROLE, LAVA},
    streaming::{ChunkStore, WorldAccess},
};

#[derive(Resource, Default)]
pub struct VolcanicSources {
    pub surfaces: Vec<[i64; 3]>,
    pub(super) fumaroles: Vec<[i64; 3]>,
    geysers: Vec<[i64; 3]>,
    next: f32,
    scan_eye: Option<Vec3>,
    inspection_seconds: Option<f64>,
    continuous_from: f64,
    pub(super) seconds: f64,
}

#[derive(Message, Clone, Copy)]
pub enum VolcanicSound {
    Bubble(Vec3),
    Steam(Vec3),
}

const GEYSER: u16 = terra_voxel::MaterialId::GeyserVent as u16;
const RANGE: f32 = 42.;

pub(super) fn install(app: &mut App) {
    let inspection_seconds = std::env::args()
        .any(|arg| arg == "--smoke-test-benchmark")
        .then(|| {
            std::env::var("TERRA_BENCH_GEOTHERMAL_SECONDS")
                .ok()?
                .parse::<f64>()
                .ok()
        })
        .flatten()
        .filter(|t| t.is_finite() && *t >= 0.);
    app.insert_resource(VolcanicSources {
        inspection_seconds,
        ..default()
    })
    .add_message::<VolcanicSound>()
    .add_systems(Update, discover.after(terra_player::PlayerSet::Movement));
}

#[allow(clippy::too_many_arguments)]
fn discover(
    time: Res<Time>,
    real: Res<Time<Real>>,
    access: Res<WorldAccess>,
    chunks: Res<ChunkStore>,
    cameras: Query<&Transform, With<TerraCamera>>,
    mut sources: ResMut<VolcanicSources>,
    snapshot: Option<Res<ReplicatedPlayers>>,
    config: Res<terra_core::GameConfig>,
) {
    let seconds = snapshot
        .as_deref()
        .filter(|s| s.tick > 0)
        .map_or(time.elapsed_secs_f64(), |s| {
            s.tick as f64 / f64::from(config.server.tick_rate.max(1))
        });
    let seconds = sources.inspection_seconds.unwrap_or(seconds);
    if seconds < sources.seconds || real.delta_secs() >= 0.55 {
        sources.continuous_from = seconds;
    }
    sources.seconds = seconds;
    let Some(camera) = cameras.iter().next().filter(|_| access.ready()) else {
        sources.surfaces.clear();
        sources.fumaroles.clear();
        sources.geysers.clear();
        sources.scan_eye = None;
        return;
    };
    let eye = camera.translation;
    if sources
        .scan_eye
        .is_none_or(|last| last.distance_squared(eye) > (RANGE * 0.5).powi(2))
    {
        // A teleport changes every audible source at once. Start from the
        // newly arrived frame instead of replaying bursts that happened en route.
        sources.continuous_from = seconds;
    } else if time.elapsed_secs() < sources.next {
        if chunks.is_changed() {
            sources
                .surfaces
                .retain(|&cell| exposed(&chunks, cell, LAVA));
            sources
                .fumaroles
                .retain(|&cell| exposed(&chunks, cell, FUMAROLE));
            sources
                .geysers
                .retain(|&cell| exposed(&chunks, cell, GEYSER));
        }
        return;
    }
    sources.scan_eye = Some(eye);
    sources.next = time.elapsed_secs() + 0.65;
    let center = terra_voxel::global_to_section(eye.floor().as_i64vec3().to_array()).0;
    let mut patches = BTreeMap::<[i64; 3], (u32, [i64; 3])>::new();
    sources.fumaroles.clear();
    sources.geysers.clear();
    let distance = |p: [i64; 3]| {
        (Vec3::from_array(p.map(|v| v as f32)) + Vec3::splat(0.5)).distance_squared(eye)
    };
    for dz in -3..=3 {
        for dx in -3..=3 {
            for dy in -3..=3 {
                let Ok(owner) = center.offset([dx, dy, dz]) else {
                    continue;
                };
                let Some(section) = chunks.loaded.get(&owner).filter(|s| {
                    [LAVA, FUMAROLE, GEYSER]
                        .into_iter()
                        .any(|raw| s.data.may_contain(raw))
                }) else {
                    continue;
                };
                let Ok(origin) = owner.origin() else { continue };
                let low = Vec3::from_array(origin.map(|v| v as f32));
                if eye.distance_squared(eye.clamp(low, low + Vec3::splat(16.))) > RANGE * RANGE {
                    continue;
                }
                let pending = chunks.voxel_base_pending(owner);
                let blocks = section.data.blocks();
                for (i, &raw) in blocks.iter().enumerate() {
                    if !matches!(raw, LAVA | FUMAROLE | GEYSER) {
                        continue;
                    }
                    let y = i / 16 % 16;
                    if y < 15 && blocks[i + 16] != 0 {
                        continue;
                    }
                    let Ok(p) = owner.global_block([i % 16, y, i / 256]) else {
                        continue;
                    };
                    if distance(p) > RANGE * RANGE
                        || ((pending || y == 15) && !exposed(&chunks, p, raw))
                    {
                        continue;
                    }
                    if raw == GEYSER {
                        sources.geysers.push(p);
                    } else if raw == FUMAROLE {
                        sources.fumaroles.push(p);
                    } else {
                        let entry = patches
                            .entry(p.map(|k| k.div_euclid(6)))
                            .or_insert((u32::MAX, p));
                        let rank = seed(p);
                        if rank < entry.0 {
                            *entry = (rank, p);
                        }
                    }
                }
            }
        }
    }
    sources.surfaces = patches.into_values().map(|(_, p)| p).collect();
    sources
        .surfaces
        .sort_by(|a, b| distance(*a).total_cmp(&distance(*b)).then(a.cmp(b)));
    sources.surfaces.truncate(48);
    sources
        .fumaroles
        .sort_by(|a, b| distance(*a).total_cmp(&distance(*b)).then(a.cmp(b)));
    sources.fumaroles.truncate(16);
    sources
        .geysers
        .sort_by(|a, b| distance(*a).total_cmp(&distance(*b)).then(a.cmp(b)));
    sources.geysers.truncate(6);
}

pub(super) fn exposed(chunks: &ChunkStore, p: [i64; 3], raw: u16) -> bool {
    chunks.block_at(p) == Some(raw) && chunks.block_at([p[0], p[1] + 1, p[2]]) == Some(0)
}

fn seed([x, y, z]: [i64; 3]) -> u32 {
    let h = (x as u32).wrapping_mul(0x9e3779b1)
        ^ (y as u32).wrapping_mul(0x85ebca77)
        ^ (z as u32).wrapping_mul(0xc2b2ae3d);
    let h = (h ^ (h >> 16)).wrapping_mul(0x7feb352d);
    h ^ (h >> 15)
}

fn bubble_age(seconds: f64, key: u32) -> f32 {
    let period = 7.5 + f64::from(key % 8192) * (9. / 8192.);
    ((seconds + f64::from(key >> 16) * 0.011).rem_euclid(period)) as f32
}

use terra_world::volcano::geyser::{self, surge as geyser_surge};

impl VolcanicSources {
    pub(super) fn visible(&self) -> bool {
        !self.surfaces.is_empty() || !self.fumaroles.is_empty() || !self.geysers.is_empty()
    }

    pub(super) fn append_geysers(
        &self,
        vapor: &mut Geometry,
        chunks: &ChunkStore,
        journal: &EditJournal,
        right: Vec3,
        up: Vec3,
    ) {
        for &cell in &self.geysers {
            if !exposed(chunks, cell, GEYSER) {
                continue;
            }
            let clearance = (1..=30)
                .take_while(|dy| geyser::open(chunks, journal, [cell[0], cell[1] + dy, cell[2]]))
                .count() as f32;
            let origin = Vec3::from_array(cell.map(|n| n as f32)) + Vec3::new(0.5, 1.02, 0.5);
            let pressure = geyser_surge(self.seconds, cell);
            let height = (0.42 + pressure * geyser::HEIGHT)
                .min(clearance - 0.18)
                .max(0.);
            let t = geyser::phase(self.seconds, cell);
            let time = self.seconds.rem_euclid(3600.) as f32;
            // Rising water stays coherent from throat to crown. The green
            // inner strands overlap vertically; detached vapour and droplets
            // only take over around their expanding outer edge.
            if pressure > 0.12 {
                for strand in 0..8 {
                    let angle = strand as f32 * 2.39996;
                    for step in 0..12 {
                        let h = (step as f32 + 0.5) / 12.;
                        let lift = (h * 14. - time * 5. + angle).sin();
                        let reach = (0.45 + 0.75 * h * h)
                            * (0.72 + 0.4 * (strand as f32 * 1.3).sin())
                            * pressure;
                        let bend = angle + 0.28 * h + (h * 9. + time * 2. + angle).sin() * 0.16;
                        let center =
                            origin + Vec3::new(bend.cos() * reach, h * height, bend.sin() * reach);
                        let width = (0.44 + h * 0.60 + lift * 0.14) * pressure;
                        let half_height = (height / 12. * 1.65)
                            .min((center.y - origin.y + 0.05).max(0.1))
                            .min((origin.y + clearance - center.y - 0.12).max(0.1));
                        let crest = (h * h).mul_add(0.43, 0.35) + 0.08 * lift;
                        vapor.wisp(
                            center,
                            width,
                            right,
                            Vec3::Y * (half_height / width),
                            [
                                0.21 + crest * 0.58,
                                0.65 + crest * 0.33,
                                0.75 + crest * 0.22,
                                (0.77 - h * 0.20) * pressure,
                            ],
                        );
                    }
                }
            }
            let strands = if pressure > 0.04 { 104 } else { 8 };
            for i in 0..strands {
                let rise = (i as f32 / strands as f32 + time * 0.73).fract();
                let angle = i as f32 * 2.39996 + rise * 0.65 + (time + i as f32).sin() * 0.12;
                let envelope =
                    geyser::BASE_RADIUS + (geyser::CROWN_RADIUS - geyser::BASE_RADIUS) * rise;
                let radius =
                    envelope * (0.22 + 0.78 * ((i * 37 % 103) as f32 / 103.).sqrt()) * pressure;
                let center =
                    origin + Vec3::new(angle.cos() * radius, height * rise, angle.sin() * radius);
                let width = 0.09 + pressure * (0.24 + rise * 0.41);
                if center.y + width < origin.y + clearance {
                    vapor.wisp(
                        center,
                        width,
                        right,
                        up * (1.8 - rise * 0.65),
                        [
                            0.49 + rise * 0.33,
                            0.84 + rise * 0.12,
                            0.90,
                            0.65 - rise * 0.26,
                        ],
                    );
                }
            }
            if clearance < geyser::HEIGHT - 1. {
                continue;
            }
            for i in 0..geyser::DROPS {
                let Some(offset) = geyser::drop_position(cell, t, i) else {
                    continue;
                };
                let p = origin + Vec3::from_array(offset);
                if geyser::open(chunks, journal, p.floor().as_i64vec3().to_array()) {
                    vapor.wisp(
                        p,
                        0.075 + (i % 3) as f32 * 0.022,
                        right,
                        up * 2.5,
                        [0.49, 0.85, 0.91, 0.83],
                    );
                }
            }
        }
    }

    pub fn nearest_lava(&self) -> Option<Vec3> {
        self.surfaces
            .first()
            .map(|p| Vec3::from_array(p.map(|v| v as f32)) + Vec3::splat(0.5))
    }

    pub(super) fn append(
        &self,
        batch: &mut Geometry,
        chunks: &ChunkStore,
        journal: &EditJournal,
        previous: f64,
        sounds: &mut MessageWriter<VolcanicSound>,
    ) {
        // Suppress transients on teleports, reconnects and after suspended frames.
        let elapsed = self.seconds - previous;
        let continuous = previous >= self.continuous_from && elapsed > 0. && elapsed < 0.55;
        for &cell in &self.geysers {
            if continuous
                && exposed(chunks, cell, GEYSER)
                && geyser_surge(previous, cell) < 0.20
                && geyser_surge(self.seconds, cell) >= 0.20
            {
                sounds.write(VolcanicSound::Steam(
                    Vec3::from_array(cell.map(|n| n as f32)) + Vec3::new(0.5, 1., 0.5),
                ));
            }
        }
        for &cell in &self.fumaroles {
            if continuous
                && exposed(chunks, cell, FUMAROLE)
                && lava::plume_height(cell, previous) < 1.7
                && lava::plume_height(cell, self.seconds) >= 1.7
            {
                sounds.write(VolcanicSound::Steam(
                    Vec3::from_array(cell.map(|v| v as f32)) + Vec3::new(0.5, 1., 0.5),
                ));
            }
        }
        for &cell in &self.surfaces {
            let key = seed(cell);
            let age = bubble_age(self.seconds, key);
            if age > 0.96 || !exposed(chunks, cell, LAVA) {
                continue;
            }
            let x = cell[0] as f64 + 0.5;
            let z = cell[2] as f64 + 0.5;
            let origin = Vec3::new(
                x as f32,
                lava::height_at(chunks, journal, cell, x, z) as f32,
                z as f32,
            );
            let burst = 0.28;
            if age < burst {
                let t = age / burst;
                let radius = 0.045 + 0.088 * t;
                for (offset, size) in [(0., 1.), (0.045, 0.68)] {
                    batch.cuboid(
                        origin + Vec3::Y * (offset * t),
                        Vec3::new(radius * size, 0.044 * t, radius * size),
                        Quat::from_rotation_y((key % 20) as f32),
                        [2.0, 0.47, 0.045, 1.],
                    );
                }
                continue;
            }
            if continuous {
                let before = bubble_age(previous, key);
                if before < burst || before > age {
                    sounds.write(VolcanicSound::Bubble(origin));
                }
            }
            let t = age - burst;
            for i in 0..5 {
                let angle = (((key >> 8) % 360) as f32).to_radians() + i as f32 * 2.40;
                let speed = 0.38 + 0.23 * (i % 3) as f32;
                let lift = 1.8 + (i % 3) as f32 * 0.68;
                let height = (lift - 4.9 * t) * t;
                if height <= 0. {
                    continue;
                }
                let p =
                    origin + Vec3::new(angle.cos() * speed * t, height, angle.sin() * speed * t);
                let radius = (0.017 + 0.007 * (i % 3) as f32) * (1. - 0.65 * t);
                batch.cuboid(
                    p,
                    Vec3::new(radius, radius * 1.45, radius),
                    Quat::from_rotation_z(angle + t * 6.),
                    [3.2, 1.15 * (1. - t), 0.035, 1.],
                );
            }
        }
    }
}

#[cfg(test)]
mod tests;
