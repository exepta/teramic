//! Sparse persistent state for actual flower and nest props, not a second prop registry.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ecology {
    pub seconds: f64,
    pub flowers: BTreeMap<u64, FlowerState>,
    pub nests: BTreeMap<u64, NestState>,
    pub bees: BTreeMap<u64, BeeState>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowerState {
    pub position: [f64; 3],
    pub nectar: u8,
    pub regenerate_at: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NestState {
    pub position: [f64; 3],
    pub nectar: u16,
    pub honey: u16,
    pub progress: f32,
}
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeeState {
    pub nest: u64,
    pub carried: u8,
    pub inside: bool,
    pub flower: Option<u64>,
}
impl Ecology {
    pub fn valid(&self) -> bool {
        let mut occupants: BTreeMap<u64, usize> = BTreeMap::new();
        for bee in self.bees.values().filter(|b| b.inside) {
            let count = occupants.entry(bee.nest).or_default();
            *count += 1;
            if *count > 10 {
                return false;
            }
        }
        let position = |p: [f64; 3]| p.iter().all(|v| v.is_finite() && v.abs() <= 999_000.0);
        self.seconds.is_finite()
            && self.seconds >= 0.0
            && self.flowers.len() <= 65_536
            && self.nests.len() <= 16_384
            && self.bees.len() <= 16_384
            && self.flowers.iter().all(|(id, f)| {
                *id != 0
                    && position(f.position)
                    && f.nectar <= 100
                    && f.regenerate_at.is_finite()
                    && f.regenerate_at >= 0.0
            })
            && self.nests.iter().all(|(id, n)| {
                *id != 0
                    && position(n.position)
                    && n.nectar <= 1000
                    && n.honey <= 100
                    && n.progress.is_finite()
                    && (0.0..=1.0).contains(&n.progress)
            })
            && self
                .bees
                .iter()
                .all(|(id, b)| *id != 0 && b.nest != 0 && b.carried <= 100)
    }
    pub fn nectar(&self, id: u64) -> u8 {
        self.flowers.get(&id).map_or(100, |f| f.nectar)
    }
    pub fn inside(&self, nest: u64) -> usize {
        self.bees
            .values()
            .filter(|b| b.nest == nest && b.inside)
            .count()
    }
    pub fn tick(&mut self, dt: f32) {
        if !dt.is_finite() || !(0.0..=0.25).contains(&dt) {
            return;
        }
        self.seconds += f64::from(dt);
        for f in self.flowers.values_mut() {
            if f.nectar == 0 && self.seconds >= f.regenerate_at {
                f.nectar = 100;
                f.regenerate_at = 0.0;
            }
        }
        let mut counts: BTreeMap<u64, usize> = BTreeMap::new();
        for bee in self.bees.values().filter(|b| b.inside) {
            *counts.entry(bee.nest).or_default() += 1;
        }
        for (&id, nest) in &mut self.nests {
            if nest.nectar >= 100 && nest.honey < 100 {
                let duration =
                    300.0 * (1.0 - 0.025 * counts.get(&id).copied().unwrap_or(0).min(10) as f32);
                nest.progress = (nest.progress + dt / duration).min(1.0);
                if nest.progress >= 1.0 {
                    nest.nectar -= 100;
                    nest.honey += 1;
                    nest.progress = 0.0;
                }
            }
        }
    }
    pub fn take_nectar(&mut self, id: u64, position: [f64; 3], amount: u8) -> u8 {
        if self.flowers.len() >= 65_536 && !self.flowers.contains_key(&id) {
            return 0;
        }
        let flower = self.flowers.entry(id).or_insert(FlowerState {
            position,
            nectar: 100,
            regenerate_at: 0.0,
        });
        let taken = amount.min(flower.nectar);
        flower.nectar -= taken;
        if taken > 0 && flower.nectar == 0 {
            flower.regenerate_at = self.seconds + 180.0;
        }
        taken
    }
    pub fn nearby(&self, position: [f64; 3]) -> Self {
        let distance = |p: [f64; 3]| (0..3).map(|i| (p[i] - position[i]).powi(2)).sum::<f64>();
        let near = |p: [f64; 3]| distance(p) < 80.0_f64.powi(2);
        let mut flowers: Vec<_> = self
            .flowers
            .iter()
            .filter(|(_, f)| near(f.position))
            .collect();
        flowers.sort_unstable_by(|(id, a), (other, b)| {
            distance(a.position)
                .total_cmp(&distance(b.position))
                .then(id.cmp(other))
        });
        Self {
            seconds: self.seconds,
            flowers: flowers
                .into_iter()
                .take(128)
                .map(|(&k, v)| (k, v.clone()))
                .collect(),
            nests: self
                .nests
                .iter()
                .filter(|(_, n)| near(n.position))
                .take(32)
                .map(|(&k, v)| (k, v.clone()))
                .collect(),
            bees: self
                .bees
                .iter()
                .filter(|(_, b)| self.nests.get(&b.nest).is_some_and(|n| near(n.position)))
                .take(128)
                .map(|(&k, v)| (k, v.clone()))
                .collect(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nectar_regenerates_after_three_minutes_and_honey_uses_occupied_nest_rate() {
        let mut eco = Ecology::default();
        assert_eq!(eco.take_nectar(1, [0.0; 3], 100), 100);
        for _ in 0..719 {
            eco.tick(0.25);
        }
        assert_eq!(eco.nectar(1), 0);
        eco.tick(0.25);
        assert_eq!(eco.nectar(1), 100);
        eco.nests.insert(
            2,
            NestState {
                position: [0.0; 3],
                nectar: 100,
                honey: 0,
                progress: 0.0,
            },
        );
        for id in 1..=4 {
            eco.bees.insert(
                id,
                BeeState {
                    nest: 2,
                    inside: true,
                    ..Default::default()
                },
            );
        }
        for _ in 0..1081 {
            eco.tick(0.25);
        }
        assert_eq!(eco.nests[&2].honey, 1);
        assert!(eco.valid());
    }
}
