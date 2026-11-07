//! Viewer-relative labels shared by territory announcements and the map.
use terra_core::{
    PlayerUuid,
    kingdom::{DiplomaticRelation, DiplomaticTarget, KingdomRegistry, KingdomRelation},
};

pub(crate) fn display(
    registry: &KingdomRegistry,
    viewer: Option<PlayerUuid>,
    target: DiplomaticTarget,
) -> (DiplomaticRelation, &'static str) {
    if let Some(realm) = viewer.and_then(|uuid| registry.active_for_member(uuid)) {
        let relation = registry.relation_to(realm.id, target);
        return (relation, relation.label());
    }
    match target {
        DiplomaticTarget::Bot(bot) => match bot.relation() {
            KingdomRelation::Friendly => (DiplomaticRelation::Allied, "Friendly"),
            KingdomRelation::Hostile => (DiplomaticRelation::Hostile, "Hostile"),
            KingdomRelation::Neutral => (DiplomaticRelation::Neutral, "Neutral"),
        },
        DiplomaticTarget::Player(_) => (DiplomaticRelation::Neutral, "Neutral"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::kingdom::BotKingdom;
    #[test]
    fn viewer_relations_require_reciprocal_alliance_refresh_live_and_restore_native_defaults() {
        let a = PlayerUuid::from_u128(31);
        let b = PlayerUuid::from_u128(32);
        let mut registry = KingdomRegistry::default();
        let own = registry.found(a, [0, 20, 0], "Homewatch").unwrap();
        let other = registry.found(b, [64, 20, 0], "Eastbank").unwrap();
        assert_eq!(
            display(&registry, Some(a), DiplomaticTarget::Player(own)).0,
            DiplomaticRelation::Own
        );
        registry
            .set_relation(
                a,
                own,
                DiplomaticTarget::Player(other),
                DiplomaticRelation::Allied,
            )
            .unwrap();
        assert_eq!(
            display(&registry, Some(a), DiplomaticTarget::Player(other)).0,
            DiplomaticRelation::Neutral
        );
        registry
            .set_relation(
                b,
                other,
                DiplomaticTarget::Player(own),
                DiplomaticRelation::Allied,
            )
            .unwrap();
        assert_eq!(
            display(&registry, Some(a), DiplomaticTarget::Player(other)).0,
            DiplomaticRelation::Allied
        );
        registry
            .set_relation(
                b,
                other,
                DiplomaticTarget::Player(own),
                DiplomaticRelation::Hostile,
            )
            .unwrap();
        assert_eq!(
            display(&registry, Some(a), DiplomaticTarget::Player(other)).0,
            DiplomaticRelation::Hostile
        );
        assert_eq!(
            display(&registry, None, DiplomaticTarget::Player(other)).0,
            DiplomaticRelation::Neutral
        );
        assert_eq!(
            display(
                &registry,
                Some(a),
                DiplomaticTarget::Bot(BotKingdom::Helteria)
            )
            .1,
            "Neutral"
        );
        assert_eq!(
            display(&registry, None, DiplomaticTarget::Bot(BotKingdom::Helteria)).1,
            "Friendly"
        );
        registry.dissolve(a, own).unwrap();
        assert_eq!(
            display(
                &registry,
                Some(a),
                DiplomaticTarget::Bot(BotKingdom::Helteria)
            )
            .1,
            "Friendly"
        );
        assert_eq!(
            display(&registry, None, DiplomaticTarget::Bot(BotKingdom::Petora)).1,
            "Hostile"
        );
    }
}
