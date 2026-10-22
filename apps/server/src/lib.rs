//! Dedicated server API shared with headless integration tests.
mod bootstrap;
pub use bootstrap::headless_app;
mod integrated;
mod persistence;
pub use integrated::IntegratedServer;
pub use persistence::DedicatedPersistence;
