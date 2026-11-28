use super::*;
use crate::creatures::{CreatureNeeds, breeding};

fn animal(kind: Kind) -> Creature {
    Creature {
        id: 1,
        kind,
        home: [0, 0],
        position: [0.0; 3],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: kind.health(),
        behavior: CreatureBehavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(kind),
    }
}

#[test]
fn butterflies_use_half_size_targets_without_retaining_the_old_wing_span() {
    assert_eq!(Kind::Butterfly.size(), [0.11, 0.11, 0.15]);
    for variant in 0..Kind::Butterfly.variant_count() {
        let mut butterfly = animal(Kind::Butterfly);
        butterfly.variant = variant;
        let boxes = for_creature(&butterfly);
        assert_eq!(boxes.scale, [0.5; 3]);
        for x in [0.0, 0.15, -0.15] {
            assert!(
                boxes
                    .ray_distance([x, 0.07, -1.0], [0.0, 0.0, 1.0], 2.0)
                    .is_some()
            );
        }
        for x in [-0.35, 0.35] {
            assert!(
                boxes
                    .ray_distance([x, 0.07, -1.0], [0.0, 0.0, 1.0], 2.0)
                    .is_none()
            );
        }
    }
}

#[test]
fn butterfly_targeting_halves_with_the_visible_wings_and_body() {
    assert_eq!(Kind::Butterfly.size(), [0.11, 0.11, 0.15]);
    let butterfly = animal(Kind::Butterfly);
    let targets = for_creature(&butterfly);
    assert_eq!(targets.count, 3);
    let mut targets = targets.iter();
    let body = targets.next().unwrap();
    assert_eq!(body.center, [0.0, 0.07, -0.01]);
    assert_eq!(body.size, [0.07, 0.125, 0.15]);
    for (target, side) in targets.zip([-1.0, 1.0]) {
        assert_eq!(target.center, [side * 0.125, 0.07, 0.0]);
        assert_eq!(target.size, [0.225, 0.06, 0.26]);
    }
}

#[test]
fn elephant_and_crocodile_movement_bounds_and_anatomical_targets_grow_together() {
    for (kind, factor, target_height) in [(Kind::Elephant, 1.55, 3.7), (Kind::Crocodile, 1.2, 0.47)]
    {
        let animal = animal(kind);
        let boxes = for_creature(&animal);
        for (i, dimension) in animal.size().into_iter().enumerate() {
            let scale = dimension / model_reference_size(kind)[i];
            assert!((scale - factor).abs() < 1e-9);
            assert!((boxes.scale[i] - scale).abs() < 1e-9);
        }
        assert!(
            boxes
                .ray_distance([0., target_height, -6.], [0., 0., 1.], 10.)
                .is_some()
        );
        assert!(
            boxes
                .ray_distance([3., target_height, -6.], [0., 0., 1.], 10.)
                .is_none()
        );
    }
}

#[test]
fn horse_picking_hits_barrel_head_and_hooves_but_not_the_empty_square_or_leg_gap() {
    let horse = animal(Kind::Horse);
    let boxes = for_creature(&horse);
    for (origin, direction) in [
        ([0.0, 1.0, -3.0], [0.0, 0.0, 1.0]),
        ([0.0, 1.55, -3.0], [0.0, 0.0, 1.0]),
        ([0.175, 0.1, -3.0], [0.0, 0.0, 1.0]),
    ] {
        assert!(boxes.ray_distance(origin, direction, 6.0).is_some());
    }
    for origin in [[0.8, 1.0, -3.0], [0.0, 0.1, -3.0], [0.0, 1.8, 0.4]] {
        assert!(
            boxes.ray_distance(origin, [0.0, 0.0, 1.0], 6.0).is_none(),
            "empty body space {origin:?}"
        );
    }
    let angle = 0.83_f64;
    let (sin, cos) = angle.sin_cos();
    let rotate = |p: [f64; 3]| [p[0] * cos + p[2] * sin, p[1], -p[0] * sin + p[2] * cos];
    let mut turned = horse;
    turned.yaw = angle as f32;
    let turned = for_creature(&turned);
    assert!(
        turned
            .ray_distance(rotate([0.8, 1.0, -3.0]), rotate([0.0, 0.0, 1.0]), 6.0)
            .is_none()
    );
    assert!(
        turned
            .ray_distance(rotate([0.0, 1.0, -3.0]), rotate([0.0, 0.0, 1.0]), 6.0)
            .is_some()
    );
}

#[test]
fn anatomical_boxes_scale_with_real_birth_age_instead_of_retaining_adult_hits() {
    let mut horse = animal(Kind::Horse);
    horse.id = breeding::next_birth_id(&Default::default(), 6).unwrap();
    assert!(horse.juvenile());
    let baby = for_creature(&horse);
    assert!(
        baby.ray_distance([0.0, 1.0, -3.0], [0.0, 0.0, 1.0], 6.0)
            .is_none()
    );
    horse.age = breeding::GROWTH_SECONDS;
    let adult = for_creature(&horse);
    assert_eq!(baby.count, adult.count);
    for (baby, adult) in baby.iter().zip(adult.iter()) {
        for i in 0..3 {
            assert!((baby.size[i] - adult.size[i] * 0.45).abs() < 1e-10);
            assert!((baby.center[i] - adult.center[i] * 0.45).abs() < 1e-10);
        }
    }
    assert!(
        adult
            .ray_distance([0.0, 1.0, -3.0], [0.0, 0.0, 1.0], 6.0)
            .is_some()
    );
}

#[test]
fn spider_body_box_tracks_wall_and_ceiling_orientation_around_the_actual_pivot() {
    for kind in [Kind::SmallSpider, Kind::MediumSpider, Kind::LargeSpider] {
        let mut spider = animal(kind);
        spider.position = [-3.0, -30.0, 7.0];
        for behavior in [
            CreatureBehavior::Crawl,
            CreatureBehavior::Climb,
            CreatureBehavior::Ceiling,
        ] {
            spider.behavior = behavior;
            spider.yaw = std::f32::consts::FRAC_PI_2;
            let boxes = for_creature(&spider);
            assert_eq!(boxes.count, 18);
            let body = boxes.boxes[0];
            let normal = crate::creatures::spiders::surface_normal(&spider).map(f64::from);
            for (axis, normal) in body.axes[1].iter().zip(normal) {
                assert!((axis - normal).abs() < 1e-6);
            }
            let expected_center = crate::creatures::spiders::visual_center(&spider);
            let height_offset = kind.size()[1] * (0.85 - 0.675);
            for (i, normal) in normal.iter().enumerate() {
                assert!(
                    (body.center[i] - expected_center[i] - normal * height_offset).abs() < 1e-6
                );
            }
            let origin = std::array::from_fn(|i| body.center[i] + body.axes[1][i] * 4.0);
            assert!(
                boxes
                    .ray_distance(origin, body.axes[1].map(|v| -v), 8.0)
                    .is_some()
            );
        }
    }
}

#[test]
fn bounded_species_boxes_have_finite_orthogonal_axes_and_reject_invalid_rays() {
    for kind in Kind::ALL {
        let mut a = animal(kind);
        if kind == Kind::SandWorm {
            a.behavior = CreatureBehavior::Attack;
        }
        a.yaw = 1.27;
        let boxes = for_creature(&a);
        assert!((1..=CAPACITY).contains(&boxes.count));
        for box_ in boxes.iter() {
            assert!(box_.center.into_iter().chain(box_.size).all(f64::is_finite));
            assert!(box_.size.into_iter().all(|v| v > 0.0));
            for i in 0..3 {
                assert!((dot(box_.axes[i], box_.axes[i]) - 1.0).abs() < 1e-10);
                assert!(dot(box_.axes[i], box_.axes[(i + 1) % 3]).abs() < 1e-10);
            }
        }
        assert!(boxes.ray_distance([0.0; 3], [f64::NAN; 3], 3.0).is_none());
        assert!(
            boxes
                .ray_distance([0.0; 3], [1.0, 0.0, 0.0], -1.0)
                .is_none()
        );
    }
}

#[test]
fn buried_worm_anatomy_is_not_pickable_until_it_emerges() {
    let mut a = animal(Kind::SandWorm);
    for behavior in [CreatureBehavior::Burrowed, CreatureBehavior::Tremor] {
        a.behavior = behavior;
        assert_eq!(for_creature(&a).count, 0);
        assert!(
            for_creature(&a)
                .ray_distance([0.0, 0.18, -4.0], [0.0, 0.0, 1.0], 40.0)
                .is_none()
        );
    }
    a.behavior = CreatureBehavior::Attack;
    assert!(
        (7..=12).contains(&for_creature(&a).count),
        "the buried rear has no attack hitboxes"
    );
    for [x, y] in [[0.87, 6.25], [-0.77, 6.85], [0.12, 7.5]] {
        assert!(
            for_creature(&a)
                .ray_distance([x, y, -4.0], [0.0, 0.0, 1.0], 8.0)
                .is_some(),
            "visible upright maw must be hittable at {x},{y}"
        );
    }
    a.timer = super::super::sandworm::EXPOSED_SECONDS * (1.0 - super::super::sandworm::BITE_PHASE);
    let head = super::super::sandworm::pose(&a, super::super::sandworm::HEAD_SEGMENT);
    assert!(head.center[1] < 1.51);
    assert!(
        for_creature(&a)
            .ray_distance([0.0, 1.3, -4.0], [0.0, 0.0, 1.0], 8.0)
            .is_some()
    );
}

#[test]
fn airborne_and_diving_worm_targets_follow_the_shared_authoritative_spine() {
    let mut a = animal(Kind::SandWorm);
    for (behavior, duration) in [
        (CreatureBehavior::Leap, super::super::sandworm::LEAP_SECONDS),
        (
            CreatureBehavior::Emerge,
            super::super::sandworm::EMERGE_SECONDS,
        ),
        (
            CreatureBehavior::Retreat,
            super::super::sandworm::RETREAT_SECONDS,
        ),
    ] {
        a.behavior = behavior;
        for t in [0.15, 0.4, 0.6, 0.9] {
            a.timer = duration * (1.0 - t);
            let boxes = for_creature(&a);
            let expected = super::super::sandworm::pose(&a, super::super::sandworm::HEAD_SEGMENT);
            if !super::super::sandworm::anatomy_visible(
                behavior,
                expected,
                super::super::sandworm::HEAD_SEGMENT,
            ) {
                assert!(boxes.iter().all(|b| {
                    b.center
                        .iter()
                        .zip(expected.center)
                        .any(|(x, y)| (*x - f64::from(y)).abs() > 0.001)
                }));
                continue;
            }
            let head = boxes.iter().last().unwrap();
            assert!(
                head.center
                    .iter()
                    .zip(super::super::sandworm::target_center(
                        expected,
                        super::super::sandworm::HEAD_SEGMENT
                    ))
                    .all(|(x, y)| (*x - f64::from(y)).abs() < 1e-5)
            );
            assert!(
                boxes
                    .ray_distance(
                        [head.center[0], head.center[1], head.center[2] - 4.0],
                        [0.0, 0.0, 1.0],
                        8.0,
                    )
                    .is_some()
            );
        }
    }
}

#[test]
fn guardian_target_rises_from_the_rubble_with_its_replicated_wake_phase() {
    let mut golem = animal(Kind::StoneGolem);
    golem.behavior = CreatureBehavior::Sleep;
    let can_hit = |a: &Creature, x, y| {
        for_creature(a)
            .ray_distance([x, y, -4.], [0., 0., 1.], 8.)
            .is_some()
    };
    assert!(can_hit(&golem, 1.95, 0.5));
    assert!(!can_hit(&golem, 0., 2.));
    golem.behavior = CreatureBehavior::Emerge;
    golem.timer = super::super::golem::WAKE_SECONDS * 0.5;
    assert!(can_hit(&golem, 0., 2.));
    assert!(!can_hit(&golem, 0., 3.5));
    golem.timer = 0.;
    assert!(can_hit(&golem, 0., 3.5));
    assert!(!can_hit(&golem, 1.95, 0.5));
}
