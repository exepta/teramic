use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The portable distribution keeps its data next to the three executables.
/// Development binaries read the same layout from the workspace.
pub fn installation_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_owned))
            .filter(|directory| directory.join("TERAMIC-VERSION").is_file())
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .ancestors()
                    .nth(2)
                    .unwrap()
                    .into()
            })
    })
}

pub fn asset_directory() -> PathBuf {
    std::env::var_os("TERRA_ASSET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| installation_root().join("assets"))
}
