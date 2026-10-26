//! Prepared material batches for a single atomic section replacement.
use bevy::{camera::primitives::Aabb, prelude::*};

#[derive(Debug)]
pub(crate) struct StagedMesh {
    pub handle: Handle<Mesh>,
    pub bounds: Option<Aabb>,
    pub rooted: bool,
    pub damp: bool,
    pub opaque_shadow: bool,
    pub coastal_quads: usize,
    pub far: Option<std::sync::Arc<super::far_batches::Geometry>>,
}
impl PartialEq for StagedMesh {
    fn eq(&self, other: &Self) -> bool {
        // A mesh asset ID uniquely identifies the captured CPU geometry, too.
        self.handle == other.handle
            && self.bounds == other.bounds
            && self.rooted == other.rooted
            && self.damp == other.damp
            && self.opaque_shadow == other.opaque_shadow
            && self.coastal_quads == other.coastal_quads
    }
}
