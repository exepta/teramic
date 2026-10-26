//! One bounded, shared solid per tool. Inventory uses the unit design space;
//! held items use its real dimensions, and world pickups retain their small proxy.
use super::*;
use crate::players::model::Builder;
use terra_core::ToolKind;

mod farming;
mod geometry;
mod palette;
#[cfg(test)]
mod tests;

use geometry::{axe, handle, pickaxe, shovel, sword};
use palette::{Palette, ToolMaterial};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Sword,
    Pickaxe,
    Axe,
    Shovel,
    WateringCan,
    Bucket(bool),
    Hoe,
    Scythe,
    Stick,
}

fn profile(item: ItemKind) -> Option<(Shape, ToolMaterial)> {
    use ToolKind::*;
    use ToolMaterial as M;
    Some(match item {
        ItemKind::Stick => (Shape::Stick, M::Wood),
        ItemKind::Tool(kind) => match kind {
            WoodSword => (Shape::Sword, M::Wood),
            StoneSword => (Shape::Sword, M::Stone),
            IronSword => (Shape::Sword, M::Iron),
            GoldSword => (Shape::Sword, M::Gold),
            DiamondSword => (Shape::Sword, M::Diamond),
            RubySword => (Shape::Sword, M::Ruby),
            WoodPickaxe => (Shape::Pickaxe, M::Wood),
            StonePickaxe => (Shape::Pickaxe, M::Stone),
            IronPickaxe => (Shape::Pickaxe, M::Iron),
            GoldPickaxe => (Shape::Pickaxe, M::Gold),
            DiamondPickaxe => (Shape::Pickaxe, M::Diamond),
            RubyPickaxe => (Shape::Pickaxe, M::Ruby),
            WoodAxe => (Shape::Axe, M::Wood),
            StoneAxe => (Shape::Axe, M::Stone),
            IronAxe => (Shape::Axe, M::Iron),
            GoldAxe => (Shape::Axe, M::Gold),
            DiamondAxe => (Shape::Axe, M::Diamond),
            RubyAxe => (Shape::Axe, M::Ruby),
            WoodShovel => (Shape::Shovel, M::Wood),
            StoneShovel => (Shape::Shovel, M::Stone),
            WoodWateringCan => (Shape::WateringCan, M::Wood),
            IronWateringCan => (Shape::WateringCan, M::Iron),
            GoldWateringCan => (Shape::WateringCan, M::Gold),
            DiamondWateringCan => (Shape::WateringCan, M::Diamond),
            RubyWateringCan => (Shape::WateringCan, M::Ruby),
            WoodHoe => (Shape::Hoe, M::Wood),
            IronHoe => (Shape::Hoe, M::Iron),
            GoldHoe => (Shape::Hoe, M::Gold),
            DiamondHoe => (Shape::Hoe, M::Diamond),
            RubyHoe => (Shape::Hoe, M::Ruby),
            WoodScythe => (Shape::Scythe, M::Wood),
            IronScythe => (Shape::Scythe, M::Iron),
            GoldScythe => (Shape::Scythe, M::Gold),
            DiamondScythe => (Shape::Scythe, M::Diamond),
            RubyScythe => (Shape::Scythe, M::Ruby),
            Bucket => (Shape::Bucket(false), M::Iron),
            WaterBucket => (Shape::Bucket(true), M::Iron),
        },
        _ => return None,
    })
}

pub(super) fn batches(item: ItemKind, material: usize) -> Result<Vec<MeshBatch>, String> {
    let (shape, kind) = profile(item).ok_or("Not a tool or stick")?;
    let palette = Palette::for_material(kind);
    let mut builder = Builder::default();
    if !matches!(shape, Shape::WateringCan | Shape::Bucket(_)) {
        handle(&mut builder, shape, &palette)?;
    }
    match shape {
        Shape::Sword => sword(&mut builder, &palette)?,
        Shape::Pickaxe => pickaxe(&mut builder, &palette)?,
        Shape::Axe => axe(&mut builder, &palette)?,
        Shape::Shovel => shovel(&mut builder, &palette)?,
        Shape::WateringCan => farming::watering_can(&mut builder, &palette)?,
        Shape::Bucket(filled) => farming::bucket(&mut builder, &palette, filled)?,
        Shape::Hoe => farming::hoe(&mut builder, &palette)?,
        Shape::Scythe => farming::scythe(&mut builder, &palette)?,
        Shape::Stick => {}
    }
    let mut mesh = builder.finish_batch()?;
    mesh.material = material;
    Ok(vec![mesh])
}

/// Real equipped dimensions, independent of the small physical pickup proxy.
pub(crate) fn world_length(item: ItemKind) -> Option<f32> {
    match profile(item)?.0 {
        Shape::Sword => Some(1.5),
        Shape::Pickaxe | Shape::Axe | Shape::Shovel | Shape::Hoe => Some(1.25),
        Shape::Scythe => Some(1.5),
        Shape::WateringCan => Some(0.85),
        Shape::Bucket(_) => Some(0.7),
        Shape::Stick => None,
    }
}

pub(crate) fn cache_vertex(item: ItemKind, point: Vec3) -> Vec3 {
    (point - Vec3::splat(0.5)) * world_length(item).unwrap_or(0.24)
}

pub(crate) fn grip(item: ItemKind) -> Option<Vec3> {
    let y = match profile(item)?.0 {
        Shape::Sword => 0.165,
        Shape::Pickaxe | Shape::Axe | Shape::Shovel => 0.20,
        Shape::Hoe | Shape::Scythe => 0.20,
        Shape::WateringCan => 0.90,
        Shape::Bucket(_) => 0.92,
        Shape::Stick => return None,
    };
    Some(cache_vertex(
        item,
        Vec3::new(
            if profile(item)?.0 == Shape::WateringCan {
                0.35
            } else {
                0.5
            },
            y,
            0.5,
        ),
    ))
}
