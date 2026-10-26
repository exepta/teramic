//! One logical targeting implementation for clients and authoritative validation.
use crate::{
    props::{PropHit, PropPose, raycast_props_with_lifecycle},
    streaming::ChunkStore,
};
use terra_voxel::{SectionPos, blocks::BlockHit};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WorldTarget {
    Block(BlockHit),
    Prop(PropHit),
}

/// Extend the clicked mineral chain, never place a neighbouring shaft by
/// accident. New shafts are placed by clicking the actual supporting rock.
pub fn item_placement(
    chunks: &ChunkStore,
    hit: BlockHit,
    material: u16,
) -> Option<([i64; 3], u16)> {
    if crate::segments::is_crystal(material) && chunks.block_at(hit.position) == Some(material) {
        let position = crate::segments::extension(hit.position, material, |p| chunks.block_at(p))?;
        return Some((position, chunks.block_at(position)?));
    }
    block_placement(chunks, hit)
}

pub fn item_placement_with_state(
    chunks: &ChunkStore,
    journal: &crate::edits::EditJournal,
    hit: BlockHit,
    material: u16,
) -> Option<([i64; 3], u16)> {
    if material == terra_core::crusher::CRANK_MATERIAL {
        let anchor = journal.crusher_primary(hit.position)?;
        let state = journal.object_state(anchor)?;
        if hit.normal.map(i64::from) != state.right() {
            return None;
        }
        let position = journal.crusher_crank_position(anchor)?;
        let expected = chunks.block_at(position)?;
        return matches!(expected, 0 | 27).then_some((position, expected));
    }
    if terra_core::construction::is_slab(material)
        && chunks
            .block_at(hit.position)
            .is_some_and(terra_core::construction::is_slab)
        && terra_core::construction::can_merge_slab(
            journal
                .object_state(hit.position)
                .unwrap_or_default()
                .facing,
            hit.normal.map(|v| v as i8),
        )
    {
        return Some((hit.position, chunks.block_at(hit.position)?));
    }
    item_placement(chunks, hit, material)
}

/// A crank connects to the gear face, never another face of the same machine cell.
pub fn hand_crank_mount_hit(
    journal: &crate::edits::EditJournal,
    hit: BlockHit,
    ray: ([f64; 3], [f64; 3]),
) -> bool {
    let Some(anchor) = journal.crusher_primary(hit.position) else {
        return false;
    };
    let Some(state) = journal.object_state(anchor) else {
        return false;
    };
    let right = state.right();
    if hit.normal.map(i64::from) != right {
        return false;
    }
    let forward = [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]][usize::from(state.facing)];
    let point: [f64; 3] = std::array::from_fn(|axis| {
        ray.0[axis] + ray.1[axis] * hit.distance
            - anchor[axis] as f64
            - if axis == 1 { 0.0 } else { 0.5 }
    });
    let x = (0..3)
        .map(|axis| point[axis] * right[axis] as f64)
        .sum::<f64>()
        + 0.5;
    let z = (0..3)
        .map(|axis| point[axis] * forward[axis] as f64)
        .sum::<f64>()
        + 0.5;
    (x - 1.997).abs() <= 0.07
        && (point[1] - 0.455).powi(2) + (z - 0.515).powi(2) <= 0.235f64.powi(2)
}

/// The same proposed half-cell volume drives local and authoritative body checks.
pub fn placement_bounds(
    chunks: &ChunkStore,
    journal: &crate::edits::EditJournal,
    hit: BlockHit,
    material: u16,
    ray: ([f64; 3], [f64; 3]),
) -> Option<terra_core::block_shapes::ShapeBox> {
    let (position, expected) = item_placement_with_state(chunks, journal, hit, material)?;
    if material == terra_core::crusher::CRANK_MATERIAL {
        let facing = placement_facing(chunks, journal, hit, material, ray);
        let mut union: Option<terra_core::block_shapes::ShapeBox> = None;
        crate::interaction_shapes::visit_placement_boxes(
            chunks,
            journal,
            &terra_core::block_properties::BlockPropertiesCatalog::default(),
            position,
            material,
            facing,
            |bounds| {
                if let Some(union) = union.as_mut() {
                    for axis in 0..3 {
                        union.minimum[axis] = union.minimum[axis].min(bounds.minimum[axis]);
                        union.maximum[axis] = union.maximum[axis].max(bounds.maximum[axis]);
                    }
                } else {
                    union = Some(bounds);
                }
            },
        );
        return union;
    }
    let facing = if terra_core::construction::is_slab(expected) {
        terra_core::construction::DOUBLE_SLAB
    } else {
        placement_facing(chunks, journal, hit, material, ray)
    };
    let mut bounds =
        terra_core::construction::slab_bounds(if terra_core::construction::is_slab(material) {
            facing
        } else {
            terra_core::construction::DOUBLE_SLAB
        });
    for (axis, value) in position.into_iter().enumerate() {
        bounds.minimum[axis] += value as f64;
        bounds.maximum[axis] += value as f64;
    }
    Some(bounds)
}

pub fn placement_facing(
    chunks: &ChunkStore,
    journal: &crate::edits::EditJournal,
    hit: BlockHit,
    material: u16,
    ray: ([f64; 3], [f64; 3]),
) -> u8 {
    if material == terra_core::crusher::CRANK_MATERIAL {
        return journal
            .crusher_primary(hit.position)
            .and_then(|anchor| journal.object_state(anchor))
            .map_or(0, |state| state.facing);
    }
    let old = journal
        .object_state(hit.position)
        .unwrap_or_default()
        .facing;
    if terra_core::construction::is_slab(material)
        && chunks
            .block_at(hit.position)
            .is_some_and(terra_core::construction::is_slab)
        && hit.normal[1] != 0
        && (5..=8).contains(&old)
    {
        return old;
    }
    terra_core::construction::placement_at(
        material,
        hit.normal.map(|v| v as i8),
        ray.1,
        std::array::from_fn(|i| ray.0[i] + ray.1[i] * hit.distance),
    )
}

/// A source is replaceable fluid, not a solid placement support. Ordinary liquid
/// occupies air cells already, so placing against the bed displaces it as well.
pub fn block_placement(chunks: &ChunkStore, hit: BlockHit) -> Option<([i64; 3], u16)> {
    use terra_voxel::MaterialId;
    let position = if chunks.block_at(hit.position) == Some(MaterialId::FluidWater as u16) {
        hit.position
    } else if hit.normal != [0; 3] {
        hit.adjacent()
    } else {
        return None;
    };
    let expected = chunks.block_at(position)?;
    matches!(
        MaterialId::from_raw(expected),
        Some(MaterialId::Air | MaterialId::FluidWater)
    )
    .then_some((position, expected))
}

pub fn pick_world(
    chunks: &ChunkStore,
    origin: [f64; 3],
    direction: [f64; 3],
) -> (Option<BlockHit>, Option<WorldTarget>) {
    pick_world_with_pose(chunks, origin, direction, PropPose::Rest)
}

pub fn pick_world_density(
    chunks: &ChunkStore,
    origin: [f64; 3],
    direction: [f64; 3],
    _density: f32,
) -> (Option<BlockHit>, Option<WorldTarget>) {
    pick_world(chunks, origin, direction)
}
pub fn pick_world_with_pose(
    chunks: &ChunkStore,
    origin: [f64; 3],
    direction: [f64; 3],
    pose: PropPose,
) -> (Option<BlockHit>, Option<WorldTarget>) {
    pick_world_with_context(
        chunks,
        &crate::edits::EditJournal::default(),
        &terra_core::block_properties::BlockPropertiesCatalog::default(),
        origin,
        direction,
        pose,
    )
}

pub fn pick_world_with_context(
    chunks: &ChunkStore,
    journal: &crate::edits::EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    origin: [f64; 3],
    direction: [f64; 3],
    pose: PropPose,
) -> (Option<BlockHit>, Option<WorldTarget>) {
    pick_context(chunks, journal, properties, origin, direction, pose, false)
}

/// Planting targets the bed through source water, while actual prop geometry still occludes it.
pub fn pick_world_for_planting(
    chunks: &ChunkStore,
    journal: &crate::edits::EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    origin: [f64; 3],
    direction: [f64; 3],
    pose: PropPose,
) -> (Option<BlockHit>, Option<WorldTarget>) {
    pick_context(chunks, journal, properties, origin, direction, pose, true)
}

fn pick_context(
    chunks: &ChunkStore,
    journal: &crate::edits::EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    origin: [f64; 3],
    direction: [f64; 3],
    pose: PropPose,
    planting: bool,
) -> (Option<BlockHit>, Option<WorldTarget>) {
    if origin
        .iter()
        .any(|v| !v.is_finite() || v.abs() > (1_i64 << 40) as f64 - 32.0)
        || direction.iter().any(|v| !v.is_finite() || v.abs() > 1.001)
    {
        return (None, None);
    }
    let solid = || {
        crate::interaction_shapes::raycast_solid(
            chunks, journal, properties, origin, direction, 8.0,
        )
    };
    let obstruction = if planting {
        solid()
    } else {
        crate::interaction_shapes::raycast_with_pose(
            chunks, journal, properties, origin, direction, 8.0, pose,
        )
    };
    let block = obstruction.filter(|hit| chunks.block_at(hit.position).is_some());
    let prop_obstruction = if block.is_some_and(|hit| chunks.block_at(hit.position) == Some(27)) {
        solid()
    } else {
        obstruction
    };
    let reach = prop_obstruction.map_or(8.0, |hit| hit.distance);
    let end: [f64; 3] = std::array::from_fn(|i| origin[i] + direction[i] * reach);
    // Maximum accepted prop scale is 2.0 (visually 2.6); its owner is the
    // supporting block, up to 3.6 metres below a tall reed's visible top.
    let margin = [2.0, 4.0, 2.0];
    let lower: [i64; 3] = std::array::from_fn(|i| {
        ((origin[i].min(end[i]) - margin[i]).floor() as i64).div_euclid(16)
    });
    let upper: [i64; 3] = std::array::from_fn(|i| {
        ((origin[i].max(end[i]) + margin[i]).floor() as i64).div_euclid(16)
    });
    let mut closest: Option<PropHit> = None;
    for z in lower[2]..=upper[2] {
        for y in lower[1]..=upper[1] {
            for x in lower[0]..=upper[0] {
                if let Some(section) = chunks.loaded.get(&SectionPos::new(x, y, z))
                    && let Some(hit) = raycast_props_with_lifecycle(
                        origin,
                        direction,
                        closest.map_or(reach, |hit| hit.distance),
                        // Crop leaves may overhang an adjacent empty field. Seed
                        // placement targets soil; occupancy is validated separately.
                        section.props.iter().filter(|p| {
                            !p.removed
                                && (!planting || p.kind.crop().is_none())
                                && crate::props::placement::supported(p, |p| chunks.block_at(p))
                                    == Some(true)
                                && !crate::props::seasonal::snow_buried(p, chunks, journal)
                        }),
                        pose,
                        Some(&journal.flowers),
                    )
                    && closest.is_none_or(|old| {
                        hit.distance < old.distance
                            || (hit.distance == old.distance && hit.prop.id < old.prop.id)
                    })
                {
                    closest = Some(hit);
                }
            }
        }
    }
    // Large silk batches retain one persisted anchor owner. Their far capture
    // surface remains selectable across two section boundaries without widening
    // every grass/flower picking query or testing their strands twice.
    let web_margin = [
        crate::props::webs::MAX_SPAN + 1.0,
        6.0,
        crate::props::webs::MAX_SPAN + 1.0,
    ];
    let web_lower: [i64; 3] = std::array::from_fn(|i| {
        ((origin[i].min(end[i]) - web_margin[i]).floor() as i64).div_euclid(16)
    });
    let web_upper: [i64; 3] = std::array::from_fn(|i| {
        ((origin[i].max(end[i]) + web_margin[i]).floor() as i64).div_euclid(16)
    });
    for z in web_lower[2]..=web_upper[2] {
        for y in web_lower[1]..=web_upper[1] {
            for x in web_lower[0]..=web_upper[0] {
                if (lower[0]..=upper[0]).contains(&x)
                    && (lower[1]..=upper[1]).contains(&y)
                    && (lower[2]..=upper[2]).contains(&z)
                {
                    continue;
                }
                if let Some(section) = chunks.loaded.get(&SectionPos::new(x, y, z))
                    && let Some(hit) = raycast_props_with_lifecycle(
                        origin,
                        direction,
                        closest.map_or(reach, |hit| hit.distance),
                        section
                            .props
                            .iter()
                            .filter(|p| !p.removed && crate::props::webs::is_construction(p)),
                        pose,
                        Some(&journal.flowers),
                    )
                    && closest.is_none_or(|old| {
                        hit.distance < old.distance
                            || (hit.distance == old.distance && hit.prop.id < old.prop.id)
                    })
                {
                    closest = Some(hit);
                }
            }
        }
    }
    (
        block,
        closest
            .map(WorldTarget::Prop)
            .or_else(|| block.map(WorldTarget::Block)),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn far_spanning_web_is_targetable_and_removable_across_two_owner_section_boundaries() {
        use super::*;
        use crate::{
            edits::EditJournal,
            props::{PropId, webs},
            streaming::{LoadedSection, PropRemoveRequest, commit_prop_removal},
        };
        let prop = webs::spanning([-1, 11, 8], [1, 0, 0], 21, 10, PropId(4119)).unwrap();
        let mut chunks = ChunkStore::default();
        for x in -1..=2 {
            let position = SectionPos::new(x, 0, 0);
            chunks.loaded.insert(
                position,
                LoadedSection {
                    data: terra_voxel::BlockSection::empty(position).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: if position == prop.section {
                        vec![prop]
                    } else {
                        Vec::new()
                    },
                },
            );
        }
        let mut journal = EditJournal::default();
        assert!(journal.place_prop(prop).unwrap());
        let point = webs::world_point(&prop, bevy_math::Vec3::new(9.32, -0.70, -0.20));
        assert_eq!(
            terra_voxel::global_to_section(point.map(|v| v.floor() as i64))
                .0
                .x
                - prop.section.x,
            2
        );
        let origin = [point[0], point[1], point[2] + 2.0];
        let (_, hit) = pick_world_with_context(
            &chunks,
            &journal,
            &Default::default(),
            origin,
            [0.0, 0.0, -1.0],
            PropPose::Rest,
        );
        let Some(WorldTarget::Prop(hit)) = hit else {
            panic!("far capture surface must be picked")
        };
        assert_eq!(hit.prop.id, prop.id);
        assert_eq!(hit.prop.section, prop.section);
        assert!(
            commit_prop_removal(
                &mut chunks,
                &mut journal,
                &PropRemoveRequest {
                    id: hit.prop.id,
                    section: hit.prop.section
                }
            )
            .unwrap()
        );
        let (_, hit) = pick_world_with_context(
            &chunks,
            &journal,
            &Default::default(),
            origin,
            [0.0, 0.0, -1.0],
            PropPose::Rest,
        );
        assert!(hit.is_none());
        let mut restored = Vec::new();
        journal.apply_prop_removals(prop.section, &mut restored);
        assert!(restored.iter().all(|p| p.removed));
    }

    #[test]
    fn hand_crank_targets_the_same_gear_from_either_parent_half_and_rejects_other_faces() {
        use super::*;
        use crate::{
            edits::{BlockObjectState, EditJournal},
            streaming::LoadedSection,
        };
        use terra_voxel::BlockSection;
        for facing in 0..4 {
            let anchor = [-1, 4, -1];
            let state = BlockObjectState {
                facing,
                ..Default::default()
            };
            let peer = std::array::from_fn(|axis| anchor[axis] + state.right()[axis]);
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            for x in -1..=1 {
                for z in -1..=1 {
                    let section = SectionPos::new(x, 0, z);
                    chunks.loaded.insert(
                        section,
                        LoadedSection {
                            data: BlockSection::empty(section).unwrap(),
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: 0,
                            water: None,
                            props: Vec::new(),
                        },
                    );
                }
            }
            for (position, other, secondary) in [(anchor, peer, false), (peer, anchor, true)] {
                let (owner, local) = terra_voxel::global_to_section(position);
                chunks
                    .loaded
                    .get_mut(&owner)
                    .unwrap()
                    .data
                    .set_block(local, 99)
                    .unwrap();
                journal.record(position, 99).unwrap();
                journal
                    .set_object_state(
                        position,
                        Some(BlockObjectState {
                            peer: Some(other),
                            secondary,
                            ..state
                        }),
                    )
                    .unwrap();
            }
            let mount = journal.crusher_crank_position(anchor).unwrap();
            let right = state.right();
            let forward = [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]][usize::from(facing)];
            let point: [f64; 3] = std::array::from_fn(|axis| {
                anchor[axis] as f64
                    + if axis == 1 {
                        0.455
                    } else {
                        0.5 + right[axis] as f64 * 1.497 + forward[axis] as f64 * 0.015
                    }
            });
            let ray = (
                std::array::from_fn(|axis| point[axis] + right[axis] as f64 * 2.0),
                right.map(|value| -(value as f64)),
            );
            for position in [anchor, peer] {
                let hit = BlockHit {
                    position,
                    normal: right.map(|value| value as i32),
                    distance: 2.0,
                };
                assert_eq!(
                    item_placement_with_state(&chunks, &journal, hit, 100),
                    Some((mount, 0))
                );
                assert!(hand_crank_mount_hit(&journal, hit, ray));
                assert_eq!(placement_facing(&chunks, &journal, hit, 100, ray), facing);
                let wrong = BlockHit {
                    normal: [0, 1, 0],
                    ..hit
                };
                assert!(item_placement_with_state(&chunks, &journal, wrong, 100).is_none());
                assert!(!hand_crank_mount_hit(&journal, wrong, ray));
                let mut high = ray;
                high.0[1] += 0.5;
                assert!(!hand_crank_mount_hit(&journal, hit, high));
            }
            journal.record(mount, 100).unwrap();
            journal
                .set_object_state(
                    mount,
                    Some(BlockObjectState {
                        peer: Some(anchor),
                        ..state
                    }),
                )
                .unwrap();
            let delta = journal.section_delta(terra_voxel::global_to_section(mount).0);
            let mut copy = EditJournal::default();
            copy.import_delta(delta).unwrap();
            assert_eq!(copy.object_state(mount), journal.object_state(mount));
        }
    }
    #[test]
    fn vertical_slabs_use_half_cell_collision_and_continue_the_same_plane_above() {
        use super::*;
        use crate::{
            edits::{BlockObjectState, EditJournal},
            streaming::LoadedSection,
        };
        use terra_voxel::BlockSection;
        let mut chunks = ChunkStore::default();
        let section = SectionPos::default();
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block([4, 4, 4], 3).unwrap();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let mut journal = EditJournal::default();
        let hit = BlockHit {
            position: [4, 4, 4],
            normal: [1, 0, 0],
            distance: 2.0,
        };
        let bounds = placement_bounds(
            &chunks,
            &journal,
            hit,
            82,
            ([7.0, 4.5, 4.5], [-1.0, 0.0, 0.0]),
        )
        .unwrap();
        assert_eq!(bounds.minimum, [5.0, 4.0, 4.0]);
        assert_eq!(bounds.maximum, [5.5, 5.0, 5.0]);
        assert!(!bounds.intersects([5.6, 4.1, 4.1], [5.9, 4.9, 4.9]));
        assert!(bounds.intersects([5.1, 4.1, 4.1], [5.4, 4.9, 4.9]));
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block([4, 4, 4], 82)
            .unwrap();
        journal.record([4, 4, 4], 82).unwrap();
        journal
            .set_object_state(
                [4, 4, 4],
                Some(BlockObjectState {
                    facing: 5,
                    ..Default::default()
                }),
            )
            .unwrap();
        let top = BlockHit {
            position: [4, 4, 4],
            normal: [0, 1, 0],
            distance: 1.0,
        };
        assert_eq!(
            placement_facing(
                &chunks,
                &journal,
                top,
                85,
                ([4.2, 6.0, 4.5], [0.0, -1.0, 0.0])
            ),
            5
        );
        let bounds = placement_bounds(
            &chunks,
            &journal,
            top,
            85,
            ([4.2, 6.0, 4.5], [0.0, -1.0, 0.0]),
        )
        .unwrap();
        assert_eq!(bounds.minimum, [4.0, 5.0, 4.0]);
        assert_eq!(bounds.maximum, [4.5, 6.0, 5.0]);
    }
    use super::*;
    use crate::{
        props::{PropId, PropKind, WorldProp},
        streaming::LoadedSection,
    };
    use terra_voxel::BlockSection;

    #[test]
    fn planting_reaches_the_bed_through_source_water_but_not_through_props_or_solid_blocks() {
        let section = SectionPos::default();
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block([4, 3, 4], 3).unwrap();
        for y in 4..=6 {
            data.set_block([4, y, 4], 27).unwrap();
        }
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let journal = crate::edits::EditJournal::default();
        let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
        let origin = [4.5, 7.5, 4.5];
        let direction = [0.0, -1.0, 0.0];
        assert_eq!(
            pick_world(&chunks, origin, direction).0.unwrap().position,
            [4, 6, 4]
        );
        let (_, selected) = pick_world_for_planting(
            &chunks,
            &journal,
            &properties,
            origin,
            direction,
            PropPose::Rest,
        );
        assert!(
            matches!(selected,Some(WorldTarget::Block(hit)) if hit.position == [4,3,4] && hit.normal == [0,1,0])
        );
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block([4, 4, 4], 0)
            .unwrap();
        let prop = WorldProp {
            id: PropId(78),
            kind: PropKind::BeeNest,
            position: [4.5, 4.0, 4.5],
            rotation: 0.0,
            scale: 1.0,
            variant: 0,
            section,
            removed: false,
        };
        chunks.loaded.get_mut(&section).unwrap().props.push(prop);
        for (_, selected) in [
            pick_world(&chunks, origin, direction),
            pick_world_for_planting(
                &chunks,
                &journal,
                &properties,
                origin,
                direction,
                PropPose::Rest,
            ),
        ] {
            assert!(matches!(selected,Some(WorldTarget::Prop(hit)) if hit.prop.id == prop.id));
        }
        chunks.loaded.get_mut(&section).unwrap().props.clear();
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block([4, 5, 4], 3)
            .unwrap();
        let (_, selected) = pick_world_for_planting(
            &chunks,
            &journal,
            &properties,
            origin,
            direction,
            PropPose::Rest,
        );
        assert!(matches!(selected,Some(WorldTarget::Block(hit)) if hit.position == [4,5,4]));
    }

    #[test]
    fn planting_ignores_overhanging_crop_leaves_but_mining_decorations_and_walls_keep_real_geometry()
     {
        let mut chunks = ChunkStore::default();
        for x in -1..=0 {
            for z in -1..=0 {
                let section = SectionPos::new(x, 0, z);
                chunks.loaded.insert(
                    section,
                    LoadedSection {
                        data: BlockSection::empty(section).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
        }
        for x in -2..=0 {
            for z in -2..=0 {
                let (owner, local) = terra_voxel::global_to_section([x, 4, z]);
                chunks
                    .loaded
                    .get_mut(&owner)
                    .unwrap()
                    .data
                    .set_block(local, terra_core::farming::FARMLAND)
                    .unwrap();
            }
        }
        let owner = SectionPos::new(-1, 0, -1);
        let corn = WorldProp {
            id: PropId(801),
            kind: PropKind::Corn,
            position: [-0.5, 5.0, -0.5],
            rotation: 0.0,
            scale: 1.0,
            variant: 7,
            section: owner,
            removed: false,
        };
        chunks.loaded.get_mut(&owner).unwrap().props.push(corn);
        let journal = crate::edits::EditJournal::default();
        let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
        let direction = [0.0, -1.0, 0.0];
        let origin = (0..=7).flat_map(|x| (0..=32).map(move |z| [0.01 + f64::from(x) * 0.04, 8.5, -1.3 + f64::from(z) * 0.05]))
            .find(|origin| matches!(pick_world_with_context(&chunks, &journal, &properties, *origin, direction, PropPose::Rest).1,
                Some(WorldTarget::Prop(hit)) if hit.prop.id == corn.id))
            .expect("mature corn has real leaves overhanging the adjacent empty field");
        let ground = [origin[0].floor() as i64, 4, origin[2].floor() as i64];
        assert_ne!(ground, crate::props::placement::support(&corn));
        let pick = |chunks: &ChunkStore| {
            pick_world_for_planting(
                chunks,
                &journal,
                &properties,
                origin,
                direction,
                PropPose::Rest,
            )
            .1
        };
        assert!(
            matches!(pick(&chunks), Some(WorldTarget::Block(hit)) if hit.position == ground && hit.normal == [0,1,0])
        );

        let obstruction = [ground[0], 7, ground[2]];
        let (obstruction_owner, local) = terra_voxel::global_to_section(obstruction);
        chunks
            .loaded
            .get_mut(&obstruction_owner)
            .unwrap()
            .data
            .set_block(local, 3)
            .unwrap();
        assert!(
            matches!(pick(&chunks), Some(WorldTarget::Block(hit)) if hit.position == obstruction)
        );
        chunks
            .loaded
            .get_mut(&obstruction_owner)
            .unwrap()
            .data
            .set_block(local, 0)
            .unwrap();
        let nest = WorldProp {
            id: PropId(802),
            kind: PropKind::BeeNest,
            position: [origin[0], 7.0, origin[2]],
            rotation: 0.0,
            scale: 1.0,
            variant: 0,
            section: obstruction_owner,
            removed: false,
        };
        let (support_owner, support_local) =
            terra_voxel::global_to_section(crate::props::placement::support(&nest));
        chunks
            .loaded
            .get_mut(&support_owner)
            .unwrap()
            .data
            .set_block(support_local, 3)
            .unwrap();
        chunks
            .loaded
            .get_mut(&obstruction_owner)
            .unwrap()
            .props
            .push(nest);
        assert!(matches!(pick(&chunks), Some(WorldTarget::Prop(hit)) if hit.prop.id == nest.id));
    }

    #[test]
    fn enlarged_placed_reed_is_pickable_above_its_support_section() {
        let mut chunks = ChunkStore::default();
        for y in -1..=1 {
            let position = SectionPos::new(-1, y, -1);
            chunks.loaded.insert(
                position,
                LoadedSection {
                    data: BlockSection::empty(position).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
        let owner = SectionPos::new(-1, -1, -1);
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block([15, 15, 15], 3)
            .unwrap();
        let prop = WorldProp {
            id: PropId(7),
            kind: PropKind::SmallPlant,
            position: [-0.5, 0.0, -0.5],
            rotation: 0.0,
            scale: 2.0,
            variant: 0,
            section: owner,
            removed: false,
        };
        chunks.loaded.get_mut(&owner).unwrap().props.push(prop);
        let (_, target) = pick_world(&chunks, [-2.0, 2.1, -0.5], [1.0, 0.0, 0.0]);
        assert!(matches!(target, Some(WorldTarget::Prop(hit)) if hit.prop.id == prop.id));
        assert!(
            pick_world(&chunks, [-2.0, 2.5, -0.5], [1.0, 0.0, 0.0])
                .1
                .is_none(),
            "air above the actual reed must not hit its old enclosing box"
        );
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block([15, 15, 15], 0)
            .unwrap();
        assert!(
            pick_world(&chunks, [-2.0, 2.1, -0.5], [1.0, 0.0, 0.0])
                .1
                .is_none()
        );
        assert!(
            !chunks.loaded[&owner].props[0].removed,
            "presentation validation does not tombstone saved vegetation"
        );
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block([15, 15, 15], 3)
            .unwrap();
        assert!(
            matches!(pick_world(&chunks, [-2.0, 2.1, -0.5], [1.0, 0.0, 0.0]).1, Some(WorldTarget::Prop(hit)) if hit.prop.id == prop.id)
        );
    }
}
