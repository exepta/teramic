//! Avoid a nested compute-pool dispatch for sparse material classes.
use bevy::{
    asset::prelude::AssetChanged,
    ecs::{
        schedule::{ConditionWithAccess, SystemWithAccess},
        system::System,
    },
    pbr::{EntitiesNeedingSpecialization, check_entities_needing_specialization},
    prelude::*,
    utils::Parallel,
};
use std::{any::type_name, hash::Hash};

/// A full material class runs the stock parallel queries directly. Sparse
/// effects usually contain a handful of meshes, or none outside their biome.
const SERIAL_PROBE_LIMIT: usize = 64;

pub(crate) fn add<M: Material>(app: &mut App) -> &mut App
where
    M::Data: Eq + Hash + Clone,
{
    app.add_plugins(MaterialPlugin::<M>::default());
    let smoke = std::env::args().any(|arg| arg.starts_with("--smoke-test"));
    let mode = smoke
        .then(|| std::env::var("TERRA_SPARSE_MATERIAL_GUARDS").ok())
        .flatten();
    if mode.as_deref() != Some("0") {
        guard::<M>(app, mode.as_deref() != Some("guard"));
    }
    app
}

fn guard<M: Material>(app: &mut App, serial: bool) {
    let specialization =
        IntoSystem::into_system(check_entities_needing_specialization::<M>).system_type();
    // Bevy's second material invalidation system is private. Without debug
    // names, or if Bevy renames it, that system keeps its stock schedule.
    let mark = format!(
        "bevy_pbr::material::mark_meshes_as_changed_if_their_materials_changed<{}>",
        type_name::<M>()
    );
    app.edit_schedule(PostUpdate, |schedule| {
        let systems = &mut schedule.graph_mut().systems;
        let nodes: Vec<_> = systems
            .iter()
            .filter_map(|(key, system, _)| {
                if system.system_type() == specialization {
                    Some((key, true))
                } else if &*system.name() == mark {
                    Some((key, false))
                } else {
                    None
                }
            })
            .collect();
        for (key, specialize) in nodes {
            if serial {
                // Retain the original schedule node, ordering edges and conditions.
                // The same query ticks and removal readers serve both class sizes.
                *systems.get_mut(key).unwrap() = if specialize {
                    SystemWithAccess::new(Box::new(IntoSystem::into_system(
                        gather_specializations::<M>,
                    )))
                } else {
                    SystemWithAccess::new(Box::new(IntoSystem::into_system(
                        mark_changed_meshes::<M>,
                    )))
                };
            }
            let condition = if specialize {
                ConditionWithAccess::new(Box::new(IntoSystem::into_system(
                    needs_specialization::<M>,
                )))
            } else {
                ConditionWithAccess::new(Box::new(IntoSystem::into_system(needs_mesh_mark::<M>)))
            };
            systems.get_conditions_mut(key).unwrap().push(condition);
        }
    });
}

type MaterialChanges<M> = Or<(Changed<MeshMaterial3d<M>>, AssetChanged<MeshMaterial3d<M>>)>;
type SpecializationChanges<M> = Or<(
    Changed<Mesh3d>,
    AssetChanged<Mesh3d>,
    Changed<MeshMaterial3d<M>>,
    AssetChanged<MeshMaterial3d<M>>,
)>;

fn mark_changed_meshes<M: Material>(
    users: Query<(), With<MeshMaterial3d<M>>>,
    mut dirty: Query<&mut Mesh3d, MaterialChanges<M>>,
) {
    if users.count() > SERIAL_PROBE_LIMIT {
        dirty.par_iter_mut().for_each(|mut mesh| mesh.set_changed());
    } else {
        for mut mesh in &mut dirty {
            mesh.set_changed();
        }
    }
}

fn gather_specializations<M: Material>(
    users: Query<(), With<MeshMaterial3d<M>>>,
    dirty: Query<Entity, (SpecializationChanges<M>, With<MeshMaterial3d<M>>)>,
    parallel: Local<Parallel<Vec<Entity>>>,
    mut pending: ResMut<EntitiesNeedingSpecialization<M>>,
    mut meshes: RemovedComponents<Mesh3d>,
    mut materials: RemovedComponents<MeshMaterial3d<M>>,
) {
    if users.count() > SERIAL_PROBE_LIMIT {
        check_entities_needing_specialization::<M>(dirty, parallel, pending, meshes, materials);
        return;
    }
    pending.changed.clear();
    pending.changed.extend(dirty.iter());
    pending.removed.clear();
    // Bevy forwards all Mesh3d removals, including users of other materials.
    pending.removed.extend(meshes.read());
    pending.removed.extend(materials.read());
}

fn needs_mesh_mark<M: Material>(
    users: Query<(), With<MeshMaterial3d<M>>>,
    dirty: Query<(), (With<Mesh3d>, MaterialChanges<M>)>,
) -> bool {
    users.count() > SERIAL_PROBE_LIMIT || !dirty.is_empty()
}

fn needs_specialization<M: Material>(
    users: Query<(), With<MeshMaterial3d<M>>>,
    dirty: Query<(), (With<MeshMaterial3d<M>>, SpecializationChanges<M>)>,
    previous: Res<EntitiesNeedingSpecialization<M>>,
    mut meshes: RemovedComponents<Mesh3d>,
    mut materials: RemovedComponents<MeshMaterial3d<M>>,
) -> bool {
    // The stock helper forwards every removal and clears its prior results.
    // Each guard has independent cursors; the helper reads the same messages.
    let removed = !meshes.is_empty() || !materials.is_empty();
    meshes.clear();
    materials.clear();
    removed
        || !previous.changed.is_empty()
        || !previous.removed.is_empty()
        || users.count() > SERIAL_PROBE_LIMIT
        || !dirty.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;

    fn history(guarded: bool) -> Vec<(Vec<Entity>, Vec<Entity>, bool)> {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .add_plugins(MaterialPlugin::<StandardMaterial>::default());
        if guarded {
            guard::<StandardMaterial>(&mut app, true);
            let schedule = app.get_schedule(PostUpdate).unwrap();
            let id =
                IntoSystem::into_system(gather_specializations::<StandardMaterial>).system_type();
            let (_, _, conditions) = schedule
                .graph()
                .systems
                .iter()
                .find(|(_, body, _)| body.system_type() == id)
                .unwrap();
            assert_eq!(conditions.len(), 1);
        }
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::default());
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let tracked = app.world_mut().spawn_empty().id();
        let unrelated = app.world_mut().spawn(Mesh3d(mesh.clone())).id();
        let mut other = Vec::new();
        let mut history = Vec::new();
        let mut last_mesh_tick = None;
        for frame in 0..25 {
            match frame {
                1 | 15 | 18 => {
                    app.world_mut().entity_mut(unrelated).remove::<Mesh3d>();
                }
                13 | 16 => {
                    app.world_mut()
                        .entity_mut(unrelated)
                        .insert(Mesh3d(mesh.clone()));
                }
                2 => {
                    app.world_mut()
                        .entity_mut(tracked)
                        .insert((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())));
                }
                5 => {
                    app.world_mut()
                        .resource_mut::<Assets<StandardMaterial>>()
                        .get_mut(&material)
                        .unwrap()
                        .metallic = 0.4;
                }
                8 => {
                    app.world_mut()
                        .resource_mut::<Assets<Mesh>>()
                        .get_mut(&mesh)
                        .unwrap()
                        .invert_winding()
                        .unwrap();
                }
                10 => {
                    app.world_mut()
                        .entity_mut(tracked)
                        .remove::<MeshMaterial3d<StandardMaterial>>();
                }
                12 => {
                    app.world_mut().entity_mut(tracked).remove::<Mesh3d>();
                }
                14 => {
                    // Activation after several empty frames crosses the probe limit.
                    for _ in 0..=SERIAL_PROBE_LIMIT {
                        other.push(
                            app.world_mut()
                                .spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())))
                                .id(),
                        );
                    }
                }
                17 => {
                    // Cross back to precisely 64 users while a removal is pending.
                    app.world_mut().despawn(other.pop().unwrap());
                }
                20 => {
                    for entity in other.drain(..) {
                        app.world_mut().despawn(entity);
                    }
                }
                _ => {}
            }
            app.update();
            let pending = app
                .world()
                .resource::<EntitiesNeedingSpecialization<StandardMaterial>>();
            let mut changed = pending.changed.clone();
            let mut removed = pending.removed.clone();
            changed.sort_unstable();
            removed.sort_unstable();
            let tick = app
                .world()
                .entity(tracked)
                .get_ref::<Mesh3d>()
                .map(|mesh| mesh.last_changed());
            history.push((changed, removed, tick.is_some() && tick != last_mesh_tick));
            last_mesh_tick = tick;
        }
        history
    }

    #[test]
    fn sparse_material_guards_preserve_stock_add_change_remove_and_clearing_frames() {
        let stock = history(false);
        assert!(!stock[2].0.is_empty());
        for frame in [1, 15, 18] {
            assert_eq!(stock[frame].1.len(), 1);
            assert!(stock[frame + 1].1.is_empty());
        }
        assert_eq!(stock, history(true));
    }
}
