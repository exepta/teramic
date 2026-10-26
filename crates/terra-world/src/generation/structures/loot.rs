use super::BuildingRole;
use terra_core::{CropItem, Inventory, ItemKind, MineralItem, ToolKind};

pub(super) fn inventory(role: BuildingRole, seed: u32) -> Inventory {
    let mut result = Inventory::default();
    let amount = 2 + (seed % 5) as u16;
    let items = match role {
        BuildingRole::Smith | BuildingRole::Mine => vec![
            (ItemKind::Mineral(MineralItem::Coal), amount + 2),
            (ItemKind::Mineral(MineralItem::IronIngot), amount),
            (ItemKind::Stick, amount * 2),
        ],
        BuildingRole::Medic => vec![
            (ItemKind::Crop(CropItem::Lettuce), amount),
            (ItemKind::Crop(CropItem::Bread), amount),
            (ItemKind::Flower(terra_core::beds::DyeColor::White), amount),
        ],
        BuildingRole::Watch | BuildingRole::Police => vec![
            (ItemKind::Tool(ToolKind::StoneSword), 1),
            (ItemKind::Block(40), amount),
        ],
        BuildingRole::Church => vec![
            (ItemKind::Mineral(MineralItem::GoldIngot), 1),
            (ItemKind::Crop(CropItem::Bread), amount),
        ],
        BuildingRole::Ruin => vec![
            (ItemKind::Mineral(MineralItem::RawIron), amount),
            (ItemKind::Mineral(MineralItem::Coal), amount),
            (ItemKind::Block(43), amount),
        ],
        _ => vec![
            (ItemKind::Crop(CropItem::WheatSeeds), amount),
            (ItemKind::Crop(CropItem::Bread), amount),
            (ItemKind::Block(16), amount),
        ],
    };
    for (item, count) in items {
        result.insert(item, count);
    }
    result
}
