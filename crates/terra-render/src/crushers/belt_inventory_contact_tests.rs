use super::*;
use terra_voxel::{BlockSection, global_to_section};
use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

fn place(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    position: [i64; 3],
    material: u16,
    state: BlockObjectState,
) {
    journal.record(position, material).unwrap();
    journal.set_object_state(position, Some(state)).unwrap();
    let (section, local) = global_to_section(position);
    chunks
        .loaded
        .entry(section)
        .or_insert_with(|| LoadedSection {
            data: BlockSection::empty(section).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        })
        .data
        .set_block(local, material)
        .unwrap();
}

fn fixture(
    facing: u8,
    end: usize,
    port_material: u16,
) -> (ChunkStore, EditJournal, Span, [i64; 3], [i64; 3]) {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let start = [-17, 15, -17];
    let step = terra_core::kinetic::forward(facing);
    let center = std::array::from_fn(|axis| start[axis] + step[axis]);
    let last = std::array::from_fn(|axis| start[axis] + step[axis] * 2);
    for (position, material, peer) in [
        (start, 144, center),
        (center, 148, last),
        (last, 144, start),
    ] {
        place(
            &mut chunks,
            &mut journal,
            position,
            material,
            BlockObjectState {
                facing,
                peer: Some(peer),
                ..Default::default()
            },
        );
    }
    let span = terra_world::conveyors::span_at(&journal, center).unwrap();
    assert!(span.loaded(&chunks, &journal));
    let endpoint = if end == 0 { start } else { last };
    let port = [endpoint[0], endpoint[1] + 1, endpoint[2]];
    let sign = if end == 0 { -1 } else { 1 };
    let inventory = std::array::from_fn(|axis| port[axis] + step[axis] * sign);
    place(
        &mut chunks,
        &mut journal,
        inventory,
        29,
        BlockObjectState::default(),
    );
    place(
        &mut chunks,
        &mut journal,
        port,
        port_material,
        BlockObjectState {
            facing,
            peer: Some(inventory),
            ..Default::default()
        },
    );
    (chunks, journal, span, port, inventory)
}

#[test]
fn mounted_inventory_ports_fill_only_the_matching_outer_half_cell_for_all_facings_and_ends() {
    for facing in 0..4 {
        for end in 0..2 {
            for port_material in [152, 153, 157, 159] {
                let (chunks, journal, span, _, inventory) = fixture(facing, end, port_material);
                let joined = contacts(&chunks, &journal, span, Contacts::default());
                assert_eq!(joined.mask, 1 << (end * 2));
                assert!(joined.inventories[end].is_some());
                let joined_geometry =
                    geometry_with_contacts(span.length, step_length(span), joined.mask);
                let root = pose(span);
                let sign = if end == 0 { -1.0 } else { 1.0 };
                let endpoint = if end == 0 { span.start } else { span.end };
                let boundary = Vec3::from_array(endpoint.map(|value| value as f32 + 0.5))
                    + Vec3::from_array(span.direction()) * sign * 0.5
                    + Vec3::Y * PROFILE_RADIUS;
                assert!(joined_geometry.positions.iter().any(|position| {
                    let point = root.transform_point(Vec3::from_array(*position));
                    (point.y - boundary.y).abs() < 1e-5
                        && ((point - boundary).dot(Vec3::from_array(span.direction()))).abs() < 1e-5
                }));
                assert!(boundary.y < inventory[1] as f32);
                let base = geometry(span.length, step_length(span));
                assert_eq!(joined_geometry.positions.len(), base.positions.len() + 24);
                assert_eq!(joined_geometry.indices.len(), base.indices.len() + 36);
            }
        }
    }
}

#[test]
fn only_previously_loaded_unchanged_inventory_contacts_survive_neighbor_unloads() {
    for facing in 0..4 {
        for material in [152, 153, 157, 159] {
            let (mut chunks, mut journal, span, port, inventory) = fixture(facing, 1, material);
            let confirmed = contacts(&chunks, &journal, span, Contacts::default());
            assert_eq!(confirmed.mask, 4);
            let owner = global_to_section(inventory).0;
            let saved = chunks.loaded.remove(&owner).unwrap();
            assert!(span.loaded(&chunks, &journal));
            assert_eq!(
                contacts(&chunks, &journal, span, Contacts::default()).mask,
                0
            );
            assert_eq!(contacts(&chunks, &journal, span, confirmed), confirmed);
            chunks.loaded.insert(owner, saved);
            assert_eq!(contacts(&chunks, &journal, span, confirmed), confirmed);

            let (_, local) = global_to_section(inventory);
            chunks
                .loaded
                .get_mut(&owner)
                .unwrap()
                .data
                .set_block(local, 3)
                .unwrap();
            assert_eq!(contacts(&chunks, &journal, span, confirmed).mask, 0);
            chunks
                .loaded
                .get_mut(&owner)
                .unwrap()
                .data
                .set_block(local, 29)
                .unwrap();
            journal.record(port, 0).unwrap();
            assert_eq!(contacts(&chunks, &journal, span, confirmed).mask, 0);
        }
    }
}

#[test]
fn wrong_inventory_side_unbound_port_and_changed_unloaded_peers_never_create_a_tongue() {
    for facing in 0..4 {
        let (mut chunks, mut journal, span, port, inventory) = fixture(facing, 1, 152);
        let confirmed = contacts(&chunks, &journal, span, Contacts::default());
        let original = journal.object_state(port).unwrap();
        journal
            .set_object_state(
                port,
                Some(BlockObjectState {
                    peer: None,
                    ..original
                }),
            )
            .unwrap();
        assert_eq!(contacts(&chunks, &journal, span, confirmed).mask, 0);
        let side = terra_core::kinetic::forward((facing + 1) % 4);
        let other = std::array::from_fn(|axis| port[axis] + side[axis]);
        place(
            &mut chunks,
            &mut journal,
            other,
            29,
            BlockObjectState::default(),
        );
        journal
            .set_object_state(
                port,
                Some(BlockObjectState {
                    peer: Some(other),
                    ..original
                }),
            )
            .unwrap();
        assert_eq!(contacts(&chunks, &journal, span, confirmed).mask, 0);
        journal.set_object_state(port, Some(original)).unwrap();
        chunks
            .loaded
            .remove(&global_to_section(inventory).0)
            .unwrap();
        journal
            .set_object_state(
                inventory,
                Some(BlockObjectState {
                    facing: 1,
                    ..Default::default()
                }),
            )
            .unwrap();
        assert_eq!(contacts(&chunks, &journal, span, confirmed).mask, 0);
    }
}
