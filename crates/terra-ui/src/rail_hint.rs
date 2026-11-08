//! Rail construction feedback never changes the looked-at block's presentation.
use bevy::prelude::*;
use serde::Serialize;
use terra_core::{ControlsConfig, GameAction, ItemKind, rails};
use terra_player::interaction::{ActiveItem, PlacementPreview};
use tilt_ui::html_shared;

#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct RailBuildView {
    pub visible: bool,
    pub invalid: bool,
    pub title: String,
    pub instruction: String,
    pub detail: String,
    pub cost: String,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<RailBuildView>().add_systems(
        Update,
        update
            .run_if(crate::cadence::ready)
            .after(terra_player::PlayerSet::Interaction),
    );
}

fn presentation(
    preview: &PlacementPreview,
    item: ItemKind,
    controls: &ControlsConfig,
) -> RailBuildView {
    let material = item.block_material().unwrap_or_default();
    let switch = matches!(material, rails::SWITCH_LEFT | rails::SWITCH_RIGHT);
    let binding = |action| short_binding(controls.binding(action));
    let title = if preview.rail_ramp {
        "Sanfte Rampe"
    } else if preview.rail_choice == Some(rails::BuildChoice::Curve) {
        "Kurve"
    } else if preview.rail_choice == Some(rails::BuildChoice::Straight) {
        "Geradeaus"
    } else {
        match material {
            rails::SWITCH_LEFT => "Weiche links",
            rails::SWITCH_RIGHT => "Weiche rechts",
            rails::CROSSING => "Altes Gleis",
            rails::BUFFER => "Prellbock",
            _ => match preview.rail_mode {
                1 => "Kurve",
                2 => "Prellbock",
                _ => "Gleis",
            },
        }
    };
    let instruction = if switch {
        format!("{}: Setzen", binding(GameAction::Place))
    } else {
        format!(
            "{}{}: Form · {}: {}",
            binding(GameAction::Use),
            if preview.rail_options {
                " / Mausrad"
            } else {
                ""
            },
            binding(GameAction::Place),
            if preview.rail_start.is_some() {
                "Ziel"
            } else if preview.rail_mode == 1 && preview.rail_choice.is_none() {
                "Start"
            } else {
                "Bauen"
            }
        )
    };
    let detail = if let Some(error) = &preview.rail_error {
        short_error(error)
    } else if preview.rail_start.is_some() {
        format!(
            "{}+{}: Abbrechen",
            binding(GameAction::FlyDown),
            binding(GameAction::Place)
        )
    } else if preview.rail_mode == 1 && preview.rail_choice.is_none() {
        "Freies Gleisende wählen".into()
    } else if preview.rail_replacing {
        "Endstück wird zur Rampe".into()
    } else if switch {
        "Weichensteller zum Umschalten".into()
    } else {
        format!(
            "{}+{}: Start merken",
            binding(GameAction::FlyDown),
            binding(GameAction::Place)
        )
    };
    let cost = if (1..u16::MAX).contains(&preview.rail_cost) {
        format!(
            "{}{} {}",
            if preview.rail_replacing { "+" } else { "" },
            preview.rail_cost,
            if preview.rail_cost == 1 {
                "Gleis"
            } else {
                "Gleise"
            }
        )
    } else if preview.rail_replacing && preview.rail_cost == 0 {
        "Kein Mehrbedarf".into()
    } else {
        String::new()
    };
    RailBuildView {
        visible: true,
        invalid: preview.rail_error.is_some(),
        title: title.into(),
        instruction,
        detail,
        cost,
    }
}

fn short_binding(binding: terra_core::InputBinding) -> String {
    use terra_core::InputBinding;
    match binding {
        InputBinding::Mouse(MouseButton::Left) => "LMB".into(),
        InputBinding::Mouse(MouseButton::Right) => "RMB".into(),
        InputBinding::Mouse(MouseButton::Middle) => "MMB".into(),
        InputBinding::Key(KeyCode::ShiftLeft | KeyCode::ShiftRight) => "Shift".into(),
        InputBinding::Key(KeyCode::ControlLeft | KeyCode::ControlRight) => "Strg".into(),
        InputBinding::Key(KeyCode::AltLeft | KeyCode::AltRight) => "Alt".into(),
        InputBinding::Chord { modifier, key } => format!(
            "{}+{}",
            short_binding(InputBinding::Key(modifier)),
            short_binding(InputBinding::Key(key))
        ),
        other => other.label(),
    }
}

/// Keep construction failures readable without exposing long planner diagnostics.
fn short_error(error: &str) -> String {
    let lower = error.to_lowercase();
    let message = if lower.contains("supporting floor") {
        "Tragfähiger Untergrund fehlt"
    } else if lower.contains("too close") || lower.contains("turning radius") {
        "Mehr Platz für die Kurve nötig"
    } else if lower.contains("farther than") || lower.contains("zu weit") {
        "Gleisende außer Reichweite"
    } else if lower.contains("same wood") {
        "Gleiche Holzart verwenden"
    } else if lower.contains("face away") || lower.contains("no free end") {
        "Freies Gleisende in Fahrtrichtung wählen"
    } else if lower.contains("overlaps an existing") {
        "Gleise überlappen · Freies Ende wählen"
    } else if lower.contains("occupied") || lower.contains("blockiert") {
        "Gleisbereich blockiert"
    } else if lower.contains("incomplete") || lower.contains("vollständig geladen") {
        "Gleisanschluss noch nicht vollständig geladen"
    } else {
        error
    };
    if message.chars().count() <= 80 {
        return message.into();
    }
    let prefix: String = message.chars().take(77).collect();
    let end = prefix.rfind(char::is_whitespace).unwrap_or(prefix.len());
    format!("{}…", prefix[..end].trim_end())
}

#[derive(bevy::ecs::system::SystemParam)]
struct Scene<'w> {
    preview: Option<Res<'w, PlacementPreview>>,
    active: Option<Res<'w, ActiveItem>>,
    controls: Option<Res<'w, ControlsConfig>>,
    blocked: Option<Res<'w, terra_player::PlayerInputBlock>>,
    access: Res<'w, terra_world::streaming::WorldAccess>,
    debug: Res<'w, terra_core::DebugSettings>,
    menu: Option<Res<'w, crate::menu::MenuState>>,
}

fn update(scene: Scene, mut view: ResMut<RailBuildView>) {
    let item = scene
        .active
        .as_ref()
        .and_then(|active| active.0)
        .filter(|item| item.block_material().is_some_and(rails::is_rail));
    let allowed = scene.access.ready()
        && !scene.debug.enabled
        && !scene.blocked.is_some_and(|blocked| blocked.0)
        && scene
            .menu
            .as_ref()
            .is_none_or(|menu| menu.page == crate::menu::MenuPage::Playing);
    let next = if allowed && let (Some(item), Some(preview)) = (item, scene.preview) {
        presentation(
            &preview,
            item,
            scene
                .controls
                .as_deref()
                .unwrap_or(&ControlsConfig::default()),
        )
    } else {
        RailBuildView::default()
    };
    if *view != next {
        *view = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_instruction_is_limited_to_docking_choices_and_ramps_show_only_extra_cost() {
        let controls = ControlsConfig::default();
        let mut preview = PlacementPreview {
            rail_choice: Some(rails::BuildChoice::Curve),
            rail_options: true,
            ..Default::default()
        };
        let view = presentation(&preview, ItemKind::Block(rails::STRAIGHT), &controls);
        assert_eq!(view.title, "Kurve");
        assert!(view.instruction.contains("Mausrad"));
        preview.rail_options = false;
        preview.rail_choice = Some(rails::BuildChoice::Straight);
        preview.rail_replacing = true;
        preview.rail_ramp = true;
        preview.rail_cost = 1;
        let view = presentation(&preview, ItemKind::Block(rails::STRAIGHT), &controls);
        assert_eq!(view.title, "Sanfte Rampe");
        assert!(!view.instruction.contains("Mausrad"));
        assert_eq!(view.cost, "+1 Gleis");
        let ordinary = presentation(
            &PlacementPreview::default(),
            ItemKind::Block(rails::STRAIGHT),
            &controls,
        );
        assert!(!ordinary.instruction.contains("Mausrad"));
    }

    #[test]
    fn independent_hint_uses_actual_bindings_and_keeps_invalid_curve_feedback_visible() {
        let mut controls = ControlsConfig::default();
        controls.bindings.insert(
            GameAction::Place,
            terra_core::InputBinding::Key(KeyCode::KeyB),
        );
        let preview = PlacementPreview {
            rail_start: Some(rails::Endpoint {
                anchor: [0; 3],
                port: 1,
            }),
            rail_error: Some("Die Gleisenden benötigen mehr Abstand für die Kurve".into()),
            rail_cost: 3,
            ..Default::default()
        };
        let view = presentation(&preview, ItemKind::Block(rails::STRAIGHT), &controls);
        assert!(view.visible && view.invalid);
        assert!(view.instruction.contains('B'));
        assert!(view.detail.contains("mehr Abstand"));
        assert_eq!(view.cost, "3 Gleise");
        let switch = presentation(
            &PlacementPreview::default(),
            ItemKind::Block(rails::SWITCH_LEFT),
            &controls,
        );
        assert_eq!(switch.title, "Weiche links");
        assert!(switch.detail.contains("Weichensteller"));
        let long = short_error(&"ö".repeat(180));
        assert!(long.chars().count() <= 80);
        assert_eq!(
            short_error("Every rail sleeper needs a loaded, full-height supporting floor"),
            "Tragfähiger Untergrund fehlt"
        );
    }
}
