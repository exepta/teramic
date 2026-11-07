use super::*;
use crate::block_shapes::ShapeBox;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartMaterial {
    Wood,
    Steel,
    SteelHead,
    Bolt,
    Red,
}

/// Shared box basis. Ramp extrusions retain a vertical cross section, so the
/// basis may include shear; consumers should mesh `corners` directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beam {
    pub center: [f64; 3],
    pub right: [f64; 3],
    pub up: [f64; 3],
    pub forward: [f64; 3],
    /// Half width, half height and half length in the corresponding local axes.
    pub half_extents: [f64; 3],
    pub material: PartMaterial,
    /// Optional start/end cross-section normals for watertight curve miters.
    pub cross_sections: Option<[[f64; 3]; 2]>,
}
impl Beam {
    pub fn translated(self, offset: [f64; 3]) -> Self {
        Self {
            center: add(self.center, offset),
            ..self
        }
    }
    pub fn bounds(self) -> ShapeBox {
        let corners = self.corners();
        ShapeBox {
            minimum: std::array::from_fn(|axis| {
                corners
                    .iter()
                    .map(|point| point[axis])
                    .fold(f64::INFINITY, f64::min)
            }),
            maximum: std::array::from_fn(|axis| {
                corners
                    .iter()
                    .map(|point| point[axis])
                    .fold(f64::NEG_INFINITY, f64::max)
            }),
            hinged: false,
        }
    }
    /// Eight corners ordered by the three sign bits of width, height and length.
    pub fn corners(self) -> [[f64; 3]; 8] {
        std::array::from_fn(|index| {
            let mut point = self.center;
            let right = self
                .cross_sections
                .map_or(self.right, |sections| sections[usize::from(index & 4 != 0)]);
            for (axis, direction) in [right, self.up, self.forward].into_iter().enumerate() {
                point = add(
                    point,
                    mul(
                        direction,
                        self.half_extents[axis] * if index & (1 << axis) == 0 { -1.0 } else { 1.0 },
                    ),
                );
            }
            point
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub position: [f64; 3],
    pub tangent: [f64; 3],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupportPoint {
    pub cell: [i32; 3],
    pub support: [i32; 3],
}

pub(super) struct Layout {
    pub beams: Arc<[Beam]>,
    pub footprint: Arc<[[i32; 3]]>,
    pub cells: BTreeMap<[i32; 3], Arc<[ShapeBox]>>,
    pub supports: Arc<[SupportPoint]>,
    pub surfaces: OnceLock<Arc<[Surface]>>,
}
static LAYOUTS: [OnceLock<Arc<Layout>>; 80] = [const { OnceLock::new() }; 80];
type DynamicEntries = VecDeque<(CurveSpec, Arc<Layout>)>;
static DYNAMIC: OnceLock<Mutex<DynamicEntries>> = OnceLock::new();

pub(super) fn layout(material: u16, facing: u8) -> Option<Arc<Layout>> {
    if !valid_facing(material, facing) {
        return None;
    }
    let index = usize::from(material - STRAIGHT) * 8 + usize::from(facing);
    Some(
        LAYOUTS[index]
            .get_or_init(|| {
                Arc::new(make_layout(
                    Piece::from_facing([0; 3], material, facing).expect("validated rail pose"),
                ))
            })
            .clone(),
    )
}
pub(super) fn piece_layout(piece: Piece) -> Option<Arc<Layout>> {
    if !piece.valid() {
        return None;
    }
    let Some(curve) = piece.curve else {
        return layout(piece.material, piece.encoded_facing());
    };
    let cache = DYNAMIC.get_or_init(|| Mutex::new(VecDeque::new()));
    {
        let mut entries = cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(index) = entries.iter().position(|(spec, _)| *spec == curve) {
            let entry = entries.remove(index).expect("located rail layout");
            let layout = entry.1.clone();
            entries.push_back(entry);
            return Some(layout);
        }
    }
    let layout = Arc::new(make_layout(Piece {
        anchor: [0; 3],
        ..piece
    }));
    let mut entries = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((_, previous)) = entries.iter().find(|(spec, _)| *spec == curve) {
        return Some(previous.clone());
    }
    if entries.len() >= DYNAMIC_CACHE_LIMIT {
        entries.pop_front();
    }
    entries.push_back((curve, layout.clone()));
    Some(layout)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PathShape {
    Line { start: [f64; 3], end: [f64; 3] },
    Curve { radius: f64, side: f64 },
    Entry,
    Exit,
    Bezier(CurveSpec),
}

pub(super) fn paths(piece: Piece) -> Vec<Path> {
    if !piece.valid() {
        return vec![];
    }
    if let Some(curve) = piece.curve {
        return vec![Path {
            piece,
            index: 0,
            ports: [0, 1],
            shape: PathShape::Bezier(curve),
        }];
    }
    let line = |length, rise| PathShape::Line {
        start: [0.5, DECK_HEIGHT, 0.0],
        end: [0.5, DECK_HEIGHT + rise, length],
    };
    let specifications = match piece.kind().expect("validated rail kind") {
        Kind::Straight | Kind::Buffer => vec![([0, 1], line(4.0, 0.0))],
        Kind::CurveSmall => vec![(
            [0, 1],
            PathShape::Curve {
                radius: 4.5,
                side: 1.0,
            },
        )],
        Kind::CurveLarge => vec![(
            [0, 1],
            PathShape::Curve {
                radius: 8.5,
                side: 1.0,
            },
        )],
        Kind::Slope => vec![([0, 1], line(4.0, 1.0))],
        Kind::SlopeEntry => vec![([0, 1], PathShape::Entry)],
        Kind::SlopeExit => vec![([0, 1], PathShape::Exit)],
        Kind::SwitchLeft | Kind::SwitchRight => vec![
            ([0, 1], line(8.0, 0.0)),
            (
                [0, 2],
                PathShape::Curve {
                    radius: 4.5,
                    side: if piece.material == SWITCH_LEFT {
                        -1.0
                    } else {
                        1.0
                    },
                },
            ),
        ],
        Kind::Crossing => vec![
            ([0, 1], line(7.0, 0.0)),
            (
                [2, 3],
                PathShape::Line {
                    start: [-3.0, DECK_HEIGHT, 3.5],
                    end: [4.0, DECK_HEIGHT, 3.5],
                },
            ),
        ],
    };
    specifications
        .into_iter()
        .enumerate()
        .map(|(index, (ports, shape))| Path {
            piece,
            index: index as u8,
            ports,
            shape,
        })
        .collect()
}

fn local_sample(shape: PathShape, t: f64) -> Sample {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    match shape {
        PathShape::Bezier(curve) => curve.sample(t),
        PathShape::Line { start, end } => Sample {
            position: add(start, mul(sub(end, start), t)),
            tangent: normalized(sub(end, start)),
        },
        PathShape::Curve { radius, side } => {
            let angle = t * std::f64::consts::FRAC_PI_2;
            Sample {
                position: [
                    0.5 + side * radius * (1.0 - angle.cos()),
                    DECK_HEIGHT,
                    radius * angle.sin(),
                ],
                tangent: [side * angle.sin(), 0.0, angle.cos()],
            }
        }
        PathShape::Entry => Sample {
            position: [0.5, DECK_HEIGHT + t * t, 8.0 * t],
            tangent: normalized([0.0, t * 0.25, 1.0]),
        },
        PathShape::Exit => Sample {
            position: [0.5, DECK_HEIGHT + 2.0 * t - t * t, 8.0 * t],
            tangent: normalized([0.0, (1.0 - t) * 0.25, 1.0]),
        },
    }
}
pub(super) fn sample(path: Path, t: f64) -> Sample {
    let local = local_sample(path.shape, t);
    Sample {
        position: add(
            rotate_point(local.position, path.piece.facing),
            path.piece.anchor.map(|value| value as f64),
        ),
        tangent: rotate_vector(local.tangent, path.piece.facing),
    }
}
pub(super) fn path_length(path: Path) -> f64 {
    match path.shape {
        PathShape::Bezier(curve) => curve.length().unwrap_or(0.0),
        PathShape::Line { start, end } => length(sub(end, start)),
        PathShape::Curve { radius, .. } => radius * std::f64::consts::FRAC_PI_2,
        PathShape::Entry | PathShape::Exit => {
            let mut previous = path.sample(0.0).position;
            let mut total = 0.0;
            for index in 1..=64 {
                let current = path.sample(index as f64 / 64.0).position;
                total += length(sub(current, previous));
                previous = current;
            }
            total
        }
    }
}

pub(super) fn ports(piece: Piece) -> Vec<Port> {
    if !piece.valid() {
        return vec![];
    }
    if let Some(curve) = piece.curve {
        return [false, true]
            .into_iter()
            .enumerate()
            .map(|(index, end)| Port {
                index: index as u8,
                position: FixedPoint(std::array::from_fn(|axis| {
                    piece.anchor[axis] * FIXED_SCALE
                        + i64::from(curve.points[if end { 3 } else { 0 }][axis])
                })),
                tangent: curve
                    .endpoint_tangent(end)
                    .expect("validated endpoint tangent"),
                connectable: true,
            })
            .collect();
    }
    let canonical = match piece.kind().expect("validated rail kind") {
        Kind::Straight | Kind::Buffer => vec![([8, 2, 0], [0, 0, -1]), ([8, 2, 64], [0, 0, 1])],
        Kind::CurveSmall => vec![([8, 2, 0], [0, 0, -1]), ([80, 2, 72], [1, 0, 0])],
        Kind::CurveLarge => vec![([8, 2, 0], [0, 0, -1]), ([144, 2, 136], [1, 0, 0])],
        Kind::Slope => vec![([8, 2, 0], [0, -1, -4]), ([8, 18, 64], [0, 1, 4])],
        Kind::SlopeEntry => vec![([8, 2, 0], [0, 0, -1]), ([8, 18, 128], [0, 1, 4])],
        Kind::SlopeExit => vec![([8, 2, 0], [0, -1, -4]), ([8, 18, 128], [0, 0, 1])],
        Kind::SwitchLeft => vec![
            ([8, 2, 0], [0, 0, -1]),
            ([8, 2, 128], [0, 0, 1]),
            ([-64, 2, 72], [-1, 0, 0]),
        ],
        Kind::SwitchRight => vec![
            ([8, 2, 0], [0, 0, -1]),
            ([8, 2, 128], [0, 0, 1]),
            ([80, 2, 72], [1, 0, 0]),
        ],
        Kind::Crossing => vec![
            ([8, 2, 0], [0, 0, -1]),
            ([8, 2, 112], [0, 0, 1]),
            ([-48, 2, 56], [-1, 0, 0]),
            ([64, 2, 56], [1, 0, 0]),
        ],
    };
    canonical
        .into_iter()
        .enumerate()
        .map(|(index, (position, tangent))| {
            let [x, y, z] = position;
            let rotated = match piece.facing {
                0 => [x, y, z],
                1 => [z, y, 16 - x],
                2 => [16 - x, y, 16 - z],
                _ => [16 - z, y, x],
            };
            let tangent_f64 = rotate_vector(tangent.map(f64::from), piece.facing);
            Port {
                index: index as u8,
                position: FixedPoint(std::array::from_fn(|axis| {
                    piece.anchor[axis] * FIXED_SCALE + rotated[axis]
                })),
                tangent: tangent_f64.map(|value| value as i16),
                connectable: piece.material != BUFFER || index == 0,
            }
        })
        .collect()
}

fn make_layout(piece: Piece) -> Layout {
    let mut beams = make_beams(piece);
    for beam in &mut beams {
        beam.center = rotate_point(beam.center, piece.facing);
        beam.right = rotate_vector(beam.right, piece.facing);
        beam.up = rotate_vector(beam.up, piece.facing);
        beam.forward = rotate_vector(beam.forward, piece.facing);
        beam.cross_sections = beam
            .cross_sections
            .map(|sections| sections.map(|right| rotate_vector(right, piece.facing)));
    }
    let mut cells: BTreeMap<[i32; 3], Vec<ShapeBox>> = BTreeMap::new();
    for beam in &beams {
        let bounds = beam.bounds();
        let low = bounds.minimum.map(|value| (value + 1.0e-8).floor() as i32);
        let high = bounds.maximum.map(|value| (value - 1.0e-8).floor() as i32);
        for x in low[0]..=high[0] {
            for y in low[1]..=high[1] {
                for z in low[2]..=high[2] {
                    let cell = [x, y, z];
                    let clipped = ShapeBox {
                        minimum: std::array::from_fn(|axis| {
                            (bounds.minimum[axis] - f64::from(cell[axis])).max(0.0)
                        }),
                        maximum: std::array::from_fn(|axis| {
                            (bounds.maximum[axis] - f64::from(cell[axis])).min(1.0)
                        }),
                        hinged: false,
                    };
                    if clipped.valid() {
                        cells.entry(cell).or_default().push(clipped);
                    }
                }
            }
        }
    }
    if piece.kind().is_some_and(Kind::is_switch) {
        for mut beam in legacy_switch_lever(Piece { facing: 0, ..piece }) {
            beam.center = rotate_point(beam.center, piece.facing);
            beam.right = rotate_vector(beam.right, piece.facing);
            beam.up = rotate_vector(beam.up, piece.facing);
            beam.forward = rotate_vector(beam.forward, piece.facing);
            let bounds = beam.bounds();
            let low = bounds.minimum.map(|v| (v + 1e-8).floor() as i32);
            let high = bounds.maximum.map(|v| (v - 1e-8).floor() as i32);
            for x in low[0]..=high[0] {
                for y in low[1]..=high[1] {
                    for z in low[2]..=high[2] {
                        cells.entry([x, y, z]).or_default();
                    }
                }
            }
        }
    }
    let footprint = cells.keys().copied().collect::<Vec<_>>();
    let mut ground = BTreeMap::new();
    for &[x, y, z] in &footprint {
        if cells.get(&[x, y, z]).is_none_or(Vec::is_empty) {
            continue;
        }
        ground
            .entry([x, z])
            .and_modify(|height: &mut i32| *height = (*height).min(y))
            .or_insert(y);
    }
    let supports = ground
        .into_iter()
        .map(|([x, z], y)| SupportPoint {
            cell: [x, y, z],
            support: [x, y - 1, z],
        })
        .collect::<Vec<_>>();
    Layout {
        beams: beams.into(),
        footprint: footprint.into(),
        cells: cells
            .into_iter()
            .map(|(cell, boxes)| (cell, boxes.into()))
            .collect(),
        supports: supports.into(),
        surfaces: OnceLock::new(),
    }
}

fn make_beams(piece: Piece) -> Vec<Beam> {
    let canonical = Piece { facing: 0, ..piece };
    let mut beams = Vec::new();
    let paths = canonical.paths();
    let mut tie_centers: Vec<[f64; 3]> = Vec::new();
    let combined_ties =
        canonical.kind().is_some_and(Kind::is_switch) || canonical.material == CROSSING;
    let mut tie_candidates = Vec::new();
    for path in &paths {
        let path_length = path.length();
        let parameters: Arc<[f64]> = match path.shape {
            PathShape::Bezier(curve) => curve.parameters().expect("validated curve samples"),
            _ => {
                let steps = (path_length * 4.0).ceil() as usize;
                (0..=steps)
                    .map(|index| index as f64 / steps as f64)
                    .collect::<Vec<_>>()
                    .into()
            }
        };
        let start = if canonical.kind().is_some_and(Kind::is_switch)
            && path.index != u8::from(canonical.switched)
        {
            (1.25 / path_length).min(0.5)
        } else {
            0.0
        };
        for parameters in parameters.windows(2) {
            let [t0, t1] = [parameters[0], parameters[1]];
            if t0 < start {
                continue;
            }
            let first = path.sample(t0);
            let second = path.sample(t1);
            let middle = path.sample((t0 + t1) * 0.5);
            if canonical.material == CROSSING
                && path.index == 0
                && [2.5, 4.5]
                    .into_iter()
                    .any(|crossing| (middle.position[2] - crossing).abs() < 0.2)
            {
                continue;
            }
            let right = normalized([middle.tangent[2], 0.0, -middle.tangent[0]]);
            let up = [0.0, 1.0, 0.0];
            for side in [-1.0, 1.0] {
                let first_right = normalized([first.tangent[2], 0.0, -first.tangent[0]]);
                let second_right = normalized([second.tangent[2], 0.0, -second.tangent[0]]);
                let rail_start = add(first.position, mul(first_right, side));
                let rail_end = add(second.position, mul(second_right, side));
                let forward = normalized(sub(rail_end, rail_start));
                for (bottom, top, width, material) in [
                    (0.0, 0.03125, 0.18, PartMaterial::Steel),
                    (0.03125, 0.140625, 0.065, PartMaterial::Steel),
                    (0.140625, RAIL_HEIGHT, 0.115, PartMaterial::SteelHead),
                ] {
                    beams.push(Beam {
                        center: add(
                            mul(add(rail_start, rail_end), 0.5),
                            mul(up, (bottom + top) * 0.5),
                        ),
                        right,
                        up,
                        forward,
                        half_extents: [
                            width * 0.5,
                            (top - bottom) * 0.5,
                            length(sub(rail_end, rail_start)) * 0.5,
                        ],
                        material,
                        cross_sections: matches!(
                            path.shape,
                            PathShape::Curve { .. } | PathShape::Bezier(_)
                        )
                        .then_some([first_right, second_right]),
                    });
                }
            }
        }
        let ties = path_length.floor() as usize;
        for index in 0..ties {
            let sample = path.sample_distance(index as f64 + 0.5);
            if tie_centers
                .iter()
                .any(|center| length(sub(*center, sample.position)) < 0.65)
            {
                continue;
            }
            tie_centers.push(sample.position);
            let right = normalized([sample.tangent[2], 0.0, -sample.tangent[0]]);
            let up = [0.0, 1.0, 0.0];
            let tie = Beam {
                center: sub(sample.position, mul(up, 0.0625)),
                right,
                up,
                forward: sample.tangent,
                half_extents: [1.45, 0.0625, 0.16],
                material: PartMaterial::Wood,
                cross_sections: None,
            };
            if combined_ties {
                tie_candidates.push(tie);
                continue;
            }
            beams.push(tie);
            for side in [-1.22, -0.78, 0.78, 1.22] {
                beams.push(Beam {
                    center: add(add(sample.position, mul(right, side)), mul(up, 0.025)),
                    right,
                    up,
                    forward: sample.tangent,
                    half_extents: [0.035, 0.025, 0.035],
                    material: PartMaterial::Bolt,
                    cross_sections: None,
                });
            }
        }
    }
    if combined_ties {
        let rows = if canonical.material == CROSSING { 7 } else { 8 };
        for row in 0..rows {
            let z = f64::from(row) + 0.5;
            let mut span: Option<(f64, f64)> = None;
            for tie in &tie_candidates {
                let bounds = tie.bounds();
                if bounds.minimum[2] < z + 0.16 && bounds.maximum[2] > z - 0.16 {
                    span = Some(
                        span.map_or((bounds.minimum[0], bounds.maximum[0]), |(low, high)| {
                            (low.min(bounds.minimum[0]), high.max(bounds.maximum[0]))
                        }),
                    );
                }
            }
            if let Some((low, high)) = span {
                beams.push(axis_beam(
                    [(low + high) * 0.5, 0.0625, z],
                    [(high - low) * 0.5, 0.0625, 0.16],
                    PartMaterial::Wood,
                ));
                for path in &paths {
                    if path.index > 0 && canonical.material == CROSSING {
                        continue;
                    }
                    let t = match path.shape {
                        PathShape::Curve { radius, .. } if z <= radius => {
                            (z / radius).asin() / std::f64::consts::FRAC_PI_2
                        }
                        PathShape::Line { start, end } if end[2] > start[2] => {
                            (z - start[2]) / (end[2] - start[2])
                        }
                        _ => continue,
                    };
                    if !(0.0..=1.0).contains(&t) {
                        continue;
                    }
                    let sample = path.sample(t);
                    for x in [sample.position[0] - 1.22, sample.position[0] + 1.22] {
                        if x > low && x < high {
                            beams.push(axis_beam(
                                [x, DECK_HEIGHT + 0.025, z],
                                [0.035, 0.025, 0.035],
                                PartMaterial::Bolt,
                            ));
                        }
                    }
                }
            }
        }
    }
    if canonical.kind().is_some_and(Kind::is_switch) {
        beams.push(legacy_switch_lever(canonical)[0]);
    }
    if canonical.material == BUFFER {
        for x in [-0.5, 1.5] {
            beams.push(axis_beam(
                [x, 0.5, 3.65],
                [0.12, 0.5, 0.12],
                PartMaterial::Steel,
            ));
        }
        beams.push(axis_beam(
            [0.5, 0.85, 3.65],
            [1.4, 0.16, 0.18],
            PartMaterial::Wood,
        ));
        for x in [-0.5, 1.5] {
            beams.push(axis_beam(
                [x, 0.85, 3.42],
                [0.22, 0.13, 0.08],
                PartMaterial::Red,
            ));
        }
    }
    beams
}

// Historical ownership includes the former lever, but its moving solids now
// belong to the separately placed switch stand. Never resurrect its collision.
fn legacy_switch_lever(canonical: Piece) -> Vec<Beam> {
    let mut beams = Vec::new();
    let side = if canonical.material == SWITCH_LEFT {
        1.0
    } else {
        -1.0
    };
    beams.push(axis_beam(
        [0.5 + side * 1.8, 0.15, 1.0],
        [0.2, 0.15, 0.25],
        PartMaterial::Steel,
    ));
    let lean = if canonical.switched { 0.35 } else { -0.35 };
    let forward = normalized([lean, 0.65, 0.0]);
    beams.push(Beam {
        center: [0.5 + side * 1.8 + lean * 0.5, 0.6, 1.0],
        right: [0.0, 0.0, 1.0],
        up: normalized(cross(forward, [0.0, 0.0, 1.0])),
        forward,
        half_extents: [0.045, 0.045, 0.4],
        material: PartMaterial::Steel,
        cross_sections: None,
    });
    beams.push(axis_beam(
        [0.5 + side * 1.8 + lean, 0.9, 1.0],
        [0.12, 0.10, 0.12],
        PartMaterial::Red,
    ));
    beams
}

fn axis_beam(center: [f64; 3], half_extents: [f64; 3], material: PartMaterial) -> Beam {
    Beam {
        center,
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        forward: [0.0, 0.0, 1.0],
        half_extents,
        material,
        cross_sections: None,
    }
}
fn rotate_point(point: [f64; 3], facing: u8) -> [f64; 3] {
    let [x, y, z] = point;
    match facing {
        0 => [x, y, z],
        1 => [z, y, 1.0 - x],
        2 => [1.0 - x, y, 1.0 - z],
        _ => [1.0 - z, y, x],
    }
}
fn rotate_vector(vector: [f64; 3], facing: u8) -> [f64; 3] {
    let [x, y, z] = vector;
    match facing {
        0 => [x, y, z],
        1 => [z, y, -x],
        2 => [-x, y, -z],
        _ => [-z, y, x],
    }
}
fn add(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| first[axis] + second[axis])
}
fn sub(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| first[axis] - second[axis])
}
fn mul(vector: [f64; 3], scale: f64) -> [f64; 3] {
    vector.map(|value| value * scale)
}
fn length(vector: [f64; 3]) -> f64 {
    vector
        .into_iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt()
}
fn normalized(vector: [f64; 3]) -> [f64; 3] {
    mul(vector, 1.0 / length(vector))
}
fn cross(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
}
