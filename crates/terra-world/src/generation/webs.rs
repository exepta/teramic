//! Sparse, world-coordinate attachment candidates reuse already generated occupancy.
use crate::props::{PropId, WorldProp, webs};
use terra_voxel::{MaterialId, SectionPos};

fn hash(seed: u64, p: [i64; 3]) -> u64 {
    let mut h = seed ^ 0x7370696465727765;
    for coordinate in p {
        h ^= (coordinate as u64).wrapping_mul(0x9e3779b97f4a7c15);
        h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    }
    h ^ (h >> 27)
}

pub(super) fn generate(
    section: SectionPos,
    seed: u64,
    bounded_world: bool,
    mut surface: impl FnMut(i64, i64) -> (f64, bool),
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> Vec<WorldProp> {
    let Ok(origin) = section.origin() else {
        return Vec::new();
    };
    if bounded_world && (origin[1] > 356 || origin[1] + 15 < -128) {
        return Vec::new();
    }
    let mut props = Vec::with_capacity(2);
    for z in 0..4 {
        for y in 0..4 {
            for x in 0..4 {
                let base = [origin[0] + x * 4, origin[1] + y * 4, origin[2] + z * 4];
                let h = hash(seed, base);
                if !h.is_multiple_of(9) {
                    continue;
                }
                let anchor =
                    std::array::from_fn(|axis| base[axis] + ((h >> (axis * 2 + 8)) & 3) as i64);
                let (height, forest) = surface(anchor[0], anchor[2]);
                let cave = anchor[1] as f64 + 5.0 < height;
                let Some(material) = block(anchor).and_then(MaterialId::from_raw) else {
                    continue;
                };
                let tree = matches!(
                    material,
                    MaterialId::Wood | MaterialId::WeatheredWood | MaterialId::SpruceLog
                );
                if !cave && (!forest || !tree) {
                    continue;
                }
                let region = anchor.map(|v| v.div_euclid(48));
                if hash(seed ^ 0x756e646572776562, region) % 5 > 1 {
                    continue;
                }
                let faces = [
                    [0, -1, 0],
                    [1, 0, 0],
                    [0, 0, 1],
                    [-1, 0, 0],
                    [0, 0, -1],
                    [0, 1, 0],
                ];
                for offset in 0..faces.len() {
                    let face = faces[(offset + (h >> 20) as usize) % faces.len()];
                    let variant = if !cave {
                        if face[1] != 0 {
                            continue;
                        }
                        9
                    } else if face[1] == -1 {
                        if h & 8 == 0 { 2 } else { 3 }
                    } else if face[1] == 1 {
                        5
                    } else {
                        [0, 1, 4, 6, 7, 8][(h >> 24) as usize % 6]
                    };
                    let Some(prop) = webs::attached(
                        anchor,
                        face,
                        variant,
                        PropId(hash(seed ^ 0x7765626964, anchor)),
                    ) else {
                        continue;
                    };
                    if webs::fits_in_world(&prop, bounded_world, &mut block) {
                        props.push(prop);
                        break;
                    }
                }
                if props.len() == 2 {
                    return props;
                }
            }
        }
    }
    props
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_section_does_not_generate_silk_on_the_outside_of_world_bedrock() {
        let mut old_exterior = 0;
        for x in -4..=4 {
            for z in -4..=4 {
                let section = SectionPos::new(x, -8, z);
                let generated = |bounded| {
                    generate(
                        section,
                        7331,
                        bounded,
                        |_, _| (80.0, false),
                        |p| Some(if p[1] >= -128 { 1 } else { 0 }),
                    )
                };
                old_exterior += generated(false).len();
                assert!(generated(true).is_empty());
            }
        }
        assert!(
            old_exterior > 0,
            "the former generator accepted the bedrock underside"
        );
        assert!(
            generate(
                SectionPos::new(-1, -9, -1),
                7331,
                true,
                |_, _| (80.0, false),
                |_| Some(1)
            )
            .is_empty()
        );
    }

    #[test]
    fn generated_silk_is_deterministic_bounded_and_survives_negative_section_edges() {
        let sample = |position| {
            generate(
                position,
                7331,
                true,
                |_, _| (80.0, false),
                |p| Some(if p[1].rem_euclid(8) == 0 { 1 } else { 0 }),
            )
        };
        let mut count = 0;
        for x in -4..=4 {
            for z in -4..=4 {
                let section = SectionPos::new(x, -2, z);
                let a = sample(section);
                assert_eq!(a, sample(section));
                assert!(a.len() <= 2);
                for prop in &a {
                    assert!(prop.valid());
                    assert_eq!(prop.section, section);
                    assert!(webs::fits(prop, |p| Some(if p[1].rem_euclid(8) == 0 {
                        1
                    } else {
                        0
                    })));
                }
                count += a.len();
            }
        }
        assert!(count > 0);
    }
}
