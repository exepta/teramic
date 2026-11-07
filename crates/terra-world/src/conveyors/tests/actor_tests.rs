use super::*;

#[test]
fn actor_contacts_follow_power_and_slope_without_affecting_airborne_or_unloaded_bodies() {
    for end in [[4, 4, 10], [4, 10, 10]] {
        let (mut chunks, mut journal) = fixture();
        let span = connect(&mut chunks, &mut journal, [4, 4, 4], end, 0);
        power(span, &mut journal);
        let segment = segment_at(&journal, span.first).unwrap();
        let mut feet = segment.center();
        feet[1] = segment.surface_height(feet).unwrap();
        let v = surface_velocity(&chunks, &journal, feet, 0.32);
        assert!(v[2].abs() > 0.6, "{v:?}");
        assert_eq!(v[1] > 0.0, end[1] > 4);
        let mut above = feet;
        above[1] += 1.0;
        assert_eq!(surface_velocity(&chunks, &journal, above, 0.32), [0.0; 3]);
        journal.replace_kinetic_states(BTreeMap::new());
        assert_eq!(surface_velocity(&chunks, &journal, feet, 0.32), [0.0; 3]);
        power(span, &mut journal);
        chunks.loaded.clear();
        assert_eq!(surface_velocity(&chunks, &journal, feet, 0.32), [0.0; 3]);
    }
}

#[test]
fn a_passive_ground_mob_rides_a_belt_and_cannot_be_pushed_through_a_wall() {
    use terra_core::creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds};
    let (mut chunks, mut journal) = fixture();
    let span = connect(&mut chunks, &mut journal, [4, 4, 4], [4, 4, 12], 0);
    power(span, &mut journal);
    let terrain = crate::TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
    let segment = segment_at(&journal, [4, 4, 6]).unwrap();
    let mut position = segment.center();
    position[1] = segment.surface_height(position).unwrap();
    let mut mob = Creature {
        id: 1,
        kind: CreatureKind::Pig,
        position,
        velocity: [0.0; 3],
        yaw: 0.0,
        home: [0, 0],
        health: 10.0,
        variant: 0,
        behavior: CreatureBehavior::Rest,
        timer: 0.0,
        age: 0.0,
        needs: CreatureNeeds::full(CreatureKind::Pig),
    };
    let initial = mob.position;
    for _ in 0..10 {
        carry_creature(
            &mut mob,
            &crate::creatures::Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            0.1,
        );
    }
    assert!(
        crate::creatures::distance(initial, mob.position) > 0.7,
        "{mob:?}"
    );
    let direction = if mob.position[2] > initial[2] { 1 } else { -1 };
    let half_depth = mob.size()[2] * 0.5;
    let near_face = if direction > 0 {
        (mob.position[2] + half_depth + 0.25).ceil()
    } else {
        (mob.position[2] - half_depth - 0.25).floor()
    };
    let wall_z = near_face as i64 - if direction > 0 { 0 } else { 1 };
    let before_wall = mob.position;
    assert_eq!(span_at(&journal, span.controller), Some(span));
    assert!(span.loaded(&chunks, &journal));
    assert!(
        surface_velocity(&chunks, &journal, mob.position, mob.size()[0] * 0.5)[2]
            * direction as f32
            > 0.6
    );
    for x in 3..=5 {
        for y in 5..=7 {
            let p = [x, y, wall_z];
            let (section, local) = global_to_section(p);
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block(local, 3)
                .unwrap();
            journal.record(p, 3).unwrap();
        }
    }
    assert_eq!(span_at(&journal, span.controller), Some(span));
    assert!(span.loaded(&chunks, &journal));
    assert!((mob.position[2] - near_face) * direction as f64 + half_depth <= -0.25);
    for _ in 0..20 {
        carry_creature(
            &mut mob,
            &crate::creatures::Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            0.1,
        );
    }
    assert!(
        (mob.position[2] - before_wall[2]) * direction as f64 > 0.5,
        "the active belt must first carry the body toward the wall"
    );
    assert!(
        (mob.position[2] - near_face) * direction as f64 + half_depth <= 1e-6,
        "the full body must remain in front of the wall"
    );
    let stopped = mob.position;
    for _ in 0..2 {
        assert_eq!(span_at(&journal, span.controller), Some(span));
        assert!(span.loaded(&chunks, &journal));
        assert!(
            surface_velocity(&chunks, &journal, mob.position, mob.size()[0] * 0.5)[2]
                * direction as f32
                > 0.6
        );
        carry_creature(
            &mut mob,
            &crate::creatures::Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            0.1,
        );
        assert_eq!(
            mob.position, stopped,
            "collision, not lost belt power, must stop the body"
        );
    }
}
