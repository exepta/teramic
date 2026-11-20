use super::*;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "terra-building-catalog-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, template: &BuildingTemplate) {
        let file = self.0.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, serde_json::to_string(template).unwrap()).unwrap();
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn simple(id: &str, count: usize) -> BuildingTemplate {
    let steps: Vec<_> = (0..count)
        .map(|n| TemplateStep {
            phase: BuildPhase::Roof,
            cells: vec![TemplateCell {
                position: [
                    (n % 9) as i64 - 4,
                    4 + (n / 81) as i64,
                    ((n / 9) % 9) as i64 - 4,
                ],
                material: 16,
                facing: 0,
                rotate_facing: false,
                peer: None,
                secondary: false,
            }],
            costs: vec![ItemStack {
                item: ItemKind::Block(16),
                count: 1,
            }],
        })
        .collect();
    BuildingTemplate {
        schema_version: TEMPLATE_SCHEMA,
        id: id.into(),
        display_name: "Test timber house".into(),
        category: TemplateCategory::Residential,
        anchor: [0; 3],
        bounds: BuildArea::new([-4, 0, -4], [4, 7, 4]).unwrap(),
        materials: material_totals(&steps).unwrap(),
        steps,
    }
}

#[test]
fn catalogs_are_recursive_deterministic_by_id_and_never_accept_client_paths() {
    let directory = Directory::new();
    directory.write("z/first.json", &simple("alpha-house", 2));
    directory.write("a/second.json", &simple("zeta-house", 3));
    let loaded = TemplateCatalog::load_directory(&directory.0).unwrap();
    let summaries = loaded.summaries();
    assert_eq!(
        summaries.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["alpha-house", "zeta-house"]
    );
    assert!(summaries.iter().all(TemplateSummary::valid));
    assert_eq!(loaded.get("alpha-house").unwrap().steps.len(), 2);
    for id in [
        "",
        "../alpha-house",
        "village/residential.json",
        "/tmp/house",
        "Uppercase",
        ".",
        "a\n",
    ] {
        assert!(!valid_template_id(id));
        assert!(loaded.get(id).is_none());
    }
    assert!(!valid_template_id(&"a".repeat(65)));
    assert!(loaded.get("missing-house").is_none());
    assert_eq!(
        loaded.summaries(),
        TemplateCatalog::load_directory(&directory.0)
            .unwrap()
            .summaries()
    );
}

#[test]
fn invalid_missing_empty_or_duplicate_catalogs_fail_with_the_actual_source() {
    let directory = Directory::new();
    assert!(
        TemplateCatalog::load_directory(&directory.0)
            .unwrap_err()
            .contains("no JSON")
    );
    assert!(TemplateCatalog::load_directory(&directory.0.join("missing")).is_err());
    directory.write("house.json", &simple("same-house", 1));
    directory.write("duplicate.json", &simple("same-house", 2));
    assert!(
        TemplateCatalog::load_directory(&directory.0)
            .unwrap_err()
            .contains("duplicate")
    );
    std::fs::remove_file(directory.0.join("duplicate.json")).unwrap();
    std::fs::write(directory.0.join("broken.json"), "{broken}").unwrap();
    assert!(
        TemplateCatalog::load_directory(&directory.0)
            .unwrap_err()
            .contains("broken.json")
    );
}

#[test]
fn template_counts_entry_counts_file_bytes_and_nesting_are_bounded() {
    let directory = Directory::new();
    for n in 0..=MAX_TEMPLATES {
        directory.write(&format!("{n}.json"), &simple(&format!("house-{n}"), 1));
    }
    assert!(
        TemplateCatalog::load_directory(&directory.0)
            .unwrap_err()
            .contains("32 templates")
    );
    let directory = Directory::new();
    std::fs::write(
        directory.0.join("large.json"),
        " ".repeat(MAX_TEMPLATE_BYTES + 1),
    )
    .unwrap();
    assert!(
        TemplateCatalog::load_directory(&directory.0)
            .unwrap_err()
            .contains("2 MiB")
    );
    let directory = Directory::new();
    directory.write("a/b/c/d/e/house.json", &simple("house", 1));
    assert!(
        TemplateCatalog::load_directory(&directory.0)
            .unwrap_err()
            .contains("nesting")
    );
    let directory = Directory::new();
    for n in 0..257 {
        std::fs::write(directory.0.join(format!("{n}.txt")), "").unwrap();
    }
    assert!(
        TemplateCatalog::load_directory(&directory.0)
            .unwrap_err()
            .contains("256 directory")
    );
}

#[cfg(unix)]
#[test]
fn catalog_does_not_follow_symlinked_files_or_subdirectories() {
    for directory_link in [false, true] {
        let directory = Directory::new();
        let external = Directory::new();
        external.write("house.json", &simple("external-house", 1));
        let source = if directory_link {
            external.0.clone()
        } else {
            external.0.join("house.json")
        };
        std::os::unix::fs::symlink(source, directory.0.join("linked.json")).unwrap();
        assert!(
            TemplateCatalog::load_directory(&directory.0)
                .unwrap_err()
                .contains("symlink")
        );
    }
}

#[test]
fn aggregate_catalog_materials_can_exceed_inventory_stack_size_and_old_json_keeps_metadata_defaults()
 {
    let template = simple("large-house", 200);
    let summary = template.summary().unwrap();
    assert_eq!(
        summary.materials,
        vec![ItemStack {
            item: ItemKind::Block(16),
            count: 200
        }]
    );
    assert!(summary.valid());
    let mut value = serde_json::to_value(simple(HOUSE_TEMPLATE, 1)).unwrap();
    value.as_object_mut().unwrap().remove("display_name");
    value.as_object_mut().unwrap().remove("category");
    let old = BuildingTemplate::from_json(&value.to_string()).unwrap();
    assert_eq!(old.display_name, "Temperate village house");
    assert_eq!(old.category, TemplateCategory::Residential);
    let mut invalid = summary;
    invalid.materials.push(invalid.materials[0]);
    assert!(!invalid.valid());
}

#[test]
fn shipped_catalog_contains_distinct_real_residential_templates_and_exact_ledgers() {
    let catalog =
        TemplateCatalog::load_directory(&crate::asset_directory().join(DEFAULT_TEMPLATE_DIRECTORY))
            .unwrap();
    let default = catalog.get(HOUSE_TEMPLATE).unwrap();
    let cottage = catalog.get("spruce-cottage-v1").unwrap();
    assert_ne!(default.steps, cottage.steps);
    assert_ne!(default.materials, cottage.materials);
    assert!(default.steps.iter().any(|s| s.cells[0].position[2] == -6));
    assert!(
        !cottage
            .steps
            .iter()
            .any(|s| s.cells[0].material == 90 || s.cells[0].material == 92)
    );
    assert!(
        cottage
            .steps
            .iter()
            .any(|s| s.phase == BuildPhase::Furnishing && s.cells[0].position == [1, 0, -5])
    );
    assert_eq!(cottage.materials, material_totals(&cottage.steps).unwrap());
    assert!(
        cottage
            .materials
            .iter()
            .any(|s| s.item == ItemKind::Block(18))
    );
    assert!(
        !cottage
            .materials
            .iter()
            .any(|s| s.item == ItemKind::Block(103))
    );
    for summary in catalog.summaries() {
        assert!(summary.valid());
    }
}

#[test]
fn category_paths_follow_new_asset_folders_and_hide_only_the_legacy_templates_prefix() {
    let directory = Directory::new();
    directory.write("templates/village/house.json", &simple("legacy-house", 1));
    directory.write("Kingdom/Wohnhäuser/house.json", &simple("kingdom-house", 1));
    directory.write("root.json", &simple("root-house", 1));
    let catalog = TemplateCatalog::load_directory(&directory.0).unwrap();
    let paths: BTreeMap<_, _> = catalog
        .summaries()
        .into_iter()
        .map(|s| (s.id, s.category_path))
        .collect();
    assert_eq!(paths["legacy-house"], "village");
    assert_eq!(paths["kingdom-house"], "Kingdom/Wohnhäuser");
    assert_eq!(paths["root-house"], "");
    for bad in [
        "/absolute",
        "a//b",
        "../b",
        "a/..",
        "a\\b",
        "C:drive",
        "a/ b",
        "a\nb",
    ] {
        assert!(!valid_category_path(bad));
    }
    assert!(!valid_category_path(&"a".repeat(129)));
}
