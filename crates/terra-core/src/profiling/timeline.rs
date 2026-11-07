//! Closed CPU spans only. Nested and parallel spans must not be added together.
use std::{
    collections::{HashMap, VecDeque},
    sync::{LazyLock, Mutex},
    thread::ThreadId,
    time::Instant,
};

const CAPACITY: usize = 32_768;
const MAX_TOP_SCOPES: usize = 12;
const MAX_GROUP_NAMES: usize = 128;
const MAX_TOP_GROUPS: usize = 24;

static ENABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("TERRA_SCOPE_TIMELINE").is_ok_and(|value| value == "1"));
// The bounded allocation is lazy: normal gameplay never initializes this ring.
static TIMELINE: LazyLock<Mutex<ScopeTimeline>> =
    LazyLock::new(|| Mutex::new(ScopeTimeline::new(CAPACITY)));

#[derive(Clone, Copy, Debug)]
struct ScopeInterval {
    name: &'static str,
    thread: ThreadId,
    start: Instant,
    end: Instant,
}

#[derive(Clone, Copy, Debug)]
pub struct ScopeOverlap {
    pub name: &'static str,
    pub thread: ThreadId,
    pub start: Instant,
    pub end: Instant,
    pub overlap_ms: f64,
    pub total_ms: f64,
}

/// Per-name closed-span totals, not exclusive CPU time. Nested or concurrent
/// spans may overlap, including multiple spans with the same name.
#[derive(Clone, Copy, Debug)]
pub struct ScopeGroup {
    pub name: &'static str,
    pub calls: usize,
    pub overlap_ms: f64,
    pub total_ms: f64,
    pub max_ms: f64,
}

#[derive(Clone, Debug)]
pub struct ScopeWindow {
    pub scopes: Vec<ScopeOverlap>,
    pub groups: Vec<ScopeGroup>,
    pub overlapping_scopes: usize,
    /// Closed spans omitted from grouping if the bounded name table filled.
    pub ungrouped_scopes: usize,
    pub evicted_scopes: u64,
    /// True if an evicted closed span could have overlapped this window.
    pub history_truncated: bool,
}

struct ScopeTimeline {
    spans: VecDeque<ScopeInterval>,
    capacity: usize,
    evicted_scopes: u64,
    evicted_through: Option<Instant>,
}

impl ScopeTimeline {
    fn new(capacity: usize) -> Self {
        let capacity = capacity.clamp(1, CAPACITY);
        Self {
            spans: VecDeque::with_capacity(capacity),
            capacity,
            evicted_scopes: 0,
            evicted_through: None,
        }
    }

    fn record(&mut self, span: ScopeInterval) {
        if self.spans.len() == self.capacity
            && let Some(old) = self.spans.pop_front()
        {
            self.evicted_scopes += 1;
            self.evicted_through =
                Some(self.evicted_through.map_or(old.end, |end| end.max(old.end)));
        }
        self.spans.push_back(span);
    }

    fn snapshot(&self, start: Instant, end: Instant, top: usize) -> ScopeWindow {
        let top = top.min(MAX_TOP_SCOPES);
        let mut result = ScopeWindow {
            scopes: Vec::with_capacity(top),
            groups: Vec::new(),
            overlapping_scopes: 0,
            ungrouped_scopes: 0,
            evicted_scopes: self.evicted_scopes,
            history_truncated: self.evicted_through.is_some_and(|lost| lost > start),
        };
        if end <= start {
            return result;
        }
        let mut groups = HashMap::<&'static str, ScopeGroup>::new();
        for span in &self.spans {
            let clipped_start = span.start.max(start);
            let clipped_end = span.end.min(end);
            if clipped_end <= clipped_start {
                continue;
            }
            result.overlapping_scopes += 1;
            let overlap = ScopeOverlap {
                name: span.name,
                thread: span.thread,
                start: span.start,
                end: span.end,
                overlap_ms: clipped_end.duration_since(clipped_start).as_secs_f64() * 1000.0,
                total_ms: span.end.duration_since(span.start).as_secs_f64() * 1000.0,
            };
            if let Some(group) = groups.get_mut(span.name) {
                group.calls += 1;
                group.overlap_ms += overlap.overlap_ms;
                group.total_ms += overlap.total_ms;
                group.max_ms = group.max_ms.max(overlap.total_ms);
            } else if groups.len() < MAX_GROUP_NAMES {
                groups.insert(
                    span.name,
                    ScopeGroup {
                        name: span.name,
                        calls: 1,
                        overlap_ms: overlap.overlap_ms,
                        total_ms: overlap.total_ms,
                        max_ms: overlap.total_ms,
                    },
                );
            } else {
                result.ungrouped_scopes += 1;
            }
            let index = result
                .scopes
                .partition_point(|old| old.overlap_ms >= overlap.overlap_ms);
            if index < top {
                if result.scopes.len() == top {
                    result.scopes.pop();
                }
                result.scopes.insert(index, overlap);
            }
        }
        result.groups = groups.into_values().collect();
        result.groups.sort_unstable_by(|a, b| {
            b.overlap_ms
                .total_cmp(&a.overlap_ms)
                .then_with(|| a.name.cmp(b.name))
        });
        result.groups.truncate(MAX_TOP_GROUPS);
        result
    }
}

pub fn timeline_enabled() -> bool {
    *ENABLED
}

pub(super) fn record(name: &'static str, thread: ThreadId, start: Instant, end: Instant) {
    if let Ok(mut timeline) = TIMELINE.lock() {
        timeline.record(ScopeInterval {
            name,
            thread,
            start,
            end,
        });
    }
}

/// Examines only spans closed before this snapshot; callers should delay the
/// query until subsequent frames so the pipelined render thread can finish.
pub fn timeline_snapshot(start: Instant, end: Instant, top: usize) -> Option<ScopeWindow> {
    if !timeline_enabled() {
        return None;
    }
    TIMELINE
        .lock()
        .ok()
        .map(|timeline| timeline.snapshot(start, end, top))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    fn span(base: Instant, name: &'static str, start: u64, end: u64) -> ScopeInterval {
        ScopeInterval {
            name,
            thread: thread::current().id(),
            start: base + Duration::from_millis(start),
            end: base + Duration::from_millis(end),
        }
    }

    #[test]
    fn clips_spans_to_exact_window_without_adding_nested_or_parallel_work() {
        let base = Instant::now();
        let mut timeline = ScopeTimeline::new(10);
        for item in [
            span(base, "outer", 0, 40),
            span(base, "inner", 12, 18),
            span(base, "left", 0, 12),
            span(base, "right", 18, 30),
            span(base, "before", 0, 10),
            span(base, "after", 20, 30),
        ] {
            timeline.record(item);
        }
        let result = timeline.snapshot(
            base + Duration::from_millis(10),
            base + Duration::from_millis(20),
            12,
        );
        assert_eq!(result.overlapping_scopes, 4);
        assert_eq!(
            result
                .scopes
                .iter()
                .map(|s| (s.name, s.overlap_ms, s.total_ms))
                .collect::<Vec<_>>(),
            vec![
                ("outer", 10.0, 40.0),
                ("inner", 6.0, 6.0),
                ("left", 2.0, 12.0),
                ("right", 2.0, 12.0),
            ]
        );
        assert!(!result.history_truncated);
        assert!(timeline.snapshot(base, base, 12).scopes.is_empty());
        assert!(
            timeline
                .snapshot(base + Duration::from_millis(20), base, 12)
                .scopes
                .is_empty()
        );
    }

    #[test]
    fn ring_evicts_at_fixed_budget_and_marks_potentially_incomplete_windows() {
        let base = Instant::now();
        let mut timeline = ScopeTimeline::new(2);
        // Completion order need not match start/end order across threads.
        timeline.record(span(base, "late", 5, 50));
        timeline.record(span(base, "early", 0, 8));
        timeline.record(span(base, "new", 20, 30));
        timeline.record(span(base, "newer", 30, 40));
        assert_eq!(timeline.spans.len(), 2);
        assert_eq!(timeline.spans.capacity(), 2);
        let lost = timeline.snapshot(
            base + Duration::from_millis(10),
            base + Duration::from_millis(45),
            12,
        );
        assert_eq!(lost.evicted_scopes, 2);
        assert!(lost.history_truncated);
        assert!(
            !timeline
                .snapshot(
                    base + Duration::from_millis(50),
                    base + Duration::from_millis(60),
                    12
                )
                .history_truncated
        );
        assert_eq!(ScopeTimeline::new(0).capacity, 1);
        assert_eq!(ScopeTimeline::new(usize::MAX).capacity, CAPACITY);
    }

    #[test]
    fn top_scope_results_have_a_hard_limit_even_for_many_overlapping_spans() {
        let base = Instant::now();
        let mut timeline = ScopeTimeline::new(100);
        for end in 1..=100 {
            timeline.record(span(base, "scope", 0, end));
        }
        let result = timeline.snapshot(base, base + Duration::from_secs(1), usize::MAX);
        assert_eq!(result.overlapping_scopes, 100);
        assert_eq!(result.scopes.len(), MAX_TOP_SCOPES);
        assert_eq!(result.scopes.first().unwrap().overlap_ms, 100.0);
        assert_eq!(result.scopes.last().unwrap().overlap_ms, 89.0);
        let counts_only = timeline.snapshot(base, base + Duration::from_secs(1), 0);
        assert_eq!(counts_only.overlapping_scopes, 100);
        assert!(counts_only.scopes.is_empty());
    }

    #[test]
    fn repeated_small_scopes_remain_visible_as_clipped_per_name_totals() {
        let base = Instant::now();
        let mut timeline = ScopeTimeline::new(200);
        timeline.record(span(base, "outer", 0, 40));
        for _ in 0..100 {
            timeline.record(span(base, "retire", 9, 12));
        }
        for _ in 0..MAX_TOP_SCOPES {
            timeline.record(span(base, "worker", 10, 14));
        }
        let result = timeline.snapshot(
            base + Duration::from_millis(10),
            base + Duration::from_millis(20),
            MAX_TOP_SCOPES,
        );
        assert_eq!(result.overlapping_scopes, 113);
        assert!(result.scopes.iter().all(|scope| scope.name != "retire"));
        let group = result.groups.first().unwrap();
        assert_eq!(group.name, "retire");
        assert_eq!(group.calls, 100);
        assert_eq!(group.overlap_ms, 200.0);
        assert_eq!(group.total_ms, 300.0);
        assert_eq!(group.max_ms, 3.0);
        assert_eq!(result.groups[1].name, "worker");
        assert_eq!(result.groups[1].overlap_ms, 48.0);
        assert_eq!(result.groups[2].name, "outer");
        assert_eq!(result.groups[2].overlap_ms, 10.0);
        assert_eq!(result.ungrouped_scopes, 0);
    }

    #[test]
    fn aggregate_name_and_output_limits_are_fixed_and_report_dropped_names() {
        let base = Instant::now();
        let mut timeline = ScopeTimeline::new(256);
        let names: Vec<&'static str> = (0..MAX_GROUP_NAMES + 2)
            .map(|index| &*Box::leak(format!("scope-{index}").into_boxed_str()))
            .collect();
        for &name in &names {
            timeline.record(span(base, name, 0, 1));
        }
        timeline.record(span(base, names[0], 0, 1));
        let result = timeline.snapshot(base, base + Duration::from_millis(1), 0);
        assert_eq!(result.overlapping_scopes, MAX_GROUP_NAMES + 3);
        assert_eq!(result.ungrouped_scopes, 2);
        assert!(result.scopes.is_empty());
        assert_eq!(result.groups.len(), MAX_TOP_GROUPS);
        assert_eq!(result.groups[0].name, names[0]);
        assert_eq!(result.groups[0].calls, 2);
        assert_eq!(result.groups[0].overlap_ms, 2.0);
        let empty = timeline.snapshot(base, base, 12);
        assert!(empty.groups.is_empty());
        assert_eq!(empty.ungrouped_scopes, 0);
    }
}
