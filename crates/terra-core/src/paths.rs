use std::path::PathBuf;

/// Override when deploying binaries outside the source checkout.
pub fn asset_directory() -> PathBuf {
    std::env::var_os("TERRA_ASSET_DIR").map_or_else(
        || PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets")),
        PathBuf::from,
    )
}
