use super::from_climate_seasonal;
use crate::{
    edits::EditJournal,
    environment::SurfaceCondition,
    streaming::{ChunkStore, WorldAccess, WorldTerrain},
};
use bevy::prelude::*;
use std::collections::{BTreeMap, HashMap};
use terra_core::WorldWeather;
use terra_voxel::MaterialId;

const MAX_SURFACES: usize = 65_536;
const NEAR_REVISIT_SECONDS: f64 = 18.0;
const MAX_COLUMN_HISTORY: usize = 8192;
const SHAPE_COATING_SECONDS: f32 = 210.0;

#[derive(Default)]
pub(crate) struct SurfaceWork {
    next: f64,
    next_shape: f64,
    shape_cursor: usize,
    shapes: Vec<[i64; 2]>,
    columns: BTreeMap<[i64; 2], Vec<terra_voxel::SectionPos>>,
    cursor: usize,
    visited: HashMap<[i64; 2], f64>,
    sequence: u64,
    weather_elapsed: Option<f64>,
}

/// Weather keeps its ordinary two-second sweep. Sparse joinery columns also
/// share a thirty-second ring, with at most one column processed per frame.
#[allow(clippy::type_complexity)]
pub(crate) fn tick(
    time: Res<Time<Real>>,
    access: Res<WorldAccess>,
    weather: Res<WorldWeather>,
    terrain: Res<WorldTerrain>,
    loaded: (
        ResMut<ChunkStore>,
        Option<Res<crate::streaming::ChunkFocus>>,
        Option<Res<crate::streaming::ChunkFoci>>,
        Option<Res<terra_core::WorldClock>>,
        Res<terra_core::block_properties::BlockPropertiesCatalog>,
    ),
    mut journal: ResMut<EditJournal>,
    mut work: Local<SurfaceWork>,
) {
    let (mut chunks, focus, foci, clock, properties) = loaded;
    if !access.authoritative()
        || clock.as_ref().is_some_and(|clock| clock.paused)
        || time.elapsed_secs_f64() < work.next.min(work.next_shape)
    {
        return;
    }
    let now = time.elapsed_secs_f64();
    let ordinary = now >= work.next;
    if ordinary {
        work.next = now + 2.0;
    }
    let previous_tick = work
        .weather_elapsed
        .replace(weather.elapsed_total)
        .unwrap_or(weather.elapsed_total)
        .min(weather.elapsed_total);
    if work.sequence != weather.sequence {
        work.sequence = weather.sequence;
        work.cursor = 0;
    }
    let existing = journal.surface_condition_count();
    let season = clock
        .as_deref()
        .copied()
        .unwrap_or_default()
        .season(terrain.0.config().seasons);
    let winter = season.enabled && season.season == terra_core::Season::Winter;
    if weather.intensity() < 0.01 && existing == 0 {
        work.next_shape = f64::INFINITY;
        return;
    }
    if ordinary {
        let mut columns: BTreeMap<[i64; 2], Vec<terra_voxel::SectionPos>> = BTreeMap::new();
        for &section in chunks.loaded.keys() {
            columns
                .entry([section.x, section.z])
                .or_default()
                .push(section);
        }
        columns.retain(|coordinate, _| chunks.prepared_column(*coordinate).is_some());
        let mut shapes = std::collections::BTreeSet::new();
        for section in journal.object_sections() {
            let coordinate = [section.x, section.z];
            if columns.contains_key(&coordinate)
                && chunks.loaded.contains_key(&section)
                && journal
                    .object_states_in_section(section)
                    .any(|(_, material, _)| crate::snow::coatable(material))
            {
                shapes.insert(coordinate);
            }
        }
        for (coordinate, sections) in &columns {
            if !shapes.contains(coordinate)
                && (sections.iter().any(|section| {
                    journal
                        .blocks_in_section(*section)
                        .any(|(_, material)| crate::snow::coatable(material))
                }) || chunks
                    .prepared_column(*coordinate)
                    .is_some_and(|column| column.has_snow_coatable_structure(sections)))
            {
                shapes.insert(*coordinate);
            }
        }
        work.columns = columns;
        work.shapes = shapes.into_iter().collect();
        let shape_count = work.shapes.len();
        work.shape_cursor %= shape_count.max(1);
        if work.shapes.is_empty() {
            work.next_shape = f64::INFINITY;
        } else {
            work.next_shape = work.next_shape.min(now + shape_interval(shape_count));
        }
    }
    if work.columns.is_empty() {
        return;
    }
    // Retain unloaded columns so returning after a season change catches up
    // snow and wetness. The history is bounded and never requests chunk loads.
    work.visited
        .retain(|_, visited| *visited <= weather.elapsed_total);
    if work.visited.len() >= MAX_COLUMN_HISTORY
        && let Some(oldest) = work
            .visited
            .iter()
            .filter(|(coordinate, _)| !work.columns.contains_key(*coordinate))
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map(|(coordinate, _)| *coordinate)
            .or_else(|| {
                work.visited
                    .iter()
                    .min_by(|a, b| a.1.total_cmp(b.1))
                    .map(|(coordinate, _)| *coordinate)
            })
    {
        work.visited.remove(&oldest);
    }
    let centers = observer_centers(focus.as_deref(), foci.as_deref());
    let coordinate = if now >= work.next_shape && !work.shapes.is_empty() {
        let coordinate = work.shapes[work.shape_cursor];
        let shape_count = work.shapes.len();
        work.shape_cursor = (work.shape_cursor + 1) % shape_count;
        work.next_shape = now + shape_interval(shape_count);
        Some(coordinate)
    } else if ordinary {
        select_column(
            &work.columns,
            &work.visited,
            &centers,
            weather.elapsed_total,
            work.cursor,
        )
    } else {
        None
    };
    let Some(coordinate) = coordinate else {
        return;
    };
    work.cursor = work.cursor.wrapping_add(1);
    let Some(mut sections) = work.columns.get(&coordinate).cloned() else {
        return;
    };
    sections.retain(|section| chunks.loaded.contains_key(section));
    let Some(prepared) = chunks.prepared_column(coordinate) else {
        return;
    };
    sections.sort_unstable_by_key(|s| std::cmp::Reverse(s.y));
    let before = work
        .visited
        .insert(coordinate, weather.elapsed_total)
        .unwrap_or(if existing > 0 { 0.0 } else { previous_tick });
    let elapsed = (weather.elapsed_total - before).clamp(0.0, 3600.0) as f32;
    if elapsed <= 0.0 {
        return;
    }
    let roofs = saved_roofs(&journal, coordinate);
    let mut remaining = MAX_SURFACES.saturating_sub(existing);
    for z in 0..16 {
        for x in 0..16 {
            let global_x = coordinate[0] * 16 + x as i64;
            let global_z = coordinate[1] * 16 + z as i64;
            let Some(surface) = prepared.surface_at(global_x, global_z) else {
                continue;
            };
            let canopy_climate = from_climate_seasonal(
                &weather,
                surface.biome,
                surface.height,
                terrain.0.config().sea_level,
                season,
            );
            let snow_canopy =
                canopy_climate.snow > 0.001 || winter && canopy_climate.temperature <= 3.0;
            let Some(position) = exposed_position(
                &chunks,
                &journal,
                &sections,
                [global_x, global_z],
                surface.height,
                snow_canopy,
            ) else {
                continue;
            };
            if position[1] < roofs[x + z * 16]
                || terrain.0.config().bounded_world && position[1] >= 356
            {
                continue;
            }
            if chunks
                .fluid_cell_at(
                    &terrain.0,
                    &journal,
                    [position[0], position[1] + 1, position[2]],
                    0,
                )
                .amount
                > 0
            {
                continue;
            }
            let local = from_climate_seasonal(
                &weather,
                surface.biome,
                position[1] as f64 + 1.0,
                terrain.0.config().sea_level,
                season,
            );
            let before = journal.surface_condition(position);
            if before == SurfaceCondition::default() && remaining == 0 {
                continue;
            }
            let mut deposition = local;
            // Do not backfill a newly started snowfall over time spent in clear
            // weather before this column's budgeted visit.
            let snow_elapsed = elapsed.min(weather.elapsed_seconds.max(0.0) as f32);
            deposition.snow *= weather.snow_accumulation_multiplier() * snow_elapsed / elapsed
                * deposition_rate(terrain.0.config().seed.0, position);
            let material = chunks.block_at(position).unwrap_or(0);
            let mut next = coat_shape_snow(
                before,
                evolve(before, deposition, elapsed, winter),
                material,
                local,
                snow_elapsed,
            );
            if !super::retains_snow(material) {
                next.snow = 0.0;
            }
            if snow_edit(&chunks, &journal, position, next.snow).is_some()
                && let Err(error) =
                    reconcile_snow(&mut chunks, &mut journal, &properties, position, next.snow)
            {
                tracing::warn!(%error, "Snow layer update rejected");
            }
            if next != before {
                match journal.set_surface_condition(position, next) {
                    Ok(true) if before == SurfaceCondition::default() => {
                        remaining = remaining.saturating_sub(1)
                    }
                    Err(error) => tracing::warn!(%error, "Weather surface update rejected"),
                    _ => {}
                }
            }
        }
    }
}

fn shape_interval(columns: usize) -> f64 {
    24.0 / columns.max(1) as f64
}

fn observer_centers(
    focus: Option<&crate::streaming::ChunkFocus>,
    foci: Option<&crate::streaming::ChunkFoci>,
) -> Vec<[i64; 2]> {
    let points = foci.and_then(|foci| foci.0.as_deref());
    let fallback = focus.map(|focus| std::slice::from_ref(&focus.0));
    let mut centers: Vec<_> = points
        .or(fallback)
        .unwrap_or_default()
        .iter()
        .filter(|point| {
            point
                .iter()
                .all(|v| v.is_finite() && v.abs() < (1_i64 << 40) as f64 - 32.0)
        })
        .map(|point| {
            [
                (point[0].floor() as i64).div_euclid(16),
                (point[2].floor() as i64).div_euclid(16),
            ]
        })
        .collect();
    centers.sort_unstable();
    centers.dedup();
    centers
}

fn saved_roofs(journal: &EditJournal, column: [i64; 2]) -> [i64; 256] {
    let mut roofs = [i64::MIN; 256];
    // Direct section lookups, not a scan of saved regions or cloned deltas.
    for y in -8..=22 {
        let section = terra_voxel::SectionPos::new(column[0], y, column[1]);
        for (local, id) in journal.blocks_in_section(section) {
            if !MaterialId::from_raw(id).is_some_and(MaterialId::is_solid) {
                continue;
            }
            let p = [
                column[0] * 16 + local[0] as i64,
                y * 16 + local[1] as i64,
                column[1] * 16 + local[2] as i64,
            ];
            if matches!(id, 5 | 98)
                && journal.object_state(p).is_some_and(|s| {
                    if id == 5 {
                        s.facing == 8
                    } else {
                        (1..=7).contains(&s.facing)
                    }
                })
            {
                continue;
            }
            let index = local[0] + local[2] * 16;
            roofs[index] = roofs[index].max(p[1]);
        }
    }
    roofs
}

fn select_column<T>(
    columns: &BTreeMap<[i64; 2], T>,
    visited: &HashMap<[i64; 2], f64>,
    centers: &[[i64; 2]],
    now: f64,
    turn: usize,
) -> Option<[i64; 2]> {
    let near = |p: &&[i64; 2]| {
        centers
            .iter()
            .any(|center| p[0].abs_diff(center[0]) <= 1 && p[1].abs_diff(center[1]) <= 1)
            && visited
                .get(*p)
                .is_none_or(|before| now - before >= NEAR_REVISIT_SECONDS)
    };
    let oldest = |a: &&[i64; 2], b: &&[i64; 2]| {
        let last = |p: &[i64; 2]| visited.get(p).copied().unwrap_or(f64::NEG_INFINITY);
        let distance = |p: &[i64; 2]| {
            centers
                .iter()
                .map(|center| {
                    (p[0] as f64 - center[0] as f64).powi(2)
                        + (p[1] as f64 - center[1] as f64).powi(2)
                })
                .fold(f64::INFINITY, f64::min)
        };
        last(a)
            .total_cmp(&last(b))
            .then_with(|| distance(a).total_cmp(&distance(b)))
            .then_with(|| a.cmp(b))
    };
    // Reserve every third job for the globally oldest loaded column. The other
    // jobs revisit aged nearby surfaces instead of waiting for a full far sweep.
    let nearby = if !turn.is_multiple_of(3) {
        columns.keys().filter(near).min_by(oldest)
    } else {
        None
    };
    nearby.or_else(|| columns.keys().min_by(oldest)).copied()
}

fn exposed_position(
    chunks: &ChunkStore,
    journal: &EditJournal,
    descending_sections: &[terra_voxel::SectionPos],
    [x, z]: [i64; 2],
    surface_height: f64,
    snow_canopy: bool,
) -> Option<[i64; 3]> {
    descending_sections
        .iter()
        .find_map(|section| {
            let data = &chunks.loaded.get(section)?.data;
            (0..16).rev().find_map(|y| {
                let id = data
                    .block([x.rem_euclid(16) as usize, y, z.rem_euclid(16) as usize])
                    .ok()?;
                let p = [x, section.y * 16 + y as i64, z];
                if id == 98
                    || id == 5 && journal.object_state(p).is_some_and(|s| s.facing == 8)
                    || snow_canopy && matches!(id, 6..=10 | 36) && journal.block(p).is_none()
                {
                    return None;
                }
                MaterialId::from_raw(id)
                    .filter(|id| id.is_solid())
                    .map(|_| [x, section.y * 16 + y as i64, z])
            })
        })
        .filter(|position| {
            if position[1] as f64 >= (surface_height - 0.5).floor() {
                return true;
            }
            // Open ravines can be lower than the nominal heightfield. Accept
            // their floor only when every intervening section is known air.
            let floor = position[1].div_euclid(16);
            let top = ((surface_height - 0.5).floor() as i64).div_euclid(16);
            top - floor <= 32 && (floor..=top).all(|y| descending_sections.iter().any(|s| s.y == y))
        })
}

/// The weather condition owns only the marked cap above its support. Player
/// blocks are never overwritten, including a snow block placed by the player.
fn snow_edit(
    chunks: &ChunkStore,
    journal: &EditJournal,
    support: [i64; 3],
    amount: f32,
) -> Option<(crate::streaming::BlockEditRequest, u8)> {
    // Natural accumulation stops at a full snow voxel, even on generated snow.
    // Player placement is handled separately and remains unrestricted.
    let material = chunks.block_at(support)?;
    let sheds_snow = !super::supports_snow_layers(material);
    let position = [support[0], support[1] + 1, support[2]];
    let current = chunks.block_at(position)?;
    let owned = matches!(current, 5 | 98)
        && journal.object_state(position).is_some_and(|s| {
            if current == 5 {
                s.facing == 8
            } else {
                (1..=7).contains(&s.facing)
            }
        });
    if current != 0 && !owned {
        return None;
    }
    if sheds_snow && !owned {
        return None;
    }
    let state = if !sheds_snow && amount.is_finite() {
        ((amount.clamp(0.0, 1.0) * 8.0 + 0.0001).floor() as u8).min(super::NATURAL_SNOW_MAX_LAYERS)
    } else {
        0
    };
    let material = match state {
        0 => 0,
        8 => 5,
        _ => 98,
    };
    if current == material
        && (state == 0
            || journal
                .object_state(position)
                .is_some_and(|s| s.facing == state))
    {
        return None;
    }
    Some((
        crate::streaming::BlockEditRequest {
            position,
            expected_material: current,
            material,
        },
        state,
    ))
}

pub(crate) fn reconcile_snow(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    support: [i64; 3],
    amount: f32,
) -> Result<(), String> {
    let Some((edit, state)) = snow_edit(chunks, journal, support, amount) else {
        return Ok(());
    };
    let crate::streaming::BlockEditRequest {
        position,
        expected_material: current,
        material,
    } = edit;
    if current != material {
        crate::streaming::commit_weather_cap(
            chunks,
            journal,
            &crate::streaming::BlockEditRequest {
                position,
                expected_material: current,
                material,
            },
            properties,
        )?;
    }
    if state > 0 {
        crate::streaming::commit_object_state(
            chunks,
            journal,
            position,
            Some(crate::edits::BlockObjectState {
                facing: state,
                ..Default::default()
            }),
        )?;
    }
    Ok(())
}

fn deposition_rate(seed: u64, position: [i64; 3]) -> f32 {
    let mut hash = seed ^ 0x27f8_954c_a69d_031b;
    for (axis, value) in position.into_iter().enumerate() {
        hash ^= (value as u64).wrapping_mul(
            [
                0x9e37_79b9_7f4a_7c15,
                0xbf58_476d_1ce4_e5b9,
                0x94d0_49bb_1331_11eb,
            ][axis],
        );
        hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    }
    hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    0.55 + ((hash ^ (hash >> 31)) & 0xffff) as f32 / 65535.0 * 0.9
}

fn evolve(
    before: SurfaceCondition,
    local: super::LocalWeather,
    elapsed: f32,
    winter: bool,
) -> SurfaceCondition {
    let mut next = before;
    // Fractional weather accumulation is retained between the eight physical states.
    let melt =
        (local.temperature.max(0.0) * 0.00014 + local.rain * 0.008 + before.fire * 0.08) * elapsed;
    next.snow = (before.snow + local.snow * super::SNOW_ACCUMULATION_PER_SECOND * elapsed - melt)
        .clamp(0.0, 1.0);
    if winter {
        next.snow = next.snow.max(before.snow);
    }
    let wet_target = (local.rain + local.wetness * 0.7).min(1.0);
    let speed = if wet_target > before.wetness {
        0.12
    } else {
        0.007
    };
    next.wetness =
        before.wetness + (wet_target - before.wetness) * (1.0 - (-speed * elapsed).exp());
    if next.wetness < 0.003 {
        next.wetness = 0.0;
    }
    if local.rain > 0.35 || next.snow > 0.25 {
        next.fire = 0.0;
    }
    next
}

fn coat_shape_snow(
    before: SurfaceCondition,
    mut next: SurfaceCondition,
    material: u16,
    local: super::LocalWeather,
    snow_elapsed: f32,
) -> SurfaceCondition {
    if crate::snow::coatable(material)
        && local.snow > 0.0
        && local.temperature <= 3.0
        && before.fire == 0.0
        && snow_elapsed > 0.0
    {
        // Only cosmetic joinery coverage has a deadline, not physical snow layers.
        // Reserve a full weather transition and sparse ring before five minutes.
        let cover = super::SHAPED_SNOW_FULL_COVER;
        let floor = (before.snow + cover / SHAPE_COATING_SECONDS * snow_elapsed).min(cover);
        next.snow = next.snow.max(floor);
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::{BlockSection, SectionPos};

    #[test]
    fn player_layer_poses_survive_natural_cap_limits_and_legacy_full_weather_caps_are_repaired() {
        let section = SectionPos::new(-1, 0, -1);
        let support = [-1, 1, -1];
        let cap = [-1, 2, -1];
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block([15, 1, 15], 3).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            crate::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut journal = EditJournal::default();
        let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
        for layers in 1..=7 {
            let expected = if layers == 1 { 0 } else { 98 };
            crate::snow::commit(&mut chunks, &mut journal, cap, expected, &properties).unwrap();
            let facing = journal.object_state(cap).unwrap().facing;
            let revision = journal.revision(section);
            for amount in [0.0, 0.125, 0.25, 1.0] {
                reconcile_snow(&mut chunks, &mut journal, &properties, support, amount).unwrap();
                assert_eq!(journal.object_state(cap).unwrap().facing, facing);
                assert_eq!(journal.revision(section), revision);
            }
        }
        crate::streaming::commit_block_edit(
            &mut chunks,
            &mut journal,
            &crate::streaming::BlockEditRequest {
                position: cap,
                expected_material: 98,
                material: 5,
            },
            &properties,
        )
        .unwrap();
        journal
            .set_object_state(
                cap,
                Some(crate::edits::BlockObjectState {
                    facing: 8,
                    ..default()
                }),
            )
            .unwrap();
        reconcile_snow(&mut chunks, &mut journal, &properties, support, 1.0).unwrap();
        assert_eq!(chunks.block_at(cap), Some(98));
        assert_eq!(journal.object_state(cap).unwrap().facing, 2);
    }

    #[test]
    fn normal_snow_coats_joinery_within_five_minutes_and_shape_work_is_spread_over_a_bounded_ring()
    {
        for rate in [0.55_f32, 0.8, 1.0, 1.45] {
            let local = super::super::LocalWeather {
                snow: rate * 0.85,
                temperature: -8.0,
                ..default()
            };
            let condition = coat_shape_snow(
                SurfaceCondition::default(),
                evolve(SurfaceCondition::default(), local, 270.0, true),
                78,
                local,
                270.0,
            );
            assert!(condition.snow >= super::super::SHAPED_SNOW_FULL_COVER);
            let melted = evolve(
                condition,
                super::super::LocalWeather {
                    temperature: 18.0,
                    rain: 0.8,
                    ..default()
                },
                3600.0,
                false,
            );
            assert_eq!(melted.snow, 0.0);
        }
        for columns in [1, 9, 64, 256, 1024] {
            let interval = shape_interval(columns);
            assert!((interval * columns as f64 - 24.0).abs() < 0.00001);
            assert!(interval > 0.0);
        }
        assert_eq!(SurfaceWork::default().shapes.len(), 0);
    }

    #[test]
    fn normal_snow_real_transition_and_sparse_round_finish_before_five_minutes_but_dry_winter_does_not()
     {
        for transition in [8.0, 24.0, 60.0] {
            let mut weather = terra_core::WorldWeather::new(42042);
            weather.set_pattern(terra_core::WeatherPattern::Snow);
            weather.transition_seconds = transition;
            weather.duration_seconds = 1000.0;
            let mut condition = SurfaceCondition::default();
            let mut coated_at = None;
            for second in 1..=300 {
                weather.advance(1.0, 42042);
                let local = super::super::LocalWeather {
                    snow: weather.intensity(),
                    temperature: -8.0,
                    ..default()
                };
                let mut deposition = local;
                deposition.snow *= weather.snow_accumulation_multiplier() * 0.55;
                condition = coat_shape_snow(
                    condition,
                    evolve(condition, deposition, 1.0, true),
                    78,
                    local,
                    1.0,
                );
                if condition.snow >= super::super::SHAPED_SNOW_FULL_COVER {
                    coated_at = Some(second);
                    break;
                }
            }
            assert!(
                coated_at.is_some_and(|second| second + 30 <= 300),
                "transition {transition}: {coated_at:?}"
            );
        }
        assert_eq!(
            evolve(
                SurfaceCondition::default(),
                super::super::LocalWeather {
                    temperature: -8.0,
                    ..default()
                },
                3600.0,
                true
            )
            .snow,
            0.0
        );
    }

    #[test]
    fn weak_climate_snow_coats_shapes_before_five_minutes_without_accelerating_ground_layers() {
        use crate::generation::biomes::{BiomeSample, BiomeWeights};
        let biome = BiomeSample {
            temperature: 0.0,
            humidity: 0.05,
            weights: BiomeWeights {
                ocean: 0.0,
                plains: 1.0,
                forest: 0.0,
                desert: 0.0,
                highland: 0.0,
                mushroom_park: 0.0,
                cliffted_desert: 0.0,
                cherry_forest: 0.0,
                spirit_blossom: 0.0,
                blood_forest: 0.0,
                ashlands: 0.0,
                savanna: 0.0,
                swamp: 0.0,
                petrified_forest: 0.0,
                crystal_highlands: 0.0,
                misty_coast: 0.0,
            },
        };
        for forced_snow in [false, true] {
            let mut weather = WorldWeather::new(42042);
            if forced_snow {
                weather.set_pattern(terra_core::WeatherPattern::Snow);
            }
            weather.from_intensity = 0.0;
            weather.target_intensity = 0.28;
            weather.transition_seconds = 60.0;
            weather.duration_seconds = 1000.0;
            let mut shape = SurfaceCondition::default();
            let mut ground = SurfaceCondition::default();
            let mut reference_ground = ground;
            let mut coated_at = None;
            for second in 1..=300 {
                weather.advance(1.0, 42042);
                let local = super::super::from_climate(&weather, biome, 8.0, 8.0);
                assert!(local.snow > 0.0 && local.snow <= weather.intensity());
                if !forced_snow {
                    assert!(local.rain > 0.0 && local.snow < weather.intensity() * 0.5);
                }
                // Even discarding the entire fade reserves its full sixty seconds.
                if weather.elapsed_seconds < weather.transition_seconds {
                    continue;
                }
                let mut deposition = local;
                deposition.snow *= weather.snow_accumulation_multiplier() * 0.55;
                shape =
                    coat_shape_snow(shape, evolve(shape, deposition, 1.0, true), 78, local, 1.0);
                ground =
                    coat_shape_snow(ground, evolve(ground, deposition, 1.0, true), 3, local, 1.0);
                reference_ground = evolve(reference_ground, deposition, 1.0, true);
                assert_eq!(ground, reference_ground);
                if shape.snow >= super::super::SHAPED_SNOW_FULL_COVER {
                    coated_at = Some(second);
                    break;
                }
            }
            assert!(
                coated_at.is_some_and(|second| second + 30 <= 300),
                "forced_snow={forced_snow}, coated_at={coated_at:?}"
            );
        }
    }

    #[test]
    fn shape_coating_counts_only_current_snow_time_and_preserves_dry_rain_fire_and_warm_melting() {
        let before = SurfaceCondition::default();
        let snow = super::super::LocalWeather {
            snow: 0.28 * 0.4 * 0.25,
            temperature: -1.0,
            ..default()
        };
        let mut weather = WorldWeather::new(42042);
        weather.set_pattern(terra_core::WeatherPattern::Snow);
        weather.elapsed_seconds = 1.0;
        let elapsed = 3600.0_f32;
        let active = elapsed.min(weather.elapsed_seconds.max(0.0) as f32);
        let mut deposition = snow;
        deposition.snow *= 0.55 * active / elapsed;
        let next = coat_shape_snow(
            before,
            evolve(before, deposition, elapsed, true),
            78,
            snow,
            active,
        );
        assert!(next.snow > 0.0 && next.snow < 0.001, "{next:?}");
        assert_eq!(
            coat_shape_snow(before, before, 78, snow, 0.0),
            before,
            "No elapsed snowfall must never backfill the clear-weather history"
        );
        for material in [78, 86, 90, 102, 3, 71, 147, 159, 160] {
            let dry = super::super::LocalWeather {
                temperature: -8.0,
                ..default()
            };
            assert_eq!(
                coat_shape_snow(
                    before,
                    evolve(before, dry, 3600.0, true),
                    material,
                    dry,
                    3600.0
                )
                .snow,
                0.0
            );
            for (condition, local) in [
                (
                    SurfaceCondition {
                        snow: 0.04,
                        ..default()
                    },
                    super::super::LocalWeather {
                        rain: 0.7,
                        temperature: 8.0,
                        ..default()
                    },
                ),
                (
                    SurfaceCondition {
                        snow: 0.04,
                        fire: 0.1,
                        ..default()
                    },
                    snow,
                ),
                (
                    SurfaceCondition {
                        snow: 0.04,
                        ..default()
                    },
                    super::super::LocalWeather {
                        temperature: 8.0,
                        ..snow
                    },
                ),
            ] {
                let expected = evolve(condition, local, 30.0, false);
                assert_eq!(
                    coat_shape_snow(condition, expected, material, local, 30.0),
                    expected
                );
            }
        }
    }

    #[test]
    fn shaped_surfaces_repair_only_old_weather_caps_and_preserve_player_snow_and_surface_state() {
        let section = SectionPos::new(-1, 0, -1);
        let support = [-1, 4, -1];
        let cap = [-1, 5, -1];
        let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
        for material in [
            74, 77, 78, 82, 85, 86, 90, 94, 101, 102, 104, 105, 106, 131, 136, 137, 138,
        ] {
            let mut data = BlockSection::empty(section).unwrap();
            data.set_block([15, 4, 15], material).unwrap();
            let mut chunks = ChunkStore::default();
            chunks.loaded.insert(
                section,
                crate::streaming::LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            let mut journal = EditJournal::default();
            let condition = SurfaceCondition {
                snow: 0.75,
                wetness: 0.2,
                ..default()
            };
            journal.set_surface_condition(support, condition).unwrap();
            reconcile_snow(&mut chunks, &mut journal, &properties, support, 1.0).unwrap();
            assert_eq!(chunks.block_at(cap), Some(0), "material {material}");
            for old_material in [98, 5] {
                crate::streaming::commit_weather_cap(
                    &mut chunks,
                    &mut journal,
                    &crate::streaming::BlockEditRequest {
                        position: cap,
                        expected_material: 0,
                        material: old_material,
                    },
                    &properties,
                )
                .unwrap();
                journal
                    .set_object_state(
                        cap,
                        Some(crate::edits::BlockObjectState {
                            facing: if old_material == 98 { 4 } else { 8 },
                            ..default()
                        }),
                    )
                    .unwrap();
                reconcile_snow(&mut chunks, &mut journal, &properties, support, 1.0).unwrap();
                assert_eq!(chunks.block_at(cap), Some(0), "weather cap over {material}");
            }
            for player_material in [98, 5] {
                crate::streaming::commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &crate::streaming::BlockEditRequest {
                        position: cap,
                        expected_material: 0,
                        material: player_material,
                    },
                    &properties,
                )
                .unwrap();
                assert!(journal.object_state(cap).is_none());
                reconcile_snow(&mut chunks, &mut journal, &properties, support, 1.0).unwrap();
                assert_eq!(
                    chunks.block_at(cap),
                    Some(player_material),
                    "player cap over {material}"
                );
                crate::streaming::commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &crate::streaming::BlockEditRequest {
                        position: cap,
                        expected_material: player_material,
                        material: 0,
                    },
                    &properties,
                )
                .unwrap();
            }
            assert_eq!(journal.surface_condition(support), condition);
            assert_eq!(chunks.block_at(support), Some(material));
        }
    }

    #[test]
    fn normal_snow_takes_minutes_per_layer_without_changing_existing_winter_snow() {
        let snowfall = super::super::LocalWeather {
            snow: 1.0,
            temperature: -8.0,
            ..default()
        };
        let one_minute = evolve(SurfaceCondition::default(), snowfall, 60.0, true);
        assert_eq!((one_minute.snow * 8.0).floor(), 0.0);
        assert!((one_minute.snow - 0.045).abs() < 0.00001);
        let three_minutes = evolve(SurfaceCondition::default(), snowfall, 180.0, true);
        assert_eq!((three_minutes.snow * 8.0).floor(), 1.0);
        assert!(three_minutes.snow < 0.003 * 180.0 * 0.251);
        let saved = SurfaceCondition {
            snow: 0.9,
            ..default()
        };
        assert_eq!(
            evolve(saved, super::super::LocalWeather::default(), 3600.0, true).snow,
            saved.snow
        );
    }

    #[test]
    fn deposition_is_per_block_seeded_and_independent_of_chunk_boundaries_and_visit_order() {
        let rates: BTreeMap<_, _> = (-16..16)
            .flat_map(|z| (-16..16).map(move |x| [x, 1, z]))
            .map(|p| (p, deposition_rate(116, p)))
            .collect();
        let reversed: BTreeMap<_, _> = rates
            .keys()
            .rev()
            .map(|&p| (p, deposition_rate(116, p)))
            .collect();
        assert_eq!(rates, reversed);
        assert!(rates.values().all(|rate| (0.55..=1.45).contains(rate)));
        let states: Vec<_> = rates
            .values()
            .map(|rate| {
                (super::super::SNOW_ACCUMULATION_PER_SECOND * 200.0 * rate * 8.0).floor() as u8
            })
            .collect();
        assert!(
            states.contains(&0) && states.contains(&1),
            "first snow is scattered, not a whole-column switch"
        );
        assert!(
            rates
                .iter()
                .any(|(&p, &rate)| deposition_rate(117, p) != rate)
        );
        for z in -16..16 {
            assert_eq!(rates[&[-1, 1, z]], deposition_rate(116, [-1, 1, z]));
            assert_eq!(rates[&[0, 1, z]], deposition_rate(116, [0, 1, z]));
        }
    }

    #[test]
    fn natural_snow_stops_below_prop_burial_and_manual_third_layer_hides_without_removing_props() {
        use crate::props::{PropId, PropKind, WorldProp};
        let section = SectionPos::new(-1, 0, -1);
        let support = [-1, 1, -1];
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block([15, 1, 15], MaterialId::Grass as u16)
            .unwrap();
        let prop = WorldProp {
            id: PropId(712),
            kind: PropKind::Flower,
            position: [-0.5, 2.0, -0.5],
            rotation: 0.0,
            scale: 0.65,
            variant: 0,
            section,
            removed: false,
        };
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            crate::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![prop],
            },
        );
        let mut journal = EditJournal::default();
        let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
        for state in 1_u8..=8 {
            reconcile_snow(
                &mut chunks,
                &mut journal,
                &properties,
                support,
                f32::from(state) / 8.0,
            )
            .unwrap();
            assert_eq!(chunks.loaded[&section].props, vec![prop]);
            assert!(!journal.prop_removed(section, prop.id));
            assert!(!crate::props::seasonal::snow_buried(
                &prop, &chunks, &journal
            ));
        }
        crate::snow::commit(&mut chunks, &mut journal, [-1, 2, -1], 98, &properties).unwrap();
        assert!(crate::props::seasonal::snow_buried(
            &prop, &chunks, &journal
        ));
        assert!(!journal.prop_removed(section, prop.id));
        assert_eq!(chunks.loaded[&section].props, vec![prop]);
        crate::streaming::commit_block_edit(
            &mut chunks,
            &mut journal,
            &crate::streaming::BlockEditRequest {
                position: [-1, 2, -1],
                expected_material: 98,
                material: 0,
            },
            &properties,
        )
        .unwrap();
        reconcile_snow(&mut chunks, &mut journal, &properties, support, 0.0).unwrap();
        assert!(!crate::props::seasonal::snow_buried(
            &prop, &chunks, &journal
        ));
        assert_eq!(chunks.loaded[&section].props, vec![prop]);
        crate::streaming::commit_block_edit(
            &mut chunks,
            &mut journal,
            &crate::streaming::BlockEditRequest {
                position: [-1, 2, -1],
                expected_material: 0,
                material: MaterialId::Stone as u16,
            },
            &properties,
        )
        .unwrap();
        assert!(
            chunks.loaded[&section].props[0].removed,
            "ordinary player construction still removes occupied plants"
        );
    }

    #[test]
    fn dry_winter_never_deposits_snow_and_intensity_controls_deposition() {
        let dry = super::super::LocalWeather {
            temperature: -10.0,
            ..default()
        };
        assert_eq!(
            evolve(SurfaceCondition::default(), dry, 3600.0, true).snow,
            0.0
        );
        let mut previous = 0.0;
        for level in 1..=5 {
            let mut weather = WorldWeather::new(1);
            weather.snow_level = level;
            let local = super::super::LocalWeather {
                snow: weather.snow_accumulation_multiplier(),
                temperature: -10.0,
                ..default()
            };
            let next = evolve(SurfaceCondition::default(), local, 30.0, true).snow;
            assert!(next > previous && next < 1.0);
            previous = next;
        }
    }

    #[test]
    fn furniture_mechanics_and_ports_shed_snow_without_snowing_through_them() {
        let mut chunks = ChunkStore::default();
        let p = SectionPos::new(-1, 0, -1);
        chunks.loaded.insert(
            p,
            crate::streaming::LoadedSection {
                data: BlockSection::empty(p).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let mut journal = EditJournal::default();
        let properties = terra_core::block_properties::BlockPropertiesCatalog::load(
            &terra_core::asset_directory().join("terra/blocks"),
        )
        .unwrap();
        for id in [28, 29, 59, 71, 72, 73, 99, 100]
            .into_iter()
            .chain(141..=153)
        {
            chunks
                .loaded
                .get_mut(&p)
                .unwrap()
                .data
                .set_block([15, 4, 15], id)
                .unwrap();
            let top = [-1, 4, -1];
            assert_eq!(
                exposed_position(&chunks, &journal, &[p], [-1, -1], 0.0, true),
                Some(top)
            );
            assert!(snow_edit(&chunks, &journal, top, 1.0).is_none());
            reconcile_snow(&mut chunks, &mut journal, &properties, top, 1.0).unwrap();
            assert_eq!(chunks.block_at([-1, 5, -1]), Some(0));
            // Repair only weather-owned caps from older saves, not player snow.
            let cap = [-1, 5, -1];
            chunks
                .loaded
                .get_mut(&p)
                .unwrap()
                .data
                .set_block([15, 5, 15], 98)
                .unwrap();
            journal.record(cap, 98).unwrap();
            journal
                .set_object_state(
                    cap,
                    Some(crate::edits::BlockObjectState {
                        facing: 4,
                        ..default()
                    }),
                )
                .unwrap();
            reconcile_snow(&mut chunks, &mut journal, &properties, top, 0.0).unwrap();
            assert_eq!(chunks.block_at(cap), Some(0));
        }
        assert_eq!(journal.surface_condition_count(), 0);
    }

    #[test]
    fn natural_snow_stops_at_two_layers_under_trees_and_never_overwrites_player_blocks() {
        let mut chunks = ChunkStore::default();
        let section = SectionPos::new(-1, 0, -1);
        let ground = [-1, 1, -1];
        let cap = [-1, 2, -1];
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block([15, 1, 15], 1).unwrap();
        data.set_block([15, 8, 15], 7).unwrap();
        chunks.loaded.insert(
            section,
            crate::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let mut journal = EditJournal::default();
        let properties = terra_core::block_properties::BlockPropertiesCatalog::load(
            &terra_core::asset_directory().join("terra/blocks"),
        )
        .unwrap();
        for state in 1..=8 {
            assert_eq!(
                exposed_position(&chunks, &journal, &[section], [-1, -1], 2.0, true),
                Some(ground)
            );
            reconcile_snow(
                &mut chunks,
                &mut journal,
                &properties,
                ground,
                f32::from(state) / 8.0,
            )
            .unwrap();
            assert_eq!(chunks.block_at(ground), Some(1));
            assert_eq!(chunks.block_at(cap), Some(98));
            let natural = state.min(super::super::NATURAL_SNOW_MAX_LAYERS);
            assert_eq!(journal.object_state(cap).unwrap().facing, natural);
            let mut actual = Vec::new();
            crate::interaction_shapes::visit_boxes(&chunks, &journal, &properties, cap, |b| {
                actual.push(b)
            });
            assert_eq!(actual[0].maximum[1], 2.0 + f64::from(natural) / 8.0);
            let revision = journal.revision(section);
            assert!(snow_edit(&chunks, &journal, ground, f32::from(state) / 8.0).is_none());
            reconcile_snow(
                &mut chunks,
                &mut journal,
                &properties,
                ground,
                f32::from(state) / 8.0,
            )
            .unwrap();
            assert_eq!(
                journal.revision(section),
                revision,
                "stable snow must not remesh every tick"
            );
        }
        assert!(
            snow_edit(&chunks, &journal, cap, 1.0).is_none(),
            "a thin natural cap cannot grow a separate second cap"
        );
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block([14, 1, 15], 5)
            .unwrap();
        assert!(
            snow_edit(&chunks, &journal, [-2, 1, -1], 1.0).is_none(),
            "generated snow blocks also stop accumulation"
        );
        assert!(
            crate::streaming::commit_block_edit(
                &mut chunks,
                &mut journal,
                &crate::streaming::BlockEditRequest {
                    position: [-2, 2, -1],
                    expected_material: 0,
                    material: 5
                },
                &properties
            )
            .unwrap(),
            "players can still stack snow blocks"
        );
        reconcile_snow(&mut chunks, &mut journal, &properties, ground, 0.5).unwrap();
        assert_eq!(chunks.block_at(cap), Some(98));
        assert_eq!(journal.object_state(cap).unwrap().facing, 2);
        reconcile_snow(&mut chunks, &mut journal, &properties, ground, 0.0).unwrap();
        assert_eq!(chunks.block_at(cap), Some(0));
        crate::streaming::commit_block_edit(
            &mut chunks,
            &mut journal,
            &crate::streaming::BlockEditRequest {
                position: cap,
                expected_material: 0,
                material: 3,
            },
            &properties,
        )
        .unwrap();
        reconcile_snow(&mut chunks, &mut journal, &properties, ground, 1.0).unwrap();
        assert_eq!(chunks.block_at(cap), Some(3));
        journal.record([-1, 100, -1], 15).unwrap();
        assert_eq!(
            saved_roofs(&journal, [-1, -1])[255],
            100,
            "unloaded player roofs still stop snowfall"
        );
    }

    #[test]
    fn unloaded_snow_catches_up_to_warm_climate_without_erasing_player_edits() {
        let mut journal = EditJournal::default();
        let position = [-17, 35, 12];
        journal.record(position, MaterialId::Stone as u16).unwrap();
        let revision = journal.revision(terra_voxel::global_to_section(position).0);
        let before = SurfaceCondition {
            snow: 1.0,
            charred: 0.4,
            ..default()
        };
        let warm = super::super::LocalWeather {
            temperature: 15.0,
            ..default()
        };
        let caught_up = evolve(before, warm, 3600.0, false);
        assert_eq!(
            evolve(before, warm, 3600.0, true).snow,
            1.0,
            "snow remains throughout winter"
        );
        journal.set_surface_condition(position, caught_up).unwrap();
        assert_eq!(caught_up.snow, 0.0);
        assert_eq!(caught_up.charred, before.charred);
        assert_eq!(journal.block(position), Some(MaterialId::Stone as u16));
        assert_eq!(
            journal.revision(terra_voxel::global_to_section(position).0),
            revision
        );
        let cold = super::super::LocalWeather {
            snow: 0.8,
            temperature: -6.0,
            ..default()
        };
        assert_eq!(
            evolve(SurfaceCondition::default(), cold, 2400.0, false).snow,
            1.0
        );
    }

    #[test]
    fn paused_world_and_stationary_weather_clock_preserve_real_surface_conditions() {
        let mut config = terra_core::GameConfig::default();
        config.client.view_distance = 1;
        config.client.vertical_view_distance = 1;
        config.world.base_height = 24.0;
        config.world.hill_amplitude = 0.0;
        config.world.mountain_amplitude = 0.0;
        config.world.terrace_strength = 0.0;
        config.world.biomes = false;
        config.world.caves = false;
        config.world.trees = false;
        config.world.rivers = false;
        config.world.water = false;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs(2),
            ))
            .add_plugins(terra_core::TerraCorePlugin(config))
            .add_plugins(crate::streaming::TerraWorldPlugin {
                mode: crate::streaming::WorldMode::Client,
            });
        app.world_mut()
            .resource_mut::<terra_core::WorldClock>()
            .paused = true;
        app.finish();
        app.cleanup();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app
            .world()
            .resource::<crate::streaming::WorldGeneration>()
            .status
            != crate::streaming::GenerationStatus::Ready
        {
            assert!(
                std::time::Instant::now() < deadline,
                "flat weather fixture failed to stream"
            );
            app.update();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let position = {
            let chunks = app.world().resource::<ChunkStore>();
            chunks
                .loaded
                .iter()
                .find_map(|(section, loaded)| {
                    loaded
                        .data
                        .blocks()
                        .iter()
                        .enumerate()
                        .find_map(|(index, &id)| {
                            if id != MaterialId::Grass as u16 {
                                return None;
                            }
                            let p = section
                                .global_block([index % 16, index / 16 % 16, index / 256])
                                .ok()?;
                            (chunks.block_at([p[0], p[1] + 1, p[2]]) == Some(0)).then_some(p)
                        })
                })
                .expect("flat streamed terrain contains exposed grass")
        };
        let condition = SurfaceCondition {
            snow: 1.0,
            wetness: 0.5,
            ..Default::default()
        };
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_surface_condition(position, condition)
            .unwrap();
        for _ in 0..20 {
            app.update();
        }
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .surface_condition(position),
            condition
        );
        app.world_mut()
            .resource_mut::<terra_core::WorldClock>()
            .paused = false;
        // A non-advancing authority must not fabricate elapsed weather time.
        for _ in 0..20 {
            app.update();
        }
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .surface_condition(position),
            condition
        );
        for _ in 0..40 {
            app.world_mut().resource_mut::<WorldWeather>().elapsed_total += 2.0;
            app.update();
        }
        assert!(
            app.world()
                .resource::<EditJournal>()
                .surface_condition(position)
                .snow
                < 1.0,
            "the same exposed snowy surface must melt once authoritative weather time advances"
        );
    }

    #[test]
    fn near_wetness_revisits_fit_thirty_seconds_without_starving_background_columns() {
        let columns: BTreeMap<_, _> = (-8..=8)
            .flat_map(|x| (-8..=8).map(move |z| ([x, z], ())))
            .collect();
        let mut visited = HashMap::new();
        for turn in 0..1000 {
            let now = turn as f64 * 2.0;
            let selected = select_column(&columns, &visited, &[[0, 0]], now, turn).unwrap();
            visited.insert(selected, now);
            if turn > 30 {
                for x in -1..=1 {
                    for z in -1..=1 {
                        let before = visited.get(&[x, z]).copied().unwrap_or(f64::NEG_INFINITY);
                        assert!(
                            now - before <= 30.0,
                            "near column [{x},{z}] age {}",
                            now - before
                        );
                    }
                }
            }
        }
        assert_eq!(
            visited.len(),
            columns.len(),
            "reserved oldest-first jobs must reach every distant column"
        );
    }

    #[test]
    fn separated_players_share_near_jobs_and_background_retains_a_fair_budget() {
        let centers = [[-16, -16], [16, 16]];
        let columns: BTreeMap<_, _> = centers
            .into_iter()
            .flat_map(|center| {
                (-8..=8)
                    .flat_map(move |x| (-8..=8).map(move |z| ([center[0] + x, center[1] + z], ())))
            })
            .collect();
        let mut visited = HashMap::new();
        let mut near_visits = [0_u32; 2];
        for turn in 0..2000 {
            let now = turn as f64 * 2.0;
            let selected = select_column(&columns, &visited, &centers, now, turn).unwrap();
            visited.insert(selected, now);
            for (index, center) in centers.iter().enumerate() {
                if selected[0].abs_diff(center[0]) <= 1 && selected[1].abs_diff(center[1]) <= 1 {
                    near_visits[index] += 1;
                }
                if turn > 60 {
                    for x in -1..=1 {
                        for z in -1..=1 {
                            let before = visited
                                .get(&[center[0] + x, center[1] + z])
                                .copied()
                                .unwrap_or(f64::NEG_INFINITY);
                            assert!(
                                now - before <= 56.0,
                                "player {index} near column age {}",
                                now - before
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(visited.len(), columns.len());
        assert!(
            near_visits[0].abs_diff(near_visits[1]) < 10,
            "no first-player-only weather work: {near_visits:?}"
        );
    }

    #[test]
    fn server_foci_override_single_focus_and_support_negative_coordinates() {
        let focus = crate::streaming::ChunkFocus([1000.0, 0.0, 1000.0]);
        let foci = crate::streaming::ChunkFoci(Some(vec![
            [-0.1, 0.0, 32.0],
            [160.0, 0.0, 160.0],
            [f64::NAN, 0.0, 0.0],
        ]));
        assert_eq!(
            observer_centers(Some(&focus), Some(&foci)),
            vec![[-1, 2], [10, 10]]
        );
        assert!(
            observer_centers(
                Some(&focus),
                Some(&crate::streaming::ChunkFoci(Some(Vec::new())))
            )
            .is_empty()
        );
        assert_eq!(
            observer_centers(
                Some(&crate::streaming::ChunkFocus([-0.1, 0.0, -16.1])),
                None
            ),
            vec![[-1, -2]]
        );
    }

    #[test]
    fn rain_hits_roofs_not_indoor_floors_and_unloaded_ceilings_remain_protected() {
        let mut chunks = ChunkStore::default();
        let section = SectionPos::new(-1, 0, -1);
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block([15, 1, 15], MaterialId::Stone as u16)
            .unwrap();
        data.set_block([15, 9, 15], MaterialId::Glass as u16)
            .unwrap();
        chunks.loaded.insert(
            section,
            crate::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        assert_eq!(
            exposed_position(
                &chunks,
                &EditJournal::default(),
                &[section],
                [-1, -1],
                2.0,
                false
            ),
            Some([-1, 9, -1])
        );
        assert_eq!(
            exposed_position(
                &chunks,
                &EditJournal::default(),
                &[section],
                [-1, -1],
                30.0,
                false
            ),
            None
        );
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block([15, 9, 15], 0)
            .unwrap();
        assert_eq!(
            exposed_position(
                &chunks,
                &EditJournal::default(),
                &[section],
                [-1, -1],
                2.0,
                false
            ),
            Some([-1, 1, -1])
        );
        assert_eq!(
            exposed_position(
                &chunks,
                &EditJournal::default(),
                &[section],
                [-1, -1],
                5.0,
                false
            ),
            Some([-1, 1, -1])
        );
    }

    #[test]
    fn snow_accumulates_without_quantization_stalls_and_melts_in_warm_rain() {
        let mut condition = SurfaceCondition::default();
        let snow = super::super::LocalWeather {
            snow: 0.8,
            temperature: -3.0,
            ..default()
        };
        for _ in 0..120 {
            condition = evolve(condition, snow, 2.0, false);
        }
        let expected = 0.8 * super::super::SNOW_ACCUMULATION_PER_SECOND * 240.0;
        assert!((condition.snow - expected).abs() < 0.00001);
        assert!(condition.snow > 0.14 && condition.snow < 0.15);
        let rain = super::super::LocalWeather {
            rain: 0.7,
            wetness: 0.8,
            temperature: 14.0,
            ..default()
        };
        for _ in 0..60 {
            condition = evolve(condition, rain, 2.0, false);
        }
        assert_eq!(condition.snow, 0.0);
        assert!(condition.wetness > 0.95);
        condition.fire = 0.8;
        assert_eq!(evolve(condition, rain, 2.0, false).fire, 0.0);
        for _ in 0..600 {
            condition = evolve(condition, super::super::LocalWeather::default(), 2.0, false);
        }
        assert_eq!(condition.wetness, 0.0);
        assert_eq!(condition.charred, 0.0);
    }
}
