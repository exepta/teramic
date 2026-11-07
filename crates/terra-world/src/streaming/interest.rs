use super::resources::*;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use terra_voxel::{SectionPos, global_to_section};

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct InterestEnvironment<'w> {
    settings: Res<'w, StreamingSettings>,
    terrain: Res<'w, WorldTerrain>,
    journal: ResMut<'w, crate::edits::EditJournal>,
    removed: MessageWriter<'w, SectionRemoved>,
    columns_changed: MessageWriter<'w, PreparedColumnChanged>,
}

#[cfg(test)]
pub(super) fn wanted_sections(
    center: SectionPos,
    radius: i64,
    vertical_radius: i64,
) -> HashSet<SectionPos> {
    let mut wanted = HashSet::new();
    extend_wanted_sections(&mut wanted, center, radius, vertical_radius);
    wanted
}

fn extend_wanted_sections(
    wanted: &mut HashSet<SectionPos>,
    center: SectionPos,
    radius: i64,
    vertical_radius: i64,
) {
    for dz in -radius..=radius {
        for dy in -vertical_radius..=vertical_radius {
            for dx in -radius..=radius {
                if let Ok(p) = center.offset([dx, dy, dz]) {
                    wanted.insert(p);
                }
            }
        }
    }
}
pub(super) fn distance_squared(p: SectionPos, center: SectionPos) -> i128 {
    [p.x - center.x, p.y - center.y, p.z - center.z]
        .into_iter()
        .map(|v| i128::from(v) * i128::from(v))
        .sum()
}
pub(super) fn refresh_interest(
    focus: Res<ChunkFocus>,
    foci: Option<Res<ChunkFoci>>,
    mut environment: InterestEnvironment,
    mut chunks: ResMut<ChunkStore>,
    mut jobs: ResMut<PendingSections>,
    mut generation: ResMut<WorldGeneration>,
    (mut previous, mut previous_range): (Local<Vec<SectionPos>>, Local<Option<[i64; 2]>>),
) {
    let _scope = terra_core::profiling::ScopeTimer::new("streaming.interest");
    let settings = *environment.settings;
    if focus
        .0
        .iter()
        .any(|v| !v.is_finite() || v.abs() > (1_i64 << 40) as f64 - 32.0)
    {
        return;
    }
    let points = foci
        .as_ref()
        .and_then(|f| f.0.as_ref())
        .map_or_else(|| vec![focus.0], Clone::clone);
    let mut centers: Vec<_> = points
        .into_iter()
        .filter(|p| {
            p.iter()
                .all(|v| v.is_finite() && v.abs() < (1_i64 << 40) as f64 - 32.0)
        })
        .map(|p| global_to_section(p.map(|v| v.floor() as i64)).0)
        .collect();
    centers.sort_unstable_by_key(|p| (p.x, p.y, p.z));
    centers.dedup();
    let range = [settings.radius, settings.vertical_radius];
    if *previous == centers && *previous_range == Some(range) && !chunks.wanted.is_empty() {
        return;
    }
    *previous = centers.clone();
    *previous_range = Some(range);
    chunks.cave_views = if settings.meshes && environment.terrain.0.config().caves {
        centers
            .iter()
            .map(|&center| (center, settings.radius))
            .collect()
    } else {
        Vec::new()
    };
    let mut wanted = HashSet::new();
    for &center in &centers {
        extend_wanted_sections(
            &mut wanted,
            center,
            settings.radius,
            settings.vertical_radius,
        );
        let surface_height = chunks.columns.get(&[center.x, center.z]).map_or(
            environment.terrain.0.config().base_height,
            |column| {
                column
                    .water_columns()
                    .iter()
                    .map(|c| c.height)
                    .fold(f64::INFINITY, f64::min)
            },
        );
        extend_cave_interest(
            &mut wanted,
            center,
            environment.terrain.0.config().caves,
            surface_height,
            settings,
        );
    }
    chunks.surface_ranges.retain(|[x, z], _| {
        centers.iter().any(|center| {
            (*x - center.x).abs() <= settings.radius && (*z - center.z).abs() <= settings.radius
        })
    });
    chunks.columns.retain(|coordinate, _| {
        let [x, z] = *coordinate;
        let keep = centers.iter().any(|center| {
            (x - center.x).abs() <= settings.radius && (z - center.z).abs() <= settings.radius
        });
        if !keep {
            environment.columns_changed.write(PreparedColumnChanged {
                coordinate: *coordinate,
            });
        }
        keep
    });
    for (&coordinate, column) in &chunks.columns {
        extend_cave_sections_for_column(&mut wanted, coordinate, column, &chunks.cave_views);
    }
    for (&[x, z], &[low, high]) in &chunks.surface_ranges {
        for y in low..=high {
            wanted.insert(SectionPos::new(x, y, z));
        }
    }
    if environment.terrain.0.config().bounded_world {
        wanted.retain(|p| (-8..=22).contains(&p.y));
    }
    chunks.loaded.retain(|position, _| {
        let keep = wanted.contains(position);
        if !keep {
            environment.removed.write(SectionRemoved {
                position: *position,
            });
        }
        keep
    });
    environment
        .journal
        .bypass_change_detection()
        .suspend_unloaded_fluids(|position| chunks.loaded.contains_key(&position));
    chunks.dirty.retain(|p| wanted.contains(p));
    chunks.water_dirty.retain(|p| wanted.contains(p));
    jobs.0
        .retain(|_, job| job.positions.iter().any(|p| wanted.contains(p)));
    let mut queue: Vec<_> = wanted
        .iter()
        .copied()
        .filter(|p| !chunks.loaded.contains_key(p) && !jobs.contains(p))
        .collect();
    queue.sort_by_cached_key(|&p| {
        (
            centers
                .iter()
                .map(|&c| distance_squared(p, c))
                .min()
                .unwrap_or(0),
            p.y,
            p.z,
            p.x,
        )
    });
    chunks.queue = queue.into();
    chunks.wanted = wanted;
    generation.status = GenerationStatus::Pending;
    generation.wanted_sections = chunks.wanted.len();
    tracing::debug!(
        ?centers,
        requested = chunks.wanted.len(),
        "Chunk interest changed"
    );
}

pub(super) fn cave_sections_for_column(
    coordinate: [i64; 2],
    column: &crate::PreparedColumn,
    views: &[(SectionPos, i64)],
) -> Vec<SectionPos> {
    let mut sections = HashSet::new();
    extend_cave_sections_for_column(&mut sections, coordinate, column, views);
    let mut sections: Vec<_> = sections.into_iter().collect();
    sections.sort_unstable_by_key(|p| {
        (
            views
                .iter()
                .map(|(center, _)| distance_squared(*p, *center))
                .min()
                .unwrap_or(0),
            p.y,
        )
    });
    sections
}

fn extend_cave_sections_for_column(
    sections: &mut HashSet<SectionPos>,
    coordinate: [i64; 2],
    column: &crate::PreparedColumn,
    views: &[(SectionPos, i64)],
) {
    // Interest already owns the deduplicating set. Only publication needs a
    // separately ordered queue; rebuilding it for every loaded column wastes
    // allocations and sorting whenever the player crosses a section boundary.
    for &(center, radius) in views {
        if (coordinate[0] - center.x).abs() > radius || (coordinate[1] - center.z).abs() > radius {
            continue;
        }
        if let Some([low, high]) = column.cave_view_sections(center.y as f64 * 16.0 + 8.0) {
            sections.extend((low..=high).map(|y| SectionPos::new(coordinate[0], y, coordinate[1])));
        }
    }
}

fn extend_cave_interest(
    wanted: &mut HashSet<SectionPos>,
    center: SectionPos,
    caves: bool,
    surface_height: f64,
    settings: StreamingSettings,
) {
    if !caves {
        return;
    }
    let y = center.y as f64 * 16.0 + 8.0;
    if y > surface_height - 6.0 {
        return;
    }
    // Only the nearby descent corridor gets two extra layers (at most 18
    // sections per focus); distant underground columns retain their old budget.
    let radius = settings.radius.min(1);
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            for depth in 1..=2 {
                if let Ok(position) = center.offset([dx, -settings.vertical_radius - depth, dz]) {
                    wanted.insert(position);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_cave_interest_preserves_publication_order_union_and_existing_sections() {
        let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
            trees: false,
            structures_revision: 0,
            cave_structure_revision: 4,
            ..Default::default()
        })
        .unwrap();
        let generator = crate::SectionGenerator::new(terrain);
        for coordinate in [[-1, -2], [0, 0], [2, 3]] {
            let column = generator.prepare_column(coordinate, true).unwrap();
            for y in [-8, -4, -1, 0, 3, 8] {
                let near = SectionPos::new(coordinate[0], y, coordinate[1]);
                let views = [
                    (near, 1),
                    (near, 1),
                    (near.offset([1, 0, -1]).unwrap(), 2),
                    (near.offset([10, -2, 10]).unwrap(), 1),
                ];
                let retained = SectionPos::new(-100, -20, 100);
                let mut direct = HashSet::from([retained]);
                extend_cave_sections_for_column(&mut direct, coordinate, &column, &views);
                let mut expected = HashSet::from([retained]);
                let mut individual = Vec::new();
                for (center, radius) in views {
                    if (coordinate[0] - center.x).abs() <= radius
                        && (coordinate[1] - center.z).abs() <= radius
                        && let Some([low, high]) =
                            column.cave_view_sections(center.y as f64 * 16.0 + 8.0)
                    {
                        individual.extend((low..=high).map(|section_y| {
                            SectionPos::new(coordinate[0], section_y, coordinate[1])
                        }));
                    }
                }
                expected.extend(individual);
                assert_eq!(direct, expected);
                let ordered = cave_sections_for_column(coordinate, &column, &views);
                assert_eq!(
                    ordered.iter().copied().collect::<HashSet<_>>(),
                    expected.into_iter().filter(|p| *p != retained).collect()
                );
                assert!(ordered.windows(2).all(|pair| {
                    let key = |position: SectionPos| {
                        (
                            views
                                .iter()
                                .map(|(center, _)| distance_squared(position, *center))
                                .min()
                                .unwrap(),
                            position.y,
                        )
                    };
                    key(pair[0]) < key(pair[1])
                }));
            }
        }
    }

    #[test]
    fn direct_interest_union_keeps_overlapping_players_and_negative_sections() {
        let mut wanted = HashSet::new();
        for center in [SectionPos::new(-2, -3, -4), SectionPos::new(-1, -3, -4)] {
            extend_wanted_sections(&mut wanted, center, 1, 1);
            extend_wanted_sections(&mut wanted, center, 1, 1);
        }
        let mut expected = HashSet::new();
        for x in -3..=0 {
            for y in -4..=-2 {
                for z in -5..=-3 {
                    expected.insert(SectionPos::new(x, y, z));
                }
            }
        }
        assert_eq!(wanted, expected);
        assert_eq!(wanted.len(), 36);
    }

    #[test]
    fn interest_messages_identify_only_actual_section_and_column_evictions() {
        let terrain = crate::TerraGenerator::new(Default::default()).unwrap();
        let generator = crate::SectionGenerator::new(terrain.clone());
        let kept = SectionPos::default();
        let evicted = SectionPos::new(2, 0, 0);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ChunkStore>()
            .init_resource::<PendingSections>()
            .init_resource::<WorldGeneration>()
            .init_resource::<crate::edits::EditJournal>()
            .add_message::<SectionRemoved>()
            .add_message::<PreparedColumnChanged>()
            .insert_resource(ChunkFocus([8.0; 3]))
            .insert_resource(WorldTerrain(terrain))
            .insert_resource(StreamingSettings {
                radius: 0,
                vertical_radius: 0,
                max_jobs: 1,
                meshes: false,
            })
            .add_systems(Update, refresh_interest);
        {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            for position in [kept, evicted] {
                chunks.loaded.insert(
                    position,
                    LoadedSection {
                        data: terra_voxel::BlockSection::empty(position).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
                chunks.columns.insert(
                    [position.x, position.z],
                    std::sync::Arc::new(
                        generator
                            .prepare_column([position.x, position.z], false)
                            .unwrap(),
                    ),
                );
            }
        }
        let mut sections = bevy::ecs::message::MessageCursor::<SectionRemoved>::default();
        let mut columns = bevy::ecs::message::MessageCursor::<PreparedColumnChanged>::default();
        app.update();
        assert_eq!(
            sections
                .read(app.world().resource::<Messages<SectionRemoved>>())
                .map(|event| event.position)
                .collect::<Vec<_>>(),
            [evicted]
        );
        assert_eq!(
            columns
                .read(app.world().resource::<Messages<PreparedColumnChanged>>())
                .map(|event| event.coordinate)
                .collect::<Vec<_>>(),
            [[2, 0]]
        );
        let chunks = app.world().resource::<ChunkStore>();
        assert!(chunks.loaded.contains_key(&kept));
        assert!(chunks.prepared_column([0, 0]).is_some());
        assert!(!chunks.loaded.contains_key(&evicted));
        assert!(chunks.prepared_column([2, 0]).is_none());
        app.update();
        assert_eq!(
            sections
                .read(app.world().resource::<Messages<SectionRemoved>>())
                .count(),
            0
        );
        assert_eq!(
            columns
                .read(app.world().resource::<Messages<PreparedColumnChanged>>())
                .count(),
            0
        );
    }

    #[test]
    fn stationary_interest_tracks_vertical_setting_changes_and_releases_evicted_sections() {
        let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
            caves: false,
            bounded_world: false,
            ..Default::default()
        })
        .unwrap();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ChunkStore>()
            .init_resource::<PendingSections>()
            .init_resource::<WorldGeneration>()
            .init_resource::<crate::edits::EditJournal>()
            .add_message::<SectionRemoved>()
            .add_message::<PreparedColumnChanged>()
            .insert_resource(ChunkFocus([-8.0; 3]))
            .insert_resource(WorldTerrain(terrain))
            .insert_resource(StreamingSettings {
                radius: 0,
                vertical_radius: 0,
                max_jobs: 1,
                meshes: false,
            })
            .add_systems(Update, refresh_interest);
        app.update();
        assert_eq!(app.world().resource::<ChunkStore>().wanted.len(), 1);
        app.world_mut()
            .resource_mut::<StreamingSettings>()
            .vertical_radius = 1;
        app.update();
        {
            let chunks = app.world().resource::<ChunkStore>();
            assert_eq!(chunks.wanted.len(), 3);
            assert_eq!(chunks.queue.len(), 3);
            assert!(chunks.wanted.contains(&SectionPos::new(-1, -2, -1)));
        }
        let evicted = SectionPos::new(-1, -2, -1);
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            evicted,
            LoadedSection {
                data: terra_voxel::BlockSection::empty(evicted).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut removed = bevy::ecs::message::MessageCursor::<SectionRemoved>::default();
        app.world_mut()
            .resource_mut::<StreamingSettings>()
            .vertical_radius = 0;
        app.update();
        let chunks = app.world().resource::<ChunkStore>();
        assert_eq!(chunks.wanted.len(), 1);
        assert!(!chunks.loaded.contains_key(&evicted));
        assert_eq!(
            removed
                .read(app.world().resource::<Messages<SectionRemoved>>())
                .count(),
            1
        );
    }

    #[test]
    fn deep_descent_prefetch_is_bounded_near_the_player_at_negative_coordinates() {
        let terrain = crate::TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
        let center = SectionPos::new(-3, -20, -2);
        let settings = StreamingSettings {
            radius: 12,
            vertical_radius: 3,
            max_jobs: 4,
            meshes: true,
        };
        let mut wanted = wanted_sections(center, settings.radius, settings.vertical_radius);
        let original = wanted.len();
        extend_cave_interest(
            &mut wanted,
            center,
            true,
            terrain.config().base_height,
            settings,
        );
        assert_eq!(wanted.len(), original + 18);
        assert!(wanted.contains(&SectionPos::new(-3, -25, -2)));
        assert!(!wanted.contains(&SectionPos::new(-5, -25, -2)));
        let elevated = SectionPos::new(-3, 100, -2);
        let mut sky = HashSet::new();
        extend_cave_interest(
            &mut sky,
            elevated,
            true,
            terrain.config().base_height,
            settings,
        );
        assert!(sky.is_empty());
    }
}
