//! Headless export of the production rail profile; no renderer or world jobs.
// Build with rustc and the workspace's existing serde artifact. The small
// ShapeBox adapter keeps this asset tool independent from the game bootstrap.
#![allow(dead_code)]

mod block_shapes {
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ShapeBox {
        pub minimum: [f64; 3],
        pub maximum: [f64; 3],
        pub hinged: bool,
    }
    impl ShapeBox {
        pub fn valid(self) -> bool {
            (0..3).all(|axis| {
                self.minimum[axis].is_finite()
                    && self.maximum[axis].is_finite()
                    && self.minimum[axis] < self.maximum[axis]
            })
        }
    }
}

#[allow(unused_imports)]
mod rails {
    include!(env!("TERRA_RAIL_PROFILE_SOURCE"));
}

fn main() {
    println!("{{\"profiles\":[");
    let mut first_profile = true;
    for kind in rails::Kind::ALL {
        for facing in if kind.is_switch() {
            &[0, 4][..]
        } else {
            &[0][..]
        } {
            if !first_profile {
                println!(",");
            }
            first_profile = false;
            let piece = rails::Piece::from_facing([0; 3], kind.material(), *facing).unwrap();
            print!(
                "{{\"material\":{},\"key\":\"{}\",\"facing\":{},\"beams\":[",
                kind.material(),
                kind.key(),
                facing
            );
            for (index, beam) in piece.local_geometry().iter().enumerate() {
                if index != 0 {
                    print!(",");
                }
                print!(
                    "{{\"center\":{:?},\"right\":{:?},\"up\":{:?},\"forward\":{:?},\"half_extents\":{:?},\"material\":\"{:?}\",\"corners\":{:?}}}",
                    beam.center,
                    beam.right,
                    beam.up,
                    beam.forward,
                    beam.half_extents,
                    beam.material,
                    beam.corners()
                );
            }
            print!("],\"surfaces\":[");
            for (index, surface) in piece.local_surfaces().iter().enumerate() {
                if index != 0 {
                    print!(",");
                }
                print!(
                    "{{\"beam\":{},\"points\":{:?},\"normal\":{:?},\"material\":\"{:?}\"}}",
                    surface.beam, surface.points, surface.normal, surface.material
                );
            }
            print!("],\"anchor_collision\":[");
            for (index, shape) in piece.cell_boxes([0; 3]).iter().enumerate() {
                if index != 0 {
                    print!(",");
                }
                print!(
                    "{{\"minimum\":{:?},\"maximum\":{:?}}}",
                    shape.minimum, shape.maximum
                );
            }
            print!("],\"footprint\":{:?}}}", piece.footprint());
        }
    }
    println!("]}}");
}
