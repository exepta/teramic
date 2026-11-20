use super::*;

fn leaf_fixture(
    wood: u16,
    leaves_material: u16,
    species: terra_core::TreeSpecies,
) -> (ChunkStore, EditJournal, Plan, usize) {
    let root = [4, 16, 4];
    let leaves: Vec<_> = (0..10)
        .flat_map(|x| (0..10).map(move |z| [x, 19, z]))
        .collect();
    let expected = leaves
        .iter()
        .filter(|cell| crate::saplings::leaf_drop_roll(**cell))
        .count();
    let section = global_to_section(root).0;
    let mut data = BlockSection::empty(section).unwrap();
    data.set_block(global_to_section(root).1, wood).unwrap();
    for cell in &leaves {
        data.set_block(global_to_section(*cell).1, leaves_material)
            .unwrap();
    }
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        section,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![],
        },
    );
    let plan = Plan {
        planted: None,
        species: Some(species),
        origin: root,
        foot: root,
        material: wood,
        leaves_material,
        wood: vec![root],
        leaves,
        revisions: vec![(section, 0)],
        domains: vec![],
    };
    (chunks, EditJournal::default(), plan, expected)
}

fn seed_count(journal: &EditJournal, species: terra_core::TreeSpecies) -> usize {
    journal
        .gameplay()
        .drops
        .iter()
        .filter(|drop| drop.stack.item == ItemKind::Prop(species.sapling()))
        .map(|drop| usize::from(drop.stack.count))
        .sum()
}

#[test]
fn player_and_worker_felling_use_the_same_leaf_roll_and_keep_actual_material_variants() {
    for (wood, leaves, species) in [
        (6, 8, terra_core::TreeSpecies::Oak),
        (6, 7, terra_core::TreeSpecies::Spruce),
        (6, 133, terra_core::TreeSpecies::Cherry),
        (10, 9, terra_core::TreeSpecies::Birch),
        (6, 195, terra_core::TreeSpecies::Apple),
    ] {
        let (mut chunks, mut journal, plan, expected) = leaf_fixture(wood, leaves, species);
        let mut queue = Queue::default();
        assert!(queue.enqueue(plan, PlayerUuid::from_u128(1)));
        let first = queue
            .advance(&mut chunks, &mut journal, &Default::default(), 0.0)
            .unwrap();
        assert_eq!(first.wood, 1);
        assert_eq!(
            seed_count(&journal, species),
            0,
            "logs never mint a free replacement seed"
        );
        let mut now = LEAF_DELAY;
        while !queue.is_empty() {
            let batch = queue
                .advance(&mut chunks, &mut journal, &Default::default(), now)
                .unwrap();
            assert!(batch.leaves <= LEAF_BUDGET && batch.drops <= 4);
            now += 1.0;
        }
        assert_eq!(seed_count(&journal, species), expected);

        let (mut chunks, mut journal, plan, expected) = leaf_fixture(wood, leaves, species);
        let mut task = plan.worker_task().unwrap();
        assert!(
            worker_cut_one(
                &mut chunks,
                &mut journal,
                &Default::default(),
                PlayerUuid::from_u128(1),
                &mut task,
                true
            )
            .unwrap()
        );
        assert_eq!(seed_count(&journal, species), 0);
        assert_eq!(
            journal.gameplay().drops[0].stack.item,
            ItemKind::Block(wood)
        );
        while !task.leaves.is_empty() {
            assert!(
                worker_cut_one(
                    &mut chunks,
                    &mut journal,
                    &Default::default(),
                    PlayerUuid::from_u128(1),
                    &mut task,
                    false
                )
                .unwrap()
            );
        }
        assert_eq!(seed_count(&journal, species), expected);
        assert_eq!(task.drops.len(), expected + 1);
        assert!(task.valid() && journal.gameplay().valid());
        let before = journal.gameplay().drops.clone();
        assert!(
            !worker_cut_one(
                &mut chunks,
                &mut journal,
                &Default::default(),
                PlayerUuid::from_u128(1),
                &mut task,
                false
            )
            .unwrap()
        );
        assert_eq!(
            journal.gameplay().drops,
            before,
            "retrying consumed leaves cannot duplicate loot"
        );
    }
}
