//! Slow, bounded surface succession. Sprouts use existing saved prop batches;
//! only the final Dirt->Grass transition invalidates terrain geometry.
use crate::{
    edits::EditJournal,
    props::{PropId, PropKind, WorldProp, placement::support},
    streaming::{BlockEditRequest, ChunkStore, WorldAccess, WorldTerrain, commit_block_edit},
};
use bevy::prelude::*;
use std::collections::BTreeMap;
use terra_core::block_properties::BlockPropertiesCatalog;
use terra_voxel::{MaterialId, global_to_section};

const ACTIVE_LIMIT: usize = 256;
const DISCOVERY_SECTIONS: usize = 2;
const CHANGES_PER_SECOND: usize = 8;

#[derive(Default)]
pub(crate) struct Growth {
    last: f64,
    scan: usize,
    active: BTreeMap<[i64; 3], f64>,
}

pub(crate) fn schedule(journal: &mut EditJournal, position: [i64; 3]) {
    for y in -1..=1 {
        for z in -1..=1 {
            for x in -1..=1 {
                if journal.grass_candidates.len() >= 4096 {
                    return;
                }
                let target = [position[0] + x, position[1] + y, position[2] + z];
                if journal.grass_queued.insert(target) {
                    journal.grass_candidates.push_back(target);
                }
            }
        }
    }
}

fn exposed(
    position: [i64; 3],
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    terrain: Option<&WorldTerrain>,
) -> bool {
    if chunks.block_at(position) != Some(MaterialId::Dirt as u16) {
        return false;
    }
    let above = [position[0], position[1] + 1, position[2]];
    let clear = chunks.block_at(above).is_some_and(|material| {
        material == 0 || properties.0.get(&material).is_some_and(|p| p.struct_block)
    });
    let condition = journal.surface_condition(position);
    clear
        && condition.fire == 0.0
        && condition.charred < 0.6
        && condition.snow < 0.5
        && journal.fluid(above).is_none_or(|cell| cell.amount == 0)
        && terrain.is_none_or(|terrain| {
            terrain
                .0
                .water_column(position[0] as f64 + 0.5, position[2] as f64 + 0.5)
                .is_none_or(|water| water.level <= above[1] as f64)
        })
}

fn grass_near(position: [i64; 3], chunks: &ChunkStore) -> bool {
    (-1..=1).any(|y| {
        (-1..=1).any(|z| {
            (-1..=1).any(|x| {
                chunks.block_at([position[0] + x, position[1] + y, position[2] + z])
                    == Some(MaterialId::Grass as u16)
            })
        })
    })
}

fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn sprout(position: [i64; 3], journal: &EditJournal, existing_tuft: bool) -> WorldProp {
    let section = global_to_section(position).0;
    let seed = mix((position[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (position[1] as u64).rotate_left(27)
        ^ (position[2] as u64).wrapping_mul(0xbf58476d1ce4e5b9)
        ^ journal.revision(section)
        ^ 0x4752_4f57_4752_4153);
    WorldProp {
        id: PropId(seed),
        kind: PropKind::Grass,
        position: [
            position[0] as f64 + 0.5,
            position[1] as f64 + 1.0,
            position[2] as f64 + 0.5,
        ],
        rotation: (seed & 3) as f32 * std::f32::consts::FRAC_PI_2,
        scale: 0.12,
        variant: if existing_tuft { 68 } else { 64 },
        section,
        removed: false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn tick(
    time: Res<Time>,
    access: Res<WorldAccess>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    properties: Res<BlockPropertiesCatalog>,
    terrain: Option<Res<WorldTerrain>>,
    weather: Option<Res<terra_core::WorldWeather>>,
    clock: Option<Res<terra_core::WorldClock>>,
    mut state: Local<Growth>,
) {
    let now = time.elapsed_secs_f64();
    if !access.authoritative()
        || clock.as_ref().is_some_and(|clock| clock.paused)
        || now - state.last < 1.0
    {
        return;
    }
    state.last = now;
    let terrain = terrain.as_deref();
    let weather = weather.as_deref().copied().unwrap_or_default();
    let clock = clock.as_deref().copied().unwrap_or_default();
    // Discovery is amortized and allocation is bounded by loaded section count,
    // never by section volume. Edits also enqueue their immediate neighbors.
    let mut sections: Vec<_> = chunks.loaded.keys().copied().collect();
    sections.sort_unstable_by_key(|section| (section.x, section.y, section.z));
    for _ in 0..DISCOVERY_SECTIONS.min(sections.len()) {
        let section = sections[state.scan % sections.len()];
        state.scan = state.scan.wrapping_add(1);
        let Some(loaded) = chunks.loaded.get(&section) else {
            continue;
        };
        for prop in loaded
            .props
            .iter()
            .filter(|prop| !prop.removed && prop.growing_grass())
        {
            if state.active.len() < ACTIVE_LIMIT {
                state.active.entry(support(prop)).or_insert(now + 8.0);
            }
        }
        for (index, &material) in loaded.data.blocks().iter().enumerate() {
            if material != MaterialId::Dirt as u16 || journal.grass_candidates.len() >= 4096 {
                continue;
            }
            let local = [index % 16, index / 16 % 16, index / 256];
            if let Ok(position) = section.global_block(local)
                && exposed(position, &chunks, &journal, &properties, terrain)
                && grass_near(position, &chunks)
                && journal.grass_queued.insert(position)
            {
                journal.grass_candidates.push_back(position);
            }
        }
    }
    for _ in 0..128.min(journal.grass_candidates.len()) {
        let Some(position) = journal.grass_candidates.pop_front() else {
            break;
        };
        journal.grass_queued.remove(&position);
        if state.active.len() < ACTIVE_LIMIT
            && exposed(position, &chunks, &journal, &properties, terrain)
            && grass_near(position, &chunks)
        {
            let delay =
                8.0 + (mix(position[0] as u64 ^ (position[2] as u64).rotate_left(17)) % 9) as f64;
            state.active.entry(position).or_insert(now + delay);
        }
    }
    let due: Vec<_> = state
        .active
        .iter()
        .filter(|(_, due)| **due <= now)
        .map(|(&position, _)| position)
        .take(CHANGES_PER_SECOND)
        .collect();
    for position in due {
        // Only cached climate: a not-yet-prepared column waits instead of
        // generating terrain synchronously or guessing that it is temperate.
        let rate = terrain.map_or(1.0, |terrain| {
            chunks
                .surface_column_at(position[0], position[2])
                .map_or(0.0, |surface| {
                    crate::weather::ecology::EcologyClimate::sample(
                        &weather,
                        surface.biome,
                        position[1] as f64 + 1.0,
                        terrain.0.config().sea_level,
                        clock.season(terrain.0.config().seasons),
                        journal.surface_condition(position).wetness,
                    )
                    .grass_rate()
                })
        });
        let owner = global_to_section(position).0;
        let Some(section) = chunks.loaded.get(&owner) else {
            state.active.remove(&position);
            continue;
        };
        let existing = section
            .props
            .iter()
            .find(|prop| !prop.removed && prop.growing_grass() && support(prop) == position)
            .copied();
        if !exposed(position, &chunks, &journal, &properties, terrain)
            || !grass_near(position, &chunks)
        {
            if let Some(prop) = existing
                && let Err(error) = journal.remove_prop(owner, prop.id)
            {
                tracing::warn!(%error, "Cannot retire grass sprout");
            }
            state.active.remove(&position);
        } else if rate <= 0.0 {
            // Winter/drought pauses an existing saved sprout; it is not deleted.
            state.active.insert(position, now + 8.0);
        } else if let Some(mut prop) = existing {
            if prop.variant % 4 == 3 {
                match commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &BlockEditRequest {
                        position,
                        expected_material: MaterialId::Dirt as u16,
                        material: MaterialId::Grass as u16,
                    },
                    &properties,
                ) {
                    Ok(true) => {
                        if let Err(error) = journal.remove_prop(owner, prop.id) {
                            tracing::warn!(%error, "Cannot complete grass sprout");
                        }
                    }
                    Ok(false) => {}
                    Err(error) => tracing::warn!(%error, "Grass succession rejected"),
                }
                state.active.remove(&position);
            } else {
                let existing_tuft = chunks.loaded.get(&owner).is_some_and(|section| {
                    section.props.iter().any(|candidate| {
                        !candidate.removed
                            && candidate.kind == PropKind::Grass
                            && !candidate.growing_grass()
                            && support(candidate) == position
                    })
                });
                let stage = (prop.variant - 64) % 4;
                prop.variant = 65 + stage + if existing_tuft { 4 } else { 0 };
                prop.scale = [0.12, 0.25, 0.43, 0.65][usize::from((prop.variant - 64) % 4)];
                if let Err(error) = journal.update_placed_prop(prop) {
                    tracing::warn!(%error, "Grass sprout update rejected");
                }
                state
                    .active
                    .insert(position, now + 8.0 / f64::from(rate.max(0.05)));
            }
        } else {
            let existing_tuft = chunks.loaded.get(&owner).is_some_and(|section| {
                section.props.iter().any(|candidate| {
                    !candidate.removed
                        && candidate.kind == PropKind::Grass
                        && !candidate.growing_grass()
                        && support(candidate) == position
                })
            });
            let prop = sprout(position, &journal, existing_tuft);
            let placed = match journal.place_prop(prop) {
                Ok(placed) => placed,
                Err(error) => {
                    tracing::warn!(%error, "Grass sprout placement rejected");
                    false
                }
            };
            if placed {
                state
                    .active
                    .insert(position, now + 8.0 / f64::from(rate.max(0.05)));
            } else {
                state.active.remove(&position);
            }
        }
        if let Some(section) = chunks.loaded.get_mut(&owner) {
            journal.apply_prop_removals(owner, &mut section.props);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::LoadedSection;
    use std::time::Duration;
    use terra_core::block_properties::BlockProperties;
    use terra_voxel::{BlockSection, SectionPos};

    const TARGET: [i64; 3] = [-14, 0, -14];
    fn scene(access: WorldAccess, cover: u16, neighbor: bool) -> App {
        let mut app = App::new();
        let mut catalog = BlockPropertiesCatalog::default();
        catalog.0.insert(
            28,
            BlockProperties {
                struct_block: true,
                ..Default::default()
            },
        );
        app.add_plugins(MinimalPlugins)
            .insert_resource(access)
            .insert_resource(catalog)
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_secs(1),
            ))
            .add_systems(Update, tick);
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_max_delta(Duration::from_secs(1));
        let owner = global_to_section(TARGET).0;
        let mut data = BlockSection::empty(owner).unwrap();
        data.set_block([2, 0, 2], MaterialId::Dirt as u16).unwrap();
        data.set_block([2, 1, 2], cover).unwrap();
        if neighbor {
            data.set_block([1, 0, 2], MaterialId::Grass as u16).unwrap();
        }
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        app
    }

    fn frames(app: &mut App, count: usize) {
        for _ in 0..count {
            app.update();
        }
    }

    #[test]
    fn prepared_regional_soil_waits_through_winter_and_resumes_in_summer() {
        let mut app = scene(WorldAccess::Server, 0, true);
        let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
            biomes: false,
            water: false,
            trees: false,
            caves: false,
            base_height: 0.,
            hill_amplitude: 0.,
            mountain_amplitude: 0.,
            ..Default::default()
        })
        .unwrap();
        let column = crate::SectionGenerator::new(terrain.clone())
            .prepare_column([-1, -1], false)
            .unwrap();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .set_prepared_column_for_test([-1, -1], Some(std::sync::Arc::new(column)));
        app.insert_resource(WorldTerrain(terrain));
        app.insert_resource(terra_core::WorldClock {
            day: 21,
            ..Default::default()
        });
        frames(&mut app, 120);
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(TARGET),
            Some(2)
        );
        assert!(
            app.world().resource::<ChunkStore>().loaded[&global_to_section(TARGET).0]
                .props
                .is_empty()
        );
        app.world_mut().resource_mut::<terra_core::WorldClock>().day = 7;
        frames(&mut app, 150);
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(TARGET),
            Some(1)
        );
        assert_eq!(app.world().resource::<EditJournal>().block(TARGET), Some(1));
    }

    #[test]
    fn dirt_visibly_sprouts_then_becomes_grass_without_early_terrain_rebuilds() {
        let mut app = scene(WorldAccess::Server, 0, true);
        frames(&mut app, 23);
        let owner = global_to_section(TARGET).0;
        let store = app.world().resource::<ChunkStore>();
        let sprout = *store.loaded[&owner]
            .props
            .iter()
            .find(|p| p.growing_grass() && !p.removed)
            .unwrap();
        assert!(sprout.scale <= 0.43);
        assert_eq!(store.block_at(TARGET), Some(2));
        assert_eq!(app.world().resource::<EditJournal>().mesh_epoch(owner), 0);
        assert!(
            crate::props::raycast_props([-13.5, 2.0, -13.5], [0.0, -1.0, 0.0], 4.0, [&sprout])
                .is_none()
        );
        frames(&mut app, 9);
        let store = app.world().resource::<ChunkStore>();
        let grown = store.loaded[&owner]
            .props
            .iter()
            .find(|p| p.id == sprout.id)
            .unwrap();
        assert!(grown.scale > sprout.scale);
        frames(&mut app, 40);
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(TARGET),
            Some(1)
        );
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.block(TARGET), Some(1));
        assert!(journal.prop_removed(owner, sprout.id));
        assert_eq!(journal.revision(owner), 1);
    }

    #[test]
    fn saved_sprout_stage_resumes_after_delta_import_and_section_reload() {
        let mut original = scene(WorldAccess::Server, 0, true);
        frames(&mut original, 28);
        let owner = global_to_section(TARGET).0;
        let delta = original
            .world()
            .resource::<EditJournal>()
            .section_delta(owner);
        assert_eq!(delta.placed_props.len(), 1);
        let saved = delta.placed_props[0];
        let mut loaded = scene(WorldAccess::Server, 0, true);
        loaded
            .world_mut()
            .resource_mut::<EditJournal>()
            .import_delta(delta)
            .unwrap();
        loaded
            .world_mut()
            .resource_scope(|world, journal: Mut<EditJournal>| {
                let mut store = world.resource_mut::<ChunkStore>();
                journal
                    .apply_prop_removals(owner, &mut store.loaded.get_mut(&owner).unwrap().props);
            });
        frames(&mut loaded, 1);
        assert_eq!(
            loaded.world().resource::<ChunkStore>().loaded[&owner].props[0],
            saved
        );
        frames(&mut loaded, 55);
        assert_eq!(
            loaded.world().resource::<ChunkStore>().block_at(TARGET),
            Some(1)
        );
    }

    #[test]
    fn four_flowers_do_not_block_soil_growth_and_existing_tufts_are_not_duplicated() {
        for (kind, count) in [(PropKind::Flower, 4), (PropKind::Grass, 1)] {
            let mut app = scene(WorldAccess::Server, 0, true);
            let owner = global_to_section(TARGET).0;
            let mut plants = Vec::new();
            for index in 0..count {
                let mut plant = WorldProp {
                    id: PropId(700 + index as u64),
                    kind,
                    position: [0.0; 3],
                    rotation: 0.0,
                    scale: 0.7,
                    variant: 0,
                    section: owner,
                    removed: false,
                };
                crate::props::placement::align(&mut plant, TARGET, index);
                plants.push(plant);
            }
            crate::props::placement::reflow(&mut plants);
            let expected = plants.clone();
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&owner)
                .unwrap()
                .props = plants;
            frames(&mut app, 24);
            let chunks = app.world().resource::<ChunkStore>();
            let growth = chunks.loaded[&owner]
                .props
                .iter()
                .find(|p| !p.removed && p.growing_grass())
                .unwrap();
            assert_eq!(growth.hidden_growth(), kind == PropKind::Grass);
            for plant in &expected {
                assert_eq!(
                    chunks.loaded[&owner]
                        .props
                        .iter()
                        .find(|p| p.id == plant.id),
                    Some(plant)
                );
            }
            frames(&mut app, 60);
            let chunks = app.world().resource::<ChunkStore>();
            assert_eq!(chunks.block_at(TARGET), Some(1));
            for plant in &expected {
                assert_eq!(
                    chunks.loaded[&owner]
                        .props
                        .iter()
                        .find(|p| p.id == plant.id),
                    Some(plant)
                );
            }
            assert_eq!(
                chunks.loaded[&owner]
                    .props
                    .iter()
                    .filter(|p| !p.removed)
                    .count(),
                expected.len()
            );
        }
    }

    #[test]
    fn covered_isolated_or_replica_dirt_does_not_grow_but_structural_cover_preserves_it() {
        for (access, cover, neighbor) in [
            (WorldAccess::Server, 3, true),
            (WorldAccess::Server, 0, false),
            (WorldAccess::Replica { ready: true }, 0, true),
        ] {
            let mut app = scene(access, cover, neighbor);
            frames(&mut app, 90);
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(TARGET),
                Some(2)
            );
            assert_eq!(
                app.world()
                    .resource::<EditJournal>()
                    .prop_epoch(SectionPos::new(-1, 0, -1)),
                0
            );
        }
        let mut app = scene(WorldAccess::Server, 28, true);
        frames(&mut app, 90);
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(TARGET),
            Some(1)
        );
    }

    #[test]
    fn newly_covered_sprouts_stop_and_candidate_queue_has_a_hard_bound() {
        let mut app = scene(WorldAccess::Server, 0, true);
        frames(&mut app, 22);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&SectionPos::new(-1, 0, -1))
            .unwrap()
            .data
            .set_block([2, 1, 2], 3)
            .unwrap();
        frames(&mut app, 25);
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(TARGET),
            Some(2)
        );
        assert!(
            app.world().resource::<ChunkStore>().loaded[&SectionPos::new(-1, 0, -1)]
                .props
                .iter()
                .all(|p| p.removed)
        );
        let mut journal = EditJournal::default();
        for x in 0..1000 {
            schedule(&mut journal, [x, 0, 0]);
        }
        assert!(journal.grass_candidates.len() <= 4096);
        assert_eq!(journal.grass_candidates.len(), journal.grass_queued.len());
    }
}
