use super::*;
use terra_voxel::BlockSection;
use terra_world::streaming::LoadedSection;

#[test]
fn falling_drop_and_transient_splash_join_without_position_or_scale_jumps() {
    let source = Source {
        cell: [0, 8, 0],
        origin: Vec3::new(0.5, 8., 0.5),
        impact: 1.,
        seed: 0,
        distance: 2.,
    };
    let fall = ((source.origin.y - 0.085 - source.impact - 0.05) * 2. / 12.).sqrt();
    for boundary in [0.55, 0.55 + fall] {
        let before = motion::pose(source, boundary - 0.00001).unwrap().0;
        let after = motion::pose(source, boundary + 0.00001).unwrap().0;
        assert!(before.translation.distance(after.translation) < 0.001);
        assert!(before.scale.distance(after.scale) < 0.001);
    }
    assert!(
        motion::pose(source, 4.9).is_none(),
        "A splash never becomes a permanent ground decal"
    );
    assert_ne!(motion::hash([-1, 8, 0]), motion::hash([1, 8, 0]));
    for frame in 0..600 {
        if let Some((pose, _)) = motion::pose(source, frame as f32 / 60.) {
            assert!(pose.translation.is_finite() && pose.scale.is_finite());
            assert!(pose.translation.y >= source.impact);
            assert!(pose.scale.min_element() > 0.);
        }
    }
}

#[test]
fn source_pool_uses_loaded_leaf_undersides_and_removes_edited_or_unloaded_leaves() {
    let mut app = App::new();
    app.init_resource::<Time>()
        .insert_resource(WorldAccess::Replica { ready: true })
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<Cache>()
        .init_resource::<BloodDripStatus>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .add_systems(Startup, setup)
        .add_systems(Update, (sources::refresh, animate).chain());
    app.world_mut()
        .spawn((TerraCamera, Transform::from_xyz(8., 5., 8.)));
    let mut section = BlockSection::empty(SectionPos::default()).unwrap();
    for x in 0..16 {
        for z in 0..16 {
            section.set_block([x, 0, z], 3).unwrap();
            section.set_block([x, 8, z], LEAVES).unwrap();
        }
    }
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        SectionPos::default(),
        LoadedSection {
            data: section,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    app.update();
    let status = app.world().resource::<BloodDripStatus>();
    assert!(status.candidates > 0 && status.candidates <= MAX_DROPS);
    assert_eq!(status.pool_capacity, MAX_DROPS);
    assert_eq!(status.scanned_sections, 1);
    let original: Vec<_> = app
        .world()
        .resource::<Cache>()
        .sources
        .iter()
        .map(|source| source.cell)
        .collect();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs(1));
    app.update();
    assert_eq!(
        original,
        app.world()
            .resource::<Cache>()
            .sources
            .iter()
            .map(|source| source.cell)
            .collect::<Vec<_>>(),
        "Stable world positions select the same droplets independent of refresh order"
    );
    let mesh_handles: Vec<_> = app
        .world_mut()
        .query::<&Mesh3d>()
        .iter(app.world())
        .map(|mesh| mesh.0.id())
        .collect();
    assert_eq!(mesh_handles.len(), MAX_DROPS);
    assert!(mesh_handles.iter().all(|handle| *handle == mesh_handles[0]));
    let material_handles: Vec<_> = app
        .world_mut()
        .query::<&MeshMaterial3d<StandardMaterial>>()
        .iter(app.world())
        .map(|material| material.0.id())
        .collect();
    assert_eq!(material_handles.len(), MAX_DROPS);
    assert!(
        material_handles
            .iter()
            .all(|handle| *handle == material_handles[0])
    );
    let materials = app.world().resource::<Assets<StandardMaterial>>();
    assert_eq!(materials.len(), 1);
    let material = materials.get(material_handles[0]).unwrap();
    let color = material.base_color.to_linear();
    let luminance = 0.2126 * color.red + 0.7152 * color.green + 0.0722 * color.blue;
    assert!(
        (0.10..0.15).contains(&luminance),
        "Visible red droplets: {color:?}"
    );
    assert!(color.red > color.blue && color.blue > color.green);
    assert_eq!(material.alpha_mode, AlphaMode::Opaque);
    assert!(!material.unlit);
    assert_eq!(material.emissive, LinearRgba::BLACK);
    let positions: Vec<_> = app
        .world()
        .resource::<Cache>()
        .sources
        .iter()
        .map(|s| s.cell)
        .collect();
    for position in positions {
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record(position, 0)
            .unwrap();
    }
    app.update();
    assert_eq!(app.world().resource::<BloodDripStatus>().falling, 0);
    assert_eq!(app.world().resource::<BloodDripStatus>().splashes, 0);
    app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs(1));
    app.update();
    assert!(app.world().resource::<Cache>().sources.is_empty());
}
