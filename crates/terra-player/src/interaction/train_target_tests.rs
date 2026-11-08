use super::*;
use terra_core::{
    PlayerUuid, WoodSpecies, train_parts,
    trains::{TrainPart, Vehicle, VehicleKind},
};
use terra_voxel::{BlockSection, global_to_section};
use terra_world::{edits::EditJournal, streaming::LoadedSection};

fn vehicle() -> Vehicle {
    let vehicle = Vehicle {
        id: 1,
        owner: PlayerUuid::from_u128(210_501),
        kind: VehicleKind::Passenger,
        anchor: [0, 4, 0],
        parts: [
            ([0, 0, 0], train_parts::CHASSIS),
            ([0, -1, 0], train_parts::BOGIE),
            ([0, -1, 1], train_parts::BOGIE),
            ([0, 1, 1], train_parts::FRAME),
            ([0, 1, 0], train_parts::SEAT),
            ([0, 0, -1], train_parts::COUPLER),
        ]
        .into_iter()
        .map(|(offset, material)| TrainPart {
            offset,
            material,
            facing: 0,
            wood: WoodSpecies::Spruce,
        })
        .collect(),
        cargo: Default::default(),
        couplings: [None; 2],
        revision: 1,
    };
    assert!(vehicle.valid());
    vehicle
}

#[test]
fn assembled_vehicle_targets_occlude_blocks_but_respect_nearer_walls_and_unknown_terrain() {
    for replica in [false, true] {
        let mut app = App::new();
        let vehicle = vehicle();
        let ray_y = (vehicle.model_origin()[1] + 0.20) as f32;
        let wall_y = f64::from(ray_y).floor() as i64;
        let mut journal = EditJournal::default();
        journal
            .gameplay_mut()
            .trains
            .insert(vehicle.clone())
            .unwrap();
        let mut chunks = ChunkStore::default();
        for z in [-1, 0] {
            let owner = SectionPos::new(0, 0, z);
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data: BlockSection::empty(owner).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
        }
        let (owner, local) = global_to_section([0, wall_y, -2]);
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(local, 3)
            .unwrap();
        app.insert_resource(chunks)
            .insert_resource(journal)
            .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<TargetBlock>()
            .init_resource::<TargetWorld>()
            .init_resource::<TargetTrain>()
            .add_systems(Update, target_block);
        if replica {
            let mut session = terra_network::client::ClientSession::default();
            session.trains.vehicles = vec![vehicle.view()];
            app.insert_resource(session);
        }
        let transform = Transform::from_xyz(0.5, ray_y, 3.5).looking_to(Vec3::NEG_Z, Vec3::Y);
        let mut look = crate::CameraLook::default();
        look.aim(transform.rotation);
        app.world_mut().spawn((TerraCamera, transform, look));
        app.update();
        assert_eq!(
            app.world().resource::<TargetTrain>().0.unwrap().vehicle,
            vehicle.id
        );
        assert!(app.world().resource::<TargetBlock>().0.is_none());
        assert!(app.world().resource::<TargetWorld>().0.is_none());
        let (owner, local) = global_to_section([0, wall_y, 2]);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(local, 3)
            .unwrap();
        app.update();
        assert!(app.world().resource::<TargetTrain>().0.is_none());
        assert_eq!(
            app.world().resource::<TargetBlock>().0.unwrap().position,
            [0, wall_y, 2]
        );
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .remove(&owner);
        app.update();
        assert!(app.world().resource::<TargetTrain>().0.is_none());
    }
}

#[test]
fn long_track_targets_stop_at_real_train_geometry_without_extending_cargo_interaction() {
    let mut car = vehicle();
    car.anchor = [0, 4, -12];
    car.kind = VehicleKind::Freight;
    car.parts[4].material = train_parts::CARGO;
    car.cargo.insert(
        4,
        terra_core::Inventory {
            slots: Vec::new(),
            ..default()
        },
    );
    assert!(car.valid());
    let origin_y = car.model_origin()[1] as f32 + 1.4;
    let mut app = App::new();
    let mut journal = EditJournal::default();
    journal.gameplay_mut().trains.insert(car.clone()).unwrap();
    let mut chunks = ChunkStore::default();
    for z in -2..=0 {
        let owner = SectionPos::new(0, 0, z);
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data: BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
    }
    let floor = [0, origin_y.floor() as i64, -16];
    let (owner, local) = global_to_section(floor);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, 3)
        .unwrap();
    app.insert_resource(chunks)
        .insert_resource(journal)
        .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<TargetBlock>()
        .init_resource::<TargetWorld>()
        .init_resource::<TargetTrain>()
        .insert_resource(super::super::ActiveItem(Some(terra_core::ItemKind::Block(
            196,
        ))))
        .add_systems(Update, target_block);
    let transform = Transform::from_xyz(0.5, origin_y, 8.0).looking_to(Vec3::NEG_Z, Vec3::Y);
    let mut look = crate::CameraLook::default();
    look.aim(transform.rotation);
    app.world_mut().spawn((TerraCamera, transform, look));
    let eye = [0.5, f64::from(origin_y), 8.0];
    let ray = terra_world::trains::raycast(
        app.world().resource::<EditJournal>(),
        eye,
        [0.0, 0.0, -1.0],
        32.0,
    )
    .unwrap();
    assert!(ray.distance > 8.0 && ray.distance < 32.0);
    assert!(
        terra_world::trains::raycast(
            app.world().resource::<EditJournal>(),
            eye,
            [0.0, 0.0, -1.0],
            33.0
        )
        .is_none()
    );
    app.update();
    assert!(
        app.world().resource::<TargetWorld>().0.is_none(),
        "the distant wagon occludes construction behind it"
    );
    assert!(app.world().resource::<TargetBlock>().0.is_none());
    assert!(
        app.world().resource::<TargetTrain>().0.is_none(),
        "cargo/seat interaction keeps its normal reach"
    );
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .vehicles
        .clear();
    app.update();
    let hit = app.world().resource::<TargetBlock>().0.unwrap();
    assert_eq!(hit.position, floor);
    assert!(hit.distance > 8.0);
    app.world_mut().resource_mut::<super::super::ActiveItem>().0 =
        Some(terra_core::ItemKind::Block(3));
    app.update();
    assert!(
        app.world().resource::<TargetWorld>().0.is_none(),
        "ordinary block reach is unchanged"
    );
}
