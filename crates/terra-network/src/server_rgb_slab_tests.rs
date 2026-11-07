use super::{
    rgb_filter_tests::{apply, fixture, look, place},
    *,
};
use terra_core::{ItemKind, ItemStack, ToolKind, beds::DyeColor};
use terra_world::edits::BlockObjectState;

fn equip(app: &mut App, peer: &Peer, item: ItemKind, count: u16) {
    assert!(
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap()
            .creative_set(0, ItemStack { item, count })
    );
}

fn block_command(
    app: &App,
    peer: &Peer,
    position: [i64; 3],
    expected: u16,
    material: u16,
) -> EditCommand {
    let journal = app.world().resource::<EditJournal>();
    EditCommand {
        request: 1,
        revision: journal.revision(terra_voxel::global_to_section(position).0),
        look: peer.look,
        source: (material != 0).then_some(InventorySource {
            slot: 0,
            revision: journal.gameplay().inventories[&peer.uuid].revision,
        }),
        action: EditAction::Block {
            position,
            expected,
            material,
        },
    }
}

#[test]
fn wool_slab_merging_rejects_different_rgb_before_any_mutation_but_accepts_equal_colors() {
    for (existing, incoming) in [
        (Some([17, 95, 213]), Some([221, 37, 103])),
        (None, Some([17, 95, 213])),
        (Some([17, 95, 213]), None),
        (None, Some([255; 3])),
        (Some([255; 3]), None),
        (Some([17, 95, 213]), Some([17, 95, 213])),
        (Some([255; 3]), Some([255; 3])),
        (None, None),
    ] {
        let (mut app, mut peer) = fixture();
        let position = [-1, 4, -1];
        place(&mut app, position, 105, Some(Default::default()));
        if let Some(rgb) = existing {
            app.world_mut()
                .resource_mut::<EditJournal>()
                .set_tint(position, Some(rgb))
                .unwrap();
        }
        let item = incoming
            .and_then(|rgb| terra_core::tints::recolored(ItemKind::Block(105), DyeColor::Rgb(rgb)))
            .unwrap_or(ItemKind::Block(105));
        equip(&mut app, &peer, item, 2);
        let center = Vec3::from_array(position.map(|v| v as f32)) + Vec3::new(0.5, 0.5, 0.5);
        look(
            &mut app,
            &mut peer,
            center + Vec3::new(0.0, 2.5, 0.4),
            center,
        );
        let command = block_command(&app, &peer, position, 105, 105);
        let owner = terra_voxel::global_to_section(position).0;
        let before = app.world().resource::<EditJournal>().section_delta(owner);
        let gameplay = app.world().resource::<EditJournal>().gameplay().clone();
        let block_revision = app.world().resource::<ChunkStore>().loaded[&owner]
            .data
            .revision();
        let outcome = apply(&mut app, &peer, command, false);
        let journal = app.world().resource::<EditJournal>();
        if existing != incoming {
            assert_eq!(
                outcome,
                EditOutcome::Invalid,
                "existing={existing:?} incoming={incoming:?}"
            );
            assert_eq!(journal.section_delta(owner), before);
            assert_eq!(journal.gameplay(), &gameplay);
            assert_eq!(
                app.world().resource::<ChunkStore>().loaded[&owner]
                    .data
                    .revision(),
                block_revision
            );
        } else {
            assert_eq!(outcome, EditOutcome::Applied);
            assert_eq!(journal.tint_at(position), existing);
            assert!(terra_core::construction::full_slab(
                journal.object_state(position).unwrap().facing
            ));
            assert_eq!(journal.gameplay().inventories[&peer.uuid].count(item), 1);
        }
    }
}

#[test]
fn equal_white_slab_metadata_roundtrips_through_real_merge_and_whole_block_mining() {
    for tint in [None, Some([255; 3])] {
        let (mut app, mut peer) = fixture();
        let position = [-1, 4, -1];
        place(&mut app, position, 105, Some(Default::default()));
        if let Some(rgb) = tint {
            app.world_mut()
                .resource_mut::<EditJournal>()
                .set_tint(position, Some(rgb))
                .unwrap();
        }
        let item = tint
            .and_then(|rgb| terra_core::tints::recolored(ItemKind::Block(105), DyeColor::Rgb(rgb)))
            .unwrap_or(ItemKind::Block(105));
        equip(&mut app, &peer, item, 2);
        let center = Vec3::from_array(position.map(|v| v as f32)) + Vec3::splat(0.5);
        look(
            &mut app,
            &mut peer,
            center + Vec3::new(0.0, 2.5, 0.4),
            center,
        );
        let merge = block_command(&app, &peer, position, 105, 105);
        assert_eq!(apply(&mut app, &peer, merge, false), EditOutcome::Applied);
        assert_eq!(
            app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].count(item),
            1
        );
        equip(&mut app, &peer, ItemKind::Tool(ToolKind::WoodPickaxe), 1);
        let command = block_command(&app, &peer, position, 105, 0);
        assert_eq!(apply(&mut app, &peer, command, true), EditOutcome::Applied);
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.gameplay().drops.len(), 1);
        assert_eq!(
            journal.gameplay().drops[0].stack,
            ItemStack { item, count: 2 }
        );
        assert_eq!(journal.tint_at(position), None);
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(position),
            Some(0)
        );
    }
}

#[test]
fn authoritative_mixed_half_mining_drops_only_the_clicked_rgb_wool_and_keeps_remaining_tint() {
    let rgb = [17, 95, 213];
    let wool = terra_core::tints::recolored(ItemKind::Block(105), DyeColor::Rgb(rgb)).unwrap();
    for (primary, secondary) in [(85, 105), (105, 85)] {
        for facing in [0, 4, 5, 6, 7, 8] {
            for selected in 0..2 {
                let (mut app, mut peer) = fixture();
                let position = [-1, 4, -1];
                let merged =
                    terra_core::construction::merged_slab(primary, facing, secondary).unwrap();
                place(
                    &mut app,
                    position,
                    primary,
                    Some(BlockObjectState {
                        facing: merged,
                        ..Default::default()
                    }),
                );
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .set_tint(position, Some(rgb))
                    .unwrap();
                let halves = terra_core::construction::slab_halves(primary, merged).unwrap();
                let bounds = terra_core::construction::slab_bounds(halves[selected].1);
                let split = (0..3)
                    .find(|&axis| bounds.maximum[axis] - bounds.minimum[axis] == 0.5)
                    .unwrap();
                let center = Vec3::from_array(std::array::from_fn(|axis| {
                    position[axis] as f32
                        + ((bounds.minimum[axis] + bounds.maximum[axis]) * 0.5) as f32
                }));
                let mut eye = center;
                eye[if split == 2 { 0 } else { 2 }] -= 3.0;
                look(&mut app, &mut peer, eye, center);
                equip(&mut app, &peer, ItemKind::Tool(ToolKind::WoodPickaxe), 1);
                let command = block_command(&app, &peer, position, primary, 0);
                assert_eq!(
                    apply(&mut app, &peer, command, true),
                    EditOutcome::Applied,
                    "primary={primary} facing={facing} selected={selected}"
                );
                let journal = app.world().resource::<EditJournal>();
                let remaining = halves[1 - selected];
                assert_eq!(journal.block(position), Some(remaining.0));
                assert_eq!(
                    app.world().resource::<ChunkStore>().block_at(position),
                    Some(remaining.0)
                );
                assert_eq!(journal.object_state(position).unwrap().facing, remaining.1);
                assert_eq!(
                    journal.tint_at(position),
                    if remaining.0 == 105 { Some(rgb) } else { None }
                );
                let owner = terra_voxel::global_to_section(position).0;
                let delta = journal.section_delta(owner);
                let mut replica = EditJournal::default();
                replica.import_delta(delta.clone()).unwrap();
                assert_eq!(replica.section_delta(owner), delta);
                let drops = &journal.gameplay().drops;
                assert_eq!(drops.len(), 1);
                assert_eq!(
                    drops[0].stack,
                    ItemStack {
                        item: if halves[selected].0 == 105 {
                            wool
                        } else {
                            ItemKind::Block(85)
                        },
                        count: 1
                    }
                );
                assert_eq!(
                    drops[0].position,
                    std::array::from_fn(|axis| position[axis] as f64
                        + (bounds.minimum[axis] + bounds.maximum[axis]) * 0.5)
                );
                assert_eq!(apply(&mut app, &peer, command, true), EditOutcome::Conflict);
                assert_eq!(
                    app.world().resource::<EditJournal>().gameplay().drops.len(),
                    1
                );
            }
        }
    }
}
