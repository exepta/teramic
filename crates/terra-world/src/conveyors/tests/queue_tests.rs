use super::*;

#[test]
fn horizontal_and_sloped_item_queues_back_up_and_drain_after_an_authored_overhang_is_removed() {
    for end in [[4, 4, 10], [4, 10, 10]] {
        let (mut chunks, mut journal) = fixture();
        let span = connect(&mut chunks, &mut journal, [4, 4, 4], end, 0);
        power(span, &mut journal);
        let obstructed = at(end, span.step, -1);
        let wall = [obstructed[0], obstructed[1] + 1, obstructed[2]];
        let (section, local) = global_to_section(wall);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 151)
            .unwrap();
        journal.record(wall, 151).unwrap();
        journal
            .set_object_state(wall, Some(BlockObjectState::default()))
            .unwrap();
        let properties =
            BlockPropertiesCatalog::load(&terra_core::asset_directory().join("terra/blocks"))
                .unwrap();
        for (position, count) in [(at(end, span.step, -4), 7), (at(end, span.step, -3), 4)] {
            let segment = segment_at(&journal, position).unwrap();
            let point = segment.drop_center().unwrap();
            assert!(journal.gameplay_mut().drop_stack(
                point,
                ItemStack {
                    item: ItemKind::Block(3),
                    count
                },
                [0.0; 3],
                0.0
            ));
            stabilize_carried_drop(journal.gameplay_mut().drops.last_mut().unwrap(), segment);
        }
        let identities: Vec<_> = journal
            .gameplay()
            .drops
            .iter()
            .map(|drop| (drop.id, drop.stack))
            .collect();
        let mut runtime = Transport::default();
        for _ in 0..50 {
            transport(&mut runtime, &chunks, &mut journal, &properties, 0.1);
            let drops = &journal.gameplay().drops;
            assert!(
                drops
                    .iter()
                    .all(|drop| carrying_segment(drop, &journal).is_some()),
                "the obstruction must stop items on the real carrying plane, not beyond the end drum"
            );
            assert!(
                (0..3)
                    .any(|axis| (drops[0].position[axis] - drops[1].position[axis]).abs() >= 0.26)
            );
        }
        let stopped: Vec<_> = journal
            .gameplay()
            .drops
            .iter()
            .map(|drop| drop.position)
            .collect();
        assert!(
            journal
                .gameplay()
                .drops
                .iter()
                .all(|drop| drop.velocity == [0.0; 3])
        );
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 0)
            .unwrap();
        journal.record(wall, 0).unwrap();
        for _ in 0..8 {
            transport(&mut runtime, &chunks, &mut journal, &properties, 0.1);
        }
        for (index, drop) in journal.gameplay().drops.iter().enumerate() {
            assert_eq!((drop.id, drop.stack), identities[index]);
            assert!(drop.position[2] > stopped[index][2] + 0.1);
        }
    }
}
