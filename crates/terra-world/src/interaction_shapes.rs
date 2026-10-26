//! Shared sub-cell furniture geometry for picking, collision and selection.
use crate::{edits::EditJournal, streaming::ChunkStore};
use terra_core::{block_properties::BlockPropertiesCatalog, block_shapes::ShapeBox};
use terra_voxel::blocks::BlockHit;

pub fn visit_boxes(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    position: [i64; 3],
    visit: impl FnMut(ShapeBox),
) -> bool {
    visit_boxes_with(journal, properties, position, |p| chunks.block_at(p), visit)
}

/// Same authored collision shapes against a prepared or loaded voxel lookup.
pub fn visit_boxes_with(
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    position: [i64; 3],
    mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
    visit: impl FnMut(ShapeBox),
) -> bool {
    visit_boxes_with_state(journal, properties, position, &mut block_at, None, visit)
}

/// Test a proposed block with the same authored shape and orientation as collision.
pub fn visit_placement_boxes(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    position: [i64; 3],
    material: u16,
    facing: u8,
    visit: impl FnMut(ShapeBox),
) -> bool {
    visit_proposed_boxes(
        chunks,
        journal,
        properties,
        position,
        material,
        crate::edits::BlockObjectState {
            facing,
            ..Default::default()
        },
        visit,
    )
}

pub fn visit_proposed_boxes(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    position: [i64; 3],
    material: u16,
    state: crate::edits::BlockObjectState,
    visit: impl FnMut(ShapeBox),
) -> bool {
    visit_boxes_with_state(
        journal,
        properties,
        position,
        |cell| {
            if cell == position {
                Some(material)
            } else {
                chunks.block_at(cell)
            }
        },
        Some(state),
        visit,
    )
}

fn visit_boxes_with_state(
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    position: [i64; 3],
    mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
    override_state: Option<crate::edits::BlockObjectState>,
    mut visit: impl FnMut(ShapeBox),
) -> bool {
    let Some(material) = block_at(position) else {
        return false;
    };
    if crate::segments::is_crystal(material) {
        let profile = crate::segments::profile(position, material, &mut block_at);
        for i in 0..4 {
            let lo = i as f32 * 0.25;
            let hi = lo + 0.25;
            let r = f64::from(
                crate::segments::radius(profile, lo).max(crate::segments::radius(profile, hi)),
            );
            visit(ShapeBox {
                hinged: false,
                minimum: [
                    position[0] as f64 + 0.5 - r,
                    position[1] as f64 + f64::from(lo),
                    position[2] as f64 + 0.5 - r,
                ],
                maximum: [
                    position[0] as f64 + 0.5 + r,
                    position[1] as f64 + f64::from(hi),
                    position[2] as f64 + 0.5 + r,
                ],
            });
        }
        return true;
    }
    let selected =
        override_state.unwrap_or_else(|| journal.object_state(position).unwrap_or_default());
    if crate::objects::is_window(material) {
        let group = crate::objects::windows::group(position, material, selected.facing, |p| {
            let id = block_at(p).or_else(|| journal.block(p))?;
            let facing = if p == position {
                selected.facing
            } else {
                journal.object_state(p)?.facing
            };
            Some((id, facing))
        });
        if group.width > 1 || group.height > 1 {
            for piece in crate::objects::windows::pieces(group.width, group.height)
                .into_iter()
                .filter(|p| p.solid)
            {
                visit(crate::objects::windows::world_box(
                    piece,
                    group,
                    f64::from(u8::from(group.facing >= 4)),
                ));
            }
            return true;
        }
    }
    let owner = if selected.secondary {
        selected.peer.unwrap_or(position)
    } else {
        position
    };
    let state = if owner == position {
        selected
    } else {
        journal.object_state(owner).unwrap_or_default()
    };
    if material == terra_core::crusher::CRANK_MATERIAL {
        let boxes = properties.shape(material);
        if boxes.is_empty() {
            return false;
        }
        for source in boxes {
            let mut bounds = source.rotated_y(state.facing);
            for (axis, value) in owner.iter().enumerate() {
                bounds.minimum[axis] += *value as f64;
                bounds.maximum[axis] += *value as f64;
            }
            visit(bounds);
        }
        return true;
    }
    if material == 98 {
        let mut bounds = terra_core::construction::snow_bounds(state.facing);
        for (axis, value) in position.iter().enumerate() {
            bounds.minimum[axis] += *value as f64;
            bounds.maximum[axis] += *value as f64;
        }
        visit(bounds);
        return true;
    }
    if terra_core::construction::is_slab(material) || terra_core::construction::is_stair(material) {
        let neighbors = std::array::from_fn(|side| {
            let mut p = position;
            p[side / 2] += if side % 2 == 0 { -1 } else { 1 };
            block_at(p).map(|id| (id, journal.object_state(p).unwrap_or_default().facing))
        });
        for mut bounds in terra_core::construction::placed_boxes(material, state.facing, neighbors)
        {
            for (axis, value) in position.iter().enumerate() {
                bounds.minimum[axis] += *value as f64;
                bounds.maximum[axis] += *value as f64;
            }
            visit(bounds);
        }
        return true;
    }
    let template = if material == 60 {
        crate::objects::chimney_basis(state.facing).map_or(material, |(id, _)| id)
    } else {
        material
    };
    let boxes = properties.shape(template);
    if boxes.is_empty() {
        return false;
    }
    let width = if matches!(material, 28 | 29 | 99) && state.peer.is_some() {
        2.0
    } else {
        1.0
    };
    let (infill_owner, infill_material, infill_facing) = if material == 59 {
        ([owner[0], owner[1] + 1, owner[2]], 60, 0)
    } else {
        (owner, material, state.facing)
    };
    for (mut bounds, _) in
        crate::objects::chimney_infill_with_states(infill_material, infill_facing, |direction| {
            let position = std::array::from_fn(|axis| infill_owner[axis] + direction[axis]);
            block_at(position).map(|id| {
                (
                    id,
                    journal.object_state(position).unwrap_or_default().facing,
                )
            })
        })
    {
        for (axis, value) in infill_owner.iter().enumerate() {
            bounds.minimum[axis] += *value as f64;
            bounds.maximum[axis] += *value as f64;
        }
        visit(bounds);
    }
    for (box_index, source) in boxes.iter().enumerate() {
        if terra_core::construction::is_fence(material)
            && let Some(arm) = terra_core::construction::fence_arm(box_index)
        {
            let side = terra_core::construction::fence_neighbor_side(arm, state.facing);
            let mut neighbor = owner;
            neighbor[side / 2] += if side.is_multiple_of(2) { -1 } else { 1 };
            if !terra_core::construction::connects(block_at(neighbor)) {
                continue;
            }
        }
        if crate::objects::is_opening(material) {
            visit(crate::objects::opening_box(*source, owner, state));
            continue;
        }
        let mut result = ShapeBox {
            minimum: [f64::INFINITY; 3],
            maximum: [f64::NEG_INFINITY; 3],
            hinged: false,
        };
        for corner in 0..8 {
            let mut point: [f64; 3] = std::array::from_fn(|axis| {
                if corner & (1 << axis) == 0 {
                    source.minimum[axis]
                } else {
                    source.maximum[axis]
                }
            });
            if material == 40 || material == 60 && state.facing >= 8 {
                point = if material == 40 {
                    terra_core::block_shapes::torch_point(state.facing, point)
                } else {
                    crate::objects::chimney_point(state.facing, point)
                };
                for axis in 0..3 {
                    let value = point[axis] + owner[axis] as f64;
                    result.minimum[axis] = result.minimum[axis].min(value);
                    result.maximum[axis] = result.maximum[axis].max(value);
                }
                continue;
            }
            point[0] = point[0] * width - 0.5;
            point[2] -= 0.5;
            let [x, y, z] = point;
            point = match state.facing % 4 {
                0 => [x, y, z],
                1 => [z, y, -x],
                2 => [-x, y, -z],
                _ => [-z, y, x],
            };
            for axis in 0..3 {
                let value = point[axis] + owner[axis] as f64 + if axis == 1 { 0.0 } else { 0.5 };
                result.minimum[axis] = result.minimum[axis].min(value);
                result.maximum[axis] = result.maximum[axis].max(value);
            }
        }
        visit(result);
    }
    true
}

pub fn raycast(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
) -> Option<BlockHit> {
    cast(
        chunks,
        journal,
        properties,
        origin,
        direction,
        reach,
        (false, crate::props::PropPose::Rest),
    )
}

pub fn raycast_with_pose(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
    pose: crate::props::PropPose,
) -> Option<BlockHit> {
    cast(
        chunks,
        journal,
        properties,
        origin,
        direction,
        reach,
        (false, pose),
    )
}

pub fn raycast_solid(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
) -> Option<BlockHit> {
    cast(
        chunks,
        journal,
        properties,
        origin,
        direction,
        reach,
        (true, crate::props::PropPose::Rest),
    )
}

fn cast(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
    options: (bool, crate::props::PropPose),
) -> Option<BlockHit> {
    let (solid_only, pose) = options;
    let length = direction.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !length.is_finite() || length < 1.0e-12 {
        return None;
    }
    let direction = direction.map(|v| v / length);
    let mut ores = std::collections::BTreeSet::new();
    let mut segments = std::collections::BTreeSet::new();
    let mut openings = std::collections::BTreeSet::new();
    let mut nearest = terra_voxel::blocks::raycast_blocks_with(
        origin,
        direction,
        reach,
        |position| {
            let material = chunks.block_at(position).unwrap_or(1);
            let below = [position[0], position[1] - 1, position[2]];
            if chunks.block_at(below) == Some(99) {
                openings.insert(below);
            }
            // Hinged leaves can extend a fraction of a cell beyond their anchor.
            for dx in -1..=1 {
                for dz in -1..=1 {
                    let neighbor = [position[0] + dx, position[1], position[2] + dz];
                    if chunks
                        .block_at(neighbor)
                        .is_some_and(crate::objects::is_opening)
                    {
                        openings.insert(neighbor);
                    }
                }
            }
            if crate::segments::is_segment(material) {
                segments.insert(position);
            }
            if !solid_only {
                for axis in [0, 2] {
                    for delta in [-1, 1] {
                        let mut neighbor = position;
                        neighbor[axis] += delta;
                        if chunks
                            .block_at(neighbor)
                            .is_some_and(crate::segments::is_segment)
                        {
                            segments.insert(neighbor);
                        }
                    }
                }
            }
            if !solid_only {
                if matches!(material, 30..=35 | 37..=39) {
                    ores.insert(position);
                }
                for axis in 0..3 {
                    for offset in [-1, 1] {
                        let mut neighbor = position;
                        let Some(value) = position[axis].checked_add(offset) else {
                            continue;
                        };
                        neighbor[axis] = value;
                        if chunks
                            .block_at(neighbor)
                            .is_some_and(|material| matches!(material, 30..=35 | 37..=39))
                        {
                            ores.insert(neighbor);
                        }
                    }
                }
            }
            if !solid_only && !chunks.cave_structures_enabled() && matches!(material, 0 | 27) {
                for (direction, limit) in [(-1, 4), (1, crate::ore_shapes::MINERAL_REACH)] {
                    for step in 1..=limit {
                        let Some(y) = position[1].checked_add(step * direction) else {
                            break;
                        };
                        let neighbor = [position[0], y, position[2]];
                        match chunks.block_at(neighbor) {
                            Some(30 | 37) => {
                                ores.insert(neighbor);
                                break;
                            }
                            Some(0 | 27) => {}
                            _ => break,
                        }
                    }
                }
            }
            Some(
                if solid_only
                    && terra_voxel::MaterialId::from_raw(material)
                        .is_some_and(|material| !material.is_solid())
                {
                    0
                } else {
                    material
                },
            )
        },
        |cell_hit, material| {
            if crate::segments::is_segment(material) {
                return crate::segments::raycast(
                    chunks,
                    cell_hit.position,
                    material,
                    origin,
                    direction,
                    reach,
                    pose,
                );
            }
            let mut nearest: Option<BlockHit> = None;
            let model = visit_boxes(chunks, journal, properties, cell_hit.position, |bounds| {
                if let Some((distance, normal)) = bounds.ray(origin, direction, reach)
                    && nearest.is_none_or(|previous| distance < previous.distance)
                {
                    let state = journal.object_state(cell_hit.position).unwrap_or_default();
                    let owner = if state.secondary {
                        state.peer.unwrap_or(cell_hit.position)
                    } else {
                        cell_hit.position
                    };
                    let primary = journal.object_state(owner).unwrap_or_default();
                    let point: [f64; 3] =
                        std::array::from_fn(|axis| origin[axis] + direction[axis] * distance);
                    let squared = |position: [i64; 3]| {
                        (0..3)
                            .map(|axis| (point[axis] - position[axis] as f64 - 0.5).powi(2))
                            .sum::<f64>()
                    };
                    let mut position = primary
                        .peer
                        .filter(|peer| squared(*peer) < squared(owner))
                        .unwrap_or(owner);
                    if crate::objects::is_window(material) {
                        position =
                            window_hit_cell(chunks, journal, cell_hit.position, material, point);
                    }
                    nearest = Some(BlockHit {
                        position,
                        normal,
                        distance,
                    });
                }
            });
            if model { nearest } else { Some(cell_hit) }
        },
    );
    for position in openings {
        visit_boxes(chunks, journal, properties, position, |bounds| {
            if let Some((distance, normal)) =
                bounds.ray(origin, direction, nearest.map_or(reach, |h| h.distance))
                && nearest.is_none_or(|hit| distance < hit.distance)
            {
                nearest = Some(BlockHit {
                    position: if chunks
                        .block_at(position)
                        .is_some_and(crate::objects::is_window)
                    {
                        window_hit_cell(
                            chunks,
                            journal,
                            position,
                            chunks.block_at(position).unwrap(),
                            std::array::from_fn(|a| origin[a] + direction[a] * distance),
                        )
                    } else {
                        position
                    },
                    normal,
                    distance,
                });
            }
        });
    }
    for position in segments {
        let Some(material) = chunks.block_at(position) else {
            continue;
        };
        if solid_only
            && !terra_voxel::MaterialId::from_raw(material)
                .is_some_and(terra_voxel::MaterialId::is_solid)
        {
            continue;
        }
        if let Some(hit) = crate::segments::raycast(
            chunks,
            position,
            material,
            origin,
            direction,
            nearest.map_or(reach, |h| h.distance),
            pose,
        ) {
            nearest = Some(hit);
        }
    }
    for position in ores {
        let Some(material) = chunks.block_at(position) else {
            continue;
        };
        let local_origin = std::array::from_fn(|axis| origin[axis] - position[axis] as f64);
        for side in 0..6 {
            let mut neighbor = position;
            neighbor[side / 2] += if side % 2 == 0 { -1 } else { 1 };
            if !matches!(chunks.block_at(neighbor), Some(0 | 27)) {
                continue;
            }
            let growth = if matches!(material, 30 | 37) && !chunks.cave_structures_enabled() {
                crate::ore_shapes::mineral_growth_height(material, position, side, |p| {
                    chunks.block_at(p)
                })
            } else {
                0.0
            };
            crate::ore_shapes::visit_shaded_face_with_growth(
                material,
                position,
                side,
                crate::ore_shapes::OreDetail::Near,
                growth,
                |triangle, _| {
                    let limit = nearest.map_or(reach, |hit| hit.distance.min(reach));
                    if let Some(distance) =
                        crate::ore_shapes::ray_triangle(triangle, local_origin, direction, limit)
                    {
                        let mut normal = [0; 3];
                        normal[side / 2] = if side % 2 == 0 { -1 } else { 1 };
                        nearest = Some(BlockHit {
                            position,
                            normal,
                            distance,
                        });
                    }
                },
            );
        }
    }
    nearest
}

fn window_hit_cell(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: [i64; 3],
    material: u16,
    point: [f64; 3],
) -> [i64; 3] {
    let facing = journal.object_state(position).unwrap_or_default().facing;
    crate::objects::windows::group(position, material, facing, |p| {
        Some((
            chunks.block_at(p).or_else(|| journal.block(p))?,
            journal.object_state(p)?.facing,
        ))
    })
    .nearest_cell(point)
}

pub fn space_intersects(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    position: [i64; 3],
    minimum: [f64; 3],
    maximum: [f64; 3],
) -> bool {
    let mut hit = false;
    if visit_boxes(chunks, journal, properties, position, |bounds| {
        hit |= bounds.intersects(minimum, maximum)
    }) {
        hit
    } else {
        chunks
            .block_at(position)
            .and_then(terra_voxel::MaterialId::from_raw)
            .is_none_or(terra_voxel::MaterialId::is_solid)
    }
}

#[cfg(test)]
mod connected_windows;
#[cfg(test)]
mod tests;
