//! Opt-in exact PostUpdate bodies without removing nodes, edges or conditions.
use super::*;
use bevy::ecs::{
    schedule::{
        FlattenedDependencies, NodeId, ScheduleBuildError, ScheduleBuildPass, ScheduleGraph,
        Schedules, SystemKey, SystemSetKey, SystemWithAccess, graph::DiGraph,
    },
    system::{CombinatorSystem, Combine, RunSystemError, System, SystemIn},
};
use bevy::platform::hash::FixedHasher;
use indexmap::IndexSet;

struct Timed;
impl<A: System<In = (), Out = ()>, B: System<In = (), Out = &'static str>> Combine<A, B> for Timed {
    type In = ();
    type Out = ();
    fn combine<T>(
        _: (),
        data: &mut T,
        a: impl FnOnce(SystemIn<'_, A>, &mut T) -> Result<(), RunSystemError>,
        b: impl FnOnce(SystemIn<'_, B>, &mut T) -> Result<&'static str, RunSystemError>,
    ) -> Result<(), RunSystemError> {
        let name = b((), data)?;
        let _scope = ScopeTimer::new(name);
        a((), data)
    }
}

#[derive(Debug)]
struct Bodies;
impl ScheduleBuildPass for Bodies {
    type EdgeOptions = ();
    fn add_dependency(&mut self, _: NodeId, _: NodeId, _: Option<&()>) {}
    fn collapse_set(
        &mut self,
        _: SystemSetKey,
        _: &IndexSet<SystemKey, FixedHasher>,
        _: &DiGraph<NodeId>,
    ) -> impl Iterator<Item = (NodeId, NodeId)> {
        std::iter::empty()
    }
    fn build(
        &mut self,
        world: &mut World,
        graph: &mut ScheduleGraph,
        _: FlattenedDependencies<'_>,
    ) -> Result<(), ScheduleBuildError> {
        let keys: Vec<_> = graph.systems.iter().map(|(key, _, _)| key).collect();
        for key in keys {
            let Some(system) = graph.systems.get_mut(key) else {
                continue;
            };
            let original_name = system.name().to_string();
            if original_name.starts_with("system.body.") || original_name.contains("ApplyDeferred")
            {
                continue;
            }
            // Allocate one bounded diagnostic label per schedule node; repeated
            // graph builds recognize their wrapper instead of allocating again.
            let name: &'static str =
                Box::leak(format!("system.body.{original_name}").into_boxed_str());
            let last_run = system.get_last_run();
            let old = std::mem::replace(
                system,
                SystemWithAccess::new(Box::new(IntoSystem::into_system(|| {}))),
            );
            let wrapped = CombinatorSystem::<Timed, _, _>::new(
                old,
                IntoSystem::into_system(move || name),
                name.into(),
            );
            *system = SystemWithAccess::new(Box::new(wrapped));
            system.initialize(world);
            system.set_last_run(last_run);
        }
        Ok(())
    }
}
pub(super) fn install(world: &mut World) {
    if let Some(schedule) = world.resource_mut::<Schedules>().get_mut(PostUpdate) {
        schedule.add_build_pass(Bodies);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Resource, Default)]
    struct Gate(bool);
    #[derive(Component)]
    struct Spawned;
    fn create(mut commands: Commands) {
        commands.spawn(Spawned);
    }
    #[test]
    fn exact_bodies_retain_conditions_order_and_deferred_commands() {
        let mut app = App::new();
        app.init_resource::<Gate>()
            .add_systems(PostUpdate, create.run_if(|gate: Res<Gate>| gate.0));
        app.world_mut()
            .resource_mut::<Schedules>()
            .get_mut(PostUpdate)
            .unwrap()
            .add_build_pass(Bodies);
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&Spawned>()
                .iter(app.world())
                .count(),
            0
        );
        app.world_mut().resource_mut::<Gate>().0 = true;
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&Spawned>()
                .iter(app.world())
                .count(),
            1
        );
    }
}
