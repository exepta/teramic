//! Opt-in A/B for the physics worker and its main-thread snapshot preparation.
use super::*;
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use terra_world::{SectionGenerator, TerraGenerator, streaming::LoadedSection};

// Include all 45 potentially relevant neighbors for each measured section.
fn woodland() -> (ChunkStore, Vec<SectionPos>) {
    let generator = SectionGenerator::new(
        TerraGenerator::new(terra_core::WorldConfig {
            seed: terra_core::WorldSeed(9_890_475_192_252_935_834),
            ..default()
        })
        .unwrap(),
    );
    let mut store = ChunkStore::default();
    let mut centers = Vec::new();
    for x in -15..=-11 {
        for z in -11..=-9 {
            let column = generator.prepare_column([x, z], false).unwrap();
            for y in 0..=6 {
                let position = SectionPos::new(x, y, z);
                let mut data = generator
                    .build_prepared(position, 0, &default(), &column)
                    .unwrap()
                    .data;
                if z == -10 && (-14..=-12).contains(&x) && (2..=4).contains(&y) {
                    centers.push(position);
                    if x == -12 && y == 4 {
                        // Include one edited section with authored and crystal geometry.
                        for (index, id) in [99, 70, 51, 148, 602, 793].into_iter().enumerate() {
                            data.set_block([index + 1, 3, 3], id).unwrap();
                        }
                    }
                }
                store.loaded.insert(
                    position,
                    LoadedSection {
                        data,
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
    (store, centers)
}

fn run_section(
    position: SectionPos,
    store: &ChunkStore,
    journal: &EditJournal,
    catalog: &BlockPropertiesCatalog,
    cached: Option<&Arc<CollisionMaterials>>,
) -> (Option<Collider>, Duration, Duration) {
    let start = Instant::now();
    let data = collision_snapshot(position, store, journal).unwrap();
    let voxels = geometry_chunks(position, store).unwrap();
    let metadata = journal.geometry_snapshot(position).unwrap();
    let (collider, handoff, worker) = if let Some(cached) = cached {
        let materials = cached.clone();
        let (shaped, crystal) = materials.section_flags(&data);
        let plain = (!shaped).then(|| (data.clone(), materials.clone()));
        black_box(crystal.then(|| profile_snapshot(position, store, journal)));
        let handoff = start.elapsed();
        let worker = Instant::now();
        let g = section_geometry(&data, &voxels, &metadata, &materials.catalog, shaped, |m| {
            materials.has_shape(m)
        })
        .unwrap();
        let result = cook_collider(&data, &g.boxes, &g.triangles, |m| materials.cube(m)).unwrap();
        let elapsed = worker.elapsed();
        black_box(plain);
        (result, handoff, elapsed)
    } else {
        let masked: Vec<_> = catalog
            .1
            .keys()
            .copied()
            .chain(materials::SEPARATE_SHAPES.iter().copied())
            .chain(terra_core::locomotive_parts::FIRST..=terra_core::locomotive_parts::LAST)
            .collect();
        let properties = catalog.clone();
        let plain = data
            .blocks()
            .iter()
            .all(|&m| catalog.shape(m).is_empty())
            .then(|| (data.blocks().to_vec(), catalog.clone()));
        black_box(
            data.blocks()
                .iter()
                .any(|&m| terra_world::segments::is_crystal(m))
                .then(|| profile_snapshot(position, store, journal)),
        );
        let handoff = start.elapsed();
        let worker = Instant::now();
        let g = model_geometry(&data, &voxels, &metadata, &properties).unwrap();
        let result = build_collider_with_geometry(&data, &g.boxes, &g.triangles, &masked).unwrap();
        let elapsed = worker.elapsed();
        black_box(plain);
        (result, handoff, elapsed)
    };
    (collider, handoff, worker)
}

#[test]
#[ignore = "explicit generated-woodland collider CPU comparison; no window or GPU"]
fn woodland_collider_cpu_comparison() {
    let (store, centers) = woodland();
    let journal = EditJournal::default();
    let catalog =
        BlockPropertiesCatalog::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    let materials = Arc::new(CollisionMaterials::new(&catalog));
    let mut triangles = 0;
    for &position in &centers {
        let old = run_section(position, &store, &journal, &catalog, None).0;
        let new = run_section(position, &store, &journal, &catalog, Some(&materials)).0;
        assert_eq!(old.is_some(), new.is_some());
        if let (Some(old), Some(new)) = (old, new) {
            let a = old.shape().as_trimesh().unwrap();
            let b = new.shape().as_trimesh().unwrap();
            assert_eq!(a.vertices(), b.vertices(), "vertices at {position:?}");
            assert_eq!(a.indices(), b.indices(), "indices at {position:?}");
            triangles += a.indices().len();
        }
    }
    assert!(triangles > 0);
    let plain = centers
        .iter()
        .filter(|p| !materials.section_flags(&store.loaded[*p].data).0)
        .count();
    assert!(plain > 0 && plain < centers.len());
    // In ordinary full-cube sections this stage consists entirely of the
    // cross-boundary searches. Measure it separately from snapshot/collider cost.
    let mut halo_time = Duration::ZERO;
    let mut empty_halos = 0;
    for &position in &centers {
        let data = &store.loaded[&position].data;
        if materials.section_flags(data).0 {
            continue;
        }
        let voxels = geometry_chunks(position, &store).unwrap();
        let metadata = journal.geometry_snapshot(position).unwrap();
        for _ in 0..32 {
            let start = Instant::now();
            let shapes = section_geometry(data, &voxels, &metadata, &catalog, false, |m| {
                materials.has_shape(m)
            })
            .unwrap();
            halo_time += start.elapsed();
            empty_halos += usize::from(shapes.boxes.is_empty() && shapes.triangles.is_empty());
            black_box(shapes);
        }
    }
    println!(
        "COLLISION_HALO plain_scans={} empty={empty_halos} mean_us={:.2}",
        32 * plain,
        halo_time.as_secs_f64() * 1e6 / (32 * plain) as f64
    );
    let mut results = [[Duration::ZERO; 2]; 2];
    for round in 0..32 {
        for mode in [round % 2, 1 - round % 2] {
            for &position in &centers {
                let (collider, handoff, worker) = run_section(
                    position,
                    &store,
                    &journal,
                    &catalog,
                    (mode == 1).then_some(&materials),
                );
                black_box(collider);
                results[mode][0] += handoff;
                results[mode][1] += worker;
            }
        }
    }
    let samples = 32 * centers.len();
    println!(
        "COLLISION_CATALOG sections={} plain={} identical_triangles={} samples_per_mode={samples}",
        centers.len(),
        plain,
        triangles
    );
    for (mode, [handoff, worker]) in ["original", "cached"].into_iter().zip(results) {
        println!(
            "COLLISION_CATALOG {mode} handoff_mean_us={:.2} worker_mean_us={:.2}",
            handoff.as_secs_f64() * 1e6 / samples as f64,
            worker.as_secs_f64() * 1e6 / samples as f64
        );
    }
}
