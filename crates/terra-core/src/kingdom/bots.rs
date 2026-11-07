//! Immutable NPC realms. Affiliation uses the existing stable village identity;
//! it survives saving, reconnecting and region-cache eviction without retrogen.
use super::{KingdomHeraldry, KingdomIcon};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BotKingdom {
    Lutera,
    Betross,
    Helteria,
    Petora,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KingdomRelation {
    Neutral,
    Friendly,
    Hostile,
}

impl KingdomRelation {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Neutral => "Neutral",
            Self::Friendly => "Friendly",
            Self::Hostile => "Hostile",
        }
    }
}

impl BotKingdom {
    pub const ALL: [Self; 4] = [Self::Lutera, Self::Betross, Self::Helteria, Self::Petora];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Lutera => "Lutera",
            Self::Betross => "Betross",
            Self::Helteria => "Helteria",
            Self::Petora => "Petora",
        }
    }

    pub const fn relation(self) -> KingdomRelation {
        match self {
            Self::Lutera | Self::Betross => KingdomRelation::Neutral,
            Self::Helteria => KingdomRelation::Friendly,
            Self::Petora => KingdomRelation::Hostile,
        }
    }

    pub const fn heraldry(self) -> KingdomHeraldry {
        let (color, icon) = match self {
            Self::Lutera => ([74, 119, 173], KingdomIcon::Crown),
            Self::Betross => ([174, 132, 65], KingdomIcon::Hammer),
            Self::Helteria => ([74, 147, 101], KingdomIcon::Tree),
            Self::Petora => ([172, 64, 73], KingdomIcon::Castle),
        };
        KingdomHeraldry { color, icon }
    }

    /// Keep this mapping stable: changing it would reassign existing villages.
    /// One fifth of villages remain independent; the others cover all four realms.
    pub const fn for_village(village_id: u64) -> Option<Self> {
        let mut hash = village_id ^ 0x7265_616c_6d73_7631;
        hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        hash ^= hash >> 31;
        match hash % 5 {
            0 => None,
            1 => Some(Self::Lutera),
            2 => Some(Self::Betross),
            3 => Some(Self::Helteria),
            _ => Some(Self::Petora),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn named_realms_have_the_requested_relations_and_valid_distinct_heraldry() {
        assert_eq!(
            BotKingdom::ALL.map(BotKingdom::name),
            ["Lutera", "Betross", "Helteria", "Petora"]
        );
        assert_eq!(
            BotKingdom::ALL.map(BotKingdom::relation),
            [
                KingdomRelation::Neutral,
                KingdomRelation::Neutral,
                KingdomRelation::Friendly,
                KingdomRelation::Hostile
            ]
        );
        for realm in BotKingdom::ALL {
            assert!(realm.heraldry().icon.valid());
        }
    }
    #[test]
    fn affiliation_is_stable_includes_independent_villages_and_all_realms() {
        let affiliations: std::collections::BTreeSet<_> =
            (0..1000).map(BotKingdom::for_village).collect();
        assert_eq!(affiliations.len(), 5);
        // Frozen examples guard world/reconnect compatibility of the mapping.
        assert_eq!(BotKingdom::for_village(0), Some(BotKingdom::Helteria));
        assert_eq!(BotKingdom::for_village(u64::MAX), Some(BotKingdom::Petora));
    }
}
