//! Equipped objects reuse the drop cache, with a lower-edge first-person grip.
use super::{BuildingWorld, drops::DropAssets};
use bevy::prelude::*;
use terra_core::{ItemKind, PlayerId};
use terra_player::{CameraPerspective, TerraCamera};

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
    let tool = matches!(item, ItemKind::Tool(_) | ItemKind::Stick);
    let half_height = 0.55 * tangent;
    let mut scale = (if tool { 1.55 } else { 1.2 }) * tangent / (65.0_f32.to_radians() * 0.5).tan();
    let rotation = Quat::from_euler(
        EulerRot::XYZ,
        -0.12 - swing * 0.75,
        if tool {
            std::f32::consts::FRAC_PI_2 - 0.4
        } else {
            -0.4
        },
        -0.18 + swing * 0.22,
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
            + camera.rotation * Vec3::new(half_height * aspect * (0.68 - swing * 0.10), y, z),
    )
    .with_rotation(camera.rotation * rotation)
    .with_scale(Vec3::splat(scale))
}

fn third_person_pose(item: ItemKind) -> Transform {
    let handle = matches!(
        item,
        ItemKind::Tool(_) | ItemKind::Stick | ItemKind::Block(40)
    );
    let grip = Vec3::new(
        if handle { 0.0 } else { 0.035 },
        if handle { -0.065 } else { -0.075 },
        0.025,
    );
    let palm = Vec3::new(0.008, -0.54 - crate::players::model::ELBOW, -0.028);
    let rotation = Quat::from_rotation_x(-0.25);
    Transform::from_translation(palm - rotation * grip * 1.4)
        .with_rotation(rotation)
        .with_scale(Vec3::splat(1.4))
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
    perspective: Res<CameraPerspective>,
    swing: Option<Res<crate::players::HandSwing>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    config: Option<Res<terra_core::ControlsConfig>>,
    cameras: Query<(&Transform, Option<&Projection>), (With<TerraCamera>, Without<HeldItem>)>,
    arms: Query<(Entity, &ChildOf, &crate::players::AvatarPart)>,
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
    if *perspective == CameraPerspective::FirstPerson
        && !world.blocked.0
        && let (Some(item), Some((camera, projection))) = (local, cameras.iter().next())
        && assets.prepare(item, &world.blocks)
        && let Some(bounds) = assets.bounds(item)
    {
        let mut pose = first_person_pose(
            camera,
            projection,
            swing.as_ref().map_or(0.0, |s| s.amount),
            item,
            bounds,
        );
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
        desired.push((None, item, None, pose));
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
            (None, local)
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
            desired.push((owner, item, Some(arm), third_person_pose(item)));
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
        if !assets.prepare(item, &world.blocks) {
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
        let root = root.id();
        assets.spawn(&mut commands, root, item, &world.blocks, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn tools_are_turned_a_quarter_turn_relative_to_blocks() {
        let bounds = (Vec3::splat(-0.12), Vec3::splat(0.12));
        let block = first_person_pose(&Transform::IDENTITY, None, 0.0, ItemKind::Block(3), bounds);
        let tool = first_person_pose(
            &Transform::IDENTITY,
            None,
            0.0,
            ItemKind::Tool(terra_core::ToolKind::WoodSword),
            bounds,
        );
        assert!(
            (block.rotation.angle_between(tool.rotation) - std::f32::consts::FRAC_PI_2).abs()
                < 1e-5
        );
    }

    #[test]
    fn third_person_tools_are_gripped_at_handle_not_flame_or_tool_head() {
        for item in [
            ItemKind::Block(40),
            ItemKind::Tool(terra_core::ToolKind::WoodSword),
        ] {
            let pose = third_person_pose(item);
            assert!(
                pose.transform_point(Vec3::new(0.0, -0.065, 0.025))
                    .abs_diff_eq(
                        Vec3::new(0.008, -0.54 - crate::players::model::ELBOW, -0.028),
                        1e-5
                    )
            );
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
        // The quarter-turned tool combines pitch and roll around closer axes.
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
