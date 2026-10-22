use bevy::prelude::*;

#[derive(Clone, Copy)]
pub(crate) enum UploadKind {
    Terrain,
    Props,
    Water,
}

/// Admitted geometry, not GPU timings or draw calls. Separate quotas prevent a
/// terrain backlog from starving interactive plants or fluid changes.
#[derive(Resource, Default, Debug)]
pub struct MeshUploadStatus {
    pub admitted_vertices: usize,
    pub peak_admitted_vertices: usize,
    pub deferred_batches: u64,
    used: [usize; 3],
    batches: [usize; 3],
}

impl MeshUploadStatus {
    pub(crate) fn reserve(&mut self, kind: UploadKind, vertices: usize) -> bool {
        let index = kind as usize;
        let limit = [64_000, 48_000, 16_000][index];
        let batch_limit = [8, 4, 4][index];
        // One oversized section must still progress; never split an atomic mesh handoff.
        if self.batches[index] >= batch_limit
            || (self.batches[index] != 0 && self.used[index].saturating_add(vertices) > limit)
        {
            self.deferred_batches += 1;
            return false;
        }
        self.used[index] = self.used[index].saturating_add(vertices);
        self.batches[index] += 1;
        self.admitted_vertices = self.admitted_vertices.saturating_add(vertices);
        self.peak_admitted_vertices = self.peak_admitted_vertices.max(self.admitted_vertices);
        true
    }

    pub(crate) fn reset(mut status: ResMut<Self>) {
        status.used = [0; 3];
        status.batches = [0; 3];
        status.admitted_vertices = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quotas_defer_without_starving_other_domains_or_oversized_sections() {
        let mut status = MeshUploadStatus::default();
        assert!(status.reserve(UploadKind::Terrain, 50_000));
        assert!(!status.reserve(UploadKind::Terrain, 20_000));
        assert!(status.reserve(UploadKind::Props, 40_000));
        assert!(status.reserve(UploadKind::Water, 10_000));
        assert_eq!(status.admitted_vertices, 100_000);
        assert_eq!(status.deferred_batches, 1);
        let mut app = App::new();
        app.insert_resource(status)
            .add_systems(First, MeshUploadStatus::reset);
        app.update();
        let mut status = app.world_mut().resource_mut::<MeshUploadStatus>();
        assert_eq!(status.admitted_vertices, 0);
        assert!(status.reserve(UploadKind::Terrain, 500_000));
        assert!(!status.reserve(UploadKind::Terrain, 1));
    }

    #[test]
    fn tiny_and_empty_batches_cannot_create_an_unbounded_handoff_frame() {
        let mut status = MeshUploadStatus::default();
        for _ in 0..8 {
            assert!(status.reserve(UploadKind::Terrain, 0));
        }
        assert!(!status.reserve(UploadKind::Terrain, 0));
        assert!(status.reserve(UploadKind::Props, 200_000));
        assert!(!status.reserve(UploadKind::Props, 0));
        assert!(status.reserve(UploadKind::Water, 1));
    }
}
