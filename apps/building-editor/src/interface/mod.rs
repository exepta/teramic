mod labels;
use crate::{
    document::Document,
    editor::{Editor, Panel},
    workspace::WorkspacePlatform,
};
use bevy::prelude::*;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use tilt_ui::html_shared;

#[derive(Clone, Serialize, PartialEq)]
pub struct ItemRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub preview: String,
    pub selected: bool,
}
#[derive(Clone, Serialize, PartialEq)]
pub struct CategoryRow {
    pub id: String,
    pub name: String,
    pub selected: bool,
}
#[derive(Clone, Serialize, PartialEq)]
pub struct FileRow {
    pub path: String,
    pub name: String,
    pub icon: String,
}
#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct EditorView {
    pub playing: bool,
    pub menu: bool,
    pub items_open: bool,
    pub files_open: bool,
    pub save: bool,
    pub options: bool,
    pub overwrite: bool,
    pub discard: bool,
    pub quit: bool,
    pub labels: BTreeMap<String, String>,
    pub items: Vec<ItemRow>,
    pub categories: Vec<CategoryRow>,
    pub platform_9: bool,
    pub platform_17: bool,
    pub platform_25: bool,
    pub platform_33: bool,
    pub files: Vec<FileRow>,
    pub error: String,
    pub status: String,
    pub selected: String,
    pub cell_count: String,
    pub dirty: bool,
    pub search: String,
    pub directory: String,
    pub filename: String,
    pub name: String,
    pub filepath: String,
    pub page: String,
    pub has_previous: bool,
    pub has_next: bool,
    pub speed: String,
    pub sensitivity: String,
    pub english: bool,
}
#[derive(Resource)]
struct ProjectionTime(Instant);

pub fn project(world: &mut World) {
    if world
        .get_resource::<ProjectionTime>()
        .is_some_and(|time| time.0.elapsed() < Duration::from_millis(75))
    {
        return;
    }
    world.insert_resource(ProjectionTime(Instant::now()));
    let editor = world.resource::<Editor>();
    let document = world.resource::<Document>();
    let palette = world.resource::<terra_player::interaction::BlockPalette>();
    let config = world.resource::<terra_core::GameConfig>();
    let labels = labels::labels(editor.english);
    let all = terra_ui::creative_items(palette);
    let search = editor.search.to_lowercase();
    let filtered: Vec<_> = all
        .into_iter()
        .filter_map(|item| {
            let (name, _, description) = terra_ui::item_metadata(item, palette);
            let name = labels::item_name(&name, editor.english);
            let id = item.identifier();
            if (!search.is_empty()
                && !name.to_lowercase().contains(&search)
                && !id.to_lowercase().contains(&search))
                || (!editor.category.is_empty()
                    && palette.item_categories.get(&item) != Some(&editor.category))
            {
                return None;
            }
            Some((
                item,
                ItemRow {
                    id,
                    name,
                    description,
                    preview: terra_ui::item_preview_key(item),
                    selected: item == editor.selected,
                },
            ))
        })
        .collect();
    let pages = filtered.len().div_ceil(36).max(1);
    let page = editor.page.min(pages - 1);
    let page_items = filtered
        .into_iter()
        .skip(page * 36)
        .take(36)
        .collect::<Vec<_>>();
    let requested = page_items.iter().map(|(item, _)| *item).collect::<Vec<_>>();
    let categories = std::iter::once(CategoryRow {
        id: String::new(),
        name: labels["all"].clone(),
        selected: editor.category.is_empty(),
    })
    .chain(
        palette
            .creative_categories
            .iter()
            .map(|category| CategoryRow {
                id: category.id.clone(),
                name: labels::item_name(&category.name, editor.english),
                selected: category.id == editor.category,
            }),
    )
    .collect();
    let mut file_error = String::new();
    let files = if matches!(editor.panel, Panel::Load | Panel::Save) {
        match crate::document::list_directory(&editor.directory) {
            Ok(entries) => entries
                .into_iter()
                .map(|entry| FileRow {
                    path: entry.path.to_string_lossy().into_owned(),
                    name: entry.label,
                    icon: format!(
                        "tilt-icon:{}@32",
                        if entry.directory { "folder" } else { "file" }
                    ),
                })
                .collect(),
            Err(error) => {
                file_error = error;
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    let selected = labels::item_name(
        &terra_ui::item_metadata(editor.selected, palette).0,
        editor.english,
    );
    let platform_size = world.get_resource::<WorkspacePlatform>().map_or_else(
        || WorkspacePlatform::default().size(),
        WorkspacePlatform::size,
    );
    let next = EditorView {
        playing: editor.panel == Panel::Playing,
        menu: editor.panel == Panel::Menu,
        items_open: editor.panel == Panel::Items,
        files_open: matches!(editor.panel, Panel::Load | Panel::Save),
        save: editor.panel == Panel::Save,
        options: editor.panel == Panel::Options,
        overwrite: editor.panel == Panel::Overwrite,
        discard: editor.panel == Panel::Discard,
        quit: editor.panel == Panel::Quit,
        labels,
        items: page_items.into_iter().map(|(_, row)| row).collect(),
        categories,
        platform_9: platform_size == 9,
        platform_17: platform_size == 17,
        platform_25: platform_size == 25,
        platform_33: platform_size == 33,
        files,
        error: if editor.error.is_empty() {
            file_error
        } else {
            editor.error.clone()
        },
        status: editor.status.clone(),
        selected,
        cell_count: document.cells.len().to_string(),
        dirty: document.dirty(),
        search: editor.search.clone(),
        directory: editor.directory.to_string_lossy().into_owned(),
        filename: editor.filename.clone(),
        name: editor.name.clone(),
        filepath: editor
            .pending_path
            .as_ref()
            .or(document.path.as_ref())
            .map_or_else(String::new, |p| p.to_string_lossy().into_owned()),
        page: format!("{} / {}", page + 1, pages),
        has_previous: page > 0,
        has_next: page + 1 < pages,
        speed: config.client.fly_speed.to_string(),
        sensitivity: config.client.mouse_sensitivity.to_string(),
        english: editor.english,
    };
    if *world.resource::<EditorView>() != next {
        world.insert_resource(next);
    }
    world
        .resource_mut::<terra_render::RenderItemPreviewRequests>()
        .replace(requested);
}

pub fn bind_previews(
    items: Res<terra_render::RenderItemPreviews>,
    blocks: Res<terra_render::RenderBlockPreviews>,
    props: Res<terra_render::RenderPropPreviews>,
    mut images: Query<(&tilt_ui::ImageMetadata, &mut ImageNode)>,
) {
    let mut sources = std::collections::HashMap::new();
    for (item, handle) in &items.images {
        sources.insert(terra_ui::item_preview_key(*item), handle);
    }
    for (&id, handle) in &blocks.images {
        sources.insert(
            terra_ui::item_preview_key(terra_core::ItemKind::Block(id)),
            handle,
        );
    }
    for (&kind, handle) in &props.images {
        sources.insert(
            terra_ui::item_preview_key(terra_core::ItemKind::Prop(kind)),
            handle,
        );
    }
    for (metadata, mut image) in &mut images {
        if let Some(handle) = metadata.alt.as_ref().and_then(|alt| sources.get(alt))
            && image.image != **handle
        {
            image.image = (**handle).clone();
        }
    }
}
