use super::{TargetBlock, TargetWorld, WorldTarget};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use terra_core::{GameAction as Action, GameMode, GameplayInput, ItemKind};
use terra_network::{
    client::{MiningRequest, PropPlaceRequest},
    protocol::session::{MiningIntent, MiningTarget},
};
use terra_voxel::MaterialId;
use terra_world::streaming::{BlockEditRequest, ChunkStore, PropRemoveRequest};

#[derive(SystemParam)]
pub(crate) struct EditRequests<'w> {
    creatures: Option<ResMut<'w, terra_network::client::CreatureActions>>,
    blocks: MessageWriter<'w, BlockEditRequest>,
    props: MessageWriter<'w, PropRemoveRequest>,
    mining: MessageWriter<'w, MiningRequest>,
    planted: MessageWriter<'w, PropPlaceRequest>,
}

#[derive(Resource, Default, Debug)]
pub struct MiningVisual {
    pub target: Option<MiningTarget>,
    pub progress: f32,
    active: Option<MiningTarget>,
    heartbeat: f32,
    repeat: f32,
    held_elapsed: f32,
}

#[derive(Resource, Default, Debug)]
pub struct PlacementPreview {
    pub position: Option<[i64; 3]>,
    pub material: u16,
    pub valid: bool,
}

#[derive(Resource, Default, Debug)]
pub struct InteractionMotion {
    pub swinging: bool,
    pub placing: bool,
    pub impulse: u64,
}

#[derive(SystemParam)]
pub(crate) struct EditPresentation<'w> {
    mining: ResMut<'w, MiningVisual>,
    preview: ResMut<'w, PlacementPreview>,
    motion: ResMut<'w, InteractionMotion>,
    time: Option<Res<'w, Time>>,
}

#[derive(SystemParam)]
pub(crate) struct EditTarget<'w, 's> {
    creature: Option<Res<'w, super::TargetCreature>>,
    world: Res<'w, TargetWorld>,
    block: Res<'w, TargetBlock>,
    blocked: Res<'w, crate::PlayerInputBlock>,
    bodies: Query<'w, 's, &'static Transform, With<crate::PlayerBody>>,
    session: Option<Res<'w, terra_network::client::ClientSession>>,
    remote: Option<Res<'w, terra_network::client::ReplicatedPlayers>>,
    active: Option<Res<'w, super::ActiveItem>>,
    journal: Option<Res<'w, terra_world::edits::EditJournal>>,
    camera:
        Query<'w, 's, (&'static Transform, &'static crate::CameraLook), With<crate::TerraCamera>>,
    perspective: Option<Res<'w, crate::CameraPerspective>>,
}
pub(crate) fn edit_target_block(
    keys: GameplayInput,
    target: EditTarget,
    chunks: Res<ChunkStore>,
    mut requests: EditRequests,
    input: Res<crate::CameraInputState>,
    palette: Res<super::BlockPalette>,
    mut presentation: EditPresentation,
) {
    let delta = presentation
        .time
        .as_ref()
        .map_or(0.0, |t| t.delta_secs())
        .min(0.1);
    presentation.mining.repeat = (presentation.mining.repeat - delta).max(0.0);
    presentation.mining.heartbeat += delta;
    presentation.preview.position = None;
    presentation.preview.valid = false;
    presentation.motion.swinging = false;
    let blocked = !input.captured
        || input.just_captured
        || target.blocked.0
        || target.session.as_ref().is_some_and(|s| {
            !s.ready || !s.allow_build || s.error.is_some() || s.game_mode == GameMode::Spectator
        });
    let creative = target
        .session
        .as_ref()
        .is_none_or(|s| s.game_mode == GameMode::Creative);
    let creature = target.creature.as_ref().and_then(|t| t.0);
    let selected_target = if blocked || creature.is_some() || !keys.pressed(Action::Break) {
        None
    } else {
        target.world.0.and_then(|hit| match hit {
            WorldTarget::Prop(hit) => Some(MiningTarget::Prop {
                section: hit.prop.section,
                id: hit.prop.id,
            }),
            WorldTarget::Block(hit) => chunks
                .block_at(hit.position)
                .filter(|m| *m != 0 && *m != MaterialId::FluidWater as u16)
                .map(|expected| MiningTarget::Block {
                    position: hit.position,
                    expected,
                }),
        })
    };
    // The server owns survival timing; release, focus loss and UI immediately
    // cancel the intent. A bounded heartbeat also covers silent disconnects.
    let active = if creative { None } else { selected_target };
    if active != presentation.mining.active {
        presentation.mining.held_elapsed = 0.0;
    }
    if active.is_some() {
        presentation.mining.held_elapsed += delta;
    }
    if active != presentation.mining.active
        || (active.is_some() && presentation.mining.heartbeat >= 0.1)
    {
        requests.mining.write(MiningRequest(MiningIntent {
            target: active,
            look: target.session.as_ref().map_or([0.0; 2], |s| s.look),
        }));
        presentation.mining.active = active;
        presentation.mining.heartbeat = 0.0;
    }
    presentation.mining.target = active;
    presentation.mining.progress = target
        .session
        .as_ref()
        .and_then(|s| s.mining)
        .filter(|m| Some(m.target) == active)
        .map_or(0.0, |m| {
            m.progress
                .clamp(0.0, 1.0)
                .min(presentation.mining.held_elapsed / m.duration.max(0.01))
        });
    if blocked {
        return;
    }
    presentation.motion.swinging = keys.pressed(Action::Break)
        && (selected_target.is_some() || creature.is_some() || target.world.0.is_none());
    if let Some(id) = creature {
        if (keys.just_pressed(Action::Break) || keys.just_pressed(Action::Place))
            && let Some(queue) = requests.creatures.as_mut()
            && queue.0.len() < 2
        {
            queue.0.push_back((id, !keys.just_pressed(Action::Break)));
        }
        return;
    }
    if selected_target.is_some() {
        presentation.motion.placing = false;
    }
    if selected_target.is_some()
        && creative
        && (keys.just_pressed(Action::Break) || presentation.mining.repeat == 0.0)
    {
        presentation.mining.repeat = 0.16;
        presentation.motion.placing = false;
        presentation.motion.impulse = presentation.motion.impulse.wrapping_add(1);
        match target.world.0 {
            Some(WorldTarget::Prop(hit)) => {
                requests.props.write(PropRemoveRequest {
                    id: hit.prop.id,
                    section: hit.prop.section,
                });
            }
            Some(WorldTarget::Block(hit)) => {
                if let Some(material) = chunks.block_at(hit.position) {
                    requests.blocks.write(BlockEditRequest {
                        position: hit.position,
                        expected_material: material,
                        material: MaterialId::Air as u16,
                    });
                }
            }
            None => {}
        }
    }
    // Opening a hinged object consumes placement, while camera and movement
    // stay active. Shift deliberately retains ordinary adjacent placement.
    if (keys.just_pressed(Action::Place) || keys.just_pressed(Action::Use))
        && !keys.keys.pressed(KeyCode::ShiftLeft)
        && !keys.keys.pressed(KeyCode::ShiftRight)
        && target.world.0.is_some_and(|target| match target {
            WorldTarget::Block(hit) => chunks.block_at(hit.position).is_some_and(|m| {
                terra_world::objects::is_opening(m) || terra_core::beds::is_bed(m)
            }),
            WorldTarget::Prop(_) => false,
        })
    {
        return;
    }
    let selected = target
        .active
        .as_ref()
        .and_then(|s| s.0)
        .or_else(|| palette.selected().map(|b| ItemKind::Block(b.id)));
    // Edible props use the same action as placement; Shift keeps planting available.
    if selected.is_some_and(|item| terra_core::food(item).is_some()) && keys.consume_pressed() {
        return;
    }
    if let Some(ItemKind::Prop(kind)) = selected
        && kind != terra_core::ResourcePropKind::Seagrass
    {
        if let Some(hit) = target.block.0
            && hit.normal == [0, 1, 0]
        {
            let has_item = creative
                || target
                    .session
                    .as_ref()
                    .is_some_and(|s| s.inventory.count(ItemKind::Prop(kind)) > 0);
            if has_item
                && matches!(chunks.block_at(hit.adjacent()), Some(0 | 27))
                && chunks
                    .block_at(hit.position)
                    .and_then(MaterialId::from_raw)
                    .is_some_and(|m| {
                        if terra_world::props::PropKind::from(kind).aquatic() {
                            m.supports_water_plants()
                        } else {
                            m.is_natural_ground()
                        }
                    })
                && keys.just_pressed(Action::Place)
            {
                requests.planted.write(PropPlaceRequest {
                    support: hit.position,
                    kind,
                });
            }
        }
        return;
    }
    if let Some(hit) = target.block.0
        && let Some(selected_item) = selected
        && let Some(selected) = selected_item.block_material()
        && let Some((position, expected)) = target.journal.as_ref().map_or_else(
            || terra_world::targeting::item_placement(&chunks, hit, selected),
            |journal| {
                terra_world::targeting::item_placement_with_state(&chunks, journal, hit, selected)
            },
        )
    {
        let has_item = creative
            || target
                .session
                .as_ref()
                .is_some_and(|s| s.inventory.count(selected_item) > 0);
        let bounds = target.camera.iter().next().and_then(|(camera, look)| {
            let origin = if target
                .perspective
                .as_deref()
                .is_some_and(|p| p.is_third_person())
            {
                target
                    .bodies
                    .iter()
                    .next()
                    .map_or(camera.translation, |body| {
                        body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET
                    })
            } else {
                camera.translation
            };
            target.journal.as_ref().and_then(|journal| {
                terra_world::targeting::placement_bounds(
                    &chunks,
                    journal,
                    hit,
                    selected,
                    (
                        origin.to_array().map(f64::from),
                        (look.rotation() * Vec3::NEG_Z).to_array().map(f64::from),
                    ),
                )
            })
        });
        let overlaps = |center: Vec3| {
            bounds.as_ref().map_or_else(
                || crate::camera::block_overlaps_body(position, center),
                |bounds| {
                    let half = Vec3::new(
                        terra_physics::character::PLAYER_RADIUS,
                        terra_physics::character::PLAYER_HALF_HEIGHT,
                        terra_physics::character::PLAYER_RADIUS,
                    );
                    bounds.intersects(
                        (center - half).to_array().map(f64::from),
                        (center + half).to_array().map(f64::from),
                    )
                },
            )
        };
        let overlaps_player = target.bodies.iter().any(|body| overlaps(body.translation))
            || target.remote.as_ref().is_some_and(|players| {
                players
                    .players
                    .values()
                    .any(|track| overlaps(track.position()))
            });
        let valid = has_item
            && (selected != expected || terra_core::construction::is_slab(selected))
            && (!MaterialId::from_raw(selected).is_some_and(MaterialId::is_solid)
                || !overlaps_player);
        presentation.preview.position = Some(position);
        presentation.preview.material = selected;
        presentation.preview.valid = valid;
        if valid && keys.just_pressed(Action::Place) {
            presentation.motion.placing = true;
            presentation.motion.impulse = presentation.motion.impulse.wrapping_add(1);
            requests.blocks.write(BlockEditRequest {
                position,
                expected_material: expected,
                material: selected,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::Messages;
    use terra_voxel::{BlockSection, SectionPos, blocks::BlockHit};
    use terra_world::{
        props::{PropHit, PropId, PropKind, WorldProp},
        streaming::LoadedSection,
    };

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ChunkStore>()
            .init_resource::<crate::PlayerInputBlock>()
            .init_resource::<super::super::BlockPalette>()
            .init_resource::<MiningVisual>()
            .init_resource::<PlacementPreview>()
            .init_resource::<InteractionMotion>()
            .insert_resource(crate::CameraInputState {
                captured: true,
                ..default()
            })
            .add_message::<BlockEditRequest>()
            .add_message::<PropRemoveRequest>()
            .add_message::<MiningRequest>()
            .add_message::<PropPlaceRequest>()
            .insert_resource(TargetBlock(Some(BlockHit {
                position: [1, 0, 1],
                normal: [0, 1, 0],
                distance: 2.0,
            })))
            .insert_resource(TargetWorld(Some(WorldTarget::Prop(PropHit {
                prop: WorldProp {
                    id: PropId(7),
                    kind: PropKind::Grass,
                    position: [1.5, 1.0, 1.5],
                    rotation: 0.0,
                    scale: 1.0,
                    variant: 0,
                    section: SectionPos::default(),
                    removed: false,
                },
                distance: 1.5,
            }))))
            .add_systems(Update, edit_target_block);
        let position = SectionPos::default();
        let mut data = BlockSection::empty(position).unwrap();
        data.set_block([1, 0, 1], MaterialId::Grass as u16).unwrap();
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        app
    }

    #[test]
    fn left_click_targets_only_the_selected_prop() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        let requests: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<PropRemoveRequest>>()
            .drain()
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].id, PropId(7));
        assert!(
            app.world()
                .resource::<Messages<BlockEditRequest>>()
                .is_empty()
        );
    }

    #[test]
    fn fluid_water_cannot_be_mined_in_either_game_mode() {
        for mode in [GameMode::Creative, GameMode::Survival] {
            let mut app = app();
            session(&mut app, mode, false);
            let hit = app.world().resource::<TargetBlock>().0.unwrap();
            app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Block(hit));
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block([1, 0, 1], MaterialId::FluidWater as u16)
                .unwrap();
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            app.update();
            no_direct_removal(&app);
            assert!(mining_requests(&mut app).is_empty());
            assert!(app.world().resource::<MiningVisual>().target.is_none());
            assert!(!app.world().resource::<InteractionMotion>().swinging);
        }
    }

    #[test]
    fn block_placement_remains_available_when_a_plant_is_in_front() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        let requests: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<BlockEditRequest>>()
            .drain()
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].position, [1, 1, 1]);
        assert_eq!(requests[0].material, MaterialId::Stone as u16);
        assert!(
            app.world()
                .resource::<Messages<PropRemoveRequest>>()
                .is_empty()
        );
    }

    #[test]
    fn place_eats_edible_props_but_shift_place_can_still_plant_them() {
        for shift in [None, Some(KeyCode::ShiftLeft), Some(KeyCode::ShiftRight)] {
            let mut app = app();
            app.insert_resource(super::super::ActiveItem(Some(ItemKind::Prop(
                terra_core::ResourcePropKind::Mushroom,
            ))));
            {
                let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                keys.press(KeyCode::KeyB);
                if let Some(shift) = shift {
                    keys.press(shift);
                }
            }
            app.update();
            let requests: Vec<_> = app
                .world_mut()
                .resource_mut::<Messages<PropPlaceRequest>>()
                .drain()
                .collect();
            assert_eq!(requests.len(), usize::from(shift.is_some()));
            assert!(
                app.world()
                    .resource::<Messages<BlockEditRequest>>()
                    .is_empty()
            );
        }
    }

    #[test]
    fn placing_a_block_replaces_a_source_instead_of_stacking_on_it() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([1, 0, 1], MaterialId::FluidWater as u16)
            .unwrap();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        let requests: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<BlockEditRequest>>()
            .drain()
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].position, [1, 0, 1]);
        assert_eq!(requests[0].expected_material, MaterialId::FluidWater as u16);
        assert_eq!(requests[0].material, MaterialId::Stone as u16);
    }

    #[test]
    fn opening_a_door_or_window_never_also_places_the_held_block() {
        for material in 65..=70 {
            for shift in [false, true] {
                let mut app = app();
                let hit = app.world().resource::<TargetBlock>().0.unwrap();
                app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Block(hit));
                app.world_mut()
                    .resource_mut::<ChunkStore>()
                    .loaded
                    .get_mut(&SectionPos::default())
                    .unwrap()
                    .data
                    .set_block([1, 0, 1], material)
                    .unwrap();
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::KeyB);
                if shift {
                    app.world_mut()
                        .resource_mut::<ButtonInput<KeyCode>>()
                        .press(KeyCode::ShiftLeft);
                }
                app.update();
                assert_eq!(
                    app.world_mut()
                        .resource_mut::<Messages<BlockEditRequest>>()
                        .drain()
                        .count(),
                    usize::from(shift)
                );
                assert!(!app.world().resource::<crate::PlayerInputBlock>().0);
                assert!(app.world().resource::<crate::CameraInputState>().captured);
            }
        }
    }

    #[test]
    fn menu_blocks_mining_and_placement_even_when_capture_is_still_enabled() {
        let mut app = app();
        app.world_mut().resource_mut::<crate::PlayerInputBlock>().0 = true;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        assert!(
            app.world()
                .resource::<Messages<BlockEditRequest>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<PropRemoveRequest>>()
                .is_empty()
        );
    }

    #[test]
    fn placement_inside_player_body_produces_no_edit_request() {
        let mut app = app();
        app.world_mut()
            .spawn((crate::PlayerBody, Transform::from_xyz(1.5, 1.9, 1.5)));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        assert!(
            app.world()
                .resource::<Messages<BlockEditRequest>>()
                .is_empty()
        );
    }

    fn session(app: &mut App, mode: GameMode, flight: bool) {
        let mut session = terra_network::client::ClientSession::default();
        session.ready = true;
        session.allow_build = true;
        session.allow_flight = flight;
        session.game_mode = mode;
        app.insert_resource(session);
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(50));
        app.insert_resource(time);
    }

    #[test]
    fn selected_harvested_plant_emits_only_an_authoritative_place_request() {
        let mut app = app();
        session(&mut app, GameMode::Survival, false);
        let item = ItemKind::Prop(terra_core::ResourcePropKind::Flower);
        app.insert_resource(super::super::ActiveItem(Some(item)));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        assert!(
            app.world()
                .resource::<Messages<PropPlaceRequest>>()
                .is_empty()
        );
        app.world_mut()
            .resource_mut::<terra_network::client::ClientSession>()
            .inventory
            .insert(item, 1);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        let requests: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<PropPlaceRequest>>()
            .drain()
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].support, [1, 0, 1]);
        assert_eq!(requests[0].kind, terra_core::ResourcePropKind::Flower);
        assert!(
            app.world()
                .resource::<Messages<BlockEditRequest>>()
                .is_empty()
        );
        assert_eq!(
            app.world()
                .resource::<terra_network::client::ClientSession>()
                .inventory
                .count(item),
            1
        );
    }

    #[test]
    fn legacy_seagrass_item_places_an_individual_waterlogged_block_segment() {
        let mut app = app();
        session(&mut app, GameMode::Survival, false);
        let item = ItemKind::Prop(terra_core::ResourcePropKind::Seagrass);
        app.insert_resource(super::super::ActiveItem(Some(item)));
        app.world_mut()
            .resource_mut::<terra_network::client::ClientSession>()
            .inventory
            .insert(item, 1);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([1, 1, 1], MaterialId::FluidWater as u16)
            .unwrap();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        let requests: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<BlockEditRequest>>()
            .drain()
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].position, [1, 1, 1]);
        assert_eq!(requests[0].expected_material, 27);
        assert_eq!(requests[0].material, 50);
        assert!(
            app.world()
                .resource::<Messages<PropPlaceRequest>>()
                .is_empty()
        );
    }

    fn mining_requests(app: &mut App) -> Vec<MiningIntent> {
        app.world_mut()
            .resource_mut::<Messages<MiningRequest>>()
            .drain()
            .map(|request| request.0)
            .collect()
    }

    fn no_direct_removal(app: &App) {
        assert!(
            app.world()
                .resource::<Messages<BlockEditRequest>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<PropRemoveRequest>>()
                .is_empty()
        );
    }

    #[test]
    fn explicit_creative_mode_removes_blocks_and_props_immediately_without_flight() {
        for prop in [false, true] {
            let mut app = app();
            session(&mut app, GameMode::Creative, false);
            if !prop {
                let block = app.world().resource::<TargetBlock>().0.unwrap();
                app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Block(block));
            }
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            app.update();
            assert!(mining_requests(&mut app).is_empty());
            assert_eq!(
                app.world().resource::<Messages<PropRemoveRequest>>().len(),
                usize::from(prop)
            );
            assert_eq!(
                app.world().resource::<Messages<BlockEditRequest>>().len(),
                usize::from(!prop)
            );
            assert_eq!(app.world().resource::<InteractionMotion>().impulse, 1);
            assert!(app.world().resource::<MiningVisual>().target.is_none());
        }
    }

    #[test]
    fn survival_uses_bounded_heartbeats_and_release_cancels_even_with_flight_permission() {
        for flight in [false, true] {
            let mut app = app();
            session(&mut app, GameMode::Survival, flight);
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            app.update();
            let first = mining_requests(&mut app);
            assert_eq!(first.len(), 1);
            assert!(matches!(
                first[0].target,
                Some(MiningTarget::Prop { id: PropId(7), .. })
            ));
            no_direct_removal(&app);
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .clear();
            app.update();
            assert!(mining_requests(&mut app).is_empty());
            app.update();
            assert_eq!(mining_requests(&mut app).len(), 1);
            no_direct_removal(&app);
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(MouseButton::Left);
            app.update();
            let canceled = mining_requests(&mut app);
            assert_eq!(canceled.len(), 1);
            assert!(canceled[0].target.is_none());
            assert!(app.world().resource::<MiningVisual>().target.is_none());
            assert!(!app.world().resource::<InteractionMotion>().swinging);
            app.update();
            assert!(mining_requests(&mut app).is_empty());
        }
    }

    #[test]
    fn menu_capture_loss_and_permission_loss_cancel_an_active_survival_hold() {
        for reason in 0..4 {
            let mut app = app();
            session(&mut app, GameMode::Survival, false);
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            app.update();
            assert_eq!(mining_requests(&mut app).len(), 1);
            match reason {
                0 => app.world_mut().resource_mut::<crate::PlayerInputBlock>().0 = true,
                1 => {
                    app.world_mut()
                        .resource_mut::<crate::CameraInputState>()
                        .captured = false
                }
                2 => {
                    app.world_mut()
                        .resource_mut::<terra_network::client::ClientSession>()
                        .allow_build = false
                }
                _ => {
                    app.world_mut()
                        .resource_mut::<terra_network::client::ClientSession>()
                        .ready = false
                }
            }
            app.update();
            let requests = mining_requests(&mut app);
            assert_eq!(requests.len(), 1);
            assert!(requests[0].target.is_none());
            no_direct_removal(&app);
            assert!(
                app.world()
                    .resource::<PlacementPreview>()
                    .position
                    .is_none()
            );
        }
    }

    #[test]
    fn switching_targets_does_not_reuse_previous_authoritative_progress() {
        let mut app = app();
        session(&mut app, GameMode::Survival, false);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        let old = mining_requests(&mut app)[0].target.unwrap();
        app.world_mut()
            .resource_mut::<terra_network::client::ClientSession>()
            .mining = Some(terra_network::protocol::session::MiningProgress {
            target: old,
            progress: 0.8,
            duration: old.duration(),
        });
        app.update();
        assert_eq!(app.world().resource::<MiningVisual>().progress, 0.4);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<MiningVisual>().progress, 0.8);
        mining_requests(&mut app);
        let block = app.world().resource::<TargetBlock>().0.unwrap();
        app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Block(block));
        app.update();
        let requests = mining_requests(&mut app);
        assert_eq!(requests.len(), 1);
        assert!(matches!(
            requests[0].target,
            Some(MiningTarget::Block {
                position: [1, 0, 1],
                expected: 1
            })
        ));
        assert_eq!(app.world().resource::<MiningVisual>().progress, 0.0);
        no_direct_removal(&app);
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at([1, 0, 1]),
            Some(1)
        );
    }

    #[test]
    fn tool_animation_follows_server_speed_without_hiding_partial_break_progress() {
        let mut app = app();
        session(&mut app, GameMode::Survival, false);
        app.insert_resource(super::super::ActiveItem(Some(ItemKind::Tool(
            terra_core::ToolKind::StoneShovel,
        ))));
        let hit = app.world().resource::<TargetBlock>().0.unwrap();
        app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Block(hit));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        let target = mining_requests(&mut app)[0].target.unwrap();
        app.world_mut()
            .resource_mut::<terra_network::client::ClientSession>()
            .mining = Some(terra_network::protocol::session::MiningProgress {
            target,
            progress: 0.7,
            duration: target.duration() / 4.0,
        });
        app.update();
        assert!(app.world().resource::<MiningVisual>().progress > 0.5);
        assert!(app.world().resource::<MiningVisual>().progress <= 0.7);
    }

    #[test]
    fn releasing_and_reholding_same_target_caps_delayed_server_progress() {
        let mut app = app();
        session(&mut app, GameMode::Survival, false);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        let target = mining_requests(&mut app)[0].target.unwrap();
        app.world_mut()
            .resource_mut::<terra_network::client::ClientSession>()
            .mining = Some(terra_network::protocol::session::MiningProgress {
            target,
            progress: 0.9,
            duration: target.duration(),
        });
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(app.world().resource::<MiningVisual>().progress, 0.9);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert_eq!(app.world().resource::<MiningVisual>().progress, 0.0);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(app.world().resource::<MiningVisual>().progress, 0.2);
        assert_eq!(app.world().resource::<MiningVisual>().target, Some(target));
        no_direct_removal(&app);
    }

    #[test]
    fn survival_preview_uses_stock_but_does_not_consume_it_before_confirmation() {
        let mut app = app();
        session(&mut app, GameMode::Survival, false);
        app.update();
        assert_eq!(
            app.world().resource::<PlacementPreview>().position,
            Some([1, 1, 1])
        );
        assert!(!app.world().resource::<PlacementPreview>().valid);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        no_direct_removal(&app);
        let item = ItemKind::Block(3);
        app.world_mut()
            .resource_mut::<terra_network::client::ClientSession>()
            .inventory
            .insert(item, 1);
        app.update();
        assert!(app.world().resource::<PlacementPreview>().valid);
        let requests: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<BlockEditRequest>>()
            .drain()
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].material, 3);
        assert_eq!(
            app.world()
                .resource::<terra_network::client::ClientSession>()
                .inventory
                .count(item),
            1
        );
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at([1, 1, 1]),
            Some(0)
        );
    }

    #[test]
    fn placement_preview_respects_remote_player_bounds() {
        use terra_network::{
            client::{RemoteTrack, ReplicatedPlayers},
            protocol::session::PlayerState,
        };
        let mut app = app();
        session(&mut app, GameMode::Creative, false);
        let id = terra_core::PlayerId(9);
        let state = PlayerState {
            player: id,
            position: [1.5, 1.9, 1.5],
            ..default()
        };
        let mut players = ReplicatedPlayers::default();
        players.players.insert(
            id,
            RemoteTrack {
                previous: state,
                current: state,
                received: std::time::Instant::now(),
                interval: 0.05,
            },
        );
        app.insert_resource(players);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        assert!(!app.world().resource::<PlacementPreview>().valid);
        no_direct_removal(&app);
        app.world_mut()
            .resource_mut::<ReplicatedPlayers>()
            .players
            .clear();
        app.update();
        assert!(app.world().resource::<PlacementPreview>().valid);
        assert_eq!(
            app.world().resource::<Messages<BlockEditRequest>>().len(),
            1
        );
    }
}
