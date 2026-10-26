use super::*;
use terra_core::creatures::{
    Creature, CreatureBehavior, CreatureKind, CreatureNeeds,
    taming::{self, AnimalBond, CompanionMode},
};

#[test]
fn rga5_explicit_pet_commands_roundtrip_but_rga3_follow_is_never_activated() {
    let root = std::env::temp_dir().join(format!("terra-pet-region-test-{}", PlayerUuid::new_v4()));
    let world = WorldId(120);
    let owner = PlayerUuid::from_u128(120);
    let id = (1 << 62) + 120;
    let mut journal = EditJournal::default();
    let state = journal.gameplay_mut();
    state.creatures.animals.insert(
        id,
        Creature {
            id,
            kind: CreatureKind::Cow,
            home: [-17, -1],
            position: [-530.5, 3.0, -16.5],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: CreatureKind::Cow.health(),
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(CreatureKind::Cow),
        },
    );
    state.creatures.owners.insert(id, owner);
    state.animal_bonds.insert(
        id,
        AnimalBond {
            mode: CompanionMode::Follow,
            ..AnimalBond::owned(owner)
        },
    );
    assert!(
        state.valid(),
        "the real definition's health/food limits apply to this fixture too"
    );
    save_edits(&root, world, &journal).unwrap();
    let legacy = load_edits(&root, world).unwrap();
    assert!(legacy.gameplay().pet_orders.is_empty());
    assert_eq!(
        taming::order(legacy.gameplay(), id, CreatureKind::Cow),
        CompanionMode::Wait
    );
    for mode in [
        CompanionMode::Follow,
        CompanionMode::Wait,
        CompanionMode::None,
    ] {
        assert!(taming::command(journal.gameplay_mut(), id, owner, mode));
        save_edits(&root, world, &journal).unwrap();
        let bytes = read(&root.join(region_name([-2, -1]))).unwrap();
        assert_eq!(bytes.get(12..16), Some(b"RGA5".as_slice()));
        let loaded = load_edits(&root, world).unwrap();
        assert_eq!(loaded.gameplay(), journal.gameplay());
        assert_eq!(
            taming::order(loaded.gameplay(), id, CreatureKind::Cow),
            mode
        );
        let mut corrupt: PetRegion = decode(b"RGA5", &bytes).unwrap();
        corrupt.orders.push((id, mode));
        fs::write(
            root.join(region_name([-2, -1])),
            encode(b"RGA5", &corrupt).unwrap(),
        )
        .unwrap();
        assert!(load_edits(&root, world).is_err());
    }
    fs::remove_dir_all(root).unwrap();
}
