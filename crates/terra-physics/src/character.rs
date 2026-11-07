//! Shared kinematic simulation for authoritative servers and client prediction.
use crate::TerrainCollisionState;
use avian3d::prelude::*;
use bevy::{ecs::system::SystemParam, prelude::*};
use terra_world::{edits::EditJournal, streaming::ChunkStore};

pub const PLAYER_RADIUS: f32 = 0.32;
pub const PLAYER_HALF_HEIGHT: f32 = 0.90;
pub const PLAYER_EYE_OFFSET: f32 = 0.72;

/// Shared picking against the actual capsule used by the character motor.
pub fn melee_ray_distance(body: Vec3, origin: Vec3, direction: Vec3, reach: f64) -> Option<f64> {
    static CAPSULE: std::sync::LazyLock<Collider> = std::sync::LazyLock::new(|| {
        Collider::capsule(PLAYER_RADIUS, (PLAYER_HALF_HEIGHT - PLAYER_RADIUS) * 2.0)
    });
    if !body.is_finite()
        || !origin.is_finite()
        || !direction.is_finite()
        || (direction.length_squared() - 1.0).abs() > 0.001
        || !reach.is_finite()
        || reach < 0.0
        || reach > f64::from(f32::MAX)
    {
        return None;
    }
    CAPSULE
        .cast_ray(body, Quat::IDENTITY, origin, direction, reach as f32, true)
        .map(|(distance, _)| f64::from(distance))
}

pub const WALK_SPEED: f32 = 5.4;
pub const SPRINT_SPEED: f32 = 8.2;
pub const JUMP_SPEED: f32 = 7.8;
const HALF_STEP_HEIGHT: f32 = 0.52;
const CLIMB_STEP_HEIGHT: f32 = 1.02;

#[derive(Component, Debug)]
pub struct CharacterBody;
#[derive(Component, Debug, Default)]
pub struct CharacterGrounded(pub bool);

#[derive(Component, Debug)]
pub struct CharacterPresentation {
    pub previous: Vec3,
    pub correction: Vec3,
}
impl CharacterPresentation {
    pub fn visual_position(&self, current: Vec3, fraction: f32) -> Vec3 {
        self.previous.lerp(current, fraction.clamp(0.0, 1.0)) + self.correction
    }

    /// Preserve the rendered pose while accepting the server's physical position.
    pub fn reconcile(&mut self, current: Vec3, corrected: Vec3) {
        let delta = corrected - current;
        if delta.length_squared() >= 0.36 {
            self.previous = corrected;
            self.correction = Vec3::ZERO;
        } else {
            self.previous += delta;
            self.correction = (self.correction - delta).clamp_length_max(0.6);
        }
    }
}
pub type ControlledBodies<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Transform,
        &'static mut LinearVelocity,
        &'static Collider,
        &'static mut CharacterGrounded,
        Option<&'static mut CharacterPresentation>,
    ),
    With<CharacterBody>,
>;

#[derive(Clone, Copy, Debug, Default)]
pub struct CharacterInput {
    pub direction: Vec3,
    pub jump: bool,
    pub sprint: bool,
    pub auto_climb: bool,
    /// Authorized creative/debug flight; bounded by the server protocol.
    pub flight_speed: Option<f32>,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct CharacterState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub grounded: bool,
}

pub fn body_bundle(position: Vec3) -> impl Bundle {
    (
        CharacterBody,
        CharacterGrounded::default(),
        CharacterPresentation {
            previous: position,
            correction: Vec3::ZERO,
        },
        RigidBody::Kinematic,
        CustomPositionIntegration,
        LockedAxes::ROTATION_LOCKED,
        Collider::capsule(PLAYER_RADIUS, (PLAYER_HALF_HEIGHT - PLAYER_RADIUS) * 2.0),
        LinearVelocity::ZERO,
        Transform::from_translation(position),
        Friction::new(0.0),
        Restitution::new(0.0),
    )
}

pub fn space_clear(chunks: &ChunkStore, position: Vec3) -> bool {
    space_clear_with_shapes(
        chunks,
        &EditJournal::default(),
        &terra_core::block_properties::BlockPropertiesCatalog::default(),
        position,
    )
}

pub fn space_clear_with_shapes(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    position: Vec3,
) -> bool {
    let half = Vec3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS);
    let low = (position - half).floor().as_i64vec3();
    let high = (position + half - Vec3::splat(0.001)).floor().as_i64vec3();
    for z in low.z..=high.z {
        for y in low.y..=high.y {
            for x in low.x..=high.x {
                if terra_world::interaction_shapes::space_intersects(
                    chunks,
                    journal,
                    properties,
                    [x, y, z],
                    (position - half).to_array().map(f64::from),
                    (position + half).to_array().map(f64::from),
                ) {
                    return false;
                }
            }
        }
    }
    true
}

#[derive(SystemParam)]
pub struct CharacterSimulation<'w, 's> {
    profile: Res<'w, crate::CharacterProfile>,
    characters: Query<'w, 's, Entity, With<CharacterBody>>,
    movement: MoveAndSlide<'w, 's>,
    spatial: SpatialQuery<'w, 's>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    collision: Res<'w, TerrainCollisionState>,
    time: Res<'w, Time<Fixed>>,
    gravity: Res<'w, Gravity>,
    config: Res<'w, terra_core::GameConfig>,
    terrain: Option<Res<'w, terra_world::streaming::WorldTerrain>>,
}

impl CharacterSimulation<'_, '_> {
    pub fn riding_position(&self, horse: &terra_core::creatures::Creature) -> Vec3 {
        Vec3::from_array(terra_world::creatures::riding::seat_feet(horse).map(|v| v as f32))
            + Vec3::Y * PLAYER_HALF_HEIGHT
    }
    pub fn ride_step(
        &self,
        horse: &mut terra_core::creatures::Creature,
        stamina: &mut f32,
        input: CharacterInput,
    ) -> bool {
        let Some(terrain) = &self.terrain else {
            return false;
        };
        terra_world::creatures::riding::step(
            horse,
            stamina,
            &terra_world::creatures::Habitat {
                terrain: &terrain.0,
                chunks: &self.chunks,
                journal: &self.journal,
            },
            terra_world::creatures::riding::RideInput {
                direction: [input.direction.x, input.direction.z],
                jump: input.jump,
                sprint: input.sprint,
            },
            self.delta_seconds(),
        )
    }
    pub fn dismount(&self, horse: &terra_core::creatures::Creature) -> Option<Vec3> {
        let terrain = self.terrain.as_ref()?;
        terra_world::creatures::riding::dismount(
            horse,
            &terra_world::creatures::Habitat {
                terrain: &terrain.0,
                chunks: &self.chunks,
                journal: &self.journal,
            },
        )
        .map(|p| Vec3::from_array(p.map(|v| v as f32)) + Vec3::Y * PLAYER_HALF_HEIGHT)
    }
    pub fn delta_seconds(&self) -> f32 {
        self.time.delta_secs()
    }
    pub fn head_underwater(&self, position: Vec3) -> bool {
        let Some(terrain) = &self.terrain else {
            return false;
        };
        head_underwater(
            &self.config.world,
            &terrain.0,
            &self.chunks,
            &self.journal,
            position,
        )
    }
    /// Replay the same stamina-limited movement on the authority and predicting client.
    pub fn survival_step(
        &self,
        entity: Entity,
        collider: &Collider,
        state: &mut CharacterState,
        mut input: CharacterInput,
        mode: terra_core::GameMode,
        vitals: &mut terra_core::SurvivalVitals,
    ) -> bool {
        if mode == terra_core::GameMode::Spectator {
            state.velocity = input.direction.clamp_length_max(1.0)
                * input.flight_speed.unwrap_or(0.0).clamp(0.0, 60.0);
            state.position += state.velocity * self.time.delta_secs();
            state.grounded = false;
            return true;
        }
        if mode == terra_core::GameMode::Creative {
            return self.step(entity, collider, state, input);
        }
        if vitals.dead() {
            return false;
        }
        let swimming = self.swimming_at(state.position);
        let submerged = self.head_underwater(state.position);
        input.sprint &= submerged || vitals.can_sprint();
        // Exhaustion never prevents surfacing or climbing out of water.
        if !swimming {
            input.jump &= vitals.can_jump();
        }
        let before = *state;
        let movement_multiplier = vitals.effects.movement_multiplier(swimming);
        if !self.step_with_movement(entity, collider, state, input, movement_multiplier) {
            return false;
        }
        let moved = state.position.distance_squared(before.position) > 0.0000001;
        vitals.advance_effort(
            terra_core::SurvivalEffort {
                moving: moved
                    && (input.direction.length_squared() > 0.001 || (swimming && input.jump)),
                sprinting: input.sprint,
                swimming,
                submerged,
                jumped: !swimming && before.grounded && input.jump && !state.grounded,
            },
            self.delta_seconds(),
        );
        true
    }

    pub fn swimming_at(&self, position: Vec3) -> bool {
        self.terrain.as_ref().is_some_and(|terrain| {
            in_water(
                &self.config.world,
                &terrain.0,
                &self.chunks,
                &self.journal,
                position,
            )
        })
    }
    /// False means collision data is not ready. Never advance through missing terrain.
    pub fn step(
        &self,
        entity: Entity,
        collider: &Collider,
        state: &mut CharacterState,
        input: CharacterInput,
    ) -> bool {
        self.step_with_movement(entity, collider, state, input, 1.0)
    }

    fn flight_step(
        &self,
        entity: Entity,
        collider: &Collider,
        state: &mut CharacterState,
        direction: Vec3,
        speed: f32,
    ) -> bool {
        let target = direction.clamp_length_max(1.0) * speed.clamp(0.0, 60.0);
        let velocity = state.velocity
            + (target - state.velocity) * (1.0 - (-10.0 * self.time.delta_secs()).exp());
        let next = state.position + velocity * self.time.delta_secs();
        let half = Vec3::new(
            PLAYER_RADIUS + 0.1,
            PLAYER_HALF_HEIGHT + 0.15,
            PLAYER_RADIUS + 0.1,
        );
        if !self.collision.ready_for_aabb(
            state.position.min(next) - half,
            state.position.max(next) + half,
            &self.chunks,
            &self.journal,
        ) {
            return false;
        }
        let filter = SpatialQueryFilter::from_excluded_entities(
            self.characters.iter().chain(std::iter::once(entity)),
        );
        let output = self.movement.move_and_slide(
            collider,
            state.position,
            Quat::IDENTITY,
            velocity,
            self.time.delta(),
            &MoveAndSlideConfig::default(),
            &filter,
            |_| MoveAndSlideHitResponse::Accept,
        );
        state.position = output.position;
        state.velocity = output.projected_velocity;
        state.grounded = false;
        true
    }

    fn step_with_movement(
        &self,
        entity: Entity,
        collider: &Collider,
        state: &mut CharacterState,
        input: CharacterInput,
        movement_multiplier: f32,
    ) -> bool {
        let _scope = terra_core::profiling::ScopeTimer::new("character.step");
        let _sample = self.profile.measure();
        let previous = *state;
        if let Some(speed) = input.flight_speed {
            return self.flight_step(entity, collider, state, input.direction, speed);
        }
        let filter = SpatialQueryFilter::from_excluded_entities(
            self.characters.iter().chain(std::iter::once(entity)),
        );
        let ground_normal = {
            let _scope = terra_core::profiling::ScopeTimer::new("character.ground");
            let ground_shape = Collider::capsule(
                PLAYER_RADIUS - 0.02,
                (PLAYER_HALF_HEIGHT - PLAYER_RADIUS) * 2.0,
            );
            (state.velocity.y <= 0.1)
                .then(|| {
                    self.spatial.cast_shape(
                        &ground_shape,
                        state.position,
                        Quat::IDENTITY,
                        Dir3::NEG_Y,
                        &ShapeCastConfig::from_max_distance(0.12),
                        &filter,
                    )
                })
                .flatten()
                .filter(|hit| hit.normal1.y > 0.65)
                .map(|hit| hit.normal1)
        };
        state.grounded = ground_normal.is_some();
        let direction = Vec3::new(input.direction.x, 0.0, input.direction.z).clamp_length_max(1.0);
        let desired = direction
            * movement_multiplier
            * if input.sprint {
                SPRINT_SPEED
            } else {
                WALK_SPEED
            };
        state.velocity.x = desired.x;
        state.velocity.z = desired.z;
        let water = {
            let _scope = terra_core::profiling::ScopeTimer::new("character.water");
            self.terrain.as_ref().and_then(|terrain| {
                swimming_column(
                    &self.config.world,
                    &terrain.0,
                    &self.chunks,
                    &self.journal,
                    state.position,
                )
            })
        };
        let motion = {
            let _scope = terra_core::profiling::ScopeTimer::new("character.current");
            self.terrain.as_ref().map_or(
                terra_world::water::FluidMotion {
                    velocity: [0.0; 3],
                    immersion: 0.0,
                },
                |terrain| {
                    let half = Vec3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS);
                    crate::current::body_motion(
                        &terrain.0,
                        &self.chunks,
                        &self.journal,
                        state.position - half,
                        state.position + half,
                    )
                },
            )
        };
        let swimming = water.is_some() || (motion.immersion > 0.0 && motion.velocity[1] < -0.1);
        let current = crate::current::advection(motion);
        if swimming {
            let surface = water.map_or(state.position.y + 0.25, |column| column.level as f32);
            let surface_jump = input.jump && water.is_some() && state.position.y > surface - 0.4;
            let swim = (if surface_jump {
                // Keep a short surface boost until the feet clear water, allowing shore exits.
                9.0
            } else if input.jump {
                3.8
            } else if input.sprint {
                // Diving is latched by the client input and replayed identically by the authority.
                input.direction.y * 3.8
            } else if input.direction.y.abs() > 0.05 {
                input.direction.y * 3.8
            } else {
                // Releasing descent brakes at the current depth, rather than
                // steering every submerged swimmer back to the surface.
                0.0
            }) * if surface_jump {
                1.0
            } else {
                movement_multiplier
            } + current.y;
            if surface_jump {
                state.velocity.y = swim;
            } else {
                state.velocity.y +=
                    (swim - state.velocity.y) * (1.0 - (-12.0 * self.time.delta_secs()).exp());
            }
            state.velocity.x *= 0.72;
            state.velocity.z *= 0.72;
            state.grounded = false;
        } else if input.jump && state.grounded {
            state.velocity.y = JUMP_SPEED;
            state.grounded = false;
        }
        if !swimming {
            state.velocity.y = (state.velocity.y
                + self.gravity.0.y * (24.0 / 9.81) * self.time.delta_secs())
            .max(-40.0);
        }
        // Add the same flow to idle swimming and active counter-movement on both
        // the authority and client prediction. Camera position is never sampled.
        state.velocity.x += current.x;
        state.velocity.z += current.z;
        let mut carry = if state.grounded && !swimming {
            Vec3::from_array(terra_world::conveyors::surface_velocity(
                &self.chunks,
                &self.journal,
                (state.position - Vec3::Y * PLAYER_HALF_HEIGHT)
                    .as_dvec3()
                    .to_array(),
                f64::from(PLAYER_RADIUS),
            ))
        } else {
            Vec3::ZERO
        };
        let horizontal_carry = Vec3::new(carry.x, 0.0, carry.z);
        let horizontal = Vec3::new(state.velocity.x, 0.0, state.velocity.z) + horizontal_carry;
        if carry.y != 0.0 && horizontal_carry.length_squared() > 0.000001 {
            // Ramp height follows the net tangent travel, including counterwalking.
            let tangent_y =
                carry.y * horizontal.dot(horizontal_carry) / horizontal_carry.length_squared();
            carry.y = tangent_y;
            // Press into the actual support without adding a downhill gravity component.
            if let Some(normal) = ground_normal {
                carry += normal * (state.velocity.y / normal.y) - Vec3::Y * state.velocity.y;
            }
        }
        let velocity = state.velocity + carry;
        let next = state.position + velocity * self.time.delta_secs();
        let half = Vec3::new(
            PLAYER_RADIUS + 0.1,
            PLAYER_HALF_HEIGHT + 0.15,
            PLAYER_RADIUS + 0.1,
        );
        if !self.collision.ready_for_aabb(
            state.position.min(next) - half,
            state.position.max(next) + half,
            &self.chunks,
            &self.journal,
        ) {
            // A collider rebuild pauses this tick; it must not cancel a jump
            // or leave authority/prediction with a partially simulated state.
            *state = previous;
            return false;
        }
        let before = state.position;
        let grounded = state.grounded;
        let output = {
            let _scope = terra_core::profiling::ScopeTimer::new("character.move");
            self.movement.move_and_slide(
                collider,
                state.position,
                Quat::IDENTITY,
                velocity,
                self.time.delta(),
                &MoveAndSlideConfig::default(),
                &filter,
                |hit| {
                    if hit.normal.y > 0.65 && state.velocity.y <= 0.0 {
                        state.grounded = true;
                    }
                    MoveAndSlideHitResponse::Accept
                },
            )
        };
        // Normal support pressure is not an attempt to walk up an obstacle.
        let attempted = horizontal * self.time.delta_secs();
        let traveled = Vec3::new(
            output.position.x - before.x,
            0.0,
            output.position.z - before.z,
        );
        if (grounded || (swimming && input.jump))
            && attempted.length_squared() > 0.00001
            && traveled.length_squared() < attempted.length_squared() * 0.96
        {
            let _scope = terra_core::profiling::ScopeTimer::new("character.climb");
            let heights: &[f32] = if swimming {
                &[CLIMB_STEP_HEIGHT]
            } else if input.auto_climb {
                &[HALF_STEP_HEIGHT, CLIMB_STEP_HEIGHT]
            } else {
                &[HALF_STEP_HEIGHT]
            };
            for &height in heights {
                let half = Vec3::new(
                    PLAYER_RADIUS + 0.1,
                    PLAYER_HALF_HEIGHT + height + 0.08,
                    PLAYER_RADIUS + 0.1,
                );
                if self.collision.ready_for_aabb(
                    before - half,
                    before + half + attempted,
                    &self.chunks,
                    &self.journal,
                ) {
                    let upward = self.spatial.cast_shape(
                        collider,
                        before,
                        Quat::IDENTITY,
                        Dir3::Y,
                        &ShapeCastConfig {
                            ignore_origin_penetration: true,
                            ..ShapeCastConfig::from_max_distance(height)
                        },
                        &filter,
                    );
                    if upward.is_none() {
                        let raised = before + Vec3::Y * height;
                        let stepped = self.movement.move_and_slide(
                            collider,
                            raised,
                            Quat::IDENTITY,
                            horizontal,
                            self.time.delta(),
                            &MoveAndSlideConfig::default(),
                            &filter,
                            |_| MoveAndSlideHitResponse::Accept,
                        );
                        let gain = Vec3::new(
                            stepped.position.x - before.x,
                            0.0,
                            stepped.position.z - before.z,
                        );
                        if gain.length_squared() > traveled.length_squared() + 0.00001 {
                            // Probe the leading foot, not the capsule's rounded lip at a step edge.
                            let ground = Collider::cuboid(0.08, 0.02, 0.08);
                            let probe = stepped.position + direction * (PLAYER_RADIUS * 0.85);
                            if let Some(hit) = self.spatial.cast_shape(
                                &ground,
                                probe,
                                Quat::IDENTITY,
                                Dir3::NEG_Y,
                                &ShapeCastConfig::from_max_distance(
                                    PLAYER_HALF_HEIGHT + height + 0.02,
                                ),
                                &filter,
                            ) && hit.normal1.y > 0.65
                                && hit.distance >= PLAYER_HALF_HEIGHT - 0.02
                            {
                                state.position = stepped.position
                                    - Vec3::Y * (hit.distance - PLAYER_HALF_HEIGHT - 0.02).max(0.0);
                                state.velocity = Vec3::new(
                                    stepped.projected_velocity.x,
                                    0.0,
                                    stepped.projected_velocity.z,
                                );
                                state.grounded = true;
                                return true;
                            }
                        }
                    }
                }
            }
        }
        state.position = output.position;
        state.velocity = output.projected_velocity;
        if grounded && carry != Vec3::ZERO {
            // Ground advection is not a jump or accumulated downhill acceleration.
            state.velocity.y = 0.0;
        }
        true
    }
}

#[cfg(test)]
#[path = "character/step_tests.rs"]
mod step_tests;

pub fn in_water(
    config: &terra_core::WorldConfig,
    terrain: &terra_world::TerraGenerator,
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: Vec3,
) -> bool {
    swimming_column(config, terrain, chunks, journal, position).is_some()
}

/// Breath follows the actual player's head, never a third-person camera.
pub fn head_underwater(
    config: &terra_core::WorldConfig,
    terrain: &terra_world::TerraGenerator,
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: Vec3,
) -> bool {
    if !config.water || !position.is_finite() {
        return false;
    }
    let head = position + Vec3::Y * PLAYER_EYE_OFFSET;
    chunks
        .water_column_at_point(terrain, journal, head.as_dvec3().to_array())
        .is_some_and(|water| {
            f64::from(head.y) < water.level - 0.05 && f64::from(head.y) > water.bed
        })
}

pub fn swimming_column(
    config: &terra_core::WorldConfig,
    terrain: &terra_world::TerraGenerator,
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: Vec3,
) -> Option<terra_world::water::WaterColumn> {
    if !config.water || !position.is_finite() {
        return None;
    }
    let probe = position - Vec3::Y * (PLAYER_HALF_HEIGHT - 0.35);
    let water = chunks
        .water_column_at_point(terrain, journal, probe.as_dvec3().to_array())
        .or_else(|| {
            chunks.water_column_at_point(terrain, journal, position.as_dvec3().to_array())
        })?;
    (position.y - PLAYER_HALF_HEIGHT < water.level as f32 - 0.35
        && position.y + PLAYER_HALF_HEIGHT > water.bed as f32 + 0.1)
        .then_some(water)
}

#[cfg(test)]
mod water_tests {
    use super::*;
    use terra_voxel::{SectionPos, blocks::BlockSection};
    use terra_world::{streaming::LoadedSection, water::FluidCell};

    #[test]
    fn swimming_uses_live_fill_levels_and_does_not_cross_solid_floors() {
        let config = terra_core::WorldConfig::default();
        let terrain = terra_world::TerraGenerator::new(config.clone()).unwrap();
        let owner = SectionPos::new(-1, 12, 0);
        let mut data = BlockSection::empty(owner).unwrap();
        data.set_block([15, 0, 4], 3).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut journal = EditJournal::default();
        let position = Vec3::new(-0.5, 193.8, 4.5);
        assert!(!in_water(&config, &terrain, &chunks, &journal, position));
        journal
            .set_fluid(
                [-1, 193, 4],
                FluidCell {
                    amount: 8,
                    flow: [1, 0],
                    source: false,
                },
            )
            .unwrap();
        journal
            .set_fluid(
                [-1, 194, 4],
                FluidCell {
                    amount: 4,
                    flow: [1, 0],
                    source: false,
                },
            )
            .unwrap();
        let water = swimming_column(&config, &terrain, &chunks, &journal, position).unwrap();
        assert_eq!(water.level, 194.5);
        assert_eq!(water.bed, 193.0);
        assert!(!head_underwater(
            &config, &terrain, &chunks, &journal, position
        ));
        assert!(head_underwater(
            &config,
            &terrain,
            &chunks,
            &journal,
            position - Vec3::Y * 0.4
        ));
        assert!(!in_water(
            &config,
            &terrain,
            &chunks,
            &journal,
            position - Vec3::Y * 3.0
        ));
        journal
            .set_fluid([-1, 193, 4], FluidCell::default())
            .unwrap();
        journal
            .set_fluid([-1, 194, 4], FluidCell::default())
            .unwrap();
        assert!(!in_water(&config, &terrain, &chunks, &journal, position));
    }
}

#[cfg(test)]
mod basin_swim_tests {
    use super::*;
    use terra_voxel::{BlockSection, global_to_section};
    use terra_world::streaming::LoadedSection;

    #[test]
    fn a_deep_finite_basin_supports_swimming_and_breath_until_drained() {
        let config = terra_core::WorldConfig {
            sea_level: -100.0,
            ..Default::default()
        };
        let terrain = terra_world::TerraGenerator::new(config.clone()).unwrap();
        let shape = terra_core::plumbing::BasinShape {
            origin: [0, 192, 0],
            size: [2, 3, 2],
        };
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for cell in shape.cells() {
            let (section, local) = global_to_section(cell);
            let section = chunks
                .loaded
                .entry(section)
                .or_insert_with(|| LoadedSection {
                    data: BlockSection::empty(section).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                });
            section
                .data
                .set_block(local, terra_core::plumbing::BASIN)
                .unwrap();
        }
        journal
            .replace_confirmed_basins([(shape, shape.capacity())])
            .unwrap();
        let swimmer = Vec3::new(0.6, 193.7, 0.6);
        assert!(swimming_column(&config, &terrain, &chunks, &journal, swimmer).is_some());
        assert!(head_underwater(
            &config, &terrain, &chunks, &journal, swimmer
        ));
        assert!(!head_underwater(
            &config,
            &terrain,
            &chunks,
            &journal,
            Vec3::new(0.05, 193.7, 0.6)
        ));
        journal.replace_confirmed_basins([(shape, 0)]).unwrap();
        assert!(!in_water(&config, &terrain, &chunks, &journal, swimmer));
        assert!(!head_underwater(
            &config, &terrain, &chunks, &journal, swimmer
        ));
    }
}

#[cfg(test)]
mod melee_tests {
    use super::*;
    #[test]
    fn player_melee_uses_rounded_capsule_caps_and_bounded_rays() {
        let body = Vec3::ZERO;
        assert!(melee_ray_distance(body, Vec3::new(0.0, 0.0, 3.0), Vec3::NEG_Z, 4.5).is_some());
        // The upper corner is inside the old bounding box but outside the rounded head.
        assert!(melee_ray_distance(body, Vec3::new(0.31, 0.89, 3.0), Vec3::NEG_Z, 4.5).is_none());
        assert!(melee_ray_distance(body, Vec3::new(0.0, 0.0, 6.0), Vec3::NEG_Z, 4.5).is_none());
        assert!(melee_ray_distance(body, Vec3::ZERO, Vec3::ZERO, 4.5).is_none());
    }
}
