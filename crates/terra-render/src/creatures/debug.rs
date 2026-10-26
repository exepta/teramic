use bevy::prelude::*;
use terra_core::{DebugSettings, creatures::Creature};
use terra_network::client::ReplicatedCreatures;

pub(super) fn install(app: &mut App) {
    app.add_systems(
        PostUpdate,
        draw.after(bevy::transform::TransformSystems::Propagate),
    );
}

fn collision_pose(animal: &Creature, feet: Vec3) -> (Vec3, Vec3, Quat) {
    let size = Vec3::from_array(animal.size().map(|v| v as f32));
    let yaw = if animal.kind.is_spider()
        && terra_core::creatures::spiders::surface_normal(animal)[1] == 0
    {
        0.0
    } else {
        animal.yaw
    };
    (
        feet + Vec3::Y * size.y * 0.5,
        size,
        Quat::from_rotation_y(yaw),
    )
}

fn corners(center: Vec3, size: Vec3, rotation: Quat) -> [Vec3; 8] {
    std::array::from_fn(|i| {
        center
            + rotation
                * (size
                    * Vec3::new(
                        if i & 1 == 0 { -0.5 } else { 0.5 },
                        if i & 2 == 0 { -0.5 } else { 0.5 },
                        if i & 4 == 0 { -0.5 } else { 0.5 },
                    ))
    })
}

fn draw(
    settings: Option<Res<DebugSettings>>,
    animals: Option<Res<ReplicatedCreatures>>,
    mounts: Option<Res<crate::players::MountedPresentation>>,
    focus: Res<terra_world::streaming::ChunkFocus>,
    roots: Query<(&super::CreatureAvatar, &Transform)>,
    mut gizmos: Gizmos,
) {
    if !settings.is_some_and(|s| s.creature_bounds) {
        return;
    }
    let focus = Vec3::from_array(focus.0.map(|v| v as f32));
    for (avatar, transform) in &roots {
        let Some((animal, feet, _)) =
            super::presented_creature(avatar.0, animals.as_deref(), mounts.as_deref())
        else {
            continue;
        };
        if feet.distance_squared(focus) > 128.0 * 128.0 {
            continue;
        }
        let (center, size, rotation) = collision_pose(animal, feet);
        let points = corners(center, size, rotation);
        for i in 0..8 {
            for bit in [1, 2, 4] {
                if i & bit == 0 {
                    gizmos.line(points[i], points[i | bit], Color::srgb(0.40, 0.47, 0.53));
                }
            }
        }
        let offset = feet - Vec3::from_array(animal.position.map(|v| v as f32));
        for box_ in terra_core::creatures::hitboxes::for_creature(animal).iter() {
            let points = box_
                .corners()
                .map(|p| Vec3::from_array(p.map(|v| v as f32)) + offset);
            for i in 0..8 {
                for bit in [1, 2, 4] {
                    if i & bit == 0 {
                        gizmos.line(points[i], points[i | bit], Color::srgb(0.95, 0.97, 1.0));
                    }
                }
            }
        }
        let head = center + transform.rotation * Vec3::Y * size.y * 0.25;
        let tip = head + transform.rotation * Vec3::NEG_Z * size.z.clamp(0.6, 2.5);
        gizmos.line(head, tip, Color::srgb(0.15, 0.8, 1.0));
        for side in [-1.0, 1.0] {
            let wing = tip + transform.rotation * Vec3::new(0.10 * side, 0.0, 0.18);
            gizmos.line(tip, wing, Color::srgb(0.15, 0.8, 1.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::creatures::{CreatureBehavior, CreatureKind, CreatureNeeds};

    #[test]
    fn debug_boxes_follow_real_rotated_footprints_and_spider_attachment_axes() {
        for kind in CreatureKind::ALL {
            let mut animal = Creature {
                id: 1,
                kind,
                home: [0, 0],
                position: [3.0, -20.0, -4.0],
                velocity: [0.0; 3],
                yaw: 0.73,
                health: kind.health(),
                behavior: CreatureBehavior::Wander,
                timer: 0.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(kind),
            };
            for behavior in [
                CreatureBehavior::Wander,
                CreatureBehavior::Climb,
                CreatureBehavior::Ceiling,
            ] {
                animal.behavior = behavior;
                let feet = Vec3::from_array(animal.position.map(|v| v as f32));
                let (center, size, rotation) = collision_pose(&animal, feet);
                assert!((center.y - size.y * 0.5 - feet.y).abs() < 0.00001);
                assert_eq!(size, Vec3::from_array(animal.size().map(|v| v as f32)));
                for corner in corners(center, size, rotation) {
                    let local = rotation.inverse() * (corner - center);
                    assert!((local.abs() - size * 0.5).abs().max_element() < 0.00001);
                }
                if kind.is_spider() && behavior == CreatureBehavior::Climb {
                    assert_eq!(rotation, Quat::IDENTITY);
                }
            }
        }
    }
}
