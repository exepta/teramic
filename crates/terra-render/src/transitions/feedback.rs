use bevy::{prelude::*, render::extract_resource::ExtractResource};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

/// Only animation candidates are copied into the render world. Feedback is
/// emitted after the complete color draw command succeeds, not merely upload.
#[derive(Resource, Clone, Default, ExtractResource)]
pub(crate) struct TransitionDraws {
    pub pending: HashSet<Entity>,
    ready: Arc<Mutex<HashSet<Entity>>>,
}

impl TransitionDraws {
    pub fn drawn(&self, entity: Entity) {
        if self.pending.contains(&entity)
            && let Ok(mut ready) = self.ready.lock()
        {
            ready.insert(entity);
        }
    }
    pub fn take_ready(&self) -> HashSet<Entity> {
        self.ready
            .lock()
            .map(|mut ready| ready.drain().collect())
            .unwrap_or_default()
    }
}
