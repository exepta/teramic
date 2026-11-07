use super::*;
use std::collections::BTreeMap;

impl KingdomRegistry {
    pub const MAX_BATCH_CLAIMS: usize = 256;

    /// Validate the final selection as one transaction. Iteration order cannot
    /// reject a rectangle whose first cell is far from its existing border.
    pub fn claim_batch(
        &mut self,
        id: KingdomId,
        changes: &[([i64; 2], ClaimKind)],
        actor: ClaimActor,
    ) -> Result<u16, KingdomError> {
        let index = self.active_index(id)?;
        if changes.is_empty() || changes.len() > Self::MAX_BATCH_CLAIMS {
            return Err(KingdomError::InvalidClaim);
        }
        if actor.authorizer().is_nil() || matches!(actor, ClaimActor::Bot { id: 0, .. }) {
            return Err(KingdomError::InvalidIdentity);
        }
        if !self.kingdoms[index].members.contains(&actor.authorizer()) {
            return Err(KingdomError::NotMember);
        }
        if !self.can_manage(actor.authorizer(), id) {
            return Err(KingdomError::PermissionDenied);
        }
        let seat = claim_chunk(self.kingdoms[index].location);
        let mut selected = BTreeMap::new();
        for &(chunk, kind) in changes {
            if !valid_claim_chunk(chunk) || selected.insert(chunk, kind).is_some() {
                return Err(KingdomError::InvalidClaim);
            }
            if chunk == seat && kind != ClaimKind::ProtectedBuilding {
                return Err(KingdomError::SeatMustStayClaimed);
            }
            if self.claim_at(chunk).is_some_and(|c| c.kingdom != id)
                || self.kingdoms.iter().any(|k| {
                    k.id != id
                        && k.state == KingdomState::Active
                        && claim_chunk(k.location) == chunk
                })
            {
                return Err(KingdomError::ClaimOccupied);
            }
        }
        let changed = selected
            .iter()
            .filter(|(p, k)| self.claim_at(**p).is_none_or(|c| c.kind != **k))
            .count();
        if changed == 0 {
            return Ok(0);
        }
        let mut next = self.claims.clone();
        next.retain(|c| {
            c.kingdom != id || selected.get(&c.chunk).is_none_or(|kind| *kind == c.kind)
        });
        next.extend(
            selected
                .into_iter()
                .filter(|(chunk, kind)| self.claim_at(*chunk).is_none_or(|c| c.kind != *kind))
                .map(|(chunk, kind)| KingdomClaim {
                    kingdom: id,
                    chunk,
                    kind,
                    actor,
                }),
        );
        if next.len() > MAX_CLAIMS {
            return Err(KingdomError::CapacityReached);
        }
        let own: Vec<_> = next.iter().filter(|c| c.kingdom == id).collect();
        let limits = self.kingdoms[index].limits();
        if own
            .iter()
            .filter(|c| c.kind == ClaimKind::ProtectedBuilding)
            .count()
            > limits.protected as usize
            || own
                .iter()
                .filter(|c| c.kind == ClaimKind::RealmTerritory)
                .count()
                > limits.territory as usize
        {
            return Err(KingdomError::ClaimLimitReached);
        }
        if !super::claims::claim_connections_preserved(id, seat, &self.claims, next.iter()) {
            return Err(KingdomError::ClaimDisconnected);
        }
        self.can_change()?;
        self.claims = next;
        self.revision += 1;
        Ok(changed as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rectangles_validate_final_connectivity_and_reject_whole_invalid_batches() {
        let owner = PlayerUuid::from_u128(55);
        let mut registry = KingdomRegistry::default();
        let id = registry.found(owner, [0, 20, 0], "Northwatch").unwrap();
        let actor = ClaimActor::Player(owner);
        let rev = registry.revision;
        let changes = [
            ([-2, 0], ClaimKind::RealmTerritory),
            ([-1, 0], ClaimKind::ProtectedBuilding),
            ([-2, -1], ClaimKind::RealmTerritory),
            ([-1, -1], ClaimKind::ProtectedBuilding),
        ];
        assert_eq!(registry.claim_batch(id, &changes, actor), Ok(4));
        assert_eq!(registry.revision, rev + 1);
        assert!(registry.valid());
        let before = registry.clone();
        for invalid in [
            vec![([20, 20], ClaimKind::ProtectedBuilding)],
            vec![([0, 0], ClaimKind::RealmTerritory)],
            vec![([1, 0], ClaimKind::ProtectedBuilding); 257],
        ] {
            assert!(registry.claim_batch(id, &invalid, actor).is_err());
            assert_eq!(registry, before);
        }
        assert_eq!(registry.claim_batch(id, &changes, actor), Ok(0));
        assert_eq!(registry, before);
        // Repainting across a bot's unchanged land must retain its provenance.
        let bot_actor = ClaimActor::Bot {
            id: 77,
            authorized_by: owner,
        };
        registry
            .claims
            .iter_mut()
            .find(|c| c.chunk == [-2, 0])
            .unwrap()
            .actor = bot_actor;
        assert_eq!(
            registry.claim_batch(
                id,
                &[
                    ([-2, 0], ClaimKind::RealmTerritory),
                    ([1, 0], ClaimKind::ProtectedBuilding)
                ],
                actor
            ),
            Ok(1)
        );
        assert_eq!(registry.claim_at([-2, 0]).unwrap().actor, bot_actor);
        assert!(registry.valid());
    }
}
