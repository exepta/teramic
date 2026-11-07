//! Owned, bounded belt spans. Shafts remain independent endpoint blocks.
use crate::{
    edits::{BeltOwner, BlockObjectState, EditJournal},
    placement::Layout,
    streaming::{BlockEditRequest, ChunkStore},
};
use terra_core::{
    ItemKind, WoodSpecies,
    kinetic::{self, Cell},
};

pub const MAX_SEGMENTS: u8 = kinetic::MAX_BELT_SEGMENTS;
pub const PROFILE_RADIUS: f32 = 0.125;

mod actors;
mod occupancy;
pub use actors::{carry_creature, surface_velocity};
pub use occupancy::{
    DropIndex, drop_space_clear, drop_step_clear, index_drops, move_indexed_drop,
    remove_indexed_drop,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub controller: Cell,
    pub first: Cell,
    pub start: Cell,
    pub end: Cell,
    pub step: Cell,
    pub length: u8,
    pub facing: u8,
    pub wood: WoodSpecies,
    pub linked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub span: Span,
    /// -1 and length are the preserved start and end shafts.
    pub index: i16,
}

/// Reconstruct the owned belt item without discarding its frame species or dye.
pub fn item(span: Span, journal: &EditJournal) -> ItemKind {
    if let Some(owner) = journal.belt_owner(span.controller) {
        return owner.item;
    }
    let item = ItemKind::for_wood(kinetic::CONVEYOR, span.wood);
    let color = journal
        .tint_at(span.controller)
        .map(terra_core::beds::DyeColor::Rgb)
        .or_else(|| {
            journal
                .object_state(span.controller)
                .and_then(|state| kinetic::belt_color(state.facing))
        });
    color
        .and_then(|color| terra_core::tints::recolored(item, color))
        .unwrap_or(item)
}

fn at(first: Cell, step: Cell, index: i64) -> Cell {
    std::array::from_fn(|i| first[i] + step[i] * index)
}
fn shaft(journal: &EditJournal, p: Cell) -> Option<BlockObjectState> {
    let material = journal.block(p)?;
    (matches!(material, 144..=146))
        .then(|| journal.object_state(p))
        .flatten()
        .filter(|state| state.valid_for(p, material) && state.facing < 6)
}

impl Span {
    pub fn cells(self) -> impl Iterator<Item = Cell> {
        (0..self.length).map(move |index| at(self.first, self.step, i64::from(index)))
    }
    pub fn direction(self) -> [f32; 3] {
        let length = self
            .step
            .iter()
            .map(|v| (*v as f32).powi(2))
            .sum::<f32>()
            .sqrt();
        self.step.map(|v| v as f32 / length)
    }
    pub fn normal(self) -> [f32; 3] {
        let direction = self.direction();
        let axis = kinetic::axis(self.facing).map(|v| v.unsigned_abs() as f32);
        let mut normal = [
            direction[1] * axis[2] - direction[2] * axis[1],
            direction[2] * axis[0] - direction[0] * axis[2],
            direction[0] * axis[1] - direction[1] * axis[0],
        ];
        if normal[1] < 0.0 {
            normal = normal.map(|v| -v);
        }
        normal
    }
    pub fn flow_sign(self) -> f32 {
        let axis = kinetic::axis(self.facing).map(|v| v as f32);
        let normal = self.normal();
        let velocity = [
            axis[1] * normal[2] - axis[2] * normal[1],
            axis[2] * normal[0] - axis[0] * normal[2],
            axis[0] * normal[1] - axis[1] * normal[0],
        ];
        velocity
            .into_iter()
            .zip(self.direction())
            .map(|(a, b)| a * b)
            .sum::<f32>()
            .signum()
    }
    pub fn loaded(self, chunks: &ChunkStore, journal: &EditJournal) -> bool {
        self.cells().chain([self.start, self.end]).all(|p| {
            chunks
                .block_at(p)
                .is_some_and(|material| journal.block(p) == Some(material))
        })
    }
    pub fn links(self, p: Cell) -> [Option<Cell>; 2] {
        if !self.linked {
            return [None; 2];
        }
        if p == self.start {
            return [None, Some(self.first)];
        }
        if p == self.end {
            return [Some(at(p, self.step, -1)), None];
        }
        [Some(at(p, self.step, -1)), Some(at(p, self.step, 1))]
    }
}
impl Segment {
    pub fn position(self) -> Cell {
        at(self.span.first, self.span.step, i64::from(self.index))
    }
    pub fn center(self) -> [f64; 3] {
        self.position().map(|v| v as f64 + 0.5)
    }
    /// Upper carrying plane. Vertical belts transmit rotation but carry no drops.
    pub fn surface_height(self, point: [f64; 3]) -> Option<f64> {
        let n = self.span.normal().map(f64::from);
        if n[1] < 0.1 {
            return None;
        }
        let center = self.center();
        Some(
            center[1]
                + (f64::from(PROFILE_RADIUS)
                    - n[0] * (point[0] - center[0])
                    - n[2] * (point[2] - center[2]))
                    / n[1],
        )
    }
    pub fn drop_center(self) -> Option<[f64; 3]> {
        let mut point = self.center();
        let slope = self.span.step[1].unsigned_abs() as f64;
        point[1] = self.surface_height(point)? + 0.12 * (1.0 + slope) + 0.01;
        Some(point)
    }
}

/// Derives ownership solely from bounded persistent metadata, even across unloads.
pub fn span_at(journal: &EditJournal, position: Cell) -> Option<Span> {
    span_with(
        position,
        |p| journal.block(p),
        |p| journal.object_state(p),
        |p| journal.belt_owner(p),
    )
}

fn span_with(
    position: Cell,
    block: impl Fn(Cell) -> Option<u16>,
    state_at: impl Fn(Cell) -> Option<BlockObjectState>,
    owner_at: impl Fn(Cell) -> Option<BeltOwner>,
) -> Option<Span> {
    let material = block(position)?;
    if !matches!(material, 144..=146 | 148) {
        return None;
    }
    let selected = state_at(position)?;
    if !selected.valid_for(position, material) {
        return None;
    }
    // Endpoint and guide peers reach the controller in at most three hops.
    // The owner is independent of a shaft's own wood and six-axis pose.
    let mut candidate = position;
    for _ in 0..4 {
        if let Some(owner) = owner_at(candidate) {
            return owned_span(position, candidate, owner, &block, &state_at);
        }
        let Some(peer) = state_at(candidate).and_then(|state| state.peer) else {
            break;
        };
        candidate = peer;
    }
    let Some(peer) = selected.peer else {
        if material != 148 {
            return None;
        }
        return Some(Span {
            controller: position,
            first: position,
            start: position,
            end: position,
            step: kinetic::forward(kinetic::belt_facing(selected.facing)),
            length: 1,
            facing: kinetic::belt_facing(selected.facing),
            wood: selected.wood,
            linked: false,
        });
    };
    let controller = if material == 148 {
        if block(peer) == Some(148) {
            peer
        } else {
            position
        }
    } else if block(peer) == Some(148) {
        peer
    } else {
        state_at(peer)?.peer?
    };
    if block(controller) != Some(148) {
        return None;
    }
    let state = state_at(controller)?;
    if !state.valid_for(controller, 148) {
        return None;
    }
    let end = state.peer?;
    let end_material = block(end)?;
    let end_state = state_at(end)?;
    if !matches!(end_material, 144..=146) || !end_state.valid_for(end, end_material) {
        return None;
    }
    let new_format = end_state.peer.is_some();
    let reference = if new_format {
        end_state.peer?
    } else {
        controller
    };
    let delta: Cell = std::array::from_fn(|i| end[i] - reference[i]);
    let distance = delta.iter().map(|v| v.unsigned_abs()).max()?;
    if distance < if new_format { 2 } else { 1 }
        || distance > u64::from(MAX_SEGMENTS) + u64::from(new_format)
        || delta
            .iter()
            .any(|v| *v != 0 && v.unsigned_abs() != distance)
    {
        return None;
    }
    let step = delta.map(i64::signum);
    let start = if new_format {
        reference
    } else {
        at(controller, step, -1)
    };
    let first = at(start, step, 1);
    let length = distance - u64::from(new_format);
    let start_material = block(start)?;
    let start_state = state_at(start)?;
    if !matches!(start_material, 144..=146)
        || !start_state.valid_for(start, start_material)
        || start_state.peer != if new_format { Some(controller) } else { None }
    {
        return None;
    }
    let facing = kinetic::belt_facing(state.facing);
    let axis = kinetic::axis(facing);
    if state.facing >= kinetic::BELT_FACING_COUNT
        || state.secondary
        || (0..3).map(|i| axis[i] * step[i]).sum::<i64>() != 0
        || kinetic::axis(start_state.facing).map(i64::abs) != axis.map(i64::abs)
        || kinetic::axis(end_state.facing).map(i64::abs) != axis.map(i64::abs)
    {
        return None;
    }
    let span = Span {
        controller,
        first,
        start,
        end,
        step,
        length: length as u8,
        facing,
        wood: state.wood,
        linked: true,
    };
    if !span.cells().any(|p| p == controller)
        || !span.cells().all(|p| {
            let Some(material) = block(p) else {
                return false;
            };
            state_at(p).is_some_and(|part| {
                part.valid_for(p, material)
                    && !part.secondary
                    && part.peer == Some(if p == controller { end } else { controller })
                    && if material == 148 {
                        part.facing == state.facing && part.wood == state.wood
                    } else {
                        new_format
                            && matches!(material, 144..=146)
                            && kinetic::axis(part.facing).map(i64::abs) == axis.map(i64::abs)
                    }
            })
        })
    {
        return None;
    }
    (position == start || position == end || span.cells().any(|p| p == position)).then_some(span)
}

fn owned_span(
    selected: Cell,
    controller: Cell,
    owner: BeltOwner,
    block: &impl Fn(Cell) -> Option<u16>,
    state_at: &impl Fn(Cell) -> Option<BlockObjectState>,
) -> Option<Span> {
    if !owner.valid_at(controller) {
        return None;
    }
    let start = state_at(owner.start)?;
    let end = state_at(owner.end)?;
    for (position, state) in [(owner.start, start), (owner.end, end)] {
        let material = block(position)?;
        if !matches!(material, 144..=146) || !state.valid_for(position, material) {
            return None;
        }
    }
    let axis = kinetic::axis(start.facing).map(i64::abs);
    let delta: Cell = std::array::from_fn(|i| owner.end[i] - owner.start[i]);
    let distance = delta.iter().map(|v| v.unsigned_abs()).max()?;
    let step = delta.map(i64::signum);
    if axis != kinetic::axis(end.facing).map(i64::abs)
        || axis.into_iter().zip(step).map(|(a, b)| a * b).sum::<i64>() != 0
        || start.peer != Some(controller)
        || end.peer != Some(owner.start)
    {
        return None;
    }
    let span = Span {
        controller,
        first: at(owner.start, step, 1),
        start: owner.start,
        end: owner.end,
        step,
        length: (distance - 1) as u8,
        facing: start.facing,
        wood: owner.item.wood_species().unwrap_or_default(),
        linked: true,
    };
    if !span.cells().all(|position| {
        let Some(material) = block(position) else {
            return false;
        };
        state_at(position).is_some_and(|state| {
            state.valid_for(position, material)
                && !state.secondary
                && state.peer
                    == Some(if position == controller {
                        owner.end
                    } else {
                        controller
                    })
                && if material == 148 {
                    kinetic::axis(kinetic::belt_facing(state.facing)).map(i64::abs) == axis
                        && state.wood == span.wood
                } else {
                    matches!(material, 144..=146)
                        && kinetic::axis(state.facing).map(i64::abs) == axis
                }
        })
    }) {
        return None;
    }
    (selected == span.start || selected == span.end || span.cells().any(|p| p == selected))
        .then_some(span)
}

pub fn connected_at(journal: &EditJournal, position: Cell) -> Vec<Span> {
    let mut spans = Vec::new();
    if let Some(span) = span_at(journal, position).filter(|s| s.linked) {
        spans.push(span);
    }
    for delta in kinetic::NEIGHBORS {
        let p = kinetic::offset(position, delta);
        if let Some(span) = span_at(journal, p)
            && span.linked
            && (span.start == position || span.end == position)
            && !spans
                .iter()
                .any(|old: &Span| old.controller == span.controller)
        {
            spans.push(span);
        }
    }
    spans
}

pub fn segment_at(journal: &EditJournal, position: Cell) -> Option<Segment> {
    if !journal
        .block(position)
        .is_some_and(|id| matches!(id, 144..=146 | 148))
    {
        return None;
    }
    if let Some(span) = span_at(journal, position) {
        let index = if span.linked && span.start == position {
            -1
        } else if span.linked && span.end == position {
            i16::from(span.length)
        } else {
            span.cells().position(|p| p == position)? as i16
        };
        return Some(Segment { span, index });
    }
    let span = connected_at(journal, position).into_iter().next()?;
    Some(Segment {
        span,
        index: if span.start == position {
            -1
        } else {
            i16::from(span.length)
        },
    })
}

pub fn layout(
    chunks: &ChunkStore,
    journal: &EditJournal,
    start: Cell,
    end: Cell,
    item: ItemKind,
) -> Result<Layout, String> {
    if item.block_material() != Some(kinetic::CONVEYOR)
        || !item.valid()
        || start
            .iter()
            .chain(end.iter())
            .any(|p| p.unsigned_abs() > 990_000)
    {
        return Err("Invalid conveyor connector".into());
    }
    let a = shaft(journal, start).ok_or("Start needs a shaft")?;
    let b = shaft(journal, end).ok_or("End needs a parallel shaft")?;
    if !connected_at(journal, start).is_empty() || !connected_at(journal, end).is_empty() {
        return Err("Shaft already has a conveyor".into());
    }
    let delta: Cell = std::array::from_fn(|i| end[i] - start[i]);
    let distance = delta.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
    let axis = kinetic::axis(a.facing);
    if !(2..=u64::from(MAX_SEGMENTS) + 1).contains(&distance)
        || delta
            .iter()
            .any(|v| *v != 0 && v.unsigned_abs() != distance)
        || axis.map(i64::abs) != kinetic::axis(b.facing).map(i64::abs)
        || (0..3).map(|i| axis[i] * delta[i]).sum::<i64>() != 0
    {
        return Err("Belt needs a straight horizontal, vertical or 45-degree span".into());
    }
    let step = delta.map(i64::signum);
    let first = at(start, step, 1);
    let mut cells = Vec::new();
    for index in 0..distance - 1 {
        let p = at(first, step, index as i64);
        let expected = chunks.block_at(p).ok_or("Belt path is not loaded")?;
        if matches!(expected, 144..=146) {
            if journal.block(p) != Some(expected) {
                return Err("Intermediate shaft state is stale".into());
            }
            let guide = shaft(journal, p).ok_or("Invalid intermediate shaft")?;
            if guide.peer.is_some()
                || !connected_at(journal, p).is_empty()
                || kinetic::axis(guide.facing).map(i64::abs) != axis.map(i64::abs)
            {
                return Err("Intermediate shafts must be free and parallel".into());
            }
        } else if !matches!(expected, 0 | 27) {
            return Err("Belt path is obstructed".into());
        }
        cells.push((p, expected));
    }
    let controller = cells
        .iter()
        .find(|(_, material)| matches!(material, 0 | 27))
        .map(|(p, _)| *p)
        .unwrap_or(first);
    let wood = item.wood_species().unwrap_or_default();
    let mut result = Layout {
        edits: Vec::new(),
        states: Vec::new(),
        belts: vec![(controller, Some(BeltOwner { start, end, item }))],
    };
    for (p, expected) in cells {
        let guide = matches!(expected, 144..=146);
        result.edits.push(BlockEditRequest {
            position: p,
            expected_material: expected,
            material: if guide { expected } else { kinetic::CONVEYOR },
        });
        result.states.push((
            p,
            Some(if guide {
                BlockObjectState {
                    peer: Some(if p == controller { end } else { controller }),
                    ..shaft(journal, p).unwrap()
                }
            } else {
                BlockObjectState {
                    facing: kinetic::belt_pose(a.facing),
                    peer: Some(if p == controller { end } else { controller }),
                    secondary: false,
                    wood,
                }
            }),
        ));
    }
    for (p, state, peer) in [(start, a, controller), (end, b, start)] {
        result.edits.push(BlockEditRequest {
            position: p,
            expected_material: chunks.block_at(p).ok_or("Endpoint is not loaded")?,
            material: journal.block(p).ok_or("Missing shaft metadata")?,
        });
        result.states.push((
            p,
            Some(BlockObjectState {
                peer: Some(peer),
                ..state
            }),
        ));
    }
    result.edits.sort_by_key(|edit| edit.position != controller);
    if [start, end]
        .into_iter()
        .any(|p| chunks.block_at(p) != journal.block(p))
    {
        return Err("Belt endpoints are not loaded".into());
    }
    clearance(
        proposed_span(&result).ok_or("Invalid conveyor profile")?,
        chunks,
        journal,
    )?;
    Ok(result)
}

pub fn proposed_span(plan: &Layout) -> Option<Span> {
    span_with(
        plan.edits.first()?.position,
        |p| {
            plan.edits
                .iter()
                .find(|edit| edit.position == p)
                .map(|edit| edit.material)
        },
        |p| {
            plan.states
                .iter()
                .find(|(position, _)| *position == p)
                .and_then(|(_, state)| *state)
        },
        |p| {
            plan.belts
                .iter()
                .find(|(position, _)| *position == p)
                .and_then(|(_, owner)| *owner)
        },
    )
}

/// Installs a real shaft without replacing the belt or its separate item metadata.
pub fn insert_shaft_layout(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: Cell,
    shaft_item: ItemKind,
) -> Result<Layout, String> {
    let material = shaft_item
        .block_material()
        .filter(|id| matches!(id, 144..=146))
        .ok_or("A shaft is required")?;
    let span = span_at(journal, position)
        .filter(|span| span.linked)
        .ok_or("A linked conveyor is required")?;
    if !shaft_item.valid()
        || chunks.block_at(position) != Some(148)
        || !span.loaded(chunks, journal)
    {
        return Err("Conveyor shaft insertion changed".into());
    }
    let owner = BeltOwner {
        start: span.start,
        end: span.end,
        item: item(span, journal),
    };
    let mut plan = Layout {
        edits: Vec::new(),
        states: Vec::new(),
        belts: vec![(span.controller, Some(owner))],
    };
    for cell in span.cells().chain([span.start, span.end]) {
        let expected = chunks.block_at(cell).ok_or("Conveyor is not loaded")?;
        let state = journal.object_state(cell).ok_or("Missing conveyor state")?;
        plan.edits.push(BlockEditRequest {
            position: cell,
            expected_material: expected,
            material: if cell == position { material } else { expected },
        });
        let peer = if cell == span.start {
            span.controller
        } else if cell == span.end {
            span.start
        } else if cell == span.controller {
            span.end
        } else {
            span.controller
        };
        plan.states.push((
            cell,
            Some(if cell == position {
                BlockObjectState {
                    facing: span.facing,
                    secondary: false,
                    wood: shaft_item.wood_species().unwrap_or_default(),
                    peer: Some(peer),
                }
            } else {
                BlockObjectState {
                    peer: Some(peer),
                    ..state
                }
            }),
        ));
    }
    plan.edits
        .sort_by_key(|edit| edit.position != span.controller);
    clearance(
        proposed_span(&plan).ok_or("Invalid inserted shaft profile")?,
        chunks,
        journal,
    )?;
    Ok(plan)
}
pub fn boxes(span: Span) -> Vec<terra_core::block_shapes::ShapeBox> {
    (-1..=i16::from(span.length))
        .flat_map(|index| crate::interaction_shapes::conveyor_boxes(Segment { span, index }))
        .collect()
}
pub(crate) fn clearance(
    span: Span,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Result<(), String> {
    use terra_core::{block_properties::BlockPropertiesCatalog, block_shapes::ShapeBox};
    let shapes = boxes(span);
    let mut owners = std::collections::BTreeSet::new();
    for b in &shapes {
        let low = b.minimum.map(|v| v.floor() as i64);
        let high = b.maximum.map(|v| (v - 0.000001).floor() as i64);
        for x in low[0]..=high[0] {
            for y in low[1]..=high[1] {
                for z in low[2]..=high[2] {
                    let p = [x, y, z];
                    if chunks.block_at(p).is_none() {
                        return Err("Belt profile clearance is not loaded".into());
                    }
                    owners.insert(p);
                    journal.visit_extended_kinetics_near(p, |owner| {
                        owners.insert(owner);
                    });
                }
            }
        }
    }
    let properties = BlockPropertiesCatalog::default();
    for owner in owners {
        if owner == span.start || owner == span.end || span.cells().any(|p| p == owner) {
            continue;
        }
        let mut blocked = false;
        let modeled =
            crate::interaction_shapes::visit_boxes(chunks, journal, &properties, owner, |b| {
                blocked |= shapes.iter().any(|s| b.intersects(s.minimum, s.maximum));
            });
        if !modeled
            && chunks
                .block_at(owner)
                .and_then(terra_voxel::MaterialId::from_raw)
                .is_some_and(terra_voxel::MaterialId::is_solid)
        {
            let b = ShapeBox {
                minimum: owner.map(|v| v as f64),
                maximum: owner.map(|v| v as f64 + 1.0),
                hinged: false,
            };
            blocked |= shapes.iter().any(|s| b.intersects(s.minimum, s.maximum));
        }
        if blocked {
            return Err("Belt profile is obstructed".into());
        }
    }
    Ok(())
}

pub fn commit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    plan: &Layout,
) -> Result<bool, String> {
    if plan.edits.is_empty()
        || plan.edits.len() > usize::from(MAX_SEGMENTS) + 3
        || plan.states.len() != plan.edits.len()
        || plan.belts.len() > 1
    {
        return Err("Invalid conveyor transaction size".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for edit in &plan.edits {
        if !seen.insert(edit.position)
            || chunks.block_at(edit.position) != Some(edit.expected_material)
            || !(matches!(edit.material, 0 | 148)
                || matches!(edit.material, 144..=146)
                    && (edit.material == edit.expected_material || edit.expected_material == 148))
            || edit.position.iter().any(|p| p.unsigned_abs() > 990_000)
            || !plan.states.iter().any(|(p, state)| {
                *p == edit.position
                    && match state {
                        Some(state) => {
                            matches!(edit.material, 144..=146 | 148)
                                && state.valid_for(*p, edit.material)
                        }
                        None => edit.material == 0,
                    }
            })
        {
            return Err("Conveyor transaction changed".into());
        }
    }
    for (position, owner) in &plan.belts {
        if !seen.contains(position) || owner.is_some_and(|owner| !owner.valid_at(*position)) {
            return Err("Invalid conveyor owner transaction".into());
        }
    }
    if plan.belts.iter().any(|(_, owner)| owner.is_some()) && proposed_span(plan).is_none() {
        return Err("Invalid conveyor span transaction".into());
    }
    if plan
        .edits
        .iter()
        .any(|edit| edit.expected_material == 148 && matches!(edit.material, 144..=146))
        && !plan.belts.iter().any(|(_, owner)| owner.is_some())
    {
        return Err("Shaft insertion requires a complete conveyor span".into());
    }
    journal.commit_rotation(chunks, plan)
}

pub fn removal_layout(
    chunks: &ChunkStore,
    journal: &EditJournal,
    span: Span,
    removed_shaft: Option<Cell>,
) -> Result<Layout, String> {
    if !span.loaded(chunks, journal) {
        return Err("Conveyor span is not loaded".into());
    }
    let mut plan = Layout {
        edits: Vec::new(),
        states: Vec::new(),
        belts: vec![(span.controller, None)],
    };
    for position in span.cells().chain([span.start, span.end]) {
        let expected = chunks
            .block_at(position)
            .ok_or("Conveyor cell is not loaded")?;
        let remove = expected == 148 || removed_shaft == Some(position);
        plan.edits.push(BlockEditRequest {
            position,
            expected_material: expected,
            material: if remove { 0 } else { expected },
        });
        plan.states.push((
            position,
            if remove {
                None
            } else {
                Some(BlockObjectState {
                    peer: None,
                    ..journal
                        .object_state(position)
                        .ok_or("Missing shaft state")?
                })
            },
        ));
    }
    Ok(plan)
}

#[derive(Default)]
pub struct Transport {
    cursor: usize,
    routing: routing::Routing,
    infusion: infusion::Runtime,
}

mod infusion;
mod routing;
pub use infusion::{
    carrying as infusion_carrying, loaded as infusion_loaded, stabilize as stabilize_infusion_drop,
};
pub use infusion::{geometry as infusion_geometry, inferred as infusion_facing};
pub use routing::{carrying as routing_carrying, loaded as routing_loaded};

pub fn belt_speed(rpm: f32) -> f32 {
    rpm / 30.0
}

/// Fixed carrying pose, derived from the belt rather than persisted as another item state.
pub fn stabilize_carried_drop(drop: &mut terra_core::WorldDrop, segment: Segment) {
    let axis = kinetic::axis(segment.span.facing).map(|v| v as f64);
    let center = segment.center();
    let sideways = (0..3)
        .map(|i| (drop.position[i] - center[i]) * axis[i])
        .sum::<f64>();
    for (i, axis) in axis.into_iter().enumerate() {
        drop.position[i] -= sideways * axis;
    }
    if let Some(height) = segment.surface_height(drop.position) {
        drop.position[1] =
            height + 0.12 * (1.0 + segment.span.step[1].unsigned_abs() as f64) + 0.01;
    }
    drop.velocity = [0.0; 3];
    drop.angular_velocity = [0.0; 3];
    drop.rotation = [0.0, 0.0, 0.0, 1.0];
    drop.grounded = true;
}

/// Geometric target selection shared by the client and authority; callers verify occlusion.
pub fn carried_drop_hit(
    drops: &[terra_core::WorldDrop],
    journal: &EditJournal,
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
) -> Option<(u64, f64)> {
    if origin
        .iter()
        .chain(direction.iter())
        .any(|v| !v.is_finite())
        || !reach.is_finite()
    {
        return None;
    }
    drops
        .iter()
        .take(terra_core::MAX_WORLD_DROPS)
        .filter_map(|drop| {
            if carrying_segment(drop, journal).is_none()
                && crate::crushers::carrying(drop, journal).is_none()
                && crate::funnels::carrying(drop, journal).is_none()
                && routing_carrying(drop, journal).is_none()
                && infusion_carrying(drop, journal).is_none()
            {
                return None;
            }
            let bounds = terra_core::block_shapes::ShapeBox {
                minimum: drop.position.map(|v| v - 0.18),
                maximum: drop.position.map(|v| v + 0.18),
                hinged: false,
            };
            bounds
                .ray(origin, direction, reach)
                .map(|(distance, _)| (drop.id, distance))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
}

pub fn dye_layout(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: Cell,
    color: terra_core::beds::DyeColor,
) -> Result<Layout, String> {
    let span = segment_at(journal, position)
        .ok_or("No conveyor at target")?
        .span;
    if !span.loaded(chunks, journal) {
        return Err("Conveyor is not fully loaded".into());
    }
    let old_item = item(span, journal);
    let dyed_item = terra_core::tints::recolored(old_item, color).ok_or("Invalid belt color")?;
    if old_item == dyed_item {
        return Err("Conveyor already has this color".into());
    }
    let mut plan = Layout {
        edits: Vec::new(),
        states: Vec::new(),
        belts: if span.linked {
            vec![(
                span.controller,
                Some(BeltOwner {
                    start: span.start,
                    end: span.end,
                    item: dyed_item,
                }),
            )]
        } else {
            Vec::new()
        },
    };
    for position in span
        .cells()
        .chain(span.linked.then_some(span.start))
        .chain(span.linked.then_some(span.end))
    {
        let material = journal.block(position).ok_or("Missing conveyor cell")?;
        let mut state = journal
            .object_state(position)
            .ok_or("Missing conveyor state")?;
        if material == 148 {
            state.facing = kinetic::dyed_belt_facing(state.facing, color);
        }
        if span.linked {
            state.peer = Some(if position == span.start {
                span.controller
            } else if position == span.end {
                span.start
            } else if position == span.controller {
                span.end
            } else {
                span.controller
            });
        }
        plan.edits.push(BlockEditRequest {
            position,
            expected_material: material,
            material,
        });
        plan.states.push((position, Some(state)));
    }
    plan.edits
        .sort_by_key(|edit| edit.position != span.controller);
    Ok(plan)
}

/// Persistent support only: no source, loaded terrain or movement is required.
pub fn carrying_segment(drop: &terra_core::WorldDrop, journal: &EditJournal) -> Option<Segment> {
    if !journal.has_kinetics() || drop.position.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let x = drop.position[0].floor() as i64;
    let z = drop.position[2].floor() as i64;
    let base = (drop.position[1] - 0.75).floor() as i64;
    for y in [base, base + 1, base - 1] {
        let Some(segment) = segment_at(journal, [x, y, z]) else {
            continue;
        };
        let Some(height) = segment.surface_height(drop.position) else {
            continue;
        };
        let axis = kinetic::axis(segment.span.facing);
        let center = segment.center();
        let relative: [f64; 3] = std::array::from_fn(|i| drop.position[i] - center[i]);
        if (0..3)
            .map(|i| relative[i] * axis[i] as f64)
            .sum::<f64>()
            .abs()
            > 0.48
        {
            continue;
        }
        let axes = bevy_math::Mat3::from_quat(bevy_math::Quat::from_array(drop.rotation));
        let size = if !drop.grounded && matches!(drop.stack.item, ItemKind::Block(51..=53)) {
            0.46
        } else {
            0.12
        };
        let half = (axes.x_axis.abs() + axes.y_axis.abs() + axes.z_axis.abs()) * size;
        let sideways = (0..3)
            .map(|i| relative[i] * axis[i] as f64)
            .sum::<f64>()
            .abs();
        let side_half = (0..3)
            .map(|i| f64::from(half[i]) * axis[i].unsigned_abs() as f64)
            .sum::<f64>();
        if sideways > (0.32 + side_half).min(0.48) {
            continue;
        }
        let direction = segment.span.direction().map(f64::from);
        let start = if segment.span.linked {
            segment.span.start.map(|v| v as f64 + 0.5)
        } else {
            center
        };
        let along = (0..3)
            .map(|i| (drop.position[i] - start[i]) * direction[i])
            .sum::<f64>();
        let length = if segment.span.linked {
            f64::from(segment.span.length + 1)
                * segment
                    .span
                    .step
                    .iter()
                    .map(|v| (*v as f64).powi(2))
                    .sum::<f64>()
                    .sqrt()
        } else {
            0.0
        };
        let padding = if segment.span.linked { 0.125 } else { 0.5 };
        if !(-padding..=length + padding).contains(&along) {
            continue;
        }
        let normal = segment.span.normal().map(f64::from);
        // Use the continuous carrying plane, not the conservative collision slices' stairs.
        let support_half = f64::from(half[1])
            + (normal[0].abs() * f64::from(half[0]) + normal[2].abs() * f64::from(half[2]))
                / normal[1];
        let gap = drop.position[1] - height - support_half;
        if (-0.025..=0.06).contains(&gap) {
            return Some(segment);
        }
    }
    None
}
fn drop_belt(
    drop: &terra_core::WorldDrop,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Option<(Segment, f32)> {
    let segment = carrying_segment(drop, journal)?;
    if !segment.span.loaded(chunks, journal) {
        return None;
    }
    let p = segment.position();
    let state = journal.kinetic_state(p);
    if !state.running() || !state.rpm.is_finite() || state.rpm.abs() > f32::from(kinetic::MAX_RPM) {
        return None;
    }
    let raw = journal.object_state(p)?.facing;
    let facing = if journal.block(p) == Some(148) {
        kinetic::belt_facing(raw)
    } else {
        raw
    };
    let axis = kinetic::axis(segment.span.facing);
    let sign = (0..3)
        .map(|i| kinetic::axis(facing)[i] * axis[i])
        .sum::<i64>() as f32;
    Some((segment, state.rpm * sign))
}
fn drop_clear(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    center: [f64; 3],
    carrier: Span,
) -> bool {
    drop_clear_with_carriers(chunks, journal, properties, center, &[carrier], None)
}

fn drop_clear_with_carriers(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    center: [f64; 3],
    carriers: &[Span],
    port: Option<Cell>,
) -> bool {
    let low = center.map(|v| (v - 0.12).floor() as i64);
    let high = center.map(|v| (v + 0.12).floor() as i64);
    let mut owners = std::collections::BTreeSet::new();
    for x in low[0]..=high[0] {
        for y in low[1]..=high[1] {
            for z in low[2]..=high[2] {
                let cell = [x, y, z];
                if chunks.block_at(cell).is_none() {
                    return false;
                }
                owners.insert(cell);
                journal.visit_extended_kinetics_near(cell, |owner| {
                    owners.insert(owner);
                });
            }
        }
    }
    for owner in owners {
        if Some(owner) == port
            || carriers.iter().any(|carrier| {
                owner == carrier.start
                    || owner == carrier.end
                    || carrier.cells().any(|p| p == owner)
            })
        {
            // Exact support was checked above; its AABB slices must not stair-step the item.
            continue;
        }
        let mut blocked = false;
        let modeled = crate::interaction_shapes::visit_collision_boxes(
            chunks,
            journal,
            properties,
            owner,
            |bounds| {
                blocked |= bounds.intersects(center.map(|v| v - 0.12), center.map(|v| v + 0.12));
            },
        );
        if blocked
            || !modeled
                && chunks
                    .block_at(owner)
                    .and_then(terra_voxel::MaterialId::from_raw)
                    .is_none_or(terra_voxel::MaterialId::is_solid)
        {
            return false;
        }
    }
    true
}
// Preserve ownership on the incoming belt until the budgeted merger grants a turn.
fn merger_holds_entry(
    segment: Segment,
    point: [f64; 3],
    next: [f64; 3],
    journal: &EditJournal,
) -> bool {
    if !journal.has_logistics() || segment.span.step[1] != 0 {
        return false;
    }
    let from = [
        point[0].floor() as i64,
        segment.position()[1],
        point[2].floor() as i64,
    ];
    let to = [next[0].floor() as i64, from[1], next[2].floor() as i64];
    if from == to {
        return false;
    }
    let port = [to[0], to[1] + 1, to[2]];
    journal.block(port).is_some_and(|material| {
        terra_core::logistics::is_router(material)
            && journal
                .object_state(port)
                .is_some_and(|state| state.valid_for(port, material))
    })
}

#[allow(clippy::too_many_arguments)]
fn carry_on_belt(
    index: usize,
    segment: Segment,
    rpm: f32,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    elapsed: f32,
    occupancy: &mut DropIndex,
) -> bool {
    let mut carried = journal.gameplay().drops[index].clone();
    let original = carried.position;
    stabilize_carried_drop(&mut carried, segment);
    let mut point = carried.position;
    let start = point;
    let travel = belt_speed(rpm) * elapsed.min(0.15) * segment.span.flow_sign();
    let direction = segment.span.direction().map(f64::from);
    let steps = ((travel.abs() / 0.05).ceil() as usize).clamp(1, 26);
    let mut moved = false;
    for _ in 0..steps {
        let next: [f64; 3] =
            std::array::from_fn(|i| point[i] + direction[i] * f64::from(travel) / steps as f64);
        if merger_holds_entry(segment, point, next, journal)
            || infusion::holds_entry(segment, point, next, journal, carried.stack.item)
            || !crate::kingdom_access::can_move_drop(chunks, journal, point, next)
            || !drop_step_clear(occupancy, point, next, carried.id)
            || !drop_clear(chunks, journal, properties, next, segment.span)
        {
            break;
        }
        point = next;
        moved = true;
    }
    let velocity = if moved {
        std::array::from_fn(|i| ((point[i] - start[i]) / f64::from(elapsed)) as f32)
    } else {
        [0.0; 3]
    };
    if moved
        || journal.gameplay().drops[index].position != point
        || journal.gameplay().drops[index].velocity != velocity
    {
        move_indexed_drop(occupancy, carried.id, original, point);
        let drop = &mut journal.gameplay_mut().drops[index];
        drop.position = point;
        drop.velocity = velocity;
        drop.angular_velocity = [0.0; 3];
        drop.rotation = [0.0, 0.0, 0.0, 1.0];
        drop.grounded = true;
        return true;
    }
    false
}

/// The server and headless/native fixtures share the same bounded world-drop path.
pub fn transport(
    runtime: &mut Transport,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    elapsed: f32,
) {
    if !elapsed.is_finite() || elapsed <= 0.0 {
        runtime.cursor = 0;
        return;
    }
    let mut occupancy = index_drops(&journal.gameplay().drops);
    let mut routed = routing::advance(
        &mut runtime.routing,
        chunks,
        journal,
        properties,
        elapsed,
        &mut occupancy,
    );
    let station = infusion::advance(
        &mut runtime.infusion,
        chunks,
        journal,
        properties,
        elapsed,
        &mut occupancy,
        &routed,
    );
    routed.extend(station);
    let active: Vec<_> = journal
        .gameplay()
        .drops
        .iter()
        .take(terra_core::MAX_WORLD_DROPS)
        .enumerate()
        .filter(|(_, drop)| !routed.contains(&drop.id) && routing_carrying(drop, journal).is_none())
        .filter_map(|(i, drop)| drop_belt(drop, chunks, journal).map(|belt| (i, belt)))
        .collect();
    if active.is_empty() {
        runtime.cursor = 0;
        return;
    }
    let mut changed = false;
    for offset in 0..active.len().min(128) {
        let (index, (segment, rpm)) = active[(runtime.cursor + offset) % active.len()];
        changed |= carry_on_belt(
            index,
            segment,
            rpm,
            chunks,
            journal,
            properties,
            elapsed,
            &mut occupancy,
        );
    }

    runtime.cursor = (runtime.cursor + 128) % active.len();
    if changed {
        let game = journal.gameplay_mut();
        game.revision = game.revision.saturating_add(1);
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tint_item_tests {
    use super::*;

    #[test]
    fn reconstructed_belt_retains_legacy_color_rgb_override_and_frame_species() {
        let controller = [-17, 2, -1];
        let mut journal = EditJournal::default();
        journal.record(controller, kinetic::CONVEYOR).unwrap();
        let mut state = BlockObjectState {
            wood: WoodSpecies::Cherry,
            ..Default::default()
        };
        state.facing = kinetic::dyed_belt_facing(state.facing, terra_core::beds::DyeColor::Red);
        journal.set_object_state(controller, Some(state)).unwrap();
        let span = span_at(&journal, controller).unwrap();
        let legacy = item(span, &journal);
        assert_eq!(legacy.wood_species(), Some(WoodSpecies::Cherry));
        assert_eq!(
            legacy,
            terra_core::tints::recolored(
                ItemKind::for_wood(148, WoodSpecies::Cherry),
                terra_core::beds::DyeColor::Red
            )
            .unwrap()
        );
        for rgb in [[1, 90, 255], [0, 0, 0], [255, 255, 255]] {
            journal.set_tint(controller, Some(rgb)).unwrap();
            assert_eq!(
                item(span, &journal),
                ItemKind::TintedBlock {
                    material: 148,
                    wood: WoodSpecies::Cherry,
                    color: rgb
                }
            );
        }
    }
}
