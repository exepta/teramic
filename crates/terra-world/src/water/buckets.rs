//! Bucket transfers use the same journal, source cells, scheduler and water mesh.
use super::{FluidCell, space::FluidSpace};
use crate::{
    edits::EditJournal,
    streaming::{BlockEditRequest, ChunkStore, commit_block_edit},
};
use terra_core::block_properties::BlockPropertiesCatalog;
use terra_voxel::global_to_section;

pub fn target(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    filled: bool,
    origin: [f64; 3],
    direction: [f64; 3],
) -> Option<[i64; 3]> {
    if !filled {
        return crate::farming::water_target(chunks, journal, origin, direction);
    }
    let (block, _) = crate::targeting::pick_world_with_context(
        chunks,
        journal,
        properties,
        origin,
        direction,
        crate::props::PropPose::Rest,
    );
    let hit = block?;
    let position = if super::space::partial(chunks.block_at(hit.position)?) {
        hit.position
    } else {
        std::array::from_fn(|i| hit.position[i] + i64::from(hit.normal[i]))
    };
    let material = chunks.block_at(position)?;
    (matches!(material, 0 | 27) || super::space::partial(material)).then_some(position)
}

pub fn source(
    chunks: &ChunkStore,
    terrain: &crate::TerraGenerator,
    journal: &EditJournal,
    position: [i64; 3],
) -> Option<FluidCell> {
    let material = chunks.block_at(position)?;
    Some(chunks.fluid_cell_at(terrain, journal, position, material))
}

pub fn transfer(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &crate::TerraGenerator,
    properties: &BlockPropertiesCatalog,
    position: [i64; 3],
    filled: bool,
) -> Result<bool, String> {
    let Some(material) = chunks.block_at(position) else {
        return Ok(false);
    };
    let Some(current) = source(chunks, terrain, journal, position) else {
        return Ok(false);
    };
    if !terrain.config().water || (filled && current.source) || (!filled && !current.source) {
        return Ok(false);
    }
    if filled && !(matches!(material, 0 | 27) || super::space::partial(material)) {
        return Ok(false);
    }
    let cell = if filled {
        FluidSpace::at(journal, position, |p| chunks.block_at(p)).clamp(FluidCell {
            amount: 8,
            source: true,
            flow: [0; 2],
        })
    } else {
        FluidCell::default()
    };
    if filled && cell.amount == 0 {
        return Ok(false);
    }
    let owner = global_to_section(position).0;
    // Reject exhausted counters before either half of the world transaction.
    if journal.fluid_epoch(owner) == u64::MAX
        || journal.revision(owner) == u64::MAX
        || chunks
            .loaded
            .get(&owner)
            .is_none_or(|s| s.data.revision() == u64::MAX)
        || (-1..=1).any(|z| {
            (-1..=1).any(|y| {
                (-1..=1).any(|x| {
                    owner.offset([x, y, z]).map_or(true, |p| {
                        journal.water_mesh_epoch(p) > u64::MAX - 16
                            || journal.mesh_epoch(p) > u64::MAX - 16
                    })
                })
            })
        })
    {
        return Err("Water transfer revision exhausted".into());
    }
    if !filled {
        if material == 27 {
            if !commit_block_edit(
                chunks,
                journal,
                &BlockEditRequest {
                    position,
                    expected_material: 27,
                    material: 0,
                },
                properties,
            )? {
                return Ok(false);
            }
        } else {
            // Pin an empty override even for a natural waterlogged construction.
            let affected = journal
                .record(position, material)
                .map_err(|e| e.to_string())?;
            chunks.invalidate_sections(affected);
        }
    }
    let changed = journal
        .set_fluid(position, cell)
        .map_err(|e| e.to_string())?;
    Ok(!filled || changed)
}
