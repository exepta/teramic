use super::*;
use bevy::time::TimeUpdateStrategy;
use std::{
    sync::{Arc, Weak},
    time::{Duration, Instant},
};
use terra_network::client::ClientSession;
use terra_world::{edits::EditJournal, streaming::LoadedSection, tree_felling::CollapseSection};

const OWNER: SectionPos = SectionPos { x: -1, y: 0, z: -1 };

fn event() -> Collapse {
    Collapse {
        foot: [-1, 2, -1],
        direction: [1.0, 0.0],
        wood_material: 6,
        leaves_material: 7,
        sections: vec![CollapseSection {
            position: OWNER,
            wood: vec![15 + 16 * 2 + 256 * 15],
            leaves: vec![],
        }],
    }
}
fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<crate::blocks::BlockAssets>()
        .init_resource::<ClientSession>()
        .init_resource::<terra_player::CameraInputState>()
        .init_resource::<terra_player::PlayerInputBlock>()
        .init_resource::<terra_player::interaction::InteractionMotion>()
        .init_resource::<terra_player::interaction::MiningVisual>()
        .init_resource::<terra_world::streaming::ChunkStore>()
        .init_resource::<RenderedSections>()
        .init_resource::<GeometryCache>()
        .init_resource::<terra_core::GameConfig>()
        .init_resource::<EditJournal>()
        .insert_resource(super::super::BuildingAssets {
            cube: default(),
            turf_cube: default(),
            outline: default(),
            fallback: default(),
            fracture: default(),
        });
    app.world_mut().resource_mut::<ClientSession>().ready = true;
    install(&mut app);
    app.add_systems(Update, update);
    app
}

struct Fixture {
    original: Entity,
    current: Entity,
    root: Entity,
    overlay: Entity,
    piece: Entity,
    handles: Vec<Weak<bevy::asset::StrongHandle>>,
}
fn weak(handle: &Handle<Mesh>) -> Weak<bevy::asset::StrongHandle> {
    match handle {
        Handle::Strong(handle) => Arc::downgrade(handle),
        _ => panic!("temporary mesh must have a strong owner"),
    }
}
fn scene(app: &mut App) -> Fixture {
    let mut data = terra_voxel::BlockSection::empty(OWNER).unwrap();
    data.set_block([15, 2, 15], 6).unwrap();
    app.world_mut()
        .resource_mut::<terra_world::streaming::ChunkStore>()
        .loaded
        .insert(
            OWNER,
            LoadedSection {
                data,
                mesh: None,
                water: None,
                mesh_epoch: 0,
                water_epoch: 0,
                props: vec![],
            },
        );
    let original = app
        .world_mut()
        .spawn((Visibility::Hidden, Transform::default()))
        .id();
    let current = app
        .world_mut()
        .spawn((Visibility::Hidden, Transform::default()))
        .id();
    app.world_mut()
        .resource_mut::<RenderedSections>()
        .sections
        .insert(
            OWNER,
            crate::meshing::RenderedSection {
                folded_leaves: None,
                micro_detail: false,
                appearance_epoch: 0,
                entity: current,
                meshes: vec![],
                mesh_epoch: 0,
                detail_level: crate::meshing::DetailLevel::Middle,
            },
        );
    let root = app
        .world_mut()
        .spawn((Visibility::Inherited, Transform::default()))
        .id();
    let overlay_mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(1.0, 1.0, 1.0));
    let piece_mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(1.0, 1.0, 1.0));
    let handles = vec![weak(&overlay_mesh), weak(&piece_mesh)];
    let overlay = app
        .world_mut()
        .spawn((
            Mesh3d(overlay_mesh.clone()),
            Visibility::Inherited,
            Transform::default(),
            ChildOf(root),
        ))
        .id();
    let piece = app
        .world_mut()
        .spawn((
            Mesh3d(piece_mesh.clone()),
            Visibility::Inherited,
            Transform::default(),
            ChildOf(root),
        ))
        .id();
    app.world_mut()
        .resource_mut::<FallingTrees>()
        .active
        .push(Active {
            event: event(),
            root,
            masks: vec![Mask {
                position: OWNER,
                original,
                epoch: 0,
                overlay,
                meshes: vec![overlay_mesh],
            }],
            meshes: vec![piece_mesh],
            pieces: 1,
            age: VISUAL_SECONDS + 0.1,
            mode: TreeFallAnimation::Current,
        });
    Fixture {
        original,
        current,
        root,
        overlay,
        piece,
        handles,
    }
}
fn assert_released(app: &App, fixture: &Fixture) {
    for root in [fixture.original, fixture.current] {
        assert_eq!(
            app.world().get::<Visibility>(root),
            Some(&Visibility::Inherited)
        );
    }
    for removed in [fixture.root, fixture.overlay, fixture.piece] {
        assert!(app.world().get_entity(removed).is_err());
    }
    assert!(
        fixture
            .handles
            .iter()
            .all(|handle| handle.strong_count() == 0),
        "no ECS component or active/mask record retains temporary mesh assets"
    );
    let state = app.world().resource::<FallingTrees>();
    assert!(state.active.is_empty());
    assert!(state.pending.is_none());
    assert!(state.waiting.is_empty());
    assert_eq!(app.world().resource::<TreeFallStatus>().meshes, 0);
}

#[test]
fn unloading_a_masked_section_restores_original_and_replaced_roots_and_releases_mesh_owners() {
    let mut app = app();
    let fixture = scene(&mut app);
    assert!(
        fixture
            .handles
            .iter()
            .all(|handle| handle.strong_count() >= 2)
    );
    app.world_mut()
        .resource_mut::<terra_world::streaming::ChunkStore>()
        .loaded
        .remove(&OWNER);
    app.update();
    assert_released(&app, &fixture);
}

#[test]
fn leaving_the_world_cancels_pending_work_and_clears_all_masks_waiting_events_and_mesh_owners() {
    let mut app = app();
    let fixture = scene(&mut app);
    app.world_mut()
        .resource_mut::<FallingTrees>()
        .waiting
        .push_back(Waiting {
            event: event(),
            age: 0.0,
        });
    app.world_mut().resource_mut::<FallingTrees>().pending = Some(Pending {
        event: event(),
        originals: vec![],
        refresh: Some(fixture.root),
        mode: TreeFallAnimation::Current,
        task: AsyncComputeTaskPool::get().spawn(std::future::pending()),
    });
    app.world_mut().resource_mut::<ClientSession>().ready = false;
    app.update();
    assert_released(&app, &fixture);
}

#[test]
fn a_refresh_with_a_missing_cached_snapshot_never_leaks_a_mask_or_leaves_terrain_hidden() {
    let mut app = app();
    let fixture = scene(&mut app);
    app.world_mut()
        .resource_mut::<RenderedSections>()
        .sections
        .get_mut(&OWNER)
        .unwrap()
        .mesh_epoch = 1;
    app.world_mut()
        .resource_mut::<crate::blocks::BlockAssets>()
        .templates = Some(Arc::new(crate::blocks::TemplateSet::default()));
    app.update();
    assert!(app.world().resource::<FallingTrees>().pending.is_some());
    let deadline = Instant::now() + Duration::from_secs(2);
    while !app.world().resource::<FallingTrees>().active.is_empty() {
        app.update();
        assert!(
            Instant::now() < deadline,
            "missing-snapshot refresh did not settle within its bounded task lifecycle"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_released(&app, &fixture);
}

#[test]
fn a_completed_refresh_cannot_resurrect_a_tree_root_already_removed_by_streaming() {
    let mut app = app();
    let root = app.world_mut().spawn(Transform::default()).id();
    app.world_mut().despawn(root);
    app.world_mut().resource_mut::<FallingTrees>().pending = Some(Pending {
        event: event(),
        originals: vec![],
        refresh: Some(root),
        mode: TreeFallAnimation::Current,
        task: AsyncComputeTaskPool::get().spawn(async {
            Ok(geometry::Geometry {
                terrain: vec![],
                pieces: vec![],
                band_height: 1.0,
                hell: vec![],
            })
        }),
    });
    let count = app.world().entities().len();
    let deadline = Instant::now() + Duration::from_secs(2);
    while app.world().resource::<FallingTrees>().pending.is_some() {
        app.update();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(app.world().entities().len(), count);
    assert!(app.world().resource::<FallingTrees>().active.is_empty());
    assert!(app.world().get_entity(root).is_err());
}
