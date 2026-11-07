use super::super::{ChunkAnimation, GameConfig, QUEUE_SHADOW_LAYER, QueuedShadow};
use super::*;
use bevy::camera::visibility::RenderLayers;

fn fixture() -> World {
    let mut world = World::new();
    world.init_resource::<Retirements>();
    world.init_resource::<Assets<Mesh>>();
    world.init_resource::<Assets<StandardMaterial>>();
    world.init_resource::<ChunkAnimationStatus>();
    world.init_resource::<TransitionDraws>();
    world.init_resource::<Time>();
    let mut config = GameConfig::default();
    config.client.chunk_animation = ChunkAnimation::Fade;
    world.insert_resource(config);
    world
}

fn root(world: &mut World) -> Entity {
    world
        .spawn((Transform::default(), Visibility::Inherited))
        .id()
}

fn model(world: &mut World, root: Entity) -> (Entity, Handle<Mesh>, Handle<StandardMaterial>) {
    let mesh = world.resource_mut::<Assets<Mesh>>().add(Cuboid::default());
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    let child = world
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Visibility::Inherited,
            ChildOf(root),
        ))
        .id();
    (child, mesh, material)
}

fn retire(world: &mut World, root: Entity, meshes: Vec<Handle<Mesh>>, mode: ChunkAnimation) {
    super::super::retire(&mut world.commands(), root, meshes, mode);
    world.flush();
}

#[test]
fn real_retire_hides_nested_meshes_and_explicitly_visible_queued_shadow_casters_before_drain() {
    let mut world = fixture();
    let root = root(&mut world);
    let intermediate = world.spawn((Transform::default(), ChildOf(root))).id();
    let (child, mesh, _) = model(&mut world, intermediate);
    super::super::enter(&mut world.commands(), root, ChunkAnimation::Fade);
    world.flush();
    assert!(world.get::<QueuedChunk>(root).is_some());
    assert!(world.get::<QueuedShadow>(child).is_some());
    assert_eq!(world.get::<Visibility>(child), Some(&Visibility::Visible));
    assert!(
        world
            .get::<RenderLayers>(child)
            .unwrap()
            .intersects(&RenderLayers::layer(QUEUE_SHADOW_LAYER))
    );
    retire(&mut world, root, vec![mesh.clone()], ChunkAnimation::Fade);
    assert!(contains(&world, root));
    assert!(world.get::<QueuedChunk>(root).is_none());
    for entity in [root, child] {
        assert_eq!(world.get::<Visibility>(entity), Some(&Visibility::Hidden));
    }
    assert!(world.resource::<Assets<Mesh>>().contains(mesh.id()));
    assert_eq!(world.resource::<Retirements>().pending.len(), 1);
    drain_with(&mut world, 1, usize::MAX, Duration::from_secs(1));
    for entity in [root, intermediate, child] {
        assert!(world.get_entity(entity).is_err());
    }
    assert!(!world.resource::<Assets<Mesh>>().contains(mesh.id()));
    assert_eq!(world.resource::<ChunkAnimationStatus>().retiring_roots, 0);
}

#[test]
fn duplicate_root_and_mesh_aliases_merge_without_releasing_either_mesh_early() {
    let mut world = fixture();
    let root = root(&mut world);
    let (_, a, _) = model(&mut world, root);
    let (_, b, _) = model(&mut world, root);
    retire(
        &mut world,
        root,
        vec![a.clone(), a.clone()],
        ChunkAnimation::None,
    );
    retire(
        &mut world,
        root,
        vec![a.clone(), b.clone(), b.clone()],
        ChunkAnimation::None,
    );
    let pending = world.resource::<Retirements>();
    assert_eq!(pending.order.len(), 1);
    assert_eq!(pending.pending.len(), 1);
    assert_eq!(pending.meshes, 2);
    for handle in [&a, &b] {
        assert!(world.resource::<Assets<Mesh>>().contains(handle.id()));
    }
    drain_with(&mut world, 1, usize::MAX, Duration::from_secs(1));
    assert!(world.get_entity(root).is_err());
    assert_eq!(world.resource::<Assets<Mesh>>().len(), 0);
    let pending = world.resource::<Retirements>();
    assert_eq!(
        (pending.pending.len(), pending.order.len(), pending.meshes),
        (0, 0, 0)
    );
}

#[test]
fn immediate_retire_keeps_active_fade_temporaries_and_retired_meshes_until_drain() {
    let mut world = fixture();
    let root = root(&mut world);
    let (child, mesh, original) = model(&mut world, root);
    super::super::start_test_fade(&mut world, root);
    let temporary = world
        .get::<MeshMaterial3d<StandardMaterial>>(child)
        .unwrap()
        .0
        .clone();
    assert_ne!(temporary.id(), original.id());
    assert!(world.resource::<TransitionDraws>().pending.contains(&child));
    let previous = world.resource_mut::<Assets<Mesh>>().add(Cuboid::default());
    world
        .get_mut::<ChunkFade>(root)
        .unwrap()
        .retired_meshes
        .push(previous.clone());
    retire(&mut world, root, vec![mesh.clone()], ChunkAnimation::None);
    assert!(world.get::<ChunkFade>(root).is_none());
    assert!(!world.resource::<TransitionDraws>().pending.contains(&child));
    assert!(
        world
            .resource::<Assets<StandardMaterial>>()
            .contains(temporary.id())
    );
    assert!(
        world
            .resource::<Assets<StandardMaterial>>()
            .contains(original.id())
    );
    for mesh in [&mesh, &previous] {
        assert!(world.resource::<Assets<Mesh>>().contains(mesh.id()));
    }
    assert_eq!(world.resource::<Retirements>().materials, 1);
    assert_eq!(world.resource::<ChunkAnimationStatus>().completed, 1);
    drain_with(&mut world, 1, usize::MAX, Duration::from_secs(1));
    assert!(world.get_entity(root).is_err());
    assert!(
        !world
            .resource::<Assets<StandardMaterial>>()
            .contains(temporary.id())
    );
    assert!(
        world
            .resource::<Assets<StandardMaterial>>()
            .contains(original.id())
    );
    assert_eq!(world.resource::<Assets<Mesh>>().len(), 0);
    assert_eq!(world.resource::<ChunkAnimationStatus>().completed, 1);
}

#[test]
fn normal_outgoing_fades_keep_the_existing_visible_animation_path() {
    let mut world = fixture();
    let root = root(&mut world);
    let (child, mesh, _) = model(&mut world, root);
    super::super::start_test_fade(&mut world, root);
    retire(&mut world, root, vec![mesh.clone()], ChunkAnimation::Fade);
    assert!(!contains(&world, root));
    let fade = world.get::<ChunkFade>(root).unwrap();
    assert!(fade.outgoing);
    assert_eq!(fade.retired_meshes, vec![mesh.clone()]);
    assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Inherited));
    assert_eq!(world.get::<Visibility>(child), Some(&Visibility::Inherited));
    drain_with(&mut world, usize::MAX, usize::MAX, Duration::from_secs(1));
    assert!(world.get_entity(root).is_ok());
    assert!(world.resource::<Assets<Mesh>>().contains(mesh.id()));
}

#[test]
fn ordinary_inherited_children_are_render_hidden_by_real_visibility_propagation() {
    use bevy::{camera::visibility::VisibilityPlugin, mesh::skinning::SkinnedMeshInverseBindposes};
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, VisibilityPlugin))
        .init_resource::<Retirements>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<SkinnedMeshInverseBindposes>>()
        .init_resource::<Assets<StandardMaterial>>();
    let root = root(app.world_mut());
    let intermediate = app
        .world_mut()
        .spawn((Transform::default(), Visibility::Inherited, ChildOf(root)))
        .id();
    let (child, mesh, _) = model(app.world_mut(), intermediate);
    app.update();
    assert!(app.world().get::<InheritedVisibility>(child).unwrap().get());
    retire(
        app.world_mut(),
        root,
        vec![mesh.clone()],
        ChunkAnimation::None,
    );
    assert_eq!(
        app.world().get::<Visibility>(child),
        Some(&Visibility::Inherited)
    );
    app.update();
    assert!(!app.world().get::<InheritedVisibility>(child).unwrap().get());
    assert!(
        app.world().get_entity(root).is_ok(),
        "visibility updates precede the deferred drain"
    );
    assert!(app.world().resource::<Assets<Mesh>>().contains(mesh.id()));
    drain_with(app.world_mut(), 1, usize::MAX, Duration::from_secs(1));
    assert!(app.world().get_entity(child).is_err());
}

#[test]
fn external_despawn_and_same_section_reload_never_destroy_new_entity_generation_or_assets() {
    let mut world = fixture();
    let old = root(&mut world);
    let (_, old_mesh, _) = model(&mut world, old);
    retire(
        &mut world,
        old,
        vec![old_mesh.clone()],
        ChunkAnimation::None,
    );
    world.despawn(old);
    let replacement = root(&mut world);
    let (_, new_mesh, _) = model(&mut world, replacement);
    let section = terra_voxel::SectionPos::new(-1, 0, -1);
    world
        .entity_mut(replacement)
        .insert(crate::meshing::TerrainSection {
            position: section,
            mesh_epoch: 0,
            detail_level: crate::meshing::DetailLevel::Near,
        });
    assert_ne!(old, replacement);
    drain_with(&mut world, usize::MAX, usize::MAX, Duration::from_secs(1));
    assert!(world.get_entity(replacement).is_ok());
    assert!(world.resource::<Assets<Mesh>>().contains(new_mesh.id()));
    assert!(!world.resource::<Assets<Mesh>>().contains(old_mesh.id()));
    assert_eq!(
        world.get::<Visibility>(replacement),
        Some(&Visibility::Inherited)
    );
}

#[test]
fn count_weight_and_zero_time_limits_keep_bounded_progress() {
    let mut world = fixture();
    let roots: Vec<_> = (0..5)
        .map(|_| {
            let root = root(&mut world);
            retire(&mut world, root, Vec::new(), ChunkAnimation::None);
            root
        })
        .collect();
    drain_with(&mut world, 2, usize::MAX, Duration::from_secs(1));
    assert_eq!(world.resource::<Retirements>().pending.len(), 3);
    assert!(
        roots[..2]
            .iter()
            .all(|&root| world.get_entity(root).is_err())
    );
    drain_with(&mut world, 0, 0, Duration::ZERO);
    assert_eq!(world.resource::<Retirements>().pending.len(), 2);

    let mut world = fixture();
    for _ in 0..2 {
        let root = root(&mut world);
        let (_, mesh, _) = model(&mut world, root);
        retire(&mut world, root, vec![mesh], ChunkAnimation::None);
    }
    assert_eq!(
        world
            .resource::<Retirements>()
            .pending
            .values()
            .next()
            .unwrap()
            .work(),
        3
    );
    drain_with(&mut world, usize::MAX, 4, Duration::from_secs(1));
    assert_eq!(
        world.resource::<Retirements>().pending.len(),
        1,
        "never admit a second root beyond remaining work budget"
    );
    drain_with(&mut world, 0, 0, Duration::ZERO);
    assert!(world.resource::<Retirements>().pending.is_empty());
}

#[test]
fn global_root_cap_releases_oldest_synchronously_and_keeps_every_pending_root_hidden() {
    let mut world = fixture();
    let mut oldest = None;
    for _ in 0..=MAX_PENDING_ROOTS {
        let root = root(&mut world);
        oldest.get_or_insert(root);
        retire(&mut world, root, Vec::new(), ChunkAnimation::None);
    }
    let pending = world.resource::<Retirements>();
    assert_eq!(pending.pending.len(), MAX_PENDING_ROOTS);
    assert_eq!(pending.order.len(), MAX_PENDING_ROOTS);
    assert_eq!(pending.overflows, 1);
    assert!(!pending.over_limit());
    for &root in &pending.order {
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Hidden));
    }
    assert!(world.get_entity(oldest.unwrap()).is_err());
    drain_with(
        &mut world,
        ROOTS_PER_FRAME,
        usize::MAX,
        Duration::from_secs(1),
    );
    assert_eq!(
        world.resource::<Retirements>().pending.len(),
        MAX_PENDING_ROOTS - ROOTS_PER_FRAME
    );
    drain_with(&mut world, usize::MAX, usize::MAX, Duration::from_secs(1));
    assert!(world.resource::<Retirements>().pending.is_empty());
}

#[test]
fn standalone_worlds_without_a_drain_stay_immediate_and_legacy_control_is_smoke_only() {
    for smoke in [false, true] {
        for value in [None, Some("0"), Some("1"), Some("unexpected")] {
            assert_eq!(legacy_control(smoke, value), smoke && value == Some("1"));
        }
    }
    let mut world = fixture();
    world.remove_resource::<Retirements>();
    assert!(!enabled(&world));
    let root = root(&mut world);
    let (_, mesh, _) = model(&mut world, root);
    retire(&mut world, root, vec![mesh.clone()], ChunkAnimation::None);
    assert!(world.get_entity(root).is_err());
    assert!(!world.resource::<Assets<Mesh>>().contains(mesh.id()));
    assert!(!world.contains_resource::<Retirements>());
}
