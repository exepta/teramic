//! Shared floor/wall mounting rules for ordinary torches.
use crate::streaming::ChunkStore;
use terra_core::block_properties::BlockPropertiesCatalog;

/// Placement has a prepared column already; explicit live liquid wins over it.
pub fn wet_placement(
    chunks: &ChunkStore,
    journal: &crate::edits::EditJournal,
    p: [i64; 3],
) -> bool {
    journal
        .fluid(p)
        .unwrap_or_else(|| {
            chunks
                .surface_column_at(p[0], p[2])
                .map_or_else(Default::default, |column| {
                    crate::water::fluid::from_columns(column.water_columns(), p[1])
                })
        })
        .amount
        > 0
}

pub(crate) fn flooded(
    terrain: &crate::TerraGenerator,
    chunks: &ChunkStore,
    journal: &crate::edits::EditJournal,
    p: [i64; 3],
) -> bool {
    let mut torch = Vec::new();
    crate::interaction_shapes::visit_boxes_with(
        journal,
        &Default::default(),
        p,
        |q| chunks.block_at(q),
        |bounds| torch.push(bounds),
    );
    (-1..=1).any(|x| {
        (-1..=1).any(|y| {
            (-1..=1).any(|z| {
                let q = [p[0] + x, p[1] + y, p[2] + z];
                let Some(material) = chunks.block_at(q) else {
                    return false;
                };
                let cell = chunks.raw_fluid_cell_at(terrain, journal, q, material);
                if cell.amount == 0 {
                    return false;
                }
                let space = crate::water::space::FluidSpace::wet_at(
                    journal,
                    q,
                    |at| chunks.block_at(at),
                    |at, id| chunks.raw_fluid_cell_at(terrain, journal, at, id),
                );
                // A torch cannot exclude water with its own collision geometry.
                let water = if q == p {
                    vec![terra_core::block_shapes::ShapeBox {
                        minimum: [0.; 3],
                        maximum: [1., cell.height(), 1.],
                        hinged: false,
                    }]
                } else {
                    space.wet_boxes(cell.height())
                };
                water.iter().any(|wet| {
                    torch.iter().any(|bounds| {
                        (0..3).all(|i| {
                            wet.maximum[i] + q[i] as f64 > bounds.minimum[i] + 1e-8
                                && wet.minimum[i] + (q[i] as f64) < bounds.maximum[i] - 1e-8
                        })
                    })
                })
            })
        })
    })
}

/// The authority uses the regular edit path so block, item and replica state
/// change together. Repeated contacts cannot create repeated item drops.
pub(crate) fn wash_away(
    chunks: &mut ChunkStore,
    journal: &mut crate::edits::EditJournal,
    positions: &[[i64; 3]],
) -> Result<(), String> {
    for &position in positions {
        let Some(material) = chunks
            .block_at(position)
            .filter(|m| matches!(*m, 40 | terra_core::lantern::LANTERN))
        else {
            continue;
        };
        if !journal.gameplay().can_drop() {
            // Keep both the lamp and its item until the bounded drop pool has
            // room. Waking this wet cell makes stable water retry removal too.
            journal.wake_fluid(position);
            continue;
        }
        if crate::streaming::commit_block_edit(
            chunks,
            journal,
            &crate::streaming::BlockEditRequest {
                position,
                expected_material: material,
                material: 0,
            },
            &Default::default(),
        )? {
            journal.gameplay_mut().drop_one(
                position.map(|v| v as f64 + 0.5),
                terra_core::ItemKind::Block(material),
            );
        }
    }
    Ok(())
}

pub fn facing(normal: [i32; 3]) -> Option<u8> {
    match normal {
        [0, 1, 0] => Some(0),
        [1, 0, 0] => Some(4),
        [-1, 0, 0] => Some(5),
        [0, 0, 1] => Some(6),
        [0, 0, -1] => Some(7),
        _ => None,
    }
}

pub fn support(position: [i64; 3], facing: u8) -> Option<[i64; 3]> {
    if facing != 0 && !(4..=7).contains(&facing) {
        return None;
    }
    let normal = terra_core::block_shapes::torch_normal(facing);
    Some(std::array::from_fn(|axis| {
        position[axis] - normal[axis] as i64
    }))
}

pub fn supported(
    chunks: &ChunkStore,
    properties: &BlockPropertiesCatalog,
    position: [i64; 3],
    facing: u8,
) -> bool {
    support(position, facing)
        .and_then(|support| chunks.block_at(support))
        .is_some_and(|material| {
            terra_voxel::MaterialId::from_raw(material)
                .is_some_and(terra_voxel::MaterialId::is_solid)
                && properties.shape(material).is_empty()
        })
}

#[cfg(test)]
#[path = "torches_tests.rs"]
mod tests;
