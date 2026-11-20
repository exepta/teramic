//! Source dependence is runtime bookkeeping, never another serialized water level.
//! Proofs walk only real wet cells, including saved fluid across unloaded seams.
use super::*;
use terra_voxel::SectionPos;

type Cell = [i64; 3];
const PROBE_BUDGET: usize = 4096;
const WAKE_BUDGET: usize = 256;
const WITNESS_LIMIT: usize = 65_536;

#[derive(Clone, Default)]
struct Cells(HashMap<SectionPos, Box<[u64; 64]>>);
impl Cells {
    fn contains(&self, p: Cell) -> bool {
        let (section, local) = global_to_section(p);
        let index = local[0] + 16 * (local[1] + 16 * local[2]);
        self.0
            .get(&section)
            .is_some_and(|bits| bits[index / 64] & (1 << (index % 64)) != 0)
    }
    fn insert(&mut self, p: Cell) -> bool {
        let (section, local) = global_to_section(p);
        let index = local[0] + 16 * (local[1] + 16 * local[2]);
        let bits = self.0.entry(section).or_insert_with(|| Box::new([0; 64]));
        let mask = 1 << (index % 64);
        let changed = bits[index / 64] & mask == 0;
        bits[index / 64] |= mask;
        changed
    }
    fn remove(&mut self, p: Cell) {
        let (section, local) = global_to_section(p);
        if let Some(bits) = self.0.get_mut(&section) {
            let index = local[0] + 16 * (local[1] + 16 * local[2]);
            bits[index / 64] &= !(1 << (index % 64));
            if bits.iter().all(|&word| word == 0) {
                self.0.remove(&section);
            }
        }
    }
    fn merge(&mut self, other: Self) {
        for (section, incoming) in other.0 {
            let bits = self.0.entry(section).or_insert_with(|| Box::new([0; 64]));
            for (out, value) in bits.iter_mut().zip(incoming.iter()) {
                *out |= value;
            }
        }
    }
    fn take(&mut self, limit: usize) -> Vec<Cell> {
        let cells: Vec<_> = self.positions().take(limit).collect();
        for &cell in &cells {
            self.remove(cell);
        }
        cells
    }
    fn positions(&self) -> impl Iterator<Item = Cell> + '_ {
        self.0.iter().flat_map(|(&section, bits)| {
            bits.iter().enumerate().flat_map(move |(word, &bits)| {
                std::iter::successors((bits != 0).then_some(bits), |bits| {
                    let next = bits & (bits - 1);
                    (next != 0).then_some(next)
                })
                .filter_map(move |bits| {
                    let index = word * 64 + bits.trailing_zeros() as usize;
                    section
                        .global_block([index % 16, index / 16 % 16, index / 256])
                        .ok()
                })
            })
        })
    }
}

#[derive(Clone)]
struct Search {
    seed: Cell,
    seen: Cells,
    queue: VecDeque<Cell>,
    waiting: HashSet<SectionPos>,
}
impl Search {
    fn new(seed: Cell) -> Self {
        let mut seen = Cells::default();
        seen.insert(seed);
        Self {
            seed,
            seen,
            queue: VecDeque::from([seed]),
            waiting: HashSet::new(),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct SourceCache {
    witnesses: HashMap<Cell, Cell>,
    roots: HashSet<Cell>,
    orphaned: Cells,
    search: Option<Search>,
    blocked: HashMap<SectionPos, Cells>,
    waiting: HashSet<SectionPos>,
    invalidated: Cells,
    wake_seen: Cells,
    remaining: usize,
}

pub(super) enum Support {
    Connected,
    Disconnected,
    Pending(Option<SectionPos>),
}

impl SourceCache {
    pub(crate) fn invalidate(&mut self) {
        let old = std::mem::take(self);
        self.invalidated = old.invalidated;
        for p in old.witnesses.into_keys() {
            self.invalidated.insert(p);
        }
        self.invalidated.merge(old.orphaned);
        if let Some(search) = old.search {
            self.invalidated.merge(search.seen);
        }
        for cells in old.blocked.into_values() {
            self.invalidated.merge(cells);
        }
    }
    pub(crate) fn invalidate_from(&mut self, p: Cell) {
        self.invalidate();
        self.invalidated.insert(p);
    }
    pub(crate) fn invalidate_around(&mut self, p: Cell) {
        if std::iter::once(p)
            .chain(
                super::super::fluid::NEIGHBORS
                    .into_iter()
                    .filter_map(|d| adjacent(p, d)),
            )
            .any(|p| {
                self.witnesses.contains_key(&p)
                    || self.roots.contains(&p)
                    || self.orphaned.contains(p)
                    || self.blocked.values().any(|cells| cells.contains(p))
                    || self.search.as_ref().is_some_and(|s| s.seen.contains(p))
            })
        {
            self.invalidate();
        }
        // Even a basin processed directly from its source can be asleep
        // without having needed any per-member source witness yet.
        self.invalidated.insert(p);
    }
    pub(super) fn begin_step(&mut self, sampler: &mut Sampler<'_>, journal: &mut EditJournal) {
        self.remaining = PROBE_BUDGET;
        // A settled downstream pool may have no active local cell near a
        // removed source after its owner unloads. Revisit its known witnesses.
        for p in self.invalidated.take(WAKE_BUDGET) {
            if !self.wake_seen.insert(p) {
                continue;
            }
            if sampler.chunks.block_at(p).is_some() {
                journal.wake_fluid(p);
            }
            let at = known_cell(sampler, journal, p);
            // An alternative real source sustains itself; do not walk an
            // entire natural ocean merely to wake the adjoining old stream.
            if matches!(at,Known::Wet(cell) if cell.source) {
                continue;
            }
            for direction in super::super::fluid::NEIGHBORS {
                let Some(q) = adjacent(p, direction) else {
                    continue;
                };
                if !self.wake_seen.contains(q)
                    && matches!(known_cell(sampler, journal, q), Known::Wet(_))
                    && (!matches!(at, Known::Wet(_)) || connected(sampler, journal, p, q))
                {
                    self.invalidated.insert(q);
                }
            }
        }
        if self.invalidated.0.is_empty() {
            self.wake_seen = Cells::default();
        }
        if self
            .waiting
            .iter()
            .any(|p| sampler.chunks.loaded.contains_key(p))
        {
            self.blocked.clear();
            self.waiting.clear();
        }
    }
    pub(super) fn forget(&mut self, p: Cell) {
        self.orphaned.remove(p);
        self.witnesses.remove(&p);
    }

    pub(super) fn can_receive(
        &mut self,
        sampler: &mut Sampler<'_>,
        journal: &mut EditJournal,
        p: Cell,
    ) -> Support {
        let mut pending = None;
        for direction in super::super::fluid::NEIGHBORS {
            let Some(q) = adjacent(p, direction) else {
                continue;
            };
            let Known::Wet(cell) = known_cell(sampler, journal, q) else {
                continue;
            };
            match if cell.source {
                Support::Connected
            } else {
                self.support(sampler, journal, q)
            } {
                Support::Connected => {
                    let source = if cell.source {
                        Some(q)
                    } else {
                        self.witnesses.get(&q).copied()
                    };
                    // Opening a model's first visible film can pay several
                    // height units at once and momentarily empty its upstream
                    // neighbor. Preserve that packet's proven real supplier;
                    // source/geometry edits invalidate it, ordinary paid flux
                    // must not turn it into an orphan between consecutive steps.
                    if let Some(source) = source {
                        if self.witnesses.len() >= WITNESS_LIMIT {
                            self.witnesses.clear();
                            self.roots.clear();
                        }
                        self.roots.insert(source);
                        self.witnesses.insert(p, source);
                    }
                    return Support::Connected;
                }
                Support::Pending(waiting) => {
                    if let Some(section) = waiting {
                        journal.sleep_fluid(q, section);
                    } else {
                        journal.wake_fluid(q);
                    }
                    if pending.is_none() || waiting.is_some() {
                        pending = Some(waiting);
                    }
                }
                _ if sampler.chunks.block_at(q).is_some() => journal.wake_fluid(q),
                _ => {}
            }
        }
        pending.map_or(Support::Disconnected, Support::Pending)
    }

    pub(super) fn support(
        &mut self,
        sampler: &mut Sampler<'_>,
        journal: &mut EditJournal,
        p: Cell,
    ) -> Support {
        if let Some((&section, _)) = self.blocked.iter().find(|(_, cells)| cells.contains(p)) {
            return Support::Pending(Some(section));
        }
        if let Some(&source) = self.witnesses.get(&p) {
            if matches!(known_cell(sampler, journal, source), Known::Wet(cell) if cell.source) {
                return Support::Connected;
            }
            let remaining = self.remaining;
            self.invalidate();
            self.remaining = remaining;
        }
        if self.orphaned.contains(p) {
            // A newly wet neighbor can reconnect an old puddle without a block
            // edit. Never let an old dry verdict delete a newly arriving front.
            let reconnecting = super::super::fluid::NEIGHBORS.into_iter().any(|d| {
                let Some(q) = adjacent(p, d) else {
                    return false;
                };
                matches!(known_cell(sampler, journal, q), Known::Wet(_))
                    && !self.orphaned.contains(q)
                    && connected(sampler, journal, p, q)
            });
            if !reconnecting {
                return Support::Disconnected;
            }
            self.orphaned.remove(p);
        }
        if self.search.is_none() {
            self.search = Some(Search::new(p));
        }
        let mut search = self.search.take().unwrap();
        let belongs = search.seen.contains(p);
        let mut found = None;
        while self.remaining > 0 {
            let Some(at) = search.queue.pop_front() else {
                break;
            };
            self.remaining -= 1;
            let Known::Wet(cell) = known_cell(sampler, journal, at) else {
                continue;
            };
            if cell.source {
                found = Some(at);
                break;
            }
            for direction in super::super::fluid::NEIGHBORS {
                let Some(q) = adjacent(at, direction) else {
                    continue;
                };
                if search.seen.contains(q) {
                    continue;
                }
                match known_cell(sampler, journal, q) {
                    Known::Wet(_) if connected(sampler, journal, at, q) => {
                        search.seen.insert(q);
                        search.queue.push_back(q);
                    }
                    Known::UnknownNatural => {
                        search.waiting.insert(global_to_section(q).0);
                    }
                    _ => {}
                }
            }
        }
        if let Some(source) = found {
            if self.witnesses.len() >= WITNESS_LIMIT {
                self.witnesses.clear();
                self.roots.clear();
            }
            self.roots.insert(source);
            for cell in search
                .seen
                .positions()
                .take(WITNESS_LIMIT - self.witnesses.len())
            {
                self.orphaned.remove(cell);
                self.witnesses.insert(cell, source);
            }
            if belongs {
                self.witnesses.insert(p, source);
            }
            return if belongs {
                Support::Connected
            } else {
                Support::Pending(None)
            };
        }
        if search.queue.is_empty() {
            if search.waiting.is_empty() {
                let belongs = search.seen.contains(p);
                self.orphaned.merge(search.seen.clone());
                self.invalidated.merge(search.seen);
                return if belongs {
                    Support::Disconnected
                } else {
                    Support::Pending(None)
                };
            }
            // Only an actual potential procedural water source needs a load.
            // An arbitrary unknown air edge never keeps a removed stream alive.
            self.waiting.extend(search.waiting.iter().copied());
            let section = *search.waiting.iter().min().unwrap();
            journal.sleep_fluid(search.seed, section);
            self.blocked.entry(section).or_default().merge(search.seen);
            return Support::Pending(Some(section));
        }
        self.search = Some(search);
        Support::Pending(None)
    }
}

#[derive(Clone, Copy)]
enum Known {
    Wet(FluidCell),
    Dry,
    UnknownNatural,
}

fn material(chunks: &ChunkStore, journal: &EditJournal, p: Cell) -> Option<u16> {
    chunks.block_at(p).or_else(|| journal.block(p))
}

fn known_cell(sampler: &mut Sampler<'_>, journal: &EditJournal, p: Cell) -> Known {
    if sampler.chunks.block_at(p).is_some() {
        return match sampler.cell(journal, p) {
            Some(cell) if cell.amount > 0 => Known::Wet(cell),
            _ => Known::Dry,
        };
    }
    if material(sampler.chunks, journal, p).is_some_and(|m| !super::super::can_hold_fluid(m)) {
        return Known::Dry;
    }
    if let Some(cell) = journal.fluid(p) {
        return if cell.amount > 0 {
            Known::Wet(cell)
        } else {
            Known::Dry
        };
    }
    if sampler.natural(p).amount > 0 {
        Known::UnknownNatural
    } else {
        Known::Dry
    }
}

fn connected(sampler: &mut Sampler<'_>, journal: &EditJournal, p: Cell, q: Cell) -> bool {
    let (Known::Wet(mut a), Known::Wet(mut b)) = (
        known_cell(sampler, journal, p),
        known_cell(sampler, journal, q),
    ) else {
        return false;
    };
    let space = |sampler: &mut Sampler<'_>, at| {
        if sampler.chunks.block_at(at).is_some() {
            sampler.space(journal, at).clone()
        } else {
            super::super::space::FluidSpace::at(journal, at, |p| {
                material(sampler.chunks, journal, p).or(Some(0))
            })
        }
    };
    let full = FluidCell {
        amount: FLUID_CAPACITY,
        ..Default::default()
    };
    if p[1] != q[1] {
        // A falling jet continues through the open air above a fractional
        // reservoir inside its voxel before meeting the real water surface.
        // The actual solid geometry, including closed slab faces, still clips
        // both sides of this vertical contact.
        a.amount = FLUID_CAPACITY;
        b.amount = FLUID_CAPACITY;
    } else {
        // The compact byte measures paid volume, not a waterfall's height. A
        // thin falling cell beside a raised outlet occupies its whole vertical
        // void; a supported shallow puddle keeps its true fractional surface.
        let falling = |sampler: &mut Sampler<'_>, at: Cell| {
            let Some(down) = adjacent(at, [0, -1, 0]) else {
                return false;
            };
            if !material(sampler.chunks, journal, down).is_some_and(super::super::can_hold_fluid)
                && !matches!(known_cell(sampler, journal, down), Known::Wet(_))
            {
                return false;
            }
            space(sampler, at).connected(full, &space(sampler, down), full, [0, -1, 0])
        };
        if !a.source && falling(sampler, p) {
            a.amount = FLUID_CAPACITY;
        }
        if !b.source && falling(sampler, q) {
            b.amount = FLUID_CAPACITY;
        }
    }
    space(sampler, p).connected(
        a,
        &space(sampler, q),
        b,
        std::array::from_fn(|i| q[i] - p[i]),
    )
}
