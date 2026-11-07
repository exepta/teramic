//! Reliable, authoritative kingdom lifecycle; chunk ownership is separate.
use serde::{Deserialize, Serialize};
use terra_core::{
    PlayerUuid,
    kingdom::{
        ClaimKind, DiplomaticRelation, DiplomaticTarget, KingdomError, KingdomHeraldry, KingdomId,
        KingdomRole, SettlementKind,
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KingdomAction {
    Inspect {
        position: [i64; 3],
    },
    Found {
        position: [i64; 3],
        name: String,
    },
    Rename {
        id: KingdomId,
        name: String,
    },
    Dissolve {
        id: KingdomId,
    },
    Join {
        id: KingdomId,
    },
    Leave {
        id: KingdomId,
    },
    Kick {
        id: KingdomId,
        member: PlayerUuid,
    },
    FoundSettlement {
        position: [i64; 3],
        name: String,
        kind: SettlementKind,
        heraldry: KingdomHeraldry,
    },
    ClaimChunk {
        id: KingdomId,
        chunk: [i64; 2],
        kind: ClaimKind,
    },
    ReleaseChunk {
        id: KingdomId,
        chunk: [i64; 2],
    },
    ChangeAppearance {
        id: KingdomId,
        heraldry: KingdomHeraldry,
    },
    ClaimBotChunk {
        id: KingdomId,
        chunk: [i64; 2],
        kind: ClaimKind,
        bot: u64,
    },
    Upgrade {
        id: KingdomId,
    },
    ReleaseBotChunk {
        id: KingdomId,
        chunk: [i64; 2],
        bot: u64,
    },
    ClaimBatch {
        id: KingdomId,
        claims: Vec<([i64; 2], ClaimKind)>,
    },
    SetRole {
        id: KingdomId,
        member: PlayerUuid,
        role: KingdomRole,
    },
    SetRelation {
        id: KingdomId,
        target: DiplomaticTarget,
        relation: DiplomaticRelation,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KingdomCommand {
    pub request: u64,
    pub action: KingdomAction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KingdomOutcome {
    Inspected {
        kingdom: Option<KingdomId>,
    },
    Founded {
        id: KingdomId,
    },
    Renamed {
        id: KingdomId,
    },
    Dissolved {
        id: KingdomId,
    },
    Joined {
        id: KingdomId,
    },
    Left {
        id: KingdomId,
    },
    Kicked {
        id: KingdomId,
        member: PlayerUuid,
    },
    Rejected(KingdomError),
    Claimed {
        id: KingdomId,
        chunk: [i64; 2],
        kind: ClaimKind,
    },
    Released {
        id: KingdomId,
        chunk: [i64; 2],
    },
    AppearanceChanged {
        id: KingdomId,
    },
    Upgraded {
        id: KingdomId,
        level: u16,
    },
    ClaimsApplied {
        id: KingdomId,
        count: u16,
    },
    RoleChanged {
        id: KingdomId,
        member: PlayerUuid,
        role: KingdomRole,
    },
    RelationChanged {
        id: KingdomId,
        target: DiplomaticTarget,
        relation: DiplomaticRelation,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KingdomResult {
    pub request: u64,
    pub outcome: KingdomOutcome,
}

impl KingdomResult {
    pub fn valid(&self) -> bool {
        self.request > 0
            && match self.outcome {
                KingdomOutcome::ClaimsApplied { id, count } => {
                    id.0 > 0
                        && usize::from(count)
                            <= terra_core::kingdom::KingdomRegistry::MAX_BATCH_CLAIMS
                }
                KingdomOutcome::Inspected { kingdom } => kingdom.is_none_or(|id| id.0 > 0),
                KingdomOutcome::Founded { id }
                | KingdomOutcome::Renamed { id }
                | KingdomOutcome::Dissolved { id }
                | KingdomOutcome::Joined { id }
                | KingdomOutcome::Left { id } => id.0 > 0,
                KingdomOutcome::RoleChanged { id, member, role } => {
                    id.0 > 0 && !member.is_nil() && role != KingdomRole::Founder
                }
                KingdomOutcome::RelationChanged {
                    id,
                    target,
                    relation,
                } => {
                    id.0 > 0
                        && relation != DiplomaticRelation::Own
                        && match target {
                            DiplomaticTarget::Player(other) => other.0 > 0 && other != id,
                            DiplomaticTarget::Bot(_) => true,
                        }
                }
                KingdomOutcome::Kicked { id, member } => id.0 > 0 && !member.is_nil(),
                KingdomOutcome::Rejected(_) => true,
                KingdomOutcome::Claimed { id, chunk, .. }
                | KingdomOutcome::Released { id, chunk } => {
                    id.0 > 0 && terra_core::kingdom::valid_claim_chunk(chunk)
                }
                KingdomOutcome::AppearanceChanged { id } => id.0 > 0,
                KingdomOutcome::Upgraded { id, level } => {
                    id.0 > 0 && (2..=terra_core::kingdom::MAX_SETTLEMENT_LEVEL).contains(&level)
                }
            }
    }
}
