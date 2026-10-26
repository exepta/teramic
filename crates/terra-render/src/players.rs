//! The world avatar and inventory portrait share articulated meshes.
pub(crate) mod model;
mod riding;
pub(crate) use riding::{MountedPresentation, MountedPresentationSet};

use bevy::{camera::visibility::RenderLayers, prelude::*};
use terra_core::{PlayerGender, PlayerId, PlayerProfile};
use terra_network::client::ReplicatedPlayers;

#[derive(Component)]
pub struct RemoteAvatar(pub PlayerId);
#[derive(Component)]
pub(crate) struct LocalAvatar;
#[derive(Component)]
pub(crate) struct WorldAvatarPart(pub(crate) Entity);
#[derive(Component)]
pub(crate) struct AvatarGender(pub(crate) PlayerGender);
type LocalBodyFilter = (
    With<terra_player::PlayerBody>,
    Without<LocalAvatar>,
    Without<terra_player::TerraCamera>,
);

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AvatarPart {
    Body,
    Head,
    Arm(i8),
    Forearm(i8),
    Leg(i8),
    Calf(i8),
}

#[derive(Component)]
pub(crate) struct AvatarJoint {
    pub(crate) root: Entity,
    pub(crate) rest: Vec3,
}

#[derive(Resource)]
pub(crate) struct AvatarAssets {
    pub(crate) parts: [Vec<(AvatarPart, Vec3, Handle<Mesh>)>; 2],
    pub(crate) eyelids: Option<Handle<Mesh>>,
    pub(crate) material: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
pub(crate) struct HandSwing {
    elapsed: f32,
    impulse: u64,
    pub amount: f32,
}
impl HandSwing {
    fn advance(&mut self, dt: f32, active: bool, impulse: u64) {
        if (impulse != self.impulse || active) && self.elapsed >= 0.32 {
            self.elapsed = 0.0;
        }
        self.impulse = impulse;
        self.elapsed = (self.elapsed + dt).min(0.32);
        self.amount = (self.elapsed / 0.32 * std::f32::consts::PI).sin().max(0.0);
        if self.elapsed >= 0.32 {
            self.amount = 0.0;
        }
    }
}

#[derive(Component)]
pub struct AvatarBlink {
    elapsed: f32,
    next: f32,
    seed: u32,
}

#[derive(Component)]
pub(crate) struct AvatarEyelidPose(pub f32);

#[derive(Default)]
pub(crate) struct SpawnedAvatar {
    pub eyelids: Option<Entity>,
}

impl AvatarBlink {
    pub fn new(seed: u32) -> Self {
        Self {
            elapsed: 0.0,
            next: 2.5 + (seed % 37) as f32 * 0.1,
            seed,
        }
    }

    pub fn advance(&mut self, dt: f32) -> f32 {
        self.elapsed += dt;
        if self.elapsed < self.next {
            return 0.0;
        }
        let phase = (self.elapsed - self.next) / 0.16;
        if phase >= 1.0 {
            self.seed = self
                .seed
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            self.elapsed = 0.0;
            self.next = if self.seed.is_multiple_of(11) {
                0.23
            } else {
                3.0 + (self.seed % 41) as f32 * 0.1
            };
            0.0
        } else {
            (1.0 - (phase * 2.0 - 1.0).abs()).clamp(0.0, 1.0)
        }
    }
}

impl Default for AvatarBlink {
    fn default() -> Self {
        Self::new(0x5445_5241)
    }
}

#[derive(Component)]
pub(crate) struct AvatarMotion {
    sleeping: bool,
    mounted: bool,
    seat: f32,
    head_yaw: f32,
    riding_pitch: f32,
    previous: Vec3,
    speed: f32,
    phase: f32,
    pitch: f32,
    swim: f32,
    stroke: f32,
    water_phase: f32,
    idle_phase: f32,
}

impl AvatarMotion {
    fn new(position: Vec3) -> Self {
        Self {
            sleeping: false,
            mounted: false,
            seat: 0.0,
            head_yaw: 0.0,
            riding_pitch: 0.0,
            previous: position,
            speed: 0.0,
            phase: 0.0,
            pitch: 0.0,
            swim: 0.0,
            stroke: 0.0,
            water_phase: 0.0,
            idle_phase: position.x * 0.73 + position.z * 0.37,
        }
    }

    fn advance(&mut self, position: Vec3, dt: f32) {
        let delta = (position - self.previous).with_y(0.0).length();
        self.previous = position;
        let target = if delta < 8.0 && dt > 0.0 {
            (delta / dt).min(12.0)
        } else {
            0.0
        };
        self.speed += (target - self.speed) * (1.0 - (-dt * 12.0).exp());
        self.phase = (self.phase + self.speed * dt * 2.4).rem_euclid(std::f32::consts::TAU);
        self.idle_phase = (self.idle_phase + dt * 1.7).rem_euclid(std::f32::consts::TAU);
    }

    fn water(&mut self, wet: bool, moving: bool, dt: f32) {
        let follow = 1.0 - (-dt * 8.0).exp();
        let immersion = if wet { 1.0 } else { 0.0 };
        let propulsion = if wet && moving { 1.0 } else { 0.0 };
        self.swim += (immersion - self.swim) * follow;
        self.stroke += (propulsion - self.stroke) * follow;
        self.water_phase =
            (self.water_phase + dt * (1.8 + self.stroke * 2.5)).rem_euclid(std::f32::consts::TAU);
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<MountedPresentation>()
        .insert_resource(HandSwing {
            elapsed: 0.32,
            ..default()
        })
        .add_systems(Startup, assets)
        .add_systems(
            Update,
            riding::prepare
                .in_set(MountedPresentationSet)
                .after(terra_player::PlayerSet::Movement),
        )
        .add_systems(
            Update,
            (swing, render, local_avatar, animate, blink)
                .chain()
                .after(MountedPresentationSet)
                .after(terra_player::PlayerSet::Interaction),
        );
    crate::armor::install(app);
    crate::player_preview::install(app);
}

fn swing(
    time: Res<Time>,
    motion: Option<Res<terra_player::interaction::InteractionMotion>>,
    mut swing: ResMut<HandSwing>,
) {
    let (active, impulse) = motion.map_or((false, 0), |m| (m.swinging, m.impulse));
    swing.advance(time.delta_secs().min(0.1), active, impulse);
}

pub(crate) fn assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let parts =
        [PlayerGender::Male, PlayerGender::Female].map(|gender| match model::parts(gender) {
            Ok(parts) => parts
                .into_iter()
                .map(|(part, position, mesh)| (part, position, meshes.add(mesh)))
                .collect(),
            Err(error) => {
                tracing::error!(%error, "Cannot build shared player model");
                Vec::new()
            }
        });
    commands.insert_resource(AvatarAssets {
        parts,
        eyelids: match model::eyelids() {
            Ok(mesh) => Some(meshes.add(mesh)),
            Err(error) => {
                tracing::error!(%error, "Cannot build avatar eyelids");
                None
            }
        },
        material: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.88,
            ..default()
        }),
    });
}

pub(crate) fn render(
    mut commands: Commands,
    players: Option<Res<ReplicatedPlayers>>,
    assets: Res<AvatarAssets>,
    session: Option<Res<terra_network::client::ClientSession>>,
    mounts: Option<Res<MountedPresentation>>,
    mut roots: Query<(
        Entity,
        &RemoteAvatar,
        &AvatarGender,
        &mut Transform,
        &mut AvatarMotion,
        &mut crate::armor::AvatarEquipment,
    )>,
) {
    let Some(players) = players else {
        for (entity, _, _, _, _, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    };
    let mut present = std::collections::HashSet::new();
    for (entity, avatar, gender, mut transform, mut motion, mut equipment) in &mut roots {
        let Some(track) = players.players.get(&avatar.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        if gender.0 != track.current.gender {
            commands.entity(entity).despawn();
            continue;
        }
        present.insert(avatar.0);
        let desired = crate::armor::AvatarEquipment::from(track.current.equipment);
        if *equipment != desired {
            *equipment = desired;
        }
        transform.translation = track.position();
        transform.rotation = Quat::from_rotation_y(track.current.look[0]);
        motion.mounted = track.current.mounted.is_some();
        motion.riding_pitch = mounts
            .as_ref()
            .and_then(|m| m.remote(avatar.0))
            .map_or(0.0, |p| p.jump.pitch * 0.65);
        let mounted_yaw = mounts
            .as_ref()
            .and_then(|m| m.remote(avatar.0))
            .map(|p| p.horse.yaw)
            .or_else(|| track.current.mounted.map(|m| m.yaw));
        motion.head_yaw =
            mounted_yaw.map_or(0.0, |yaw| riding::relative_yaw(track.current.look[0], yaw));
        if let Some(yaw) = mounted_yaw {
            transform.translation = mounts
                .as_ref()
                .and_then(|m| m.remote(avatar.0))
                .map_or(transform.translation, |p| p.center);
            transform.rotation = Quat::from_rotation_y(yaw);
        }
        motion.pitch = track.current.look[1];
        let sleep = session
            .as_ref()
            .and_then(|session| {
                session
                    .sleepers
                    .iter()
                    .find(|sleep| sleep.player == avatar.0)
            })
            .copied();
        motion.sleeping = sleep.is_some();
        if let Some(sleep) = sleep {
            *transform = sleep_transform(sleep);
        }
    }
    for (&id, track) in &players.players {
        if present.contains(&id) {
            continue;
        }
        let sleep = session
            .as_ref()
            .and_then(|session| session.sleepers.iter().find(|sleep| sleep.player == id))
            .copied();
        let position = mounts
            .as_ref()
            .and_then(|m| m.remote(id))
            .map_or_else(|| track.position(), |p| p.center);
        let mut motion = AvatarMotion::new(position);
        motion.sleeping = sleep.is_some();
        motion.mounted = track.current.mounted.is_some();
        motion.riding_pitch = mounts
            .as_ref()
            .and_then(|m| m.remote(id))
            .map_or(0.0, |p| p.jump.pitch * 0.65);
        motion.seat = if motion.mounted { 1.0 } else { 0.0 };
        let mounted_yaw = mounts
            .as_ref()
            .and_then(|m| m.remote(id))
            .map(|p| p.horse.yaw)
            .or_else(|| track.current.mounted.map(|m| m.yaw));
        motion.head_yaw =
            mounted_yaw.map_or(0.0, |yaw| riding::relative_yaw(track.current.look[0], yaw));
        let root = commands
            .spawn((
                RemoteAvatar(id),
                AvatarGender(track.current.gender),
                crate::armor::AvatarEquipment::from(track.current.equipment),
                motion,
                Name::new(format!("Player {}", id.0)),
                sleep.map_or_else(
                    || {
                        Transform::from_translation(position).with_rotation(Quat::from_rotation_y(
                            mounted_yaw.unwrap_or(track.current.look[0]),
                        ))
                    },
                    sleep_transform,
                ),
                Visibility::default(),
            ))
            .id();
        spawn_parts(
            &mut commands,
            &assets,
            root,
            RenderLayers::default(),
            track.current.gender,
        );
    }
}

fn sleep_transform(sleep: terra_network::protocol::session::SleepingPlayer) -> Transform {
    Transform::from_translation(sleep.center()).with_rotation(
        Quat::from_rotation_y(f32::from(sleep.facing) * std::f32::consts::FRAC_PI_2)
            * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
    )
}

pub(crate) fn spawn_parts(
    commands: &mut Commands,
    assets: &AvatarAssets,
    root: Entity,
    layers: RenderLayers,
    gender: PlayerGender,
) -> SpawnedAvatar {
    let world = layers.intersects(&RenderLayers::default())
        || layers.intersects(&RenderLayers::layer(terra_core::miniature::WORLD_LAYER));
    let layers = if world {
        layers.with(terra_core::miniature::WORLD_LAYER)
    } else {
        layers
    };
    let mut spawned = SpawnedAvatar::default();
    let mut joints = Vec::with_capacity(10);
    for (part, position, mesh) in &assets.parts[usize::from(gender == PlayerGender::Female)] {
        let parent_part = match part {
            AvatarPart::Forearm(side) => Some(AvatarPart::Arm(*side)),
            AvatarPart::Calf(side) => Some(AvatarPart::Leg(*side)),
            _ => None,
        };
        let parent = parent_part
            .and_then(|wanted| {
                joints
                    .iter()
                    .find_map(|(part, entity)| (*part == wanted).then_some(*entity))
            })
            .unwrap_or(root);
        let entity = commands
            .spawn((
                *part,
                AvatarJoint {
                    root,
                    rest: *position,
                },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(assets.material.clone()),
                Transform::from_translation(*position),
                layers.clone(),
                ChildOf(parent),
            ))
            .id();
        if world {
            commands.entity(entity).insert((
                terra_core::miniature::MiniatureMesh::Player,
                WorldAvatarPart(root),
            ));
        }
        joints.push((*part, entity));
        if *part == AvatarPart::Head
            && let Some(mesh) = &assets.eyelids
        {
            let eyelids = commands
                .spawn((
                    AvatarBlink::new(root.index_u32()),
                    Name::new("Avatar eyelids"),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(assets.material.clone()),
                    Transform::from_xyz(0.0, model::EYE_CENTER, 0.0)
                        .with_scale(Vec3::new(1.0, 0.001, 1.0)),
                    layers.clone(),
                    ChildOf(entity),
                ))
                .id();
            if world {
                commands.entity(eyelids).insert((
                    terra_core::miniature::MiniatureMesh::Player,
                    WorldAvatarPart(root),
                ));
            }
            spawned.eyelids = Some(eyelids);
        }
    }
    spawned
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn local_avatar(
    mut commands: Commands,
    assets: Res<AvatarAssets>,
    perspective: Res<terra_player::CameraPerspective>,
    miniature: Option<Res<terra_core::miniature::MiniatureView>>,
    fixed: Res<Time<Fixed>>,
    profile: Option<Res<PlayerProfile>>,
    session: Option<Res<terra_network::client::ClientSession>>,
    mounts: Option<Res<MountedPresentation>>,
    bodies: Query<(&Transform, Option<&terra_player::CharacterPresentation>), LocalBodyFilter>,
    cameras: Query<
        &terra_player::CameraLook,
        (With<terra_player::TerraCamera>, Without<LocalAvatar>),
    >,
    mut roots: Query<
        (
            Entity,
            &mut Transform,
            &mut AvatarMotion,
            &mut crate::armor::AvatarEquipment,
        ),
        With<LocalAvatar>,
    >,
    mut parts: Query<(&WorldAvatarPart, &mut RenderLayers)>,
) {
    let body = bodies.iter().next();
    let sleep = session.as_ref().and_then(|session| session.sleeping());
    let first_person =
        *perspective == terra_player::CameraPerspective::FirstPerson && sleep.is_none();
    if body.is_none()
        || session
            .as_ref()
            .is_some_and(|s| s.game_mode == terra_core::GameMode::Spectator)
        || (first_person && !miniature.is_some_and(|view| view.active))
    {
        for (entity, _, _, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    let Some((body, presentation)) = body else {
        return;
    };
    let (yaw, pitch, _) = cameras.iter().next().map_or((0.0, 0.0, 0.0), |camera| {
        camera.rotation().to_euler(EulerRot::YXZ)
    });
    let position = mounts.as_ref().and_then(|m| m.local()).map_or_else(
        || {
            presentation.map_or(body.translation, |presentation| {
                presentation.visual_position(body.translation, fixed.overstep_fraction())
            })
        },
        |p| p.center,
    );
    let mounted_yaw = mounts
        .as_ref()
        .and_then(|m| m.local())
        .map(|p| p.horse.yaw)
        .or_else(|| session.as_ref().and_then(|s| s.mounted).map(|m| m.yaw));
    let transform = sleep.map_or_else(
        || {
            Transform::from_translation(position)
                .with_rotation(Quat::from_rotation_y(mounted_yaw.unwrap_or(yaw)))
        },
        sleep_transform,
    );
    let layers = if first_person {
        RenderLayers::layer(terra_core::miniature::WORLD_LAYER)
    } else {
        crate::miniature::layers()
    };
    if roots.is_empty() {
        let mut motion = AvatarMotion::new(position);
        motion.sleeping = sleep.is_some();
        motion.mounted = mounted_yaw.is_some();
        motion.riding_pitch = mounts
            .as_ref()
            .and_then(|m| m.local())
            .map_or(0.0, |p| p.jump.pitch * 0.65);
        motion.seat = if motion.mounted { 1.0 } else { 0.0 };
        motion.head_yaw = mounted_yaw.map_or(0.0, |horse| riding::relative_yaw(yaw, horse));
        let root = commands
            .spawn((
                LocalAvatar,
                AvatarGender(
                    profile
                        .as_ref()
                        .map_or(PlayerGender::default(), |p| p.gender),
                ),
                crate::armor::AvatarEquipment::from(
                    session
                        .as_ref()
                        .map_or(terra_core::EquipmentSlots::default(), |session| {
                            session.stats.equipment
                        }),
                ),
                motion,
                transform,
                Visibility::default(),
                Name::new("Local player avatar"),
            ))
            .id();
        spawn_parts(
            &mut commands,
            &assets,
            root,
            layers,
            profile
                .as_ref()
                .map_or(PlayerGender::default(), |p| p.gender),
        );
    } else {
        for (root, mut current, mut motion, mut equipment) in &mut roots {
            let desired = crate::armor::AvatarEquipment::from(
                session
                    .as_ref()
                    .map_or(terra_core::EquipmentSlots::default(), |session| {
                        session.stats.equipment
                    }),
            );
            if *equipment != desired {
                *equipment = desired;
            }
            *current = transform;
            motion.pitch = pitch;
            motion.sleeping = sleep.is_some();
            motion.mounted = mounted_yaw.is_some();
            motion.riding_pitch = mounts
                .as_ref()
                .and_then(|m| m.local())
                .map_or(0.0, |p| p.jump.pitch * 0.65);
            motion.head_yaw = mounted_yaw.map_or(0.0, |horse| riding::relative_yaw(yaw, horse));
            for (part, mut current_layers) in &mut parts {
                if part.0 == root && *current_layers != layers {
                    *current_layers = layers.clone();
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn animate(
    time: Res<Time>,
    mut roots: Query<(Entity, &mut Transform, &mut AvatarMotion), Without<AvatarPart>>,
    mut parts: Query<(&AvatarJoint, &AvatarPart, &mut Transform), Without<AvatarMotion>>,
    terrain: Option<Res<terra_world::streaming::WorldTerrain>>,
    chunks: Option<Res<terra_world::streaming::ChunkStore>>,
    journal: Option<Res<terra_world::edits::EditJournal>>,
    swing: Res<HandSwing>,
    local: Query<(), With<LocalAvatar>>,
    remotes: Query<&RemoteAvatar>,
    players: Option<Res<ReplicatedPlayers>>,
    session: Option<Res<terra_network::client::ClientSession>>,
    selected: Option<Res<terra_network::client::SelectedInventorySlot>>,
    keys: terra_core::GameplayInput,
    blocked: Option<Res<terra_player::PlayerInputBlock>>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (entity, mut transform, mut motion) in &mut roots {
        let seat = if motion.mounted { 1.0 } else { 0.0 };
        motion.seat += (seat - motion.seat) * (1.0 - (-dt * 12.0).exp());
        if motion.sleeping {
            motion.previous = transform.translation;
            motion.speed = 0.0;
            motion.phase = 0.0;
            motion.swim = 0.0;
            motion.stroke = 0.0;
            motion.pitch = 0.0;
            continue;
        }
        motion.advance(transform.translation, dt);
        if motion.mounted {
            motion.swim = 0.0;
            motion.stroke = 0.0;
            riding::sit(&mut transform, motion.riding_pitch);
            continue;
        }
        let wet = match (&terrain, &chunks, &journal) {
            (Some(t), Some(c), Some(j)) => terra_world::creatures::Habitat {
                terrain: &t.0,
                chunks: c,
                journal: j,
            }
            .wet(transform.translation.to_array().map(f64::from)),
            _ => false,
        };
        let moving = if local.contains(entity) {
            !blocked.as_ref().is_some_and(|b| b.0)
                && [
                    terra_core::GameAction::Forward,
                    terra_core::GameAction::Backward,
                    terra_core::GameAction::Left,
                    terra_core::GameAction::Right,
                ]
                .into_iter()
                .any(|a| keys.pressed(a))
        } else {
            motion.speed > 0.35
        };
        motion.water(wet, moving, dt);
        let gait = (motion.speed / 5.0).clamp(0.0, 1.0);
        let yaw = transform.rotation.to_euler(EulerRot::YXZ).0;
        transform.rotation = Quat::from_euler(
            EulerRot::YXZ,
            yaw,
            -gait * 0.055 * (1.0 - motion.swim),
            motion.phase.sin() * gait * 0.025 * (1.0 - motion.swim),
        );
        transform.translation.y +=
            (motion.phase.cos().abs() - 0.5) * gait * 0.025 * (1.0 - motion.swim);
        if motion.swim > 0.001 {
            let yaw = transform.rotation.to_euler(EulerRot::YXZ).0;
            transform.rotation = Quat::from_rotation_y(yaw)
                * Quat::from_rotation_x((-1.2 + motion.pitch * 0.4) * motion.swim * motion.stroke);
            transform.translation.y +=
                motion.water_phase.sin() * 0.015 * motion.swim * (1.0 - motion.stroke);
        }
    }
    for (joint, part, mut transform) in &mut parts {
        let Ok((_, _, motion)) = roots.get(joint.root) else {
            continue;
        };
        if motion.sleeping {
            transform.rotation = Quat::IDENTITY;
            transform.translation = joint.rest;
            transform.scale = Vec3::ONE;
            continue;
        }
        let stride = motion.phase.sin() * (motion.speed / 5.0).clamp(0.0, 1.0) * 0.52;
        let attack = if local.contains(joint.root) {
            swing.amount
        } else {
            remotes
                .get(joint.root)
                .ok()
                .and_then(|a| players.as_ref()?.players.get(&a.0))
                .filter(|t| t.current.mining)
                .map_or(0.0, |_| {
                    (time.elapsed_secs() * std::f32::consts::PI / 0.32)
                        .sin()
                        .abs()
                })
        };
        let consumption = if local.contains(joint.root) {
            session
                .as_ref()
                .filter(|session| session.ready && !session.dead())
                .and_then(|session| session.display_consumption())
                .filter(|consumption| {
                    !blocked.as_ref().is_some_and(|blocked| blocked.0)
                        && keys.consume_pressed()
                        && selected.as_ref().and_then(|selected| selected.0)
                            == Some(consumption.slot)
                        && session
                            .as_ref()
                            .and_then(|session| {
                                session.inventory.slot(usize::from(consumption.slot))
                            })
                            .is_some_and(|stack| stack.item == consumption.item)
                })
        } else {
            remotes
                .get(joint.root)
                .ok()
                .and_then(|avatar| players.as_ref()?.players.get(&avatar.0))
                .and_then(|track| track.display_consumption())
        };
        let (eating, bite) =
            consumption.map_or((0.0, 0.0), |c| eating_motion(c.progress, c.duration));
        let standing = match part {
            AvatarPart::Head => Quat::from_euler(
                EulerRot::YXZ,
                motion.idle_phase.sin() * 0.012 * (1.0 - motion.swim),
                motion.pitch.clamp(-0.65, 0.65) * (1.0 - motion.swim * (1.0 - motion.stroke) * 0.7),
                -motion.phase.sin() * (motion.speed / 5.0).min(1.0) * 0.012,
            ),
            AvatarPart::Arm(side) => {
                let pose = arm_pose(*side, stride, motion, attack);
                if *side == 1 {
                    pose.slerp(eating_arm_pose(bite), eating)
                } else {
                    pose
                }
            }
            AvatarPart::Forearm(side) => {
                let pose = forearm_pose(*side, motion, attack);
                if *side == 1 {
                    pose.slerp(eating_forearm_pose(bite), eating)
                } else {
                    pose
                }
            }
            AvatarPart::Leg(side) => leg_pose(*side, stride, motion),
            AvatarPart::Calf(side) => calf_pose(*side, motion),
            AvatarPart::Body => Quat::from_rotation_y(
                -motion.phase.sin() * (motion.speed / 5.0).min(1.0) * 0.035 * (1.0 - motion.swim),
            ),
        };
        transform.rotation = standing.slerp(seated_pose(*part, motion, attack), motion.seat);
        transform.translation = joint.rest;
        if let AvatarPart::Leg(side) = part {
            transform.translation.x += f32::from(*side) * 0.08 * motion.seat;
        }
        if matches!(
            part,
            AvatarPart::Body | AvatarPart::Head | AvatarPart::Arm(_)
        ) {
            transform.translation.y += motion.idle_phase.sin() * 0.0035;
        }
        if *part == AvatarPart::Body {
            transform.scale = Vec3::new(1.0, 1.0 + motion.idle_phase.sin() * 0.004, 1.0);
        }
    }
}

fn seated_pose(part: AvatarPart, motion: &AvatarMotion, attack: f32) -> Quat {
    match part {
        AvatarPart::Leg(side) => {
            Quat::from_rotation_z(f32::from(side) * 0.65) * Quat::from_rotation_x(0.95)
        }
        AvatarPart::Calf(_) => Quat::from_rotation_x(-1.15),
        AvatarPart::Arm(side) => Quat::from_euler(
            EulerRot::XYZ,
            0.65 + if side == 1 { attack * 0.8 } else { 0.0 },
            0.0,
            f32::from(side) * 0.10,
        ),
        AvatarPart::Forearm(_) => Quat::from_rotation_x(1.05),
        AvatarPart::Head => Quat::from_euler(
            EulerRot::YXZ,
            motion.head_yaw,
            motion.pitch.clamp(-0.65, 0.65),
            0.0,
        ),
        AvatarPart::Body => Quat::IDENTITY,
    }
}

pub(crate) fn eating_motion(progress: f32, duration: f32) -> (f32, f32) {
    let elapsed = progress.clamp(0.0, 1.0) * duration;
    let blend = (elapsed / 0.18)
        .min((duration - elapsed) / 0.16)
        .clamp(0.0, 1.0);
    let bite = (elapsed * std::f32::consts::TAU / 0.34).sin();
    (blend * blend * (3.0 - 2.0 * blend), bite)
}

fn eating_arm_pose(bite: f32) -> Quat {
    Quat::from_rotation_z(0.64) * Quat::from_rotation_x(2.10 + bite * 0.035)
}

fn eating_forearm_pose(bite: f32) -> Quat {
    Quat::from_rotation_x(1.30 + bite * 0.07)
}

fn arm_pose(side: i8, stride: f32, motion: &AvatarMotion, attack: f32) -> Quat {
    let phase = motion.water_phase;
    let idle = 1.0 - motion.stroke;
    let relaxed = (0.08 + motion.idle_phase.sin() * 0.01)
        * (1.0 - motion.swim)
        * if side == 1 { 1.0 - attack } else { 1.0 };
    Quat::from_euler(
        EulerRot::XYZ,
        stride * f32::from(side) * (1.0 - motion.swim)
            + motion.swim
                * ((0.20 + phase.cos() * 0.12) * idle
                    - (0.18 + phase.cos() * 0.22) * motion.stroke)
            + if side == 1 { attack * 1.35 } else { 0.0 },
        motion.phase.sin() * 0.035 * (motion.speed / 5.0).min(1.0) * (1.0 - motion.swim),
        f32::from(side)
            * (relaxed
                + motion.swim
                    * ((0.5 + phase.sin() * 0.12) * idle
                        + (1.05 + phase.sin() * 0.3) * motion.stroke)),
    )
}

fn forearm_pose(side: i8, motion: &AvatarMotion, attack: f32) -> Quat {
    let gait = (motion.speed / 5.0).clamp(0.0, 1.0);
    let recovery = (motion.phase.sin() * f32::from(side)).max(0.0);
    let ground = 0.10 + gait * (0.13 + recovery * 0.28) + motion.idle_phase.sin() * 0.014;
    let paddle = 0.34 + motion.water_phase.sin() * 0.20 + motion.stroke * 0.25;
    Quat::from_euler(
        EulerRot::XYZ,
        ground * (1.0 - motion.swim)
            + paddle * motion.swim
            + if side == 1 { attack * 0.30 } else { 0.0 },
        motion.swim * f32::from(side) * (0.18 + motion.water_phase.cos() * 0.16),
        0.0,
    )
}

fn calf_pose(side: i8, motion: &AvatarMotion) -> Quat {
    let gait = (motion.speed / 5.0).clamp(0.0, 1.0);
    let swing = (motion.phase.sin() * f32::from(side)).max(0.0);
    let ground = -0.035 - swing * swing * gait * 0.75;
    let water = -0.16
        - (motion.water_phase * 2.0 + f32::from(side)).sin().max(0.0)
            * (0.12 + motion.stroke * 0.36);
    Quat::from_rotation_x(ground * (1.0 - motion.swim) + water * motion.swim)
}

fn leg_pose(side: i8, stride: f32, motion: &AvatarMotion) -> Quat {
    let idle = 1.0 - motion.stroke;
    Quat::from_euler(
        EulerRot::XYZ,
        -stride * f32::from(side) * (1.0 - motion.swim)
            + motion.swim
                * ((0.1 + (motion.water_phase + f32::from(side)).sin() * 0.12) * idle
                    + (motion.water_phase * 2.0).sin() * f32::from(side) * 0.24 * motion.stroke),
        0.0,
        motion.swim * idle * f32::from(side) * 0.08,
    )
}

fn blink(
    time: Res<Time>,
    mut lids: Query<(
        &mut AvatarBlink,
        &mut Transform,
        Option<&AvatarEyelidPose>,
        Option<&WorldAvatarPart>,
    )>,
    roots: Query<&AvatarMotion>,
) {
    for (mut blink, mut transform, pose, part) in &mut lids {
        let sleeping = part
            .and_then(|part| roots.get(part.0).ok())
            .is_some_and(|motion| motion.sleeping);
        let closure = if sleeping {
            1.0
        } else {
            pose.map_or_else(
                || blink.advance(time.delta_secs().clamp(0.0, 0.1)),
                |pose| pose.0.clamp(0.0, 1.0),
            )
        };
        transform.scale.y = closure.max(0.001);
        transform.translation.y = model::EYE_CENTER + (1.0 - closure) * model::EYE_HALF_HEIGHT;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn riding_pose_straddles_the_horse_with_bent_knees_instead_of_a_walking_cycle() {
        let mut motion = AvatarMotion::new(Vec3::ZERO);
        motion.mounted = true;
        motion.seat = 1.0;
        motion.head_yaw = 0.7;
        for side in [-1, 1] {
            let hip = Vec3::new(f32::from(side) * (0.115 + 0.08), -0.23, 0.0);
            let upper = seated_pose(AvatarPart::Leg(side), &motion, 0.0);
            let knee = hip + upper * Vec3::new(0.0, model::KNEE, 0.0);
            let calf = seated_pose(AvatarPart::Calf(side), &motion, 0.0);
            let foot = knee + upper * calf * Vec3::new(0.0, -0.30, 0.0);
            assert!(knee.z < -0.20 && knee.x.abs() > 0.28);
            assert!(foot.x.abs() > 0.40 && foot.y < knee.y);
            motion.phase = 2.4;
            motion.speed = 10.0;
            assert!(upper.abs_diff_eq(seated_pose(AvatarPart::Leg(side), &motion, 0.0), 0.0001));
        }
        let pelvis_over_back = terra_player::PLAYER_HALF_HEIGHT - riding::SEATED_DROP - 0.285;
        assert!((0.0..0.03).contains(&pelvis_over_back));
        let head = seated_pose(AvatarPart::Head, &motion, 0.0)
            .to_euler(EulerRot::YXZ)
            .0;
        assert!((head - 0.7).abs() < 0.0001);
    }

    #[test]
    fn sleeping_pose_lies_face_up_with_its_head_on_the_pillow_in_every_direction() {
        use terra_network::protocol::session::SleepingPlayer;
        for facing in 0..4 {
            let sleep = SleepingPlayer {
                player: PlayerId(1),
                anchor: [-16, 42, -16],
                facing,
            };
            let pose = sleep_transform(sleep);
            let forward =
                Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2) * Vec3::Z;
            assert!((pose.rotation * Vec3::Y).abs_diff_eq(forward, 0.0001));
            assert!((pose.rotation * Vec3::NEG_Z).abs_diff_eq(Vec3::Y, 0.0001));
            let head = pose.translation + pose.rotation * Vec3::new(0.0, 0.65, 0.0);
            assert!((head.y - 43.02).abs() < 0.0001);
            assert!((head - pose.translation).dot(forward) > 0.6);
        }
    }

    #[test]
    fn sleeping_makes_the_local_avatar_visible_from_first_person_and_waking_hides_it_again() {
        let mut app = App::new();
        let mut session = terra_network::client::ClientSession::default();
        session.ready = true;
        session.player = Some(PlayerId(1));
        session
            .sleepers
            .push(terra_network::protocol::session::SleepingPlayer {
                player: PlayerId(1),
                anchor: [0, 0, 0],
                facing: 0,
            });
        app.insert_resource(Time::<Fixed>::from_seconds(0.02))
            .insert_resource(terra_player::CameraPerspective::FirstPerson)
            .insert_resource(session)
            .insert_resource(AvatarAssets {
                parts: [
                    vec![(AvatarPart::Body, Vec3::ZERO, Handle::default())],
                    Vec::new(),
                ],
                eyelids: None,
                material: Handle::default(),
            })
            .add_systems(Update, local_avatar);
        app.world_mut()
            .spawn((terra_player::PlayerBody, Transform::from_xyz(2.0, 1.0, 2.0)));
        app.update();
        let world = app.world_mut();
        let pose = world
            .query_filtered::<&Transform, With<LocalAvatar>>()
            .single(world)
            .unwrap();
        assert!(
            pose.translation
                .abs_diff_eq(Vec3::new(0.5, 1.02, 1.0), 0.0001)
        );
        let layers = world
            .query_filtered::<&RenderLayers, With<WorldAvatarPart>>()
            .single(world)
            .unwrap();
        assert!(layers.intersects(&RenderLayers::default()));
        world
            .resource_mut::<terra_network::client::ClientSession>()
            .sleepers
            .clear();
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<LocalAvatar>>()
                .iter(world)
                .count(),
            0
        );
    }

    #[test]
    fn eating_brings_the_held_grip_to_the_mouth_for_both_avatar_shapes() {
        let elbow = Vec3::new(0.0, model::ELBOW, 0.0);
        let palm = Vec3::new(0.008, -0.54 - model::ELBOW, -0.028);
        for shoulder in [0.235, 0.283] {
            for bite in [-1.0, 0.0, 1.0] {
                let hand = Vec3::new(shoulder, 0.305, 0.0)
                    + eating_arm_pose(bite) * (elbow + eating_forearm_pose(bite) * palm);
                assert!(hand.x.abs() < 0.11, "{hand:?}");
                assert!((0.56..0.72).contains(&hand.y), "{hand:?}");
                assert!((-0.22..-0.07).contains(&hand.z), "{hand:?}");
            }
        }
        assert_eq!(eating_motion(0.0, 1.6).0, 0.0);
        assert_eq!(eating_motion(1.0, 1.6).0, 0.0);
        assert_eq!(eating_motion(0.5, 1.6).0, 1.0);
    }
    #[test]
    fn gait_bends_elbows_and_knees_without_snapping_and_idle_keeps_breathing() {
        let mut motion = AvatarMotion::new(Vec3::ZERO);
        motion.speed = 5.0;
        let mut previous = [Quat::IDENTITY; 4];
        let mut range = [0.0_f32; 4];
        for frame in 0..240 {
            motion.phase = frame as f32 / 240.0 * std::f32::consts::TAU;
            let pose = [
                forearm_pose(-1, &motion, 0.0),
                forearm_pose(1, &motion, 0.0),
                calf_pose(-1, &motion),
                calf_pose(1, &motion),
            ];
            for index in 0..4 {
                if frame > 0 {
                    assert!(pose[index].angle_between(previous[index]) < 0.08);
                }
                range[index] = range[index].max(pose[index].angle_between(Quat::IDENTITY));
            }
            previous = pose;
        }
        assert!(range[..2].iter().all(|range| *range > 0.45));
        assert!(range[2..].iter().all(|range| *range > 0.70));
        let phase = motion.idle_phase;
        motion.advance(Vec3::ZERO, 0.1);
        assert!(motion.idle_phase > phase);
    }
    #[test]
    fn attack_drives_the_right_hand_toward_avatar_forward() {
        let motion = AvatarMotion::new(Vec3::ZERO);
        let hand = Vec3::new(0.0, -0.48, 0.0);
        let idle = arm_pose(1, 0.0, &motion, 0.0) * hand;
        let hit = arm_pose(1, 0.0, &motion, 1.0) * hand;
        assert!((hit - idle).dot(Vec3::NEG_Z) > 0.4);
    }
    #[test]
    fn stationary_swimmers_settle_upright_but_keep_arms_and_legs_moving() {
        let mut motion = AvatarMotion::new(Vec3::ZERO);
        motion.swim = 1.0;
        motion.stroke = 1.0;
        for _ in 0..120 {
            motion.water(true, false, 1.0 / 60.0);
        }
        assert!(motion.stroke < 0.001 && motion.swim > 0.999);
        let arm = arm_pose(1, 0.0, &motion, 0.0);
        let leg = leg_pose(1, 0.0, &motion);
        for _ in 0..60 {
            motion.water(true, false, 1.0 / 60.0);
        }
        assert!(arm.angle_between(arm_pose(1, 0.0, &motion, 0.0)) > 0.03);
        assert!(leg.angle_between(leg_pose(1, 0.0, &motion)) > 0.03);
        for _ in 0..120 {
            motion.water(true, true, 1.0 / 60.0);
        }
        assert!(motion.stroke > 0.999);
    }

    #[test]
    fn breaststroke_arms_extend_sideways_not_over_the_head() {
        let mut motion = AvatarMotion::new(Vec3::ZERO);
        motion.swim = 1.0;
        motion.stroke = 1.0;
        for step in 0..32 {
            motion.water_phase = step as f32 / 32.0 * std::f32::consts::TAU;
            for side in [-1, 1] {
                let direction =
                    Quat::from_rotation_x(-1.2) * arm_pose(side, 0.0, &motion, 0.0) * Vec3::NEG_Y;
                assert!(direction.x * f32::from(side) > 0.65);
                assert!(direction.y.abs() < 0.45);
            }
        }
    }

    #[test]
    fn hand_swing_starts_on_input_and_finishes_after_release() {
        let mut swing = HandSwing {
            elapsed: 0.32,
            ..default()
        };
        swing.advance(0.08, true, 0);
        assert!(swing.amount > 0.65);
        for _ in 0..8 {
            swing.advance(0.05, false, 0);
        }
        assert_eq!(swing.amount, 0.0);
        swing.advance(0.08, false, 1);
        assert!(swing.amount > 0.65);
    }

    #[test]
    fn walk_motion_settles_and_teleports_do_not_drive_stride() {
        let mut motion = AvatarMotion::new(Vec3::ZERO);
        motion.advance(Vec3::new(0.1, 0.0, 0.0), 0.02);
        assert!(motion.speed > 0.0);
        for _ in 0..120 {
            motion.advance(Vec3::new(0.1, 0.0, 0.0), 0.02);
        }
        assert!(motion.speed < 0.001);
        motion.advance(Vec3::splat(200.0), 0.02);
        assert!(motion.speed < 0.001);
    }

    #[test]
    fn blinks_are_short_infrequent_and_desynchronized() {
        let mut a = AvatarBlink::new(17);
        let mut b = AvatarBlink::new(24);
        let mut closed = 0;
        let mut different = 0;
        for _ in 0..6000 {
            let left = a.advance(0.01);
            let right = b.advance(0.01);
            assert!((0.0..=1.0).contains(&left));
            closed += usize::from(left > 0.2);
            different += usize::from((left - right).abs() > 0.2);
        }
        assert!((50..300).contains(&closed));
        assert!(different > 40);
    }

    #[test]
    fn first_person_miniature_has_real_avatar_without_showing_body_in_gameplay() {
        let mut app = App::new();
        app.insert_resource(Time::<Fixed>::from_seconds(0.02))
            .insert_resource(terra_player::CameraPerspective::FirstPerson)
            .insert_resource(terra_core::miniature::MiniatureView { active: true })
            .insert_resource(AvatarAssets {
                parts: [
                    vec![(AvatarPart::Body, Vec3::ZERO, Handle::default())],
                    Vec::new(),
                ],
                eyelids: None,
                material: Handle::default(),
            })
            .add_systems(Update, local_avatar);
        app.world_mut().spawn((
            terra_player::PlayerBody,
            Transform::from_xyz(-60.0, 80.0, -30.0),
        ));
        app.update();
        let world = app.world_mut();
        let layers = world
            .query_filtered::<&RenderLayers, With<WorldAvatarPart>>()
            .single(world)
            .unwrap();
        assert!(layers.intersects(&RenderLayers::layer(terra_core::miniature::WORLD_LAYER)));
        assert!(!layers.intersects(&RenderLayers::default()));
        world
            .resource_mut::<terra_core::miniature::MiniatureView>()
            .active = false;
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<LocalAvatar>>()
                .iter(world)
                .count(),
            0
        );
    }

    #[test]
    fn front_avatar_uses_presented_body_position_and_player_aim() {
        let mut app = App::new();
        let mut fixed = Time::<Fixed>::from_seconds(0.02);
        fixed.accumulate_overstep(std::time::Duration::from_millis(5));
        app.insert_resource(fixed)
            .insert_resource(terra_player::CameraPerspective::ThirdPersonFront)
            .insert_resource(AvatarAssets {
                parts: [Vec::new(), Vec::new()],
                eyelids: None,
                material: Handle::default(),
            })
            .add_systems(Update, local_avatar);
        let current = Vec3::new(8.0, 3.0, 7.0);
        let presentation = terra_player::CharacterPresentation {
            previous: Vec3::new(7.0, 3.0, 6.0),
            correction: Vec3::new(0.12, 0.0, -0.08),
        };
        let expected = presentation.visual_position(current, 0.25);
        app.world_mut().spawn((
            terra_player::PlayerBody,
            Transform::from_translation(current),
            presentation,
        ));
        app.world_mut().spawn((
            terra_player::TerraCamera,
            Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        ));
        app.update();
        let avatar = app
            .world_mut()
            .query_filtered::<&Transform, With<LocalAvatar>>()
            .single(app.world())
            .unwrap();
        assert!(!expected.abs_diff_eq(current, 0.001));
        assert!(avatar.translation.abs_diff_eq(expected, 0.0001));
        assert!(avatar.rotation.abs_diff_eq(Quat::IDENTITY, 0.0001));
    }
}
