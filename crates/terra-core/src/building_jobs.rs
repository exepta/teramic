//! Persistent local housing projects. World access and material movement stay authoritative.
mod model;
mod registry;
mod resources;
pub mod template;
pub use model::*;
pub use registry::BuildingRegistry;
pub use resources::*;

pub const MAX_BUILD_JOBS: usize = 32;
pub const MAX_SETTLEMENTS: usize = 128;
pub const MAX_CRAFTSMEN: usize = 32;
pub const MAX_HOUSE_WORKERS: usize = 4;
pub const MAX_BUILD_STEPS: usize = 8192;
pub const HOUSE_TEMPLATE: &str = "village-house-temperate-v1";

pub fn default_template_id() -> String {
    HOUSE_TEMPLATE.to_owned()
}

/// Effective wall-clock work rate while this many craftsmen are actually onsite.
pub fn house_seconds(workers: usize) -> Option<f64> {
    match workers {
        1 => Some(1800.0),
        2 => Some(900.0),
        3 => Some(600.0),
        4 => Some(300.0),
        _ => None,
    }
}

pub(crate) fn valid_cell(cell: [i64; 3]) -> bool {
    cell.into_iter().all(|v| v.unsigned_abs() <= 999_000)
}

#[cfg(test)]
mod tests;
