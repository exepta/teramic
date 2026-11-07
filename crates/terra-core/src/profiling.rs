//! Opt-in CPU scope counters. Disabled during normal gameplay; no per-frame log output.
use std::{
    collections::BTreeMap,
    sync::{LazyLock, Mutex},
    thread::{self, ThreadId},
    time::Instant,
};

mod timeline;
pub use timeline::{ScopeGroup, ScopeOverlap, ScopeWindow, timeline_enabled, timeline_snapshot};

static ENABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("TERRA_CPU_SCOPES").is_ok_and(|value| value == "1"));
static SAMPLES: Mutex<BTreeMap<&'static str, ScopeSample>> = Mutex::new(BTreeMap::new());

#[derive(Clone, Copy, Default)]
pub struct ScopeSample {
    pub calls: u64,
    pub total_ms: f64,
    pub max_ms: f64,
    pub over_8_ms: u64,
}

pub struct ScopeTimer {
    name: &'static str,
    start: Option<Instant>,
    thread: Option<ThreadId>,
}

impl ScopeTimer {
    pub fn new(name: &'static str) -> Self {
        let timeline = timeline_enabled();
        Self {
            name,
            start: (*ENABLED || timeline).then(Instant::now),
            thread: timeline.then(|| thread::current().id()),
        }
    }
}

impl Drop for ScopeTimer {
    fn drop(&mut self) {
        let Some(start) = self.start else { return };
        let end = Instant::now();
        let ms = end.duration_since(start).as_secs_f64() * 1000.0;
        if *ENABLED && let Ok(mut samples) = SAMPLES.lock() {
            let sample = samples.entry(self.name).or_default();
            sample.calls += 1;
            sample.total_ms += ms;
            sample.max_ms = sample.max_ms.max(ms);
            sample.over_8_ms += u64::from(ms > 8.0);
        }
        if let Some(thread) = self.thread {
            timeline::record(self.name, thread, start, end);
        }
    }
}

pub fn snapshot() -> Vec<(&'static str, ScopeSample)> {
    SAMPLES.lock().map_or_else(
        |_| Vec::new(),
        |samples| {
            samples
                .iter()
                .map(|(&name, &sample)| (name, sample))
                .collect()
        },
    )
}
