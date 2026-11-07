use super::*;
use terra_voxel::BlockSection;

// Keep the previous predicate independent so the shortcut cannot weaken roof,
// snow, or coastal invalidation when no new voxel data is present.
fn legacy(
    exposure: &Exposure,
    position: SectionPos,
    section: Option<&BlockSection>,
    sea: f64,
) -> bool {
    let low = position.y.saturating_mul(16);
    let high = low.saturating_add(15);
    if exposure
        .roofs
        .iter()
        .chain(&exposure.snow_roofs)
        .any(|&roof| (low..=high).contains(&i64::from(roof)))
        || exposure.coastal(sea) && (low..=high).contains(&water_cell_y(sea))
    {
        return true;
    }
    exposure
        .roofs
        .iter()
        .chain(&exposure.snow_roofs)
        .any(|&roof| i64::from(roof) <= high)
        && section.is_some_and(|section| {
            section
                .blocks()
                .iter()
                .any(|&block| MaterialId::from_raw(block).is_some_and(MaterialId::is_solid))
        })
}

fn exposure(roofs: [i16; 256], snow_roofs: [i16; 256]) -> Exposure {
    Exposure {
        natural: None,
        roofs,
        snow_roofs,
        coast: [0.0; 256],
        coast_vertices: [0.0; 289],
        water: [f32::NEG_INFINITY; 256],
    }
}

#[test]
fn missing_and_empty_shortcuts_match_legacy_for_roof_snow_coast_and_all_publication_kinds() {
    let sparse = |background, roof| {
        let mut heights = [background; 256];
        heights[255] = roof;
        heights
    };
    let masks = [
        exposure([i16::MIN; 256], [i16::MIN; 256]),
        exposure([8; 256], [3; 256]),
        exposure([-17; 256], [-33; 256]),
        exposure(sparse(80, -1), sparse(64, -32)),
    ];
    for mask in &masks {
        for y in -5..=7 {
            let position = SectionPos::new(-2, y, -3);
            for sea in [-32.0, -16.25, 0.0, 9.0, 81.0] {
                for material in [None, Some(0), Some(27), Some(14), Some(3)] {
                    let section = material.map(|material| {
                        let mut section = BlockSection::empty(position).unwrap();
                        section.set_block([15; 3], material).unwrap();
                        section
                    });
                    assert_eq!(
                        mask.section_affects(position, section.as_ref(), sea),
                        legacy(mask, position, section.as_ref(), sea),
                        "section y={y}, sea={sea}, material={material:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn removing_or_emptying_a_real_roof_or_waterline_still_recaptures_but_empty_sky_does_not() {
    let roof = SectionPos::new(-2, -2, -3);
    for (mask, position, sea) in [
        (exposure([-17; 256], [-60; 256]), roof, 0.0),
        (exposure([-60; 256], [-17; 256]), roof, 0.0),
        // The coastal water cell (-16) lies above the roof's section (-17).
        (
            exposure([-17; 256], [-60; 256]),
            SectionPos::new(-2, -1, -3),
            -15.75,
        ),
    ] {
        let empty = BlockSection::empty(position).unwrap();
        assert!(mask.section_affects(position, Some(&empty), sea));
        assert!(mask.section_affects(position, None, sea));
    }
    let mask = exposure([8; 256], [3; 256]);
    let sky = SectionPos::new(-2, 4, -3);
    let empty = BlockSection::empty(sky).unwrap();
    assert!(!mask.section_affects(sky, None, 0.0));
    assert!(!mask.section_affects(sky, Some(&empty), 0.0));
    let mut water = empty.clone();
    water.set_block([15; 3], 27).unwrap();
    assert!(!mask.section_affects(sky, Some(&water), 0.0));
    let mut solid = empty;
    solid.set_block([15; 3], 3).unwrap();
    assert!(mask.section_affects(sky, Some(&solid), 0.0));
    let mut interior = BlockSection::empty(SectionPos::new(-2, -4, -3)).unwrap();
    interior.set_block([15; 3], 3).unwrap();
    assert!(!mask.section_affects(interior.position(), Some(&interior), 0.0));
}
