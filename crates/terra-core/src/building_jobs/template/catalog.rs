//! Filesystem loading belongs to the server; wire summaries never expose paths.
use super::*;
use std::path::PathBuf;

pub const MAX_TEMPLATES: usize = 32;
pub const MAX_TEMPLATE_MATERIALS: usize = 64;
const MAX_CATALOG_BYTES: u64 = 8 * 1024 * 1024;
const MAX_DIRECTORY_ENTRIES: usize = 256;
const MAX_DIRECTORY_DEPTH: usize = 4;

pub fn valid_template_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.as_bytes()[0].is_ascii_lowercase()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
}
pub(super) fn default_display_name() -> String {
    "Temperate village house".into()
}
pub(super) fn valid_display_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 80
        && name.trim() == name
        && !name.chars().any(char::is_control)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateCategory {
    #[default]
    Residential,
}

/// Authored permanent inputs only. A resolved job adds its paid site scaffolds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateSummary {
    pub id: String,
    pub display_name: String,
    pub category: TemplateCategory,
    /// Validated relative folder labels, never an absolute filesystem path.
    #[serde(default)]
    pub category_path: String,
    pub dimensions: [u16; 3],
    pub steps: u16,
    pub materials: Vec<ItemStack>,
}
impl TemplateSummary {
    pub fn valid(&self) -> bool {
        let mut items = BTreeSet::new();
        valid_template_id(&self.id)
            && valid_display_name(&self.display_name)
            && valid_category_path(&self.category_path)
            && self
                .dimensions
                .into_iter()
                .zip([9, 12, 11])
                .all(|(size, max)| (1..=max).contains(&size))
            && self.steps > 0
            && usize::from(self.steps) <= MAX_BUILD_STEPS
            && !self.materials.is_empty()
            && self.materials.len() <= MAX_TEMPLATE_MATERIALS
            && self
                .materials
                .iter()
                .all(|s| s.item.valid() && s.count > 0 && items.insert(s.item))
            && self
                .materials
                .iter()
                .map(|s| u32::from(s.count))
                .sum::<u32>()
                <= (2 * MAX_BUILD_STEPS) as u32
    }
}
impl BuildingTemplate {
    pub fn summary(&self) -> Result<TemplateSummary, String> {
        self.validate()?;
        let summary = self.summary_unchecked();
        summary
            .valid()
            .then_some(summary)
            .ok_or_else(|| "Invalid building summary".into())
    }
    fn summary_unchecked(&self) -> TemplateSummary {
        TemplateSummary {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            category: self.category,
            category_path: String::new(),
            dimensions: std::array::from_fn(|i| {
                (self.bounds.max[i] - self.bounds.min[i] + 1) as u16
            }),
            steps: self.steps.len() as u16,
            materials: self.materials.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TemplateCatalog {
    templates: BTreeMap<String, BuildingTemplate>,
    category_paths: BTreeMap<String, String>,
}
impl TemplateCatalog {
    /// Deterministic, bounded traversal. Invalid files and duplicate IDs fail the
    /// entire catalog, so a typo cannot silently remove a player's selection.
    pub fn load_directory(directory: &Path) -> Result<Self, String> {
        let root = directory
            .canonicalize()
            .map_err(|e| format!("{}: {e}", directory.display()))?;
        if !root.is_dir() {
            return Err(format!(
                "{}: building catalog is not a directory",
                root.display()
            ));
        }
        let mut pending = vec![(root.clone(), 0)];
        let mut paths = Vec::<PathBuf>::new();
        let mut entries = 0;
        let mut bytes = 0_u64;
        while let Some((folder, depth)) = pending.pop() {
            let remaining = MAX_DIRECTORY_ENTRIES - entries;
            let mut children = std::fs::read_dir(&folder)
                .map_err(|e| format!("{}: {e}", folder.display()))?
                .take(remaining + 1)
                .map(|entry| entry.map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            if children.len() > remaining {
                return Err("Building catalog exceeds 256 directory entries".into());
            }
            entries += children.len();
            children.sort_by_key(std::fs::DirEntry::file_name);
            for entry in children {
                let path = entry.path();
                let kind = entry
                    .file_type()
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                if kind.is_symlink() {
                    return Err(format!(
                        "{}: symlinks are not allowed in a building catalog",
                        path.display()
                    ));
                }
                if kind.is_dir() {
                    if depth >= MAX_DIRECTORY_DEPTH {
                        return Err("Building catalog directory nesting exceeds four levels".into());
                    }
                    pending.push((path, depth + 1));
                } else if kind.is_file() && path.extension().is_some_and(|e| e == "json") {
                    bytes = bytes
                        .checked_add(entry.metadata().map_err(|e| e.to_string())?.len())
                        .ok_or("Building catalog size overflows")?;
                    paths.push(path);
                    if paths.len() > MAX_TEMPLATES || bytes > MAX_CATALOG_BYTES {
                        return Err("Building catalog exceeds 32 templates or 8 MiB".into());
                    }
                }
            }
        }
        paths.sort();
        let mut templates = BTreeMap::new();
        let mut category_paths = BTreeMap::new();
        for path in paths {
            let template = BuildingTemplate::load(&path)?;
            template.summary()?;
            let id = template.id.clone();
            let relative = path
                .parent()
                .and_then(|p| p.strip_prefix(&root).ok())
                .ok_or("Invalid category directory")?;
            let mut parts = relative
                .components()
                .map(|c| {
                    c.as_os_str()
                        .to_str()
                        .map(str::to_owned)
                        .ok_or("Category directory is not UTF-8")
                })
                .collect::<Result<Vec<_>, _>>()?;
            if parts.first().is_some_and(|p| p == "templates") {
                parts.remove(0);
            }
            let category_path = parts.join("/");
            if !valid_category_path(&category_path) {
                return Err(format!("{}: invalid category path", path.display()));
            }
            category_paths.insert(id.clone(), category_path);
            if templates.insert(id.clone(), template).is_some() {
                return Err(format!(
                    "{}: duplicate building template ID {id}",
                    path.display()
                ));
            }
        }
        if templates.is_empty() {
            return Err("Building catalog contains no JSON templates".into());
        }
        Ok(Self {
            templates,
            category_paths,
        })
    }
    pub fn get(&self, id: &str) -> Option<&BuildingTemplate> {
        valid_template_id(id)
            .then(|| self.templates.get(id))
            .flatten()
    }
    pub fn summaries(&self) -> Vec<TemplateSummary> {
        self.templates
            .values()
            .map(|template| {
                let mut summary = template.summary_unchecked();
                summary.category_path = self
                    .category_paths
                    .get(&template.id)
                    .cloned()
                    .unwrap_or_default();
                summary
            })
            .collect()
    }
}

/// Display-only relative labels. Slash is a separator, never a filesystem request.
pub fn valid_category_path(path: &str) -> bool {
    path.is_empty()
        || path.len() <= 128
            && path.split('/').count() <= 4
            && path.split('/').all(|part| {
                !part.is_empty()
                    && part.len() <= 64
                    && part != "."
                    && part != ".."
                    && part.trim() == part
                    && !part
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '\\' | ':'))
            })
}
