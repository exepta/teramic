//! Bounded, deterministic one-click rail docking from an actual world hit.
use super::{Cell, cells, complete, owner_at, placement};
use crate::{edits::EditJournal, placement::Layout, streaming::ChunkStore};
use std::collections::BTreeSet;
use terra_core::{
    WoodSpecies,
    rails::{BuildChoice, Endpoint, FixedPoint, Piece, Port},
};
use terra_voxel::{SectionPos, blocks::BlockHit, global_to_section};

#[derive(Clone, Debug)]
pub struct ContextChoice {
    pub choice: BuildChoice,
    pub source: Option<Endpoint>,
    pub replaced: Option<Piece>,
    pub layout: Option<Layout>,
    pub valid: bool,
    pub error: Option<String>,
}

fn rejected(choice: BuildChoice, message: impl Into<String>) -> ContextChoice {
    ContextChoice {
        choice,
        source: None,
        replaced: None,
        layout: None,
        valid: false,
        error: Some(message.into()),
    }
}

fn candidate(
    choice: BuildChoice,
    source: Endpoint,
    replaced: Option<Piece>,
    layout: Result<Layout, String>,
    chunks: &ChunkStore,
    journal: &EditJournal,
    eye: [f64; 3],
) -> ContextChoice {
    let mut result = ContextChoice {
        choice,
        source: Some(source),
        replaced,
        layout: None,
        valid: false,
        error: None,
    };
    match layout {
        Err(error) => result.error = Some(error),
        Ok(layout) => {
            let check = if let Some(old) = replaced {
                placement::replacement_environment(&layout, chunks, journal, old)
            } else {
                placement::environment(&layout, chunks, journal)
            };
            let check = check.and_then(|()| {
                if let Some(end) = placement::connection_target(&layout)
                    && distance(end.position.position(), eye) > 4.5 * 4.5 + 1e-8
                {
                    return Err("The rail destination is farther than 4.5 blocks away".into());
                }
                Ok(())
            });
            result.valid = check.is_ok();
            result.error = check.err();
            result.layout = Some(layout);
        }
    }
    result
}

fn distance(first: [f64; 3], second: [f64; 3]) -> f64 {
    (0..3)
        .map(|axis| (first[axis] - second[axis]).powi(2))
        .sum()
}

fn nearby_pieces(journal: &EditJournal, point: [f64; 3]) -> Vec<Piece> {
    let low = global_to_section(point.map(|value| (value - 4.5).floor() as i64)).0;
    let high = global_to_section(point.map(|value| (value + 4.5).floor() as i64)).0;
    let mut anchors = BTreeSet::new();
    for x in low.x..=high.x {
        for y in low.y..=high.y {
            for z in low.z..=high.z {
                for (cell, material, state) in
                    journal.object_states_in_section(SectionPos::new(x, y, z))
                {
                    if terra_core::rails::is_rail(material)
                        && (0..3)
                            .map(|axis| {
                                let gap = (cell[axis] as f64 - point[axis])
                                    .max(point[axis] - cell[axis] as f64 - 1.0)
                                    .max(0.0);
                                gap * gap
                            })
                            .sum::<f64>()
                            <= 4.5 * 4.5 + 1e-8
                    {
                        anchors.insert(state.peer.unwrap_or(cell));
                    }
                }
            }
        }
    }
    anchors
        .into_iter()
        .filter_map(|anchor| owner_at(journal, anchor))
        .filter(|piece| complete(journal, *piece))
        .collect()
}

fn free_ports(journal: &EditJournal, eye: [f64; 3]) -> Vec<(Piece, Port)> {
    nearby_pieces(journal, eye)
        .into_iter()
        .flat_map(|piece| {
            piece
                .ports()
                .into_iter()
                .filter(move |port| {
                    port.connectable
                        && distance(port.position.position(), eye) <= 4.5 * 4.5 + 1e-8
                        && !placement::port_connected(journal, piece, *port)
                })
                .map(move |port| (piece, port))
        })
        .collect()
}

fn horizontal_curve(layout: &Layout) -> bool {
    let Some(spec) = placement::proposed_piece(layout).and_then(|piece| piece.curve) else {
        return false;
    };
    let first = spec.points[0];
    let last = spec.points[3];
    spec.points[1..3].iter().any(|point| {
        i64::from(point[0] - first[0]) * i64::from(last[2] - first[2])
            != i64::from(point[2] - first[2]) * i64::from(last[0] - first[0])
    })
}

fn push_best(choices: &mut Vec<ContextChoice>, next: ContextChoice) {
    if let Some(index) = choices.iter().position(|old| old.choice == next.choice) {
        if !choices[index].valid && next.valid {
            choices[index] = next;
        }
    } else {
        choices.push(next);
    }
}

fn raised_floor_port(start: Port, hit: BlockHit) -> Option<Port> {
    let cell = if hit.normal == [0, 1, 0] {
        hit.adjacent()
    } else if matches!(hit.normal, [1, 0, 0] | [-1, 0, 0] | [0, 0, 1] | [0, 0, -1]) {
        [hit.position[0], hit.position[1] + 1, hit.position[2]]
    } else {
        return None;
    };
    if start.tangent[1] != 0 {
        return None;
    }
    let direction = start.tangent;
    let end = Port {
        index: 0,
        connectable: true,
        tangent: direction.map(|value| -value),
        position: FixedPoint(std::array::from_fn(|axis| {
            cell[axis] * 16
                + if axis == 1 {
                    2
                } else if direction[axis] > 0 {
                    0
                } else if direction[axis] < 0 {
                    16
                } else {
                    8
                }
        })),
    };
    (end.position.0[0] == start.position.0[0]
        && end.position.0[2] == start.position.0[2]
        && end.position.0[1].abs_diff(start.position.0[1]) == 16)
        .then_some(end)
}

/// Actual straight and curved docking alternatives, preferred first. No nearby
/// reachable free rail end means ordinary, unconnected placement stays in use.
pub fn context_choices(
    chunks: &ChunkStore,
    journal: &EditJournal,
    hit: BlockHit,
    ray: ([f64; 3], [f64; 3]),
    look: [f32; 2],
    wood: WoodSpecies,
) -> Vec<ContextChoice> {
    if ray
        .0
        .iter()
        .chain(ray.1.iter())
        .any(|value| !value.is_finite())
        || look.iter().any(|value| !value.is_finite())
        || !hit.distance.is_finite()
        || !(0.0..=4.5 + 1e-8).contains(&hit.distance)
        || ray.0.iter().any(|value| value.abs() > 990_000.0)
        || hit
            .position
            .iter()
            .any(|value| value.unsigned_abs() > 990_000)
    {
        return Vec::new();
    }
    let point = std::array::from_fn(|axis| ray.0[axis] + ray.1[axis] * hit.distance);
    let mut ports = free_ports(journal, ray.0);
    ports.sort_by(|first, second| {
        distance(first.1.position.position(), point)
            .total_cmp(&distance(second.1.position.position(), point))
            .then(first.0.anchor.cmp(&second.0.anchor))
            .then(first.1.index.cmp(&second.1.index))
    });
    let mut choices = Vec::new();
    if let Some(clicked) = owner_at(journal, hit.position) {
        let Some(&(source, start)) = ports.iter().find(|(piece, port)| {
            *piece == clicked && distance(port.position.position(), point) <= 1.5 * 1.5
        }) else {
            return choices;
        };
        let endpoint = Endpoint {
            anchor: source.anchor,
            port: start.index,
        };
        let straight =
            placement::placement(chunks, journal, hit, terra_core::rails::STRAIGHT, ray, look)
                .or_else(|_| {
                    placement::proposal(
                        chunks,
                        journal,
                        hit,
                        terra_core::rails::STRAIGHT,
                        ray,
                        look,
                    )
                })
                .map(|mut layout| {
                    placement::set_wood(&mut layout, wood);
                    layout
                });
        push_best(
            &mut choices,
            candidate(
                BuildChoice::Straight,
                endpoint,
                None,
                straight,
                chunks,
                journal,
                ray.0,
            ),
        );
        for &(other, end) in ports.iter().filter(|(piece, _)| *piece != source).take(4) {
            let replacement = if start.position.0[0] == end.position.0[0]
                && start.position.0[2] == end.position.0[2]
                && start.position.0[1].abs_diff(end.position.0[1]) == 16
            {
                if start.position.0[1] < end.position.0[1] {
                    Some((source, start, end))
                } else {
                    Some((other, end, start))
                }
            } else {
                None
            };
            if let Some((old, near, raised)) = replacement {
                let source_endpoint = Endpoint {
                    anchor: old.anchor,
                    port: near.index,
                };
                let layout =
                    placement::ramp_replacement(chunks, journal, old, near.index, raised, wood);
                let next = candidate(
                    BuildChoice::Straight,
                    source_endpoint,
                    Some(old),
                    layout,
                    chunks,
                    journal,
                    ray.0,
                );
                if next.valid || choices.iter().all(|choice| !choice.valid) {
                    choices.retain(|choice| choice.choice != BuildChoice::Straight);
                    choices.push(next);
                }
                continue;
            }
            let target_hit = BlockHit {
                position: other.anchor,
                normal: [0, 1, 0],
                distance: hit.distance,
            };
            let plans =
                placement::connection_candidates(chunks, journal, endpoint, target_hit, look, wood);
            match plans {
                Ok(plans) => {
                    for layout in plans {
                        if placement::connection_target(&layout)
                            .is_none_or(|port| !port.connects(end))
                        {
                            continue;
                        }
                        let choice = if horizontal_curve(&layout) {
                            BuildChoice::Curve
                        } else {
                            BuildChoice::Straight
                        };
                        push_best(
                            &mut choices,
                            candidate(choice, endpoint, None, Ok(layout), chunks, journal, ray.0),
                        );
                    }
                }
                Err(error) => push_best(
                    &mut choices,
                    ContextChoice {
                        source: Some(endpoint),
                        ..rejected(BuildChoice::Curve, error)
                    },
                ),
            }
        }
    } else {
        for &(source, start) in ports.iter().take(4) {
            let endpoint = Endpoint {
                anchor: source.anchor,
                port: start.index,
            };
            if let Some(end) = raised_floor_port(start, hit) {
                let layout =
                    placement::ramp_replacement(chunks, journal, source, start.index, end, wood);
                push_best(
                    &mut choices,
                    candidate(
                        BuildChoice::Straight,
                        endpoint,
                        Some(source),
                        layout,
                        chunks,
                        journal,
                        ray.0,
                    ),
                );
                continue;
            }
            match placement::connection_candidates(chunks, journal, endpoint, hit, look, wood) {
                Ok(plans) => {
                    for layout in plans {
                        let choice = if horizontal_curve(&layout) {
                            BuildChoice::Curve
                        } else {
                            BuildChoice::Straight
                        };
                        push_best(
                            &mut choices,
                            candidate(choice, endpoint, None, Ok(layout), chunks, journal, ray.0),
                        );
                    }
                }
                Err(error) => push_best(
                    &mut choices,
                    ContextChoice {
                        source: Some(endpoint),
                        ..rejected(BuildChoice::Curve, error)
                    },
                ),
            }
        }
    }
    choices.sort_by_key(|choice| {
        (
            !choice.valid,
            if choice.replaced.is_some() {
                0
            } else if choice.choice == BuildChoice::Curve {
                1
            } else {
                2
            },
        )
    });
    choices
}

/// Recompute the selected intent from the current world instead of accepting
/// client control points, source identity, replacement cells or costs.
pub fn context_choice(
    chunks: &ChunkStore,
    journal: &EditJournal,
    hit: BlockHit,
    ray: ([f64; 3], [f64; 3]),
    look: [f32; 2],
    wood: WoodSpecies,
    choice: BuildChoice,
) -> ContextChoice {
    context_choices(chunks, journal, hit, ray, look, wood)
        .into_iter()
        .find(|candidate| choice == BuildChoice::Automatic || candidate.choice == choice)
        .unwrap_or_else(|| rejected(choice, "No matching nearby free rail dock"))
}

/// Every existing module whose port is consumed, plus all rewritten/new cells.
pub fn context_sites(journal: &EditJournal, choice: &ContextChoice) -> Option<Vec<Cell>> {
    let layout = choice.layout.as_ref()?;
    let source = choice.source?;
    let old = owner_at(journal, source.anchor)?;
    if old.anchor != source.anchor || !complete(journal, old) {
        return None;
    }
    let mut result: BTreeSet<_> = cells(old)
        .into_iter()
        .chain(layout.edits.iter().map(|edit| edit.position))
        .collect();
    if let Some(replaced) = choice.replaced {
        if replaced != old || !complete(journal, replaced) {
            return None;
        }
        result.extend(cells(replaced));
    }
    if let Some(next) = placement::proposed_piece(layout) {
        for port in next.ports() {
            for piece in nearby_pieces(journal, port.position.position()) {
                if piece != old && piece.ports().into_iter().any(|other| port.connects(other)) {
                    result.extend(cells(piece));
                }
            }
        }
    }
    Some(result.into_iter().collect())
}

/// Stable intent fingerprint; authorization and state validation remain separate.
pub fn context_signature(choice: &ContextChoice) -> u64 {
    fn add(hash: &mut u64, value: u64) {
        for byte in value.to_le_bytes() {
            *hash ^= u64::from(byte);
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    let mut hash = 0xcbf29ce484222325;
    add(
        &mut hash,
        match choice.choice {
            BuildChoice::Automatic => 0,
            BuildChoice::Straight => 1,
            BuildChoice::Curve => 2,
        },
    );
    if let Some(source) = choice.source {
        add(&mut hash, 1);
        for coordinate in source.anchor {
            add(&mut hash, coordinate as u64);
        }
        add(&mut hash, u64::from(source.port));
    } else {
        add(&mut hash, 0);
    }
    if let Some(old) = choice.replaced {
        add(&mut hash, 1);
        for coordinate in old.anchor {
            add(&mut hash, coordinate as u64);
        }
        add(&mut hash, u64::from(old.encoded_facing()));
    } else {
        add(&mut hash, 0);
    }
    if let Some(layout) = &choice.layout
        && let Some(piece) = placement::proposed_piece(layout)
    {
        add(&mut hash, 1);
        for coordinate in piece.anchor {
            add(&mut hash, coordinate as u64);
        }
        add(&mut hash, u64::from(piece.material));
        add(&mut hash, u64::from(piece.encoded_facing()));
        if let Some(spec) = piece.curve {
            add(&mut hash, 1);
            for coordinate in spec.points.into_iter().flatten() {
                add(&mut hash, coordinate as u64);
            }
        } else {
            add(&mut hash, 0);
        }
        if let Some(state) = layout
            .states
            .iter()
            .find(|(cell, _)| *cell == piece.anchor)
            .and_then(|(_, state)| *state)
        {
            add(&mut hash, state.wood as u64);
        }
    } else {
        add(&mut hash, 0);
    }
    hash
}
