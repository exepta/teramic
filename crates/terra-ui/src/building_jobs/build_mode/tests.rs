use super::*;
use terra_core::{
    ItemKind, PlayerUuid,
    building_jobs::{TemplatePreview, TemplatePreviewCell},
};

#[test]
fn enter_confirms_the_selected_site_once_and_respects_inactive_or_modal_input() {
    use terra_network::client::BuildingRequest;
    let mut app = App::new();
    app.add_message::<BuildingRequest>();
    let world = app.world_mut();
    world.init_resource::<ClientSession>();
    world.resource_mut::<ClientSession>().ready = true;
    world.insert_resource(ConstructionCamera {
        active: true,
        ..Default::default()
    });
    world.insert_resource(State {
        kingdom: Some(KingdomId(1)),
        origin: Some([4, 16, 22]),
        facing: 2,
        placing: true,
        template: "test-house".into(),
        preview: Some(TemplatePreview {
            id: "test-house".into(),
            cells: vec![TemplatePreviewCell {
                position: [0; 3],
                material: 3,
                facing: 0,
            }],
        }),
        ..Default::default()
    });
    world.init_resource::<ButtonInput<KeyCode>>();
    world
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    keyboard(world);
    camera::confirm(world);
    let requests: Vec<_> = world
        .resource_mut::<Messages<BuildingRequest>>()
        .drain()
        .collect();
    assert_eq!(
        requests.len(),
        1,
        "keyboard and repeated confirmation cannot duplicate a pending order"
    );
    assert!(
        matches!(&requests[0].0.action,BuildingAction::QueueHouse { kingdom:KingdomId(1),origin:[4,16,22],facing:2,template } if template=="test-house")
    );
    assert_eq!(
        world.resource::<State>().pending.unwrap().0,
        requests[0].0.request
    );
    world.resource_mut::<State>().pending = None;
    world.insert_resource(crate::InventoryView {
        open: true,
        ..Default::default()
    });
    keyboard(world);
    assert!(world.resource::<Messages<BuildingRequest>>().is_empty());
    world.resource_mut::<crate::InventoryView>().open = false;
    world.resource_mut::<ConstructionCamera>().active = false;
    camera::confirm(world);
    assert!(world.resource::<Messages<BuildingRequest>>().is_empty());
    world.resource_mut::<ConstructionCamera>().active = true;
    world.resource_mut::<State>().origin = None;
    camera::confirm(world);
    assert!(world.resource::<Messages<BuildingRequest>>().is_empty());
}

#[test]
fn construction_hitboxes_follow_real_node_and_parent_visibility_without_losing_layout() {
    use bevy::{camera::visibility::VisibilityPlugin, mesh::skinning::SkinnedMeshInverseBindposes};
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, VisibilityPlugin))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<SkinnedMeshInverseBindposes>>();
    let parent = app
        .world_mut()
        .spawn((Visibility::Visible, Transform::default()))
        .id();
    let panel = app
        .world_mut()
        .spawn((
            Node::default(),
            tilt_ui::ElementId("mini-map".into()),
            ComputedNode {
                size: Vec2::new(120.0, 80.0),
                ..Default::default()
            },
            UiGlobalTransform::from_xy(100.0, 100.0),
            Visibility::Inherited,
            ChildOf(parent),
        ))
        .id();
    let pointer = Vec2::new(100.0, 100.0);
    app.update();
    assert!(camera::over_ui(app.world_mut(), pointer));
    assert!(!camera::over_ui(app.world_mut(), Vec2::ZERO));

    // A local hide already applies before PostUpdate propagates inheritance.
    *app.world_mut().get_mut::<Visibility>(panel).unwrap() = Visibility::Hidden;
    assert!(app.world().get::<InheritedVisibility>(panel).unwrap().get());
    assert!(!camera::over_ui(app.world_mut(), pointer));
    app.update();
    assert!(!camera::over_ui(app.world_mut(), pointer));
    assert_eq!(
        app.world().get::<ComputedNode>(panel).unwrap().size(),
        Vec2::new(120.0, 80.0),
        "hiding keeps the real layout rectangle"
    );

    *app.world_mut().get_mut::<Visibility>(panel).unwrap() = Visibility::Inherited;
    *app.world_mut().get_mut::<Visibility>(parent).unwrap() = Visibility::Hidden;
    app.update();
    assert!(!camera::over_ui(app.world_mut(), pointer));
    assert_eq!(
        app.world().get::<Visibility>(panel),
        Some(&Visibility::Inherited)
    );
    assert!(
        !app.world().get::<InheritedVisibility>(panel).unwrap().get(),
        "actual Bevy propagation hides the child through its parent"
    );

    *app.world_mut().get_mut::<Visibility>(parent).unwrap() = Visibility::Visible;
    app.update();
    assert!(camera::over_ui(app.world_mut(), pointer));

    // Bevy explicitly Visible children escape their parent's Hidden state.
    *app.world_mut().get_mut::<Visibility>(parent).unwrap() = Visibility::Hidden;
    *app.world_mut().get_mut::<Visibility>(panel).unwrap() = Visibility::Visible;
    app.update();
    assert!(camera::over_ui(app.world_mut(), pointer));
}

#[test]
fn h_toggles_only_construction_areas_without_changing_manual_selection_or_debug_chords() {
    let mut world = World::new();
    world.init_resource::<ClientSession>();
    world.resource_mut::<ClientSession>().ready = true;
    world.insert_resource(ConstructionCamera {
        active: true,
        ..Default::default()
    });
    world.insert_resource(ConstructionAreaDisplay {
        kingdom: Some(KingdomId(1)),
        visible: true,
    });
    world.insert_resource(terra_core::work_areas::WorkAreaDisplay {
        target: Some(terra_core::work_areas::WorkAreaTarget::Worker(7)),
    });
    world.init_resource::<ButtonInput<KeyCode>>();
    world
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyH);
    keyboard(&mut world);
    assert!(!world.resource::<ConstructionAreaDisplay>().visible);
    assert_eq!(
        world
            .resource::<terra_core::work_areas::WorkAreaDisplay>()
            .target,
        Some(terra_core::work_areas::WorkAreaTarget::Worker(7))
    );
    world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    world
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyH);
    keyboard(&mut world);
    assert!(world.resource::<ConstructionAreaDisplay>().visible);
    world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    world
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F1);
    world
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyH);
    keyboard(&mut world);
    assert!(world.resource::<ConstructionAreaDisplay>().visible);
    world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    world
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyH);
    world.insert_resource(crate::NpcWorkView {
        direct: true,
        ..Default::default()
    });
    keyboard(&mut world);
    assert!(world.resource::<ConstructionAreaDisplay>().visible);
    world.resource_mut::<ConstructionCamera>().active = false;
    world.remove_resource::<crate::NpcWorkView>();
    keyboard(&mut world);
    assert!(world.resource::<ConstructionAreaDisplay>().visible);
}

#[test]
fn acknowledged_preview_survives_later_stock_only_responses() {
    let mut world = World::new();
    world.init_resource::<ClientSession>();
    world.insert_resource(State {
        template: "test-house".into(),
        ..Default::default()
    });
    let preview = TemplatePreview {
        id: "test-house".into(),
        cells: vec![TemplatePreviewCell {
            position: [0; 3],
            material: 3,
            facing: 0,
        }],
    };
    world.resource_mut::<ClientSession>().buildings.preview = Some(preview.clone());
    sync(&mut world);
    world.resource_mut::<ClientSession>().buildings.preview = None;
    sync(&mut world);
    assert_eq!(world.resource::<State>().preview, Some(preview));
    world.resource_mut::<State>().template = "different-house".into();
    sync(&mut world);
    assert_ne!(
        world.resource::<State>().preview.as_ref().unwrap().id,
        world.resource::<State>().template
    );
}

#[test]
fn build_claim_uses_euclidean_negative_coordinates_and_rejects_territory_and_foreign_realms() {
    let actor = PlayerUuid::from_u128(241);
    let mut session = ClientSession::default();
    session.uuid = Some(actor);
    let own = session.kingdoms.found(actor, [-1, 12, -1], "Own").unwrap();
    assert_eq!(
        super::super::stock::build_claim(&session, [-1, 12, -1]),
        Some(own)
    );
    assert_eq!(super::super::stock::build_claim(&session, [0, 12, 0]), None);
    session
        .kingdoms
        .found(PlayerUuid::from_u128(242), [32, 12, 32], "Other")
        .unwrap();
    assert_eq!(
        super::super::stock::build_claim(&session, [32, 12, 32]),
        None
    );
}

#[test]
fn material_availability_never_uses_another_towns_stock_or_converts_creative_to_real_items() {
    let mut session = ClientSession::default();
    let town = KingdomId(1);
    session.buildings.stock_kingdom = Some(town);
    session.buildings.stock.configured = true;
    session.buildings.stock.complete = true;
    session
        .buildings
        .stock
        .items
        .insert(ItemKind::Block(6), 100);
    assert_eq!(
        super::super::stock::supply(&session, Some(town), ItemKind::Block(6), 50),
        ("100 / 50".into(), true)
    );
    assert_eq!(
        super::super::stock::supply(&session, Some(KingdomId(2)), ItemKind::Block(6), 50),
        ("? / 50".into(), false)
    );
    session.buildings.unlimited_resources = true;
    assert_eq!(
        super::super::stock::supply(&session, Some(town), ItemKind::Block(3), 900),
        ("∞ / 900".into(), true)
    );
    assert_eq!(session.buildings.stock.count(ItemKind::Block(6)), 100);
}

#[test]
fn silhouette_mesh_removes_internal_faces_of_adjacent_voxels() {
    let preview = TemplatePreview {
        id: "test-house".into(),
        cells: vec![
            TemplatePreviewCell {
                position: [0, 0, 0],
                material: 3,
                facing: 0,
            },
            TemplatePreviewCell {
                position: [1, 0, 0],
                material: 3,
                facing: 0,
            },
        ],
    };
    let mesh = preview::mesh(&preview);
    assert_eq!(mesh.count_vertices(), 40);
    assert_eq!(mesh.indices().unwrap().len(), 60);
}

#[test]
fn detached_camera_presentation_does_not_move_the_player_body() {
    let mut world = World::new();
    world.init_resource::<ClientSession>();
    world.init_resource::<State>();
    world.insert_resource(ConstructionCamera {
        active: true,
        center: Vec3::new(3.0, 12.0, 8.0),
        height: 40.0,
    });
    world.insert_resource(terra_world::streaming::ChunkFocus([0.0; 3]));
    let body = world
        .spawn((
            terra_player::PlayerBody,
            Transform::from_xyz(3.0, 13.0, 8.0),
        ))
        .id();
    let camera = world
        .spawn((terra_player::TerraCamera, Transform::default()))
        .id();
    camera::update(&mut world);
    assert_eq!(
        world.get::<Transform>(body).unwrap().translation,
        Vec3::new(3.0, 13.0, 8.0)
    );
    assert_eq!(
        *world.get::<Transform>(camera).unwrap(),
        world.resource::<ConstructionCamera>().pose()
    );
}

#[test]
fn ghost_cell_centers_match_the_real_server_plan_in_all_four_facings() {
    let template = terra_core::building_jobs::template::BuildingTemplate::load(
        &terra_core::asset_directory()
            .join(terra_core::building_jobs::template::DEFAULT_HOUSE_PATH),
    )
    .unwrap();
    let source = TemplatePreview::from_template(&template).unwrap();
    let origin = [-24, 12, -40];
    for facing in 0..4 {
        let pose = preview::placement_pose(origin, facing);
        let placed = template.placed(origin, facing).unwrap();
        let actual: std::collections::BTreeSet<_> = placed
            .iter()
            .flat_map(|step| &step.cells)
            .map(|cell| cell.position)
            .collect();
        let mut transformed = std::collections::BTreeSet::new();
        for cell in &source.cells {
            let center = pose
                .transform_point(Vec3::from_array(cell.position.map(f32::from)) + Vec3::splat(0.5));
            let expected = (center - Vec3::splat(0.5))
                .to_array()
                .map(|axis| axis.round() as i64);
            assert!(center.abs_diff_eq(
                Vec3::from_array(expected.map(|p| p as f32)) + Vec3::splat(0.5),
                0.0001
            ));
            assert!(
                actual.contains(&expected),
                "Ghost differs from authoritative occupied cells: facing={facing}, cell={expected:?}"
            );
            transformed.insert(expected);
        }
        assert_eq!(transformed, actual);
    }
}
