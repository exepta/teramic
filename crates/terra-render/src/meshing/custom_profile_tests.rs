use super::*;
use crate::blocks::{BlockTemplate, PrimitiveTemplate, definitions::BlockRenderKind};

fn templates() -> TemplateSet {
    let mut templates = TemplateSet::default();
    for id in (74..=89).chain([98, 104, 105]) {
        templates.blocks.insert(
            id,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [usize::from(id); 6],
                side_cap: None,
                details: Vec::new(),
                model: Vec::new(),
            },
        );
    }
    templates
}

fn cell(material: u16, facing: u8) -> ModelCell {
    ModelCell {
        cell: [0; 3],
        material,
        state: terra_world::edits::BlockObjectState {
            facing,
            ..Default::default()
        },
        peer_wood: Default::default(),
        neighbors: [None; 6],
        neighbor_facing: [0; 6],
    }
}

fn assert_identical(actual: &[PrimitiveTemplate], expected: &[PrimitiveTemplate]) {
    assert_eq!(actual.len(), expected.len());
    for (a, b) in actual.iter().zip(expected) {
        assert_eq!(a.material, b.material);
        assert_eq!(a.positions, b.positions);
        assert_eq!(a.normals, b.normals);
        assert_eq!(a.uvs, b.uvs);
        assert_eq!(a.indices, b.indices);
    }
}

#[test]
fn repeated_construction_profiles_reuse_geometry_without_losing_corner_poses_or_materials() {
    let templates = templates();
    let mut cache = ConstructionProfiles::default();
    for material in [74, 77, 78, 82, 85, 86, 98, 104, 105] {
        for facing in [0, 1, 4, 7] {
            let mut model = cell(material, facing);
            let expected = construction_profile(&model, &templates).unwrap();
            assert_identical(&cache.get(&model, &templates).unwrap(), &expected);
            let count = cache.0.len();
            model.cell = [12, 8, 5];
            model.neighbors = [Some(3); 6];
            assert_identical(&cache.get(&model, &templates).unwrap(), &expected);
            assert_eq!(
                cache.0.len(),
                count,
                "ordinary neighbours do not create new profiles"
            );
            model.neighbors[5] = Some(77);
            model.neighbor_facing[5] = 1;
            assert_identical(
                &cache.get(&model, &templates).unwrap(),
                &construction_profile(&model, &templates).unwrap(),
            );
        }
    }
}

#[test]
fn varied_structure_poses_keep_the_per_job_profile_cache_bounded() {
    let templates = templates();
    let mut cache = ConstructionProfiles::default();
    for material in 74..=81 {
        for facing in 0..24 {
            let model = cell(material, facing);
            assert_identical(
                &cache.get(&model, &templates).unwrap(),
                &construction_profile(&model, &templates).unwrap(),
            );
            assert!(cache.0.len() <= ConstructionProfiles::LIMIT);
        }
    }
    assert_eq!(cache.0.len(), ConstructionProfiles::LIMIT);
}

#[test]
#[ignore = "opt-in CPU comparison; timing is not a CI assertion"]
fn benchmark_repeated_roof_profiles() {
    use std::{hint::black_box, time::Instant};
    let templates = templates();
    let model = cell(77, 1);
    for pass in 0..3 {
        let start = Instant::now();
        for _ in 0..20_000 {
            black_box(construction_profile(black_box(&model), &templates).unwrap());
        }
        let previous = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        let mut cache = ConstructionProfiles::default();
        for _ in 0..20_000 {
            black_box(cache.get(black_box(&model), &templates).unwrap());
        }
        let cached = start.elapsed().as_secs_f64() * 1000.0;
        println!(
            "structure_roof_profile pass={pass} requests=20000 previous_ms={previous:.3} cached_ms={cached:.3}"
        );
    }
}
