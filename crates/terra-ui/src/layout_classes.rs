//! Index the few Tilt classes that require native post-layout geometry.
use bevy::prelude::*;
use tilt_ui::{ElementClasses, ParkedUi};

#[derive(Resource, Default)]
pub(crate) struct LayoutClasses {
    pub groups: Vec<Entity>,
    pub descriptions: Vec<Entity>,
    pub cardinals: Vec<Entity>,
}

pub(crate) fn install(app: &mut App) {
    if !app.world().contains_resource::<LayoutClasses>() {
        app.init_resource::<LayoutClasses>()
            .add_systems(PostUpdate, track.before(bevy::ui::UiSystems::Prepare));
    }
}

fn set(entries: &mut Vec<Entity>, entity: Entity, present: bool) {
    match (entries.iter().position(|&id| id == entity), present) {
        (Some(index), false) => {
            entries.swap_remove(index);
        }
        (None, true) => entries.push(entity),
        _ => {}
    }
}

pub(crate) fn track(
    changed: Query<(Entity, &ElementClasses), Changed<ElementClasses>>,
    mut removed: RemovedComponents<ElementClasses>,
    mut index: ResMut<LayoutClasses>,
) {
    // Process removals first so a remove/reinsert in one frame sees the new value.
    for entity in removed.read() {
        set(&mut index.groups, entity, false);
        set(&mut index.descriptions, entity, false);
        set(&mut index.cardinals, entity, false);
    }
    for (entity, classes) in &changed {
        let mut group = false;
        let mut description = false;
        let mut cardinal = false;
        for class in &classes.classes {
            match class.as_str() {
                "copper-options-tabs"
                | "kingdom-tabs"
                | "character-tabs"
                | "world-create-tabs"
                | "build-mode-categories"
                | "editor-categories" => group = true,
                "setting-description" => description = true,
                "map-cardinal" => cardinal = true,
                _ => {}
            }
        }
        set(&mut index.groups, entity, group);
        set(&mut index.descriptions, entity, description);
        set(&mut index.cardinals, entity, cardinal);
    }
}

pub(crate) fn active_categories(
    index: Res<LayoutClasses>,
    active: Query<(), Without<ParkedUi>>,
) -> bool {
    index
        .groups
        .iter()
        .chain(&index.descriptions)
        .any(|&id| active.contains(id))
}

pub(crate) fn active_cardinals(
    index: Res<LayoutClasses>,
    active: Query<(), Without<ParkedUi>>,
) -> bool {
    index.cardinals.iter().any(|&id| active.contains(id))
}
