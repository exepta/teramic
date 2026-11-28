//! Shared, bounded discovery for visible heat, bubbling and the local soundscape.
use super::Geometry;
mod geyser_render;
use bevy::prelude::*;
pub(super) use geyser_render::setup as setup_geothermal;
use std::collections::BTreeMap;
use terra_network::client::ReplicatedPlayers;
use terra_player::TerraCamera;
use terra_world::{
    edits::EditJournal,
    lava::{self, FUMAROLE, LAVA},
    streaming::{ChunkStore, WorldAccess, WorldTerrain},
    volcano::geyser,
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
const FUMAROLE_HISS_HEIGHT: f64 = 0.12;

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
    .add_systems(Update, discover.after(terra_player::PlayerSet::Movement))
    .add_systems(
        PostUpdate,
        geyser_render::update
            .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
    );
}

#[allow(clippy::too_many_arguments)]
fn discover(
    time: Res<Time>,
    real: Res<Time<Real>>,
    access: Res<WorldAccess>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    terrain: Option<Res<WorldTerrain>>,
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
    let geyser_wet = |cell| {
        terrain
            .as_deref()
            .is_some_and(|terrain| geyser::submerged(&chunks, &journal, &terrain.0, cell))
    };
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
            sources.geysers.retain(|&cell| geyser_wet(cell));
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
                    if raw == LAVA && y < 15 && blocks[i + 16] != 0 {
                        continue;
                    }
                    let Ok(p) = owner.global_block([i % 16, y, i / 256]) else {
                        continue;
                    };
                    if distance(p) > RANGE * RANGE {
                        continue;
                    }
                    if raw == FUMAROLE {
                        if exposed(&chunks, p, FUMAROLE) {
                            sources.fumaroles.push(p);
                        }
                        continue;
                    }
                    if raw == GEYSER {
                        if geyser_wet(p) {
                            sources.geysers.push(p);
                        }
                    } else if (y < 15 && !pending) || exposed(&chunks, p, LAVA) {
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

fn fumarole_origin(cell: [i64; 3], block: impl Fn([i64; 3]) -> Option<u16>) -> Option<Vec3> {
    (block(cell) == Some(FUMAROLE) && block([cell[0], cell[1] + 1, cell[2]]) == Some(0))
        .then(|| Vec3::from_array(cell.map(|n| n as f32)) + Vec3::new(0.5, 1.008, 0.5))
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

impl VolcanicSources {
    pub(super) fn has_lava(&self) -> bool {
        !self.surfaces.is_empty()
    }

    pub(super) fn append_fumaroles(
        &self,
        vapor: &mut Geometry,
        (before, seconds): (Option<f64>, f64),
        wind: Vec2,
        (right, up): (Vec3, Vec3),
        block: impl Fn([i64; 3]) -> Option<u16>,
        mut hiss: impl FnMut(Vec3),
    ) {
        const WISPS: usize = 18;
        const STEPS: usize = 20;
        const STEM_HEIGHT: f32 = 0.55;
        let wind = Vec3::new(wind.x, 0., wind.y).clamp_length_max(6.);
        let ascent = (seconds * 0.78).fract() as f32;
        let swirl = (seconds * 0.59).rem_euclid(std::f64::consts::TAU) as f32;
        for &cell in &self.fumaroles {
            let pressure = lava::plume_height(cell, seconds);
            if pressure <= 0. {
                continue;
            }
            let Some(origin) = fumarole_origin(cell, &block) else {
                continue;
            };
            if pressure >= FUMAROLE_HISS_HEIGHT
                && before.is_some_and(|previous| {
                    previous < seconds
                        && seconds - previous < 0.55
                        && lava::plume_height(cell, previous) < FUMAROLE_HISS_HEIGHT
                })
            {
                hiss(origin);
            }
            let pressure = pressure as f32;
            let key = seed(cell);
            let theta = (key % 4096) as f32 * (std::f32::consts::TAU / 4096.);
            let tilt = Vec3::new(theta.cos(), 0., theta.sin())
                * [0., 0.2, 0.46, 0.85][(key as usize >> 12) & 3];
            let lift = pressure * 1.63;
            let strength = (pressure / 0.95).min(1.);
            let spread_at = |height: f32| {
                ((height - STEM_HEIGHT) / (lift - STEM_HEIGHT).max(0.001)).clamp(0., 1.)
            };
            let rise = |age: f32| {
                let height = lift * age;
                origin
                    + Vec3::Y * height
                    + (tilt * pressure + wind * 0.19) * spread_at(height).powf(1.8)
            };
            let open = |p: Vec3| block(p.floor().as_i64vec3().to_array()) == Some(0);
            let clearance = (1..=STEPS)
                .find(|step| !open(rise(*step as f32 / STEPS as f32)))
                .map_or(1., |step| (step - 1) as f32 / STEPS as f32);
            let stem = (lift * clearance).min(STEM_HEIGHT);
            if stem <= 0. {
                continue;
            }
            let root_radius = 0.085 + 0.018 * strength;
            vapor.wisp(
                origin + Vec3::Y * stem * 0.5,
                root_radius,
                right,
                Vec3::Y * (stem * 0.5 / root_radius),
                [0.79, 0.78, 0.75, strength * 0.64],
            );
            for i in 0..WISPS {
                let i = i as f32;
                let age = (ascent + i / WISPS as f32 + theta).fract();
                if age > clearance {
                    continue;
                }
                let phase = i * 2.39996 + theta + swirl;
                let expansion = spread_at(lift * age);
                let spread = 1.05 * expansion.powf(1.55) * (0.7 + pressure * 0.1);
                let p = rise(age)
                    + Vec3::new(phase.cos(), 0.12 * (phase * 1.4).sin(), phase.sin()) * spread;
                if !open(p) {
                    continue;
                }
                let radius = root_radius + expansion * 0.97;
                let vertical = (p.y - origin.y).clamp(0., radius) / radius;
                let fade =
                    (age * 19.).min(1.) * ((clearance - age) * 5.).min(1.) * (1. - age).powf(0.85);
                vapor.wisp(
                    p,
                    radius,
                    right,
                    up * vertical,
                    [
                        0.79,
                        0.78,
                        0.75,
                        fade * strength * (0.39 + pressure * 0.055),
                    ],
                );
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
