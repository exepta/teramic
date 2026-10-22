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

#[derive(Default)]
pub(crate) struct SurfaceWork {
    next: f64,
    cursor: usize,
    visited: HashMap<[i64; 2], f64>,
    sequence: u64,
    weather_elapsed: Option<f64>,
}

/// One already-loaded 16x16 column per two seconds. Never dispatches generation.
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
        || time.elapsed_secs_f64() < work.next
    {
        return;
    }
    work.next = time.elapsed_secs_f64() + 2.0;
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
        return;
    }
    let mut columns: BTreeMap<[i64; 2], Vec<terra_voxel::SectionPos>> = BTreeMap::new();
    for &section in chunks.loaded.keys() {
        columns
            .entry([section.x, section.z])
            .or_default()
            .push(section);
    }
    // A still-streaming column without prepared climate data is not a job yet;
    // otherwise its unchanged "unvisited" priority could block all ready work.
    columns.retain(|coordinate, _| chunks.prepared_column(*coordinate).is_some());
    if columns.is_empty() {
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
            .filter(|(coordinate, _)| !columns.contains_key(*coordinate))
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
    let Some(coordinate) = select_column(
        &columns,
        &work.visited,
        &centers,
        weather.elapsed_total,
        work.cursor,
    ) else {
        return;
    };
    work.cursor = work.cursor.wrapping_add(1);
    let Some(mut sections) = columns.remove(&coordinate) else {
        return;
    };
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
            deposition.snow *= weather.snow_accumulation_multiplier() * snow_elapsed / elapsed;
            let mut next = evolve(before, deposition, elapsed, winter);
            if !super::retains_snow(chunks.block_at(position).unwrap_or(0)) {
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
    let sheds_snow = material == 5 || !super::retains_snow(material);
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
        (amount.clamp(0.0, 1.0) * 8.0 + 0.0001).floor() as u8
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
        crate::streaming::commit_block_edit(
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
    next.snow = (before.snow + local.snow * 0.003 * elapsed - melt).clamp(0.0, 1.0);
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

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::{BlockSection, SectionPos};

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
    fn beds_furnaces_chests_and_workbenches_shed_snow_without_snowing_through_them() {
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
        for id in [28, 29, 59, 71, 72, 73] {
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
    fn snow_builds_eight_real_layers_under_trees_and_never_overwrites_player_blocks() {
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
            assert_eq!(chunks.block_at(cap), Some(if state == 8 { 5 } else { 98 }));
            assert_eq!(journal.object_state(cap).unwrap().facing, state);
            let mut actual = Vec::new();
            crate::interaction_shapes::visit_boxes(&chunks, &journal, &properties, cap, |b| {
                actual.push(b)
            });
            if state < 8 {
                assert_eq!(actual[0].maximum[1], 2.0 + f64::from(state) / 8.0);
            }
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
            "a full natural snow cap cannot grow another cap"
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
        assert_eq!(journal.object_state(cap).unwrap().facing, 4);
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
            evolve(SurfaceCondition::default(), cold, 600.0, false).snow,
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
        assert!(condition.snow > 0.5 && condition.snow < 0.6);
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
