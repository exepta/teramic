//! Live, validated territorial policy. Existing saved claims remain historical data.
use super::{ClaimLimits, MAX_CLAIMS, MAX_SETTLEMENT_LEVEL, SettlementKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KingdomRules {
    pub require_adjacent_claims: bool,
    pub kingdom_build_base: u32,
    pub kingdom_build_per_level: u32,
    pub kingdom_territory_base: u32,
    pub kingdom_territory_per_level: u32,
    pub outposts_per_level: u16,
    pub outpost_build_base: u32,
    pub outpost_build_per_level: u32,
    pub outpost_territory_base: u32,
    pub outpost_territory_per_level: u32,
    pub realm_min_border_gap: u16,
    pub village_requires_territory: bool,
    pub village_build_base: u32,
    pub village_build_per_level: u32,
    pub city_min_kingdom_level: u16,
    pub city_requires_territory: bool,
    pub city_build_base: u32,
    pub city_build_per_level: u32,
    pub city_min_build_gap: u16,
    pub settlements_follow_kingdom_level: bool,
}
impl Default for KingdomRules {
    fn default() -> Self {
        Self {
            require_adjacent_claims: true,
            kingdom_build_base: 32,
            kingdom_build_per_level: 6,
            kingdom_territory_base: 64,
            kingdom_territory_per_level: 16,
            outposts_per_level: 1,
            outpost_build_base: 10,
            outpost_build_per_level: 2,
            outpost_territory_base: 22,
            outpost_territory_per_level: 6,
            realm_min_border_gap: 16,
            village_requires_territory: true,
            village_build_base: 10,
            village_build_per_level: 2,
            city_min_kingdom_level: 3,
            city_requires_territory: true,
            city_build_base: 20,
            city_build_per_level: 4,
            city_min_build_gap: 10,
            settlements_follow_kingdom_level: false,
        }
    }
}
impl KingdomRules {
    pub fn valid(self) -> bool {
        let bounded = |base: u32, growth: u32| {
            base > 0
                && u64::from(base) + u64::from(growth) * u64::from(MAX_SETTLEMENT_LEVEL - 1)
                    <= MAX_CLAIMS as u64
        };
        bounded(self.kingdom_build_base, self.kingdom_build_per_level)
            && bounded(
                self.kingdom_territory_base,
                self.kingdom_territory_per_level,
            )
            && bounded(self.outpost_build_base, self.outpost_build_per_level)
            && bounded(
                self.outpost_territory_base,
                self.outpost_territory_per_level,
            )
            && bounded(self.village_build_base, self.village_build_per_level)
            && bounded(self.city_build_base, self.city_build_per_level)
            && self.outposts_per_level <= 256
            && self.realm_min_border_gap <= 4096
            && self.city_min_build_gap <= 4096
            && (1..=MAX_SETTLEMENT_LEVEL).contains(&self.city_min_kingdom_level)
    }
    pub fn limits(self, kind: SettlementKind, level: u16) -> ClaimLimits {
        let growth = u32::from(level.clamp(1, MAX_SETTLEMENT_LEVEL) - 1);
        let (build, build_step, territory, territory_step) = match kind {
            SettlementKind::Kingdom => (
                self.kingdom_build_base,
                self.kingdom_build_per_level,
                self.kingdom_territory_base,
                self.kingdom_territory_per_level,
            ),
            SettlementKind::Outpost => (
                self.outpost_build_base,
                self.outpost_build_per_level,
                self.outpost_territory_base,
                self.outpost_territory_per_level,
            ),
            SettlementKind::Village => {
                (self.village_build_base, self.village_build_per_level, 0, 0)
            }
            SettlementKind::City => (self.city_build_base, self.city_build_per_level, 0, 0),
        };
        ClaimLimits {
            protected: build.saturating_add(build_step.saturating_mul(growth)),
            territory: territory.saturating_add(territory_step.saturating_mul(growth)),
        }
    }
}
