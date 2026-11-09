//! Give newly initialized, conditional systems a real startup tick.
//!
//! Bevy 0.20 initializes last_run to present - MAX_CHANGE_AGE. Its first
//! periodic tick scan therefore reports never-run systems as billions of ticks
//! old, even in a short menu session. Keep existing systems and log filters
//! untouched, and retain first-run change detection for startup resources.
use bevy::ecs::schedule::graph::DiGraph;
use bevy::{
    ecs::{
        change_detection::Tick,
        schedule::{
            FlattenedDependencies, NodeId, ScheduleBuildError, ScheduleBuildPass, ScheduleGraph,
            Schedules, SystemKey, SystemSetKey,
        },
        system::System,
    },
    platform::hash::FixedHasher,
    prelude::*,
};
use indexmap::IndexSet;

#[derive(Debug, Default)]
struct InitialTicks;

fn normalize(system: &mut impl System, present: Tick) {
    // A fresh world starts at tick 1. Tick 0 still predates every insertion,
    // while leaving ample room before the overflow maintenance threshold.
    if present.get() < Tick::MAX.get() / 2
        && system.get_last_run().get() == present.get().wrapping_sub(Tick::MAX.get())
    {
        system.set_last_run(Tick::new(0));
    }
}

impl ScheduleBuildPass for InitialTicks {
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
        let present = world.change_tick();
        let keys: Vec<_> = graph.systems.iter().map(|(key, _, _)| key).collect();
        for key in keys {
            if let Some(system) = graph.systems.get_mut(key) {
                normalize(system, present);
            }
            if let Some(conditions) = graph.systems.get_conditions_mut(key) {
                for condition in conditions {
                    normalize(condition, present);
                }
            }
        }
        let sets: Vec<_> = graph.system_sets.iter().map(|(key, _, _)| key).collect();
        for key in sets {
            if let Some(conditions) = graph.system_sets.get_conditions_mut(key) {
                for condition in conditions {
                    normalize(condition, present);
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn install(world: &mut World) {
    for (_, schedule) in world.resource_mut::<Schedules>().iter_mut() {
        schedule.add_build_pass(InitialTicks);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Resource, Default)]
    struct Gate(bool);
    #[derive(Resource, Default)]
    struct Observed {
        first_added: bool,
        changed: bool,
        calls: usize,
    }
    #[derive(Resource)]
    struct Value(u32);
    fn inspect(value: Res<Value>, mut observed: ResMut<Observed>) {
        observed.first_added = value.is_added();
        observed.changed = value.is_changed();
        observed.calls += value.0 as usize;
    }
    #[test]
    fn delayed_first_run_keeps_added_and_changed_detection_without_sentinel_age() {
        let mut world = World::new();
        world.insert_resource(Value(1));
        world.init_resource::<Gate>();
        world.init_resource::<Observed>();
        let mut schedule = Schedule::default();
        schedule.add_build_pass(InitialTicks);
        schedule.add_systems(inspect.run_if(|gate: Res<Gate>| gate.0));
        schedule.run(&mut world);
        for (_, system) in schedule.systems().unwrap() {
            assert!(
                world
                    .change_tick()
                    .get()
                    .wrapping_sub(system.get_last_run().get())
                    < Tick::MAX.get() / 2
            );
        }
        // Trigger the same periodic scan as a long menu wait without a live wait.
        for _ in 0..bevy::ecs::change_detection::CHECK_TICK_THRESHOLD + 1 {
            world.increment_change_tick();
        }
        world.check_change_ticks();
        world.resource_mut::<Gate>().0 = true;
        world.resource_mut::<Value>().0 = 2;
        schedule.run(&mut world);
        let observed = world.resource::<Observed>();
        assert!(observed.first_added && observed.changed);
        assert_eq!(observed.calls, 2);
        schedule.run(&mut world);
        assert!(!world.resource::<Observed>().changed);
    }
}
