use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn expanded_street_catalog_still_rejects_oversized_books_atomically() {
    let mut book = RecipeBook::builtins().clone();
    assert!(book.error.is_none(), "{:?}", book.error);
    assert!(book.iter().count() > 512);
    let before = book.snapshot();
    let recipe = before[0].clone();
    assert!(
        book.extend(vec![recipe; MAX_RECIPES + 1])
            .unwrap_err()
            .contains("Too many recipes")
    );
    assert_eq!(book.snapshot(), before);
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "terra-recipes-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn write(&self, file: &str, id: &str) {
        let path = self.0.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut recipe = RecipeBook::builtins().iter().next().unwrap().clone();
        recipe.id = format!("test::{id}");
        std::fs::write(path, serde_json::to_string(&recipe).unwrap()).unwrap();
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn nested_shipped_assets_equal_the_embedded_catalog_in_every_recipe_field() {
    let mut book = RecipeBook::default();
    book.load_directory(&crate::asset_directory().join("crafting"))
        .unwrap();
    let embedded = RecipeBook::builtins();
    for recipe in book.iter() {
        assert_eq!(embedded.get(recipe.key()), Some(recipe), "{}", recipe.id);
    }
    assert_eq!(book.iter().count(), embedded.iter().count());
    assert_eq!(book.iter().count(), 524);
}

#[test]
fn nested_and_flat_mod_files_load_independently_of_creation_order() {
    let directory = Directory::new();
    directory.write("z/deep/second.json", "second");
    directory.write("first.json", "first");
    std::fs::write(directory.0.join("README.md"), "Not a recipe").unwrap();
    let mut book = RecipeBook::default();
    book.load_directory(&directory.0).unwrap();
    assert_eq!(book.iter().count(), 2);
    assert!(book.get(recipe_key("test::first")).is_some());
    assert!(book.get(recipe_key("test::second")).is_some());
}

#[test]
fn only_the_three_declared_process_schema_roots_are_excluded_from_workbench_loading() {
    let directory = Directory::new();
    directory.write("building/first.json", "first");
    for schema in ["smelting", "crushing", "brewing"] {
        let folder = directory.0.join("processes").join(schema);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("definition.json"), "{different schema}").unwrap();
    }
    let mut book = RecipeBook::default();
    book.load_directory(&directory.0).unwrap();
    assert_eq!(book.iter().count(), 1);
    std::fs::create_dir_all(directory.0.join("processes/unknown")).unwrap();
    std::fs::write(
        directory.0.join("processes/unknown/bad.json"),
        "{different schema}",
    )
    .unwrap();
    let before = book.snapshot();
    assert!(
        book.load_directory(&directory.0)
            .unwrap_err()
            .contains("bad.json")
    );
    assert_eq!(book.snapshot(), before);
}

#[test]
fn every_shipped_process_recipe_matches_its_actual_runtime_catalog() {
    let root = crate::asset_directory().join("crafting/processes");
    for (kind, count) in [("smelting", 11), ("crushing", 20), ("brewing", 12)] {
        let files: Vec<_> = std::fs::read_dir(root.join(kind))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect();
        assert_eq!(files.len(), count);
        let mut ids = std::collections::BTreeSet::new();
        for path in files {
            let source = std::fs::read_to_string(&path).unwrap();
            let json: serde_json::Value = serde_json::from_str(&source).unwrap();
            assert!(ids.insert(json["id"].as_u64().unwrap()));
            match kind {
                "smelting" => {
                    let recipe: crate::furnace::SmeltingRecipe =
                        serde_json::from_str(&source).unwrap();
                    assert!(recipe.valid());
                    assert!(crate::furnace::recipes().contains(&recipe));
                }
                "crushing" => {
                    let recipe: crate::crusher::CrusherRecipe =
                        serde_json::from_str(&source).unwrap();
                    assert!(recipe.valid());
                    assert!(crate::crusher::recipes().contains(&recipe));
                }
                _ => {
                    let recipe: crate::brewing::BrewingRecipe =
                        serde_json::from_str(&source).unwrap();
                    assert!(recipe.valid());
                    assert!(crate::brewing::recipes().contains(&recipe));
                }
            }
        }
    }
}

#[test]
fn duplicate_ids_in_different_folders_report_both_files_without_any_partial_load() {
    let directory = Directory::new();
    directory.write("a/one.json", "duplicate");
    directory.write("z/two.json", "duplicate");
    let mut book = RecipeBook::builtins().clone();
    let before = book.snapshot();
    let error = book.load_directory(&directory.0).unwrap_err();
    assert!(
        error.contains("test::duplicate")
            && error.contains("one.json")
            && error.contains("two.json"),
        "{error}"
    );
    assert_eq!(book.snapshot(), before);
}

#[test]
fn invalid_nested_file_and_existing_id_conflicts_leave_the_book_unchanged() {
    let directory = Directory::new();
    directory.write("a/good.json", "good");
    std::fs::create_dir_all(directory.0.join("z")).unwrap();
    std::fs::write(directory.0.join("z/bad.json"), "{invalid}").unwrap();
    let mut book = RecipeBook::default();
    assert!(
        book.load_directory(&directory.0)
            .unwrap_err()
            .contains("bad.json")
    );
    assert!(book.iter().next().is_none());
    std::fs::remove_file(directory.0.join("z/bad.json")).unwrap();
    book.load_directory(&directory.0).unwrap();
    let before = book.snapshot();
    assert!(
        book.load_directory(&directory.0)
            .unwrap_err()
            .contains("test::good")
    );
    assert_eq!(book.snapshot(), before);
}

#[cfg(unix)]
#[test]
fn symbolic_link_cycles_are_rejected_instead_of_recursed_or_partially_loaded() {
    let directory = Directory::new();
    directory.write("a/good.json", "good");
    std::os::unix::fs::symlink(&directory.0, directory.0.join("cycle")).unwrap();
    let mut book = RecipeBook::default();
    assert!(
        book.load_directory(&directory.0)
            .unwrap_err()
            .contains("symbolic links")
    );
    assert!(book.iter().next().is_none());
}
