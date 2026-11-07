//! Military residents use the existing persistent humanoid species and schedules.
//! Appended role values keep every existing profession and save tag intact.
use super::{Creature, CreatureKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoldierLoadout {
    Sword,
    Spear,
    Bow,
}
impl SoldierLoadout {
    pub const ALL: [Self; 3] = [Self::Sword, Self::Spear, Self::Bow];
    pub const fn role(self) -> u8 {
        match self {
            Self::Sword => 10,
            Self::Spear => 11,
            Self::Bow => 12,
        }
    }
    pub const fn from_role(role: u8) -> Option<Self> {
        match role {
            10 => Some(Self::Sword),
            11 => Some(Self::Spear),
            12 => Some(Self::Bow),
            _ => None,
        }
    }
    pub fn of(creature: &Creature) -> Option<Self> {
        (creature.kind == CreatureKind::Villager)
            .then(|| Self::from_role(creature.variant))
            .flatten()
    }
    /// Stagger guard shifts: half stay awake at night, the other half at noon.
    /// Feeding and access to a real assigned bed use the resident schedule.
    pub fn sleeping(self, id: u64, phase: f64) -> bool {
        if id.is_multiple_of(2) {
            super::villagers::sleeping(id, phase)
        } else {
            (0.375..0.625).contains(&phase)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn military_roles_append_without_reclassifying_old_professions_and_guard_shifts_overlap() {
        for role in 0..super::super::villagers::CIVILIAN_ROLE_COUNT {
            assert!(SoldierLoadout::from_role(role).is_none());
        }
        for loadout in SoldierLoadout::ALL {
            assert_eq!(SoldierLoadout::from_role(loadout.role()), Some(loadout));
            for phase in [0.0, 0.1, 0.25, 0.4, 0.5, 0.7, 0.9] {
                assert!(
                    !(loadout.sleeping(0, phase) && loadout.sleeping(1, phase)),
                    "At least one guard shift stays on duty"
                );
            }
        }
    }
}
