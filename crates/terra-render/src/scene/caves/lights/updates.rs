use super::*;
use bevy::ecs::{message::MessageCursor, system::SystemParam};
use std::collections::BTreeSet;
use terra_world::streaming::{SectionEditApplied, SectionPublished, SectionRemoved};

const MAX_PENDING: usize = 32_768;
const DISCOVERY_BUDGET: usize = 128;

#[derive(SystemParam)]
pub(super) struct Messages<'w> {
    published: Option<Res<'w, bevy::ecs::message::Messages<SectionPublished>>>,
    removed: Option<Res<'w, bevy::ecs::message::Messages<SectionRemoved>>>,
    edited: Option<Res<'w, bevy::ecs::message::Messages<SectionEditApplied>>>,
}

impl Messages<'_> {
    pub(super) fn available(&self) -> bool {
        self.published.is_some() && self.removed.is_some() && self.edited.is_some()
    }
}

#[derive(Default)]
pub(super) struct State {
    pub enabled: bool,
    published: MessageCursor<SectionPublished>,
    removed: MessageCursor<SectionRemoved>,
    edited: MessageCursor<SectionEditApplied>,
    pending: BTreeMap<SectionPos, bool>,
    overflow: bool,
    emitters_dirty: bool,
    initialized: bool,
    reach: Option<f32>,
    definitions: BTreeMap<u16, BlockLight>,
    #[cfg(test)]
    seeds: usize,
    #[cfg(test)]
    source_scans: usize,
}

impl State {
    pub(super) fn clear_pending(&mut self) {
        self.pending.clear();
        self.overflow = false;
        self.emitters_dirty = false;
        self.initialized = false;
    }
}

pub(super) fn clear(cache: &mut Lamps) {
    cache.sections.clear();
    cache.emitters.clear();
    cache.focus = None;
    cache.updates.clear_pending();
}

fn queue(
    pending: &mut BTreeMap<SectionPos, bool>,
    overflow: &mut bool,
    section: SectionPos,
    replace: bool,
) {
    if let Some(old) = pending.get_mut(&section) {
        *old |= replace;
    } else if pending.len() < MAX_PENDING {
        pending.insert(section, replace);
    } else {
        // Rare oversized bursts reconcile through the bounded legacy scanner;
        // dropping a queued edit silently would leave a permanent stale lamp.
        *overflow = true;
    }
}

fn queue_neighbors(
    pending: &mut BTreeMap<SectionPos, bool>,
    overflow: &mut bool,
    position: SectionPos,
    replace: bool,
) {
    queue(pending, overflow, position, replace);
    for delta in NEIGHBORS {
        if let Ok(neighbor) = position.offset(delta) {
            queue(pending, overflow, neighbor, false);
        }
    }
}

pub(super) fn collect(cache: &mut Lamps, chunks: &ChunkStore, messages: &Messages) -> bool {
    let State {
        published,
        removed,
        edited,
        pending,
        overflow,
        emitters_dirty,
        ..
    } = &mut cache.updates;
    let mut urgent = false;
    if let Some(messages) = &messages.published {
        for event in published.read(messages) {
            queue_neighbors(pending, overflow, event.position, true);
        }
    }
    if let Some(messages) = &messages.removed {
        for event in removed.read(messages) {
            // A remove followed by a reload in the same frame must keep the
            // current generation; actual loaded state wins over event order.
            if !chunks.loaded.contains_key(&event.position) {
                *emitters_dirty |= cache.sections.remove(&event.position).is_some();
            }
            queue_neighbors(pending, overflow, event.position, false);
            urgent = true;
        }
    }
    if let Some(messages) = &messages.edited {
        for event in edited.read(messages) {
            queue_neighbors(pending, overflow, event.position, false);
            urgent = true;
        }
    }
    urgent
}

fn epoch(section: SectionPos, chunks: &ChunkStore, journal: &EditJournal) -> u64 {
    journal
        .mesh_epoch(section)
        .wrapping_add(chunks.loaded[&section].data.revision().rotate_left(32))
        .wrapping_add(journal.prop_epoch(section).rotate_left(17))
}

fn distance(section: SectionPos, focus: Vec3) -> Option<f32> {
    let origin = section.origin().ok()?;
    Some(
        (Vec3::from_array(origin.map(|value| value as f32)) + Vec3::splat(8.0))
            .distance_squared(focus),
    )
}

fn same_light(a: &BlockLight, b: &BlockLight) -> bool {
    a.color == b.color
        && a.intensity == b.intensity
        && a.range == b.range
        && a.falloff == b.falloff
        && a.flicker == b.flicker
        && a.cave_intensity == b.cave_intensity
        && a.offset == b.offset
        && a.shadows == b.shadows
}

pub(super) fn refresh(
    cache: &mut Lamps,
    chunks: &ChunkStore,
    journal: &EditJournal,
    definitions: &BTreeMap<u16, BlockLight>,
    focus: Vec3,
    reach: f32,
) {
    let changed = definitions.len() != cache.updates.definitions.len()
        || definitions.iter().any(|(material, light)| {
            cache
                .updates
                .definitions
                .get(material)
                .is_none_or(|old| !same_light(old, light))
        });
    if changed {
        cache.sections.clear();
        cache.emitters.clear();
        cache.updates.pending.clear();
        cache.updates.definitions = definitions.clone();
        cache.updates.initialized = false;
    }
    let focus_cell = (focus / 16.0).floor().as_i64vec3().to_array();
    let limit = (reach * 1.75).powi(2);
    if !cache.updates.initialized
        || cache.focus != Some(focus_cell)
        || cache.updates.reach != Some(reach)
    {
        let _scope = terra_core::profiling::ScopeTimer::new("lights.discover_seed");
        #[cfg(test)]
        {
            cache.updates.seeds += 1;
        }
        cache.focus = Some(focus_cell);
        cache.updates.reach = Some(reach);
        cache.updates.initialized = true;
        for &position in chunks.loaded.keys() {
            if distance(position, focus).is_some_and(|distance| distance <= limit)
                && cache
                    .sections
                    .get(&position)
                    .is_none_or(|old| old.epoch != epoch(position, chunks, journal))
            {
                queue(
                    &mut cache.updates.pending,
                    &mut cache.updates.overflow,
                    position,
                    false,
                );
            }
        }
    }
    if cache.updates.overflow {
        discover(cache, chunks, journal, definitions, focus, reach);
        cache.updates.pending.clear();
        cache.updates.overflow = cache.dirty;
        cache.updates.emitters_dirty = false;
        return;
    }
    if cache.updates.pending.is_empty() && !cache.updates.emitters_dirty {
        return;
    }
    let _scope = terra_core::profiling::ScopeTimer::new("lights.discover");
    let mut immediate = Vec::new();
    let mut discoveries = Vec::new();
    let mut discard = Vec::new();
    let mut affected = BTreeSet::new();
    for (&position, &replace) in &cache.updates.pending {
        if !chunks.loaded.contains_key(&position) {
            discard.push(position);
            continue;
        }
        if cache.sections.contains_key(&position) {
            immediate.push((position, replace));
        } else if let Some(distance) =
            distance(position, focus).filter(|distance| *distance <= limit)
        {
            if journal.revision(position) > 0 {
                immediate.push((position, replace));
            } else {
                discoveries.push((position, replace, distance));
            }
        } else {
            discard.push(position);
        }
    }
    if discoveries.len() > DISCOVERY_BUDGET {
        discoveries.select_nth_unstable_by(DISCOVERY_BUDGET, |a, b| {
            a.2.total_cmp(&b.2).then(a.0.cmp(&b.0))
        });
        discoveries.truncate(DISCOVERY_BUDGET);
    }
    for position in discard {
        cache.updates.pending.remove(&position);
        cache.updates.emitters_dirty |= cache.sections.remove(&position).is_some();
        affected.insert(position);
    }
    for (position, replace) in immediate.into_iter().chain(
        discoveries
            .into_iter()
            .map(|(position, replace, _)| (position, replace)),
    ) {
        cache.updates.pending.remove(&position);
        let epoch = epoch(position, chunks, journal);
        if !replace
            && let Some(old) = cache
                .sections
                .get_mut(&position)
                .filter(|old| old.epoch == epoch)
        {
            if !old.sources.is_empty() {
                old.lamps = exposed_sources(&old.sources, chunks, journal);
                old.neighbors = neighbor_mask(position, chunks);
                cache.updates.emitters_dirty = true;
                affected.insert(position);
            }
            continue;
        }
        #[cfg(test)]
        {
            cache.updates.source_scans += 1;
        }
        if !refresh_section(cache, chunks, journal, definitions, position, epoch) {
            queue(
                &mut cache.updates.pending,
                &mut cache.updates.overflow,
                position,
                true,
            );
        }
        cache.updates.emitters_dirty = true;
        affected.insert(position);
    }
    if cache.updates.emitters_dirty {
        cache
            .emitters
            .retain(|(position, _)| !affected.contains(&global_to_section(*position).0));
        cache.emitters.extend(
            affected
                .iter()
                .filter_map(|position| cache.sections.get(position))
                .flat_map(|section| section.lamps.iter().copied()),
        );
        cache.updates.emitters_dirty = false;
    }
    cache.dirty = false;
}

#[cfg(test)]
#[path = "updates_tests.rs"]
mod tests;
