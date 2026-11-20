use crate::TerraCamera;
use bevy::prelude::*;
#[cfg(test)]
use terra_voxel::SectionPos;
use terra_voxel::blocks::BlockHit;
use terra_world::streaming::ChunkStore;
pub use terra_world::targeting::WorldTarget;
#[cfg(test)]
use terra_world::targeting::pick_world;
#[derive(Resource, Default, Debug)]
pub struct TargetBlock(pub Option<BlockHit>);

#[derive(Resource, Default, Debug)]
pub struct TargetWorld(pub Option<WorldTarget>);
#[derive(Resource, Default, Debug)]
pub struct TargetCreature(pub Option<u64>);
#[derive(Resource, Default, Debug)]
pub struct TargetTrain(pub Option<terra_world::trains::TrainHit>);

#[cfg(test)]
#[path = "train_target_tests.rs"]
mod train_tests;

/// Gameplay rays use the physical eye. Camera interpolation, stair smoothing,
/// walking sway and damage motion are presentation and must not change the
/// cell or face subsequently validated by the server.
pub(super) fn interaction_origin(
    camera: Vec3,
    body: Option<Vec3>,
    online: bool,
    mode: Option<crate::ControllerMode>,
    third_person: bool,
) -> Vec3 {
    if online || mode == Some(crate::ControllerMode::Walk) || third_person {
        body.map_or(camera, |body| {
            body + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET
        })
    } else {
        // Local fly cameras really are detached and have no authoritative body.
        camera
    }
}

pub(crate) fn target_creature(
    construction: Option<Res<crate::ConstructionCamera>>,
    camera: Query<(&Transform, &crate::CameraLook), With<TerraCamera>>,
    bodies: Query<&Transform, With<crate::PlayerBody>>,
    creatures: Option<Res<terra_network::client::ReplicatedCreatures>>,
    block: Res<TargetBlock>,
    train: Option<Res<TargetTrain>>,
    mut target: ResMut<TargetCreature>,
) {
    if construction.as_ref().is_some_and(|camera| camera.active) {
        return;
    }
    target.0 = None;
    let Some(creatures) = creatures else {
        return;
    };
    let Some((camera, look)) = camera.iter().next() else {
        return;
    };
    let origin = bodies
        .iter()
        .next()
        .map_or(camera.translation, |b| {
            b.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET
        })
        .to_array()
        .map(f64::from);
    let direction = (look.rotation() * Vec3::NEG_Z).to_array().map(f64::from);
    let reach = block.0.map_or(4.5, |b| b.distance.min(4.5)).min(
        train
            .as_ref()
            .and_then(|target| target.0)
            .map_or(4.5, |hit| hit.distance),
    );
    target.0 = creatures
        .creatures
        .values()
        .filter_map(|track| {
            let mut creature = track.current.clone();
            creature.position = track.position().to_array().map(f64::from);
            terra_world::creatures::ray_distance(&creature, origin, direction, reach)
                .map(|distance| (distance, creature.id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn target_block(
    construction: Option<Res<crate::ConstructionCamera>>,
    camera: Query<(&Transform, &crate::CameraLook), With<TerraCamera>>,
    chunks: Res<ChunkStore>,
    journal: Res<terra_world::edits::EditJournal>,
    properties: Res<terra_core::block_properties::BlockPropertiesCatalog>,
    mut target: ResMut<TargetBlock>,
    mut world_target: ResMut<TargetWorld>,
    time: Option<Res<Time>>,
    weather: Option<Res<terra_core::WorldWeather>>,
    perspective: Option<Res<crate::CameraPerspective>>,
    bodies: Query<&Transform, (With<crate::PlayerBody>, Without<TerraCamera>)>,
    active: Option<Res<super::ActiveItem>>,
    input: terra_core::GameplayInput,
    calendar: (
        Option<Res<terra_core::GameConfig>>,
        Option<Res<terra_core::WorldClock>>,
    ),
    mut train: Option<ResMut<TargetTrain>>,
    controller: (
        Option<Res<terra_network::client::ClientSession>>,
        Option<Res<crate::ControllerMode>>,
    ),
) {
    let (session, mode) = controller;
    if construction.as_ref().is_some_and(|camera| camera.active) {
        return;
    }
    if let Some(train) = &mut train {
        train.0 = None;
    }
    let Some((camera, look)) = camera.iter().next() else {
        target.0 = None;
        world_target.0 = None;
        return;
    };
    let origin = interaction_origin(
        camera.translation,
        bodies.iter().next().map(|body| body.translation),
        session
            .as_ref()
            .is_some_and(|session| session.ready && session.player.is_some()),
        mode.as_deref().copied(),
        perspective
            .as_deref()
            .is_some_and(|mode| mode.is_third_person()),
    );
    let rail_placement = !input.pressed(terra_core::GameAction::Break)
        && active
            .as_ref()
            .and_then(|item| item.0)
            .and_then(terra_core::ItemKind::block_material)
            .is_some_and(terra_core::rails::is_rail);
    let picker = if input.pressed(terra_core::GameAction::Break) {
        terra_world::targeting::pick_world_for_mining
    } else if rail_placement {
        terra_world::targeting::pick_world_for_rails
    } else if active
        .as_ref()
        .and_then(|item| item.0)
        .and_then(terra_core::ItemKind::block_material)
        .is_some()
    {
        terra_world::targeting::pick_world_for_placement
    } else if active.as_ref().is_some_and(|item| {
        matches!(item.0, Some(terra_core::ItemKind::Prop(kind)) if terra_core::TreeSpecies::from_sapling(kind).is_some())
    }) {
        terra_world::targeting::pick_world_for_saplings
    } else if active.as_ref().is_some_and(|item| {
        item.0.is_some_and(|item| match item {
            terra_core::ItemKind::Prop(_) | terra_core::ItemKind::Flower(_) => true,
            terra_core::ItemKind::Crop(seed) => seed.crop_kind().is_some(),
            _ => false,
        })
    }) {
        terra_world::targeting::pick_world_for_planting
    } else {
        terra_world::targeting::pick_world_with_context
    };
    (target.0, world_target.0) = picker(
        &chunks,
        &journal,
        &properties,
        origin.to_array().map(f64::from),
        (look.rotation() * Vec3::NEG_Z).to_array().map(f64::from),
        time.map_or(terra_world::props::PropPose::Rest, |t| {
            terra_world::props::PropPose::WeatherWind {
                time: t.elapsed_secs_wrapped(),
                frame: weather.as_ref().map_or([0.86, 0.51, 1.0, 1.0], |weather| {
                    terra_world::props::weather_wind_frame(weather.wind())
                }),
            }
        })
        .with_season(
            calendar.1.as_deref().copied().unwrap_or_default().season(
                calendar
                    .0
                    .as_deref()
                    .map_or_else(Default::default, |config| config.world.seasons),
            ),
        ),
    );
    if let Some(train) = &mut train {
        let origin = origin.to_array().map(f64::from);
        let direction = (look.rotation() * Vec3::NEG_Z).to_array().map(f64::from);
        let limit = if rail_placement {
            terra_core::rails::RAIL_BUILD_REACH
        } else {
            4.5
        };
        let reach = world_target.0.map_or(limit, |target| match target {
            WorldTarget::Block(hit) => hit.distance.min(limit),
            WorldTarget::Prop(hit) => hit.distance.min(limit),
        });
        let hit = session.as_ref().map_or_else(
            || terra_world::trains::raycast(&journal, origin, direction, reach),
            |session| {
                terra_world::trains::raycast_views(
                    &session.trains.vehicles,
                    origin,
                    direction,
                    reach,
                )
            },
        );
        if let Some(hit) = hit {
            // A replicated vehicle never lets a ray see through unknown terrain.
            let loaded = (0..=(hit.distance * 16.0).ceil() as usize).all(|step| {
                let distance = (step as f64 / 16.0).min(hit.distance);
                chunks
                    .block_at(std::array::from_fn(|axis| {
                        (origin[axis] + direction[axis] * distance).floor() as i64
                    }))
                    .is_some()
            });
            if loaded {
                // Extended construction rays still cannot interact with a
                // distant cab or cargo slot through the normal-use controls.
                train.0 = (hit.distance <= 4.5).then_some(hit);
                target.0 = None;
                world_target.0 = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::{BlockSection, MaterialId};
    use terra_world::{
        props::{PropId, PropKind, WorldProp},
        streaming::LoadedSection,
    };

    #[test]
    fn building_and_mining_keep_the_authoritative_eye_at_bobbed_block_edges() {
        let owner = SectionPos::default();
        let mut data = BlockSection::empty(owner).unwrap();
        for x in [3, 4] {
            data.set_block([x, 3, 5], 3).unwrap();
        }
        let mut chunks = ChunkStore::default();
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
        let eye = Vec3::new(3.99, 3.5, 9.5);
        let mut session = terra_network::client::ClientSession::default();
        session.ready = true;
        session.player = Some(terra_core::PlayerId(1));
        let mut app = App::new();
        app.insert_resource(chunks)
            .init_resource::<terra_world::edits::EditJournal>()
            .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<TargetBlock>()
            .init_resource::<TargetWorld>()
            .insert_resource(session)
            .insert_resource(super::super::ActiveItem(Some(terra_core::ItemKind::Block(
                3,
            ))))
            .add_systems(Update, target_block);
        app.world_mut().spawn((
            crate::PlayerBody,
            Transform::from_translation(
                eye - Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET,
            ),
        ));
        app.world_mut().spawn((
            TerraCamera,
            // Normal gait and interpolation can cross this voxel edge visually.
            Transform::from_translation(eye + Vec3::new(0.04, 0.04, 0.)),
            crate::CameraLook::default(),
        ));
        for breaking in [false, true] {
            if breaking {
                app.world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .press(MouseButton::Left);
            }
            app.update();
            let hit = app.world().resource::<TargetBlock>().0.unwrap();
            assert_eq!(hit.position, [3, 3, 5]);
            assert_eq!(hit.normal, [0, 0, 1]);
            let chunks = app.world().resource::<ChunkStore>();
            let journal = app.world().resource::<terra_world::edits::EditJournal>();
            let properties = app
                .world()
                .resource::<terra_core::block_properties::BlockPropertiesCatalog>();
            let authority = terra_world::targeting::pick_world_for_mining(
                chunks,
                journal,
                properties,
                eye.to_array().map(f64::from),
                [0., 0., -1.],
                terra_world::props::PropPose::Rest,
            )
            .0
            .unwrap();
            assert_eq!(hit, authority);
            assert_eq!(
                terra_world::targeting::item_placement_with_state(chunks, journal, hit, 3),
                Some(([3, 3, 6], 0)),
            );
        }
        // The shared eye still selects the actual intervening wall.
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block([3, 3, 7], 3)
            .unwrap();
        app.update();
        assert_eq!(
            app.world().resource::<TargetBlock>().0.unwrap().position,
            [3, 3, 7]
        );
        // A default/disconnected session resource does not attach a fly camera.
        {
            let mut session = app
                .world_mut()
                .resource_mut::<terra_network::client::ClientSession>();
            session.ready = false;
            session.player = None;
        }
        app.insert_resource(crate::ControllerMode::Fly);
        app.update();
        assert_eq!(
            app.world().resource::<TargetBlock>().0.unwrap().position,
            [4, 3, 5]
        );
    }

    #[test]
    fn detached_local_flight_keeps_its_camera_eye() {
        let camera = Vec3::new(10., 30., 40.);
        let body = Vec3::new(1., 2., 3.);
        assert_eq!(
            interaction_origin(
                camera,
                Some(body),
                false,
                Some(crate::ControllerMode::Fly),
                false
            ),
            camera,
        );
        for (online, mode, third_person) in [
            (true, Some(crate::ControllerMode::Fly), false),
            (false, Some(crate::ControllerMode::Walk), false),
            (false, None, true),
        ] {
            assert_eq!(
                interaction_origin(camera, Some(body), online, mode, third_person),
                body + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET,
            );
        }
    }

    fn scene() -> ChunkStore {
        let mut chunks = ChunkStore::default();
        for y in 0..=1 {
            let position = SectionPos::new(-1, y, -1);
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
        let owner = SectionPos::new(-1, 0, -1);
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block([12, 15, 15], MaterialId::Farmland as u16)
            .unwrap();
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .props
            .push(WorldProp {
                id: PropId(1),
                kind: PropKind::Bush,
                position: [-3.5, 16.0, -0.5],
                rotation: 0.0,
                scale: 1.0,
                variant: 0,
                section: owner,
                removed: false,
            });
        chunks
    }

    #[test]
    fn props_on_negative_coordinates_and_vertical_section_boundaries_are_selectable() {
        let chunks = scene();
        let (_, target) = pick_world(&chunks, [-0.5, 16.3, -0.5], [-1.0, 0.0, 0.0]);
        assert!(matches!(target, Some(WorldTarget::Prop(hit)) if hit.prop.id == PropId(1)));
        assert_eq!(
            target,
            terra_world::targeting::pick_world_density(
                &chunks,
                [-0.5, 16.3, -0.5],
                [-1.0, 0.0, 0.0],
                0.0
            )
            .1,
            "Legacy density values must not hide selectable world props"
        );
    }

    #[test]
    fn solid_blocks_and_unloaded_sections_occlude_prop_targets() {
        let mut chunks = scene();
        chunks
            .loaded
            .get_mut(&SectionPos::new(-1, 1, -1))
            .unwrap()
            .data
            .set_block([14, 0, 15], MaterialId::Stone as u16)
            .unwrap();
        let (_, target) = pick_world(&chunks, [-0.5, 16.3, -0.5], [-1.0, 0.0, 0.0]);
        assert!(matches!(target, Some(WorldTarget::Block(hit)) if hit.position == [-2, 16, -1]));
        chunks.loaded.remove(&SectionPos::new(-1, 1, -1));
        assert_eq!(
            pick_world(&chunks, [-0.5, 16.3, -0.5], [-1.0, 0.0, 0.0]),
            (None, None)
        );
    }

    #[test]
    fn seeds_in_hand_do_not_hide_crops_from_the_break_action() {
        let mut chunks = scene();
        let crop = &mut chunks
            .loaded
            .get_mut(&SectionPos::new(-1, 0, -1))
            .unwrap()
            .props[0];
        crop.kind = PropKind::Wheat;
        crop.variant = 7;
        let origin = (1..20)
            .map(|step| [-0.5, 16.0 + f64::from(step) * 0.1, -0.5])
            .find(|origin| {
                matches!(pick_world(&chunks, *origin, [-1.0, 0.0, 0.0]).1,
                Some(WorldTarget::Prop(hit)) if hit.prop.id == PropId(1))
            })
            .expect("ripe wheat must be selectable");
        let transform = Transform::from_translation(Vec3::from_array(origin.map(|v| v as f32)))
            .looking_to(Vec3::NEG_X, Vec3::Y);
        let mut look = crate::CameraLook::default();
        look.aim(transform.rotation);
        let mut app = App::new();
        app.insert_resource(chunks)
            .init_resource::<terra_world::edits::EditJournal>()
            .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<TargetBlock>()
            .init_resource::<TargetWorld>()
            .insert_resource(super::super::ActiveItem(Some(terra_core::ItemKind::Crop(
                terra_core::CropKind::Wheat.seed(),
            ))))
            .add_systems(Update, target_block);
        app.world_mut().spawn((TerraCamera, transform, look));
        app.update();
        assert!(app.world().resource::<TargetWorld>().0.is_none());
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(matches!(app.world().resource::<TargetWorld>().0,
            Some(WorldTarget::Prop(hit)) if hit.prop.id == PropId(1)));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert!(app.world().resource::<TargetWorld>().0.is_none());
    }
}

#[cfg(test)]
mod water_break_tests {
    use super::*;
    use terra_voxel::BlockSection;
    use terra_world::streaming::LoadedSection;

    #[test]
    fn pressing_break_switches_from_source_surface_to_underwater_ground() {
        let owner = SectionPos::default();
        let mut data = BlockSection::empty(owner).unwrap();
        data.set_block([4, 3, 4], 3).unwrap();
        data.set_block([4, 4, 4], 27).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let mut app = App::new();
        app.insert_resource(chunks)
            .init_resource::<terra_world::edits::EditJournal>()
            .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<TargetBlock>()
            .init_resource::<TargetWorld>()
            .add_systems(Update, target_block);
        let transform = Transform::from_xyz(4.5, 7.5, 4.5).looking_to(Vec3::NEG_Y, Vec3::Z);
        let mut look = crate::CameraLook::default();
        look.aim(transform.rotation);
        app.world_mut().spawn((TerraCamera, transform, look));
        app.update();
        assert_eq!(
            app.world().resource::<TargetBlock>().0.unwrap().position,
            [4, 4, 4]
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(
            app.world().resource::<TargetBlock>().0.unwrap().position,
            [4, 3, 4]
        );
    }
    #[test]
    fn construction_in_hand_looks_through_the_submerged_eye_cell_and_vessels_keep_water_targeting()
    {
        let owner = SectionPos::default();
        let mut data = BlockSection::empty(owner).unwrap();
        data.set_block([4, 3, 4], 3).unwrap();
        for y in 4..=6 {
            data.set_block([4, y, 4], 27).unwrap();
        }
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let mut app = App::new();
        app.insert_resource(chunks)
            .init_resource::<terra_world::edits::EditJournal>()
            .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<TargetBlock>()
            .init_resource::<TargetWorld>()
            .insert_resource(super::super::ActiveItem(Some(terra_core::ItemKind::Block(
                3,
            ))))
            .add_systems(Update, target_block);
        let transform = Transform::from_xyz(4.5, 6.5, 4.5).looking_to(Vec3::NEG_Y, Vec3::Z);
        let mut look = crate::CameraLook::default();
        look.aim(transform.rotation);
        app.world_mut().spawn((TerraCamera, transform, look));
        app.update();
        let hit = app.world().resource::<TargetBlock>().0.unwrap();
        assert_eq!(hit.position, [4, 3, 4]);
        assert_eq!(
            terra_world::targeting::block_placement(app.world().resource::<ChunkStore>(), hit),
            Some(([4, 4, 4], 27))
        );
        for tool in [
            terra_core::ToolKind::Bucket,
            terra_core::ToolKind::IronWateringCan,
        ] {
            app.world_mut().resource_mut::<super::super::ActiveItem>().0 =
                Some(terra_core::ItemKind::Tool(tool));
            app.update();
            assert_eq!(
                app.world().resource::<TargetBlock>().0.unwrap().position,
                [4, 6, 4]
            );
        }
    }
}
