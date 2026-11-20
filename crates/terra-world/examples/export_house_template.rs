//! Export the actual ordinary village house as the editable construction default.
use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args_os().nth(1).map_or_else(
        || {
            terra_core::asset_directory()
                .join(terra_core::building_jobs::template::DEFAULT_HOUSE_PATH)
        },
        PathBuf::from,
    );
    let template = terra_world::building_jobs::export_default_house_template()?;
    let text = serde_json::to_string_pretty(&template)? + "\n";
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, text)?;
    println!(
        "Exported {}: {} steps, {} cells, {} material kinds",
        path.display(),
        template.steps.len(),
        template
            .steps
            .iter()
            .map(|step| step.cells.len())
            .sum::<usize>(),
        template.materials.len()
    );
    Ok(())
}
