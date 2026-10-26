//! Behaviour-edge acceptance for the production squid ink pass, including Low quality.
use super::*;
use terra_core::creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds};
use terra_network::client::{CreatureTrack, ReplicatedCreatures};

#[derive(Default)]
pub(super) struct InkProbe {
    id: Option<u64>,
    baseline: u64,
    frames: usize,
    armed: bool,
    pub finished: bool,
}

pub(super) fn prepare(
    world: &mut World,
    state: &mut ReefSmoke,
    view: Transform,
) -> Result<(), String> {
    *world.resource_mut::<terra_player::ControllerMode>() = terra_player::ControllerMode::Walk;
    if world
        .query_filtered::<Entity, With<terra_player::PlayerBody>>()
        .iter(world)
        .next()
        .is_none()
    {
        // No fake network session and no simulated collider: this actor exists
        // solely to exercise the same physical-head hit test as normal gameplay.
        world.spawn((
            terra_player::PlayerBody,
            Transform::from_translation(
                view.translation - Vec3::Y * terra_player::PLAYER_EYE_OFFSET,
            ),
        ));
    }
    if state.stage == 13 {
        world
            .resource_mut::<terra_core::GameConfig>()
            .client
            .water_quality = terra_core::WaterQuality::Low;
    }
    state.ink.frames = 0;
    state.ink.armed = false;
    state.ink.baseline = world.resource::<terra_render::InkExposure>().hits;
    if state.stage == 12 {
        set_behavior(world, state, CreatureBehavior::Swim)?;
        return Ok(());
    }
    let position = view.translation + *view.forward() * 1.5 - Vec3::Y * 0.475;
    let home = [
        (position.x as i64).div_euclid(16),
        (position.z as i64).div_euclid(16),
    ];
    let id = terra_core::creatures::creature_id(home, 3)
        .ok_or("Squid fixture identity is out of range")?;
    state.ink.id = Some(id);
    let kind = CreatureKind::Squid;
    let animal = Creature {
        id,
        home,
        kind,
        position: position.to_array().map(f64::from),
        velocity: [0.0; 3],
        yaw: 0.0,
        health: kind.health(),
        behavior: CreatureBehavior::Swim,
        timer: 3.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(kind),
    };
    world
        .resource_mut::<ReplicatedCreatures>()
        .creatures
        .insert(
            id,
            CreatureTrack {
                previous: animal.clone(),
                current: animal,
                received: Instant::now(),
                playback: Default::default(),
                interval: 0.1,
            },
        );
    Ok(())
}

fn set_behavior(
    world: &mut World,
    state: &ReefSmoke,
    behavior: CreatureBehavior,
) -> Result<(), String> {
    let id = state.ink.id.ok_or("No fixture squid exists")?;
    let mut animals = world.resource_mut::<ReplicatedCreatures>();
    let track = animals
        .creatures
        .get_mut(&id)
        .ok_or("Fixture squid disappeared")?;
    track.previous = track.current.clone();
    track.current.behavior = behavior;
    track.received = Instant::now();
    Ok(())
}

pub(super) fn ready(world: &mut World, state: &mut ReefSmoke) -> Result<bool, String> {
    state.ink.frames += 1;
    if state.stage == 12 {
        if state.since.elapsed() < Duration::from_millis(4400) {
            return Ok(false);
        }
        let exposure = world.resource::<terra_render::InkExposure>();
        if exposure.intensity > 0.001 || exposure.hits != state.ink.baseline {
            return Err(format!(
                "Squid ink did not decay naturally without repeated hits: {exposure:?}"
            ));
        }
        return Ok(true);
    }
    if !state.ink.armed
        && state.ink.frames >= 3
        && state.since.elapsed() >= Duration::from_millis(180)
    {
        set_behavior(world, state, CreatureBehavior::Ink)?;
        state.ink.armed = true;
        state.ink.frames = 0;
        return Ok(false);
    }
    if !state.ink.armed || state.ink.frames < 3 {
        return Ok(false);
    }
    let exposure = world.resource::<terra_render::InkExposure>();
    if exposure.hits <= state.ink.baseline {
        if state.since.elapsed() < Duration::from_secs(2) {
            return Ok(false);
        }
        return Err(format!(
            "Nearby squid ink release never hit the physical player head: {exposure:?}"
        ));
    }
    if exposure.age < 0.25 {
        return Ok(false);
    }
    if exposure.intensity <= 0.1 {
        return Err(format!(
            "Ink hit was not visible before its capture: {exposure:?}"
        ));
    }
    info!(
        stage = state.stage,
        hits = exposure.hits,
        intensity = exposure.intensity,
        age = exposure.age,
        low_quality = state.stage == 13,
        "Production squid behaviour edge hit the submerged player body"
    );
    Ok(true)
}

pub(super) fn finish(world: &mut World, state: &mut ReefSmoke, center: Vec3) -> Result<(), String> {
    let target = Vec3::new(center.x, state.level + 2.0, center.z);
    let view = Transform::from_translation(target).looking_at(target + Vec3::NEG_Z, Vec3::Y);
    set_camera(world, view, center);
    set_behavior(world, state, CreatureBehavior::Swim)?;
    state.ink.frames += 1;
    if state.ink.frames < 3 || state.since.elapsed() < Duration::from_millis(250) {
        return Ok(());
    }
    let exposure = world.resource::<terra_render::InkExposure>();
    if exposure.intensity != 0.0 || exposure.hits != 0 {
        return Err(format!(
            "Surfacing did not clear squid ink from the real player head: {exposure:?}"
        ));
    }
    state.ink.finished = true;
    Ok(())
}

pub(super) fn begin_finish(state: &mut ReefSmoke) {
    state.ink.frames = 0;
}
