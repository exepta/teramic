//! Immutable generated settlement footprints, resolved through live claims.
use super::{
    BotKingdom, KingdomHeraldry, KingdomRecord, KingdomRegistry, claim_chunk, valid_location,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DomainKind {
    Village,
    Kingdom,
    Outpost,
}
impl DomainKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Village => "Village",
            Self::Kingdom => "Kingdom",
            Self::Outpost => "Outpost",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementDomain {
    pub id: u64,
    pub center: [i64; 3],
    pub bounds: [[i64; 3]; 2],
    pub kind: DomainKind,
    pub native: Option<BotKingdom>,
}
impl SettlementDomain {
    pub fn valid(&self) -> bool {
        valid_location(self.center)
            && self.bounds.into_iter().all(valid_location)
            && (0..3)
                .all(|a| self.bounds[0][a] <= self.center[a] && self.center[a] <= self.bounds[1][a])
            && [0, 2]
                .into_iter()
                .all(|a| self.bounds[1][a].abs_diff(self.bounds[0][a]) <= 512)
            && self.bounds[1][1].abs_diff(self.bounds[0][1]) <= 512
    }
    pub fn contains(&self, position: [i64; 3]) -> bool {
        [0, 2]
            .into_iter()
            .all(|a| (self.bounds[0][a]..=self.bounds[1][a]).contains(&position[a]))
    }
    pub fn contains_chunk(&self, chunk: [i64; 2]) -> bool {
        let low = claim_chunk(self.bounds[0]);
        let high = claim_chunk(self.bounds[1]);
        (0..2).all(|a| (low[a]..=high[a]).contains(&chunk[a]))
    }
}
impl BotKingdom {
    pub const fn for_settlement(id: u64, required: bool) -> Option<Self> {
        if required {
            Some(Self::ALL[(id % 4) as usize])
        } else {
            Self::for_village(id)
        }
    }
}
impl KingdomRegistry {
    /// A direct claim wins locally. Capturing the central chunk transfers the
    /// remaining village affiliation; dissolving/releasing restores native colors.
    pub fn domain_owner(
        &self,
        domain: &SettlementDomain,
        position: [i64; 3],
    ) -> Option<&KingdomRecord> {
        self.region_at(position)
            .or_else(|| self.region_at(domain.center))
    }
    pub fn domain_heraldry(
        &self,
        domain: &SettlementDomain,
        position: [i64; 3],
    ) -> Option<KingdomHeraldry> {
        self.domain_owner(domain, position)
            .map(|r| r.heraldry)
            .or_else(|| domain.native.map(BotKingdom::heraldry))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PlayerUuid,
        kingdom::{ClaimActor, ClaimKind, KingdomIcon},
    };
    #[test]
    fn village_capture_changes_all_native_colors_and_local_foreign_claims_win() {
        let mut registry = KingdomRegistry::default();
        let native = SettlementDomain {
            id: 4,
            center: [0, 20, 0],
            bounds: [[-32, 10, -32], [63, 50, 63]],
            kind: DomainKind::Village,
            native: Some(BotKingdom::Petora),
        };
        assert!(native.valid());
        let point = [34, 25, 34];
        assert_eq!(
            registry.domain_heraldry(&native, point),
            Some(BotKingdom::Petora.heraldry())
        );
        let owner = PlayerUuid::from_u128(51);
        let id = registry.found(owner, [0, 20, 0], "Westhold").unwrap();
        assert_eq!(registry.domain_owner(&native, point).unwrap().id, id);
        let style = KingdomHeraldry {
            color: [12, 55, 205],
            icon: KingdomIcon::Anchor,
        };
        registry.change_appearance(owner, id, style).unwrap();
        assert_eq!(registry.domain_heraldry(&native, point), Some(style));
        let stranger = PlayerUuid::from_u128(52);
        let other = registry.found(stranger, [48, 20, 48], "Eastwatch").unwrap();
        registry
            .claim(
                other,
                [2, 3],
                ClaimKind::RealmTerritory,
                ClaimActor::Player(stranger),
            )
            .unwrap();
        assert_eq!(
            registry.domain_owner(&native, [34, 25, 50]).unwrap().id,
            other
        );
        registry.dissolve(owner, id).unwrap();
        assert_eq!(
            registry.domain_heraldry(&native, point),
            Some(BotKingdom::Petora.heraldry())
        );
    }
}
