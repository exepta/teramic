//! Inspect assembly berths in a disposable world copy; never write to its journal.
use std::{collections::BTreeSet, path::PathBuf};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().ok_or("world-library directory required")?);
    let id = terra_core::WorldId(args.next().ok_or("world id required")?.parse()?);
    let library = terra_storage::WorldLibrary { root };
    let entry = library.load(id)?;
    let journal: EditJournal = terra_storage::load_edits(&library.path(id), id)?;
    let terrain = terra_world::TerraGenerator::new(entry.manifest.world)?;
    let generator = terra_world::SectionGenerator::new(terrain.clone());
    if std::env::var_os("TERRA_YARD_WATER").is_some() {
        let mut seen = BTreeSet::new();
        for (cell, material, _) in journal.object_states() {
            if !terra_core::rails::is_rail(material) {
                continue;
            }
            let Some(piece) = terra_world::rails::owner_at(&journal, cell) else {
                continue;
            };
            if !seen.insert(piece.anchor) {
                continue;
            }
            for path in piece.active_paths() {
                let middle = path.sample_distance(path.length() * 0.5).position;
                if let Some(water) = terrain.water_column(middle[0], middle[2]) {
                    println!(
                        "water_track={middle:?} level={} anchor={:?} curve={}",
                        water.level,
                        piece.anchor,
                        piece.curve.is_some()
                    );
                }
            }
        }
    }
    let stations = journal
        .object_states()
        .filter_map(|(cell, material, _)| (material == 217).then_some(cell))
        .collect::<Vec<_>>();
    let mut owners = BTreeSet::new();
    for &station in &stations {
        let a =
            terra_voxel::global_to_section([station[0] - 24, station[1] - 4, station[2] - 24]).0;
        let b =
            terra_voxel::global_to_section([station[0] + 24, station[1] + 16, station[2] + 24]).0;
        for x in a.x..=b.x {
            for y in a.y..=b.y {
                for z in a.z..=b.z {
                    owners.insert(terra_voxel::SectionPos::new(x, y, z));
                }
            }
        }
    }
    let mut chunks = ChunkStore::default();
    for owner in owners {
        let generated = generator.build(
            owner,
            journal.revision(owner),
            &journal.snapshot(owner)?,
            false,
        )?;
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data: generated.data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
    }
    println!(
        "stations={} loaded_sections={}",
        stations.len(),
        chunks.loaded.len()
    );
    let actor = journal.gameplay().inventories.keys().next().copied();
    for station in stations {
        let track = terra_world::train_yard::track(&chunks, &journal, station);
        println!("station={station:?} track={track:?}");
        for wheelbase in [3, 8, 15] {
            let guide = terra_world::train_yard::blueprint(
                &chunks,
                &journal,
                None,
                station,
                Default::default(),
                wheelbase,
                3,
            );
            if let Some(guide) = guide {
                let chassis = guide
                    .cells
                    .iter()
                    .find(|part| part.material == 210)
                    .unwrap();
                let built = actor.map(|actor| {
                    terra_world::trains::plan_blueprint(
                        &chunks,
                        &journal,
                        terra_world::trains::BuildRequest {
                            actor,
                            station,
                            chassis: chassis.cell,
                            facing: chassis.facing,
                            kind: Default::default(),
                            wheelbase,
                            width: 3,
                            creative: true,
                        },
                    )
                    .map(|plan| plan.cells.len())
                });
                println!(
                    "wheelbase={wheelbase} guide={} chassis={:?} build={built:?}",
                    guide.cells.len(),
                    chassis.cell
                );
            } else {
                println!("wheelbase={wheelbase} guide=none");
            }
        }
    }
    Ok(())
}
