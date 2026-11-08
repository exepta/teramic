use super::*;
use crate::blocks::{BlockTemplate, PrimitiveTemplate, definitions::BlockRenderKind};

fn templates() -> TemplateSet {
    let mut templates = TemplateSet::default();
    for id in (74..=89)
        .chain([98, 104, 105, 206, 207, 208])
        .chain(270..=293)
    {
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

#[test]
fn grate_construction_profiles_keep_visible_holes_in_all_poses_and_mixed_slab_materials() {
    use terra_core::{construction, locomotive_parts};
    let templates = templates();
    for material in 288..=293 {
        let poses = if material == 290 {
            24
        } else if material == 289 {
            10
        } else {
            4
        };
        for facing in 0..poses {
            let profile = construction_profile(&cell(material, facing), &templates).unwrap();
            let boxes = locomotive_parts::grate_boxes(material, facing).unwrap();
            assert!(
                boxes.len() >= 10,
                "grate must preserve the individual lattice bars"
            );
            assert_identical(
                &profile,
                &crate::blocks::cube::box_model(&boxes, 0.0, [usize::from(material); 6]),
            );
        }
    }
    for (first, second) in [(82, 289), (289, 82)] {
        for facing in [0, 4, 5, 6, 7, 8] {
            let merged = construction::merged_slab(first, facing, second).unwrap();
            let profile = construction_profile(&cell(first, merged), &templates).unwrap();
            let halves = construction::slab_halves(first, merged).unwrap();
            let expected = halves
                .into_iter()
                .flat_map(|(id, pose)| {
                    let bounds = locomotive_parts::grate_boxes(id, pose)
                        .unwrap_or_else(|| vec![construction::slab_bounds(pose)]);
                    crate::blocks::cube::box_model(&bounds, 0.0, [usize::from(id); 6])
                })
                .collect::<Vec<_>>();
            assert_identical(&profile, &expected);
            assert!(profile.iter().any(|part| part.material == 82));
            assert!(profile.iter().any(|part| part.material == 289));
        }
    }
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

#[test]
fn ballast_quarters_and_legacy_thirds_have_distinct_neighbor_cache_keys() {
    let templates = templates();
    let mut cache = ConstructionProfiles::default();
    let mut model = cell(207, 0);
    model.neighbors[5] = Some(77);
    model.neighbor_facing[5] = 1;
    let thirds = construction_profile(&model, &templates).unwrap();
    assert_identical(&cache.get(&model, &templates).unwrap(), &thirds);
    model.neighbors[5] = Some(207);
    let quarters = construction_profile(&model, &templates).unwrap();
    assert_identical(&cache.get(&model, &templates).unwrap(), &quarters);
    assert_eq!(cache.0.len(), 2);
    assert_ne!(
        thirds.len(),
        quarters.len(),
        "third- and quarter-step corners require different real surfaces"
    );
    model.neighbors[5] = Some(78);
    assert_identical(&cache.get(&model, &templates).unwrap(), &thirds);
    assert_eq!(
        cache.0.len(),
        2,
        "ordinary three-step materials should still share a profile"
    );
}
