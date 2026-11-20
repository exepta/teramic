use super::*;

#[test]
fn explicit_block_mining_survives_a_grass_wind_envelope_but_not_a_real_wall() {
    let (mut app, mut peer, support) = fixture(ResourcePropKind::SmallStones, false);
    peer.mode = Some(GameMode::Creative);
    let owner = terra_voxel::global_to_section(support).0;
    let grass = terra_world::props::WorldProp {
        id: terra_world::props::PropId(6001),
        kind: PropKind::Grass,
        position: [4.5, 4., 4.5],
        rotation: 0.,
        scale: 2.,
        variant: 0,
        section: owner,
        removed: false,
    };
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .props
        .push(grass);
    let eye = Vec3::new(4.5, 7., 7.5);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    let pose = PropPose::WeatherSwept {
        frame: terra_world::props::weather_wind_frame(
            app.world().resource::<terra_core::WorldWeather>().wind(),
        ),
    };
    let meshes = terra_world::props::visual::template(
        PropKind::Grass,
        grass.template_variant(),
        terra_world::props::visual::PropDetail::Near,
        &terra_world::props::visual::PropPalette::default(),
    )
    .unwrap();
    let rays = meshes.iter().flat_map(|mesh| {
        mesh.indices.chunks_exact(3).map(move |triangle| {
            let target: [f64; 3] = std::array::from_fn(|axis| {
                grass.position[axis]
                    + triangle
                        .iter()
                        .map(|i| {
                            f64::from(mesh.positions[*i as usize][axis])
                                * f64::from(grass.visual_scale())
                                / 3.
                        })
                        .sum::<f64>()
            });
            let target = Vec3::from_array(target.map(|v| v as f32));
            let rotation = Transform::from_translation(eye)
                .looking_at(target, Vec3::Y)
                .rotation;
            let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
            let direction = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.) * Vec3::NEG_Z;
            ([yaw, pitch], direction)
        })
    });
    let chunks = app.world().resource::<ChunkStore>();
    let journal = app.world().resource::<EditJournal>();
    let properties = app
        .world()
        .resource::<terra_core::block_properties::BlockPropertiesCatalog>();
    let (look, direction, block) = rays
        .filter_map(|(look, direction)| {
            let (block, selected) = terra_world::targeting::pick_world_for_mining(
                chunks,
                journal,
                properties,
                eye.to_array().map(f64::from),
                direction.to_array().map(f64::from),
                pose,
            );
            (matches!(selected, Some(WorldTarget::Prop(hit)) if hit.prop.id == grass.id))
                .then_some((look, direction, block?))
        })
        .next()
        .expect("the authority's grass envelope must cover this block ray");
    let command = EditCommand {
        request: 1,
        revision: journal.revision(owner),
        look,
        source: None,
        action: EditAction::Block {
            position: block.position,
            expected: 3,
            material: 0,
        },
    };
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let world = system.get_mut(app.world_mut()).unwrap();
        assert_eq!(check(command, &peer, &world), EditOutcome::Applied);
    }
    let obstruction = (eye + direction * 1.5).floor().as_i64vec3().to_array();
    assert_ne!(obstruction, block.position);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(obstruction.map(|v| v as usize), 3)
        .unwrap();
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let world = system.get_mut(app.world_mut()).unwrap();
    assert_eq!(check(command, &peer, &world), EditOutcome::Occluded);
}

#[test]
fn a_click_across_a_prediction_seam_keeps_its_face_for_building_and_mining() {
    let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
    peer.mode = Some(GameMode::Creative);
    // The client crossed x=5 before its movement packet reached authority.
    let eye = Vec3::new(5.08, 7., 8.5);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    let point = [4.96, 4., 5.5];
    let look = [0., -std::f32::consts::FRAC_PI_4];
    let actions = [
        EditAction::Block {
            position: [4, 3, 5],
            expected: 3,
            material: 0,
        },
        EditAction::Block {
            position: [4, 4, 5],
            expected: 0,
            material: 3,
        },
    ];
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    let aimed = block_look_at(point, look, &peer, &world);
    for action in actions {
        let command = EditCommand {
            request: 1,
            revision: 0,
            look,
            action,
            source: None,
        };
        assert_eq!(check(command, &peer, &world), EditOutcome::Occluded);
        assert_eq!(
            check(
                EditCommand {
                    look: aimed,
                    ..command
                },
                &peer,
                &world
            ),
            EditOutcome::Applied
        );
    }
    world
        .chunks
        .loaded
        .get_mut(&terra_voxel::SectionPos::default())
        .unwrap()
        .data
        .set_block([5, 6, 7], 3)
        .unwrap();
    for action in actions {
        let command = EditCommand {
            request: 1,
            revision: 0,
            look: aimed,
            action,
            source: None,
        };
        assert_eq!(check(command, &peer, &world), EditOutcome::Occluded);
    }
}

#[test]
fn building_into_a_bile_source_displaces_it_and_its_flow_drains() {
    let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
    peer.mode = Some(GameMode::Creative);
    let p = [7, 4, 7];
    let owner = global_to_section(p).0;
    let eye = Vec3::new(7.5, 7., 10.5);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    assert!(
        terra_world::bile::deposit_from_bucket(&mut world.chunks, &mut world.journal, p).unwrap()
    );
    for _ in 0..6 {
        terra_world::bile::advance(&mut world.chunks, &mut world.journal).unwrap();
    }
    assert!(
        world
            .journal
            .object_states()
            .filter(|(_, id, _)| *id == terra_core::bile::FLUID)
            .count()
            > 1
    );
    let command = EditCommand {
        request: 1,
        revision: world.journal.revision(owner),
        look: [0., -std::f32::consts::FRAC_PI_4],
        source: None,
        action: EditAction::Block {
            position: p,
            expected: terra_core::bile::FLUID,
            material: 3,
        },
    };
    assert_eq!(
        apply(command, &peer, &mut world, false),
        EditOutcome::Applied
    );
    assert_eq!(world.chunks.block_at(p), Some(3));
    assert!(world.journal.object_state(p).is_none());
    for _ in 0..100 {
        terra_world::bile::advance(&mut world.chunks, &mut world.journal).unwrap();
    }
    assert!(
        !world
            .journal
            .object_states()
            .any(|(_, id, _)| id == terra_core::bile::FLUID)
    );
    assert!(world.journal.gameplay().drops.is_empty());
}

#[test]
fn held_mining_keeps_the_clicked_surface_when_older_movement_arrives() {
    use crate::server::gameplay;
    use lightyear::prelude::{Connected, MessageSender, PeerId, RemoteId};
    let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
    let eye = Vec3::new(5.08, 7., 8.5);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = eye - Vec3::Y * PLAYER_EYE_OFFSET;
    // Dirt can be gathered with an empty hand in survival.
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&terra_voxel::SectionPos::default())
        .unwrap()
        .data
        .set_block([4, 3, 5], MaterialId::Dirt as u16)
        .unwrap();
    let intent = MiningIntent {
        target: Some(MiningTarget::Block {
            position: [4, 3, 5],
            expected: MaterialId::Dirt as u16,
        }),
        look: [0., -std::f32::consts::FRAC_PI_4],
        fell_tree: false,
    };
    let mut sender = MessageSender::<ServerControl>::default();
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        gameplay::intent(
            intent,
            Some([4.96, 4., 5.5]),
            &mut peer,
            &mut sender,
            &mut world,
        );
        assert!(peer.mining.is_some());
    }
    peer.look = [std::f32::consts::PI, 0.];
    let connection = app
        .world_mut()
        .spawn((peer, RemoteId(PeerId::Netcode(1)), sender))
        .id();
    app.world_mut().entity_mut(connection).insert(Connected);
    app.init_resource::<Time>()
        .init_resource::<terra_world::tree_felling::Queue>()
        .add_systems(Update, gameplay::tick);
    app.update();
    assert!(
        app.world()
            .get::<Peer>(connection)
            .unwrap()
            .mining
            .is_some()
    );
}
