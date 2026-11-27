//! Skip torch cubemaps only in strong, verified open sunlight. Unknown space is sheltered.
use super::*;
use bevy::ecs::message::MessageCursor;
use std::{collections::HashSet, sync::LazyLock};
use terra_world::streaming::{
    PreparedColumnChanged, SectionEditApplied, SectionPublished, SectionRemoved, WorldTerrain,
};

const MIN_Y: i64 = -128;
const MAX_Y: i64 = 356;
const COLUMN_SECTIONS: usize = 31;
const MAX_COLUMNS: usize = 48;
const MAX_PROBES: usize = 4096;
const REFRESH_SECONDS: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visibility {
    Sunlit,
    Sheltered,
    Unknown,
}

struct Dependency {
    column: [i64; 2],
    revisions: [u64; COLUMN_SECTIONS],
}

impl Dependency {
    fn capture(column: [i64; 2], journal: &EditJournal) -> Self {
        Self {
            column,
            revisions: std::array::from_fn(|i| {
                journal.revision(SectionPos::new(column[0], i as i64 - 8, column[1]))
            }),
        }
    }

    fn changed(&self, journal: &EditJournal) -> bool {
        self.revisions.iter().enumerate().any(|(i, &revision)| {
            journal.revision(SectionPos::new(
                self.column[0],
                i as i64 - 8,
                self.column[1],
            )) != revision
        })
    }
}

struct Entry {
    origin: Vec3,
    checked: f32,
    visibility: Visibility,
    dependencies: Vec<Dependency>,
}

#[derive(Resource, Default)]
pub(super) struct State {
    entries: HashMap<[i64; 3], Entry>,
    published: MessageCursor<SectionPublished>,
    removed: MessageCursor<SectionRemoved>,
    edited: MessageCursor<SectionEditApplied>,
    prepared: MessageCursor<PreparedColumnChanged>,
    last_report: Option<(usize, usize, usize, bool)>,
}

impl State {
    pub(super) fn shadows(&self, source: [i64; 3]) -> bool {
        self.entries
            .get(&source)
            .is_none_or(|entry| entry.visibility != Visibility::Sunlit)
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct SunlightWorld<'w> {
    time: Res<'w, Time>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    terrain: Res<'w, WorldTerrain>,
    graphics: Option<Res<'w, GraphicsConfig>>,
    cave: Option<Res<'w, crate::scene::caves::CaveState>>,
    underwater: Option<Res<'w, crate::water::UnderwaterState>>,
    published: Option<Res<'w, Messages<SectionPublished>>>,
    removed: Option<Res<'w, Messages<SectionRemoved>>>,
    edited: Option<Res<'w, Messages<SectionEditApplied>>>,
    prepared: Option<Res<'w, Messages<PreparedColumnChanged>>>,
}

pub(super) fn update(
    world: SunlightWorld,
    lamps: Query<&VoxelLamp>,
    sun: Query<(&DirectionalLight, &Transform), With<crate::scene::daylight::Sun>>,
    mut state: ResMut<State>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("lights.shadow_visibility");
    let direction = sun
        .iter()
        .next()
        .and_then(|(light, transform)| sunlight(light.illuminance, *transform.back()))
        .filter(|_| {
            world.terrain.0.config().bounded_world
                && world
                    .cave
                    .as_ref()
                    .is_none_or(|cave| cave.blend <= 0.001 && cave.roof_coverage <= 0.001)
                && world
                    .underwater
                    .as_ref()
                    .is_none_or(|water| !water.submerged && water.blend <= 0.001)
        });
    let mut changed = HashSet::new();
    let mut edited = HashSet::new();
    if let Some(messages) = &world.published {
        changed.extend(
            state
                .published
                .read(messages)
                .map(|m| [m.position.x, m.position.z]),
        );
    }
    if let Some(messages) = &world.removed {
        changed.extend(
            state
                .removed
                .read(messages)
                .map(|m| [m.position.x, m.position.z]),
        );
    }
    if let Some(messages) = &world.edited {
        // Fluid, prop and appearance deltas share this message; only changed
        // terrain revisions invalidate rays through an otherwise stable column.
        edited.extend(
            state
                .edited
                .read(messages)
                .map(|m| [m.position.x, m.position.z]),
        );
    }
    if let Some(messages) = &world.prepared {
        changed.extend(state.prepared.read(messages).map(|m| m.coordinate));
    }
    let untracked = world.chunks.is_changed()
        && (world.published.is_none() || world.removed.is_none() || world.prepared.is_none());
    let budget = match world
        .graphics
        .as_ref()
        .map_or(CaveQuality::Medium, |g| g.cave_quality)
    {
        CaveQuality::Low => 4,
        CaveQuality::Medium => 8,
        CaveQuality::High => 12,
        CaveQuality::Ultra => 16,
    };
    let mut sources: Vec<_> = lamps
        .iter()
        .filter(|lamp| {
            lamp.1.shadows
                && world
                    .chunks
                    .block_at(lamp.0)
                    .map(|raw| world.journal.block(lamp.0).unwrap_or(raw))
                    .is_some_and(terra_core::block_shapes::is_torch)
        })
        .map(|lamp| (lamp.0, lamp.2))
        .collect();
    sources.sort_unstable_by_key(|(position, _)| *position);
    sources.truncate(budget);
    state
        .entries
        .retain(|source, _| direction.is_some() && sources.iter().any(|(p, _)| p == source));
    let now = world.time.elapsed_secs();
    let mut probes = 0;
    let mut cells = 0;
    if let Some(direction) = direction {
        for (owner, origin) in sources {
            if state.entries.get(&owner).is_some_and(|entry| {
                !untracked
                    && !world.terrain.is_changed()
                    && entry.origin.distance_squared(origin) < 0.0001
                    && now >= entry.checked
                    && now - entry.checked < REFRESH_SECONDS
                    && entry.dependencies.iter().all(|dependency| {
                        !changed.contains(&dependency.column)
                            && (!(world.journal.is_changed()
                                || edited.contains(&dependency.column))
                                || !dependency.changed(&world.journal))
                    })
            }) {
                continue;
            }
            let _probe_scope = terra_core::profiling::ScopeTimer::new("lights.shadow_probe");
            let mut columns = Columns {
                chunks: &world.chunks,
                journal: &world.journal,
                bounds: HashMap::new(),
                dependencies: Vec::new(),
                probes: 0,
            };
            let visibility = classify(owner, origin, direction, |p| columns.read(p));
            probes += 1;
            cells += columns.probes;
            state.entries.insert(
                owner,
                Entry {
                    origin,
                    checked: now,
                    visibility,
                    dependencies: columns.dependencies,
                },
            );
        }
    }
    let sunlit = state
        .entries
        .values()
        .filter(|e| e.visibility == Visibility::Sunlit)
        .count();
    let sheltered = state
        .entries
        .values()
        .filter(|e| e.visibility == Visibility::Sheltered)
        .count();
    let unknown = state
        .entries
        .values()
        .filter(|e| e.visibility == Visibility::Unknown)
        .count();
    let report = (sunlit, sheltered, unknown, direction.is_some());
    static DIAGNOSTICS: LazyLock<bool> = LazyLock::new(|| {
        std::env::var("TERRA_LIGHT_DIAGNOSTICS").is_ok_and(|v| matches!(v.as_str(), "1" | "true"))
    });
    if *DIAGNOSTICS && state.last_report != Some(report) {
        info!(
            omitted_torch_cubemaps = sunlit,
            sheltered_torches = sheltered,
            uncertain_torches = unknown,
            strong_sun = direction.is_some(),
            probes,
            cells,
            "Torch shadow visibility (other light properties unchanged)"
        );
    }
    state.last_report = Some(report);
}

fn sunlight(illuminance: f32, direction: Vec3) -> Option<Vec3> {
    // Dusk, night, weak/cloud-attenuated sunlight and invalid light poses retain cubemaps.
    (illuminance.is_finite()
        && illuminance >= 60_000.0
        && direction.is_finite()
        && direction.length_squared() > 0.99
        && direction.y >= 0.65)
        .then(|| direction.normalize())
}

struct Columns<'a> {
    chunks: &'a ChunkStore,
    journal: &'a EditJournal,
    bounds: HashMap<[i64; 2], f64>,
    dependencies: Vec<Dependency>,
    probes: usize,
}

impl Columns<'_> {
    fn read(&mut self, p: [i64; 3]) -> Option<u16> {
        self.probes += 1;
        if self.probes > MAX_PROBES || p[1] < MIN_Y {
            return None;
        }
        if p[1] > MAX_Y {
            return Some(0);
        }
        let coordinate = [p[0].div_euclid(16), p[2].div_euclid(16)];
        let top = match self.bounds.get(&coordinate) {
            Some(top) => *top,
            None => {
                if self.bounds.len() >= MAX_COLUMNS {
                    return None;
                }
                let prepared = self.chunks.prepared_column(coordinate)?;
                let top = known_top(
                    self.chunks,
                    self.journal,
                    coordinate,
                    prepared.maximum_height(),
                )?;
                self.dependencies
                    .push(Dependency::capture(coordinate, self.journal));
                self.bounds.insert(coordinate, top);
                top
            }
        };
        if p[1] as f64 > top {
            return Some(0);
        }
        self.chunks
            .block_at(p)
            .map(|raw| self.journal.block(p).unwrap_or(raw))
    }
}

pub(in crate::scene::caves) fn known_top(
    chunks: &ChunkStore,
    journal: &EditJournal,
    column: [i64; 2],
    natural: f64,
) -> Option<f64> {
    if !natural.is_finite() || natural > MAX_Y as f64 + 16.0 {
        return None;
    }
    let mut top = natural.ceil();
    // O(31) indexed lookups, never scan historical edits or section voxel arrays.
    for y in -8..=22 {
        let section = SectionPos::new(column[0], y, column[1]);
        if journal.revision(section) > 0
            || chunks
                .loaded
                .get(&section)
                .is_some_and(|s| !s.data.is_empty())
        {
            top = top.max((y * 16 + 15).min(MAX_Y) as f64);
        }
    }
    Some(top)
}

fn classify(
    owner: [i64; 3],
    origin: Vec3,
    direction: Vec3,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> Visibility {
    if !origin.is_finite() || origin.y < MIN_Y as f32 || origin.y > MAX_Y as f32 {
        return Visibility::Unknown;
    }
    // Roof coverage is separate from the sun ray: a doorway or window must not
    // misclassify a torch inside a building as an outdoor source. The small
    // footprint also covers the existing flame-position flicker.
    for offset in [
        Vec3::ZERO,
        Vec3::X * 0.2,
        Vec3::NEG_X * 0.2,
        Vec3::Z * 0.2,
        Vec3::NEG_Z * 0.2,
    ] {
        let result = ray(owner, origin + offset, Vec3::Y, &mut block);
        if result != Visibility::Sunlit {
            return result;
        }
    }
    ray(owner, origin, direction, block)
}

fn ray(
    owner: [i64; 3],
    origin: Vec3,
    direction: Vec3,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> Visibility {
    let mut start = origin.to_array().map(f64::from);
    let direction = direction.to_array().map(f64::from);
    if direction[1] < 0.65 || !direction.iter().all(|v| v.is_finite()) {
        return Visibility::Unknown;
    }
    // Reuse the existing DDA's 128 m contract in bounded segments to the top of
    // bounded generation. Prepared air above known live roofs needs no block load.
    while start[1] <= MAX_Y as f64 + 1.0 {
        let reach = ((MAX_Y as f64 + 2.0 - start[1]) / direction[1]).min(128.0);
        let mut unknown = false;
        let hit = terra_voxel::blocks::raycast_blocks(start, direction, reach, |p| {
            let Some(material) = block(p) else {
                unknown = true;
                return Some(1);
            };
            Some(u16::from(material != 0 && !(p == owner && material == 40)))
        });
        if unknown {
            return Visibility::Unknown;
        }
        if hit.is_some() {
            return Visibility::Sheltered;
        }
        start = std::array::from_fn(|i| start[i] + direction[i] * reach);
    }
    Visibility::Sunlit
}

#[cfg(test)]
mod tests;
