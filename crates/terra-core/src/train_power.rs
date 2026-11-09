//! Physical boiler length, discrete gears and persistent steam resources.
use crate::trains::TrainPart;
use serde::{Deserialize, Serialize};
pub const FORWARD_SPEEDS: [f64; 6] = [4.0, 10.0, 16.0, 20.0, 30.0, 50.0];
pub const WATER_CAPACITY: f64 = 8_000.0;
pub const BUCKET_WATER: f64 = 1_000.0;
pub const STEAM_PRESSURE: f64 = 8.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SteamState {
    pub water: f64,
    pub pressure_bar: f64,
}
impl SteamState {
    pub fn valid(self) -> bool {
        self.water.is_finite()
            && (0.0..=WATER_CAPACITY).contains(&self.water)
            && self.pressure_bar.is_finite()
            && (0.0..=16.0).contains(&self.pressure_bar)
    }
    pub fn ready(self) -> bool {
        self.water > 0.0 && self.pressure_bar >= STEAM_PRESSURE
    }
    /// Heating requires actual fuel and water; pulling steam spends both resources.
    pub fn advance(&mut self, installed: bool, burning: bool, pulling: bool, dt: f64) {
        if !self.valid() || !dt.is_finite() || !(0.0..=0.25).contains(&dt) {
            return;
        }
        if !installed || self.water <= 0.0 {
            self.pressure_bar = (self.pressure_bar - 1.5 * dt).max(0.0);
            return;
        }
        let pressure = if burning { 0.75 } else { -0.15 } - if pulling { 0.45 } else { 0.0 };
        self.pressure_bar = (self.pressure_bar + pressure * dt).clamp(0.0, 16.0);
        if burning {
            self.water = (self.water - dt * if pulling { 4.0 } else { 0.5 }).max(0.0);
        }
    }
}

/// Count a continuous axial boiler, not unrelated decorative pieces nearby.
pub fn boiler_length(parts: &[TrainPart]) -> usize {
    let cells: std::collections::BTreeSet<_> = parts
        .iter()
        .filter(|p| crate::train_boiler::is_boiler(p.material))
        .map(|p| (p.offset, p.facing % 2))
        .collect();
    cells
        .iter()
        .map(|&(cell, axis)| {
            let mut next = cell;
            let mut length = 0;
            while cells.contains(&(next, axis)) {
                length += 1;
                next[if axis == 0 { 2 } else { 0 }] += 1;
            }
            length
        })
        .max()
        .unwrap_or(0)
}
pub fn steam_installed(parts: &[TrainPart]) -> bool {
    boiler_length(parts) >= 6
        && [218, 229, 254]
            .into_iter()
            .all(|id| parts.iter().any(|p| p.material == id))
        && parts.iter().any(|p| matches!(p.material, 230 | 231))
}
pub fn max_gear(parts: &[TrainPart], steam: SteamState) -> i8 {
    if steam_installed(parts) && steam.ready() {
        6
    } else if boiler_length(parts) >= 6 {
        5
    } else if boiler_length(parts) >= 4 {
        4
    } else {
        3
    }
}
pub fn gear_speed(gear: i8, max_gear: i8) -> f64 {
    let index = gear.unsigned_abs().min(max_gear.clamp(1, 6) as u8);
    if index == 0 {
        0.0
    } else {
        FORWARD_SPEEDS[usize::from(index - 1)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parts(length: i16) -> Vec<TrainPart> {
        (0..length)
            .map(|z| TrainPart {
                offset: [0, 1, z],
                material: 219,
                facing: 4,
                wood: Default::default(),
            })
            .collect()
    }
    #[test]
    fn higher_gears_require_contiguous_length_and_real_steam_resources() {
        for (length, expected) in [(3, 3), (4, 4), (5, 4), (6, 5), (10, 5)] {
            assert_eq!(max_gear(&parts(length), SteamState::default()), expected);
        }
        let mut train = parts(6);
        for (x, material) in [218, 229, 230, 254].into_iter().enumerate() {
            train.push(TrainPart {
                offset: [x as i16, 0, 0],
                material,
                facing: 0,
                wood: Default::default(),
            });
        }
        let mut state = SteamState {
            water: BUCKET_WATER,
            pressure_bar: 0.0,
        };
        assert_eq!(max_gear(&train, state), 5);
        for _ in 0..60 {
            state.advance(true, true, false, 0.25);
        }
        assert_eq!(max_gear(&train, state), 6);
        let before = state.water;
        state.advance(true, true, true, 0.25);
        assert!(state.water < before);
        state.water = 0.0;
        assert_eq!(max_gear(&train, state), 5);
        train.retain(|p| p.offset != [0, 1, 3]);
        assert_eq!(boiler_length(&train), 3);
    }
}
