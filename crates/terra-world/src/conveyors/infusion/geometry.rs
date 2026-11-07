use super::*;

pub(super) fn hold(position: Cell, journal: &EditJournal) -> Option<u8> {
    (journal.block(position) == Some(infusion::HOLD_BLOCK))
        .then(|| journal.object_state(position))
        .flatten()
        .filter(|state| state.valid_for(position, infusion::HOLD_BLOCK))
        .map(|state| state.facing)
}

pub(super) fn center(p: Cell) -> [f64; 3] {
    [
        p[0] as f64 + 0.5,
        p[1] as f64 + infusion::DROP_HEIGHT,
        p[2] as f64 + 0.5,
    ]
}

pub(super) fn dot(v: [f64; 3], axis: Cell) -> f64 {
    (0..3).map(|i| v[i] * axis[i] as f64).sum()
}

pub(super) fn relative(point: [f64; 3], position: Cell, facing: u8) -> (f64, f64) {
    let d = kinetic::forward(facing);
    let offset = std::array::from_fn(|i| point[i] - center(position)[i]);
    (
        dot(offset, d),
        offset[0] * d[2] as f64 - offset[2] * d[0] as f64,
    )
}

fn neighbor(position: Cell, facing: u8, journal: &EditJournal) -> Option<Segment> {
    let direction = kinetic::forward(facing);
    let segment = segment_at(journal, std::array::from_fn(|i| position[i] + direction[i]))?;
    (segment.span.step[1] == 0
        && dot(segment.span.direction().map(f64::from), direction).abs() > 0.999
        && (segment.drop_center()?[1] - center(position)[1]).abs() < 0.001)
        .then_some(segment)
}

fn velocity(segment: Segment, journal: &EditJournal) -> Option<[f64; 3]> {
    let p = segment.position();
    let drive = journal.kinetic_state(p);
    if !drive.running() || !drive.rpm.is_finite() || drive.rpm.abs() > f32::from(kinetic::MAX_RPM) {
        return None;
    }
    let raw = journal.object_state(p)?.facing;
    let raw = if journal.block(p) == Some(kinetic::CONVEYOR) {
        kinetic::belt_facing(raw)
    } else {
        raw
    };
    let axis = kinetic::axis(segment.span.facing);
    let sign = (0..3).map(|i| kinetic::axis(raw)[i] * axis[i]).sum::<i64>() as f32;
    let magnitude = belt_speed(drive.rpm * sign) * segment.span.flow_sign();
    Some(segment.span.direction().map(|v| f64::from(v * magnitude)))
}

/// Geometry supplies the axis; the adjoining belts' actual motion supplies its sign.
pub fn inferred(journal: &EditJournal, position: Cell, preferred: u8) -> (u8, u8) {
    let preferred = preferred % 4;
    let neighbors: [Option<Segment>; 4] =
        std::array::from_fn(|i| neighbor(position, i as u8, journal));
    let connections = neighbors.iter().enumerate().fold(0, |mask, (i, segment)| {
        mask | (u8::from(segment.is_some()) << i)
    });
    let score = |axis: usize| {
        let first = neighbors[axis].is_some();
        let second = neighbors[axis + 2].is_some();
        u8::from(first) + u8::from(second) + 4 * u8::from(first && second)
    };
    let base = match score(0).cmp(&score(1)) {
        std::cmp::Ordering::Greater => 0,
        std::cmp::Ordering::Less => 1,
        std::cmp::Ordering::Equal => preferred % 2,
    };
    let direction = kinetic::forward(base);
    let signed: f64 = [base, base + 2]
        .into_iter()
        .filter_map(|facing| neighbors[usize::from(facing)])
        .filter_map(|segment| velocity(segment, journal))
        .map(|v| dot(v, direction))
        .sum();
    let facing = if signed > 0.05 {
        base
    } else if signed < -0.05 {
        base + 2
    } else {
        // Stopping a drive must not exchange the upstream and downstream gates.
        journal
            .infusion_view(position)
            .filter(|view| view.facing % 2 == base)
            .map_or(base + (preferred / 2) * 2, |view| view.facing)
    };
    (facing, connections)
}

pub fn geometry(position: Cell, journal: &EditJournal) -> Option<infusion::HoldVisual> {
    let preferred = hold(position, journal)?;
    let (facing, connections) = inferred(journal, position, preferred);
    let speed = [facing, (facing + 2) % 4]
        .into_iter()
        .filter_map(|side| neighbor(position, side, journal))
        .filter_map(|segment| velocity(segment, journal))
        .map(|v| dot(v, kinetic::forward(facing)).max(0.) as f32)
        .fold(0., f32::max);
    Some(infusion::HoldVisual {
        position,
        facing,
        connections,
        speed,
        ..Default::default()
    })
}

pub fn loaded(position: Cell, chunks: &ChunkStore, journal: &EditJournal) -> bool {
    chunks.block_at(position) == Some(infusion::HOLD_BLOCK) && hold(position, journal).is_some()
}

/// Persisted support remains valid with stopped belts and across section eviction.
pub fn carrying(drop: &WorldDrop, journal: &EditJournal) -> Option<Cell> {
    if !journal.has_holds()
        || drop
            .position
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1_000_001.0)
        || drop.velocity[1] > 0.5
    {
        return None;
    }
    let [x, y, z] = drop.position.map(|v| v.floor() as i64);
    for y in [y, y - 1] {
        for [dx, dz] in [[0, 0], [-1, 0], [1, 0], [0, -1], [0, 1]] {
            let p = [x + dx, y, z + dz];
            let Some(g) = geometry(p, journal) else {
                continue;
            };
            if (drop.position[1] - center(p)[1]).abs() > 0.08 {
                continue;
            }
            let (along, across) = relative(drop.position, p, g.facing);
            let own = (-0.500001..=0.500001).contains(&along) && across.abs() <= 0.43;
            let front = (0.5..0.999999).contains(&along) && g.connections & (1 << g.facing) != 0;
            let back = (-0.999999..=-0.5).contains(&along)
                && g.connections & (1 << ((g.facing + 2) % 4)) != 0;
            if own || across.abs() <= 0.13 && (front || back) {
                return Some(p);
            }
        }
    }
    None
}

pub fn stabilize(drop: &mut WorldDrop, p: Cell) {
    drop.position[1] = center(p)[1];
    drop.velocity = [0.; 3];
    drop.angular_velocity = [0.; 3];
    drop.rotation = [0., 0., 0., 1.];
    drop.grounded = true;
}

pub(super) fn drive(
    position: Cell,
    facing: u8,
    inlet: bool,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Option<(Segment, f64)> {
    let segment = neighbor(
        position,
        if inlet { (facing + 2) % 4 } else { facing },
        journal,
    )?;
    if !segment.span.loaded(chunks, journal) {
        return None;
    }
    let speed = dot(velocity(segment, journal)?, kinetic::forward(facing));
    (speed > 0.05).then_some((segment, speed))
}

pub(super) fn candidate(
    drop: &WorldDrop,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Option<Cell> {
    if let Some(p) = carrying(drop, journal) {
        return Some(p);
    }
    let (segment, _) = drop_belt(drop, chunks, journal)?;
    if segment.span.step[1] != 0 {
        return None;
    }
    let base = [
        drop.position[0].floor() as i64,
        segment.position()[1],
        drop.position[2].floor() as i64,
    ];
    for direction in [
        kinetic::forward(0),
        kinetic::forward(1),
        kinetic::forward(2),
        kinetic::forward(3),
    ] {
        let p = std::array::from_fn(|i| base[i] + direction[i]);
        let Some(g) = geometry(p, journal) else {
            continue;
        };
        let (along, across) = relative(drop.position, p, g.facing);
        if (-1.3..=-0.5).contains(&along)
            && across.abs() <= 0.13
            && (drop.position[1] - center(p)[1]).abs() <= 0.08
            && drive(p, g.facing, true, chunks, journal).is_some()
        {
            return Some(p);
        }
    }
    None
}

/// Departed items still share the station's output queue. Move them before its followers.
pub(super) fn departure(
    drop: &WorldDrop,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Option<(Segment, f32, f64)> {
    let (segment, rpm) = drop_belt(drop, chunks, journal)?;
    if segment.span.step[1] != 0 || candidate(drop, chunks, journal).is_some() {
        return None;
    }
    let motion = segment
        .span
        .direction()
        .map(|v| f64::from(v * belt_speed(rpm) * segment.span.flow_sign()));
    let facing = (0..4).find(|&f| {
        dot(motion, kinetic::forward(f)) > 0.05
            && dot(segment.span.direction().map(f64::from), kinetic::forward(f)).abs() > 0.999
    })?;
    let direction = kinetic::forward(facing);
    for endpoint in [segment.span.start, segment.span.end] {
        let hold = std::array::from_fn(|i| endpoint[i] - direction[i]);
        let Some(g) = geometry(hold, journal) else {
            continue;
        };
        let Some(output) = neighbor(hold, facing, journal) else {
            continue;
        };
        if g.facing == facing
            && output.span.controller == segment.span.controller
            && output.span.start == segment.span.start
            && output.span.end == segment.span.end
        {
            let along = relative(drop.position, hold, facing).0;
            if along >= 0.999999 {
                return Some((segment, rpm, along));
            }
        }
    }
    None
}

/// Normal belt substeps may approach the gate, but never run through an unclaimed station.
pub(in crate::conveyors) fn holds_entry(
    segment: Segment,
    from: [f64; 3],
    to: [f64; 3],
    journal: &EditJournal,
    item: ItemKind,
) -> bool {
    if !journal.has_holds() || segment.span.step[1] != 0 {
        return false;
    }
    let base = [
        to[0].floor() as i64,
        segment.position()[1],
        to[2].floor() as i64,
    ];
    for direction in [
        kinetic::forward(0),
        kinetic::forward(1),
        kinetic::forward(2),
        kinetic::forward(3),
    ] {
        let p = std::array::from_fn(|i| base[i] + direction[i]);
        let Some(g) = geometry(p, journal) else {
            continue;
        };
        let requires_gate = journal.infusion_view(p).is_some_and(|view| view.gates)
            || super::mounted(p, journal) && infusion::fillable(item);
        if !requires_gate {
            continue;
        }
        let (old, _) = relative(from, p, g.facing);
        let (next, across) = relative(to, p, g.facing);
        if old < super::ENTRY
            && next >= super::ENTRY
            && across.abs() <= 0.43
            && (to[1] - center(p)[1]).abs() <= 0.08
        {
            return true;
        }
    }
    false
}
