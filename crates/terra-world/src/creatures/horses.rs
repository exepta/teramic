use super::{Behavior, Creature, Habitat, support};

pub(super) fn launch(animal: &mut Creature, habitat: &Habitat, destination: [f64; 3]) -> bool {
    let Some(traits) = animal.horse_traits() else {
        return false;
    };
    let rise = destination[1] - animal.position[1];
    if !(0.30..=f64::from(traits.jump_height) - 0.05).contains(&rise)
        || !habitat.clear_rotated(destination, animal.size(), false, f64::from(animal.yaw))
        || animal.behavior == Behavior::Rest && animal.timer > 0.0
    {
        return false;
    }
    let dx = destination[0] - animal.position[0];
    let dz = destination[2] - animal.position[2];
    let length = dx.hypot(dz).max(0.001);
    let speed = animal.kind.speed() as f32;
    let vertical_speed = (24.0 * traits.jump_height).sqrt();
    // Sweep the ascent: a blocked body or ceiling is not another jump request.
    let duration = (length / f64::from(speed)).min(f64::from(vertical_speed) / 12.0);
    for i in 1..=16 {
        let t = duration * f64::from(i) / 16.0;
        let distance = (f64::from(speed) * t / length).min(1.0);
        let probe = [
            animal.position[0] + dx * distance,
            animal.position[1] + f64::from(vertical_speed) * t - 6.0 * t * t,
            animal.position[2] + dz * distance,
        ];
        if !habitat.clear_swimmer_rotated(probe, animal.size(), f64::from(animal.yaw)) {
            return false;
        }
    }
    animal.velocity = [
        (dx / length) as f32 * speed,
        vertical_speed,
        (dz / length) as f32 * speed,
    ];
    animal.behavior = Behavior::Leap;
    animal.timer = 2.0;
    animal.needs.food = (animal.needs.food - 0.05).max(0.0);
    true
}

pub(super) fn advance(animal: &mut Creature, habitat: &Habitat, dt: f32) {
    let before = animal.position;
    let count = (animal
        .velocity
        .iter()
        .map(|v| v.abs())
        .fold(0.0_f32, f32::max)
        * dt
        / 0.08)
        .ceil()
        .clamp(1.0, 64.0) as usize;
    let step = f64::from(dt) / count as f64;
    for _ in 0..count {
        // Integrate the ballistic arc within each collision substep. Applying
        // gravity once before a 10-Hz server tick loses enough height to miss ledges.
        let vertical_speed = animal.velocity[1];
        animal.velocity[1] = (vertical_speed - 12.0 * step as f32).max(-16.0);
        let mut vertical = animal.position;
        vertical[1] += f64::from((vertical_speed + animal.velocity[1]) * 0.5) * step;
        if habitat.clear_swimmer_rotated(
            [vertical[0], vertical[1] - 0.019999, vertical[2]],
            animal.size(),
            f64::from(animal.yaw),
        ) {
            animal.position = vertical;
        } else if animal.velocity[1] < 0.0 {
            support::settle(animal, habitat, dt / count as f32);
            animal.behavior = Behavior::Rest;
            animal.timer = 1.25;
            break;
        } else {
            animal.velocity[1] = 0.0;
        }
        let mut horizontal = animal.position;
        horizontal[0] += f64::from(animal.velocity[0]) * step;
        horizontal[2] += f64::from(animal.velocity[2]) * step;
        if habitat.clear_swimmer_rotated(horizontal, animal.size(), f64::from(animal.yaw)) {
            animal.position = horizontal;
        }
    }
    for (axis, start) in before.into_iter().enumerate().filter(|(i, _)| *i != 1) {
        animal.velocity[axis] = ((animal.position[axis] - start) / f64::from(dt)) as f32;
    }
    if animal.timer <= 0.0
        && habitat
            .ground_rotated(animal.position, animal.kind, f64::from(animal.yaw))
            .is_some_and(|p| (p[1] - animal.position[1]).abs() <= 0.025)
    {
        animal.behavior = Behavior::Rest;
        animal.timer = 1.25;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        TerraGenerator,
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection},
    };
    use terra_core::{
        WorldConfig,
        creatures::{CreatureKind, CreatureNeeds, creature_id},
    };
    use terra_voxel::{BlockSection, SectionPos};

    #[test]
    fn horse_jump_respects_its_fixed_height_and_lands_without_crossing_solids() {
        let terrain = TerraGenerator::new(WorldConfig {
            water: false,
            ..Default::default()
        })
        .unwrap();
        let mut chunks = ChunkStore::default();
        let mut data = BlockSection::empty(SectionPos::new(0, 0, 0)).unwrap();
        for z in 0..16 {
            for x in 0..16 {
                data.set_block([x, 2, z], 3).unwrap();
                for y in 3..8 {
                    data.set_block([12, y, z], 3).unwrap();
                }
            }
        }
        chunks.loaded.insert(
            data.position(),
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let mut journal = EditJournal::default();
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut horse = Creature {
            id: creature_id([0, 0], 0).unwrap(),
            home: [0, 0],
            kind: CreatureKind::Horse,
            position: [8.0, 3.0, 8.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 20.0,
            behavior: Behavior::Wander,
            timer: 1.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(CreatureKind::Horse),
        };
        let height = f64::from(horse.horse_traits().unwrap().jump_height);
        assert!(!launch(
            &mut horse,
            &habitat,
            [10.0, 3.0 + height + 0.1, 8.0]
        ));
        assert!(launch(&mut horse, &habitat, [10.0, 4.0, 8.0]));
        let mut population = terra_core::creatures::CreaturePopulation::default();
        let mut navigation = super::super::navigation::Navigation::default();
        let mut peak = horse.position[1];
        for _ in 0..300 {
            if horse.behavior != Behavior::Leap {
                break;
            }
            super::super::ecology::step_with_navigation(
                &mut horse,
                &mut population.ecology,
                &habitat,
                &[],
                1.0 / 60.0,
                false,
                true,
                Some(&mut navigation),
            );
            let y = horse.position[1];
            let id = horse.id;
            population.animals.insert(id, horse.clone());
            super::super::navigation::separate(&mut population, &habitat, &[id], 1.0 / 60.0);
            horse = population.animals.remove(&id).unwrap();
            assert_eq!(
                horse.position[1], y,
                "authoritative separation must preserve the jump arc"
            );
            peak = peak.max(horse.position[1]);
            assert!(horse.valid());
            assert!(habitat.clear_swimmer(horse.position, horse.size()));
        }
        assert!(peak > 4.0 && peak <= 3.0 + height + 0.001);
        assert_eq!(horse.behavior, Behavior::Rest);
        assert!((horse.position[1] - 3.0).abs() < 0.001);
        assert!(horse.position[0] + horse.size()[2] * 0.5 < 12.001);
        let again = [
            horse.position[0],
            horse.position[1] + 1.0,
            horse.position[2],
        ];
        assert!(
            !launch(&mut horse, &habitat, again),
            "landing has a real recovery period"
        );
        let mut navigation = super::super::navigation::Navigation::default();
        assert!(navigation.horse_probe(horse.id, 0.0));
        navigation.horse_jumped(horse.id, 0.0);
        for frame in 1..150 {
            assert!(!navigation.horse_probe(horse.id, f64::from(frame) / 60.0));
        }
        assert!(navigation.horse_probe(horse.id, 2.5));
        // The ordinary server AI must initiate and complete a real ledge jump,
        // including its final separation pass at the actual 10-Hz cadence.
        for x in 10..12 {
            for z in 5..12 {
                chunks
                    .loaded
                    .get_mut(&SectionPos::default())
                    .unwrap()
                    .data
                    .set_block([x, 3, z], 3)
                    .unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        horse.position = [8.0, 3.0, 8.0];
        horse.behavior = Behavior::Wander;
        horse.timer = 10.0;
        horse.yaw = -std::f32::consts::FRAC_PI_2;
        horse.needs = CreatureNeeds::full(CreatureKind::Horse);
        let mut navigation = super::super::navigation::Navigation::default();
        super::super::ecology::step_with_navigation(
            &mut horse,
            &mut population.ecology,
            &habitat,
            &[],
            0.1,
            false,
            false,
            Some(&mut navigation),
        );
        assert_eq!(horse.behavior, Behavior::Leap, "autonomous ledge takeoff");
        let id = horse.id;
        population.animals.insert(id, horse);
        let mut peak = 3.0_f64;
        for _ in 0..30 {
            let animal = population.animals.get_mut(&id).unwrap();
            super::super::ecology::step_with_navigation(
                animal,
                &mut population.ecology,
                &habitat,
                &[],
                0.1,
                false,
                false,
                Some(&mut navigation),
            );
            super::super::navigation::separate(&mut population, &habitat, &[id], 0.1);
            let animal = &population.animals[&id];
            peak = peak.max(animal.position[1]);
            if animal.behavior != Behavior::Leap {
                break;
            }
        }
        horse = population.animals.remove(&id).unwrap();
        assert!(peak > 4.0);
        assert!(
            (horse.position[1] - 4.0).abs() < 0.001,
            "landed on ledge: {:?}",
            horse.position
        );
        // Falling/resting horses settle on the real half-height profile, not
        // on the invisible unit cube or a corner of an oversized square body.
        for x in 6..10 {
            for z in 5..12 {
                chunks
                    .loaded
                    .get_mut(&SectionPos::default())
                    .unwrap()
                    .data
                    .set_block([x, 2, z], 82)
                    .unwrap();
            }
        }
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([8, 4, 9], 82)
            .unwrap();
        journal.record([8, 4, 9], 82).unwrap();
        journal
            .set_object_state(
                [8, 4, 9],
                Some(crate::edits::BlockObjectState {
                    facing: 5,
                    ..Default::default()
                }),
            )
            .unwrap();
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        assert!(habitat.clear_swimmer_rotated([8.85, 4.0, 8.8], horse.size(), 0.0));
        assert!(!habitat.clear_swimmer_rotated(
            [8.85, 4.0, 8.8],
            horse.size(),
            std::f64::consts::FRAC_PI_2
        ));
        horse.position = [8.0, 7.0, 8.0];
        horse.velocity = [0.0; 3];
        horse.behavior = Behavior::Rest;
        horse.timer = 10.0;
        horse.needs = CreatureNeeds::full(CreatureKind::Horse);
        for _ in 0..60 {
            super::super::ecology::step_with_navigation(
                &mut horse,
                &mut population.ecology,
                &habitat,
                &[],
                0.1,
                false,
                false,
                Some(&mut navigation),
            );
            if (horse.position[1] - 2.5).abs() < 0.001 && horse.velocity[1] == 0.0 {
                break;
            }
        }
        assert!(
            (horse.position[1] - 2.5).abs() < 0.001,
            "resting horse hovered: {:?}",
            horse.position
        );
        assert_eq!(horse.velocity[1], 0.0);
        // The endpoint is free but an overhang blocks the ascent.
        for x in 6..=8 {
            for z in 6..=10 {
                chunks
                    .loaded
                    .get_mut(&SectionPos::default())
                    .unwrap()
                    .data
                    .set_block([x, 5, z], 3)
                    .unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        horse.position = [8.0, 3.0, 8.0];
        horse.behavior = Behavior::Wander;
        horse.timer = 1.0;
        let destination = [10.5, 4.0, 8.0];
        assert!(habitat.clear(destination, horse.size(), false));
        for _ in 0..100 {
            assert!(!launch(&mut horse, &habitat, destination));
            assert_eq!(horse.behavior, Behavior::Wander);
        }
    }
}
