//! Configured founding and expansion policy, independent of physical keep distance.
use super::*;
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "territory_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "build_connectivity_tests.rs"]
mod build_connectivity_tests;

fn free_gap(chunk: [i64; 2], low: [i64; 2], high: [i64; 2]) -> u64 {
    (0..2)
        .map(|axis| {
            if chunk[axis] < low[axis] {
                low[axis].abs_diff(chunk[axis]).saturating_sub(1)
            } else if chunk[axis] > high[axis] {
                chunk[axis].abs_diff(high[axis]).saturating_sub(1)
            } else {
                0
            }
        })
        .max()
        .unwrap_or(0)
}

fn realm(kind: SettlementKind) -> bool {
    matches!(kind, SettlementKind::Kingdom | SettlementKind::Outpost)
}
impl KingdomRegistry {
    /// Dependent settlements inherit membership and diplomacy from one main kingdom.
    pub fn realm_id(&self, id: KingdomId) -> KingdomId {
        self.get(id).and_then(|record| record.parent).unwrap_or(id)
    }
    pub fn limits_with_rules(&self, id: KingdomId, rules: &KingdomRules) -> ClaimLimits {
        self.get(id).map_or(ClaimLimits::default(), |record| {
            let level = if rules.settlements_follow_kingdom_level {
                self.get(self.realm_id(id))
                    .map_or(record.level, |parent| parent.level)
            } else {
                record.level
            };
            rules.limits(record.kind, level)
        })
    }
    pub(super) fn sync_members(&mut self, id: KingdomId) {
        let Some(parent) = self.get(id).cloned() else {
            return;
        };
        for child in self
            .kingdoms
            .iter_mut()
            .filter(|record| record.parent == Some(id) && record.state == KingdomState::Active)
        {
            child.founder = parent.founder;
            child.founder_name = parent.founder_name.clone();
            child.members = parent.members.clone();
            child.member_names = parent.member_names.clone();
        }
    }
    pub(super) fn valid_hierarchy(&self) -> bool {
        self.kingdoms.iter().all(|record| {
            record.parent.is_none_or(|parent| {
                parent != record.id
                    && record.kind != SettlementKind::Kingdom
                    && self.get(parent).is_some_and(|main| {
                        main.parent.is_none()
                            && main.kind == SettlementKind::Kingdom
                            && (record.state == KingdomState::Dissolved
                                || main.state == KingdomState::Active
                                    && record.founder == main.founder
                                    && record.founder_name == main.founder_name
                                    && record.members == main.members
                                    && record.member_names == main.member_names)
                    })
            })
        })
    }
    fn parent_territory(&self, id: KingdomId, chunk: [i64; 2]) -> bool {
        self.get(id)
            .and_then(|record| record.parent)
            .is_some_and(|parent| {
                self.claims.iter().any(|claim| {
                    self.realm_id(claim.kingdom) == parent
                        && claim.chunk == chunk
                        && claim.kind == ClaimKind::RealmTerritory
                })
            })
    }
    pub(super) fn compatible_overlap(&self, first: &KingdomClaim, second: &KingdomClaim) -> bool {
        let (build, territory) = if first.kind == ClaimKind::ProtectedBuilding
            && (second.kind == ClaimKind::RealmTerritory)
        {
            (first, second)
        } else if second.kind == ClaimKind::ProtectedBuilding
            && first.kind == ClaimKind::RealmTerritory
        {
            (second, first)
        } else {
            return false;
        };
        self.get(build.kingdom).is_some_and(|child| {
            child.parent == Some(self.realm_id(territory.kingdom))
                && matches!(child.kind, SettlementKind::Village | SettlementKind::City)
        })
    }
    fn validate_land(
        &self,
        id: KingdomId,
        chunk: [i64; 2],
        kind: ClaimKind,
        rules: &KingdomRules,
        domains: &[SettlementDomain],
        ownership: &KingdomRegistry,
    ) -> Result<(), KingdomError> {
        let record = self.get(id).ok_or(KingdomError::UnknownKingdom)?;
        if matches!(record.kind, SettlementKind::Village | SettlementKind::City) {
            if kind == ClaimKind::RealmTerritory {
                return Err(KingdomError::SettlementTerritoryForbidden);
            }
            let required = if record.kind == SettlementKind::Village {
                rules.village_requires_territory
            } else {
                rules.city_requires_territory
            };
            if required && record.parent.is_some() && !self.parent_territory(id, chunk) {
                return Err(KingdomError::ParentTerritoryRequired);
            }
        }
        if realm(record.kind) && rules.realm_min_border_gap > 0 {
            for other in self.kingdoms.iter().filter(|other| {
                other.id != id && other.state == KingdomState::Active && realm(other.kind)
            }) {
                let mut land = self
                    .claims
                    .iter()
                    .filter(|claim| claim.kingdom == other.id)
                    .map(|claim| claim.chunk)
                    .peekable();
                if land.peek().is_none() {
                    let seat = claim_chunk(other.location);
                    if free_gap(chunk, seat, seat) < u64::from(rules.realm_min_border_gap) {
                        return Err(KingdomError::RealmBorderTooClose);
                    }
                }
                for border in land {
                    if free_gap(chunk, border, border) < u64::from(rules.realm_min_border_gap) {
                        return Err(KingdomError::RealmBorderTooClose);
                    }
                }
            }
            for domain in domains
                .iter()
                .filter(|domain| matches!(domain.kind, DomainKind::Kingdom | DomainKind::Outpost))
            {
                if ownership
                    .domain_owner(domain, domain.center)
                    .is_some_and(|owner| owner.id == id)
                {
                    continue;
                }
                if free_gap(
                    chunk,
                    claim_chunk(domain.bounds[0]),
                    claim_chunk(domain.bounds[1]),
                ) < u64::from(rules.realm_min_border_gap)
                {
                    return Err(KingdomError::RealmBorderTooClose);
                }
            }
        }
        if kind == ClaimKind::ProtectedBuilding && rules.city_min_build_gap > 0 {
            for claim in self
                .claims
                .iter()
                .filter(|claim| claim.kingdom != id && claim.kind == ClaimKind::ProtectedBuilding)
            {
                let Some(other) = self
                    .get(claim.kingdom)
                    .filter(|other| other.state == KingdomState::Active)
                else {
                    continue;
                };
                let opposite = record.kind == SettlementKind::City
                    && matches!(
                        other.kind,
                        SettlementKind::Kingdom | SettlementKind::Village
                    )
                    || other.kind == SettlementKind::City
                        && matches!(
                            record.kind,
                            SettlementKind::Kingdom | SettlementKind::Village
                        );
                if opposite
                    && self.realm_id(other.id) == self.realm_id(id)
                    && free_gap(chunk, claim.chunk, claim.chunk)
                        < u64::from(rules.city_min_build_gap)
                {
                    return Err(KingdomError::CityBorderTooClose);
                }
            }
            if record.kind == SettlementKind::City {
                for domain in domains
                    .iter()
                    .filter(|domain| domain.kind == DomainKind::Village)
                {
                    if self
                        .domain_owner(domain, domain.center)
                        .is_some_and(|owner| self.realm_id(owner.id) == self.realm_id(id))
                        && free_gap(
                            chunk,
                            claim_chunk(domain.bounds[0]),
                            claim_chunk(domain.bounds[1]),
                        ) < u64::from(rules.city_min_build_gap)
                    {
                        return Err(KingdomError::CityBorderTooClose);
                    }
                }
            }
        }
        Ok(())
    }
    pub fn claim_with_rules(
        &mut self,
        id: KingdomId,
        chunk: [i64; 2],
        kind: ClaimKind,
        actor: ClaimActor,
        rules: &KingdomRules,
        domains: &[SettlementDomain],
    ) -> Result<(), KingdomError> {
        self.claim_batch_with_rules(id, &[(chunk, kind)], actor, rules, domains)
            .map(|_| ())
    }
    pub fn claim_batch_with_rules(
        &mut self,
        id: KingdomId,
        changes: &[([i64; 2], ClaimKind)],
        actor: ClaimActor,
        rules: &KingdomRules,
        domains: &[SettlementDomain],
    ) -> Result<u16, KingdomError> {
        if !rules.valid() {
            return Err(KingdomError::InvalidKingdomRules);
        }
        let index = self.active_index(id)?;
        if !actor.valid() {
            return Err(KingdomError::InvalidIdentity);
        }
        if self.role_for(id, actor.authorizer()).is_none() {
            return Err(KingdomError::NotMember);
        }
        if !self.can_manage(actor.authorizer(), id) {
            return Err(KingdomError::PermissionDenied);
        }
        if changes.is_empty() || changes.len() > Self::MAX_BATCH_CLAIMS {
            return Err(KingdomError::InvalidClaim);
        }
        let mut selected = BTreeMap::new();
        let mut changed = 0;
        for &(chunk, kind) in changes {
            if !valid_claim_chunk(chunk) || selected.insert(chunk, kind).is_some() {
                return Err(KingdomError::InvalidClaim);
            }
            if chunk == claim_chunk(self.kingdoms[index].location)
                && kind != ClaimKind::ProtectedBuilding
            {
                return Err(KingdomError::SeatMustStayClaimed);
            }
            let candidate = KingdomClaim {
                kingdom: id,
                chunk,
                kind,
                actor,
            };
            for existing in self
                .claims
                .iter()
                .filter(|claim| claim.chunk == chunk && claim.kingdom != id)
            {
                if !self.compatible_overlap(&candidate, existing) {
                    return Err(KingdomError::ClaimOccupied);
                }
            }
            if self.kingdoms.iter().any(|other| {
                other.id != id
                    && other.state == KingdomState::Active
                    && claim_chunk(other.location) == chunk
            }) {
                return Err(KingdomError::ClaimOccupied);
            }
            if self
                .claims
                .iter()
                .any(|claim| claim.kingdom == id && claim.chunk == chunk && claim.kind == kind)
            {
                continue;
            }
            self.validate_land(id, chunk, kind, rules, domains, self)?;
            changed += 1;
        }
        if changed == 0 {
            return Ok(0);
        }
        let mut next = self.claims.clone();
        for (&chunk, &kind) in &selected {
            if let Some(existing) = next
                .iter_mut()
                .find(|claim| claim.kingdom == id && claim.chunk == chunk)
            {
                if existing.kind != kind {
                    *existing = KingdomClaim {
                        kingdom: id,
                        chunk,
                        kind,
                        actor,
                    };
                }
            } else {
                next.push(KingdomClaim {
                    kingdom: id,
                    chunk,
                    kind,
                    actor,
                });
            }
        }
        if next.len() > MAX_CLAIMS {
            return Err(KingdomError::CapacityReached);
        }
        let own = next
            .iter()
            .filter(|claim| claim.kingdom == id)
            .collect::<Vec<_>>();
        let limits = self.limits_with_rules(id, rules);
        if own
            .iter()
            .filter(|claim| claim.kind == ClaimKind::ProtectedBuilding)
            .count()
            > limits.protected as usize
            || own
                .iter()
                .filter(|claim| claim.kind == ClaimKind::RealmTerritory)
                .count()
                > limits.territory as usize
        {
            return Err(KingdomError::ClaimLimitReached);
        }
        if rules.require_adjacent_claims
            && !super::claims::claim_connections_preserved(
                id,
                claim_chunk(self.kingdoms[index].location),
                &self.claims,
                next.iter(),
            )
        {
            return Err(KingdomError::ClaimDisconnected);
        }
        self.can_change()?;
        self.claims = next;
        self.revision += 1;
        Ok(changed)
    }
    pub fn release_with_rules(
        &mut self,
        actor: PlayerUuid,
        id: KingdomId,
        chunk: [i64; 2],
        rules: &KingdomRules,
    ) -> Result<(), KingdomError> {
        let index = self.active_index(id)?;
        if self.role_for(id, actor).is_none() {
            return Err(KingdomError::NotMember);
        }
        if !self.can_manage(actor, id) {
            return Err(KingdomError::PermissionDenied);
        }
        if chunk == claim_chunk(self.kingdoms[index].location) {
            return Err(KingdomError::SeatMustStayClaimed);
        }
        let position = self
            .claims
            .iter()
            .position(|claim| claim.kingdom == id && claim.chunk == chunk)
            .ok_or(KingdomError::ClaimMissing)?;
        if self.claims[position].kind == ClaimKind::RealmTerritory
            && self.claims.iter().any(|claim| {
                claim.chunk == chunk
                    && claim.kingdom != id
                    && claim.kind == ClaimKind::ProtectedBuilding
                    && self.get(claim.kingdom).is_some_and(|child| {
                        child.parent == Some(self.realm_id(id))
                            && matches!(child.kind, SettlementKind::Village | SettlementKind::City)
                    })
            })
        {
            return Err(KingdomError::ParentTerritoryInUse);
        }
        if rules.require_adjacent_claims
            && !super::claims::claim_connections_preserved(
                id,
                claim_chunk(self.kingdoms[index].location),
                &self.claims,
                self.claims
                    .iter()
                    .enumerate()
                    .filter_map(|(index, claim)| (index != position).then_some(claim)),
            )
        {
            return Err(KingdomError::ClaimDisconnected);
        }
        self.can_change()?;
        self.claims.remove(position);
        self.revision += 1;
        Ok(())
    }
    // Keep the founding inputs parallel to the legacy API; the two additional
    // contexts are the live policy and immutable native settlement boundaries.
    #[allow(clippy::too_many_arguments)]
    pub fn found_settlement_with_rules(
        &mut self,
        actor: PlayerUuid,
        actor_name: &str,
        position: [i64; 3],
        name: &str,
        kind: SettlementKind,
        heraldry: KingdomHeraldry,
        rules: &KingdomRules,
        domains: &[SettlementDomain],
    ) -> Result<KingdomId, KingdomError> {
        if !rules.valid() {
            return Err(KingdomError::InvalidKingdomRules);
        }
        if !heraldry.icon.valid() {
            return Err(KingdomError::InvalidHeraldry);
        }
        if kind == SettlementKind::Kingdom {
            let mut next = self.clone();
            let id = next.found_settlement(actor, actor_name, position, name, kind, heraldry)?;
            next.validate_land(
                id,
                claim_chunk(position),
                ClaimKind::ProtectedBuilding,
                rules,
                domains,
                self,
            )?;
            *self = next;
            return Ok(id);
        }
        let parent = self
            .active_for_member(actor)
            .filter(|parent| parent.kind == SettlementKind::Kingdom && parent.parent.is_none())
            .cloned()
            .ok_or(KingdomError::ParentKingdomRequired)?;
        if !self.can_manage(actor, parent.id) {
            return Err(KingdomError::PermissionDenied);
        }
        if kind == SettlementKind::City && parent.level < rules.city_min_kingdom_level {
            return Err(KingdomError::KingdomLevelRequired);
        }
        if kind == SettlementKind::Outpost
            && self
                .kingdoms
                .iter()
                .filter(|child| {
                    child.parent == Some(parent.id)
                        && child.state == KingdomState::Active
                        && child.kind == SettlementKind::Outpost
                })
                .count()
                >= usize::from(parent.level) * usize::from(rules.outposts_per_level)
        {
            return Err(KingdomError::OutpostLimitReached);
        }
        if !valid_location(position) {
            return Err(KingdomError::InvalidLocation);
        }
        let name = normalized_name(name)?;
        if self.active_at(position).is_some() {
            return Err(KingdomError::KeepAlreadyBound);
        }
        if self.kingdoms.iter().any(|record| {
            record.state == KingdomState::Active
                && record.name.to_lowercase() == name.to_lowercase()
        }) {
            return Err(KingdomError::NameTaken);
        }
        if self.kingdoms.len() >= MAX_KINGDOMS || self.claims.len() >= MAX_CLAIMS {
            return Err(KingdomError::CapacityReached);
        }
        self.can_change()?;
        let next_id = self
            .next_id
            .checked_add(1)
            .ok_or(KingdomError::CapacityReached)?;
        let id = KingdomId(self.next_id);
        let mut next = self.clone();
        next.kingdoms.push(KingdomRecord {
            id,
            name,
            founder: parent.founder,
            founder_name: parent.founder_name.clone(),
            members: parent.members.clone(),
            member_names: parent.member_names.clone(),
            location: position,
            state: KingdomState::Active,
            kind,
            level: 1,
            heraldry: parent.heraldry,
            parent: Some(parent.id),
        });
        next.validate_land(
            id,
            claim_chunk(position),
            ClaimKind::ProtectedBuilding,
            rules,
            domains,
            self,
        )?;
        let candidate = KingdomClaim {
            kingdom: id,
            chunk: claim_chunk(position),
            kind: ClaimKind::ProtectedBuilding,
            actor: ClaimActor::Player(actor),
        };
        if next.claims.iter().any(|existing| {
            existing.chunk == candidate.chunk && !next.compatible_overlap(&candidate, existing)
        }) {
            return Err(KingdomError::ClaimOccupied);
        }
        next.claims.push(candidate);
        next.next_id = next_id;
        next.revision += 1;
        *self = next;
        Ok(id)
    }
}
