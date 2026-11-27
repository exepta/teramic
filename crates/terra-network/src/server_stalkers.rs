//! Ephemeral grab control: no teleport, flight permission or persistent player state.
use super::*;
use terra_core::{ItemKind, PlayerUuid};
use terra_world::creatures::stalker::Pull;

const MAX_GRABS: usize = 128;
const FRESHNESS: Duration = Duration::from_millis(350);

#[derive(Resource, Default)]
pub(super) struct Grips(HashMap<PlayerUuid, (StalkerGrip, Instant)>);

pub(super) fn held_light(peer: &Peer, journal: &EditJournal) -> bool {
    peer.held_slot
        .and_then(|slot| {
            journal
                .gameplay()
                .inventories
                .get(&peer.uuid)?
                .slot(usize::from(slot))
        })
        .is_some_and(|stack| {
            matches!(
                stack.item,
                ItemKind::Block(
                    40 | terra_core::block_shapes::VIOLET_TORCH | terra_core::lantern::LANTERN
                )
            )
        })
}

impl Grips {
    pub(super) fn refresh(&mut self, pull: Pull) {
        let grip = StalkerGrip {
            creature: pull.creature,
            destination: pull.destination,
            remaining_meters: pull.remaining_meters,
        };
        if grip.valid() && (self.0.contains_key(&pull.player) || self.0.len() < MAX_GRABS) {
            self.0.insert(pull.player, (grip, Instant::now()));
        }
    }

    pub(super) fn retain(&mut self, alive: impl Fn(PlayerUuid) -> bool) {
        self.0
            .retain(|uuid, (_, at)| at.elapsed() <= FRESHNESS && alive(*uuid));
    }

    pub(super) fn release(&mut self, uuid: PlayerUuid) {
        self.0.remove(&uuid);
    }

    pub(super) fn get(&self, uuid: PlayerUuid) -> Option<StalkerGrip> {
        self.0
            .get(&uuid)
            .filter(|(_, at)| at.elapsed() <= FRESHNESS)
            .map(|(grip, _)| *grip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_grabs_expire_and_cannot_survive_disconnect_or_explicit_release() {
        let uuid = PlayerUuid::default();
        let grip = StalkerGrip {
            creature: 1,
            destination: [2.0; 3],
            remaining_meters: 40.0,
        };
        let mut grips = Grips::default();
        grips
            .0
            .insert(uuid, (grip, Instant::now() - Duration::from_secs(1)));
        assert!(grips.get(uuid).is_none());
        grips.retain(|_| true);
        assert!(grips.0.is_empty());
        grips.0.insert(uuid, (grip, Instant::now()));
        assert_eq!(grips.get(uuid), Some(grip));
        grips.retain(|_| false);
        assert!(grips.0.is_empty());
        grips.0.insert(uuid, (grip, Instant::now()));
        grips.release(uuid);
        assert!(grips.get(uuid).is_none());
    }

    #[test]
    fn grip_motion_never_grants_flight_or_exceeds_walk_speed() {
        let grip = StalkerGrip {
            creature: 1,
            destination: [8.0, 4.0, -8.0],
            remaining_meters: 40.0,
        };
        assert!(grip.valid());
        let movement = grip.movement(Vec3::ZERO, 1.0 / 60.0, 1.0);
        assert!(movement.flight_speed.is_none() && !movement.sprint && movement.auto_climb);
        assert!(
            (movement.direction.length() * terra_physics::character::WALK_SPEED - 2.3).abs()
                < 0.001
        );
        assert!(
            !StalkerGrip {
                destination: [f64::NAN; 3],
                ..grip
            }
            .valid()
        );
        assert!(
            !StalkerGrip {
                remaining_meters: 40.1,
                ..grip
            }
            .valid()
        );
    }

    #[test]
    fn grip_registry_is_bounded_recipient_local_and_invalid_refresh_is_atomic() {
        let mut grips = Grips::default();
        let recipient = PlayerUuid::new_v4();
        let other = PlayerUuid::new_v4();
        let pull = Pull {
            creature: 7,
            player: recipient,
            destination: [-25.5, 8.9, -14.5],
            remaining_meters: 40.0,
        };
        grips.refresh(pull);
        assert_eq!(grips.get(recipient).unwrap().creature, 7);
        assert!(grips.get(other).is_none());
        for remaining_meters in [f32::NAN, f32::INFINITY, -0.1, 40.01] {
            grips.refresh(Pull {
                remaining_meters,
                ..pull
            });
            assert_eq!(grips.get(recipient).unwrap().remaining_meters, 40.0);
        }
        for destination in [[f64::NAN; 3], [f64::INFINITY; 3], [-1_000_001.0; 3]] {
            grips.refresh(Pull {
                destination,
                ..pull
            });
            assert_eq!(grips.get(recipient).unwrap().destination, pull.destination);
        }
        while grips.0.len() < MAX_GRABS {
            grips.refresh(Pull {
                player: PlayerUuid::new_v4(),
                ..pull
            });
        }
        grips.refresh(Pull {
            player: other,
            ..pull
        });
        assert!(grips.get(other).is_none());
        grips.refresh(Pull {
            remaining_meters: 3.0,
            ..pull
        });
        assert_eq!(grips.get(recipient).unwrap().remaining_meters, 3.0);
        grips.retain(|uuid| uuid != recipient);
        assert!(grips.get(recipient).is_none());
        assert_eq!(grips.0.len(), MAX_GRABS - 1);
    }

    #[test]
    fn grip_endpoints_and_remaining_budget_never_overshoot_even_at_negative_coordinates() {
        use terra_physics::character::WALK_SPEED;
        let origin = Vec3::new(-321.5, 14.9, -654.5);
        let grip = StalkerGrip {
            creature: 7,
            destination: (origin + Vec3::X * 0.051).to_array().map(f64::from),
            remaining_meters: 40.0,
        };
        let movement = grip.movement(origin, 0.1, 1.0);
        let displacement = movement.direction * WALK_SPEED * 0.1;
        assert!(displacement.x > 0.05 && displacement.x <= 0.0511);
        assert!(!movement.jump && movement.flight_speed.is_none());
        let near = StalkerGrip {
            destination: (origin + Vec3::X * 0.04).to_array().map(f64::from),
            ..grip
        };
        assert_eq!(near.movement(origin, 0.1, 1.0).direction, Vec3::ZERO);
        let high = StalkerGrip {
            destination: (origin + Vec3::Y).to_array().map(f64::from),
            ..grip
        };
        assert!(
            !high.movement(origin, 0.1, 1.0).jump,
            "standing directly beneath a target cannot repeatedly jump"
        );
        for dt in [1.0 / 60.0, 1.0 / 30.0, 0.1] {
            let remaining = StalkerGrip {
                destination: (origin + Vec3::X * 10.0).to_array().map(f64::from),
                remaining_meters: 0.01,
                ..grip
            };
            assert!(
                (remaining.movement(origin, dt, 1.0).direction * WALK_SPEED * dt).length()
                    <= 0.010001
            );
        }
        for dt in [0.0, -0.01, 0.1001, f32::NAN, f32::INFINITY] {
            let stopped = grip.movement(origin, dt, 1.0);
            assert_eq!(stopped.direction, Vec3::ZERO);
            assert!(!stopped.jump && stopped.flight_speed.is_none());
        }
        assert_eq!(
            StalkerGrip {
                remaining_meters: 0.0,
                ..grip
            }
            .movement(origin, 0.1, 1.0)
            .direction,
            Vec3::ZERO
        );
        assert_eq!(
            grip.movement(Vec3::splat(f32::NAN), 0.1, 1.0).direction,
            Vec3::ZERO
        );
    }

    #[test]
    fn status_speed_cannot_exceed_grab_speed_or_the_actual_endpoint_and_budget() {
        use terra_physics::character::WALK_SPEED;
        let origin = Vec3::new(-321.5, 14.9, -654.5);
        let grip = StalkerGrip {
            creature: 7,
            destination: (origin + Vec3::X * 0.051).to_array().map(f64::from),
            remaining_meters: 0.06,
        };
        for effect_speed in [0.01, 0.15, 0.5, 1.0, 2.0, 4.0] {
            let input = grip.movement(origin, 0.1, effect_speed);
            let speed = input.direction.length() * WALK_SPEED * effect_speed;
            assert!(speed <= 2.30001);
            assert!(speed * 0.1 <= 0.0511);
            assert!(input.direction.length() <= 1.0);
        }
        for effect_speed in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::MAX] {
            let stopped = grip.movement(origin, 0.1, effect_speed);
            assert_eq!(stopped.direction, Vec3::ZERO);
            assert!(!stopped.jump && stopped.flight_speed.is_none());
        }
    }

    #[test]
    fn raised_grab_targets_use_collision_step_climbing_without_repeated_jump_input() {
        let grip = StalkerGrip {
            creature: 7,
            destination: [-31.0, 9.9, -13.0],
            remaining_meters: 40.0,
        };
        let mut position = Vec3::new(-32.0, 8.9, -13.0);
        for _ in 0..120 {
            let movement = grip.movement(position, 1.0 / 60.0, 1.0);
            assert!(!movement.jump);
            assert!(movement.flight_speed.is_none());
            if movement.direction.length_squared() > 0.0 {
                assert!(movement.auto_climb);
            }
            position += movement.direction * terra_physics::character::WALK_SPEED / 60.0;
        }
        assert!((position.x - grip.destination[0] as f32).abs() <= 0.05);
    }
}
