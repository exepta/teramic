//! Loaded voxels only: drawing an overlay never runs world generation.
use terra_voxel::MaterialId;
use terra_world::{edits::EditJournal, streaming::ChunkStore};

fn supports_band(material: u16) -> bool {
    MaterialId::from_raw(material).is_some_and(|material| {
        // Ignore foliage and trunks; a forest's working area follows its floor.
        let tree = matches!(material as u16, 6..=10 | 36 | 132..=134 | 195 | 310..=313);
        !tree && (material.is_natural_ground() || material.is_opaque())
    })
}

pub(super) fn height(
    chunks: &ChunkStore,
    journal: &EditJournal,
    x: i64,
    z: i64,
    fallback: i64,
) -> Option<f32> {
    let hint = chunks
        .surface_column_at(x, z)
        .map_or(fallback, |column| column.height.ceil() as i64);
    let mut entered = false;
    for y in (hint.saturating_sub(32)..=hint.saturating_add(12)).rev() {
        let p = [x, y, z];
        let raw = match chunks.block_at(p) {
            Some(raw) => {
                entered = true;
                raw
            }
            None if entered => return None,
            None => continue,
        };
        let material = journal.block(p).unwrap_or(raw);
        if supports_band(material) {
            return Some(y as f32 + 1.045);
        }
    }
    None
}
