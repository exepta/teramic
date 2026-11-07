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
        if access.authoritative() {
            let result = if request.material == MaterialId::SnowLayer as u16 {
                crate::snow::commit(
                    &mut chunks,
                    &mut journal,
                    request.position,
                    request.expected_material,
                    &properties,
                )
            } else {
                commit_block_edit(&mut chunks, &mut journal, request, &properties)
            };
            if let Err(error) = result {
                tracing::error!(%error,"Block edit failed");
            }
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
    commit(chunks, journal, request, properties, true, true, true)
}

/// A compound-hearth upgrade transfers ownership to its attached inventory.
/// The caller must retain and restore those contents as part of the upgrade.
pub fn commit_block_edit_preserving_inventory(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    request: &BlockEditRequest,
    properties: &BlockPropertiesCatalog,
) -> Result<bool, String> {
    commit(chunks, journal, request, properties, true, true, false)
}

pub(crate) fn commit_weather_cap(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    request: &BlockEditRequest,
    properties: &BlockPropertiesCatalog,
) -> Result<bool, String> {
    if !matches!(request.material, 0 | 5 | 98) {
        return Err("Invalid weather cap material".into());
    }
    commit(chunks, journal, request, properties, true, false, true)
}

/// Pins already-generated support for persistent gameplay without changing its material.
pub(crate) fn ensure_block_record(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    position: [i64; 3],
    material: u16,
) -> Result<bool, String> {
    if chunks.block_at(position) != Some(material) {
        return Ok(false);
    }
    if journal.block(position) == Some(material) {
        return Ok(true);
    }
    for section in journal
        .record(position, material)
        .map_err(|error| error.to_string())?
    {
        if chunks.is_wanted(section) {
            chunks.dirty.insert(section);
        }
    }
    Ok(true)
}

pub(super) fn commit_segment_edit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    request: &BlockEditRequest,
    properties: &BlockPropertiesCatalog,
) -> Result<bool, String> {
    commit(chunks, journal, request, properties, false, true, true)
}

fn commit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    request: &BlockEditRequest,
    properties: &BlockPropertiesCatalog,
    reconcile: bool,
    retire_props: bool,
    drop_inventory: bool,
) -> Result<bool, String> {
    if chunks.block_at(request.position) != Some(request.expected_material)
        || request.material == request.expected_material
    {
        return Ok(false);
    }
    let dependent_drops = crate::lanterns::released_dependents(
        chunks,
        journal,
        request.position,
        request.material,
        properties,
    )
    .len();
    if dependent_drops > 0
        && (journal
            .gameplay()
            .drops
            .len()
            .saturating_add(dependent_drops)
            > terra_core::MAX_WORLD_DROPS
            || journal
                .gameplay()
                .next_drop
                .checked_add(dependent_drops as u64)
                .is_none())
    {
        return Err("Mounted-light support removal exceeds item-drop capacity".into());
    }
    let (owner, local) = global_to_section(request.position);
    let inventory_drops = (drop_inventory
        && (matches!(request.expected_material, 29 | 59 | 99 | 156 | 161 | 162)
            || journal.container_at(request.position).is_some()
            || journal.attached_campfire_at(request.position).is_some()))
    .then(|| journal.block_inventory_drops(request.position));
    if inventory_drops.is_some() {
        journal.preflight_inventory_removal([request.position], 0)?;
        // record() also invalidates fluid/appearance meshes. Check their bounded
        // mutation budget before it can retire machine-owned persistent data.
        if journal.revision(owner) > u64::MAX - 16
            || journal.fluid_epoch(owner) > u64::MAX - 16
            || journal.appearance_epoch(owner) > u64::MAX - 16
        {
            return Err("Container block revision exhausted".into());
        }
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let neighbor = owner
                        .offset([dx, dy, dz])
                        .map_err(|error| error.to_string())?;
                    if journal.mesh_epoch(neighbor) > u64::MAX - 16
                        || journal.water_mesh_epoch(neighbor) > u64::MAX - 16
                    {
                        return Err("Container mesh revision exhausted".into());
                    }
                }
            }
        }
    }
    if request.material == 40 && crate::torches::wet_placement(chunks, journal, request.position) {
        return Err("Torches cannot be placed in water".into());
    }
    if request.material == terra_core::lantern::LANTERN
        && crate::torches::wet_placement(chunks, journal, request.position)
    {
        return Err("Lanterns cannot be placed in water".into());
    }
    // Headless generation caches 16x16 columns; mesh generation adds an 18x18
    // halo. Query global coordinates instead of assuming either slice layout.
    let retained_water = (crate::water::space::partial(request.material)
        && !matches!(request.material, 40 | terra_core::lantern::LANTERN))
    .then(|| {
        if request.expected_material == MaterialId::SolidWater as u16 {
            return crate::water::FluidCell {
                amount: 8,
                source: true,
                ..Default::default()
            };
        }
        journal.fluid(request.position).unwrap_or_else(|| {
            chunks
                .columns
                .get(&[owner.x, owner.z])
                .and_then(|column| column.surface_at(request.position[0], request.position[2]))
                .map_or_else(Default::default, |surface| {
                    crate::water::fluid::from_columns(surface.water_columns(), request.position[1])
                })
        })
    });
    // A pipe exactly above an integral lake plane is dry in its own voxel.
    // Preserve touching implicit water too, so its terrain surround cannot
    // cover the annulus at that plane or at a wet neighbouring face.
    let touching_water: Vec<_> = if terra_core::plumbing::is_pipe(request.material)
        && retained_water.is_none_or(|water| water.amount == 0)
    {
        terra_core::plumbing::DIRECTIONS
            .into_iter()
            .filter_map(|direction| {
                let p = std::array::from_fn(|axis| request.position[axis] + direction[axis]);
                if !chunks.block_at(p).is_some_and(crate::water::can_hold_fluid) {
                    return None;
                }
                let (section, _) = global_to_section(p);
                let water = journal.fluid(p).unwrap_or_else(|| {
                    chunks
                        .columns
                        .get(&[section.x, section.z])
                        .and_then(|column| column.surface_at(p[0], p[2]))
                        .map_or_else(Default::default, |surface| {
                            crate::water::fluid::from_columns(surface.water_columns(), p[1])
                        })
                });
                (water.amount > 0 && (p[1] >= request.position[1] || water.amount == 8))
                    .then_some((p, water))
            })
            .collect()
    } else {
        vec![]
    };
    let Some(section) = chunks.loaded.get_mut(&owner) else {
        return Ok(false);
    };
    if section.data.revision() == u64::MAX {
        return Err("Block revision overflow".into());
    }
    let affected = journal
        .record(request.position, request.material)
        .map_err(|e| e.to_string())?;
    if let Some(water) = retained_water.filter(|water| water.amount > 0) {
        journal
            .set_fluid(request.position, water)
            .map_err(|e| e.to_string())?;
    }
    for (position, water) in touching_water {
        journal
            .set_fluid(position, water)
            .map_err(|e| e.to_string())?;
    }
    section
        .data
        .set_block(local, request.material)
        .map_err(|e| e.to_string())?;
    if let Some(stacks) = inventory_drops {
        journal.finish_inventory_removal(request.position, stacks);
    }
    if retire_props
        && (request.expected_material != MaterialId::Dirt as u16
            || request.material != MaterialId::Grass as u16)
    {
        retire_affected_props(request, chunks, journal);
    }
    for p in affected {
        if chunks.is_wanted(p) {
            chunks.dirty.insert(p);
        }
    }
    tracing::debug!(position=?request.position,material=request.material,"Block edited");
    for offset in [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ] {
        let position = std::array::from_fn(|i| request.position[i] + offset[i]);
        let Some(material) = chunks
            .block_at(position)
            .filter(|m| *m == 40 || terra_core::lantern::is_part(*m))
        else {
            continue;
        };
        let facing = journal.object_state(position).map_or(0, |s| s.facing);
        let (anchor, supported) = if material == 40 {
            (
                crate::torches::support(position, facing),
                Some(crate::torches::supported(
                    chunks, properties, position, facing,
                )),
            )
        } else {
            (
                terra_core::lantern::support(position, material, facing),
                crate::lanterns::support_status(chunks, properties, position, material, facing),
            )
        };
        if anchor != Some(request.position) || supported != Some(false) {
            continue;
        }
        if commit_block_edit(
            chunks,
            journal,
            &BlockEditRequest {
                position,
                expected_material: material,
                material: 0,
            },
            properties,
        )? {
            journal.gameplay_mut().drop_one(
                position.map(|v| v as f64 + 0.5),
                terra_core::ItemKind::Block(material),
            );
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
    // The remote anchor may be edited after the first anchor owner unloaded.
    // Retire the sparse saved record as well, so reload cannot resurrect silk
    // whose load-bearing rock was removed while it was outside streaming range.
    for dx in -2..=2 {
        for dz in -2..=2 {
            for dy in 0..=1 {
                let Ok(position) = owner.offset([dx, dy, dz]) else {
                    continue;
                };
                if chunks.loaded.contains_key(&position) {
                    continue;
                }
                let removed: Vec<_> = journal
                    .placed_props(position)
                    .filter(|prop| {
                        if !crate::props::webs::is_construction(prop) {
                            return false;
                        }
                        let bounds = prop.bounds();
                        crate::props::webs::anchors(prop)
                            .is_some_and(|anchors| anchors.contains(&request.position))
                            || request.material != 0
                                && (0..3).all(|axis| {
                                    bounds[1][axis] > request.position[axis] as f64 + 0.001
                                        && bounds[0][axis] < request.position[axis] as f64 + 0.999
                                })
                    })
                    .map(|prop| prop.id)
                    .collect();
                for id in removed {
                    if let Err(error) = journal.remove_prop(position, id) {
                        tracing::error!(%error, "Cannot retire spanning silk with an unloaded owner");
                    }
                }
            }
        }
    }
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
    // A spanning silk construction can be anchored two sections away. Only
    // inspect that bounded halo for large webs affected by this exact edit.
    for dx in -2..=2 {
        for dz in -2..=2 {
            for dy in 0..=1 {
                let Ok(position) = owner.offset([dx, dy, dz]) else {
                    continue;
                };
                if owners.contains(&position) {
                    continue;
                }
                if chunks.loaded.get(&position).is_some_and(|section| {
                    section.props.iter().any(|prop| {
                        if prop.removed || !crate::props::webs::is_construction(prop) {
                            return false;
                        }
                        let anchor_removed = crate::props::webs::anchors(prop)
                            .is_some_and(|anchors| anchors.contains(&request.position));
                        let bounds = prop.bounds();
                        anchor_removed
                            || request.material != 0
                                && (0..3).all(|axis| {
                                    bounds[1][axis] > request.position[axis] as f64 + 0.001
                                        && bounds[0][axis] < request.position[axis] as f64 + 0.999
                                })
                    })
                }) {
                    owners.push(position);
                }
            }
        }
    }
    for position in owners {
        let Some(section) = chunks.loaded.get_mut(&position) else {
            continue;
        };
        for prop in &mut section.props {
            if prop.removed || prop.kind == crate::props::PropKind::AppleFruit {
                continue;
            }
            let support = crate::props::placement::support(prop);
            let covered = request.material != 0
                && !(prop.water_tolerant() && request.material == MaterialId::FluidWater as u16)
                && if prop.kind.crop().is_some() {
                    request.position[0] == support[0]
                        && request.position[2] == support[2]
                        && (support[1] + 1
                            ..=support[1]
                                + if prop.kind == crate::props::PropKind::Lettuce {
                                    1
                                } else {
                                    2
                                })
                            .contains(&request.position[1])
                } else if prop.wall_attached()
                    || prop.kind.aquatic()
                    || prop.kind == crate::props::PropKind::SpiderWeb
                {
                    let bounds = prop.bounds();
                    (0..3).all(|axis| {
                        bounds[1][axis] > request.position[axis] as f64 + 0.001
                            && bounds[0][axis] < request.position[axis] as f64 + 0.999
                    })
                } else {
                    request.position == [support[0], support[1] + 1, support[2]]
                };
            let web_anchor = crate::props::webs::anchors(prop)
                .is_some_and(|anchors| anchors.contains(&request.position));
            if support != request.position && !covered && !web_anchor {
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
    fn pinning_prepared_support_invalidates_only_interested_halos_once() {
        let support = [-1, 15, -1];
        let (owner, local) = global_to_section(support);
        let halo = owner.offset([1, 1, 1]).unwrap();
        let unrelated = owner.offset([-1, 0, 0]).unwrap();
        let mut chunks = ChunkStore::default();
        let mut data = BlockSection::empty(owner).unwrap();
        data.set_block(local, terra_core::farming::FARMLAND)
            .unwrap();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        chunks.wanted.extend([owner, halo, unrelated]);
        let mut journal = EditJournal::default();
        assert!(
            !ensure_block_record(&mut chunks, &mut journal, support, MaterialId::Stone as u16)
                .unwrap()
        );
        assert_eq!(journal.revision(owner), 0);
        assert!(chunks.dirty.is_empty());
        assert!(
            ensure_block_record(
                &mut chunks,
                &mut journal,
                support,
                terra_core::farming::FARMLAND
            )
            .unwrap()
        );
        assert_eq!(journal.block(support), Some(terra_core::farming::FARMLAND));
        assert_eq!(journal.revision(owner), 1);
        assert_eq!(journal.mesh_epoch(owner), 1);
        assert_eq!(journal.mesh_epoch(halo), 1);
        assert_eq!(chunks.dirty, [owner, halo].into_iter().collect());
        chunks.dirty.clear();
        assert!(
            ensure_block_record(
                &mut chunks,
                &mut journal,
                support,
                terra_core::farming::FARMLAND
            )
            .unwrap()
        );
        assert!(chunks.dirty.is_empty());
        assert_eq!(journal.revision(owner), 1);
        assert_eq!(journal.mesh_epoch(owner), 1);
        assert!(
            !ensure_block_record(
                &mut chunks,
                &mut journal,
                [100, 15, 100],
                terra_core::farming::FARMLAND
            )
            .unwrap()
        );
    }

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

#[cfg(test)]
mod waterlogged_placement_regression {
    use super::*;

    #[test]
    fn weather_caps_accept_unpadded_and_padded_columns_at_all_section_edges() {
        let generator = crate::SectionGenerator::new(
            crate::TerraGenerator::new(terra_core::WorldConfig {
                base_height: 0.,
                sea_level: -32.,
                hill_amplitude: 0.,
                mountain_amplitude: 0.,
                biomes: false,
                caves: false,
                trees: false,
                rivers: false,
                landscape_revision: 0,
                structures_revision: 0,
                ..Default::default()
            })
            .unwrap(),
        );
        for coordinate in [[0, 0], [-1, -1]] {
            for padded in [false, true] {
                let column =
                    std::sync::Arc::new(generator.prepare_column(coordinate, padded).unwrap());
                assert_eq!(column.water_columns().len(), if padded { 324 } else { 256 });
                let owner = terra_voxel::SectionPos::new(coordinate[0], 0, coordinate[1]);
                let mut chunks = ChunkStore::default();
                chunks.loaded.insert(
                    owner,
                    super::super::LoadedSection {
                        data: terra_voxel::BlockSection::empty(owner).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: vec![],
                    },
                );
                chunks.set_prepared_column_for_test(coordinate, Some(column));
                let mut journal = EditJournal::default();
                for z in 0..16 {
                    for x in 0..16 {
                        let position = [coordinate[0] * 16 + x, 7, coordinate[1] * 16 + z];
                        // Both snowfall and melting pass through the weather edit entrypoint.
                        for (expected_material, material) in [(0, 98), (98, 0)] {
                            assert!(
                                commit_weather_cap(
                                    &mut chunks,
                                    &mut journal,
                                    &BlockEditRequest {
                                        position,
                                        expected_material,
                                        material
                                    },
                                    &Default::default()
                                )
                                .unwrap(),
                                "{position:?}, padded={padded}"
                            );
                            assert_eq!(chunks.block_at(position), Some(material));
                            assert!(journal.fluid(position).is_none());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn partial_block_commit_keeps_implicit_lake_water_in_the_replica_delta() {
        let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
            base_height: 0.,
            sea_level: 8.,
            hill_amplitude: 0.,
            mountain_amplitude: 0.,
            biomes: false,
            caves: false,
            trees: false,
            rivers: false,
            landscape_revision: 0,
            structures_revision: 0,
            ..Default::default()
        })
        .unwrap();
        let generator = crate::SectionGenerator::new(terrain.clone());
        for coordinate in [[0, 0], [-1, -1]] {
            for padded in [false, true] {
                for material in [
                    163, 164, 165, 172, 74, 82, 90, 92, 28, 59, 60, 99, 166, 167, 168,
                ] {
                    for y in if terra_core::plumbing::is_pipe(material) {
                        vec![7, 8]
                    } else {
                        vec![7]
                    } {
                        let p = [coordinate[0] * 16 + 15, y, coordinate[1] * 16 + 15];
                        let (owner, _) = global_to_section(p);
                        let mut chunks = ChunkStore::default();
                        chunks.loaded.insert(
                            owner,
                            super::super::LoadedSection {
                                data: terra_voxel::BlockSection::empty(owner).unwrap(),
                                mesh: None,
                                mesh_epoch: 0,
                                water_epoch: 0,
                                water: None,
                                props: vec![],
                            },
                        );
                        chunks.set_prepared_column_for_test(
                            coordinate,
                            Some(std::sync::Arc::new(
                                generator.prepare_column(coordinate, padded).unwrap(),
                            )),
                        );
                        let mut journal = EditJournal::default();
                        assert!(journal.fluid(p).is_none());
                        assert!(
                            commit_block_edit(
                                &mut chunks,
                                &mut journal,
                                &BlockEditRequest {
                                    position: p,
                                    expected_material: 0,
                                    material
                                },
                                &Default::default()
                            )
                            .unwrap()
                        );
                        let wet = [p[0], 7, p[2]];
                        let water = journal.fluid(wet).unwrap();
                        assert_eq!(water.amount, 8);
                        assert!(water.source);
                        let mut replica = EditJournal::default();
                        for section in journal.changed_sections() {
                            replica
                                .import_delta(journal.section_delta(section))
                                .unwrap();
                        }
                        assert_eq!(replica.fluid(wet), Some(water));
                        if y == 8 {
                            assert!(journal.fluid(p).is_none_or(|water| water.amount == 0));
                        }
                    }
                }
            }
        }
    }
}
