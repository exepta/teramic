use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, UiBindingStore, UiStore, html_fn};

#[derive(Default, serde::Serialize, serde::Deserialize, UiStore)]
#[ui_store(mutable)]
pub struct KingdomIntent {
    pub member: String,
    pub icon: String,
    pub role: String,
    pub target: String,
}

#[html_fn("kingdom_command")]
fn kingdom_command(
    In(event): In<HtmlEvent>,
    elements: Query<&ElementId>,
    parents: Query<&ChildOf>,
    store: Res<UiBindingStore>,
    mut actions: MessageWriter<crate::KingdomMenuAction>,
) {
    let mut target = Some(event.target);
    for _ in 0..32 {
        let Some(entity) = target else { return };
        if let Ok(id) = elements.get(entity) {
            use crate::KingdomMenuAction as Action;
            use terra_core::kingdom::{
                BotKingdom, ClaimKind, DiplomaticRelation, DiplomaticTarget, KingdomIcon,
                KingdomId, KingdomRole, SettlementKind,
            };
            let action = match id.0.as_str() {
                "kingdom-close" => Some(Action::Close),
                "kingdom-icons" => Some(Action::OpenIcons),
                "kingdom-icons-close" => Some(Action::CloseIcons),
                "kingdom-icon-option" => store
                    .get_store::<KingdomIntent>()
                    .and_then(|s| KingdomIcon::from_key(&s.icon))
                    .map(Action::Icon),
                "kingdom-found" => Some(Action::Found),
                "kingdom-rename" => Some(Action::Rename),
                "kingdom-join" => Some(Action::Join),
                "kingdom-leave" => Some(Action::Leave),
                "kingdom-save-appearance" => Some(Action::SaveAppearance),
                "kingdom-upgrade" => Some(Action::Upgrade),
                "kingdom-claim-protected" => Some(Action::Claim(ClaimKind::ProtectedBuilding)),
                "kingdom-claim-territory" => Some(Action::Claim(ClaimKind::RealmTerritory)),
                "kingdom-release" => Some(Action::Release),
                "kingdom-dissolve-prompt" => Some(Action::ConfirmDissolve),
                "kingdom-dissolve-cancel" => Some(Action::CancelDissolve),
                "kingdom-dissolve" => Some(Action::Dissolve),
                "kingdom-tab-overview" => Some(Action::Tab("overview".into())),
                "kingdom-tab-members" => Some(Action::Tab("members".into())),
                "kingdom-tab-diplomacy" => Some(Action::Tab("diplomacy".into())),
                "kingdom-tab-work" => Some(Action::Tab("work".into())),
                "kingdom-edit-role" => store
                    .get_store::<KingdomIntent>()
                    .and_then(|s| s.member.parse().ok())
                    .map(Action::EditRole),
                "kingdom-set-role" => store.get_store::<KingdomIntent>().and_then(|s| {
                    Some(Action::SetRole {
                        member: s.member.parse().ok()?,
                        role: KingdomRole::from_key(&s.role)?,
                    })
                }),
                "kingdom-relation-neutral"
                | "kingdom-relation-allied"
                | "kingdom-relation-hostile" => store.get_store::<KingdomIntent>().and_then(|s| {
                    let target = if let Some(id) = s.target.strip_prefix("player:") {
                        DiplomaticTarget::Player(KingdomId(id.parse().ok()?))
                    } else {
                        let key = s.target.strip_prefix("bot:")?;
                        DiplomaticTarget::Bot(
                            BotKingdom::ALL
                                .into_iter()
                                .find(|bot| bot.name().eq_ignore_ascii_case(key))?,
                        )
                    };
                    let relation =
                        DiplomaticRelation::from_key(id.0.strip_prefix("kingdom-relation-")?)?;
                    Some(Action::SetRelation { target, relation })
                }),
                "kingdom-kick" => store
                    .get_store::<KingdomIntent>()
                    .and_then(|s| s.member.parse().ok())
                    .map(Action::Kick),
                value => value
                    .strip_prefix("kingdom-kind-")
                    .and_then(SettlementKind::from_key)
                    .map(Action::Kind)
                    .or_else(|| {
                        value
                            .strip_prefix("kingdom-icon-")
                            .and_then(KingdomIcon::from_key)
                            .map(Action::Icon)
                    }),
            };
            if let Some(action) = action {
                actions.write(action);
                return;
            }
        }
        target = parents.get(entity).ok().map(ChildOf::parent);
    }
}

#[html_fn("kingdom_name")]
fn kingdom_name(In(event): In<HtmlEvent>, mut actions: MessageWriter<crate::KingdomMenuAction>) {
    if let Some(name) = event.value {
        actions.write(crate::KingdomMenuAction::Name(name));
    }
}

#[html_fn("kingdom_color")]
fn kingdom_color(In(event): In<HtmlEvent>, mut actions: MessageWriter<crate::KingdomMenuAction>) {
    if let Some(value) = event.value {
        actions.write(crate::KingdomMenuAction::Color(value));
    }
}

#[html_fn("kingdom_icon_search")]
fn kingdom_icon_search(
    In(event): In<HtmlEvent>,
    mut actions: MessageWriter<crate::KingdomMenuAction>,
) {
    if let Some(value) = event.value {
        actions.write(crate::KingdomMenuAction::SearchIcons(value));
    }
}
