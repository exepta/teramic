use super::*;
use geometry::{ContourKind, Geometry};
use terra_core::{building_jobs::BuildArea, work_areas::WorkAreaKind};
use terra_voxel::{BlockSection, SectionPos, global_to_section};
use terra_world::streaming::LoadedSection;

pub(super) fn terrain() -> (ChunkStore, EditJournal) {
    let mut chunks = ChunkStore::default();
    for x in -1..=0 {
        for z in -1..=0 {
            let position = SectionPos::new(x, 0, z);
            chunks.loaded.insert(
                position,
                LoadedSection {
                    data: BlockSection::empty(position).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
    }
    for x in -4..=4 {
        for z in -4..=4 {
            let y = i64::from(x >= 0);
            let (owner, local) = global_to_section([x, y, z]);
            chunks
                .loaded
                .get_mut(&owner)
                .unwrap()
                .data
                .set_block(local, 1)
                .unwrap();
        }
    }
    (chunks, EditJournal::default())
}

#[test]
fn inclusive_negative_areas_produce_only_the_ground_rectangle() {
    let area = BuildArea::new([-16, 3, -16], [-1, 5, -1]).unwrap();
    let mut contours = Vec::new();
    append_area(
        &mut contours,
        WorkAreaMark {
            kind: WorkAreaKind::Work,
            bounds: area,
        },
    );
    assert_eq!(contours.len(), 4);
    assert_eq!(contours[0].from, [-16, -16]);
    assert_eq!(contours[1].to, [0, 0]);
    assert!(contours.iter().all(|edge| edge.height_hint == 3));
    assert!(!geometry::near_area(area, [100.0, 3.0, 100.0], 32.0));
    assert!(geometry::near_area(area, [-1.0, 30.0, -1.0], 0.0));
}

#[test]
fn neighboring_build_claims_remove_the_shared_edge_at_negative_coordinates() {
    let claims = BTreeSet::from([[-1, 0], [0, 0]]);
    let mut contours = Vec::new();
    append_claims(&mut contours, &claims, [-1.0, 2.0, 8.0], 128.0);
    assert_eq!(contours.len(), 6);
    assert!(
        contours
            .iter()
            .all(|edge| edge.kind == ContourKind::BuildBorder)
    );
    assert!(
        !contours
            .iter()
            .any(|edge| edge.from[0] == 0 && edge.to[0] == 0)
    );
    assert!(
        contours
            .iter()
            .any(|edge| edge.from == [-16, 0] && edge.to == [-16, 16])
    );
    assert!(
        contours
            .iter()
            .any(|edge| edge.from == [16, 0] && edge.to == [16, 16])
    );
}

#[test]
fn mist_banks_follow_loaded_ground_with_a_low_ceiling_and_bounded_geometry() {
    let (chunks, journal) = terrain();
    let edge = geometry::Contour {
        kind: ContourKind::Work,
        from: [-2, 0],
        to: [2, 0],
        height_hint: 0,
    };
    let mut remaining = 3;
    let geometry = Geometry::from_contours(
        &[edge],
        ContourKind::Work,
        &chunks,
        &journal,
        &mut remaining,
    );
    assert_eq!(geometry.segments, 3);
    assert_eq!(remaining, 0);
    assert_eq!(geometry.positions.len(), 3 * geometry::VERTICES_PER_BANK);
    let mesh = geometry.mesh();
    let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("Proxy positions");
    };
    let Some(bevy::mesh::VertexAttributeValues::Float32x3(centers)) =
        mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        panic!("Density domains");
    };
    let Some(bevy::mesh::VertexAttributeValues::Float32x4(bounds)) =
        mesh.attribute(Mesh::ATTRIBUTE_COLOR)
    else {
        panic!("Volume bounds");
    };
    let Some(bevy::mesh::VertexAttributeValues::Float32x2(slopes)) =
        mesh.attribute(Mesh::ATTRIBUTE_UV_0)
    else {
        panic!("Ground slopes");
    };
    for (index, vertices) in positions.chunks_exact(8).enumerate() {
        let center = Vec3::from_array(centers[index * 8]);
        let half = Vec3::from_array(bounds[index * 8][..3].try_into().unwrap());
        let slope = Vec2::from_array(slopes[index * 8]);
        let profile = bounds[index * 8][3];
        assert!((profile - geometry::MIST_HEIGHT * 0.5).abs() < 0.000_001);
        // The whole sloping profile must fit inside the actual overlapping
        // domain. Comparing to a literal one-meter delta is incorrect in f32:
        // 2.045_f32 - 1.045_f32 is 1.000000119, not exactly 1.0.
        let required_half_height = profile + slope.x.abs() * half.x + slope.y.abs() * half.z;
        assert!((half.y - required_half_height).abs() < 0.000_001);
        assert_eq!(half.x, 1.0, "The Hann domain spans adjacent centers");
        assert!((half.z - 0.78).abs() < 0.000_001);
        assert!(vertices.iter().all(|p| {
            (Vec3::from_array(*p) - center)
                .abs()
                .abs_diff_eq(half, 0.0001)
        }));
        assert!(vertices.iter().any(|p| p[1] > center.y));
        assert!(vertices.iter().any(|p| p[1] < center.y));
        let base = center.y - geometry::MIST_HEIGHT * 0.5;
        let expected_base = [1.045, 1.545, 2.045][index];
        assert!((base - expected_base).abs() < 0.000_001);
        for x in [-half.x, half.x] {
            for z in [-half.z, half.z] {
                let floor = base + slope.dot(Vec2::new(x, z));
                assert!(center.y - half.y <= floor + 0.000_001);
                assert!(center.y + half.y + 0.000_001 >= floor + geometry::MIST_HEIGHT);
            }
        }
    }
    assert!(slopes.iter().any(|s| s[0].abs() > 0.5));
    let indices = mesh.indices().unwrap().iter().collect::<Vec<_>>();
    assert_eq!(indices.len(), 3 * 36);
    for triangle in indices.chunks_exact(3) {
        let [a, b, c] =
            [triangle[0], triangle[1], triangle[2]].map(|i| Vec3::from_array(positions[i]));
        let center = Vec3::from_array(centers[triangle[0]]);
        assert!(
            (b - a).cross(c - a).dot((a + b + c) / 3.0 - center) > 0.0,
            "Proxy exits require outward triangle winding"
        );
    }
}

#[test]
fn flow_drifts_spatially_and_wraps_without_noise_discontinuities() {
    for time in [0.0, 0.4, -10.0, 1.0e15, f64::NAN, f64::INFINITY] {
        assert!(material::flow(time).is_finite());
    }
    assert_ne!(
        material::flow(0.0).truncate(),
        material::flow(0.4).truncate()
    );
    assert_eq!(material::flow(0.25), material::flow(2048.25));
    for drift in [2048.0 * 0.25, 2048.0 * 0.125, 2048.0 * 0.125] {
        assert_eq!(drift % 64.0, 0.0);
    }
    let texture = material::density_texture();
    assert_eq!(
        texture.texture_descriptor.dimension,
        bevy::render::render_resource::TextureDimension::D3
    );
    assert_eq!(texture.data.unwrap().len(), 32 * 32 * 32);
    assert!(!MistMaterial::enable_prepass());
    assert!(!MistMaterial::enable_shadows());
}

#[test]
fn mist_shader_parses_with_and_without_depth_prepass() {
    use wesl::{
        pass::{Features, condcomp},
        syntax::TranslationUnit,
    };
    let shader = include_str!("../../../../assets/shaders/terra_area_mist.wesl");
    for depth in [false, true] {
        let mut module = shader.parse::<TranslationUnit>().unwrap();
        let mut features = Features::new();
        features.set("DEPTH_PREPASS", depth);
        condcomp(&mut module, &features).unwrap();
        let source = module.to_string();
        assert_eq!(source.contains("prepass_depth(input.position"), depth);
        assert!(source.contains("frag_depth"));
        assert!(source.contains("first_hit"));
        assert!(source.contains("i < 8u"));
        assert!(source.contains("texture_3d<f32>"));
    }
}

#[test]
fn confirmed_edits_move_ground_contours_but_unknown_voxels_never_create_fake_floors() {
    let (chunks, mut journal) = terrain();
    assert_eq!(ground::height(&chunks, &journal, 0, 0, 0), Some(2.045));
    journal.record([0, 2, 0], 43).unwrap();
    assert_eq!(ground::height(&chunks, &journal, 0, 0, 0), Some(3.045));
    assert_eq!(ground::height(&chunks, &journal, 1000, 1000, 3), None);
    let edge = geometry::Contour {
        kind: ContourKind::Storage,
        from: [1000, 1000],
        to: [2000, 1000],
        height_hint: 0,
    };
    let mut remaining = 10;
    let geometry = Geometry::from_contours(
        &[edge],
        ContourKind::Storage,
        &chunks,
        &journal,
        &mut remaining,
    );
    assert_eq!(geometry.segments, 0);
    assert!(geometry.positions.is_empty());
    assert_eq!(remaining, 0);
}

#[test]
fn animation_is_finite_soft_and_changes_without_rebuilding_geometry() {
    for kind in 0..5 {
        for time in [0.0, 0.4, -10.0, 1.0e15, f32::NAN, f32::INFINITY] {
            let alpha = geometry::pulse(time, kind);
            assert!(alpha.is_finite() && (0.60..=0.92).contains(&alpha));
        }
    }
    assert_ne!(geometry::pulse(0.0, 0), geometry::pulse(0.4, 0));
}

#[test]
fn animation_uploads_advection_to_shared_materials_without_replacing_geometry() {
    let mut app = App::new();
    let mut session = ClientSession::default();
    session.ready = true;
    app.init_resource::<Time<Real>>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<MistMaterial>>()
        .init_resource::<WorkAreaOverlayStatus>()
        .init_resource::<ConstructionAreaDisplay>()
        .init_resource::<Cache>()
        .insert_resource(session)
        .insert_resource(WorkAreaDisplay {
            target: Some(WorkAreaTarget::Worker(7)),
        })
        .add_systems(Startup, setup)
        .add_systems(Update, animate);
    app.update();
    let material = app.world().resource::<Cache>().materials[0].clone();
    let mesh = app.world().resource::<Cache>().meshes[0].clone();
    let first = app
        .world()
        .resource::<Assets<MistMaterial>>()
        .get(&material)
        .unwrap()
        .flow;
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(std::time::Duration::from_millis(500));
    app.update();
    let second = app
        .world()
        .resource::<Assets<MistMaterial>>()
        .get(&material)
        .unwrap()
        .flow;
    assert!(second.x > first.x && second.y > first.y && second.z > first.z);
    assert_eq!(
        app.world().resource::<WorkAreaOverlayStatus>().mist_drift,
        [second.x, second.y]
    );
    assert_eq!(app.world().resource::<Cache>().meshes[0], mesh);
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 5);
    assert_eq!(app.world().resource::<Assets<MistMaterial>>().len(), 5);
    assert_eq!(app.world().resource::<Assets<Image>>().len(), 1);
    assert_eq!(
        app.world().resource::<WorkAreaOverlayStatus>().generation,
        0
    );
}

#[test]
fn construction_hiding_overrides_manual_areas_and_keeps_the_same_entities() {
    let mut app = App::new();
    let mut session = ClientSession::default();
    session.ready = true;
    app.init_resource::<Time<Real>>()
        .init_resource::<Assets<MistMaterial>>()
        .init_resource::<WorkAreaOverlayStatus>()
        .init_resource::<terra_core::HudVisibility>()
        .insert_resource(session)
        .insert_resource(WorkAreaDisplay {
            target: Some(WorkAreaTarget::Worker(7)),
        })
        .insert_resource(ConstructionAreaDisplay {
            kingdom: Some(KingdomId(1)),
            visible: false,
        })
        .insert_resource(Cache {
            segments: [4, 4, 0, 0, 64],
            vertices: 100,
            ..Default::default()
        })
        .add_systems(Update, animate);
    let border = app
        .world_mut()
        .spawn((ContourBatch(4), Visibility::Inherited))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(border),
        Some(&Visibility::Hidden)
    );
    assert!(!app.world().resource::<WorkAreaOverlayStatus>().visible);
    app.world_mut()
        .resource_mut::<ConstructionAreaDisplay>()
        .visible = true;
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(border),
        Some(&Visibility::Inherited)
    );
    assert!(app.world().resource::<WorkAreaOverlayStatus>().visible);
    assert_eq!(
        app.world()
            .resource::<WorkAreaOverlayStatus>()
            .build_segments,
        64
    );
    app.world_mut()
        .resource_mut::<terra_core::HudVisibility>()
        .visible = false;
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(border),
        Some(&Visibility::Hidden)
    );
    app.world_mut()
        .resource_mut::<terra_core::HudVisibility>()
        .visible = true;
    app.world_mut().resource_mut::<ClientSession>().ready = false;
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(border),
        Some(&Visibility::Hidden)
    );
}
