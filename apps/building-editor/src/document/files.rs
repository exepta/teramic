use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use terra_core::building_jobs::template::{BuildingTemplate, valid_template_id};

#[derive(Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub label: String,
    pub directory: bool,
}

pub fn list_directory(directory: &Path) -> Result<Vec<FileEntry>, String> {
    let raw = fs::read_dir(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?
        .take(257)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    if raw.len() > 256 {
        return Err("Directory contains more than 256 entries".into());
    }
    let mut entries = raw
        .into_iter()
        .map(Ok::<_, std::io::Error>)
        .map(|entry| {
            let entry = entry.map_err(|error| error.to_string())?;
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            if kind.is_symlink()
                || (!kind.is_dir()
                    && entry
                        .path()
                        .extension()
                        .is_none_or(|extension| extension != "json"))
            {
                return Ok(None);
            }
            Ok(Some(FileEntry {
                path: entry.path(),
                label: entry.file_name().to_string_lossy().into_owned(),
                directory: kind.is_dir(),
            }))
        })
        .collect::<Result<Vec<_>, String>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if entries.len() > 256 {
        return Err("Directory contains more than 256 entries".into());
    }
    entries.sort_by_key(|entry| (!entry.directory, entry.label.to_lowercase()));
    Ok(entries)
}

/// Guard the real game catalog against a second JSON with an existing ID.
/// The caller can overwrite that same file after confirmation, or choose a new ID.
pub(super) fn ensure_catalog_identity(root: &Path, target: &Path, id: &str) -> Result<(), String> {
    let Ok(root) = root.canonicalize() else {
        return Ok(());
    };
    let Some(parent) = target.parent() else {
        return Ok(());
    };
    let Ok(parent) = parent.canonicalize() else {
        return Ok(());
    };
    if !parent.starts_with(&root) {
        return Ok(());
    }
    if parent
        .strip_prefix(&root)
        .map_err(|error| error.to_string())?
        .components()
        .count()
        > 4
    {
        return Err("Building category exceeds four directory levels".into());
    }
    let canonical_target = target.canonicalize().ok();
    let mut pending = vec![root];
    let mut files = 0;
    let mut entries = 0;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|error| error.to_string())? {
            entries += 1;
            if entries > 256 {
                return Err("Building catalog exceeds 256 directory entries".into());
            }
            let entry = entry.map_err(|error| error.to_string())?;
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            if kind.is_symlink() {
                return Err("Building catalogs do not allow symlinks".into());
            }
            if kind.is_dir() {
                pending.push(entry.path());
                continue;
            }
            if !kind.is_file() || entry.path().extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            if canonical_target.as_ref().is_some_and(|target| {
                entry
                    .path()
                    .canonicalize()
                    .is_ok_and(|path| &path == target)
            }) {
                continue;
            }
            files += 1;
            if files >= terra_core::building_jobs::template::MAX_TEMPLATES {
                return Err("Building catalog has no free template slots".into());
            }
            let existing = BuildingTemplate::load(&entry.path())?;
            if existing.id == id {
                return Err(format!(
                    "Building ID '{id}' already belongs to {}. Choose a different filename / Anderen Dateinamen wählen.",
                    entry.path().display()
                ));
            }
        }
    }
    Ok(())
}

/// Create-only by default. Overwriting requires the caller's explicit confirmation.
pub fn save_template(
    path: &Path,
    template: &BuildingTemplate,
    overwrite: bool,
) -> Result<(), String> {
    template.validate()?;
    if path.extension().is_none_or(|extension| extension != "json")
        || !valid_template_id(path.file_stem().and_then(|s| s.to_str()).unwrap_or(""))
    {
        return Err(
            "Use a safe lowercase JSON filename / Kleinen Dateinamen ohne Sonderzeichen verwenden"
                .into(),
        );
    }
    ensure_catalog_identity(
        &terra_core::asset_directory().join("buildings"),
        path,
        &template.id,
    )?;
    let serialized = serde_json::to_vec_pretty(template).map_err(|error| error.to_string())?;
    let text = std::str::from_utf8(&serialized).map_err(|error| error.to_string())?;
    if BuildingTemplate::from_json(text)? != *template {
        return Err("Building JSON did not roundtrip exactly".into());
    }
    let parent = path.parent().ok_or("Missing save directory")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    if !overwrite {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        file.write_all(&serialized)
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_all())
            .map_err(|error| error.to_string())?;
        return Ok(());
    }
    let temporary = parent.join(format!(".{}.{}.tmp", template.id, std::process::id()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(&serialized)
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_all())
            .map_err(|error| error.to_string())?;
        fs::rename(&temporary, path).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
