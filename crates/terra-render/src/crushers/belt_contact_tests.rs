use super::*;
use terra_voxel::{BlockSection, global_to_section};
use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

fn place(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    cell: [i64; 3],
    material: u16,
    state: BlockObjectState,
) {
    journal.record(cell, material).unwrap();
    journal.set_object_state(cell, Some(state)).unwrap();
    let (owner, local) = global_to_section(cell);
    chunks
        .loaded
        .entry(owner)
        .or_insert_with(|| LoadedSection {
            data: BlockSection::empty(owner).unwrap(),
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

fn fixture(facing: u8, perpendicular: bool) -> (ChunkStore, EditJournal, Span, [i64; 3]) {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let start = [-18, 15, -18];
    let step = terra_core::kinetic::forward(facing);
    let first = std::array::from_fn(|axis| start[axis] + step[axis]);
    let end = std::array::from_fn(|axis| start[axis] + 2 * step[axis]);
    for (cell, material, peer) in [(start, 144, first), (first, 148, end), (end, 144, start)] {
        place(
            &mut chunks,
            &mut journal,
            cell,
            material,
            BlockObjectState {
                facing,
                peer: Some(peer),
                ..Default::default()
            },
        );
    }
    let span = terra_world::conveyors::span_at(&journal, first).unwrap();
    assert!(span.linked && span.loaded(&chunks, &journal));
    let target = std::array::from_fn(|axis| end[axis] + step[axis]);
    place(
        &mut chunks,
        &mut journal,
        target,
        148,
        BlockObjectState {
            facing: (facing + u8::from(perpendicular)) % 4,
            ..Default::default()
        },
    );
    let port = [target[0], target[1] + 1, target[2]];
    place(
        &mut chunks,
        &mut journal,
        port,
        158,
        BlockObjectState::default(),
    );
    (chunks, journal, span, port)
}

#[test]
fn only_real_loaded_cardinal_merger_contacts_extend_the_existing_ribbon() {
    for facing in 0..4 {
        for perpendicular in [false, true] {
            let (mut chunks, mut journal, span, port) = fixture(facing, perpendicular);
            let expected = if perpendicular { 8 } else { 12 };
            assert_eq!(contact_mask(&chunks, &journal, span), expected);
            assert_eq!(
                contact_mask(
                    &chunks,
                    &journal,
                    Span {
                        linked: false,
                        ..span
                    }
                ),
                0
            );
            assert_eq!(
                contact_mask(
                    &chunks,
                    &journal,
                    Span {
                        step: [1, 1, 0],
                        ..span
                    }
                ),
                0
            );
            let owner = global_to_section(port).0;
            let removed = chunks.loaded.remove(&owner).unwrap();
            assert_eq!(
                contact_mask(&chunks, &journal, span),
                0,
                "unloaded journal metadata never creates a bridge"
            );
            chunks.loaded.insert(owner, removed);
            journal.record(port, 0).unwrap();
            assert_eq!(
                contact_mask(&chunks, &journal, span),
                0,
                "removing the junction removes its contact tongue"
            );
        }
    }
}

#[test]
fn transfer_tongues_close_the_measured_inline_and_side_gaps_without_changing_the_belt_profile() {
    let base = geometry(3, 1.0);
    for contacts in 1..=15 {
        let joined = geometry_with_contacts(3, 1.0, contacts);
        assert_eq!(&joined.positions[..base.positions.len()], base.positions);
        assert_eq!(&joined.normals[..base.normals.len()], base.normals);
        assert_eq!(&joined.tangents[..base.tangents.len()], base.tangents);
        assert_eq!(&joined.uvs[..base.uvs.len()], base.uvs);
        assert_eq!(&joined.colors[..base.colors.len()], base.colors);
        assert_eq!(&joined.indices[..base.indices.len()], base.indices);
        let count = usize::from(contacts & 3 != 0) + usize::from(contacts >> 2 != 0);
        assert_eq!(joined.positions.len(), base.positions.len() + count * 24);
        assert_eq!(joined.indices.len(), base.indices.len() + count * 36);
        for end in 0..2 {
            let code = (contacts >> (end * 2)) & 3;
            if code == 0 {
                continue;
            }
            let reach = match code {
                1 => 0.5,
                2 => 0.68,
                _ => 0.625,
            };
            let z = if end == 0 { -reach } else { 4.0 + reach };
            assert!(
                joined
                    .positions
                    .iter()
                    .any(|p| (p[2] - z).abs() < 1e-6 && p[1] == PROFILE_RADIUS)
            );
        }
        for triangle in joined.indices[base.indices.len()..].chunks_exact(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                .map(|i| Vec3::from_array(joined.positions[i as usize]));
            let normal = Vec3::from_array(joined.normals[triangle[0] as usize]);
            assert!((b - a).cross(c - a).dot(normal) > 0.0);
        }
        for (index, normal) in joined.normals.iter().enumerate().skip(base.positions.len()) {
            if normal[1] > 0.99 {
                assert_eq!(joined.positions[index][1], PROFILE_RADIUS);
                assert!(
                    (joined.uvs[index][0] - (HALF_WIDTH - joined.positions[index][0])).abs() < 1e-6
                );
                assert_eq!(joined.uvs[index][1], joined.positions[index][2]);
            }
        }
    }
}

#[test]
fn distributor_uses_the_same_exact_bounded_four_way_belt_contacts_as_the_merger() {
    for facing in 0..4 {
        for perpendicular in [false, true] {
            let (mut chunks, mut journal, span, port) = fixture(facing, perpendicular);
            let merger = contact_mask(&chunks, &journal, span);
            journal.record(port, 160).unwrap();
            journal
                .set_object_state(port, Some(BlockObjectState::default()))
                .unwrap();
            let (owner, local) = global_to_section(port);
            chunks
                .loaded
                .get_mut(&owner)
                .unwrap()
                .data
                .set_block(local, 160)
                .unwrap();
            assert_eq!(contact_mask(&chunks, &journal, span), merger);
        }
    }
}
