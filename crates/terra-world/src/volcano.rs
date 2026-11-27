//! Seeded ballistic arcs shared by the authoritative collision pass and the renderer.
pub mod geyser;
use serde::{Deserialize, Serialize};

pub const EJECTA_COUNT: u8 = 64;
pub const DURATION: f32 = 26.;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vent {
    pub origin: [f64; 3],
    pub seed: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Eruption {
    pub vent: Vent,
    pub cycle: u32,
    pub age: f32,
    /// Terrain and creature impacts retire a stone for every observer.
    pub landed: u64,
}

impl Vent {
    pub fn eruption(self, now: f64) -> Option<Eruption> {
        let period = 96. + f64::from(self.seed % 83);
        let phase = now + f64::from(self.seed.rotate_left(13) % 157);
        let age = phase.rem_euclid(period) as f32;
        (age < DURATION).then_some(Eruption {
            vent: self,
            cycle: (phase / period).floor() as u32,
            age,
            landed: 0,
        })
    }
}

impl Eruption {
    /// Shaking and low-frequency sound share the eruption's attack and decay.
    pub fn tremor(&self, listener: [f64; 3]) -> f32 {
        let distance = self
            .vent
            .origin
            .iter()
            .zip(listener)
            .map(|(source, ear)| (source - ear).powi(2))
            .sum::<f64>()
            .sqrt() as f32;
        let range = (1. - distance / 100.).clamp(0., 1.);
        range * self.pressure()
    }

    pub fn pressure(&self) -> f32 {
        let attack = (self.age / 0.8).clamp(0., 1.);
        let decay = ((DURATION - self.age) / 6.).clamp(0., 1.);
        let pulse = 0.70 + 0.30 * (self.age * 1.7 + (self.age * 0.74).sin()).sin();
        attack * decay * pulse
    }

    pub fn fragment(self, index: u8) -> Fragment {
        let random = super::generation::seed::layer_seed(
            u64::from(self.vent.seed) ^ u64::from(self.cycle).wrapping_mul(0x517cc1b727220a95),
            u64::from(index),
        );
        let azimuth = f64::from(random & 1023) * std::f64::consts::TAU / 1024.;
        let duration = 4.3 + f64::from((random >> 10) & 31) * 0.045;
        let fountain = index % 4 == 1;
        let range = if fountain {
            5. + f64::from((random >> 15) & 31) * 0.72
        } else if index == 0 {
            100.
        } else {
            48. + f64::from((random >> 15) & 255) / 255. * 52.
        };
        // Long shots must clear the tallest basalt lip before crossing
        // the valley. Shorter bombs can still strike the crater walls.
        let height = if fountain {
            15. + f64::from((random >> 23) & 31)
        } else {
            27. + 0.42 * range + f64::from((random >> 23) & 31)
        };
        Fragment {
            vent: self.vent,
            delay: 0.6 + f32::from(index / 8) * 2.0 + f32::from(index % 8) * 0.115,
            duration: if fountain { duration * 0.72 } else { duration },
            range,
            height,
            direction: [azimuth.cos(), azimuth.sin()],
            molten: fountain || (index % 4 != 3 && random & (1 << 28) != 0),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Fragment {
    vent: Vent,
    pub delay: f32,
    duration: f64,
    range: f64,
    height: f64,
    direction: [f64; 2],
    pub molten: bool,
}
impl Fragment {
    pub fn position(self, age: f32) -> Option<[f64; 3]> {
        let t = f64::from(age - self.delay) / self.duration;
        if !(0.0..=1.4).contains(&t) {
            return None;
        }
        // Air drag rounds off the horizontal approach on the descending limb.
        let drag = ((t - 0.8) / 0.4).clamp(0., 1.);
        let distance = self.range * (t.min(0.8) + 0.4 * drag * (1. - 0.5 * drag));
        Some([
            self.vent.origin[0] + distance * self.direction[0],
            self.vent.origin[1] + 1.3 + 4. * self.height * t * (1. - t),
            self.vent.origin[2] + distance * self.direction[1],
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eruption_pressure_fades_at_one_hundred_blocks_and_only_during_the_blast() {
        let mut event = Eruption {
            vent: Vent {
                origin: [0., 90., 0.],
                seed: 17,
            },
            cycle: 2,
            age: 6.,
            landed: 0,
        };
        let close = event.tremor([0., 90., 0.]);
        assert!(close > 0.4 && close <= 1.);
        assert!((event.tremor([60., 90., 0.]) - close * 0.4).abs() < 0.0001);
        assert!(event.tremor([0., 90., 99.]) > 0.);
        for p in [[0., 90., 100.], [100., 100., 0.], [0., 200., 0.]] {
            assert_eq!(event.tremor(p), 0.);
        }
        for age in [-1., 0., DURATION, DURATION + 4.] {
            event.age = age;
            assert_eq!(event.tremor(event.vent.origin), 0.);
        }
    }

    #[test]
    fn eruption_has_a_quiet_interval_and_sweeps_both_kinds_of_ejecta() {
        let vent = Vent {
            origin: [18.5, 40., -50.5],
            seed: 207,
        };
        let event = (0..200)
            .find_map(|s| vent.eruption(f64::from(s)).filter(|e| e.age < 1.))
            .unwrap();
        assert!((0..200).any(|s| vent.eruption(f64::from(s)).is_none()));
        let mut max_range: f64 = 0.;
        let mut kinds = [false; 2];
        for i in 0..EJECTA_COUNT {
            let stone = event.fragment(i);
            kinds[usize::from(stone.molten)] = true;
            assert_eq!(stone.position(stone.delay - 0.1), None);
            for frame in 0..800 {
                if let Some(p) = stone.position(stone.delay + frame as f32 * 0.01) {
                    max_range = max_range.max((p[0] - vent.origin[0]).hypot(p[2] - vent.origin[2]));
                }
            }
        }
        assert_eq!(kinds, [true, true]);
        assert!((99.5..=100.001).contains(&max_range));
    }
}
