use super::*;
use terra_core::{block_properties::BlockPropertiesCatalog, block_shapes::ShapeBox};
use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

#[test]
fn crusher_hopper_lips_have_clipped_colliders_above_positive_and_negative_section_seams() {
    for y in [-17, -1, 15, 31] {
        for facing in 0..4 {
            let anchor = [-1, y, -1];
            let state = BlockObjectState {
                facing,
                ..default()
            };
            let peer = std::array::from_fn(|axis| anchor[axis] + state.right()[axis]);
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            for (position, other, secondary) in [(anchor, peer, false), (peer, anchor, true)] {
                let (section, local) = global_to_section(position);
                let upper = section.offset([0, 1, 0]).unwrap();
                for owner in [section, upper] {
                    chunks.loaded.entry(owner).or_insert_with(|| LoadedSection {
                        data: BlockSection::empty(owner).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    });
                }
                chunks
                    .loaded
                    .get_mut(&section)
                    .unwrap()
                    .data
                    .set_block(local, 99)
                    .unwrap();
                journal.record(position, 99).unwrap();
                journal
                    .set_object_state(
                        position,
                        Some(BlockObjectState {
                            peer: Some(other),
                            secondary,
                            ..state
                        }),
                    )
                    .unwrap();
            }
            let properties = BlockPropertiesCatalog::default();
            let upper_sections: HashSet<_> = [anchor, peer]
                .map(|position| global_to_section(position).0.offset([0, 1, 0]).unwrap())
                .into_iter()
                .collect();
            for upper in upper_sections {
                let data = &chunks.loaded[&upper].data;
                let shapes = model_shapes(data, &chunks, &journal, &properties).unwrap();
                assert!(
                    !shapes.is_empty(),
                    "missing hopper collider at seam {y}, facing {facing}, upper {upper:?}"
                );
                assert!(
                    shapes
                        .iter()
                        .all(|shape| shape.minimum[1] >= 0.0 && shape.maximum[1] <= 0.231)
                );
                assert!(
                    shapes
                        .iter()
                        .any(|shape| (shape.maximum[1] - 0.23).abs() < 0.000001)
                );
                let collider = build_collider_with_models(data, &shapes, &[99])
                    .unwrap()
                    .unwrap();
                let triangles = collider.shape().as_trimesh().unwrap();
                assert!(!triangles.indices().is_empty());
                assert!(
                    triangles
                        .vertices()
                        .iter()
                        .all(|vertex| vertex.is_finite() && vertex.y >= 0.0 && vertex.y <= 0.231)
                );
                assert_eq!(
                    shapes,
                    model_shapes(data, &chunks, &journal, &properties).unwrap()
                );
            }
        }
    }
}

#[test]
fn real_capsule_can_walk_through_open_doors_and_retreat_from_windows() {
    #[derive(Resource)]
    struct Probe(Vec3, Vec3);
    fn step(movement: MoveAndSlide, mut probe: ResMut<Probe>) {
        let capsule = Collider::capsule(0.32, 1.16);
        let output = movement.move_and_slide(
            &capsule,
            probe.0,
            Quat::IDENTITY,
            probe.1,
            std::time::Duration::from_secs_f64(1.0 / 60.0),
            &MoveAndSlideConfig::default(),
            &SpatialQueryFilter::default(),
            |_| MoveAndSlideHitResponse::Accept,
        );
        probe.0 = output.position;
    }
    use bevy::ecs::system::RunSystemOnce;
    for material in 65..=70 {
        for facing in 0..4 {
            let section = SectionPos::default();
            let mut data = BlockSection::empty(section).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    data.set_block([x, 0, z], 3).unwrap();
                }
            }
            data.set_block([8, 1, 8], material).unwrap();
            if material <= 67 {
                data.set_block([8, 2, 8], material).unwrap();
            }
            let mut chunks = ChunkStore::default();
            chunks.loaded.insert(
                section,
                LoadedSection {
                    data: data.clone(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
            let mut journal = EditJournal::default();
            journal.record([8, 1, 8], material).unwrap();
            journal
                .set_object_state(
                    [8, 1, 8],
                    Some(BlockObjectState {
                        facing: facing + 4,
                        peer: (material <= 67).then_some([8, 2, 8]),
                        ..default()
                    }),
                )
                .unwrap();
            if material <= 67 {
                journal.record([8, 2, 8], material).unwrap();
                journal
                    .set_object_state(
                        [8, 2, 8],
                        Some(BlockObjectState {
                            facing: facing + 4,
                            peer: Some([8, 1, 8]),
                            secondary: true,
                            ..default()
                        }),
                    )
                    .unwrap();
            }
            let shapes =
                model_shapes(&data, &chunks, &journal, &BlockPropertiesCatalog::default()).unwrap();
            let collider = build_collider_with_models(&data, &shapes, &[material])
                .unwrap()
                .unwrap();
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs_f64(1.0 / 60.0),
            ));
            app.world_mut()
                .spawn((RigidBody::Static, collider, Transform::default()));
            let rotation = Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2);
            let start = Vec3::new(8.5, 1.91, 8.5) + rotation * Vec3::new(0.025, 0.0, -1.1);
            let forward = rotation * Vec3::Z * 3.0;
            app.insert_resource(Probe(start, forward));
            app.finish();
            app.cleanup();
            for _ in 0..5 {
                app.update();
            }
            for _ in 0..60 {
                app.update();
                app.world_mut().run_system_once(step).unwrap();
            }
            let end = app.world().resource::<Probe>().0;
            if material <= 67 {
                assert!(
                    (end - start).dot(forward.normalize()) > 2.6,
                    "door {material}/{facing} trapped capsule: {start:?} -> {end:?}"
                );
            } else {
                assert!(
                    (end - start).dot(forward.normalize()) < 1.0,
                    "window {material}/{facing} did not participate in physics: {start:?} -> {end:?}"
                );
            }
            app.world_mut().resource_mut::<Probe>().1 = -forward;
            for _ in 0..60 {
                app.update();
                app.world_mut().run_system_once(step).unwrap();
            }
            let back = app.world().resource::<Probe>().0;
            assert!(
                (end - back).dot(forward.normalize()) > 2.6,
                "opening {material}/{facing} prevents retreat: {end:?} -> {back:?}"
            );
        }
    }
}

#[test]
fn furnace_upper_collision_and_picking_survive_unloaded_anchor_without_becoming_a_cube() {
    let origin = [2, 15, 2];
    let upper = [2, 16, 2];
    for facing in 0..4 {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let properties = BlockPropertiesCatalog::default();
        for (position, peer, secondary) in [(origin, upper, false), (upper, origin, true)] {
            let (section, cell) = global_to_section(position);
            let mut data = BlockSection::empty(section).unwrap();
            data.set_block(cell, 59).unwrap();
            journal.record(position, 59).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        facing,
                        peer: Some(peer),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
            chunks.loaded.insert(
                section,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
        let (primary_section, _) = global_to_section(origin);
        let (upper_section, _) = global_to_section(upper);
        let before = model_shapes(
            &chunks.loaded[&upper_section].data,
            &chunks,
            &journal,
            &properties,
        )
        .unwrap();
        assert!(!before.is_empty());
        assert!(
            before
                .iter()
                .all(|shape| shape.minimum[1] >= 0.0 && shape.maximum[1] <= 1.0)
        );
        chunks.loaded.remove(&primary_section);
        let data = &chunks.loaded[&upper_section].data;
        let shapes = model_shapes(data, &chunks, &journal, &properties).unwrap();
        assert_eq!(shapes, before);
        let collider = build_collider_with_models(data, &shapes, &[59])
            .unwrap()
            .unwrap();
        let origin = Vec3::new(2.5, 0.7, 4.0);
        assert!(
            collider
                .cast_ray(Vec3::ZERO, Quat::IDENTITY, origin, Vec3::NEG_Z, 4.0, true)
                .is_some()
        );
        assert!(
            collider
                .cast_ray(
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    Vec3::new(2.05, 0.7, 4.0),
                    Vec3::NEG_Z,
                    4.0,
                    true
                )
                .is_none()
        );
        let hit = terra_world::interaction_shapes::raycast(
            &chunks,
            &journal,
            &properties,
            [2.5, 16.7, 4.0],
            [0.0, 0.0, -1.0],
            4.0,
        )
        .unwrap();
        assert_eq!(hit.position, upper);
    }
}

#[test]
fn torches_keep_authored_picking_boxes_but_never_enter_movement_colliders() {
    let properties = BlockPropertiesCatalog::default();
    assert!(!properties.shape(40).is_empty());
    assert!(!terra_voxel::MaterialId::Torch.is_solid());
    let mut data = BlockSection::empty(SectionPos::default()).unwrap();
    data.set_block([1, 2, 3], 40).unwrap();
    assert!(build_collider(&data).unwrap().is_none());
    assert!(
        model_shapes(
            &data,
            &ChunkStore::default(),
            &EditJournal::default(),
            &properties
        )
        .unwrap()
        .is_empty()
    );
}

fn rotate([x, y, z]: [f64; 3], facing: u8) -> [f64; 3] {
    match facing {
        0 => [x, y, z],
        1 => [z, y, -x],
        2 => [-x, y, -z],
        _ => [-z, y, x],
    }
}
fn point(origin: [i64; 3], source: [f64; 3], facing: u8, width: f64) -> [f64; 3] {
    let p = rotate(
        [source[0] * width - 0.5, source[1], source[2] - 0.5],
        facing,
    );
    std::array::from_fn(|axis| origin[axis] as f64 + p[axis] + if axis == 1 { 0.0 } else { 0.5 })
}

#[test]
fn furniture_collision_and_server_picking_match_authored_gaps_in_every_facing_across_section_edges()
{
    for material in [28, 29] {
        for facing in 0..4 {
            let origin = [-1, 4, -1];
            let owner = global_to_section(origin).0;
            let section_origin = owner.origin().unwrap();
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            let properties = BlockPropertiesCatalog::default();
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let position = owner.offset([dx, 0, dz]).unwrap();
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
            let state = BlockObjectState {
                facing,
                ..default()
            };
            let second = std::array::from_fn(|axis| origin[axis] + state.right()[axis]);
            for (position, peer, secondary) in [(origin, second, false), (second, origin, true)]
                .into_iter()
                .take(if material == 28 { 2 } else { 1 })
            {
                let (section, cell) = global_to_section(position);
                chunks
                    .loaded
                    .get_mut(&section)
                    .unwrap()
                    .data
                    .set_block(cell, material)
                    .unwrap();
                journal.record(position, material).unwrap();
                journal
                    .set_object_state(
                        position,
                        Some(BlockObjectState {
                            peer: (material == 28).then_some(peer),
                            secondary,
                            ..state
                        }),
                    )
                    .unwrap();
            }
            let mut boxes = Vec::new();
            assert!(terra_world::interaction_shapes::visit_boxes(
                &chunks,
                &journal,
                &properties,
                origin,
                |mut shape| {
                    for (axis, value) in section_origin.iter().enumerate() {
                        shape.minimum[axis] -= *value as f64;
                        shape.maximum[axis] -= *value as f64;
                    }
                    boxes.push(shape);
                }
            ));
            let data = &chunks.loaded[&owner].data;
            let collider = build_collider_with_models(data, &boxes, &[28, 29])
                .unwrap()
                .unwrap();
            let triangles = collider.shape().as_trimesh().unwrap();
            assert!(triangles.vertices().iter().all(|vertex| vertex.is_finite()));
            assert!(!triangles.indices().is_empty());
            for indices in triangles.indices() {
                assert!(
                    indices
                        .iter()
                        .all(|index| (*index as usize) < triangles.vertices().len())
                );
                let [a, b, c] = indices.map(|index| triangles.vertices()[index as usize]);
                assert!((b - a).cross(c - a).length_squared() > 0.0);
            }
            let probes: &[(f64, f64, bool)] = if material == 28 {
                &[(0.53, 0.52, false), (0.53, 0.70, true)]
            } else {
                &[
                    (0.5, 0.85, false),
                    (0.5, 0.4, true),
                    (0.02, 0.4, false),
                    (0.5, 0.78, true),
                ]
            };
            for &(x, y, expected_hit) in probes {
                let ray_origin = point(
                    origin,
                    [x, y, 2.5],
                    facing,
                    if material == 28 { 2.0 } else { 1.0 },
                );
                let direction = rotate([0.0, 0.0, -1.0], facing);
                let picked = terra_world::interaction_shapes::raycast(
                    &chunks,
                    &journal,
                    &properties,
                    ray_origin,
                    direction,
                    4.0,
                );
                let local: [f64; 3] =
                    std::array::from_fn(|axis| ray_origin[axis] - section_origin[axis] as f64);
                let collision = collider.cast_ray(
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    Vec3::from_array(local.map(|v| v as f32)),
                    Vec3::from_array(direction.map(|v| v as f32)),
                    4.0,
                    true,
                );
                let authored = boxes
                    .iter()
                    .filter_map(|bounds| bounds.ray(local, direction, 4.0))
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                assert_eq!(
                    picked.is_some(),
                    expected_hit,
                    "material {material}, facing {facing}, {x}/{y}"
                );
                assert_eq!(collision.is_some(), expected_hit);
                assert_eq!(authored.is_some(), expected_hit);
                if let (Some(picked), Some((distance, normal)), Some(authored)) =
                    (picked, collision, authored)
                {
                    assert!((picked.distance - f64::from(distance)).abs() < 1.0e-4);
                    assert!((authored.0 - f64::from(distance)).abs() < 1.0e-4);
                    assert!(normal.is_finite());
                }
            }
        }
    }
}

#[test]
fn ordinary_solid_blocks_keep_the_original_collider_and_custom_models_never_fall_back_to_cubes() {
    let mut data = BlockSection::empty(SectionPos::default()).unwrap();
    data.set_block([4, 4, 4], 3).unwrap();
    let ordinary = build_collider(&data).unwrap().unwrap();
    let custom_aware = build_collider_with_models(&data, &[], &[28, 29])
        .unwrap()
        .unwrap();
    let ray = |collider: &Collider| {
        collider
            .cast_ray(
                Vec3::ZERO,
                Quat::IDENTITY,
                Vec3::new(4.01, 4.01, 6.0),
                Vec3::NEG_Z,
                4.0,
                true,
            )
            .unwrap()
    };
    assert_eq!(ray(&ordinary), ray(&custom_aware));
    assert_eq!(ray(&ordinary).0, 1.0);
    data.set_block([4, 4, 4], 28).unwrap();
    assert!(
        build_collider_with_models(&data, &[], &[28, 29])
            .unwrap()
            .is_none()
    );
    let shapes = [ShapeBox {
        hinged: false,
        minimum: [4.2, 4.2, 4.2],
        maximum: [4.8, 4.8, 4.8],
    }];
    let collider = build_collider_with_models(&data, &shapes, &[28, 29])
        .unwrap()
        .unwrap();
    assert!(
        collider
            .cast_ray(
                Vec3::ZERO,
                Quat::IDENTITY,
                Vec3::new(4.01, 4.01, 6.0),
                Vec3::NEG_Z,
                4.0,
                true
            )
            .is_none()
    );
}

#[test]
fn secondary_sections_own_clipped_furniture_even_before_primary_chunk_arrives() {
    for material in [28, 29] {
        for (facing, origin) in [[-1, 4, -1], [-1, 4, 0], [0, 4, -1], [-1, 4, -1]]
            .into_iter()
            .enumerate()
        {
            let facing = facing as u8;
            let state = BlockObjectState {
                facing,
                ..default()
            };
            let second = std::array::from_fn(|axis| origin[axis] + state.right()[axis]);
            let primary_section = global_to_section(origin).0;
            let secondary_section = global_to_section(second).0;
            assert_ne!(primary_section, secondary_section);
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            let properties = BlockPropertiesCatalog::default();
            for (position, peer, secondary) in [(origin, second, false), (second, origin, true)] {
                let (owner, cell) = global_to_section(position);
                let mut data = BlockSection::empty(owner).unwrap();
                data.set_block(cell, material).unwrap();
                journal.record(position, material).unwrap();
                journal
                    .set_object_state(
                        position,
                        Some(BlockObjectState {
                            peer: Some(peer),
                            secondary,
                            ..state
                        }),
                    )
                    .unwrap();
                chunks.loaded.insert(
                    owner,
                    LoadedSection {
                        data,
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
            let before = model_shapes(
                &chunks.loaded[&secondary_section].data,
                &chunks,
                &journal,
                &properties,
            )
            .unwrap();
            assert!(!before.is_empty());
            for section in [primary_section, secondary_section] {
                let shapes = model_shapes(
                    &chunks.loaded[&section].data,
                    &chunks,
                    &journal,
                    &properties,
                )
                .unwrap();
                assert!(!shapes.is_empty());
                assert!(shapes.iter().all(|bounds| (0..3).all(|axis| {
                    bounds.minimum[axis] >= 0.0
                        && bounds.maximum[axis] <= 16.0
                        && bounds.minimum[axis] < bounds.maximum[axis]
                })));
            }
            chunks.loaded.remove(&primary_section);
            let secondary = &chunks.loaded[&secondary_section];
            let shapes = model_shapes(&secondary.data, &chunks, &journal, &properties).unwrap();
            assert_eq!(shapes, before);
            let collider = build_collider_with_models(&secondary.data, &shapes, &[28, 29])
                .unwrap()
                .unwrap();
            let y = if material == 28 { 0.70 } else { 0.40 };
            let eye = point(origin, [0.8, y, 2.5], facing, 2.0);
            let section_origin = secondary_section.origin().unwrap();
            let local =
                std::array::from_fn(|axis| (eye[axis] - section_origin[axis] as f64) as f32);
            let direction = rotate([0.0, 0.0, -1.0], facing).map(|v| v as f32);
            assert!(
                collider
                    .cast_ray(
                        Vec3::ZERO,
                        Quat::IDENTITY,
                        Vec3::from_array(local),
                        Vec3::from_array(direction),
                        4.0,
                        true
                    )
                    .is_some(),
                "material {material}, facing {facing}"
            );
            let epoch = journal.mesh_epoch(secondary_section);
            chunks
                .loaded
                .get_mut(&secondary_section)
                .unwrap()
                .mesh_epoch = epoch;
            let mut collision_state = TerrainCollisionState::default();
            collision_state.sections.insert(
                secondary_section,
                ReadySection {
                    profile: None,
                    entity: None,
                    epoch,
                },
            );
            let p = Vec3::from_array(second.map(|v| v as f32 + 0.5));
            assert!(collision_state.ready_for_aabb(
                p - Vec3::splat(0.1),
                p + Vec3::splat(0.1),
                &chunks,
                &journal
            ));
            journal.record(second, 0).unwrap();
            assert!(!collision_state.ready_for_aabb(
                p - Vec3::splat(0.1),
                p + Vec3::splat(0.1),
                &chunks,
                &journal
            ));
        }
    }
}

#[test]
fn same_section_pair_never_duplicates_its_model_boxes() {
    let origin = [4, 4, 4];
    let second = [5, 4, 4];
    let owner = global_to_section(origin).0;
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let properties = BlockPropertiesCatalog::default();
    let mut data = BlockSection::empty(owner).unwrap();
    for (position, peer, secondary) in [(origin, second, false), (second, origin, true)] {
        data.set_block(global_to_section(position).1, 28).unwrap();
        journal.record(position, 28).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    ..default()
                }),
            )
            .unwrap();
    }
    chunks.loaded.insert(
        owner,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    let both = model_shapes(&chunks.loaded[&owner].data, &chunks, &journal, &properties).unwrap();
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(global_to_section(second).1, 0)
        .unwrap();
    let once = model_shapes(&chunks.loaded[&owner].data, &chunks, &journal, &properties).unwrap();
    assert_eq!(both, once);
    assert!(!both.is_empty());
    for (index, bounds) in both.iter().enumerate() {
        assert!(!both[..index].contains(bounds));
    }
}
