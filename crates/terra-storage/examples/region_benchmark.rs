//! Compare the old TOML shape with the current binary save using identical data.
use serde::Serialize;
use std::{fs, path::Path, time::Instant};
use terra_core::{GameplayState, PlayerUuid, WorldId};
use terra_world::edits::{EditJournal, SectionDelta};

#[derive(Serialize)]
struct Legacy {
    save_format_version: u32,
    world: WorldId,
    sections: Vec<SectionDelta>,
    gameplay: GameplayState,
}
fn size(path: &Path) -> std::io::Result<u64> {
    let mut total = 0;
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        total += if path.is_dir() {
            size(&path)?
        } else {
            fs::metadata(path)?.len()
        };
    }
    Ok(total)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root =
        std::env::temp_dir().join(format!("terra-region-benchmark-{}", PlayerUuid::new_v4()));
    let world = WorldId(7);
    let mut journal = EditJournal::default();
    for x in -64..64 {
        for z in -64..64 {
            journal.record([x * 16, 20, z * 16], if (x + z) % 19 == 0 { 27 } else { 3 })?;
        }
    }
    for n in 1..=16 {
        journal
            .gameplay_mut()
            .inventories
            .entry(PlayerUuid::from_u128(n))
            .or_default()
            .insert(terra_core::ItemKind::Block(3), 128);
    }
    let mut sections: Vec<_> = journal
        .changed_sections()
        .map(|p| journal.section_delta(p))
        .collect();
    sections.sort_unstable_by_key(|s| (s.position.x, s.position.y, s.position.z));
    let legacy = Legacy {
        save_format_version: 1,
        world,
        sections,
        gameplay: journal.gameplay().clone(),
    };
    let start = Instant::now();
    let text = toml::to_string_pretty(&legacy)?;
    let toml_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    terra_storage::save_edits(&root, world, &journal)?;
    let save_ms = start.elapsed().as_secs_f64() * 1000.0;
    let bytes = size(&root)?;
    let start = Instant::now();
    terra_storage::save_edits(&root, world, &journal)?;
    let repeat_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let loaded = terra_storage::load_edits(&root, world)?;
    let load_ms = start.elapsed().as_secs_f64() * 1000.0;
    if loaded.edited_blocks() != journal.edited_blocks() || loaded.gameplay() != journal.gameplay()
    {
        return Err("Save roundtrip changed state".into());
    }
    println!(
        "blocks={} regions={} players=16 legacy_toml_bytes={} binary_total_bytes={bytes} legacy_serialize_ms={toml_ms:.3} binary_fsynced_save_ms={save_ms:.3} unchanged_save_ms={repeat_ms:.3} binary_load_ms={load_ms:.3}",
        journal.edited_blocks(),
        fs::read_dir(root.join("regions"))?.count(),
        text.len()
    );
    fs::remove_dir_all(root)?;
    Ok(())
}
