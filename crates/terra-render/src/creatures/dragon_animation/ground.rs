//! Fit the walking skeleton to the actual four treads under its claws.
use super::{Joint, Pose, Presentation};
use bevy::prelude::*;
use terra_core::creatures::CreatureBehavior as B;
use terra_world::{
    creatures::Habitat,
    edits::EditJournal,
    streaming::{ChunkStore, WorldTerrain},
};

const LEGS: [(i8, bool); 4] = [(-1, false), (1, false), (-1, true), (1, true)];

#[derive(Clone, Copy, Default)]
pub(super) struct Stance {
    heights: [f32; 4],
    weight: f32,
    last_root_y: Option<f32>,
    planted: [Option<Vec3>; 4],
    ankle_offset: [Vec3; 4],
    crouch: f32,
    body: Isometry3d,
    legs: [[Isometry3d; 3]; 4],
}

pub(in crate::creatures) fn fit(
    terrain: Option<Res<WorldTerrain>>,
    chunks: Option<Res<ChunkStore>>,
    journal: Option<Res<EditJournal>>,
    time: Res<Time>,
    mut avatars: Query<(&Transform, &mut Presentation)>,
) {
    let (Some(terrain), Some(chunks), Some(journal)) = (terrain, chunks, journal) else {
        return;
    };
    let habitat = Habitat {
        terrain: &terrain.0,
        chunks: &chunks,
        journal: &journal,
    };
    for (root, mut presentation) in &mut avatars {
        let pose = presentation.pose;
        let scale = presentation.scale;
        let vertical = scale * root.scale.y;
        let walking = pose.kind != terra_core::creatures::CreatureKind::WaterDragon
            && matches!(pose.behavior, B::Wander | B::Hunt | B::Rest);
        let mut levels = [None; 4];
        let stance = &mut presentation.ground;
        if let Some(last) = stance.last_root_y {
            // IK history is local to the root. Rebase it when the physical
            // collider drops off the previous step, keeping claws in world space.
            let dy = ((root.translation.y - last) / vertical).clamp(-5., 5.);
            for height in &mut stance.heights {
                *height -= dy;
            }
        }
        stance.last_root_y = Some(root.translation.y);
        for (i, (side, hind)) in LEGS.into_iter().enumerate() {
            if !walking {
                stance.planted[i] = None;
                stance.ankle_offset[i] = Vec3::ZERO;
                continue;
            }
            let joint = Joint::Paw { side, hind };
            let ball = Vec3::from(pose.bone(joint).translation) - Vec3::Z * 0.65;
            let world = root.transform_point(ball * scale);
            let supporting = pose.paw_lift(side, hind) < 0.035;
            let contact = if supporting {
                let anchor = stance.planted[i].get_or_insert(world);
                // Slip only the excess when turning on a planted paw.
                *anchor = world
                    + (*anchor - world)
                        .with_y(0.)
                        .clamp_length_max(vertical * 1.55);
                *anchor
            } else {
                stance.planted[i] = None;
                world
            };
            let desired =
                (root.rotation.inverse() * (contact - world).with_y(0.)) / (root.scale * scale);
            // Carry the previous offset into the recovery stroke. Erasing it on
            // liftoff used to teleport the claw and yank the attached knee.
            stance.ankle_offset[i] = stance.ankle_offset[i].lerp(
                desired,
                1. - (-(if supporting { 42. } else { 15. }) * time.delta_secs()).exp(),
            );
            levels[i] = habitat
                .paw_surface(
                    [
                        contact.x.into(),
                        root.translation.y.into(),
                        contact.z.into(),
                    ],
                    f64::from((vertical * 5.).max(1.1)),
                )
                .map(|y| (y as f32 - root.translation.y) / vertical);
        }
        presentation.ground.update(pose, levels, time.delta_secs());
    }
}

impl Stance {
    fn update(&mut self, pose: Pose, levels: [Option<f32>; 4], dt: f32) {
        let blend = 1. - (-dt * 14.).exp();
        let contact = if levels.iter().any(Option::is_some) {
            1.
        } else {
            0.
        };
        self.weight += (contact - self.weight) * blend;
        for (y, level) in self.heights.iter_mut().zip(levels) {
            *y += (level.unwrap_or(*y).clamp(-5., 1.5) - *y) * blend;
        }
        if self.weight < 0.001 {
            self.weight = 0.;
            return;
        }
        // A broad animal carries its chest and hips at different heights on a flight of stairs.
        let [fl, fr, hl, hr] = self.heights;
        let pitch = ((fl + fr - hl - hr) / 11.3).atan().clamp(-0.72, 0.72);
        let roll = ((fr + hr - fl - hl) / 10.6).atan().clamp(-0.5, 0.5);
        let height = (fl + fr + hl + hr) * 0.25 - 0.19;
        let rotation = Quat::from_rotation_z(roll) * Quat::from_rotation_x(pitch);
        let trunk = Joint::Trunk.pivot();
        self.body = Isometry3d::new(Vec3::Y * height + trunk - rotation * trunk, rotation);

        // Lower the hips enough for the shortest loaded leg to actually
        // reach its tread. Clamping only the IK endpoint leaves claws in air.
        let targets = LEGS.map(|(side, hind)| {
            let i = 2 * usize::from(hind) + usize::from(side > 0);
            let paw = Joint::Paw { side, hind };
            (Vec3::from(pose.bone(paw).translation) + self.ankle_offset[i])
                .with_y(self.heights[i] + paw.pivot().y + pose.paw_lift(side, hind))
        });
        let mut crouch: f32 = 0.;
        for (i, (side, hind)) in LEGS.into_iter().enumerate() {
            if levels[i].is_none() {
                continue;
            }
            let hip = Joint::Thigh { side, hind };
            let knee = Joint::Foot { side, hind }.pivot();
            let ankle = Joint::Paw { side, hind }.pivot();
            let length = knee.distance(hip.pivot()) + knee.distance(ankle) - 0.035;
            let d = Vec3::from((self.body * pose.bone(hip)).translation) - targets[i];
            let reach = (length * length - d.with_y(0.).length_squared())
                .max(0.)
                .sqrt();
            crouch = crouch.max(d.y - reach);
        }
        self.crouch += (crouch.min(1.5) - self.crouch) * (1. - (-dt * 12.).exp());
        self.body.translation.y -= self.crouch;
        for (i, (side, hind)) in LEGS.into_iter().enumerate() {
            let upper = Joint::Thigh { side, hind };
            let lower = Joint::Foot { side, hind };
            let paw = Joint::Paw { side, hind };
            let target = targets[i];
            self.legs[i] = solve_leg(
                self.body * pose.bone(upper),
                self.body * pose.bone(lower),
                upper,
                lower,
                paw,
                target,
            );
        }
    }

    pub(super) fn bone(&self, pose: Pose, joint: Joint) -> Isometry3d {
        let original = pose.bone(joint);
        if self.weight == 0. || joint.is_effect() {
            return original;
        }
        let fitted = match joint {
            Joint::Thigh { side, hind }
            | Joint::Foot { side, hind }
            | Joint::Paw { side, hind } => {
                let index = 2 * usize::from(hind) + usize::from(side > 0);
                let part = match joint {
                    Joint::Thigh { .. } => 0,
                    Joint::Foot { .. } => 1,
                    _ => 2,
                };
                self.legs[index][part]
            }
            _ => self.body * original,
        };
        Isometry3d::new(
            original.translation.lerp(fitted.translation, self.weight),
            original.rotation.slerp(fitted.rotation, self.weight),
        )
    }
}

fn solve_leg(
    hip: Isometry3d,
    knee: Isometry3d,
    upper: Joint,
    lower: Joint,
    paw: Joint,
    target: Vec3,
) -> [Isometry3d; 3] {
    let thigh = hip.rotation * (lower.pivot() - upper.pivot());
    let shin = knee.rotation * (paw.pivot() - lower.pivot());
    let base = Vec3::from(hip.translation);
    let to_target = target - base;
    let axis = to_target.normalize_or(Vec3::NEG_Y);
    let (a, b) = (thigh.length(), shin.length());
    let distance = to_target
        .length()
        .clamp((a - b).abs() + 0.001, a + b - 0.001);
    let along = (distance * distance + a * a - b * b) / (2. * distance);
    // Keep the knee on the anatomical side of the leg even when the claw is
    // directly under the hip. A pole derived from the moving thigh can flip
    // through 180 degrees as a fully extended step crosses that line.
    let neutral_axis = (paw.pivot() - upper.pivot()).normalize();
    let pole = hip.rotation * (lower.pivot() - upper.pivot()).reject_from_normalized(neutral_axis);
    let bend = pole.reject_from_normalized(axis).normalize_or(Vec3::Z);
    let elbow = base + along * axis + (a * a - along * along).max(0.).sqrt() * bend;
    let end = base + axis * distance;
    let align = |old: Vec3, new: Vec3| Quat::from_rotation_arc(old.normalize(), new.normalize());
    [
        Isometry3d::new(base, align(thigh, elbow - base) * hip.rotation),
        Isometry3d::new(elbow, align(shin, end - elbow) * knee.rotation),
        Isometry3d::from_translation(end),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::creatures::{Creature, CreatureKind, CreatureNeeds};

    #[test]
    fn dragon_paws_plant_on_descending_and_crosswise_steps_with_connected_knees() {
        let kind = CreatureKind::EarthDragon;
        let dragon = Creature {
            id: 1,
            kind,
            home: [0; 2],
            position: [0.; 3],
            velocity: [0.; 3],
            yaw: 0.,
            health: kind.health(),
            behavior: B::Rest,
            timer: 0.,
            age: 1200.,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        };
        let pose = Pose::of(&dragon, 0.);
        for treads in [
            [-1., -1., 0., 0.],
            [0., 0., -1., -1.],
            [-1., 0., -1., 0.],
            [-4., -4., -1., -1.],
        ] {
            let mut stance = Stance::default();
            for _ in 0..80 {
                stance.update(pose, treads.map(Some), 1. / 60.);
            }
            for (i, (side, hind)) in LEGS.into_iter().enumerate() {
                let thigh = Joint::Thigh { side, hind };
                let shin = Joint::Foot { side, hind };
                let paw = Joint::Paw { side, hind };
                let top = stance.bone(pose, thigh);
                let knee = stance.bone(pose, shin);
                let foot = stance.bone(pose, paw);
                assert!((foot.translation.y - paw.pivot().y - treads[i]).abs() < 0.025);
                assert!(
                    top.transform_point(shin.pivot() - thigh.pivot())
                        .distance(knee.translation)
                        < 0.002
                );
                assert!(
                    knee.transform_point(paw.pivot() - shin.pivot())
                        .distance(foot.translation)
                        < 0.002
                );
                assert!((foot.rotation * Vec3::Y - Vec3::Y).length() < 0.002);
            }
        }
    }
}
