use super::*;
use terra_core::kingdom::{KingdomId, STARTER_COUNT};

fn root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "terra-starter-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn founded() -> (EditJournal, KingdomId) {
    let mut journal = EditJournal::default();
    record_keep(&mut journal, [8, 2, 8]);
    let mut next = journal.gameplay().kingdoms.clone();
    let id = next
        .found(PlayerUuid::from_u128(80), [8, 2, 8], "Persistent Starters")
        .unwrap();
    let positions =
        std::array::from_fn(|i| [2.5 + (i % 3) as f64 * 2., 2., 2.5 + (i / 3) as f64 * 2.]);
    journal
        .gameplay_mut()
        .commit_founding_with_starters(next, id, positions)
        .unwrap();
    (journal, id)
}

fn record_keep(journal: &mut EditJournal, position: [i64; 3]) {
    journal
        .record(position, terra_core::castle::BERGFRIED)
        .unwrap();
    journal
        .set_object_state(
            position,
            Some(terra_world::edits::BlockObjectState::default()),
        )
        .unwrap();
}

#[test]
fn id23_persists_exact_starters_and_never_replenishes_dead_or_removed_residents() {
    let root = root("roundtrip");
    fs::create_dir_all(&root).unwrap();
    let world = WorldId(80023);
    let (journal, id) = founded();
    save_edits(&root, world, &journal).unwrap();
    let bytes = read(&root.join(INDEX)).unwrap();
    assert_eq!(bytes.get(12..16), Some(b"ID23".as_slice()));
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    let ids = loaded.gameplay().kingdom_starters.grants[&id].ids;
    assert_eq!(loaded.gameplay().creatures.animals.len(), STARTER_COUNT);
    let game = loaded.gameplay_mut();
    let npc = game.creatures.animals.get_mut(&ids[0]).unwrap();
    npc.health = 0.;
    npc.behavior = terra_core::creatures::CreatureBehavior::Dead;
    game.creatures.animals.remove(&ids[1]);
    game.creatures.owners.remove(&ids[1]);
    game.revision += 1;
    save_edits(&root, world, &loaded).unwrap();
    let again = load_edits(&root, world).unwrap();
    assert_eq!(again.gameplay().kingdom_starters.grants[&id].ids, ids);
    assert_eq!(again.gameplay().creatures.animals.len(), STARTER_COUNT - 1);
    assert_eq!(again.gameplay().creatures.animals[&ids[0]].health, 0.);
    assert!(!again.gameplay().creatures.animals.contains_key(&ids[1]));
    save_edits(&root, world, &again).unwrap();
    assert_eq!(
        load_edits(&root, world).unwrap().gameplay(),
        again.gameplay()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn frozen_id22_kingdoms_load_without_receipts_or_retroactive_spawns() {
    let root = root("legacy22");
    fs::create_dir_all(&root).unwrap();
    let world = WorldId(80022);
    let mut journal = EditJournal::default();
    record_keep(&mut journal, [8, 2, 8]);
    journal
        .gameplay_mut()
        .kingdoms
        .found(PlayerUuid::from_u128(80), [8, 2, 8], "Legacy Kingdom")
        .unwrap();
    journal.gameplay_mut().jobs.revision = 1;
    save_edits(&root, world, &journal).unwrap();
    assert_eq!(
        read(&root.join(INDEX)).unwrap().get(12..16),
        Some(b"ID22".as_slice())
    );
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().kingdoms, journal.gameplay().kingdoms);
    assert!(loaded.gameplay().kingdom_starters.grants.is_empty());
    assert!(loaded.gameplay().creatures.animals.is_empty());
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(
        read(&root.join(INDEX)).unwrap().get(12..16),
        Some(b"ID22".as_slice())
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn id23_rejects_duplicate_mismatched_or_orphaned_receipts() {
    let root = root("invalid23");
    fs::create_dir_all(&root).unwrap();
    let world = WorldId(80024);
    let (journal, id) = founded();
    save_edits(&root, world, &journal).unwrap();
    let bytes = read(&root.join(INDEX)).unwrap();
    for invalid in 0..3 {
        let mut index: StarterIndex = decode(b"ID23", &bytes).unwrap();
        let grant = index.starters.grants.get_mut(&id).unwrap();
        match invalid {
            0 => grant.ids[1] = grant.ids[0],
            1 => grant.founder = PlayerUuid::from_u128(99),
            _ => index.starters.grants.clear(),
        }
        // Deliberately replace the saved bytes to exercise corrupt-index loading;
        // the production write helper intentionally refuses existing files.
        fs::write(root.join(INDEX), encode(b"ID23", &index).unwrap()).unwrap();
        assert!(load_edits(&root, world).is_err());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn distant_starter_claim_homes_roundtrip_beyond_the_legacy_habitat_boundary() {
    let root = root("distant");
    fs::create_dir_all(&root).unwrap();
    let world = WorldId(80025);
    let mut journal = EditJournal::default();
    let keep = [900_008, 2, -900_008];
    record_keep(&mut journal, keep);
    let mut next = journal.gameplay().kingdoms.clone();
    let id = next
        .found(PlayerUuid::from_u128(80), keep, "Distant Starters")
        .unwrap();
    let home = terra_core::kingdom::claim_chunk(keep);
    let positions = std::array::from_fn(|i| {
        [
            (home[0] * 16) as f64 + 2.5 + (i % 3) as f64 * 2.,
            2.,
            (home[1] * 16) as f64 + 2.5 + (i / 3) as f64 * 2.,
        ]
    });
    journal
        .gameplay_mut()
        .commit_founding_with_starters(next, id, positions)
        .unwrap();
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    assert_eq!(loaded.gameplay().creatures.animals.len(), STARTER_COUNT);
    fs::remove_dir_all(root).unwrap();
}
