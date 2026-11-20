//! Toggleable, live display of confirmed assignments after their dialogs close.
use bevy::prelude::*;
use serde::Serialize;
use terra_core::work_areas::{
    ConstructionAreaDisplay, WorkAreaDisplay, WorkAreaKind, WorkAreaTarget,
};
use terra_network::{
    client::{
        ClientSession,
        work_areas::{settlement_for_worker, visit_construction_areas, visit_saved_areas},
    },
    protocol::{building_jobs::BuildingAction, npc_work::WorkAction},
};
use tilt_ui::html_shared;

#[derive(Message)]
pub enum WorkAreaUiAction {
    Hide,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct WorkAreaLegendRow {
    pub kind: String,
    pub label: String,
    pub class_name: String,
}

#[html_shared]
#[derive(Resource, Default, Debug, Serialize, PartialEq)]
pub struct WorkAreasView {
    pub visible: bool,
    pub available: bool,
    pub construction: bool,
    pub owner: String,
    pub items: Vec<WorkAreaLegendRow>,
}

#[derive(Resource, Default)]
struct Poll {
    next: f64,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<WorkAreaDisplay>()
        .init_resource::<ConstructionAreaDisplay>()
        .init_resource::<WorkAreasView>()
        .init_resource::<Poll>()
        .add_message::<WorkAreaUiAction>()
        .add_systems(Update, update);
}

fn update(world: &mut World) {
    if world
        .resource_mut::<Messages<WorkAreaUiAction>>()
        .drain()
        .next()
        .is_some()
    {
        if world
            .resource::<ConstructionAreaDisplay>()
            .kingdom
            .is_some()
        {
            world.resource_mut::<ConstructionAreaDisplay>().visible = false;
        } else {
            world.resource_mut::<WorkAreaDisplay>().target = None;
        }
    }
    let Some(session) = world.get_resource::<ClientSession>() else {
        world.resource_mut::<WorkAreaDisplay>().target = None;
        *world.resource_mut::<ConstructionAreaDisplay>() = Default::default();
        *world.resource_mut::<WorkAreasView>() = Default::default();
        return;
    };
    if !session.ready || session.error.is_some() {
        world.resource_mut::<WorkAreaDisplay>().target = None;
        *world.resource_mut::<ConstructionAreaDisplay>() = Default::default();
        *world.resource_mut::<WorkAreasView>() = Default::default();
        return;
    }
    let construction = *world.resource::<ConstructionAreaDisplay>();
    if construction.kingdom.is_some() && !construction.visible {
        *world.resource_mut::<WorkAreasView>() = Default::default();
        return;
    }
    let target = construction
        .kingdom
        .map(WorkAreaTarget::Settlement)
        .or(world.resource::<WorkAreaDisplay>().target);
    let Some(target) = target else {
        *world.resource_mut::<WorkAreasView>() = Default::default();
        return;
    };
    let mut kinds = [false; 4];
    let mut track = |mark: terra_core::work_areas::WorkAreaMark| {
        kinds[match mark.kind {
            WorkAreaKind::Work => 0,
            WorkAreaKind::Storage => 1,
            WorkAreaKind::Residential => 2,
            WorkAreaKind::BuildingOrigin => 3,
        }] = true
    };
    if let Some(kingdom) = construction.kingdom {
        visit_construction_areas(session, kingdom, &mut track);
    } else {
        visit_saved_areas(session, Some(target), &mut track);
    }
    let build_chunks = construction.kingdom.is_some_and(|kingdom| {
        session.kingdoms.claims.iter().any(|claim| {
            claim.kind == terra_core::kingdom::ClaimKind::ProtectedBuilding
                && session.kingdoms.realm_id(claim.kingdom) == session.kingdoms.realm_id(kingdom)
        })
    });
    let town = match target {
        WorkAreaTarget::Worker(npc) => settlement_for_worker(session, npc),
        WorkAreaTarget::Settlement(id) => Some(id),
    };
    let owner = match target {
        WorkAreaTarget::Worker(npc) => session
            .work
            .workers
            .iter()
            .find(|worker| worker.npc == npc)
            .map_or_else(String::new, |worker| worker.name.clone()),
        WorkAreaTarget::Settlement(id) => session
            .kingdoms
            .get(id)
            .map_or_else(String::new, |town| town.name.clone()),
    };
    let modal = world
        .get_resource::<crate::npc_work::WorkUiState>()
        .is_some_and(crate::npc_work::WorkUiState::blocks_input)
        || world
            .get_resource::<crate::KingdomView>()
            .is_some_and(|view| view.visible)
        || world
            .get_resource::<crate::NpcWorkView>()
            .is_some_and(|view| view.visible || view.selection_active)
        || world
            .get_resource::<crate::BuildingView>()
            .is_some_and(|view| view.selection_active)
        || world
            .get_resource::<crate::InventoryView>()
            .is_some_and(|view| view.open)
        || world
            .get_resource::<crate::menu::MenuState>()
            .is_some_and(|menu| menu.page != crate::menu::MenuPage::Playing);
    let mut items: Vec<_> = [
        ("work", "Arbeitsgebiet"),
        ("storage", "Lager"),
        ("residential", "Wohngebiet"),
        ("building", "Baupositionen"),
    ]
    .into_iter()
    .zip(kinds)
    .filter(|(_, visible)| *visible)
    .map(|((kind, label), _)| WorkAreaLegendRow {
        kind: kind.into(),
        label: label.into(),
        class_name: format!("work-area-legend work-area-{kind}"),
    })
    .collect();
    if build_chunks {
        items.push(WorkAreaLegendRow {
            kind: "build-chunks".into(),
            label: "Bau-Chunk-Grenze".into(),
            class_name: "work-area-legend work-area-build-chunks".into(),
        });
    }
    let next = WorkAreasView {
        visible: !modal,
        available: build_chunks || kinds.into_iter().any(|present| present),
        construction: construction.kingdom.is_some(),
        owner,
        items,
    };
    if *world.resource::<WorkAreasView>() != next {
        *world.resource_mut::<WorkAreasView>() = next;
    }
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |time| time.elapsed_secs_f64());
    if modal
        || world
            .get_resource::<crate::BuildModeView>()
            .is_some_and(|view| view.pending)
        || world
            .get_resource::<crate::NpcWorkView>()
            .is_some_and(|view| view.pending)
        || now < world.resource::<Poll>().next
    {
        return;
    }
    world.resource_mut::<Poll>().next = now + 2.0;
    if construction.kingdom.is_some() {
        // Building/stock responses already come from construction's poller.
        // Only refresh the authorized worker roster here; never replace a
        // pending construction acknowledgement with a background inspection.
        let request = world
            .resource_mut::<ClientSession>()
            .make_work_request(WorkAction::Inspect);
        if let Some(request) = request {
            world.write_message(request);
        }
        return;
    }
    if let WorkAreaTarget::Worker(npc) = target {
        let request = world
            .resource_mut::<ClientSession>()
            .make_work_request(WorkAction::InspectNpc { npc });
        if let Some(request) = request {
            world.write_message(request);
        }
    }
    if let Some(kingdom) = town {
        let request = world
            .resource_mut::<ClientSession>()
            .make_building_request(BuildingAction::Inspect { kingdom });
        if let Some(request) = request {
            world.write_message(request);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_legend_reads_confirmed_areas_without_overwriting_building_acknowledgements() {
        let mut app = App::new();
        let actor = terra_core::PlayerUuid::from_u128(243);
        let mut session = ClientSession::default();
        session.ready = true;
        session.uuid = Some(actor);
        let kingdom = session.kingdoms.found(actor, [0, 12, 0], "Own").unwrap();
        let mut site = terra_core::building_jobs::SettlementBuilding::new(actor);
        site.storage =
            Some(terra_core::building_jobs::BuildArea::new([2, 12, 2], [4, 13, 4]).unwrap());
        site.residential =
            Some(terra_core::building_jobs::BuildArea::new([8, 12, 8], [15, 12, 15]).unwrap());
        session.buildings.settlements.insert(kingdom, site);
        app.insert_resource(session)
            .add_message::<terra_network::client::BuildingRequest>()
            .add_message::<terra_network::client::NpcWorkRequest>()
            .insert_resource(crate::BuildModeView {
                pending: true,
                ..Default::default()
            });
        install(&mut app);
        *app.world_mut().resource_mut::<ConstructionAreaDisplay>() = ConstructionAreaDisplay {
            kingdom: Some(kingdom),
            visible: true,
        };
        app.world_mut().resource_mut::<WorkAreaDisplay>().target = Some(WorkAreaTarget::Worker(7));
        app.update();
        let view = app.world().resource::<WorkAreasView>();
        assert!(view.visible && view.available && view.construction);
        assert_eq!(
            view.items
                .iter()
                .map(|row| row.kind.as_str())
                .collect::<Vec<_>>(),
            ["storage", "residential", "build-chunks"]
        );
        assert!(
            app.world()
                .resource::<Messages<terra_network::client::BuildingRequest>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<terra_network::client::NpcWorkRequest>>()
                .is_empty()
        );
        app.world_mut().write_message(WorkAreaUiAction::Hide);
        app.update();
        assert!(!app.world().resource::<ConstructionAreaDisplay>().visible);
        assert!(!app.world().resource::<WorkAreasView>().visible);
        assert_eq!(
            app.world().resource::<WorkAreaDisplay>().target,
            Some(WorkAreaTarget::Worker(7))
        );
    }

    #[test]
    fn hide_and_disconnect_clear_session_local_area_visualization() {
        let mut app = App::new();
        app.init_resource::<ClientSession>();
        install(&mut app);
        app.world_mut().resource_mut::<ClientSession>().ready = true;
        app.world_mut().resource_mut::<WorkAreaDisplay>().target = Some(WorkAreaTarget::Worker(7));
        app.world_mut().write_message(WorkAreaUiAction::Hide);
        app.update();
        assert!(app.world().resource::<WorkAreaDisplay>().target.is_none());
        app.world_mut().resource_mut::<WorkAreaDisplay>().target = Some(WorkAreaTarget::Worker(7));
        app.world_mut().resource_mut::<ClientSession>().ready = false;
        app.update();
        assert!(app.world().resource::<WorkAreaDisplay>().target.is_none());
        assert!(!app.world().resource::<WorkAreasView>().visible);
    }
}
