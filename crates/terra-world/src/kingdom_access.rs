//! Shared live access checks for players, jobs and item/fluid transport.
use crate::{edits::EditJournal, streaming::ChunkStore};
use terra_core::{
    PlayerUuid,
    kingdom::{DiplomaticTarget, KingdomPermission, KingdomRegistry, claim_chunk},
};

pub fn can_use(
    registry: &KingdomRegistry,
    chunks: &ChunkStore,
    actor: PlayerUuid,
    permission: KingdomPermission,
    position: [i64; 3],
) -> bool {
    if registry.claim_at(claim_chunk(position)).is_some() {
        return registry.can(actor, permission, position);
    }
    chunks
        .village_settlements()
        .find(|site| site.domain().contains(position))
        .is_none_or(|site| registry.can_in_domain(actor, permission, position, &site.domain()))
}
fn owner(
    registry: &KingdomRegistry,
    chunks: &ChunkStore,
    position: [i64; 3],
) -> Option<DiplomaticTarget> {
    registry
        .region_at(position)
        .map(|r| DiplomaticTarget::Player(r.id))
        .or_else(|| {
            let site = chunks
                .village_settlements()
                .find(|site| site.domain().contains(position))?;
            let domain = site.domain();
            registry
                .domain_owner(&domain, position)
                .map(|r| DiplomaticTarget::Player(r.id))
                .or_else(|| domain.native.map(DiplomaticTarget::Bot))
        })
}
pub fn can_transport(
    chunks: &ChunkStore,
    journal: &EditJournal,
    from: [i64; 3],
    to: [i64; 3],
) -> bool {
    if claim_chunk(from) == claim_chunk(to) {
        // A single chunk can still cross the edge of a native generated domain.
        if journal
            .gameplay()
            .kingdoms
            .claim_at(claim_chunk(from))
            .is_some()
        {
            return true;
        }
    }
    let registry = &journal.gameplay().kingdoms;
    registry.can_transport_between(owner(registry, chunks, from), owner(registry, chunks, to))
}
pub fn can_move_drop(
    chunks: &ChunkStore,
    journal: &EditJournal,
    from: [f64; 3],
    to: [f64; 3],
) -> bool {
    if from[0].floor() == to[0].floor() && from[2].floor() == to[2].floor() {
        return true;
    }
    can_transport(
        chunks,
        journal,
        from.map(|v| v.floor() as i64),
        to.map(|v| v.floor() as i64),
    )
}

/// A paired chest is one port inventory; both halves must be authorized before
/// the journal chooses a slot. This also covers native settlement boundaries.
pub fn can_transport_inventory(
    chunks: &ChunkStore,
    journal: &EditJournal,
    from: [i64; 3],
    inventory: [i64; 3],
) -> bool {
    can_transport(chunks, journal, from, inventory)
        && (journal.block(inventory) != Some(29)
            || journal
                .object_state(inventory)
                .and_then(|s| s.peer)
                .is_none_or(|peer| can_transport(chunks, journal, from, peer)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::structures::{
        SettlementLayout, VillageSettlement, VillageSize, VillageStyle,
    };
    use std::sync::Arc;
    use terra_core::kingdom::{BotKingdom, KingdomRole};

    #[test]
    fn native_bot_access_and_captured_roles_resolve_through_cached_settlement_columns() {
        let generator = crate::SectionGenerator::new(
            crate::TerraGenerator::new(terra_core::WorldConfig {
                biomes: false,
                trees: false,
                water: false,
                caves: false,
                structures_revision: 0,
                ..Default::default()
            })
            .unwrap(),
        );
        let permissions = [
            KingdomPermission::Build,
            KingdomPermission::Break,
            KingdomPermission::Inventories,
            KingdomPermission::NpcWork,
            KingdomPermission::Openings,
        ];
        let visitor = PlayerUuid::from_u128(10);
        let founder = PlayerUuid::from_u128(11);
        let p = [20, 20, 4];
        for layout in [
            SettlementLayout::Village,
            SettlementLayout::KingdomTemperate,
            SettlementLayout::Outpost,
        ] {
            for (bot, expected) in [
                (BotKingdom::Lutera, [false, false, false, false, true]),
                (BotKingdom::Betross, [false, false, false, false, true]),
                (BotKingdom::Helteria, [true; 5]),
                (BotKingdom::Petora, [false; 5]),
            ] {
                let id = (0..1000)
                    .find(|&id| {
                        BotKingdom::for_settlement(id, layout != SettlementLayout::Village)
                            == Some(bot)
                    })
                    .unwrap();
                let site = Arc::new(VillageSettlement {
                    id,
                    size: VillageSize::Small,
                    layout,
                    style: VillageStyle::Temperate,
                    center: [4, 20, 4],
                    bounds: [[0, 0, 0], [31, 60, 31]],
                    homes: Vec::new(),
                    services: Vec::new(),
                    residents: Vec::new(),
                });
                let mut chunks = ChunkStore::default();
                let column = generator
                    .prepare_column([0, 0], false)
                    .unwrap()
                    .with_settlement_for_test(site);
                chunks.set_prepared_column_for_test([0, 0], Some(Arc::new(column)));
                let mut registry = KingdomRegistry::default();
                assert_eq!(
                    permissions
                        .map(|permission| can_use(&registry, &chunks, visitor, permission, p)),
                    expected
                );
                let realm = registry.found(founder, [4, 20, 4], "Captured").unwrap();
                assert!(
                    permissions
                        .into_iter()
                        .all(|permission| !can_use(&registry, &chunks, visitor, permission, p))
                );
                registry.join(visitor, realm).unwrap();
                registry
                    .set_role(founder, realm, visitor, KingdomRole::Worker)
                    .unwrap();
                assert_eq!(
                    permissions
                        .map(|permission| can_use(&registry, &chunks, visitor, permission, p)),
                    [false, false, true, true, true]
                );
                registry.dissolve(founder, realm).unwrap();
                assert_eq!(
                    permissions
                        .map(|permission| can_use(&registry, &chunks, visitor, permission, p)),
                    expected
                );
            }
        }
    }
}
