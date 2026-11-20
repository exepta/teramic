//! The building editor uses the game's actual authored inventory catalog.
use terra_core::{GameMode, Inventory, ItemKind};
use terra_player::interaction::BlockPalette;

pub fn creative_items(palette: &BlockPalette) -> Vec<ItemKind> {
    crate::inventory::available_items(GameMode::Creative, &Inventory::default(), palette)
}

pub fn item_metadata(item: ItemKind, palette: &BlockPalette) -> (String, String, String) {
    crate::inventory::editor_item_metadata(item, palette)
}

pub fn item_preview_key(item: ItemKind) -> String {
    crate::inventory::preview_key(item)
}
