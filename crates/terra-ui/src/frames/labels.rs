//! Translated control captions stay on one line. Descriptive copy can wrap at
//! word boundaries; arbitrary character breaks must not split German compounds.
use bevy::{prelude::*, text::LineBreak};
use tilt_ui::{ElementClasses, ElementKind, TiltElement};

type Ancestors<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static TiltElement>,
        Option<&'static ElementClasses>,
        Option<&'static ChildOf>,
    ),
>;

fn caption_break(mut entity: Entity, ancestors: &Ancestors) -> Option<LineBreak> {
    let mut caption = false;
    for _ in 0..32 {
        let Ok((element, classes, parent)) = ancestors.get(entity) else {
            break;
        };
        if classes.is_some_and(|classes| {
            classes.classes.iter().any(|class| {
                matches!(
                    class.as_str(),
                    "setting-hint"
                        | "keep-roster-button"
                        | "build-mode-template"
                        | "recipe-alternative"
                        | "editor-item"
                )
            })
        }) {
            return Some(LineBreak::WordBoundary);
        }
        if let Some(element) = element {
            match element.kind {
                // Editable values/carets and single-line dropdown rows have
                // their own layout and must retain it.
                ElementKind::Input
                | ElementKind::TextArea
                | ElementKind::ChoiceBox
                | ElementKind::Option
                | ElementKind::ColorPicker
                | ElementKind::Icon => return None,
                ElementKind::Button | ElementKind::ToggleButton | ElementKind::Label => {
                    caption = true;
                }
                _ => {}
            }
        }
        caption |= classes.is_some_and(|classes| {
            classes.classes.iter().any(|class| {
                matches!(
                    class.as_str(),
                    "setting-title"
                        | "setting-label"
                        | "surface-label"
                        | "button-title"
                        | "kingdom-location"
                )
            })
        });
        let Some(parent) = parent else { break };
        entity = parent.parent();
    }
    caption.then_some(LineBreak::NoWrap)
}

#[allow(clippy::type_complexity)] // Caption updates are filtered to retained text/style changes.
pub(super) fn wrap_control_captions(
    ancestors: Ancestors,
    mut captions: Query<
        (Entity, &Text, &mut TextLayout),
        Or<(
            Added<Text>,
            Changed<Text>,
            Changed<TextLayout>,
            Changed<ChildOf>,
        )>,
    >,
) {
    for (entity, text, mut layout) in &mut captions {
        // Slot counts and icon-only controls keep their compact geometry.
        if !text.0.chars().any(char::is_alphabetic) {
            continue;
        }
        let Some(linebreak) = caption_break(entity, &ancestors) else {
            continue;
        };
        // Widths belong to the control/group, never to percentage-constrained
        // text children. This avoids intrinsic-size cycles and narrow labels.
        if layout.linebreak != linebreak {
            layout.linebreak = linebreak;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, wrap_control_captions);
        app
    }

    #[test]
    fn translated_captions_keep_whole_words_and_their_intrinsic_geometry() {
        let mut app = app();
        let button = app
            .world_mut()
            .spawn(TiltElement {
                kind: ElementKind::Button,
            })
            .id();
        let text = app
            .world_mut()
            .spawn((
                Text::new("Mehrspielereinstellungen zurücksetzen"),
                TextLayout::linebreak(LineBreak::WordBoundary),
                Node {
                    min_width: Val::Px(280.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                ChildOf(button),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<TextLayout>(text).unwrap().linebreak,
            LineBreak::NoWrap
        );
        let node = app.world().get::<Node>(text).unwrap();
        assert_eq!(node.min_width, Val::Px(280.0));
        assert_eq!(node.max_width, Val::Auto);
        assert_eq!(node.flex_shrink, 0.0);
        assert_eq!(
            app.world().get::<Text>(text).unwrap().0,
            "Mehrspielereinstellungen zurücksetzen"
        );
    }

    #[test]
    fn explicitly_single_line_button_captions_keep_their_authored_layout() {
        let mut app = app();
        let button = app
            .world_mut()
            .spawn(TiltElement {
                kind: ElementKind::Button,
            })
            .id();
        let caption = app
            .world_mut()
            .spawn((
                Text::new("Gründen"),
                TextLayout::linebreak(LineBreak::NoWrap),
                ChildOf(button),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<TextLayout>(caption).unwrap().linebreak,
            LineBreak::NoWrap
        );
    }

    #[test]
    fn editable_values_dropdown_options_and_icons_keep_their_authored_line_breaks() {
        let mut app = app();
        let button = app
            .world_mut()
            .spawn(TiltElement {
                kind: ElementKind::Button,
            })
            .id();
        let mut labels = Vec::new();
        for kind in [
            ElementKind::Input,
            ElementKind::TextArea,
            ElementKind::ChoiceBox,
            ElementKind::Option,
            ElementKind::Icon,
        ] {
            let root = app
                .world_mut()
                .spawn((TiltElement { kind }, ChildOf(button)))
                .id();
            labels.push(
                app.world_mut()
                    .spawn((
                        Text::new("Inventarbenutzertext"),
                        TextLayout::linebreak(LineBreak::NoWrap),
                        ChildOf(root),
                    ))
                    .id(),
            );
        }
        app.update();
        for label in labels {
            assert_eq!(
                app.world().get::<TextLayout>(label).unwrap().linebreak,
                LineBreak::NoWrap
            );
        }
    }

    #[test]
    fn numeric_slot_counts_and_unrelated_world_text_are_not_reformatted() {
        let mut app = app();
        let button = app
            .world_mut()
            .spawn(TiltElement {
                kind: ElementKind::Button,
            })
            .id();
        let count = app
            .world_mut()
            .spawn((
                Text::new("128"),
                TextLayout::linebreak(LineBreak::NoWrap),
                ChildOf(button),
            ))
            .id();
        let unrelated = app
            .world_mut()
            .spawn((
                Text::new("Freier Welttext"),
                TextLayout::linebreak(LineBreak::NoWrap),
            ))
            .id();
        app.update();
        for label in [count, unrelated] {
            assert_eq!(
                app.world().get::<TextLayout>(label).unwrap().linebreak,
                LineBreak::NoWrap
            );
        }
    }
}

#[cfg(test)]
mod description_tests {
    use super::*;
    #[test]
    fn hints_wrap_at_spaces_while_setting_titles_keep_german_compounds_whole() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, wrap_control_captions);
        let mut labels = Vec::new();
        for (class, expected) in [
            ("setting-title", LineBreak::NoWrap),
            ("setting-hint", LineBreak::WordBoundary),
        ] {
            let entity = app
                .world_mut()
                .spawn((
                    Text::new("Mehrspielereinstellungen zurücksetzen"),
                    TextLayout::linebreak(LineBreak::WordOrCharacter),
                    ElementClasses {
                        classes: vec![class.into()],
                    },
                ))
                .id();
            labels.push((entity, expected));
        }
        app.update();
        for (entity, expected) in labels {
            assert_eq!(
                app.world().get::<TextLayout>(entity).unwrap().linebreak,
                expected
            );
        }
    }
}
