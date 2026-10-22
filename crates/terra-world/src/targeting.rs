//! One logical targeting implementation for clients and authoritative validation.
use crate::{
    props::{PropHit, PropPose, raycast_props_with_pose},
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

/// The same proposed half-cell volume drives local and authoritative body checks.
pub fn placement_bounds(
    chunks: &ChunkStore,
    journal: &crate::edits::EditJournal,
    hit: BlockHit,
    material: u16,
    ray: ([f64; 3], [f64; 3]),
) -> Option<terra_core::block_shapes::ShapeBox> {
    let (position, expected) = item_placement_with_state(chunks, journal, hit, material)?;
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
                    && let Some(hit) = raycast_props_with_pose(
                        origin,
                        direction,
                        closest.map_or(reach, |hit| hit.distance),
                        section.props.iter().filter(|p| !p.removed),
                        pose,
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
    }
}
