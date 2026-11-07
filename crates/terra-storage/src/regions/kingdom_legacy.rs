//! Frozen postcard layouts: appended parent fields must never consume the next
//! record's bytes when decoding historical IDX9/ID10/ID11 saves.
use super::{KingdomIndex, TimedIndex};
use serde::{Deserialize, Serialize};
use terra_core::{PlayerUuid, kingdom::*};

#[derive(Serialize, Deserialize)]
enum SettlementKind9 {
    Kingdom,
    Village,
    City,
}
impl From<SettlementKind9> for SettlementKind {
    fn from(old: SettlementKind9) -> Self {
        match old {
            SettlementKind9::Kingdom => Self::Kingdom,
            SettlementKind9::Village => Self::Village,
            SettlementKind9::City => Self::City,
        }
    }
}
#[derive(Serialize, Deserialize)]
pub(super) struct KingdomRecord9 {
    id: KingdomId,
    name: String,
    founder: PlayerUuid,
    founder_name: String,
    members: Vec<PlayerUuid>,
    member_names: Vec<(PlayerUuid, String)>,
    location: [i64; 3],
    state: KingdomState,
    kind: SettlementKind9,
    level: u16,
    heraldry: KingdomHeraldry,
}
impl From<KingdomRecord9> for KingdomRecord {
    fn from(old: KingdomRecord9) -> Self {
        Self {
            id: old.id,
            name: old.name,
            founder: old.founder,
            founder_name: old.founder_name,
            members: old.members,
            member_names: old.member_names,
            location: old.location,
            state: old.state,
            kind: old.kind.into(),
            level: old.level,
            heraldry: old.heraldry,
            parent: None,
        }
    }
}
#[derive(Serialize, Deserialize)]
struct KingdomRegistry10 {
    next_id: u64,
    revision: u64,
    kingdoms: Vec<KingdomRecord9>,
    claims: Vec<KingdomClaim>,
    roles: Vec<MemberRole>,
    relations: Vec<RelationOffer>,
}
impl From<KingdomRegistry10> for KingdomRegistry {
    fn from(old: KingdomRegistry10) -> Self {
        Self {
            next_id: old.next_id,
            revision: old.revision,
            kingdoms: old.kingdoms.into_iter().map(Into::into).collect(),
            claims: old.claims,
            roles: old.roles,
            relations: old.relations,
        }
    }
}
#[derive(Serialize, Deserialize)]
pub(super) struct KingdomIndex10 {
    prior: TimedIndex,
    kingdoms: KingdomRegistry10,
}
impl From<KingdomIndex10> for KingdomIndex {
    fn from(old: KingdomIndex10) -> Self {
        Self {
            prior: old.prior,
            kingdoms: old.kingdoms.into(),
        }
    }
}
#[derive(Serialize, Deserialize)]
pub(super) struct TreeIndex11 {
    pub(super) prior: KingdomIndex10,
    pub(super) trees: terra_core::TreeRegistry,
}

// Test fixtures can write historical files only when their data is expressible
// by the frozen format; production never writes a new record under an old tag.
#[cfg(test)]
impl From<KingdomRecord> for KingdomRecord9 {
    fn from(record: KingdomRecord) -> Self {
        assert!(record.parent.is_none());
        let kind = match record.kind {
            SettlementKind::Kingdom => SettlementKind9::Kingdom,
            SettlementKind::Village => SettlementKind9::Village,
            SettlementKind::City => SettlementKind9::City,
            SettlementKind::Outpost => panic!("Outposts cannot be written under a legacy tag"),
        };
        Self {
            id: record.id,
            name: record.name,
            founder: record.founder,
            founder_name: record.founder_name,
            members: record.members,
            member_names: record.member_names,
            location: record.location,
            state: record.state,
            kind,
            level: record.level,
            heraldry: record.heraldry,
        }
    }
}
#[cfg(test)]
impl From<KingdomIndex> for KingdomIndex10 {
    fn from(current: KingdomIndex) -> Self {
        let registry = current.kingdoms;
        Self {
            prior: current.prior,
            kingdoms: KingdomRegistry10 {
                next_id: registry.next_id,
                revision: registry.revision,
                kingdoms: registry.kingdoms.into_iter().map(Into::into).collect(),
                claims: registry.claims,
                roles: registry.roles,
                relations: registry.relations,
            },
        }
    }
}
