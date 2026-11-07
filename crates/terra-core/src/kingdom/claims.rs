use super::*;
use std::collections::{BTreeMap, VecDeque};

/// Matches the block world's 16×16 streaming columns, independent of height.
pub const CLAIM_CHUNK_SIZE: i64 = 16;
/// Bounds decoded snapshots and prevents unbounded full-registry broadcasts.
pub const MAX_CLAIMS: usize = 65_536;
pub const MAX_SETTLEMENT_LEVEL: u16 = 64;

pub(super) fn initial_level() -> u16 {
    1
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementKind {
    #[default]
    Kingdom,
    Village,
    City,
    Outpost,
}
impl SettlementKind {
    pub const ALL: [Self; 4] = [Self::Kingdom, Self::Village, Self::City, Self::Outpost];
    pub const fn key(self) -> &'static str {
        match self {
            Self::Kingdom => "kingdom",
            Self::Village => "village",
            Self::City => "city",
            Self::Outpost => "outpost",
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Kingdom => "Kingdom",
            Self::Village => "Village",
            Self::City => "City",
            Self::Outpost => "Outpost",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key)
    }
}

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum KingdomIcon {
    #[default]
    Castle,
    Crown,
    Tree,
    Mountain,
    Wheat,
    Anchor,
    Hammer,
    Star,
    /// Stable catalog-name hash, appended to preserve the eight legacy save tags.
    Catalog(u64),
}
#[path = "icon_catalog.rs"]
mod icon_catalog;

impl KingdomIcon {
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Castle => "landmark",
            _ => self.key(),
        }
    }
    pub const ALL: [Self; 8] = [
        Self::Castle,
        Self::Crown,
        Self::Tree,
        Self::Mountain,
        Self::Wheat,
        Self::Anchor,
        Self::Hammer,
        Self::Star,
    ];
    pub const fn key(self) -> &'static str {
        match self {
            Self::Castle => "castle",
            Self::Crown => "crown",
            Self::Tree => "tree",
            Self::Mountain => "mountain",
            Self::Wheat => "wheat",
            Self::Anchor => "anchor",
            Self::Hammer => "hammer",
            Self::Star => "star",
            Self::Catalog(id) => match Self::catalog_glyph(id) {
                Some(glyph) => glyph.name,
                None => "",
            },
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Castle => "Castle",
            Self::Crown => "Crown",
            Self::Tree => "Tree",
            Self::Mountain => "Mountain",
            Self::Wheat => "Wheat",
            Self::Anchor => "Anchor",
            Self::Hammer => "Hammer",
            Self::Star => "Star",
            Self::Catalog(_) => self.key(),
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|icon| icon.key() == key)
            .or_else(|| {
                icon_catalog::GLYPHS
                    .iter()
                    .find(|glyph| glyph.name == key)
                    .map(|glyph| Self::Catalog(glyph.id))
            })
    }
    const fn catalog_glyph(id: u64) -> Option<&'static icon_catalog::Glyph> {
        let mut index = 0;
        while index < icon_catalog::GLYPHS.len() {
            let glyph = &icon_catalog::GLYPHS[index];
            if glyph.id == id {
                return Some(glyph);
            }
            index += 1;
        }
        None
    }
    pub const fn valid(self) -> bool {
        match self {
            Self::Catalog(id) => Self::catalog_glyph(id).is_some(),
            _ => true,
        }
    }
    /// Complete named catalog, retaining the original eight serialized emblems.
    pub fn catalog() -> impl Iterator<Item = Self> {
        Self::ALL.into_iter().chain(
            icon_catalog::GLYPHS
                .iter()
                .filter(|glyph| !Self::ALL.iter().any(|icon| icon.glyph() == glyph.name))
                .map(|glyph| Self::Catalog(glyph.id)),
        )
    }
    /// Shared, UI-independent glyph for cloth, capes and world banners.
    /// Row zero is the top; bit x is the opaque pixel at horizontal coordinate x.
    pub fn glyph_mask(self) -> Option<&'static [u32; 32]> {
        let glyph = match self {
            Self::Catalog(id) => Self::catalog_glyph(id),
            _ => icon_catalog::GLYPHS
                .iter()
                .find(|glyph| glyph.name == self.glyph()),
        }?;
        Some(&glyph.mask)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct KingdomHeraldry {
    pub color: [u8; 3],
    pub icon: KingdomIcon,
}
impl Default for KingdomHeraldry {
    fn default() -> Self {
        Self {
            color: [184, 137, 73],
            icon: KingdomIcon::Castle,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClaimKind {
    ProtectedBuilding,
    RealmTerritory,
}
impl ClaimKind {
    pub const ALL: [Self; 2] = [Self::ProtectedBuilding, Self::RealmTerritory];
    pub const fn key(self) -> &'static str {
        match self {
            Self::ProtectedBuilding => "protected_building",
            Self::RealmTerritory => "realm_territory",
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProtectedBuilding => "Protected Building",
            Self::RealmTerritory => "Realm Territory",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimActor {
    Player(PlayerUuid),
    Bot { id: u64, authorized_by: PlayerUuid },
}
impl ClaimActor {
    pub fn authorizer(self) -> PlayerUuid {
        match self {
            Self::Player(id) => id,
            Self::Bot { authorized_by, .. } => authorized_by,
        }
    }
    pub(super) fn valid(self) -> bool {
        !self.authorizer().is_nil() && !matches!(self, Self::Bot { id: 0, .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KingdomClaim {
    pub kingdom: KingdomId,
    pub chunk: [i64; 2],
    pub kind: ClaimKind,
    pub actor: ClaimActor,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimCounts {
    pub protected: u32,
    pub territory: u32,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimLimits {
    pub protected: u32,
    pub territory: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KingdomUpgradeCost {
    pub stone_wall: u16,
    pub gold_ingot: u16,
}
impl KingdomRecord {
    pub fn upgrade_cost(&self) -> Option<KingdomUpgradeCost> {
        (self.level < MAX_SETTLEMENT_LEVEL).then_some(KingdomUpgradeCost {
            stone_wall: self.level * 8,
            gold_ingot: self.level,
        })
    }
    pub fn limits(&self) -> ClaimLimits {
        let growth = u32::from(self.level.saturating_sub(1));
        ClaimLimits {
            protected: 32 + growth * 6,
            territory: 64 + growth * 16,
        }
    }
}
pub fn claim_chunk(position: [i64; 3]) -> [i64; 2] {
    [
        position[0].div_euclid(CLAIM_CHUNK_SIZE),
        position[2].div_euclid(CLAIM_CHUNK_SIZE),
    ]
}
pub fn valid_claim_chunk(chunk: [i64; 2]) -> bool {
    chunk.into_iter().all(|v| (-61_875..=61_875).contains(&v))
}

fn reachable_claims(anchor: [i64; 2], positions: &BTreeSet<[i64; 2]>) -> BTreeSet<[i64; 2]> {
    let mut reached = BTreeSet::from([anchor]);
    let mut queue = VecDeque::from([anchor]);
    while let Some([x, z]) = queue.pop_front() {
        for next in [[x - 1, z], [x + 1, z], [x, z - 1], [x, z + 1]] {
            if positions.contains(&next) && reached.insert(next) {
                queue.push_back(next);
            }
        }
    }
    reached
}

/// Preserve both the settlement's land graph and its Build-only route to the keep.
/// Historical islands may remain or shrink, but new cells cannot extend them and
/// cells previously connected to the keep cannot lose that connection.
pub(super) fn claim_connections_preserved<'a>(
    kingdom: KingdomId,
    seat: [i64; 2],
    before: &[KingdomClaim],
    after: impl Iterator<Item = &'a KingdomClaim>,
) -> bool {
    let positions = |claims: Vec<&KingdomClaim>| {
        let land = claims
            .iter()
            .map(|claim| claim.chunk)
            .collect::<BTreeSet<_>>();
        let build = claims
            .iter()
            .filter(|claim| claim.kind == ClaimKind::ProtectedBuilding)
            .map(|claim| claim.chunk)
            .collect::<BTreeSet<_>>();
        (land, build)
    };
    let (previous_land, previous_build) = positions(
        before
            .iter()
            .filter(|claim| claim.kingdom == kingdom)
            .collect(),
    );
    let (next_land, next_build) =
        positions(after.filter(|claim| claim.kingdom == kingdom).collect());
    let preserved = |previous: &BTreeSet<_>, next: &BTreeSet<_>| {
        let next_reached = reachable_claims(seat, next);
        if next.is_subset(&next_reached) {
            return true;
        }
        let previous_reached = reachable_claims(seat, previous);
        next.difference(&next_reached)
            .all(|chunk| previous.contains(chunk) && !previous_reached.contains(chunk))
    };
    preserved(&previous_land, &next_land) && preserved(&previous_build, &next_build)
}

impl KingdomRegistry {
    pub fn chunk_at(position: [i64; 3]) -> [i64; 2] {
        claim_chunk(position)
    }
    pub fn claim_at(&self, chunk: [i64; 2]) -> Option<&KingdomClaim> {
        self.claims
            .iter()
            .filter(|claim| claim.chunk == chunk)
            .min_by_key(|claim| claim.kind != ClaimKind::ProtectedBuilding)
    }
    pub fn region_at(&self, position: [i64; 3]) -> Option<&KingdomRecord> {
        let claim = self.claim_at(claim_chunk(position))?;
        self.get(claim.kingdom)
            .filter(|k| k.state == KingdomState::Active)
    }
    pub fn can_build(&self, player: PlayerUuid, position: [i64; 3]) -> bool {
        self.can(player, KingdomPermission::Build, position)
    }

    pub fn claim_counts(&self, id: KingdomId) -> ClaimCounts {
        let mut result = ClaimCounts::default();
        for claim in self.claims.iter().filter(|c| c.kingdom == id) {
            match claim.kind {
                ClaimKind::ProtectedBuilding => result.protected += 1,
                ClaimKind::RealmTerritory => result.territory += 1,
            }
        }
        result
    }
    pub fn heraldry_for_member(&self, player: PlayerUuid) -> Option<KingdomHeraldry> {
        self.active_for_member(player).map(|k| k.heraldry)
    }
    pub fn heraldry_at(&self, position: [i64; 3]) -> Option<KingdomHeraldry> {
        self.region_at(position)
            .or_else(|| self.active_at(position))
            .map(|k| k.heraldry)
    }
    pub fn found_settlement(
        &mut self,
        founder: PlayerUuid,
        founder_name: &str,
        position: [i64; 3],
        name: &str,
        kind: SettlementKind,
        heraldry: KingdomHeraldry,
    ) -> Result<KingdomId, KingdomError> {
        if !heraldry.icon.valid() {
            return Err(KingdomError::InvalidHeraldry);
        }
        let id = self.found_with_name(founder, founder_name, position, name)?;
        let record = self.kingdoms.last_mut().expect("newly founded settlement");
        record.kind = kind;
        record.heraldry = heraldry;
        Ok(id)
    }
    pub fn change_appearance(
        &mut self,
        actor: PlayerUuid,
        id: KingdomId,
        heraldry: KingdomHeraldry,
    ) -> Result<(), KingdomError> {
        if !heraldry.icon.valid() {
            return Err(KingdomError::InvalidHeraldry);
        }
        let id = self.realm_id(id);
        let index = self.active_index(id)?;
        if !self.can_manage(actor, id) {
            return Err(KingdomError::PermissionDenied);
        }
        if self.kingdoms[index].heraldry != heraldry {
            self.can_change()?;
            self.kingdoms[index].heraldry = heraldry;
            for child in self
                .kingdoms
                .iter_mut()
                .filter(|child| child.parent == Some(id) && child.state == KingdomState::Active)
            {
                child.heraldry = heraldry;
            }
            self.revision += 1;
        }
        Ok(())
    }
    pub fn claim(
        &mut self,
        id: KingdomId,
        chunk: [i64; 2],
        kind: ClaimKind,
        actor: ClaimActor,
    ) -> Result<(), KingdomError> {
        let index = self.active_index(id)?;
        if !actor.valid() {
            return Err(KingdomError::InvalidIdentity);
        }
        if !self.kingdoms[index].members.contains(&actor.authorizer()) {
            return Err(KingdomError::NotMember);
        }
        if !self.can_manage(actor.authorizer(), id) {
            return Err(KingdomError::PermissionDenied);
        }
        if !valid_claim_chunk(chunk) {
            return Err(KingdomError::InvalidClaim);
        }
        if chunk == claim_chunk(self.kingdoms[index].location)
            && kind != ClaimKind::ProtectedBuilding
        {
            return Err(KingdomError::SeatMustStayClaimed);
        }
        let existing = self.claims.iter().position(|c| c.chunk == chunk);
        if let Some(existing) = existing {
            if self.claims[existing].kingdom != id {
                return Err(KingdomError::ClaimOccupied);
            }
            if self.claims[existing].kind == kind {
                return Ok(());
            }
        }
        if self.kingdoms.iter().any(|k| {
            k.id != id && k.state == KingdomState::Active && claim_chunk(k.location) == chunk
        }) {
            return Err(KingdomError::ClaimOccupied);
        }
        if existing.is_none() && self.claims.len() >= MAX_CLAIMS {
            return Err(KingdomError::CapacityReached);
        }
        let counts = self.claim_counts(id);
        let limits = self.kingdoms[index].limits();
        if match kind {
            ClaimKind::ProtectedBuilding => counts.protected >= limits.protected,
            ClaimKind::RealmTerritory => counts.territory >= limits.territory,
        } {
            return Err(KingdomError::ClaimLimitReached);
        }
        let claim = KingdomClaim {
            kingdom: id,
            chunk,
            kind,
            actor,
        };
        if !claim_connections_preserved(
            id,
            claim_chunk(self.kingdoms[index].location),
            &self.claims,
            self.claims
                .iter()
                .filter(|existing| existing.kingdom != id || existing.chunk != chunk)
                .chain(std::iter::once(&claim)),
        ) {
            return Err(KingdomError::ClaimDisconnected);
        }
        self.can_change()?;
        if let Some(existing) = existing {
            self.claims[existing] = claim;
        } else {
            self.claims.push(claim);
        }
        self.revision += 1;
        Ok(())
    }
    pub fn release(
        &mut self,
        actor: PlayerUuid,
        id: KingdomId,
        chunk: [i64; 2],
    ) -> Result<(), KingdomError> {
        let index = self.active_index(id)?;
        if !self.kingdoms[index].members.contains(&actor) {
            return Err(KingdomError::NotMember);
        }
        if !self.can_manage(actor, id) {
            return Err(KingdomError::PermissionDenied);
        }
        if chunk == claim_chunk(self.kingdoms[index].location) {
            return Err(KingdomError::SeatMustStayClaimed);
        }
        let claim_index = self
            .claims
            .iter()
            .position(|c| c.kingdom == id && c.chunk == chunk)
            .ok_or(KingdomError::ClaimMissing)?;
        if !claim_connections_preserved(
            id,
            claim_chunk(self.kingdoms[index].location),
            &self.claims,
            self.claims
                .iter()
                .enumerate()
                .filter_map(|(index, claim)| (index != claim_index).then_some(claim)),
        ) {
            return Err(KingdomError::ClaimDisconnected);
        }
        self.can_change()?;
        self.claims.remove(claim_index);
        self.revision += 1;
        Ok(())
    }
    pub(super) fn valid_claims(&self) -> bool {
        if self.claims.len() > MAX_CLAIMS {
            return false;
        }
        let mut seen: BTreeMap<[i64; 2], Vec<&KingdomClaim>> = BTreeMap::new();
        let mut owners = BTreeSet::new();
        let mut seats = BTreeSet::new();
        for claim in &self.claims {
            if !valid_claim_chunk(claim.chunk)
                || !claim.actor.valid()
                || self
                    .get(claim.kingdom)
                    .is_none_or(|record| record.state != KingdomState::Active)
            {
                return false;
            }
            if seen.get(&claim.chunk).is_some_and(|others| {
                others
                    .iter()
                    .any(|other| !self.compatible_overlap(claim, other))
            }) {
                return false;
            }
            owners.insert(claim.kingdom);
            if self
                .get(claim.kingdom)
                .is_some_and(|record| claim_chunk(record.location) == claim.chunk)
            {
                if claim.kind != ClaimKind::ProtectedBuilding {
                    return false;
                }
                seats.insert(claim.kingdom);
            }
            seen.entry(claim.chunk).or_default().push(claim);
        }
        // Pre-claim saves can have no land. Once land exists, its keep remains
        // protected even when live policy permits disconnected expansion.
        owners.is_subset(&seats)
    }
}

impl crate::GameplayState {
    /// Atomically commits the real material payment, inventory revision and level.
    /// There is deliberately no network-facing arbitrary level setter.
    pub fn upgrade_kingdom(
        &mut self,
        actor: PlayerUuid,
        id: KingdomId,
    ) -> Result<u16, KingdomError> {
        let index = self.kingdoms.active_index(id)?;
        let record = &self.kingdoms.kingdoms[index];
        if !self.kingdoms.can_manage(actor, id) {
            return Err(KingdomError::PermissionDenied);
        }
        let cost = record
            .upgrade_cost()
            .ok_or(KingdomError::UpgradeLimitReached)?;
        self.kingdoms.can_change()?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(KingdomError::CapacityReached)?;
        let mut inventory = self.inventories.get(&actor).cloned().unwrap_or_default();
        let inventory_revision = inventory
            .revision
            .checked_add(1)
            .ok_or(KingdomError::CapacityReached)?;
        let stone = crate::ItemKind::Block(crate::castle::STONE_WALL);
        let gold = crate::ItemKind::Mineral(crate::MineralItem::GoldIngot);
        if inventory.count(stone) < cost.stone_wall || inventory.count(gold) < cost.gold_ingot {
            return Err(KingdomError::UpgradeMaterialsMissing);
        }
        for _ in 0..cost.stone_wall {
            if !inventory.take_one(stone) {
                return Err(KingdomError::UpgradeMaterialsMissing);
            }
        }
        for _ in 0..cost.gold_ingot {
            if !inventory.take_one(gold) {
                return Err(KingdomError::UpgradeMaterialsMissing);
            }
        }
        inventory.revision = inventory_revision;
        let level = record.level + 1;
        self.inventories.insert(actor, inventory);
        self.kingdoms.kingdoms[index].level = level;
        self.kingdoms.revision += 1;
        self.revision = revision;
        Ok(level)
    }
}

#[cfg(test)]
#[path = "claim_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "icon_tests.rs"]
mod icon_tests;

impl crate::GameplayState {
    pub fn upgrade_kingdom_with_rules(
        &mut self,
        actor: PlayerUuid,
        id: KingdomId,
        rules: &KingdomRules,
    ) -> Result<u16, KingdomError> {
        if !rules.valid() {
            return Err(KingdomError::InvalidKingdomRules);
        }
        if rules.settlements_follow_kingdom_level
            && self
                .kingdoms
                .get(id)
                .is_some_and(|record| record.parent.is_some())
        {
            return Err(KingdomError::SettlementLevelInherited);
        }
        self.upgrade_kingdom(actor, id)
    }
}
