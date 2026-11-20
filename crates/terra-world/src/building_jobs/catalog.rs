//! Atomic catalog snapshots; polling reads memory, explicit manager reload reads files.
use super::*;
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock, RwLock},
};
use terra_core::building_jobs::template::{DEFAULT_TEMPLATE_DIRECTORY, TemplateCatalog};

struct CatalogCache {
    path: PathBuf,
    state: RwLock<Result<Arc<TemplateCatalog>, String>>,
}
impl CatalogCache {
    fn load(path: &std::path::Path) -> Result<Arc<TemplateCatalog>, String> {
        TemplateCatalog::load_directory(path)
            .map(Arc::new)
            .inspect_err(|error| tracing::error!(%error,"Cannot load building catalog"))
    }
    fn new(path: PathBuf) -> Self {
        let state = RwLock::new(Self::load(&path));
        Self { path, state }
    }
    fn snapshot(&self) -> Result<Arc<TemplateCatalog>, BuildError> {
        self.state
            .read()
            .map_err(|_| BuildError::InvalidTemplate)?
            .as_ref()
            .map(Arc::clone)
            .map_err(|_| BuildError::InvalidTemplate)
    }
    fn error(&self) -> Option<String> {
        match self.state.read() {
            Ok(state) => state.as_ref().err().cloned(),
            Err(_) => Some("Building catalog lock unavailable".into()),
        }
    }
    fn reload(&self) -> Result<Arc<TemplateCatalog>, String> {
        // Parse the complete bounded directory before replacing the snapshot.
        // Errors replace the lookup state too; old held Arcs remain immutable,
        // but future plans cannot pretend stale files are a successful reload.
        let next = Self::load(&self.path);
        *self
            .state
            .write()
            .map_err(|_| "Building catalog lock unavailable")? = next.clone();
        next
    }
}
fn cache() -> &'static CatalogCache {
    static CACHE: OnceLock<CatalogCache> = OnceLock::new();
    CACHE.get_or_init(|| {
        CatalogCache::new(terra_core::asset_directory().join(DEFAULT_TEMPLATE_DIRECTORY))
    })
}
pub fn template_catalog() -> Result<Arc<TemplateCatalog>, BuildError> {
    cache().snapshot()
}
pub fn reload_template_catalog() -> Result<Arc<TemplateCatalog>, String> {
    cache().reload()
}
pub fn template_catalog_error() -> Option<String> {
    cache().error()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reload_adds_new_categories_atomically_reports_partial_files_and_keeps_held_snapshots_immutable()
     {
        let root = std::env::temp_dir().join(format!(
            "terra-catalog-reload-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let original = terra_core::building_jobs::template::BuildingTemplate::load(
            &terra_core::asset_directory()
                .join(terra_core::building_jobs::template::DEFAULT_HOUSE_PATH),
        )
        .unwrap();
        std::fs::write(
            root.join("old.json"),
            serde_json::to_vec(&original).unwrap(),
        )
        .unwrap();
        let cache = CatalogCache::new(root.clone());
        let first = cache.snapshot().unwrap();
        let sub = root.join("Kingdom/Wohnhäuser");
        std::fs::create_dir_all(&sub).unwrap();
        let mut added = original;
        added.id = "new-residential-v1".into();
        let file = sub.join("new.json");
        std::fs::write(&file, serde_json::to_vec(&added).unwrap()).unwrap();
        let second = cache.reload().unwrap();
        assert!(first.get(&added.id).is_none());
        assert!(second.get(&added.id).is_some());
        assert_eq!(
            second
                .summaries()
                .iter()
                .find(|s| s.id == added.id)
                .unwrap()
                .category_path,
            "Kingdom/Wohnhäuser"
        );
        std::fs::write(&file, "{incomplete").unwrap();
        assert!(cache.reload().is_err());
        assert!(cache.snapshot().is_err());
        assert!(cache.error().unwrap().contains("new.json"));
        assert!(
            second.get(&added.id).is_some(),
            "an already held plan snapshot is immutable"
        );
        std::fs::remove_file(file).unwrap();
        let repaired = cache.reload().unwrap();
        assert!(cache.error().is_none());
        assert!(repaired.get(&added.id).is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}
