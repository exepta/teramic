//! Read-only inspection of a real save; never opens a server or writes the world.
use std::{collections::BTreeMap, path::PathBuf};
use terra_core::{WorldId, kingdom::ClaimKind};

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !matches!(args.len(), 2 | 5 | 6 | 7) {
        return Err("Usage: inspect_building_world <world directory> <decimal world ID> [x y z [facing [template ID]]]".into());
    }
    let path = PathBuf::from(&args[0]);
    let id = WorldId(
        args[1]
            .to_str()
            .ok_or("Invalid ID")?
            .parse()
            .map_err(|_| "Invalid ID")?,
    );
    let library = terra_storage::WorldLibrary {
        root: path
            .parent()
            .ok_or("World directory needs a parent")?
            .into(),
    };
    let entry = library.load(id)?;
    let journal = terra_storage::load_edits(&path, id)?;
    println!(
        "world={} seed={} edits={}",
        entry.title,
        entry.manifest.world.seed.0,
        journal.edited_blocks()
    );
    for (player, location) in journal.player_locations() {
        println!("player={player} location={location:?}");
    }
    let game = journal.gameplay();
    for realm in &game.kingdoms.kingdoms {
        println!(
            "kingdom={} name={} founder={} location={:?} state={:?}",
            realm.id, realm.name, realm.founder, realm.location, realm.state
        );
    }
    for claim in game
        .kingdoms
        .claims
        .iter()
        .filter(|c| c.kind == ClaimKind::ProtectedBuilding)
    {
        println!(
            "build-claim kingdom={} chunk={:?}",
            claim.kingdom, claim.chunk
        );
    }
    for (id, site) in &game.buildings.settlements {
        println!(
            "site={id} storage={:?} residential={:?} template={} craftsmen={:?}",
            site.storage, site.residential, site.selected_template, site.craftsmen
        );
        for npc in &site.craftsmen {
            if let Some(creature) = game.creatures.animals.get(npc) {
                println!(
                    "craftsman={npc} position={:?} velocity={:?} behavior={:?} timer={} age={} needs={:?}",
                    creature.position,
                    creature.velocity,
                    creature.behavior,
                    creature.timer,
                    creature.age,
                    creature.needs
                );
            }
        }
    }
    for job in game.buildings.jobs.values() {
        println!(
            "job={} kingdom={} origin={:?} template={} status={:?} step={}/{}",
            job.id,
            job.kingdom,
            job.origin,
            job.template,
            job.status,
            job.next_step,
            job.steps.len()
        );
    }
    let mut edits = BTreeMap::<(i64, u16), usize>::new();
    let mut examples = BTreeMap::<u16, Vec<[i64; 3]>>::new();
    for section in journal.changed_sections() {
        for (position, material) in journal.snapshot(section).map_err(|e| e.to_string())? {
            if [
                position[0].div_euclid(16),
                position[1].div_euclid(16),
                position[2].div_euclid(16),
            ] != [section.x, section.y, section.z]
            {
                continue;
            }
            if game
                .kingdoms
                .claim_at(terra_core::kingdom::claim_chunk(position))
                .is_some_and(|c| c.kind == ClaimKind::ProtectedBuilding)
            {
                *edits.entry((position[1], material)).or_default() += 1;
                let list = examples.entry(material).or_default();
                if list.len() < 8 {
                    list.push(position);
                }
            }
        }
    }
    println!("build-claim edits by (height, material): {edits:?}");
    println!("build-claim edited-cell examples: {examples:?}");
    if args.len() == 2 {
        return Ok(());
    }
    let number = |index: usize| -> Result<i64, String> {
        args[index]
            .to_str()
            .ok_or("Invalid coordinate")?
            .parse()
            .map_err(|_| "Coordinates must be integers".into())
    };
    let origin = [number(2)?, number(3)?, number(4)?];
    if origin.iter().any(|v| v.unsigned_abs() > 998_900) {
        return Err("Coordinates exceed the construction limit".into());
    }
    let facing = if args.len() >= 6 {
        u8::try_from(number(5)?).map_err(|_| "Invalid facing")?
    } else {
        0
    };
    if facing > 3 {
        return Err("Facing must be 0..3".into());
    }
    let template = args
        .get(6)
        .map_or(Ok(terra_core::building_jobs::HOUSE_TEMPLATE), |s| {
            s.to_str().ok_or("Invalid template ID")
        })?;
    let claim = game
        .kingdoms
        .claim_at([origin[0].div_euclid(16), origin[2].div_euclid(16)])
        .ok_or("Site is not claimed")?;
    let realm = game
        .kingdoms
        .get(claim.kingdom)
        .ok_or("Missing settlement")?;
    let terrain =
        terra_world::TerraGenerator::new(entry.manifest.world).map_err(|e| e.to_string())?;
    let generator = terra_world::SectionGenerator::new(terrain.clone());
    let mut chunks = terra_world::streaming::ChunkStore::default();
    let mut position = journal.changed_sections().next().unwrap_or_default();
    for x in (origin[0] - 7).div_euclid(16)..=(origin[0] + 7).div_euclid(16) {
        for z in (origin[2] - 7).div_euclid(16)..=(origin[2] + 7).div_euclid(16) {
            let column = generator
                .prepare_column([x, z], false)
                .map_err(|e| e.to_string())?;
            for y in (origin[1] - 3).div_euclid(16)..=(origin[1] + 9).div_euclid(16) {
                position.x = x;
                position.y = y;
                position.z = z;
                let generated = generator
                    .build_prepared(
                        position,
                        0,
                        &journal.snapshot(position).map_err(|e| e.to_string())?,
                        &column,
                    )
                    .map_err(|e| e.to_string())?;
                chunks.loaded.insert(
                    position,
                    terra_world::streaming::LoadedSection {
                        data: generated.data,
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: generated.props,
                    },
                );
            }
        }
    }
    let result = terra_world::building_jobs::plan_template_detailed(
        &chunks,
        &journal,
        &terrain,
        realm.id,
        realm.founder,
        origin,
        facing,
        template,
    );
    match result {
        Ok(job) => println!(
            "ACCEPTED origin={origin:?} facing={facing} template={template} steps={} excavation={}",
            job.steps.len(),
            job.steps
                .iter()
                .flat_map(|s| &s.cells)
                .filter(|c| c.material == 0)
                .count()
        ),
        Err(issue) => println!(
            "REJECTED origin={origin:?} facing={facing} template={template}: {issue:?} ({issue})"
        ),
    }
    let mut material_counts = BTreeMap::<(i64, u16, Option<u16>), usize>::new();
    let mut material_examples = BTreeMap::<(u16, Option<u16>), [i64; 3]>::new();
    for dx in -7..=7 {
        for dz in -7..=7 {
            for dy in -3..=9 {
                let p = [origin[0] + dx, origin[1] + dy, origin[2] + dz];
                if let Some(material) = chunks.block_at(p).filter(|m| *m != 0) {
                    let edit = journal.block(p);
                    *material_counts.entry((dy, material, edit)).or_default() += 1;
                    material_examples.entry((material, edit)).or_insert(p);
                }
            }
        }
    }
    println!("site terrain by (relative height, material, saved override): {material_counts:?}");
    println!("sample coordinates by (material, saved override): {material_examples:?}");
    Ok(())
}
