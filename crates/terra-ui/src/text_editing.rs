//! Compatibility for programmatic TiltUI replacements in Parley 0.9.
use bevy::{
    prelude::*,
    text::{EditableText, FontCx, LayoutCx, apply_text_edits},
};

pub(crate) fn install(app: &mut App) {
    app.add_systems(PostUpdate, refresh_edit_layouts.before(apply_text_edits));
}

fn refresh_edit_layouts(
    mut inputs: Query<&mut EditableText, Changed<EditableText>>,
    mut fonts: ResMut<FontCx>,
    mut layouts: ResMut<LayoutCx>,
) {
    for mut input in &mut inputs {
        if (!input.pending_edits.is_empty() || input.pending_paste.is_some())
            && input.editor.try_layout().is_none()
        {
            // set_text invalidates the layout but keeps its old byte-index selection.
            // Refresh before an insertion/deletion can use that stale selection.
            input.editor.refresh_layout(&mut fonts, &mut layouts.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (EditableText, FontCx, LayoutCx) {
        let mut fonts = FontCx::default();
        let mut layouts = LayoutCx::default();
        let mut input = EditableText::new("abc");
        input.pending_edits.clear();
        input
            .editor
            .driver(&mut fonts, &mut layouts.0)
            .move_to_text_end();
        assert_eq!(input.editor.raw_selection().text_range(), 3..3);
        (input, fonts, layouts)
    }

    #[test]
    #[should_panic(expected = "char_boundary")]
    fn reproduces_stale_selection_inside_replacement_emoji() {
        let (mut input, mut fonts, mut layouts) = fixture();
        input.editor.set_text("\u{1f41d}");
        input
            .editor
            .driver(&mut fonts, &mut layouts.0)
            .insert_or_replace_selection("x");
    }

    #[test]
    fn refreshes_programmatic_replacements_before_pending_edits() {
        for replacement in ["\u{1f41d}", "\u{e4}", "", "\u{65}\u{301}", "short"] {
            let (mut input, fonts, layouts) = fixture();
            input.editor.set_text(replacement);
            input.queue_edit(bevy::text::TextEdit::TextEnd(false));
            let mut app = App::new();
            app.insert_resource(fonts)
                .insert_resource(layouts)
                .add_systems(Update, refresh_edit_layouts);
            let entity = app.world_mut().spawn(input).id();
            app.update();
            app.world_mut()
                .resource_scope(|world, mut fonts: Mut<FontCx>| {
                    world.resource_scope(|world, mut layouts: Mut<LayoutCx>| {
                        let mut input = world.get_mut::<EditableText>(entity).unwrap();
                        let range = input.editor.raw_selection().text_range();
                        assert!(input.editor.raw_text().is_char_boundary(range.start));
                        assert!(input.editor.raw_text().is_char_boundary(range.end));
                        input
                            .editor
                            .driver(&mut fonts, &mut layouts.0)
                            .insert_or_replace_selection("x");
                        assert_eq!(
                            input
                                .editor
                                .raw_text()
                                .chars()
                                .filter(|c| *c == 'x')
                                .count(),
                            1
                        );
                    });
                });
        }
    }

    #[test]
    fn queued_insert_preceding_tilt_cursor_reset_uses_refreshed_selection() {
        let (mut input, fonts, layouts) = fixture();
        input.queue_edit(bevy::text::TextEdit::Insert("x".into()));
        input.editor.set_text("\u{1f41d}");
        input.queue_edit(bevy::text::TextEdit::TextEnd(false));
        let mut app = App::new();
        app.insert_resource(fonts)
            .insert_resource(layouts)
            .init_resource::<bevy::clipboard::Clipboard>()
            .add_systems(PostUpdate, apply_text_edits);
        install(&mut app);
        let entity = app.world_mut().spawn(input).id();
        app.update();
        let input = app.world().get::<EditableText>(entity).unwrap();
        assert!(input.pending_edits.is_empty());
        assert_eq!(input.editor.raw_text().replace('x', ""), "\u{1f41d}");
        assert_eq!(input.editor.raw_text().chars().count(), 2);
        let end = input.editor.raw_text().len();
        assert_eq!(input.editor.raw_selection().text_range(), end..end);
    }
}
