use super::*;
use crate::{
    edits::{EditJournal, SectionDelta},
    props::{PropId, PropKind, WorldProp},
    water::FluidCell,
};

const OWNER: SectionPos = SectionPos::new(-1, 0, -1);

fn crop(index: usize) -> WorldProp {
    WorldProp {
        id: PropId(index as u64 + 1),
        kind: PropKind::Corn,
        position: [-15.5 + (index % 8) as f64, 1.0, -15.5 + (index / 8) as f64],
        rotation: 0.0,
        scale: 1.0,
        variant: 7,
        section: OWNER,
        removed: false,
    }
}

fn fixture() -> (ChunkStore, EditJournal, EditJournal) {
    let mut source = EditJournal::default();
    for index in 0..64 {
        assert!(source.place_prop(crop(index)).unwrap());
    }
    let mut chunks = ChunkStore::default();
    chunks.wanted.insert(OWNER);
    chunks.loaded.insert(
        OWNER,
        LoadedSection {
            data: BlockSection::empty(OWNER).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    let mut replica = EditJournal::default();
    assert_eq!(replica.prop_epoch(OWNER), 0);
    chunks
        .apply_confirmed_delta(&mut replica, source.section_delta(OWNER))
        .unwrap();
    assert_eq!(chunks.loaded[&OWNER].props.len(), 64);
    assert_eq!(replica.prop_epoch(OWNER), 64);
    (chunks, replica, source)
}

#[test]
fn water_and_terrain_deltas_leave_unchanged_loaded_crop_groups_completely_untouched() {
    let (mut chunks, mut replica, mut source) = fixture();
    // Ordering is not part of prop identity. Reversing it witnesses an otherwise
    // invisible redundant reflow without instrumentation in production code.
    chunks.loaded.get_mut(&OWNER).unwrap().props.reverse();
    let expected = chunks.loaded[&OWNER].props.clone();
    let pointer = chunks.loaded[&OWNER].props.as_ptr();
    let capacity = chunks.loaded[&OWNER].props.capacity();
    for tick in 0..12 {
        let fluid = FluidCell {
            amount: tick % 7 + 1,
            flow: [0, 1],
            source: false,
        };
        assert!(source.set_fluid([-1, 2, -1], fluid).unwrap());
        let delta = source.section_delta(OWNER);
        assert_eq!(delta.prop_epoch, 64);
        chunks.apply_confirmed_delta(&mut replica, delta).unwrap();
        assert_eq!(replica.fluid([-1, 2, -1]), Some(fluid));
        assert_eq!(chunks.loaded[&OWNER].props, expected);
        assert!(chunks.dirty.is_empty());
        assert!(chunks.water_dirty.contains(&OWNER));
    }
    for material in [3, 2, 3] {
        source.record([-1, 3, -1], material).unwrap();
        chunks
            .apply_confirmed_delta(&mut replica, source.section_delta(OWNER))
            .unwrap();
        assert_eq!(replica.block([-1, 3, -1]), Some(material));
        assert_eq!(chunks.loaded[&OWNER].props, expected);
        assert!(chunks.dirty.contains(&OWNER));
    }
    assert_eq!(chunks.loaded[&OWNER].props.as_ptr(), pointer);
    assert_eq!(chunks.loaded[&OWNER].props.capacity(), capacity);
}

#[test]
fn changed_prop_epochs_still_apply_initial_growth_removal_and_insertion() {
    let (mut chunks, mut replica, mut source) = fixture();
    let mut growing = crop(17);
    growing.variant = 3;
    assert!(source.update_placed_prop(growing).unwrap());
    chunks
        .apply_confirmed_delta(&mut replica, source.section_delta(OWNER))
        .unwrap();
    assert_eq!(replica.prop_epoch(OWNER), 65);
    assert_eq!(
        chunks.loaded[&OWNER]
            .props
            .iter()
            .find(|prop| prop.id == growing.id),
        Some(&growing)
    );
    let stale = source.section_delta(OWNER);
    assert!(source.remove_prop(OWNER, growing.id).unwrap());
    let additional = WorldProp {
        position: [-1.5, 1.0, -1.5],
        id: PropId(1000),
        ..crop(0)
    };
    assert!(source.place_prop(additional).unwrap());
    chunks
        .apply_confirmed_delta(&mut replica, source.section_delta(OWNER))
        .unwrap();
    assert_eq!(replica.prop_epoch(OWNER), 67);
    assert!(
        chunks.loaded[&OWNER]
            .props
            .iter()
            .any(|prop| prop.id == growing.id && prop.removed)
    );
    assert!(chunks.loaded[&OWNER].props.contains(&additional));
    chunks.loaded.get_mut(&OWNER).unwrap().props.reverse();
    let expected = chunks.loaded[&OWNER].props.clone();
    chunks.apply_confirmed_delta(&mut replica, stale).unwrap();
    assert_eq!(chunks.loaded[&OWNER].props, expected);
    chunks
        .apply_confirmed_delta(&mut replica, source.section_delta(OWNER))
        .unwrap();
    assert_eq!(chunks.loaded[&OWNER].props, expected);
    assert_eq!(replica.prop_epoch(OWNER), 67);
}

#[test]
fn same_prop_epoch_rehydrates_replaced_sections_and_reapplies_tombstones() {
    let (mut chunks, mut replica, mut source) = fixture();
    assert!(source.remove_prop(OWNER, crop(17).id).unwrap());
    let delta = source.section_delta(OWNER);
    chunks
        .apply_confirmed_delta(&mut replica, delta.clone())
        .unwrap();
    let epoch = replica.prop_epoch(OWNER);
    let mut section = chunks.loaded.remove(&OWNER).unwrap();
    // A replacement worker can contain its original natural prop snapshot but
    // none of the player placements already imported by the journal.
    section.props = vec![crop(17)];
    chunks.loaded.insert(OWNER, section);
    chunks
        .apply_confirmed_delta(&mut replica, delta.clone())
        .unwrap();
    assert_eq!(replica.prop_epoch(OWNER), epoch);
    assert_eq!(chunks.loaded[&OWNER].props.len(), 64);
    for index in 0..64 {
        let matches: Vec<_> = chunks.loaded[&OWNER]
            .props
            .iter()
            .filter(|prop| prop.id == crop(index).id)
            .collect();
        assert_eq!(matches.len(), 1, "prop {index}");
        assert_eq!(matches[0].removed, index == 17);
    }
    // Reusing the same Vec allocation does not conceal a missing placement.
    let mut section = chunks.loaded.remove(&OWNER).unwrap();
    section.props.retain(|prop| prop.id != crop(8).id);
    chunks.loaded.insert(OWNER, section);
    chunks
        .apply_confirmed_delta(&mut replica, delta.clone())
        .unwrap();
    assert_eq!(chunks.loaded[&OWNER].props.len(), 64);
    chunks.loaded.get_mut(&OWNER).unwrap().props.reverse();
    let expected = chunks.loaded[&OWNER].props.clone();
    chunks.apply_confirmed_delta(&mut replica, delta).unwrap();
    assert_eq!(chunks.loaded[&OWNER].props, expected);
}

#[test]
fn same_epoch_hydration_repairs_growth_and_support_without_reflowing_derived_flower_groups() {
    let (mut chunks, mut replica, mut source) = fixture();
    chunks.loaded.get_mut(&OWNER).unwrap().props[0].variant = 2;
    chunks
        .apply_confirmed_delta(&mut replica, source.section_delta(OWNER))
        .unwrap();
    assert!(chunks.loaded[&OWNER].props.iter().all(|p| p.variant == 7));
    chunks.loaded.get_mut(&OWNER).unwrap().props[0].position[0] += 1.0;
    chunks
        .apply_confirmed_delta(&mut replica, source.section_delta(OWNER))
        .unwrap();
    for index in 0..64 {
        assert!(chunks.loaded[&OWNER].props.contains(&crop(index)));
    }
    // Crops cannot use a flower group's offset, even on the same support cell.
    chunks.loaded.get_mut(&OWNER).unwrap().props[0].position[0] += 0.125;
    chunks
        .apply_confirmed_delta(&mut replica, source.section_delta(OWNER))
        .unwrap();
    for index in 0..64 {
        assert!(chunks.loaded[&OWNER].props.contains(&crop(index)));
    }

    for id in [2001, 2002] {
        assert!(
            source
                .place_prop(WorldProp {
                    id: PropId(id),
                    kind: PropKind::Flower,
                    position: [-2.5, 1.0, -2.5],
                    scale: 0.5,
                    variant: 0,
                    ..crop(0)
                })
                .unwrap()
        );
    }
    chunks
        .apply_confirmed_delta(&mut replica, source.section_delta(OWNER))
        .unwrap();
    assert!(
        chunks.loaded[&OWNER]
            .props
            .iter()
            .any(|p| { p.kind == PropKind::Flower && p.position != [-2.5, 1.0, -2.5] })
    );
    assert!(!replica.props_need_sync(OWNER, &chunks.loaded[&OWNER].props));
    chunks.loaded.get_mut(&OWNER).unwrap().props.reverse();
    let expected = chunks.loaded[&OWNER].props.clone();
    source
        .set_fluid(
            [-1, 2, -1],
            FluidCell {
                amount: 4,
                flow: [0, 1],
                source: false,
            },
        )
        .unwrap();
    chunks
        .apply_confirmed_delta(&mut replica, source.section_delta(OWNER))
        .unwrap();
    assert_eq!(chunks.loaded[&OWNER].props, expected);
}

#[test]
#[ignore = "opt-in CPU comparison; timing is not a machine-dependent CI threshold"]
fn benchmark_water_only_deltas_do_not_reflow_sixty_four_mature_crops() {
    use std::{hint::black_box, time::Instant};

    let (_, _, mut source) = fixture();
    let deltas: Vec<SectionDelta> = (0..1000)
        .map(|tick| {
            source
                .set_fluid(
                    [-1, 2, -1],
                    FluidCell {
                        amount: (tick % 7 + 1) as u8,
                        flow: [0, 1],
                        source: false,
                    },
                )
                .unwrap();
            source.section_delta(OWNER)
        })
        .collect();
    for pass in 0..3 {
        let (mut previous_chunks, mut previous_journal, _) = fixture();
        let start = Instant::now();
        for delta in &deltas {
            previous_chunks
                .apply_confirmed_delta(&mut previous_journal, black_box(delta.clone()))
                .unwrap();
            previous_journal.apply_prop_removals(
                OWNER,
                &mut previous_chunks.loaded.get_mut(&OWNER).unwrap().props,
            );
        }
        let previous = start.elapsed().as_secs_f64() * 1000.0;
        let (mut chunks, mut journal, _) = fixture();
        let start = Instant::now();
        for delta in &deltas {
            chunks
                .apply_confirmed_delta(&mut journal, black_box(delta.clone()))
                .unwrap();
        }
        let guarded = start.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(
            chunks.loaded[&OWNER].props,
            previous_chunks.loaded[&OWNER].props
        );
        eprintln!(
            "confirmed_delta_crops pass={pass} updates=1000 mature_crops=64 previous_ms={previous:.3} guarded_ms={guarded:.3}"
        );
    }
}
