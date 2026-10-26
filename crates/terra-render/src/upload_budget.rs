use bevy::prelude::*;

pub(crate) const TERRAIN_VERTICES: usize = 128_000;
pub(crate) const PROP_VERTICES: usize = 96_000;

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
    sections: [usize; 3],
}

impl MeshUploadStatus {
    pub(crate) fn reserve(&mut self, kind: UploadKind, vertices: usize) -> bool {
        let index = kind as usize;
        let limit = [TERRAIN_VERTICES, PROP_VERTICES, 16_000][index];
        let section_limit = [8, 4, 4][index];
        // Reserve once for the complete section, not once per material. One
        // oversized section may progress alone without fragmenting GPU batches.
        if self.sections[index] >= section_limit
            || (self.sections[index] != 0 && self.used[index].saturating_add(vertices) > limit)
        {
            self.deferred_batches += 1;
            return false;
        }
        self.used[index] = self.used[index].saturating_add(vertices);
        self.sections[index] += 1;
        self.admitted_vertices = self.admitted_vertices.saturating_add(vertices);
        self.peak_admitted_vertices = self.peak_admitted_vertices.max(self.admitted_vertices);
        true
    }

    pub(crate) fn reset(mut status: ResMut<Self>) {
        status.used = [0; 3];
        status.sections = [0; 3];
        status.admitted_vertices = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quotas_defer_without_starving_other_domains_or_oversized_sections() {
        let mut status = MeshUploadStatus::default();
        assert!(status.reserve(UploadKind::Terrain, TERRAIN_VERTICES - 10_000));
        assert!(!status.reserve(UploadKind::Terrain, 20_000));
        assert!(status.reserve(UploadKind::Props, 40_000));
        assert!(status.reserve(UploadKind::Water, 10_000));
        assert_eq!(status.admitted_vertices, TERRAIN_VERTICES + 40_000);
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
