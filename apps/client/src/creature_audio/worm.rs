//! Observed monster actions drive bounded positional cues, never a periodic roar.
#[path = "worm/smoke.rs"]
mod smoke;
mod synthesis;
use bevy::prelude::*;
use terra_core::creatures::{Creature, CreatureBehavior as B, sandworm as pose};

pub(super) const COUNT: usize = 8;
pub(super) const FIRST: usize = super::clips::PATHS.len() + 18 + super::ambience::DERIVED;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(super) enum Sound {
    Roar,
    Dig,
    Land,
    Bite,
    Spit,
    Hurt,
    Death,
    Breath,
    Rumble,
}
impl Sound {
    const ALL: [Self; COUNT] = [
        Self::Roar,
        Self::Dig,
        Self::Land,
        Self::Bite,
        Self::Spit,
        Self::Hurt,
        Self::Death,
        Self::Breath,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Roar => "roar",
            Self::Dig => "dig",
            Self::Land => "landing",
            Self::Bite => "bite",
            Self::Spit => "spit",
            Self::Hurt => "hurt",
            Self::Death => "death",
            Self::Breath => "breath",
            Self::Rumble => "rumble",
        }
    }
    pub(super) fn clip(self) -> usize {
        if self == Self::Rumble {
            super::ambience::WORM_RUMBLE
        } else {
            FIRST + self as usize
        }
    }
    pub(super) fn radius(self) -> f32 {
        match self {
            Self::Roar => 64.,
            Self::Death => 48.,
            Self::Land => 38.,
            Self::Bite => 26.,
            Self::Spit => 32.,
            Self::Hurt => 30.,
            Self::Breath => 14.,
            Self::Dig => 18.,
            Self::Rumble => 28.,
        }
    }
    fn level(self) -> f32 {
        match self {
            Self::Roar => 0.62,
            Self::Death => 0.53,
            Self::Land => 0.38,
            Self::Bite => 0.34,
            Self::Spit => 0.32,
            Self::Hurt => 0.30,
            Self::Breath => 0.10,
            Self::Dig => 0.16,
            Self::Rumble => 0.30,
        }
    }
    pub(super) fn ground_borne(self) -> bool {
        matches!(self, Self::Dig | Self::Land | Self::Rumble)
    }
    pub(super) fn position(self, animal: &Creature, root: Vec3) -> Vec3 {
        if matches!(self, Self::Dig | Self::Rumble) {
            return root + Vec3::Y * 0.12;
        }
        let mouth = mouth(animal, root);
        if self == Self::Land {
            mouth.with_y(root.y + 0.12)
        } else {
            mouth
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Emission {
    pub sound: Sound,
    pub level: f32,
}
impl From<Sound> for Emission {
    fn from(sound: Sound) -> Self {
        Self {
            sound,
            level: sound.level(),
        }
    }
}

pub(super) fn mouth(animal: &Creature, root: Vec3) -> Vec3 {
    let head = pose::mouth_position(animal);
    root + Vec3::from_array(std::array::from_fn(|axis| {
        (head[axis] - animal.position[axis]) as f32
    }))
}
fn visible(animal: &Creature) -> bool {
    pose::anatomy_visible(
        animal.behavior,
        pose::pose(animal, pose::HEAD_SEGMENT),
        pose::HEAD_SEGMENT,
    ) && pose::mouth_position(animal)[1] - animal.position[1] > 0.08
}
// Replicated timer -> normalized phase loses a few ULP at an exact action
// boundary (4.2 *0.8 round-trips to0.199999988). Use the same comparison for
// baselines, crossings and current actions so that boundary snapshots fire once.
fn bite_reached(progress: f32) -> bool {
    progress + 4. * f32::EPSILON >= pose::BITE_PHASE
}
#[derive(Default)]
pub(super) struct Cadence {
    initialized: bool,
    behavior: B,
    progress: f32,
    visible: bool,
    health: f32,
    age: f64,
    observed: f32,
    roared: bool,
    bitten: bool,
    landed: bool,
    next_dig: f32,
    next_breath: f32,
    next_hurt: f32,
}
impl Cadence {
    fn baseline(&mut self, animal: &Creature, now: f32, head: bool, progress: f32) {
        *self = Self {
            initialized: true,
            behavior: animal.behavior,
            progress,
            visible: head,
            health: animal.health,
            age: animal.age,
            observed: now,
            // An already visible head on joining is a baseline, not a fresh eruption.
            roared: head || matches!(animal.behavior, B::Attack | B::Spit | B::Retreat | B::Dead),
            bitten: animal.behavior == B::Attack && bite_reached(progress),
            landed: animal.behavior == B::Leap && !head && progress > 0.2,
            next_dig: now + 0.2,
            next_breath: now + 1.6,
            next_hurt: now,
        };
    }
    pub(super) fn update(&mut self, animal: &Creature, now: f32) -> Option<Emission> {
        if !now.is_finite() || !animal.health.is_finite() || !animal.age.is_finite() {
            return None;
        }
        let head = visible(animal);
        let progress = pose::phase_progress(animal.behavior, animal.timer);
        if !self.initialized
            || now < self.observed
            || now - self.observed > 0.8
            || animal.age + 0.01 < self.age
            || animal.age - self.age > 1.0
            || (self.health <= 0.0 && animal.health > 0.0)
        {
            self.baseline(animal, now, head, progress);
            return None;
        }
        let changed = self.behavior != animal.behavior;
        if animal.behavior == B::Burrowed || changed && animal.behavior == B::Tremor {
            self.roared = false;
        }
        if changed && animal.behavior == B::Leap {
            self.landed = false;
        }
        if changed && animal.behavior == B::Attack {
            self.bitten = false;
            self.next_breath = now + 1.6;
        }
        let dead =
            self.health > 0.0 && (animal.health <= 0.0 || changed && animal.behavior == B::Dead);
        let hurt = animal.health < self.health - 0.01
            && animal.health > 0.0
            && head
            && now >= self.next_hurt;
        let roar = head
            && !self.visible
            && !self.roared
            && animal.health > 0.0
            && matches!(animal.behavior, B::Leap | B::Emerge | B::Attack);
        if roar {
            self.roared = true;
        }
        if hurt {
            self.next_hurt = now + 0.48;
        }
        let bite = animal.behavior == B::Attack
            && !self.bitten
            && bite_reached(progress)
            && (self.behavior == B::Attack && !bite_reached(self.progress)
                || changed && progress < pose::BITE_PHASE + 0.10);
        if bite {
            self.bitten = true;
        }
        let spit = changed && animal.behavior == B::Spit && progress < 0.25;
        let land = !self.landed
            && self.visible
            && !head
            && (animal.behavior == B::Leap
                || self.behavior == B::Leap && animal.behavior == B::Emerge);
        if land {
            self.landed = true;
        }
        let retreat = animal.behavior == B::Retreat && self.visible && !head;
        let dig = now >= self.next_dig
            && (animal.behavior == B::Tremor
                || animal.behavior == B::Burrowed
                    && Vec3::from_array(animal.velocity).length_squared() > 0.0225);
        if dig {
            self.next_dig = now
                + if animal.behavior == B::Tremor {
                    0.72
                } else {
                    0.78
                };
        }
        let breath = animal.behavior == B::Attack && head && now >= self.next_breath;
        if breath {
            self.next_breath = now + 2.3;
        }
        self.behavior = animal.behavior;
        self.progress = progress;
        self.visible = head;
        self.health = animal.health;
        self.age = animal.age;
        self.observed = now;
        let sound = if dead {
            Sound::Death
        } else if roar {
            Sound::Roar
        } else if hurt {
            Sound::Hurt
        } else if spit {
            Sound::Spit
        } else if bite {
            Sound::Bite
        } else if land {
            Sound::Land
        } else if retreat {
            Sound::Dig
        } else if dig {
            if animal.behavior == B::Tremor {
                Sound::Rumble
            } else {
                Sound::Dig
            }
        } else if breath {
            Sound::Breath
        } else {
            return None;
        };
        Some(if sound == Sound::Rumble {
            Emission {
                sound,
                level: 0.26 + pose::warning(animal) * 0.12,
            }
        } else {
            sound.into()
        })
    }
}

pub(super) fn append(bank: &mut Vec<Vec<f32>>) {
    debug_assert_eq!(bank.len(), FIRST);
    let sounds = [
        synthesis::roar(false, false),
        synthesis::sand(&bank[16], false),
        synthesis::sand(&bank[17], true),
        synthesis::bite(&bank[18], &bank[54]),
        synthesis::spit(&bank[9]),
        synthesis::roar(false, true),
        synthesis::roar(true, false),
        synthesis::breath(),
    ];
    bank.extend(sounds);
}
pub(super) fn install(app: &mut App) {
    smoke::install(app);
}

pub(super) fn export(bank: &[Vec<f32>]) -> Result<(), String> {
    if !std::env::args().any(|arg| arg == "--smoke-test-steps")
        || std::env::var_os("TERRA_WORM_AUDIO_INSPECT").is_none()
    {
        return Ok(());
    }
    let Some(directory) = std::env::var_os("TERRA_WORM_AUDIO_EXPORT")
        .or_else(|| std::env::var_os("TERRA_CAPTURE_DIR"))
    else {
        return Ok(());
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    for sound in Sound::ALL {
        let clip = &bank[sound.clip()];
        let bytes = (clip.len() * 2) as u32;
        let mut wave = Vec::with_capacity(bytes as usize + 44);
        wave.extend_from_slice(b"RIFF");
        wave.extend_from_slice(&(bytes + 36).to_le_bytes());
        wave.extend_from_slice(b"WAVEfmt ");
        wave.extend_from_slice(&16_u32.to_le_bytes());
        wave.extend_from_slice(&1_u16.to_le_bytes());
        wave.extend_from_slice(&1_u16.to_le_bytes());
        wave.extend_from_slice(&24_000_u32.to_le_bytes());
        wave.extend_from_slice(&48_000_u32.to_le_bytes());
        wave.extend_from_slice(&2_u16.to_le_bytes());
        wave.extend_from_slice(&16_u16.to_le_bytes());
        wave.extend_from_slice(b"data");
        wave.extend_from_slice(&bytes.to_le_bytes());
        for sample in clip {
            wave.extend_from_slice(&((sample.clamp(-1., 1.) * 32767.) as i16).to_le_bytes());
        }
        std::fs::write(directory.join(format!("worm-{}.wav", sound.name())), wave)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
