use super::*;
use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

fn fixture() -> (ChunkStore, EditJournal) {
    let center = SectionPos::default();
    let mut chunks = ChunkStore::default();
    for x in -1..=1 {
        for y in -2..=2 {
            for z in -1..=1 {
                let section = center.offset([x, y, z]).unwrap();
                chunks.loaded.insert(
                    section,
                    LoadedSection {
                        data: BlockSection::empty(section).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
        }
    }
    let mut journal = EditJournal::default();
    for x in 0..16 {
        for y in 0..16 {
            let cell = [x, y, 8];
            chunks
                .loaded
                .get_mut(&center)
                .unwrap()
                .data
                .set_block([x as usize, y as usize, 8], 70)
                .unwrap();
            journal.record(cell, 70).unwrap();
            journal
                .set_object_state(cell, Some(BlockObjectState::default()))
                .unwrap();
        }
    }
    for index in 0..2000 {
        journal.record([1600 + index * 16, 4, 8], 3).unwrap();
    }
    (chunks, journal)
}

#[test]
fn worker_views_are_bounded_and_keep_the_exact_authored_geometry() {
    let (chunks, journal) = fixture();
    let center = SectionPos::default();
    let voxels = geometry_chunks(center, &chunks).unwrap();
    let metadata = journal.geometry_snapshot(center).unwrap();
    assert_eq!(voxels.loaded.len(), 45);
    assert_eq!(metadata.changed_sections().count(), 1);
    assert!(voxels.loaded.values().all(|section| section.mesh.is_none()
        && section.water.is_none()
        && section.props.is_empty()));
    let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
    let data = &chunks.loaded[&center].data;
    let live = model_geometry(data, &chunks, &journal, &properties).unwrap();
    let worker = model_geometry(data, &voxels, &metadata, &properties).unwrap();
    assert!(!live.boxes.is_empty());
    assert_eq!(live.boxes, worker.boxes);
    assert_eq!(live.triangles, worker.triangles);
}

#[test]
fn opening_group_parity_keeps_its_remote_row_anchor_beyond_the_voxel_halo() {
    let mut chunks = ChunkStore::default();
    let center = SectionPos::default();
    chunks.loaded.insert(
        center,
        LoadedSection {
            data: BlockSection::empty(center).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    let mut journal = EditJournal::default();
    for x in -33..=16 {
        let cell = [x, 4, 8];
        journal.record(cell, 70).unwrap();
        journal
            .set_object_state(cell, Some(BlockObjectState::default()))
            .unwrap();
        if (0..16).contains(&x) {
            chunks
                .loaded
                .get_mut(&center)
                .unwrap()
                .data
                .set_block([x as usize, 4, 8], 70)
                .unwrap();
        }
    }
    let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
    model_shapes(&chunks.loaded[&center].data, &chunks, &journal, &properties).unwrap();
    assert!(
        journal
            .geometry_snapshot(center)
            .unwrap()
            .object_state([-33, 4, 8])
            .is_some()
    );
}

/// CPU timings only: compare the old synchronous shape stage with the new
/// bounded handoff, excluding collider cooking (which was already asynchronous).
#[test]
#[ignore = "explicit CPU dispatch microbenchmark; no window or GPU"]
fn authored_geometry_dispatch_benchmark() {
    let (chunks, journal) = fixture();
    let center = SectionPos::default();
    let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
    let data = &chunks.loaded[&center].data;
    let _ = model_geometry(data, &chunks, &journal, &properties).unwrap();
    let mut before = Vec::new();
    let mut after = Vec::new();
    for _ in 0..16 {
        let start = std::time::Instant::now();
        std::hint::black_box(model_geometry(data, &chunks, &journal, &properties).unwrap());
        before.push(start.elapsed().as_secs_f64() * 1000.0);
        let start = std::time::Instant::now();
        let voxels = geometry_chunks(center, &chunks).unwrap();
        let metadata = journal.geometry_snapshot(center).unwrap();
        after.push(start.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box((voxels, metadata));
    }
    before.sort_by(f64::total_cmp);
    after.sort_by(f64::total_cmp);
    println!(
        "COLLISION_DISPATCH_CPU samples={} old_shape_mean_ms={:.6} handoff_mean_ms={:.6} old_shape_p95_ms={:.6} handoff_p95_ms={:.6} old_shape_max_ms={:.6} handoff_max_ms={:.6} local_sections=45 remote_history_sections=2000",
        before.len(),
        before.iter().sum::<f64>() / before.len() as f64,
        after.iter().sum::<f64>() / after.len() as f64,
        before[15],
        after[15],
        before[15],
        after[15]
    );
}
