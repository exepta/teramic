use super::{
    DetailLevel,
    builder::{FoliageOwner, MeshBatch, variation},
};
use crate::blocks::BlockTemplate;
use bevy::prelude::*;
use std::{
    collections::BTreeMap,
    sync::{LazyLock, OnceLock},
};
use terra_world::generation::TreeAppearance;

mod broadleaves;
mod needles;

pub(super) struct CrownStyle {
    pub material: u16,
    pub species: Option<TreeAppearance>,
    pub cluster: Option<crate::blocks::FoliageClusterMaterials>,
}

pub(super) fn append_crown(
    batches: &mut BTreeMap<usize, MeshBatch>,
    block: &BlockTemplate,
    global: [i64; 3],
    local: [u32; 3],
    exposure: u8,
    level: DetailLevel,
    style: CrownStyle,
) -> Result<(), String> {
    // Fine cutout needles occupy only fractions of a pixel at middle distance.
    // The closed bough body uses the same snow shelves as the far cube.
    // Generated snowy and green crowns both carry their source tree species.
    // A loose building block has no crown owner and keeps its full block faces.
    let conifer = matches!(
        style.species,
        Some(TreeAppearance::Spruce | TreeAppearance::Fir)
    );
    if conifer
        && (level == DetailLevel::Middle
            || style.cluster.and_then(|cluster| cluster.needles).is_none())
    {
        return needles::append(
            batches,
            block,
            global,
            local,
            exposure,
            level,
            (style.material, style.species == Some(TreeAppearance::Fir)),
        );
    }
    broadleaves::append(batches, block, global, local, exposure, level, style)
}

pub(super) fn append_twig(
    batches: &mut BTreeMap<usize, MeshBatch>,
    material: usize,
    local: Vec3,
    side: usize,
    seed: u64,
) -> Result<(), String> {
    let axis = side / 2;
    if axis == 1 || !seed.is_multiple_of(7) {
        return Ok(());
    }
    let mut direction = Vec3::ZERO;
    direction[axis] = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
    let anchor = local + Vec3::splat(0.5) + direction * 0.46;
    let tip = anchor + direction * (0.24 + ((seed >> 8) % 5) as f32 * 0.035) + Vec3::Y * 0.14;
    let offset = tip - anchor;
    crate::props::add_box(
        batches,
        material,
        (anchor + tip) * 0.5,
        Vec3::new(0.075, offset.length(), 0.075),
        Quat::from_rotation_arc(Vec3::Y, offset.normalize()),
    )?;
    let fork = tip + Vec3::Y * 0.15 + direction * 0.07;
    let offset = fork - tip;
    crate::props::add_box(
        batches,
        material,
        (tip + fork) * 0.5,
        Vec3::new(0.045, offset.length(), 0.045),
        Quat::from_rotation_arc(Vec3::Y, offset.normalize()),
    )
}

#[cfg(test)]
mod tests;
