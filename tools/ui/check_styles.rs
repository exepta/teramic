//! Validate shipped styles with the same parser used by the runtime asset loader.
use std::path::{Path, PathBuf};
#[cfg(test)]
use tilt_ui::tilt_ui_css;

pub fn validate(root: &Path) -> Result<usize, String> {
    let mut directories = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(&directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?
        {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            if kind.is_dir() {
                directories.push(path);
            } else if kind.is_file() && path.extension().is_some_and(|extension| extension == "css")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut errors = Vec::new();
    for path in &files {
        let source = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if let Err(error) = tilt_ui_css::parse_stylesheet(&source) {
            errors.push(format!("{}: {error}", path.display()));
        }
    }
    if files.is_empty() {
        errors.push(format!("No stylesheets found in {}", root.display()));
    }
    if errors.is_empty() {
        Ok(files.len())
    } else {
        Err(errors.join("\n"))
    }
}

#[cfg(not(test))]
fn main() {
    let root = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("src-ui"), PathBuf::from);
    match validate(&root) {
        Ok(count) => {
            println!("Validated {count} shipped TiltUI stylesheets without a window or GPU.")
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

#[test]
fn every_shipped_stylesheet_uses_the_runtime_asset_parser() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-ui");
    validate(&root).expect("All shipped CSS must load with the actual TiltUI stylesheet parser");
}

#[test]
fn every_component_triplet_has_its_own_folder_and_generated_asset_paths() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-ui");
    let mut discovered = std::collections::BTreeSet::new();
    for category in ["components", "pages"] {
        for entry in std::fs::read_dir(root.join(category)).unwrap() {
            let entry = entry.unwrap();
            assert!(
                entry.file_type().unwrap().is_dir(),
                "{}",
                entry.path().display()
            );
            let name = entry.file_name().into_string().unwrap();
            for extension in ["rs", "html", "css"] {
                assert!(
                    entry
                        .path()
                        .join(format!("{name}.component.{extension}"))
                        .is_file(),
                    "Incomplete component {name}"
                );
            }
            discovered.insert(name);
        }
    }
    assert!(!root.join("app-shell.component.html").exists());
    for component in crate::tilt_ui_components() {
        let category = match component.kind {
            tilt_ui::ComponentKind::Page => "pages",
            tilt_ui::ComponentKind::Component => "components",
        };
        assert!(
            discovered.remove(component.name),
            "Unexpected generated component {}",
            component.name
        );
        assert_eq!(
            component.template_asset_path,
            format!(
                "tilt-ui://{category}/{0}/{0}.component.html",
                component.name
            )
        );
        assert_eq!(
            component.stylesheet_asset_path,
            format!("tilt-ui://{category}/{0}/{0}.component.css", component.name)
        );
    }
    assert!(
        discovered.is_empty(),
        "Component discovery missed {discovered:?}"
    );
}
