//! Persistent work orders and small worker bags; world access stays on the authority.
mod model;
mod registry;
#[cfg(test)]
mod tests;

pub use model::*;
pub use registry::WorkRegistry;

pub const MAX_WORK_ORDERS: usize = 128;
pub const MAX_WORKERS: usize = 128;
pub const WORKER_SLOTS: usize = 9;
pub const MAX_PRIORITY: u8 = 9;

fn valid_cell(cell: [i64; 3]) -> bool {
    cell.iter()
        .all(|coordinate| coordinate.unsigned_abs() <= 999_000)
}
