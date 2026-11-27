//! Ventral drag, surfacing growl, fire jet and body impact follow the replicated worm.
use bevy::prelude::Vec3;
use terra_core::creatures::{Creature, CreatureBehavior as B, lava_worm as pose};

pub(super) const FIRST: usize = super::worm::FIRST
    + super::worm::BANK_LEN
    + super::dragons::FAMILIES.len() * super::dragons::CUES.len();
pub(super) const COUNT: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub(super) enum Sound {
    Swim,
    Breach,
    Jet,
    Impact,
}

impl Sound {
    pub(super) fn clip(self) -> usize {
        FIRST + self as usize
    }
    pub(super) fn radius(self) -> f32 {
        match self {
            Self::Swim => 19.,
            Self::Breach => 42.,
            Self::Jet => 34.,
            Self::Impact => 48.,
        }
    }
    pub(super) fn level(self) -> f32 {
        match self {
            Self::Swim => 0.29,
            Self::Breach => 0.55,
            Self::Jet => 0.42,
            Self::Impact => 0.68,
        }
    }
}

#[derive(Default)]
pub(super) struct Cadence {
    last: Option<(B, f32, f64, Vec3)>,
    distance: f32,
    stroke: u32,
}

impl Cadence {
    pub(super) fn update(&mut self, a: &Creature, p: Vec3) -> Option<Sound> {
        let (before, timer, age, last_point) =
            self.last.replace((a.behavior, a.timer, a.age, p))?;
        let travel = (p - last_point).with_y(0.).length();
        if !(0.0..=1.4).contains(&(a.age - age)) || travel > 4. {
            self.distance = 0.;
            return None;
        }
        let start = before != a.behavior || a.timer > timer + 0.15;
        if matches!(a.behavior, B::Burrowed | B::Swim) {
            self.distance += travel;
            let spacing = 2.1 + ((a.id % 7 + u64::from(self.stroke) * 3) % 7) as f32 * 0.12;
            if self.distance >= spacing {
                self.distance -= spacing;
                self.stroke = self.stroke.wrapping_add(1);
                return Some(Sound::Swim);
            }
            return None;
        }
        self.distance = 0.;
        let (threshold, cue) = match a.behavior {
            B::Emerge => (pose::RISE * 0.68, Sound::Breach),
            B::Spit => (pose::SPIT - pose::RELEASE, Sound::Jet),
            B::Attack => (pose::SLAM - pose::IMPACT, Sound::Impact),
            _ => return None,
        };
        ((start || timer > threshold) && a.timer <= threshold).then_some(cue)
    }
}

/// Pressure oscillations and gravelly plate friction are baked once into the existing sample bank.
pub(super) fn append(bank: &mut Vec<Vec<f32>>) {
    debug_assert_eq!(bank.len(), FIRST);
    let cues: [Sound; COUNT] = [Sound::Swim, Sound::Breach, Sound::Jet, Sound::Impact];
    for sound in cues {
        let (duration, fundamental, descent, rasp, air, decay) = match sound {
            Sound::Swim => (0.93, 89., 47., 0.37, 0.14, 1.35),
            Sound::Breach => (1.65, 114., 52., 0.75, 0.18, 0.7),
            Sound::Jet => (0.91, 138., 65., 0.25, 0.83, 1.5),
            Sound::Impact => (1.22, 114., 82., 0.58, 0.28, 3.6),
        };
        let count = (duration * 24_000.) as usize;
        let mut rng = 0x731b_58afu32 + sound as u32;
        let (mut phase, mut low, mut gravel, mut bubble_phase) = (0.0_f32, 0., 0., 0.);
        let mut samples = Vec::with_capacity(count);
        for i in 0..count {
            let t = i as f32 / 24_000.;
            let u = i as f32 / (count - 1) as f32;
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let noise = rng as f32 / u32::MAX as f32 * 2. - 1.;
            low += (noise - low) * 0.021;
            gravel += (noise - gravel) * 0.19;
            phase += std::f32::consts::TAU / 24_000.
                * (fundamental - descent * u.sqrt() + 7. * (t * 13.).sin() + 12. * low);
            let pocket = (t * 5.6 + 0.14 * (t * 7.).sin()).fract();
            bubble_phase += std::f32::consts::TAU / 24_000. * (172. + 146. * pocket);
            let bubble = bubble_phase.sin() * (1. - pocket).powi(7);
            let rumble = phase.sin() * 0.30 + (phase * 1.51).sin() * 0.14;
            let scrape = low * 4. * (0.55 + 0.45 * (phase * 0.13).sin());
            let signal = rumble * (1. - air)
                + rasp * scrape
                + air * (gravel + 0.22 * noise)
                + bubble * if sound == Sound::Swim { 0.31 } else { 0.10 };
            let attack = if sound == Sound::Impact { 0.009 } else { 0.07 };
            let envelope = (t / attack).min(1.) * (1. - u).powf(decay) * ((1. - u) * 28.).min(1.);
            samples.push((signal * 1.45).tanh() * envelope * 0.78);
        }
        bank.push(samples);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::creatures::{CreatureKind, CreatureNeeds};
    #[test]
    fn molten_foley_has_four_distinct_enveloped_cues_with_headroom() {
        let mut clips = vec![Vec::new(); FIRST];
        append(&mut clips);
        let cues = &clips[FIRST..];
        assert_eq!(cues.len(), COUNT);
        for (i, cue) in cues.iter().enumerate() {
            assert!(cue.iter().all(|v| v.is_finite() && v.abs() < 0.79));
            let rms = (cue.iter().map(|s| s * s).sum::<f32>() / cue.len() as f32).sqrt();
            assert!((0.035..0.4).contains(&rms), "cue {i}: {rms}");
            assert_eq!(cue.first(), Some(&0.));
            assert_eq!(cue.last(), Some(&0.));
            assert!(cues[..i].iter().all(|other| other != cue));
        }
    }

    #[test]
    fn strokes_require_travel_and_jet_release_plays_once() {
        let kind = CreatureKind::LavaWorm;
        let mut a = Creature {
            id: 1,
            kind,
            home: [0; 2],
            position: [0.; 3],
            velocity: [0.; 3],
            yaw: 0.,
            health: kind.health(),
            behavior: B::Swim,
            timer: 4.,
            age: 30.,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        };
        let mut cadence = Cadence::default();
        assert_eq!(cadence.update(&a, Vec3::ZERO), None);
        assert_eq!(cadence.update(&a, Vec3::ZERO), None);
        assert_eq!(cadence.update(&a, Vec3::X * 2.8), Some(Sound::Swim));
        a.behavior = B::Spit;
        a.timer = pose::SPIT;
        assert_eq!(cadence.update(&a, Vec3::X * 2.8), None);
        a.timer -= pose::RELEASE + 0.02;
        assert_eq!(cadence.update(&a, Vec3::X * 2.8), Some(Sound::Jet));
        assert_eq!(cadence.update(&a, Vec3::X * 2.8), None);
    }
}
