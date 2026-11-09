//! Read-only train/save diagnostics; never repair or rewrite the supplied world.
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("Usage: inspect_train_world <world directory> <decimal world ID>".into());
    }
    let id = terra_core::WorldId(args[2].parse().map_err(|_| "Invalid world ID")?);
    let journal = terra_storage::load_edits(std::path::Path::new(&args[1]), id)?;
    println!(
        "edited_blocks={} sections={} furnaces={} trains={}",
        journal.edited_blocks(),
        journal.changed_sections().count(),
        journal.furnaces().count(),
        journal.gameplay().trains.vehicles.len()
    );
    for (cell, state) in journal.furnaces() {
        println!(
            "furnace cell={cell:?} material={:?} valid={} burn={}",
            journal.block(cell),
            state.valid(),
            state.burn_remaining
        );
    }
    for car in journal.gameplay().trains.vehicles.values() {
        println!(
            "train id={} kind={:?} anchor={:?} bounds={:?} parts={} speed={} boiler={} steam={:?}",
            car.id,
            car.kind,
            car.anchor,
            car.model_bounds(),
            car.parts.len(),
            car.motion.speed_mps,
            terra_core::train_power::boiler_length(&car.parts),
            car.steam
        );
    }
    let mut rails = std::collections::BTreeMap::new();
    for (cell, material, state) in journal.object_states() {
        if terra_core::rails::is_rail(material) {
            rails.entry(state.peer.unwrap_or(cell)).or_insert(material);
        }
    }
    println!("rail_modules={}", rails.len());
    for (anchor, material) in rails.iter().take(12) {
        println!("rail anchor={anchor:?} material={material}");
    }
    Ok(())
}
