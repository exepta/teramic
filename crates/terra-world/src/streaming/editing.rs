use super::{BlockEditRequest, ChunkStore, PropRemoveRequest, WorldAccess};
use crate::edits::EditJournal;
use bevy::prelude::*;
use terra_core::block_properties::BlockPropertiesCatalog;
use terra_voxel::{MaterialId, global_to_section};

/// Object poses change collision geometry even when the occupied material stays
/// identical. Queue their halo sections just like an ordinary block edit.
pub fn commit_object_state(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    position: [i64; 3],
    state: Option<crate::edits::BlockObjectState>,
) -> Result<(), String> {
    for section in journal.set_object_state(position, state)? {
        if chunks.is_wanted(section) {
            chunks.dirty.insert(section);
        }
    }
    Ok(())
}

pub(super) fn apply_block_edits(
    access: Res<WorldAccess>,
    mut requests: MessageReader<BlockEditRequest>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    properties: Res<BlockPropertiesCatalog>,
) {
    for request in requests.read() {
        if access.authoritative()
            && let Err(error) = commit_block_edit(&mut chunks, &mut journal, request, &properties)
        {
            tracing::error!(%error,"Block edit failed");
        }
    }
}

/// Commit only after server/fixture validation. Network replicas never call this
/// for local input; they import confirmed journal deltas instead.
/// Use the authoritative catalog so data-defined structural objects preserve ground cover.
pub fn commit_block_edit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    request: &BlockEditRequest,
    properties: &BlockPropertiesCatalog,
) -> Result<bool, String> {
    commit(chunks, journal, request, properties, true)
}

pub(super) fn commit_segment_edit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    request: &BlockEditRequest,
    properties: &BlockPropertiesCatalog,
) -> Result<bool, String> {
    commit(chunks, journal, request, properties, false)
}

fn commit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    request: &BlockEditRequest,
    properties: &BlockPropertiesCatalog,
    reconcile: bool,
) -> Result<bool, String> {
    if chunks.block_at(request.position) != Some(request.expected_material)
        || request.material == request.expected_material
    {
        return Ok(false);
    }
    if request.expected_material == MaterialId::Chest as u16
        && request.material != MaterialId::Chest as u16
        && journal.container_nonempty(request.position)
    {
        return Err("Empty this chest before removing it".into());
    }
    let (owner, local) = global_to_section(request.position);
    let Some(section) = chunks.loaded.get_mut(&owner) else {
        return Ok(false);
    };
    if section.data.revision() == u64::MAX {
        return Err("Block revision overflow".into());
    }
    let affected = journal
        .record(request.position, request.material)
        .map_err(|e| e.to_string())?;
    section
        .data
        .set_block(local, request.material)
        .map_err(|e| e.to_string())?;
    if request.expected_material != MaterialId::Dirt as u16
        || request.material != MaterialId::Grass as u16
    {
        retire_affected_props(request, chunks, journal);
    }
    for p in affected {
        if chunks.is_wanted(p) {
            chunks.dirty.insert(p);
        }
    }
    tracing::debug!(position=?request.position,material=request.material,"Block edited");
    if !MaterialId::from_raw(request.material).is_some_and(MaterialId::is_solid) {
        for offset in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            let position = std::array::from_fn(|i| request.position[i] + offset[i]);
            if chunks.block_at(position) != Some(40) {
                continue;
            }
            let facing = journal.object_state(position).map_or(0, |s| s.facing);
            let normal = terra_core::block_shapes::torch_normal(facing);
            let support = std::array::from_fn(|i| position[i] - normal[i] as i64);
            if support != request.position {
                continue;
            }
            if commit_block_edit(
                chunks,
                journal,
                &BlockEditRequest {
                    position,
                    expected_material: 40,
                    material: 0,
                },
                properties,
            )? {
                journal.gameplay_mut().drop_one(
                    position.map(|v| v as f64 + 0.5),
                    terra_core::ItemKind::Block(40),
                );
            }
        }
    }
    if MaterialId::from_raw(request.material).is_some_and(MaterialId::is_solid)
        && !matches!(request.material, 5 | 98)
        && !properties
            .0
            .get(&request.material)
            .is_some_and(|definition| definition.struct_block)
        && let Some(y) = request.position[1].checked_sub(1)
    {
        let below = [request.position[0], y, request.position[2]];
        if chunks.block_at(below) == Some(MaterialId::Grass as u16) {
            commit_block_edit(
                chunks,
                journal,
                &BlockEditRequest {
                    position: below,
                    expected_material: MaterialId::Grass as u16,
                    material: MaterialId::Dirt as u16,
                },
                properties,
            )?;
        }
    }
    if reconcile {
        super::support::after_edit(chunks, journal, request.position, properties)?;
    }
    Ok(true)
}

fn retire_affected_props(
    request: &BlockEditRequest,
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
) {
    let (owner, local) = global_to_section(request.position);
    let mut owners = vec![owner];
    for axis in 0..3 {
        if matches!(local[axis], 0 | 15) {
            let mut offset = [0; 3];
            offset[axis] = if local[axis] == 0 { -1 } else { 1 };
            if let Ok(neighbor) = owner.offset(offset) {
                owners.push(neighbor);
            }
        }
    }
    if MaterialId::from_raw(request.material).is_some_and(MaterialId::is_solid) {
        // Fronds can reach from the preceding section into this cell. Include
        // adjacent footprints too, but never scan the entire loaded world.
        for dx in -1..=1 {
            if dx != 0 && !(dx == -1 && local[0] == 0 || dx == 1 && local[0] == 15) {
                continue;
            }
            for dz in -1..=1 {
                if dz != 0 && !(dz == -1 && local[2] == 0 || dz == 1 && local[2] == 15) {
                    continue;
                }
                for dy in [-1, 0] {
                    if let Ok(neighbor) = owner.offset([dx, dy, dz])
                        && !owners.contains(&neighbor)
                    {
                        owners.push(neighbor);
                    }
                }
            }
        }
    }
    for position in owners {
        let Some(section) = chunks.loaded.get_mut(&position) else {
            continue;
        };
        for prop in &mut section.props {
            if prop.removed {
                continue;
            }
            let support = crate::props::placement::support(prop);
            let covered = request.material != 0
                && !(prop.water_tolerant() && request.material == MaterialId::FluidWater as u16)
                && if prop.wall_attached() || prop.kind.aquatic() {
                    let bounds = prop.bounds();
                    (0..3).all(|axis| {
                        bounds[1][axis] > request.position[axis] as f64 + 0.001
                            && bounds[0][axis] < request.position[axis] as f64 + 0.999
                    })
                } else {
                    request.position == [support[0], support[1] + 1, support[2]]
                };
            if support != request.position && !covered {
                continue;
            }
            match journal.remove_prop(position, prop.id) {
                Ok(_) => prop.removed = true,
                Err(error) => {
                    tracing::error!(%error,"Cannot retire vegetation affected by a block edit")
                }
            }
        }
    }
}

pub fn commit_prop_removal(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    request: &PropRemoveRequest,
) -> Result<bool, String> {
    let Some(section) = chunks.loaded.get_mut(&request.section) else {
        return Ok(false);
    };
    let Some(prop) = section
        .props
        .iter_mut()
        .find(|p| p.id == request.id && !p.removed)
    else {
        return Ok(false);
    };
    let changed = journal
        .remove_prop(request.section, request.id)
        .map_err(|e| e.to_string())?;
    prop.removed = true;
    crate::props::placement::reflow(&mut section.props);
    Ok(changed)
}

pub fn commit_prop_placement(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    mut prop: crate::props::WorldProp,
) -> Result<bool, String> {
    let Some(section) = chunks.loaded.get_mut(&prop.section) else {
        return Ok(false);
    };
    if !prop.valid() || section.props.iter().any(|p| p.id == prop.id) {
        return Ok(false);
    }
    let support = crate::props::placement::support(&prop);
    let Some(slot) = crate::props::placement::next_slot(&section.props, support, prop.kind) else {
        return Ok(false);
    };
    crate::props::placement::align(&mut prop, support, slot);
    if !journal.place_prop(prop).map_err(|e| e.to_string())? {
        return Ok(false);
    }
    section.props.push(prop);
    crate::props::placement::reflow(&mut section.props);
    Ok(true)
}

pub(super) fn apply_prop_removals(
    access: Res<WorldAccess>,
    mut requests: MessageReader<PropRemoveRequest>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
) {
    for request in requests.read() {
        if access.authoritative()
            && let Err(error) = commit_prop_removal(&mut chunks, &mut journal, request)
        {
            tracing::error!(%error,"Prop removal failed");
        }
    }
}

#[cfg(test)]
mod attached_tests {
    use super::*;
    use crate::{
        props::{PropId, PropKind, WorldProp},
        streaming::LoadedSection,
    };
    use terra_voxel::{BlockSection, SectionPos};

    #[test]
    fn removing_support_or_covering_wall_fungi_retires_only_that_cluster_across_section_edges() {
        let owner = SectionPos::new(0, 0, -1);
        let neighbor = SectionPos::new(1, 0, -1);
        let prop = WorldProp {
            id: PropId(700),
            kind: PropKind::Mushroom,
            position: [16.14, 0.20, -0.5],
            rotation: std::f32::consts::FRAC_PI_2,
            scale: 0.4,
            variant: 128,
            section: owner,
            removed: false,
        };
        let other = WorldProp {
            id: PropId(701),
            position: [16.14, 3.20, -0.5],
            ..prop
        };
        assert!(prop.valid() && other.valid());
        for edit in [
            BlockEditRequest {
                position: [15, 0, -1],
                expected_material: 6,
                material: 0,
            },
            BlockEditRequest {
                position: [16, 0, -1],
                expected_material: 0,
                material: 3,
            },
        ] {
            let mut chunks = ChunkStore::default();
            let mut data = BlockSection::empty(owner).unwrap();
            data.set_block([15, 0, 15], 6).unwrap();
            data.set_block([15, 3, 15], 6).unwrap();
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![prop, other],
                },
            );
            chunks.loaded.insert(
                neighbor,
                LoadedSection {
                    data: BlockSection::empty(neighbor).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            let mut journal = EditJournal::default();
            assert!(
                commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &edit,
                    &BlockPropertiesCatalog::default()
                )
                .unwrap()
            );
            assert!(journal.prop_removed(owner, prop.id));
            assert!(!journal.prop_removed(owner, other.id));
            assert!(chunks.loaded[&owner].props[0].removed);
            assert!(!chunks.loaded[&owner].props[1].removed);
        }
    }
}
