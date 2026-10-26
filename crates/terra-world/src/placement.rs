//! Shared placement layout and authored collision for prediction and authority.
use crate::{
    edits::{BlockObjectState, EditJournal},
    streaming::{BlockEditRequest, ChunkStore},
};
use bevy_math::{Quat, Vec3};
use terra_core::{block_properties::BlockPropertiesCatalog, block_shapes::ShapeBox};

#[derive(Clone, Debug)]
pub struct Layout {
    pub edits: Vec<BlockEditRequest>,
    pub states: Vec<([i64; 3], Option<BlockObjectState>)>,
}
pub fn furniture(material: u16) -> bool {
    matches!(material, 28..=29 | 59..=73 | 99..=100 | 131)
        || terra_core::construction::is_shape(material)
        || terra_core::textiles::is_carpet(material)
}
fn offset(p: [i64; 3], d: [i64; 3], sign: i64) -> [i64; 3] {
    std::array::from_fn(|i| p[i] + d[i] * sign)
}

pub fn layout(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: [i64; 3],
    expected: u16,
    material: u16,
    look: [f32; 2],
    crank_owner: Option<[i64; 3]>,
) -> Result<Layout, String> {
    if position.iter().any(|p| p.unsigned_abs() > 990_000) || look.iter().any(|v| !v.is_finite()) {
        return Err("Invalid placement coordinates".into());
    }
    let mut result = Layout {
        edits: vec![BlockEditRequest {
            position,
            expected_material: expected,
            material,
        }],
        states: Vec::new(),
    };
    if material == terra_core::flower_pot::MATERIAL || terra_core::textiles::is_carpet(material) {
        let support = chunks
            .block_at([position[0], position[1] - 1, position[2]])
            .ok_or("Decorative support is not loaded")?;
        if !terra_core::beds::supports_bed_floor(support) {
            return Err("A full supporting surface is required".into());
        }
    }
    if !furniture(material) || terra_core::textiles::is_carpet(material) {
        return Ok(result);
    }
    let mut state = BlockObjectState::from_yaw(look[0]);
    if material == terra_core::flower_pot::MATERIAL {
        state.facing = 0;
    }
    if material == 100 {
        let anchor = crank_owner
            .or_else(|| {
                (0..4).find_map(|facing| {
                    let state = BlockObjectState {
                        facing,
                        ..Default::default()
                    };
                    let anchor = offset(position, state.right(), -2);
                    (journal.valid_crusher(anchor)
                        && journal
                            .object_state(anchor)
                            .is_some_and(|s| s.facing == facing))
                    .then_some(anchor)
                })
            })
            .ok_or("Crank needs a crusher drive gear")?;
        let machine = journal
            .object_state(anchor)
            .ok_or("Missing crusher drive")?;
        if journal.crusher_crank_position(anchor) != Some(position)
            || chunks.block_at(anchor) != Some(99)
            || machine.peer.is_none_or(|p| chunks.block_at(p) != Some(99))
        {
            return Err("Crank attachment is not loaded".into());
        }
        state = BlockObjectState {
            peer: Some(anchor),
            secondary: false,
            ..machine
        };
    }
    if terra_core::beds::is_bed(material) {
        state.facing = (state.facing + 2) % 4;
    }
    if matches!(material,28|59|65..=67|71..=73|99|139) {
        let direction = if terra_core::beds::is_bed(material) {
            [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]][usize::from(state.facing % 4)]
        } else if matches!(material, 28 | 99) {
            state.right()
        } else {
            [0, 1, 0]
        };
        let peer = offset(position, direction, 1);
        state.peer = Some(peer);
        result.edits.push(BlockEditRequest {
            position: peer,
            expected_material: 0,
            material,
        });
        result.states.push((
            peer,
            Some(BlockObjectState {
                peer: Some(position),
                secondary: true,
                ..state
            }),
        ));
    } else if material == 29 {
        let mut neighbors = Vec::new();
        for sign in [-1, 1] {
            let peer = offset(position, state.right(), sign);
            if chunks.block_at(peer) == Some(29) {
                let other = journal.object_state(peer).unwrap_or_default();
                if other.facing == state.facing {
                    if other.peer.is_some() {
                        return Err("A double chest cannot merge again".into());
                    }
                    neighbors.push((peer, sign, other));
                }
            }
        }
        if neighbors.len() > 1 {
            return Err("Chest would create a triple merge".into());
        }
        if let Some((peer, sign, other)) = neighbors.pop() {
            state.peer = Some(peer);
            state.secondary = sign < 0;
            result.states.push((
                peer,
                Some(BlockObjectState {
                    peer: Some(position),
                    secondary: sign > 0,
                    ..other
                }),
            ));
        }
    }
    if material == 60 {
        state.facing = chimney(position, look, chunks, journal, &mut result.states)?;
    }
    result.states.push((position, Some(state)));
    Ok(result)
}

fn chimney(
    position: [i64; 3],
    look: [f32; 2],
    chunks: &ChunkStore,
    journal: &EditJournal,
    states: &mut Vec<([i64; 3], Option<BlockObjectState>)>,
) -> Result<u8, String> {
    use crate::objects::{DIRECTIONS, chimney_ports, chimney_pose};
    let connects = |cell: [i64; 3], direction: [i64; 3]| {
        let other = offset(cell, direction, 1);
        let id = chunks.block_at(other)?;
        if id == 59
            && direction == [0, -1, 0]
            && journal.object_state(other).is_some_and(|s| s.secondary)
        {
            return Some(());
        }
        chimney_ports(id, journal.object_state(other).unwrap_or_default().facing)?
            .contains(&direction.map(|v| -v))
            .then_some(())
    };
    let mut incoming: Vec<_> = DIRECTIONS
        .into_iter()
        .filter(|d| connects(position, *d).is_some())
        .collect();
    if incoming.is_empty() {
        for direction in DIRECTIONS {
            let neighbor = offset(position, direction, 1);
            if chunks.block_at(neighbor) != Some(60) {
                continue;
            }
            let old = journal.object_state(neighbor).unwrap_or_default();
            let Some(ports) = chimney_ports(60, old.facing) else {
                continue;
            };
            let linked: Vec<_> = ports
                .into_iter()
                .filter(|d| connects(neighbor, *d).is_some())
                .collect();
            if let [upstream] = linked.as_slice()
                && let Some(pose) = chimney_pose([*upstream, direction.map(|v| -v)])
            {
                states.push((
                    neighbor,
                    Some(BlockObjectState {
                        facing: pose,
                        ..old
                    }),
                ));
                incoming.push(direction);
                break;
            }
        }
    }
    if incoming.len() > 2 {
        return Err("Chimney cannot form a three-way junction".into());
    }
    let ray = Quat::from_euler(bevy_math::EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let preferred = if ray.y.abs() > 0.65 {
        [0, if ray.y > 0.0 { 1 } else { -1 }, 0]
    } else if ray.x.abs() > ray.z.abs() {
        [if ray.x > 0.0 { 1 } else { -1 }, 0, 0]
    } else {
        [0, 0, if ray.z > 0.0 { 1 } else { -1 }]
    };
    let ports = match incoming.as_slice() {
        [a, b] => [*a, *b],
        [a] => [
            *a,
            if preferred == *a {
                a.map(|v| -v)
            } else {
                preferred
            },
        ],
        _ => [preferred.map(|v| -v), preferred],
    };
    let pose = chimney_pose(ports).ok_or("Invalid chimney connection")?;
    Ok(pose
        + if (0.25..0.85).contains(&look[1].abs()) && ports[0] != ports[1].map(|v| -v) {
            32
        } else {
            0
        })
}

pub fn environment(
    plan: &Layout,
    chunks: &ChunkStore,
    journal: &EditJournal,
    bounded: bool,
) -> Result<(), String> {
    for edit in &plan.edits {
        if bounded && !(-128..=356).contains(&edit.position[1]) {
            return Err("Object exceeds world height bounds".into());
        }
        if chunks.block_at(edit.position) != Some(edit.expected_material) {
            return Err("Object cell unavailable".into());
        }
        if edit.material == 0 || terra_core::construction::is_shape(edit.material) {
            continue;
        }
        if edit.material == 99
            && chunks.block_at([edit.position[0], edit.position[1] + 1, edit.position[2]])
                != Some(0)
        {
            return Err("Crusher needs free space above its hoppers".into());
        }
        let below = [edit.position[0], edit.position[1] - 1, edit.position[2]];
        if terra_core::beds::is_bed(edit.material)
            && !chunks.block_at(below).is_some_and(|m| {
                terra_core::beds::supports_bed_floor(m)
                    || terra_core::construction::is_slab(m)
                        && journal.object_state(below).is_some_and(|s| {
                            s.facing == 4 || terra_core::construction::full_slab(s.facing)
                        })
            })
        {
            return Err("Bed requires a full-height supporting floor".into());
        }
        let planned = plan
            .edits
            .iter()
            .any(|other| other.position == below && matches!(other.material, 59 | 65..=67 | 139));
        let wall = crate::objects::is_window(edit.material)
            && crate::objects::DIRECTIONS.iter().any(|d| {
                chunks
                    .block_at(offset(edit.position, *d, 1))
                    .and_then(terra_voxel::MaterialId::from_raw)
                    .is_some_and(terra_voxel::MaterialId::is_solid)
            });
        if furniture(edit.material)
            && !matches!(edit.material, 60..=64 | 100)
            && !planned
            && !wall
            && !chunks
                .block_at(below)
                .and_then(terra_voxel::MaterialId::from_raw)
                .is_some_and(terra_voxel::MaterialId::is_solid)
        {
            return Err("Object requires solid floor".into());
        }
    }
    Ok(())
}

pub fn boxes(
    plan: &Layout,
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
) -> Vec<ShapeBox> {
    let mut result = Vec::new();
    let positions: std::collections::BTreeSet<_> = plan
        .edits
        .iter()
        .map(|e| e.position)
        .chain(plan.states.iter().map(|(p, _)| *p))
        .collect();
    for position in positions {
        let material = plan
            .edits
            .iter()
            .find(|e| e.position == position)
            .map(|e| e.material)
            .or_else(|| chunks.block_at(position))
            .unwrap_or(0);
        if material == 0 {
            continue;
        }
        let state = plan
            .states
            .iter()
            .find(|(p, _)| *p == position)
            .and_then(|(_, s)| *s)
            .unwrap_or_default();
        if state.secondary {
            continue;
        }
        if !crate::interaction_shapes::visit_proposed_boxes(
            chunks,
            journal,
            properties,
            position,
            material,
            state,
            |b| result.push(b),
        ) {
            result.push(ShapeBox {
                minimum: position.map(|v| v as f64),
                maximum: position.map(|v| v as f64 + 1.0),
                hinged: false,
            });
        }
    }
    result
}

/// A complete proposed object, before any inventory mutation or world edit.
pub fn propose(
    chunks: &ChunkStore,
    journal: &EditJournal,
    hit: terra_voxel::blocks::BlockHit,
    item: terra_core::ItemKind,
    ray: ([f64; 3], [f64; 3]),
    look: [f32; 2],
) -> Result<Layout, String> {
    let material = item.block_material().ok_or("Item cannot be placed")?;
    let (position, expected) =
        crate::targeting::item_placement_with_state(chunks, journal, hit, material)
            .ok_or("No placement cell")?;
    let crank = (material == 100)
        .then(|| journal.crusher_primary(hit.position))
        .flatten();
    let mut plan = layout(chunks, journal, position, expected, material, look, crank)?;
    if terra_core::construction::is_shape(material) {
        let facing = if terra_core::construction::is_slab(expected) {
            terra_core::construction::merged_slab(
                expected,
                journal.object_state(position).unwrap_or_default().facing,
                material,
            )
            .ok_or("Slabs cannot merge")?
        } else {
            crate::targeting::placement_facing(chunks, journal, hit, material, ray)
        };
        if terra_core::construction::full_slab(facing)
            && terra_core::construction::is_slab(expected)
        {
            plan.edits[0].material = expected;
        }
        plan.states.retain(|(p, _)| *p != position);
        plan.states.push((
            position,
            Some(BlockObjectState {
                facing,
                ..Default::default()
            }),
        ));
    } else if material == 40 {
        let facing = match hit.normal {
            [1, 0, 0] => 4,
            [-1, 0, 0] => 5,
            [0, 0, 1] => 6,
            [0, 0, -1] => 7,
            [0, 1, 0] => 0,
            _ => return Err("Torch needs floor or wall".into()),
        };
        plan.states.push((
            position,
            Some(BlockObjectState {
                facing,
                ..Default::default()
            }),
        ));
    }
    for (p, state) in &mut plan.states {
        if let Some(state) = state {
            if let terra_core::ItemKind::WoodenBlock { wood, .. } = item
                && (*p == position || material == 28)
            {
                state.wood = wood;
            }
            if let terra_core::ItemKind::DyedBed { color, .. } = item {
                state.facing = terra_core::beds::dyed_facing(state.facing, color);
            }
        }
    }
    Ok(plan)
}

pub fn resolve(
    chunks: &ChunkStore,
    journal: &EditJournal,
    hit: terra_voxel::blocks::BlockHit,
    item: terra_core::ItemKind,
    ray: ([f64; 3], [f64; 3]),
    look: [f32; 2],
    bounded: bool,
) -> Result<Layout, String> {
    let plan = propose(chunks, journal, hit, item, ray, look)?;
    environment(&plan, chunks, journal, bounded)?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::LoadedSection;
    use terra_voxel::{BlockSection, SectionPos};
    fn fixture() -> (ChunkStore, EditJournal) {
        let mut data = BlockSection::empty(SectionPos::default()).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 1, z], 3).unwrap();
            }
        }
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            SectionPos::default(),
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        (chunks, EditJournal::default())
    }
    #[test]
    fn shared_furniture_layout_keeps_all_four_facings_and_real_multicell_axes() {
        let (chunks, journal) = fixture();
        for material in [28, 29, 59, 65, 66, 67, 68, 69, 70, 71, 72, 73, 99] {
            for facing in 0..4 {
                let yaw = facing as f32 * std::f32::consts::FRAC_PI_2;
                let plan =
                    layout(&chunks, &journal, [6, 2, 6], 0, material, [yaw, 0.0], None).unwrap();
                environment(&plan, &chunks, &journal, true).unwrap();
                let state = plan
                    .states
                    .iter()
                    .find(|(p, _)| *p == [6, 2, 6])
                    .unwrap()
                    .1
                    .unwrap();
                assert_eq!(
                    state.facing,
                    if terra_core::beds::is_bed(material) {
                        (facing + 2) % 4
                    } else {
                        facing
                    }
                );
                if let Some(peer) = state.peer {
                    assert_eq!(plan.edits.len(), 2);
                    assert_eq!(
                        peer[1],
                        if matches!(material, 59 | 65..=67 | 139) {
                            3
                        } else {
                            2
                        }
                    );
                    let secondary = plan
                        .states
                        .iter()
                        .find(|(p, _)| *p == peer)
                        .unwrap()
                        .1
                        .unwrap();
                    assert!(secondary.secondary && secondary.peer == Some([6, 2, 6]));
                } else {
                    assert_eq!(plan.edits.len(), 1);
                }
            }
        }
    }
    #[test]
    fn invalid_station_keeps_its_exact_proposed_footprint_without_mutating_the_world() {
        let (mut chunks, journal) = fixture();
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([7, 2, 6], 3)
            .unwrap();
        let plan = layout(&chunks, &journal, [6, 2, 6], 0, 28, [0.0; 2], None).unwrap();
        assert_eq!(plan.edits[1].position, [7, 2, 6]);
        assert!(environment(&plan, &chunks, &journal, true).is_err());
        assert_eq!(chunks.block_at([6, 2, 6]), Some(0));
    }
    #[test]
    fn combined_chest_preview_includes_the_existing_primary_and_preserves_both_woods() {
        let (mut chunks, mut journal) = fixture();
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([6, 2, 6], 29)
            .unwrap();
        journal.record([6, 2, 6], 29).unwrap();
        journal
            .set_object_state(
                [6, 2, 6],
                Some(BlockObjectState {
                    wood: terra_core::WoodSpecies::Birch,
                    ..Default::default()
                }),
            )
            .unwrap();
        let plan = layout(&chunks, &journal, [7, 2, 6], 0, 29, [0.0; 2], None).unwrap();
        assert!(plan.states.iter().any(|(p, s)| *p == [6, 2, 6]
            && s.is_some_and(|s| !s.secondary
                && s.peer == Some([7, 2, 6])
                && s.wood == terra_core::WoodSpecies::Birch)));
        assert!(
            plan.states.iter().any(|(p, s)| *p == [7, 2, 6]
                && s.is_some_and(|s| s.secondary && s.peer == Some([6, 2, 6])))
        );
        assert!(!boxes(&plan, &chunks, &journal, &BlockPropertiesCatalog::default()).is_empty());
    }

    #[test]
    fn proposed_collision_matches_committed_objects_for_facings_mounts_and_mixed_slabs() {
        let properties = BlockPropertiesCatalog::default();
        for material in [
            28, 29, 40, 59, 60, 65, 68, 71, 74, 75, 76, 80, 81, 82, 99, 104, 105,
        ] {
            let facings: Vec<_> = if terra_core::construction::is_slab(material) {
                (0..=terra_core::construction::LAST_MIXED_SLAB)
                    .filter(|&f| terra_core::construction::valid_facing(material, f))
                    .collect()
            } else if terra_core::construction::is_stair(material) {
                (0..24).collect()
            } else if material == 60 {
                (0..64)
                    .filter(|&f| crate::objects::chimney_basis(f).is_some())
                    .collect()
            } else if material == 40 {
                vec![0, 4, 5, 6, 7]
            } else {
                (0..4).collect()
            };
            for facing in facings {
                let (mut chunks, mut journal) = fixture();
                let yaw = if terra_core::beds::is_bed(material) {
                    (facing + 2) % 4
                } else {
                    facing % 4
                } as f32
                    * std::f32::consts::FRAC_PI_2;
                let mut plan =
                    layout(&chunks, &journal, [6, 2, 6], 0, material, [yaw, 0.0], None).unwrap();
                if plan.states.is_empty() {
                    plan.states
                        .push(([6, 2, 6], Some(BlockObjectState::default())));
                }
                for (_, state) in &mut plan.states {
                    state.as_mut().unwrap().facing = facing;
                }
                let expected = boxes(&plan, &chunks, &journal, &properties);
                assert!(!expected.is_empty(), "{material}/{facing}");
                for edit in &plan.edits {
                    let (section, local) = terra_voxel::global_to_section(edit.position);
                    chunks
                        .loaded
                        .get_mut(&section)
                        .unwrap()
                        .data
                        .set_block(local, edit.material)
                        .unwrap();
                    journal.record(edit.position, edit.material).unwrap();
                }
                for (position, state) in &plan.states {
                    journal.set_object_state(*position, *state).unwrap();
                }
                let mut actual = Vec::new();
                let positions: std::collections::BTreeSet<_> = plan
                    .edits
                    .iter()
                    .map(|e| e.position)
                    .chain(plan.states.iter().map(|(p, _)| *p))
                    .collect();
                for position in positions {
                    if journal.object_state(position).is_some_and(|s| s.secondary) {
                        continue;
                    }
                    crate::interaction_shapes::visit_boxes(
                        &chunks,
                        &journal,
                        &properties,
                        position,
                        |b| actual.push(b),
                    );
                }
                assert_eq!(expected, actual, "{material}/{facing}");
            }
        }
    }

    #[test]
    fn carpets_and_pots_require_a_real_full_support_and_pots_start_empty_independent_of_yaw() {
        let (chunks, journal) = fixture();
        for material in [106, 118, 131] {
            for yaw in [0.0, 1.5, 3.0, -1.5] {
                let plan =
                    layout(&chunks, &journal, [6, 2, 6], 0, material, [yaw, 0.0], None).unwrap();
                if material == 131 {
                    assert_eq!(plan.states[0].1.unwrap().facing, 0);
                } else {
                    assert!(plan.states.is_empty());
                }
            }
            assert!(layout(&chunks, &journal, [6, 3, 6], 0, material, [0.0; 2], None).is_err());
        }
    }
}
