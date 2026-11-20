//! Transactional recursive recipe discovery shared by shipped assets and mod packs.
use super::*;
use std::path::PathBuf;

const MAX_DEPTH: usize = 16;

fn paths(root: &Path, depth: usize, result: &mut Vec<PathBuf>) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "Recipe folder exceeds {MAX_DEPTH} levels: {}",
            root.display()
        ));
    }
    let mut entries = std::fs::read_dir(root)
        .map_err(|error| format!("{}: {error}", root.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("{}: {error}", root.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if kind.is_symlink() {
            return Err(format!(
                "Recipe symbolic links are not supported: {}",
                path.display()
            ));
        }
        if kind.is_dir() {
            // These explicit roots use different, independently loaded schemas.
            // Unknown folders still participate in ordinary recipe validation.
            let process_schema = depth == 1
                && root.file_name().is_some_and(|name| name == "processes")
                && ["smelting", "crushing", "brewing"]
                    .iter()
                    .any(|name| entry.file_name() == *name);
            if !process_schema {
                paths(&path, depth + 1, result)?;
            }
        } else if kind.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext == "json" || ext == "toml")
        {
            result.push(path);
            if result.len() > MAX_RECIPES {
                return Err(format!("Too many recipe files (maximum {MAX_RECIPES})"));
            }
        }
    }
    Ok(())
}

fn parse(path: &Path) -> Result<RecipeFile, String> {
    if std::fs::metadata(path)
        .map_err(|error| error.to_string())?
        .len()
        > 512 * 1024
    {
        return Err("Recipe file exceeds 512 KiB".into());
    }
    let source = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let file: RecipeFile = if path.extension().is_some_and(|ext| ext == "json") {
        let value: serde_json::Value =
            serde_json::from_str(&source).map_err(|error| error.to_string())?;
        if value.get("id").is_some() {
            RecipeFile {
                schema_version: 1,
                recipes: vec![serde_json::from_value(value).map_err(|error| error.to_string())?],
            }
        } else {
            serde_json::from_value(value).map_err(|error| error.to_string())?
        }
    } else {
        toml::from_str(&source).map_err(|error| error.to_string())?
    };
    if file.schema_version != 1 {
        return Err("Unsupported recipe schema".into());
    }
    Ok(file)
}

pub(super) fn load_directory(book: &mut RecipeBook, root: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("{}: {error}", root.display())),
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!(
                "Recipe root must be a real folder: {}",
                root.display()
            ));
        }
        Ok(_) => {}
    }
    let mut files = Vec::new();
    paths(root, 0, &mut files)?;
    let mut recipes = Vec::new();
    let mut sources = BTreeMap::<u64, PathBuf>::new();
    for path in files {
        let file = parse(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        for recipe in file.recipes {
            recipe
                .validate()
                .map_err(|error| format!("{}: {error}", path.display()))?;
            if let Some(previous) = sources.insert(recipe.key(), path.clone()) {
                return Err(format!(
                    "Duplicate recipe ID or key collision {}: {} and {}",
                    recipe.id,
                    previous.display(),
                    path.display()
                ));
            }
            recipes.push(recipe);
            if recipes.len() > MAX_RECIPES {
                return Err(format!("Too many mod recipes (maximum {MAX_RECIPES})"));
            }
        }
    }
    book.extend(recipes)
}

#[cfg(test)]
mod tests;
