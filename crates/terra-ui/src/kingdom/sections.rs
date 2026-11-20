//! Presentation of real settlement registries and the currently replicated roster.
//! Loaded soldiers are observations, never invented global military statistics.
use super::*;
use terra_core::{
    creatures::{CreatureBehavior, soldiers::SoldierLoadout},
    npc_jobs::{WorkKind, WorkStatus},
};
use terra_network::client::ReplicatedCreatures;

#[derive(Clone, Default, Serialize, PartialEq)]
pub struct SectionStat {
    pub key: String,
    pub label: String,
    pub value: String,
    pub icon: String,
}

#[derive(Clone, Serialize, PartialEq)]
pub struct ArmyRow {
    pub id: String,
    pub name: String,
    pub loadout: String,
    pub icon: String,
    pub status: String,
    pub location: String,
    pub health: String,
}

#[derive(Clone, Default, Serialize, PartialEq)]
pub struct KingdomSectionsView {
    pub realm_stats: Vec<SectionStat>,
    pub own_realm: bool,
    pub worker_stats: Vec<SectionStat>,
    pub professions: Vec<SectionStat>,
    pub army_stats: Vec<SectionStat>,
    pub army: Vec<ArmyRow>,
    pub army_empty: bool,
    pub worker_count: String,
}

fn icon_source(name: &str) -> String {
    format!("tilt-icon:{name}@32")
}

fn stat(key: &str, label: &str, value: usize, icon: &str) -> SectionStat {
    SectionStat {
        key: key.into(),
        label: label.into(),
        value: value.to_string(),
        icon: icon_source(icon),
    }
}

pub(super) fn project(
    world: &World,
    session: &ClientSession,
    record: Option<&KingdomRecord>,
) -> KingdomSectionsView {
    let mut result = KingdomSectionsView::default();
    let Some(record) = record else {
        return result;
    };
    let realm = session.kingdoms.realm_id(record.id);
    let settlements: Vec<_> = session
        .kingdoms
        .kingdoms
        .iter()
        .filter(|other| {
            other.state == KingdomState::Active && session.kingdoms.realm_id(other.id) == realm
        })
        .collect();
    result.realm_stats = [
        (SettlementKind::Village, "villages", "Dörfer", "trees"),
        (SettlementKind::City, "cities", "Städte", "landmark"),
        (SettlementKind::Outpost, "outposts", "Außenposten", "flag"),
    ]
    .into_iter()
    .map(|(kind, key, label, icon)| {
        stat(
            key,
            label,
            settlements
                .iter()
                .filter(|settlement| settlement.kind == kind)
                .count(),
            icon,
        )
    })
    .collect();
    result.own_realm = session
        .uuid
        .and_then(|player| session.kingdoms.active_for_member(player))
        .is_some_and(|owned| owned.id == realm);
    if !result.own_realm {
        result.army_empty = true;
        return result;
    }
    let assigned = session
        .work
        .workers
        .iter()
        .filter(|worker| {
            worker.order.as_ref().is_some_and(|order| order.enabled)
                || session.buildings.is_craftsman(worker.npc)
        })
        .count();
    let working = session
        .work
        .workers
        .iter()
        .filter(|worker| {
            matches!(
                worker.status,
                WorkStatus::Working | WorkStatus::Travelling | WorkStatus::Delivering
            )
        })
        .count();
    result.worker_count = session.work.workers.len().to_string();
    result.worker_stats = vec![
        stat(
            "known",
            "Verfügbare Arbeitskräfte",
            session.work.workers.len(),
            "users-round",
        ),
        stat("assigned", "Mit Auftrag", assigned, "layers"),
        stat("working", "Bei der Arbeit", working, "settings-wrench"),
    ];
    result.professions = WorkKind::ALL
        .into_iter()
        .map(|kind| {
            let count = session
                .work
                .workers
                .iter()
                .filter(|worker| {
                    worker
                        .order
                        .as_ref()
                        .is_some_and(|order| order.kind == kind)
                        && !session.buildings.is_craftsman(worker.npc)
                })
                .count();
            let (key, icon) = match kind {
                WorkKind::Miner => ("miner", "settings-wrench"),
                WorkKind::Farmer => ("farmer", "trees"),
                WorkKind::Collector => ("collector", "archive"),
                WorkKind::Lumberjack => ("lumberjack", "trees"),
            };
            stat(key, kind.label(), count, icon)
        })
        .chain(std::iter::once(stat(
            "craftsman",
            "Handwerker",
            session
                .work
                .workers
                .iter()
                .filter(|worker| session.buildings.is_craftsman(worker.npc))
                .count(),
            "settings-wrench",
        )))
        .collect();
    let mut counts = [0; 3];
    let mut moving = 0;
    if let Some(creatures) = world.get_resource::<ReplicatedCreatures>() {
        for (id, track) in &creatures.creatures {
            let creature = &track.current;
            let Some(loadout) = SoldierLoadout::of(creature).filter(|_| creature.health > 0.0)
            else {
                continue;
            };
            let Some(owner) = creatures.bonds.get(id).and_then(|bond| bond.owner) else {
                continue;
            };
            if !session
                .kingdoms
                .active_for_member(owner)
                .is_some_and(|kingdom| kingdom.id == realm)
            {
                continue;
            }
            let (index, label, icon) = match loadout {
                SoldierLoadout::Sword => (0, "Schwertkämpfer", "swords"),
                SoldierLoadout::Spear => (1, "Lanzenträger", "shield"),
                SoldierLoadout::Bow => (2, "Bogenschützen", "shield"),
            };
            counts[index] += 1;
            let in_build = session
                .kingdoms
                .claim_at(terra_core::kingdom::claim_chunk(
                    creature.position.map(|p| p.floor() as i64),
                ))
                .is_some_and(|claim| {
                    claim.kind == ClaimKind::ProtectedBuilding
                        && session.kingdoms.realm_id(claim.kingdom) == realm
                });
            let walking = creature.velocity[0].hypot(creature.velocity[2]) > 0.05;
            moving += usize::from(walking && in_build);
            let status = match creature.behavior {
                CreatureBehavior::Attack => "Im Kampf",
                CreatureBehavior::Sleep => "Ruht",
                _ if walking && in_build => "Im Baugebiet unterwegs",
                _ if walking => "Unterwegs",
                _ => "Bereit / Pause",
            };
            result.army.push(ArmyRow {
                id: id.to_string(),
                name: terra_core::creatures::villagers::name(*id).into(),
                loadout: label.into(),
                icon: icon_source(icon),
                status: status.into(),
                location: format!(
                    "{} / {} / {}",
                    creature.position[0].floor() as i64,
                    creature.position[1].floor() as i64,
                    creature.position[2].floor() as i64
                ),
                health: format!("{:.0}", creature.health),
            });
        }
    }
    result.army_stats = vec![
        stat("sword", "Schwertkämpfer", counts[0], "swords"),
        stat("spear", "Lanzenträger", counts[1], "shield"),
        stat("bow", "Bogenschützen", counts[2], "shield"),
        stat("moving", "Im Baugebiet unterwegs", moving, "map"),
    ];
    result.army_empty = result.army.is_empty();
    result
}

#[cfg(test)]
mod tests;
