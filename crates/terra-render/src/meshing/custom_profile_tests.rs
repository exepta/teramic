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
fn snapshot_checks_colliding_material_ids_inside_and_across_the_section_border() {
    let mut templates = templates();
    let model_id = 82;
    let plain_id = model_id + 32;
    templates.blocks.get_mut(&model_id).unwrap().model =
        crate::blocks::cube::box_model(terra_core::construction::boxes(model_id), 0.0, [0; 6]);
    let mut section = BlockSection::empty(SectionPos::new(0, 0, 0)).unwrap();
    for (x, id) in [model_id, plain_id, model_id].into_iter().enumerate() {
        section.set_block([x, 0, 0], id).unwrap();
    }
    let snapshot = capture(&section, &templates, &Default::default(), |p| match p {
        [-1, 0, 0 | 2] => Some(model_id),
        [-1, 0, 1] => Some(plain_id),
        _ => None,
    })
    .unwrap();
    assert_eq!(
        snapshot
            .models
            .iter()
            .map(|cell| cell.cell)
            .collect::<Vec<_>>(),
        [[0, 0, 0], [2, 0, 0]]
    );
    assert_eq!(snapshot.border_models, [[-1, 0, 0], [-1, 0, 2]]);
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
fn village_roof_geometry_matches_before_and_after_server_subscription() {
    use crate::meshing::DetailLevel::{Far, Middle, Near};
    use terra_core::{WorldConfig, WorldSeed, construction};
    use terra_voxel::global_to_section;
    use terra_world::{SectionGenerator, TerraGenerator, edits::EditJournal};

    // A pitched house roof in seed 1337 straddles the x=208 section edge.
    let generator = SectionGenerator::new(
        TerraGenerator::new(WorldConfig {
            seed: WorldSeed(1337),
            structures_revision: 3,
            landscape_revision: 0,
            geology_revision: 0,
            ocean_revision: 0,
            cave_structure_revision: 0,
            base_height: 16.,
            sea_level: 0.,
            hill_amplitude: 0.,
            mountain_amplitude: 0.,
            terrace_strength: 0.,
            caves: false,
            trees: false,
            water: false,
            rivers: false,
            biomes: false,
            ..Default::default()
        })
        .unwrap(),
    );
    let owner = SectionPos::new(13, 1, 10);
    let column = generator.prepare_column([owner.x, owner.z], true).unwrap();
    let generated = generator
        .build_prepared(owner, 0, &Default::default(), &column)
        .unwrap();
    let mut data = BlockSection::empty(owner).unwrap();
    let mut assets = templates();
    let stairs: Vec<_> = column
        .model_objects_in_halo(owner)
        .filter(|o| construction::is_stair(o.material))
        .cloned()
        .collect();
    for stair in &stairs {
        let (section, cell) = global_to_section(stair.position);
        if section != owner {
            continue;
        }
        assert_eq!(generated.data.block(cell).unwrap(), stair.material);
        data.set_block(cell, stair.material).unwrap();
        let asset = assets.blocks.get_mut(&stair.material).unwrap();
        asset.model = crate::blocks::cube::box_model(
            &construction::placed_boxes(stair.material, 0, [None; 6]),
            asset.bevel_width,
            asset.face_materials,
        );
    }
    assert!(
        stairs
            .iter()
            .any(|o| global_to_section(o.position).0 != owner)
    );
    let loaded = |p| {
        let (section, cell) = global_to_section(p);
        (section == owner).then(|| data.block(cell).unwrap())
    };
    let snapshot = |journal: &EditJournal| {
        capture_prepared(
            &data,
            &assets,
            journal,
            Some(&column),
            &journal.snapshot(owner).unwrap(),
            loaded,
        )
        .unwrap()
    };
    let mut journal = EditJournal::default();
    let distant = snapshot(&journal);
    let mut directions = std::collections::HashSet::new();
    for model in &distant.models {
        directions.insert(model.state.facing);
    }
    assert!(
        directions.len() >= 2,
        "both pitches must have their own pose"
    );
    assert_eq!(journal.edited_blocks(), 0);
    journal.initialize_structures(&stairs).unwrap();
    let confirmed = snapshot(&journal);
    let geometry = |snapshot: &ModelSnapshot, detail_level| {
        compose(
            &BlockMesh::default(),
            snapshot,
            &assets,
            owner,
            &RenderOptions {
                detail_level,
                terrain: None,
                water_level: None,
                exposure: None,
                conditions: Default::default(),
                tints: Default::default(),
            },
        )
        .unwrap()
    };
    let expected = geometry(&confirmed, Near);
    assert!(expected.iter().any(|b| !b.positions.is_empty()));
    for level in [Near, Middle, Far] {
        assert!(
            geometry(&distant, level) == expected,
            "{level:?} changed the roof geometry"
        );
    }
    // Only the adjacent section's network delta is present: the owning stair
    // must still use the server's rotation when selecting its corner geometry.
    let (model, side, neighbor) = distant
        .models
        .iter()
        .find_map(|model| {
            (0..6).find_map(|side| {
                let mut p = owner.global_block(model.cell.map(|v| v as usize)).unwrap();
                p[side / 2] += if side % 2 == 0 { -1 } else { 1 };
                (global_to_section(p).0 != owner
                    && model.neighbors[side].is_some_and(construction::is_stair))
                .then_some((model, side, p))
            })
        })
        .expect("the roof row must continue into an unpublished section");
    let mut rotated = journal.object_state(neighbor).unwrap();
    rotated.facing = (rotated.facing + 1) % 4;
    journal.set_object_state(neighbor, Some(rotated)).unwrap();
    let mut replica = EditJournal::default();
    replica
        .import_delta(journal.section_delta(global_to_section(neighbor).0))
        .unwrap();
    let mixed = snapshot(&replica);
    let corner = mixed.models.iter().find(|m| m.cell == model.cell).unwrap();
    assert_eq!(corner.state, model.state);
    assert_eq!(corner.neighbor_facing[side], rotated.facing);
    let stair = stairs
        .iter()
        .find(|o| global_to_section(o.position).0 == owner && o.state.unwrap().facing != 0)
        .unwrap();
    journal.set_object_state(stair.position, None).unwrap();
    let cell = global_to_section(stair.position).1.map(|v| v as i32);
    assert_eq!(
        snapshot(&journal)
            .models
            .iter()
            .find(|m| m.cell == cell)
            .unwrap()
            .state
            .facing,
        0
    );
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

fn deck_uv_at(profile: &[PrimitiveTemplate], material: usize, point: bevy::math::Vec2) -> [f32; 2] {
    use bevy::math::{Vec2, Vec3};
    for primitive in profile.iter().filter(|p| p.material == material) {
        for triangle in primitive.indices.chunks_exact(3) {
            let ids = [
                triangle[0] as usize,
                triangle[1] as usize,
                triangle[2] as usize,
            ];
            if ids.iter().any(|&i| primitive.normals[i][1] < 0.999) {
                continue;
            }
            let p = ids.map(|i| Vec3::from_array(primitive.positions[i]));
            let projected = p.map(|p| Vec2::new(p.x, p.z));
            let a = projected[1] - projected[0];
            let b = projected[2] - projected[0];
            let determinant = a.perp_dot(b);
            if determinant.abs() < 0.000001 {
                continue;
            }
            let offset = point - projected[0];
            let second = offset.perp_dot(b) / determinant;
            let third = a.perp_dot(offset) / determinant;
            let weights = [1.0 - second - third, second, third];
            if weights.iter().any(|w| *w < -0.00001) {
                continue;
            }
            let uv = ids.map(|i| Vec2::from_array(primitive.uvs[i]));
            let duv = [uv[1] - uv[0], uv[2] - uv[0]];
            let uv_determinant = duv[0].perp_dot(duv[1]);
            let tangent = ((p[1] - p[0]) * duv[1].y - (p[2] - p[0]) * duv[0].y) / uv_determinant;
            let bitangent = ((p[2] - p[0]) * duv[0].x - (p[1] - p[0]) * duv[1].x) / uv_determinant;
            let normal = Vec3::from_array(primitive.normals[ids[0]]);
            for i in ids {
                let frame = primitive.tangents[i];
                let stored = Vec3::new(frame[0], frame[1], frame[2]);
                assert!(stored.dot(tangent.normalize()) > 0.999);
                assert!((normal.cross(stored) * frame[3]).dot(bitangent.normalize()) > 0.999);
            }
            return (uv[0] * weights[0] + uv[1] * weights[1] + uv[2] * weights[2]).to_array();
        }
    }
    panic!("missing walking-deck triangle at {point:?}");
}

#[test]
fn sidewalk_arcs_have_a_round_street_face_and_matching_free_corner_in_every_heading() {
    use bevy::math::Vec3;
    use terra_core::{construction, street};
    for id in [street::ARC_LEFT, street::ARC_RIGHT] {
        for facing in 0..4 {
            let mesh = crate::blocks::sidewalk::arc(id, facing, [0, 0, 0, 1, 0, 0]);
            let top = mesh.iter().find(|p| p.material == 1).unwrap();
            let center = Vec3::from_array(top.positions[0]);
            let mut area = 0.;
            for tri in top.indices.chunks_exact(3) {
                let [a, b, c] =
                    [tri[0], tri[1], tri[2]].map(|i| Vec3::from_array(top.positions[i as usize]));
                let n = (b - a).cross(c - a);
                assert!(n.y > 0.);
                area += n.y / 2.;
                assert_eq!(a.y, 0.25);
                assert!(((b - center).length() - 1.).abs() < 0.0001);
            }
            assert!((area - std::f32::consts::FRAC_PI_4).abs() < 0.001);
            let contacts = construction::placed_boxes(id, facing, [None; 6]);
            for x in 0..32 {
                for z in 0..32 {
                    let p = Vec3::new((x as f32 + 0.5) / 32., center.y, (z as f32 + 0.5) / 32.);
                    let radius = (p - center).length();
                    if (radius - 1.).abs() < 0.025 {
                        continue;
                    }
                    let inside = contacts.iter().any(|b| {
                        [0, 2]
                            .into_iter()
                            .all(|i| (b.minimum[i]..=b.maximum[i]).contains(&f64::from(p[i])))
                    });
                    assert_eq!(inside, radius < 1.);
                }
            }
        }
    }
}

#[test]
fn sidewalk_paint_and_normal_frames_follow_the_adjacent_road_in_every_facing() {
    use bevy::{
        image::{CompressedImageFormats, ImageSampler, ImageType},
        math::Vec2,
        prelude::Image,
    };
    use terra_core::{block_shapes::ShapeBox, street};
    let directory = terra_core::asset_directory().join("terra/blocks");
    let image = |name: &str| {
        Image::from_buffer(
            &std::fs::read(directory.join(name)).unwrap(),
            ImageType::Extension("png"),
            CompressedImageFormats::empty(),
            true,
            ImageSampler::default(),
            bevy::asset::RenderAssetUsages::MAIN_WORLD,
        )
        .unwrap()
    };
    let brightness = |image: &Image, uv: [f32; 2]| {
        let size = image.texture_descriptor.size;
        let x = (uv[0].clamp(0., 1.) * (size.width - 1) as f32).round() as usize;
        let y = (uv[1].clamp(0., 1.) * (size.height - 1) as f32).round() as usize;
        let pixel = (x + y * size.width as usize) * 4;
        image.data.as_ref().unwrap()[pixel..pixel + 3]
            .iter()
            .map(|v| f32::from(*v))
            .sum::<f32>()
    };
    let paving = image("textures/street/paving.png");
    for (id, slug) in [
        (street::NORMAL, "sidewalk"),
        (street::LEFT, "sidewalk_left"),
        (street::RIGHT, "sidewalk_right"),
        (street::ARC_LEFT, "sidewalk_arc_left"),
        (street::ARC_RIGHT, "sidewalk_arc_right"),
        (street::LOWERED, "sidewalk_lowered"),
    ] {
        let definition: crate::blocks::definitions::BlockDefinition = serde_json::from_str(
            &std::fs::read_to_string(directory.join(format!("{slug}.json"))).unwrap(),
        )
        .unwrap();
        let mut templates = templates();
        let deck_material = 1000;
        let mut template = templates.blocks[&98].clone();
        template.bevel_width = definition.bevel_width;
        template.face_materials = definition
            .face_materials
            .unwrap()
            .map(|name| if name == "Deck" { deck_material } else { 999 });
        templates.blocks.insert(id, template);
        let deck = image(&definition.textures["Deck"]);
        let x = match id {
            street::ARC_LEFT => 0.875,
            street::ARC_RIGHT => 0.125,
            _ => 0.5,
        };
        for road in [[0, 0, -1], [-1, 0, 0], [0, 0, 1], [1, 0, 0]] {
            let facing = street::placement_facing(id, [0; 3], [0., 0., -1.], |p| {
                (p == road).then_some(terra_core::roads::ASPHALT)
            });
            let profile = construction_profile(&cell(id, facing), &templates).unwrap();
            let placed = |z| {
                let p = [x, 0., z];
                let rotated = ShapeBox {
                    minimum: p,
                    maximum: p,
                    hinged: false,
                }
                .rotated_y(facing)
                .minimum;
                Vec2::new(rotated[0] as f32, rotated[2] as f32)
            };
            let edge = placed(0.1);
            assert!(
                (edge - Vec2::splat(0.5)).dot(Vec2::new(road[0] as f32, road[2] as f32)) > 0.35
            );
            let dark_uv = deck_uv_at(&profile, deck_material, edge);
            let light_uv = deck_uv_at(&profile, deck_material, placed(0.9));
            let curb_shade = brightness(&deck, dark_uv) / brightness(&paving, dark_uv);
            let deck_shade = brightness(&deck, light_uv) / brightness(&paving, light_uv);
            assert!(
                (0.73..0.79).contains(&curb_shade),
                "id={id} facing={facing} curb={curb_shade}"
            );
            assert!(
                (0.99..1.01).contains(&deck_shade),
                "id={id} facing={facing} deck={deck_shade}"
            );
        }
    }
}
