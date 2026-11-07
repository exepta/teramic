use super::*;
use crate::streaming::LoadedSection;
use std::collections::BTreeMap;
use terra_core::{ItemStack, block_properties::BlockPropertiesCatalog, kinetic::KineticState};
use terra_voxel::{BlockSection, SectionPos, global_to_section};

mod actor_tests;
mod infusion_tests;
mod owner_tests;
mod queue_tests;

fn fixture() -> (ChunkStore, EditJournal) {
    let mut chunks = ChunkStore::default();
    for x in [-1, 0] {
        let position = SectionPos { x, y: 0, z: 0 };
        chunks.loaded.insert(
            position,
            LoadedSection {
                data: BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
    }
    (chunks, EditJournal::default())
}
fn shaft_at(chunks: &mut ChunkStore, journal: &mut EditJournal, p: Cell, facing: u8) {
    let (owner, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, 144)
        .unwrap();
    journal.record(p, 144).unwrap();
    journal
        .set_object_state(
            p,
            Some(BlockObjectState {
                facing,
                wood: WoodSpecies::Birch,
                ..Default::default()
            }),
        )
        .unwrap();
}
fn connect(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    start: Cell,
    end: Cell,
    facing: u8,
) -> Span {
    shaft_at(chunks, journal, start, facing);
    shaft_at(chunks, journal, end, facing);
    let plan = layout(
        chunks,
        journal,
        start,
        end,
        ItemKind::for_wood(148, WoodSpecies::Cherry),
    )
    .unwrap();
    crate::placement::environment(&plan, chunks, journal, false).unwrap();
    let span = proposed_span(&plan).unwrap();
    assert!(commit(chunks, journal, &plan).unwrap());
    span
}
fn power(span: Span, journal: &mut EditJournal) {
    journal.replace_kinetic_states(
        span.cells()
            .chain([span.start, span.end])
            .map(|p| {
                (
                    p,
                    KineticState {
                        rpm: 30.0,
                        overloaded: false,
                        ..Default::default()
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
    );
}

#[test]
fn spans_preserve_endpoints_share_ownership_and_roundtrip_metadata_in_all_supported_slopes() {
    for end in [[4, 4, 10], [4, 10, 10], [4, 10, 4]] {
        let (mut chunks, mut journal) = fixture();
        let span = connect(&mut chunks, &mut journal, [4, 4, 4], end, 0);
        assert_eq!(span.length, 5);
        for p in span.cells() {
            assert_eq!(span_at(&journal, p), Some(span));
            assert_eq!(journal.object_state(p).unwrap().wood, WoodSpecies::Cherry);
        }
        for p in [span.start, span.end] {
            assert_eq!(chunks.block_at(p), Some(144));
            assert_eq!(journal.object_state(p).unwrap().wood, WoodSpecies::Birch);
            assert!(segment_at(&journal, p).is_some());
            assert!(crate::wrench::propose(&chunks, &journal, p, false).is_err());
        }
        assert!(crate::wrench::propose(&chunks, &journal, span.first, false).is_err());
        let mut reloaded = EditJournal::default();
        reloaded
            .import_delta(journal.section_delta(SectionPos::default()))
            .unwrap();
        assert_eq!(span_at(&reloaded, span.first), Some(span));
        assert!(span.loaded(&chunks, &reloaded));
        assert_eq!(span.normal()[1] > 0.1, end[2] != 4);
        if end[2] == 4 {
            assert!(
                segment_at(&journal, span.first)
                    .unwrap()
                    .drop_center()
                    .is_none()
            );
        }
    }
}

#[test]
fn negative_section_crossing_stays_persistent_but_unloading_an_end_disables_the_whole_span() {
    let (mut chunks, mut journal) = fixture();
    let span = connect(&mut chunks, &mut journal, [-3, 4, 4], [3, 4, 4], 3);
    assert!(span.loaded(&chunks, &journal));
    let owner = global_to_section(span.end).0;
    let section = chunks.loaded.remove(&owner).unwrap();
    assert!(!span.loaded(&chunks, &journal));
    assert_eq!(span_at(&journal, span.first), Some(span));
    chunks.loaded.insert(owner, section);
    assert!(span.loaded(&chunks, &journal));
}

#[test]
fn bad_spans_are_rejected_without_inventory_or_partial_world_mutation() {
    let (mut chunks, mut journal) = fixture();
    let start = [4, 4, 4];
    let end = [4, 4, 10];
    shaft_at(&mut chunks, &mut journal, start, 0);
    shaft_at(&mut chunks, &mut journal, end, 1);
    let before = journal.section_delta(SectionPos::default());
    assert!(layout(&chunks, &journal, start, end, ItemKind::Block(148)).is_err());
    shaft_at(&mut chunks, &mut journal, end, 0);
    chunks
        .loaded
        .get_mut(&SectionPos::default())
        .unwrap()
        .data
        .set_block([4, 4, 7], 3)
        .unwrap();
    assert!(layout(&chunks, &journal, start, end, ItemKind::Block(148)).is_err());
    assert_eq!(
        journal.section_delta(SectionPos::default()).blocks,
        before.blocks
    );
    assert!(layout(&chunks, &journal, start, [4, 4, 38], ItemKind::Block(148)).is_err());
    assert!(layout(&chunks, &journal, start, end, ItemKind::Block(3)).is_err());
    assert_eq!(chunks.block_at([4, 4, 5]), Some(0));
}

#[test]
fn horizontal_and_uphill_transport_use_real_drops_and_preserve_stack_identity() {
    for end in [[4, 4, 10], [4, 10, 10]] {
        let (mut chunks, mut journal) = fixture();
        let span = connect(&mut chunks, &mut journal, [4, 4, 4], end, 0);
        power(span, &mut journal);
        let segment = segment_at(&journal, span.first).unwrap();
        let start = segment.drop_center().unwrap();
        let stack = ItemStack {
            item: ItemKind::Block(3),
            count: 17,
        };
        assert!(
            journal
                .gameplay_mut()
                .drop_stack(start, stack, [0.0; 3], 0.0)
        );
        let id = journal.gameplay().drops[0].id;
        let mut runtime = Transport::default();
        for _ in 0..20 {
            transport(
                &mut runtime,
                &chunks,
                &mut journal,
                &BlockPropertiesCatalog::default(),
                0.1,
            );
        }
        let drop = &journal.gameplay().drops[0];
        assert_eq!((drop.id, drop.stack), (id, stack));
        assert!(
            drop.position[2] > start[2] + 0.5,
            "{end:?}: {:?}",
            drop.position
        );
        if end[1] > 4 {
            assert!(drop.position[1] > start[1] + 0.5);
        }
        let stopped = drop.position;
        journal.replace_kinetic_states(Default::default());
        transport(
            &mut runtime,
            &chunks,
            &mut journal,
            &BlockPropertiesCatalog::default(),
            0.1,
        );
        assert_eq!(journal.gameplay().drops[0].position, stopped);
        assert_eq!(journal.gameplay().drops.len(), 1);
    }
}

#[test]
fn one_controller_handles_a_span_and_duplicate_attachment_is_rejected() {
    let (mut chunks, mut journal) = fixture();
    let span = connect(&mut chunks, &mut journal, [4, 4, 4], [4, 4, 10], 0);
    assert!(
        layout(
            &chunks,
            &journal,
            span.start,
            span.end,
            ItemKind::Block(148)
        )
        .is_err()
    );
    let plan = removal_layout(&chunks, &journal, span, None).unwrap();
    assert!(commit(&mut chunks, &mut journal, &plan).unwrap());
    assert!(span.cells().all(|p| chunks.block_at(p) == Some(0)));
    assert_eq!(chunks.block_at(span.start), Some(144));
    assert_eq!(chunks.block_at(span.end), Some(144));
    assert_eq!(journal.object_state(span.start).unwrap().peer, None);
    assert_eq!(journal.object_state(span.end).unwrap().peer, None);
    assert!(connected_at(&journal, span.start).is_empty());
}

#[test]
fn dye_updates_the_whole_span_and_survives_reload_without_changing_geometry() {
    let (mut chunks, mut journal) = fixture();
    let span = connect(&mut chunks, &mut journal, [4, 4, 4], [4, 4, 10], 0);
    let before = boxes(span);
    let original = journal.section_delta(SectionPos::default());
    assert!(
        dye_layout(
            &chunks,
            &journal,
            span.first,
            terra_core::beds::DyeColor::Rgb([12, 96, 211])
        )
        .is_ok()
    );
    assert_eq!(journal.section_delta(SectionPos::default()), original);
    for color in terra_core::beds::DyeColor::ALL {
        let plan = dye_layout(&chunks, &journal, [4, 4, 7], color).unwrap();
        assert_eq!(plan.edits.len(), usize::from(span.length) + 2);
        for endpoint in [span.start, span.end] {
            assert_eq!(
                plan.states.iter().find(|(p, _)| *p == endpoint).unwrap().1,
                journal.object_state(endpoint)
            );
        }
        assert!(commit(&mut chunks, &mut journal, &plan).unwrap());
        assert_eq!(span_at(&journal, span.first), Some(span));
        assert_eq!(boxes(span_at(&journal, span.first).unwrap()), before);
        assert!(
            span.cells().all(
                |p| kinetic::belt_color(journal.object_state(p).unwrap().facing) == Some(color)
            )
        );
        assert!(dye_layout(&chunks, &journal, span.start, color).is_err());
        assert_eq!(journal.object_state(span.start).unwrap().facing, 0);
        let mut restored = EditJournal::default();
        restored
            .import_delta(journal.section_delta(SectionPos::default()))
            .unwrap();
        assert_eq!(span_at(&restored, span.first), Some(span));
        assert_eq!(
            kinetic::belt_color(restored.object_state(span.first).unwrap().facing),
            Some(color)
        );
    }
    let rgb = [12, 96, 211];
    let plan = dye_layout(
        &chunks,
        &journal,
        span.first,
        terra_core::beds::DyeColor::Rgb(rgb),
    )
    .unwrap();
    assert!(commit(&mut chunks, &mut journal, &plan).unwrap());
    for cell in span.cells() {
        journal.set_tint(cell, Some(rgb)).unwrap();
    }
    let mut restored = EditJournal::default();
    restored
        .import_delta(journal.section_delta(SectionPos::default()))
        .unwrap();
    assert_eq!(
        crate::conveyors::item(span, &restored),
        ItemKind::TintedBlock {
            material: 148,
            wood: span.wood,
            color: rgb
        }
    );
    assert_eq!(boxes(span_at(&restored, span.first).unwrap()), before);
    let section = chunks.loaded.remove(&SectionPos::default()).unwrap();
    assert!(
        dye_layout(
            &chunks,
            &journal,
            span.first,
            terra_core::beds::DyeColor::Red
        )
        .is_err()
    );
    chunks.loaded.insert(SectionPos::default(), section);
    let mut invalid = journal.object_state(span.first).unwrap();
    invalid.facing = kinetic::BELT_FACING_COUNT;
    assert!(!invalid.valid_for(span.first, 148));
}

#[test]
fn carried_drops_center_and_stay_upright_on_horizontal_and_sloped_belts() {
    for end in [[4, 4, 10], [4, 10, 10]] {
        let (mut chunks, mut journal) = fixture();
        let span = connect(&mut chunks, &mut journal, [4, 4, 4], end, 0);
        power(span, &mut journal);
        let segment = segment_at(&journal, span.first).unwrap();
        let mut position = segment.drop_center().unwrap();
        position[0] += 0.2;
        assert!(journal.gameplay_mut().drop_stack(
            position,
            ItemStack {
                item: ItemKind::Block(3),
                count: 8
            },
            [0.0; 3],
            0.0
        ));
        assert!(carrying_segment(&journal.gameplay().drops[0], &journal).is_some());
        let mut runtime = Transport::default();
        for _ in 0..10 {
            transport(
                &mut runtime,
                &chunks,
                &mut journal,
                &BlockPropertiesCatalog::default(),
                0.1,
            );
            let drop = &journal.gameplay().drops[0];
            assert!((drop.position[0] - 4.5).abs() < 0.00001);
            assert_eq!(drop.rotation, [0.0, 0.0, 0.0, 1.0]);
            assert_eq!(drop.angular_velocity, [0.0; 3]);
            assert!(
                (f64::from(drop.velocity[1]).hypot(f64::from(drop.velocity[2])) - 1.0).abs()
                    < 0.0001
            );
            assert!(
                carrying_segment(drop, &journal).is_some(),
                "{end:?}: {:?}",
                drop.position
            );
        }
        let drop = &journal.gameplay().drops[0];
        let eye = [drop.position[0], drop.position[1], drop.position[2] + 2.0];
        assert_eq!(
            carried_drop_hit(
                &journal.gameplay().drops,
                &journal,
                eye,
                [0.0, 0.0, -1.0],
                6.0
            )
            .unwrap()
            .0,
            drop.id
        );
        assert!(
            carried_drop_hit(
                &journal.gameplay().drops,
                &journal,
                eye,
                [1.0, 0.0, 0.0],
                6.0
            )
            .is_none()
        );
    }
}

#[test]
fn smooth_slope_contact_stays_tight_between_collision_slices_and_walls_still_stop_transport() {
    let (mut chunks, mut journal) = fixture();
    let span = connect(&mut chunks, &mut journal, [4, 4, 4], [4, 10, 10], 0);
    power(span, &mut journal);
    journal.replace_kinetic_states(
        span.cells()
            .chain([span.start, span.end])
            .map(|p| {
                (
                    p,
                    KineticState {
                        rpm: f32::from(kinetic::MAX_RPM),
                        overloaded: false,
                        ..Default::default()
                    },
                )
            })
            .collect(),
    );
    let segment = segment_at(&journal, span.first).unwrap();
    let center = segment.drop_center().unwrap();
    journal.gameplay_mut().drop_stack(
        center,
        ItemStack {
            item: ItemKind::Block(3),
            count: 1,
        },
        [0.0; 3],
        0.0,
    );
    let mut drop = journal.gameplay().drops[0].clone();
    for offset in [0.0, 0.07, 0.125, 0.19, 0.36, 0.49, 0.51] {
        drop.position =
            std::array::from_fn(|i| center[i] + f64::from(span.direction()[i]) * offset);
        assert!(
            carrying_segment(&drop, &journal).is_some(),
            "{offset}: {:?}",
            drop.position
        );
    }
    drop.position[1] += 0.2;
    assert!(carrying_segment(&drop, &journal).is_none());
    drop.position[1] -= 0.7;
    assert!(carrying_segment(&drop, &journal).is_none());
    chunks
        .loaded
        .get_mut(&SectionPos::default())
        .unwrap()
        .data
        .set_block([4, 7, 6], 3)
        .unwrap();
    assert!(span.loaded(&chunks, &journal));
    let mut runtime = Transport::default();
    for _ in 0..20 {
        transport(
            &mut runtime,
            &chunks,
            &mut journal,
            &BlockPropertiesCatalog::default(),
            0.15,
        );
    }
    let drop = &journal.gameplay().drops[0];
    assert!(drop.position[2] > center[2] + 0.1);
    let wall_gap = 7.0 - (drop.position[1] + 0.12);
    assert!((0.0..=0.05).contains(&wall_gap), "{:?}", drop.position);
    assert_eq!(drop.velocity, [0.0; 3]);
    assert_eq!(drop.stack.count, 1);
}

#[test]
fn high_rpm_transport_preserves_full_speed_in_both_directions_with_bounded_sweeps() {
    for rpm in [
        120.0,
        -120.0,
        f32::from(kinetic::MAX_RPM),
        -f32::from(kinetic::MAX_RPM),
    ] {
        for elapsed in [0.1, 0.15] {
            let (mut chunks, mut journal) = fixture();
            let span = connect(&mut chunks, &mut journal, [4, 4, 4], [4, 4, 14], 0);
            journal.replace_kinetic_states(
                span.cells()
                    .chain([span.start, span.end])
                    .map(|p| {
                        (
                            p,
                            KineticState {
                                rpm,
                                overloaded: false,
                                ..Default::default()
                            },
                        )
                    })
                    .collect(),
            );
            let position = segment_at(&journal, [4, 4, 9])
                .unwrap()
                .drop_center()
                .unwrap();
            let stack = ItemStack {
                item: ItemKind::Block(3),
                count: 17,
            };
            assert!(
                journal
                    .gameplay_mut()
                    .drop_stack(position, stack, [0.0; 3], 0.0)
            );
            let id = journal.gameplay().drops[0].id;
            transport(
                &mut Transport::default(),
                &chunks,
                &mut journal,
                &BlockPropertiesCatalog::default(),
                elapsed,
            );
            let drop = &journal.gameplay().drops[0];
            let expected = f64::from(belt_speed(rpm) * elapsed);
            assert!(
                (drop.position[2] - position[2] - expected).abs() < 0.00001,
                "rpm={rpm} dt={elapsed}: {:?}",
                drop.position
            );
            assert!((drop.velocity[2] - belt_speed(rpm)).abs() < 0.0001);
            assert_eq!(drop.position[0], position[0]);
            assert_eq!(drop.position[1], position[1]);
            assert_eq!((drop.id, drop.stack), (id, stack));
            assert!(carrying_segment(drop, &journal).is_some());
        }
    }
}

#[test]
fn three_shaft_spans_preserve_guides_colors_and_ownership_in_every_perpendicular_plane() {
    for (start, end, guide, facing) in [
        ([-3, 4, 4], [3, 10, 4], [0, 7, 4], 1),
        ([4, 4, 4], [4, 10, 10], [4, 7, 7], 0),
        ([-3, 4, 4], [3, 4, 10], [0, 4, 7], 4),
        ([4, 4, 4], [4, 4, 10], [4, 4, 5], 0),
    ] {
        let (mut chunks, mut journal) = fixture();
        for p in [start, end, guide] {
            shaft_at(&mut chunks, &mut journal, p, facing);
        }
        let mut original = journal.object_state(guide).unwrap();
        original.wood = WoodSpecies::Spruce;
        journal.set_object_state(guide, Some(original)).unwrap();
        let plan = layout(
            &chunks,
            &journal,
            start,
            end,
            ItemKind::for_wood(148, WoodSpecies::Cherry),
        )
        .unwrap();
        let span = proposed_span(&plan).unwrap();
        crate::placement::environment(&plan, &chunks, &journal, false).unwrap();
        assert!(commit(&mut chunks, &mut journal, &plan).unwrap());
        assert_eq!(span_at(&journal, start), Some(span));
        assert_eq!(span_at(&journal, end), Some(span));
        assert_eq!(span_at(&journal, guide), Some(span));
        assert_eq!(chunks.block_at(guide), Some(144));
        assert_eq!(
            journal.object_state(guide).unwrap().wood,
            WoodSpecies::Spruce
        );
        assert_eq!(journal.object_state(guide).unwrap().facing, facing);
        assert!(
            journal
                .extended_kinetics_in_section(global_to_section(guide).0)
                .any(|p| p == guide)
        );
        let width = kinetic::axis(span.facing).map(|v| v as f32);
        assert!(
            span.normal()
                .into_iter()
                .zip(width)
                .map(|(a, b)| a * b)
                .sum::<f32>()
                .abs()
                < 0.00001
        );
        assert_eq!(
            segment_at(&journal, guide).unwrap().drop_center().is_some(),
            facing != 4
        );
        let nodes: Vec<_> = span
            .cells()
            .chain([start, end])
            .map(|p| kinetic::Node {
                position: p,
                material: journal.block(p).unwrap(),
                facing: journal.object_state(p).unwrap().facing,
                secondary: journal.block(p) == Some(148) && p != span.controller,
                source_rpm: if p == start { 30.0 } else { 0.0 },
                source_capacity: 500,
                belt_load: false,
                links: span.links(p),
            })
            .collect();
        let nodes: BTreeMap<_, _> = nodes
            .into_iter()
            .map(|node| (node.position, node))
            .collect();
        assert!(
            kinetic::solve_component(start, &mut 1024, |position| nodes.get(&position).copied())
                .iter()
                .all(|(_, state)| state.running())
        );
        let dye = dye_layout(&chunks, &journal, guide, terra_core::beds::DyeColor::Green).unwrap();
        assert!(
            dye.edits
                .iter()
                .all(|edit| matches!(edit.material, 144..=146 | 148))
        );
        assert!(
            dye.states
                .iter()
                .filter(|(p, _)| journal.block(*p) == Some(144))
                .all(|(p, state)| *state == journal.object_state(*p)),
            "dyeing the belt cannot change any guide shaft's pose or wood"
        );
        commit(&mut chunks, &mut journal, &dye).unwrap();
        assert_eq!(journal.object_state(guide).unwrap().wood, original.wood);
        assert_eq!(journal.object_state(guide).unwrap().facing, original.facing);
        assert_eq!(span_at(&journal, span.controller), Some(span));
        let mut reloaded = EditJournal::default();
        for section in journal.changed_sections() {
            reloaded
                .import_delta(journal.section_delta(section))
                .unwrap();
        }
        assert_eq!(span_at(&reloaded, guide), Some(span));
        assert!(
            reloaded
                .extended_kinetics_in_section(global_to_section(guide).0)
                .any(|p| p == guide)
        );
        assert_eq!(
            kinetic::belt_color(reloaded.object_state(span.controller).unwrap().facing),
            Some(terra_core::beds::DyeColor::Green)
        );
        let section = global_to_section(span.controller).0;
        let loaded = chunks.loaded.remove(&section).unwrap();
        assert!(!span.loaded(&chunks, &reloaded));
        assert_eq!(span_at(&reloaded, guide), Some(span));
        assert!(removal_layout(&chunks, &reloaded, span, None).is_err());
        chunks.loaded.insert(section, loaded);
        let removal = removal_layout(&chunks, &journal, span, None).unwrap();
        assert!(commit(&mut chunks, &mut journal, &removal).unwrap());
        assert!(commit(&mut chunks, &mut journal, &removal).is_err());
        for p in [start, end, guide] {
            assert_eq!(chunks.block_at(p), Some(144));
            assert_eq!(journal.object_state(p).unwrap().peer, None);
        }
        assert_eq!(journal.object_state(guide), Some(original));
        assert!(
            !journal
                .extended_kinetics_in_section(global_to_section(guide).0)
                .any(|p| p == guide)
        );
    }
}

#[test]
fn historical_belt_metadata_remains_valid_without_endpoint_peer_records() {
    let (mut chunks, mut journal) = fixture();
    let start = [4, 4, 4];
    let end = [4, 4, 8];
    let first = [4, 4, 5];
    for p in [start, end] {
        shaft_at(&mut chunks, &mut journal, p, 0);
    }
    for z in 5..8 {
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([4, 4, z], 148)
            .unwrap();
        let p = [4, 4, z as i64];
        journal.record(p, 148).unwrap();
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    peer: Some(if p == first { end } else { first }),
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let span = span_at(&journal, first).unwrap();
    assert_eq!(
        (span.controller, span.first, span.start, span.end),
        (first, first, start, end)
    );
    assert!(span.loaded(&chunks, &journal));
    assert_eq!(connected_at(&journal, start), vec![span]);
    let removal = removal_layout(&chunks, &journal, span, Some(start)).unwrap();
    commit(&mut chunks, &mut journal, &removal).unwrap();
    assert_eq!(chunks.block_at(start), Some(0));
    assert_eq!(chunks.block_at(end), Some(144));
    assert_eq!(journal.object_state(end).unwrap().peer, None);
}

#[test]
fn legacy_single_cell_belts_keep_index_zero_for_carrying_collection_and_transport() {
    for facing in 0..4 {
        for dyed in [false, true] {
            let (mut chunks, mut journal) = fixture();
            let owner = [7, 4, 7];
            chunks
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block([7, 4, 7], 148)
                .unwrap();
            journal.record(owner, 148).unwrap();
            let raw = if dyed {
                kinetic::dyed_belt_facing(facing, terra_core::beds::DyeColor::LightBlue)
            } else {
                facing
            };
            journal
                .set_object_state(
                    owner,
                    Some(BlockObjectState {
                        facing: raw,
                        ..Default::default()
                    }),
                )
                .unwrap();
            let segment = segment_at(&journal, owner).unwrap();
            assert_eq!(segment.index, 0);
            assert_eq!(segment.position(), owner);
            assert!(!segment.span.linked);
            let position = segment.drop_center().unwrap();
            let stack = ItemStack {
                item: ItemKind::Block(3),
                count: 7,
            };
            journal
                .gameplay_mut()
                .drop_stack(position, stack, [0.0; 3], 0.0);
            let id = journal.gameplay().drops[0].id;
            assert_eq!(
                carrying_segment(&journal.gameplay().drops[0], &journal),
                Some(segment)
            );
            assert_eq!(
                carried_drop_hit(
                    &journal.gameplay().drops,
                    &journal,
                    [position[0], position[1], position[2] + 2.0],
                    [0.0, 0.0, -1.0],
                    6.0
                )
                .unwrap()
                .0,
                id
            );
            journal.replace_kinetic_states(BTreeMap::from([(
                owner,
                KineticState {
                    rpm: 30.0,
                    overloaded: false,
                    ..Default::default()
                },
            )]));
            transport(
                &mut Transport::default(),
                &chunks,
                &mut journal,
                &BlockPropertiesCatalog::default(),
                0.1,
            );
            let drop = &journal.gameplay().drops[0];
            let direction = kinetic::forward(facing);
            for axis in 0..3 {
                assert!(
                    (drop.position[axis] - position[axis] - direction[axis] as f64 * 0.1).abs()
                        < 0.00001
                );
            }
            assert_eq!((drop.id, drop.stack), (id, stack));
            assert!(carrying_segment(drop, &journal).is_some());
        }
    }
}
