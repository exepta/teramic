use super::*;
use bevy::ui::widget::ImageNode;
use tilt_ui::ImageMetadata;

fn fixture() -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Image>>()
        .init_resource::<BlockPreviewImages>()
        .init_resource::<PropPreviewImages>()
        .init_resource::<ItemPreviewImages>()
        .init_resource::<PlayerPreviewImage>()
        .add_systems(Update, sync_preview_images);
    app
}

fn image(app: &mut App) -> Handle<Image> {
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default())
}

fn node(app: &mut App, alt: &str) -> Entity {
    app.world_mut()
        .spawn((
            ImageMetadata {
                alt: Some(alt.into()),
                ..default()
            },
            ImageNode {
                color: Color::srgba(0.2, 0.3, 0.4, 0.37),
                ..default()
            },
        ))
        .id()
}

fn assert_bound(app: &App, entity: Entity, expected: &Handle<Image>) {
    let image = app.world().get::<ImageNode>(entity).unwrap();
    assert_eq!(&image.image, expected);
    assert_eq!(image.color, Color::srgba(0.2, 0.3, 0.4, 0.37));
    assert_eq!(
        app.world().get::<Visibility>(entity),
        Some(&Visibility::Inherited)
    );
}

#[test]
fn late_preview_sources_rebind_existing_hidden_nodes_without_resetting_opacity() {
    let mut app = fixture();
    let block = node(&mut app, "Block preview 3");
    let prop = node(&mut app, "Prop preview flower");
    let item = node(&mut app, "Item preview teramic::stick");
    let portrait = node(&mut app, "Player preview");
    app.update();
    for entity in [block, prop, item, portrait] {
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().get::<ImageNode>(entity).unwrap().image,
            Handle::default()
        );
    }
    let handle = image(&mut app);
    app.world_mut()
        .resource_mut::<BlockPreviewImages>()
        .0
        .insert(3, handle.clone());
    app.world_mut()
        .resource_mut::<PropPreviewImages>()
        .0
        .insert(ResourcePropKind::Flower, handle.clone());
    app.world_mut()
        .resource_mut::<ItemPreviewImages>()
        .0
        .insert(ItemKind::Stick, handle.clone());
    app.world_mut().resource_mut::<PlayerPreviewImage>().0 = handle.clone();
    app.update();
    for entity in [block, prop, item, portrait] {
        assert_bound(&app, entity, &handle);
    }
}

#[test]
fn metadata_changes_and_late_image_components_use_the_current_source_index() {
    let mut app = fixture();
    let first = image(&mut app);
    let second = image(&mut app);
    app.world_mut()
        .resource_mut::<BlockPreviewImages>()
        .0
        .insert(3, first.clone());
    app.world_mut()
        .resource_mut::<ItemPreviewImages>()
        .0
        .insert(ItemKind::Stick, second.clone());
    let entity = node(&mut app, "Menu background");
    let original_image = app.world().get::<ImageNode>(entity).unwrap().image.clone();
    app.update();
    assert_eq!(
        app.world().get::<ImageNode>(entity).unwrap().image,
        original_image
    );
    app.world_mut()
        .get_mut::<ImageMetadata>(entity)
        .unwrap()
        .alt = Some("Block preview 3".into());
    app.update();
    assert_bound(&app, entity, &first);
    app.world_mut()
        .get_mut::<ImageMetadata>(entity)
        .unwrap()
        .alt = Some("Item preview teramic::stick".into());
    app.update();
    assert_bound(&app, entity, &second);
    let late = app
        .world_mut()
        .spawn(ImageMetadata {
            alt: Some("Block preview 3".into()),
            ..default()
        })
        .id();
    app.update();
    app.world_mut()
        .entity_mut(late)
        .insert(ImageNode::default());
    app.update();
    assert_eq!(app.world().get::<ImageNode>(late).unwrap().image, first);
}

#[test]
fn external_image_handle_resets_are_repaired_while_inherited_color_is_preserved() {
    let mut app = fixture();
    let handle = image(&mut app);
    app.world_mut()
        .resource_mut::<ItemPreviewImages>()
        .0
        .insert(ItemKind::Stick, handle.clone());
    let entity = node(&mut app, "Item preview teramic::stick");
    app.update();
    {
        let mut image = app.world_mut().get_mut::<ImageNode>(entity).unwrap();
        image.image = Handle::default();
        image.color = Color::srgba(0.6, 0.4, 0.2, 0.0);
    }
    app.update();
    let image = app.world().get::<ImageNode>(entity).unwrap();
    assert_eq!(image.image, handle);
    assert_eq!(image.color, Color::srgba(0.6, 0.4, 0.2, 0.0));
}

#[test]
fn unchanged_previews_do_not_write_image_or_visibility_change_ticks() {
    let mut app = fixture();
    let handle = image(&mut app);
    app.world_mut()
        .resource_mut::<ItemPreviewImages>()
        .0
        .insert(ItemKind::Stick, handle.clone());
    let entity = node(&mut app, "Item preview teramic::stick");
    app.update();
    app.update();
    let image_tick = app
        .world()
        .entity(entity)
        .get_ref::<ImageNode>()
        .unwrap()
        .last_changed();
    let visibility_tick = app
        .world()
        .entity(entity)
        .get_ref::<Visibility>()
        .unwrap()
        .last_changed();
    for _ in 0..32 {
        app.update();
    }
    app.world_mut()
        .resource_mut::<BlockPreviewImages>()
        .0
        .insert(3, handle.clone());
    app.update();
    assert_bound(&app, entity, &handle);
    assert_eq!(
        app.world()
            .entity(entity)
            .get_ref::<ImageNode>()
            .unwrap()
            .last_changed(),
        image_tick
    );
    assert_eq!(
        app.world()
            .entity(entity)
            .get_ref::<Visibility>()
            .unwrap()
            .last_changed(),
        visibility_tick
    );
}

#[test]
fn arbitrary_rgb_dyes_share_one_preview_preserve_opacity_and_reset_when_rebound() {
    use terra_core::beds::DyeColor;
    let mut app = fixture();
    let handle = image(&mut app);
    app.world_mut()
        .resource_mut::<ItemPreviewImages>()
        .0
        .insert(ItemKind::Dye(DyeColor::White), handle.clone());
    app.world_mut()
        .resource_mut::<ItemPreviewImages>()
        .0
        .insert(ItemKind::Stick, handle.clone());
    let rgb = [32, 144, 240];
    let entity = node(&mut app, &preview_key(ItemKind::Dye(DyeColor::Rgb(rgb))));
    app.update();
    let image = app.world().get::<ImageNode>(entity).unwrap();
    assert_eq!(image.image, handle);
    assert_eq!(
        image.color,
        Color::srgba_u8(32, 144, 240, 255).with_alpha(0.37)
    );
    assert_eq!(app.world().resource::<ItemPreviewImages>().0.len(), 2);
    app.world_mut()
        .get_mut::<ImageNode>(entity)
        .unwrap()
        .color
        .set_alpha(0.12);
    app.update();
    assert_eq!(
        app.world().get::<ImageNode>(entity).unwrap().color.alpha(),
        0.12
    );
    app.world_mut()
        .get_mut::<ImageMetadata>(entity)
        .unwrap()
        .alt = Some(preview_key(ItemKind::Stick));
    app.update();
    assert_eq!(
        app.world().get::<ImageNode>(entity).unwrap().color,
        Color::srgba(0.2, 0.3, 0.4, 0.12)
    );
    assert!(app.world().get::<PreviewTint>(entity).is_none());
}

#[test]
fn portrait_blinks_do_not_rebind_unchanged_item_images() {
    let mut app = fixture();
    let item_handle = image(&mut app);
    let open_eyes = image(&mut app);
    let closed_eyes = image(&mut app);
    app.world_mut()
        .resource_mut::<ItemPreviewImages>()
        .0
        .insert(ItemKind::Stick, item_handle.clone());
    app.world_mut().resource_mut::<PlayerPreviewImage>().0 = open_eyes.clone();
    let item = node(&mut app, "Item preview teramic::stick");
    let portrait = node(&mut app, "Player preview");
    app.update();
    app.update();
    let item_tick = app
        .world()
        .entity(item)
        .get_ref::<ImageNode>()
        .unwrap()
        .last_changed();
    for handle in [closed_eyes, open_eyes] {
        app.world_mut().resource_mut::<PlayerPreviewImage>().0 = handle.clone();
        app.update();
        assert_bound(&app, portrait, &handle);
        assert_bound(&app, item, &item_handle);
        assert_eq!(
            app.world()
                .entity(item)
                .get_ref::<ImageNode>()
                .unwrap()
                .last_changed(),
            item_tick
        );
    }
}

#[test]
fn removed_source_resources_clear_cached_handles_and_unrelated_images_are_untouched() {
    let mut app = fixture();
    let handle = image(&mut app);
    app.world_mut()
        .resource_mut::<ItemPreviewImages>()
        .0
        .insert(ItemKind::Stick, handle.clone());
    let item = node(&mut app, "Item preview teramic::stick");
    let unrelated = node(&mut app, "Menu background");
    app.world_mut()
        .get_mut::<ImageNode>(unrelated)
        .unwrap()
        .image = handle.clone();
    app.update();
    app.world_mut().remove_resource::<ItemPreviewImages>();
    app.update();
    assert_eq!(
        app.world().get::<ImageNode>(item).unwrap().image,
        Handle::default()
    );
    assert_eq!(
        app.world().get::<Visibility>(item),
        Some(&Visibility::Hidden)
    );
    assert_eq!(
        app.world().get::<ImageNode>(unrelated).unwrap().image,
        handle
    );
}
