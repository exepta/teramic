use super::{FluidSnapshot, WaterPatch};
use crate::SurfaceColumn;
use std::collections::HashMap;
use terra_core::block_shapes::ShapeBox;
use terra_voxel::SectionPos;

mod occlusion;
mod partial;
mod pipes;
pub(crate) mod rails;
mod tongue;

#[cfg(test)]
mod construction_tests;

/// Collision boxes stay authoritative for construction. Pipes additionally
/// retain their round profile so water never inherits a square collision cutout.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct FluidOccluders {
    boxes: HashMap<[i64; 3], Vec<ShapeBox>>,
    pub dry_regions: HashMap<[i64; 3], Vec<ShapeBox>>,
    pub rails: HashMap<[i64; 3], std::sync::Arc<[rails::Profile]>>,
    pub pipes: HashMap<[i64; 3], u8>,
    pub fixtures: HashMap<[i64; 3], (u16, u8)>,
}
impl FluidOccluders {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn profiled(&self, p: &[i64; 3]) -> bool {
        self.pipes.contains_key(p) || self.fixtures.contains_key(p) || self.rails.contains_key(p)
    }
    fn space(&self, p: [i64; 3]) -> super::space::FluidSpace {
        let mut space =
            super::space::FluidSpace::from_global(self.get(&p).map_or(&[], Vec::as_slice), p);
        if let Some(regions) = self.dry_regions.get(&p) {
            space
                .solids
                .extend(super::space::FluidSpace::from_global(regions, p).solids);
        }
        space
    }
}
impl std::ops::Deref for FluidOccluders {
    type Target = HashMap<[i64; 3], Vec<ShapeBox>>;
    fn deref(&self) -> &Self::Target {
        &self.boxes
    }
}
impl std::ops::DerefMut for FluidOccluders {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.boxes
    }
}
impl<const N: usize> From<[([i64; 3], Vec<ShapeBox>); N]> for FluidOccluders {
    fn from(boxes: [([i64; 3], Vec<ShapeBox>); N]) -> Self {
        Self {
            boxes: HashMap::from(boxes),
            ..Self::default()
        }
    }
}

pub(crate) struct FluidMeshOptions<'a> {
    river_surface: bool,
    occluders: Option<&'a FluidOccluders>,
    precise_levels: Option<&'a HashMap<[i64; 3], f64>>,
}

impl From<bool> for FluidMeshOptions<'_> {
    fn from(river_surface: bool) -> Self {
        Self {
            river_surface,
            occluders: None,
            precise_levels: None,
        }
    }
}

impl<'a> From<(bool, &'a FluidOccluders)> for FluidMeshOptions<'a> {
    fn from((river_surface, occluders): (bool, &'a FluidOccluders)) -> Self {
        Self {
            river_surface,
            occluders: Some(occluders),
            precise_levels: None,
        }
    }
}

impl<'a> From<(bool, &'a FluidOccluders, &'a HashMap<[i64; 3], f64>)> for FluidMeshOptions<'a> {
    fn from(
        (river_surface, occluders, precise_levels): (
            bool,
            &'a FluidOccluders,
            &'a HashMap<[i64; 3], f64>,
        ),
    ) -> Self {
        Self {
            river_surface,
            occluders: Some(occluders),
            precise_levels: Some(precise_levels),
        }
    }
}

fn filled_covered_pocket(space: &super::space::FluidSpace, height: f64) -> bool {
    if height >= 1. - 1e-8 || space.solids.is_empty() {
        return false;
    }
    let volume = |height| {
        space
            .wet_boxes(height)
            .iter()
            .map(|b| {
                (0..3)
                    .map(|axis| b.maximum[axis] - b.minimum[axis])
                    .product::<f64>()
            })
            .sum::<f64>()
    };
    (volume(1.) - volume(height)).abs() < 1e-8
}

pub(crate) fn append_fluid_patch<'a>(
    patch: &mut Option<WaterPatch>,
    position: SectionPos,
    columns: &[SurfaceColumn],
    padded: &[u16],
    fluids: &FluidSnapshot,
    options: impl Into<FluidMeshOptions<'a>>,
) {
    let options = options.into();
    let river_surface = options.river_surface;
    if fluids.is_empty() {
        return;
    }
    let Ok(origin) = position.origin() else {
        return;
    };
    let volume_at = |p: [i64; 3]| -> Option<(super::FluidCell, f64, super::space::FluidSpace)> {
        let local = std::array::from_fn::<_, 3, _>(|i| p[i] - origin[i] + 1);
        if !local.iter().all(|v| (0..18).contains(v)) {
            return None;
        }
        let [x, y, z] = local.map(|v| v as usize);
        let material = padded[x + 18 * (y + 18 * z)];
        if !super::can_hold_fluid(material) {
            return None;
        }
        let (cell, level) = if let Some(cell) = fluids.get(&p) {
            (
                *cell,
                options
                    .precise_levels
                    .and_then(|levels| levels.get(&p).copied())
                    .unwrap_or(p[1] as f64 + cell.height()),
            )
        } else {
            let water = columns[x + 18 * z].water_columns().at(p[1])?;
            (
                super::fluid::from_column(Some(water), p[1]),
                water.level.min(p[1] as f64 + 1.0),
            )
        };
        if cell.amount == 0 {
            return None;
        }
        let space = if super::space::partial(material)
            || options
                .occluders
                .is_some_and(|boxes| boxes.contains_key(&p))
        {
            options.occluders?.get(&p)?;
            // A ceiling can cover the water without lowering the surrounding
            // reservoir's head. Keep the worker's precise natural level; the
            // visible complement below the ceiling is clipped separately.
            let masks = options.occluders?;
            let space = if masks.rails.contains_key(&p) {
                super::space::FluidSpace { solids: vec![] }
            } else {
                masks.space(p)
            };
            if space.wet_boxes(level - p[1] as f64).is_empty() {
                return None;
            }
            space
        } else {
            super::space::FluidSpace { solids: vec![] }
        };
        Some((cell, level.min(p[1] as f64 + 1.), space))
    };
    let sample = |p: [i64; 3]| -> Option<f64> {
        let (cell, mut level, space) = volume_at(p)?;
        let height = level - p[1] as f64;
        if filled_covered_pocket(&space, height) {
            // A completely wetted pocket under a solid ceiling has no local
            // free surface. Its clamped storage height must not pull down a
            // connected reservoir. Raising only this sampling head adds zero
            // wet volume: any still-dry upper pocket fails the volume check.
            for direction in super::fluid::NEIGHBORS {
                let Some(q) = super::fluid::adjacent(p, direction) else {
                    continue;
                };
                if let Some((neighbor, head, other)) = volume_at(q)
                    && head > level
                    && space.connected(cell, &other, neighbor, direction)
                {
                    level = head.min(p[1] as f64 + 1.);
                }
            }
        }
        Some(level)
    };
    let connected_above = |p: [i64; 3]| {
        let q = [p[0], p[1] + 1, p[2]];
        let Some(level) = sample(q) else { return false };
        let Some(occluders) = options.occluders.filter(|o| o.contains_key(&q)) else {
            return true;
        };
        occluders
            .space(q)
            .wet_boxes(level - q[1] as f64)
            .iter()
            .any(|b| b.minimum[1] < 1e-8)
    };
    let canonical_level = |p: [i64; 3], cell: super::FluidCell| -> Option<f64> {
        // The complete worker snapshot also keeps explicit full sources at the
        // natural plane in its original (upper) section, avoiding a two-tile
        // ownership migration while independent mesh jobs finish.
        let levels = options.precise_levels?;
        if let Some(&level) = levels.get(&p) {
            return Some(level);
        }
        let x = usize::try_from(p[0] - origin[0] + 1).ok()?;
        let z = usize::try_from(p[2] - origin[2] + 1).ok()?;
        if x >= 18 || z >= 18 {
            return None;
        }
        let natural = columns[x + 18 * z].water_columns().at(p[1])?;
        let level = p[1] as f64 + cell.height();
        ((natural.level - level).abs() < 1e-8).then_some(level)
    };
    let mut cells: Vec<_> = fluids
        .iter()
        .filter(|(p, cell)| {
            let own_column = [0, 2]
                .into_iter()
                .all(|i| p[i] >= origin[i] && p[i] < origin[i] + 16);
            let own_volume = p[1] >= origin[1] && p[1] < origin[1] + 16;
            let own_natural_top = p[1] == origin[1] - 1
                && canonical_level(**p, **cell)
                    .is_some_and(|level| (level - origin[1] as f64).abs() < 1e-8);
            cell.amount > 0 && own_column && (own_volume || own_natural_top)
        })
        .collect();
    cells.sort_unstable_by_key(|(p, _)| **p);
    for (&p, &cell) in cells {
        let Some(height) = sample(p) else { continue };
        let natural_surface = canonical_level(p, cell).is_some();
        if p[1] < origin[1] {
            // The natural grid assigns integral boundary planes to the upper
            // section. Its source volume can live in an unloaded section below;
            // emit only the clipped top here, without sampling a second halo.
            if let Some(occluders) = options.occluders {
                partial::append(
                    patch,
                    p,
                    cell,
                    origin,
                    occluders,
                    [0.0; 4],
                    natural_surface,
                    sample,
                );
            }
            continue;
        }
        if options.occluders.is_some_and(|boxes| {
            boxes.contains_key(&p)
                || boxes.contains_key(&[p[0], p[1] + 1, p[2]])
                || super::fluid::NEIGHBORS.into_iter().any(|direction| {
                    super::fluid::adjacent(p, direction).is_some_and(|q| boxes.profiled(&q))
                })
        }) {
            let below = [p[0], p[1] - 1, p[2]];
            let local: [usize; 3] = std::array::from_fn(|i| (below[i] - origin[i] + 1) as usize);
            let open_below =
                super::can_hold_fluid(padded[local[0] + 18 * (local[1] + 18 * local[2])]);
            let open_below = open_below && {
                let shapes = options.occluders.unwrap();
                let current = shapes.space(p);
                let floor = shapes.space(below);
                current.connected(
                    cell,
                    &floor,
                    super::FluidCell {
                        amount: 8,
                        ..Default::default()
                    },
                    [0, -1, 0],
                )
            };
            let column =
                columns[(p[0] - origin[0] + 1) as usize + 18 * (p[2] - origin[2] + 1) as usize];
            let reservoir_impact = column
                .water_columns()
                .at(p[1] - 1)
                .is_some_and(|w| (w.level - p[1] as f64).abs() < 0.01);
            let covered = connected_above(p);
            let effects = if covered || sample(below).is_some() || open_below {
                [
                    if covered { 8.0 } else { 0.0 },
                    if open_below && !reservoir_impact {
                        8.0
                    } else {
                        0.0
                    },
                    1.0,
                    5.0,
                ]
            } else {
                [0.0; 4]
            };
            partial::append(
                patch,
                p,
                cell,
                origin,
                options.occluders.unwrap(),
                effects,
                natural_surface,
                sample,
            );
            continue;
        }
        let covered = connected_above(p);
        let water = patch.get_or_insert_with(empty_patch);
        water.normals.resize(water.positions.len(), [0.0, 1.0, 0.0]);
        water.flow.resize(water.positions.len(), [0.0; 2]);
        water
            .surface_effects
            .resize(water.positions.len(), [0.0; 4]);
        let x = (p[0] - origin[0]) as f32;
        let z = (p[2] - origin[2]) as f32;
        let y = (p[1] - origin[1]) as f32;
        let corners = [[0, 0], [0, 1], [1, 1], [1, 0]];
        let tops = corners.map(|corner| {
            let mut total = 0.0;
            let mut count = 0;
            let mut natural = None;
            let mut construction = None::<f64>;
            let mut incoming_head = None::<f64>;
            let mut falling = false;
            for dz in -1..=0 {
                for dx in -1..=0 {
                    let neighbor = [p[0] + corner[0] + dx, p[1], p[2] + corner[1] + dz];
                    if let Some(level) = sample(neighbor) {
                        // A sealed lower pocket is not an exposed water level
                        // and cannot pull down an unrelated surface outside it.
                        if options
                            .occluders
                            .is_some_and(|shapes| shapes.contains_key(&neighbor))
                            && volume_at(neighbor).is_some_and(|(_, _, space)| {
                                filled_covered_pocket(&space, level - neighbor[1] as f64)
                            })
                        {
                            continue;
                        }
                        let covered = connected_above(neighbor);
                        falling |= covered;
                        // A moving front and the full cell feeding it share
                        // the same incoming edge, including at a chunk seam.
                        // Read only the upstream halo; the next cell beyond a
                        // front may lie outside this worker's snapshot.
                        if !covered
                            && let Some(&moving) = fluids.get(&neighbor)
                            && moving.flow != [0; 2]
                            && canonical_level(neighbor, moving).is_none()
                        {
                            let axis = usize::from(moving.flow[0] == 0);
                            let coordinate = if axis == 0 { 0 } else { 2 };
                            let component = moving.flow[axis];
                            let edge = neighbor[coordinate] + i64::from(component < 0);
                            let corner_at = if coordinate == 0 {
                                p[0] + corner[0]
                            } else {
                                p[2] + corner[1]
                            };
                            let mut back = neighbor;
                            back[coordinate] -= i64::from(component);
                            if corner_at == edge
                                && let Some(head) = sample(back)
                                && head > level + 0.0001
                            {
                                incoming_head =
                                    Some(incoming_head.map_or(head, |old| old.max(head)));
                            }
                        }
                        let local: [usize; 3] =
                            std::array::from_fn(|i| (neighbor[i] - origin[i] + 1) as usize);
                        if super::space::partial(padded[local[0] + 18 * (local[1] + 18 * local[2])])
                            || options
                                .occluders
                                .is_some_and(|boxes| boxes.contains_key(&neighbor))
                        {
                            let level = if covered {
                                neighbor[1] as f64 + 1.0
                            } else {
                                level
                            };
                            construction = Some(construction.map_or(level, |old| old.max(level)));
                        }
                        total += level;
                        count += 1;
                        let cx = (neighbor[0] - origin[0] + 1) as usize;
                        let cz = (neighbor[2] - origin[2] + 1) as usize;
                        if !fluids.contains_key(&neighbor)
                            && let Some(water) = columns[cx + 18 * cz].water_columns().at(p[1])
                            && let level = water.level
                            && level <= p[1] as f64 + 1.0
                        {
                            natural = Some(level);
                        }
                    }
                }
            }
            let level = construction.or(natural).or(incoming_head).map_or_else(
                || {
                    if count > 0 {
                        total / f64::from(count)
                    } else {
                        height
                    }
                },
                |level| {
                    if river_surface && construction.is_none() && natural.is_some() {
                        super::channels::shared_corner_height(
                            columns,
                            (p[0] - origin[0] + corner[0]) as usize,
                            (p[2] - origin[2] + corner[1]) as usize,
                            level,
                        )
                    } else {
                        level
                    }
                },
            );
            // A falling cell joins the cell above at its base even when its
            // volume is fractional. Otherwise stacked cells expose air slits.
            let level = if falling { p[1] as f64 + 1.0 } else { level };
            (level - origin[1] as f64) as f32
        });
        let mut points = [
            [x, tops[0], z],
            [x, tops[1], z + 1.0],
            [x + 1.0, tops[2], z + 1.0],
            [x + 1.0, tops[3], z],
        ];
        let mut top_normal = [0.0, 1.0, 0.0];
        if !natural_surface && !covered && cell.flow != [0; 2] {
            let axis = usize::from(cell.flow[0] == 0);
            let component = cell.flow[axis];
            let coordinate = if axis == 0 { 0 } else { 2 };
            let mut back = p;
            let mut ahead = p;
            back[coordinate] -= i64::from(component);
            ahead[coordinate] += i64::from(component);
            let local: [i64; 3] = std::array::from_fn(|i| ahead[i] - origin[i] + 1);
            let dry = local.iter().all(|v| (0..18).contains(v)) && {
                let [lx, ly, lz] = local.map(|v| v as usize);
                super::tongue::front_boundary(padded[lx + 18 * (ly + 18 * lz)])
                    && sample(ahead).is_none()
            };
            if dry
                && let Some(head) = sample(back)
                && let Some(front) = terra_core::fluid_surface::Front::new(
                    cell.height() as f32,
                    (head - p[1] as f64) as f32,
                    axis,
                    component > 0,
                )
            {
                let block = |q: [i64; 3]| {
                    let l = std::array::from_fn::<_, 3, _>(|i| q[i] - origin[i] + 1);
                    if !l.iter().all(|v| (0..18).contains(v)) {
                        return None;
                    }
                    Some(padded[(l[0] + 18 * (l[1] + 18 * l[2])) as usize])
                };
                if let Some(tongue) =
                    super::tongue::Tongue::on_bed(front, p, block, |q| sample(q).is_some())
                {
                    tongue::append(water, &tongue, [x, y, z], cell.flow.map(f32::from));
                    continue;
                }
                let shape = [
                    [x, y + cell.height() as f32, z],
                    [x, y + cell.height() as f32, z + 1.],
                    [x + 1., y + cell.height() as f32, z + 1.],
                    [x + 1., y + cell.height() as f32, z],
                ];
                points = shape.map(|point| front.deform(point, [x, y, z], cell.height() as f32));
                top_normal = front.normal();
            }
        }
        if !natural_surface && covered && cell.amount < 8 {
            let width = (cell.height() as f32).sqrt();
            for point in &mut points {
                point[0] = x + 0.5 + (point[0] - x - 0.5) * width;
                point[2] = z + 0.5 + (point[2] - z - 0.5) * width;
            }
        }
        let flow = cell.flow.map(f32::from);
        let upper_owns_top = natural_surface && (height - (origin[1] + 16) as f64).abs() < 1e-8;
        if !covered && !upper_owns_top {
            quad(
                water,
                points,
                top_normal,
                flow,
                cell.height() as f32,
                [0.0; 4],
            );
        }
        let below = (p[0] - origin[0] + 1) as usize
            + 18 * ((p[1] - origin[1]) as usize + 18 * (p[2] - origin[2] + 1) as usize);
        if super::can_hold_fluid(padded[below]) && sample([p[0], p[1] - 1, p[2]]).is_none() {
            quad(
                water,
                [
                    [points[0][0], y, points[0][2]],
                    [points[3][0], y, points[3][2]],
                    [points[2][0], y, points[2][2]],
                    [points[1][0], y, points[1][2]],
                ],
                [0.0, -1.0, 0.0],
                flow,
                cell.height() as f32,
                [0.0; 4],
            );
        }
        let neighbors = [[-1, 0], [0, 1], [1, 0], [0, -1]];
        for (side, offset) in neighbors.into_iter().enumerate() {
            let q = [p[0] + offset[0], p[1], p[2] + offset[1]];
            let ix = (q[0] - origin[0] + 1) as usize;
            let iy = (q[1] - origin[1] + 1) as usize;
            let iz = (q[2] - origin[2] + 1) as usize;
            let occluders = options.occluders.and_then(|boxes| boxes.get(&q));
            let neighbor_water = sample(q);
            let partial_neighbor = super::space::partial(padded[ix + 18 * (iy + 18 * iz)]);
            if (!super::can_hold_fluid(padded[ix + 18 * (iy + 18 * iz)]) && occluders.is_none())
                || neighbor_water.is_some() && !partial_neighbor
            {
                continue;
            }
            let a = points[side];
            let b = points[(side + 1) % 4];
            let flowing_down = covered
                || sample([p[0], p[1] - 1, p[2]]).is_some()
                || super::can_hold_fluid(padded[below]);
            let column =
                columns[(p[0] - origin[0] + 1) as usize + 18 * (p[2] - origin[2] + 1) as usize];
            let reservoir_impact = column
                .water_columns()
                .at(p[1] - 1)
                .is_some_and(|w| (w.level - p[1] as f64).abs() < 0.01);
            let effects = if flowing_down {
                [
                    if covered { 8.0 } else { 0.0 },
                    if super::can_hold_fluid(padded[below]) && !reservoir_impact {
                        8.0
                    } else {
                        0.0
                    },
                    1.0,
                    5.0,
                ]
            } else {
                [0.0; 4]
            };
            let points = [b, a, [a[0], y, a[2]], [b[0], y, b[2]]];
            let normal = [offset[0] as f32, 0.0, offset[1] as f32];
            if let Some(occluders) = occluders {
                let mut masks = occluders.clone();
                if partial_neighbor && let Some(level) = neighbor_water {
                    let space = super::space::FluidSpace::from_global(occluders, q);
                    let height = if connected_above(q) {
                        1.0
                    } else {
                        level - q[1] as f64
                    };
                    masks.extend(space.wet_boxes(height).into_iter().map(|b| ShapeBox {
                        minimum: std::array::from_fn(|i| b.minimum[i] + q[i] as f64),
                        maximum: std::array::from_fn(|i| b.maximum[i] + q[i] as f64),
                        hinged: false,
                    }));
                }
                let top = a[1].max(b[1]);
                for fragment in occlusion::fragments(points, normal, origin, &masks) {
                    let fragment = occlusion::clip(&fragment, 1, y, true);
                    let mut fragment_effects = effects;
                    if effects[3] == 5.0 {
                        let fragment_top = fragment
                            .iter()
                            .map(|p| p[1])
                            .fold(f32::NEG_INFINITY, f32::max);
                        let fragment_bottom =
                            fragment.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
                        fragment_effects[0] += top - fragment_top;
                        fragment_effects[1] += fragment_bottom - y;
                    }
                    polygon(
                        water,
                        &fragment,
                        normal,
                        flow,
                        cell.height() as f32,
                        fragment_effects,
                    );
                }
            } else if a[1] < y || b[1] < y {
                // Shore corners follow the shared reservoir surface and can
                // descend below this cell. Clip its walls to its own volume:
                // an inverted wall overlaps the next falling cell and shimmers.
                let points = occlusion::clip(&points, 1, y, true);
                polygon(water, &points, normal, flow, cell.height() as f32, effects);
            } else {
                quad(water, points, normal, flow, cell.height() as f32, effects);
            }
        }
    }
    // A completely covered cell may create the accumulator without emitting
    // any faces. It is a valid simulation state, not a renderable water mesh.
    if patch.as_ref().is_some_and(|water| water.indices.is_empty()) {
        *patch = None;
    } else if let Some(water) = patch {
        // These cells materialize the unchanged natural surface so construction
        // can clip it. A neighboring worker may not see the construction two
        // cells beyond its halo, and must still displace their shared edge in
        // exactly the same way. Actual changed fluid levels remain pinned.
        let wave_edits: Option<FluidSnapshot> = options.precise_levels.map(|natural| {
            fluids
                .iter()
                .filter(|(p, _)| !natural.contains_key(*p))
                .map(|(&p, &cell)| (p, cell))
                .collect()
        });
        super::geometry::assign_wave_weights(
            water,
            origin,
            columns,
            Some(wave_edits.as_ref().unwrap_or(fluids)),
            padded,
        );
    }
}

pub(crate) fn empty_patch() -> WaterPatch {
    WaterPatch {
        normals: Vec::new(),
        flow: Vec::new(),
        positions: Vec::new(),
        depth_shore: Vec::new(),
        surface_effects: Vec::new(),
        wave_weights: Vec::new(),
        indices: Vec::new(),
    }
}

fn quad(
    patch: &mut WaterPatch,
    points: [[f32; 3]; 4],
    normal: [f32; 3],
    flow: [f32; 2],
    depth: f32,
    effect: [f32; 4],
) {
    polygon(patch, &points, normal, flow, depth, effect);
}

fn polygon(
    patch: &mut WaterPatch,
    points: &[[f32; 3]],
    normal: [f32; 3],
    flow: [f32; 2],
    depth: f32,
    effect: [f32; 4],
) {
    if points.len() < 3 {
        return;
    }
    let outward = |i: usize| {
        let u: [f32; 3] = std::array::from_fn(|axis| points[i][axis] - points[0][axis]);
        let v: [f32; 3] = std::array::from_fn(|axis| points[i + 1][axis] - points[0][axis]);
        let cross = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        cross.iter().zip(normal).map(|(a, b)| a * b).sum::<f32>() > 1e-8
    };
    if !(1..points.len() - 1).any(outward) {
        return;
    }
    let first = patch.positions.len() as u32;
    patch.positions.extend_from_slice(points);
    patch
        .normals
        .extend(std::iter::repeat_n(normal, points.len()));
    patch.flow.extend(std::iter::repeat_n(flow, points.len()));
    patch
        .depth_shore
        .extend(std::iter::repeat_n([depth.max(0.12), 0.5], points.len()));
    if effect[3] == 5.0 {
        let top = points
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max);
        let bottom = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        patch.surface_effects.extend(points.iter().map(|p| {
            [
                effect[0] + top - p[1],
                effect[1] + p[1] - bottom,
                effect[2],
                effect[3],
            ]
        }));
    } else {
        patch
            .surface_effects
            .extend(std::iter::repeat_n(effect, points.len()));
    }
    for i in (1..points.len() - 1).filter(|&i| outward(i)) {
        patch
            .indices
            .extend([first, first + i as u32, first + i as u32 + 1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TerraGenerator, water::FluidCell};
    use terra_core::WorldConfig;
    fn columns() -> Vec<SurfaceColumn> {
        let terrain = TerraGenerator::new(WorldConfig {
            base_height: 5.0,
            sea_level: 3.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            caves: false,
            biomes: false,
            trees: false,
            rivers: false,
            ..Default::default()
        })
        .unwrap();
        vec![terrain.surface_column(0.5, 0.5); 18 * 18]
    }

    #[test]
    fn first_water_front_covers_a_fraction_of_the_next_cell_instead_of_a_flat_square() {
        let fluids = FluidSnapshot::from([
            (
                [7, 7, 8],
                FluidCell {
                    amount: 8,
                    source: true,
                    flow: [0; 2],
                },
            ),
            (
                [8, 7, 8],
                FluidCell {
                    amount: 1,
                    source: false,
                    flow: [1, 0],
                },
            ),
        ]);
        let mut patch = None;
        append_fluid_patch(
            &mut patch,
            SectionPos::default(),
            &columns(),
            &vec![0; 18 * 18 * 18],
            &fluids,
            false,
        );
        let patch = patch.unwrap();
        let front: Vec<_> = patch
            .positions
            .iter()
            .filter(|p| p[0] > 8.0001 && p[2] >= 8. && p[2] <= 9.)
            .collect();
        assert!(!front.is_empty());
        assert!(
            front.iter().all(|p| p[0] <= 8.251),
            "new eighth must advance only a quarter cell against a full head"
        );
    }

    #[test]
    fn free_fluid_cells_keep_shared_sloping_corners_beside_a_partial_block_across_sections() {
        let columns = columns();
        let p = [15, 7, 8];
        let q = [16, 7, 8];
        let slab = [14, 7, 8];
        let fluids = FluidSnapshot::from([
            (
                p,
                FluidCell {
                    amount: 8,
                    ..Default::default()
                },
            ),
            (
                q,
                FluidCell {
                    amount: 2,
                    ..Default::default()
                },
            ),
        ]);
        let occluders = FluidOccluders::from([(
            slab,
            vec![ShapeBox {
                minimum: [14.0, 7.0, 8.0],
                maximum: [15.0, 7.5, 9.0],
                hinged: false,
            }],
        )]);
        let mut edges = Vec::new();
        for position in [SectionPos::default(), SectionPos::new(1, 0, 0)] {
            let origin = position.origin().unwrap();
            let mut padded = vec![0; 18 * 18 * 18];
            let local: [i64; 3] = std::array::from_fn(|i| slab[i] - origin[i] + 1);
            if local.iter().all(|v| (0..18).contains(v)) {
                padded[local[0] as usize + 18 * (local[1] as usize + 18 * local[2] as usize)] = 82;
            }
            let mut patch = None;
            append_fluid_patch(
                &mut patch,
                position,
                &columns,
                &padded,
                &fluids,
                (false, &occluders),
            );
            let patch = patch.unwrap();
            let mut edge: Vec<_> = patch
                .positions
                .iter()
                .zip(&patch.normals)
                .filter(|(point, normal)| normal[1] > 0.5 && point[0] + origin[0] as f32 == 16.0)
                .map(|(point, _)| [point[1], point[2]])
                .collect();
            edge.sort_by(|a, b| a[1].total_cmp(&b[1]));
            assert_eq!(edge.len(), 2);
            assert!(
                edge.iter().all(|p| p[0] == 7.625),
                "construction proximity must not replace one side with a flat box top"
            );
            edges.push(edge);
        }
        assert_eq!(edges[0], edges[1]);
    }

    #[test]
    fn waterlogged_slab_pins_free_surface_edges_and_culls_both_sides_of_the_wet_interface() {
        let columns = columns();
        let p = [15, 7, 8];
        let q = [16, 7, 8];
        let fluids = FluidSnapshot::from([
            (
                p,
                FluidCell {
                    amount: 6,
                    ..Default::default()
                },
            ),
            (
                q,
                FluidCell {
                    amount: 1,
                    ..Default::default()
                },
            ),
        ]);
        let occluders = FluidOccluders::from([(
            p,
            vec![ShapeBox {
                minimum: [15.0, 7.0, 8.0],
                maximum: [16.0, 7.5, 9.0],
                hinged: false,
            }],
        )]);
        let mut edges = Vec::new();
        for (position, normal) in [
            (SectionPos::default(), [1.0, 0.0, 0.0]),
            (SectionPos::new(1, 0, 0), [-1.0, 0.0, 0.0]),
        ] {
            let origin = position.origin().unwrap();
            let mut padded = vec![0; 18 * 18 * 18];
            let local: [usize; 3] = std::array::from_fn(|i| (p[i] - origin[i] + 1) as usize);
            padded[local[0] + 18 * (local[1] + 18 * local[2])] = 82;
            let mut patch = None;
            append_fluid_patch(
                &mut patch,
                position,
                &columns,
                &padded,
                &fluids,
                (false, &occluders),
            );
            let patch = patch.unwrap();
            assert_eq!(
                side_area(&patch, normal),
                0.0,
                "connected wet interface cannot expose overlapping interior triangles"
            );
            let mut edge: Vec<_> = patch
                .positions
                .iter()
                .zip(&patch.normals)
                .filter(|(p, n)| n[1] > 0.5 && p[0] + origin[0] as f32 == 16.0)
                .map(|(p, _)| [p[1], p[2]])
                .collect();
            edge.sort_by(|a, b| a[1].total_cmp(&b[1]));
            edge.dedup();
            assert_eq!(edge.len(), 2);
            assert!(edge.iter().all(|p| p[0] == 7.75));
            edges.push(edge);
        }
        assert_eq!(edges[0], edges[1]);
    }

    #[test]
    fn enclosed_fluid_at_section_boundary_does_not_create_an_empty_render_patch() {
        let columns = columns();
        for section in [SectionPos::default(), SectionPos::new(-1, -1, -1)] {
            let origin = section.origin().unwrap();
            let p = [origin[0] + 8, origin[1] + 15, origin[2] + 8];
            let upper = [p[0], p[1] + 1, p[2]];
            let mut padded = vec![3; 18 * 18 * 18];
            padded[9 + 18 * (16 + 18 * 9)] = 0;
            padded[9 + 18 * (17 + 18 * 9)] = 0;
            let water = FluidCell {
                amount: 8,
                source: true,
                flow: [0; 2],
            };
            let fluids = FluidSnapshot::from([(p, water), (upper, water)]);
            let mut patch = None;
            append_fluid_patch(&mut patch, section, &columns, &padded, &fluids, false);
            assert!(patch.is_none());

            // Covered cells must not remove an existing visible lake surface.
            let mut visible = empty_patch();
            quad(
                &mut visible,
                [
                    [0.0, 1.0, 0.0],
                    [0.0, 1.0, 1.0],
                    [1.0, 1.0, 1.0],
                    [1.0, 1.0, 0.0],
                ],
                [0.0, 1.0, 0.0],
                [0.0; 2],
                1.0,
                [0.0; 4],
            );
            let mut patch = Some(visible);
            append_fluid_patch(&mut patch, section, &columns, &padded, &fluids, false);
            assert_eq!(patch.as_ref().unwrap().positions.len(), 4);
            assert_eq!(patch.as_ref().unwrap().indices.len(), 6);
        }
    }
    #[test]
    fn flowing_water_mesh_has_valid_indices_finite_vertices_and_shared_negative_edges() {
        let columns = columns();
        let padded = vec![0; 18 * 18 * 18];
        let fluids = FluidSnapshot::from([
            (
                [-1, 7, -1],
                FluidCell {
                    amount: 8,
                    flow: [1, 0],
                    source: false,
                },
            ),
            (
                [0, 7, -1],
                FluidCell {
                    amount: 2,
                    flow: [1, 0],
                    source: false,
                },
            ),
        ]);
        let build = |position| {
            let mut patch = None;
            append_fluid_patch(&mut patch, position, &columns, &padded, &fluids, true);
            patch.unwrap()
        };
        let left = build(SectionPos::new(-1, 0, -1));
        let right = build(SectionPos::new(0, 0, -1));
        for mesh in [&left, &right] {
            assert_eq!(mesh.positions.len(), mesh.normals.len());
            assert_eq!(mesh.positions.len(), mesh.flow.len());
            assert_eq!(mesh.positions.len(), mesh.depth_shore.len());
            assert_eq!(mesh.positions.len(), mesh.surface_effects.len());
            assert!(mesh.positions.iter().flatten().all(|v| v.is_finite()));
            assert!(
                mesh.indices
                    .iter()
                    .all(|&i| (i as usize) < mesh.positions.len())
            );
            for indices in mesh.indices.chunks_exact(3) {
                let [a, b, c] =
                    [indices[0], indices[1], indices[2]].map(|i| mesh.positions[i as usize]);
                let u: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
                let v: [f32; 3] = std::array::from_fn(|i| c[i] - a[i]);
                let cross = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                let normal = mesh.normals[indices[0] as usize];
                assert!(cross.iter().zip(normal).map(|(a, b)| a * b).sum::<f32>() > 0.0);
            }
        }
        let edge = |mesh: &WaterPatch, x| {
            let mut vertices: Vec<_> = mesh.positions[..4]
                .iter()
                .filter(|p| p[0] == x)
                .map(|p| [p[1], p[2]])
                .collect();
            vertices.sort_by(|a, b| a[1].total_cmp(&b[1]));
            vertices
        };
        assert_eq!(edge(&left, 16.0), edge(&right, 0.0));
    }

    #[test]
    fn a_low_dry_bank_never_inverts_fluid_walls_below_their_cell_or_duplicates_the_lip() {
        let mut columns = columns();
        for z in 0..18 {
            for x in 0..18 {
                columns[x + 18 * z].height = 8.0;
                columns[x + 18 * z].water_level = (x < 17).then_some(10.5);
            }
        }
        let fluids = FluidSnapshot::from([(
            [15, 10, 8],
            FluidCell {
                amount: 4,
                source: false,
                flow: [1, 0],
            },
        )]);
        let mut patch = None;
        append_fluid_patch(
            &mut patch,
            SectionPos::default(),
            &columns,
            &vec![0; 18 * 18 * 18],
            &fluids,
            true,
        );
        let patch = patch.unwrap();
        assert!(
            patch.positions.iter().any(|p| p[1] < 10.0),
            "the top must retain the shared natural shore slope"
        );
        for triangle in patch.indices.chunks_exact(3) {
            let [a, b, c] = std::array::from_fn(|i| patch.positions[triangle[i] as usize]);
            let u: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
            let v: [f32; 3] = std::array::from_fn(|i| c[i] - a[i]);
            let cross = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let normal = patch.normals[triangle[0] as usize];
            assert!(
                cross.iter().zip(normal).map(|(a, b)| a * b).sum::<f32>() > 1e-8,
                "a reversed/degenerate wall can shimmer against the lower volume"
            );
            if normal[1] == 0.0 {
                assert!([a, b, c].iter().all(|p| p[1] >= 10.0));
            }
        }
    }

    #[test]
    fn fractional_falling_cells_join_at_vertical_section_edges_and_close_the_front() {
        let columns = columns();
        let padded = vec![0; 18 * 18 * 18];
        let fluids = FluidSnapshot::from([
            (
                [-1, 15, -1],
                FluidCell {
                    amount: 1,
                    ..Default::default()
                },
            ),
            (
                [-1, 16, -1],
                FluidCell {
                    amount: 3,
                    ..Default::default()
                },
            ),
        ]);
        let mut lower = None;
        let mut upper = None;
        append_fluid_patch(
            &mut lower,
            SectionPos::new(-1, 0, -1),
            &columns,
            &padded,
            &fluids,
            false,
        );
        append_fluid_patch(
            &mut upper,
            SectionPos::new(-1, 1, -1),
            &columns,
            &padded,
            &fluids,
            false,
        );
        let lower = lower.unwrap();
        let upper = upper.unwrap();
        assert!(lower.positions.iter().any(|p| p[1] == 16.0));
        assert!(upper.positions.iter().any(|p| p[1] == 0.0));
        assert_eq!(
            lower
                .normals
                .iter()
                .filter(|n| **n == [0.0, -1.0, 0.0])
                .count(),
            4
        );
        assert!(lower.normals.iter().all(|n| *n != [0.0, 1.0, 0.0]));
        assert!(upper.normals.iter().all(|n| *n != [0.0, -1.0, 0.0]));
        for patch in [&lower, &upper] {
            assert!(
                patch
                    .indices
                    .iter()
                    .all(|&i| (i as usize) < patch.positions.len())
            );
        }
    }
    #[test]
    fn natural_and_dynamic_surfaces_share_exact_river_and_ocean_vertices() {
        let mut columns = columns();
        for z in 0..18 {
            for x in 0..=8 {
                columns[x + 18 * z].height = 8.0;
                columns[x + 18 * z].water_level = Some(10.5);
            }
        }
        for column in &mut columns {
            if column.water_level.is_none() {
                column.height = 12.0;
            }
        }
        let padded = vec![0; 18 * 18 * 18];
        let fluids = FluidSnapshot::from([(
            [8, 10, 8],
            FluidCell {
                amount: 2,
                ..Default::default()
            },
        )]);
        for river in [false, true] {
            let mut patch = None;
            append_fluid_patch(
                &mut patch,
                SectionPos::default(),
                &columns,
                &padded,
                &fluids,
                river,
            );
            let patch = patch.unwrap();
            let border: Vec<_> = patch.positions[..4]
                .iter()
                .filter(|p| p[0] == 8.0)
                .collect();
            assert_eq!(border.len(), 2);
            for p in border {
                let expected = if river {
                    super::super::channels::shared_corner_height(
                        &columns,
                        p[0] as usize,
                        p[2] as usize,
                        10.5,
                    )
                } else {
                    10.5
                };
                assert_eq!(p[1], expected as f32);
            }
        }
    }

    fn side_area(patch: &WaterPatch, normal: [f32; 3]) -> f32 {
        patch
            .indices
            .chunks_exact(3)
            .filter(|triangle| patch.normals[triangle[0] as usize] == normal)
            .map(|triangle| {
                let [a, b, c] = std::array::from_fn(|i| patch.positions[triangle[i] as usize]);
                let u: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
                let v: [f32; 3] = std::array::from_fn(|i| c[i] - a[i]);
                let cross = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                let oriented_area = cross.iter().zip(normal).map(|(a, b)| a * b).sum::<f32>();
                assert!(
                    oriented_area > 0.0,
                    "invalid water-face winding or zero-area triangle"
                );
                oriented_area * 0.5
            })
            .sum()
    }

    #[test]
    fn falling_water_beside_all_slab_and_stair_rotations_exposes_only_the_free_face() {
        let columns = columns();
        for section in [SectionPos::default(), SectionPos::new(-1, -1, -1)] {
            let origin = section.origin().unwrap();
            let p = [origin[0] + 15, origin[1] + 7, origin[2] + 15];
            let fluids = FluidSnapshot::from([(
                p,
                FluidCell {
                    amount: 8,
                    ..Default::default()
                },
            )]);
            for (material, facings) in [(74, (0..24).collect::<Vec<_>>()), (82, (0..58).collect())]
            {
                assert!(super::super::can_hold_fluid(material));
                for facing in facings {
                    for [dx, dz] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
                        let q = [p[0] + dx, p[1], p[2] + dz];
                        let mut boxes =
                            terra_core::construction::placed_boxes(material, facing, [None; 6]);
                        for bounds in &mut boxes {
                            for (i, coordinate) in q.into_iter().enumerate() {
                                bounds.minimum[i] += coordinate as f64;
                                bounds.maximum[i] += coordinate as f64;
                            }
                        }
                        let normal = [dx as f32, 0.0, dz as f32];
                        let axis = if dx == 0 { 2 } else { 0 };
                        let tangent = 2 - axis;
                        let plane = p[axis] as f64 + if normal[axis] > 0.0 { 1.0 } else { 0.0 };
                        let mut free_sixths = 0;
                        for u in 0..6 {
                            for v in 0..6 {
                                let mut point = p.map(|x| x as f64);
                                point[axis] = plane;
                                point[tangent] += (f64::from(u) + 0.5) / 6.0;
                                point[1] += (f64::from(v) + 0.5) / 6.0;
                                if !boxes.iter().any(|bounds| {
                                    (0..3).all(|i| {
                                        point[i] >= bounds.minimum[i]
                                            && point[i] <= bounds.maximum[i]
                                    })
                                }) {
                                    free_sixths += 1;
                                }
                            }
                        }
                        let occluders = FluidOccluders::from([(q, boxes)]);
                        let mut padded = vec![0; 18 * 18 * 18];
                        let [x, y, z] = std::array::from_fn(|i| (q[i] - origin[i] + 1) as usize);
                        padded[x + 18 * (y + 18 * z)] = material;
                        let mut patch = None;
                        append_fluid_patch(
                            &mut patch,
                            section,
                            &columns,
                            &padded,
                            &fluids,
                            (false, &occluders),
                        );
                        let patch = patch.unwrap();
                        let expected = free_sixths as f32 / 36.0;
                        assert!(
                            (side_area(&patch, normal) - expected).abs() < 0.00001,
                            "material={material} facing={facing} normal={normal:?} expected={expected}"
                        );
                        assert_eq!(patch.positions.len(), patch.surface_effects.len());
                        assert!(patch.positions.iter().flatten().all(|v| v.is_finite()));
                        for ((point, effect), n) in patch
                            .positions
                            .iter()
                            .zip(&patch.surface_effects)
                            .zip(&patch.normals)
                        {
                            if *n == normal {
                                assert!((effect[0] - (8.0 - point[1])).abs() < 0.00001);
                                assert!((effect[1] - (8.0 + point[1] - 7.0)).abs() < 0.00001);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn falling_water_preserves_occlusion_for_full_neighbors_and_below_a_slab_top() {
        let columns = columns();
        let p = [8, 7, 8];
        let q = [9, 7, 8];
        let fluids = FluidSnapshot::from([(
            p,
            FluidCell {
                amount: 4,
                ..Default::default()
            },
        )]);
        let mut padded = vec![0; 18 * 18 * 18];
        padded[10 + 18 * (8 + 18 * 9)] = 3;
        let mut opaque = None;
        append_fluid_patch(
            &mut opaque,
            SectionPos::default(),
            &columns,
            &padded,
            &fluids,
            false,
        );
        assert_eq!(side_area(opaque.as_ref().unwrap(), [1.0, 0.0, 0.0]), 0.0);

        padded[10 + 18 * (8 + 18 * 9)] = 82;
        let bounds = ShapeBox {
            minimum: [9.0, 7.0, 8.0],
            maximum: [10.0, 7.5, 9.0],
            hinged: false,
        };
        let occluders = FluidOccluders::from([(q, vec![bounds])]);
        let mut half = None;
        append_fluid_patch(
            &mut half,
            SectionPos::default(),
            &columns,
            &padded,
            &fluids,
            (false, &occluders),
        );
        assert_eq!(side_area(half.as_ref().unwrap(), [1.0, 0.0, 0.0]), 0.0);
    }
}

#[cfg(test)]
#[path = "fluid_mesh/shore_transition_tests.rs"]
mod shore_transition_tests;
