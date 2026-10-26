use super::*;
use crate::{
    SectionGenerator, TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{WorldConfig, creatures::CreaturePopulation};
use terra_voxel::SectionPos;

#[test]
fn residents_really_explore_generated_houses_and_paths_without_walking_in_place() {
    let terrain = TerraGenerator::new(WorldConfig {
        trees: false,
        caves: false,
        ecology_revision: 0,
        structures_revision: 4,
        ..Default::default()
    })
    .unwrap();
    let settlement = terrain
        .village_settlement([-2, 2])
        .expect("seed 42042 village");
    let generator = SectionGenerator::new(terrain.clone());
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let mut columns = BTreeSet::new();
    for site in &settlement.residents {
        for z in (site.home_bounds[0][2] - 12).div_euclid(16)
            ..=(site.home_bounds[1][2] + 12).div_euclid(16)
        {
            for x in (site.home_bounds[0][0] - 12).div_euclid(16)
                ..=(site.home_bounds[1][0] + 12).div_euclid(16)
            {
                columns.insert([x, z]);
            }
        }
    }
    for [x, z] in columns {
        let prepared = generator.prepare_column([x, z], false).unwrap();
        for y in -1..=7 {
            let owner = SectionPos::new(x, y, z);
            let section = generator
                .build_prepared(owner, 0, &Default::default(), &prepared)
                .unwrap();
            journal
                .initialize_structures(&prepared.structure_objects(owner))
                .unwrap();
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data: section.data,
                    mesh: None,
                    water: section.water,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    props: section.props,
                },
            );
        }
    }
    let mut residents = Residents::default();
    let mut population = CreaturePopulation::default();
    for site in &settlement.residents {
        residents
            .sites
            .insert(resident_id(site).unwrap(), site.clone());
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    residents.discover(&mut population, &h, &[settlement.center.map(|v| v as f64)]);
    assert!(population.animals.len() >= 6);
    let mut nav = navigation::Navigation::default();
    let mut moved = BTreeMap::<u64, f64>::new();
    let mut last_motion = BTreeMap::<u64, f64>::new();
    for _ in 0..400 {
        for a in population.animals.values_mut() {
            let before = a.position;
            let door = residents.step(
                a,
                &Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal,
                },
                0.5,
                0.1,
                true,
                &mut nav,
            );
            if let Some(door) = door {
                let state = journal.object_state(door).unwrap();
                for cell in [Some(door), state.peer].into_iter().flatten() {
                    let mut state = journal.object_state(cell).unwrap();
                    state.facing = state.facing % 4 + 4;
                    journal.set_object_state(cell, Some(state)).unwrap();
                }
            }
            let step = (a.position[0] - before[0]).hypot(a.position[2] - before[2]);
            assert!(step <= 0.25, "resident moved discontinuously: {step}");
            *moved.entry(a.id).or_default() += step;
            if step > 0.02 {
                last_motion.insert(a.id, a.age);
            }
            assert!(a.valid());
        }
    }
    for a in population.animals.values() {
        eprintln!(
            "resident={} moved={:.2} position={:?} behavior={:?} goal={:?}",
            a.id, moved[&a.id], a.position, a.behavior, a.needs.goal
        );
        assert!(
            a.age - last_motion.get(&a.id).copied().unwrap_or(0.0) < 12.0,
            "resident {} stopped for {:.1}s at {:?}",
            a.id,
            a.age - last_motion.get(&a.id).copied().unwrap_or(0.0),
            a.position
        );
        if moved[&a.id] < 5.0 {
            let h = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            for n in 0..8 {
                let angle = n as f64 * std::f64::consts::FRAC_PI_4;
                let target = [
                    a.position[0] + angle.sin(),
                    a.position[1],
                    a.position[2] + angle.cos(),
                ];
                eprintln!(
                    "escape {n} => {:?} ground={:?}",
                    h.walk_step(a.position, target, a.kind, angle),
                    h.ground(target, a.kind)
                );
            }
        }
    }
    let exploring = moved.values().filter(|&&distance| distance > 5.0).count();
    assert_eq!(
        exploring,
        moved.len(),
        "only {exploring}/{} residents moved over five meters",
        moved.len()
    );
}
