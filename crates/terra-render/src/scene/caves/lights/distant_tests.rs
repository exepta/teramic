use super::*;
use bevy::mesh::VertexAttributeValues;
use terra_voxel::{BlockSection, SectionPos};
use terra_world::streaming::LoadedSection;

fn definition() -> BlockLight {
    BlockLight {
        color: [1.0, 0.76, 0.40],
        intensity: 1500.0,
        range: 24.0,
        falloff: Default::default(),
        flicker: 0.0,
        cave_intensity: 0.6,
        shadows: false,
        offset: None,
    }
}

fn definitions() -> BTreeMap<u16, BlockLight> {
    crate::blocks::definitions::BlockLibrary::load(
        &terra_core::asset_directory().join("terra/blocks"),
    )
    .unwrap()
    .blocks
    .into_iter()
    .filter_map(|block| block.light.map(|light| (block.id, light)))
    .collect()
}

fn add(chunks: &mut ChunkStore, cell: [i64; 3], material: u16) {
    let (section, local) = global_to_section(cell);
    let loaded = chunks
        .loaded
        .entry(section)
        .or_insert_with(|| LoadedSection {
            data: BlockSection::empty(section).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        });
    loaded.data.set_block(local, material).unwrap();
}

#[test]
fn far_sources_survive_the_near_point_light_budget_but_never_survive_an_edit_or_unload() {
    let light = definition();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let mut cache = Lamps::default();
    for z in [-160, -96, -48, 96] {
        let cell = [1, 3, z];
        add(&mut chunks, cell, terra_core::lantern::LANTERN);
        cache.emitters.push((cell, light));
    }
    let camera = Transform::default();
    let first = candidates(
        &cache,
        &chunks,
        &journal,
        &camera,
        &definitions(),
        192.0,
        256,
    );
    assert_eq!(
        first.len(),
        3,
        "front-facing loaded sources have no near-light radius cutoff"
    );
    assert!(first.iter().any(|(point, _, _)| point.z < -150.0));
    journal.record([1, 3, -96], 0).unwrap();
    chunks.loaded.remove(&global_to_section([1, 3, -160]).0);
    let remaining = candidates(
        &cache,
        &chunks,
        &journal,
        &camera,
        &definitions(),
        192.0,
        256,
    );
    assert_eq!(remaining.len(), 1);
    assert!(remaining[0].0.z > -49.0);
    assert!(
        candidates(
            &cache,
            &chunks,
            &journal,
            &camera,
            &definitions(),
            32.0,
            256
        )
        .is_empty()
    );
}

#[test]
fn a_prop_lantern_on_normal_stone_is_visible_at_eighty_metres_until_removed() {
    use terra_world::props::{PropId, PropKind, WorldProp};
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let support = [1, 2, -80];
    add(&mut chunks, support, 3);
    let section = global_to_section(support).0;
    let prop = WorldProp {
        id: PropId(1),
        kind: PropKind::Scarecrow,
        position: [1.5, 3.0, -79.5],
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section,
        removed: false,
    };
    let id = prop.id;
    assert!(journal.place_prop(prop).unwrap());
    let mut cache = Lamps::default();
    let defs = definitions();
    discover(&mut cache, &chunks, &journal, &defs, Vec3::ZERO, 120.0);
    assert_eq!(cache.emitters.len(), 1);
    let count = |journal: &EditJournal| {
        candidates(
            &cache,
            &chunks,
            journal,
            &Transform::default(),
            &defs,
            120.0,
            4,
        )
        .len()
    };
    assert_eq!(count(&journal), 1);
    journal.remove_prop(section, id).unwrap();
    assert_eq!(count(&journal), 0);
}

#[test]
fn visible_crystals_have_a_real_light_beyond_a_dense_nearby_group() {
    let defs = definitions();
    let crystal = 486;
    let mut chunks = ChunkStore::default();
    let mut emitters = Vec::new();
    for x in 2..13 {
        for z in 2..13 {
            let cell = [x, 2, z];
            add(&mut chunks, cell, crystal);
            emitters.push((cell, defs[&crystal]));
        }
    }
    let far = [2, 2, -80];
    for z in (-80..=0).step_by(16) {
        add(&mut chunks, [2, 2, z], 0);
    }
    add(&mut chunks, far, crystal);
    emitters.push((far, defs[&crystal]));
    let eye = Vec3::new(2.5, 2.5, 0.5);
    let lights = |chunks: &ChunkStore| {
        select_emitters(
            &emitters,
            chunks,
            &EditJournal::default(),
            eye,
            Vec3::NEG_Z,
            4,
        )
    };
    let selected = lights(&chunks);
    assert_eq!(selected.len(), 4);
    assert!(selected.contains_key(&far));
    add(&mut chunks, [2, 2, -50], 3);
    assert!(!lights(&chunks).contains_key(&far));
}

#[test]
fn cemetery_sources_share_the_existing_distant_batch_and_decorative_mist_is_not_a_light() {
    let mut cache = Lamps::default();
    let mut chunks = ChunkStore::default();
    for (i, id) in [358, 369, 370, 371, 372, 373, 386, 395, 396, 397, 399]
        .into_iter()
        .enumerate()
    {
        let cell = [i as i64, 3, -64];
        add(&mut chunks, cell, id);
        cache.emitters.push((cell, definition()));
    }
    let all = candidates(
        &cache,
        &chunks,
        &EditJournal::default(),
        &Transform::default(),
        &definitions(),
        192.,
        budget(CaveQuality::Low),
    );
    assert_eq!(all.len(), 10);
    assert!(all.iter().all(|(p, _, _)| p.x < 10.));
    let bounded = candidates(
        &cache,
        &chunks,
        &EditJournal::default(),
        &Transform::default(),
        &definitions(),
        192.,
        4,
    );
    assert_eq!(bounded.len(), 4);
}

#[test]
fn every_authored_light_can_be_seen_from_a_distance_without_an_id_allowlist() {
    let definitions = definitions();
    let mut cache = Lamps::default();
    let mut chunks = ChunkStore::default();
    for (index, (&material, &light)) in definitions.iter().enumerate() {
        let cell = [index as i64, 3, -80];
        add(&mut chunks, cell, material);
        cache.emitters.push((cell, light));
    }
    let sources = candidates(
        &cache,
        &chunks,
        &EditJournal::default(),
        &Transform::default(),
        &definitions,
        192.0,
        budget(CaveQuality::Low),
    );
    assert_eq!(sources.len(), definitions.len());
    assert!(
        definitions.contains_key(&318),
        "Spirit lanterns are included"
    );
    assert!(
        !definitions.contains_key(&399),
        "Decorative mist is not a lamp"
    );
}

#[test]
fn selection_is_quality_bounded_deterministic_and_independent_of_cache_order() {
    let mut cache = Lamps::default();
    let mut chunks = ChunkStore::default();
    let journal = EditJournal::default();
    for z in 0..600 {
        let cell = [z % 24, 2, -32 - z / 24];
        add(&mut chunks, cell, 40);
        cache.emitters.push((cell, definition()));
    }
    for quality in [
        CaveQuality::Low,
        CaveQuality::Medium,
        CaveQuality::High,
        CaveQuality::Ultra,
    ] {
        let count = budget(quality);
        let first = candidates(
            &cache,
            &chunks,
            &journal,
            &Transform::default(),
            &definitions(),
            192.0,
            count,
        );
        assert_eq!(first.len(), count);
        let first: Vec<_> = first.into_iter().map(|(point, _, _)| point).collect();
        cache.emitters.reverse();
        let second: Vec<_> = candidates(
            &cache,
            &chunks,
            &journal,
            &Transform::default(),
            &definitions(),
            192.0,
            count,
        )
        .into_iter()
        .map(|(point, _, _)| point)
        .collect();
        assert_eq!(first, second);
    }
}

#[test]
fn distance_blend_is_continuous_and_the_batch_retains_real_world_depth() {
    assert_eq!(opacity(0.0, 24.0, 192.0), 0.0);
    assert_eq!(opacity(192.0, 24.0, 192.0), 0.0);
    assert!(opacity(96.0, 24.0, 192.0) > 0.5);
    for distance in (0..1920).map(|value| value as f32 * 0.1) {
        assert!((0.0..=0.72).contains(&opacity(distance, 24.0, 192.0)));
        assert!(
            (opacity(distance + 0.01, 24.0, 192.0) - opacity(distance, 24.0, 192.0)).abs() < 0.002
        );
        assert!((0.06..=0.42).contains(&radius(distance)));
    }
    let center = Vec3::new(-17.5, 12.3, -128.5);
    let geometry = mesh(&[(center, definition(), 0.7)], &Transform::default());
    let Some(VertexAttributeValues::Float32x3(points)) =
        geometry.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("position attribute missing")
    };
    assert_eq!(points.len(), SEGMENTS + 1);
    assert_eq!(Vec3::from_array(points[0]), center);
    assert!(
        points
            .iter()
            .all(|point| (point[2] - center.z).abs() < 0.00001)
    );
    assert_eq!(geometry.indices().unwrap().len(), SEGMENTS * 3);
    let Some(VertexAttributeValues::Float32x4(colors)) = geometry.attribute(Mesh::ATTRIBUTE_COLOR)
    else {
        panic!("color attribute missing")
    };
    assert_eq!(colors[0][3], 0.7);
    assert!(colors[1..].iter().all(|color| color[3] == 0.0));
    let material = material();
    assert_eq!(material.alpha_mode, AlphaMode::Blend);
    assert!(material.fog_enabled && material.unlit);
    // Alpha blend in the normal PBR pass keeps depth testing; there is no
    // overlay camera, custom depth override, or per-source point light.
    assert_eq!(material.depth_bias, 0.0);
    assert_ne!(global_to_section([-17, 12, -129]).0, SectionPos::default());
}

#[test]
fn overlapping_differently_colored_lights_are_submitted_far_to_near() {
    let mut near = definition();
    near.color = [0.2, 0.4, 1.0];
    let mut far = definition();
    far.color = [1.0, 0.3, 0.1];
    let near_point = Vec3::new(0.0, 0.0, -32.0);
    let far_point = Vec3::new(0.0, 0.0, -128.0);
    let geometry = mesh(
        &[(near_point, near, 0.7), (far_point, far, 0.7)],
        &Transform::default(),
    );
    let Some(VertexAttributeValues::Float32x3(points)) =
        geometry.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("position attribute missing")
    };
    let Some(VertexAttributeValues::Float32x4(colors)) = geometry.attribute(Mesh::ATTRIBUTE_COLOR)
    else {
        panic!("color attribute missing")
    };
    assert_eq!(Vec3::from_array(points[0]), far_point);
    assert_eq!(Vec3::from_array(points[SEGMENTS + 1]), near_point);
    assert!(colors[0][0] > colors[0][2]);
    assert!(colors[SEGMENTS + 1][2] > colors[SEGMENTS + 1][0]);
}
