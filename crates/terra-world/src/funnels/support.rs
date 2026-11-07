use crate::edits::EditJournal;

/// The existing feet rest on full floors or the flat carrying run of a real belt.
pub fn supported_with(
    position: [i64; 3],
    journal: &EditJournal,
    mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
) -> bool {
    let Some(y) = position[1].checked_sub(1) else {
        return false;
    };
    let below = [position[0], y, position[2]];
    let Some(material) = block_at(below) else {
        return false;
    };
    if terra_core::beds::supports_bed_floor(material) {
        return true;
    }
    if material != terra_core::kinetic::CONVEYOR || journal.block(below) != Some(material) {
        return false;
    }
    let Some(segment) = crate::conveyors::segment_at(journal, below) else {
        return false;
    };
    segment.span.step[1] == 0 && segment.span.normal()[1] > 0.999
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edits::BlockObjectState;

    #[test]
    fn funnel_feet_rest_on_loaded_flat_belts_but_never_air_unknown_or_vertical_parts() {
        let position = [-16, 0, -16];
        let below = [-16, -1, -16];
        let mut journal = EditJournal::default();
        for floor in [None, Some(0), Some(27), Some(3)] {
            assert_eq!(
                supported_with(position, &journal, |_| floor),
                floor == Some(3)
            );
        }
        assert!(
            !supported_with(position, &journal, |_| Some(148)),
            "a raw block without a real belt is not support"
        );
        journal.record(below, 148).unwrap();
        for facing in [0, 1, 2, 3, 56, 57] {
            let end = [below[0], below[1], below[2] + 1];
            journal
                .set_object_state(
                    below,
                    Some(BlockObjectState {
                        facing,
                        peer: (facing >= 56).then_some(end),
                        ..Default::default()
                    }),
                )
                .unwrap();
            if facing >= 56 {
                let start = [below[0], below[1], below[2] - 1];
                for (cell, peer) in [(start, below), (end, start)] {
                    journal.record(cell, 144).unwrap();
                    journal
                        .set_object_state(
                            cell,
                            Some(BlockObjectState {
                                facing: terra_core::kinetic::belt_facing(facing),
                                peer: Some(peer),
                                ..Default::default()
                            }),
                        )
                        .unwrap();
                }
                let segment = crate::conveyors::segment_at(&journal, below).unwrap();
                assert!(segment.span.linked);
                assert_eq!(segment.span.normal()[1], 0.0);
            }
            assert_eq!(
                supported_with(position, &journal, |cell| (cell == below).then_some(148)),
                facing < 4
            );
        }
        assert!(!supported_with(position, &journal, |_| None));
        let start = [below[0], below[1] - 1, below[2] - 1];
        let end = [below[0], below[1] + 1, below[2] + 1];
        for (cell, material, peer) in [(start, 144, below), (below, 148, end), (end, 144, start)] {
            journal.record(cell, material).unwrap();
            journal
                .set_object_state(
                    cell,
                    Some(BlockObjectState {
                        peer: Some(peer),
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        assert!(crate::conveyors::segment_at(&journal, below).is_some());
        assert!(
            !supported_with(position, &journal, |cell| (cell == below).then_some(148)),
            "sloped runs cannot carry flat support feet"
        );
    }
}
