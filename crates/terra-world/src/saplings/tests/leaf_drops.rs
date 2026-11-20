use super::*;

#[test]
fn leaf_roll_is_twenty_percent_across_a_fixed_negative_and_positive_sample() {
    let cells: Vec<_> = (-100..100_i64)
        .flat_map(|x| (-25..25_i64).map(move |z| [x, -7, z]))
        .collect();
    let expected: Vec<_> = cells.iter().copied().map(leaf_drop_roll).collect();
    let successes = expected.iter().filter(|roll| **roll).count();
    assert!(
        (1900..=2100).contains(&successes),
        "{successes}/10000 leaf drops"
    );
    // Retrying or traversing another chunk order cannot reroll a mined leaf.
    let reversed: Vec<_> = cells.iter().rev().copied().map(leaf_drop_roll).collect();
    assert!(expected.iter().rev().eq(reversed.iter()));
}

#[test]
fn leaf_roll_is_twenty_percent_along_negative_rows_and_mixes_small_crowns() {
    let negative: Vec<_> = (-10_000..0_i64)
        .map(|x| leaf_drop_roll([x, -1, -1]))
        .collect();
    let successes = negative.iter().filter(|roll| **roll).count();
    assert!(
        (1900..=2100).contains(&successes),
        "{successes}/10000 leaf drops along a negative row"
    );
    // A coordinate avalanche also avoids the previously observed residue
    // starvation in these actual compact crown/negative-region fixtures.
    for cells in [
        (-32..0_i64).map(|x| [x, -1, -1]).collect::<Vec<_>>(),
        (1..16_i64).map(|x| [x, 18, 0]).collect::<Vec<_>>(),
    ] {
        assert!(cells.iter().copied().any(leaf_drop_roll));
        assert!(cells.iter().copied().any(|cell| !leaf_drop_roll(cell)));
    }
}

#[test]
fn every_grown_species_retains_its_own_seed_with_the_shared_twenty_percent_roll() {
    for species in TreeSpecies::ALL {
        let (mut chunks, mut journal, prop) = fixture(species, 811);
        assert!(plant(&mut chunks, &mut journal, prop, PlayerUuid::from_u128(1)).unwrap());
        ready(&mut journal, 811);
        assert!(grow(&mut chunks, &mut journal, 811).unwrap());
        let tree = journal.gameplay().trees.get(811).unwrap();
        let mut successes = 0;
        for cell in tree.leaves.iter().map(|offset| tree.global(*offset)) {
            let actual = leaf_drop(&chunks, &journal, cell, species.leaves());
            let expected = leaf_drop_roll(cell).then_some(species);
            assert_eq!(actual, expected, "{species:?} at {cell:?}");
            assert_eq!(leaf_drop(&chunks, &journal, cell, species.leaves()), actual);
            successes += usize::from(actual.is_some());
        }
        assert!(successes > 0, "fixture crown includes successful rolls");
    }
}

#[test]
fn placed_or_unattributed_leaf_materials_use_their_matching_seed_fallback() {
    let chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let winning = (-32..0_i64)
        .map(|x| [x, -1, -1])
        .find(|cell| leaf_drop_roll(*cell))
        .unwrap();
    let losing = (-32..0_i64)
        .map(|x| [x, -1, -1])
        .find(|cell| !leaf_drop_roll(*cell))
        .unwrap();
    for (material, species) in [
        (7, TreeSpecies::Oak),
        (8, TreeSpecies::Oak),
        (9, TreeSpecies::Birch),
        (132, TreeSpecies::Cherry),
        (133, TreeSpecies::Cherry),
        (195, TreeSpecies::Apple),
    ] {
        assert_eq!(leaf_species(material), Some(species));
        assert_eq!(
            leaf_drop(&chunks, &journal, winning, material),
            Some(species)
        );
        assert_eq!(leaf_drop(&chunks, &journal, losing, material), None);
        journal.record(winning, material).unwrap();
        assert_eq!(
            leaf_drop(&chunks, &journal, winning, material),
            Some(species)
        );
    }
    for material in [0, 1, 6, 10, 36, 134] {
        assert!(leaf_species(material).is_none());
        assert!(leaf_drop(&chunks, &journal, winning, material).is_none());
    }
}
