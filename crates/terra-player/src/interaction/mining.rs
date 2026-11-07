use super::{TargetBlock, TargetWorld, WorldTarget};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use terra_core::{GameAction as Action, GameMode, GameplayInput, ItemKind};
use terra_network::{
    client::{FarmToolRequest, MiningRequest, PropPlaceRequest},
    protocol::session::{MiningIntent, MiningTarget},
};
use terra_voxel::MaterialId;
use terra_world::streaming::{BlockEditRequest, ChunkStore, PropRemoveRequest};

#[derive(SystemParam)]
pub(crate) struct EditRequests<'w> {
    creatures: Option<ResMut<'w, terra_network::client::CreatureActions>>,
    player_attacks: Option<ResMut<'w, terra_network::client::PlayerAttacks>>,
    nests: Option<ResMut<'w, terra_network::client::BeeNestActions>>,
    blocks: MessageWriter<'w, BlockEditRequest>,
    props: MessageWriter<'w, PropRemoveRequest>,
    mining: MessageWriter<'w, MiningRequest>,
    planted: MessageWriter<'w, PropPlaceRequest>,
    fruit: MessageWriter<'w, terra_network::client::TreeHarvestRequest>,
    farming: MessageWriter<'w, FarmToolRequest>,
    rotations: MessageWriter<'w, terra_network::client::ObjectRotateRequest>,
    conveyors: MessageWriter<'w, terra_network::client::ConveyorConnectRequest>,
    belt_interactions: MessageWriter<'w, terra_network::client::ConveyorInteractionRequest>,
    inventory: MessageWriter<'w, terra_network::client::InventoryRequest>,
}

#[derive(Resource, Default, Debug)]
pub struct MiningVisual {
    pub target: Option<MiningTarget>,
    pub progress: f32,
    active: Option<MiningTarget>,
    heartbeat: f32,
    repeat: f32,
    held_elapsed: f32,
    half: Option<terra_world::slabs::HalfRemoval>,
    fell_tree: bool,
}
impl MiningVisual {
    pub fn slab_half(&self) -> Option<terra_world::slabs::HalfRemoval> {
        self.half
    }
}

#[derive(Resource, Default, Debug)]
pub struct PlacementPreview {
    pub position: Option<[i64; 3]>,
    pub material: u16,
    pub valid: bool,
    pub plan: Option<terra_world::placement::Layout>,
    pub conveyor_start: Option<([i64; 3], u64)>,
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
    vessel: ResMut<'w, super::VesselMotion>,
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
    selected: Option<Res<'w, terra_network::client::SelectedInventorySlot>>,
    remote: Option<Res<'w, terra_network::client::ReplicatedPlayers>>,
    active: Option<Res<'w, super::ActiveItem>>,
    journal: Option<Res<'w, terra_world::edits::EditJournal>>,
    terrain: Option<Res<'w, terra_world::streaming::WorldTerrain>>,
    properties: Option<Res<'w, terra_core::block_properties::BlockPropertiesCatalog>>,
    config: Option<Res<'w, terra_core::config::GameConfig>>,
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
    presentation.preview.plan = None;
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
    let selected = target.active.as_ref().and_then(|s| s.0).or_else(|| {
        if target.session.is_none() {
            palette.selected().map(|b| ItemKind::Block(b.id))
        } else {
            None
        }
    });
    presentation.vessel.begin_frame(
        selected,
        target
            .session
            .as_deref()
            .map_or((0, 0), |s| s.watering_reserve),
        !blocked,
        (keys.pressed(Action::Place) || keys.pressed(Action::Use)) && !keys.pressed(Action::Break),
    );
    if selected.and_then(ItemKind::block_material) != Some(148) || keys.just_pressed(Action::Break)
    {
        presentation.preview.conveyor_start = None;
    }
    let creature = target.creature.as_ref().and_then(|t| t.0);
    let player = if !blocked
        && creature.is_none()
        && keys.pressed(Action::Break)
        && target
            .config
            .as_deref()
            .is_some_and(|c| c.world.game_rules.pvp)
        && let (Some(remote), Some((camera, look))) =
            (target.remote.as_deref(), target.camera.iter().next())
    {
        let eye = target
            .bodies
            .iter()
            .next()
            .map_or(camera.translation, |body| {
                body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET
            });
        let direction = look.rotation() * Vec3::NEG_Z;
        let reach = target.world.0.map_or(4.5, |hit| match hit {
            WorldTarget::Block(hit) => hit.distance.min(4.5),
            WorldTarget::Prop(hit) => hit.distance.min(4.5),
        });
        remote
            .players
            .iter()
            .filter(|(_, track)| !track.current.survival.dead())
            .filter_map(|(id, track)| {
                terra_physics::character::melee_ray_distance(
                    track.position(),
                    eye,
                    direction,
                    reach,
                )
                .map(|distance| (distance, *id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    } else {
        None
    };

    // Faucet clicks adjust its stage. Only Shift + break starts removal.
    let faucet_control = !keys.keys.pressed(KeyCode::ShiftLeft)
        && !keys.keys.pressed(KeyCode::ShiftRight)
        && target.world.0.is_some_and(|hit| matches!(hit,
            WorldTarget::Block(hit) if chunks.block_at(hit.position) == Some(terra_core::plumbing::FAUCET)));
    let selected_target = if blocked
        || creature.is_some()
        || player.is_some()
        || !keys.pressed(Action::Break)
        || faucet_control
    {
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
    let fell_tree = keys.pressed(Action::FlyDown)
        && (!creative || terra_world::tree_felling::is_axe(selected))
        && selected_target.is_some_and(|target_block| {
            let MiningTarget::Block { position, .. } = target_block else {
                return false;
            };
            target.journal.as_deref().is_some_and(|journal| {
                terra_world::tree_felling::eligible(&chunks, journal, position)
            })
        });
    // The server owns timing; release, focus loss and UI immediately
    // cancel the intent. A bounded heartbeat also covers silent disconnects.
    let active = if creative && !fell_tree {
        None
    } else {
        selected_target
    };
    let half = active.and_then(|target_block| {
        let MiningTarget::Block { position, .. } = target_block else {
            return None;
        };
        let journal = target.journal.as_ref()?;
        let (camera, look) = target.camera.iter().next()?;
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
        terra_world::slabs::targeted_half(
            &chunks,
            journal,
            position,
            (
                origin.to_array().map(f64::from),
                (look.rotation() * Vec3::NEG_Z).to_array().map(f64::from),
            ),
        )
    });
    let half_changed = half != presentation.mining.half;
    let tree_changed = fell_tree != presentation.mining.fell_tree;
    if active != presentation.mining.active || half_changed || tree_changed {
        presentation.mining.held_elapsed = 0.0;
    }
    if active.is_some() {
        presentation.mining.held_elapsed += delta;
    }
    if active != presentation.mining.active
        || half_changed
        || tree_changed
        || (active.is_some() && presentation.mining.heartbeat >= 0.1)
    {
        requests.mining.write(MiningRequest(MiningIntent {
            target: active,
            look: target.session.as_ref().map_or([0.0; 2], |s| s.look),
            fell_tree,
        }));
        presentation.mining.active = active;
        presentation.mining.heartbeat = 0.0;
    }
    presentation.mining.half = half;
    presentation.mining.fell_tree = fell_tree;
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
    if blocked || faucet_control && keys.pressed(Action::Break) {
        return;
    }
    if (keys.just_pressed(Action::Place) || keys.just_pressed(Action::Use))
        && let Some(WorldTarget::Prop(hit)) = target.world.0
        && hit.prop.kind == terra_world::props::PropKind::AppleFruit
    {
        if !blocked
            && !keys.pressed(Action::Break)
            && hit.prop.variant == 0
            && let Some(journal) = target.journal.as_deref()
            && let Some((tree, slot)) = journal
                .gameplay()
                .trees
                .trees
                .values()
                .filter(|tree| {
                    tree.species == terra_core::TreeSpecies::Apple && tree.stage.is_mature()
                })
                .filter_map(|tree| {
                    terra_world::fruit::slot(tree.id, hit.prop.id).map(|slot| (tree, slot))
                })
                .next()
            && let Some(fruit) = tree.fruit
        {
            requests
                .fruit
                .write(terra_network::client::TreeHarvestRequest {
                    id: tree.id,
                    fruit_revision: fruit.revision,
                    slot,
                });
            presentation.motion.placing = true;
            presentation.motion.impulse = presentation.motion.impulse.wrapping_add(1);
        }
        return;
    }
    if (keys.just_pressed(Action::Place) || keys.just_pressed(Action::Use))
        && !keys.pressed(Action::Break)
        && let Some(journal) = target.journal.as_deref()
    {
        if let Some(ItemKind::Dye(color)) = selected
            && let Some(hit) = target.block.0
            && (terra_world::conveyors::segment_at(journal, hit.position).is_some()
                || chunks.block_at(hit.position).is_some_and(|material| {
                    terra_core::tints::cell_dyeable(
                        material,
                        journal
                            .object_state(hit.position)
                            .unwrap_or_default()
                            .facing,
                    ) && !terra_core::beds::is_bed(material)
                }))
        {
            let request = if terra_world::conveyors::segment_at(journal, hit.position).is_some() {
                terra_network::client::ConveyorInteractionRequest::Dye {
                    position: hit.position,
                    color,
                }
            } else {
                terra_network::client::ConveyorInteractionRequest::DyeFabric {
                    position: hit.position,
                    color,
                }
            };
            requests.belt_interactions.write(request);
            return;
        }
        if let Some(session) = target.session.as_deref()
            && let Some((camera, look)) = target.camera.iter().next()
        {
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
            let limit = target.world.0.map_or(6.0, |hit| match hit {
                WorldTarget::Block(hit) => hit.distance.min(6.0),
                WorldTarget::Prop(hit) => hit.distance.min(6.0),
            });
            if let Some((id, _)) = terra_world::conveyors::carried_drop_hit(
                &session.drops,
                journal,
                origin.to_array().map(f64::from),
                (look.rotation() * Vec3::NEG_Z).to_array().map(f64::from),
                limit,
            ) {
                requests
                    .belt_interactions
                    .write(terra_network::client::ConveyorInteractionRequest::Collect { id });
                return;
            }
        }
    }
    if selected == Some(ItemKind::Tool(terra_core::ToolKind::Wrench))
        && (keys.just_pressed(Action::Place) || keys.just_pressed(Action::Use))
    {
        if let Some(WorldTarget::Block(hit)) = target.world.0
            && let Some(material) = chunks.block_at(hit.position)
            && terra_world::wrench::supported(material)
        {
            requests
                .rotations
                .write(terra_network::client::ObjectRotateRequest {
                    position: hit.position,
                    expected: material,
                    facing: target
                        .journal
                        .as_deref()
                        .and_then(|journal| journal.object_state(hit.position))
                        .unwrap_or_default()
                        .facing,
                    reverse: keys.pressed(Action::FlyDown),
                });
            presentation.motion.placing = true;
            presentation.motion.impulse = presentation.motion.impulse.wrapping_add(1);
        }
        return;
    }
    if let Some(item) = selected.filter(|item| item.block_material() == Some(148))
        && !keys.pressed(Action::Break)
    {
        if keys.pressed(Action::FlyDown) {
            presentation.preview.conveyor_start = None;
            return;
        }
        if let Some(hit) = target.block.0
            && let Some(journal) = target.journal.as_deref()
            && chunks
                .block_at(hit.position)
                .is_some_and(|id| matches!(id, 144..=146))
        {
            if let Some((start, start_revision)) = presentation.preview.conveyor_start
                && start != hit.position
            {
                if let Ok(plan) =
                    terra_world::conveyors::layout(&chunks, journal, start, hit.position, item)
                {
                    let valid = terra_world::placement::environment(
                        &plan,
                        &chunks,
                        journal,
                        target
                            .config
                            .as_deref()
                            .is_none_or(|c| c.world.bounded_world),
                    )
                    .is_ok();
                    presentation.preview.position = Some(plan.edits[0].position);
                    presentation.preview.material = 148;
                    presentation.preview.valid = valid;
                    presentation.preview.plan = Some(plan);
                    if valid && keys.just_pressed(Action::Place) {
                        requests
                            .conveyors
                            .write(terra_network::client::ConveyorConnectRequest {
                                start,
                                end: hit.position,
                                start_revision,
                                item,
                            });
                        presentation.preview.conveyor_start = None;
                    }
                }
            } else if keys.just_pressed(Action::Place) {
                presentation.preview.conveyor_start = Some((
                    hit.position,
                    journal.revision(terra_voxel::global_to_section(hit.position).0),
                ));
            }
        }
        return;
    }
    if (keys.just_pressed(Action::Place) || keys.just_pressed(Action::Use))
        && !keys.pressed(Action::FlyDown)
        && !keys.pressed(Action::Break)
        && let Some(WorldTarget::Prop(hit)) = target.world.0
        && hit.prop.kind == terra_world::props::PropKind::BeeNest
    {
        if let Some(queue) = requests.nests.as_mut()
            && queue.0.len() < 2
        {
            queue.0.push_back((hit.prop.section, hit.prop.id));
            presentation.motion.placing = true;
            presentation.motion.impulse = presentation.motion.impulse.wrapping_add(1);
        }
        return;
    }
    if selected == Some(ItemKind::GlassBottle)
        && (keys.pressed(Action::Place) || keys.pressed(Action::Use))
        && !keys.pressed(Action::Break)
    {
        let mut filling = false;
        if let (Some(journal), Some((camera, look)), Some(session), Some(slot)) = (
            target.journal.as_deref(),
            target.camera.iter().next(),
            target.session.as_deref(),
            target.selected.as_deref().and_then(|selected| selected.0),
        ) {
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
            if let Some(position) = super::vessels::water_target(
                &chunks,
                journal,
                Some(session),
                origin.to_array().map(f64::from),
                (look.rotation() * Vec3::NEG_Z).to_array().map(f64::from),
                1,
            ) {
                filling = true;
                if presentation.vessel.fill_from(
                    ItemKind::GlassBottle,
                    position,
                    slot,
                    session.inventory.revision,
                    delta,
                ) {
                    requests
                        .inventory
                        .write(terra_network::client::InventoryRequest(
                        terra_network::protocol::inventory::InventoryCommand {
                            revision: session.inventory.revision,
                            action:
                                terra_network::protocol::inventory::InventoryAction::FillBottle {
                                    slot,
                                    position,
                                },
                        },
                    ));
                    presentation.motion.placing = true;
                    presentation.motion.impulse = presentation.motion.impulse.wrapping_add(1);
                }
            }
        }
        if !filling {
            presentation.vessel.cancel_fill();
        }
        return;
    }
    if let Some(ItemKind::Tool(
        tool @ (terra_core::ToolKind::Bucket | terra_core::ToolKind::WaterBucket),
    )) = selected
        && (keys.pressed(Action::Place) || keys.pressed(Action::Use))
        && !keys.pressed(Action::Break)
    {
        let mut filling = false;
        if let (Some(journal), Some(properties), Some((camera, look))) = (
            target.journal.as_deref(),
            target.properties.as_deref(),
            target.camera.iter().next(),
        ) {
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
            let filled = tool == terra_core::ToolKind::WaterBucket;
            let ray_origin = origin.to_array().map(f64::from);
            let ray_direction = (look.rotation() * Vec3::NEG_Z).to_array().map(f64::from);
            let position = if filled {
                terra_world::water::buckets::target(
                    &chunks,
                    journal,
                    properties,
                    true,
                    ray_origin,
                    ray_direction,
                )
            } else {
                super::vessels::water_target(
                    &chunks,
                    journal,
                    target.session.as_deref(),
                    ray_origin,
                    ray_direction,
                    8,
                )
            };
            if let Some(position) = position {
                filling =
                    !filled && chunks.block_at(position) == Some(terra_core::plumbing::FAUCET);
                // Natural water fills on a single use. Faucets keep their visible
                // hold because their finite buffer can arrive in several portions.
                let ready =
                    if filled || chunks.block_at(position) != Some(terra_core::plumbing::FAUCET) {
                        keys.just_pressed(Action::Place) || keys.just_pressed(Action::Use)
                    } else if let (Some(session), Some(slot)) = (
                        target.session.as_deref(),
                        target.selected.as_deref().and_then(|selected| selected.0),
                    ) {
                        presentation.vessel.fill_from(
                            ItemKind::Tool(tool),
                            position,
                            slot,
                            session.inventory.revision,
                            delta,
                        )
                    } else {
                        false
                    };
                if ready {
                    requests.farming.write(FarmToolRequest {
                        position,
                        action: if filled {
                            terra_core::farming::FarmingAction::EmptyBucket
                        } else {
                            terra_core::farming::FarmingAction::FillBucket
                        },
                    });
                    presentation.motion.placing = true;
                    presentation.motion.impulse = presentation.motion.impulse.wrapping_add(1);
                }
            }
        }
        if !filling {
            presentation.vessel.cancel_fill();
        }
        return;
    }
    if let Some(tool) = selected.and_then(ItemKind::tool_kind)
        && let Some(role) = tool.farm_kind()
        && (keys.pressed(Action::Place) || keys.pressed(Action::Use))
        && !keys.pressed(Action::Break)
    {
        use terra_core::farming::{FarmToolKind, FarmingAction};
        let support = target
            .world
            .0
            .map(|hit| match hit {
                WorldTarget::Prop(hit) => terra_world::props::placement::support(&hit.prop),
                WorldTarget::Block(hit) => hit.position,
            })
            .or_else(|| target.block.0.map(|hit| hit.position));
        let water = target.camera.iter().next().and_then(|(camera, look)| {
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
            target.journal.as_deref().and_then(|journal| {
                super::vessels::water_target(
                    &chunks,
                    journal,
                    target.session.as_deref(),
                    origin.to_array().map(f64::from),
                    (look.rotation() * Vec3::NEG_Z).to_array().map(f64::from),
                    1,
                )
            })
        });
        if role == FarmToolKind::WateringCan
            && (creative
                || target
                    .session
                    .as_deref()
                    .is_some_and(|_| selected.is_some_and(|item| item.watering_units() > 0)))
            && let Some(position) = support.filter(|position| {
                chunks.block_at(*position) == Some(terra_core::farming::FARMLAND)
            })
        {
            presentation.vessel.pour(position);
        }
        let action = match role {
            FarmToolKind::WateringCan
                if support.is_some_and(|position| {
                    target.session.as_deref().is_some_and(|session| {
                        session
                            .crop_progress
                            .get(&position)
                            .is_some_and(|crop| crop.moisture < 0.999)
                            && (creative || selected.is_some_and(|item| item.watering_units() > 0))
                    })
                }) =>
            {
                support.map(|position| (position, FarmingAction::Water))
            }
            FarmToolKind::WateringCan
                if water.is_some()
                    && selected
                        .is_some_and(|item| item.watering_units() < tool.watering_capacity()) =>
            {
                water.map(|position| (position, FarmingAction::Refill))
            }
            FarmToolKind::WateringCan => None,
            FarmToolKind::Hoe => target
                .block
                .0
                .filter(|hit| {
                    hit.normal == [0, 1, 0]
                        && matches!(chunks.block_at(hit.position), Some(1 | 2))
                        && chunks.block_at([hit.position[0], hit.position[1] + 1, hit.position[2]])
                            == Some(0)
                        && chunks
                            .loaded
                            .get(&terra_voxel::global_to_section(hit.position).0)
                            .is_some_and(|section| {
                                section.props.iter().all(|prop| {
                                    prop.removed
                                        || terra_world::props::placement::support(prop)
                                            != hit.position
                                })
                            })
                })
                .map(|hit| (hit.position, FarmingAction::Till)),
            FarmToolKind::Scythe => support
                .filter(|position| {
                    target.session.as_deref().is_some_and(|session| {
                        session.crop_progress.values().any(|crop| {
                            crop.progress >= 1.0
                                && crop.support[1] == position[1]
                                && (crop.support[0] - position[0]).abs() <= tool.farm_radius()
                                && (crop.support[2] - position[2]).abs() <= tool.farm_radius()
                                && chunks
                                    .loaded
                                    .get(&terra_voxel::global_to_section(crop.support).0)
                                    .is_some_and(|section| {
                                        section
                                            .props
                                            .iter()
                                            .any(|prop| !prop.removed && prop.id.0 == crop.prop_id)
                                    })
                        })
                    })
                })
                .map(|position| (position, FarmingAction::Harvest)),
        };
        if let Some((position, action)) = action {
            if action == FarmingAction::Refill {
                let ready = if chunks.block_at(position) != Some(terra_core::plumbing::FAUCET) {
                    presentation.vessel.cancel_fill();
                    keys.just_pressed(Action::Place) || keys.just_pressed(Action::Use)
                } else if let (Some(session), Some(slot)) = (
                    target.session.as_deref(),
                    target.selected.as_deref().and_then(|selected| selected.0),
                ) {
                    presentation.vessel.fill_from(
                        selected.unwrap(),
                        position,
                        slot,
                        session.inventory.revision,
                        delta,
                    )
                } else {
                    false
                };
                if ready {
                    requests.farming.write(FarmToolRequest { position, action });
                    presentation.motion.impulse = presentation.motion.impulse.wrapping_add(1);
                }
                return;
            }
            presentation.vessel.cancel_fill();
            if action == FarmingAction::Water {
                presentation.vessel.pour(position);
            }
            presentation.motion.swinging = true;
            presentation.motion.placing = false;
            if presentation.mining.repeat == 0.0 {
                presentation.mining.repeat = tool.farm_cooldown();
                presentation.motion.impulse = presentation.motion.impulse.wrapping_add(1);
                requests.farming.write(FarmToolRequest { position, action });
            }
        } else {
            presentation.vessel.cancel_fill();
        }
        return;
    }
    presentation.motion.swinging = keys.pressed(Action::Break)
        && (selected_target.is_some()
            || creature.is_some()
            || player.is_some()
            || target.world.0.is_none());
    if let Some(player) = player {
        if keys.just_pressed(Action::Break)
            && let Some(queue) = requests.player_attacks.as_mut()
            && queue.0.len() < 2
        {
            queue.0.push_back(player);
        }
        return;
    }
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
        && !fell_tree
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
                terra_world::objects::is_opening(m)
                    || terra_core::beds::is_bed(m)
                    || m == terra_core::flower_pot::MATERIAL
                    || matches!(m, 166..=167)
                        && !selected.is_some_and(|item| {
                            item.block_material()
                                .is_some_and(terra_core::plumbing::is_part)
                        })
            }),
            WorldTarget::Prop(_) => false,
        })
    {
        return;
    }
    // A mounted crank consumes held use without blocking camera or movement.
    if (keys.pressed(Action::Place) || keys.pressed(Action::Use))
        && !selected.is_some_and(|item| matches!(item.block_material(), Some(100 | 141..=160)))
        && !keys.keys.pressed(KeyCode::ShiftLeft)
        && !keys.keys.pressed(KeyCode::ShiftRight)
        && target.world.0.is_some_and(|hit| match hit {
            WorldTarget::Block(hit) => {
                chunks.block_at(hit.position) == Some(terra_core::crusher::CRANK_MATERIAL)
                    && target
                        .journal
                        .as_ref()
                        .is_some_and(|journal| journal.crank_anchor(hit.position).is_some())
            }
            WorldTarget::Prop(_) => false,
        })
    {
        presentation.motion.placing = false;
        return;
    }
    // Edible props use the same action as placement; Shift keeps planting available.
    if selected.is_some_and(|item| {
        terra_core::food(item).is_some() || terra_core::brewing::drink_seconds(item).is_some()
    }) && keys.consume_pressed()
    {
        return;
    }
    if selected == Some(ItemKind::Block(98)) {
        if let Some((hit, journal)) = target.block.0.zip(target.journal.as_deref())
            && let Some((position, expected)) = terra_world::snow::target(&chunks, journal, hit)
            && let Some(plan) = terra_world::snow::plan(&chunks, journal, position, expected)
        {
            let half = Vec3::new(
                terra_physics::character::PLAYER_RADIUS,
                terra_physics::character::PLAYER_HALF_HEIGHT,
                terra_physics::character::PLAYER_RADIUS,
            );
            let overlaps = |center: Vec3| {
                terra_world::snow::bounds(position, plan).is_some_and(|bounds| {
                    bounds.intersects(
                        (center - half).to_array().map(f64::from),
                        (center + half).to_array().map(f64::from),
                    )
                })
            };
            let valid = (creative
                || target
                    .session
                    .as_ref()
                    .is_some_and(|session| session.inventory.count(ItemKind::Block(98)) > 0))
                && !target.bodies.iter().any(|body| overlaps(body.translation))
                && !target.remote.as_ref().is_some_and(|players| {
                    players
                        .players
                        .values()
                        .any(|track| overlaps(track.position()))
                });
            presentation.preview.position = Some(position);
            presentation.preview.material = 98;
            presentation.preview.valid = valid;
            if valid && keys.just_pressed(Action::Place) {
                presentation.motion.placing = true;
                presentation.motion.impulse = presentation.motion.impulse.wrapping_add(1);
                requests.blocks.write(BlockEditRequest {
                    position,
                    expected_material: expected,
                    material: 98,
                });
            }
        }
        return;
    }
    let selected_prop = selected.and_then(|item| match item {
        ItemKind::Prop(kind) => Some(kind),
        ItemKind::Flower(_) => Some(terra_core::ResourcePropKind::Flower),
        ItemKind::Crop(seed) => seed.crop_kind().map(|kind| kind.resource_prop()),
        _ => None,
    });
    if let Some(kind) = selected_prop
        && kind != terra_core::ResourcePropKind::Seagrass
    {
        if let Some(species) = terra_core::TreeSpecies::from_sapling(kind) {
            if let Some(hit) = target.block.0
                && hit.normal == [0, 1, 0]
            {
                let has_item = creative
                    || target
                        .session
                        .as_ref()
                        .is_some_and(|s| selected.is_some_and(|item| s.inventory.count(item) > 0));
                let valid = has_item
                    && target.journal.as_ref().is_some_and(|j| {
                        let epoch = j.prop_epoch(terra_voxel::global_to_section(hit.position).0);
                        let prop =
                            terra_world::saplings::placement_prop(hit.position, species, epoch);
                        let owner = target.session.as_ref().and_then(|s| s.uuid);
                        target
                            .terrain
                            .as_ref()
                            .map_or_else(
                                || terra_world::saplings::placement_check(&chunks, j, prop, owner),
                                |terrain| {
                                    terra_world::saplings::placement_check_in_world(
                                        &chunks, j, &terrain.0, prop, owner,
                                    )
                                },
                            )
                            .is_ok()
                    });
                presentation.preview.position = Some(hit.adjacent());
                presentation.preview.material = 0;
                presentation.preview.valid = valid;
                // Authority returns a specific planting reason on an invalid click.
                // No inventory or terrain is changed speculatively by this preview.
                if has_item && keys.just_pressed(Action::Place) {
                    requests.planted.write(PropPlaceRequest {
                        support: hit.position,
                        kind,
                    });
                    presentation.motion.placing = valid;
                    if valid {
                        presentation.motion.impulse = presentation.motion.impulse.wrapping_add(1);
                    }
                }
            }
            return;
        }
        if let Some(hit) = target.block.0
            && (hit.normal == [0, 1, 0] || kind == terra_core::ResourcePropKind::SpiderWeb)
        {
            let has_item = creative
                || target
                    .session
                    .as_ref()
                    .is_some_and(|s| selected.is_some_and(|item| s.inventory.count(item) > 0));
            if has_item
                && matches!(chunks.block_at(hit.adjacent()), Some(0 | 27))
                && chunks
                    .block_at(hit.position)
                    .and_then(MaterialId::from_raw)
                    .is_some_and(|m| {
                        if matches!(
                            kind,
                            terra_core::ResourcePropKind::Scarecrow
                                | terra_core::ResourcePropKind::SpiderWeb
                        ) {
                            m.is_solid()
                        } else if terra_world::props::PropKind::from(kind).aquatic() {
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
        let plan = target.camera.iter().next().and_then(|(camera, look)| {
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
                let (yaw, pitch, _) = look.rotation().to_euler(EulerRot::YXZ);
                terra_world::placement::propose(
                    &chunks,
                    journal,
                    hit,
                    selected_item,
                    (
                        origin.to_array().map(f64::from),
                        (look.rotation() * Vec3::NEG_Z).to_array().map(f64::from),
                    ),
                    [yaw, pitch],
                )
                .ok()
            })
        });
        let boxes = plan
            .as_ref()
            .zip(target.journal.as_ref())
            .and_then(|(plan, journal)| {
                target.properties.as_ref().map(|properties| {
                    terra_world::placement::boxes(plan, &chunks, journal, properties)
                })
            });
        let environment_valid = target.journal.as_ref().is_none_or(|journal| {
            plan.as_ref().is_some_and(|plan| {
                terra_world::placement::environment(
                    plan,
                    &chunks,
                    journal,
                    target.config.as_ref().is_none_or(|c| c.world.bounded_world),
                )
                .is_ok()
            })
        });
        let mount_valid = if terra_core::lantern::is_part(selected) {
            target.properties.as_ref().is_none_or(|properties| {
                terra_core::lantern::facing(selected, hit.normal).is_some_and(|facing| {
                    terra_world::lanterns::supported(
                        &chunks, properties, position, selected, facing,
                    )
                })
            })
        } else {
            selected != 40
                || target.properties.as_ref().is_none_or(|properties| {
                    terra_world::torches::facing(hit.normal).is_some_and(|facing| {
                        terra_world::torches::supported(&chunks, properties, position, facing)
                    })
                })
        };
        let overlaps = |center: Vec3| {
            boxes.as_ref().map_or_else(
                || crate::camera::block_overlaps_body(position, center),
                |boxes| {
                    let half = Vec3::new(
                        terra_physics::character::PLAYER_RADIUS,
                        terra_physics::character::PLAYER_HALF_HEIGHT,
                        terra_physics::character::PLAYER_RADIUS,
                    );
                    boxes.iter().any(|bounds| {
                        bounds.intersects(
                            (center - half).to_array().map(f64::from),
                            (center + half).to_array().map(f64::from),
                        )
                    })
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
            && environment_valid
            && mount_valid
            && (selected != expected || terra_core::construction::is_slab(selected))
            && (!MaterialId::from_raw(selected).is_some_and(MaterialId::is_solid)
                || !overlaps_player);
        presentation.preview.position = Some(position);
        presentation.preview.material = selected;
        presentation.preview.valid = valid;
        presentation.preview.plan = plan;
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
            .init_resource::<super::super::VesselMotion>()
            .insert_resource(crate::CameraInputState {
                captured: true,
                ..default()
            })
            .add_message::<BlockEditRequest>()
            .add_message::<PropRemoveRequest>()
            .add_message::<MiningRequest>()
            .add_message::<PropPlaceRequest>()
            .add_message::<FarmToolRequest>()
            .add_message::<terra_network::client::TreeHarvestRequest>()
            .add_message::<terra_network::client::InventoryRequest>()
            .add_message::<terra_network::client::ObjectRotateRequest>()
            .add_message::<terra_network::client::ConveyorConnectRequest>()
            .add_message::<terra_network::client::ConveyorInteractionRequest>()
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
    fn faucet_left_click_only_mines_when_shift_is_held_in_survival_and_creative() {
        for mode in [GameMode::Survival, GameMode::Creative] {
            for shift in [false, true] {
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
                    .set_block([1, 0, 1], terra_core::plumbing::FAUCET)
                    .unwrap();
                let mut journal = terra_world::edits::EditJournal::default();
                journal
                    .record(hit.position, terra_core::plumbing::FAUCET)
                    .unwrap();
                journal
                    .set_object_state(hit.position, Some(Default::default()))
                    .unwrap();
                app.insert_resource(journal)
                    .insert_resource(super::super::ActiveItem(Some(ItemKind::Tool(
                        terra_core::ToolKind::WoodPickaxe,
                    ))));
                if shift {
                    app.world_mut()
                        .resource_mut::<ButtonInput<KeyCode>>()
                        .press(KeyCode::ShiftLeft);
                }
                app.world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .press(MouseButton::Left);
                app.update();
                let intents = mining_requests(&mut app);
                let requests: Vec<_> = app
                    .world_mut()
                    .resource_mut::<Messages<BlockEditRequest>>()
                    .drain()
                    .collect();
                if !shift {
                    assert!(intents.iter().all(|intent| intent.target.is_none()));
                    assert!(requests.is_empty());
                    assert!(app.world().resource::<MiningVisual>().target.is_none());
                } else if mode == GameMode::Survival {
                    assert!(intents.iter().any(|intent| matches!(intent.target,
                        Some(MiningTarget::Block { position, expected: terra_core::plumbing::FAUCET }) if position == hit.position)));
                } else {
                    assert!(
                        requests.iter().any(
                            |request| request.position == hit.position && request.material == 0
                        )
                    );
                }
                assert_eq!(
                    app.world().resource::<ChunkStore>().block_at(hit.position),
                    Some(terra_core::plumbing::FAUCET)
                );
            }
        }
    }

    #[test]
    fn snow_plates_request_shape_coating_and_same_cell_stacking_without_local_world_mutation() {
        for expected in [78, 98] {
            let mut app = app();
            let position = [1, 0, 1];
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block([1, 0, 1], expected)
                .unwrap();
            let mut journal = terra_world::edits::EditJournal::default();
            journal.record(position, expected).unwrap();
            journal
                .set_object_state(position, Some(Default::default()))
                .unwrap();
            let before = journal.section_delta(SectionPos::default());
            app.insert_resource(journal)
                .insert_resource(super::super::ActiveItem(Some(ItemKind::Block(98))))
                .insert_resource(TargetWorld(Some(WorldTarget::Block(BlockHit {
                    position,
                    normal: [0, 1, 0],
                    distance: 2.0,
                }))));
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
            assert_eq!(requests[0].position, position);
            assert_eq!(requests[0].expected_material, expected);
            assert_eq!(requests[0].material, 98);
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(position),
                Some(expected)
            );
            assert_eq!(
                app.world()
                    .resource::<terra_world::edits::EditJournal>()
                    .section_delta(SectionPos::default()),
                before
            );
        }
    }

    #[test]
    fn right_click_collects_carried_items_or_dyes_belts_without_local_mutation() {
        for dye in [false, true] {
            let mut app = app();
            session(&mut app, GameMode::Survival, false);
            let position = [7, 4, 7];
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block([7, 4, 7], 148)
                .unwrap();
            let mut journal = terra_world::edits::EditJournal::default();
            journal.record(position, 148).unwrap();
            journal
                .set_object_state(position, Some(Default::default()))
                .unwrap();
            journal.gameplay_mut().drop_stack(
                [7.5, 4.755, 7.5],
                terra_core::ItemStack {
                    item: ItemKind::Block(3),
                    count: 9,
                },
                [0.0; 3],
                0.0,
            );
            let id = journal.gameplay().drops[0].id;
            app.world_mut()
                .resource_mut::<terra_network::client::ClientSession>()
                .drops = journal.gameplay().drops.clone();
            app.insert_resource(journal);
            app.world_mut()
                .spawn((crate::TerraCamera, Transform::from_xyz(7.5, 4.755, 10.5)));
            let color = terra_core::beds::DyeColor::Red;
            app.insert_resource(super::super::ActiveItem(if dye {
                Some(ItemKind::Dye(color))
            } else {
                None
            }));
            if dye {
                let hit = BlockHit {
                    position,
                    normal: [0, 1, 0],
                    distance: 3.0,
                };
                app.insert_resource(TargetBlock(Some(hit)))
                    .insert_resource(TargetWorld(Some(WorldTarget::Block(hit))));
                app.world_mut()
                    .resource_mut::<terra_network::client::ClientSession>()
                    .inventory
                    .insert(ItemKind::Dye(color), 2);
            } else {
                app.insert_resource(TargetBlock(None))
                    .insert_resource(TargetWorld(None));
            }
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyB);
            app.update();
            let requests: Vec<_> = app
                .world_mut()
                .resource_mut::<Messages<terra_network::client::ConveyorInteractionRequest>>()
                .drain()
                .collect();
            assert_eq!(requests.len(), 1);
            if dye {
                assert!(
                    matches!(requests[0], terra_network::client::ConveyorInteractionRequest::Dye { position: p, color: c } if p == position && c == color)
                );
                assert_eq!(
                    app.world()
                        .resource::<terra_network::client::ClientSession>()
                        .inventory
                        .count(ItemKind::Dye(color)),
                    2
                );
                assert_eq!(
                    app.world()
                        .resource::<terra_world::edits::EditJournal>()
                        .object_state(position)
                        .unwrap()
                        .facing,
                    0
                );
            } else {
                assert!(
                    matches!(requests[0], terra_network::client::ConveyorInteractionRequest::Collect { id: target } if target == id)
                );
            }
            assert_eq!(
                app.world()
                    .resource::<terra_world::edits::EditJournal>()
                    .gameplay()
                    .drops[0]
                    .stack
                    .count,
                9
            );
            assert!(
                app.world()
                    .resource::<Messages<BlockEditRequest>>()
                    .is_empty()
            );
        }
    }

    #[test]
    fn rgb_dye_requests_real_fabric_interactions_for_full_blocks_shapes_and_stone_owned_wool_halves()
     {
        for material in [103, 104, 105, 106, 85] {
            let mut app = app();
            session(&mut app, GameMode::Survival, false);
            let position = [7, 4, 7];
            let color = terra_core::beds::DyeColor::Rgb([17, 95, 213]);
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block([7, 4, 7], material)
                .unwrap();
            let mut journal = terra_world::edits::EditJournal::default();
            journal.record(position, material).unwrap();
            if material == 85 {
                journal
                    .set_object_state(
                        position,
                        Some(terra_world::edits::BlockObjectState {
                            facing: terra_core::construction::merged_slab(85, 0, 105).unwrap(),
                            ..Default::default()
                        }),
                    )
                    .unwrap();
            }
            app.insert_resource(journal);
            app.insert_resource(super::super::ActiveItem(Some(ItemKind::Dye(color))));
            app.world_mut()
                .resource_mut::<terra_network::client::ClientSession>()
                .inventory
                .insert(ItemKind::Dye(color), 2);
            let hit = BlockHit {
                position,
                normal: [0, 1, 0],
                distance: 3.0,
            };
            app.insert_resource(TargetBlock(Some(hit)))
                .insert_resource(TargetWorld(Some(WorldTarget::Block(hit))));
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyB);
            app.update();
            let requests = app
                .world_mut()
                .resource_mut::<Messages<terra_network::client::ConveyorInteractionRequest>>()
                .drain()
                .collect::<Vec<_>>();
            assert!(
                matches!(requests.as_slice(),[terra_network::client::ConveyorInteractionRequest::DyeFabric {position:p,color:c}] if *p==position&&*c==color)
            );
            assert_eq!(
                app.world()
                    .resource::<terra_network::client::ClientSession>()
                    .inventory
                    .count(ItemKind::Dye(color)),
                2
            );
            assert_eq!(
                app.world()
                    .resource::<terra_world::edits::EditJournal>()
                    .tint_at(position),
                None
            );
            assert!(
                app.world()
                    .resource::<Messages<BlockEditRequest>>()
                    .is_empty()
            );
        }
    }

    #[test]
    fn conveyor_two_click_input_sends_one_owned_span_without_local_edits_and_shift_cancels() {
        for cancel in [false, true] {
            let mut app = app();
            session(&mut app, GameMode::Survival, false);
            let item = ItemKind::for_wood(148, terra_core::WoodSpecies::Birch);
            app.insert_resource(super::super::ActiveItem(Some(item)));
            app.world_mut()
                .resource_mut::<terra_network::client::ClientSession>()
                .inventory
                .insert(item, 1);
            let start = [4, 4, 4];
            let end = [4, 4, 8];
            let mut journal = terra_world::edits::EditJournal::default();
            for p in [start, end] {
                app.world_mut()
                    .resource_mut::<ChunkStore>()
                    .loaded
                    .get_mut(&SectionPos::default())
                    .unwrap()
                    .data
                    .set_block(p.map(|v| v as usize), 144)
                    .unwrap();
                journal.record(p, 144).unwrap();
                journal
                    .set_object_state(p, Some(Default::default()))
                    .unwrap();
            }
            app.insert_resource(journal);
            let hit = BlockHit {
                position: start,
                normal: [0, 1, 0],
                distance: 2.0,
            };
            app.insert_resource(TargetBlock(Some(hit)))
                .insert_resource(TargetWorld(Some(WorldTarget::Block(hit))));
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyB);
            app.update();
            assert_eq!(
                app.world()
                    .resource::<PlacementPreview>()
                    .conveyor_start
                    .unwrap()
                    .0,
                start
            );
            assert!(
                app.world()
                    .resource::<Messages<terra_network::client::ConveyorConnectRequest>>()
                    .is_empty()
            );
            {
                let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                keys.clear();
                keys.release(KeyCode::KeyB);
            }
            app.update();
            if cancel {
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::ShiftLeft);
                app.update();
                assert!(
                    app.world()
                        .resource::<PlacementPreview>()
                        .conveyor_start
                        .is_none()
                );
                assert!(
                    app.world()
                        .resource::<Messages<terra_network::client::ConveyorConnectRequest>>()
                        .is_empty()
                );
            } else {
                let hit = BlockHit {
                    position: end,
                    ..hit
                };
                app.insert_resource(TargetBlock(Some(hit)))
                    .insert_resource(TargetWorld(Some(WorldTarget::Block(hit))));
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::KeyB);
                app.update();
                let requests: Vec<_> = app
                    .world_mut()
                    .resource_mut::<Messages<terra_network::client::ConveyorConnectRequest>>()
                    .drain()
                    .collect();
                assert_eq!(requests.len(), 1);
                assert_eq!(
                    (requests[0].start, requests[0].end, requests[0].item),
                    (start, end, item)
                );
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .clear();
                app.update();
                assert!(
                    app.world()
                        .resource::<Messages<terra_network::client::ConveyorConnectRequest>>()
                        .is_empty()
                );
                assert!(
                    app.world()
                        .resource::<PlacementPreview>()
                        .conveyor_start
                        .is_none()
                );
            }
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at([4, 4, 5]),
                Some(0)
            );
            assert_eq!(
                app.world()
                    .resource::<terra_network::client::ClientSession>()
                    .inventory
                    .count(item),
                1
            );
            assert!(
                app.world()
                    .resource::<Messages<BlockEditRequest>>()
                    .is_empty()
            );
        }
    }

    #[test]
    fn wrench_uses_rebound_place_once_with_reverse_without_local_edit_or_consumption() {
        for reverse in [false, true] {
            let mut app = app();
            session(&mut app, GameMode::Survival, false);
            let item = ItemKind::Tool(terra_core::ToolKind::Wrench);
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
                .set_block([1, 0, 1], 145)
                .unwrap();
            app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Block(BlockHit {
                position: [1, 0, 1],
                normal: [0, 0, 1],
                distance: 2.0,
            }));
            if reverse {
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::ShiftLeft);
            }
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyB);
            app.update();
            let requests: Vec<_> = app
                .world_mut()
                .resource_mut::<Messages<terra_network::client::ObjectRotateRequest>>()
                .drain()
                .collect();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].position, [1, 0, 1]);
            assert_eq!(requests[0].expected, 145);
            assert_eq!(requests[0].reverse, reverse);
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
    }

    #[test]
    fn nest_use_queues_the_real_identity_once_and_does_not_place_or_remove_the_prop() {
        let mut app = app();
        app.init_resource::<terra_network::client::BeeNestActions>();
        let Some(WorldTarget::Prop(mut hit)) = app.world().resource::<TargetWorld>().0 else {
            panic!("fixture prop");
        };
        hit.prop.kind = PropKind::BeeNest;
        app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Prop(hit));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        assert_eq!(
            app.world()
                .resource::<terra_network::client::BeeNestActions>()
                .0
                .front(),
            Some(&(hit.prop.section, hit.prop.id))
        );
        assert!(
            app.world_mut()
                .resource_mut::<Messages<PropRemoveRequest>>()
                .drain()
                .next()
                .is_none()
        );
        assert!(
            app.world_mut()
                .resource_mut::<Messages<BlockEditRequest>>()
                .drain()
                .next()
                .is_none()
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        assert_eq!(
            app.world()
                .resource::<terra_network::client::BeeNestActions>()
                .0
                .len(),
            1
        );
    }

    #[test]
    fn moving_between_combined_slab_halves_immediately_restarts_local_mining_and_sends_intent() {
        let mut app = app();
        let position = [1, 0, 1];
        let mut journal = terra_world::edits::EditJournal::default();
        journal.record(position, 82).unwrap();
        journal
            .set_object_state(
                position,
                Some(terra_world::edits::BlockObjectState {
                    facing: terra_core::construction::merged_slab(82, 5, 85).unwrap(),
                    ..default()
                }),
            )
            .unwrap();
        app.insert_resource(journal);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([1, 0, 1], 82)
            .unwrap();
        let hit = BlockHit {
            position,
            normal: [0, 0, 1],
            distance: 2.0,
        };
        app.insert_resource(TargetBlock(Some(hit)))
            .insert_resource(TargetWorld(Some(WorldTarget::Block(hit))));
        let mut session = terra_network::client::ClientSession::default();
        session.ready = true;
        session.allow_build = true;
        session.game_mode = GameMode::Survival;
        app.insert_resource(session);
        let camera = app
            .world_mut()
            .spawn((
                crate::TerraCamera,
                Transform::from_xyz(1.25, 0.5, 4.0),
                crate::CameraLook::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(
            app.world().resource::<MiningVisual>().half.unwrap().removed,
            82
        );
        app.world_mut()
            .resource_mut::<Messages<MiningRequest>>()
            .clear();
        app.world_mut().resource_mut::<MiningVisual>().held_elapsed = 0.8;
        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .translation
            .x = 1.75;
        app.update();
        let visual = app.world().resource::<MiningVisual>();
        assert_eq!(visual.half.unwrap().removed, 85);
        assert_eq!(visual.held_elapsed, 0.0);
        assert_eq!(app.world().resource::<Messages<MiningRequest>>().len(), 1);
    }

    #[test]
    fn holding_a_mounted_crank_never_places_a_block_or_releases_capture() {
        use terra_world::edits::{BlockObjectState, EditJournal};
        let mut app = app();
        let mut journal = EditJournal::default();
        for (position, material, peer, secondary) in [
            ([1, 0, 1], 99, [2, 0, 1], false),
            ([2, 0, 1], 99, [1, 0, 1], true),
            ([3, 0, 1], 100, [1, 0, 1], false),
        ] {
            journal.record(position, material).unwrap();
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
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block(position.map(|p| p as usize), material)
                .unwrap();
        }
        app.insert_resource(journal)
            .insert_resource(super::super::ActiveItem(Some(ItemKind::Block(3))));
        let mut controls = terra_core::ControlsConfig::default();
        controls
            .rebind(
                Action::Place,
                terra_core::InputBinding::Mouse(MouseButton::Right),
            )
            .unwrap();
        app.insert_resource(controls);
        let hit = BlockHit {
            position: [3, 0, 1],
            normal: [1, 0, 0],
            distance: 2.0,
        };
        app.world_mut().resource_mut::<TargetBlock>().0 = Some(hit);
        app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Block(hit));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        for _ in 0..3 {
            app.update();
            assert!(
                app.world()
                    .resource::<Messages<BlockEditRequest>>()
                    .is_empty()
            );
            assert!(!app.world().resource::<PlacementPreview>().valid);
            assert!(!app.world().resource::<crate::PlayerInputBlock>().0);
            assert!(app.world().resource::<crate::CameraInputState>().captured);
        }
        app.insert_resource(super::super::ActiveItem(Some(ItemKind::Block(145))));
        app.update();
        assert_eq!(
            app.world().resource::<PlacementPreview>().position,
            Some([4, 0, 1])
        );
    }

    #[test]
    fn filtered_tunnel_uses_the_crank_inventory_mount_instead_of_consuming_manual_use() {
        use terra_world::edits::{BlockObjectState, EditJournal};
        let mut app = app();
        let mut journal = EditJournal::default();
        let owner = [1, 2, 1];
        let crank = [2, 2, 1];
        for (position, material, peer) in [(owner, 99, None), (crank, 100, Some(owner))] {
            journal.record(position, material).unwrap();
            journal
                .set_object_state(position, Some(BlockObjectState { peer, ..default() }))
                .unwrap();
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block(position.map(|value| value as usize), material)
                .unwrap();
        }
        assert_eq!(journal.crank_anchor(crank), Some(owner));
        let before = journal.section_delta(SectionPos::default());
        app.insert_resource(journal)
            .insert_resource(super::super::ActiveItem(Some(ItemKind::Block(159))));
        app.world_mut().spawn((
            crate::TerraCamera,
            Transform::from_xyz(5.5, 3.0, 3.5),
            crate::CameraLook::default(),
        ));
        let hit = BlockHit {
            position: crank,
            normal: [1, 0, 0],
            distance: 3.0,
        };
        app.world_mut().resource_mut::<TargetBlock>().0 = Some(hit);
        app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Block(hit));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        let preview = app.world().resource::<PlacementPreview>();
        assert!(
            preview.valid,
            "the real filtered-tunnel/crank layout must pass clearance"
        );
        assert_eq!(preview.position, Some([3, 2, 1]));
        assert_eq!(preview.material, 159);
        assert_eq!(
            preview.plan.as_ref().unwrap().states[0].1.unwrap().peer,
            Some(crank)
        );
        let requests: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<BlockEditRequest>>()
            .drain()
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            (
                requests[0].position,
                requests[0].expected_material,
                requests[0].material
            ),
            ([3, 2, 1], 0, 159)
        );
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .section_delta(SectionPos::default()),
            before
        );
        assert!(!app.world().resource::<crate::PlayerInputBlock>().0);
        assert!(app.world().resource::<crate::CameraInputState>().captured);
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
    fn glass_bottles_hold_to_fill_visible_water_once_without_mutating_inventory_or_world() {
        use terra_network::protocol::inventory::{InventoryAction, InventoryCommand};
        for (mode, occluded, expected) in [
            (GameMode::Survival, false, true),
            (GameMode::Creative, false, true),
            (GameMode::Survival, true, false),
            (GameMode::Spectator, false, false),
        ] {
            let mut app = app();
            session(&mut app, mode, false);
            let position = [4, 4, 4];
            let mut journal = terra_world::edits::EditJournal::default();
            journal.record(position, 27).unwrap();
            app.insert_resource(journal)
                .insert_resource(super::super::ActiveItem(Some(ItemKind::GlassBottle)))
                .insert_resource(terra_network::client::SelectedInventorySlot(Some(0)));
            app.world_mut()
                .spawn((crate::TerraCamera, Transform::from_xyz(4.5, 4.5, 8.5)));
            {
                let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
                let data = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
                data.set_block([4, 4, 4], 27).unwrap();
                if occluded {
                    data.set_block([4, 4, 6], 1).unwrap();
                }
            }
            app.world_mut()
                .resource_mut::<terra_network::client::ClientSession>()
                .inventory
                .insert(ItemKind::GlassBottle, 2);
            let before = app
                .world()
                .resource::<terra_network::client::ClientSession>()
                .inventory
                .clone();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyB);
            app.update();
            assert!(
                app.world()
                    .resource::<Messages<terra_network::client::InventoryRequest>>()
                    .is_empty()
            );
            let mut requests = Vec::new();
            for _ in 0..30 {
                app.update();
                requests.extend(
                    app.world_mut()
                        .resource_mut::<Messages<terra_network::client::InventoryRequest>>()
                        .drain(),
                );
            }
            assert_eq!(requests.len(), usize::from(expected));
            if expected {
                assert_eq!(
                    requests[0].0,
                    InventoryCommand {
                        revision: before.revision,
                        action: InventoryAction::FillBottle { slot: 0, position },
                    }
                );
            }
            assert_eq!(
                app.world()
                    .resource::<terra_network::client::ClientSession>()
                    .inventory,
                before
            );
            assert_eq!(
                app.world().resource::<ChunkStore>().block_at(position),
                Some(27)
            );
            assert!(
                app.world()
                    .resource::<Messages<BlockEditRequest>>()
                    .is_empty()
            );
        }
    }

    #[test]
    fn one_right_click_fills_buckets_and_cans_from_visible_water_without_local_mutation() {
        use terra_core::{ToolKind, farming::FarmingAction};
        for tool in [
            ToolKind::Bucket,
            ToolKind::WoodWateringCan,
            ToolKind::IronWateringCan,
        ] {
            for (mode, obstruction, fluid, expected) in [
                (GameMode::Survival, false, None, true),
                (GameMode::Creative, false, None, true),
                (GameMode::Survival, false, Some(true), true),
                (GameMode::Survival, false, Some(false), false),
                (GameMode::Survival, true, None, false),
                (GameMode::Spectator, false, None, false),
            ] {
                let mut app = app();
                session(&mut app, mode, false);
                let item = ItemKind::Tool(tool);
                let position = [4, 4, 4];
                let mut journal = terra_world::edits::EditJournal::default();
                if let Some(source) = fluid {
                    journal
                        .set_fluid(
                            position,
                            terra_world::water::FluidCell {
                                amount: 8,
                                source,
                                flow: [0; 2],
                            },
                        )
                        .unwrap();
                }
                app.insert_resource(journal)
                    .insert_resource(
                        terra_core::block_properties::BlockPropertiesCatalog::default(),
                    )
                    .insert_resource(super::super::ActiveItem(Some(item)))
                    .insert_resource(terra_network::client::SelectedInventorySlot(Some(0)));
                let mut controls = terra_core::ControlsConfig::default();
                controls.bindings.insert(
                    Action::Place,
                    terra_core::InputBinding::Mouse(MouseButton::Right),
                );
                app.insert_resource(controls);
                app.world_mut()
                    .spawn((crate::TerraCamera, Transform::from_xyz(4.5, 4.5, 8.5)));
                {
                    let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
                    let data = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
                    data.set_block([4, 4, 4], 27).unwrap();
                    if obstruction {
                        data.set_block([4, 4, 6], 1).unwrap();
                    }
                }
                app.world_mut()
                    .resource_mut::<terra_network::client::ClientSession>()
                    .inventory
                    .insert(item, 1);
                let before = app
                    .world()
                    .resource::<terra_network::client::ClientSession>()
                    .inventory
                    .clone();
                app.world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .press(MouseButton::Right);
                app.update();
                let requests: Vec<_> = app
                    .world_mut()
                    .resource_mut::<Messages<FarmToolRequest>>()
                    .drain()
                    .collect();
                assert_eq!(
                    requests.len(),
                    usize::from(expected),
                    "{tool:?}, {mode:?}, {obstruction}, {fluid:?}"
                );
                if expected {
                    assert_eq!(requests[0].position, position);
                    assert_eq!(
                        requests[0].action,
                        if tool == ToolKind::Bucket {
                            FarmingAction::FillBucket
                        } else {
                            FarmingAction::Refill
                        }
                    );
                }
                // Holding the button must not repeat a natural-water transaction.
                app.world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .clear();
                app.update();
                assert!(
                    app.world()
                        .resource::<Messages<FarmToolRequest>>()
                        .is_empty()
                );
                assert_eq!(
                    app.world()
                        .resource::<terra_network::client::ClientSession>()
                        .inventory,
                    before
                );
                assert_eq!(
                    app.world()
                        .resource::<terra_network::client::ClientSession>()
                        .watering_reserve,
                    (0, 0)
                );
                assert_eq!(
                    app.world().resource::<ChunkStore>().block_at(position),
                    Some(27)
                );
                assert!(
                    app.world()
                        .resource::<Messages<BlockEditRequest>>()
                        .is_empty()
                );
            }
        }
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
    fn sapling_preview_and_click_use_local_footprint_without_mutating_props_or_inventory() {
        let mut app = app();
        session(&mut app, GameMode::Survival, false);
        let item = ItemKind::Prop(terra_core::ResourcePropKind::OakSapling);
        app.insert_resource(super::super::ActiveItem(Some(item)));
        app.init_resource::<terra_world::edits::EditJournal>();
        app.world_mut()
            .resource_mut::<terra_network::client::ClientSession>()
            .inventory
            .insert(item, 2);
        let grass = match app.world().resource::<TargetWorld>().0.unwrap() {
            WorldTarget::Prop(hit) => hit.prop,
            _ => unreachable!(),
        };
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&grass.section)
            .unwrap()
            .props
            .push(grass);
        // A low roof used to reject placement via the thirteen-cell light test.
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&grass.section)
            .unwrap()
            .data
            .set_block([1, 4, 1], 3)
            .unwrap();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        assert!(app.world().resource::<PlacementPreview>().valid);
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<PropPlaceRequest>>()
                .drain()
                .count(),
            1
        );
        assert!(!app.world().resource::<ChunkStore>().loaded[&grass.section].props[0].removed);
        assert_eq!(
            app.world()
                .resource::<terra_network::client::ClientSession>()
                .inventory
                .count(item),
            2
        );
        assert!(
            app.world_mut()
                .resource_mut::<terra_world::edits::EditJournal>()
                .place_prop(grass)
                .unwrap()
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        assert!(!app.world().resource::<PlacementPreview>().valid);
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<PropPlaceRequest>>()
                .drain()
                .count(),
            1,
            "authority supplies a specific occupied reason instead of a silent click"
        );
        assert_eq!(
            app.world()
                .resource::<terra_network::client::ClientSession>()
                .inventory
                .count(item),
            2
        );
        assert!(!app.world().resource::<ChunkStore>().loaded[&grass.section].props[0].removed);
    }

    #[test]
    fn every_crop_seed_uses_the_rebound_place_action_without_local_inventory_changes() {
        for crop in terra_core::CropKind::ALL {
            let mut app = app();
            session(&mut app, GameMode::Survival, false);
            let item = ItemKind::Crop(crop.seed());
            app.insert_resource(super::super::ActiveItem(Some(item)));
            app.world_mut()
                .resource_mut::<terra_network::client::ClientSession>()
                .inventory
                .insert(item, 2);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyB);
            app.update();
            let requests: Vec<_> = app
                .world_mut()
                .resource_mut::<Messages<PropPlaceRequest>>()
                .drain()
                .collect();
            assert_eq!(requests.len(), 1, "{crop:?}");
            assert_eq!(requests[0].support, [1, 0, 1]);
            assert_eq!(requests[0].kind, crop.resource_prop());
            assert_eq!(
                app.world()
                    .resource::<terra_network::client::ClientSession>()
                    .inventory
                    .count(item),
                2
            );
            assert!(
                app.world()
                    .resource::<Messages<BlockEditRequest>>()
                    .is_empty()
            );
        }
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

    #[test]
    fn releasing_sneak_restarts_the_same_target_as_normal_mining_immediately() {
        let mut app = app();
        session(&mut app, GameMode::Survival, false);
        let block = app.world().resource::<TargetBlock>().0.unwrap();
        app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Block(block));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        let target = mining_requests(&mut app)[0].target;
        {
            let mut visual = app.world_mut().resource_mut::<MiningVisual>();
            visual.fell_tree = true;
            visual.held_elapsed = 12.0;
            visual.heartbeat = 0.0;
        }
        app.update();
        let requests = mining_requests(&mut app);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].target, target);
        assert!(!requests[0].fell_tree);
        assert!(app.world().resource::<MiningVisual>().held_elapsed <= 0.11);
        no_direct_removal(&app);
    }

    #[test]
    fn creative_releasing_sneak_cancels_the_tree_timer_and_restores_instant_single_mining() {
        let mut app = app();
        session(&mut app, GameMode::Creative, false);
        app.insert_resource(super::super::ActiveItem(Some(ItemKind::Tool(
            terra_core::ToolKind::WoodAxe,
        ))));
        let block = app.world().resource::<TargetBlock>().0.unwrap();
        app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Block(block));
        {
            let mut visual = app.world_mut().resource_mut::<MiningVisual>();
            visual.active = Some(MiningTarget::Block {
                position: block.position,
                expected: MaterialId::Grass as u16,
            });
            visual.fell_tree = true;
            visual.held_elapsed = 12.0;
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        let cancelled = mining_requests(&mut app);
        assert_eq!(cancelled.len(), 1);
        assert!(cancelled[0].target.is_none() && !cancelled[0].fell_tree);
        assert!(!app.world().resource::<MiningVisual>().fell_tree);
        assert!(app.world().resource::<MiningVisual>().held_elapsed < 0.11);
        let edits: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<BlockEditRequest>>()
            .drain()
            .collect();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].position, block.position);
        assert_eq!(edits[0].material, 0);
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
        app.insert_resource(super::super::ActiveItem(Some(ItemKind::Block(3))));
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
        app.insert_resource(super::super::ActiveItem(Some(ItemKind::Block(3))));
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

    #[test]
    fn empty_network_hand_never_uses_stale_palette_even_with_stock_and_offline_palette_still_works()
    {
        for mode in [GameMode::Creative, GameMode::Survival] {
            for active_resource in [false, true] {
                let mut app = app();
                session(&mut app, mode, false);
                if active_resource {
                    app.insert_resource(super::super::ActiveItem(None));
                }
                app.insert_resource(terra_network::client::SelectedInventorySlot(Some(0)));
                app.world_mut()
                    .resource_mut::<terra_network::client::ClientSession>()
                    .inventory
                    .creative_set(
                        9,
                        terra_core::ItemStack {
                            item: ItemKind::Block(3),
                            count: 32,
                        },
                    );
                assert!(
                    app.world()
                        .resource::<terra_network::client::ClientSession>()
                        .inventory
                        .slot(0)
                        .is_none()
                );
                assert_eq!(
                    app.world()
                        .resource::<terra_network::client::ClientSession>()
                        .inventory
                        .count(ItemKind::Block(3)),
                    32
                );
                assert_eq!(
                    app.world()
                        .resource::<super::super::BlockPalette>()
                        .selected()
                        .unwrap()
                        .id,
                    3
                );
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::KeyB);
                for _ in 0..3 {
                    app.update();
                    let preview = app.world().resource::<PlacementPreview>();
                    assert!(
                        preview.position.is_none() && preview.plan.is_none() && !preview.valid,
                        "{mode:?}/{active_resource}"
                    );
                    assert!(
                        app.world()
                            .resource::<Messages<BlockEditRequest>>()
                            .is_empty()
                    );
                    assert!(!app.world().resource::<InteractionMotion>().placing);
                }
                app.world_mut()
                    .remove_resource::<terra_network::client::ClientSession>();
                app.update();
                let preview = app.world().resource::<PlacementPreview>();
                assert_eq!(preview.position, Some([1, 1, 1]));
                assert!(preview.valid);
                let requests: Vec<_> = app
                    .world_mut()
                    .resource_mut::<Messages<BlockEditRequest>>()
                    .drain()
                    .collect();
                assert_eq!(requests.len(), 1);
                assert_eq!(requests[0].material, 3);
            }
        }
    }
    #[test]
    fn fruit_use_consumes_the_click_before_bucket_or_block_placement() {
        use terra_core::TreeStage;
        for (selected, stage) in [
            (
                ItemKind::Tool(terra_core::ToolKind::Bucket),
                TreeStage::Mature,
            ),
            (ItemKind::Block(3), TreeStage::Mature),
            (
                ItemKind::Tool(terra_core::ToolKind::Bucket),
                TreeStage::Generated,
            ),
            (ItemKind::Block(3), TreeStage::Generated),
        ] {
            let mut app = app();
            session(&mut app, GameMode::Survival, false);
            app.insert_resource(super::super::ActiveItem(Some(selected)));
            let mut journal = terra_world::edits::EditJournal::default();
            let tree = terra_core::TreeRecord {
                id: 41,
                root: [1, 1, 1],
                species: terra_core::TreeSpecies::Apple,
                seed: 41,
                planted_by: (stage == TreeStage::Generated)
                    .then_some(terra_core::PlayerUuid::from_u128(1)),
                stage,
                growth_seconds: 0.,
                wood: if stage == TreeStage::Generated {
                    vec![[0, 0, 0]]
                } else {
                    vec![]
                },
                leaves: vec![],
                bounds: (stage == TreeStage::Generated).then_some([[0, 0, 0]; 2]),
                fruit: Some(terra_core::FruitState {
                    phase: terra_core::FruitPhase::Ripe,
                    remaining: 8,
                    picked: 0,
                    cycle_day: 0,
                    last_harvest_day: None,
                    revision: 7,
                }),
                revision: 7,
            };
            assert!(journal.gameplay_mut().trees.insert(tree));
            app.insert_resource(journal);
            let Some(WorldTarget::Prop(mut hit)) = app.world().resource::<TargetWorld>().0 else {
                panic!("prop fixture");
            };
            hit.prop.kind = PropKind::AppleFruit;
            hit.prop.id = terra_world::fruit::prop_id(41, 2);
            hit.prop.variant = 0;
            app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Prop(hit));
            app.world_mut()
                .resource_mut::<terra_network::client::ClientSession>()
                .inventory
                .insert(selected, 1);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyB);
            app.update();
            let requests = app
                .world_mut()
                .resource_mut::<Messages<terra_network::client::TreeHarvestRequest>>()
                .drain()
                .collect::<Vec<_>>();
            assert_eq!(requests.len(), 1);
            assert_eq!(
                (requests[0].id, requests[0].slot, requests[0].fruit_revision),
                (41, 2, 7)
            );
            assert!(
                app.world()
                    .resource::<Messages<BlockEditRequest>>()
                    .is_empty()
            );
            assert!(
                app.world()
                    .resource::<Messages<FarmToolRequest>>()
                    .is_empty()
            );
            assert!(
                app.world()
                    .resource::<Messages<PropPlaceRequest>>()
                    .is_empty()
            );
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .clear();
            app.update();
            assert!(
                app.world()
                    .resource::<Messages<terra_network::client::TreeHarvestRequest>>()
                    .is_empty()
            );
        }
    }
}
