//! Shared client/server access to the keep from one's currently claimed land.
use super::{KingdomRecord, KingdomRegistry, KingdomState, valid_location};
use crate::PlayerUuid;

impl KingdomRegistry {
    /// A dependent village/city opens its parent realm's keep. Membership and
    /// current land ownership must both agree; diplomacy alone grants no access.
    pub fn territorial_keep(
        &self,
        player: PlayerUuid,
        position: [i64; 3],
    ) -> Option<&KingdomRecord> {
        if player.is_nil() || !valid_location(position) {
            return None;
        }
        let region = self.region_at(position)?;
        let realm = self.get(self.realm_id(region.id))?;
        (realm.state == KingdomState::Active && realm.members.contains(&player)).then_some(realm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kingdom::{ClaimActor, ClaimKind};

    #[test]
    fn territory_access_requires_current_membership_and_claim_including_negative_chunks() {
        let owner = PlayerUuid::from_u128(236);
        let visitor = PlayerUuid::from_u128(237);
        let mut registry = KingdomRegistry::default();
        let id = registry.found(owner, [-8, 4, -8], "Westkeep").unwrap();
        registry
            .claim(
                id,
                [-2, -1],
                ClaimKind::RealmTerritory,
                ClaimActor::Player(owner),
            )
            .unwrap();
        assert_eq!(
            registry
                .territorial_keep(owner, [-20, 99, -8])
                .map(|r| r.id),
            Some(id)
        );
        assert_eq!(
            registry.territorial_keep(owner, [-8, 4, -8]).map(|r| r.id),
            Some(id)
        );
        assert!(registry.territorial_keep(visitor, [-20, 4, -8]).is_none());
        assert!(registry.territorial_keep(owner, [200, 4, 200]).is_none());
        registry.dissolve(owner, id).unwrap();
        assert!(registry.territorial_keep(owner, [-20, 4, -8]).is_none());
    }
}
