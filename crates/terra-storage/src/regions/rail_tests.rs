use super::*;
use terra_core::rails::{self, Endpoint, Piece};
use terra_world::rails as world_rails;

fn record_piece(journal: &mut EditJournal, piece: Piece) {
    for cell in world_rails::cells(piece) {
        assert!(
            journal
                .block(cell)
                .is_none_or(|material| material == piece.material)
        );
        journal.record(cell, piece.material).unwrap();
        journal
            .set_object_state(cell, Some(world_rails::object_state(piece, cell)))
            .unwrap();
    }
}

fn temporary_world() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "teramic-rail-save-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn all_rail_modules_switch_state_and_exact_graph_survive_save_reload_at_negative_chunk_seams() {
    let root = temporary_world();
    let world = WorldId(620);
    let mut journal = EditJournal::default();
    let first = Piece::new([-17, 16, -17], rails::STRAIGHT, 0).unwrap();
    let second = Piece::new([-17, 16, -13], rails::STRAIGHT, 0).unwrap();
    record_piece(&mut journal, first);
    record_piece(&mut journal, second);
    let mut pieces = vec![first, second];
    for (index, material) in rails::MATERIALS.into_iter().enumerate() {
        let facing = (index % 4) as u8
            + if matches!(material, rails::SWITCH_LEFT | rails::SWITCH_RIGHT) {
                4
            } else {
                0
            };
        let piece = Piece::new([index as i64 * 96 - 385, 32, -129], material, facing).unwrap();
        record_piece(&mut journal, piece);
        pieces.push(piece);
    }
    let expected = world_rails::network(&journal).unwrap();
    assert_eq!(expected.len(), pieces.len());
    let start = Endpoint {
        anchor: first.anchor,
        port: 0,
    };
    let end = Endpoint {
        anchor: second.anchor,
        port: 1,
    };
    assert!(expected.route(start, end, 128).is_ok());
    let sections: BTreeSet<_> = pieces
        .iter()
        .flat_map(|piece| world_rails::cells(*piece))
        .map(|cell| terra_voxel::global_to_section(cell).0)
        .collect();
    assert!(
        sections
            .iter()
            .any(|section| section.x < 0 && section.z < 0)
    );
    save_edits(&root, world, &journal).unwrap();
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(world_rails::network(&loaded).unwrap(), expected);
    assert_eq!(loaded.rail_module_count(), pieces.len());
    for piece in &pieces {
        assert!(world_rails::complete(&loaded, *piece));
        for cell in world_rails::cells(*piece) {
            assert_eq!(world_rails::owner_at(&loaded, cell), Some(*piece));
            assert_eq!(loaded.object_state(cell), journal.object_state(cell));
        }
    }
    // A late join may receive sections in any order; no loaded ChunkStore is needed.
    let mut replica = EditJournal::default();
    for section in sections.iter().rev() {
        replica
            .import_delta(loaded.section_delta(*section))
            .unwrap();
    }
    assert_eq!(world_rails::network(&replica).unwrap(), expected);
    let switch = pieces
        .iter()
        .copied()
        .find(|piece| piece.material == rails::SWITCH_LEFT)
        .unwrap();
    assert!(switch.switched);
    let entry = Endpoint {
        anchor: switch.anchor,
        port: 0,
    };
    let main = Endpoint {
        anchor: switch.anchor,
        port: 1,
    };
    let branch = Endpoint {
        anchor: switch.anchor,
        port: 2,
    };
    assert!(expected.route(entry, main, 128).is_err());
    assert!(expected.route(entry, branch, 128).is_ok());
    let switched_back = Piece::new(switch.anchor, switch.material, switch.facing).unwrap();
    for cell in world_rails::cells(switch) {
        loaded
            .set_object_state(cell, Some(world_rails::object_state(switched_back, cell)))
            .unwrap();
    }
    save_edits(&root, world, &loaded).unwrap();
    let reloaded = load_edits(&root, world).unwrap();
    let graph = world_rails::network(&reloaded).unwrap();
    assert!(graph.route(entry, main, 128).is_ok());
    assert!(graph.route(entry, branch, 128).is_err());
    assert_eq!(graph.len(), pieces.len());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn saved_missing_secondary_is_not_a_traversable_module_and_invalid_owner_offsets_are_rejected() {
    let root = temporary_world();
    let world = WorldId(621);
    let mut journal = EditJournal::default();
    let piece = Piece::new([-1, 16, -1], rails::CURVE_LARGE, 0).unwrap();
    record_piece(&mut journal, piece);
    let secondary = *world_rails::cells(piece).last().unwrap();
    journal.record(secondary, 0).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert!(!world_rails::complete(&loaded, piece));
    assert!(world_rails::network(&loaded).unwrap().is_empty());
    let mut invalid = loaded.section_delta(terra_voxel::global_to_section(piece.anchor).0);
    let local = terra_voxel::global_to_section(piece.anchor)
        .1
        .map(|coordinate| coordinate as u8);
    invalid
        .objects
        .iter_mut()
        .find(|(cell, _)| *cell == local)
        .unwrap()
        .1
        .peer = Some([1000, 16, 1000]);
    assert!(EditJournal::default().import_delta(invalid).is_err());
    fs::remove_dir_all(root).unwrap();
}
