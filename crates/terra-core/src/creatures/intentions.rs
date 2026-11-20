//! Bounded runtime inspection, deliberately separate from persistent creature data.
use super::{Creature, CreatureBehavior as B, NeedKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentReason {
    #[default]
    Idle,
    Explore,
    Food,
    Water,
    Threat,
    Hunt,
    Attack,
    DayRefuge,
    Corner,
    Surface,
    Silk,
    Stalk,
    Unsupported,
    Blocked,
    FollowOwner,
    WaitForOwner,
}
impl IntentReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Ruhe / keine feste Bewegung",
            Self::Explore => "Umgebung erkunden",
            Self::Food => "Futter erreichen",
            Self::Water => "Wasser erreichen",
            Self::Threat => "Abstand zur Bedrohung gewinnen",
            Self::Hunt => "Beute verfolgen",
            Self::Attack => "Ziel in Angriffsreichweite",
            Self::DayRefuge => "Tagsueber geschuetzten Unterschlupf suchen",
            Self::Corner => "In einer Ecke verstecken",
            Self::Surface => "Wand oder Decke erkunden",
            Self::Silk => "Am verankerten Faden abseilen",
            Self::Stalk => "Sichtbares Ziel anschleichen",
            Self::Unsupported => "Kein Bodenkontakt / faellt",
            Self::Blocked => "Weg blockiert / budgetierte Umleitung",
            Self::FollowOwner => "Besitzer folgen",
            Self::WaitForOwner => "Auf Besitzer warten",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatureIntent {
    pub id: u64,
    pub reason: IntentReason,
    pub destination: Option<[f64; 3]>,
    pub path: Vec<[f64; 3]>,
    pub blocked: bool,
}
impl CreatureIntent {
    pub fn valid(&self) -> bool {
        let point = |p: &[f64; 3]| p.iter().all(|v| v.is_finite() && v.abs() <= 999_000.0);
        self.id != 0
            && self.destination.as_ref().is_none_or(point)
            && self.path.len() <= 8
            && self.path.iter().all(point)
    }
    pub fn state(animal: &Creature) -> Self {
        let reason = if animal.behavior == B::Descend {
            IntentReason::Silk
        } else {
            match animal.needs.goal.map(|goal| goal.kind) {
                Some(NeedKind::Shelter) => IntentReason::DayRefuge,
                Some(NeedKind::Food) => IntentReason::Food,
                Some(NeedKind::Water) => IntentReason::Water,
                _ => match animal.behavior {
                    B::Hunt => IntentReason::Hunt,
                    B::Attack | B::Spit => IntentReason::Attack,
                    B::Flee => IntentReason::Threat,
                    B::Wander | B::Swim | B::Fly => IntentReason::Explore,
                    B::Crawl | B::Climb | B::Ceiling => IntentReason::Surface,
                    _ => IntentReason::Idle,
                },
            }
        };
        Self {
            id: animal.id,
            reason,
            destination: animal
                .needs
                .goal
                .map(|g| g.position.map(|v| v as f64 + 0.5)),
            path: Vec::new(),
            blocked: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inspection_payloads_are_bounded_and_reject_nonfinite_world_points() {
        let mut state = CreatureIntent {
            id: 1,
            reason: IntentReason::DayRefuge,
            destination: Some([-16.5, -21.0, 0.5]),
            path: vec![[-15.5, -21.0, 0.5]; 8],
            blocked: true,
        };
        assert!(state.valid());
        assert_eq!(
            serde_json::from_str::<CreatureIntent>(&serde_json::to_string(&state).unwrap())
                .unwrap(),
            state
        );
        state.path.push([0.0; 3]);
        assert!(!state.valid());
        state.path.pop();
        state.destination = Some([f64::NAN, 0.0, 0.0]);
        assert!(!state.valid());
        assert!(IntentReason::Stalk.label().contains("anschleichen"));
    }
}
