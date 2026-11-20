//! Native assertions use shaped text and the actual materialized option buttons.
use bevy::{
    prelude::*,
    text::{LineBreak, TextLayoutInfo},
};
use std::path::Path;
use terra_ui::ElementId;

fn caption_ancestor(world: &World, mut entity: Entity, class: &str) -> Option<Entity> {
    for _ in 0..32 {
        if world
            .get::<tilt_ui::ElementClasses>(entity)
            .is_some_and(|classes| classes.classes.iter().any(|value| value == class))
        {
            return Some(entity);
        }
        entity = world.get::<ChildOf>(entity)?.parent();
    }
    None
}

pub(super) fn check(world: &mut World, output: &Path, language: &str) -> Result<(), String> {
    let mut buttons: Vec<_> = world
        .query::<(Entity, &ElementId, &ComputedNode)>()
        .iter(world)
        .filter(|(_, id, node)| {
            node.size().x > 0.0
                && matches!(
                    id.0.as_str(),
                    "graphics" | "controls" | "gameplay-options" | "audio" | "multiplayer-options"
                )
        })
        .map(|(entity, id, node)| (entity, id.0.clone(), node.size().x))
        .collect();
    buttons.sort_by(|a, b| a.1.cmp(&b.1));
    if buttons.len() != 5 {
        return Err(format!(
            "Expected five visible category buttons, got {}",
            buttons.len()
        ));
    }
    let low = buttons.iter().map(|v| v.2).fold(f32::MAX, f32::min);
    let high = buttons.iter().map(|v| v.2).fold(0.0_f32, f32::max);
    if high - low > 1.1 {
        return Err(format!("Category buttons have unequal widths: {buttons:?}"));
    }
    let mut report = Vec::new();
    for (button, id, width) in buttons {
        let mut pending = world
            .get::<Children>(button)
            .ok_or("Category has no content")?
            .to_vec();
        let mut captions = Vec::new();
        while let Some(entity) = pending.pop() {
            if world
                .get::<tilt_ui::TiltElement>(entity)
                .is_some_and(|e| e.kind == tilt_ui::ElementKind::Icon)
            {
                continue;
            }
            if let Some(text) = world.get::<Text>(entity) {
                if !text.0.trim().is_empty() {
                    let layout = world
                        .get::<TextLayout>(entity)
                        .ok_or("Caption missing layout")?;
                    if layout.linebreak != LineBreak::NoWrap {
                        return Err(format!("Category caption may wrap: {}", text.0));
                    }
                    let info = world
                        .get::<TextLayoutInfo>(entity)
                        .ok_or("Caption missing shaped text")?;
                    if info.size.x <= 0.0 || info.size.x > width + 1.1 {
                        return Err(format!(
                            "Caption does not fit category: {} {} > {width}",
                            text.0, info.size.x
                        ));
                    }
                    captions.push(serde_json::json!({"text":text.0,"glyph_width_px":info.size.x}));
                }
            } else if let Some(children) = world.get::<Children>(entity) {
                pending.extend(children.iter());
            }
        }
        if captions.is_empty() {
            return Err(format!("No translated category caption for {id}"));
        }
        report.push(serde_json::json!({"id":id,"width_px":width,"captions":captions}));
    }
    let mut title_count = 0;
    for (entity, text, layout, info, node) in world
        .query::<(Entity, &Text, &TextLayout, &TextLayoutInfo, &ComputedNode)>()
        .iter(world)
    {
        if node.size().min_element() <= 0.0 {
            continue;
        }
        let Some(wrapper) = caption_ancestor(world, entity, "setting-title") else {
            continue;
        };
        let wrapper_node = world
            .get::<ComputedNode>(wrapper)
            .ok_or("Setting title wrapper has no layout")?;
        title_count += 1;
        if layout.linebreak != LineBreak::NoWrap || info.size.x > wrapper_node.size().x + 1.1 {
            return Err(format!(
                "Translated setting title is wrapped or clipped: {} {:?} {} > {}",
                text.0,
                layout.linebreak,
                info.size.x,
                wrapper_node.size().x
            ));
        }
    }
    if title_count == 0 {
        return Err("No visible settings titles checked".into());
    }
    std::fs::create_dir_all(output).map_err(|e| e.to_string())?;
    let data = serde_json::json!({"language":language,"status":"passed","buttons":report,"single_line_setting_titles":title_count});
    std::fs::write(
        output.join(format!("category-layout-{language}.json")),
        serde_json::to_string_pretty(&data).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
