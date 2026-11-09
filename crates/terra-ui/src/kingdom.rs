//! Bergfried management uses the replicated registry and authoritative commands.
use bevy::{
    input_focus::{FocusCause, InputFocus},
    prelude::*,
};
use serde::Serialize;
use terra_core::{
    GameAction, GameMode, PlayerUuid,
    kingdom::{
        BotKingdom, ClaimActor, ClaimKind, DiplomaticRelation, DiplomaticTarget, KingdomHeraldry,
        KingdomIcon, KingdomId, KingdomRecord, KingdomRole, KingdomState, SettlementKind,
    },
};
use terra_network::{
    client::{ClientSession, KingdomRequest},
    protocol::kingdom::{KingdomAction, KingdomOutcome},
};
use terra_player::{CaptureRequest, PlayerInputBlock, PlayerSet};
use tilt_ui::{ElementId, html_shared};

pub(crate) fn emblem_source(icon: KingdomIcon) -> String {
    match icon {
        KingdomIcon::Wheat => concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../src-ui/assets/heraldry-wheat.svg"
        )
        .into(),
        KingdomIcon::Hammer => concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../src-ui/assets/heraldry-hammer.svg"
        )
        .into(),
        _ => format!("tilt-icon:{}@32", icon.glyph()),
    }
}

#[derive(Resource, Default)]
pub(crate) struct KingdomMenuState {
    position: Option<[i64; 3]>,
    resume_pending: bool,
    pub consumed_escape: bool,
    draft: String,
    draft_edited: bool,
    draft_kind: SettlementKind,
    draft_heraldry: KingdomHeraldry,
    appearance_edited: bool,
    invalid_color: bool,
    icon_picker: bool,
    icon_search: String,
    tab: String,
    role_member: Option<PlayerUuid>,
    displayed: Option<KingdomId>,
    pending: Option<(u64, f64)>,
    confirm_dissolve: bool,
    focus_pending: bool,
    error: String,
    status: String,
    claim_preview: Option<ClaimPreview>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ClaimPreviewKey {
    registry_revision: u64,
    settlement: KingdomId,
    actor: PlayerUuid,
    chunk: [i64; 2],
    limits: terra_core::kingdom::ClaimLimits,
    rules: terra_core::kingdom::KingdomRules,
    domains_revision: Option<u64>,
}

#[derive(Clone, Copy)]
struct ClaimPreview {
    key: ClaimPreviewKey,
    allowed: [bool; 2],
}

impl KingdomMenuState {
    pub(crate) fn blocks_input(&self) -> bool {
        self.position.is_some() || self.resume_pending
    }
}

#[derive(Message, Clone, Debug)]
pub enum KingdomMenuAction {
    Open([i64; 3]),
    Close,
    Name(String),
    Kind(SettlementKind),
    Color(String),
    Icon(KingdomIcon),
    OpenIcons,
    CloseIcons,
    SearchIcons(String),
    SaveAppearance,
    Claim(ClaimKind),
    Release,
    Upgrade,
    Found,
    Rename,
    Join,
    Leave,
    Kick(PlayerUuid),
    ConfirmDissolve,
    Dissolve,
    CancelDissolve,
    Tab(String),
    EditRole(PlayerUuid),
    SetRole {
        member: PlayerUuid,
        role: KingdomRole,
    },
    SetRelation {
        target: DiplomaticTarget,
        relation: DiplomaticRelation,
    },
}

#[derive(Clone, Serialize, PartialEq)]
pub struct KingdomMemberView {
    pub id: String,
    pub name: String,
    pub role: String,
    pub can_kick: bool,
    pub can_edit_role: bool,
    pub editing: bool,
    pub permissions: String,
    pub build: bool,
    pub break_blocks: bool,
    pub inventories: bool,
    pub npc_work: bool,
    pub roles: Vec<KingdomChoiceView>,
}

#[derive(Clone, Serialize, PartialEq)]
pub struct KingdomChoiceView {
    pub key: String,
    pub label: String,
    pub icon: String,
    pub selected: bool,
}

#[derive(Clone, Serialize, PartialEq)]
pub struct KingdomCapabilityView {
    pub label: String,
    pub icon: String,
    pub allowed: bool,
}
#[derive(Clone, Serialize, PartialEq)]
pub struct KingdomDiplomacyView {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub emblem: String,
    pub color: String,
    pub relation: String,
    pub relation_key: String,
    pub offered: String,
    pub hint: String,
    pub pending_alliance: bool,
    pub can_neutral: bool,
    pub can_ally: bool,
    pub can_hostile: bool,
}
fn target_key(target: DiplomaticTarget) -> String {
    match target {
        DiplomaticTarget::Player(id) => format!("player:{}", id.0),
        DiplomaticTarget::Bot(bot) => format!("bot:{}", bot.name().to_ascii_lowercase()),
    }
}

#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct KingdomView {
    pub visible: bool,
    pub bound: bool,
    pub title: String,
    pub kingdom_id: String,
    pub location: String,
    pub founder: String,
    pub members: Vec<KingdomMemberView>,
    pub member_count: String,
    pub name: String,
    pub pending: bool,
    pub can_found: bool,
    pub can_rename: bool,
    pub can_join: bool,
    pub can_leave: bool,
    pub founder_controls: bool,
    pub manager_controls: bool,
    pub overview_tab: bool,
    pub members_tab: bool,
    pub diplomacy_tab: bool,
    pub own_role: String,
    pub access_hint: String,
    pub capabilities: Vec<KingdomCapabilityView>,
    pub diplomacy: Vec<KingdomDiplomacyView>,
    pub confirm_dissolve: bool,
    pub membership: String,
    pub status: String,
    pub error: String,
    pub settlement_kind: String,
    pub kinds: Vec<KingdomChoiceView>,
    pub icons: Vec<KingdomChoiceView>,
    pub icon_picker: bool,
    pub icon_search: String,
    pub icon_label: String,
    pub color: String,
    pub emblem_source: String,
    pub emblem_style: String,
    pub can_edit_appearance: bool,
    pub can_save_appearance: bool,
    pub level: String,
    pub protected_claims: String,
    pub territory_claims: String,
    pub current_chunk: String,
    pub current_claim: String,
    pub claim_hint: String,
    pub can_claim_protected: bool,
    pub can_claim_territory: bool,
    pub can_release: bool,
    pub can_upgrade: bool,
    pub next_level: String,
    pub level_gain: String,
    pub parent_name: String,
    pub outposts: String,
    pub upgrade_walls: String,
    pub upgrade_gold: String,
    pub walls_available: bool,
    pub gold_available: bool,
    pub upgrade_cost: String,
    pub upgrade_hint: String,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KingdomInputSet;

pub(crate) fn install(app: &mut App) {
    app.init_resource::<KingdomMenuState>()
        .init_resource::<KingdomView>()
        .add_message::<KingdomMenuAction>()
        .add_message::<KingdomRequest>()
        .add_systems(
            PreUpdate,
            opening_frame
                .in_set(KingdomInputSet)
                .after(bevy::input::InputSystems),
        )
        .add_systems(
            Update,
            update
                .before(crate::menu::MenuInputSet)
                .before(PlayerSet::Movement),
        )
        .add_systems(
            Update,
            retain_input
                .after(crate::survival::SurvivalUiSet)
                .before(PlayerSet::Movement),
        )
        .add_systems(
            PostUpdate,
            focus_name.after(bevy::ui::UiSystems::PostLayout),
        );
}

#[derive(bevy::ecs::system::SystemParam)]
struct InputContext<'w> {
    active: Option<Res<'w, terra_player::interaction::ActiveItem>>,
    session: Option<Res<'w, ClientSession>>,
    menu: Option<Res<'w, crate::menu::MenuState>>,
    inventory: Option<Res<'w, crate::inventory::InventoryState>>,
    chat: Option<Res<'w, crate::chat::ChatState>>,
    pets: Option<Res<'w, crate::pets::PetMenuState>>,
    fullmap: Option<Res<'w, crate::minimap::fullmap::FullMapState>>,
    settings: Option<Res<'w, crate::SettingsView>>,
    camera: Option<Res<'w, terra_player::CameraInputState>>,
    target: Option<Res<'w, terra_player::TargetWorld>>,
    chunks: Option<Res<'w, terra_world::streaming::ChunkStore>>,
}

impl InputContext<'_> {
    fn playing(&self) -> bool {
        self.session.as_ref().is_some_and(|s| {
            s.ready
                && s.error.is_none()
                && !s.dead()
                && !s.teleporting
                && s.game_mode != GameMode::Spectator
                && s.mounted.is_none()
        }) && self
            .menu
            .as_ref()
            .is_none_or(|menu| menu.page == crate::menu::MenuPage::Playing)
    }

    fn unoccupied(&self) -> bool {
        !self.inventory.as_ref().is_some_and(|v| v.blocks_input())
            && !self.chat.as_ref().is_some_and(|v| v.blocks_input())
            && !self.pets.as_ref().is_some_and(|v| v.blocks_input())
            && !self.fullmap.as_ref().is_some_and(|v| v.blocks_input())
            && !self.settings.as_ref().is_some_and(|v| v.open)
    }
}

fn opening_frame(
    input: terra_core::GameplayInput,
    context: InputContext,
    mut state: ResMut<KingdomMenuState>,
    mut actions: MessageWriter<KingdomMenuAction>,
    mut blocked: ResMut<PlayerInputBlock>,
    mut capture: MessageWriter<CaptureRequest>,
) {
    state.consumed_escape = false;
    if state.position.is_some() {
        if input.just_pressed(GameAction::Pause) {
            state.consumed_escape = true;
            actions.write(if state.icon_picker {
                KingdomMenuAction::CloseIcons
            } else {
                KingdomMenuAction::Close
            });
        }
        blocked.0 = true;
        return;
    }
    if state.resume_pending {
        let released = input.keys.get_pressed().next().is_none()
            && input
                .mouse
                .as_ref()
                .is_none_or(|m| m.get_pressed().next().is_none());
        if released {
            state.resume_pending = false;
            if context.playing() && context.unoccupied() {
                blocked.0 = false;
                capture.write(CaptureRequest(true));
            }
        }
        return;
    }
    if !context.playing()
        || !context.unoccupied()
        || crate::inventory::rail_mode_priority(&input, context.active.as_deref())
        || !context.camera.as_ref().is_some_and(|c| c.captured)
        || input.pressed(GameAction::FlyDown)
        || (input.binding(GameAction::FlyDown) == GameAction::FlyDown.default_binding()
            && input.keys.pressed(KeyCode::ShiftRight))
        || !(input.just_pressed(GameAction::Place) || input.just_pressed(GameAction::Use))
    {
        return;
    }
    let Some(terra_player::WorldTarget::Block(hit)) =
        context.target.as_ref().and_then(|target| target.0)
    else {
        return;
    };
    if context
        .chunks
        .as_ref()
        .and_then(|chunks| chunks.block_at(hit.position))
        != Some(terra_core::castle::BERGFRIED)
        || !context
            .session
            .as_ref()
            .is_some_and(|session| in_reach(session, hit.position))
    {
        return;
    }
    // Own the opening click before placement/mining sees the same input frame.
    state.position = Some(hit.position);
    blocked.0 = true;
    capture.write(CaptureRequest(false));
    actions.write(KingdomMenuAction::Open(hit.position));
}

fn ready(world: &World) -> bool {
    world.get_resource::<ClientSession>().is_some_and(|s| {
        s.ready
            && s.error.is_none()
            && !s.dead()
            && !s.teleporting
            && s.game_mode != GameMode::Spectator
            && s.mounted.is_none()
    }) && world
        .get_resource::<crate::menu::MenuState>()
        .is_none_or(|menu| menu.page == crate::menu::MenuPage::Playing)
        && !world
            .get_resource::<crate::SettingsView>()
            .is_some_and(|v| v.open)
}

fn other_panel(world: &World) -> bool {
    world
        .get_resource::<crate::inventory::InventoryState>()
        .is_some_and(|v| v.blocks_input())
        || world
            .get_resource::<crate::chat::ChatState>()
            .is_some_and(|v| v.blocks_input())
        || world
            .get_resource::<crate::pets::PetMenuState>()
            .is_some_and(|v| v.blocks_input())
        || world
            .get_resource::<crate::minimap::fullmap::FullMapState>()
            .is_some_and(|v| v.blocks_input())
}

fn near_keep(world: &World, position: [i64; 3]) -> bool {
    let Some(session) = world.get_resource::<ClientSession>() else {
        return false;
    };
    in_reach(session, position)
        && world
            .get_resource::<terra_world::streaming::ChunkStore>()
            .is_none_or(|chunks| chunks.block_at(position) == Some(terra_core::castle::BERGFRIED))
}

fn in_reach(session: &ClientSession, position: [i64; 3]) -> bool {
    let eye = session
        .confirmed_state()
        .map_or(session.spawn, |p| Vec3::from_array(p.position))
        + Vec3::Y * terra_player::PLAYER_EYE_OFFSET;
    let center = Vec3::from_array(position.map(|v| v as f32 + 0.5));
    eye.distance_squared(center) <= 64.0
}

fn now(world: &World) -> f64 {
    world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |time| time.elapsed_secs_f64())
}

fn close(world: &mut World, resume: bool) {
    if let Some(focus) = world.get_resource::<InputFocus>().and_then(InputFocus::get) {
        let mut node = Some(focus);
        let mut owned = false;
        for _ in 0..32 {
            let Some(entity) = node else { break };
            if world
                .get::<ElementId>(entity)
                .is_some_and(|id| id.0 == "kingdom-dialog")
            {
                owned = true;
                break;
            }
            node = world.get::<ChildOf>(entity).map(ChildOf::parent);
        }
        if owned {
            world.resource_mut::<InputFocus>().clear();
        }
    }
    let consumed_escape = world.resource::<KingdomMenuState>().consumed_escape;
    *world.resource_mut::<KingdomMenuState>() = KingdomMenuState {
        resume_pending: resume,
        consumed_escape,
        ..default()
    };
    *world.resource_mut::<KingdomView>() = default();
}

fn send(world: &mut World, action: KingdomAction) {
    let Some(request) = world
        .resource_mut::<ClientSession>()
        .kingdom_request(action)
    else {
        world.resource_mut::<KingdomMenuState>().error =
            "The kingdom request limit was reached. Reconnect to continue.".into();
        return;
    };
    let expected = request.0.request;
    let started = now(world);
    {
        let mut state = world.resource_mut::<KingdomMenuState>();
        state.pending = Some((expected, started));
        state.error.clear();
        state.status = "Waiting for the server…".into();
    }
    world.write_message(request);
}

fn current(world: &World) -> Option<KingdomRecord> {
    let position = world.resource::<KingdomMenuState>().position?;
    world
        .resource::<ClientSession>()
        .kingdoms
        .active_at(position)
        .cloned()
}

fn player_chunk(session: &ClientSession) -> Option<[i64; 2]> {
    let position = session
        .confirmed_state()
        .map_or(session.spawn, |state| Vec3::from_array(state.position));
    if !position.is_finite() {
        return None;
    }
    let chunk = terra_core::kingdom::claim_chunk(position.to_array().map(|v| v.floor() as i64));
    terra_core::kingdom::valid_claim_chunk(chunk).then_some(chunk)
}

fn picker_color(value: &str) -> Option<[u8; 3]> {
    let color = tilt_ui::tilt_ui_css::parse_color_value(value).ok()?;
    if !color.alpha.is_finite() || (color.alpha - 1.0).abs() > 0.0001 {
        return None;
    }
    let rgb = [color.red, color.green, color.blue];
    rgb.into_iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
        .then(|| rgb.map(|v| (v * 255.0).round() as u8))
}

fn color_text([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn upgrade_materials(
    session: &ClientSession,
    record: &KingdomRecord,
) -> Option<(u32, u32, terra_core::kingdom::KingdomUpgradeCost)> {
    let cost = record.upgrade_cost()?;
    Some((
        u32::from(
            session
                .inventory
                .count(terra_core::ItemKind::Block(terra_core::castle::STONE_WALL)),
        ),
        u32::from(session.inventory.count(terra_core::ItemKind::Mineral(
            terra_core::MineralItem::GoldIngot,
        ))),
        cost,
    ))
}

fn can_afford_upgrade(session: &ClientSession, record: &KingdomRecord) -> bool {
    upgrade_materials(session, record).is_some_and(|(walls, gold, cost)| {
        walls >= u32::from(cost.stone_wall) && gold >= u32::from(cost.gold_ingot)
    })
}

fn open(world: &mut World, position: [i64; 3]) {
    if !ready(world) || other_panel(world) || !near_keep(world, position) {
        close(world, ready(world) && !other_panel(world));
        return;
    }
    *world.resource_mut::<KingdomMenuState>() = KingdomMenuState {
        position: Some(position),
        focus_pending: true,
        ..default()
    };
    world.resource_mut::<PlayerInputBlock>().0 = true;
    world.write_message(CaptureRequest(false));
    send(world, KingdomAction::Inspect { position });
}

fn update(world: &mut World) {
    let actions: Vec<_> = world
        .resource_mut::<Messages<KingdomMenuAction>>()
        .drain()
        .collect();
    for action in actions {
        if let KingdomMenuAction::Open(position) = action {
            open(world, position);
            continue;
        }
        if matches!(action, KingdomMenuAction::Close) {
            let resume = ready(world) && !other_panel(world);
            close(world, resume);
            continue;
        }
        if world.resource::<KingdomMenuState>().position.is_none()
            || world.resource::<KingdomMenuState>().pending.is_some()
        {
            continue;
        }
        if !ready(world)
            || other_panel(world)
            || !near_keep(
                world,
                world.resource::<KingdomMenuState>().position.unwrap(),
            )
        {
            close(world, ready(world) && !other_panel(world));
            continue;
        }
        let record = current(world);
        let player = world.resource::<ClientSession>().uuid;
        let founder = record.as_ref().is_some_and(|k| Some(k.founder) == player);
        let manages = record.as_ref().is_some_and(|k| {
            player.is_some_and(|uuid| {
                world.resource::<ClientSession>().allow_build
                    && world
                        .resource::<ClientSession>()
                        .kingdoms
                        .can_manage(uuid, k.id)
            })
        });
        if let KingdomMenuAction::Tab(ref tab) = action {
            if matches!(tab.as_str(), "overview" | "members" | "diplomacy") {
                world.resource_mut::<KingdomMenuState>().tab = tab.clone();
            }
            continue;
        }
        if let KingdomMenuAction::EditRole(member) = action {
            if manages
                && record.as_ref().is_some_and(|k| {
                    player.is_some_and(|actor| {
                        world.resource::<ClientSession>().kingdoms.can_set_role(
                            actor,
                            k.id,
                            member,
                            KingdomRole::Visitor,
                        )
                    })
                })
            {
                let mut state = world.resource_mut::<KingdomMenuState>();
                state.role_member = if state.role_member == Some(member) {
                    None
                } else {
                    Some(member)
                };
            }
            continue;
        }
        if let KingdomMenuAction::Name(name) = action {
            if record.is_some() && !manages {
                continue;
            }
            let mut state = world.resource_mut::<KingdomMenuState>();
            state.draft = name
                .chars()
                .take(terra_core::kingdom::MAX_KINGDOM_NAME_CHARS)
                .collect();
            state.draft_edited = true;
            state.error.clear();
            continue;
        }
        match action {
            KingdomMenuAction::Kind(kind) => {
                if record.is_none() {
                    world.resource_mut::<KingdomMenuState>().draft_kind = kind;
                }
                continue;
            }
            KingdomMenuAction::Color(ref color) if record.is_none() || manages => {
                let mut state = world.resource_mut::<KingdomMenuState>();
                if let Some(color) = picker_color(color) {
                    state.draft_heraldry.color = color;
                    state.appearance_edited = true;
                    state.invalid_color = false;
                    state.error.clear();
                } else {
                    state.invalid_color = true;
                    state.appearance_edited = true;
                    state.error = "Choose an opaque color for your settlement.".into();
                }
                continue;
            }
            KingdomMenuAction::OpenIcons if record.is_none() || manages => {
                let mut state = world.resource_mut::<KingdomMenuState>();
                state.icon_picker = true;
                state.icon_search.clear();
                state.focus_pending = true;
                continue;
            }
            KingdomMenuAction::CloseIcons => {
                world.resource_mut::<KingdomMenuState>().icon_picker = false;
                continue;
            }
            KingdomMenuAction::SearchIcons(ref search) => {
                world.resource_mut::<KingdomMenuState>().icon_search =
                    search.chars().take(48).collect();
                continue;
            }
            KingdomMenuAction::Icon(icon) if icon.valid() && (record.is_none() || manages) => {
                let mut state = world.resource_mut::<KingdomMenuState>();
                state.draft_heraldry.icon = icon;
                state.icon_picker = false;
                state.appearance_edited = true;
                state.error.clear();
                continue;
            }
            _ => {}
        }
        if matches!(action, KingdomMenuAction::CancelDissolve) {
            world.resource_mut::<KingdomMenuState>().confirm_dissolve = false;
            continue;
        }
        if matches!(action, KingdomMenuAction::ConfirmDissolve) {
            if founder {
                world.resource_mut::<KingdomMenuState>().confirm_dissolve = true;
            }
            continue;
        }
        let draft = world.resource::<KingdomMenuState>().draft.clone();
        let command = match (action, record.as_ref()) {
            (KingdomMenuAction::Found, None) => {
                let session = world.resource::<ClientSession>();
                let kind = world.resource::<KingdomMenuState>().draft_kind;
                let eligible = session.uuid.is_some_and(|actor| {
                    match session.kingdoms.active_for_member(actor) {
                        None => kind == SettlementKind::Kingdom,
                        Some(parent) => {
                            kind != SettlementKind::Kingdom
                                && parent.kind == SettlementKind::Kingdom
                                && session.kingdoms.can_manage(actor, parent.id)
                        }
                    }
                });
                if world.resource::<KingdomMenuState>().invalid_color || !eligible {
                    continue;
                }
                match terra_core::kingdom::normalized_name(&draft) {
                    Ok(name) => Some(KingdomAction::FoundSettlement {
                        position: world.resource::<KingdomMenuState>().position.unwrap(),
                        name,
                        kind: world.resource::<KingdomMenuState>().draft_kind,
                        heraldry: world.resource::<KingdomMenuState>().draft_heraldry,
                    }),
                    Err(error) => {
                        world.resource_mut::<KingdomMenuState>().error = error.to_string();
                        None
                    }
                }
            }
            (KingdomMenuAction::Rename, Some(k)) if manages => {
                match terra_core::kingdom::normalized_name(&draft) {
                    Ok(name) if name != k.name => Some(KingdomAction::Rename { id: k.id, name }),
                    Ok(_) => None,
                    Err(error) => {
                        world.resource_mut::<KingdomMenuState>().error = error.to_string();
                        None
                    }
                }
            }
            (KingdomMenuAction::Dissolve, Some(k))
                if founder && world.resource::<KingdomMenuState>().confirm_dissolve =>
            {
                Some(KingdomAction::Dissolve { id: k.id })
            }
            (KingdomMenuAction::Join, Some(k))
                if player.is_some_and(|uuid| {
                    world
                        .resource::<ClientSession>()
                        .kingdoms
                        .active_for_member(uuid)
                        .is_none()
                }) =>
            {
                Some(KingdomAction::Join { id: k.id })
            }
            (KingdomMenuAction::Leave, Some(k))
                if !founder && player.is_some_and(|uuid| k.members.contains(&uuid)) =>
            {
                Some(KingdomAction::Leave { id: k.id })
            }
            (KingdomMenuAction::Kick(member), Some(k))
                if manages
                    && player.is_some_and(|actor| {
                        world.resource::<ClientSession>().kingdoms.can_set_role(
                            actor,
                            k.id,
                            member,
                            KingdomRole::Visitor,
                        )
                    }) =>
            {
                Some(KingdomAction::Kick { id: k.id, member })
            }
            (KingdomMenuAction::SaveAppearance, Some(k))
                if manages && !world.resource::<KingdomMenuState>().invalid_color =>
            {
                let heraldry = world.resource::<KingdomMenuState>().draft_heraldry;
                (heraldry != k.heraldry)
                    .then_some(KingdomAction::ChangeAppearance { id: k.id, heraldry })
            }
            (KingdomMenuAction::Claim(kind), Some(k)) if manages => {
                let session = world.resource::<ClientSession>();
                if let (Some(chunk), Some(actor)) = (player_chunk(session), session.uuid) {
                    let rules = world
                        .get_resource::<terra_core::GameConfig>()
                        .map_or_else(Default::default, |config| config.world.kingdom_rules);
                    let mut preview = session.kingdoms.clone();
                    match preview.claim_with_rules(
                        k.id,
                        chunk,
                        kind,
                        ClaimActor::Player(actor),
                        &rules,
                        &[],
                    ) {
                        Ok(()) => Some(KingdomAction::ClaimChunk {
                            id: k.id,
                            chunk,
                            kind,
                        }),
                        Err(error) => {
                            world.resource_mut::<KingdomMenuState>().error = error.to_string();
                            None
                        }
                    }
                } else {
                    None
                }
            }
            (KingdomMenuAction::Release, Some(k)) if manages => {
                let session = world.resource::<ClientSession>();
                player_chunk(session)
                    .filter(|chunk| {
                        *chunk != terra_core::kingdom::claim_chunk(k.location)
                            && session
                                .kingdoms
                                .claim_at(*chunk)
                                .is_some_and(|claim| claim.kingdom == k.id)
                    })
                    .map(|chunk| KingdomAction::ReleaseChunk { id: k.id, chunk })
            }
            (KingdomMenuAction::SetRole { member, role }, Some(k))
                if manages
                    && player.is_some_and(|actor| {
                        world
                            .resource::<ClientSession>()
                            .kingdoms
                            .can_set_role(actor, k.id, member, role)
                    }) =>
            {
                Some(KingdomAction::SetRole {
                    id: k.id,
                    member,
                    role,
                })
            }
            (KingdomMenuAction::SetRelation { target, relation }, Some(k))
                if manages
                    && relation != DiplomaticRelation::Own
                    && target != DiplomaticTarget::Player(k.id)
                    && !matches!(
                        (target, relation),
                        (
                            DiplomaticTarget::Bot(BotKingdom::Petora),
                            DiplomaticRelation::Allied
                        )
                    ) =>
            {
                let valid = match target {
                    DiplomaticTarget::Player(other) => world
                        .resource::<ClientSession>()
                        .kingdoms
                        .get(other)
                        .is_some_and(|r| r.state == KingdomState::Active),
                    DiplomaticTarget::Bot(_) => true,
                };
                valid.then_some(KingdomAction::SetRelation {
                    id: k.id,
                    target,
                    relation,
                })
            }
            (KingdomMenuAction::Upgrade, Some(k)) if manages => {
                can_afford_upgrade(world.resource::<ClientSession>(), k)
                    .then_some(KingdomAction::Upgrade { id: k.id })
            }
            _ => None,
        };
        if let Some(command) = command {
            world.resource_mut::<KingdomMenuState>().confirm_dissolve = false;
            send(world, command);
        }
    }

    let Some(position) = world.resource::<KingdomMenuState>().position else {
        return;
    };
    if !ready(world) || other_panel(world) || !near_keep(world, position) {
        let resume = ready(world) && !other_panel(world);
        close(world, resume);
        return;
    }
    let pending = world.resource::<KingdomMenuState>().pending;
    let result = world
        .resource::<ClientSession>()
        .last_kingdom_result
        .clone()
        .filter(|result| pending.is_some_and(|(expected, _)| result.request == expected));
    if let Some(result) = result {
        let mut state = world.resource_mut::<KingdomMenuState>();
        state.pending = None;
        state.status.clear();
        match result.outcome {
            KingdomOutcome::Rejected(error) => state.error = error.to_string(),
            KingdomOutcome::Inspected { .. } => {}
            KingdomOutcome::Founded { .. } => {
                state.status = "Settlement founded.".into();
                state.draft_edited = false;
                state.appearance_edited = false;
            }
            KingdomOutcome::Renamed { .. } => {
                state.status = "Settlement renamed.".into();
                state.draft_edited = false;
            }
            KingdomOutcome::Dissolved { .. } => state.status = "Settlement dissolved.".into(),
            KingdomOutcome::Joined { .. } => state.status = "You joined this settlement.".into(),
            KingdomOutcome::Left { .. } => state.status = "You left this settlement.".into(),
            KingdomOutcome::Kicked { .. } => state.status = "Member removed.".into(),
            KingdomOutcome::RoleChanged { role, .. } => {
                state.status = format!("Member role saved: {}.", role.label());
                state.role_member = None;
            }
            KingdomOutcome::RelationChanged { relation, .. } => {
                state.status = format!(
                    "Diplomatic stance saved: {}. Alliances require acceptance.",
                    relation.label()
                )
            }
            KingdomOutcome::Claimed { kind, .. } => {
                state.status = format!("{} claim saved.", kind.label())
            }
            KingdomOutcome::ClaimsApplied { count, .. } => {
                state.status = format!("{count} claims saved.")
            }
            KingdomOutcome::Released { .. } => state.status = "Chunk released.".into(),
            KingdomOutcome::AppearanceChanged { .. } => {
                state.status = "Settlement appearance saved.".into();
                state.appearance_edited = false;
            }
            KingdomOutcome::Upgraded { level, .. } => {
                state.status = format!("Settlement upgraded to level {level}.");
            }
        }
    } else if pending.is_some_and(|(_, started)| now(world) - started > 10.0) {
        let mut state = world.resource_mut::<KingdomMenuState>();
        state.pending = None;
        state.status.clear();
        state.error =
            "The server did not answer. Close and reopen the Bergfried to refresh.".into();
    }
    sync_view(world, position);
}

fn member_name(world: &World, record: &KingdomRecord, uuid: PlayerUuid) -> String {
    if let Some((_, name)) = record.member_names.iter().find(|(id, _)| *id == uuid) {
        return name.clone();
    }
    if world.get_resource::<ClientSession>().and_then(|s| s.uuid) == Some(uuid)
        && let Some(profile) = world.get_resource::<terra_core::PlayerProfile>()
    {
        return profile.name.clone();
    }
    format!("Player {}", &uuid.simple().to_string()[..8])
}

fn permission_summary(role: KingdomRole) -> String {
    let p = role.permissions();
    let permissions = [
        ("Build", p.build),
        ("Break", p.break_blocks),
        ("Inventories", p.inventories),
        ("NPC Work", p.npc_work),
    ];
    let allowed: Vec<_> = permissions
        .into_iter()
        .filter_map(|(name, allowed)| allowed.then_some(name))
        .collect();
    if allowed.is_empty() {
        "No protected-land permissions".into()
    } else {
        allowed.join(" · ")
    }
}
fn diplomacy_hint(
    registry: &terra_core::kingdom::KingdomRegistry,
    source: &KingdomRecord,
    target: DiplomaticTarget,
    effective: DiplomaticRelation,
    offered: DiplomaticRelation,
) -> String {
    match target {
        DiplomaticTarget::Player(other)=>{
            let theirs=registry.relation_offer(other,DiplomaticTarget::Player(source.id));
            if effective==DiplomaticRelation::Allied {"Both realms accepted. Authorized members share inventories, NPC work and transport; building stays private."}
            else if theirs==DiplomaticRelation::Hostile {"They declared Hostile. Their stance must change before peace or an alliance can take effect."}
            else if offered==DiplomaticRelation::Hostile {"Your hostile stance blocks all shared access. Choose Neutral to withdraw it."}
            else if offered==DiplomaticRelation::Allied {"Alliance offer pending: their Founder or Officer must also offer an alliance."}
            else if theirs==DiplomaticRelation::Allied {"They offered an alliance. Offer an alliance here to accept it."}
            else {"Neutral realms do not share protected storage, NPC work or transport. Alliances need both realms' consent."}
        }
        DiplomaticTarget::Bot(BotKingdom::Petora)=>"Petora remains hostile: no building, mining or interactions; alliances are not accepted.",
        DiplomaticTarget::Bot(_) if effective==DiplomaticRelation::Hostile=>"Hostile: no building, mining or interactions. Withdraw your hostile stance to restore the native attitude.",
        DiplomaticTarget::Bot(_) if effective==DiplomaticRelation::Allied=>"Friendly through alliance: full building, mining and interaction access, including transport.",
        DiplomaticTarget::Bot(BotKingdom::Helteria)=>"Friendly: full building, mining and interaction access. Cross-realm transport needs an alliance.",
        DiplomaticTarget::Bot(_)=>if source.level<2 {"Neutral: doors and windows only. Alliances need level 2; earlier offers stay pending."} else {"Neutral: doors and windows only. Offer an alliance for full friendly access."},
    }.into()
}

fn claim_availability(
    world: &mut World,
    record: Option<&KingdomRecord>,
    rules: terra_core::kingdom::KingdomRules,
) -> [bool; 2] {
    let session = world.resource::<ClientSession>();
    let (Some(record), Some(actor), Some(chunk)) = (record, session.uuid, player_chunk(session))
    else {
        return [false; 2];
    };
    if world.resource::<KingdomMenuState>().pending.is_some()
        || !session.allow_build
        || !session.kingdoms.can_manage(actor, record.id)
    {
        return [false; 2];
    }
    let known = world.get_resource::<crate::settlement_map::SettlementMap>();
    let key = ClaimPreviewKey {
        registry_revision: session.kingdoms.revision,
        settlement: record.id,
        actor,
        chunk,
        limits: session.kingdoms.limits_with_rules(record.id, &rules),
        rules,
        domains_revision: known.map(|known| known.revision()),
    };
    if let Some(preview) = world.resource::<KingdomMenuState>().claim_preview
        && preview.key == key
    {
        return preview.allowed;
    }
    let domains = known
        .into_iter()
        .flat_map(|known| known.domains().copied())
        .collect::<Vec<_>>();
    let allowed = [ClaimKind::ProtectedBuilding, ClaimKind::RealmTerritory].map(|kind| {
        let mut preview = session.kingdoms.clone();
        preview
            .claim_batch_with_rules(
                record.id,
                &[(chunk, kind)],
                ClaimActor::Player(actor),
                &rules,
                &domains,
            )
            .is_ok_and(|changed| changed != 0)
    });
    world.resource_mut::<KingdomMenuState>().claim_preview = Some(ClaimPreview { key, allowed });
    allowed
}

fn sync_view(world: &mut World, position: [i64; 3]) {
    let record = current(world);
    if let Some(k) = record.as_ref() {
        let editable = {
            let session = world.resource::<ClientSession>();
            session
                .uuid
                .is_some_and(|uuid| session.allow_build && session.kingdoms.can_manage(uuid, k.id))
        };
        let mut state = world.resource_mut::<KingdomMenuState>();
        if !editable {
            state.icon_picker = false;
            state.role_member = None;
        }
        let changed = state.displayed != Some(k.id);
        if changed || !state.draft_edited || !editable {
            state.draft = k.name.clone();
        }
        if changed || !state.appearance_edited || !editable {
            state.draft_kind = k.kind;
            state.draft_heraldry = k.heraldry;
            state.invalid_color = false;
        }
        state.displayed = Some(k.id);
    } else {
        world.resource_mut::<KingdomMenuState>().displayed = None;
    }
    let rules = world
        .get_resource::<terra_core::GameConfig>()
        .map_or_else(Default::default, |config| config.world.kingdom_rules);
    let [can_claim_protected, can_claim_territory] =
        claim_availability(world, record.as_ref(), rules);
    let session = world.resource::<ClientSession>();
    let uuid = session.uuid;
    let membership = uuid.and_then(|id| session.kingdoms.active_for_member(id));
    let founder = record.as_ref().is_some_and(|k| Some(k.founder) == uuid);
    let manages = record.as_ref().is_some_and(|k| {
        uuid.is_some_and(|actor| session.allow_build && session.kingdoms.can_manage(actor, k.id))
    });
    let role = record
        .as_ref()
        .and_then(|k| uuid.and_then(|actor| session.kingdoms.role_for(k.id, actor)));
    let member = record
        .as_ref()
        .is_some_and(|k| uuid.is_some_and(|id| k.members.contains(&id)));
    let state = world.resource::<KingdomMenuState>();
    let pending = state.pending.is_some();
    let name_valid = terra_core::kingdom::normalized_name(&state.draft).is_ok();
    let chunk = player_chunk(session);
    let claim = chunk.and_then(|chunk| {
        record
            .as_ref()
            .and_then(|record| {
                session
                    .kingdoms
                    .claims
                    .iter()
                    .find(|claim| claim.kingdom == record.id && claim.chunk == chunk)
            })
            .or_else(|| session.kingdoms.claim_at(chunk))
    });
    let counts = record
        .as_ref()
        .map(|k| session.kingdoms.claim_counts(k.id))
        .unwrap_or_default();
    let limits = record
        .as_ref()
        .map(|record| session.kingdoms.limits_with_rules(record.id, &rules))
        .unwrap_or_default();
    let at_base = record
        .as_ref()
        .is_some_and(|k| chunk == Some(terra_core::kingdom::claim_chunk(k.location)));
    let owned_claim = record
        .as_ref()
        .is_some_and(|k| claim.is_some_and(|claim| claim.kingdom == k.id));
    let current_claim = claim.map_or_else(
        || "Unclaimed land".into(),
        |claim| {
            let owner = session.kingdoms.get(claim.kingdom);
            let owner_name = owner.map_or_else(
                || format!("Settlement #{}", claim.kingdom),
                |k| k.name.clone(),
            );
            let actor = match claim.actor {
                ClaimActor::Player(id) => owner.map_or_else(
                    || format!("Player {}", &id.simple().to_string()[..8]),
                    |k| member_name(world, k, id),
                ),
                ClaimActor::Bot { id, authorized_by } => owner.map_or_else(
                    || format!("Bot #{id}"),
                    |k| format!("Bot #{id} · {}", member_name(world, k, authorized_by)),
                ),
            };
            format!("{} · {owner_name} · {actor}", claim.kind.label())
        },
    );
    let can_found_kind = uuid.is_some_and(|actor| match membership {
        None => state.draft_kind == SettlementKind::Kingdom,
        Some(parent) => {
            state.draft_kind != SettlementKind::Kingdom
                && parent.kind == SettlementKind::Kingdom
                && session.kingdoms.can_manage(actor, parent.id)
                && (state.draft_kind != SettlementKind::City
                    || parent.level >= rules.city_min_kingdom_level)
                && (state.draft_kind != SettlementKind::Outpost
                    || session
                        .kingdoms
                        .kingdoms
                        .iter()
                        .filter(|child| {
                            child.parent == Some(parent.id)
                                && child.kind == SettlementKind::Outpost
                                && child.state == KingdomState::Active
                        })
                        .count()
                        < usize::from(parent.level) * usize::from(rules.outposts_per_level))
        }
    });
    let inherited_level = record
        .as_ref()
        .is_some_and(|k| k.parent.is_some() && rules.settlements_follow_kingdom_level);
    let upgrade = record
        .as_ref()
        .filter(|_| !inherited_level)
        .and_then(|k| upgrade_materials(session, k));
    let can_upgrade = manages
        && !record.as_ref().is_some_and(|record| {
            record.parent.is_some() && rules.settlements_follow_kingdom_level
        })
        && !pending
        && record
            .as_ref()
            .is_some_and(|k| can_afford_upgrade(session, k));
    let color = color_text(state.draft_heraldry.color);
    let icon_search = state.icon_search.trim().to_ascii_lowercase();
    let members = record.as_ref().map_or_else(Vec::new, |k| {
        k.members
            .iter()
            .map(|id| {
                let role = session
                    .kingdoms
                    .role_for(k.id, *id)
                    .unwrap_or(KingdomRole::Visitor);
                let editable = manages
                    && uuid.is_some_and(|actor| {
                        session
                            .kingdoms
                            .can_set_role(actor, k.id, *id, KingdomRole::Visitor)
                    });
                KingdomMemberView {
                    id: id.to_string(),
                    name: member_name(world, k, *id),
                    role: role.label().into(),
                    can_kick: editable && !pending,
                    can_edit_role: editable && !pending,
                    editing: editable && state.role_member == Some(*id),
                    permissions: permission_summary(role),
                    build: role.permissions().build,
                    break_blocks: role.permissions().break_blocks,
                    inventories: role.permissions().inventories,
                    npc_work: role.permissions().npc_work,
                    roles: KingdomRole::ALL
                        .into_iter()
                        .filter(|&choice| {
                            manages
                                && uuid.is_some_and(|actor| {
                                    session.kingdoms.can_set_role(actor, k.id, *id, choice)
                                })
                        })
                        .map(|choice| KingdomChoiceView {
                            key: choice.key().into(),
                            label: choice.label().into(),
                            icon: String::new(),
                            selected: choice == role,
                        })
                        .collect(),
                }
            })
            .collect()
    });
    let capabilities = role
        .map(|role| {
            let p = role.permissions();
            [
                ("Build", "hammer", p.build),
                ("Break", "pickaxe", p.break_blocks),
                ("Inventories", "package", p.inventories),
                ("NPC Work", "users", p.npc_work),
            ]
            .into_iter()
            .map(|(label, icon, allowed)| KingdomCapabilityView {
                label: label.into(),
                icon: format!("tilt-icon:{icon}@24"),
                allowed,
            })
            .collect()
        })
        .unwrap_or_default();
    let diplomacy = record.as_ref().map_or_else(Vec::new, |source| {
        let players = session
            .kingdoms
            .kingdoms
            .iter()
            .filter(|k| {
                k.state == KingdomState::Active
                    && k.parent.is_none()
                    && session.kingdoms.realm_id(k.id) != session.kingdoms.realm_id(source.id)
            })
            .map(|k| {
                (
                    DiplomaticTarget::Player(k.id),
                    k.name.clone(),
                    k.kind.label().to_string(),
                    k.heraldry,
                )
            });
        let bots = [
            BotKingdom::Helteria,
            BotKingdom::Lutera,
            BotKingdom::Betross,
            BotKingdom::Petora,
        ]
        .into_iter()
        .map(|bot| {
            (
                DiplomaticTarget::Bot(bot),
                bot.name().to_string(),
                "NPC Kingdom".to_string(),
                bot.heraldry(),
            )
        });
        players
            .chain(bots)
            .map(|(target, name, kind, heraldry)| {
                let effective = session.kingdoms.relation_to(source.id, target);
                let offered = session.kingdoms.relation_offer(source.id, target);
                let hint = diplomacy_hint(&session.kingdoms, source, target, effective, offered);
                KingdomDiplomacyView {
                    key: target_key(target),
                    name,
                    kind,
                    emblem: emblem_source(heraldry.icon),
                    color: format!("color: {};", color_text(heraldry.color)),
                    relation: effective.label().into(),
                    relation_key: effective.key().into(),
                    offered: offered.label().into(),
                    hint,
                    pending_alliance: offered == DiplomaticRelation::Allied
                        && effective != DiplomaticRelation::Allied,
                    can_neutral: manages && !pending && offered != DiplomaticRelation::Neutral,
                    can_ally: manages
                        && !pending
                        && offered != DiplomaticRelation::Allied
                        && target != DiplomaticTarget::Bot(BotKingdom::Petora),
                    can_hostile: manages && !pending && offered != DiplomaticRelation::Hostile,
                }
            })
            .collect()
    });
    let next = KingdomView {
        visible: true,
        bound: record.is_some(),
        title: record.as_ref().map_or_else(
            || format!("Found a {}", state.draft_kind.label()),
            |k| k.name.clone(),
        ),
        kingdom_id: record
            .as_ref()
            .map_or_else(String::new, |k| format!("#{} · Active", k.id)),
        location: format!(
            "Bergfried · {} / {} / {}",
            position[0], position[1], position[2]
        ),
        founder: record
            .as_ref()
            .map_or_else(String::new, |k| member_name(world, k, k.founder)),
        member_count: record.as_ref().map_or_else(String::new, |k| {
            format!(
                "{} / {}",
                k.members.len(),
                terra_core::kingdom::MAX_KINGDOM_MEMBERS
            )
        }),
        members,
        name: state.draft.clone(),
        pending,
        can_found: record.is_none()
            && uuid.is_some()
            && can_found_kind
            && name_valid
            && !state.invalid_color
            && !pending,
        can_rename: manages
            && name_valid
            && !pending
            && record
                .as_ref()
                .is_some_and(|k| state.draft.trim() != k.name),
        can_join: record.is_some() && membership.is_none() && uuid.is_some() && !pending,
        can_leave: member && !founder && !pending,
        founder_controls: founder && session.allow_build,
        manager_controls:manages,overview_tab:!matches!(state.tab.as_str(),"members"|"diplomacy"),members_tab:state.tab=="members",diplomacy_tab:state.tab=="diplomacy",
        own_role:role.map_or("Outsider",KingdomRole::label).into(),
        access_hint:if manages {"Founder and Officers manage claims, roles and relations. Officers cannot change peers or grant Officer."}else{"Management: Founder / Officer. New joins: Visitor. Legacy members: Member."}.into(),capabilities,diplomacy,
        confirm_dissolve: state.confirm_dissolve,
        membership: membership.map_or_else(
            || "You do not belong to a settlement.".into(),
            |k| format!("Your settlement: {}", k.name),
        ),
        status: state.status.clone(),
        error: if state.invalid_color {
            "Choose an opaque color for your settlement.".into()
        } else {
            state.error.clone()
        },
        settlement_kind: state.draft_kind.label().into(),
        kinds: SettlementKind::ALL
            .into_iter()
            .map(|kind| KingdomChoiceView {
                key: kind.key().into(),
                label: kind.label().into(),
                icon: format!(
                    "tilt-icon:{}@32",
                    match kind {
                        SettlementKind::Kingdom => "crown",
                        SettlementKind::Village => "trees",
                        SettlementKind::City => "landmark",
                        SettlementKind::Outpost => "flag",
                    }
                ),
                selected: kind == state.draft_kind,
            })
            .collect(),
        icon_picker: state.icon_picker,
        icon_search: state.icon_search.clone(),
        icon_label: state.draft_heraldry.icon.label().replace('-', " "),
        icons: if state.icon_picker {
            KingdomIcon::catalog()
                .filter(|icon| icon.key().contains(&icon_search))
                .map(|icon| KingdomChoiceView {
                    key: icon.key().into(),
                    label: icon.label().into(),
                    icon: emblem_source(icon),
                    selected: icon == state.draft_heraldry.icon,
                })
                .collect()
        } else {
            Vec::new()
        },
        emblem_source: emblem_source(state.draft_heraldry.icon),
        emblem_style: format!("color: {color};"),
        color,
        can_edit_appearance: (record.is_none() || manages) && !pending,
        can_save_appearance: manages
            && !pending
            && !state.invalid_color
            && record
                .as_ref()
                .is_some_and(|k| k.heraldry != state.draft_heraldry),
        level: record
            .as_ref()
            .map_or_else(|| "Level 1".into(), |k|format!("Level {}{}",if rules.settlements_follow_kingdom_level {session.kingdoms.get(session.kingdoms.realm_id(k.id)).map_or(k.level,|parent|parent.level)}else{k.level},if k.parent.is_some()&&rules.settlements_follow_kingdom_level {" · Kingdom"}else{""})),
        protected_claims: format!("{} / {}", counts.protected, limits.protected),
        territory_claims: format!("{} / {}", counts.territory, limits.territory),
        current_chunk: chunk.map_or_else(
            || "Waiting for player position".into(),
            |[x, z]| {
                format!(
                    "Current chunk · {x} / {z} · {} × {} blocks",
                    terra_core::kingdom::CLAIM_CHUNK_SIZE,
                    terra_core::kingdom::CLAIM_CHUNK_SIZE
                )
            },
        ),
        current_claim,
        claim_hint: if !manages {
            "A Founder or Officer manages claims. Joining grants Visitor access until promoted."
        } else if at_base && owned_claim {
            "The Bergfried chunk stays protected until the settlement is dissolved."
        } else if claim.is_some() && !owned_claim {
            "This chunk belongs to another settlement."
        } else if rules.require_adjacent_claims {
            "Build claims connect through build land to the Bergfried; territory can adjoin any own land. Releases must preserve these connections."
        } else {
            "Adjacency is disabled for this world; the other claim limits still apply."
        }
        .into(),
        can_claim_protected,
        can_claim_territory,
        can_release: manages && owned_claim && !at_base && !pending,
        can_upgrade,
        next_level: if inherited_level {
            "Königreich".into()
        } else {
            record.as_ref().map_or_else(|| "Level 2".into(), |k| {
                if k.level < terra_core::kingdom::MAX_SETTLEMENT_LEVEL {
                    format!("Level {}", k.level + 1)
                } else {
                    "Max.".into()
                }
            })
        },
        level_gain: {
            let kind = record.as_ref().map_or(state.draft_kind, |k| k.kind);
            let (build, territory) = match kind {
                SettlementKind::Kingdom => (rules.kingdom_build_per_level,rules.kingdom_territory_per_level),
                SettlementKind::Outpost => (rules.outpost_build_per_level,rules.outpost_territory_per_level),
                SettlementKind::Village => (rules.village_build_per_level,0),
                SettlementKind::City => (rules.city_build_per_level,0),
            };
            format!("Bau +{build} · Territorium +{territory}")
        },
        parent_name: record.as_ref().and_then(|k| k.parent).and_then(|id| session.kingdoms.get(id)).or_else(|| if record.is_none() { membership } else {None}).map_or_else(|| "Eigenständig".into(), |parent| parent.name.clone()),
        outposts: record.as_ref().map_or_else(|| "0 / 0".into(), |k| {
            let parent = session.kingdoms.realm_id(k.id);
            let count = session.kingdoms.kingdoms.iter().filter(|child| child.parent == Some(parent) && child.kind == SettlementKind::Outpost && child.state == KingdomState::Active).count();
            let max = session.kingdoms.get(parent).map_or(k.level, |parent| parent.level) * rules.outposts_per_level;
            format!("{count} / {max}")
        }),
        upgrade_walls: upgrade.map_or_else(|| "—".into(), |(have,_,cost)| format!("{have} / {}",cost.stone_wall)),
        upgrade_gold: upgrade.map_or_else(|| "—".into(), |(_,have,cost)| format!("{have} / {}",cost.gold_ingot)),
        walls_available: upgrade.is_some_and(|(have,_,cost)| have >= u32::from(cost.stone_wall)),
        gold_available: upgrade.is_some_and(|(_,have,cost)| have >= u32::from(cost.gold_ingot)),
        upgrade_cost: upgrade.map_or_else(
            || "Maximum level reached".into(),
            |(walls, gold, cost)| {
                format!(
                    "Stone Walls {walls} / {} · Gold Ingots {gold} / {}",
                    cost.stone_wall, cost.gold_ingot
                )
            },
        ),
        upgrade_hint: if inherited_level {
            "This settlement follows its kingdom level. Upgrade the parent kingdom."
        } else if !manages {
            "Only the Founder or an Officer can upgrade this settlement."
        } else if upgrade.is_none() {
            "Your settlement has reached the maximum level."
        } else if can_upgrade {
            "Upgrading consumes the listed materials from your inventory."
        } else {
            "Collect the listed materials to unlock the next level."
        }
        .into(),
    };
    if *world.resource::<KingdomView>() != next {
        *world.resource_mut::<KingdomView>() = next;
    }
}

fn retain_input(state: Res<KingdomMenuState>, mut blocked: ResMut<PlayerInputBlock>) {
    if state.blocks_input() {
        blocked.0 = true;
    }
}

fn focus_name(
    mut state: ResMut<KingdomMenuState>,
    view: Res<KingdomView>,
    mut focus: Option<ResMut<InputFocus>>,
    nodes: Query<(Entity, &ElementId, &ComputedNode)>,
) {
    if !state.focus_pending || !view.visible || view.pending {
        return;
    }
    let target = if state.icon_picker {
        "kingdom-icon-search"
    } else {
        "kingdom-name"
    };
    if let Some((entity, _, _)) = nodes
        .iter()
        .find(|(_, id, node)| id.0 == target && node.size().min_element() > 0.0)
    {
        if let Some(focus) = focus.as_mut() {
            focus.set(entity, FocusCause::Navigated);
        }
        state.focus_pending = false;
    } else if view.bound && !view.manager_controls {
        state.focus_pending = false;
    }
}

#[cfg(test)]
#[path = "kingdom_tests.rs"]
mod tests;
