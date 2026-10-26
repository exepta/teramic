use super::*;
use terra_voxel::{BlockSection, SectionPos};
use terra_world::{
    edits::SectionDelta,
    props::{PropId, PropKind, WorldProp},
    streaming::LoadedSection,
};

#[test]
fn client_edits_are_explicitly_ordered_after_the_held_slot_publication() {
    use bevy::ecs::schedule::{IntoSystemSet, NodeId, Schedules, SystemSet};

    let mut app = App::new();
    install(&mut app);
    let schedules = app.world().resource::<Schedules>();
    let graph = schedules.get(Update).unwrap().graph();
    let held = graph
        .system_sets
        .get_key(send_mining.into_system_set().intern())
        .unwrap();
    let edits = graph
        .system_sets
        .get_key(send_edits.into_system_set().intern())
        .unwrap();
    let edit_systems: Vec<_> = graph
        .hierarchy()
        .graph()
        .edges(NodeId::Set(edits))
        .map(|(_, child)| child)
        .collect();
    assert_eq!(edit_systems.len(), 1);
    assert!(
        graph
            .dependency()
            .graph()
            .contains_edge(NodeId::Set(held), edit_systems[0]),
        "the newly selected slot must reach the server before its pot/farm edit"
    );
}

#[derive(Resource, Default)]
struct Incoming(Vec<SectionDelta>);

#[derive(Resource, Default)]
struct Observed {
    changes: Vec<(bool, bool, Vec<SectionPos>)>,
    errors: Vec<String>,
}

fn import_queued(
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    mut applied: MessageWriter<SectionEditApplied>,
    mut incoming: ResMut<Incoming>,
    mut observed: ResMut<Observed>,
) {
    for delta in std::mem::take(&mut incoming.0) {
        if let Err(error) = apply_section_delta(&mut chunks, &mut journal, &mut applied, delta) {
            observed.errors.push(error);
        }
    }
}

fn observe(
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    mut applied: MessageReader<SectionEditApplied>,
    mut observed: ResMut<Observed>,
) {
    observed.changes.push((
        chunks.is_changed(),
        journal.is_changed(),
        applied.read().map(|m| m.position).collect(),
    ));
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<Incoming>()
        .init_resource::<Observed>()
        .add_message::<SectionEditApplied>()
        .add_systems(Update, (import_queued, observe).chain());
    app
}

#[test]
fn confirmed_prop_only_delta_is_local_and_identical_or_stale_imports_are_not_world_changes() {
    let owner = SectionPos::new(-1, 0, -2);
    let prop = WorldProp {
        id: PropId(7),
        kind: PropKind::Flower,
        position: [-8.0, 1.0, -24.0],
        rotation: 0.0,
        scale: 0.8,
        variant: 0,
        section: owner,
        removed: false,
    };
    let mut app = app();
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        owner,
        LoadedSection {
            data: BlockSection::empty(owner).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![prop],
        },
    );
    let mut authority = EditJournal::default();
    authority.remove_prop(owner, prop.id).unwrap();
    let delta = authority.section_delta(owner);
    app.update();
    app.world_mut()
        .resource_mut::<Incoming>()
        .0
        .push(delta.clone());
    app.update();
    assert_eq!(
        app.world().resource::<Observed>().changes.last(),
        Some(&(true, true, vec![owner]))
    );
    assert!(app.world().resource::<ChunkStore>().loaded[&owner].props[0].removed);
    assert_eq!(app.world().resource::<EditJournal>().mesh_epoch(owner), 0);
    for incoming in [
        delta,
        SectionDelta {
            position: owner,
            ..default()
        },
    ] {
        app.world_mut().resource_mut::<Incoming>().0.push(incoming);
        app.update();
        assert_eq!(
            app.world().resource::<Observed>().changes.last(),
            Some(&(false, false, vec![]))
        );
    }
    assert!(app.world().resource::<Observed>().errors.is_empty());
    assert!(
        app.world()
            .resource::<EditJournal>()
            .prop_removed(owner, prop.id)
    );
}

#[test]
fn same_epoch_block_payload_changes_still_invalidate_and_duplicate_validation_is_not_bypassed() {
    let owner = SectionPos::new(-1, 0, -2);
    let mut app = app();
    app.update();
    let original = SectionDelta {
        position: owner,
        revision: 1,
        blocks: vec![([0, 0, 0], 2)],
        ..default()
    };
    app.world_mut()
        .resource_mut::<Incoming>()
        .0
        .push(original.clone());
    app.update();
    let previous = app.world().resource::<EditJournal>().mesh_epoch(owner);
    let changed = SectionDelta {
        blocks: vec![([0, 0, 0], 3)],
        ..original
    };
    app.world_mut()
        .resource_mut::<Incoming>()
        .0
        .push(changed.clone());
    app.update();
    assert!(app.world().resource::<EditJournal>().mesh_epoch(owner) > previous);
    assert_eq!(
        app.world().resource::<Observed>().changes.last(),
        Some(&(true, true, vec![owner]))
    );
    app.world_mut()
        .resource_mut::<Incoming>()
        .0
        .push(changed.clone());
    app.update();
    assert_eq!(
        app.world().resource::<Observed>().changes.last(),
        Some(&(false, false, vec![]))
    );
    app.world_mut()
        .resource_mut::<Incoming>()
        .0
        .push(SectionDelta {
            blocks: vec![([16, 0, 0], 3)],
            ..changed
        });
    app.update();
    assert_eq!(app.world().resource::<Observed>().errors.len(), 1);
    assert_eq!(
        app.world().resource::<Observed>().changes.last(),
        Some(&(false, false, vec![]))
    );
}
