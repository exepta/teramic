use super::*;
use crate::MaterialId;
#[test]
fn material_membership_remains_conservative_after_collisions_and_replacements() {
    let mut section = BlockSection::empty(SectionPos::default()).unwrap();
    for (index, material) in [3, 259, 369, 65535].into_iter().enumerate() {
        section.set_block([index, 0, 0], material).unwrap();
        assert!(section.may_contain(material));
    }
    assert!(!section.may_contain(193));
    section.set_block([0, 0, 0], 0).unwrap();
    assert!(section.may_contain(3), "259 shares the conservative bucket");
    section.set_block([1, 0, 0], 0).unwrap();
    assert!(!section.may_contain(3));
    let restored = BlockSection::from_blocks(
        section.position(),
        section.revision(),
        section.blocks().to_vec(),
    )
    .unwrap();
    assert_eq!(section, restored, "index must also agree after save/load");
    for &material in section.blocks() {
        assert!(section.may_contain(material));
    }
}
#[test]
fn section_coordinates_cover_negative_blocks_and_columns() {
    for (global, section, local) in [
        (0, 0, 0),
        (15, 0, 15),
        (16, 1, 0),
        (-1, -1, 15),
        (-16, -1, 0),
        (-17, -2, 15),
    ] {
        let (p, l) = global_to_section([global; 3]);
        assert_eq!(p, SectionPos::new(section, section, section));
        assert_eq!(l, [local; 3]);
        assert_eq!(p.global_block(l).unwrap(), [global; 3]);
    }
    let (p, l) = global_to_section([-1, 33, -17]);
    assert_eq!(p, SectionPos::new(-1, 2, -2));
    assert_eq!(l, [15, 1, 15]);
    assert_eq!(p.column(), ColumnPos { x: -1, z: -2 });
    assert!(SectionPos::new(i64::MAX, 0, 0).origin().is_err());
}
#[test]
fn block_indexing_and_mutation_are_checked() {
    assert_eq!(block_index([1, 0, 0]).unwrap(), 1);
    assert_eq!(block_index([0, 1, 0]).unwrap(), 16);
    assert_eq!(block_index([0, 0, 1]).unwrap(), 256);
    assert_eq!(block_index([15; 3]).unwrap(), 4095);
    let mut section = BlockSection::empty(SectionPos::default()).unwrap();
    assert!(section.is_empty());
    assert!(
        section
            .set_block([15; 3], MaterialId::Stone as u16)
            .unwrap()
    );
    assert!(
        !section
            .set_block([15; 3], MaterialId::Stone as u16)
            .unwrap()
    );
    assert_eq!(section.revision(), 1);
    assert!(!section.is_empty());
    section.set_block([15; 3], 0).unwrap();
    assert!(section.is_empty());
    assert_eq!(section.revision(), 2);
    for l in [[16, 0, 0], [0, 16, 0], [0, 0, 16], [usize::MAX; 3]] {
        assert!(section.block(l).is_err());
        assert!(section.set_block(l, 1).is_err());
    }
    assert!(BlockSection::from_blocks(SectionPos::default(), 0, vec![]).is_err());
}
#[test]
fn raycast_targets_negative_blocks_and_respects_reach() {
    let sample = |p: [i64; 3]| Some(if p == [-1, 0, 0] { 3 } else { 0 });
    let hit = raycast_blocks([1.5, 0.5, 0.5], [-1.0, 0.0, 0.0], 4.0, sample).unwrap();
    assert_eq!(hit.position, [-1, 0, 0]);
    assert_eq!(hit.normal, [1, 0, 0]);
    assert_eq!(hit.adjacent(), [0, 0, 0]);
    assert!((hit.distance - 1.5).abs() < 1e-9);
    assert!(raycast_blocks([1.5, 0.5, 0.5], [-1.0, 0.0, 0.0], 1.0, sample).is_none());
    assert!(raycast_blocks([0.5; 3], [0.0; 3], 4.0, sample).is_none());
    assert!(raycast_blocks([f64::NAN; 3], [1.0, 0.0, 0.0], 4.0, sample).is_none());
}
#[test]
fn raycast_rebases_large_coordinates_and_stops_at_unloaded_sections() {
    let base = 1_i64 << 36;
    let hit = raycast_blocks([base as f64 + 0.5, 0.5, 0.5], [1.0, 0.0, 0.0], 6.0, |p| {
        Some(if p[0] == base + 3 { 1 } else { 0 })
    })
    .unwrap();
    assert_eq!(hit.position, [base + 3, 0, 0]);
    assert!((hit.distance - 2.5).abs() < 1e-9);
    assert!(
        raycast_blocks([0.5; 3], [1.0, 0.0, 0.0], 6.0, |p| if p[0] == 1 {
            None
        } else {
            Some(3)
        })
        .is_some()
    );
    assert!(
        raycast_blocks([0.5; 3], [1.0, 0.0, 0.0], 6.0, |p| if p[0] == 1 {
            None
        } else {
            Some(0)
        })
        .is_none()
    );
}

#[test]
fn uniform_sections_share_readonly_voxels_and_edit_independently() {
    for material in [MaterialId::Air as u16, MaterialId::Stone as u16] {
        let mut a =
            BlockSection::from_blocks(SectionPos::default(), 7, vec![material; SECTION_VOLUME])
                .unwrap();
        let b = a.clone();
        assert_eq!(a.blocks().as_ptr(), b.blocks().as_ptr());
        a.set_block([15; 3], 259).unwrap();
        assert_eq!(b.block([15; 3]).unwrap(), material);
        assert_eq!(a.block([15; 3]).unwrap(), 259);
        assert_eq!(a.revision(), 8);
        assert!(a.may_contain(259));
        a.set_block([15; 3], material).unwrap();
        assert_eq!(a.blocks(), b.blocks());
        if material == 0 {
            assert_eq!(a.blocks().as_ptr(), b.blocks().as_ptr());
        }
    }
}
