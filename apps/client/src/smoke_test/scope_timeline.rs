//! Bounded evidence for the same Update-to-Update windows used by the FPS sample.
use std::{
    thread::{self, ThreadId},
    time::Instant,
};
use terra_core::profiling::{self, ScopeWindow};

pub(super) struct FrameScopeTrace {
    start: Instant,
    end: Instant,
    observed_frame: u64,
    sampling_thread: ThreadId,
    resolved: bool,
    window: Option<ScopeWindow>,
}

impl FrameScopeTrace {
    pub fn new(start: Instant, end: Instant, observed_frame: u64) -> Option<Self> {
        profiling::timeline_enabled().then(|| Self {
            start,
            end,
            observed_frame,
            sampling_thread: thread::current().id(),
            resolved: false,
            window: None,
        })
    }

    pub fn resolve(&mut self, frame: u64) {
        self.resolve_with(frame, |start, end| {
            profiling::timeline_snapshot(start, end, 12)
        });
    }

    fn resolve_with(
        &mut self,
        frame: u64,
        snapshot: impl FnOnce(Instant, Instant) -> Option<ScopeWindow>,
    ) {
        if self.resolved || frame.saturating_sub(self.observed_frame) < 2 {
            return;
        }
        // The render sub-app is pipelined. Delay without extending the sampled
        // interval, then freeze the evidence before the global ring evicts it.
        self.window = snapshot(self.start, self.end);
        self.resolved = true;
    }

    pub fn resolved(&self) -> bool {
        self.resolved
    }

    pub fn report(&self, frame_ms: f64, sample_time: f64) {
        let Some(window) = &self.window else {
            tracing::warn!(
                frame_ms,
                sample_time,
                "Worst-frame CPU scope timeline unavailable"
            );
            return;
        };
        tracing::info!(
            frame_ms,
            sample_time,
            overlapping_closed_scopes = window.overlapping_scopes,
            reported_scopes = window.scopes.len(),
            reported_groups = window.groups.len(),
            ungrouped_scopes = window.ungrouped_scopes,
            history_truncated = window.history_truncated,
            ring_evictions = window.evicted_scopes,
            "Worst-frame CPU scope window (closed instrumented spans only; delayed two frames; overlapping/nested spans are not additive; uninstrumented or still-open work is absent; no GPU attribution)"
        );
        for scope in &window.scopes {
            tracing::info!(
                frame_ms,
                sample_time,
                scope = scope.name,
                thread = ?scope.thread,
                same_sampling_thread = scope.thread == self.sampling_thread,
                overlap_ms = scope.overlap_ms,
                total_scope_ms = scope.total_ms,
                start_from_frame_ms = offset_ms(scope.start, self.start),
                end_from_frame_ms = offset_ms(scope.end, self.start),
                "Closed CPU scope overlapping worst sampled frame (thread identifies scope start, not CPU execution attribution)"
            );
        }
        for group in &window.groups {
            tracing::info!(
                frame_ms,
                sample_time,
                scope = group.name,
                calls = group.calls,
                summed_overlap_ms = group.overlap_ms,
                summed_total_scope_ms = group.total_ms,
                max_individual_scope_ms = group.max_ms,
                "Closed CPU scope group within worst sampled frame (nested/concurrent spans are not additive CPU time)"
            );
        }
    }
}

fn offset_ms(at: Instant, origin: Instant) -> f64 {
    if at >= origin {
        at.duration_since(origin).as_secs_f64() * 1000.0
    } else {
        -origin.duration_since(at).as_secs_f64() * 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, time::Duration};

    #[test]
    fn waits_two_frames_and_freezes_the_original_sampling_window_once() {
        let start = Instant::now();
        let end = start + Duration::from_millis(40);
        let mut trace = FrameScopeTrace {
            start,
            end,
            observed_frame: 10,
            sampling_thread: thread::current().id(),
            resolved: false,
            window: None,
        };
        let calls = Cell::new(0);
        for frame in [9, 10, 11, 12, 13] {
            trace.resolve_with(frame, |from, to| {
                calls.set(calls.get() + 1);
                assert_eq!((from, to), (start, end));
                Some(ScopeWindow {
                    scopes: Vec::new(),
                    groups: Vec::new(),
                    overlapping_scopes: 0,
                    ungrouped_scopes: 0,
                    evicted_scopes: 0,
                    history_truncated: false,
                })
            });
            assert_eq!(trace.resolved(), frame >= 12);
        }
        assert_eq!(calls.get(), 1);
        assert!(trace.window.is_some());
    }

    #[test]
    fn snapshot_failure_completes_the_pending_trace_instead_of_blocking_exit() {
        let now = Instant::now();
        let mut trace = FrameScopeTrace {
            start: now,
            end: now,
            observed_frame: 0,
            sampling_thread: thread::current().id(),
            resolved: false,
            window: None,
        };
        trace.resolve_with(2, |_, _| None);
        assert!(trace.resolved());
        assert!(trace.window.is_none());
        assert_eq!(offset_ms(now - Duration::from_millis(2), now), -2.0);
        assert_eq!(offset_ms(now + Duration::from_millis(3), now), 3.0);
    }
}
