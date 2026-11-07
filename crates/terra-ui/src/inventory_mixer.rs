use super::*;
use view::InventoryItemView;

pub(super) fn parse_target(value: &str) -> Option<[u8; 3]> {
    let value = value.strip_prefix('#')?;
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some([
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ])
}

pub(super) fn format_target([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

pub(super) struct RecipeView {
    pub inputs: Vec<InventoryItemView>,
    pub requirements: String,
}

pub(super) fn recipe_view(
    actual: &[InventoryItemView],
    inventory: &terra_core::Inventory,
    target: [u8; 3],
    palette: &BlockPalette,
) -> RecipeView {
    let recipe = terra_core::mixer::recipe(target);
    let available = |item| {
        inventory
            .slots
            .iter()
            .take(terra_core::mixer::INPUT_SLOTS)
            .filter(|stack| stack.item == item)
            .map(|stack| u32::from(stack.count))
            .sum::<u32>()
    };
    let requirements = recipe
        .inputs
        .iter()
        .map(|required| {
            let item = view::item_view(required.item, required.count, palette);
            format!(
                "{} {}/{}",
                item.name,
                available(required.item),
                required.count
            )
        })
        .collect::<Vec<_>>()
        .join(" / ");
    let mut missing = recipe.inputs.iter().filter_map(|required| {
        let count = u32::from(required.count).saturating_sub(available(required.item));
        (count > 0).then_some(terra_core::ItemStack {
            count: count as u16,
            ..*required
        })
    });
    let inputs = (0..terra_core::mixer::INPUT_SLOTS)
        .map(|index| {
            let mut slot = actual
                .get(index)
                .cloned()
                .unwrap_or_else(|| InventoryItemView {
                    id: format!("container-{index}"),
                    index: index.to_string(),
                    command: "container".into(),
                    empty: true,
                    ..default()
                });
            if slot.empty
                && let Some(required) = missing.next()
            {
                let ghost = view::item_view(required.item, required.count, palette);
                slot.ghost_item = Some(required.item);
                slot.ghost_preview = ghost.preview_key;
                slot.ghost_count = ghost.count;
                slot.ghost_name = format!("{} x{}", ghost.name, required.count);
            }
            slot
        })
        .collect();
    RecipeView {
        inputs,
        requirements,
    }
}

pub(super) fn status(
    machine: &terra_core::mixer::MixerView,
    drive: Option<&terra_network::protocol::session::KineticVisual>,
) -> String {
    let Some(drive) = drive else {
        return "No rotational power".into();
    };
    if drive.overloaded {
        "Overloaded".into()
    } else if drive.rpm == 0.0 {
        "No rotational power".into()
    } else if !machine.working {
        format!("{:.1} RPM - Waiting", drive.rpm)
    } else {
        format!("{:.1} RPM - {:.0}%", drive.rpm, machine.progress * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{ItemStack, beds::DyeColor};

    #[test]
    fn picker_rgb_is_byte_exact_and_rejects_alpha_or_invalid_text() {
        for rgb in [[0, 0, 0], [255, 255, 255], [17, 128, 240]] {
            assert_eq!(parse_target(&format_target(rgb)), Some(rgb));
        }
        assert_eq!(parse_target("#aBcD01"), Some([171, 205, 1]));
        for invalid in ["FFFFFF", "#FFF", "#FFFFFF80", "#GG0000", "#éffff"] {
            assert_eq!(parse_target(invalid), None);
        }
    }

    #[test]
    fn recipe_ghosts_never_become_real_items_or_replace_occupied_slots() {
        let target = [70, 120, 190];
        let palette = BlockPalette::default();
        let recipe = terra_core::mixer::recipe(target);
        let mut inventory = terra_core::Inventory {
            slots: vec![ItemStack::EMPTY; terra_core::mixer::SLOTS],
            ..default()
        };
        let slots = recipe_view(&[], &inventory, target, &palette).inputs;
        assert_eq!(slots.len(), terra_core::mixer::INPUT_SLOTS);
        for (index, slot) in slots.iter().enumerate() {
            assert!(slot.empty && slot.item_id.is_empty() && slot.preview_key.is_empty());
            assert_eq!(slot.command, "container");
            assert_eq!(slot.index, index.to_string());
            if let Some(required) = recipe.inputs.get(index) {
                assert_eq!(slot.ghost_preview, preview_key(required.item));
                assert_eq!(slot.ghost_count, required.count.to_string());
            } else {
                assert!(slot.ghost_preview.is_empty());
            }
        }
        let occupied = view::item_view(ItemKind::Dye(DyeColor::Red), 9, &palette);
        inventory.slots[0] = ItemStack {
            item: ItemKind::Dye(DyeColor::Red),
            count: 9,
        };
        let slots = recipe_view(
            std::slice::from_ref(&occupied),
            &inventory,
            target,
            &palette,
        )
        .inputs;
        assert_eq!(slots[0], occupied);
        assert_eq!(
            recipe.output,
            ItemStack {
                item: ItemKind::Dye(DyeColor::Rgb(target)),
                count: recipe.output.count
            }
        );
    }

    #[test]
    fn ghosts_subtract_all_real_inputs_and_use_empty_slots_not_recipe_indices() {
        let target = [70, 120, 190];
        let palette = BlockPalette::default();
        let recipe = terra_core::mixer::recipe(target);
        let required = recipe.inputs[0];
        let mut inventory = terra_core::Inventory {
            slots: vec![ItemStack::EMPTY; terra_core::mixer::SLOTS],
            ..default()
        };
        inventory.slots[4] = required;
        inventory.slots[0] = ItemStack {
            item: ItemKind::Dye(DyeColor::White),
            count: 64,
        };
        let actual = (0..terra_core::mixer::INPUT_SLOTS)
            .map(|index| {
                inventory.slot(index).map_or_else(
                    || InventoryItemView {
                        empty: true,
                        ..default()
                    },
                    |stack| view::item_view(stack.item, stack.count, &palette),
                )
            })
            .collect::<Vec<_>>();
        let result = recipe_view(&actual, &inventory, target, &palette);
        assert!(result.inputs[0].ghost_preview.is_empty());
        assert!(result.inputs[4].ghost_preview.is_empty());
        assert!(
            !result
                .inputs
                .iter()
                .any(|slot| slot.ghost_preview == preview_key(required.item))
        );
        for needed in &recipe.inputs {
            let available: u16 = inventory.slots[..terra_core::mixer::INPUT_SLOTS]
                .iter()
                .filter(|stack| stack.item == needed.item)
                .map(|stack| stack.count)
                .sum();
            if available < needed.count {
                assert!(result.inputs.iter().any(|slot| slot.empty
                    && slot.ghost_preview == preview_key(needed.item)
                    && slot.ghost_count == (needed.count - available).to_string()));
            }
            assert!(
                result
                    .requirements
                    .contains(&format!("{available}/{}", needed.count))
            );
        }
        for slot in &mut inventory.slots[..terra_core::mixer::INPUT_SLOTS] {
            *slot = ItemStack {
                item: ItemKind::Dye(DyeColor::White),
                count: 64,
            };
        }
        let full = (0..terra_core::mixer::INPUT_SLOTS)
            .map(|_| view::item_view(ItemKind::Dye(DyeColor::White), 64, &palette))
            .collect::<Vec<_>>();
        let result = recipe_view(&full, &inventory, target, &palette);
        assert!(
            result
                .inputs
                .iter()
                .all(|slot| slot.ghost_preview.is_empty())
        );
        assert_eq!(
            result.requirements.matches("0/").count(),
            recipe
                .inputs
                .iter()
                .filter(|stack| stack.item != ItemKind::Dye(DyeColor::White))
                .count()
        );
    }

    #[test]
    fn kinetic_status_distinguishes_stopped_waiting_running_and_overloaded() {
        let mut machine = terra_core::mixer::MixerView {
            target: [1, 2, 3],
            progress: 0.5,
            working: true,
        };
        let mut drive = terra_network::protocol::session::KineticVisual::default();
        assert_eq!(status(&machine, None), "No rotational power");
        assert_eq!(status(&machine, Some(&drive)), "No rotational power");
        drive.rpm = -7.5;
        assert_eq!(status(&machine, Some(&drive)), "-7.5 RPM - 50%");
        machine.working = false;
        assert_eq!(status(&machine, Some(&drive)), "-7.5 RPM - Waiting");
        drive.overloaded = true;
        assert_eq!(status(&machine, Some(&drive)), "Overloaded");
    }
}
