use super::{
    acoustics, clips,
    source::{Category, Controls, Cue, RecordedSoundscape},
};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::Ordering},
};
use terra_core::creatures::{CreatureBehavior as Behavior, CreatureKind as Kind};
use terra_world::{
    creatures::Habitat,
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess, WorldTerrain},
};

#[derive(Default)]
struct Timing {
    call: f32,
    step: f32,
    eat: f32,
    behavior: Behavior,
}
#[derive(Resource, Default)]
pub(super) struct Soundscape {
    pub(super) controls: Arc<Controls>,
    bank: Option<clips::Bank>,
    loading: Option<Task<Result<clips::Bank, String>>>,
    pub(super) entity: Option<Entity>,
    animals: BTreeMap<u64, Timing>,
    next_update: f32,
    next_environment: f32,
    next_fire: f32,
    pub(super) room: terra_world::enclosure::Enclosure,
}
pub(super) fn load(mut sounds: ResMut<Soundscape>) {
    sounds.loading = Some(AsyncComputeTaskPool::get().spawn(async { clips::load() }));
}
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct AudioWorld<'w, 's> {
    pub(super) time: Res<'w, Time>,
    clock: Option<Res<'w, terra_core::WorldClock>>,
    weather: Option<Res<'w, terra_core::WorldWeather>>,
    pub(super) access: Res<'w, WorldAccess>,
    animals: Option<Res<'w, terra_network::client::ReplicatedCreatures>>,
    menu: Option<Res<'w, terra_ui::menu::MenuState>>,
    config: Option<Res<'w, terra_core::AudioConfig>>,
    terrain: Res<'w, WorldTerrain>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    pub(super) bodies: Query<'w, 's, &'static Transform, With<terra_player::PlayerBody>>,
    pub(super) cameras: Query<'w, 's, &'static Transform, With<terra_player::TerraCamera>>,
    torches: Option<Res<'w, terra_render::TorchSources>>,
}
pub(super) fn update(
    world: AudioWorld,
    mut sounds: ResMut<Soundscape>,
    mut assets: ResMut<Assets<RecordedSoundscape>>,
    mut commands: Commands,
    mut weather_probe: Local<terra_world::weather::SurfaceWeatherProbe>,
) {
    if let Some(result) = sounds
        .loading
        .as_mut()
        .and_then(|task| future::block_on(future::poll_once(task)))
    {
        sounds.loading = None;
        match result {
            Ok(bank) => {
                tracing::info!(
                    clips = bank.len(),
                    "Recorded animal/environment audio decoded"
                );
                sounds.bank = Some(bank);
            }
            Err(error) => tracing::error!(%error, "Cannot load recorded animal/environment audio"),
        }
    }
    let active = world.access.ready() && world.menu.as_ref().is_none_or(|m| m.world_audio_active());
    let defaults = terra_core::AudioConfig::default();
    let config = world.config.as_deref().unwrap_or(&defaults);
    sounds.controls.configure(config, active);
    let body = world.bodies.iter().next();
    if !active || body.is_none() {
        if let Some(entity) = sounds.entity.take() {
            commands.entity(entity).despawn();
        }
        sounds.animals.clear();
        sounds.next_environment = 0.0;
        for gain in &sounds.controls.ambient {
            gain.store(0, Ordering::Relaxed);
        }
        for gain in &sounds.controls.fire {
            gain.store(0, Ordering::Relaxed);
        }
        for gain in &sounds.controls.rain {
            gain.store(0, Ordering::Relaxed);
        }
        if let Ok(mut queue) = sounds.controls.queue.lock() {
            queue.clear();
        }
        return;
    }
    let Some(body) = body else { return };
    if sounds.entity.is_none()
        && let Some(bank) = sounds.bank.clone()
    {
        let handle = assets.add(RecordedSoundscape {
            bank,
            controls: sounds.controls.clone(),
        });
        sounds.entity = Some(
            commands
                .spawn((
                    Name::new("Recorded animal/environment soundscape"),
                    AudioPlayer(handle),
                ))
                .id(),
        );
    }
    let now = world.time.elapsed_secs();
    if now < sounds.next_update {
        return;
    }
    sounds.next_update = now + 0.05;
    let listener = body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let right = world.cameras.iter().next().map_or(Vec3::X, |c| *c.right());
    let habitat = Habitat {
        terrain: &world.terrain.0,
        chunks: &world.chunks,
        journal: &world.journal,
    };
    let submerged = habitat.wet(listener.to_array().map(f64::from));
    sounds
        .controls
        .submerged
        .store(u32::from(submerged), Ordering::Relaxed);
    if now >= sounds.next_fire {
        sounds.next_fire = now + 0.1;
        let mut energy = [0.0_f32; 2];
        let mut torches: Vec<_> = world
            .torches
            .as_ref()
            .into_iter()
            .flat_map(|s| s.0.iter().copied())
            .filter(|p| p.distance_squared(listener) < 12.0_f32.powi(2))
            .collect();
        torches.sort_by(|a, b| {
            a.distance_squared(listener)
                .total_cmp(&b.distance_squared(listener))
        });
        for position in torches.into_iter().take(3) {
            let delta = position - listener;
            let pan = delta.normalize_or_zero().dot(right).clamp(-1.0, 1.0);
            let gain = 0.13
                * (1.0 - delta.length() / 12.0).powi(2)
                * crate::water_audio::scene::acoustic_transmission(listener, position, |p| {
                    habitat.block(p)
                });
            for (sum, value) in energy
                .iter_mut()
                .zip([gain * (0.7 - pan * 0.3), gain * (0.7 + pan * 0.3)])
            {
                *sum += value * value;
            }
        }
        for (target, energy) in sounds.controls.fire.iter().zip(energy) {
            target.store(energy.sqrt().min(0.2).to_bits(), Ordering::Relaxed);
        }
    }
    if now >= sounds.next_environment {
        sounds.next_environment = now + 0.5;
        let height = world
            .terrain
            .0
            .surface_column(f64::from(listener.x), f64::from(listener.z))
            .height;
        sounds.room =
            terra_world::enclosure::sample(listener.to_array().map(f64::from), height, |p| {
                habitat.block(p)
            });
        let biome = world
            .terrain
            .0
            .biome_at(f64::from(listener.x), f64::from(listener.z));
        let phase = world.clock.as_ref().map_or(0.5, |clock| clock.phase);
        let levels = environment(
            sounds.room.cave,
            phase as f32,
            biome.humidity as f32,
            (biome.weights.forest + biome.weights.plains + biome.weights.mushroom_park) as f32,
        );
        let local = world.weather.as_ref().map(|weather| {
            weather_probe.sample(
                weather,
                &world.terrain.0,
                &world.chunks,
                listener.to_array().map(f64::from),
                world.clock.as_deref().copied().unwrap_or_default(),
            )
        });
        let weather_intensity = local.map_or(0.0, |local| local.rain + local.snow);
        for (index, (target, gain)) in sounds.controls.ambient.iter().zip(levels).enumerate() {
            let gain = gain
                * if index < 2 {
                    1.0 - weather_intensity * 0.72
                } else {
                    1.0
                };
            target.store(gain.to_bits(), Ordering::Relaxed);
        }
        let position = listener.floor().as_i64vec3().to_array();
        let roof = (1..=64).any(|y| {
            habitat
                .block([position[0], position[1] + y, position[2]])
                .and_then(terra_voxel::MaterialId::from_raw)
                .is_some_and(terra_voxel::MaterialId::is_solid)
        });
        let impact = if local.is_some_and(|local| local.rain > 0.001) {
            acoustics::rain_surface_proximity(listener, |position| {
                let block = habitat.block(position)?;
                if terra_voxel::MaterialId::from_raw(block)
                    .is_some_and(terra_voxel::MaterialId::is_solid)
                {
                    return Some(true);
                }
                // Prepared data only: this acoustic probe must not evaluate world noise.
                world
                    .chunks
                    .prepared_column([position[0].div_euclid(16), position[2].div_euclid(16)])?;
                Some(
                    world
                        .chunks
                        .fluid_cell_at(&world.terrain.0, &world.journal, position, block)
                        .amount
                        > 0,
                )
            })
        } else {
            0.0
        };
        let gain = acoustics::rain_gain(
            local.map_or(0.0, |local| local.rain),
            roof,
            (height - f64::from(listener.y)).max(0.0) as f32,
            sounds.room.cave,
            impact,
        );
        sounds.controls.rain[0].store(gain.to_bits(), Ordering::Relaxed);
        sounds.controls.rain[1].store(
            (if roof { 1.0_f32 } else { 0.0 }).to_bits(),
            Ordering::Relaxed,
        );
    }
    let Some(animals) = world.animals else { return };
    sounds
        .animals
        .retain(|id, _| animals.creatures.contains_key(id));
    let mut nearby: Vec<_> = animals
        .creatures
        .values()
        .filter_map(|track| {
            let position = track.position() + Vec3::Y * (track.current.size()[1] as f32 * 0.55);
            let d = listener.distance(position);
            (d < if track.current.kind.is_aquatic() && track.current.kind != Kind::Fish {
                if submerged { 14.0 } else { 2.0 }
            } else if matches!(track.current.kind, Kind::Fish | Kind::Bee) {
                5.0
            } else {
                24.0
            })
            .then_some((d, position, track))
        })
        .collect();
    nearby.sort_by(|a, b| a.0.total_cmp(&b.0));
    let controls = sounds.controls.clone();
    let mut budget = 8;
    for (distance, position, track) in nearby {
        if budget == 0 {
            break;
        }
        let animal = &track.current;
        let timing = sounds.animals.entry(animal.id).or_insert_with(|| Timing {
            call: now + 2.0 + (animal.id % 110) as f32 * 0.1,
            step: now,
            eat: now,
            behavior: Behavior::Rest,
        });
        let speed = Vec3::from_array(animal.velocity).length();
        let cue = if animal.kind == Kind::Stalker {
            stalker_cue(animal, timing, now)
        } else if animal.kind.is_spider() {
            spider_cue(animal, timing, now, speed)
        } else if animal.kind.is_aquatic() {
            aquatic_cue(animal, timing, now, speed, submerged)
        } else if animal.behavior == Behavior::Sleep {
            // Sleeping suppresses calls, chewing and walking, including overdue calls.
            timing.call = now + 3.0;
            if now >= timing.eat {
                timing.eat = now + 4.0 + (animal.id % 13) as f32 * 0.1;
                Some((33, 0.075))
            } else {
                None
            }
        } else if animal.kind == Kind::Bee {
            if animal.behavior == Behavior::InNest && timing.behavior != Behavior::InNest {
                Some((28, 0.18))
            } else if timing.behavior == Behavior::InNest && animal.behavior != Behavior::InNest {
                Some((29, 0.18))
            } else if animal.behavior == Behavior::InNest && now >= timing.step {
                timing.step = now + 1.15;
                Some((26, 0.045))
            } else if matches!(
                animal.behavior,
                Behavior::Land | Behavior::Crawl | Behavior::Nectar
            ) && timing.behavior == Behavior::Fly
            {
                Some((27, 0.09))
            } else if matches!(animal.behavior, Behavior::Fly | Behavior::Flee)
                && now >= timing.step
            {
                timing.step = now + 1.15;
                Some((26, 0.17))
            } else {
                None
            }
        } else if animal.kind == Kind::Sheep
            && animal.behavior == Behavior::Shake
            && timing.behavior != Behavior::Shake
        {
            Some((8, 0.17))
        } else if matches!(animal.kind, Kind::Sheep | Kind::Cow | Kind::Pig) && now >= timing.call {
            timing.call = now + 9.0 + ((animal.id ^ now.to_bits() as u64) % 140) as f32 * 0.1;
            let base = match animal.kind {
                Kind::Sheep => 0,
                Kind::Cow => 2,
                _ => 4,
            };
            Some((base + ((animal.id + animal.age as u64) % 2) as usize, 0.5))
        } else if matches!(animal.behavior, Behavior::Graze | Behavior::Drink) && now >= timing.eat
        {
            timing.eat = now + 1.1;
            Some((
                if animal.behavior == Behavior::Drink {
                    10
                } else {
                    6
                },
                0.2,
            ))
        } else {
            None
        };
        timing.behavior = animal.behavior;
        if let Some((sound, level)) = cue {
            let transmission =
                crate::water_audio::scene::acoustic_transmission(listener, position, |p| {
                    habitat.block(p)
                });
            let direction = (position - listener).normalize_or_zero();
            let pan = direction.dot(right).clamp(-1.0, 1.0);
            let gain = level * (1.0 - distance / 24.0).powi(2) * transmission;
            controls.enqueue(Cue {
                category: Category::Animals,
                sound,
                gains: [gain * (0.7 - pan * 0.3), gain * (0.7 + pan * 0.3)],
                pitch: 0.96 + (animal.id % 9) as f32 * 0.01,
                reverb: if animal.kind.is_spider() || animal.kind == Kind::Stalker {
                    sounds.room.cave * 0.45
                } else {
                    0.0
                },
            });
            budget -= 1;
        }
    }
}

fn stalker_cue(
    animal: &terra_core::creatures::Creature,
    timing: &mut Timing,
    now: f32,
) -> Option<(usize, f32)> {
    if animal.behavior == Behavior::Hold && timing.behavior != Behavior::Hold {
        timing.call = now + 2.0;
        return Some((clips::STALKER_SCREAM_CLIP, 0.48));
    }
    if now >= timing.call {
        let fast = matches!(animal.behavior, Behavior::Attack | Behavior::Hold);
        timing.call = now
            + if fast {
                1.8
            } else {
                4.5 + (animal.id % 5) as f32 * 0.3
            };
        return Some((
            if fast {
                clips::STALKER_FAST_BREATH_CLIP
            } else {
                clips::STALKER_BREATH_CLIP
            },
            if fast { 0.22 } else { 0.13 },
        ));
    }
    if now >= timing.step {
        timing.step = now + 3.5 + (animal.id % 7) as f32 * 0.21;
        return Some((clips::STALKER_SCRATCH_CLIP, 0.09));
    }
    None
}

fn spider_cue(
    animal: &terra_core::creatures::Creature,
    timing: &mut Timing,
    now: f32,
    speed: f32,
) -> Option<(usize, f32)> {
    if animal.kind == Kind::LargeSpider
        && matches!(animal.behavior, Behavior::Attack | Behavior::Flee)
        && now >= timing.call
    {
        timing.call = now + 6.0 + (animal.id % 5) as f32;
        return Some((clips::SPIDER_HISS_CLIP, 0.40));
    }
    if speed > 0.03 && now >= timing.step {
        timing.step = now + (0.42 - speed.min(2.0) * 0.07);
        return Some((
            clips::SPIDER_STEP_CLIP,
            match animal.kind {
                Kind::SmallSpider => 0.025,
                Kind::MediumSpider => 0.12,
                _ => 0.24,
            },
        ));
    }
    if now >= timing.eat {
        timing.eat = now + 6.0 + (animal.id % 7) as f32;
        return Some((
            clips::SPIDER_IDLE_CLIP,
            if animal.kind == Kind::SmallSpider {
                0.014
            } else {
                0.07
            },
        ));
    }
    None
}

fn aquatic_cue(
    animal: &terra_core::creatures::Creature,
    timing: &mut Timing,
    now: f32,
    speed: f32,
    submerged: bool,
) -> Option<(usize, f32)> {
    if !submerged {
        return None;
    }
    if animal.kind == Kind::Squid
        && animal.behavior == Behavior::Ink
        && timing.behavior != Behavior::Ink
    {
        timing.step = now + 1.4;
        return Some((clips::INK_JET_CLIP, 0.15));
    }
    if now < timing.step {
        return None;
    }
    let variation = (animal.id % 11) as f32 * 0.06;
    if animal.kind.is_jelly() {
        timing.step =
            now + if animal.behavior == Behavior::Rest {
                4.8
            } else {
                2.8
            } + variation;
        Some((clips::JELLY_PULSE_CLIP, 0.035))
    } else if speed > 0.12 {
        timing.step = now + 1.6 + variation;
        Some(if animal.kind == Kind::Squid {
            (clips::SQUID_JET_CLIP, 0.085)
        } else if animal.kind.is_shark() {
            (clips::SHARK_STROKE_CLIP, 0.10)
        } else {
            (9, 0.065)
        })
    } else {
        None
    }
}

fn environment(cave: f32, phase: f32, humidity: f32, habitat: f32) -> [f32; 3] {
    let sun = ((phase - 0.25) * std::f32::consts::TAU).sin();
    let day = ((sun + 0.12) / 0.36).clamp(0.0, 1.0);
    let ecology = habitat.clamp(0.0, 1.0);
    [
        (1.0 - cave) * day * ecology * 0.08,
        (1.0 - cave) * (1.0 - day) * (0.02 + ecology * humidity * 0.12),
        cave * 0.14,
    ]
}

pub(super) fn water_contacts(
    world: AudioWorld,
    sounds: Res<Soundscape>,
    mut contacts: MessageReader<terra_core::footsteps::WaterMotion>,
    mut cadence: Local<WaterCadence>,
) {
    use terra_core::footsteps::WaterMotionKind;
    let Some(body) = world
        .bodies
        .iter()
        .next()
        .filter(|_| world.access.ready() && sounds.entity.is_some())
    else {
        contacts.clear();
        return;
    };
    let listener = body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let right = world.cameras.iter().next().map_or(Vec3::X, |c| *c.right());
    let now = world.time.elapsed_secs();
    for contact in contacts.read().take(16) {
        if contact.kind == WaterMotionKind::Bubble {
            continue;
        }
        let distance = listener.distance(contact.position);
        if distance > 24.0 {
            continue;
        }
        if contact.kind == WaterMotionKind::Wake
            && !cadence.stroke(contact.position, now, contact.strength)
        {
            continue;
        }
        let pan = (contact.position - listener)
            .normalize_or_zero()
            .dot(right)
            .clamp(-1.0, 1.0);
        let level = if contact.kind == WaterMotionKind::Splash {
            0.25 * contact.strength.sqrt()
        } else {
            0.09 + contact.strength.clamp(0.0, 1.0) * 0.08
        };
        let gain = level * (1.0 - distance / 24.0).powi(2);
        sounds.controls.enqueue(Cue {
            category: Category::Environment,
            sound: if contact.kind == WaterMotionKind::Splash {
                30
            } else {
                cadence.take()
            },
            gains: [gain * (0.7 - pan * 0.3), gain * (0.7 + pan * 0.3)],
            pitch: (1.02 - contact.strength * 0.06 + cadence.jitter()).clamp(0.92, 1.1),
            reverb: sounds.room.cave * 0.65,
        });
    }
}

pub(super) fn contacts(
    world: AudioWorld,
    sounds: Res<Soundscape>,
    mut steps: MessageReader<terra_core::footsteps::Footstep>,
    mut variation: Local<clips::ContactVariation>,
    mut weather_probe: Local<terra_world::weather::SurfaceWeatherProbe>,
) {
    let Some(body) = world.bodies.iter().next() else {
        steps.clear();
        return;
    };
    if !world.access.ready() || sounds.entity.is_none() {
        steps.clear();
        return;
    }
    let listener = body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let right = world.cameras.iter().next().map_or(Vec3::X, |c| *c.right());
    let habitat = Habitat {
        terrain: &world.terrain.0,
        chunks: &world.chunks,
        journal: &world.journal,
    };
    for step in steps.read().take(12) {
        let distance = listener.distance(step.position);
        if distance > 24.0 {
            continue;
        }
        let direction = (step.position - listener).normalize_or_zero();
        let pan = direction.dot(right).clamp(-1.0, 1.0);
        let transmission = crate::water_audio::scene::acoustic_transmission(
            listener,
            step.position + Vec3::Y * 0.15,
            |p| habitat.block(p),
        );
        let level = match step.walker {
            terra_core::footsteps::Walker::Player => 0.34,
            terra_core::footsteps::Walker::Stalker => 0.09,
            _ => 0.12,
        };
        let take = variation.next(step.material, step.alternate);
        let contact = (step.position - Vec3::Y * 0.08)
            .floor()
            .as_i64vec3()
            .to_array();
        let rain = world.weather.as_ref().map_or(0.0, |weather| {
            weather_probe
                .sample(
                    weather,
                    &world.terrain.0,
                    &world.chunks,
                    step.position.to_array().map(f64::from),
                    world.clock.as_deref().copied().unwrap_or_default(),
                )
                .rain
        });
        let wet = acoustics::wet_step_mix(
            rain,
            world.journal.surface_condition(contact).wetness,
            clips::Surface::material(step.material),
        );
        let gain = level * take.level * (1.0 - distance / 24.0).powi(2) * transmission;
        sounds.controls.enqueue(Cue {
            category: Category::Footsteps,
            sound: take.sound,
            gains: [gain * (0.7 - pan * 0.3), gain * (0.7 + pan * 0.3)]
                .map(|gain| gain * (1.0 - wet * 0.18)),
            pitch: take.pitch,
            reverb: sounds.room.cave * 0.65,
        });
        if wet > 0.01 {
            sounds.controls.enqueue(Cue {
                category: Category::Footsteps,
                sound: clips::WET_CONTACT_CLIPS[usize::from(step.alternate)],
                gains: [
                    gain * wet * (0.7 - pan * 0.3),
                    gain * wet * (0.7 + pan * 0.3),
                ],
                pitch: take.pitch,
                reverb: sounds.room.cave * 0.65,
            });
        }
        if clips::Surface::material(step.material) == clips::Surface::Grass {
            // The recorded grass impact and quiet leaf contact retain both body and rustle.
            let rustle = variation.next(terra_voxel::MaterialId::Leaves as u16, step.alternate);
            sounds.controls.enqueue(Cue {
                category: Category::Footsteps,
                sound: rustle.sound,
                gains: [
                    gain * 0.30 * (0.7 - pan * 0.3),
                    gain * 0.30 * (0.7 + pan * 0.3),
                ],
                pitch: rustle.pitch,
                reverb: sounds.room.cave * 0.65,
            });
        }
    }
    steps.clear();
}

#[derive(Default)]
pub(super) struct WaterCadence {
    nearby: Vec<(Vec3, f32)>,
    seed: u32,
    previous: usize,
}
impl WaterCadence {
    fn stroke(&mut self, position: Vec3, now: f32, strength: f32) -> bool {
        self.nearby.retain(|(_, until)| *until > now);
        if self
            .nearby
            .iter()
            .any(|(other, _)| position.distance_squared(*other) < 2.25)
            || self.nearby.len() >= 16
        {
            return false;
        }
        self.nearby
            .push((position, now + 0.78 - strength.clamp(0.0, 1.0) * 0.22));
        true
    }

    fn random(&mut self) -> u32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        self.seed
    }

    fn take(&mut self) -> usize {
        let mut index = (self.random() >> 16) as usize % (clips::SWIM_CLIPS.len() - 1);
        if index >= self.previous {
            index += 1;
        }
        self.previous = index;
        clips::SWIM_CLIPS[index]
    }

    fn jitter(&mut self) -> f32 {
        (self.random() >> 24) as f32 / 255.0 * 0.05 - 0.025
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aquatic_movement_is_underwater_only_and_ink_plays_once_per_release() {
        let mut animal = terra_core::creatures::Creature {
            kind: Kind::Squid,
            id: 7,
            behavior: Behavior::Ink,
            home: [0; 2],
            position: [0.0; 3],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: Kind::Squid.health(),
            timer: 1.0,
            age: 0.0,
            variant: 0,
            needs: terra_core::creatures::CreatureNeeds::full(Kind::Squid),
        };
        let mut timing = Timing::default();
        assert!(aquatic_cue(&animal, &mut timing, 0.0, 1.0, false).is_none());
        assert_eq!(
            aquatic_cue(&animal, &mut timing, 0.0, 1.0, true),
            Some((clips::INK_JET_CLIP, 0.15))
        );
        timing.behavior = Behavior::Ink;
        assert!(aquatic_cue(&animal, &mut timing, 0.5, 1.0, true).is_none());
        for kind in Kind::ALL.into_iter().filter(|kind| kind.is_aquatic()) {
            animal.kind = kind;
            animal.behavior = Behavior::Swim;
            let cue = aquatic_cue(&animal, &mut Timing::default(), 8.0, 1.0, true);
            assert!(cue.is_some(), "{kind:?}");
            assert!(cue.unwrap().1 <= 0.10);
        }
    }
    #[test]
    fn independent_swimmers_keep_their_own_strokes_and_do_not_repeat_takes() {
        let mut cadence = WaterCadence::default();
        assert!(cadence.stroke(Vec3::ZERO, 0.0, 0.5));
        assert!(!cadence.stroke(Vec3::X * 0.3, 0.2, 0.5));
        assert!(cadence.stroke(Vec3::X * 3.0, 0.2, 0.5));
        assert!(cadence.stroke(Vec3::ZERO, 1.0, 0.5));
        let mut previous = usize::MAX;
        let mut used = std::collections::HashSet::new();
        for _ in 0..64 {
            let take = cadence.take();
            assert_ne!(take, previous);
            assert!(clips::SWIM_CLIPS.contains(&take));
            used.insert(take);
            previous = take;
            assert!(cadence.jitter().abs() <= 0.0251);
        }
        assert_eq!(used.len(), 4);
    }
    #[test]
    fn environment_switches_day_birds_to_night_insects_and_suppresses_both_in_caves() {
        let day = environment(0.0, 0.5, 0.8, 1.0);
        let night = environment(0.0, 0.0, 0.8, 1.0);
        assert!(day[0] > 0.0 && day[1] == 0.0);
        assert!(night[0] == 0.0 && night[1] > 0.0);
        assert_eq!(environment(1.0, 0.5, 0.8, 1.0), [0.0, 0.0, 0.14]);
        assert!(environment(0.0, 0.5, 0.1, 0.1)[0] < day[0]);
    }
}
