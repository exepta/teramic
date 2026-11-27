//! Bounded status cards and four recycled one-second regeneration sprites.
use bevy::prelude::*;
use serde::Serialize;
use terra_core::{EffectKind, SurvivalVitals, effect_definition};
use terra_network::client::ClientSession;
use tilt_ui::{ElementClasses, ElementId, html_shared};

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct EffectCardView {
    pub id: String,
    pub name: String,
    pub time_label: String,
    pub level: u8,
    pub icon_class: String,
    pub night_vision: bool,
    pub tooltip: String,
    pub class: String,
}

#[html_shared]
#[derive(Resource, Debug, Default, Serialize, PartialEq)]
pub struct StatusEffectView {
    pub cards: Vec<EffectCardView>,
    pub overflow: usize,
    pub overflow_title: String,
    pub expanded: bool,
    pub regeneration: bool,
    pub shield: bool,
    pub shield_value: f32,
    pub shield_label: String,
}

#[derive(Component)]
struct EffectIcon(usize);

pub(super) fn install(app: &mut App) {
    app.init_resource::<StatusEffectView>()
        .add_systems(
            Update,
            update
                .run_if(crate::cadence::ready)
                .after(super::SurvivalUiSet)
                .before(terra_player::PlayerSet::Movement),
        )
        .add_systems(
            PostUpdate,
            (tag_icons, atlas_frames, float_hearts)
                .chain()
                .before(bevy::ui::UiSystems::Prepare),
        );
}

fn duration_label(seconds: f32, exposed: bool) -> String {
    if exposed {
        return "--:--".into();
    }
    let seconds = seconds
        .ceil()
        .clamp(0.0, terra_core::effects::MAX_EFFECT_SECONDS) as u32;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn describe(vitals: SurvivalVitals, compact: bool) -> StatusEffectView {
    if vitals.active_effects().next().is_none() {
        return StatusEffectView::default();
    }
    let active: Vec<_> = vitals.active_effects().collect();
    let expanded = active.len() <= 2 && !compact;
    let limit = if expanded { 2 } else { 3 };
    let mut overflow = Vec::new();
    let mut cards = Vec::with_capacity(limit);
    for effect in active {
        let definition = effect_definition(effect.kind);
        let exposed = effect.kind == EffectKind::Poison && vitals.effects.smoke_exposed();
        let time_label = duration_label(effect.remaining_seconds, exposed);
        let duration = if exposed {
            "While in smoke"
        } else {
            &time_label
        };
        let tooltip = format!(
            "{} {} | {}\n{}",
            definition.name, effect.level, duration, definition.description
        );
        if cards.len() == limit {
            overflow.push(format!(
                "{} {} | {}",
                definition.name, effect.level, duration
            ));
            continue;
        }
        cards.push(EffectCardView {
            id: definition.id.clone(),
            name: definition.name.clone(),
            time_label,
            level: effect.level,
            icon_class: format!("atlas-cell-{}", effect.kind.index()),
            night_vision: effect.kind == EffectKind::NightVision,
            tooltip,
            class: format!(
                "status-effect-card {} {} effect-{}",
                if expanded {
                    "effect-expanded"
                } else {
                    "effect-compact"
                },
                if definition.beneficial {
                    "effect-buff"
                } else {
                    "effect-debuff"
                },
                effect.kind.index()
            ),
        });
    }
    let shield = vitals.effects.shield_points();
    let capacity = vitals.effects.shield_capacity();
    StatusEffectView {
        regeneration: super::chrome::primary_effect(vitals, true) == Some(EffectKind::Regeneration),
        cards,
        overflow: overflow.len(),
        overflow_title: overflow.join("\n\n"),
        expanded,
        shield: capacity > 0.0 && shield > 0.0,
        shield_value: if capacity > 0.0 {
            shield / capacity * 100.0
        } else {
            0.0
        },
        shield_label: format!(
            "Shield Power: {} / {}",
            shield.ceil() as u16,
            capacity.ceil() as u16
        ),
    }
}

fn update(
    session: Option<Res<ClientSession>>,
    hud: Res<super::SurvivalView>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Option<Res<bevy::ui::UiScale>>,
    mut view: ResMut<StatusEffectView>,
) {
    let compact = windows.iter().next().is_some_and(|window| {
        window.width() / scale.as_ref().map_or(1.0, |s| s.0).max(0.1) < 420.0
    });
    let next = if hud.visible && !hud.dead && !hud.editing {
        session
            .filter(|s| s.ready && s.game_mode == terra_core::GameMode::Survival)
            .map_or_else(StatusEffectView::default, |s| {
                describe(s.stats.survival, compact)
            })
    } else {
        StatusEffectView::default()
    };
    if *view != next {
        *view = next;
    }
}

type IconChanges<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static ElementClasses, Option<&'static EffectIcon>),
    (
        With<ImageNode>,
        Or<(Added<ImageNode>, Changed<ElementClasses>)>,
    ),
>;

fn tag_icons(mut commands: Commands, images: IconChanges) {
    for (entity, classes, current) in &images {
        let Some(index) = classes
            .classes
            .iter()
            .find_map(|class| class.strip_prefix("atlas-cell-"))
            .and_then(|index| index.parse::<usize>().ok())
            .filter(|index| *index <= EffectKind::ShieldPower.index())
        else {
            if current.is_some() {
                commands.entity(entity).remove::<EffectIcon>();
            }
            continue;
        };
        if current.is_none_or(|icon| icon.0 != index) {
            commands.entity(entity).insert(EffectIcon(index));
        }
    }
}

fn atlas_frames(assets: Res<Assets<Image>>, mut images: Query<(&EffectIcon, &mut ImageNode)>) {
    for (icon, mut image) in &mut images {
        let Some(atlas) = assets.get(&image.image) else {
            continue;
        };
        let cell = Vec2::new(atlas.width() as f32, atlas.height() as f32) / 4.0;
        let min = Vec2::new((icon.0 % 4) as f32, (icon.0 / 4) as f32) * cell;
        let rect = Some(Rect::from_corners(min, min + cell));
        if image.rect != rect {
            image.rect = rect;
        }
    }
}

fn heart_particle(seconds: f32, index: usize) -> (f32, f32, f32) {
    let age = (seconds + index as f32 * 0.25).rem_euclid(1.0);
    let x = 13.0 + index as f32 * 1.7 + (age * 5.0 + index as f32).sin() * 1.2;
    (x, 37.0 - age * 36.0, (1.0 - age).powi(2) * 0.92)
}

fn float_hearts(
    time: Res<Time>,
    view: Res<StatusEffectView>,
    mut images: Query<(&ElementId, &mut Node, &mut ImageNode), With<EffectIcon>>,
) {
    if !view.regeneration {
        return;
    }
    for (id, mut node, mut image) in &mut images {
        let Some(index) =
            id.0.strip_prefix("regen-heart-")
                .and_then(|s| s.parse::<usize>().ok())
        else {
            continue;
        };
        let (x, y, alpha) = heart_particle(time.elapsed_secs(), index);
        node.left = Val::Percent(x);
        node.top = Val::Percent(y);
        image.color = Color::srgba(1.0, 1.0, 1.0, alpha);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translated_alt_text_still_renders_only_the_active_atlas_cell() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .add_systems(Update, (tag_icons, atlas_frames).chain());
        let mut image = Image::default();
        image.resize(bevy::render::render_resource::Extent3d {
            width: 1254,
            height: 1254,
            depth_or_array_layers: 1,
        });
        let handle = app.world_mut().resource_mut::<Assets<Image>>().add(image);
        let mut vitals = SurvivalVitals::default();
        vitals.effects.apply(EffectKind::Bleeding, 5.0, 1);
        let card = describe(vitals, false).cards.pop().unwrap();
        let entity = app
            .world_mut()
            .spawn((
                ImageNode::new(handle),
                ElementClasses {
                    classes: vec!["status-effect-icon".into(), card.icon_class],
                },
                tilt_ui::ImageMetadata {
                    alt: Some("Blutung".into()),
                    ..default()
                },
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<ImageNode>(entity).unwrap().rect,
            Some(Rect::new(627.0, 0.0, 940.5, 313.5))
        );
    }

    #[test]
    fn cards_are_bounded_and_keep_all_remaining_effects_in_overflow() {
        let mut vitals = SurvivalVitals::default();
        for kind in EffectKind::ALL {
            assert!(vitals.effects.apply(kind, 30.0, 1));
        }
        let view = describe(vitals, false);
        assert_eq!(view.cards.len(), 3);
        assert_eq!(view.overflow, terra_core::effects::EFFECT_COUNT - 3);
        assert!(!view.expanded);
        assert!(!view.regeneration && view.shield);
        assert!(view.overflow_title.contains("Regeneration"));
        assert_eq!(view.cards[0].id, "teramic::poison");
        assert_eq!(view.cards[0].time_label, "00:30");
    }

    #[test]
    fn exposure_and_food_regeneration_share_the_same_status_presentation() {
        let mut vitals = SurvivalVitals {
            regeneration_seconds: 5.0,
            regeneration_rate: 0.4,
            ..default()
        };
        vitals.effects.set_smoke_exposure(true);
        let view = describe(vitals, false);
        assert_eq!(view.cards.len(), 2);
        assert!(view.expanded && !view.regeneration);
        assert_eq!(view.cards[0].time_label, "--:--");
        assert!(view.cards[0].tooltip.contains("While in smoke"));
        assert_eq!(view.cards[1].icon_class, "atlas-cell-11");
        vitals.effects.set_smoke_exposure(false);
        assert!(describe(vitals, false).regeneration);
        assert_eq!(duration_label(0.05, false), "00:01");
        assert_eq!(duration_label(90.0, false), "01:30");
    }

    #[test]
    fn regeneration_particles_fade_and_rise_during_exactly_one_second() {
        let start = heart_particle(0.0, 0);
        let middle = heart_particle(0.5, 0);
        let end = heart_particle(0.999, 0);
        assert!(start.1 > middle.1 && middle.1 > end.1);
        assert!(start.2 > middle.2 && end.2 < 0.00001);
        assert_eq!(heart_particle(1.0, 0), start);
    }

    #[test]
    fn exhausted_shield_keeps_its_status_card_without_an_empty_overlay() {
        let mut vitals = SurvivalVitals::default();
        assert!(vitals.effects.apply(EffectKind::ShieldPower, 30.0, 1));
        let charged = describe(vitals, false);
        assert!(charged.shield);
        assert_eq!(charged.shield_value, 100.0);
        vitals.effects.absorb_damage(20.0);
        let exhausted = describe(vitals, false);
        assert!(!exhausted.shield);
        assert_eq!(exhausted.shield_value, 0.0);
        assert_eq!(exhausted.cards.len(), 1);
        assert_eq!(exhausted.cards[0].id, "teramic::shield_power");
    }

    #[test]
    fn compact_cards_keep_the_same_effects_and_no_subsecond_store_churn() {
        let mut vitals = SurvivalVitals::default();
        assert!(vitals.effects.apply(EffectKind::Freeze, 10.8, 1));
        let first = describe(vitals, true);
        assert!(!first.expanded);
        assert_eq!(first.cards.len(), 1);
        vitals.effects.advance(0.1);
        assert_eq!(first, describe(vitals, true));
        assert!(vitals.effects.remove(EffectKind::Freeze));
        assert_eq!(describe(vitals, false).cards.len(), 0);
    }
}
