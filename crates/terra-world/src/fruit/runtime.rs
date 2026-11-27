use super::*;
use crate::{
    edits::EditJournal,
    props::{PropKind, WorldProp},
    streaming::{ChunkStore, SectionPublished, WorldAccess, WorldTerrain},
};
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use terra_voxel::{SectionPos, global_to_section};

const WOODLAND_FRUIT_SCALE: f32 = 1.45;
const DISCOVERY_BUDGET: usize = 32;
const REFRESH_BUDGET: usize = 32;
#[derive(PartialEq, Eq)]
struct Source {
    position: SectionPos,
    revision: Option<u64>,
    publication: u64,
}
struct Cached {
    revision: u64,
    source: Vec<Source>,
    props: Vec<WorldProp>,
}

#[derive(Resource, Default)]
pub struct FruitProps {
    pub revision: u64,
    pub props: Vec<WorldProp>,
}
#[derive(Default)]
pub(crate) struct Work {
    next: f64,
    column_cursor: usize,
    cursor: u64,
    trees: BTreeMap<u64, Cached>,
    published: BTreeMap<SectionPos, Vec<WorldProp>>,
    publications: BTreeMap<SectionPos, u64>,
    republished: BTreeSet<SectionPos>,
}

fn source(
    root: [i64; 3],
    chunks: &ChunkStore,
    publications: &BTreeMap<SectionPos, u64>,
    woodland: bool,
) -> Vec<Source> {
    let radius = if woodland { 7 } else { 4 };
    let height = if woodland { 23 } else { 16 };
    let low = global_to_section([root[0] - radius, root[1], root[2] - radius]).0;
    let high = global_to_section([root[0] + radius, root[1] + height, root[2] + radius]).0;
    let mut source = Vec::new();
    for x in low.x..=high.x {
        for y in low.y..=high.y {
            for z in low.z..=high.z {
                let position = SectionPos::new(x, y, z);
                source.push(Source {
                    position,
                    revision: chunks.loaded.get(&position).map(|s| s.data.revision()),
                    publication: publications.get(&position).copied().unwrap_or(0),
                });
            }
        }
    }
    source
}

/// Only touched fruit sections scan their local prop vector. Ordinary terrain,
/// prop epochs and ChunkStore change detection deliberately remain untouched.
fn publish(
    chunks: &mut ChunkStore,
    presentation: &mut FruitProps,
    work: &mut Work,
    registry: &terra_core::TreeRegistry,
) {
    let mut by_section: BTreeMap<SectionPos, Vec<WorldProp>> = BTreeMap::new();
    for (&id, cached) in &work.trees {
        let Some(tree) = registry.get(id).filter(|t| t.stage.is_mature()) else {
            continue;
        };
        let Some(fruit) = tree.fruit else {
            continue;
        };
        let variant = match fruit.phase {
            FruitPhase::Dormant => continue,
            FruitPhase::Blossom => 2,
            FruitPhase::Unripe => 1,
            FruitPhase::Ripe => 0,
        };
        for prop in &cached.props {
            if prop.variant != variant
                || slot(id, prop.id).is_none_or(|slot| {
                    fruit.phase == FruitPhase::Ripe && fruit.picked & (1 << slot) != 0
                })
            {
                continue;
            }
            if chunks.loaded.contains_key(&prop.section) {
                by_section.entry(prop.section).or_default().push(*prop);
            }
        }
    }
    for props in by_section.values_mut() {
        props.sort_unstable_by_key(|p| p.id);
    }
    let touched = work
        .published
        .keys()
        .chain(by_section.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    for position in touched {
        if work.published.get(&position) == by_section.get(&position)
            && !work.republished.contains(&position)
        {
            continue;
        }
        if let Some(section) = chunks.loaded.get_mut(&position) {
            section.props.retain(|p| p.kind != PropKind::AppleFruit);
            if let Some(props) = by_section.get(&position) {
                section.props.extend_from_slice(props);
            }
        }
    }
    work.published = by_section;
    work.republished.clear();
    let props = work
        .published
        .values()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    if presentation.props != props {
        presentation.props = props;
        presentation.revision = presentation.revision.wrapping_add(1);
    }
}

/// Locate actual exposed canopy cells; both replicas and the authority derive
/// the same eight slots. Unknown space and replaced leaves never hold fruit.
fn positions(
    tree: &TreeRecord,
    chunks: &ChunkStore,
    woodland: bool,
    scale: f32,
) -> [Option<[f64; 3]>; 8] {
    let directions = [
        [1, 0],
        [1, 1],
        [0, 1],
        [-1, 1],
        [-1, 0],
        [-1, -1],
        [0, -1],
        [1, -1],
    ];
    let legacy_radius = if tree.planted_by.is_some() { 2 } else { 3 };
    let distances: &[i64] = if woodland {
        &[6, 4, 2]
    } else {
        std::slice::from_ref(&legacy_radius)
    };
    let max_y = if woodland { 23 } else { 15 };
    let mut used = BTreeSet::new();
    std::array::from_fn(|slot| {
        let [dx, dz] = directions[slot];
        for &radius in distances {
            for ox in [0, -1, 1] {
                for oz in [0, -1, 1] {
                    for y in 2..=max_y {
                        let p = [
                            tree.root[0] + dx * radius + ox,
                            tree.root[1] + y,
                            tree.root[2] + dz * radius + oz,
                        ];
                        if !used.contains(&p)
                            && chunks.block_at(p) == Some(195)
                            && chunks.block_at([p[0], p[1] - 1, p[2]]) == Some(0)
                        {
                            used.insert(p);
                            return Some([
                                p[0] as f64 + 0.5,
                                p[1] as f64 - 0.425 * f64::from(scale),
                                p[2] as f64 + 0.5,
                            ]);
                        }
                    }
                }
            }
        }
        None
    })
}

fn derive(tree: &TreeRecord, chunks: &ChunkStore, woodland: bool) -> Vec<WorldProp> {
    let Some(fruit) = tree.fruit else {
        return Vec::new();
    };
    if !tree.stage.is_mature()
        || tree.species != TreeSpecies::Apple
        || fruit.phase == FruitPhase::Dormant
        || chunks.block_at(tree.root) != Some(tree.species.wood())
    {
        return Vec::new();
    }
    let scale = if woodland { WOODLAND_FRUIT_SCALE } else { 1.0 };
    positions(tree, chunks, woodland, scale)
        .into_iter()
        .enumerate()
        .filter_map(|(slot, position)| {
            if fruit.phase == FruitPhase::Ripe && fruit.picked & (1 << slot) != 0 {
                return None;
            }
            let position = position?;
            let mut prop = WorldProp {
                id: prop_id(tree.id, slot as u8),
                kind: PropKind::AppleFruit,
                position,
                rotation: 0.0,
                scale,
                variant: match fruit.phase {
                    FruitPhase::Blossom => 2,
                    FruitPhase::Unripe => 1,
                    _ => 0,
                },
                section: SectionPos::new(0, 0, 0),
                removed: false,
            };
            prop.section = global_to_section(crate::props::placement::support(&prop)).0;
            prop.valid().then_some(prop)
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn tick(
    time: Res<Time>,
    access: Res<WorldAccess>,
    terrain: Res<WorldTerrain>,
    clock: Option<Res<terra_core::WorldClock>>,
    mut sections: MessageReader<SectionPublished>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    mut presentation: ResMut<FruitProps>,
    mut work: Local<Work>,
) {
    // Read every frame: section messages can expire before the 4 Hz work interval.
    for event in sections.read() {
        let version = work.publications.entry(event.position).or_default();
        *version = version.wrapping_add(1);
        work.republished.insert(event.position);
    }
    if !access.ready() {
        work.trees.clear();
        work.publications.clear();
        publish(
            chunks.bypass_change_detection(),
            presentation.bypass_change_detection(),
            &mut work,
            &journal.gameplay().trees,
        );
        work.next = 0.;
        return;
    }
    if time.elapsed_secs_f64() < work.next {
        return;
    }
    work.next = time.elapsed_secs_f64() + 0.25;
    work.publications
        .retain(|p, _| chunks.loaded.contains_key(p));
    let clock = clock
        .as_deref()
        .copied()
        .or(journal.clock)
        .unwrap_or_default();
    let config = terrain.0.config();
    if access.authoritative()
        && config.ecology_revision >= 3
        && journal.gameplay().trees.trees.len() < terra_core::trees::MAX_TREES
    {
        // Discovery reads prepared columns. Each vertical stack already has
        // one entry here, and the root voxel is checked before registration.
        let mut columns: Vec<_> = chunks.prepared_column_coordinates().collect();
        columns.sort_unstable();
        let start = work.column_cursor.min(columns.len());
        for &coordinate in columns
            .iter()
            .skip(start)
            .chain(columns.iter().take(start))
            .take(DISCOVERY_BUDGET)
        {
            let Some(column) = chunks.prepared_column(coordinate) else {
                continue;
            };
            for (root, seed) in column.natural_apples() {
                let game = journal.gameplay();
                if game.revision == u64::MAX
                    || game.trees.revision == u64::MAX
                    || game.trees.trees.len() >= terra_core::trees::MAX_TREES
                {
                    break;
                }
                let id = natural_id(config.seed.0, root);
                if game.trees.get(id).is_some()
                    || chunks.block_at(root) != Some(6)
                    || journal.block(root).is_some()
                {
                    continue;
                }
                let tree = TreeRecord {
                    id,
                    root,
                    species: TreeSpecies::Apple,
                    seed,
                    planted_by: None,
                    stage: TreeStage::Mature,
                    growth_seconds: 0.0,
                    wood: Vec::new(),
                    leaves: Vec::new(),
                    bounds: None,
                    fruit: Some(terra_core::FruitState::new(clock.day)),
                    revision: 0,
                };
                if tree.valid() && journal.gameplay_mut().trees.insert(tree) {
                    journal.gameplay_mut().revision += 1;
                }
            }
        }
        work.column_cursor = if columns.is_empty() {
            0
        } else {
            (start + DISCOVERY_BUDGET) % columns.len()
        };
    }
    // A fair bounded cursor covers the entire authoritative registry. Each visit
    // checks the bounded canopy section stamps; expensive canopy searches run
    // only when a tree/voxel/publication stamp changed.
    let ids = visit_ids(&journal.gameplay().trees.trees, work.cursor);
    for id in ids {
        work.cursor = id;
        let Some(old) = journal.gameplay().trees.get(id) else {
            continue;
        };
        if old.species != TreeSpecies::Apple
            || !old.stage.is_mature()
            || chunks.block_at(old.root).is_none()
        {
            work.trees.remove(&id);
            continue;
        }
        if access.authoritative()
            && journal.gameplay().revision < u64::MAX
            && journal.gameplay().trees.revision < u64::MAX
            && old.revision < u64::MAX
        {
            let mut next = old.clone();
            let changed = if chunks.block_at(next.root) != Some(next.species.wood())
                || next.planted_by.is_none() && journal.block(next.root).is_some()
            {
                next.stage = next.stage.retired();
                next.fruit = None;
                next.revision += 1;
                true
            } else {
                advance(&mut next, clock, config.seasons)
            };
            if changed {
                let game = journal.gameplay_mut();
                game.trees.trees.insert(id, next);
                game.trees.changed();
                game.revision += 1;
            }
        }
        let tree = journal.gameplay().trees.get(id).unwrap();
        let source = source(
            tree.root,
            &chunks,
            &work.publications,
            config.ecology_revision >= 14,
        );
        if work
            .trees
            .get(&id)
            .is_some_and(|cached| cached.revision == tree.revision && cached.source == source)
        {
            continue;
        }
        let props = derive(tree, &chunks, config.ecology_revision >= 14);
        work.trees.insert(
            id,
            Cached {
                revision: tree.revision,
                source,
                props,
            },
        );
    }
    // Record removal/tombstones apply immediately without searching any voxels.
    work.trees.retain(|id, _| {
        journal
            .gameplay()
            .trees
            .get(*id)
            .is_some_and(|t| t.stage.is_mature())
    });
    publish(
        chunks.bypass_change_detection(),
        presentation.bypass_change_detection(),
        &mut work,
        &journal.gameplay().trees,
    );
}

fn visit_ids(trees: &BTreeMap<u64, TreeRecord>, cursor: u64) -> Vec<u64> {
    trees
        .range((
            std::ops::Bound::Excluded(cursor),
            std::ops::Bound::Unbounded,
        ))
        .chain(trees.range(..=cursor))
        .take(REFRESH_BUDGET)
        .map(|(&id, _)| id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::LoadedSection;
    use std::time::Duration;
    use terra_voxel::BlockSection;
    fn app(fruit: bool) -> App {
        let mut app = App::new();
        let config = terra_core::WorldConfig {
            ecology_revision: 0,
            ..Default::default()
        };
        app.insert_resource(Time::<()>::default())
            .insert_resource(WorldAccess::Replica { ready: true })
            .insert_resource(WorldTerrain(crate::TerraGenerator::new(config).unwrap()))
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .init_resource::<FruitProps>()
            .add_message::<SectionPublished>()
            .add_systems(Update, tick);
        let position = SectionPos::new(0, 0, 0);
        let mut data = BlockSection::empty(position).unwrap();
        if fruit {
            data.set_block([8, 1, 8], 6).unwrap();
            data.set_block([11, 6, 8], 195).unwrap();
            data.set_block([8, 6, 11], 195).unwrap();
        }
        let grass = WorldProp {
            id: crate::props::PropId(992),
            kind: PropKind::Grass,
            position: [2.5, 1., 2.5],
            rotation: 0.,
            scale: 1.,
            variant: 0,
            section: position,
            removed: false,
        };
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![grass; 4096],
            },
        );
        if fruit {
            let mut tree = super::super::tests::tree();
            advance(
                &mut tree,
                terra_core::WorldClock {
                    day: 11,
                    ..Default::default()
                },
                terra_core::SeasonConfig::default(),
            );
            assert!(
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .gameplay_mut()
                    .trees
                    .insert(tree)
            );
        }
        app
    }
    fn tick_app(app: &mut App) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(300));
        app.world_mut().run_schedule(Update);
    }
    #[test]
    fn taller_forest_crowns_hold_pickable_fruit_above_the_legacy_search_window() {
        let mut app = app(true);
        let tree = app
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .trees
            .trees
            .values()
            .next()
            .unwrap()
            .clone();
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        let high = SectionPos::new(0, 1, 0);
        let mut section = BlockSection::empty(high).unwrap();
        section.set_block([14, 3, 8], 195).unwrap();
        chunks.loaded.insert(
            high,
            LoadedSection {
                data: section,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        assert!(
            derive(&tree, &chunks, false)
                .iter()
                .all(|p| p.position[1] < 16.)
        );
        let props = derive(&tree, &chunks, true);
        let hanging = props.iter().find(|p| p.position[1] > 16.).unwrap();
        assert!(hanging.valid());
        assert_eq!(crate::props::placement::support(hanging), [14, 19, 8]);
        assert_eq!(hanging.section, high);
        assert!(slot(tree.id, hanging.id).is_some());
    }

    #[test]
    fn stable_fruit_tick_leaves_chunk_store_and_regular_prop_epochs_unchanged() {
        for fruit in [false, true] {
            let mut app = app(fruit);
            tick_app(&mut app);
            let position = SectionPos::new(0, 0, 0);
            let before = app.world().resource::<ChunkStore>().loaded[&position]
                .props
                .clone();
            let revision = app.world().resource::<FruitProps>().revision;
            assert_eq!(
                app.world().resource::<FruitProps>().props.len(),
                if fruit { 2 } else { 0 }
            );
            app.world_mut().clear_trackers();
            tick_app(&mut app);
            assert!(!app.world().resource_ref::<ChunkStore>().is_changed());
            assert!(!app.world().resource_ref::<EditJournal>().is_changed());
            assert_eq!(app.world().resource::<FruitProps>().revision, revision);
            assert_eq!(
                app.world().resource::<ChunkStore>().loaded[&position].props,
                before
            );
            assert_eq!(
                app.world().resource::<EditJournal>().prop_epoch(position),
                0
            );
            assert_eq!(
                app.world().resource::<ChunkStore>().loaded[&position].mesh_epoch,
                0
            );
        }
    }
    #[test]
    fn replacement_sections_restore_derived_fruit_and_leaf_edits_remove_only_supported_fruit() {
        let mut app = app(true);
        tick_app(&mut app);
        let position = SectionPos::new(0, 0, 0);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&position)
            .unwrap()
            .props
            .retain(|p| p.kind != PropKind::AppleFruit);
        app.world_mut().write_message(SectionPublished { position });
        app.world_mut().clear_trackers();
        tick_app(&mut app);
        assert!(!app.world().resource_ref::<ChunkStore>().is_changed());
        assert_eq!(
            app.world().resource::<ChunkStore>().loaded[&position]
                .props
                .iter()
                .filter(|p| p.kind == PropKind::AppleFruit)
                .count(),
            2
        );
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&position)
            .unwrap()
            .data
            .set_block([11, 6, 8], 0)
            .unwrap();
        app.world_mut().clear_trackers();
        tick_app(&mut app);
        assert!(!app.world().resource_ref::<ChunkStore>().is_changed());
        assert_eq!(app.world().resource::<FruitProps>().props.len(), 1);
        assert_eq!(
            app.world().resource::<ChunkStore>().loaded[&position]
                .props
                .iter()
                .filter(|p| p.kind == PropKind::Grass)
                .count(),
            4096
        );
    }
    #[test]
    fn bounded_registry_cursor_visits_all_trees_without_starving_low_ids() {
        let mut trees = BTreeMap::new();
        for id in 1..=1024 {
            let mut tree = super::super::tests::tree();
            tree.id = id;
            trees.insert(id, tree);
        }
        let mut cursor = 0;
        let mut visited = BTreeSet::new();
        for _ in 0..32 {
            let ids = visit_ids(&trees, cursor);
            assert_eq!(ids.len(), REFRESH_BUDGET);
            cursor = *ids.last().unwrap();
            visited.extend(ids);
        }
        assert_eq!(visited.len(), trees.len());
        assert_eq!(visit_ids(&trees, cursor).first(), Some(&1));
    }
}
