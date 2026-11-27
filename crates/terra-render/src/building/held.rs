//! Equipped objects reuse the drop cache, with a lower-edge first-person grip.
use super::{BuildingWorld, drops::DropAssets};
use bevy::prelude::*;
use terra_core::farming::FarmToolKind;
use terra_core::{ItemKind, PlayerId};
use terra_player::{CameraPerspective, TerraCamera};

#[path = "vessels.rs"]
mod vessels;
pub(super) fn install(app: &mut App) {
    vessels::install(app);
}

fn farm_kind(item: ItemKind) -> Option<FarmToolKind> {
    item.tool_kind().and_then(|tool| tool.farm_kind())
}

#[derive(Component)]
pub(super) struct HeldItem {
    owner: Option<PlayerId>,
    item: ItemKind,
    parent: Option<Entity>,
}

fn first_person_pose(
    camera: &Transform,
    projection: Option<&Projection>,
    swing: f32,
    item: ItemKind,
    bounds: (Vec3, Vec3),
) -> Transform {
    let (fov, aspect) = match projection {
        Some(Projection::Perspective(p)) => (p.fov, p.aspect_ratio),
        _ => (65.0_f32.to_radians(), 16.0 / 9.0),
    };
    let tangent = (fov * 0.5).tan();
    let tool = item.tool_kind().is_some() || item == ItemKind::Stick;
    let scythe = farm_kind(item) == Some(FarmToolKind::Scythe);
    let watering = farm_kind(item) == Some(FarmToolKind::WateringCan);
    let half_height = 0.55 * tangent;
    let view_scale = if item.tool_kind().is_some() || vessels::is_vessel(item) {
        0.46 / (bounds.1.y - bounds.0.y).max(0.01)
    } else if tool {
        1.55
    } else {
        1.2
    };
    let mut scale = view_scale * tangent / (65.0_f32.to_radians() * 0.5).tan();
    let rotation = Quat::from_euler(
        EulerRot::XYZ,
        if vessels::is_vessel(item) {
            0.40 - swing * 0.12
        } else {
            -0.12 - swing * if scythe || watering { 0.20 } else { 0.75 }
        },
        if item.tool_kind().is_some() {
            std::f32::consts::FRAC_PI_2 + 0.10 + if scythe { swing * 0.95 } else { 0.0 }
        } else if tool {
            -0.30
        } else {
            -0.4
        },
        -0.18 + swing * if watering { -0.65 } else { 0.22 },
    );
    let z = -0.55 - swing * 0.20;
    let mut nearest = 0.0_f32;
    for x in [bounds.0.x, bounds.1.x] {
        for y in [bounds.0.y, bounds.1.y] {
            for z in [bounds.0.z, bounds.1.z] {
                nearest = nearest.max((rotation * Vec3::new(x, y, z)).z);
            }
        }
    }
    if nearest > 0.0 {
        scale = scale.min((-z - 0.12) / nearest);
    }
    // Place the actual model's lower support below the screen edge, including
    // thin items. Cached bounds avoid scanning mesh vertices every frame.
    let support = rotation
        * Vec3::new(
            (bounds.0.x + bounds.1.x) * 0.5,
            bounds.0.y,
            (bounds.0.z + bounds.1.z) * 0.5,
        )
        * scale;
    let y = 1.12 * (z + support.z) * tangent - support.y;
    Transform::from_translation(
        camera.translation
            + camera.rotation
                * Vec3::new(
                    half_height * aspect * (if tool { 0.48 } else { 0.68 } - swing * 0.10)
                        - if scythe { swing * 0.10 } else { 0.0 },
                    y,
                    z,
                ),
    )
    .with_rotation(camera.rotation * rotation)
    .with_scale(Vec3::splat(scale))
}

fn third_person_pose(item: ItemKind) -> Transform {
    let palm = Vec3::new(0.008, -0.54 - crate::players::model::ELBOW, -0.028);
    hand_pose(item, palm)
}

fn hand_pose(item: ItemKind, palm: Vec3) -> Transform {
    let rotation = Quat::from_rotation_x(-0.25)
        * if item.tool_kind().is_some() {
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
        } else {
            Quat::IDENTITY
        };
    if let Some(grip) = crate::previews::tools::grip(item) {
        return Transform::from_translation(palm - rotation * grip).with_rotation(rotation);
    }
    let handle = matches!(
        item.preview_item(),
        ItemKind::Tool(_)
            | ItemKind::Stick
            | ItemKind::Block(40 | terra_core::block_shapes::VIOLET_TORCH)
    );
    let grip = Vec3::new(
        if handle { 0.0 } else { 0.035 },
        if handle { -0.065 } else { -0.075 },
        0.025,
    );
    Transform::from_translation(palm - rotation * grip * 1.4)
        .with_rotation(rotation)
        .with_scale(Vec3::splat(1.4))
}

fn third_person_action_pose(item: ItemKind, swing: f32) -> Transform {
    action_pose(item, swing, third_person_pose(item))
}

fn hand_action_pose(item: ItemKind, swing: f32, palm: Vec3) -> Transform {
    action_pose(item, swing, hand_pose(item, palm))
}

fn action_pose(item: ItemKind, swing: f32, mut pose: Transform) -> Transform {
    if swing > 0.0
        && let Some(grip) = crate::previews::tools::grip(item)
    {
        let grip_position = pose.transform_point(grip);
        let motion = match farm_kind(item) {
            Some(FarmToolKind::Scythe) => Quat::from_rotation_y(-swing * 0.95),
            Some(FarmToolKind::WateringCan) => Quat::from_rotation_z(-swing * 0.65),
            _ => return pose,
        };
        pose.rotation *= motion;
        pose.translation = grip_position - pose.rotation * grip;
    }
    pose
}

fn first_person_eating_pose(
    idle: Transform,
    camera: &Transform,
    progress: f32,
    duration: f32,
    bounds: (Vec3, Vec3),
) -> Transform {
    let (blend, bite) = crate::players::eating_motion(progress, duration);
    let rotation = Quat::from_euler(EulerRot::XYZ, -0.34 + bite * 0.06, -0.50, 0.12);
    let position = Vec3::new(0.075 + bite * 0.009, -0.16 + bite * 0.014, -0.43);
    let mut scale = idle.scale.x;
    for x in [bounds.0.x, bounds.1.x] {
        for y in [bounds.0.y, bounds.1.y] {
            for z in [bounds.0.z, bounds.1.z] {
                let nearest = (rotation * Vec3::new(x, y, z)).z;
                if nearest > 0.0 {
                    scale = scale.min((-position.z - 0.12) / nearest);
                }
            }
        }
    }
    let mut pose = Transform {
        translation: idle
            .translation
            .lerp(camera.translation + camera.rotation * position, blend),
        rotation: idle.rotation.slerp(camera.rotation * rotation, blend),
        scale: idle.scale.lerp(Vec3::splat(scale), blend),
    };
    // A corner can rotate past the near plane between two individually safe endpoints.
    let mut nearest = f32::NEG_INFINITY;
    for x in [bounds.0.x, bounds.1.x] {
        for y in [bounds.0.y, bounds.1.y] {
            for z in [bounds.0.z, bounds.1.z] {
                let local = camera.rotation.inverse()
                    * (pose.transform_point(Vec3::new(x, y, z)) - camera.translation);
                nearest = nearest.max(local.z);
            }
        }
    }
    pose.translation -= camera.rotation * Vec3::Z * (nearest + 0.12).max(0.0);
    pose
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn render(
    mut commands: Commands,
    world: BuildingWorld,
    mut assets: DropAssets,
    selected: Option<Res<terra_network::client::SelectedInventorySlot>>,
    remotes: Option<Res<terra_network::client::ReplicatedPlayers>>,
    (perspective, hud): (
        Res<CameraPerspective>,
        Option<Res<terra_core::HudVisibility>>,
    ),
    swing: Option<Res<crate::players::HandSwing>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    config: Option<Res<terra_core::ControlsConfig>>,
    (vessel, mut pour): (
        Option<Res<terra_player::interaction::VesselMotion>>,
        Local<(Option<ItemKind>, f32)>,
    ),
    cameras: Query<(&Transform, Option<&Projection>), (With<TerraCamera>, Without<HeldItem>)>,
    arms: Query<(Entity, &ChildOf, &crate::players::AvatarPart)>,
    villager_hands: Query<(Entity, &crate::creatures::VillagerHand)>,
    avatars: Query<(
        Option<&crate::players::LocalAvatar>,
        Option<&crate::players::RemoteAvatar>,
    )>,
    mut items: Query<(Entity, &HeldItem, &mut Transform), Without<TerraCamera>>,
) {
    let use_held = keys.is_some_and(|keys| {
        terra_core::GameplayInput {
            keys,
            mouse,
            config,
        }
        .consume_pressed()
    });
    let mut desired = Vec::new();
    let local = world
        .session
        .as_ref()
        .filter(|s| {
            s.ready
                && !s.dead()
                && s.sleeping().is_none()
                && s.game_mode != terra_core::GameMode::Spectator
        })
        .and_then(|s| {
            selected
                .as_ref()?
                .0
                .and_then(|i| s.inventory.slot(usize::from(i)))
        })
        .map(|s| s.item);
    let local_model = if local == Some(ItemKind::Apple)
        && use_held
        && world
            .session
            .as_ref()
            .and_then(|s| s.display_consumption())
            .is_some_and(|c| c.item == ItemKind::Apple && c.progress >= 0.72)
    {
        Some(ItemKind::AppleCore)
    } else {
        local
    };
    let pour = &mut *pour;
    if pour.0 != local {
        *pour = (local, 0.0);
    }
    let pouring = vessel
        .as_deref()
        .is_some_and(|motion| motion.item == local && motion.pouring);
    let target = if pouring { 1.0 } else { 0.0 };
    pour.1 += (target - pour.1) * (1.0 - (-12.0 * world.time.delta_secs().min(0.1)).exp());
    if *perspective == CameraPerspective::FirstPerson
        && hud.as_ref().is_none_or(|hud| hud.visible)
        && !world.blocked.0
        && let (Some(item), Some((camera, projection))) = (local, cameras.iter().next())
        && assets.prepare(vessels::base(local_model.unwrap_or(item)), &world.blocks)
        && let Some(bounds) = assets.bounds(vessels::base(local_model.unwrap_or(item)))
    {
        let mut pose = first_person_pose(
            camera,
            projection,
            swing.as_ref().map_or(0.0, |s| s.amount),
            item,
            bounds,
        );
        pose = vessels::action_pose(pose, item, pour.1);
        if vessels::is_vessel(item) {
            pose = vessels::camera_safe(pose, camera, bounds);
        }
        if let Some(consumption) = world
            .session
            .as_ref()
            .and_then(|session| session.display_consumption())
            .filter(|consumption| {
                use_held
                    && consumption.item == item
                    && selected.as_ref().and_then(|selected| selected.0) == Some(consumption.slot)
            })
        {
            pose = first_person_eating_pose(
                pose,
                camera,
                consumption.progress,
                consumption.duration,
                bounds,
            );
        }
        desired.push((None, local_model.unwrap_or(item), None, pose));
    }
    for (arm, parent, part) in &arms {
        if *part != crate::players::AvatarPart::Forearm(1) {
            continue;
        }
        let Ok((_, shoulder, _)) = arms.get(parent.parent()) else {
            continue;
        };
        let Ok((is_local, remote)) = avatars.get(shoulder.parent()) else {
            continue;
        };
        let (owner, item) = if is_local.is_some() {
            (None, local_model)
        } else if let Some(remote) = remote {
            if world.session.as_ref().is_some_and(|session| {
                session
                    .sleepers
                    .iter()
                    .any(|sleep| sleep.player == remote.0)
            }) {
                continue;
            }
            (
                Some(remote.0),
                remotes
                    .as_ref()
                    .and_then(|p| p.players.get(&remote.0))
                    .and_then(|t| t.current.held_item),
            )
        } else {
            continue;
        };
        if let Some(item) = item {
            let amount = if owner.is_none() {
                swing.as_ref().map_or(0.0, |s| s.amount)
            } else {
                0.0
            };
            desired.push((
                owner,
                item,
                Some(arm),
                vessels::action_pose(
                    third_person_action_pose(item, amount),
                    item,
                    if owner.is_none() { pour.1 } else { 0.0 },
                ),
            ));
        }
    }
    for (arm, hand) in &villager_hands {
        if hand.visible {
            desired.push((
                None,
                hand.item,
                Some(arm),
                hand_action_pose(hand.item, hand.swing, crate::creatures::VillagerHand::PALM),
            ));
        }
    }
    for (entity, held, mut transform) in &mut items {
        if let Some(i) = desired.iter().position(|(owner, item, parent, _)| {
            *owner == held.owner && *item == held.item && *parent == held.parent
        }) {
            *transform = desired.swap_remove(i).3;
        } else {
            commands.entity(entity).despawn();
        }
    }
    for (owner, item, parent, transform) in desired {
        if !assets.prepare(vessels::base(item), &world.blocks) {
            continue;
        }
        let mut root = commands.spawn((
            HeldItem {
                owner,
                item,
                parent,
            },
            transform,
            Visibility::default(),
            Name::new("Equipped item"),
        ));
        if let Some(parent) = parent {
            root.insert(ChildOf(parent));
        }
        let villager_arm = parent.filter(|&parent| villager_hands.get(parent).is_ok());
        if let Some(arm) = villager_arm {
            root.insert(crate::creatures::VillagerHeldItem { item, arm });
        }
        let root = root.id();
        if villager_arm.is_some() {
            assets.spawn_creature(&mut commands, root, vessels::base(item), &world.blocks);
        } else {
            assets.spawn(
                &mut commands,
                root,
                vessels::base(item),
                &world.blocks,
                false,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_hud_removes_only_first_person_equipment_and_restores_current_item_after_inventory() {
        let mut app = super::super::tests::app();
        app.init_resource::<terra_core::HudVisibility>()
            .insert_resource(terra_network::client::SelectedInventorySlot(Some(0)));
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        {
            let mut blocks = app.world_mut().resource_mut::<crate::blocks::BlockAssets>();
            blocks.templates = Some(std::sync::Arc::new(crate::blocks::TemplateSet::default()));
            blocks.materials = vec![material];
        }
        app.world_mut().spawn((TerraCamera, Transform::default()));
        let viewmodels = |app: &mut App| {
            app.world_mut()
                .query::<&HeldItem>()
                .iter(app.world())
                .filter(|item| item.owner.is_none() && item.parent.is_none())
                .map(|item| item.item)
                .collect::<Vec<_>>()
        };
        for item in [
            ItemKind::Prop(terra_core::ResourcePropKind::OakSapling),
            ItemKind::Tool(terra_core::ToolKind::Bucket),
            ItemKind::Tool(terra_core::ToolKind::WoodWateringCan)
                .with_watering_units(1)
                .unwrap(),
        ] {
            app.world_mut()
                .resource_mut::<terra_network::client::ClientSession>()
                .inventory
                .creative_set(0, terra_core::ItemStack { item, count: 1 });
            app.world_mut()
                .resource_mut::<terra_core::HudVisibility>()
                .visible = true;
            app.update();
            assert_eq!(viewmodels(&mut app), vec![item]);
            app.world_mut()
                .resource_mut::<terra_core::HudVisibility>()
                .visible = false;
            app.update();
            assert!(
                viewmodels(&mut app).is_empty(),
                "hidden HUD retained equipped {item:?}"
            );
        }
        let avatar = app
            .world_mut()
            .spawn((crate::players::LocalAvatar, Transform::default()))
            .id();
        let shoulder = app
            .world_mut()
            .spawn((
                crate::players::AvatarPart::Arm(1),
                Transform::default(),
                ChildOf(avatar),
            ))
            .id();
        let forearm = app
            .world_mut()
            .spawn((
                crate::players::AvatarPart::Forearm(1),
                Transform::default(),
                ChildOf(shoulder),
            ))
            .id();
        for perspective in [
            CameraPerspective::ThirdPerson,
            CameraPerspective::ThirdPersonFront,
        ] {
            *app.world_mut().resource_mut::<CameraPerspective>() = perspective;
            app.update();
            assert!(viewmodels(&mut app).is_empty());
            assert!(
                app.world_mut()
                    .query::<&HeldItem>()
                    .iter(app.world())
                    .any(|held| held.parent == Some(forearm)),
                "HUD toggle hid the real third-person avatar equipment"
            );
        }
        *app.world_mut().resource_mut::<CameraPerspective>() = CameraPerspective::FirstPerson;
        app.world_mut()
            .resource_mut::<terra_player::PlayerInputBlock>()
            .0 = true;
        app.world_mut()
            .resource_mut::<terra_core::HudVisibility>()
            .visible = true;
        app.update();
        assert!(
            viewmodels(&mut app).is_empty(),
            "opening an interactive inventory exposed a viewmodel"
        );
        assert!(
            app.world_mut()
                .query::<&HeldItem>()
                .iter(app.world())
                .any(|held| held.parent == Some(forearm)),
            "inventory miniature lost its equipped item"
        );
        app.world_mut()
            .resource_mut::<terra_player::PlayerInputBlock>()
            .0 = false;
        app.update();
        assert_eq!(viewmodels(&mut app).len(), 1);
    }

    #[test]
    fn villagers_share_player_tool_grips_scale_and_action_orientation() {
        let player_palm = Vec3::new(0.008, -0.54 - crate::players::model::ELBOW, -0.028);
        let villager_palm = crate::creatures::VillagerHand::PALM;
        for tool in terra_core::ToolKind::ALL {
            let item = ItemKind::Tool(tool);
            let grip = crate::previews::tools::grip(item).unwrap();
            for swing in [0.0, 0.5, 1.0] {
                let player = third_person_action_pose(item, swing);
                let villager = hand_action_pose(item, swing, villager_palm);
                assert!(villager.rotation.abs_diff_eq(player.rotation, 1e-6));
                assert_eq!(villager.scale, player.scale);
                assert!(
                    villager
                        .transform_point(grip)
                        .abs_diff_eq(villager_palm, 1e-5)
                );
                assert!(player.transform_point(grip).abs_diff_eq(player_palm, 1e-5));
                assert!(
                    (villager.translation - player.translation)
                        .abs_diff_eq(villager_palm - player_palm, 1e-5)
                );
            }
        }
    }

    #[test]
    fn villager_items_reuse_player_mesh_handles_follow_the_arm_and_hide_for_sleep() {
        use crate::creatures::{VillagerHand, VillagerHeldItem};
        use bevy::ecs::system::RunSystemOnce;
        let mut app = super::super::tests::app();
        app.add_plugins(TransformPlugin);
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        {
            let mut blocks = app.world_mut().resource_mut::<crate::blocks::BlockAssets>();
            blocks.templates = Some(std::sync::Arc::new(crate::blocks::TemplateSet::default()));
            blocks.materials = vec![material];
        }
        let item = ItemKind::Tool(terra_core::ToolKind::IronPickaxe);
        let arm = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.3, 1.15, 0.0),
                Visibility::Inherited,
                VillagerHand {
                    item,
                    work_only: false,
                    visible: true,
                    swing: 0.0,
                },
            ))
            .id();
        app.update();
        let held = app
            .world_mut()
            .query_filtered::<Entity, With<VillagerHeldItem>>()
            .single(app.world())
            .unwrap();
        assert_eq!(app.world().get::<ChildOf>(held).unwrap().parent(), arm);
        assert_eq!(
            app.world().get::<VillagerHeldItem>(held).unwrap().item,
            item
        );
        let npc_parts: Vec<_> = app.world().get::<Children>(held).unwrap().iter().collect();
        let mesh_count = app.world().resource::<Assets<Mesh>>().len();
        let player = app
            .world_mut()
            .run_system_once(
                move |mut commands: Commands,
                      mut assets: DropAssets,
                      blocks: Res<crate::blocks::BlockAssets>| {
                    assert!(assets.prepare(item, &blocks));
                    let root = commands
                        .spawn((Transform::default(), Visibility::Inherited))
                        .id();
                    assets.spawn(&mut commands, root, item, &blocks, false);
                    root
                },
            )
            .unwrap();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), mesh_count);
        let player_parts: Vec<_> = app
            .world()
            .get::<Children>(player)
            .unwrap()
            .iter()
            .collect();
        assert_eq!(npc_parts.len(), player_parts.len());
        for (npc, player) in npc_parts.into_iter().zip(player_parts) {
            assert_eq!(
                app.world().get::<Mesh3d>(npc).unwrap().id(),
                app.world().get::<Mesh3d>(player).unwrap().id()
            );
            assert_eq!(
                app.world()
                    .get::<MeshMaterial3d<StandardMaterial>>(npc)
                    .unwrap()
                    .0
                    .id(),
                app.world()
                    .get::<MeshMaterial3d<StandardMaterial>>(player)
                    .unwrap()
                    .0
                    .id()
            );
            assert_eq!(
                app.world().get::<terra_core::miniature::MiniatureMesh>(npc),
                Some(&terra_core::miniature::MiniatureMesh::Creature)
            );
        }
        let grip = crate::previews::tools::grip(item).unwrap();
        for angle in [-0.7, 0.0, 0.55] {
            let pose =
                Transform::from_xyz(0.3, 1.15, 0.0).with_rotation(Quat::from_rotation_x(angle));
            *app.world_mut().get_mut::<Transform>(arm).unwrap() = pose;
            app.update();
            let held_global = app.world().get::<GlobalTransform>(held).unwrap();
            assert!(
                held_global
                    .transform_point(grip)
                    .abs_diff_eq(pose.transform_point(VillagerHand::PALM), 1e-5)
            );
        }
        app.world_mut()
            .get_mut::<VillagerHand>(arm)
            .unwrap()
            .visible = false;
        app.update();
        assert!(
            app.world_mut()
                .query::<&VillagerHeldItem>()
                .iter(app.world())
                .next()
                .is_none()
        );
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), mesh_count);
        app.world_mut()
            .get_mut::<VillagerHand>(arm)
            .unwrap()
            .visible = true;
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&VillagerHeldItem>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), mesh_count);
    }

    #[test]
    fn real_size_tools_attach_to_the_grip_and_stay_in_front_of_the_first_person_camera() {
        for tool in terra_core::ToolKind::ALL {
            let item = ItemKind::Tool(tool);
            let length = crate::previews::tools::world_length(item).unwrap();
            let third = third_person_pose(item);
            assert_eq!(third.scale, Vec3::ONE);
            assert!(
                third
                    .transform_point(crate::previews::tools::grip(item).unwrap())
                    .abs_diff_eq(
                        Vec3::new(0.008, -0.54 - crate::players::model::ELBOW, -0.028),
                        1e-5
                    )
            );
            let bounds = (
                Vec3::new(-0.44, -0.5, -0.13) * length,
                Vec3::new(0.44, 0.5, 0.13) * length,
            );
            for fov in [50.0_f32, 65.0, 90.0, 110.0] {
                let projection = Projection::Perspective(PerspectiveProjection {
                    fov: fov.to_radians(),
                    aspect_ratio: 16.0 / 9.0,
                    ..default()
                });
                for swing in [0.0, 0.5, 1.0] {
                    let pose = first_person_pose(
                        &Transform::IDENTITY,
                        Some(&projection),
                        swing,
                        item,
                        bounds,
                    );
                    for x in [bounds.0.x, bounds.1.x] {
                        for y in [bounds.0.y, bounds.1.y] {
                            for z in [bounds.0.z, bounds.1.z] {
                                assert!(
                                    pose.transform_point(Vec3::new(x, y, z)).z < -0.119,
                                    "{tool:?}, fov={fov}, swing={swing}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn eating_lifts_food_into_view_and_keeps_it_in_front_of_the_near_plane() {
        let item = ItemKind::Animal(terra_core::creatures::AnimalProduct::CookedFish);
        let bounds = (Vec3::splat(-0.12), Vec3::splat(0.12));
        for fov in [50.0_f32, 65.0, 90.0, 110.0] {
            let projection = Projection::Perspective(PerspectiveProjection {
                fov: fov.to_radians(),
                aspect_ratio: 16.0 / 9.0,
                ..default()
            });
            let camera = Transform::from_xyz(-3.0, 17.0, 9.0).with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                -0.7,
                0.5,
                0.0,
            ));
            let idle = first_person_pose(&camera, Some(&projection), 0.0, item, bounds);
            for step in 0..=100 {
                let pose =
                    first_person_eating_pose(idle, &camera, step as f32 / 100.0, 1.6, bounds);
                for x in [bounds.0.x, bounds.1.x] {
                    for y in [bounds.0.y, bounds.1.y] {
                        for z in [bounds.0.z, bounds.1.z] {
                            let local = camera.rotation.inverse()
                                * (pose.transform_point(Vec3::new(x, y, z)) - camera.translation);
                            assert!(local.z < -0.115, "fov={fov}, step={step}, {local:?}");
                        }
                    }
                }
                if step == 50 {
                    let local = camera.rotation.inverse() * (pose.translation - camera.translation);
                    let before =
                        camera.rotation.inverse() * (idle.translation - camera.translation);
                    assert!(local.y > before.y && local.x < before.x);
                }
            }
        }
    }

    #[test]
    fn lower_item_support_stays_out_of_frame_across_fov_aspect_and_swing() {
        for fov in [50.0_f32, 65.0, 90.0, 110.0] {
            for aspect in [4.0 / 3.0, 16.0 / 9.0, 21.0 / 9.0] {
                let projection = Projection::Perspective(PerspectiveProjection {
                    fov: fov.to_radians(),
                    aspect_ratio: aspect,
                    ..default()
                });
                for item in [
                    ItemKind::Block(3),
                    ItemKind::Tool(terra_core::ToolKind::StonePickaxe),
                ] {
                    for height in [0.04, 0.12] {
                        let bounds = (
                            Vec3::new(-0.12, -height, -0.12),
                            Vec3::new(0.12, height, 0.12),
                        );
                        for swing in [0.0, 0.5, 1.0] {
                            let camera = Transform::from_xyz(4.0, 2.0, 9.0)
                                .with_rotation(Quat::from_euler(EulerRot::YXZ, 1.2, -0.4, 0.0));
                            let pose =
                                first_person_pose(&camera, Some(&projection), swing, item, bounds);
                            let support = pose.transform_point(Vec3::new(0.0, -height, 0.0));
                            let local = camera.rotation.inverse() * (support - camera.translation);
                            let ndc_y = local.y / (-local.z * (fov.to_radians() * 0.5).tan());
                            assert!((ndc_y + 1.12).abs() < 1e-4, "{ndc_y}");
                            assert!(local.z < -0.1);
                            for x in [bounds.0.x, bounds.1.x] {
                                for y in [bounds.0.y, bounds.1.y] {
                                    for z in [bounds.0.z, bounds.1.z] {
                                        let p = camera.rotation.inverse()
                                            * (pose.transform_point(Vec3::new(x, y, z))
                                                - camera.translation);
                                        assert!(
                                            p.z < -0.119,
                                            "Model crossed the near plane: {p:?}"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn equipped_tools_point_their_working_edge_into_the_scene_not_across_the_view() {
        let bounds = (Vec3::splat(-0.12), Vec3::splat(0.12));
        let tool = first_person_pose(
            &Transform::IDENTITY,
            None,
            0.0,
            ItemKind::Tool(terra_core::ToolKind::WoodSword),
            bounds,
        );
        let face = tool.rotation * Vec3::Z;
        let toward_eye = -tool.translation.normalize();
        assert!(face.dot(toward_eye).abs() < 0.85);
        assert!((tool.rotation * Vec3::X).dot(Vec3::NEG_Z) > 0.95);
        for kind in terra_core::ToolKind::ALL {
            let pose = third_person_pose(ItemKind::Tool(kind));
            assert!((pose.rotation * Vec3::X).dot(Vec3::NEG_Z) > 0.95);
        }
    }

    #[test]
    fn scythes_sweep_laterally_and_watering_cans_tip_without_losing_the_grip() {
        let bounds = (Vec3::splat(-0.25), Vec3::splat(0.25));
        let camera = Transform::IDENTITY;
        let item = ItemKind::Tool(terra_core::ToolKind::IronScythe);
        let idle = first_person_pose(&camera, None, 0.0, item, bounds);
        let cut = first_person_pose(&camera, None, 1.0, item, bounds);
        let blade = Vec3::new(0.25, 0.25, 0.0);
        assert!(cut.transform_point(blade).x < idle.transform_point(blade).x - 0.10);
        for item in [item, ItemKind::Tool(terra_core::ToolKind::IronWateringCan)] {
            let third = third_person_pose(item);
            let action = third_person_action_pose(item, 1.0);
            let grip = crate::previews::tools::grip(item).unwrap();
            assert!(
                third
                    .transform_point(grip)
                    .abs_diff_eq(action.transform_point(grip), 1e-5)
            );
            assert!(third.rotation.angle_between(action.rotation) > 0.6);
            for amount in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let pose = first_person_pose(&camera, None, amount, item, bounds);
                for x in [bounds.0.x, bounds.1.x] {
                    for y in [bounds.0.y, bounds.1.y] {
                        for z in [bounds.0.z, bounds.1.z] {
                            assert!(pose.transform_point(Vec3::new(x, y, z)).z < -0.119);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn third_person_tools_are_gripped_at_handle_not_flame_or_tool_head() {
        for item in [
            ItemKind::Block(40),
            ItemKind::Tool(terra_core::ToolKind::WoodSword),
        ] {
            let pose = third_person_pose(item);
            let grip = crate::previews::tools::grip(item).unwrap_or(Vec3::new(0.0, -0.065, 0.025));
            assert!(pose.transform_point(grip).abs_diff_eq(
                Vec3::new(0.008, -0.54 - crate::players::model::ELBOW, -0.028),
                1e-5
            ));
        }
    }
    #[test]
    fn first_person_swing_pushes_item_forward_without_moving_camera() {
        let camera = Transform::from_xyz(2.0, 3.0, 4.0);
        let item = ItemKind::Tool(terra_core::ToolKind::WoodSword);
        let bounds = (Vec3::splat(-0.12), Vec3::splat(0.12));
        let idle = first_person_pose(&camera, None, 0.0, item, bounds);
        let hit = first_person_pose(&camera, None, 1.0, item, bounds);
        assert!(hit.translation.z < idle.translation.z - 0.15);
        // The blade pitches forward with the same fixed camera transform.
        assert!(hit.rotation.angle_between(idle.rotation) > 0.45);
        assert!(
            idle.scale.x
                > first_person_pose(&camera, None, 0.0, ItemKind::Block(3), bounds)
                    .scale
                    .x
        );
        // Test the striking tip, not only the model origin, for arbitrary view yaw/pitch.
        for rotation in [
            Quat::IDENTITY,
            Quat::from_euler(EulerRot::YXZ, 1.3, 0.5, 0.0),
        ] {
            let camera = Transform::from_rotation(rotation);
            let idle = first_person_pose(&camera, None, 0.0, item, bounds);
            let hit = first_person_pose(&camera, None, 1.0, item, bounds);
            let tip = Vec3::Y * 0.12;
            assert!(
                ((hit.transform_point(tip) - idle.transform_point(tip))
                    .dot(rotation * Vec3::NEG_Z))
                    > 0.2
            );
        }
    }
}
