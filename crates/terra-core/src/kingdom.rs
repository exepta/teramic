//! Persistent settlement identity, membership and horizontal chunk ownership.
mod batch;
mod bots;
mod claims;
mod domains;
#[cfg(test)]
mod permission_tests;
mod permissions;
mod rules;
mod territory;
use crate::PlayerUuid;
pub use bots::*;
pub use claims::*;
pub use domains::*;
pub use permissions::*;
pub use rules::KingdomRules;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};

pub const MAX_KINGDOMS: usize = 256;
pub const MAX_KINGDOM_MEMBERS: usize = 64;
pub const MAX_KINGDOM_NAME_CHARS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KingdomId(pub u64);

impl fmt::Display for KingdomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KingdomState {
    Active,
    Dissolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KingdomRecord {
    pub id: KingdomId,
    pub name: String,
    pub founder: PlayerUuid,
    pub founder_name: String,
    pub members: Vec<PlayerUuid>,
    pub member_names: Vec<(PlayerUuid, String)>,
    pub location: [i64; 3],
    pub state: KingdomState,
    #[serde(default)]
    pub kind: SettlementKind,
    #[serde(default = "initial_level")]
    pub level: u16,
    #[serde(default)]
    pub heraldry: KingdomHeraldry,
    #[serde(default)]
    pub parent: Option<KingdomId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KingdomRegistry {
    /// Next unused ID; dissolved records remain as history and IDs are never reused.
    pub next_id: u64,
    pub revision: u64,
    pub kingdoms: Vec<KingdomRecord>,
    pub claims: Vec<KingdomClaim>,
    pub roles: Vec<MemberRole>,
    pub relations: Vec<RelationOffer>,
}

impl Default for KingdomRegistry {
    fn default() -> Self {
        Self {
            next_id: 1,
            revision: 0,
            kingdoms: Vec::new(),
            claims: Vec::new(),
            roles: Vec::new(),
            relations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KingdomError {
    InvalidName,
    NameTaken,
    InvalidIdentity,
    InvalidLocation,
    CapacityReached,
    AlreadyMember,
    NotMember,
    FounderOnly,
    FounderMustDissolve,
    NotActive,
    UnknownKingdom,
    KeepAlreadyBound,
    NotBergfried,
    OutOfReach,
    Occluded,
    NotAllowed,
    RateLimited,
    StaleRequest,
    ClaimOccupied,
    ClaimMissing,
    ClaimLimitReached,
    ClaimDisconnected,
    InvalidClaim,
    InvalidBot,
    SeatMustStayClaimed,
    UpgradeLimitReached,
    UpgradeMaterialsMissing,
    InvalidHeraldry,
    PermissionDenied,
    InvalidRelation,
    InvalidKingdomRules,
    ParentKingdomRequired,
    ParentTerritoryRequired,
    OutpostLimitReached,
    KingdomLevelRequired,
    RealmBorderTooClose,
    CityBorderTooClose,
    SettlementTerritoryForbidden,
    ParentTerritoryInUse,
    SettlementLevelInherited,
}

impl fmt::Display for KingdomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidName => "Use 1–32 printable characters for the kingdom name.",
            Self::NameTaken => "An active kingdom already uses this name.",
            Self::InvalidIdentity => "A valid player identity is required.",
            Self::InvalidLocation => "The Bergfried location is outside the world bounds.",
            Self::CapacityReached => "The kingdom or member limit has been reached.",
            Self::AlreadyMember => "You already belong to an active kingdom.",
            Self::NotMember => "This player is not a member of the kingdom.",
            Self::FounderOnly => "Only the founder can change this kingdom.",
            Self::FounderMustDissolve => "The founder must dissolve the kingdom before leaving.",
            Self::NotActive => "This kingdom has already been dissolved.",
            Self::UnknownKingdom => "This kingdom no longer exists.",
            Self::KeepAlreadyBound => "This Bergfried already belongs to an active kingdom.",
            Self::NotBergfried => "Found a kingdom at a placed Bergfried structure block.",
            Self::OutOfReach => "Move closer to this kingdom's Bergfried.",
            Self::Occluded => "The Bergfried must be visible and unobstructed.",
            Self::NotAllowed => "You cannot manage a kingdom in the current player state.",
            Self::RateLimited => "Please wait before sending another kingdom request.",
            Self::StaleRequest => "This kingdom request has already been processed.",
            Self::ClaimOccupied => "This chunk belongs to another settlement or contains its Bergfried.",
            Self::ClaimMissing => "This chunk is not claimed by your settlement.",
            Self::ClaimLimitReached => "This claim type has reached the settlement's level limit.",
            Self::ClaimDisconnected => "Build claims must connect to the Bergfried through Build claims; Territory must stay connected to your land.",
            Self::InvalidClaim => "This chunk is outside the claimable world.",
            Self::InvalidBot => "This bot must be alive, nearby and owned by an authorized settlement member.",
            Self::SeatMustStayClaimed => "The Bergfried chunk must stay protected. Dissolve the settlement to release it.",
            Self::UpgradeLimitReached => "This settlement has reached the maximum level of 64.",
            Self::UpgradeMaterialsMissing => "Your inventory does not contain the stone walls and gold ingots needed for this upgrade.",
            Self::InvalidHeraldry => "Choose a valid heraldry icon.",
            Self::PermissionDenied=>"Your kingdom role does not permit this action.",
            Self::InvalidRelation=>"Choose another active realm and a permitted diplomatic stance.",
            Self::InvalidKingdomRules=>"The kingdom rules are invalid.",
            Self::ParentKingdomRequired=>"Join and manage a kingdom before founding this settlement.",
            Self::ParentTerritoryRequired=>"This settlement must be inside its kingdom's territorial claims.",
            Self::OutpostLimitReached=>"Your kingdom level does not allow another outpost.",
            Self::KingdomLevelRequired=>"Your kingdom must reach the configured level before founding a city.",
            Self::RealmBorderTooClose=>"Leave the configured free-chunk border between kingdoms and outposts.",
            Self::CityBorderTooClose=>"Leave the configured free-chunk border between city and kingdom/village build land.",
            Self::SettlementTerritoryForbidden=>"Villages and cities use their kingdom's territory; claim build land only.",
            Self::ParentTerritoryInUse=>"A dependent settlement still occupies this territorial chunk.",
            Self::SettlementLevelInherited=>"This settlement inherits its kingdom level under the current rules.",
        })
    }
}

impl std::error::Error for KingdomError {}

pub fn normalized_name(name: &str) -> Result<String, KingdomError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_KINGDOM_NAME_CHARS
        || name.chars().any(|c| c.is_control() || matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2069}' | '\u{feff}'))
    {
        return Err(KingdomError::InvalidName);
    }
    Ok(name.to_owned())
}

fn fallback_member_name(player: PlayerUuid) -> String {
    format!("Player {}", &player.to_string()[..8])
}

fn member_name(name: &str) -> Result<String, KingdomError> {
    normalized_name(name).map_err(|_| KingdomError::InvalidIdentity)
}

impl KingdomRegistry {
    pub fn get(&self, id: KingdomId) -> Option<&KingdomRecord> {
        self.kingdoms.iter().find(|kingdom| kingdom.id == id)
    }

    pub fn active_at(&self, location: [i64; 3]) -> Option<&KingdomRecord> {
        self.kingdoms
            .iter()
            .find(|k| k.state == KingdomState::Active && k.location == location)
    }

    pub fn active_for_member(&self, player: PlayerUuid) -> Option<&KingdomRecord> {
        self.kingdoms.iter().find(|k| {
            k.state == KingdomState::Active && k.parent.is_none() && k.members.contains(&player)
        })
    }

    pub fn valid(&self) -> bool {
        if self.next_id == 0
            || self.kingdoms.len() > MAX_KINGDOMS
            || self.revision < self.kingdoms.len() as u64
        {
            return false;
        }
        let mut ids = BTreeSet::new();
        let mut locations = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut active_members = BTreeSet::new();
        self.kingdoms.iter().all(|k| {
            k.id.0 > 0
                && k.id.0 < self.next_id
                && ids.insert(k.id)
                && normalized_name(&k.name).is_ok_and(|name| name == k.name)
                && !k.founder.is_nil()
                && valid_location(k.location)
                && k.heraldry.icon.valid()
                && (1..=MAX_SETTLEMENT_LEVEL).contains(&k.level)
                && member_name(&k.founder_name).is_ok_and(|name| name == k.founder_name)
                && !k.members.is_empty()
                && k.members.len() <= MAX_KINGDOM_MEMBERS
                && k.members.contains(&k.founder)
                && k.members.iter().all(|id| !id.is_nil())
                && k.members.iter().copied().collect::<BTreeSet<_>>().len() == k.members.len()
                && k.member_names.len() == k.members.len()
                && k.member_names
                    .iter()
                    .map(|(id, _)| *id)
                    .collect::<BTreeSet<_>>()
                    == k.members.iter().copied().collect()
                && k.member_names.iter().all(|(id, name)| {
                    member_name(name).is_ok_and(|valid| valid == *name)
                        && (*id != k.founder || *name == k.founder_name)
                })
                && (k.state == KingdomState::Dissolved
                    || (locations.insert(k.location)
                        && names.insert(k.name.to_lowercase())
                        && (k.parent.is_some()
                            || k.members.iter().all(|id| active_members.insert(*id)))))
        }) && self.valid_hierarchy()
            && self.valid_claims()
            && self.valid_permissions()
    }

    fn active_index(&self, id: KingdomId) -> Result<usize, KingdomError> {
        let index = self
            .kingdoms
            .iter()
            .position(|k| k.id == id)
            .ok_or(KingdomError::UnknownKingdom)?;
        if self.kingdoms[index].state != KingdomState::Active {
            return Err(KingdomError::NotActive);
        }
        Ok(index)
    }

    fn can_change(&self) -> Result<(), KingdomError> {
        if self.revision == u64::MAX {
            Err(KingdomError::CapacityReached)
        } else {
            Ok(())
        }
    }

    pub fn found(
        &mut self,
        founder: PlayerUuid,
        location: [i64; 3],
        name: &str,
    ) -> Result<KingdomId, KingdomError> {
        self.found_with_name(founder, &fallback_member_name(founder), location, name)
    }

    pub fn found_with_name(
        &mut self,
        founder: PlayerUuid,
        founder_name: &str,
        location: [i64; 3],
        name: &str,
    ) -> Result<KingdomId, KingdomError> {
        let name = normalized_name(name)?;
        let founder_name = member_name(founder_name)?;
        if founder.is_nil() {
            return Err(KingdomError::InvalidIdentity);
        }
        if !valid_location(location) {
            return Err(KingdomError::InvalidLocation);
        }
        if self.active_for_member(founder).is_some() {
            return Err(KingdomError::AlreadyMember);
        }
        if self.active_at(location).is_some() {
            return Err(KingdomError::KeepAlreadyBound);
        }
        if self.kingdoms.iter().any(|k| {
            k.state == KingdomState::Active && k.name.to_lowercase() == name.to_lowercase()
        }) {
            return Err(KingdomError::NameTaken);
        }
        if self.claim_at(claim_chunk(location)).is_some()
            || self.kingdoms.iter().any(|k| {
                k.state == KingdomState::Active && claim_chunk(k.location) == claim_chunk(location)
            })
        {
            return Err(KingdomError::ClaimOccupied);
        }
        if self.claims.len() >= MAX_CLAIMS {
            return Err(KingdomError::CapacityReached);
        }
        self.can_change()?;
        let next = self
            .next_id
            .checked_add(1)
            .ok_or(KingdomError::CapacityReached)?;
        if self.kingdoms.len() >= MAX_KINGDOMS {
            return Err(KingdomError::CapacityReached);
        }
        let id = KingdomId(self.next_id);
        self.kingdoms.push(KingdomRecord {
            id,
            name,
            founder,
            founder_name: founder_name.clone(),
            members: vec![founder],
            member_names: vec![(founder, founder_name)],
            location,
            state: KingdomState::Active,
            kind: SettlementKind::Kingdom,
            level: 1,
            heraldry: KingdomHeraldry::default(),
            parent: None,
        });
        self.claims.push(KingdomClaim {
            kingdom: id,
            chunk: claim_chunk(location),
            kind: ClaimKind::ProtectedBuilding,
            actor: ClaimActor::Player(founder),
        });
        self.next_id = next;
        self.revision += 1;
        Ok(id)
    }

    pub fn rename(
        &mut self,
        actor: PlayerUuid,
        id: KingdomId,
        name: &str,
    ) -> Result<(), KingdomError> {
        let name = normalized_name(name)?;
        let index = self.active_index(id)?;
        if !self.can_manage(actor, id) {
            return Err(KingdomError::PermissionDenied);
        }
        if self.kingdoms.iter().any(|k| {
            k.id != id
                && k.state == KingdomState::Active
                && k.name.to_lowercase() == name.to_lowercase()
        }) {
            return Err(KingdomError::NameTaken);
        }
        if self.kingdoms[index].name != name {
            self.can_change()?;
            self.kingdoms[index].name = name;
            self.revision += 1;
        }
        Ok(())
    }

    pub fn dissolve(&mut self, actor: PlayerUuid, id: KingdomId) -> Result<(), KingdomError> {
        let index = self.active_index(id)?;
        if self.kingdoms[index].founder != actor {
            return Err(KingdomError::FounderOnly);
        }
        self.can_change()?;
        let ids = self
            .kingdoms
            .iter()
            .filter(|record| record.id == id || record.parent == Some(id))
            .map(|record| record.id)
            .collect::<BTreeSet<_>>();
        for record in &mut self.kingdoms {
            if ids.contains(&record.id) {
                record.state = KingdomState::Dissolved;
            }
        }
        self.claims.retain(|claim| !ids.contains(&claim.kingdom));
        self.roles.retain(|role| !ids.contains(&role.kingdom));
        self.relations.retain(|relation| {
            !ids.contains(&relation.from)
                && !matches!(relation.target,DiplomaticTarget::Player(id) if ids.contains(&id))
        });
        self.revision += 1;
        Ok(())
    }

    pub fn join(&mut self, actor: PlayerUuid, id: KingdomId) -> Result<(), KingdomError> {
        self.join_with_name(actor, &fallback_member_name(actor), id)
    }

    pub fn join_with_name(
        &mut self,
        actor: PlayerUuid,
        name: &str,
        id: KingdomId,
    ) -> Result<(), KingdomError> {
        let id = self.realm_id(id);
        let name = member_name(name)?;
        let index = self.active_index(id)?;
        if actor.is_nil() {
            return Err(KingdomError::InvalidIdentity);
        }
        if self.active_for_member(actor).is_some() {
            return Err(KingdomError::AlreadyMember);
        }
        if self.kingdoms[index].members.len() >= MAX_KINGDOM_MEMBERS {
            return Err(KingdomError::CapacityReached);
        }
        self.can_change()?;
        self.kingdoms[index].members.push(actor);
        self.kingdoms[index].member_names.push((actor, name));
        self.roles.push(MemberRole {
            kingdom: id,
            member: actor,
            role: KingdomRole::Visitor,
        });
        self.sync_members(id);
        self.revision += 1;
        Ok(())
    }

    pub fn leave(&mut self, actor: PlayerUuid, id: KingdomId) -> Result<(), KingdomError> {
        let id = self.realm_id(id);
        let index = self.active_index(id)?;
        if self.kingdoms[index].founder == actor {
            return Err(KingdomError::FounderMustDissolve);
        }
        if !self.kingdoms[index].members.contains(&actor) {
            return Err(KingdomError::NotMember);
        }
        self.can_change()?;
        self.kingdoms[index].members.retain(|id| *id != actor);
        self.roles.retain(|r| r.kingdom != id || r.member != actor);
        self.kingdoms[index]
            .member_names
            .retain(|(id, _)| *id != actor);
        self.sync_members(id);
        self.revision += 1;
        Ok(())
    }

    pub fn kick(
        &mut self,
        actor: PlayerUuid,
        id: KingdomId,
        member: PlayerUuid,
    ) -> Result<(), KingdomError> {
        self.active_index(id)?;
        if !self.can_set_role(actor, id, member, KingdomRole::Visitor) {
            return Err(KingdomError::PermissionDenied);
        }
        self.leave(member, id)
    }
}

pub fn valid_location(location: [i64; 3]) -> bool {
    location
        .iter()
        .all(|coordinate| coordinate.unsigned_abs() <= 990_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn membership_lifecycle_preserves_history_and_never_reuses_ids() {
        let founder = PlayerUuid::from_u128(1);
        let visitor = PlayerUuid::from_u128(2);
        let mut registry = KingdomRegistry::default();
        let id = registry
            .found(founder, [-17, 30, 9], "  Oak Crown  ")
            .unwrap();
        assert_eq!(registry.get(id).unwrap().name, "Oak Crown");
        registry.join(visitor, id).unwrap();
        assert_eq!(
            registry.rename(visitor, id, "Other"),
            Err(KingdomError::PermissionDenied)
        );
        assert_eq!(
            registry.dissolve(visitor, id),
            Err(KingdomError::FounderOnly)
        );
        assert_eq!(
            registry.leave(founder, id),
            Err(KingdomError::FounderMustDissolve)
        );
        registry.kick(founder, id, visitor).unwrap();
        registry.join(visitor, id).unwrap();
        registry.leave(visitor, id).unwrap();
        registry.rename(founder, id, "Birch Crown").unwrap();
        registry.dissolve(founder, id).unwrap();
        assert!(registry.active_for_member(founder).is_none());
        assert!(registry.active_at([-17, 30, 9]).is_none());
        let next = registry
            .found(founder, [-17, 30, 9], "Birch Crown")
            .unwrap();
        assert!(next.0 > id.0);
        assert_eq!(registry.get(id).unwrap().state, KingdomState::Dissolved);
        assert!(registry.valid());
    }

    #[test]
    fn invalid_and_concurrent_foundings_are_atomic() {
        let founder = PlayerUuid::from_u128(1);
        let visitor = PlayerUuid::from_u128(2);
        let mut registry = KingdomRegistry::default();
        registry.found(founder, [0, 20, 0], "First").unwrap();
        for (who, location, name, expected) in [
            (visitor, [1, 20, 0], "first", KingdomError::NameTaken),
            (
                visitor,
                [0, 20, 0],
                "Second",
                KingdomError::KeepAlreadyBound,
            ),
            (founder, [1, 20, 0], "Second", KingdomError::AlreadyMember),
            (visitor, [1, 20, 0], "\n", KingdomError::InvalidName),
            (
                visitor,
                [i64::MIN, 20, 0],
                "Second",
                KingdomError::InvalidLocation,
            ),
        ] {
            let before = registry.clone();
            assert_eq!(registry.found(who, location, name), Err(expected));
            assert_eq!(registry, before);
        }
        registry.revision = u64::MAX;
        let before = registry.clone();
        assert_eq!(
            registry.dissolve(founder, KingdomId(1)),
            Err(KingdomError::CapacityReached)
        );
        assert_eq!(registry, before);
    }

    #[test]
    fn snapshots_reject_duplicate_owners_members_locations_and_invalid_names() {
        let mut registry = KingdomRegistry::default();
        registry
            .found(PlayerUuid::from_u128(1), [0, 20, 0], "First")
            .unwrap();
        registry
            .found(PlayerUuid::from_u128(2), [16, 20, 0], "Second")
            .unwrap();
        assert!(registry.valid());
        let mut duplicate = registry.clone();
        duplicate.kingdoms[1].members.push(PlayerUuid::from_u128(1));
        duplicate.kingdoms[1].member_names.push((
            PlayerUuid::from_u128(1),
            registry.kingdoms[0].founder_name.clone(),
        ));
        assert!(!duplicate.valid());
        duplicate = registry.clone();
        duplicate.kingdoms[1].location = [0, 20, 0];
        assert!(!duplicate.valid());
        duplicate = registry.clone();
        duplicate.kingdoms[1].name = "FIRST".into();
        assert!(!duplicate.valid());
        duplicate = registry;
        duplicate.kingdoms[1].founder = PlayerUuid::nil();
        assert!(!duplicate.valid());
    }
}
