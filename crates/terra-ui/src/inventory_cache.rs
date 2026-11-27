use super::*;
use tilt_ui::ParkedUiBindings;
use tilt_ui::TiltUiComponentRuntimeSet;

#[derive(Resource, Default)]
struct InventoryDomCache {
    root: Option<Entity>,
    parked: Option<ParkedUiBindings>,
    shell_layout: Option<Node>,
    viewport: Option<UVec2>,
}

impl InventoryDomCache {
    fn restore_layout(&mut self, world: &mut World) {
        if let Some(layout) = self.shell_layout.take()
            && let Some(root) = self.root
            && let Ok(mut shell) = world.get_entity_mut(root)
        {
            shell.insert_if_new(layout);
        }
    }
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<InventoryDomCache>().add_systems(
        Update,
        restore_before_bindings
            .after(view::update_view)
            .before(TiltUiComponentRuntimeSet::Reactivity),
    );
}

fn restore_before_bindings(world: &mut World) {
    let _scope = terra_core::profiling::ScopeTimer::new("ui.inventory.restore_bindings");
    world.resource_scope(|world, mut cache: Mut<InventoryDomCache>| {
        let visible = world.resource::<InventoryView>().visible;
        if cache
            .root
            .is_some_and(|root| world.get_entity(root).is_err())
        {
            *cache = InventoryDomCache::default();
        }
        if !visible {
            cache.restore_layout(world);
            if let Some(bindings) = cache.parked.take() {
                bindings.restore(world);
            }
            *cache = InventoryDomCache::default();
            return;
        }
        let viewport = world
            .query_filtered::<&Window, With<bevy::window::PrimaryWindow>>()
            .iter(world)
            .next()
            .map(Window::physical_size);
        let resized = cache.viewport.is_some() && cache.viewport != viewport;
        cache.viewport = viewport;
        if resized && !world.resource::<InventoryView>().open {
            world.resource_mut::<InventoryView>().prewarming = true;
        }
        let view = world.resource::<InventoryView>();
        let state = world.get_resource::<InventoryState>();
        let pending_open = state.is_some_and(|state| state.container_open_response.is_some());
        // Keep the old subtree until the authoritative block snapshot is ready.
        // The view can temporarily describe the player while that request is pending,
        // or still describe an open block inventory just after Close was processed.
        let open = view.open && state.is_none_or(|state| state.open);
        let prewarming = view.prewarming && !view.open;
        let hidden = !view.open && !prewarming;
        let source_container = view.source_container;
        if !hidden {
            cache.restore_layout(world);
        }
        if (open || prewarming) && !pending_open {
            // An ACK can clear the pending flag before the view's cadence runs.
            // Restore only after that projection has caught up to the actual surface.
            let projection_ready = world.get_resource::<ClientSession>().is_none_or(|session| {
                source_container
                    == session
                        .container
                        .as_ref()
                        .map(|container| (container.anchor, container.view))
            });
            if cache.parked.is_some() && !projection_ready {
                return;
            }
            if let Some(parked) = cache.parked.take() {
                parked.restore(world);
            }
            return;
        }
        if cache.parked.is_none() {
            let root = world
                .query::<(Entity, &ElementId)>()
                .iter(world)
                .find_map(|(entity, id)| (id.0 == "inventory-shell").then_some(entity));
            if let Some(root) = root {
                cache.root = Some(root);
                cache.parked = Some(ParkedUiBindings::take(world, root));
            }
        }
        // Display::None still makes Bevy walk the entire retained tree. Wait
        // for a hidden layout pass to clear the descendants' geometry before
        // taking the shell out of layout. Keep the ECS hierarchy for teardown.
        if hidden
            && cache.shell_layout.is_none()
            && let Some(root) = cache.root
            && world
                .get::<ComputedNode>(root)
                .is_some_and(ComputedNode::is_empty)
        {
            cache.shell_layout = world.entity_mut(root).take::<Node>();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tilt_ui::ParkedUi;

    fn cache_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(tilt_ui::TiltUiCodePlugin)
            .insert_resource(InventoryState {
                open: true,
                ..default()
            })
            .insert_resource(InventoryView {
                visible: true,
                open: true,
                player: false,
                ..default()
            });
        install(&mut app);
        let shell = app
            .world_mut()
            .spawn((
                ElementId("inventory-shell".into()),
                Node::default(),
                tilt_ui::PropertyBindings {
                    bindings: vec![tilt_ui::PropertyBinding {
                        name: "class".into(),
                        expression: "inventoryView.player ? 'player' : 'block'".into(),
                    }],
                },
            ))
            .id();
        app.update();
        assert_eq!(
            app.world()
                .get::<tilt_ui::ElementClasses>(shell)
                .unwrap()
                .classes,
            ["block"]
        );
        (app, shell)
    }

    #[test]
    fn closed_shell_leaves_layout_after_geometry_clears_and_resumes_before_prewarm() {
        use bevy::ui::{ui_layout_system, ui_surface::UiSurface};

        let (mut app, shell) = cache_app();
        app.init_resource::<UiSurface>()
            .init_resource::<bevy::text::FontCx>()
            .init_resource::<bevy::text::RemSize>()
            .add_systems(PostUpdate, ui_layout_system);
        let root = app
            .world_mut()
            .spawn(Node {
                position_type: PositionType::Absolute,
                width: Val::Px(800.0),
                height: Val::Px(600.0),
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .id();
        let backdrop = app
            .world_mut()
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(300.0),
                    ..default()
                },
                ChildOf(root),
            ))
            .id();
        app.world_mut().entity_mut(shell).insert((
            Node {
                width: Val::Px(428.0),
                height: Val::Px(160.0),
                ..default()
            },
            ChildOf(backdrop),
        ));
        let child = app
            .world_mut()
            .spawn((
                Node {
                    width: Val::Percent(50.0),
                    height: Val::Px(32.0),
                    ..default()
                },
                ChildOf(shell),
            ))
            .id();
        let hotbar = app
            .world_mut()
            .spawn((
                Node {
                    width: Val::Px(90.0),
                    height: Val::Px(20.0),
                    ..default()
                },
                ChildOf(root),
            ))
            .id();
        let size = |app: &App, entity| app.world().get::<ComputedNode>(entity).unwrap().size();
        app.update();
        assert_eq!(size(&app, shell), Vec2::new(428.0, 160.0));
        assert_eq!(size(&app, child), Vec2::new(214.0, 32.0));
        app.world_mut().resource_mut::<InventoryState>().open = false;
        app.world_mut().resource_mut::<InventoryView>().open = false;
        app.world_mut().get_mut::<Node>(backdrop).unwrap().display = Display::None;
        app.update();
        assert!(app.world().get::<Node>(shell).is_some());
        assert_eq!(size(&app, child), Vec2::ZERO);
        app.update();
        assert!(app.world().get::<Node>(shell).is_none());
        assert!(
            app.world_mut()
                .resource_mut::<UiSurface>()
                .get_layout(shell, true)
                .is_err()
        );
        assert_eq!(app.world().get::<ChildOf>(child).unwrap().parent(), shell);
        assert_eq!(size(&app, hotbar), Vec2::new(90.0, 20.0));
        app.world_mut().resource_mut::<InventoryView>().prewarming = true;
        app.world_mut().get_mut::<Node>(backdrop).unwrap().display = Display::Flex;
        app.update();
        assert!(app.world().get::<ParkedUi>(shell).is_none());
        assert_eq!(size(&app, shell), Vec2::new(428.0, 160.0));
        assert_eq!(size(&app, child), Vec2::new(214.0, 32.0));
    }

    #[test]
    fn pending_container_snapshot_and_async_close_keep_the_retained_block_projection() {
        let (mut app, shell) = cache_app();
        app.world_mut().resource_mut::<InventoryState>().open = false;
        app.world_mut().resource_mut::<InventoryView>().open = false;
        app.update();
        {
            let mut state = app.world_mut().resource_mut::<InventoryState>();
            state.open = true;
            state.container_open_response = Some(9);
        }
        {
            let mut view = app.world_mut().resource_mut::<InventoryView>();
            view.open = true;
            view.player = true;
        }
        for _ in 0..3 {
            app.update();
            assert!(app.world().get::<ParkedUi>(shell).is_some());
            assert!(
                app.world()
                    .get::<tilt_ui::PropertyBindings>(shell)
                    .is_none()
            );
            assert_eq!(
                app.world()
                    .get::<tilt_ui::ElementClasses>(shell)
                    .unwrap()
                    .classes,
                ["block"]
            );
        }
        app.world_mut()
            .resource_mut::<InventoryState>()
            .container_open_response = None;
        app.world_mut().resource_mut::<InventoryView>().player = false;
        app.update();
        assert!(app.world().get::<ParkedUi>(shell).is_none());
        assert!(
            app.world()
                .get::<tilt_ui::PropertyBindings>(shell)
                .is_some()
        );
        // Close is authoritative even before the view's slower cadence catches up.
        app.world_mut().resource_mut::<InventoryState>().open = false;
        app.world_mut().resource_mut::<InventoryView>().player = true;
        app.update();
        assert!(app.world().get::<ParkedUi>(shell).is_some());
        assert_eq!(
            app.world()
                .get::<tilt_ui::ElementClasses>(shell)
                .unwrap()
                .classes,
            ["block"]
        );
        app.world_mut().resource_mut::<InventoryView>().open = false;
        app.update();
        app.world_mut().resource_mut::<InventoryState>().open = true;
        app.world_mut().resource_mut::<InventoryView>().open = true;
        app.update();
        assert!(app.world().get::<ParkedUi>(shell).is_none());
        assert_eq!(
            app.world()
                .get::<tilt_ui::ElementClasses>(shell)
                .unwrap()
                .classes,
            ["player"]
        );
    }

    #[test]
    fn pending_request_parks_live_bindings_and_failure_waits_for_a_real_reopen_or_prewarm() {
        let (mut app, shell) = cache_app();
        app.world_mut()
            .resource_mut::<InventoryState>()
            .container_open_response = Some(9);
        app.world_mut().resource_mut::<InventoryView>().player = true;
        app.world_mut().resource_mut::<InventoryView>().prewarming = true;
        app.update();
        assert!(app.world().get::<ParkedUi>(shell).is_some());
        assert_eq!(
            app.world()
                .get::<tilt_ui::ElementClasses>(shell)
                .unwrap()
                .classes,
            ["block"]
        );
        {
            let mut state = app.world_mut().resource_mut::<InventoryState>();
            state.open = false;
            state.container_open_response = None;
        }
        app.update();
        assert!(app.world().get::<ParkedUi>(shell).is_some());
        assert!(
            app.world()
                .get::<tilt_ui::PropertyBindings>(shell)
                .is_none()
        );
        {
            let mut view = app.world_mut().resource_mut::<InventoryView>();
            view.open = false;
            view.prewarming = true;
        }
        app.update();
        assert!(app.world().get::<ParkedUi>(shell).is_none());
        assert_eq!(
            app.world()
                .get::<tilt_ui::ElementClasses>(shell)
                .unwrap()
                .classes,
            ["player"]
        );
    }

    #[test]
    fn container_ack_waits_for_the_next_matching_view_projection_before_restoring_bindings() {
        use terra_network::protocol::inventory::{ContainerHalf, ContainerSnapshot};
        let (mut app, shell) = cache_app();
        app.world_mut().resource_mut::<InventoryState>().open = false;
        app.world_mut().resource_mut::<InventoryView>().open = false;
        app.update();
        let anchor = [12, 60, -8];
        app.init_resource::<ClientSession>();
        app.world_mut().resource_mut::<ClientSession>().container = Some(ContainerSnapshot {
            anchor,
            view: ContainerView::Primary,
            attached_campfire: false,
            workbench: None,
            furnace: None,
            crusher: None,
            mixer: None,
            filter: None,
            distributor: None,
            tunnel_random: None,
            cauldron: None,
            campfire: None,
            halves: vec![ContainerHalf {
                position: anchor,
                inventory: default(),
            }],
        });
        {
            let mut state = app.world_mut().resource_mut::<InventoryState>();
            state.open = true;
            state.container_open_response = Some(9);
        }
        {
            let mut view = app.world_mut().resource_mut::<InventoryView>();
            view.open = true;
            view.player = true;
            view.source_container = None;
        }
        app.update();
        app.world_mut()
            .resource_mut::<InventoryState>()
            .container_open_response = None;
        // The ACK has arrived, but the UI cadence still has its previous player view.
        for _ in 0..3 {
            app.update();
            assert!(app.world().get::<ParkedUi>(shell).is_some());
            assert!(
                app.world()
                    .get::<tilt_ui::PropertyBindings>(shell)
                    .is_none()
            );
            assert_eq!(
                app.world()
                    .get::<tilt_ui::ElementClasses>(shell)
                    .unwrap()
                    .classes,
                ["block"]
            );
        }
        {
            let mut view = app.world_mut().resource_mut::<InventoryView>();
            view.player = false;
            view.source_container = Some((anchor, ContainerView::Primary));
        }
        app.update();
        assert!(app.world().get::<ParkedUi>(shell).is_none());
        assert!(
            app.world()
                .get::<tilt_ui::PropertyBindings>(shell)
                .is_some()
        );
        // Updating an active surface must not automatically park its controls.
        app.world_mut()
            .resource_mut::<ClientSession>()
            .container
            .as_mut()
            .unwrap()
            .view = ContainerView::AttachedCampfire;
        app.update();
        assert!(app.world().get::<ParkedUi>(shell).is_none());
        app.world_mut().resource_mut::<InventoryState>().open = false;
        app.update();
        app.world_mut().resource_mut::<InventoryState>().open = true;
        app.update();
        assert!(
            app.world().get::<ParkedUi>(shell).is_some(),
            "Same anchor with a different logical view is still stale"
        );
        app.world_mut()
            .resource_mut::<InventoryView>()
            .source_container = Some((anchor, ContainerView::AttachedCampfire));
        app.update();
        assert!(app.world().get::<ParkedUi>(shell).is_none());
        app.world_mut().resource_mut::<ClientSession>().container = None;
        app.update();
        assert!(app.world().get::<ParkedUi>(shell).is_none());
        app.world_mut().resource_mut::<InventoryState>().open = false;
        app.update();
        app.world_mut().resource_mut::<InventoryState>().open = true;
        app.update();
        assert!(app.world().get::<ParkedUi>(shell).is_some());
        {
            let mut view = app.world_mut().resource_mut::<InventoryView>();
            view.player = true;
            view.source_container = None;
        }
        app.update();
        assert!(app.world().get::<ParkedUi>(shell).is_none());
        assert_eq!(
            app.world()
                .get::<tilt_ui::ElementClasses>(shell)
                .unwrap()
                .classes,
            ["player"]
        );
    }

    #[test]
    fn inventory_cache_retains_one_shell_but_releases_it_on_world_exit() {
        let mut world = World::new();
        world.init_resource::<InventoryDomCache>();
        world.insert_resource(InventoryView {
            visible: true,
            ..default()
        });
        let shell = world
            .spawn((
                ElementId("inventory-shell".into()),
                Node::default(),
                tilt_ui::PropertyBindings::default(),
            ))
            .id();
        restore_before_bindings(&mut world);
        assert!(world.get::<ParkedUi>(shell).is_some());
        assert!(world.get::<tilt_ui::PropertyBindings>(shell).is_none());
        let retained_entities = world.entities().count_spawned();
        for _ in 0..16 {
            world.resource_mut::<InventoryView>().open = true;
            restore_before_bindings(&mut world);
            assert!(world.get::<ParkedUi>(shell).is_none());
            assert!(world.get::<tilt_ui::PropertyBindings>(shell).is_some());
            world.resource_mut::<InventoryView>().open = false;
            restore_before_bindings(&mut world);
            assert_eq!(world.entities().count_spawned(), retained_entities);
            assert_eq!(
                world
                    .query::<&ElementId>()
                    .iter(&world)
                    .filter(|id| id.0 == "inventory-shell")
                    .count(),
                1
            );
        }
        world.resource_mut::<InventoryView>().visible = false;
        world.entity_mut(shell).despawn();
        restore_before_bindings(&mut world);
        assert!(world.resource::<InventoryDomCache>().parked.is_none());
        assert_eq!(world.entities().count_spawned(), retained_entities - 1);
        assert_eq!(world.query::<&ElementId>().iter(&world).count(), 0);
        let replacement = world
            .spawn((
                ElementId("inventory-shell".into()),
                Node::default(),
                tilt_ui::PropertyBindings::default(),
            ))
            .id();
        world.resource_mut::<InventoryView>().visible = true;
        restore_before_bindings(&mut world);
        assert!(world.get::<ParkedUi>(replacement).is_some());
        world.resource_mut::<InventoryView>().open = true;
        restore_before_bindings(&mut world);
        assert!(
            world
                .get::<tilt_ui::PropertyBindings>(replacement)
                .is_some()
        );
        assert_ne!(shell, replacement);
    }
}
