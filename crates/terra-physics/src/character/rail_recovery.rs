//! A stationary builder can settle above deeply intersecting new rail geometry.
use super::*;
use avian3d::collision::collider::contact_query::contact;
use terra_core::block_shapes::ShapeBox;

const MAX_SHAPES: usize = 2048;
const MIN_LIFT: f32 = 0.45;
const SKIN: f32 = 0.015;

fn penetrates(capsule: &Collider, position: Vec3, bounds: ShapeBox) -> bool {
    let low = Vec3::from_array(bounds.minimum.map(|value| value as f32));
    let high = Vec3::from_array(bounds.maximum.map(|value| value as f32));
    let size = high - low;
    contact(
        capsule,
        position,
        Quat::IDENTITY,
        &Collider::cuboid(size.x, size.y, size.z),
        (low + high) * 0.5,
        Quat::IDENTITY,
        0.0,
    )
    .is_ok_and(|contact| contact.is_some_and(|contact| contact.penetration > 0.005))
}

impl CharacterSimulation<'_, '_> {
    pub(super) fn recover_rail_support(
        &self,
        entity: Entity,
        collider: &Collider,
        state: &mut CharacterState,
        input: CharacterInput,
    ) {
        // Walking, falling and jumping retain the ordinary motor. This handles
        // initial overlap around a stationary actor, including validated hover.
        if !state.position.is_finite()
            || !state.velocity.is_finite()
            || state.velocity.length_squared() > 0.04
            || input.direction.length_squared() > 0.0001
            || input.jump
        {
            return;
        }
        let half = Vec3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS);
        let low = (state.position - half).floor().as_i64vec3();
        let high = (state.position + half - Vec3::splat(0.0001))
            .floor()
            .as_i64vec3();
        // Ordinary stationary characters never pay for rail contacts, support
        // rays or extra readiness checks when their local cells contain no rail.
        if !(low.x..=high.x).any(|x| {
            (low.y..=high.y).any(|y| {
                (low.z..=high.z).any(|z| {
                    self.chunks
                        .block_at([x, y, z])
                        .is_some_and(terra_core::rails::is_rail)
                })
            })
        }) {
            return;
        }
        if !self.collision_ready(state.position - half, state.position + half) {
            return;
        }
        let feet = state.position.y - PLAYER_HALF_HEIGHT;
        if input.flight_speed.is_none() {
            let origin = [
                f64::from(state.position.x),
                f64::from(feet + 0.08),
                f64::from(state.position.z),
            ];
            let Some(ground) = terra_world::interaction_shapes::raycast_solid(
                &self.chunks,
                &self.journal,
                &self.properties,
                origin,
                [0.0, -1.0, 0.0],
                0.20,
            ) else {
                return;
            };
            if ground.normal != [0, 1, 0]
                || (feet - (origin[1] as f32 - ground.distance as f32)).abs() > 0.12
            {
                return;
            }
        }
        let mut top = feet;
        let mut blocked = false;
        let mut visited = 0;
        for x in low.x..=high.x {
            for y in low.y..=high.y {
                for z in low.z..=high.z {
                    let cell = [x, y, z];
                    let Some(material) = self.chunks.block_at(cell) else {
                        return;
                    };
                    let rail = terra_core::rails::is_rail(material);
                    terra_world::interaction_shapes::visit_collision_boxes(
                        &self.chunks,
                        &self.journal,
                        &self.properties,
                        cell,
                        |bounds| {
                            visited += 1;
                            if visited > MAX_SHAPES || blocked {
                                return;
                            }
                            if !bounds.intersects(
                                (state.position - half).to_array().map(f64::from),
                                (state.position + half).to_array().map(f64::from),
                            ) {
                                return;
                            }
                            // Shallow rail contact already has normal Avian
                            // depenetration. Avoid constructing contact queries there.
                            if rail && bounds.maximum[1] as f32 - feet <= MIN_LIFT {
                                return;
                            }
                            if penetrates(collider, state.position, bounds) {
                                if rail {
                                    top = top.max(bounds.maximum[1] as f32);
                                } else {
                                    blocked = true;
                                }
                            }
                        },
                    );
                    if blocked || visited > MAX_SHAPES {
                        return;
                    }
                }
            }
        }
        let lift = top + SKIN - feet;
        if lift <= MIN_LIFT || lift > CLIMB_STEP_HEIGHT {
            return;
        }
        let target = state.position + Vec3::Y * lift;
        let margin = half + Vec3::splat(0.02);
        if !self.collision_ready(
            state.position.min(target) - margin,
            state.position.max(target) + margin,
        ) {
            return;
        }
        let filter = SpatialQueryFilter::from_excluded_entities(
            self.characters.iter().chain(std::iter::once(entity)),
        );
        let current_intersections =
            self.spatial
                .shape_intersections(collider, state.position, Quat::IDENTITY, &filter);
        if current_intersections.is_empty()
            || current_intersections
                .iter()
                .any(|entity| self.train_colliders.contains(*entity))
        {
            return;
        }
        // Lift is shorter than the capsule's straight middle. The original and
        // destination capsule union covers its entire vertical sweep. Original
        // nonrail contacts were rejected above; the endpoint must be fully clear.
        if !self
            .spatial
            .shape_intersections(collider, target, Quat::IDENTITY, &filter)
            .is_empty()
        {
            return;
        }
        state.position = target;
        state.velocity.y = 0.0;
    }
}
