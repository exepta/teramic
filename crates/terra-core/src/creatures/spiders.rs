//! Shared attachment poses and venom data, independent of rendering and world access.
use super::{Creature, CreatureBehavior, CreatureKind};

pub fn venom(kind: CreatureKind) -> Option<(u8, f32)> {
    let combat = super::definitions::get(kind).combat?;
    (combat.poison_level > 0).then_some((combat.poison_level, combat.poison_seconds))
}

pub fn floor_size(kind: CreatureKind) -> [f64; 3] {
    let [width, height, length] = kind.size();
    [width * 2.4, height * 1.35, length * 1.3]
}

pub fn surface_normal(animal: &Creature) -> [i8; 3] {
    match animal.behavior {
        CreatureBehavior::Ceiling => [0, -1, 0],
        CreatureBehavior::Climb if animal.kind.is_spider() => {
            let (sin, cos) = animal.yaw.sin_cos();
            if sin.abs() > cos.abs() {
                [if sin < 0.0 { -1 } else { 1 }, 0, 0]
            } else {
                [0, 0, if cos < 0.0 { -1 } else { 1 }]
            }
        }
        _ => [0, 1, 0],
    }
}

pub fn occupied_size(animal: &Creature) -> [f64; 3] {
    let [width, height, length] = floor_size(animal.kind).map(|v| v * animal.growth_scale());
    let normal = surface_normal(animal);
    if normal[0] != 0 {
        [height, length, width]
    } else if normal[2] != 0 {
        [width, length, height]
    } else {
        [width, height, length]
    }
}

pub fn model_pivot(kind: CreatureKind) -> [f64; 3] {
    [0.0, kind.size()[1] * 0.675, 0.0]
}

pub fn visual_center(animal: &Creature) -> [f64; 3] {
    [
        animal.position[0],
        animal.position[1] + occupied_size(animal)[1] * 0.5,
        animal.position[2],
    ]
}

/// Silk expands from a tight knot into a net along a locked, shallow arc.
/// Both the collision sweep and the rendered knot use this same clock.
pub const WEB_WINDUP: f32 = 0.52;
pub const WEB_CAST_TIME: f32 = WEB_WINDUP + 1.2;

pub struct WebShot {
    pub origin: bevy_math::DVec3,
    pub target: bevy_math::DVec3,
    pub flight_time: f32,
}

impl WebShot {
    pub fn of(a: &Creature) -> Option<Self> {
        use bevy_math::DVec3;
        let aim = a
            .needs
            .goal
            .filter(|g| a.kind.is_spider() && g.kind == super::NeedKind::AttackAim)?;
        let center = DVec3::from_array(visual_center(a));
        let target = DVec3::from_array(aim.position.map(|v| v as f64 + 0.5));
        let direction = (target - center).normalize_or_zero();
        let origin = center + direction * (a.kind.size()[2] * 0.48 * a.growth_scale());
        let distance = origin.distance(target);
        if !(0.01..=20.).contains(&distance) {
            return None;
        }
        Some(Self {
            origin,
            target,
            flight_time: (distance / 16.5) as f32,
        })
    }

    pub fn at(&self, elapsed: f32) -> bevy_math::DVec3 {
        let t = f64::from((elapsed / self.flight_time).clamp(0., 1.));
        self.origin.lerp(self.target, t) + bevy_math::DVec3::Y * (1.6 * t * (1. - t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creatures::{CreatureNeeds, spider_id};

    #[test]
    fn spider_body_dimensions_venom_and_append_only_ids_are_stable() {
        assert_eq!(CreatureKind::Horse as u8, 15);
        for (slot, (kind, size, venom_data)) in [
            (CreatureKind::SmallSpider, [0.145, 0.1, 0.195], (2, 16.0)),
            (CreatureKind::MediumSpider, [0.645, 0.5, 0.695], (1, 25.0)),
            (CreatureKind::LargeSpider, [1.345, 1.25, 1.895], (3, 5.0)),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(kind.size(), size);
            assert_eq!(venom(kind), Some(venom_data));
            let mut animal = Creature {
                id: spider_id([-4, 8], slot as u8).unwrap(),
                kind,
                home: [-4, 8],
                position: [-120.5, -30.0, 260.5],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: kind.health(),
                behavior: CreatureBehavior::Climb,
                timer: 0.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(kind),
            };
            assert!(animal.valid());
            assert_eq!(surface_normal(&animal), [0, 0, 1]);
            assert_eq!(animal.size()[1], size[2] * 1.3);
            animal.behavior = CreatureBehavior::Ceiling;
            assert_eq!(surface_normal(&animal), [0, -1, 0]);
            assert_eq!(animal.size(), floor_size(kind));
        }
    }
}
