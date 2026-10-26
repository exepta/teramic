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

pub(crate) fn target_creature(
    camera: Query<(&Transform, &crate::CameraLook), With<TerraCamera>>,
    bodies: Query<&Transform, With<crate::PlayerBody>>,
    creatures: Option<Res<terra_network::client::ReplicatedCreatures>>,
    block: Res<TargetBlock>,
    mut target: ResMut<TargetCreature>,
) {
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
    let reach = block.0.map_or(4.5, |b| b.distance.min(4.5));
    target.0 = creatures
        .creatures
        .values()
        .filter_map(|t| {
            let mut a = t.current.clone();
            a.position = t.position().to_array().map(f64::from);
            terra_world::creatures::ray_distance(&a, origin, direction, reach)
                .map(|distance| (distance, a.id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn target_block(
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
) {
    let Some((camera, look)) = camera.iter().next() else {
        target.0 = None;
        world_target.0 = None;
        return;
    };
    let origin = if perspective
        .as_deref()
        .is_some_and(|mode| mode.is_third_person())
    {
        bodies.iter().next().map_or(camera.translation, |body| {
            body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET
        })
    } else {
        camera.translation
    };
    let picker = if !input.pressed(terra_core::GameAction::Break)
        && active.as_ref().is_some_and(|item| {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::{BlockSection, MaterialId};
    use terra_world::{
        props::{PropId, PropKind, WorldProp},
        streaming::LoadedSection,
    };

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
