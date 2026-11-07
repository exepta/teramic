//! Split stable geometry on the worker, without cutting a prop or its crossfade.

pub(super) const VERTEX_LIMIT: usize = 1 << 16;

#[derive(Clone, Copy)]
pub(super) struct Entry {
    pub vertices: usize,
    pub indices: usize,
    pub rooted: bool,
    pub pending: bool,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub(super) struct Piece {
    pub vertices: usize,
    pub indices: usize,
    pub instances: usize,
    pub rooted: bool,
}

impl Piece {
    fn append(&mut self, entry: Entry) {
        self.vertices += entry.vertices;
        self.indices += entry.indices;
        self.instances += 1;
    }
}

pub(super) struct Plan {
    pub pieces: Vec<Piece>,
    /// Empty templates have no destination and never allocate a render asset.
    pub destinations: Vec<Option<usize>>,
}

pub(super) fn plan(entries: &[Entry]) -> Plan {
    let mut plan = Plan {
        pieces: Vec::new(),
        destinations: vec![None; entries.len()],
    };
    for rooted in [false, true] {
        let total: usize = entries
            .iter()
            .filter(|entry| entry.rooted == rooted && entry.indices != 0)
            .map(|entry| entry.vertices)
            .sum();
        let split = total > VERTEX_LIMIT;
        let mut current = None::<usize>;
        for (index, &entry) in entries.iter().enumerate() {
            if entry.rooted != rooted || entry.indices == 0 || (split && rooted && entry.pending) {
                continue;
            }
            let destination = if let Some(current) = current
                && (!split || plan.pieces[current].vertices + entry.vertices <= VERTEX_LIMIT)
            {
                current
            } else {
                let next = plan.pieces.len();
                plan.pieces.push(Piece {
                    rooted,
                    ..Default::default()
                });
                current = Some(next);
                next
            };
            // An intrinsic template larger than the limit stays intact and
            // progresses alone via the uploader's existing oversize exception.
            plan.pieces[destination].append(entry);
            plan.destinations[index] = Some(destination);
        }
        if split && rooted {
            let mut pending = None;
            for (index, &entry) in entries.iter().enumerate() {
                if !entry.rooted || !entry.pending || entry.indices == 0 {
                    continue;
                }
                let destination = *pending.get_or_insert_with(|| {
                    let next = plan.pieces.len();
                    plan.pieces.push(Piece {
                        rooted,
                        ..Default::default()
                    });
                    next
                });
                // New/old crop variants share the final batch. Its clock starts
                // only when all stable pieces have reached the render world.
                plan.pieces[destination].append(entry);
                plan.destinations[index] = Some(destination);
            }
        }
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(vertices: usize, rooted: bool, pending: bool) -> Entry {
        Entry {
            vertices,
            indices: 6,
            rooted,
            pending,
        }
    }

    #[test]
    fn small_layouts_keep_their_original_order_and_batch_count() {
        let plan = plan(&[
            entry(12, true, false),
            entry(16, false, false),
            entry(20, true, true),
            Entry {
                indices: 0,
                ..entry(24, false, false)
            },
        ]);
        assert_eq!(plan.destinations, [Some(1), Some(0), Some(1), None]);
        assert_eq!(
            plan.pieces,
            [
                Piece {
                    vertices: 16,
                    indices: 6,
                    instances: 1,
                    rooted: false
                },
                Piece {
                    vertices: 32,
                    indices: 12,
                    instances: 2,
                    rooted: true
                },
            ]
        );
    }

    #[test]
    fn stable_pieces_fit_u16_and_pending_variants_wait_together_at_the_end() {
        let plan = plan(&[
            entry(24_000, true, true),
            entry(40_000, true, false),
            entry(40_000, false, false),
            entry(24_000, true, true),
            entry(40_000, true, false),
            entry(40_000, false, false),
        ]);
        assert_eq!(
            plan.destinations,
            [Some(4), Some(2), Some(0), Some(4), Some(3), Some(1)]
        );
        assert!(
            plan.pieces
                .iter()
                .all(|piece| piece.vertices <= VERTEX_LIMIT)
        );
        assert_eq!(
            plan.pieces
                .iter()
                .map(|piece| piece.vertices)
                .sum::<usize>(),
            208_000
        );
        assert_eq!(plan.pieces[4].vertices, 48_000);
        assert!(plan.pieces[4].rooted);
    }

    #[test]
    fn intrinsically_large_templates_and_atomic_pending_batches_remain_explicit_exceptions() {
        let plan = plan(&[
            entry(VERTEX_LIMIT + 1, false, false),
            entry(4, false, false),
            entry(40_000, true, true),
            entry(40_000, true, true),
        ]);
        assert_eq!(plan.destinations, [Some(0), Some(1), Some(2), Some(2)]);
        assert_eq!(plan.pieces[0].vertices, VERTEX_LIMIT + 1);
        assert_eq!(plan.pieces[1].vertices, 4);
        assert_eq!(plan.pieces[2].vertices, 80_000);
    }
}
