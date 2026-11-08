//! Shared identities, mass and authored collision volumes for modular train parts.
use crate::block_shapes::ShapeBox;
use serde::Deserialize;
use std::sync::OnceLock;

pub const CHASSIS: u16 = 210;
pub const BOGIE: u16 = 211;
pub const FRAME: u16 = 212;
pub const COUPLER: u16 = 213;
pub const CAB: u16 = 214;
pub const SEAT: u16 = 215;
pub const CARGO: u16 = 216;
pub const STATION: u16 = 217;
pub use crate::locomotive_parts::ENGINE;
/// The gauge-spanning wheelset may cross horizontal owner-cell boundaries.
pub const BOGIE_MIN_HORIZONTAL: f64 = -0.61;
pub const BOGIE_MAX_HORIZONTAL: f64 = 1.61;

pub const fn is_part(material: u16) -> bool {
    material >= CHASSIS && material <= CARGO || crate::locomotive_parts::is_part(material)
}

pub const fn is_train(material: u16) -> bool {
    material >= CHASSIS && material <= STATION || crate::locomotive_parts::is_part(material)
}

pub const fn mass_kg(material: u16) -> Option<u32> {
    match material {
        CHASSIS => Some(600),
        BOGIE => Some(450),
        FRAME => Some(150),
        COUPLER => Some(50),
        CAB => Some(300),
        SEAT => Some(20),
        CARGO => Some(80),
        _ => match crate::locomotive_parts::definition(material) {
            Some(part) => Some(match part.kind {
                crate::locomotive_parts::Kind::Engine => 1000,
                crate::locomotive_parts::Kind::Tank => 400,
                crate::locomotive_parts::Kind::Structure => 200,
                crate::locomotive_parts::Kind::Wheel | crate::locomotive_parts::Kind::Axle => 150,
                crate::locomotive_parts::Kind::Body => 80,
                _ => 25,
            }),
            None => None,
        },
    }
}

pub fn base_boxes(material: u16) -> &'static [ShapeBox] {
    if crate::locomotive_parts::is_part(material) {
        return crate::locomotive_parts::base_boxes(material);
    }
    static PROFILES: [OnceLock<Vec<ShapeBox>>; 8] = [const { OnceLock::new() }; 8];
    const SOURCES: [&str; 8] = [
        include_str!("../../../assets/terra/blocks/train_chassis.json"),
        include_str!("../../../assets/terra/blocks/train_bogie.json"),
        include_str!("../../../assets/terra/blocks/train_frame.json"),
        include_str!("../../../assets/terra/blocks/train_coupler.json"),
        include_str!("../../../assets/terra/blocks/train_cab.json"),
        include_str!("../../../assets/terra/blocks/train_seat.json"),
        include_str!("../../../assets/terra/blocks/train_cargo.json"),
        include_str!("../../../assets/terra/blocks/train_assembly_station.json"),
    ];
    #[derive(Deserialize)]
    struct Definition {
        collision_boxes: Vec<ShapeBox>,
    }
    if !is_train(material) {
        return &[];
    }
    let index = usize::from(material - CHASSIS);
    PROFILES[index].get_or_init(|| {
        serde_json::from_str::<Definition>(SOURCES[index])
            .expect("valid authored train part collision")
            .collision_boxes
    })
}

pub fn boxes(material: u16, facing: u8) -> Vec<ShapeBox> {
    if crate::locomotive_parts::is_slab(material) || crate::locomotive_parts::is_stair(material) {
        return crate::construction::placed_boxes(material, facing, [None; 6]);
    }
    if facing >= 4 {
        return Vec::new();
    }
    base_boxes(material)
        .iter()
        .map(|bounds| bounds.rotated_y(facing))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_train_parts_have_bounded_shared_volumes_and_real_mass() {
        for material in CHASSIS..=STATION {
            for facing in 0..4 {
                let volumes = boxes(material, facing);
                assert!(!volumes.is_empty());
                assert!(volumes.iter().all(|bounds| bounds.valid_for(material)));
                assert!(volumes.iter().all(|bounds| (0..3).all(|axis| {
                    let extended = material == BOGIE && axis != 1;
                    bounds.minimum[axis].is_finite()
                        && bounds.maximum[axis].is_finite()
                        && bounds.minimum[axis] >= if extended { -1.0 } else { -1e-8 }
                        && bounds.maximum[axis] <= if extended { 2.0 } else { 1.0 + 1e-8 }
                        && bounds.minimum[axis] < bounds.maximum[axis]
                })));
            }
            assert_eq!(mass_kg(material).is_some(), is_part(material));
        }
    }

    #[test]
    fn frame_mounted_couplers_and_furniture_reach_their_attachment_faces() {
        let coupling = base_boxes(COUPLER);
        for edge in [0.0, 1.0] {
            assert!(coupling.iter().any(|bounds| {
                (bounds.minimum[2] - edge).abs() < 1e-8 || (bounds.maximum[2] - edge).abs() < 1e-8
            }));
        }
        for material in [SEAT, CARGO, CAB, ENGINE] {
            assert!(
                base_boxes(material)
                    .iter()
                    .any(|bounds| bounds.minimum[1] == 0.0)
            );
        }
    }

    #[test]
    fn only_bogies_accept_the_bounded_horizontal_gauge_overhang() {
        let gauge = ShapeBox {
            minimum: [-0.60, 0.0, 0.0],
            maximum: [1.60, 1.0, 1.0],
            hinged: false,
        };
        for facing in 0..4 {
            let shape = gauge.rotated_y(facing);
            assert!(shape.valid_for(BOGIE));
            for material in [CHASSIS, FRAME, COUPLER, CAB, SEAT, CARGO, STATION] {
                assert!(!shape.valid_for(material));
            }
        }
        let mut shape = gauge;
        shape.minimum[0] = -0.62;
        assert!(!shape.valid_for(BOGIE));
        shape = gauge;
        shape.maximum[0] = 1.62;
        assert!(!shape.valid_for(BOGIE));
        shape = gauge;
        shape.maximum[1] = 1.02;
        assert!(!shape.valid_for(BOGIE));
    }
}
