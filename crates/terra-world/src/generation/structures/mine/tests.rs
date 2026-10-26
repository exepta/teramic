use super::*;
use crate::{SectionGenerator, edits::EditJournal, generation::structures::StructureKind};
use terra_core::{WorldConfig, WorldSeed};

fn terrain(seed: u64, hills: f64, revision: u8) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        seed: WorldSeed(seed),
        structures_revision: revision,
        landscape_revision: 0,
        geology_revision: 0,
        ocean_revision: 0,
        base_height: 72.0,
        hill_amplitude: hills,
        mountain_amplitude: 0.0,
        terrace_strength: 0.0,
        caves: false,
        trees: false,
        water: false,
        rivers: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn current_mine_chests_face_the_exit_in_every_cardinal_adit_and_old_poses_remain_stable() {
    for inward in DIRECTIONS {
        let facing = chest_facing(inward, 3);
        let front = [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]][usize::from(facing)];
        assert_eq!(front, inward.map(|v| -v));
        assert_eq!((chest_facing(inward, 2) + 2) % 4, facing);
        assert_eq!(chest_facing(inward, 1), chest_facing(inward, 2));
    }
}

#[test]
fn flat_ground_has_no_revision_two_mine_but_retains_the_revision_one_descending_mine() {
    let flat = terrain(1, 0.0, 2);
    assert!(find(&flat, [192, 192], 7).is_none());
    assert!(super::super::build::plan(&flat, [0, 0], 7).is_none());
    let legacy = terrain(1, 0.0, 1);
    let site = super::super::build::plan(&legacy, [0, 0], 7).unwrap();
    assert_eq!(site.summary.kind, StructureKind::Mine);
    assert!(site.summary.bounds[0][1] < site.summary.center[1] - 8);
}

#[test]
fn cover_validation_checks_real_natural_density_roof_support_and_all_four_orientations() {
    let terrain = terrain(7, 0.0, 2);
    for inward in DIRECTIONS {
        let adit = Adit {
            start: [-160, 40, -160],
            inward,
            mouth: 5,
            length: 24,
        };
        let mut natural = Natural::new(&terrain);
        for step in -3..=adit.end() + 1 {
            for lateral in -3..=3 {
                let p = adit.point(step, lateral, 0);
                let mut column = terrain.surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5);
                column.height = if step == -3 { 40.0 } else { 60.0 };
                column.soil_depth = 0.0;
                column.surface_material = terra_voxel::MaterialId::Stone;
                natural.columns.insert([p[0], p[2]], column);
            }
        }
        assert!(natural.covered(adit));
        let p = adit.point(adit.mouth + 9, 0, 0);
        let key = [p[0], p[2]];
        let original = natural.columns[&key];
        natural.columns.get_mut(&key).unwrap().height = 42.0;
        assert!(
            !natural.covered(adit),
            "an exposed ceiling is not a mountain adit"
        );
        natural.columns.insert(key, original);
        natural.columns.get_mut(&key).unwrap().river_cavity = Some([35.0, 55.0]);
        assert!(
            !natural.covered(adit),
            "an actual eroded cavity must not become an unsupported mine"
        );
        natural.columns.insert(key, original);
        let approach = adit.point(-1, 0, 0);
        natural
            .columns
            .get_mut(&[approach[0], approach[2]])
            .unwrap()
            .height = 36.0;
        assert!(
            !natural.covered(adit),
            "a portal approach cannot bridge an unsupported shaft"
        );
        assert!(natural.columns.len() < MAX_COLUMNS);
    }
}

#[test]
fn actual_seeded_mountain_adits_are_horizontal_covered_deterministic_and_inside_the_region() {
    for seed in [1, 71, 42042] {
        let terrain = terrain(seed, 180.0, 2);
        let adit = (-2..=2)
            .flat_map(|z| (-2..=2).map(move |x| [x, z]))
            .find_map(|region| {
                let center = region.map(|v| v * REGION_SIZE + 192);
                find(&terrain, center, 7).map(|adit| (center, adit))
            })
            .unwrap_or_else(|| {
                panic!("seed {seed} has no valid mountain slope in the bounded fixture")
            });
        let (center, adit) = adit;
        assert_eq!(find(&terrain, center, 7), Some(adit));
        assert!(Natural::new(&terrain).covered(adit));
        let outside = adit.point(-3, 0, 0);
        let outside_height =
            terrain.surface_height(outside[0] as f64 + 0.5, outside[2] as f64 + 0.5);
        assert_eq!(adit.start[1], (outside_height - 0.5).floor() as i64 + 1);
        assert_eq!(
            adit.point(adit.mouth, 0, 0)[1],
            adit.point(adit.end(), 0, 0)[1]
        );
        let uphill = terrain.surface_height(
            adit.point(28, 0, 0)[0] as f64 + 0.5,
            adit.point(28, 0, 0)[2] as f64 + 0.5,
        );
        assert!(uphill >= adit.start[1] as f64 + 12.0);
        for (i, axis) in [0, 2].into_iter().enumerate() {
            let origin = center[i].div_euclid(REGION_SIZE) * REGION_SIZE;
            assert!(adit.bounds()[0][axis] >= origin + 96);
            assert!(adit.bounds()[1][axis] <= origin + 288);
        }
    }
}

#[test]
fn real_mountain_mine_voxels_halos_wall_torches_and_loot_use_existing_generation_and_journal() {
    let terrain = terrain(42042, 180.0, 2);
    let (region, site) = (-3..=3)
        .flat_map(|z| (-3..=3).map(move |x| [x, z]))
        .filter_map(|region| terrain.planned_site(region).map(|site| (region, site)))
        .find(|(_, site)| site.summary.kind == StructureKind::Mine)
        .expect("bounded mountainous region fixture contains a real lateral mine");
    let entrance = site.summary.buildings[0].entrance;
    eprintln!(
        "mountain mine fixture seed=42042 region={region:?} entrance={entrance:?} bounds={:?}",
        site.summary.bounds
    );
    assert_eq!(site.summary.center, entrance);
    assert_eq!(
        site.summary.buildings.len(),
        1,
        "no surface hut disguises the entrance"
    );
    let objects: Vec<_> = site
        .sections
        .values()
        .flat_map(|patch| patch.objects.iter().cloned())
        .collect();
    let mut journal = EditJournal::default();
    assert!(!journal.initialize_structures(&objects).unwrap().is_empty());
    assert!(journal.initialize_structures(&objects).unwrap().is_empty());
    assert_eq!(site.summary.containers.len(), 1);
    assert!(
        journal
            .container_at(site.summary.containers[0])
            .unwrap()
            .occupied_slots()
            > 0
    );
    assert!(
        objects
            .iter()
            .filter(|object| object.material == 40)
            .count()
            >= 2
    );
    for torch in objects.iter().filter(|object| object.material == 40) {
        let state = torch.state.unwrap();
        let normal = match state.facing {
            4 => [1, 0, 0],
            5 => [-1, 0, 0],
            6 => [0, 0, 1],
            7 => [0, 0, -1],
            facing => panic!("unsupported wall torch facing {facing}"),
        };
        let support = std::array::from_fn(|i| torch.position[i] - normal[i]);
        let material = if let Some(material) = site.block_at(support) {
            material
        } else {
            let sample = terrain.sample_with_column(
                support.map(|v| v as f64 + 0.5),
                terrain.surface_column(support[0] as f64 + 0.5, support[2] as f64 + 0.5),
            );
            assert!(sample.density <= 0.0, "torch support {support:?} is air");
            sample.material
        };
        assert!(
            terra_voxel::MaterialId::from_raw(material)
                .unwrap()
                .is_solid()
        );
    }
    let generator = SectionGenerator::new(terrain.clone());
    for owner in [
        terra_voxel::global_to_section(entrance).0,
        terra_voxel::global_to_section(site.summary.containers[0]).0,
    ] {
        let prepared = generator.prepare_column([owner.x, owner.z], true).unwrap();
        let section = generator
            .build_prepared(owner, 0, &Default::default(), &prepared)
            .unwrap();
        for &(index, material) in &site.sections[&owner].cells {
            let i = usize::from(index);
            assert_eq!(
                section.data.block([i % 16, i / 16 % 16, i / 256]).unwrap(),
                material
            );
        }
        let start = owner.origin().unwrap().map(|v| v - 1);
        let mut halo = vec![u16::MAX; 18_usize.pow(3)];
        site.stamp(start, 18, &mut halo);
        for z in [0, 17] {
            for x in [0, 17] {
                for y in 0..18 {
                    let p = [
                        start[0] + x as i64,
                        start[1] + y as i64,
                        start[2] + z as i64,
                    ];
                    if let Some(material) = site.block_at(p) {
                        assert_eq!(halo[x + 18 * (y + 18 * z)], material);
                        assert_eq!(generator.block_at(p).unwrap(), material);
                    }
                }
            }
        }
    }
}
