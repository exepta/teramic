//! Cultivation extends saved props; it does not add a second vegetation registry.
use crate::props::{PropId, PropKind, WorldProp};

/// A crop plot has at most one saved identity per species, across unlimited harvests.
pub fn crop_id(support: [i64; 3], kind: terra_core::CropKind) -> PropId {
    let mut hash = 0x4352_4f50_534f_5732_u64;
    // Hash the fixed-width tuple, not kind XOR x: adjacent mixed crops could
    // otherwise cancel those two fields and acquire exactly the same identity.
    for byte in [kind as u8]
        .into_iter()
        .chain(support.into_iter().flat_map(i64::to_le_bytes))
    {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
    }
    PropId(hash.max(1))
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use std::collections::HashSet;
    use terra_core::CropKind;

    #[test]
    fn adjacent_mixed_crops_do_not_cancel_species_against_coordinates() {
        for support in [[4, 3, 4], [-2, 15, -1], [14, -17, 16]] {
            let neighbor = [support[0] + 1, support[1], support[2]];
            assert_ne!(
                crop_id(support, CropKind::Wheat),
                crop_id(neighbor, CropKind::Corn),
            );
        }
        let mut identities = HashSet::new();
        for x in [-17, -16, -1, 0, 1, 15, 16, 17] {
            for y in [-17, -16, -1, 0, 1, 15, 16, 17] {
                for z in [-17, -16, -1, 0, 1, 15, 16, 17] {
                    for kind in CropKind::ALL {
                        let id = crop_id([x, y, z], kind);
                        assert_ne!(id.0, 0);
                        assert_eq!(id, crop_id([x, y, z], kind));
                        assert!(identities.insert(id), "{kind:?} at {x}/{y}/{z}");
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ForageRegrowth {
    pub prop: WorldProp,
    pub remaining_seconds: f32,
}

impl ForageRegrowth {
    pub fn valid(self) -> bool {
        self.prop.valid()
            && !self.prop.removed
            && !self.prop.wall_attached()
            && !self.prop.growing_grass()
            && matches!(
                self.prop.kind,
                PropKind::Grass | PropKind::Flower | PropKind::Mushroom | PropKind::SmallPlant
            )
            && self.remaining_seconds.is_finite()
            && (0.0..=600.0).contains(&self.remaining_seconds)
    }
}

#[cfg(feature = "bevy")]
mod simulation;
#[cfg(feature = "bevy")]
pub(crate) use simulation::pollinate_near;
#[cfg(feature = "bevy")]
pub(crate) use simulation::tick;
#[cfg(feature = "bevy")]
pub use simulation::{commit_crop_state, plant_crop, planting_environment_valid, water_target};
