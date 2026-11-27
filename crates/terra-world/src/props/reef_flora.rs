//! Plants owned by a submerged rock face. The template origin lies just outside
//! the support so that picking, streaming and harvesting agree across sections.
use super::{PropId, PropKind, WorldProp};
use terra_voxel::{MaterialId, global_to_section};

pub const FACES: [[i8; 3]; 5] = [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0], [0, -1, 0]];

pub fn substrate(material: u16) -> bool {
    MaterialId::from_raw(material).is_some_and(|m| {
        m.supports_water_plants()
            || matches!(
                m,
                MaterialId::Stone
                    | MaterialId::MarineLimestone
                    | MaterialId::ReefStone
                    | MaterialId::MossyStone
            )
    })
}

pub fn attached(kind: PropKind, rock: [i64; 3], face: [i8; 3], id: PropId) -> Option<WorldProp> {
    let turn = FACES.iter().position(|f| *f == face)?;
    if (turn == 4 && kind != PropKind::HangingPearlKelp) || (turn < 4 && kind != PropKind::ReefFan)
    {
        return None;
    }
    Some(WorldProp {
        id,
        kind,
        position: std::array::from_fn(|a| rock[a] as f64 + 0.5 + f64::from(face[a]) * 0.501),
        variant: (id.0 % 6) as u8,
        rotation: (turn % 4) as f32 * std::f32::consts::FRAC_PI_2,
        scale: 0.8 + ((id.0 >> 16) % 20) as f32 * 0.01,
        section: global_to_section(rock).0,
        removed: false,
    })
}

pub(super) fn support(prop: &WorldProp) -> [i64; 3] {
    let face = if prop.kind == PropKind::HangingPearlKelp {
        FACES[4]
    } else {
        FACES[(prop.rotation / std::f32::consts::FRAC_PI_2).round() as usize % 4]
    };
    std::array::from_fn(|a| (prop.position[a] - f64::from(face[a]) * 0.501).floor() as i64)
}

pub(crate) fn colonize(
    origin: [i64; 3],
    seed: u64,
    mut wet: impl FnMut([i64; 3]) -> bool,
    mut material: impl FnMut([i64; 3]) -> u16,
    props: &mut Vec<WorldProp>,
) {
    for z in 0..16 {
        for x in 0..16 {
            let px = origin[0] + x;
            let pz = origin[2] + z;
            let mut h =
                (px as u64).wrapping_mul(0x9e3779b97f4a7c15) ^ (pz as u64).rotate_left(37) ^ seed;
            h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            h ^= h >> 27;
            if !h.is_multiple_of(13) {
                continue;
            }
            for y in 0..16 {
                let rock = [px, origin[1] + y, pz];
                if !substrate(material(rock)) {
                    continue;
                }
                let id = h ^ (rock[1] as u64).wrapping_mul(0x94d049bb133111eb);
                // Prefer undersides when both an overhang and its side are exposed.
                for face in [FACES[4], FACES[id as usize % 4]] {
                    let cell = std::array::from_fn(|a| rock[a] + i64::from(face[a]));
                    if wet(cell) && matches!(material(cell), 0 | 27) {
                        let kind = if face[1] < 0 {
                            PropKind::HangingPearlKelp
                        } else {
                            PropKind::ReefFan
                        };
                        if let Some(p) = attached(kind, rock, face, PropId(id ^ 0xa52d8edb808af3cb))
                        {
                            props.push(p);
                        }
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(feature = "bevy")]
pub fn submerged(
    prop: &WorldProp,
    chunks: &crate::streaming::ChunkStore,
    journal: &crate::edits::EditJournal,
) -> bool {
    if !prop.kind.reef_attachment() {
        return true;
    }
    let cell = prop.position.map(|v| v.floor() as i64);
    if !matches!(chunks.block_at(cell), Some(0 | 27)) {
        return false;
    }
    journal.fluid(cell).map_or_else(
        || {
            chunks.surface_column_at(cell[0], cell[2]).is_some_and(|c| {
                crate::water::falls::sampled(c.water_columns(), cell, journal).amount >= 7
            }) || chunks.block_at(cell) == Some(27)
        },
        |f| f.amount >= 7,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mounts_live_on_the_correct_side_and_keep_the_support_owner() {
        for face in FACES {
            let kind = if face[1] == -1 {
                PropKind::HangingPearlKelp
            } else {
                PropKind::ReefFan
            };
            let p = attached(kind, [-16, 0, -16], face, PropId(44)).unwrap();
            assert_eq!(support(&p), [-16, 0, -16]);
            assert!(p.valid());
            let root = p.position.map(|v| v.floor() as i64);
            assert_eq!(
                root,
                std::array::from_fn(|i| support(&p)[i] + i64::from(face[i]))
            );
            for detail in [
                crate::props::visual::PropDetail::Near,
                crate::props::visual::PropDetail::Middle,
                crate::props::visual::PropDetail::Far,
            ] {
                let geometry = crate::props::visual::template(
                    kind,
                    p.template_variant(),
                    detail,
                    &Default::default(),
                )
                .unwrap();
                assert!(!geometry.is_empty());
            }
            let bounds = p.bounds();
            if face[1] < 0 {
                assert!(bounds[0][1] < -0.35 && bounds[1][1] < 0.01);
            }
            assert_eq!(
                crate::props::placement::supported(&p, |cell| Some(if cell == support(&p) {
                    MaterialId::Stone as u16
                } else {
                    27
                })),
                Some(true)
            );
            assert_eq!(
                crate::props::placement::supported(&p, |_| Some(0)),
                Some(false)
            );
        }
    }

    #[test]
    fn undercut_reef_carries_floor_independent_harvestable_plants() {
        let rock = |p: [i64; 3]| {
            if p[1] == 9 || (p[0] % 2 == 0 && (2..=6).contains(&p[1])) {
                MaterialId::MarineLimestone as u16
            } else {
                0
            }
        };
        let mut all = Vec::new();
        colonize([0, 0, 0], 42042, |p| p[1] < 12, rock, &mut all);
        for kind in [PropKind::ReefFan, PropKind::HangingPearlKelp] {
            let p = all.iter().find(|p| p.kind == kind).unwrap();
            assert_eq!(rock(support(p)), MaterialId::MarineLimestone as u16);
            assert!(p.valid());
            let mut journal = crate::edits::EditJournal::default();
            journal.remove_prop(p.section, p.id).unwrap();
            let mut copy = vec![*p];
            journal.apply_prop_removals(p.section, &mut copy);
            assert!(copy.iter().all(|p| p.removed));
        }
        let mut dry = Vec::new();
        colonize([0, 0, 0], 42042, |_| false, rock, &mut dry);
        assert!(dry.is_empty());
    }
}
