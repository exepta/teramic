//! Read-only target presentation driven by the same picking and authority state as gameplay.
use bevy::prelude::*;
use serde::Serialize;
use terra_core::{
    ItemKind, ResourcePropKind, ToolKind,
    block_properties::{HudStats, StatDisplay, ToolClass},
};
use terra_network::client::{ClientSession, ReplicatedCreatures};
use terra_player::{
    TargetWorld, WorldTarget,
    interaction::{ActiveItem, BlockPalette, MiningVisual, TargetCreature},
};
use terra_world::streaming::ChunkStore;
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
    #[serde(skip)]
    pub tool_item: Option<ItemKind>,
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
struct Scene<'w, 's> {
    rail_preview: Option<Res<'w, terra_player::interaction::PlacementPreview>>,
    controls: Option<Res<'w, terra_core::ControlsConfig>>,
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
    vessel: Option<Res<'w, terra_player::interaction::VesselMotion>>,
    active: Res<'w, ActiveItem>,
    palette: Res<'w, BlockPalette>,
    chunks: Res<'w, terra_world::streaming::ChunkStore>,
    properties: Res<'w, terra_core::block_properties::BlockPropertiesCatalog>,
    animals: Option<Res<'w, ReplicatedCreatures>>,
    styles: Res<'w, Presentations>,
    access: Res<'w, terra_world::streaming::WorldAccess>,
    journal: Option<Res<'w, terra_world::edits::EditJournal>>,
    ray: SceneRay<'w, 's>,
}
#[derive(bevy::ecs::system::SystemParam)]
struct SceneRay<'w, 's> {
    cameras: Query<
        'w,
        's,
        (&'static Transform, &'static terra_player::CameraLook),
        With<terra_player::TerraCamera>,
    >,
    bodies: Query<
        'w,
        's,
        &'static Transform,
        (
            With<terra_player::PlayerBody>,
            Without<terra_player::TerraCamera>,
        ),
    >,
    perspective: Option<Res<'w, terra_player::CameraPerspective>>,
}
fn hover_material(scene: &Scene, position: [i64; 3]) -> u16 {
    let material = scene.chunks.block_at(position).unwrap_or(0);
    if !terra_core::construction::is_slab(material) {
        return material;
    }
    let Some(journal) = scene.journal.as_ref() else {
        return material;
    };
    let Some((camera, look)) = scene.ray.cameras.iter().next() else {
        return material;
    };
    // Match PlayerSet::Targeting: the front-facing third-person camera is
    // cosmetic; gameplay still aims from the player's eye using CameraLook.
    let origin = if scene
        .ray
        .perspective
        .as_deref()
        .is_some_and(|p| p.is_third_person())
    {
        scene
            .ray
            .bodies
            .iter()
            .next()
            .map_or(camera.translation, |body| {
                body.translation + Vec3::Y * terra_player::PLAYER_EYE_OFFSET
            })
    } else {
        camera.translation
    };
    terra_world::slabs::targeted_half(
        &scene.chunks,
        journal,
        position,
        (
            origin.to_array().map(f64::from),
            (look.rotation() * Vec3::NEG_Z).to_array().map(f64::from),
        ),
    )
    .map_or(material, |half| half.removed)
}
fn tool(
    properties: terra_core::block_properties::BlockProperties,
    held: Option<ItemKind>,
) -> (bool, String, String, bool, Option<ItemKind>) {
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
        required.map(ItemKind::Tool),
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
fn crop_growth(progress: f32) -> StatRow {
    let value = progress.clamp(0.0, 1.0) * 100.0;
    StatRow {
        id: "crop-growth".into(),
        label: if progress >= 1.0 {
            "Erntereif"
        } else {
            "Wachstum"
        }
        .into(),
        text: format!("{value:.0}%"),
        bar: true,
        value,
    }
}

fn sapling_growth(
    prop: &terra_world::props::WorldProp,
    session: Option<&ClientSession>,
    journal: Option<&terra_world::edits::EditJournal>,
) -> Option<StatRow> {
    let species = prop.kind.sapling()?;
    let support = terra_world::props::placement::support(prop);
    let root = [support[0], support[1] + 1, support[2]];
    let tree = match session {
        Some(session) => session.trees.get(prop.id.0),
        None => journal.and_then(|j| j.gameplay().trees.get(prop.id.0)),
    }
    .filter(|tree| {
        !prop.removed
            && tree.species == species
            && tree.root == root
            && tree.stage == terra_core::TreeStage::Sapling
            && tree.growth_seconds.is_finite()
    });
    let Some(tree) = tree else {
        return Some(StatRow {
            id: "sapling-growth".into(),
            label: "Wachstum".into(),
            text: "Wird synchronisiert".into(),
            bar: false,
            value: 0.0,
        });
    };
    let value = (f64::from(tree.growth_seconds) * 100.0 / f64::from(species.growth_seconds()))
        .clamp(0.0, 100.0) as f32;
    Some(StatRow {
        id: "sapling-growth".into(),
        label: if value >= 100.0 {
            "Wartet auf Wachstum"
        } else {
            "Wachstum"
        }
        .into(),
        text: format!("{}%", value.floor() as u8),
        bar: true,
        value,
    })
}

fn kinetic_stats(rows: &mut Vec<StatRow>, state: terra_core::kinetic::KineticState) {
    rows.push(StatRow {
        id: "kinetic-rpm".into(),
        label: "Drehzahl".into(),
        text: format!("{} U/min", state.rpm),
        bar: false,
        value: 0.0,
    });
    rows.push(StatRow {
        id: "kinetic-drive".into(),
        label: "Antrieb".into(),
        text: if state.overloaded {
            "Ueberlastet"
        } else if state.running() {
            "In Betrieb"
        } else {
            "Stillstand"
        }
        .into(),
        bar: false,
        value: 0.0,
    });
    rows.push(StatRow {
        id: "kinetic-stress".into(),
        label: "Stress / Kapazitaet".into(),
        text: format!("{:.1} / {:.1}", state.stress, state.capacity),
        bar: true,
        value: if state.capacity > 0.0 {
            (state.stress / state.capacity * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        },
    });
    rows.push(StatRow {
        id: "kinetic-remaining".into(),
        label: "Am Anschluss verfuegbar".into(),
        text: format!("{:.1}", state.available_capacity),
        bar: false,
        value: 0.0,
    });
}

fn crop_progress(prop: &terra_world::props::WorldProp, session: Option<&ClientSession>) -> f32 {
    let support = terra_world::props::placement::support(prop);
    session
        .and_then(|session| session.crop_progress.get(&support))
        .filter(|sample| {
            sample.valid() && sample.prop_id == prop.id.0 && Some(sample.kind) == prop.kind.crop()
        })
        .map_or(f32::from(prop.variant.min(7)) / 7.0, |sample| {
            sample.progress
        })
}

fn crop_water_stats(
    rows: &mut Vec<StatRow>,
    prop: &terra_world::props::WorldProp,
    session: Option<&ClientSession>,
    journal: Option<&terra_world::edits::EditJournal>,
) {
    let support = terra_world::props::placement::support(prop);
    let moisture = session
        .and_then(|session| session.crop_progress.get(&support))
        .filter(|sample| {
            sample.valid() && sample.prop_id == prop.id.0 && Some(sample.kind) == prop.kind.crop()
        })
        .map(|sample| sample.moisture)
        .or_else(|| {
            journal
                .and_then(|journal| journal.crop_at(support))
                .filter(|crop| {
                    crop.valid() && crop.prop_id == prop.id.0 && Some(crop.kind) == prop.kind.crop()
                })
                .map(|crop| crop.moisture)
        });
    if let Some(moisture) = moisture {
        rows.push(StatRow {
            id: "crop-moisture".into(),
            label: "Bodenfeuchte".into(),
            text: format!("{:.0}%", moisture * 100.0),
            bar: true,
            value: moisture * 100.0,
        });
    }
    rows.push(StatRow {
        id: "crop-water-needed".into(),
        label: "Wasserbedarf".into(),
        text: if crop_progress(prop, session) >= 1.0 {
            "Erntereif"
        } else {
            match moisture {
                Some(value) if value <= terra_core::farming::DRY_MOISTURE => "Giessen erforderlich",
                Some(value) if value <= 0.25 => "Bald giessen",
                Some(_) => "Ausreichend versorgt",
                None => "Wird erfasst",
            }
        }
        .into(),
        bar: false,
        value: 0.0,
    });
}

fn farmland_crop(chunks: &ChunkStore, support: [i64; 3]) -> Option<&terra_world::props::WorldProp> {
    chunks
        .loaded
        .get(&terra_voxel::global_to_section(support).0)?
        .props
        .iter()
        .find(|prop| {
            !prop.removed
                && prop.kind.crop().is_some()
                && terra_world::props::placement::support(prop) == support
        })
}

fn farmland_stat(
    chunks: &ChunkStore,
    session: Option<&ClientSession>,
    support: [i64; 3],
) -> StatRow {
    farmland_crop(chunks, support).map_or_else(
        || StatRow {
            id: "crop-seeds".into(),
            label: "Aussaat".into(),
            text: "Keine Samen erkannt".into(),
            bar: false,
            value: 0.0,
        },
        |prop| crop_growth(crop_progress(prop, session)),
    )
}

fn companion_stats(
    rows: &mut Vec<StatRow>,
    kind: terra_core::creatures::CreatureKind,
    bond: Option<&terra_core::creatures::taming::CompanionView>,
    viewer: Option<terra_core::PlayerUuid>,
) {
    use terra_core::creatures::{CreatureKind as K, taming};
    if !taming::tameable(kind) {
        return;
    }
    let bond = bond.filter(|bond| bond.valid());
    let trust = bond.map_or(0, |bond| bond.trust);
    rows.push(StatRow {
        id: "animal-trust".into(),
        label: "Vertrauen".into(),
        text: format!("{trust} / 100"),
        bar: true,
        value: f32::from(trust),
    });
    let owner = bond.and_then(|bond| bond.owner);
    rows.push(StatRow {
        id: "animal-owner".into(),
        label: "Besitzer".into(),
        text: owner.map_or_else(
            || "Wild".into(),
            |owner| {
                if Some(owner) == viewer {
                    "Du".into()
                } else {
                    format!("Spieler {}", &owner.simple().to_string()[..8])
                }
            },
        ),
        bar: false,
        value: 0.0,
    });
    if owner.is_none() {
        rows.push(StatRow {
            id: "animal-food".into(),
            label: "Futter".into(),
            text: match kind {
                K::Cow | K::Sheep => "Weizen",
                K::Horse => "Weizen, Mais, Salat",
                K::Pig => "Mais, Salat, Pilze",
                _ => "Rohes Fleisch / roher Fisch",
            }
            .into(),
            bar: false,
            value: 0.0,
        });
    }
}

fn animal_sex(animal: &terra_core::creatures::Creature) -> StatRow {
    StatRow {
        id: "animal-sex".into(),
        label: "Geschlecht".into(),
        text: animal.sex().label().into(),
        bar: false,
        value: 0.0,
    }
}

fn held_watering_stat(item: ItemKind) -> Option<StatRow> {
    let capacity = item.tool_kind()?.watering_capacity();
    (capacity > 0).then(|| StatRow {
        id: "held-watering-can".into(),
        label: "Gießkanne".into(),
        text: format!("{} / {capacity}", item.watering_units()),
        bar: true,
        value: 100.0 * f32::from(item.watering_units()) / f32::from(capacity),
    })
}

fn plumbing_stats(
    rows: &mut Vec<StatRow>,
    state: &terra_network::protocol::session::PlumbingVisual,
    material: u16,
    facing: u8,
) {
    rows.push(StatRow {
        id: "water-storage".into(),
        label: "Wasser".into(),
        text: format!("{} / {}", state.amount, state.capacity),
        bar: true,
        value: 100.0 * f32::from(state.amount) / f32::from(state.capacity.max(1)),
    });
    if matches!(material, 166..=167) {
        rows.push(StatRow {
            id: "water-valve".into(),
            label: if material == 166 { "Ventil" } else { "Hahn" }.into(),
            text: if material == 167 {
                let level = terra_core::plumbing::faucet_level(facing);
                if level == 0 {
                    "Zu · Stufe 0 / 3".into()
                } else {
                    format!("Stufe {level} / 3")
                }
            } else {
                if state.open { "Offen" } else { "Geschlossen" }.into()
            },
            bar: false,
            value: 0.0,
        });
    }
    if material == terra_core::plumbing::FAUCET {
        rows.push(StatRow {
            id: "water-controls".into(),
            label: "Bedienung".into(),
            text: "LMB: − · RMB: + · Shift+LMB: Abbauen".into(),
            bar: false,
            value: 0.0,
        });
    }
    rows.push(StatRow {
        id: "water-flow".into(),
        label: "Durchfluss".into(),
        text: if material == terra_core::infusion::FLUID_INFUSER {
            if state.supplied {
                "Versorgt"
            } else {
                "Kein Wasser"
            }
        } else if state.flowing {
            "Fliesst"
        } else {
            "Still"
        }
        .into(),
        bar: false,
        value: 0.0,
    });
}

fn family_stats(
    rows: &mut Vec<StatRow>,
    animal: &terra_core::creatures::Creature,
    family: Option<&terra_core::creatures::breeding::FamilyView>,
) {
    if animal.juvenile() {
        rows.push(StatRow {
            id: "animal-growth".into(),
            label: "Wachstum".into(),
            text: format!("{}%", (animal.growth() * 100.0).floor() as u8),
            bar: true,
            value: animal.growth() * 100.0,
        });
    }
    if let Some(family) = family.filter(|family| family.valid()) {
        rows.push(StatRow {
            id: "animal-breeding".into(),
            label: "Zucht".into(),
            text: if animal.juvenile() {
                "Zu jung".into()
            } else if family.willing {
                "Bereit".into()
            } else if family.cooldown_seconds > 0.0 {
                let seconds = family.cooldown_seconds.ceil() as u32;
                format!("{}:{:02}", seconds / 60, seconds % 60)
            } else {
                "Nicht bereit".into()
            },
            bar: false,
            value: 0.0,
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
        v.stats.push(animal_sex(a));
        v.title = a.title();
        if a.juvenile() {
            v.title = format!("Jungtier: {}", v.title);
        }
        v.health = a.health / a.kind.health() * 100.0;
        v.health_label = format!("{} / {}", a.health.ceil() as u16, a.kind.health() as u16);
        companion_stats(
            &mut v.stats,
            a.kind,
            scene
                .animals
                .as_ref()
                .and_then(|animals| animals.bonds.get(&a.id)),
            scene.session.as_ref().and_then(|session| session.uuid),
        );
        family_stats(
            &mut v.stats,
            a,
            scene
                .animals
                .as_ref()
                .and_then(|animals| animals.families.get(&a.id)),
        );
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
            B::Hunt => "Jagt",
            B::Attack => "Greift an",
            B::Climb => "Klettert",
            B::Ceiling => "An der Decke",
            B::Descend => "Seilt sich ab",
            _ if a.kind.water() > 0.0 && a.needs.water < a.kind.water() * 0.35 => "Durstig",
            _ if a.kind.food() > 0.0 && a.needs.food < a.kind.food() * 0.35 => "Hungrig",
            _ => "Entspannt",
        }
        .into();
    } else if let Some(target) = scene.target.0 {
        v.visible = true;
        match target {
            WorldTarget::Block(hit) => {
                let material = hover_material(&scene, hit.position);
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
                (v.hand, v.tool_preview, v.tool_label, v.harvest, v.tool_item) =
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
                if terra_core::kinetic::is_part(material) {
                    let state = scene
                        .session
                        .as_ref()
                        .and_then(|session| {
                            session
                                .kinetics
                                .iter()
                                .find(|part| part.position == hit.position)
                        })
                        .map(|part| terra_core::kinetic::KineticState {
                            rpm: part.rpm,
                            overloaded: part.overloaded,
                            stress: part.stress,
                            capacity: part.capacity,
                            available_capacity: part.available_capacity,
                        })
                        .or_else(|| {
                            scene
                                .journal
                                .as_ref()
                                .map(|journal| journal.kinetic_state(hit.position))
                        })
                        .unwrap_or_default();
                    kinetic_stats(&mut v.stats, state);
                }
                if material == terra_core::infusion::HOLD_BLOCK {
                    if scene
                        .chunks
                        .block_at(terra_core::infusion::infuser_above(hit.position))
                        == Some(terra_core::infusion::FLUID_INFUSER)
                        && scene
                            .chunks
                            .block_at(terra_core::infusion::gap_above(hit.position))
                            != Some(0)
                    {
                        v.stats.push(StatRow {
                            id: "hold-air-gap".into(),
                            label: "Luftspalt".into(),
                            text: "Blockiert · Zwischenraum freihalten".into(),
                            bar: false,
                            value: 0.,
                        });
                    }
                    let facing = scene
                        .session
                        .as_ref()
                        .and_then(|session| {
                            session
                                .infusion
                                .iter()
                                .find(|view| view.position == hit.position)
                        })
                        .map(|view| view.facing)
                        .or_else(|| {
                            scene
                                .journal
                                .as_ref()
                                .and_then(|journal| {
                                    terra_world::conveyors::infusion_geometry(hit.position, journal)
                                })
                                .map(|view| view.facing)
                        })
                        .unwrap_or(0);
                    v.stats.push(StatRow {
                        id: "hold-direction".into(),
                        label: "Bandrichtung".into(),
                        text: ["Süd", "Ost", "Nord", "West"][usize::from(facing)].into(),
                        bar: false,
                        value: 0.0,
                    });
                    if let Some(view) = scene.session.as_ref().and_then(|session| {
                        session
                            .infusion
                            .iter()
                            .find(|view| view.position == hit.position)
                    }) {
                        use terra_core::infusion::HoldPhase;
                        v.stats.push(StatRow {
                            id: "hold-phase".into(),
                            label: "Station".into(),
                            text: match view.phase {
                                HoldPhase::Idle if view.speed > 0.0 => "Durchlauf",
                                HoldPhase::Idle => "Bereit",
                                HoldPhase::Approaching => "Gefäß wird eingelegt",
                                HoldPhase::WaitingForFluid => "Wartet auf Wasser",
                                HoldPhase::Filling => "Befüllt",
                                HoldPhase::Releasing => "Weitertransport",
                            }
                            .into(),
                            bar: false,
                            value: 0.0,
                        });
                    }
                }
                if material == terra_core::infusion::FLUID_INFUSER
                    && (scene
                        .chunks
                        .block_at(terra_core::infusion::hold_below(hit.position))
                        != Some(terra_core::infusion::HOLD_BLOCK)
                        || scene.chunks.block_at(terra_core::infusion::gap_above(
                            terra_core::infusion::hold_below(hit.position),
                        )) != Some(0))
                {
                    v.stats.push(StatRow {
                        id: "infuser-mount".into(),
                        label: "Montage".into(),
                        text: "Ein freier Luftblock zwischen Hold Block und Infuser".into(),
                        bar: false,
                        value: 0.0,
                    });
                }
                if terra_core::plumbing::is_part(material)
                    && let Some(state) = scene.session.as_deref().and_then(|session| {
                        session
                            .plumbing
                            .iter()
                            .find(|state| state.position == hit.position)
                    })
                {
                    plumbing_stats(
                        &mut v.stats,
                        state,
                        material,
                        scene
                            .journal
                            .as_ref()
                            .and_then(|j| j.object_state(hit.position))
                            .map_or(if state.open { 0 } else { 4 }, |pose| pose.facing),
                    );
                }
                if let Some(motion) = scene
                    .vessel
                    .as_deref()
                    .filter(|motion| motion.filling && motion.target == Some(hit.position))
                {
                    v.visible = true;
                    if material == 27 {
                        v.title = "Wasserquelle".into();
                        v.identifier = "teramic::water".into();
                    }
                    v.stats.push(StatRow {
                        id: "vessel-fill".into(),
                        label: "Fuellen".into(),
                        text: String::new(),
                        bar: true,
                        value: motion.progress.clamp(0.0, 1.0) * 100.0,
                    });
                }
                if material == terra_core::farming::FARMLAND {
                    v.stats.push(farmland_stat(
                        &scene.chunks,
                        scene.session.as_deref(),
                        hit.position,
                    ));
                    if let Some(prop) = farmland_crop(&scene.chunks, hit.position) {
                        crop_water_stats(
                            &mut v.stats,
                            prop,
                            scene.session.as_deref(),
                            scene.journal.as_deref(),
                        );
                    }
                }
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
                if let Some(growth) = sapling_growth(
                    &hit.prop,
                    scene.session.as_deref(),
                    scene.journal.as_deref(),
                ) {
                    v.stats.push(growth);
                }
                if let Some(crop) = terra_core::CropKind::from_prop(kind) {
                    v.title = match crop {
                        terra_core::CropKind::Wheat => "Weizen",
                        terra_core::CropKind::Corn => "Mais",
                        terra_core::CropKind::Lettuce => "Salat",
                    }
                    .into();
                    v.identifier = format!("teramic::{}", crop.slug());
                    v.stats.push(crop_growth(crop_progress(
                        &hit.prop,
                        scene.session.as_deref(),
                    )));
                    crop_water_stats(
                        &mut v.stats,
                        &hit.prop,
                        scene.session.as_deref(),
                        scene.journal.as_deref(),
                    );
                }
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
                        v.stats.push(StatRow {
                            id: "honey-ready".into(),
                            label: "Honigernte".into(),
                            text: if n.honey > 0 {
                                "Bereit"
                            } else {
                                "Noch kein reifer Honig"
                            }
                            .into(),
                            bar: false,
                            value: 0.0,
                        });
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
        if v.visible
            && let Some(row) = scene.active.0.and_then(held_watering_stat)
        {
            v.stats.push(row);
        }
        v.progress = scene.mining.progress.clamp(0.0, 1.0) * 100.0;
    }
    if let Some(preview) = scene.rail_preview.as_ref()
        && scene.active.0.is_some_and(|item| {
            item.block_material()
                .is_some_and(terra_core::rails::is_rail)
        })
    {
        let mode = [
            "Gleis",
            "Weiche links",
            "Weiche rechts",
            "Kreuzung",
            "Prellbock",
        ]
        .get(usize::from(preview.rail_mode))
        .copied()
        .unwrap_or("Gleis");
        let binding = |action: terra_core::GameAction| {
            scene.controls.as_ref().map_or_else(
                || action.default_binding().label(),
                |controls| controls.binding(action).label(),
            )
        };
        v.visible = true;
        v.status = if preview.rail_start.is_some() {
            format!(
                "{mode}: Ziel waehlen · {}+{}: Abbrechen",
                binding(terra_core::GameAction::FlyDown),
                binding(terra_core::GameAction::Place)
            )
        } else {
            format!(
                "{mode} · {}: Bauform · Gleis anklicken: Verbinden",
                binding(terra_core::GameAction::Use)
            )
        };
        if preview.rail_cost > 0 && preview.rail_cost != u16::MAX {
            v.stats.push(StatRow {
                id: "rail-cost".into(),
                label: "Schienen".into(),
                text: preview.rail_cost.to_string(),
                bar: false,
                value: 100.,
            });
        }
    }
    if *view != v {
        *view = v;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use terra_world::{
        props::{PropId, PropKind, WorldProp},
        streaming::LoadedSection,
    };

    #[test]
    fn hovered_saplings_show_confirmed_growth_for_each_species_and_ignore_stale_occupants() {
        use terra_core::{PlayerUuid, TreeRecord, TreeSpecies, TreeStage};
        use terra_world::{edits::EditJournal, props::PropHit};
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<terra_core::DebugSettings>()
            .init_resource::<crate::inventory::InventoryState>()
            .init_resource::<crate::SettingsView>()
            .init_resource::<crate::chat::ChatState>()
            .init_resource::<TargetCreature>()
            .init_resource::<MiningVisual>()
            .init_resource::<ActiveItem>()
            .init_resource::<Presentations>()
            .init_resource::<terra_world::streaming::WorldAccess>()
            .init_resource::<TargetView>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .init_resource::<BlockPalette>()
            .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
            .init_resource::<ClientSession>()
            .init_resource::<TargetWorld>()
            .add_systems(Update, update);
        let root = [-17, 16, -1];
        for species in TreeSpecies::ALL {
            let prop = WorldProp {
                id: PropId(201),
                kind: species.sapling().into(),
                position: [-16.5, 16.0, -0.5],
                rotation: 0.0,
                scale: 1.0,
                variant: 0,
                section: terra_voxel::global_to_section([-17, 15, -1]).0,
                removed: false,
            };
            let tree = TreeRecord {
                id: prop.id.0,
                root,
                species,
                seed: 201,
                planted_by: Some(PlayerUuid::from_u128(1)),
                stage: TreeStage::Sapling,
                growth_seconds: 0.0,
                bounds: None,
                wood: Vec::new(),
                leaves: Vec::new(),
                fruit: None,
                revision: 0,
            };
            app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Prop(PropHit {
                prop,
                distance: 2.0,
            }));
            app.world_mut()
                .resource_mut::<ClientSession>()
                .trees
                .trees
                .clear();
            assert!(
                app.world_mut()
                    .resource_mut::<ClientSession>()
                    .trees
                    .insert(tree.clone())
            );
            assert!(
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .gameplay_mut()
                    .trees
                    .insert(tree.clone())
            );
            for percent in [0, 37, 99, 100] {
                app.world_mut()
                    .resource_mut::<ClientSession>()
                    .trees
                    .get_mut(prop.id.0)
                    .unwrap()
                    .growth_seconds = species.growth_seconds() / 100.0 * percent as f32;
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_millis(100));
                app.update();
                let view = app.world().resource::<TargetView>();
                assert!(view.visible && !view.mining);
                assert_eq!(view.title, species.sapling().label());
                let row = view
                    .stats
                    .iter()
                    .find(|r| r.id == "sapling-growth")
                    .unwrap();
                assert_eq!(row.text, format!("{percent}%"));
                assert_eq!(row.value, percent as f32);
                assert!(row.bar);
                assert_eq!(
                    row.label,
                    if percent == 100 {
                        "Wartet auf Wachstum"
                    } else {
                        "Wachstum"
                    }
                );
            }
            for stale in 0..4 {
                let mut mismatched = tree.clone();
                match stale {
                    0 => mismatched.root[0] += 1,
                    1 => {
                        mismatched.species = TreeSpecies::ALL
                            .into_iter()
                            .find(|s| *s != species)
                            .unwrap()
                    }
                    2 => mismatched.stage = TreeStage::Felled,
                    _ => mismatched.id += 1,
                }
                {
                    let mut session = app.world_mut().resource_mut::<ClientSession>();
                    session.trees.trees.clear();
                    assert!(session.trees.insert(mismatched));
                }
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_millis(100));
                app.update();
                let view = app.world().resource::<TargetView>();
                let row = view
                    .stats
                    .iter()
                    .find(|r| r.id == "sapling-growth")
                    .unwrap();
                assert_eq!(row.text, "Wird synchronisiert");
                assert!(
                    !row.bar,
                    "stale progress must not override the confirmed replica"
                );
            }
            app.world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .trees
                .trees
                .clear();
        }
    }

    #[test]
    fn held_watering_hud_uses_the_selected_stack_fill_including_empty_and_partial() {
        for tool in [
            ToolKind::WoodWateringCan,
            ToolKind::RubyWateringCan,
            ToolKind::IronWateringCan,
        ] {
            for units in 0..=tool.watering_capacity() {
                let item = ItemKind::Tool(tool).with_watering_units(units).unwrap();
                let row = held_watering_stat(item).unwrap();
                assert_eq!(row.text, format!("{units} / {}", tool.watering_capacity()));
                assert_eq!(
                    row.value,
                    100.0 * f32::from(units) / f32::from(tool.watering_capacity())
                );
            }
        }
        assert!(held_watering_stat(ItemKind::Tool(ToolKind::Bucket)).is_none());
    }

    #[test]
    fn plumbing_hud_reports_real_reservoir_capacity_and_shutoff() {
        let state = terra_network::protocol::session::PlumbingVisual {
            intake: None,
            position: [1, 2, 3],
            amount: 1500,
            capacity: 2000,
            open: false,
            flowing: false,
            supplied: false,
            pump_rpm: 0.0,
            basin: None,
        };
        let mut rows = Vec::new();
        plumbing_stats(&mut rows, &state, 169, 0);
        assert_eq!(rows[0].text, "1500 / 2000");
        assert_eq!(rows[0].value, 75.0);
        assert_eq!(rows.len(), 2);
        rows.clear();
        plumbing_stats(&mut rows, &state, 166, 0);
        assert_eq!(rows[1].text, "Geschlossen");
        assert_eq!(rows[2].text, "Still");
    }

    #[test]
    fn faucet_hud_reports_each_real_stage() {
        let state = terra_network::protocol::session::PlumbingVisual {
            intake: None,
            position: [1, 2, 3],
            amount: 8,
            capacity: 8,
            open: true,
            flowing: true,
            supplied: true,
            pump_rpm: 0.0,
            basin: None,
        };
        for level in 0..=3 {
            let mut rows = Vec::new();
            plumbing_stats(
                &mut rows,
                &state,
                167,
                terra_core::plumbing::faucet_facing(0, level),
            );
            assert!(rows[1].text.contains(&format!("{level} / 3")));
        }
    }

    #[test]
    fn kinetic_hud_distinguishes_signed_speed_stopped_and_overloaded_networks() {
        for (rpm, overloaded, expected) in [
            (30.0, false, "In Betrieb"),
            (-30.0, false, "In Betrieb"),
            (7.5, false, "In Betrieb"),
            (0.0, false, "Stillstand"),
            (0.0, true, "Ueberlastet"),
        ] {
            let mut rows = Vec::new();
            kinetic_stats(
                &mut rows,
                terra_core::kinetic::KineticState {
                    rpm,
                    overloaded,
                    ..Default::default()
                },
            );
            assert_eq!(rows.len(), 4);
            assert_eq!(rows[0].text, format!("{rpm} U/min"));
            assert_eq!(rows[1].text, expected);
            assert!(rows[2].bar);
            assert_eq!(rows[2].value, 0.0);
        }
    }

    #[test]
    fn mixed_slab_hover_shows_aimed_title_identifier_and_tool_level_without_changing_edit_target() {
        use terra_core::block_properties::{BlockProperties, BlockPropertiesCatalog};
        use terra_player::{
            CameraLook, CameraPerspective, PlayerBody, TerraCamera, interaction::BlockChoice,
        };
        let position = [-17, 4, -16];
        let (section, local) = terra_voxel::global_to_section(position);
        let mut chunks = ChunkStore::default();
        let mut data = terra_voxel::BlockSection::empty(section).unwrap();
        data.set_block(local, 82).unwrap();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut journal = terra_world::edits::EditJournal::default();
        journal.record(position, 82).unwrap();
        journal
            .set_object_state(
                position,
                Some(terra_world::edits::BlockObjectState {
                    facing: terra_core::construction::merged_slab(82, 5, 85).unwrap(),
                    ..Default::default()
                }),
            )
            .unwrap();
        let palette = BlockPalette {
            choices: [
                (82, "Oak Half Slab", "teramic::oak_slab"),
                (85, "Stone Half Slab", "teramic::stone_slab"),
            ]
            .into_iter()
            .map(|(id, name, identifier)| BlockChoice {
                id,
                name: name.into(),
                identifier: identifier.into(),
                description: String::new(),
            })
            .collect(),
            ..Default::default()
        };
        let mut properties = BlockPropertiesCatalog::default();
        properties.0.insert(82, BlockProperties::default());
        properties.0.insert(
            85,
            BlockProperties {
                required_tool: Some(ToolClass::Pickaxe),
                mining_level: 2,
                ..Default::default()
            },
        );
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<terra_core::DebugSettings>()
            .init_resource::<crate::inventory::InventoryState>()
            .init_resource::<crate::SettingsView>()
            .init_resource::<crate::chat::ChatState>()
            .init_resource::<TargetCreature>()
            .init_resource::<MiningVisual>()
            .init_resource::<ActiveItem>()
            .init_resource::<Presentations>()
            .init_resource::<terra_world::streaming::WorldAccess>()
            .init_resource::<TargetView>()
            .insert_resource(chunks)
            .insert_resource(journal)
            .insert_resource(palette)
            .insert_resource(properties)
            .insert_resource(TargetWorld(Some(WorldTarget::Block(
                terra_voxel::blocks::BlockHit {
                    position,
                    normal: [0, 0, 1],
                    distance: 2.0,
                },
            ))))
            .add_systems(Update, update);
        let camera = app
            .world_mut()
            .spawn((TerraCamera, CameraLook::default(), Transform::default()))
            .id();
        let body = app
            .world_mut()
            .spawn((PlayerBody, Transform::default()))
            .id();
        for perspective in [
            CameraPerspective::FirstPerson,
            CameraPerspective::ThirdPersonFront,
        ] {
            app.insert_resource(perspective);
            for selected in 0..2 {
                let eye = Vec3::new(
                    position[0] as f32 + if selected == 0 { 0.25 } else { 0.75 },
                    position[1] as f32 + 0.5,
                    position[2] as f32 + 3.0,
                );
                let opposite = Vec3::new(
                    position[0] as f32 + if selected == 0 { 0.75 } else { 0.25 },
                    eye.y,
                    eye.z,
                );
                app.world_mut()
                    .get_mut::<Transform>(camera)
                    .unwrap()
                    .translation = if perspective.is_third_person() {
                    opposite
                } else {
                    eye
                };
                app.world_mut()
                    .get_mut::<Transform>(body)
                    .unwrap()
                    .translation = if perspective.is_third_person() {
                    eye
                } else {
                    opposite
                } - Vec3::Y * terra_player::PLAYER_EYE_OFFSET;
                for held in [
                    None,
                    Some(ToolKind::WoodPickaxe),
                    Some(ToolKind::StoneAxe),
                    Some(ToolKind::StonePickaxe),
                ] {
                    app.world_mut().resource_mut::<ActiveItem>().0 = held.map(ItemKind::Tool);
                    app.world_mut()
                        .resource_mut::<Time>()
                        .advance_by(std::time::Duration::from_millis(100));
                    app.update();
                    let view = app.world().resource::<TargetView>();
                    assert!(view.visible);
                    assert!(!view.mining, "hover must work before holding Break");
                    assert_eq!(
                        view.title,
                        if selected == 0 {
                            "Oak Half Slab"
                        } else {
                            "Stone Half Slab"
                        }
                    );
                    assert_eq!(
                        view.identifier,
                        if selected == 0 {
                            "teramic::oak_slab"
                        } else {
                            "teramic::stone_slab"
                        }
                    );
                    assert_eq!(view.hand, selected == 0);
                    assert_eq!(
                        view.harvest,
                        selected == 0 || held == Some(ToolKind::StonePickaxe)
                    );
                    assert_eq!(
                        view.tool_label,
                        if selected == 0 {
                            "Hand / Level 0"
                        } else {
                            "stone pickaxe / Level 2"
                        }
                    );
                    assert_eq!(
                        app.world().resource::<ChunkStore>().block_at(position),
                        Some(82)
                    );
                    assert!(
                        matches!(app.world().resource::<TargetWorld>().0, Some(WorldTarget::Block(hit)) if hit.position == position)
                    );
                    assert!(app.world().resource::<MiningVisual>().target.is_none());
                }
            }
        }
    }

    fn farmland_scene(support: [i64; 3]) -> ChunkStore {
        let (section, local) = terra_voxel::global_to_section(support);
        let mut data = terra_voxel::BlockSection::empty(section).unwrap();
        data.set_block(local, terra_core::farming::FARMLAND)
            .unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        chunks
    }

    #[test]
    fn empty_or_removed_farmland_reports_no_seeds_even_with_a_stale_progress_snapshot() {
        let support = [-17, 15, -1];
        let section = terra_voxel::global_to_section(support).0;
        let mut chunks = farmland_scene(support);
        let mut session = ClientSession::default();
        session.crop_progress.insert(
            support,
            terra_network::protocol::session::CropProgress {
                support,
                kind: terra_core::farming::CropKind::Lettuce,
                prop_id: 103,
                progress: 0.574,
                moisture: 0.8,
            },
        );
        for removed in [false, true] {
            if removed {
                chunks
                    .loaded
                    .get_mut(&section)
                    .unwrap()
                    .props
                    .push(WorldProp {
                        id: PropId(103),
                        kind: PropKind::Lettuce,
                        position: [
                            support[0] as f64 + 0.5,
                            support[1] as f64 + 1.0,
                            support[2] as f64 + 0.5,
                        ],
                        rotation: 0.0,
                        scale: 1.0,
                        variant: 4,
                        section,
                        removed: true,
                    });
            }
            let row = farmland_stat(&chunks, Some(&session), support);
            assert_eq!(row.id, "crop-seeds");
            assert_eq!(row.text, "Keine Samen erkannt");
            assert!(!row.bar);
            assert_eq!(row.value, 0.0);
        }
    }

    #[test]
    fn planted_farmland_and_crop_share_exact_matching_growth_and_reject_old_crop_ids_or_kinds() {
        let support = [-17, 15, -1];
        let section = terra_voxel::global_to_section(support).0;
        let mut chunks = farmland_scene(support);
        let mut prop = WorldProp {
            id: PropId(104),
            kind: PropKind::Lettuce,
            position: [
                support[0] as f64 + 0.5,
                support[1] as f64 + 1.0,
                support[2] as f64 + 0.5,
            ],
            rotation: 0.0,
            scale: 1.0,
            variant: 2,
            section,
            removed: false,
        };
        chunks.loaded.get_mut(&section).unwrap().props.push(prop);
        let mut session = ClientSession::default();
        session.crop_progress.insert(
            support,
            terra_network::protocol::session::CropProgress {
                support,
                kind: terra_core::farming::CropKind::Lettuce,
                prop_id: prop.id.0,
                progress: 0.574,
                moisture: 0.8,
            },
        );
        let row = farmland_stat(&chunks, Some(&session), support);
        assert_eq!(row, crop_growth(crop_progress(&prop, Some(&session))));
        assert!(row.bar);
        assert_eq!(row.text, "57%");
        assert!((row.value - 57.4).abs() < 0.0001);
        assert!((crop_progress(&prop, None) - 2.0 / 7.0).abs() < 0.00001);
        session.crop_progress.get_mut(&support).unwrap().prop_id -= 1;
        assert!((crop_progress(&prop, Some(&session)) - 2.0 / 7.0).abs() < 0.00001);
        session.crop_progress.get_mut(&support).unwrap().prop_id = prop.id.0;
        prop.kind = PropKind::Wheat;
        assert!((crop_progress(&prop, Some(&session)) - 2.0 / 7.0).abs() < 0.00001);
        chunks.loaded.get_mut(&section).unwrap().props.clear();
        assert!(!farmland_stat(&chunks, Some(&session), support).bar);
    }

    #[test]
    fn crop_bar_uses_actual_fraction_not_quantized_model_stage() {
        let row = crop_growth(0.574);
        assert!(row.bar);
        assert_eq!(row.label, "Wachstum");
        assert_eq!(row.text, "57%");
        assert!((row.value - 57.4).abs() < 0.001);
        assert_eq!(crop_growth(0.0).value, 0.0);
        assert_eq!(crop_growth(1.0).label, "Erntereif");
        assert_eq!(crop_growth(1.5).value, 100.0);
    }

    #[test]
    fn crop_water_display_matches_real_dry_threshold_and_ignores_old_occupants() {
        use terra_core::CropKind;
        let support = [-17, 15, -1];
        let prop = WorldProp {
            id: PropId(104),
            kind: PropKind::Lettuce,
            position: [-16.5, 16.0, -0.5],
            rotation: 0.0,
            scale: 1.0,
            variant: 2,
            section: terra_voxel::global_to_section(support).0,
            removed: false,
        };
        let mut session = ClientSession::default();
        for (moisture, expected) in [
            (0.0, "Giessen erforderlich"),
            (terra_core::farming::DRY_MOISTURE, "Giessen erforderlich"),
            (0.15, "Bald giessen"),
            (0.25, "Bald giessen"),
            (0.8, "Ausreichend versorgt"),
        ] {
            session.crop_progress.insert(
                support,
                terra_network::protocol::session::CropProgress {
                    support,
                    kind: CropKind::Lettuce,
                    prop_id: prop.id.0,
                    progress: 0.4,
                    moisture,
                },
            );
            let mut rows = Vec::new();
            crop_water_stats(&mut rows, &prop, Some(&session), None);
            assert_eq!(rows.len(), 2);
            assert!(rows[0].bar);
            assert_eq!(rows[0].value, moisture * 100.0);
            assert_eq!(rows[1].text, expected);
        }
        session.crop_progress.get_mut(&support).unwrap().progress = 1.0;
        session.crop_progress.get_mut(&support).unwrap().moisture = 0.0;
        let mut rows = Vec::new();
        crop_water_stats(&mut rows, &prop, Some(&session), None);
        assert_eq!(rows[1].text, "Erntereif");
        for stale in [true, false] {
            let sample = session.crop_progress.get_mut(&support).unwrap();
            sample.prop_id = if stale { 103 } else { prop.id.0 };
            sample.kind = if stale {
                CropKind::Lettuce
            } else {
                CropKind::Wheat
            };
            let mut rows = Vec::new();
            crop_water_stats(&mut rows, &prop, Some(&session), None);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].text, "Wird erfasst");
        }
        let mut journal = terra_world::edits::EditJournal::default();
        journal
            .record(support, terra_core::farming::FARMLAND)
            .unwrap();
        journal.place_prop(prop).unwrap();
        let mut state = terra_core::CropState::new(CropKind::Lettuce, prop.id.0);
        state.progress_seconds = state.growth_duration_seconds * 2.0 / 7.0 + 0.001;
        state.moisture = 0.73;
        journal.set_crop(support, state).unwrap();
        let mut rows = Vec::new();
        crop_water_stats(&mut rows, &prop, Some(&session), Some(&journal));
        assert_eq!(rows[0].value, 73.0);
        assert_eq!(rows[1].text, "Ausreichend versorgt");
    }
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
    #[test]
    fn target_hud_shows_trust_owner_and_food_but_no_disabled_companion_mode() {
        use terra_core::creatures::{CreatureKind as K, taming::*};
        let viewer = terra_core::PlayerUuid::from_u128(1);
        let mut rows = Vec::new();
        companion_stats(&mut rows, K::Sheep, None, Some(viewer));
        assert_eq!(rows[0].value, 0.0);
        assert_eq!(rows[1].text, "Wild");
        assert_eq!(rows[2].text, "Weizen");
        let mut bond = CompanionView {
            id: 1,
            owner: None,
            player: viewer,
            trust: 50,
            mode: CompanionMode::Wait,
        };
        rows.clear();
        companion_stats(&mut rows, K::Horse, Some(&bond), Some(viewer));
        assert_eq!(rows[0].value, 50.0);
        assert_eq!(rows[2].text, "Weizen, Mais, Salat");
        bond.trust = 100;
        bond.owner = Some(viewer);
        bond.mode = CompanionMode::Follow;
        rows.clear();
        companion_stats(&mut rows, K::Horse, Some(&bond), Some(viewer));
        assert_eq!(rows[1].text, "Du");
        assert_eq!(rows.len(), 2);
        rows.clear();
        bond.mode = CompanionMode::Wait;
        companion_stats(&mut rows, K::Horse, Some(&bond), None);
        assert!(rows[1].text.starts_with("Spieler "));
        assert_eq!(rows.len(), 2);
        rows.clear();
        companion_stats(&mut rows, K::Fish, None, Some(viewer));
        assert!(rows.is_empty());
    }

    #[test]
    fn offspring_growth_and_parent_cooldown_are_actual_target_stats_not_companion_controls() {
        use terra_core::creatures::{
            Creature, CreatureBehavior, CreatureKind, CreatureNeeds,
            breeding::{self, AnimalFamily, FamilyView},
        };
        let mut child = Creature {
            id: breeding::next_birth_id(&Default::default(), 0).unwrap(),
            kind: CreatureKind::Cow,
            home: [0, 0],
            position: [1.0, 3.0, 1.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 10.0,
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 600.0,
            variant: 0,
            needs: CreatureNeeds::full(CreatureKind::Cow),
        };
        let family = AnimalFamily {
            parents: Some([4, 5]),
            active_age: 600.0,
            ready_age: breeding::GROWTH_SECONDS,
            willing_until_age: 0.0,
        };
        let mut rows = Vec::new();
        family_stats(&mut rows, &child, Some(&FamilyView::new(&child, family)));
        assert!(
            rows.iter()
                .any(|r| r.id == "animal-growth" && r.bar && r.value == 50.0 && r.text == "50%")
        );
        assert!(
            rows.iter()
                .any(|r| r.id == "animal-breeding" && r.text == "Zu jung")
        );
        assert!(rows.iter().all(|r| r.id != "animal-companion"));
        child.age = breeding::GROWTH_SECONDS;
        rows.clear();
        let cooldown = AnimalFamily {
            ready_age: child.age + 300.0,
            ..family
        };
        family_stats(&mut rows, &child, Some(&FamilyView::new(&child, cooldown)));
        assert!(!rows.iter().any(|r| r.id == "animal-growth"));
        assert_eq!(rows[0].text, "5:00");
        let sex = animal_sex(&child);
        assert_eq!(sex.id, "animal-sex");
        assert_eq!(sex.label, "Geschlecht");
        assert_eq!(sex.text, "Maennlich");
        assert!(!sex.bar);
        child.id = (1 << 62) + 1;
        assert_eq!(animal_sex(&child).text, "Weiblich");
    }
}
