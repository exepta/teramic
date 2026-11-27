use super::*;
use crate::{
    SectionGenerator, TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::WorldConfig;
use terra_voxel::SectionPos;
pub(super) fn load_surface(generator: &SectionGenerator, home: [i64; 2]) -> ChunkStore {
    let mut chunks = ChunkStore::default();
    for dx in 0..2 {
        for dz in 0..2 {
            let column = [home[0] * 2 + dx, home[1] * 2 + dz];
            let prepared = std::sync::Arc::new(generator.prepare_column(column, false).unwrap());
            let mut low = i64::MAX;
            let mut high = i64::MIN;
            for x in 0..16 {
                for z in 0..16 {
                    let s = prepared.local_surface(x, z).unwrap();
                    low = low.min((s.height.floor() as i64 - 3).div_euclid(16));
                    high = high
                        .max((s.lava_level.unwrap_or(s.height).ceil() as i64 + 4).div_euclid(16));
                }
            }
            for y in low..=high {
                let p = SectionPos::new(column[0], y, column[1]);
                let g = generator
                    .build_prepared(p, 0, &Default::default(), &prepared)
                    .unwrap();
                chunks.loaded.insert(
                    p,
                    LoadedSection {
                        data: g.data,
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: g.water,
                        props: g.props,
                    },
                );
            }
            chunks.set_prepared_column_for_test(column, Some(prepared));
        }
    }
    chunks
}

#[test]
fn generated_savanna_herds_have_ground_and_forage_on_their_native_turf() {
    let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
    let generator = SectionGenerator::new(terrain.clone());
    let journal = EditJournal::default();
    let mut found = [false; 2];
    // The default seed has an open acacia corridor west of X=0 at Z=960..1440.
    for hz in 29..=46 {
        for hx in -9..=1 {
            let home = [hx, hz];
            let seed = random(creature_id(home, 0).unwrap(), terrain.config().seed.0);
            if (seed >> 32).is_multiple_of(3) {
                continue;
            }
            let slot = usize::from(!(seed >> 36).is_multiple_of(3));
            if found[slot] {
                continue;
            }
            let x = (hx * 32 + 8 + (seed % 8) as i64) as f64 + 0.5;
            let z = (hz * 32 + 8 + ((seed >> 8) % 8) as i64) as f64 + 0.5;
            let surface = terrain.surface_column(x, z);
            if biome_slug(terrain.surface_biome(&surface)) != "savanna"
                || terrain.water_column(x, z).is_some()
            {
                continue;
            }
            let chunks = load_surface(&generator, home);
            let h = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            let mut pop = CreaturePopulation::default();
            assert!(spawn_cell(&mut pop, &h, home));
            let kind = if slot == 0 {
                Kind::Elephant
            } else {
                Kind::Zebra
            };
            if pop.animals.values().filter(|a| a.kind == kind).count() < 2 {
                continue;
            }
            for a in pop.animals.values_mut().filter(|a| a.kind == kind) {
                assert!(
                    a.valid()
                        && h.ground_rotated(a.position, kind, f64::from(a.yaw))
                            .is_some()
                );
                let [x, y, z] = a.position.map(|n| n.floor() as i64);
                if !matches!(
                    h.block([x, y - 1, z])
                        .and_then(terra_voxel::MaterialId::from_raw),
                    Some(
                        terra_voxel::MaterialId::SavannaTurf
                            | terra_voxel::MaterialId::DrySavannaTurf
                    )
                ) {
                    continue;
                }
                a.needs.food = 2.;
                a.needs.search_in = 0.;
                for _ in 0..40 {
                    ecology::step(a, &mut pop.ecology, &h, &[], 0.1, true, false);
                }
                assert!(a.needs.food > 6., "{kind:?} failed to graze: {:?}", a.needs);
                found[slot] = true;
                eprintln!(
                    "Natural {kind:?} herd, seed {}: {:?}",
                    terrain.config().seed.0,
                    a.position
                );
                break;
            }
            if found == [true; 2] {
                return;
            }
        }
    }
    panic!("missing naturally grazing elephant or zebra herd: {found:?}");
}
