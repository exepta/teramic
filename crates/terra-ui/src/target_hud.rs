//! Read-only target presentation driven by the same picking and authority state as gameplay.
use bevy::prelude::*;
use serde::Serialize;
use terra_core::{
    ItemKind, ResourcePropKind, ToolKind,
    block_properties::{HudStats, StatDisplay, ToolClass},
};
use terra_network::client::ReplicatedCreatures;
use terra_player::{
    TargetWorld, WorldTarget,
    interaction::{ActiveItem, BlockPalette, MiningVisual, TargetCreature},
};
use tilt_ui::html_shared;
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct StatRow {
    pub id: String,
    pub label: String,
    pub text: String,
    pub bar: bool,
    pub value: f32,
}
#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct TargetView {
    pub style: String,
    pub visible: bool,
    pub title: String,
    pub identifier: String,
    pub mob: bool,
    pub status: String,
    pub health: f32,
    pub health_label: String,
    pub hand: bool,
    pub tool_preview: String,
    pub tool_label: String,
    pub harvest: bool,
    pub mining: bool,
    pub progress: f32,
    pub stats: Vec<StatRow>,
}
#[derive(Resource, Default)]
struct Presentations(std::collections::BTreeMap<ResourcePropKind, HudStats>);
pub(crate) fn install(app: &mut App) {
    app.init_resource::<TargetView>()
        .init_resource::<Presentations>()
        .add_systems(Startup, load)
        .add_systems(
            Update,
            update
                .run_if(crate::cadence::ready)
                .after(terra_player::PlayerSet::Targeting),
        );
}
fn load(mut p: ResMut<Presentations>) {
    match terra_core::block_properties::prop_hud_stats() {
        Ok(rows) => p.0 = rows,
        Err(error) => warn!(%error,"Cannot load target stat styles"),
    }
}
#[derive(bevy::ecs::system::SystemParam)]
struct Scene<'w> {
    session: Option<Res<'w, terra_network::client::ClientSession>>,
    layout: Option<Res<'w, crate::hud_layout::HudWidgetStyles>>,
    time: Res<'w, Time>,
    debug: Res<'w, terra_core::DebugSettings>,
    menu: Option<Res<'w, crate::menu::MenuState>>,
    inventory: Res<'w, crate::inventory::InventoryState>,
    settings: Res<'w, crate::SettingsView>,
    chat: Res<'w, crate::chat::ChatState>,
    target: Res<'w, TargetWorld>,
    creature: Res<'w, TargetCreature>,
    mining: Res<'w, MiningVisual>,
    active: Res<'w, ActiveItem>,
    palette: Res<'w, BlockPalette>,
    chunks: Res<'w, terra_world::streaming::ChunkStore>,
    properties: Res<'w, terra_core::block_properties::BlockPropertiesCatalog>,
    animals: Option<Res<'w, ReplicatedCreatures>>,
    styles: Res<'w, Presentations>,
    access: Res<'w, terra_world::streaming::WorldAccess>,
}
fn tool(
    properties: terra_core::block_properties::BlockProperties,
    held: Option<ItemKind>,
) -> (bool, String, String, bool) {
    let required = match properties.required_tool {
        None => None,
        Some(ToolClass::Pickaxe) => Some(match properties.mining_level {
            0 | 1 => ToolKind::WoodPickaxe,
            2 => ToolKind::StonePickaxe,
            _ => ToolKind::IronPickaxe,
        }),
        Some(ToolClass::Axe) => Some(ToolKind::StoneAxe),
        Some(ToolClass::Shovel) => Some(ToolKind::StoneShovel),
    };
    let hand = required.is_none();
    let label = required.map_or("Hand / Level 0".into(), |t| {
        format!(
            "{} / Level {}",
            t.slug().replace('_', " "),
            properties.mining_level
        )
    });
    (
        hand,
        required
            .map(|t| format!("Item preview {}", ItemKind::Tool(t).identifier()))
            .unwrap_or_default(),
        label,
        properties.can_harvest(held),
    )
}
fn stat(
    rows: &mut Vec<StatRow>,
    id: &str,
    label: &str,
    value: f32,
    max: f32,
    style: Option<StatDisplay>,
) {
    if let Some(style) = style {
        rows.push(StatRow {
            id: id.into(),
            label: label.into(),
            text: format!("{} / {}", value.ceil() as u16, max as u16),
            bar: style == StatDisplay::Bar,
            value: (value / max.max(1.0) * 100.0).clamp(0.0, 100.0),
        });
    }
}
fn update(scene: Scene, mut view: ResMut<TargetView>, mut next: Local<f32>) {
    let allowed = !scene.debug.enabled
        && scene.access.ready()
        && scene
            .menu
            .as_ref()
            .is_none_or(|m| m.world_hud_active() && m.page != crate::menu::MenuPage::HudLayout);
    if !allowed {
        if scene
            .menu
            .as_ref()
            .is_some_and(|m| m.page == crate::menu::MenuPage::HudLayout)
        {
            *view = TargetView {
                visible: true,
                title: "Block / Entity".into(),
                identifier: "teramic::stone".into(),
                hand: true,
                harvest: true,
                tool_label: "Hand / Level 0".into(),
                style: scene
                    .layout
                    .as_ref()
                    .map_or_else(String::new, |s| s.target.clone()),
                ..Default::default()
            };
            return;
        }
        if view.visible {
            view.visible = false;
        }
        return;
    }
    if (scene.inventory.open
        || scene.settings.open
        || scene.chat.blocks_input()
        || scene
            .menu
            .as_ref()
            .is_some_and(|m| m.page != crate::menu::MenuPage::Playing))
        && view.visible
    {
        return;
    }
    if scene.time.elapsed_secs() < *next {
        return;
    }
    *next = scene.time.elapsed_secs() + 0.05;
    let mut v = TargetView {
        style: scene
            .layout
            .as_ref()
            .map_or_else(String::new, |s| s.target.clone()),
        ..Default::default()
    };
    if let Some(a) = scene
        .creature
        .0
        .and_then(|id| scene.animals.as_ref()?.creatures.get(&id))
        .map(|t| &t.current)
    {
        use terra_core::creatures::CreatureBehavior as B;
        v.visible = true;
        v.mob = true;
        v.title = terra_core::creatures::definitions::get(a.kind)
            .variant_title(a.variant)
            .to_owned();
        v.health = a.health / a.kind.health() * 100.0;
        v.health_label = format!("{} / {}", a.health.ceil() as u16, a.kind.health() as u16);
        if let Some(traits) = a.horse_traits() {
            v.stats.push(StatRow {
                id: "horse-stamina".into(),
                label: "Stamina".into(),
                text: traits.stamina.to_string(),
                bar: false,
                value: 100.0,
            });
            v.stats.push(StatRow {
                id: "horse-jump".into(),
                label: "Jump".into(),
                text: format!("{:.2} m", traits.jump_height),
                bar: false,
                value: 100.0,
            });
        }
        v.status = match a.behavior {
            B::Flee => "Aufgeschreckt",
            B::Sleep => "Schlaeft",
            B::InNest => "Im Nest",
            B::Nectar => "Sammelt Nektar",
            B::Drink => "Trinkt",
            B::Graze => "Frisst",
            B::Leap => "Springt",
            B::Hunt => "Jagt Fische",
            B::Attack => "Greift an",
            B::Climb => "Klettert",
            _ if a.kind.water() > 0.0 && a.needs.water < a.kind.water() * 0.35 => "Durstig",
            _ if a.kind.food() > 0.0 && a.needs.food < a.kind.food() * 0.35 => "Hungrig",
            _ => "Entspannt",
        }
        .into();
    } else if let Some(target) = scene.target.0 {
        v.visible = true;
        match target {
            WorldTarget::Block(hit) => {
                let material = scene.chunks.block_at(hit.position).unwrap_or(0);
                if matches!(material, 0 | 14 | 27) {
                    v.visible = false;
                }
                if let Some(b) = scene.palette.choices.iter().find(|b| b.id == material) {
                    v.title = b.name.clone();
                    v.identifier = b.identifier.clone();
                }
                let properties = scene
                    .properties
                    .0
                    .get(&material)
                    .copied()
                    .unwrap_or_default();
                (v.hand, v.tool_preview, v.tool_label, v.harvest) =
                    tool(properties, scene.active.0);
                if material == 41
                    && scene
                        .session
                        .as_ref()
                        .is_none_or(|s| s.game_mode != terra_core::GameMode::Creative)
                {
                    v.harvest = false;
                    v.tool_label = "Unzerstoerbar".into();
                }
                v.mining = matches!(scene.mining.target,Some(terra_network::protocol::session::MiningTarget::Block{position,..}) if position==hit.position);
            }
            WorldTarget::Prop(hit) => {
                let kind: ResourcePropKind = hit.prop.kind.into();
                v.title = kind.label().into();
                v.identifier = ItemKind::Prop(kind).identifier();
                v.hand = true;
                v.harvest = true;
                v.tool_label = "Hand / Level 0".into();
                v.mining = matches!(scene.mining.target,Some(terra_network::protocol::session::MiningTarget::Prop{id,..}) if id==hit.prop.id);
                let style = scene.styles.0.get(&kind).copied().unwrap_or_default();
                if let Some(animals) = &scene.animals {
                    if kind == ResourcePropKind::Flower {
                        stat(
                            &mut v.stats,
                            "nectar",
                            "Nektar",
                            f32::from(animals.ecology.nectar(hit.prop.id.0)),
                            100.0,
                            style.nectar,
                        );
                    }
                    if let Some(n) = animals.ecology.nests.get(&hit.prop.id.0) {
                        stat(
                            &mut v.stats,
                            "nectar",
                            "Nektar",
                            f32::from(n.nectar),
                            1000.0,
                            style.nectar,
                        );
                        stat(
                            &mut v.stats,
                            "honey",
                            "Honig",
                            f32::from(n.honey),
                            100.0,
                            style.honey,
                        );
                        stat(
                            &mut v.stats,
                            "occupants",
                            "Bienen",
                            animals.ecology.inside(hit.prop.id.0) as f32,
                            10.0,
                            style.occupants,
                        );
                        stat(
                            &mut v.stats,
                            "processing",
                            "Reifung",
                            n.progress * 100.0,
                            100.0,
                            style.processing,
                        );
                    }
                }
            }
        }
        v.progress = scene.mining.progress.clamp(0.0, 1.0) * 100.0;
    }
    if *view != v {
        *view = v;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn correct_tool_and_tier_is_required_not_just_any_equipped_item() {
        use terra_core::block_properties::BlockProperties;
        let p = BlockProperties {
            required_tool: Some(ToolClass::Pickaxe),
            mining_level: 2,
            ..default()
        };
        assert!(!tool(p, None).3);
        assert!(!tool(p, Some(ItemKind::Tool(ToolKind::WoodPickaxe))).3);
        assert!(!tool(p, Some(ItemKind::Tool(ToolKind::StoneAxe))).3);
        assert!(tool(p, Some(ItemKind::Tool(ToolKind::IronPickaxe))).3);
        assert!(tool(BlockProperties::default(), Some(ItemKind::Block(3))).3);
    }
    #[test]
    fn hud_styles_use_tilt_parser() {
        tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
            "../../../src-ui/components/target-card.component.css"
        ))
        .unwrap();
    }
}
