//! Saved, server-confirmed outlines shared by UI and rendering. Drafts stay out.
use super::ClientSession;
use terra_core::{
    building_jobs::{BuildArea, BuildStatus, MAX_BUILD_JOBS},
    kingdom::KingdomId,
    work_areas::{WorkAreaKind, WorkAreaMark, WorkAreaTarget},
};

pub fn settlement_for_worker(session: &ClientSession, npc: u64) -> Option<KingdomId> {
    session
        .buildings
        .settlements
        .iter()
        .find_map(|(id, site)| site.craftsmen.contains(&npc).then_some(*id))
}

pub fn visit_saved_areas(
    session: &ClientSession,
    target: Option<WorkAreaTarget>,
    mut emit: impl FnMut(WorkAreaMark),
) {
    if !session.ready || session.error.is_some() {
        return;
    }
    let mut mark = |kind, bounds: BuildArea| {
        if bounds.valid() {
            emit(WorkAreaMark { kind, bounds });
        }
    };
    let settlement = match target {
        Some(WorkAreaTarget::Worker(npc)) => {
            let Some(worker) = session.work.workers.iter().find(|worker| worker.npc == npc) else {
                return;
            };
            if let Some(order) = &worker.order {
                mark(
                    WorkAreaKind::Work,
                    BuildArea {
                        min: order.area.min,
                        max: order.area.max,
                    },
                );
                if let Ok(area) = BuildArea::new(order.storage, order.storage) {
                    mark(WorkAreaKind::Storage, area);
                }
            }
            settlement_for_worker(session, npc)
        }
        Some(WorkAreaTarget::Settlement(id)) => Some(id),
        None => None,
    };
    let Some(id) = settlement else { return };
    let Some(site) = session.buildings.settlements.get(&id) else {
        return;
    };
    if let Some(area) = site.storage {
        mark(WorkAreaKind::Storage, area);
    }
    if let Some(area) = site.residential {
        mark(WorkAreaKind::Residential, area);
    }
    for job in session
        .buildings
        .jobs
        .values()
        .filter(|job| job.kingdom == id && job.status != BuildStatus::Cancelled)
        .take(MAX_BUILD_JOBS)
    {
        if let Ok(area) = BuildArea::new(job.origin, job.origin) {
            mark(WorkAreaKind::BuildingOrigin, area);
        }
    }
}

pub fn has_saved_areas(session: &ClientSession, target: WorkAreaTarget) -> bool {
    let mut found = false;
    visit_saved_areas(session, Some(target), |_| found = true);
    found
}

/// Visit confirmed assignments of the current realm while planning. This reads
/// the existing authorized replicas; it never requests private foreign data.
pub fn visit_construction_areas(
    session: &ClientSession,
    kingdom: KingdomId,
    mut emit: impl FnMut(WorkAreaMark),
) {
    use terra_core::kingdom::{KingdomPermission, KingdomState};
    if !session.ready || session.error.is_some() {
        return;
    }
    let Some(actor) = session.uuid else { return };
    let registry = &session.kingdoms;
    let realm = registry.realm_id(kingdom);
    let Some(owner) = registry
        .get(realm)
        .filter(|record| record.state == KingdomState::Active && record.members.contains(&actor))
    else {
        return;
    };
    let permitted = |position| registry.can(actor, KingdomPermission::NpcWork, position);
    for id in session.buildings.settlements.keys().copied().filter(|id| {
        registry.realm_id(*id) == realm
            && registry.get(*id).is_some_and(|record| {
                record.state == KingdomState::Active && permitted(record.location)
            })
    }) {
        visit_saved_areas(session, Some(WorkAreaTarget::Settlement(id)), &mut emit);
    }
    for order in session
        .work
        .workers
        .iter()
        .filter_map(|worker| worker.order.as_ref())
    {
        if !owner.members.contains(&order.owner)
            || !permitted(order.area.min)
            || !permitted(order.area.max)
        {
            continue;
        }
        if let Ok(bounds) = BuildArea::new(order.area.min, order.area.max) {
            emit(WorkAreaMark {
                kind: WorkAreaKind::Work,
                bounds,
            });
        }
        if permitted(order.storage)
            && let Ok(bounds) = BuildArea::new(order.storage, order.storage)
        {
            emit(WorkAreaMark {
                kind: WorkAreaKind::Storage,
                bounds,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::npc_work::WorkerView;
    use terra_core::{PlayerUuid, building_jobs::SettlementBuilding, npc_jobs::*};

    #[test]
    fn construction_visits_own_confirmed_sites_and_worker_orders_without_foreign_areas() {
        let actor = PlayerUuid::from_u128(243);
        let foreign = PlayerUuid::from_u128(244);
        let mut session = ClientSession {
            ready: true,
            uuid: Some(actor),
            ..Default::default()
        };
        let own = session.kingdoms.found(actor, [0, 3, 0], "Own").unwrap();
        let member = PlayerUuid::from_u128(245);
        session.kingdoms.join(member, own).unwrap();
        let other = session
            .kingdoms
            .found(foreign, [64, 3, 64], "Other")
            .unwrap();
        let mut site = SettlementBuilding::new(actor);
        site.storage = Some(BuildArea::new([2, 3, 2], [3, 4, 3]).unwrap());
        site.residential = Some(BuildArea::new([8, 3, 8], [14, 4, 14]).unwrap());
        session.buildings.settlements.insert(own, site);
        let mut other_site = SettlementBuilding::new(foreign);
        other_site.storage = Some(BuildArea::new([65, 3, 65], [66, 4, 66]).unwrap());
        session.buildings.settlements.insert(other, other_site);
        for (npc, owner, area) in [
            (7, actor, WorkArea::new([-16, 3, -16], [-1, 4, -1]).unwrap()),
            (8, foreign, WorkArea::new([64, 3, 64], [66, 4, 66]).unwrap()),
            (9, member, WorkArea::new([4, 3, 8], [6, 4, 10]).unwrap()),
        ] {
            session.work.workers.push(WorkerView {
                npc,
                name: "Worker".into(),
                position: [1.5, 3.0, 1.5],
                order: Some(WorkOrder {
                    id: npc,
                    npc,
                    owner,
                    kind: WorkKind::Lumberjack,
                    area,
                    storage: area.min,
                    priority: 5,
                    enabled: true,
                    revision: 1,
                }),
                status: WorkStatus::Idle,
                carried_slots: 0,
            });
        }
        let mut marks = Vec::new();
        visit_construction_areas(&session, own, |mark| marks.push(mark));
        assert_eq!(marks.len(), 6);
        assert_eq!(
            marks.iter().map(|mark| mark.kind).collect::<Vec<_>>(),
            [
                WorkAreaKind::Storage,
                WorkAreaKind::Residential,
                WorkAreaKind::Work,
                WorkAreaKind::Storage,
                WorkAreaKind::Work,
                WorkAreaKind::Storage,
            ]
        );
        assert!(marks.iter().all(|mark| mark.bounds.max[0] < 64));
        marks.clear();
        visit_construction_areas(&session, other, |mark| marks.push(mark));
        assert!(marks.is_empty());
        session.ready = false;
        visit_construction_areas(&session, own, |mark| marks.push(mark));
        assert!(marks.is_empty());
    }

    #[test]
    fn saved_negative_work_area_and_storage_are_inclusive_and_never_use_a_draft() {
        let mut session = ClientSession {
            ready: true,
            ..Default::default()
        };
        let area = WorkArea::new([-16, 4, -16], [-1, 6, -1]).unwrap();
        session.work.workers.push(WorkerView {
            npc: 7,
            name: "Worker".into(),
            position: [-2.5, 5.0, -2.5],
            order: Some(WorkOrder {
                id: 1,
                npc: 7,
                owner: PlayerUuid::from_u128(7),
                kind: WorkKind::Lumberjack,
                area,
                storage: [-1, 4, -1],
                priority: 5,
                enabled: true,
                revision: 1,
            }),
            status: WorkStatus::Idle,
            carried_slots: 0,
        });
        let mut marks = Vec::new();
        visit_saved_areas(&session, Some(WorkAreaTarget::Worker(7)), |a| marks.push(a));
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0].bounds.min, area.min);
        assert_eq!(marks[0].bounds.max, area.max);
        assert_eq!(marks[1].bounds.min, [-1, 4, -1]);
        session.work.workers[0].order = None;
        assert!(!has_saved_areas(&session, WorkAreaTarget::Worker(7)));
        session.ready = false;
        assert!(!has_saved_areas(&session, WorkAreaTarget::Worker(7)));
    }

    #[test]
    fn settlement_visuals_use_confirmed_storage_and_residential_area_and_do_not_leak_other_towns() {
        let mut session = ClientSession {
            ready: true,
            ..Default::default()
        };
        let id = KingdomId(1);
        let mut site = SettlementBuilding::new(PlayerUuid::from_u128(1));
        site.storage = Some(BuildArea::new([0, 3, 0], [3, 5, 3]).unwrap());
        site.residential = Some(BuildArea::new([8, 3, 8], [31, 5, 31]).unwrap());
        session.buildings.settlements.insert(id, site.clone());
        let mut marks = Vec::new();
        visit_saved_areas(&session, Some(WorkAreaTarget::Settlement(id)), |a| {
            marks.push(a)
        });
        assert_eq!(
            marks.iter().map(|a| a.kind).collect::<Vec<_>>(),
            [WorkAreaKind::Storage, WorkAreaKind::Residential]
        );
        assert_eq!(marks[0].bounds, site.storage.unwrap());
        assert!(!has_saved_areas(
            &session,
            WorkAreaTarget::Settlement(KingdomId(2))
        ));
    }
}
