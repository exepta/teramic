//! Player snow shares weather surfaces, but owns its physical layer height.
use crate::{
    edits::{BlockObjectState, EditJournal},
    streaming::{BlockEditRequest, ChunkStore},
};
use terra_voxel::{MaterialId, blocks::BlockHit, global_to_section};

pub fn coatable(material: u16) -> bool {
    crate::weather::retains_snow(material)
        && (terra_core::construction::is_shape(material)
            || matches!(material, 101 | 102)
            || terra_core::textiles::is_carpet(material))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnowPlacement {
    Layer { material: u16, facing: Option<u8> },
    Coating,
}

pub fn target(
    chunks: &ChunkStore,
    journal: &EditJournal,
    hit: BlockHit,
) -> Option<([i64; 3], u16)> {
    let clicked = chunks.block_at(hit.position)?;
    let position = if clicked == 98 || coatable(clicked) {
        hit.position
    } else if hit.normal != [0; 3] {
        hit.adjacent()
    } else {
        return None;
    };
    let expected = chunks.block_at(position)?;
    plan(chunks, journal, position, expected).map(|_| (position, expected))
}

pub fn plan(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: [i64; 3],
    expected: u16,
) -> Option<SnowPlacement> {
    if chunks.block_at(position) != Some(expected) {
        return None;
    }
    if coatable(expected) {
        return (journal.surface_condition(position).snow < 1.0).then_some(SnowPlacement::Coating);
    }
    let layers = if expected == 98 {
        terra_core::construction::snow_layers(
            journal.object_state(position).unwrap_or_default().facing,
        ) + 1
    } else if expected == 0 {
        let below = [position[0], position[1].checked_sub(1)?, position[2]];
        let support = chunks.block_at(below)?;
        if support != MaterialId::Snow as u16 && !crate::weather::supports_snow_layers(support) {
            return None;
        }
        1
    } else {
        return None;
    };
    Some(SnowPlacement::Layer {
        material: if layers >= 8 { 5 } else { 98 },
        facing: terra_core::construction::manual_snow_facing(layers),
    })
}

pub fn bounds(
    position: [i64; 3],
    plan: SnowPlacement,
) -> Option<terra_core::block_shapes::ShapeBox> {
    let SnowPlacement::Layer { material, facing } = plan else {
        return None;
    };
    let mut bounds = terra_core::construction::snow_bounds(if material == 5 {
        8
    } else {
        facing.unwrap_or_default()
    });
    for (axis, value) in position.into_iter().enumerate() {
        bounds.minimum[axis] += value as f64;
        bounds.maximum[axis] += value as f64;
    }
    Some(bounds)
}

/// All fallible metadata changes are checked before touching occupancy or pose.
pub fn commit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    position: [i64; 3],
    expected: u16,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
) -> Result<bool, String> {
    let Some(plan) = plan(chunks, journal, position, expected) else {
        return Ok(false);
    };
    let owner = global_to_section(position).0;
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                let section = owner.offset([x, y, z]).map_err(|e| e.to_string())?;
                if journal.revision(section) > u64::MAX - 16
                    || journal.mesh_epoch(section) > u64::MAX - 16
                    || journal.water_mesh_epoch(section) > u64::MAX - 16
                    || journal.appearance_epoch(section) > u64::MAX - 16
                {
                    return Err("Snow placement revision exhausted".into());
                }
            }
        }
    }
    if chunks
        .loaded
        .get(&owner)
        .is_none_or(|section| section.data.revision() > u64::MAX - 16)
    {
        return Err("Snow placement section is unavailable".into());
    }
    match plan {
        SnowPlacement::Coating => {
            let mut condition = journal.surface_condition(position);
            condition.snow = 1.0;
            journal
                .set_surface_condition(position, condition)
                .map_err(|e| e.to_string())
        }
        SnowPlacement::Layer { material, facing } => {
            if expected == material {
                crate::streaming::ensure_block_record(chunks, journal, position, material)?;
            }
            if expected != material
                && !crate::streaming::commit_weather_cap(
                    chunks,
                    journal,
                    &BlockEditRequest {
                        position,
                        expected_material: expected,
                        material,
                    },
                    properties,
                )?
            {
                return Ok(false);
            }
            crate::streaming::commit_object_state(
                chunks,
                journal,
                position,
                facing.map(|facing| BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )?;
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests;
