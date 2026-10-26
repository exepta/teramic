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
