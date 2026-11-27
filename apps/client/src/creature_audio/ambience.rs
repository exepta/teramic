//! Biome-weighted beds and sparse distant details, mixed at the player's body.
#[path = "ambience/design.rs"]
mod design;
use super::{
    clips,
    source::{Category, Controls, Cue},
};

pub const LAYERS: usize = 11;
pub const WIND: usize = 87;
pub const CROWS: usize = 88;
pub const OCEAN: usize = 89;
pub const WOODLAND_BIRDS: usize = 90;
pub const DERIVED: usize = 14;
const FIRST: usize = clips::PATHS.len() + 18;
pub const BLOOD: usize = FIRST;
pub const SPIRIT: usize = FIRST + 1;
pub const DESERT: usize = FIRST + 2;
pub const HIGHLAND: usize = FIRST + 3;
pub const FOREST: usize = FIRST + 4;
pub const CAVE_DETAIL: usize = FIRST + 5;
const BIRD_DETAILS: [usize; 2] = [FIRST + 6, FIRST + 7];
pub const CHERRY: usize = FIRST + 8;
pub const MUSHROOM: usize = FIRST + 9;
const SPIRIT_RUSTLE: usize = FIRST + 10;
const CHERRY_GLINT: usize = FIRST + 11;
const MUSHROOM_DETAIL: usize = FIRST + 12;
const BLOOD_DETAIL: usize = FIRST + 13;
pub const LOOP_CLIPS: [usize; LAYERS] = [
    11, 12, 13, BLOOD, SPIRIT, DESERT, OCEAN, HIGHLAND, FOREST, CHERRY, MUSHROOM,
];

#[derive(Clone, Copy, Debug)]
pub struct Profile {
    pub levels: [f32; LAYERS],
    pub cave: f32,
    pub ghost: f32,
    pub blood: f32,
    pub life: f32,
    pub cherry: f32,
    pub mushroom: f32,
}
pub fn profile(weights: [f64; 16], cave: f32, phase: f32, altitude: f32, rain: f32) -> Profile {
    let mut w = weights.map(|w| {
        if w.is_finite() {
            w.clamp(0.0, 1.0) as f32
        } else {
            0.0
        }
    });
    // The generator supplies normalized weights. Also keep corrupt or partial
    // inputs bounded so the square-root power blend cannot multiply the mix.
    let sum: f32 = w.iter().sum();
    if sum > 0.0 {
        w.iter_mut().for_each(|weight| *weight /= sum);
    }
    let finite = |value: f32| if value.is_finite() { value } else { 0.0 };
    let cave = finite(cave).clamp(0.0, 1.0);
    let altitude = finite(altitude);
    let phase = finite(phase);
    let outside = 1.0 - cave;
    let ground = (1.0 - (altitude.max(0.0) - 5.0) / 45.0).clamp(0.0, 1.0);
    let day = ((((phase - 0.25) * std::f32::consts::TAU).sin() + 0.12) / 0.36).clamp(0.0, 1.0);
    let life =
        (w[1] + w[2] + w[5] + w[7] + w[11] * 0.75 + w[12] * 0.85).min(1.0) * outside * ground;
    let weather = 1.0 - finite(rain).clamp(0.0, 1.0) * 0.72;
    // Real birds/insects remain the foundation even in magical forests. Dark
    // regions are quieter, rather than replaced with a sustained synthetic tone.
    let natural = (w[1]
        + w[2]
        + w[5] * 0.50
        + w[11] * 0.65
        + w[12] * 0.80
        + w[7] * 0.90
        + w[8] * 0.18
        + w[9] * 0.06)
        .min(1.0);
    let outdoor = |weight: f32| weight.min(1.0).sqrt() * outside;
    // A player's eye height is not exposure to mountain wind. Flying can add
    // a restrained highland layer, while haunted forests retain their hush.
    let exposed = ((altitude - 12.0) / 70.0).clamp(0.0, 1.0);
    let haunted = (w[8] + w[9]).min(1.0);
    Profile {
        levels: [
            outdoor(natural) * ground * day.sqrt() * weather * 0.075,
            outdoor(natural) * ground * (1.0 - day).sqrt() * weather * 0.085,
            (cave + outside * w[10] * 0.28) * 0.13,
            outdoor(w[9]) * ground * 0.16,
            outdoor(w[8]) * ground * 0.15,
            outdoor(w[3] + w[6] + w[11] * 0.35 + w[10] * 0.68 + w[13] * 0.7) * 0.065,
            outdoor(w[0] + w[15] * 0.82) * 0.15,
            outdoor(w[4] + w[14] + w[15] * 0.30 + exposed * 0.25 * (1.0 - haunted)) * 0.060,
            outdoor(w[1] + w[2] + w[5] + w[7] + w[11] * 0.75 + w[12] * 0.85)
                * ground
                * weather
                * 0.025,
            outdoor(w[7]) * ground * (0.72 + day * 0.28) * 0.13,
            outdoor(w[5]) * ground * (1.0 - day * 0.22) * 0.14,
        ],
        cave,
        ghost: w[8] * outside * ground,
        blood: w[9] * outside * ground,
        life: life * day * weather,
        cherry: w[7] * outside * ground,
        mushroom: w[5] * outside * ground,
    }
}

#[derive(Default)]
pub struct Cadence {
    next: f32,
    next_magic: f32,
    random: u32,
    previous: usize,
}
impl Cadence {
    fn random(&mut self) -> f32 {
        if self.random == 0 {
            self.random = 0xb947_53d1;
        }
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        (self.random & 0xffff) as f32 / 65535.0
    }
    pub fn reset(&mut self) {
        self.next = 0.0;
        self.next_magic = 0.0;
    }
    pub fn update(&mut self, now: f32, p: Profile, controls: &Controls) {
        self.fantasy(now, p, controls);
        if now < self.next {
            return;
        }
        // A pause after entering a world prevents every emitter starting at once.
        if self.next == 0.0 {
            self.next = now + 3.0 + self.random() * 5.0;
            return;
        }
        self.next = now
            + if p.ghost > p.blood && p.ghost > 0.20 {
                22.0 + self.random() * 32.0
            } else {
                9.0 + self.random() * 17.0
            };
        let (sound, gain, reverb) = if p.cave > 0.65 {
            (CAVE_DETAIL, 0.11 * p.cave, 0.80)
        } else if p.ghost > p.blood && p.ghost > 0.20 {
            (CROWS, 0.050 * p.ghost, 0.16)
        } else if p.blood > 0.20 {
            (CROWS, 0.065 * p.blood, 0.12)
        } else if p.life > 0.12 {
            let sound = if self.previous == BIRD_DETAILS[0] {
                BIRD_DETAILS[1]
            } else {
                BIRD_DETAILS[0]
            };
            (sound, 0.055 * p.life, 0.04)
        } else {
            return;
        };
        self.previous = sound;
        let pan = (self.random() * 2.0 - 1.0) * 0.75;
        let pitch = 0.98 + self.random() * 0.04;
        controls.enqueue(Cue {
            category: Category::Environment,
            sound,
            gains: [gain * (0.72 - pan * 0.28), gain * (0.72 + pan * 0.28)],
            pitch,
            reverb,
        });
    }

    fn fantasy(&mut self, now: f32, p: Profile, controls: &Controls) {
        if now < self.next_magic {
            return;
        }
        if self.next_magic == 0.0 {
            self.next_magic = now + 6.0 + self.random() * 8.0;
            return;
        }
        let ghost = p.ghost >= p.cherry.max(p.mushroom).max(p.blood) && p.ghost > 0.12;
        self.next_magic = now
            + if ghost {
                45.0 + self.random() * 45.0
            } else {
                55.0 + self.random() * 40.0
            };
        // Rare local details sit inside the recorded forest. A single quiet
        // crystalline glint gives Cherry its fantasy accent without a melody.
        let (sound, weight, reverb) = if p.cave > 0.35 {
            return;
        } else if ghost {
            (SPIRIT_RUSTLE, p.ghost, 0.12)
        } else if p.blood >= p.cherry.max(p.mushroom) && p.blood > 0.12 {
            (BLOOD_DETAIL, p.blood, 0.10)
        } else if p.cherry >= p.mushroom && p.cherry > 0.12 {
            (CHERRY_GLINT, p.cherry, 0.10)
        } else if p.mushroom > 0.12 {
            (MUSHROOM_DETAIL, p.mushroom, 0.05)
        } else {
            return;
        };
        let pan = (self.random() * 2.0 - 1.0) * 0.68;
        let gain = weight * (0.030 + self.random() * 0.020);
        // Preserve recognizable natural timbres. Width, distance and irregular
        // spacing provide the variation instead of slowed, voice-like effects.
        let pitch = 0.99 + self.random() * 0.02;
        controls.enqueue(Cue {
            category: Category::Environment,
            sound,
            gains: [gain * (0.72 - pan * 0.28), gain * (0.72 + pan * 0.28)],
            pitch,
            reverb,
        });
    }
}

fn filtered(
    source: &[f32],
    seconds: f32,
    rate: f32,
    cutoff: f32,
    gain: f32,
    offset: usize,
) -> Vec<f32> {
    if source.is_empty() {
        return vec![];
    }
    let n = (seconds * 24_000.0) as usize;
    let mut filter = design::LowPass::new(cutoff);
    let mut bass = 0.0;
    let dc_alpha = 1.0 - (-std::f32::consts::TAU * 22.0 / 24_000.0).exp();
    (0..n)
        .map(|i| {
            let pos = (i as f64 * rate as f64 + offset as f64) % source.len() as f64;
            let index = pos as usize;
            let a = source[index];
            let b = source[(index + 1) % source.len()];
            let sample = a + (b - a) * pos.fract() as f32;
            let low = filter.process(sample);
            bass += (low - bass) * dc_alpha;
            let fade = design::edge(i, n, 1800);
            (low - bass) * gain * fade
        })
        .collect()
}
pub fn append(bank: &mut Vec<Vec<f32>>, foliage: &[f32]) {
    // The outside world starts with recorded foliage at its natural speed.
    // Different seeds change the quiet gaps, not the pitch or source identity.
    let soft_foliage = filtered(
        foliage,
        foliage.len() as f32 / 24_000.0,
        1.0,
        3800.0,
        0.65,
        0,
    );
    let blood = design::rustles(foliage, &soft_foliage, 29.0, 0xb19a_3207);
    let spirit = design::rustles(&soft_foliage, foliage, 31.0, 0x812b_c903);
    let desert = filtered(&bank[WIND], 37.0, 0.87, 1200.0, 0.65, 31000);
    let highland = filtered(&bank[WIND], 33.0, 1.03, 3800.0, 0.58, 97000);
    let forest = design::rustles(foliage, &soft_foliage, 39.0, 0x7413_da29);
    let detail = filtered(&bank[13], 3.6, 0.78, 3900.0, 0.9, 477000);
    let bird_a = filtered(&bank[WOODLAND_BIRDS], 3.8, 1.0, 6700.0, 0.7, 96000);
    let bird_b = filtered(&bank[11], 3.2, 1.0, 6200.0, 0.7, 185000);
    let cherry = design::rustles(foliage, &soft_foliage, 35.0, 0x1a73_5f21);
    let mushroom = design::rustles(&soft_foliage, foliage, 41.0, 0x520b_7813);
    let spirit_rustle = filtered(
        foliage,
        foliage.len() as f32 / 24_000.0,
        1.0,
        4800.0,
        0.70,
        0,
    );
    let cherry_glint = filtered(
        &bank[66],
        bank[66].len() as f32 / 24_000.0,
        1.0,
        6200.0,
        0.12,
        0,
    );
    let mushroom_detail = filtered(
        foliage,
        foliage.len() as f32 / 24_000.0,
        1.0,
        4000.0,
        0.50,
        0,
    );
    let blood_detail = filtered(&bank[CROWS], 3.2, 1.0, 6200.0, 0.60, 24000);
    bank.extend([
        blood,
        spirit,
        desert,
        highland,
        forest,
        detail,
        bird_a,
        bird_b,
        cherry,
        mushroom,
        spirit_rustle,
        cherry_glint,
        mushroom_detail,
        blood_detail,
    ]);
    debug_assert_eq!(bank.len(), FIRST + DERIVED);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn regions_keep_natural_day_night_life_and_suppress_it_below_ground() {
        for biome in 0..12 {
            let mut weights = [0.0; 16];
            weights[biome] = 1.0;
            let day = profile(weights, 0.0, 0.5, 2.0, 0.0);
            let cave = profile(weights, 1.0, 0.5, -20.0, 0.0);
            assert!(day.levels.iter().any(|&v| v > 0.0), "biome {biome}");
            assert_eq!(cave.levels.iter().filter(|&&v| v > 0.0).count(), 1);
            assert_eq!(cave.levels[2], 0.13);
            if [0, 3, 4, 6].contains(&biome) {
                assert_eq!(day.levels[0], 0.0);
            }
        }
        let mut forest = [0.0; 16];
        forest[2] = 1.0;
        assert!(profile(forest, 0.0, 0.0, 2.0, 0.0).levels[1] > 0.0);
        assert_eq!(profile(forest, 0.0, 0.0, 2.0, 0.0).levels[0], 0.0);
        assert!(profile(forest, 0.0, 0.5, 60.0, 0.0).levels[7] > 0.0);
        assert_eq!(profile(forest, 0.0, 0.5, 60.0, 0.0).levels[0], 0.0);
        for biome in [8, 9] {
            let mut dark = [0.0; 16];
            dark[biome] = 1.0;
            let p = profile(dark, 0.0, 0.5, 2.0, 0.0);
            assert!(
                p.levels[0] > 0.0
                    && p.levels[0] < profile(forest, 0.0, 0.5, 2.0, 0.0).levels[0] * 0.5,
                "haunted forests retain quieter real wildlife"
            );
        }
    }
    #[test]
    fn details_have_irregular_bounded_spacing_panning_and_correct_volume_category() {
        let controls = Controls::default();
        let mut cadence = Cadence::default();
        let mut weights = [0.0; 16];
        weights[8] = 1.0;
        let p = profile(weights, 0.0, 0.5, 1.0, 0.0);
        let mut details = Vec::new();
        let mut crow_times = Vec::new();
        let mut presence_times = Vec::new();
        for step in 0..800 {
            cadence.update(step as f32 * 0.5, p, &controls);
            let mut queue = controls.queue.lock().unwrap();
            if queue.iter().any(|cue| cue.sound == CROWS) {
                crow_times.push(step as f32 * 0.5);
            }
            if queue.iter().any(|cue| cue.sound == SPIRIT_RUSTLE) {
                presence_times.push(step as f32 * 0.5);
            }
            details.extend(queue.drain(..));
        }
        assert!((4..=18).contains(&crow_times.len()));
        assert!(
            crow_times
                .windows(2)
                .all(|times| (22.0..=54.5).contains(&(times[1] - times[0])))
        );
        assert!((3..=10).contains(&presence_times.len()));
        let pauses: std::collections::BTreeSet<_> = presence_times
            .windows(2)
            .map(|times| ((times[1] - times[0]) * 2.0) as u32)
            .collect();
        assert!(
            pauses.len() > 1,
            "a local detail must not repeat on a fixed beat"
        );
        assert!(
            pauses
                .iter()
                .all(|&half_seconds| (90..=181).contains(&half_seconds))
        );
        assert!(details.iter().all(|c| c.category == Category::Environment
            && [CROWS, SPIRIT_RUSTLE].contains(&c.sound)
            && c.gains.iter().all(|g| *g > 0.0 && *g < 0.11)));
        assert!(details.iter().any(|c| c.sound == SPIRIT_RUSTLE));
        assert!(details.iter().any(|c| c.gains[0] > c.gains[1]));
        assert!(details.iter().any(|c| c.gains[1] > c.gains[0]));
    }

    #[test]
    fn outdoor_beds_do_not_generate_tones_or_reuse_the_dungeon_recording() {
        let mut sources = vec![vec![0.0; 24_000]; FIRST];
        // A loud cave sample must never make a Spirit/Cherry/Mushroom field
        // when its real foliage inputs are silent.
        sources[13] = (0..24_000)
            .map(|i| (i as f32 * 0.027).sin() * 0.5)
            .collect();
        append(&mut sources, &[0.0; 24_000]);
        for clip in [BLOOD, SPIRIT, FOREST, CHERRY, MUSHROOM] {
            assert!(
                sources[clip].iter().all(|&sample| sample == 0.0),
                "outdoor layer {clip} created synthetic/cave sound"
            );
        }
        assert!(
            sources[CAVE_DETAIL]
                .iter()
                .any(|sample| sample.abs() > 0.01)
        );
    }

    #[test]
    fn walking_does_not_add_highland_wind_and_haunted_forests_remain_hushed_at_height() {
        for biome in 0..12 {
            let mut weights = [0.0; 16];
            weights[biome] = 1.0;
            let walking = profile(weights, 0.0, 0.5, 1.7, 0.0);
            if biome != 4 {
                assert_eq!(walking.levels[7], 0.0, "eye-height wind in biome {biome}");
            } else {
                assert!(walking.levels[7] < 0.07);
            }
            if [8, 9].contains(&biome) {
                let elevated = profile(weights, 0.0, 0.5, 100.0, 0.0);
                assert_eq!(elevated.levels[7], 0.0, "haunted forest wind bleed");
            }
        }
        let mut woods = [0.0; 16];
        woods[2] = 1.0;
        let p = profile(woods, 0.0, 0.5, 1.7, 0.0);
        assert!(
            p.levels[0] > p.levels[8] * 2.0,
            "foliage movements must sit below wildlife"
        );
    }

    #[test]
    fn actual_wind_recordings_have_substantially_less_mix_energy_than_the_previous_design() {
        let bank = clips::load().unwrap();
        let rms = |clip: &[f32]| {
            (clip.iter().map(|sample| sample * sample).sum::<f32>() / clip.len() as f32).sqrt()
        };
        for (biome, layer, clip, rate, cutoff, old_source_gain, offset, old_mix_gain) in [
            (3, 5, DESERT, 0.87, 1600.0, 0.86, 31000, 0.15),
            (4, 7, HIGHLAND, 1.03, 6500.0, 1.0, 97000, 0.16),
        ] {
            let mut weights = [0.0; 16];
            weights[biome] = 1.0;
            let current = rms(&bank[clip]) * profile(weights, 0.0, 0.5, 1.7, 0.0).levels[layer];
            let previous = rms(&filtered(
                &bank[WIND],
                bank[clip].len() as f32 / 24_000.0,
                rate,
                cutoff,
                old_source_gain,
                offset,
            )) * old_mix_gain;
            assert!(
                current < previous * 0.40,
                "wind still dominates biome {biome}: {current}/{previous}"
            );
        }
    }

    #[test]
    fn unrelated_region_beds_preserve_power_at_the_boundary() {
        let mut blood = [0.0; 16];
        blood[9] = 1.0;
        let mut spirit = [0.0; 16];
        spirit[8] = 1.0;
        let left = profile(blood, 0.0, 0.5, 0.0, 0.0);
        let right = profile(spirit, 0.0, 0.5, 0.0, 0.0);
        for step in 0..=100 {
            let t = step as f64 / 100.0;
            let mut weights = [0.0; 16];
            weights[9] = 1.0 - t;
            weights[8] = t;
            let mixed = profile(weights, 0.0, 0.5, 0.0, 0.0);
            let normalized_power = (mixed.levels[3] / left.levels[3]).powi(2)
                + (mixed.levels[4] / right.levels[4]).powi(2);
            assert!((normalized_power - 1.0).abs() < 0.00001);
        }
    }

    #[test]
    fn invalid_environment_inputs_and_duplicate_weights_remain_finite_and_bounded() {
        for weights in [[f64::NAN; 16], [f64::INFINITY; 16], [-1.0; 16], [1.0; 16]] {
            let p = profile(weights, f32::NAN, f32::INFINITY, f32::NAN, f32::INFINITY);
            assert!(
                p.levels
                    .iter()
                    .all(|level| level.is_finite() && (0.0..=0.16).contains(level))
            );
        }
    }

    #[test]
    fn filtered_resampling_rejects_dc_and_fades_both_edges() {
        let constant = filtered(&[0.3; 2400], 1.0, 0.87, 1600.0, 1.0, 931);
        assert_eq!(constant[0], 0.0);
        assert_eq!(*constant.last().unwrap(), 0.0);
        assert!(
            constant[12000..]
                .iter()
                .all(|sample| sample.abs() < 0.00001)
        );
        let silent = filtered(&[0.0; 2400], 1.0, 1.03, 6500.0, 1.0, 131);
        assert!(silent.iter().all(|sample| *sample == 0.0));
    }
}
