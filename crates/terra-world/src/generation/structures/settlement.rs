use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VillageSize {
    Small,
    Medium,
    Large,
}
impl VillageSize {
    pub(crate) fn from_seed(seed: u32) -> Self {
        match (seed >> 20) % 3 {
            0 => Self::Small,
            1 => Self::Medium,
            _ => Self::Large,
        }
    }
    pub(crate) fn houses(self, seed: u32) -> usize {
        match self {
            Self::Small => 6 + seed as usize % 3,
            Self::Medium => 10 + seed as usize % 7,
            Self::Large => 18 + seed as usize % 7,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VillageBed {
    pub foot: [i64; 3],
    pub facing: u8,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VillageResidence {
    pub id: u16,
    pub entrance: [i64; 3],
    pub bounds: [[i64; 3]; 2],
    pub beds: Vec<VillageBed>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidentSite {
    pub layout: SettlementLayout,
    pub slot: u16,
    pub role: u8,
    /// Primary foot cell; head = foot + cardinal forward[bed_facing].
    pub bed: [i64; 3],
    pub bed_facing: u8,
    /// Actual lower doorway cell, not the external path portal.
    pub door: [i64; 3],
    /// Existing building bounds include its one-cell eaves around the room.
    pub home_bounds: [[i64; 3]; 2],
    pub village_id: u64,
    pub village_center: [i64; 3],
    pub village_bounds: [[i64; 3]; 2],
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VillageSettlement {
    pub id: u64,
    pub size: VillageSize,
    pub layout: SettlementLayout,
    pub style: VillageStyle,
    pub center: [i64; 3],
    pub bounds: [[i64; 3]; 2],
    pub homes: Vec<VillageResidence>,
    pub services: Vec<BuildingSummary>,
    pub residents: Vec<ResidentSite>,
}

impl VillageSettlement {
    pub fn domain(&self) -> terra_core::kingdom::SettlementDomain {
        use terra_core::kingdom::{DomainKind, SettlementDomain};
        let kind = match self.layout {
            SettlementLayout::Village => DomainKind::Village,
            SettlementLayout::Outpost => DomainKind::Outpost,
            _ => DomainKind::Kingdom,
        };
        SettlementDomain {
            id: self.id,
            center: self.center,
            bounds: self.bounds,
            kind,
            native: self.bot_kingdom(),
        }
    }

    pub fn bot_kingdom(&self) -> Option<terra_core::kingdom::BotKingdom> {
        terra_core::kingdom::BotKingdom::for_settlement(
            self.id,
            self.layout != SettlementLayout::Village,
        )
    }
}

impl ResidentSite {
    pub fn bot_kingdom(&self) -> Option<terra_core::kingdom::BotKingdom> {
        terra_core::kingdom::BotKingdom::for_settlement(
            self.village_id,
            self.layout != SettlementLayout::Village,
        )
    }
}

pub(super) fn describe(
    summary: &SiteSummary,
    sections: &BTreeMap<SectionPos, StructurePatch>,
    seed: u32,
) -> Option<Arc<VillageSettlement>> {
    let layout = summary.layout()?;
    let mut buildings: Vec<_> = summary.buildings.iter().collect();
    buildings.sort_unstable_by_key(|b| b.entrance);
    let mut homes = Vec::new();
    let mut residents = Vec::new();
    let mut claimed = std::collections::BTreeSet::new();
    for building in buildings {
        let mut beds: Vec<_> = sections
            .values()
            .flat_map(|s| &s.objects)
            .filter_map(|o| {
                let state = o.state?;
                ((71..=73).contains(&o.material)
                    && !state.secondary
                    && (0..3).all(|a| {
                        (building.bounds[0][a]..=building.bounds[1][a]).contains(&o.position[a])
                    })
                    && claimed.insert(o.position))
                .then_some(VillageBed {
                    foot: o.position,
                    facing: state.facing % 4,
                })
            })
            .collect();
        beds.sort_unstable_by_key(|b| b.foot);
        if building.role == BuildingRole::House {
            homes.push(VillageResidence {
                id: homes.len() as u16,
                entrance: building.entrance,
                bounds: building.bounds,
                beds: beds.clone(),
            });
        }
        for bed in beds {
            if residents.len() >= 256 {
                break;
            }
            let role = match building.role {
                BuildingRole::Church => 5,
                BuildingRole::Medic => 7,
                BuildingRole::Smith => 8,
                BuildingRole::Police => 4,
                BuildingRole::Barracks => 10 + (residents.len() % 3) as u8,
                BuildingRole::Mine => terra_core::creatures::villagers::MINER,
                BuildingRole::Market => terra_core::creatures::villagers::SELLER,
                _ => civilian_role(seed, residents.len()),
            };
            residents.push(ResidentSite {
                layout,
                slot: residents.len() as u16,
                role,
                bed: bed.foot,
                bed_facing: bed.facing,
                door: building.entrance,
                home_bounds: building.bounds,
                village_id: summary.id,
                village_center: summary.center,
                village_bounds: summary.bounds,
            });
        }
    }
    Some(Arc::new(VillageSettlement {
        id: summary.id,
        size: VillageSize::from_seed(seed),
        layout,
        style: summary.style,
        center: summary.center,
        bounds: summary.bounds,
        homes,
        services: summary
            .buildings
            .iter()
            .filter(|b| b.role != BuildingRole::House)
            .cloned()
            .collect(),
        residents,
    }))
}

/// Bed slots and resident identities stay stable while unoccupied homes gain the new jobs.
fn civilian_role(seed: u32, resident_index: usize) -> u8 {
    use terra_core::creatures::villagers::*;
    const HOUSE_ROLES: [u8; 18] = [
        0,
        1,
        8,
        9,
        6,
        2,
        3,
        ENGINEER,
        MINER,
        UNEMPLOYED,
        BUTCHER,
        SELLER,
        TRAVELLING_MERCHANT,
        NOMAD_A,
        NOMAD_B,
        NOMAD_C,
        NOMAD_D,
        CRAFTSMAN,
    ];
    HOUSE_ROLES[(resident_index + seed as usize) % HOUSE_ROLES.len()]
}

#[cfg(test)]
mod profession_tests {
    use super::*;

    #[test]
    fn settlement_beds_generate_every_new_civilian_without_adding_a_garrison() {
        use terra_core::creatures::{soldiers::SoldierLoadout, villagers};
        for seed in [0, 522, u32::MAX] {
            let roles: std::collections::BTreeSet<_> =
                (0..34).map(|slot| civilian_role(seed, slot)).collect();
            for role in villagers::ENGINEER..villagers::ROLE_COUNT {
                assert!(roles.contains(&role));
            }
            assert!(roles.iter().all(|role| villagers::is_civilian(*role)));
            assert!(
                roles
                    .iter()
                    .all(|role| SoldierLoadout::from_role(*role).is_none())
            );
        }
    }
}
