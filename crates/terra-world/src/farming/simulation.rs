use super::ForageRegrowth;
use crate::{
    edits::EditJournal,
    props::{PropId, WorldProp, placement},
    streaming::{BlockEditRequest, ChunkStore, WorldAccess, WorldTerrain, commit_block_edit},
};
use bevy::prelude::*;
use std::collections::{BTreeSet, HashMap};
use terra_core::{
    WorldClock, WorldWeather,
    block_properties::BlockPropertiesCatalog,
    farming::{CropKind, CropState, FARMLAND},
};
use terra_voxel::{MaterialId, SectionPos, global_to_section};

const BUDGET: usize = 128;

/// Uses the same bounded voxel traversal on client and authority; solid walls and
/// unloaded sections prevent refilling through geometry.
pub fn water_target(
    chunks: &ChunkStore,
    journal: &EditJournal,
    origin: [f64; 3],
    direction: [f64; 3],
) -> Option<[i64; 3]> {
    let wet = |position, material| {
        if material == terra_core::plumbing::FAUCET {
            return crate::plumbing::tap_available(chunks, journal, position, 1);
        }
        if !crate::water::can_hold_fluid(material) {
            return false;
        }
        journal.fluid(position).map_or_else(
            || {
                material == MaterialId::FluidWater as u16
                    || chunks
                        .surface_column_at(position[0], position[2])
                        .is_some_and(|column| column.water_columns().at(position[1]).is_some())
            },
            |cell| cell.source && cell.amount > 0,
        )
    };
    let hit = terra_voxel::blocks::raycast_blocks(origin, direction, 8.0, |position| {
        let material = chunks.block_at(position)?;
        Some(if wet(position, material) {
            MaterialId::FluidWater as u16
        } else if material == MaterialId::FluidWater as u16 {
            MaterialId::Air as u16
        } else {
            material
        })
    })?;
    wet(hit.position, chunks.block_at(hit.position)?).then_some(hit.position)
}

#[derive(Default)]
pub(crate) struct FarmWork {
    next: f64,
    crop_cursor: usize,
    forage_cursor: usize,
    crop_visited: HashMap<[i64; 3], f64>,
    forage_visited: HashMap<PropId, f64>,
}

pub fn planting_environment_valid(chunks: &ChunkStore, support: [i64; 3], kind: CropKind) -> bool {
    matches!(chunks.block_at(support), Some(material) if material == FARMLAND || material == MaterialId::Dirt as u16 || material == MaterialId::Grass as u16)
        && (1..=if kind == CropKind::Lettuce { 1 } else { 2 }).all(|dy| {
            chunks.block_at([support[0], support[1] + dy, support[2]])
                == Some(MaterialId::Air as u16)
        })
}

/// Validates capacity before changing soil; sowing uses the existing prop journal and placement.
pub fn plant_crop(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    properties: &BlockPropertiesCatalog,
    mut prop: WorldProp,
) -> Result<bool, String> {
    let Some(kind) = prop.kind.crop() else {
        return Ok(false);
    };
    let support = placement::support(&prop);
    prop.id = super::crop_id(support, kind);
    prop.variant = 0;
    prop.scale = 1.0;
    placement::align(&mut prop, support, 0);
    if !journal.can_place_crop_prop(&prop)
        || !planting_environment_valid(chunks, support, kind)
        || chunks.loaded.get(&prop.section).is_none_or(|section| {
            section
                .props
                .iter()
                .any(|existing| !existing.removed && placement::support(existing) == support)
                || placement::next_slot(&section.props, support, prop.kind).is_none()
        })
    {
        return Ok(false);
    }
    let expected = chunks.block_at(support).ok_or("Missing crop support")?;
    if expected != FARMLAND
        && !commit_block_edit(
            chunks,
            journal,
            &BlockEditRequest {
                position: support,
                expected_material: expected,
                material: FARMLAND,
            },
            properties,
        )?
    {
        return Ok(false);
    }
    // Pin validated generated/prepared soil once before persistent crops reference it.
    if expected == FARMLAND
        && journal.block(support) != Some(FARMLAND)
        && !crate::streaming::ensure_block_record(chunks, journal, support, FARMLAND)?
    {
        return Ok(false);
    }
    if !journal.place_crop_prop(prop)? {
        return Ok(false);
    }
    if let Some(section) = chunks.loaded.get_mut(&prop.section) {
        if let Some(previous) = section
            .props
            .iter_mut()
            .find(|previous| previous.id == prop.id)
        {
            *previous = prop;
        } else {
            section.props.push(prop);
        }
        journal.apply_prop_removals(prop.section, &mut section.props);
    }
    journal.set_crop(support, CropState::new(kind, prop.id.0))?;
    Ok(true)
}

pub fn commit_crop_state(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    support: [i64; 3],
    state: CropState,
) -> Result<bool, String> {
    let owner = global_to_section(support).0;
    let epoch = journal.prop_epoch(owner);
    let changed = journal.advance_crop(support, state)?;
    if journal.prop_epoch(owner) != epoch
        && let Some(section) = chunks.loaded.get_mut(&owner)
    {
        journal.apply_prop_removals(owner, &mut section.props);
    }
    Ok(changed)
}

/// A completed nectar visit benefits at most four real, moist crops within three metres.
pub(crate) fn pollinate_near(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    flower: [f64; 3],
) -> Result<bool, String> {
    let owner = global_to_section(flower.map(|v| v.floor() as i64)).0;
    let mut nearby = Vec::new();
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                let Ok(section) = owner.offset([dx, dy, dz]) else {
                    continue;
                };
                if let Some(section) = chunks.loaded.get(&section) {
                    for prop in &section.props {
                        let support = placement::support(prop);
                        let distance = (0..3)
                            .map(|i| (prop.position[i] - flower[i]).powi(2))
                            .sum::<f64>();
                        if prop.kind.crop().is_some()
                            && !prop.removed
                            && !journal.prop_removed(prop.section, prop.id)
                            && distance <= 9.0
                            && journal.crop_at(support).is_some_and(|s| {
                                s.prop_id == prop.id.0 && !s.mature() && s.moisture > 0.1
                            })
                            && planting_environment_valid(
                                chunks,
                                support,
                                prop.kind.crop().expect("validated crop"),
                            )
                            && (0..=16).all(|step| {
                                let t = f64::from(step) / 16.0;
                                let cell = std::array::from_fn(|i| {
                                    (flower[i]
                                        + (prop.position[i] - flower[i]) * t
                                        + if i == 1 { 0.3 } else { 0.0 })
                                    .floor() as i64
                                });
                                matches!(chunks.block_at(cell), Some(0))
                            })
                        {
                            nearby.push((distance, support));
                        }
                    }
                }
            }
        }
    }
    nearby.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    nearby.dedup_by_key(|p| p.1);
    let mut changed = false;
    for (_, support) in nearby.into_iter().take(4) {
        let mut state = *journal.crop_at(support).expect("validated crop state");
        state.progress_seconds = (state.progress_seconds + 5.0).min(state.growth_duration_seconds);
        changed |= commit_crop_state(chunks, journal, support, state)?;
    }
    Ok(changed)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SkyExposure {
    light: f32,
    rain: f32,
}

fn reconcile_crop_sections(
    chunks: &mut ChunkStore,
    journal: &EditJournal,
    owners: &BTreeSet<SectionPos>,
) -> usize {
    let mut reconciled = 0;
    for owner in owners {
        if let Some(section) = chunks.loaded.get_mut(owner) {
            journal.apply_prop_removals(*owner, &mut section.props);
            reconciled += 1;
        }
    }
    reconciled
}

#[cfg(test)]
#[derive(Resource, Default)]
pub(crate) struct CropReconciliations(usize);

fn sky(chunks: &ChunkStore, terrain: &WorldTerrain, position: [i64; 3]) -> SkyExposure {
    let height = chunks
        .prepared_column([position[0].div_euclid(16), position[2].div_euclid(16)])
        .and_then(|column| {
            column
                .surface_at(position[0], position[2])
                .map(|surface| surface.height)
        })
        .unwrap_or_else(|| {
            terrain
                .0
                .surface_height(position[0] as f64 + 0.5, position[2] as f64 + 0.5)
        });
    let ceiling = (height.ceil() as i64 + 2).max(position[1] + 16).min(356);
    let mut exposure = SkyExposure {
        light: 1.0,
        rain: 1.0,
    };
    for y in position[1] + 1..=ceiling {
        match chunks.block_at([position[0], y, position[2]]) {
            Some(material) if terra_voxel::MaterialId::FOLIAGE.contains(&material) => {
                exposure.light *= 0.76;
                exposure.rain *= 0.76;
            }
            Some(15 | 80 | 88 | 96) => {
                exposure.light *= 0.90;
                exposure.rain = 0.0;
            }
            Some(0 | 40) => {}
            Some(_) => {
                return SkyExposure {
                    light: 0.0,
                    rain: 0.0,
                };
            }
            None if y as f64 <= height => {
                return SkyExposure {
                    light: 0.0,
                    rain: 0.0,
                };
            }
            None => {}
        }
    }
    exposure
}

fn irrigation(chunks: &ChunkStore, journal: &EditJournal, support: [i64; 3]) -> bool {
    (-4..=4).any(|dx| {
        (-4..=4).any(|dz| {
            [-1, 0, 1].into_iter().any(|dy| {
                let position = [support[0] + dx, support[1] + dy, support[2] + dz];
                chunks.block_at(position) == Some(MaterialId::FluidWater as u16)
                    || journal.fluid(position).is_some_and(|cell| cell.amount > 0)
            })
        })
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn tick(
    time: Res<Time>,
    access: Res<WorldAccess>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    terrain: Res<WorldTerrain>,
    clock: Option<Res<WorldClock>>,
    weather: Option<Res<WorldWeather>>,
    mut work: Local<FarmWork>,
    #[cfg(test)] mut reconciliations: Option<ResMut<CropReconciliations>>,
) {
    let now = time.elapsed_secs_f64();
    if !access.authoritative() || now < work.next {
        return;
    }
    work.next = now + 1.0;
    let _scope = terra_core::profiling::ScopeTimer::new("world.farming");
    work.crop_visited.retain(|support, _| {
        chunks.loaded.contains_key(&global_to_section(*support).0)
            && journal.crop_at(*support).is_some()
    });
    work.forage_visited.retain(|id, _| {
        journal
            .forage_regrowth
            .get(id)
            .is_some_and(|state| chunks.loaded.contains_key(&state.prop.section))
    });
    let clock = clock.as_deref().copied().unwrap_or_default();
    if clock.paused {
        work.crop_visited.values_mut().for_each(|last| *last = now);
        work.forage_visited
            .values_mut()
            .for_each(|last| *last = now);
        return;
    }
    let season = clock.season(terrain.0.config().seasons);
    let mut positions: Vec<_> = journal
        .cultivated_sections()
        .filter(|section| chunks.loaded.contains_key(section))
        .flat_map(|section| journal.crops_in_section(section))
        .collect();
    positions.sort_unstable();
    let mut stage_sections = BTreeSet::new();
    for offset in 0..BUDGET.min(positions.len()) {
        let support = positions[(work.crop_cursor + offset) % positions.len()];
        let elapsed = (now - work.crop_visited.insert(support, now).unwrap_or(now - 1.0))
            .clamp(0.0, 30.0) as f32;
        let Some(mut state) = journal.crop_at(support).copied() else {
            continue;
        };
        if (0..=if state.kind == CropKind::Lettuce {
            1
        } else {
            2
        })
            .any(|dy| {
                chunks
                    .block_at([support[0], support[1] + dy, support[2]])
                    .is_none()
            })
        {
            continue;
        }
        if !planting_environment_valid(&chunks, support, state.kind) {
            let owner = global_to_section(support).0;
            if let Err(error) = crate::streaming::commit_prop_removal(
                &mut chunks,
                &mut journal,
                &crate::streaming::PropRemoveRequest {
                    section: owner,
                    id: PropId(state.prop_id),
                },
            ) {
                tracing::warn!(%error, "Cannot retire unsupported crop");
            }
            continue;
        }
        if state.mature() {
            continue;
        }
        let exposure = sky(&chunks, &terrain, support);
        let day = ((clock.hour() as f32 - 6.0) / 12.0 * std::f32::consts::PI)
            .sin()
            .max(0.0);
        let local = weather.as_deref().map(|weather| {
            crate::weather::sample_at(
                weather,
                &terrain.0,
                [
                    support[0] as f64 + 0.5,
                    support[1] as f64 + 1.0,
                    support[2] as f64 + 0.5,
                ],
                clock,
            )
        });
        let water = if irrigation(&chunks, &journal, support) {
            1.0
        } else {
            exposure.rain * local.map_or(0.0, |weather| weather.rain)
        };
        state.advance(elapsed, exposure.light * day, water, season);
        let owner = global_to_section(support).0;
        let previous_epoch = journal.prop_epoch(owner);
        match journal
            .bypass_change_detection()
            .advance_crop(support, state)
        {
            Ok(changed) => {
                if changed {
                    journal.set_changed();
                }
                if journal.prop_epoch(owner) != previous_epoch {
                    stage_sections.insert(owner);
                }
            }
            Err(error) => {
                journal.set_changed();
                tracing::warn!(%error, "Crop growth commit rejected");
            }
        }
    }
    // A stage boundary may affect a whole field at once. Reconcile its live
    // props once per owner; invisible progress never mutates ChunkStore.
    if !stage_sections.is_empty() {
        let count =
            reconcile_crop_sections(chunks.bypass_change_detection(), &journal, &stage_sections);
        if count > 0 {
            chunks.set_changed();
        }
        #[cfg(test)]
        if let Some(calls) = reconciliations.as_deref_mut() {
            calls.0 += count;
        }
    }
    work.crop_cursor = work.crop_cursor.wrapping_add(BUDGET);
    let mut regrowth: Vec<ForageRegrowth> = journal
        .forage_regrowth()
        .filter(|state| chunks.loaded.contains_key(&state.prop.section))
        .copied()
        .collect();
    regrowth.sort_unstable_by_key(|state| state.prop.id);
    for offset in 0..BUDGET.min(regrowth.len()) {
        let mut state = regrowth[(work.forage_cursor + offset) % regrowth.len()];
        let elapsed = (now
            - work
                .forage_visited
                .insert(state.prop.id, now)
                .unwrap_or(now - 1.0))
        .clamp(0.0, 30.0) as f32;
        let support = placement::support(&state.prop);
        if !chunks
            .block_at(support)
            .and_then(MaterialId::from_raw)
            .is_some_and(MaterialId::is_natural_ground)
            || chunks.block_at([support[0], support[1] + 1, support[2]]) != Some(0)
        {
            journal.forage_regrowth.remove(&state.prop.id);
            continue;
        }
        let growth = if season.enabled {
            (1.0 - season.weights[3]).clamp(0.0, 1.0)
        } else {
            1.0
        };
        state.remaining_seconds = (state.remaining_seconds - elapsed * growth).max(0.0);
        if state.remaining_seconds <= 0.0 {
            let available = chunks
                .loaded
                .get(&state.prop.section)
                .is_some_and(|section| {
                    placement::next_slot(&section.props, support, state.prop.kind).is_some()
                });
            if available {
                match journal.revive_forage(state.prop) {
                    Ok(true) => {
                        if let Some(section) = chunks.loaded.get_mut(&state.prop.section) {
                            journal.apply_prop_removals(state.prop.section, &mut section.props);
                        }
                    }
                    Ok(false) => {}
                    Err(error) => tracing::warn!(%error, "Forage regrowth rejected"),
                }
            }
        } else if let Err(error) = journal.set_forage_regrowth(state) {
            tracing::warn!(%error, "Forage regrowth timer rejected");
        }
    }
    work.forage_cursor = work.forage_cursor.wrapping_add(BUDGET);
}

#[cfg(test)]
mod tests;
