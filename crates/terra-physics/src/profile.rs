use bevy::prelude::*;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

/// Bounded CPU instrumentation for the shared prediction/authority controller.
#[derive(Resource, Default)]
pub struct CharacterProfile {
    calls: AtomicU64,
    nanos: AtomicU64,
    max_nanos: AtomicU64,
}
impl CharacterProfile {
    pub(crate) fn measure(&self) -> Sample<'_> {
        Sample {
            profile: self,
            start: Instant::now(),
        }
    }
    pub fn snapshot(&self) -> (u64, f64, f64) {
        let calls = self.calls.load(Ordering::Relaxed);
        (
            calls,
            self.nanos.load(Ordering::Relaxed) as f64 / calls.max(1) as f64 / 1_000_000.0,
            self.max_nanos.load(Ordering::Relaxed) as f64 / 1_000_000.0,
        )
    }
}
pub(crate) struct Sample<'a> {
    profile: &'a CharacterProfile,
    start: Instant,
}
impl Drop for Sample<'_> {
    fn drop(&mut self) {
        let nanos = self.start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        self.profile.calls.fetch_add(1, Ordering::Relaxed);
        self.profile.nanos.fetch_add(nanos, Ordering::Relaxed);
        self.profile.max_nanos.fetch_max(nanos, Ordering::Relaxed);
    }
}
