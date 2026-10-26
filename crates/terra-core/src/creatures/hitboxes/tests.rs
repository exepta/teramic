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
