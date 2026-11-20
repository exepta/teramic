use super::*;
use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};
use terra_core::roads;
use terra_world::edits::{BlockObjectState, EditJournal};

fn templates() -> TemplateSet {
    let mut templates = TemplateSet::default();
    for material in roads::TAR_MATERIALS {
        templates.blocks.insert(
            material,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::Boxes,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0, 0, 0, 1, 0, 0],
                side_cap: None,
                details: Vec::new(),
                model: crate::blocks::cube::box_model(
                    terra_core::construction::boxes(material),
                    0.0,
                    [0, 0, 0, 1, 0, 0],
                ),
            },
        );
    }
    templates
}

#[test]
fn tar_films_keep_earth_beneath_them_and_match_pick_and_collision_height_at_every_volume() {
    let templates = templates();
    for material in roads::TAR_MATERIALS {
        for amount in 1..=8 {
            for stage in [0, 15, 30] {
                let facing =
                    roads::tar_state(amount, if material == roads::WET_TAR { stage } else { 0 });
                let owner = SectionPos::new(-1, 0, -1);
                let mut section = BlockSection::empty(owner).unwrap();
                section.set_block([8, 8, 8], material).unwrap();
                let p = [-8, 8, -8];
                let mut journal = EditJournal::default();
                journal.record(p, material).unwrap();
                journal
                    .set_object_state(
                        p,
                        Some(BlockObjectState {
                            facing,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
                let snapshot = capture(&section, &templates, &journal, |_| Some(0)).unwrap();
                let batches = compose(
                    &BlockMesh::default(),
                    &snapshot,
                    &templates,
                    owner,
                    &super::super::builder::RenderOptions {
                        water_level: None,
                        detail_level: super::super::DetailLevel::Near,
                        terrain: None,
                        exposure: None,
                        tints: Default::default(),
                        conditions: Default::default(),
                    },
                )
                .unwrap();
                let soil = batches.iter().find(|b| b.material == 0).unwrap();
                let film = batches.iter().find(|b| b.material == 1).unwrap();
                let bounds = terra_core::construction::placed_boxes(material, facing, [None; 6])[0];
                let height = roads::tar_height(facing);
                for batch in [&soil, &film] {
                    for point in &batch.positions {
                        let local = point.map(|v| f64::from(v - 8.0));
                        assert!((0..3).all(|i| local[i] >= bounds.minimum[i] - 1e-6
                            && local[i] <= bounds.maximum[i] + 1e-6));
                    }
                }
                assert!(
                    soil.positions
                        .iter()
                        .all(|p| p[1] <= 8.0 + roads::PATH_HEIGHT)
                );
                assert!(
                    film.positions
                        .iter()
                        .all(|p| p[1] >= 8.0 + roads::PATH_HEIGHT)
                );
                assert!(film.positions.iter().any(|p| p[1] == 8.0 + height));
                assert!(film.normals.iter().any(|p| p[1] > 0.99));
                if material == roads::WET_TAR {
                    assert_eq!(film.surface_water.len(), film.positions.len());
                    assert!(
                        film.surface_water
                            .iter()
                            .all(|p| *p == [f32::from(stage) / 31.0, -3.0])
                    );
                }
            }
        }
    }
}

#[test]
fn tar_profile_cache_uses_volume_without_rotating_or_caching_every_drying_age() {
    let templates = templates();
    let mut cache = ConstructionProfiles::default();
    for stage in 0..=31 {
        for amount in 1..=8 {
            let model = ModelCell {
                cell: [0; 3],
                material: roads::WET_TAR,
                state: BlockObjectState {
                    facing: roads::tar_state(amount, stage),
                    ..Default::default()
                },
                peer_wood: Default::default(),
                neighbors: [None; 6],
                neighbor_facing: [0; 6],
            };
            let profile = cache.get(&model, &templates).unwrap();
            let highest = profile
                .iter()
                .flat_map(|p| &p.positions)
                .map(|p| p[1])
                .fold(0.0, f32::max);
            assert_eq!(highest, roads::tar_height(model.state.facing));
        }
    }
    assert_eq!(cache.0.len(), 8);
}

#[test]
fn tar_leading_film_advances_over_visible_prepared_soil_and_tracks_neighbor_pressure_in_cache() {
    let mut templates = templates();
    let mut path = templates.blocks[&roads::WET_TAR].clone();
    path.face_materials = [2; 6];
    templates.blocks.insert(terra_world::roads::PATH, path);
    let mut model = ModelCell {
        cell: [0; 3],
        material: roads::WET_TAR,
        state: BlockObjectState {
            facing: roads::tar_state(1, 0),
            liquid: Some(terra_world::edits::LiquidState {
                origin: [-1, 0, 0],
                volume: 1,
                age: 0,
                source: false,
            }),
            ..Default::default()
        },
        peer_wood: Default::default(),
        neighbors: [
            Some(roads::WET_TAR),
            Some(terra_world::roads::PATH),
            None,
            None,
            None,
            None,
        ],
        neighbor_facing: [64, 0, 0, 0, 0, 0],
    };
    let mut cache = ConstructionProfiles::default();
    let width = |profile: &[crate::blocks::PrimitiveTemplate]| {
        let film = profile.iter().filter(|face| face.material == 1);
        film.flat_map(|face| &face.positions)
            .map(|p| p[0])
            .fold(0., f32::max)
    };
    let full_head = cache.get(&model, &templates).unwrap().into_owned();
    assert!((width(&full_head) - 1. / 32.).abs() < 0.00001);
    let soil_top = full_head.iter().find(|face| face.material == 2).unwrap();
    assert!(soil_top.positions.iter().any(|p| p[0] == 1.));
    assert!(
        soil_top
            .positions
            .iter()
            .all(|p| p[1] == roads::PATH_HEIGHT)
    );
    model.neighbor_facing[0] = 32;
    let half_head = cache.get(&model, &templates).unwrap().into_owned();
    assert!((width(&half_head) - 1. / 16.).abs() < 0.00001);
    assert_eq!(
        cache.0.len(),
        2,
        "incoming height participates in front geometry cache"
    );
}
