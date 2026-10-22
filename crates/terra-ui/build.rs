fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-ui");
    println!("cargo:rerun-if-changed={}", root.display());
    tilt_ui_build::build_from(&tilt_ui_build::UiSourceRoot::new(root))?;
    Ok(())
}
