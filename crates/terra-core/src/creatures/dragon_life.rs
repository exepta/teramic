//! Growth in months of the world's own calendar. Only hatched dragons need a record.
use super::{Creature, breeding};
use crate::SeasonConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragonStage {
    Baby,
    Young,
    Adolescent,
    YoungAdult,
    Adult,
}

impl DragonStage {
    pub fn at(months: f64) -> Self {
        match months {
            m if m < 1. => Self::Baby,
            m if m < 3. => Self::Young,
            m if m < 6. => Self::Adolescent,
            m if m < 12. => Self::YoungAdult,
            _ => Self::Adult,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Baby => "Baby",
            Self::Young => "Jung",
            Self::Adolescent => "Jugendlich",
            Self::YoungAdult => "Junger Erwachsener",
            Self::Adult => "Ausgewachsen",
        }
    }
    pub fn can_fly(self) -> bool {
        matches!(self, Self::Adolescent | Self::YoungAdult | Self::Adult)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DragonLife {
    pub months: f64,
    pub nest: Option<[i64; 3]>,
    pub affection: f32,
    pub sheltered: bool,
    pub training: f32,
    pub last_contact_age: Option<f64>,
}

impl DragonLife {
    pub fn valid(self) -> bool {
        (0.0..=12.0).contains(&self.months)
            && self
                .nest
                .is_none_or(|p| p.into_iter().all(|axis| axis.unsigned_abs() <= 999_000))
            && (0.0..=100.0).contains(&self.affection)
            && (0.0..=100.0).contains(&self.training)
            && self
                .last_contact_age
                .is_none_or(|v| (0.0..=1e12).contains(&v))
    }
    /// Regions written before the calendar system used one continuous growth bar.
    pub fn legacy(animal: &Creature) -> Self {
        Self {
            months: 12. * (animal.age / breeding::GROWTH_SECONDS).clamp(0., 1.),
            ..Default::default()
        }
    }
    pub fn advance(&mut self, days: f64, seasons: SeasonConfig) {
        let year: u32 = seasons.lengths().iter().sum();
        if days.is_finite() && days > 0. && year > 0 {
            self.months = (self.months + days * 12. / f64::from(year)).min(12.);
        }
    }
    /// A hatchling is about the height of a player. Growth is continuous at each stage.
    pub fn scale(self) -> f64 {
        let mut start = (0., 0.15);
        for end in [(1., 0.28), (3., 0.46), (6., 0.68), (12., 1.)] {
            if self.months <= end.0 {
                let t = (self.months - start.0) / (end.0 - start.0);
                return start.1 + (end.1 - start.1) * t;
            }
            start = end;
        }
        1.
    }
    pub fn pet(&mut self, age: f64) -> bool {
        if self.last_contact_age.is_some_and(|last| age - last < 12.) {
            return false;
        }
        self.last_contact_age = Some(age);
        self.affection = (self.affection + 4.).min(100.);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stages_follow_the_world_year() {
        let seasons = SeasonConfig::default();
        let mut life = DragonLife::default();
        let mut months = 0.;
        for (end, stage) in [
            (1., DragonStage::Young),
            (3., DragonStage::Adolescent),
            (6., DragonStage::YoungAdult),
            (12., DragonStage::Adult),
        ] {
            let before = life.scale();
            life.advance((end - months) * 28. / 12., seasons);
            assert_eq!(DragonStage::at(life.months), stage);
            assert!(life.scale() > before);
            months = end;
        }
        assert!(!DragonStage::Young.can_fly());
        assert!(DragonStage::Adolescent.can_fly());
    }
}
