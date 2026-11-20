//! Read-only town stock: only acknowledged server inventories feed the HUD.
use super::*;
use terra_core::{
    ItemKind,
    kingdom::{ClaimKind, claim_chunk},
};

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ResourceRow {
    pub label: String,
    pub amount: String,
    pub preview_key: String,
}

#[html_shared]
#[derive(Resource, Default, Clone, Debug, Serialize, PartialEq)]
pub struct KingdomResourcesView {
    pub visible: bool,
    pub name: String,
    pub status: String,
    pub style: String,
    pub rows: Vec<ResourceRow>,
}

#[derive(Resource, Default)]
struct Poll {
    last: f64,
    kingdom: Option<KingdomId>,
}

pub(super) fn build_claim(session: &ClientSession, position: [i64; 3]) -> Option<KingdomId> {
    let actor = session.uuid?;
    let own = session.kingdoms.active_for_member(actor)?;
    let claim = session.kingdoms.claim_at(claim_chunk(position))?;
    (claim.kind == ClaimKind::ProtectedBuilding
        && session.kingdoms.realm_id(claim.kingdom) == session.kingdoms.realm_id(own.id))
    .then_some(claim.kingdom)
}

pub(super) fn supply(
    session: &ClientSession,
    kingdom: Option<KingdomId>,
    item: ItemKind,
    required: u32,
) -> (String, bool) {
    if kingdom.is_none() || session.buildings.stock_kingdom != kingdom {
        return (format!("? / {required}"), false);
    }
    if session.buildings.unlimited_resources {
        return (format!("∞ / {required}"), true);
    }
    let stock = &session.buildings.stock;
    if !stock.configured {
        return (format!("0 / {required}"), false);
    }
    let count = stock.count(item);
    (
        format!(
            "{}{count} / {required}",
            if stock.complete { "" } else { "≥" }
        ),
        count >= required,
    )
}

pub(super) fn can_inspect(session: &ClientSession, kingdom: KingdomId) -> bool {
    session.allow_build
        && session
            .uuid
            .zip(session.kingdoms.get(kingdom))
            .is_some_and(|(actor, record)| {
                session.kingdoms.can(
                    actor,
                    terra_core::kingdom::KingdomPermission::NpcWork,
                    record.location,
                )
            })
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Poll>()
        .init_resource::<KingdomResourcesView>()
        .add_systems(
            Update,
            update
                .after(super::update)
                .before(super::build_mode::BuildModeSet),
        );
}

fn update(world: &mut World) {
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |time| time.elapsed_secs_f64());
    let context = world
        .get_resource::<ClientSession>()
        .filter(|session| session.ready && !session.dead() && session.error.is_none())
        .and_then(|session| {
            session.confirmed_state().and_then(|body| {
                build_claim(session, body.position.map(|p| f64::from(p).floor() as i64))
            })
        });
    // The keep can inspect a different town in the same realm. While its dialog
    // owns the building response stream, the background HUD must not replace
    // that town's stock or consume an outstanding command acknowledgement.
    let modal = world
        .get_resource::<crate::menu::MenuState>()
        .is_some_and(|menu| menu.page != crate::menu::MenuPage::Playing)
        || world
            .get_resource::<crate::InventoryView>()
            .is_some_and(|view| view.open)
        || world.resource::<crate::KingdomView>().visible
        || world.resource::<crate::NpcWorkView>().direct;
    if let Some(kingdom) = context {
        let busy = world
            .get_resource::<super::build_mode::State>()
            .is_some_and(|state| state.pending.is_some())
            || world
                .get_resource::<super::BuildingUiState>()
                .is_some_and(|state| state.pending.is_some());
        let poll = world.resource::<Poll>();
        if !modal
            && !busy
            && can_inspect(world.resource::<ClientSession>(), kingdom)
            && (poll.kingdom != Some(kingdom) || now - poll.last >= 1.2)
        {
            let request = world
                .resource_mut::<ClientSession>()
                .make_building_request(BuildingAction::Inspect { kingdom });
            if let Some(request) = request {
                world.write_message(request);
            }
            let mut poll = world.resource_mut::<Poll>();
            poll.kingdom = Some(kingdom);
            poll.last = now;
        }
    } else {
        world.resource_mut::<Poll>().kingdom = None;
    }
    let Some(kingdom) = context else {
        *world.resource_mut::<KingdomResourcesView>() = Default::default();
        return;
    };
    let session = world.resource::<ClientSession>();
    let ready = session.buildings.stock_kingdom == Some(kingdom);
    let stock = &session.buildings.stock;
    let unlimited = ready && session.buildings.unlimited_resources;
    let permitted = can_inspect(session, kingdom);
    let status = if !permitted {
        "Keine Berechtigung für Lagerbestände"
    } else if !ready {
        "Lagerbestände werden geladen …"
    } else if unlimited {
        "Creative: unbegrenzte Baumaterialien"
    } else if !stock.configured {
        "Stadtlager im Bergfried festlegen"
    } else if !stock.complete {
        "Lager teilweise nicht geladen"
    } else {
        "Stadtlager"
    };
    let rows = [
        ("Holz", stock.wood, ItemKind::Block(6)),
        ("Stein", stock.stone, ItemKind::Block(3)),
        (
            "Eisen",
            stock.iron,
            ItemKind::Mineral(terra_core::MineralItem::IronIngot),
        ),
        (
            "Kohle",
            stock.coal,
            ItemKind::Mineral(terra_core::MineralItem::Coal),
        ),
    ]
    .into_iter()
    .map(|(label, amount, item)| ResourceRow {
        label: label.into(),
        amount: if !permitted {
            "—".into()
        } else if !ready {
            "…".into()
        } else if unlimited {
            "∞".into()
        } else {
            amount.to_string()
        },
        preview_key: crate::inventory::preview_key(item),
    })
    .collect();
    let name = session
        .kingdoms
        .get(kingdom)
        .map_or_else(String::new, |record| record.name.clone());
    let hud = world
        .get_resource::<crate::HudVisibility>()
        .is_none_or(|hud| hud.visible);
    let width = world
        .query::<&Window>()
        .iter(world)
        .next()
        .map_or(1280.0, |window| window.width());
    let map = world
        .get_resource::<terra_core::GameplayConfig>()
        .map_or(260.0, |config| f32::from(config.minimap.size));
    let style = if width < 1200.0 {
        "left: 12px; right: auto; width: 256px;".to_owned()
    } else {
        format!("right: {:.0}px; left: auto; width: 344px;", map + 28.0)
    };
    let next = KingdomResourcesView {
        visible: hud && !modal,
        name,
        status: status.into(),
        style,
        rows,
    };
    if *world.resource::<KingdomResourcesView>() != next {
        *world.resource_mut::<KingdomResourcesView>() = next;
    }
}
