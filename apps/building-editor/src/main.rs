mod bootstrap;
mod document;
mod editor;
mod interface;
mod scene;
mod smoke;
mod workspace;

tilt_ui::include_components!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|argument| argument == "--help") {
        println!(
            "terra-building-editor [--load PATH.json] [--editor-acceptance]\nStandalone local building authoring. E: real item catalog; mouse: look; WASD: fly; Space/Shift: up/down; Esc: editor menu.\nThe protected support platform is excluded from templates. Resize it in Options (9/17/25/33)."
        );
        return Ok(());
    }
    let exit = bootstrap::app()?.run();
    if exit.is_error() {
        return Err("Building editor exited with an error".into());
    }
    Ok(())
}
