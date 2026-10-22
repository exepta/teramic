//! Grounded benchmark route using the normal controller and network authority.
use super::*;
use terra_world::streaming::ChunkStore;

const LENGTH: f32 = 64.0;

#[derive(Default)]
pub(super) struct Walk {
    started: Option<Instant>,
    origin: Vec3,
    heading: Vec3,
    length: f32,
    returning: bool,
    last_report: Option<Instant>,
    last_ack: u64,
    ack_at: Option<Instant>,
    last_progress: Option<(Instant, Vec3)>,
    jump_at: Option<Instant>,
    maximum_distance: f32,
    cycles: u64,
}

impl Walk {
    pub(super) fn started(&self) -> bool {
        self.started.is_some()
    }

    pub(super) fn advance(&mut self, world: &mut World) -> Result<(), String> {
        let session = world.resource::<ClientSession>();
        if let Some(error) = &session.error {
            return Err(error.clone());
        }
        if !session.ready || session.teleporting {
            return Ok(());
        }
        let Some(confirmed) = session.confirmed_state() else {
            return Ok(());
        };
        let (ack, authoritative, grounded) =
            (confirmed.ack, confirmed.position, confirmed.grounded);
        let pending = session.pending_inputs;
        let Some(position) = world
            .query_filtered::<&Transform, With<PlayerBody>>()
            .iter(world)
            .next()
            .map(|body| body.translation)
        else {
            return Ok(());
        };
        let now = Instant::now();
        if self.started.is_none() {
            let (heading, length) = corridor(&world.resource::<WorldTerrain>().0, position)?;
            self.heading = heading;
            self.length = length;
            self.origin = position;
            self.started = Some(now);
            self.last_progress = Some((now, position));
            tracing::info!(origin=?position.to_array(), heading=?heading.to_array(), length,
                "Grounded performance route armed; no flight, teleport or terrain edits");
        }
        if self.last_ack != ack {
            self.last_ack = ack;
            self.ack_at = Some(now);
        } else if self
            .ack_at
            .is_some_and(|at| at.elapsed().as_secs_f32() > 15.0)
        {
            return Err(
                "Grounded route stopped receiving authoritative input acknowledgements".into(),
            );
        }
        let offset = position - self.origin;
        let distance = Vec3::new(offset.x, 0.0, offset.z).dot(self.heading);
        self.maximum_distance = self.maximum_distance.max(distance);
        if !self.returning && distance >= self.length - 0.8 {
            self.returning = true;
            tracing::info!(distance, "Grounded performance route returning");
        } else if self.returning && distance <= 0.8 {
            self.returning = false;
            self.cycles += 1;
            tracing::info!(cycles=self.cycles, maximum_distance=self.maximum_distance,
                position=?position.to_array(), authoritative=?authoritative,
                "Grounded performance route completed a real return");
        }
        if let Some((since, old_position)) = self.last_progress {
            let travelled = position - old_position;
            if travelled.x * travelled.x + travelled.z * travelled.z > 0.5 {
                self.last_progress = Some((now, position));
            } else if since.elapsed().as_secs_f32() > 12.0 {
                return Err(format!(
                    "Grounded route blocked at {:?}, travelled {:.1}m",
                    position.to_array(),
                    self.maximum_distance
                ));
            }
        }
        if world.resource::<terra_player::PlayerInputBlock>().0 {
            return Err("Gameplay input blocked during grounded route".into());
        }
        let target = self.origin + self.heading * if self.returning { 0.0 } else { self.length };
        let error = target - position;
        let movement = Vec3::new(error.x, 0.0, error.z).normalize_or_zero();
        let feet = position.y - terra_physics::character::PLAYER_HALF_HEIGHT;
        let ahead = position + movement * 0.8;
        let obstacle = world
            .resource::<ChunkStore>()
            .block_at([
                ahead.x.floor() as i64,
                (feet + 0.15).floor() as i64,
                ahead.z.floor() as i64,
            ])
            .and_then(terra_voxel::MaterialId::from_raw)
            .is_some_and(terra_voxel::MaterialId::is_opaque);
        let jump = grounded
            && obstacle
            && self
                .jump_at
                .is_none_or(|at| at.elapsed().as_secs_f32() > 0.45);
        if jump {
            self.jump_at = Some(now);
        }
        *world.resource_mut::<ControllerMode>() = ControllerMode::Walk;
        world.resource_mut::<CameraInputState>().captured = true;
        let yaw = (-movement.x).atan2(-movement.z);
        for mut camera in world
            .query_filtered::<&mut Transform, With<TerraCamera>>()
            .iter_mut(world)
        {
            camera.rotation = Quat::from_euler(EulerRot::YXZ, yaw, -0.08, 0.0);
        }
        super::inject_bindings(
            world,
            &[
                (GameAction::Forward, true),
                (GameAction::Backward, false),
                (GameAction::Right, false),
                (GameAction::Left, false),
                (GameAction::Jump, jump),
                (GameAction::FlyDown, false),
            ],
        );
        if self
            .last_report
            .is_none_or(|at| at.elapsed().as_secs_f32() >= 10.0)
        {
            self.last_report = Some(now);
            tracing::info!(elapsed=self.started.map_or(0.0, |at| at.elapsed().as_secs_f64()),
                position=?position.to_array(), authoritative=?authoritative, grounded,
                distance, cycles=self.cycles, acknowledged_input=ack, pending_inputs=pending,
                "Grounded performance route progress");
        }
        Ok(())
    }
}

fn corridor(terrain: &terra_world::TerraGenerator, origin: Vec3) -> Result<(Vec3, f32), String> {
    let _scope = terra_core::profiling::ScopeTimer::new("benchmark.walk.corridor");
    let mut best = (Vec3::X, 0.0);
    for [x, z] in [
        [1.0, 0.0],
        [-1.0, 0.0],
        [0.0, 1.0],
        [0.0, -1.0],
        [1.0, 1.0],
        [-1.0, 1.0],
        [1.0, -1.0],
        [-1.0, -1.0],
    ] {
        let heading = Vec3::new(x, 0.0, z).normalize();
        let mut previous = terrain.surface_height(f64::from(origin.x), f64::from(origin.z));
        let mut length = 0.0;
        for step in 1..=LENGTH as i32 {
            let p = origin + heading * step as f32;
            let height = terrain.surface_height(f64::from(p.x), f64::from(p.z));
            if height < terrain.config().sea_level + 1.0 || (height - previous).abs() > 1.1 {
                break;
            }
            previous = height;
            length = step as f32;
        }
        if length > best.1 {
            best = (heading, length);
        }
    }
    if best.1 < 20.0 {
        return Err(format!(
            "No dry grounded corridor >=20m at {:?}; choose a mainland benchmark pose",
            origin.to_array()
        ));
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grounded_route_remains_unstarted_until_real_scene_readiness() {
        assert!(!Walk::default().started());
    }
}
