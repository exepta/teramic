//! Create a playable world from the same block catalog used by the client.
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt::Write,
    fs,
    path::Path,
};
use terra_core::{GameMode, PlayerUuid, WorldConfig};
use terra_render::blocks::definitions::{BlockDefinition, BlockLibrary};
use terra_storage::{WorldLibrary, load_edits, save_edits};
use terra_voxel::MaterialId as M;
use terra_world::{
    TerraGenerator,
    edits::{BlockObjectState, EditJournal, LiquidState},
    placement,
    spawn::find_land_spawn,
    streaming::ChunkStore,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const NAME: &str = "Blockgalerie";
const COLUMNS: usize = 24;
const SPACING: i64 = 5;
const SWATCHES: [u16; 5] = [
    M::Grass as u16,
    M::Sand as u16,
    M::Gravel as u16,
    M::Stone as u16,
    M::Leaves as u16,
];
const ORIGIN: [i64; 2] = [8, 24];

fn main() -> Result<()> {
    let library = WorldLibrary::default();
    let destination = library.root.join(NAME);
    let catalog = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks"))?;
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--verify"] {
        verify(&library, &catalog)?;
        println!(
            "{} Exponate, Gleismodule, Materialproben und Einstieg geprüft.",
            catalog.blocks.len()
        );
        return Ok(());
    }
    if !args.is_empty() {
        return Err("Aufruf: block_gallery [--verify]".into());
    }
    fs::create_dir_all(&library.root)?;
    if destination.try_exists()? {
        return Err(format!(
            "Der Zielordner existiert bereits: {}",
            destination.display()
        )
        .into());
    }
    // Only a complete, reloaded save becomes visible in the world picker.
    let stage = library
        .root
        .join(format!(".blockgalerie-{}", PlayerUuid::new_v4()));
    fs::create_dir(&stage)?;
    let result = build_gallery(&stage, &catalog).and_then(|()| {
        if destination.try_exists()? {
            return Err("Der Zielordner wurde zwischenzeitlich angelegt.".into());
        }
        fs::rename(stage.join(NAME), &destination)?;
        Ok(())
    });
    fs::remove_dir_all(stage)?;
    result?;
    println!(
        "{} Blöcke in {}. Im Einzelspielermenü „{NAME}“ öffnen.",
        catalog.blocks.len(),
        destination.display()
    );
    Ok(())
}

fn build_gallery(stage: &Path, catalog: &BlockLibrary) -> Result<()> {
    let library = WorldLibrary {
        root: stage.to_owned(),
    };
    let mut template = WorldConfig {
        base_height: 44.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        terrace_strength: 0.0,
        biomes: false,
        trees: false,
        caves: false,
        rivers: false,
        sea_level: -10.0,
        natural_mob_limit: 0,
        ..WorldConfig::default()
    };
    template.structures.set_all(false);
    template.time.start_hour = 11.5;
    template.game_rules.daylight_cycle = false;
    template.game_rules.weather_cycle = false;
    template.game_rules.mob_griefing = false;
    template.game_rules.fire_spread = false;
    let entry = library.create(NAME, Some(74_372_696), GameMode::Creative, &template)?;
    let id = entry.manifest.world.id;
    let world = library.path(id);
    let terrain = TerraGenerator::new(entry.manifest.world)?;
    let mut journal = EditJournal::default();
    let spawn = find_land_spawn(&terrain, &journal, ORIGIN)?;
    let [cx, floor, cz] = spawn.ground;
    let mut rail_blocks = Vec::new();
    let mut categories = BTreeMap::<&str, Vec<&BlockDefinition>>::new();
    for block in &catalog.blocks {
        if terra_core::rails::is_rail(block.id) {
            rail_blocks.push(block);
            continue;
        }
        categories
            .entry(block.creative_category.as_deref().unwrap_or("other"))
            .or_default()
            .push(block);
    }
    let rows: usize = categories
        .values()
        .map(|blocks| blocks.len().div_ceil(COLUMNS))
        .sum();
    let first_z = cz - 7;
    let half_width = COLUMNS as i64 / 2 * SPACING + 3;
    let rail_start = first_z - rows as i64 * SPACING;
    let last_z = rail_start - rail_blocks.len().div_ceil(4) as i64 * 32;
    // A neutral court with a wide central aisle. Keep the arrival pad natural so
    // normal server spawn selection also accepts the finished, edited world.
    for z in last_z..=cz + 20 {
        for x in cx - half_width..=cx + half_width {
            let stone = if (x - cx).abs() <= 1 && (z - cz).abs() <= 1 {
                3
            } else {
                12
            };
            journal.record([x, floor, z], stone)?;
        }
    }
    let mut index = String::from("Kategorie\tReihe\tPlatz\tID\tName\tX\tY\tZ\n");
    let mut start_row = 0;
    let chunks = ChunkStore::default();
    for (category, blocks) in categories {
        let band = match category {
            "building" => 16,
            "decoration" => 21,
            "nature" => 13,
            "technic" => 19,
            "terrain" => 11,
            "trains" => 43,
            "wood" => 18,
            _ => 10,
        };
        let section_z = first_z - start_row as i64 * SPACING + 2;
        for x in cx - half_width..=cx + half_width {
            journal.record([x, floor, section_z], band)?;
        }
        for row in start_row..start_row + blocks.len().div_ceil(COLUMNS) {
            inlay_row_number(
                &mut journal,
                [cx, floor, first_z - row as i64 * SPACING],
                row + 1,
            )?;
        }
        for (i, block) in blocks.iter().enumerate() {
            let col = i % COLUMNS;
            let row = start_row + i / COLUMNS;
            let k = col as i64 - COLUMNS as i64 / 2;
            let x = cx + k * SPACING + if k < 0 { 0 } else { SPACING };
            let z = first_z - row as i64 * SPACING;
            let cell = [x, floor + 2, z];
            for dx in -1..=1 {
                for dz in -1..=1 {
                    journal.record([x + dx, floor + 1, z + dz], band)?;
                }
            }
            place_exhibit(&mut journal, &chunks, cell, block)?;
            writeln!(
                index,
                "{category}\t{}\t{}\t{}\t{}\t{x}\t{}\t{z}",
                row + 1,
                col + 1,
                block.id,
                block.name,
                cell[1]
            )?;
        }
        start_row += blocks.len().div_ceil(COLUMNS);
    }

    // Full native rail modules include steep grades and a 25 m turntable.
    // Give them their own yard and persist every owned slice and real support.
    for (i, block) in rail_blocks.into_iter().enumerate() {
        let col = i % 4;
        let row = i / 4;
        let x = cx - 48 + col as i64 * 32;
        let z = rail_start - 16 - row as i64 * 32;
        let anchor = [x, floor + 1, z];
        let piece = terra_core::rails::Piece::from_facing(anchor, block.id, 0)
            .ok_or("Ungültiges Gleismodul")?;
        for support in piece.supports().iter() {
            let [dx, dy, dz] = support.support.map(i64::from);
            for y in floor..=anchor[1] + dy {
                journal.record([x + dx, y, z + dz], 43)?;
            }
        }
        for cell in terra_world::rails::cells(piece) {
            journal.record(cell, block.id)?;
            journal.set_object_state(cell, Some(terra_world::rails::object_state(piece, cell)))?;
        }
        writeln!(
            index,
            "trains/Gleishof\t{}\t{}\t{}\t{}\t{x}\t{}\t{z}",
            rows + row + 1,
            col + 1,
            block.id,
            block.name,
            anchor[1]
        )?;
    }

    for (cell, material) in swatches(spawn.ground) {
        journal.record(cell, material)?;
    }

    fs::write(world.join("Blockverzeichnis.tsv"), index)?;
    save_edits(&world, id, &journal)?;
    drop(journal);
    verify(&library, catalog)
}

fn verify(library: &WorldLibrary, catalog: &BlockLibrary) -> Result<()> {
    let entry = library
        .list()?
        .0
        .into_iter()
        .find(|e| e.title == NAME && library.path(e.manifest.world.id) == library.root.join(NAME))
        .ok_or("Die Blockgalerie fehlt im Einzelspielermenü.")?;
    let id = entry.manifest.world.id;
    library.load(id)?.compatible()?;
    let world = library.path(id);
    let saved = load_edits(&world, id)?;
    let mut seen = BTreeSet::new();
    for line in fs::read_to_string(world.join("Blockverzeichnis.tsv"))?
        .lines()
        .skip(1)
    {
        let fields: Vec<_> = line.split('\t').collect();
        let [_, _, _, raw, _, x, y, z] = fields.as_slice() else {
            return Err(format!("Ungültiger Indexeintrag: {line}").into());
        };
        let cell = [x.parse()?, y.parse()?, z.parse()?];
        let material: u16 = raw.parse()?;
        if saved.block(cell) != Some(material) || !seen.insert(material) {
            return Err(format!(
                "Block {material} fehlt an {cell:?} oder ist doppelt verzeichnet."
            )
            .into());
        }
        if terra_core::rails::is_rail(material)
            && !terra_world::rails::owner_at(&saved, cell)
                .is_some_and(|piece| terra_world::rails::complete(&saved, piece))
        {
            return Err(format!("Unvollständiges Gleismodul an {cell:?}.").into());
        }
        if material == M::FluidWater as u16
            && !saved.fluid(cell).is_some_and(|water| {
                water.amount == terra_world::water::FLUID_CAPACITY && water.source
            })
        {
            return Err(format!("Wasserquelle fehlt an {cell:?}.").into());
        }
        if material == terra_core::bile::FLUID
            && !saved
                .object_state(cell)
                .and_then(|state| state.liquid)
                .is_some_and(|liquid| liquid.source && liquid.volume == terra_core::bile::CAPACITY)
        {
            return Err(format!("Gallequelle fehlt an {cell:?}.").into());
        }
    }
    if seen != catalog.blocks.iter().map(|block| block.id).collect() {
        return Err("Blockverzeichnis und Blockkatalog unterscheiden sich.".into());
    }
    let terrain = TerraGenerator::new(entry.manifest.world)?;
    let spawn = find_land_spawn(&terrain, &EditJournal::default(), ORIGIN)?;
    if find_land_spawn(&terrain, &saved, ORIGIN)? != spawn {
        return Err("Der Einstieg ist nach dem Laden versperrt.".into());
    }
    for (cell, material) in swatches(spawn.ground) {
        if saved.block(cell) != Some(material) {
            return Err(format!("Materialprobe {material} ist an {cell:?} unvollständig.").into());
        }
    }
    Ok(())
}

// Exposed 7 × 7 surfaces meet across both section axes.
fn swatches([cx, floor, cz]: [i64; 3]) -> impl Iterator<Item = ([i64; 3], u16)> {
    let edge = terra_voxel::blocks::SECTION_SIZE as i64;
    let z = cz.div_euclid(edge) * edge + edge;
    SWATCHES
        .into_iter()
        .enumerate()
        .flat_map(move |(i, material)| {
            let x = (cx.div_euclid(edge) + i as i64 - 2) * edge;
            (-3..=3)
                .flat_map(move |dz| (-3..=3).map(move |dx| ([x + dx, floor + 1, z + dz], material)))
        })
}

fn place_exhibit(
    journal: &mut EditJournal,
    chunks: &ChunkStore,
    cell: [i64; 3],
    block: &BlockDefinition,
) -> Result<()> {
    let [x, y, z] = cell;
    let id = block.id;
    if matches!(id, 28 | 59) || terra_core::beds::is_bed(id) || terra_world::objects::is_door(id) {
        let layout = placement::layout(chunks, journal, cell, 0, id, [0.; 2], None)?;
        for edit in layout.edits {
            journal.record(edit.position, edit.material)?;
        }
        for (position, state) in layout.states {
            journal.set_object_state(position, state)?;
        }
    } else {
        journal.record(cell, id)?;
        let default = BlockObjectState::default();
        if id == terra_core::crusher::HAND_CRANK_MATERIAL {
            let shaft = [x, y - 1, z];
            let vertical = BlockObjectState {
                facing: 4,
                ..default
            };
            journal.record(shaft, terra_core::kinetic::WOODEN_SHAFT)?;
            journal.set_object_state(shaft, Some(vertical))?;
            journal.set_object_state(
                cell,
                Some(BlockObjectState {
                    peer: Some(shaft),
                    ..vertical
                }),
            )?;
        } else if id == terra_core::bile::FLUID {
            let liquid = LiquidState {
                origin: cell,
                volume: terra_core::bile::CAPACITY,
                age: terra_core::bile::FINE,
                source: true,
            };
            journal.set_object_state(
                cell,
                Some(BlockObjectState {
                    facing: terra_core::bile::facing(liquid.volume, liquid.age),
                    liquid: Some(liquid),
                    ..default
                }),
            )?;
        } else if placement::furniture(id) && default.valid_for(cell, id) {
            journal.set_object_state(cell, Some(default))?;
        }
    }
    if matches!(id, 54 | 55)
        || terra_world::segments::fixed_mineral_root(id) == Some(1)
        || matches!(
            M::from_raw(id),
            Some(
                M::HangingBlossomLantern
                    | M::HangingIvy
                    | M::HangingBloodVines
                    | M::HangingBloodLantern
                    | M::HangingMoss
            )
        )
    {
        journal.record([x, y + 1, z], 3)?;
        journal.record([x, y + 1, z - 1], 3)?;
        journal.record([x, y, z - 1], 3)?;
    }
    if id == M::FluidWater as u16
        || id == terra_world::lava::LAVA
        || id == terra_core::bile::FLUID
        || id == terra_core::roads::WET_TAR
    {
        for [dx, dz] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
            journal.record([x + dx, y, z + dz], 15)?;
        }
    }
    Ok(())
}

// Three-by-five stone numerals identify each row from the entrance.
fn inlay_row_number(journal: &mut EditJournal, [x, y, z]: [i64; 3], row: usize) -> Result<()> {
    const DIGITS: [[u8; 5]; 10] = [
        [7, 5, 5, 5, 7],
        [2, 6, 2, 2, 7],
        [7, 1, 7, 4, 7],
        [7, 1, 7, 1, 7],
        [5, 5, 7, 1, 1],
        [7, 4, 7, 1, 7],
        [7, 4, 7, 5, 7],
        [7, 1, 1, 1, 1],
        [7, 5, 7, 5, 7],
        [7, 5, 7, 1, 7],
    ];
    let digits = row.to_string();
    let start_x = x - (digits.len() as i64 * 4 - 1) / 2;
    for (i, digit) in digits.bytes().enumerate() {
        for (dz, bits) in DIGITS[usize::from(digit - b'0')].iter().enumerate() {
            for dx in 0..3 {
                if bits & (4 >> dx) != 0 {
                    journal.record([start_x + i as i64 * 4 + dx, y, z + dz as i64 - 2], 18)?;
                }
            }
        }
    }
    Ok(())
}
