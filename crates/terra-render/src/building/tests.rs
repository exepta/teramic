use super::*;
use bevy::{
    gizmos::{AppGizmoBuilder, GizmoAsset, config::DefaultGizmoConfigGroup},
    mesh::VertexAttributeValues,
    time::TimeUpdateStrategy,
};
use std::time::Duration;
use terra_core::{GameMode, ItemStack, WorldDrop};
use terra_network::{
    client::ConfirmedEdit,
    protocol::session::{EditAction, EditOutcome, MiningTarget},
};
use terra_player::{
    CameraPerspective, TerraCamera,
    interaction::{BlockPalette, InteractionMotion, MiningVisual},
};
use terra_voxel::{BlockSection, SectionPos};
use terra_world::streaming::LoadedSection;

#[derive(Resource)]
struct SharedAssets {
    meshes: usize,
    materials: usize,
}

fn assert_asset_counts(app: &App, transient_meshes: usize) {
    let shared = app.world().resource::<SharedAssets>();
    assert_eq!(
        app.world().resource::<Assets<Mesh>>().len(),
        shared.meshes + transient_meshes,
        "only the requested overlay/drop may add a mesh beyond shared startup assets"
    );
    assert_eq!(
        app.world().resource::<Assets<StandardMaterial>>().len(),
        shared.materials,
        "feedback and drops must reuse shared materials"
    );
}

pub(super) fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<GizmoAsset>>()
        .init_gizmo_group::<DefaultGizmoConfigGroup>()
        .init_resource::<BlockAssets>()
        .init_resource::<ChunkStore>()
        .init_resource::<ClientSession>()
        .init_resource::<CameraInputState>()
        .init_resource::<PlayerInputBlock>()
        .init_resource::<MiningVisual>()
        .init_resource::<PlacementPreview>()
        .init_resource::<InteractionMotion>()
        .init_resource::<BlockPalette>()
        .init_resource::<CameraPerspective>();
    app.world_mut().resource_mut::<ClientSession>().ready = true;
    let section = SectionPos::default();
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        section,
        LoadedSection {
            data: BlockSection::empty(section).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    install(&mut app);
    app.update();
    // Includes the two shared vessel meshes and their one water material.
    let shared = SharedAssets {
        meshes: app.world().resource::<Assets<Mesh>>().len(),
        materials: app.world().resource::<Assets<StandardMaterial>>().len(),
    };
    app.insert_resource(shared);
    app
}

fn removal(request: u64, outcome: EditOutcome) -> ConfirmedEdit {
    ConfirmedEdit {
        request,
        action: EditAction::Block {
            position: [8, 8, 8],
            material: 0,
            expected: 3,
        },
        outcome,
    }
}

fn oriented_preview(app: &mut App, position: [i64; 3], material: u16) {
    let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
    let mut templates = crate::blocks::TemplateSet::default();
    templates.blocks.insert(
        material,
        crate::blocks::BlockTemplate {
            surface: None,
            render_kind: crate::blocks::definitions::BlockRenderKind::CustomMesh,
            bevel_width: 0.0,
            faces: std::array::from_fn(|_| Vec::new()),
            face_materials: [0; 6],
            side_cap: None,
            details: Vec::new(),
            model: crate::blocks::cube::box_model(properties.shape(material), 0.0, [0; 6]),
        },
    );
    app.world_mut().resource_mut::<BlockAssets>().templates = Some(std::sync::Arc::new(templates));
    app.init_resource::<terra_world::edits::EditJournal>();
    *app.world_mut().resource_mut::<PlacementPreview>() = PlacementPreview {
        position: Some(position),
        material,
        valid: false,
        conveyor_start: None,
        plan: Some(terra_world::placement::Layout {
            edits: vec![terra_world::streaming::BlockEditRequest {
                position,
                expected_material: 0,
                material,
            }],
            states: Vec::new(),
            belts: Vec::new(),
        }),
        ..Default::default()
    };
}

#[test]
fn mining_feedback_emits_small_cube_chips_within_the_existing_particle_budget() {
    let mut app = app();
    app.world_mut()
        .write_message(removal(1, EditOutcome::Applied));
    app.update();
    let particles: Vec<_> = app
        .world_mut()
        .query_filtered::<&Transform, With<effects::BreakParticle>>()
        .iter(app.world())
        .collect();
    assert!(!particles.is_empty() && particles.len() <= 48);
    assert!(particles.iter().all(|particle| {
        let scale = particle.scale;
        scale.is_finite()
            && scale.x > 0.0
            && scale.x < 0.1
            && scale.x == scale.y
            && scale.y == scale.z
    }));
}

#[test]
fn twelve_fracture_stages_are_monotonic_finite_and_bounded_inside_face_edges() {
    for progress in [-1.0, 0.0, 0.024, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(fracture::stage(progress), 0);
    }
    assert_eq!(fracture::stage(0.025), 1);
    assert_eq!(fracture::stage(1.0), 12);
    assert_eq!(fracture::stage(4.0), 12);
    let stages: Vec<_> = (0..=100)
        .map(|n| fracture::stage(n as f32 / 100.0))
        .collect();
    assert!(stages.windows(2).all(|pair| pair[1] >= pair[0]));
    let mut previous_segments = 0;
    let mut previous_vertices = 0;
    for stage in 1..=12 {
        assert!(stages.contains(&stage));
        let (mesh, segments) = fracture::geometry([-2, 8, -3], stage, 63);
        let Some(VertexAttributeValues::Float32x3(points)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("Missing fracture positions");
        };
        assert!(segments >= previous_segments);
        assert!(points.len() >= previous_vertices);
        assert!(points.len() < 3_000);
        assert!(
            points
                .iter()
                .all(|p| p.iter().all(|v| v.is_finite() && v.abs() <= 0.504))
        );
        assert!(mesh.indices().unwrap().iter().all(|i| i < points.len()));
        for triangle in mesh
            .indices()
            .unwrap()
            .iter()
            .collect::<Vec<_>>()
            .chunks_exact(3)
        {
            let [a, b, c] =
                [triangle[0], triangle[1], triangle[2]].map(|i| Vec3::from_array(points[i]));
            assert!((b - a).cross(c - a).length_squared() > 1e-14);
        }
        previous_segments = segments;
        previous_vertices = points.len();
    }
}

#[test]
fn cracks_only_cover_visible_faces_and_cancel_with_gameplay_input() {
    let mut app = app();
    app.world_mut().resource_mut::<CameraInputState>().captured = true;
    {
        let mut mining = app.world_mut().resource_mut::<MiningVisual>();
        mining.target = Some(MiningTarget::Block {
            position: [8, 8, 8],
            expected: 3,
        });
        mining.progress = 0.5;
    }
    app.update();
    let before = app.world().resource::<BuildingStatus>().crack_segments;
    assert!(before > 0);
    let entity = app
        .world_mut()
        .query_filtered::<Entity, With<fracture::FractureOverlay>>()
        .single(app.world())
        .unwrap();
    let mesh = app
        .world_mut()
        .query_filtered::<&Mesh3d, With<fracture::FractureOverlay>>()
        .single(app.world())
        .unwrap()
        .0
        .clone();
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&SectionPos::default())
        .unwrap()
        .data
        .set_block([9, 8, 8], 3)
        .unwrap();
    app.update();
    assert!(app.world().resource::<BuildingStatus>().crack_segments < before);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<fracture::FractureOverlay>>()
            .single(app.world())
            .unwrap(),
        entity
    );
    assert_eq!(
        app.world_mut()
            .query_filtered::<&Mesh3d, With<fracture::FractureOverlay>>()
            .single(app.world())
            .unwrap()
            .0,
        mesh
    );
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([9, 8, 8]),
        Some(3)
    );
    app.world_mut().resource_mut::<PlayerInputBlock>().0 = true;
    app.update();
    assert_eq!(app.world().resource::<BuildingStatus>().crack_segments, 0);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<fracture::FractureOverlay>>()
            .iter(app.world())
            .count(),
        0
    );
    assert_asset_counts(&app, 0);
    app.world_mut().resource_mut::<PlayerInputBlock>().0 = false;
    app.world_mut().resource_mut::<MiningVisual>().target = None;
    app.update();
    assert_eq!(app.world().resource::<BuildingStatus>().crack_segments, 0);
}

#[test]
fn oriented_outline_reuses_one_entity_mesh_and_orange_material_for_both_validities() {
    let mut app = app();
    *app.world_mut().resource_mut::<DebugSettings>() = DebugSettings {
        enabled: true,
        block_placement: true,
        ..default()
    };
    app.world_mut().resource_mut::<CameraInputState>().captured = true;
    oriented_preview(&mut app, [-2, 8, -3], 82);
    app.update();
    let (entity, transform, material) = app
        .world_mut()
        .query_filtered::<(
            Entity,
            &Transform,
            &MeshMaterial3d<PlacementOutlineMaterial>,
        ), With<PlacementOutline>>()
        .single(app.world())
        .unwrap();
    assert_eq!(transform.translation, Vec3::new(-1.5, 8.5, -2.5));
    assert_eq!(material.0, app.world().resource::<BuildingAssets>().outline);
    assert!(app.world().resource::<BuildingStatus>().preview_visible);
    assert!(!app.world().resource::<BuildingStatus>().preview_valid);
    app.world_mut().resource_mut::<PlacementPreview>().valid = true;
    app.update();
    let (next_entity, material) = app
        .world_mut()
        .query_filtered::<(Entity, &MeshMaterial3d<PlacementOutlineMaterial>), With<PlacementOutline>>()
        .single(app.world())
        .unwrap();
    assert_eq!(next_entity, entity);
    assert_eq!(material.0, app.world().resource::<BuildingAssets>().outline);
    assert!(app.world().resource::<BuildingStatus>().preview_valid);
    assert_asset_counts(&app, 1);
    assert_eq!(
        app.world()
            .resource::<Assets<PlacementOutlineMaterial>>()
            .len(),
        1
    );
    app.world_mut().resource_mut::<ClientSession>().ready = false;
    app.update();
    assert!(!app.world().resource::<BuildingStatus>().preview_visible);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<PlacementOutline>>()
            .iter(app.world())
            .count(),
        0
    );
}

#[test]
fn invalid_rail_preview_is_red_and_other_placement_outlines_keep_their_existing_color() {
    for seconds in [0.0, 0.4, 1.2] {
        let ordinary = outline_color(seconds).to_srgba();
        for material in terra_core::rails::MATERIALS {
            let invalid = placement_outline_color(material, false, seconds).to_srgba();
            assert!(invalid.red > 0.9 && invalid.green < 0.2 && invalid.blue < 0.1);
            assert_eq!(invalid.alpha, ordinary.alpha);
            assert_eq!(
                placement_outline_color(material, true, seconds).to_srgba(),
                ordinary
            );
        }
        assert_eq!(
            placement_outline_color(82, false, seconds).to_srgba(),
            ordinary
        );
    }
}

#[test]
fn oriented_outline_is_available_without_debug_and_never_adds_a_filled_or_debug_cube() {
    let mut app = app();
    app.world_mut().resource_mut::<CameraInputState>().captured = true;
    oriented_preview(&mut app, [8; 3], 82);
    for (enabled, block_placement, visible) in [
        (false, false, true),
        (true, false, true),
        (false, true, true),
        (true, true, true),
        (false, true, true),
    ] {
        *app.world_mut().resource_mut::<DebugSettings>() = DebugSettings {
            enabled,
            block_placement,
            ..default()
        };
        app.update();
        assert_eq!(
            app.world().resource::<BuildingStatus>().preview_visible,
            visible
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<PlacementOutline>>()
                .iter(app.world())
                .count(),
            usize::from(visible)
        );
    }
}

#[test]
fn plain_cubes_never_get_a_placement_outline_even_with_stale_oriented_plan_or_debug_enabled() {
    let mut app = app();
    app.world_mut().resource_mut::<CameraInputState>().captured = true;
    oriented_preview(&mut app, [-17, 8, -1], 82);
    app.update();
    assert!(app.world().resource::<BuildingStatus>().preview_visible);
    for material in [
        0, 1, 2, 3, 4, 5, 6, 7, 12, 15, 16, 30, 41, 49, 50, 51, 98, 101,
    ] {
        app.world_mut().resource_mut::<PlacementPreview>().material = material;
        for debug in [false, true] {
            *app.world_mut().resource_mut::<DebugSettings>() = DebugSettings {
                enabled: debug,
                block_placement: debug,
                ..default()
            };
            app.update();
            assert!(
                !app.world().resource::<BuildingStatus>().preview_visible,
                "{material}"
            );
            assert_eq!(
                app.world_mut()
                    .query_filtered::<Entity, With<PlacementOutline>>()
                    .iter(app.world())
                    .count(),
                0
            );
        }
    }
    for material in [28, 29, 40, 59, 60, 65, 68, 71, 74, 77, 82, 85, 90, 99, 100] {
        assert!(orientation_preview(material), "{material}");
    }
}

#[test]
fn placement_outline_pulses_orange_without_rebuilding_geometry_or_growing_assets() {
    let mut app = app();
    app.world_mut().resource_mut::<CameraInputState>().captured = true;
    oriented_preview(&mut app, [8; 3], 82);
    app.update();
    let (entity, mesh) = app
        .world_mut()
        .query_filtered::<(Entity, &Mesh3d), With<PlacementOutline>>()
        .single(app.world())
        .map(|(e, m)| (e, m.0.clone()))
        .unwrap();
    let geometry = app.world().resource::<Assets<Mesh>>().get(&mesh).unwrap();
    assert_eq!(
        geometry.primitive_topology(),
        bevy::mesh::PrimitiveTopology::TriangleList
    );
    assert_eq!(geometry.count_vertices(), 48);
    assert_eq!(geometry.indices().unwrap().len(), 72);
    let material = app.world().resource::<BuildingAssets>().outline.clone();
    let mut range = [1.0_f32, 0.0_f32];
    for _ in 0..200 {
        app.update();
        let surface = app
            .world()
            .resource::<Assets<PlacementOutlineMaterial>>()
            .get(&material)
            .unwrap();
        let color = Color::from(surface.color).to_srgba();
        assert!((color.red - 1.0).abs() < 0.00001);
        assert!((color.green - 0.68).abs() < 0.00001);
        assert!((color.blue - 0.2).abs() < 0.00001);
        assert_eq!(surface.parameters.x, 2.0);
        assert!((0.3999..=0.9501).contains(&color.alpha));
        range[0] = range[0].min(color.alpha);
        range[1] = range[1].max(color.alpha);
        assert_eq!(app.world().get::<Mesh3d>(entity).unwrap().0, mesh);
        assert_asset_counts(&app, 1);
        assert_eq!(
            app.world()
                .resource::<Assets<PlacementOutlineMaterial>>()
                .len(),
            1
        );
    }
    assert!(range[1] - range[0] > 0.5);
}

#[test]
fn missing_orientation_geometry_never_falls_back_to_a_filled_cube() {
    let mut app = app();
    app.world_mut().resource_mut::<CameraInputState>().captured = true;
    *app.world_mut().resource_mut::<PlacementPreview>() = PlacementPreview {
        position: Some([8; 3]),
        material: 29,
        valid: true,
        conveyor_start: None,
        plan: None,
        ..Default::default()
    };
    app.update();
    assert!(!app.world().resource::<BuildingStatus>().preview_visible);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<PlacementOutline>>()
            .iter(app.world())
            .count(),
        0
    );
    assert_asset_counts(&app, 0);
}

#[test]
fn progressive_chipping_reuses_one_overlay_and_never_mutates_the_block() {
    let mut app = app();
    app.world_mut().resource_mut::<CameraInputState>().captured = true;
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&SectionPos::default())
        .unwrap()
        .data
        .set_block([8; 3], 3)
        .unwrap();
    app.world_mut().resource_mut::<MiningVisual>().target = Some(MiningTarget::Block {
        position: [8; 3],
        expected: 3,
    });
    let mut overlay = None;
    for stage in 1..=12 {
        app.world_mut().resource_mut::<MiningVisual>().progress = stage as f32 / 12.0;
        app.update();
        if app.world().resource::<BuildingStatus>().crack_segments == 0 {
            assert_asset_counts(&app, 0);
            continue;
        }
        let current = app
            .world_mut()
            .query_filtered::<Entity, With<fracture::FractureOverlay>>()
            .single(app.world())
            .unwrap();
        if let Some(overlay) = overlay {
            assert_eq!(current, overlay);
        }
        overlay = Some(current);
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at([8; 3]),
            Some(3)
        );
        assert!(app.world().resource::<BuildingStatus>().particles <= 48);
        assert_asset_counts(&app, 1);
    }
    assert!(app.world().resource::<BuildingStatus>().particles > 0);
    app.world_mut().resource_mut::<MiningVisual>().target = None;
    for _ in 0..35 {
        app.update();
    }
    assert_eq!(app.world().resource::<BuildingStatus>().particles, 0);
    assert_asset_counts(&app, 0);
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([8; 3]),
        Some(3)
    );
}

#[test]
fn empty_initial_damage_stage_never_uploads_a_zero_byte_mesh() {
    let mut app = app();
    app.world_mut().resource_mut::<CameraInputState>().captured = true;
    {
        let mut mining = app.world_mut().resource_mut::<MiningVisual>();
        mining.target = Some(MiningTarget::Block {
            position: [8; 3],
            expected: 3,
        });
        mining.progress = 0.03;
    }
    app.update();
    assert_eq!(app.world().resource::<BuildingStatus>().crack_segments, 0);
    assert_asset_counts(&app, 0);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<fracture::FractureOverlay>>()
            .iter(app.world())
            .count(),
        0
    );
    app.world_mut().resource_mut::<MiningVisual>().progress = 0.7;
    app.update();
    assert!(app.world().resource::<BuildingStatus>().crack_segments > 0);
    app.world_mut().resource_mut::<MiningVisual>().progress = 0.03;
    app.update();
    assert_asset_counts(&app, 0);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<fracture::FractureOverlay>>()
            .iter(app.world())
            .count(),
        0
    );
}

#[test]
fn identical_transparent_neighbors_hide_shared_damage_faces() {
    let mut app = app();
    for material in [7, 8, 9, 14, 15] {
        {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            let data = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
            for position in [
                [7, 8, 8],
                [9, 8, 8],
                [8, 7, 8],
                [8, 9, 8],
                [8, 8, 7],
                [8, 8, 9],
            ] {
                data.set_block(position, material).unwrap();
            }
        }
        let chunks = app.world().resource::<ChunkStore>();
        assert_eq!(fracture::exposed_faces(chunks, [8; 3], material), 0);
        assert_eq!(fracture::exposed_faces(chunks, [8; 3], 3), 63);
    }
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&SectionPos::default())
        .unwrap()
        .data
        .set_block([9, 8, 8], 3)
        .unwrap();
    assert_eq!(
        fracture::exposed_faces(app.world().resource::<ChunkStore>(), [8; 3], 3),
        61
    );
    assert_eq!(
        fracture::exposed_faces(app.world().resource::<ChunkStore>(), [20; 3], 15),
        0
    );
}

#[test]
fn rejected_edits_never_emit_particles_or_placement_flashes() {
    let mut app = app();
    for (request, outcome) in [
        EditOutcome::Conflict,
        EditOutcome::OutOfReach,
        EditOutcome::Occluded,
        EditOutcome::Invalid,
        EditOutcome::RateLimited,
    ]
    .into_iter()
    .enumerate()
    {
        app.world_mut()
            .write_message(removal(request as u64, outcome));
        let mut placement = removal(request as u64 + 10, outcome);
        placement.action = EditAction::Block {
            position: [8; 3],
            material: 3,
            expected: 0,
        };
        app.world_mut().write_message(placement);
    }
    app.update();
    assert_eq!(app.world().resource::<BuildingStatus>().particles, 0);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<effects::BreakParticle>>()
            .iter(app.world())
            .count(),
        0
    );
    assert!(app.world().resource::<Assets<GizmoAsset>>().is_empty());
}

#[test]
fn mining_hides_the_preview_and_grass_feedback_is_not_neutral_white() {
    let mut app = app();
    *app.world_mut().resource_mut::<DebugSettings>() = DebugSettings {
        enabled: true,
        block_placement: true,
        ..default()
    };
    app.world_mut().resource_mut::<CameraInputState>().captured = true;
    oriented_preview(&mut app, [8; 3], 82);
    app.world_mut().resource_mut::<InteractionMotion>().swinging = true;
    app.update();
    assert!(!app.world().resource::<BuildingStatus>().preview_visible);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<PlacementOutline>>()
            .iter(app.world())
            .count(),
        0
    );
    let mut event = removal(1, EditOutcome::Applied);
    event.action = EditAction::Block {
        position: [8; 3],
        expected: 1,
        material: 0,
    };
    app.world_mut().write_message(event);
    app.update();
    let turf = app.world().resource::<BuildingAssets>().turf_cube.clone();
    let particles: Vec<_> = app
        .world_mut()
        .query_filtered::<&Mesh3d, With<effects::BreakParticle>>()
        .iter(app.world())
        .map(|m| m.0.clone())
        .collect();
    assert_eq!(particles.len(), 12);
    assert!(particles.iter().all(|mesh| *mesh == turf));
    let mesh = app.world().resource::<Assets<Mesh>>().get(&turf).unwrap();
    let Some(bevy::mesh::VertexAttributeValues::Float32x4(colors)) =
        mesh.attribute(Mesh::ATTRIBUTE_COLOR)
    else {
        panic!("Grass feedback needs vertex tint");
    };
    assert_eq!(colors.len(), mesh.count_vertices());
    assert!(
        colors
            .iter()
            .all(|c| c[1] > c[0] && c[0] > c[2] && c[3] == 1.0)
    );
    app.world_mut().resource_mut::<InteractionMotion>().swinging = false;
    app.update();
    assert!(app.world().resource::<BuildingStatus>().preview_visible);
}

#[test]
fn confirmed_break_particles_have_a_hard_budget_and_expire_without_asset_growth() {
    let mut app = app();
    for request in 1..=40 {
        app.world_mut()
            .write_message(removal(request, EditOutcome::Applied));
    }
    app.update();
    assert_eq!(app.world().resource::<BuildingStatus>().particles, 48);
    let cube = app.world().resource::<BuildingAssets>().cube.clone();
    let parts: Vec<_> = app
        .world_mut()
        .query_filtered::<(&Mesh3d, &Transform), With<effects::BreakParticle>>()
        .iter(app.world())
        .map(|(mesh, transform)| (mesh.0.clone(), *transform))
        .collect();
    assert_eq!(parts.len(), 48);
    assert!(parts.iter().all(|(mesh, transform)| mesh == &cube
        && transform.translation.is_finite()
        && transform.scale.min_element() > 0.0));
    for _ in 0..35 {
        app.update();
    }
    assert_eq!(app.world().resource::<BuildingStatus>().particles, 0);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<effects::BreakParticle>>()
            .iter(app.world())
            .count(),
        0
    );
    assert_asset_counts(&app, 0);
    app.world_mut()
        .write_message(removal(100, EditOutcome::Applied));
    app.update();
    assert_eq!(app.world().resource::<BuildingStatus>().particles, 12);
    app.world_mut().resource_mut::<ClientSession>().ready = false;
    app.update();
    assert_eq!(app.world().resource::<BuildingStatus>().particles, 0);
    app.world_mut()
        .write_message(removal(101, EditOutcome::Applied));
    app.update();
    app.world_mut().resource_mut::<ClientSession>().ready = true;
    app.update();
    assert_eq!(
        app.world().resource::<BuildingStatus>().particles,
        0,
        "disconnected effects must not replay next session"
    );
}

#[test]
fn no_first_person_hand_is_spawned_in_either_game_mode() {
    let mut app = app();
    use bevy::ecs::system::RunSystemOnce;
    app.world_mut()
        .run_system_once(crate::players::assets)
        .unwrap();
    app.world_mut()
        .spawn((TerraCamera, Transform::from_xyz(8.0, 9.0, 8.0)));
    app.world_mut().resource_mut::<CameraInputState>().captured = true;
    let count = |app: &mut App| {
        app.world_mut()
            .query_filtered::<Entity, With<Mesh3d>>()
            .iter(app.world())
            .count()
    };
    app.update();
    assert_eq!(count(&mut app), 0);
    app.world_mut().resource_mut::<ClientSession>().game_mode = GameMode::Creative;
    app.update();
    assert_eq!(count(&mut app), 0);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.game_mode = GameMode::Survival;
        session.inventory.insert(ItemKind::Block(3), 1);
    }
    app.update();
    assert_eq!(count(&mut app), 0);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .take_one(ItemKind::Block(3));
    app.update();
    assert_eq!(count(&mut app), 0);
    *app.world_mut().resource_mut::<CameraPerspective>() = CameraPerspective::ThirdPerson;
    app.update();
    assert_eq!(count(&mut app), 0);
    *app.world_mut().resource_mut::<CameraPerspective>() = CameraPerspective::FirstPerson;
    app.world_mut().resource_mut::<PlayerInputBlock>().0 = true;
    app.update();
    assert_eq!(count(&mut app), 0);
}

#[test]
fn resource_drop_markers_share_assets_obey_the_cap_and_cleanup_on_disconnect() {
    let mut app = app();
    let material = app.world().resource::<BuildingAssets>().fallback.clone();
    let template = crate::blocks::BlockTemplate {
        surface: None,
        render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
        bevel_width: 0.0,
        faces: crate::blocks::cube_faces(0.0, [0; 6], None),
        face_materials: [0; 6],
        side_cap: None,
        details: Vec::new(),
        model: Vec::new(),
    };
    let mut templates = crate::blocks::TemplateSet::default();
    templates.blocks.insert(3, template);
    {
        let mut blocks = app.world_mut().resource_mut::<BlockAssets>();
        blocks.templates = Some(std::sync::Arc::new(templates));
        blocks.materials.push(material);
    }
    app.world_mut().resource_mut::<ClientSession>().drops = (1..=300)
        .map(|id| WorldDrop {
            rotation: [0.0, 0.0, 0.0, 1.0],
            angular_velocity: [0.0; 3],
            grounded: false,
            pickup_delay_seconds: 0.0,
            velocity: [0.0; 3],
            age_seconds: 0.0,
            id,
            position: [8.0, 8.0, 8.0],
            stack: ItemStack {
                item: ItemKind::Block(3),
                count: 1,
            },
        })
        .collect();
    app.update();
    assert_eq!(app.world().resource::<BuildingStatus>().drops, 256);
    let before: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<drops::ResourceDrop>>()
        .iter(app.world())
        .collect();
    assert_eq!(before.len(), 256);
    app.update();
    let after: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<drops::ResourceDrop>>()
        .iter(app.world())
        .collect();
    assert_eq!(before, after);
    assert_asset_counts(&app, 1);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .drops
        .truncate(1);
    app.update();
    assert_eq!(app.world().resource::<BuildingStatus>().drops, 1);
    app.world_mut().resource_mut::<ClientSession>().ready = false;
    app.update();
    assert_eq!(app.world().resource::<BuildingStatus>().drops, 0);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<drops::ResourceDrop>>()
            .iter(app.world())
            .count(),
        0
    );
}
