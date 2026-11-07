use crate::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess},
};
use bevy::prelude::*;

/// Sparse, bounded authority simulation. Clients only consume confirmed journal state.
pub(crate) fn tick(
    time: Res<Time>,
    access: Res<WorldAccess>,
    config: Option<Res<terra_core::GameConfig>>,
    chunks: Res<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    mut last: Local<f64>,
) {
    if !access.authoritative() || time.elapsed_secs_f64() - *last < 1.0 {
        return;
    }
    let elapsed = (time.elapsed_secs_f64() - *last).min(1.0) as f32;
    *last = time.elapsed_secs_f64();
    let mut active: Vec<_> = journal
        .active_fires()
        .filter(|(position, _)| chunks.block_at(*position).is_some())
        .collect();
    active.sort_by_key(|(p, _)| *p);
    active.truncate(256);
    let remaining = 256_usize.saturating_sub(journal.active_fires().take(256).count());
    let mut changes = std::collections::BTreeMap::new();
    for (position, mut condition) in active {
        if condition.wetness > 0.6
            || condition.snow > 0.25
            || !chunks
                .block_at(position)
                .and_then(terra_voxel::MaterialId::from_raw)
                .is_some_and(terra_voxel::MaterialId::flammable)
        {
            condition.fire = 0.0;
        } else {
            condition.fire = (condition.fire - 0.125 * elapsed).max(0.0);
            condition.charred = (condition.charred + 0.13 * elapsed).min(1.0);
            for offset in [[-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1], [0, 1, 0]] {
                if config
                    .as_ref()
                    .is_some_and(|config| !config.world.game_rules.fire_spread)
                {
                    break;
                }
                let target = std::array::from_fn(|i| position[i] + offset[i]);
                if !chunks
                    .block_at(target)
                    .and_then(terra_voxel::MaterialId::from_raw)
                    .is_some_and(terra_voxel::MaterialId::flammable)
                    || ![[-1, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1]]
                        .iter()
                        .any(|offset| {
                            chunks.block_at(std::array::from_fn(|i| target[i] + offset[i]))
                                == Some(0)
                        })
                {
                    continue;
                }
                let mut neighbor = journal.surface_condition(target);
                let hash = (target[0] as u64).wrapping_mul(0x9e3779b9)
                    ^ (target[2] as u64).rotate_left(17)
                    ^ (*last as u64);
                if neighbor.fire == 0.0
                    && neighbor.charred < 0.8
                    && neighbor.wetness < 0.6
                    && neighbor.snow < 0.25
                    && hash.is_multiple_of(4)
                    && changes.len() < remaining
                {
                    neighbor.fire = 1.0;
                    changes.entry(target).or_insert(neighbor);
                }
            }
        }
        changes.insert(position, condition);
    }
    for (position, condition) in changes {
        if let Err(error) = journal.set_surface_condition(position, condition) {
            tracing::warn!(%error, "Fire update rejected");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{environment::SurfaceCondition, streaming::LoadedSection};
    use terra_voxel::{BlockSection, SectionPos};
    fn scene(access: WorldAccess) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(access)
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs(1),
            ))
            .add_systems(Update, tick);
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_max_delta(std::time::Duration::from_secs(1));
        let position = SectionPos::default();
        let mut data = BlockSection::empty(position).unwrap();
        for z in 0..8 {
            for x in 0..8 {
                data.set_block([x, 0, z], 1).unwrap();
            }
        }
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_surface_condition(
                [3, 0, 3],
                SurfaceCondition {
                    fire: 1.0,
                    ..default()
                },
            )
            .unwrap();
        app
    }
    #[test]
    fn authority_spreads_bounded_fire_and_water_extinguishes_it() {
        let mut app = scene(WorldAccess::Local);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_surface_condition(
                [4, 0, 3],
                SurfaceCondition {
                    wetness: 1.0,
                    ..default()
                },
            )
            .unwrap();
        for _ in 0..5 {
            app.update();
        }
        let journal = app.world().resource::<EditJournal>();
        assert!(journal.active_fires().count() > 1);
        assert!(journal.active_fires().count() <= 256);
        assert_eq!(journal.surface_condition([4, 0, 3]).fire, 0.0);
        assert!(journal.surface_condition([3, 0, 3]).charred > 0.0);
        let mut condition = journal.surface_condition([3, 0, 3]);
        condition.wetness = 1.0;
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_surface_condition([3, 0, 3], condition)
            .unwrap();
        app.update();
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .surface_condition([3, 0, 3])
                .fire,
            0.0
        );
    }
    #[test]
    fn disabling_fire_spread_preserves_existing_fire_decay_without_igniting_neighbors() {
        let mut app = scene(WorldAccess::Local);
        let mut config = terra_core::GameConfig::default();
        config.world.game_rules.fire_spread = false;
        app.insert_resource(config);
        for _ in 0..5 {
            app.update();
        }
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.active_fires().count(), 1);
        assert!(journal.surface_condition([3, 0, 3]).fire < 1.0);
        assert!(journal.surface_condition([3, 0, 3]).charred > 0.0);
        assert_eq!(journal.surface_condition([4, 0, 3]).fire, 0.0);
    }
    #[test]
    fn replicas_never_advance_fire_and_old_surface_data_defaults_safely() {
        let mut app = scene(WorldAccess::Replica { ready: true });
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .surface_condition([3, 0, 3])
                .fire,
            1.0
        );
        let old = SurfaceCondition::new(0.2, 0.3).unwrap();
        assert_eq!(old.snow, 0.0);
        assert_eq!(old.fire, 0.0);
    }
}
