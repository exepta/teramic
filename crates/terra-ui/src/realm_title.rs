//! Territory-entry announcement, keyed by realm identity rather than chunk edges.
use bevy::prelude::*;
use terra_core::kingdom::{DiplomaticTarget, KingdomId, KingdomRegistry, SettlementDomain};
use terra_network::client::ClientSession;
use terra_world::streaming::ChunkStore;
use tilt_ui::html_shared;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RealmIdentity {
    Player(KingdomId),
    Village(u64),
    Neutral,
}

#[html_shared]
#[derive(Resource, Default, serde::Serialize, PartialEq)]
pub struct RealmTitleView {
    pub visible: bool,
    pub name: String,
    pub subtitle: String,
    pub style: String,
}

#[derive(Resource, Default)]
struct EntryState {
    current: Option<RealmIdentity>,
    entered: f64,
    name: String,
    subtitle: String,
    disconnected: bool,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<EntryState>()
        .init_resource::<RealmTitleView>()
        .add_systems(Update, update.after(crate::menu::MenuInputSet));
}

fn alpha(elapsed: f64) -> f64 {
    if !(0.0..6.0).contains(&elapsed) {
        0.0
    } else if elapsed < 0.8 {
        elapsed / 0.8
    } else if elapsed > 4.5 {
        (6.0 - elapsed) / 1.5
    } else {
        1.0
    }
}

fn village_entry<'a>(
    mut domains: impl Iterator<Item = &'a SettlementDomain>,
    registry: &KingdomRegistry,
    viewer: Option<terra_core::PlayerUuid>,
    position: [i64; 3],
) -> Option<(RealmIdentity, String, String)> {
    let domain = domains.find(|domain| domain.contains(position))?;
    if let Some(owner) = registry.domain_owner(domain, position) {
        return Some((
            RealmIdentity::Player(owner.id),
            owner.name.clone(),
            format!(
                "{} · Level {} · {}",
                owner.kind.label(),
                owner.level,
                crate::realm_relations::display(
                    registry,
                    viewer,
                    DiplomaticTarget::Player(owner.id)
                )
                .1
            ),
        ));
    }
    let Some(realm) = domain.native else {
        return Some((
            RealmIdentity::Village(domain.id),
            format!("Independent {}", domain.kind.label()),
            format!("{} · Neutral", domain.kind.label()),
        ));
    };
    Some((
        RealmIdentity::Village(domain.id),
        realm.name().into(),
        format!(
            "{} · {}",
            domain.kind.label(),
            crate::realm_relations::display(registry, viewer, DiplomaticTarget::Bot(realm)).1
        ),
    ))
}

#[allow(clippy::too_many_arguments)]
fn update(
    session: Option<Res<ClientSession>>,
    time: Res<Time<Real>>,
    menu: Option<Res<crate::menu::MenuState>>,
    hud: Option<Res<crate::HudVisibility>>,
    chunks: Option<Res<ChunkStore>>,
    known: Option<Res<crate::settlement_map::SettlementMap>>,
    mut state: ResMut<EntryState>,
    mut view: ResMut<RealmTitleView>,
) {
    let Some(session) = session.filter(|s| s.ready && s.error.is_none()) else {
        *state = EntryState {
            disconnected: true,
            ..default()
        };
        if *view != RealmTitleView::default() {
            *view = default();
        }
        return;
    };
    if session.teleporting || session.dead() {
        view.visible = false;
        return;
    }
    let position = session
        .confirmed_state()
        .map_or(session.spawn.to_array(), |p| p.position)
        .map(|v| v.floor() as i64);
    // Explicit player claims have priority over the immutable NPC village bounds.
    let entry = session
        .kingdoms
        .region_at(position)
        .map(|realm| {
            (
                RealmIdentity::Player(realm.id),
                realm.name.clone(),
                format!(
                    "{} · Level {} · {}",
                    realm.kind.label(),
                    realm.level,
                    crate::realm_relations::display(
                        &session.kingdoms,
                        session.uuid,
                        DiplomaticTarget::Player(realm.id)
                    )
                    .1
                ),
            )
        })
        .or_else(|| {
            known
                .as_ref()
                .and_then(|known| {
                    village_entry(known.domains(), &session.kingdoms, session.uuid, position)
                })
                .or_else(|| {
                    let domains = chunks
                        .as_ref()?
                        .village_settlements()
                        .map(|site| site.domain())
                        .collect::<Vec<_>>();
                    village_entry(domains.iter(), &session.kingdoms, session.uuid, position)
                })
        })
        .unwrap_or_else(|| {
            (
                RealmIdentity::Neutral,
                "Neutral Territory".into(),
                "Wilderness · Unclaimed".into(),
            )
        });
    let current = Some(entry.0);
    if current != state.current || state.disconnected {
        state.current = current;
        state.disconnected = false;
        state.entered = time.elapsed_secs_f64();
    }
    // Live diplomacy, names and capture updates refresh the label without
    // announcing every chunk crossing again or restarting an existing fade.
    state.name = entry.1;
    state.subtitle = entry.2;
    let opacity = alpha(time.elapsed_secs_f64() - state.entered);
    let visible = !state.name.is_empty()
        && opacity > 0.0
        && menu
            .as_ref()
            .is_none_or(|m| m.page == crate::menu::MenuPage::Playing)
        && hud.as_ref().is_none_or(|h| h.visible);
    let next = RealmTitleView {
        visible,
        name: state.name.clone(),
        subtitle: state.subtitle.clone(),
        // Quantization avoids serializing imperceptible changes at uncapped render rates.
        style: format!("opacity: {:.2};", (opacity * 50.0).round() / 50.0),
    };
    if *view != next {
        *view = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn title_fades_in_holds_and_fades_out_without_an_opaque_backdrop() {
        assert_eq!(alpha(0.0), 0.0);
        assert!(alpha(0.4) > 0.0 && alpha(0.4) < 1.0);
        assert_eq!(alpha(2.0), 1.0);
        assert!(alpha(5.0) < 1.0);
        assert_eq!(alpha(6.0), 0.0);
    }

    #[test]
    fn crossing_same_realm_chunks_does_not_restart_but_reentry_and_reconnect_do() {
        use terra_core::{
            PlayerUuid,
            kingdom::{ClaimActor, ClaimKind},
        };
        let owner = PlayerUuid::from_u128(55);
        let mut session = ClientSession::default();
        session.ready = true;
        session.uuid = Some(owner);
        session.spawn = Vec3::new(1.0, 50.0, 1.0);
        let id = session
            .kingdoms
            .found(owner, [0, 50, 0], "Oakwatch")
            .unwrap();
        session
            .kingdoms
            .claim(
                id,
                [1, 0],
                ClaimKind::RealmTerritory,
                ClaimActor::Player(owner),
            )
            .unwrap();
        let mut app = App::new();
        app.insert_resource(session).init_resource::<Time<Real>>();
        install(&mut app);
        app.update();
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_secs(1));
        app.update();
        assert_eq!(app.world().resource::<RealmTitleView>().name, "Oakwatch");
        assert!(app.world().resource::<RealmTitleView>().visible);
        let entered = app.world().resource::<EntryState>().entered;
        app.world_mut().resource_mut::<ClientSession>().spawn.x = 20.0;
        app.update();
        assert_eq!(app.world().resource::<EntryState>().entered, entered);
        app.world_mut().resource_mut::<ClientSession>().spawn.x = -20.0;
        app.update();
        assert_eq!(
            app.world().resource::<RealmTitleView>().name,
            "Neutral Territory"
        );
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_secs(1));
        app.update();
        assert!(app.world().resource::<RealmTitleView>().visible);
        let neutral_entered = app.world().resource::<EntryState>().entered;
        app.world_mut().resource_mut::<ClientSession>().spawn.x = -40.0;
        app.update();
        assert_eq!(
            app.world().resource::<EntryState>().entered,
            neutral_entered
        );
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_secs(6));
        app.update();
        assert!(!app.world().resource::<RealmTitleView>().visible);
        app.world_mut().resource_mut::<ClientSession>().spawn.x = 1.0;
        app.update();
        assert!(app.world().resource::<EntryState>().entered > entered);
        app.world_mut().resource_mut::<ClientSession>().ready = false;
        app.update();
        assert!(!app.world().resource::<RealmTitleView>().visible);
        app.world_mut().resource_mut::<ClientSession>().ready = true;
        app.update();
        assert!(app.world().resource::<EntryState>().current == Some(RealmIdentity::Player(id)));
    }

    #[test]
    fn title_relations_refresh_in_place_after_diplomacy_changes_without_restarting_entry() {
        use terra_core::{PlayerUuid, kingdom::DiplomaticRelation};
        let viewer = PlayerUuid::from_u128(301);
        let neighbor = PlayerUuid::from_u128(302);
        let mut session = ClientSession::default();
        session.ready = true;
        session.uuid = Some(viewer);
        session.spawn = Vec3::new(65.0, 20.0, 1.0);
        let own = session
            .kingdoms
            .found(viewer, [0, 20, 0], "Homewatch")
            .unwrap();
        let other = session
            .kingdoms
            .found(neighbor, [64, 20, 0], "Eastbank")
            .unwrap();
        let mut app = App::new();
        app.insert_resource(session).init_resource::<Time<Real>>();
        install(&mut app);
        app.update();
        let entered = app.world().resource::<EntryState>().entered;
        assert!(
            app.world()
                .resource::<RealmTitleView>()
                .subtitle
                .ends_with("Neutral")
        );
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session
                .kingdoms
                .set_relation(
                    viewer,
                    own,
                    DiplomaticTarget::Player(other),
                    DiplomaticRelation::Allied,
                )
                .unwrap();
            session
                .kingdoms
                .set_relation(
                    neighbor,
                    other,
                    DiplomaticTarget::Player(own),
                    DiplomaticRelation::Allied,
                )
                .unwrap();
        }
        app.update();
        assert!(
            app.world()
                .resource::<RealmTitleView>()
                .subtitle
                .ends_with("Allied")
        );
        assert_eq!(app.world().resource::<EntryState>().entered, entered);
        app.world_mut()
            .resource_mut::<ClientSession>()
            .kingdoms
            .set_relation(
                neighbor,
                other,
                DiplomaticTarget::Player(own),
                DiplomaticRelation::Hostile,
            )
            .unwrap();
        app.update();
        assert!(
            app.world()
                .resource::<RealmTitleView>()
                .subtitle
                .ends_with("Hostile")
        );
        assert_eq!(app.world().resource::<EntryState>().entered, entered);
    }

    #[test]
    fn loaded_village_entry_uses_stable_affiliation_and_its_real_bounds() {
        let site = SettlementDomain {
            id: 0,
            center: [0; 3],
            bounds: [[-10, 0, -20], [10, 15, 20]],
            kind: terra_core::kingdom::DomainKind::Village,
            native: Some(terra_core::kingdom::BotKingdom::Helteria),
        };
        let registry = KingdomRegistry::default();
        let entry = village_entry(std::iter::once(&site), &registry, None, [0, 5, 0]).unwrap();
        assert!(entry.0 == RealmIdentity::Village(0));
        assert_eq!(entry.1, "Helteria");
        assert_eq!(entry.2, "Village · Friendly");
        assert!(village_entry(std::iter::once(&site), &registry, None, [11, 5, 0]).is_none());
        let independent = SettlementDomain {
            native: None,
            ..site
        };
        let entry =
            village_entry(std::iter::once(&independent), &registry, None, [0, 5, 0]).unwrap();
        assert!(entry.0 == RealmIdentity::Village(site.id));
        assert_eq!(entry.1, "Independent Village");
        assert_eq!(entry.2, "Village · Neutral");
    }
    #[test]
    fn outpost_title_uses_capture_of_its_center_outside_the_claimed_chunk() {
        use terra_core::{
            PlayerUuid,
            kingdom::{BotKingdom, DomainKind},
        };
        let domain = SettlementDomain {
            id: 19,
            center: [0, 10, 0],
            bounds: [[-48, 0, -48], [48, 30, 48]],
            kind: DomainKind::Outpost,
            native: Some(BotKingdom::Betross),
        };
        let mut registry = KingdomRegistry::default();
        let point = [35, 10, 35];
        assert_eq!(
            village_entry(std::iter::once(&domain), &registry, None, point)
                .unwrap()
                .2,
            "Outpost · Neutral"
        );
        let id = registry
            .found(PlayerUuid::from_u128(91), domain.center, "Southwatch")
            .unwrap();
        let entry = village_entry(std::iter::once(&domain), &registry, None, point).unwrap();
        assert!(entry.0 == RealmIdentity::Player(id));
        assert_eq!(entry.1, "Southwatch");
    }
}
