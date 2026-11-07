//! Regenerates UI-independent world/cape glyph masks from the canonical Tilt catalog.
//! Compile this helper against the workspace's `resvg` and `tilt_ui_icons` rlibs,
//! then run it from the repository root. No image asset or runtime UI dependency
//! is added to terra-core. Run rustfmt on the generated Rust catalog afterward.
use std::{fmt::Write as _, fs};
use tilt_ui_icons::{Icon, IconSize};
fn id(name: &str) -> u64 {
    name.bytes().fold(0xcbf29ce484222325, |v, b| {
        (v ^ u64::from(b)).wrapping_mul(0x100000001b3)
    })
}
fn mask(svg: &str) -> [u32; 32] {
    let tree = resvg::usvg::Tree::from_str(
        &svg.replace("currentColor", "#ffffff"),
        &resvg::usvg::Options::default(),
    )
    .unwrap();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(32, 32).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let mut rows = [0; 32];
    for y in 0..32 {
        for x in 0..32 {
            if pixmap.data()[(y * 32 + x) * 4 + 3] >= 96 {
                rows[y] |= 1 << x;
            }
        }
    }
    assert!(rows.iter().any(|r| *r != 0));
    rows
}
fn main() {
    let mut generated_source = String::from(
        "//! Stable heraldry glyphs generated from the TiltUI icon catalog.\n//! Names use FNV-1a IDs; saved emblems never depend on catalog ordering.\n//! Regenerate with tools/generate_heraldry_catalog.rs; 32 rows, bit x = column x.\n\npub(super) struct Glyph { pub id: u64, pub name: &'static str, pub mask: [u32; 32] }\npub(super) static GLYPHS: &[Glyph] = &[\n",
    );
    let mut icons = Icon::ALL
        .iter()
        .map(|icon| (icon.name(), icon.svg(IconSize::Px32)))
        .collect::<Vec<_>>();
    for name in ["wheat", "hammer"] {
        icons.push((
            name,
            fs::read_to_string(format!("src-ui/assets/heraldry-{name}.svg")).unwrap(),
        ));
    }
    icons.sort_by_key(|(name, _)| *name);
    icons.dedup_by_key(|(name, _)| *name);
    let mut ids = std::collections::HashSet::new();
    for (name, svg) in icons {
        assert!(ids.insert(id(name)));
        let rows = mask(&svg);
        write!(
            generated_source,
            "    Glyph {{ id: 0x{:016x}, name: {name:?}, mask: [",
            id(name)
        )
        .unwrap();
        for row in rows {
            write!(generated_source, "0x{row:08x},").unwrap();
        }
        generated_source.push_str("] },\n");
    }
    generated_source.push_str("];\n");
    fs::write(
        "crates/terra-core/src/kingdom/icon_catalog.rs",
        generated_source,
    )
    .unwrap();
}
