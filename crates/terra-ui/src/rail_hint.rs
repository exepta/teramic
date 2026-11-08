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
    let binding = |action| controls.binding(action).label();
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
            rails::CROSSING => "Kreuzung",
            rails::BUFFER => "Prellbock",
            _ => match preview.rail_mode {
                1 => "Kreuzung",
                2 => "Prellbock",
                _ => "Gleis",
            },
        }
    };
    let instruction = if switch {
        format!(
            "{}: Weiche setzen · Freie Enden mit Gleisen verlängern",
            binding(GameAction::Place)
        )
    } else if preview.rail_options {
        format!(
            "{}: Vorschau bauen · Mausrad: Geradeaus / Kurve · {}+{}: Start merken",
            binding(GameAction::Place),
            binding(GameAction::FlyDown),
            binding(GameAction::Place)
        )
    } else if preview.rail_replacing {
        format!(
            "{}: Vorhandenes Endstück zur Rampe anpassen",
            binding(GameAction::Place)
        )
    } else if preview.rail_choice.is_some() {
        format!(
            "{}: Am Gleisende anbauen · {}+{} am Gleisende: Start merken",
            binding(GameAction::Place),
            binding(GameAction::FlyDown),
            binding(GameAction::Place)
        )
    } else if preview.rail_start.is_some() {
        format!(
            "{}: Ziel bestätigen · {}+{}: Abbrechen",
            binding(GameAction::Place),
            binding(GameAction::FlyDown),
            binding(GameAction::Place)
        )
    } else {
        format!(
            "{}: Gleisende wählen / Bauen · {}: Bauform",
            binding(GameAction::Place),
            binding(GameAction::Use)
        )
    };
    let mut detail = preview.rail_error.clone().unwrap_or_else(|| {
        if preview.rail_replacing {
            "Der vorhandene Anschluss bleibt erhalten".into()
        } else if preview.rail_start.is_some() && preview.plan.is_none() {
            "Zum Ziel gehen und auf Boden oder ein freies Gleisende zeigen".into()
        } else if switch {
            "Zum Schalten einen Weichensteller daneben setzen".into()
        } else {
            "Kurven verbinden freie Enden · Für Abzweigungen eine Weiche setzen".into()
        }
    });
    if (1..u16::MAX).contains(&preview.rail_cost) {
        detail = format!(
            "{detail} · {}: {}",
            if preview.rail_replacing {
                "Zusätzlich"
            } else {
                "Bedarf"
            },
            preview.rail_cost
        );
    } else if preview.rail_replacing && preview.rail_cost == 0 {
        detail = format!("{detail} · Keine zusätzlichen Schienen");
    }
    RailBuildView {
        visible: true,
        invalid: preview.rail_error.is_some(),
        title: title.into(),
        instruction,
        detail,
    }
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
        assert!(view.detail.contains("Zusätzlich: 1"));
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
        assert!(view.detail.contains("mehr Abstand") && view.detail.ends_with("Bedarf: 3"));
        let switch = presentation(
            &PlacementPreview::default(),
            ItemKind::Block(rails::SWITCH_LEFT),
            &controls,
        );
        assert_eq!(switch.title, "Weiche links");
        assert!(switch.detail.contains("Weichensteller"));
    }
}
