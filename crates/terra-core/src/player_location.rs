use serde::{Deserialize, Serialize};

/// Authoritative capsule-center position, not camera or avatar coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerLocation {
    pub position: [f32; 3],
    pub look: [f32; 2],
    pub flying: bool,
}

impl PlayerLocation {
    pub fn valid(self) -> bool {
        self.position
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 999_000.0)
            && self.look.iter().all(|v| v.is_finite())
            && self.look[0].abs() <= std::f32::consts::TAU + 0.001
            && self.look[1].abs() <= 1.501
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_locations_match_network_coordinate_and_look_limits() {
        let original = PlayerLocation {
            position: [-512.25, -24.1, 800.5],
            look: [2.1, -0.8],
            flying: false,
        };
        assert!(original.valid());
        for value in [f32::NAN, f32::INFINITY, -f32::INFINITY, 1_000_000.0] {
            for axis in 0..3 {
                let mut invalid = original;
                invalid.position[axis] = value;
                assert!(!invalid.valid());
            }
        }
        for look in [[f32::NAN, 0.0], [0.0, 1.6], [7.0, 0.0]] {
            assert!(!PlayerLocation { look, ..original }.valid());
        }
    }
}
