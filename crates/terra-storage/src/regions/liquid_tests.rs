use super::*;
use terra_world::edits::{BlockObjectState, LiquidState};
#[test]
fn acid_sources_and_fine_tar_volume_survive_real_region_save_and_section_delta() {
    let root = std::env::temp_dir().join(format!("terra-liquid-save-{}", PlayerUuid::new_v4()));
    let world = WorldId(6439);
    let mut journal = EditJournal::default();
    let records = [
        (
            [-1, 7, -1],
            439,
            LiquidState {
                origin: [-1, 7, -1],
                volume: 8,
                age: 0,
                source: true,
            },
        ),
        (
            [0, 7, -1],
            439,
            LiquidState {
                origin: [-1, 7, -1],
                volume: 6,
                age: 0,
                source: false,
            },
        ),
        (
            [511, 3, 1],
            437,
            LiquidState {
                origin: [511, 3, 1],
                volume: 23,
                age: 17,
                source: true,
            },
        ),
        (
            [512, 3, 1],
            437,
            LiquidState {
                origin: [511, 3, 1],
                volume: 22,
                age: 17,
                source: false,
            },
        ),
        (
            [514, 3, 1],
            438,
            LiquidState {
                origin: [511, 3, 1],
                volume: 2,
                age: 0,
                source: false,
            },
        ),
    ];
    for (p, id, liquid) in records {
        journal.record(p, id).unwrap();
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    facing: if id == 439 {
                        liquid.volume - 1
                    } else {
                        terra_core::roads::tar_state(liquid.volume.div_ceil(8), liquid.age)
                    },
                    liquid: Some(liquid),
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    // An old packed tar record still uses the frozen legacy shape.
    let legacy = [20, 7, 0];
    journal.record(legacy, 437).unwrap();
    journal
        .set_object_state(
            legacy,
            Some(BlockObjectState {
                facing: terra_core::roads::tar_state(4, 21),
                ..Default::default()
            }),
        )
        .unwrap();
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    for (p, id, _) in records {
        assert_eq!(loaded.block(p), Some(id));
        assert_eq!(loaded.object_state(p), journal.object_state(p));
    }
    assert_eq!(loaded.object_state(legacy), journal.object_state(legacy));
    let mut replica = EditJournal::default();
    for section in loaded.changed_sections() {
        let bytes = postcard::to_stdvec(&loaded.section_delta(section)).unwrap();
        replica
            .import_delta(postcard::from_bytes(&bytes).unwrap())
            .unwrap();
    }
    for (p, _, _) in records {
        assert_eq!(replica.object_state(p), loaded.object_state(p));
    }
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(
        load_edits(&root, world).unwrap().object_state(records[2].0),
        journal.object_state(records[2].0)
    );
    fs::remove_dir_all(root).unwrap();
}
