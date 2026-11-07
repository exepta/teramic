//! Authoritative realm capabilities and consent-based diplomatic offers.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KingdomRole {
    Founder,
    Officer,
    Builder,
    Worker,
    Visitor,
    /// Full ordinary membership; management remains Founder/Officer only.
    Member,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KingdomPermission {
    Build,
    Break,
    Inventories,
    NpcWork,
    /// Operating doors/windows; player-owned land retains inventory access rules.
    Openings,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct KingdomPermissions {
    pub build: bool,
    pub break_blocks: bool,
    pub inventories: bool,
    pub npc_work: bool,
}
impl KingdomPermissions {
    pub const fn allows(self, permission: KingdomPermission) -> bool {
        match permission {
            KingdomPermission::Build => self.build,
            KingdomPermission::Break => self.break_blocks,
            KingdomPermission::Inventories | KingdomPermission::Openings => self.inventories,
            KingdomPermission::NpcWork => self.npc_work,
        }
    }
}
impl KingdomRole {
    pub const ALL: [Self; 6] = [
        Self::Founder,
        Self::Officer,
        Self::Member,
        Self::Builder,
        Self::Worker,
        Self::Visitor,
    ];
    pub const fn label(self) -> &'static str {
        match self {
            Self::Founder => "Founder",
            Self::Officer => "Officer",
            Self::Builder => "Builder",
            Self::Worker => "Worker",
            Self::Visitor => "Visitor",
            Self::Member => "Member",
        }
    }
    pub const fn key(self) -> &'static str {
        match self {
            Self::Founder => "founder",
            Self::Officer => "officer",
            Self::Builder => "builder",
            Self::Worker => "worker",
            Self::Visitor => "visitor",
            Self::Member => "member",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.key() == key)
    }
    pub const fn permissions(self) -> KingdomPermissions {
        match self {
            Self::Founder | Self::Officer | Self::Member => KingdomPermissions {
                build: true,
                break_blocks: true,
                inventories: true,
                npc_work: true,
            },
            Self::Builder => KingdomPermissions {
                build: true,
                break_blocks: true,
                inventories: true,
                npc_work: false,
            },
            Self::Worker => KingdomPermissions {
                build: false,
                break_blocks: false,
                inventories: true,
                npc_work: true,
            },
            Self::Visitor => KingdomPermissions {
                build: false,
                break_blocks: false,
                inventories: false,
                npc_work: false,
            },
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberRole {
    pub kingdom: KingdomId,
    pub member: PlayerUuid,
    pub role: KingdomRole,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DiplomaticTarget {
    Player(KingdomId),
    Bot(BotKingdom),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiplomaticRelation {
    Own,
    Neutral,
    Allied,
    Hostile,
}
impl DiplomaticRelation {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Own => "Your realm",
            Self::Neutral => "Neutral",
            Self::Allied => "Allied",
            Self::Hostile => "Hostile",
        }
    }
    pub const fn key(self) -> &'static str {
        match self {
            Self::Own => "own",
            Self::Neutral => "neutral",
            Self::Allied => "allied",
            Self::Hostile => "hostile",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        [Self::Own, Self::Neutral, Self::Allied, Self::Hostile]
            .into_iter()
            .find(|r| r.key() == key)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationOffer {
    pub from: KingdomId,
    pub target: DiplomaticTarget,
    pub relation: DiplomaticRelation,
}
impl KingdomRegistry {
    pub fn role_for(&self, id: KingdomId, player: PlayerUuid) -> Option<KingdomRole> {
        let id = self.realm_id(id);
        let realm = self
            .get(id)
            .filter(|r| r.state == KingdomState::Active && r.members.contains(&player))?;
        if realm.founder == player {
            Some(KingdomRole::Founder)
        } else {
            self.roles
                .iter()
                .find(|r| r.kingdom == id && r.member == player)
                .map(|r| r.role)
        }
    }
    pub fn can_manage(&self, actor: PlayerUuid, id: KingdomId) -> bool {
        matches!(
            self.role_for(id, actor),
            Some(KingdomRole::Founder | KingdomRole::Officer)
        )
    }
    pub fn can_set_role(
        &self,
        actor: PlayerUuid,
        id: KingdomId,
        target: PlayerUuid,
        role: KingdomRole,
    ) -> bool {
        if role == KingdomRole::Founder
            || self
                .role_for(id, target)
                .is_none_or(|r| r == KingdomRole::Founder)
        {
            return false;
        }
        match self.role_for(id, actor) {
            Some(KingdomRole::Founder) => true,
            Some(KingdomRole::Officer) => {
                actor != target
                    && role != KingdomRole::Officer
                    && self.role_for(id, target) != Some(KingdomRole::Officer)
            }
            _ => false,
        }
    }
    pub fn set_role(
        &mut self,
        actor: PlayerUuid,
        id: KingdomId,
        member: PlayerUuid,
        role: KingdomRole,
    ) -> Result<(), KingdomError> {
        let id = self.realm_id(id);
        self.active_index(id)?;
        if !self.can_set_role(actor, id, member, role) {
            return Err(KingdomError::PermissionDenied);
        }
        let entry = self
            .roles
            .iter()
            .position(|r| r.kingdom == id && r.member == member)
            .ok_or(KingdomError::NotMember)?;
        if self.roles[entry].role != role {
            self.can_change()?;
            self.roles[entry].role = role;
            self.revision += 1;
        }
        Ok(())
    }
    pub fn relation_offer(&self, from: KingdomId, target: DiplomaticTarget) -> DiplomaticRelation {
        let from = self.realm_id(from);
        let target = match target {
            DiplomaticTarget::Player(id) => DiplomaticTarget::Player(self.realm_id(id)),
            other => other,
        };
        self.relations
            .iter()
            .find(|r| r.from == from && r.target == target)
            .map_or(DiplomaticRelation::Neutral, |r| r.relation)
    }
    pub fn relation(&self, from: KingdomId, to: KingdomId) -> DiplomaticRelation {
        self.relation_to(from, DiplomaticTarget::Player(to))
    }
    pub fn relation_to(&self, from: KingdomId, target: DiplomaticTarget) -> DiplomaticRelation {
        let from = self.realm_id(from);
        let target = match target {
            DiplomaticTarget::Player(id) => DiplomaticTarget::Player(self.realm_id(id)),
            other => other,
        };
        if self
            .get(from)
            .is_none_or(|r| r.state != KingdomState::Active)
        {
            return DiplomaticRelation::Neutral;
        }
        let ours = self.relation_offer(from, target);
        match target {
            DiplomaticTarget::Player(to) => {
                if from == to {
                    return DiplomaticRelation::Own;
                }
                if self.get(to).is_none_or(|r| r.state != KingdomState::Active) {
                    return DiplomaticRelation::Neutral;
                }
                let theirs = self.relation_offer(to, DiplomaticTarget::Player(from));
                if ours == DiplomaticRelation::Hostile || theirs == DiplomaticRelation::Hostile {
                    DiplomaticRelation::Hostile
                } else if ours == DiplomaticRelation::Allied && theirs == DiplomaticRelation::Allied
                {
                    DiplomaticRelation::Allied
                } else {
                    DiplomaticRelation::Neutral
                }
            }
            DiplomaticTarget::Bot(bot) => {
                if ours == DiplomaticRelation::Hostile || bot.relation() == KingdomRelation::Hostile
                {
                    DiplomaticRelation::Hostile
                } else if ours == DiplomaticRelation::Allied
                    && (bot.relation() == KingdomRelation::Friendly
                        || self.get(from).is_some_and(|r| r.level >= 2))
                {
                    DiplomaticRelation::Allied
                } else {
                    DiplomaticRelation::Neutral
                }
            }
        }
    }
    pub fn set_relation(
        &mut self,
        actor: PlayerUuid,
        id: KingdomId,
        target: DiplomaticTarget,
        relation: DiplomaticRelation,
    ) -> Result<(), KingdomError> {
        let id = self.realm_id(id);
        let target = match target {
            DiplomaticTarget::Player(id) => DiplomaticTarget::Player(self.realm_id(id)),
            other => other,
        };
        self.active_index(id)?;
        if !self.can_manage(actor, id) {
            return Err(KingdomError::PermissionDenied);
        }
        if relation == DiplomaticRelation::Own || target == DiplomaticTarget::Player(id) {
            return Err(KingdomError::InvalidRelation);
        }
        if let DiplomaticTarget::Player(other) = target {
            self.active_index(other)?;
        }
        if matches!(target, DiplomaticTarget::Bot(BotKingdom::Petora))
            && relation == DiplomaticRelation::Allied
        {
            return Err(KingdomError::InvalidRelation);
        }
        if self.relation_offer(id, target) == relation {
            return Ok(());
        }
        self.can_change()?;
        self.relations
            .retain(|r| !(r.from == id && r.target == target));
        if relation != DiplomaticRelation::Neutral {
            self.relations.push(RelationOffer {
                from: id,
                target,
                relation,
            });
        }
        self.revision += 1;
        Ok(())
    }
    fn realm_allows(
        &self,
        actor: PlayerUuid,
        id: KingdomId,
        permission: KingdomPermission,
    ) -> bool {
        if let Some(role) = self.role_for(id, actor) {
            return role.permissions().allows(permission);
        }
        matches!(
            permission,
            KingdomPermission::Inventories
                | KingdomPermission::NpcWork
                | KingdomPermission::Openings
        ) && self.active_for_member(actor).is_some_and(|own| {
            self.role_for(own.id, actor)
                .is_some_and(|role| role.permissions().allows(permission))
                && self.relation(own.id, id) == DiplomaticRelation::Allied
        })
    }
    pub fn can(
        &self,
        actor: PlayerUuid,
        permission: KingdomPermission,
        position: [i64; 3],
    ) -> bool {
        self.claim_at(claim_chunk(position)).is_none_or(|claim| {
            (claim.kind == ClaimKind::RealmTerritory
                && matches!(
                    permission,
                    KingdomPermission::Build | KingdomPermission::Break
                ))
                || self.realm_allows(actor, claim.kingdom, permission)
        })
    }
    pub fn can_in_domain(
        &self,
        actor: PlayerUuid,
        permission: KingdomPermission,
        position: [i64; 3],
        domain: &SettlementDomain,
    ) -> bool {
        if self.claim_at(claim_chunk(position)).is_some() {
            return self.can(actor, permission, position);
        }
        if let Some(owner) = self.domain_owner(domain, position) {
            return self.realm_allows(actor, owner.id, permission);
        }
        let Some(bot) = domain.native else {
            return true;
        };
        match self.bot_access_relation(actor, bot) {
            KingdomRelation::Friendly => true,
            KingdomRelation::Neutral => permission == KingdomPermission::Openings,
            KingdomRelation::Hostile => false,
        }
    }
    /// Native attitude applies even to visitors without their own kingdom.
    /// A live hostile declaration overrides friendliness; an accepted alliance
    /// makes a neutral NPC realm friendly. Captured/player claims use roles above.
    pub fn bot_access_relation(&self, actor: PlayerUuid, bot: BotKingdom) -> KingdomRelation {
        let relation = self
            .active_for_member(actor)
            .map(|own| self.relation_to(own.id, DiplomaticTarget::Bot(bot)));
        match relation {
            Some(DiplomaticRelation::Hostile) => KingdomRelation::Hostile,
            Some(DiplomaticRelation::Allied | DiplomaticRelation::Own) => KingdomRelation::Friendly,
            _ => bot.relation(),
        }
    }
    /// Item/fluid movers have no player actor: permit shared storage inside one
    /// realm or a mutually allied pair, deny borders to unclaimed/foreign land.
    pub fn can_transport(&self, from: [i64; 3], to: [i64; 3]) -> bool {
        self.can_transport_between(
            self.region_at(from).map(|r| DiplomaticTarget::Player(r.id)),
            self.region_at(to).map(|r| DiplomaticTarget::Player(r.id)),
        )
    }
    pub fn can_transport_between(
        &self,
        from: Option<DiplomaticTarget>,
        to: Option<DiplomaticTarget>,
    ) -> bool {
        match (from, to) {
            (None, None) => true,
            (Some(a), Some(b)) if a == b => true,
            (Some(DiplomaticTarget::Player(a)), Some(b))
            | (Some(b), Some(DiplomaticTarget::Player(a))) => {
                matches!(
                    self.relation_to(a, b),
                    DiplomaticRelation::Own | DiplomaticRelation::Allied
                )
            }
            _ => false,
        }
    }
    pub(super) fn valid_permissions(&self) -> bool {
        if self.roles.len() > MAX_KINGDOMS * MAX_KINGDOM_MEMBERS
            || self.relations.len() > MAX_KINGDOMS * (MAX_KINGDOMS + 4)
        {
            return false;
        }
        let mut roles = std::collections::BTreeSet::new();
        let mut offers = std::collections::BTreeSet::new();
        self.roles.iter().all(|r| {
            r.role != KingdomRole::Founder
                && roles.insert((r.kingdom, r.member))
                && self.get(r.kingdom).is_some_and(|k| {
                    k.state == KingdomState::Active
                        && k.parent.is_none()
                        && k.founder != r.member
                        && k.members.contains(&r.member)
                })
        }) && self
            .kingdoms
            .iter()
            .filter(|k| k.state == KingdomState::Active && k.parent.is_none())
            .all(|k| {
                k.members.iter().all(|permission| {
                    *permission == k.founder || roles.contains(&(k.id, *permission))
                })
            })
            && self.relations.iter().all(|r| {
                !matches!(
                    r.relation,
                    DiplomaticRelation::Own | DiplomaticRelation::Neutral
                ) && offers.insert((r.from, r.target))
                    && self
                        .get(r.from)
                        .is_some_and(|k| k.state == KingdomState::Active)
                    && match r.target {
                        DiplomaticTarget::Player(other) => {
                            other != r.from
                                && self
                                    .get(other)
                                    .is_some_and(|k| k.state == KingdomState::Active)
                        }
                        DiplomaticTarget::Bot(_) => true,
                    }
            })
    }
    /// Frozen pre-F57 members retain all ordinary access, including owned NPC commands.
    pub fn migrate_legacy_roles(&mut self) {
        self.roles.clear();
        self.relations.clear();
        for k in self
            .kingdoms
            .iter()
            .filter(|k| k.state == KingdomState::Active)
        {
            for &member in &k.members {
                if member != k.founder {
                    self.roles.push(MemberRole {
                        kingdom: k.id,
                        member,
                        role: KingdomRole::Member,
                    });
                }
            }
        }
    }
}
