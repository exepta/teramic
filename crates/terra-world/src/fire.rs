use crate::{
    edits::EditJournal,
    streaming::{BlockEditRequest, ChunkStore, WorldAccess, commit_block_edit},
};
use bevy::prelude::*;
use std::collections::BTreeMap;
use terra_voxel::MaterialId as M;

fn damp(chunks: &ChunkStore, journal: &EditJournal, p: [i64; 3]) -> bool {
    let surface = journal.surface_condition(p);
    surface.wetness > 0.6
        || surface.snow > 0.25
        || crate::lava::NEIGHBORS
            .iter()
            .any(|d| crate::lava::wet(chunks, journal, std::array::from_fn(|i| p[i] + d[i])))
}

pub(crate) fn ignite_from_lava(chunks: &ChunkStore, journal: &mut EditJournal, p: [i64; 3]) {
    let mut s = journal.surface_condition(p);
    if s.fire == 0.
        && s.charred < 0.9
        && !damp(chunks, journal, p)
        && journal.active_fires().take(256).count() < 256
    {
        s.fire = 0.16;
        if let Err(error) = journal.set_surface_condition(p, s) {
            tracing::warn!(%error,"Lava ignition failed");
        }
    }
}

#[derive(Default)]
pub(crate) struct FireStep {
    last: f64,
    heat: BTreeMap<[i64; 3], f32>,
}

pub(crate) fn tick(
    time: Res<Time>,
    access: Res<WorldAccess>,
    config: Option<Res<terra_core::GameConfig>>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    properties: Option<Res<terra_core::block_properties::BlockPropertiesCatalog>>,
    mut step: Local<FireStep>,
) {
    let now = time.elapsed_secs_f64();
    if !access.authoritative() || now - step.last < 0.95 {
        return;
    }
    let dt = (now - step.last).min(1.1) as f32;
    step.last = now;
    let spread = config
        .as_ref()
        .is_none_or(|c| c.world.game_rules.fire_spread);
    let mut active: Vec<_> = journal
        .active_fires()
        .filter(|(p, _)| chunks.block_at(*p).is_some())
        .collect();
    active.sort_by_key(|(p, _)| *p);
    active.truncate(256);
    // Unheated cells cool. Neighbouring flames contribute additively before
    // deciding which cells have dried and ignited in this step.
    step.heat.retain(|_, h| {
        *h *= 0.82;
        *h > 0.06
    });
    let mut capacity = 256usize.saturating_sub(journal.active_fires().take(256).count());
    for (p, mut s) in active {
        let Some(material) = chunks.block_at(p).and_then(M::from_raw) else {
            continue;
        };
        if !material.flammable() || damp(&chunks, &journal, p) {
            s.fire = 0.;
        } else {
            let quick = M::FOLIAGE.contains(&(material as u16)) || material == M::Grass;
            s.charred = (s.charred
                + dt * (if quick { 0.093 } else { 0.045 }) * (0.4 + 0.6 * s.fire))
                .min(1.);
            let flame = (1. - s.charred * 0.65).min(0.99);
            s.fire = (s.fire + 0.18 * dt).min(flame);
            if spread {
                for offset in crate::lava::NEIGHBORS {
                    let target = std::array::from_fn(|i| p[i] + offset[i]);
                    let Some(fuel) = chunks.block_at(target).and_then(M::from_raw) else {
                        continue;
                    };
                    if !fuel.flammable()
                        || journal.surface_condition(target).fire > 0.
                        || damp(&chunks, &journal, target)
                    {
                        continue;
                    }
                    if !crate::lava::NEIGHBORS.iter().any(|d| {
                        chunks.block_at(std::array::from_fn(|i| target[i] + d[i])) == Some(0)
                    }) {
                        continue;
                    }
                    let rise = match offset[1] {
                        1 => 0.73,
                        -1 => 0.17,
                        _ => 0.46,
                    };
                    *step.heat.entry(target).or_default() += dt * s.fire * rise;
                }
            }
            if s.charred >= 0.995 {
                s.fire = 0.;
                let residue = if quick && material == M::Grass {
                    M::Dirt
                } else if matches!(material, M::Wood | M::SpruceLog | M::WeatheredWood) {
                    M::CharredLog
                } else {
                    M::Air
                };
                let fallback = terra_core::block_properties::BlockPropertiesCatalog::default();
                if let Ok(true) = commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &BlockEditRequest {
                        position: p,
                        expected_material: material as u16,
                        material: residue as u16,
                    },
                    properties.as_deref().unwrap_or(&fallback),
                ) {
                    continue;
                }
            }
        }
        if let Err(error) = journal.set_surface_condition(p, s) {
            tracing::warn!(%error,"Fire update rejected");
        }
    }
    step.heat.retain(|&p, heat| {
        let Some(fuel) = chunks.block_at(p).and_then(M::from_raw) else {
            return false;
        };
        let mut s = journal.surface_condition(p);
        if !spread || !fuel.flammable() || damp(&chunks, &journal, p) || s.fire > 0. {
            return false;
        }
        let ignition = if M::FOLIAGE.contains(&(fuel as u16)) || fuel == M::Grass {
            0.78
        } else {
            1.38
        };
        if *heat < ignition || capacity == 0 {
            // Gradual preheating darkens the exposed fuel before a flame transfers.
            let scorch = 0.13 * (*heat / ignition).clamp(0., 1.);
            if scorch > s.charred + 0.025 {
                s.charred = scorch;
                if let Err(error) = journal.set_surface_condition(p, s) {
                    tracing::warn!(%error, "Could not update scorched fuel");
                }
            }
            return true;
        }
        s.fire = 0.20;
        s.charred = s.charred.max(0.05);
        if journal.set_surface_condition(p, s).is_ok() {
            capacity -= 1;
        }
        false
    });
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
