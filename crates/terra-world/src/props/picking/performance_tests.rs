//! Manual microbenchmark, never a hardware-dependent CI timing assertion.
use super::*;
use crate::{
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
    targeting::{WorldTarget, pick_world_with_context},
};
use bevy_math::DVec3;
use std::{hint::black_box, time::Instant};
use terra_core::{CropKind, block_properties::BlockPropertiesCatalog};
use terra_voxel::{BlockSection, SectionPos};

fn field() -> (ChunkStore, Vec<WorldProp>) {
    let owner = SectionPos::new(0, 0, 0);
    let mut data = BlockSection::empty(owner).unwrap();
    for z in 0..16 {
        for x in 0..16 {
            data.set_block([x, 0, z], 3).unwrap();
        }
    }
    let mut props = Vec::with_capacity(28);
    for z in 8..12 {
        for x in 5..12 {
            data.set_block([x, 0, z], terra_core::farming::FARMLAND)
                .unwrap();
            props.push(WorldProp {
                id: crate::farming::crop_id([x as i64, 0, z as i64], CropKind::Wheat),
                kind: PropKind::Wheat,
                position: [x as f64 + 0.5, 1.0, z as f64 + 0.5],
                rotation: 0.0,
                scale: 1.0,
                variant: 7,
                section: owner,
                removed: false,
            });
        }
    }
    assert!(props.iter().all(WorldProp::valid));
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        owner,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: props.clone(),
        },
    );
    (chunks, props)
}

fn aim(prop: &WorldProp, pose: PropPose) -> ([f64; 3], [f64; 3]) {
    let mesh = mesh(prop).unwrap();
    let triangle = mesh
        .indices
        .chunks_exact(3)
        .find(|t| {
            let [a, b, c] = [t[0], t[1], t[2]]
                .map(|i| DVec3::from_array(mesh.positions[i as usize].map(f64::from)));
            (b - a).cross(c - a).normalize_or_zero().z.abs() > 0.8 && (a.y + b.y + c.y) / 3.0 > 0.75
        })
        .unwrap();
    let pose = if matches!(pose, PropPose::WeatherWind { .. }) {
        pose
    } else {
        PropPose::Rest
    };
    let point = triangle
        .iter()
        .map(|i| DVec3::from_array(super::point(prop, mesh.positions[*i as usize], pose)))
        .sum::<DVec3>()
        / 3.0;
    let origin = DVec3::new(point.x, 2.62, point.z - 7.7);
    assert!((point - origin).length() < 8.0);
    (origin.to_array(), (point - origin).normalize().to_array())
}

fn measure(label: &str, mut operation: impl FnMut()) {
    for _ in 0..5 {
        operation();
    }
    let mut times = Vec::with_capacity(200);
    for _ in 0..200 {
        let start = Instant::now();
        operation();
        times.push(start.elapsed().as_nanos() as u64);
    }
    times.sort_unstable();
    let mean = times.iter().sum::<u64>() as f64 / times.len() as f64 / 1000.0;
    eprintln!(
        "crop_pick_bench case={label} n={} mean_us={mean:.3} median_us={:.3} p95_us={:.3} p99_us={:.3} max_us={:.3}",
        times.len(),
        times[times.len() / 2] as f64 / 1000.0,
        times[times.len() * 95 / 100] as f64 / 1000.0,
        times[times.len() * 99 / 100] as f64 / 1000.0,
        times[times.len() - 1] as f64 / 1000.0
    );
}

#[test]
#[ignore = "manual deterministic CPU microbenchmark; run alone with --ignored --nocapture"]
fn benchmark_real_wheat_field_targeting_and_selection_edges() {
    let (chunks, props) = field();
    let journal = EditJournal::default();
    let properties = BlockPropertiesCatalog::default();
    let prop = &props[3];
    let pickmesh = mesh(prop).unwrap();
    let vertices = pickmesh.positions.len();
    let triangles = pickmesh.indices.len() / 3;
    let mut edges = 0;
    prop.visit_selection_edges(|_| edges += 1);
    eprintln!(
        "crop_pick_bench field_props={} mature_wheat_vertices={vertices} triangles={triangles} outline_edges={edges} outline_upload_vertices={} rest_position_vec_bytes={} swept_position_and_offset_vec_bytes={} swept_test_triangles_per_candidate={}",
        props.len(),
        edges * 2,
        vertices * size_of::<DVec3>(),
        vertices * size_of::<DVec3>() * 2,
        triangles * 8
    );
    assert_eq!(props.len(), 28);
    assert!(vertices > 1000 && edges > 1000);
    for (name, pose) in [
        ("rest", PropPose::Rest),
        (
            "wind",
            PropPose::WeatherWind {
                time: 4.25,
                frame: [0.86, 0.51, 1.0, 1.0],
            },
        ),
        (
            "swept",
            PropPose::WeatherSwept {
                frame: [0.86, 0.51, 1.0, 1.0],
            },
        ),
    ] {
        let (origin, direction) = aim(prop, pose);
        assert!(
            hit(prop, origin, direction, 8.0, pose).is_some(),
            "real geometry ray must hit {name}"
        );
        assert!(
            matches!(
                pick_world_with_context(&chunks, &journal, &properties, origin, direction, pose).1,
                Some(WorldTarget::Prop(_))
            ),
            "real loaded field target must be a crop for {name}"
        );
        measure(&format!("single-precise-{name}"), || {
            let _ = black_box(hit(
                black_box(prop),
                black_box(origin),
                black_box(direction),
                8.0,
                black_box(pose),
            ));
        });
        measure(&format!("field-target-hit-{name}"), || {
            let _ = black_box(pick_world_with_context(
                black_box(&chunks),
                black_box(&journal),
                black_box(&properties),
                black_box(origin),
                black_box(direction),
                black_box(pose),
            ));
        });
        measure(&format!("field-target-sky-miss-{name}"), || {
            let _ = black_box(pick_world_with_context(
                black_box(&chunks),
                black_box(&journal),
                black_box(&properties),
                [8.5, 3.5, 0.5],
                [0.0, 0.0, 1.0],
                black_box(pose),
            ));
        });
    }
    measure("selected-wheat-weather-outline-cpu-only", || {
        let mut emitted = 0;
        prop.visit_weather_edges(black_box(4.25), black_box([0.86, 0.51, 1.0, 1.0]), |edge| {
            black_box(edge);
            emitted += 1;
        });
        assert_eq!(emitted, edges);
    });
    // A ray inside the conservative crop box can miss all thin stems. This
    // isolates broad-phase false positives without replacing geometry by boxes.
    measure("single-aabb-candidate-miss-swept", || {
        let _ = black_box(hit(
            prop,
            [prop.position[0] + 0.46, 1.45, 0.5],
            [0.0, 0.0, 1.0],
            8.0,
            PropPose::WeatherSwept {
                frame: [0.86, 0.51, 1.0, 1.0],
            },
        ));
    });
}
