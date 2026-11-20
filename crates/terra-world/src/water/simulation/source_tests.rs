use super::*;
use crate::streaming::{BlockEditRequest, LoadedSection, commit_block_edit};
use terra_core::WorldConfig;
use terra_voxel::{BlockSection, SectionPos};

fn fixture() -> (TerraGenerator, ChunkStore, EditJournal) {
    let terrain = TerraGenerator::new(WorldConfig {
        sea_level: -100.,
        caves: false,
        rivers: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for x in 0..=1 {
        let p = SectionPos::new(x, 0, 0);
        chunks.loaded.insert(
            p,
            LoadedSection {
                data: BlockSection::from_blocks(p, 0, vec![3; 4096]).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
    }
    let mut journal = EditJournal::default();
    // A sealed channel crosses a streaming seam. Nothing can drain by gravity
    // or spread out, so disappearing water proves source recession specifically.
    for x in 12..=20 {
        let p = [x, 8, 8];
        let (owner, local) = global_to_section(p);
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(local, 0)
            .unwrap();
        journal.record(p, 0).unwrap();
        journal
            .set_fluid(
                p,
                FluidCell {
                    amount: 8,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    (terrain, chunks, journal)
}

fn source(chunks: &mut ChunkStore, journal: &mut EditJournal, p: [i64; 3], place: bool) {
    commit_block_edit(
        chunks,
        journal,
        &BlockEditRequest {
            position: p,
            expected_material: if place { 0 } else { 27 },
            material: if place { 27 } else { 0 },
        },
        &Default::default(),
    )
    .unwrap();
}
fn advance(
    terrain: &TerraGenerator,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    ticks: usize,
) -> u64 {
    (0..ticks)
        .map(|_| {
            simulate_fluid_step(terrain, chunks, journal, 4096)
                .unwrap()
                .drained
        })
        .sum()
}
fn wet(journal: &EditJournal, range: std::ops::RangeInclusive<i64>) -> u64 {
    range
        .map(|x| u64::from(journal.fluid([x, 8, 8]).unwrap_or_default().amount))
        .sum()
}

#[test]
fn removing_the_only_source_drains_even_a_trapped_level_pool_and_survives_reload() {
    let (terrain, mut chunks, mut journal) = fixture();
    let p = [14, 8, 8];
    source(&mut chunks, &mut journal, p, true);
    assert_eq!(advance(&terrain, &chunks, &mut journal, 12), 0);
    assert!(wet(&journal, 16..=20) > 0);
    source(&mut chunks, &mut journal, p, false);
    assert_eq!(journal.fluid(p), Some(FluidCell::default()));
    let mut reloaded = EditJournal::default();
    for owner in journal.changed_sections() {
        let bytes = serde_json::to_vec(&journal.section_delta(owner)).unwrap();
        reloaded
            .import_delta(serde_json::from_slice(&bytes).unwrap())
            .unwrap();
    }
    assert!(advance(&terrain, &chunks, &mut reloaded, 64) > 0);
    assert_eq!(
        wet(&reloaded, 12..=20),
        0,
        "Trapped downstream water survived its only source"
    );
    assert!(
        chunks
            .water_column_at_point(&terrain, &reloaded, [18.5, 8.2, 8.5])
            .is_none()
    );
}

#[test]
fn unloaded_dry_edges_do_not_keep_the_removed_sources_visible_stream_alive() {
    let (terrain, mut chunks, mut journal) = fixture();
    source(&mut chunks, &mut journal, [14, 8, 8], true);
    advance(&terrain, &chunks, &mut journal, 10);
    assert_eq!(
        journal.fluid_work_counts().active,
        0,
        "The regression needs a fully sleeping downstream basin"
    );
    source(&mut chunks, &mut journal, [14, 8, 8], false);
    chunks.loaded.remove(&SectionPos::new(0, 0, 0));
    assert!(advance(&terrain, &chunks, &mut journal, 64) > 0);
    assert_eq!(
        wet(&journal, 16..=20),
        0,
        "A dry unknown seam sustained an orphaned stream"
    );
}

#[test]
fn saved_offscreen_alternative_source_preserves_water_until_it_is_removed_too() {
    let (terrain, mut chunks, mut journal) = fixture();
    source(&mut chunks, &mut journal, [13, 8, 8], true);
    source(&mut chunks, &mut journal, [12, 8, 8], true);
    advance(&terrain, &chunks, &mut journal, 10);
    source(&mut chunks, &mut journal, [12, 8, 8], false);
    let left = chunks.loaded.remove(&SectionPos::new(0, 0, 0)).unwrap();
    // Force an actual source proof in the visible owner. Merely leaving a
    // sleeping offscreen-supported pool untouched would not test its witness.
    journal.wake_fluid_neighbors([18, 8, 8]);
    let proof = simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
    assert!(proof.visited > 0);
    assert_eq!(proof.drained, 0);
    assert_eq!(advance(&terrain, &chunks, &mut journal, 32), 0);
    assert!(
        wet(&journal, 16..=20) > 0,
        "The real offscreen source was forgotten"
    );
    chunks.loaded.insert(SectionPos::new(0, 0, 0), left);
    source(&mut chunks, &mut journal, [13, 8, 8], false);
    chunks.loaded.remove(&SectionPos::new(0, 0, 0));
    assert!(advance(&terrain, &chunks, &mut journal, 64) > 0);
    assert_eq!(wet(&journal, 16..=20), 0);
}

#[test]
fn an_explicit_source_cut_off_by_a_new_solid_bank_cannot_keep_its_old_pool_alive() {
    let (terrain, mut chunks, mut journal) = fixture();
    source(&mut chunks, &mut journal, [13, 8, 8], true);
    advance(&terrain, &chunks, &mut journal, 10);
    commit_block_edit(
        &mut chunks,
        &mut journal,
        &BlockEditRequest {
            position: [15, 8, 8],
            expected_material: 0,
            material: 3,
        },
        &Default::default(),
    )
    .unwrap();
    assert!(advance(&terrain, &chunks, &mut journal, 64) > 0);
    assert_eq!(wet(&journal, 16..=20), 0);
    assert!(journal.fluid([13, 8, 8]).unwrap().source);
}
