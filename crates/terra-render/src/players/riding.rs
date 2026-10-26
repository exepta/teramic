use bevy::prelude::*;
use std::collections::HashMap;
use terra_core::{PlayerId, creatures::Creature};
use terra_network::{
    client::{ClientSession, ReplicatedPlayers},
    protocol::session::MountedState,
};

pub(super) const SEATED_DROP: f32 = 0.60;

pub(super) fn sit(transform: &mut Transform, pitch: f32) {
    transform.translation.y -= SEATED_DROP;
    let yaw = transform.rotation.to_euler(EulerRot::YXZ).0;
    transform.rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
    let pelvis = Vec3::Y * -0.285;
    transform.translation += pelvis - transform.rotation * pelvis;
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MountedPresentationSet;

pub(crate) struct MountedPose {
    pub horse: Creature,
    pub center: Vec3,
    pub jump: crate::creatures::horse_pose::JumpPose,
}

#[derive(Resource, Default)]
pub(crate) struct MountedPresentation {
    pub horses: HashMap<u64, MountedPose>,
    riders: HashMap<PlayerId, u64>,
    local: Option<u64>,
    jumps: HashMap<u64, crate::creatures::horse_pose::JumpPose>,
}
impl MountedPresentation {
    pub(super) fn remote(&self, rider: PlayerId) -> Option<&MountedPose> {
        self.horses.get(self.riders.get(&rider)?)
    }
    pub(super) fn local(&self) -> Option<&MountedPose> {
        self.horses.get(&self.local?)
    }
}

fn at_rider(
    mount: MountedState,
    center: Vec3,
    yaw: f32,
    jump: crate::creatures::horse_pose::JumpPose,
) -> MountedPose {
    let mut horse = mount.horse();
    let seat =
        Vec3::from_array(terra_world::creatures::riding::seat_feet(&horse).map(|v| v as f32))
            + Vec3::Y * terra_player::PLAYER_HALF_HEIGHT;
    let shift = (center - seat).to_array().map(f64::from);
    horse.position = std::array::from_fn(|axis| horse.position[axis] + shift[axis]);
    horse.yaw = yaw;
    MountedPose {
        horse,
        center,
        jump,
    }
}

pub(super) fn prepare(
    session: Option<Res<ClientSession>>,
    players: Option<Res<ReplicatedPlayers>>,
    fixed: Res<Time<Fixed>>,
    time: Res<Time>,
    bodies: Query<
        (&Transform, Option<&terra_player::CharacterPresentation>),
        With<terra_player::PlayerBody>,
    >,
    mut presentation: ResMut<MountedPresentation>,
) {
    presentation.horses.clear();
    presentation.riders.clear();
    presentation.local = None;
    if session.as_ref().is_some_and(|s| !s.ready) {
        presentation.jumps.clear();
        return;
    }
    if let Some(players) = players {
        for (&id, track) in &players.players {
            if session.as_ref().and_then(|s| s.player) == Some(id) {
                continue;
            }
            let Some(mount) = track.current.mounted.filter(|m| m.valid()) else {
                continue;
            };
            let yaw = track.mounted_yaw().unwrap_or(mount.yaw);
            let mut jump = presentation
                .jumps
                .get(&mount.id)
                .copied()
                .unwrap_or_default();
            jump.advance(&mount.horse(), time.delta_secs());
            presentation.jumps.insert(mount.id, jump);
            presentation
                .horses
                .insert(mount.id, at_rider(mount, track.position(), yaw, jump));
            presentation.riders.insert(id, mount.id);
        }
    }
    if let Some(mount) = session
        .as_ref()
        .and_then(|s| s.mounted)
        .filter(|m| m.valid())
        && let Some((body, visual)) = bodies.iter().next()
    {
        let center = visual.map_or(body.translation, |p| {
            p.visual_position(body.translation, fixed.overstep_fraction())
        });
        let mut jump = presentation
            .jumps
            .get(&mount.id)
            .copied()
            .unwrap_or_default();
        jump.advance(&mount.horse(), time.delta_secs());
        presentation.jumps.insert(mount.id, jump);
        presentation
            .horses
            .insert(mount.id, at_rider(mount, center, mount.yaw, jump));
        presentation.local = Some(mount.id);
    }
    let active: std::collections::HashSet<_> = presentation.horses.keys().copied().collect();
    presentation.jumps.retain(|id, _| active.contains(id));
}

pub(super) fn relative_yaw(look: f32, horse: f32) -> f32 {
    ((look - horse + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI)
        .clamp(-1.1, 1.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_mount_cache_tracks_the_body_and_releases_snapshots_on_dismount() {
        let horse = test_horse();
        let mut app = App::new();
        app.init_resource::<ClientSession>()
            .init_resource::<Time<Fixed>>()
            .init_resource::<Time>()
            .init_resource::<MountedPresentation>()
            .add_systems(Update, prepare);
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session.ready = true;
            session.player = Some(PlayerId(1));
            session.mounted = Some(MountedState::new(&horse, 10.0, true));
        }
        let body = app
            .world_mut()
            .spawn((
                terra_player::PlayerBody,
                Transform::from_xyz(-16.5, 12.5, -16.5),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world()
                .resource::<MountedPresentation>()
                .local()
                .unwrap()
                .center,
            Vec3::new(-16.5, 12.5, -16.5)
        );
        app.world_mut()
            .get_mut::<Transform>(body)
            .unwrap()
            .translation
            .x += 0.37;
        app.update();
        let presented = app
            .world()
            .resource::<MountedPresentation>()
            .local()
            .unwrap();
        assert!((presented.center.x + 16.13).abs() < 0.0001);
        assert_eq!(
            app.world().resource::<MountedPresentation>().horses.len(),
            1
        );
        app.world_mut().resource_mut::<ClientSession>().mounted = None;
        app.update();
        assert!(
            app.world()
                .resource::<MountedPresentation>()
                .horses
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<MountedPresentation>()
                .local()
                .is_none()
        );
        assert!(
            app.world()
                .resource::<MountedPresentation>()
                .jumps
                .is_empty()
        );
    }
    fn test_horse() -> Creature {
        Creature {
            id: terra_core::creatures::creature_id([-1, -1], 0).unwrap(),
            kind: terra_core::creatures::CreatureKind::Horse,
            home: [-1, -1],
            position: [-18.5, 8.0, -20.5],
            velocity: [2.0, 0.0, 1.0],
            yaw: 0.8,
            health: 20.0,
            behavior: terra_core::creatures::CreatureBehavior::Wander,
            timer: 0.0,
            age: 9000.0,
            variant: 0,
            needs: terra_core::creatures::CreatureNeeds::full(
                terra_core::creatures::CreatureKind::Horse,
            ),
        }
    }
    #[test]
    fn horse_uses_the_exact_presented_rider_center_not_its_independent_network_interpolation() {
        let mut horse = test_horse();
        for yaw in [0.0, 0.7, -1.5, std::f32::consts::PI] {
            horse.yaw = yaw;
            let snapshot = MountedState::new(&horse, 10.0, true);
            let center = Vec3::new(-31.25, 42.75, -65.50);
            let pose = at_rider(snapshot, center, yaw, Default::default());
            let actual = Vec3::from_array(
                terra_world::creatures::riding::seat_feet(&pose.horse).map(|v| v as f32),
            ) + Vec3::Y * terra_player::PLAYER_HALF_HEIGHT;
            assert!(actual.abs_diff_eq(center, 0.00001));
            assert_eq!(pose.center, center);
            assert_eq!(pose.horse.yaw, yaw);
        }
        assert!(relative_yaw(-3.13, 3.13).abs() < 0.03);
    }

    #[test]
    fn rider_pelvis_stays_on_the_shared_seat_while_jumping_and_leaning() {
        let center = Vec3::new(-16.0, 23.0, -32.0);
        for yaw in [0.0, 0.8, -2.0] {
            for pitch in [-0.16, 0.0, 0.16] {
                let mut transform =
                    Transform::from_translation(center).with_rotation(Quat::from_rotation_y(yaw));
                sit(&mut transform, pitch * 0.65);
                let pelvis = transform.transform_point(Vec3::Y * -0.285);
                let seat = center - Vec3::Y * terra_player::PLAYER_HALF_HEIGHT;
                assert!(pelvis.abs_diff_eq(seat + Vec3::Y * 0.015, 0.00001));
                assert!(
                    (transform.rotation.to_euler(EulerRot::YXZ).1 - pitch * 0.65).abs() < 0.0001
                );
            }
        }
    }
}
