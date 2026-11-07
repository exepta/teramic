//! Visible valve/pump sweeps from tools/assets/build_hydraulics.mjs.
//! Picking and collision deliberately keep their inexpensive bounding boxes.
use super::*;
use terra_core::plumbing as core;

impl Slice {
    fn block(&mut self, center: [f64; 3], size: [f64; 3]) {
        let center = DVec3::from_array(center);
        let half = DVec3::from_array(size) * 0.5;
        let rings: [[DVec3; 4]; 2] = [-1., 1.].map(|x| {
            [[-1., -1.], [1., -1.], [1., 1.], [-1., 1.]]
                .map(|[y, z]| center + half * DVec3::new(x, y, z))
        });
        self.prism(rings);
    }

    fn ring(&mut self, axis: usize, start: f64, end: f64, radius: f64, bore: f64) {
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        for sector in 0..8 {
            let rings: [[DVec3; 4]; 2] = [start, end].map(|t| {
                [
                    (sector, radius),
                    (sector + 1, radius),
                    (sector + 1, bore),
                    (sector, bore),
                ]
                .map(|(sector, r)| {
                    let angle = sector as f64 * TAU / 8.;
                    let mut p = DVec3::splat(0.5);
                    p[axis] = t;
                    p[u] += angle.cos() * r;
                    p[v] += angle.sin() * r;
                    p
                })
            });
            self.prism(rings);
        }
    }

    fn flange(&mut self, axis: usize, t: f64) {
        self.ring(axis, t - 0.035, t + 0.035, 0.215, 0.152);
        for bolt in 0..4 {
            let angle = (bolt as f64 + 0.5) * TAU / 4.;
            let mut center = [0.5; 3];
            center[axis] = t + if t < 0.5 { 0.04 } else { -0.04 };
            center[(axis + 1) % 3] += angle.cos() * 0.215 * 0.82;
            center[(axis + 2) % 3] += angle.sin() * 0.215 * 0.82;
            let mut size = [0.043; 3];
            size[axis] = 0.027;
            self.block(center, size);
        }
    }

    fn valve(&mut self, closed: bool) {
        self.cylinder(2, 0., 1., 0.155, 8);
        self.flange(2, 0.055);
        self.flange(2, 0.945);
        self.block([0.5; 3], [0.40, 0.36, 0.34]);
        self.cylinder(1, 0.60, 0.78, 0.065, 8);
        let rotation = self.rotation;
        if closed {
            self.rotation *= DMat3::from_rotation_y(std::f64::consts::FRAC_PI_4);
        }
        self.ring(1, 0.79, 0.87, 0.205, 0.16);
        self.block([0.5, 0.83, 0.5], [0.30, 0.045, 0.045]);
        self.block([0.5, 0.83, 0.5], [0.045, 0.045, 0.30]);
        self.block([0.5, 0.84, 0.5], [0.072, 0.07, 0.072]);
        self.rotation = rotation;
    }

    fn pump(&mut self) {
        self.block([0.5, 0.075, 0.5], [0.86, 0.15, 0.80]);
        for y in [0.22, 0.77] {
            self.block([0.5, y, 0.5], [0.70, 0.13, 0.65]);
            self.block([0.5, y + 0.015, 0.5], [0.75, 0.045, 0.70]);
        }
        for x in [0.19, 0.81] {
            for z in [0.22, 0.78] {
                self.block([x, 0.49, z], [0.085, 0.54, 0.085]);
                for y in [0.25, 0.75] {
                    self.block([x, y, z], [0.115, 0.045, 0.115]);
                }
            }
        }
        // The sealed transparent chamber contains separate hydraulic water;
        // ambient lake water must not create a second coplanar sheet inside it.
        self.block([0.5; 3], [0.51, 0.43, 0.488]);
        self.cylinder(2, 0., 0.265, 0.13, 8);
        self.flange(2, 0.045);
        self.cylinder(2, 0.735, 1., 0.13, 8);
        self.flange(2, 0.955);
        self.cylinder(1, 0.81, 0.94, 0.075, 8);
        self.block([0.5, 0.96, 0.5], [0.16, 0.06, 0.16]);
        self.ring(0, 0.82, 0.93, 0.20, 0.145);
        self.cylinder(0, 0., 1., 0.045, 8);
        for x in [0.065, 0.935] {
            self.cylinder(0, x - 0.035, x + 0.035, 0.082, 8);
        }
        for tooth in 0..8 {
            let angle = tooth as f64 * TAU / 8.;
            self.block(
                [0.875, 0.5 + angle.cos() * 0.20, 0.5 + angle.sin() * 0.20],
                [0.10, 0.075, 0.075],
            );
        }
    }

    fn faucet(&mut self, facing: u8) {
        self.flange(2, 0.055);
        self.cylinder(2, 0., 0.63, 0.125, 8);
        self.cylinder_at(1, 0.29, 0.50, 0.125, 8, DVec3::new(0.5, 0.5, 0.63));
        self.cylinder_at(1, 0.275, 0.32, 0.153, 8, DVec3::new(0.5, 0.5, 0.63));
        self.block([0.5, 0.5, 0.61], [0.25; 3]);
        self.cylinder(1, 0.59, 0.73, 0.052, 8);
        let rotation = self.rotation;
        self.rotation *= DMat3::from_rotation_y(
            f64::from(3 - core::faucet_level(facing)) * std::f64::consts::FRAC_PI_6,
        );
        self.block([0.5, 0.75, 0.5], [0.30, 0.055, 0.055]);
        self.block([0.5, 0.75, 0.5], [0.06, 0.075, 0.06]);
        self.rotation = rotation;
    }
}

pub(crate) fn fragments(
    visible: Vec<Vec<[f32; 3]>>,
    face: [[f32; 3]; 4],
    normal: [f32; 3],
    origin: [f32; 3],
    material: u16,
    facing: u8,
    own_cell: bool,
) -> Vec<Vec<[f32; 3]>> {
    let axis = normal.iter().position(|v| *v != 0.).unwrap();
    let plane = f64::from(face[0][axis] - origin[axis]);
    if visible.is_empty() || !(-EPS..=1. + EPS).contains(&plane) {
        return visible;
    }
    let tangents: Vec<_> = (0..3).filter(|i| *i != axis).collect();
    let tangents = [tangents[0], tangents[1]];
    let pose = core::orientation(material, facing);
    let yaw = |facing| DMat3::from_rotation_y(f64::from(facing) * FRAC_PI_2);
    let rotation = if material == core::VALVE {
        // The authored valve is yawed once by its asset builder, then its +X
        // layout follows the exact same six poses as ShapeBox::rotated_axis.
        (match pose {
            4 => DMat3::from_rotation_z(FRAC_PI_2),
            5 => DMat3::from_rotation_z(-FRAC_PI_2),
            _ => yaw(pose),
        }) * yaw(1)
    } else {
        yaw(pose)
    };
    let mut slice = Slice {
        axis,
        plane,
        tangents,
        cuts: vec![],
        rotation,
    };
    let end = if plane.abs() < EPS {
        Some(axis * 2)
    } else if (plane - 1.).abs() < EPS {
        Some(axis * 2 + 1)
    } else {
        None
    };
    if !own_cell && end.is_some_and(|side| core::connects(material, facing, core::DIRECTIONS[side]))
    {
        let (radius, bore) = match material {
            core::VALVE => (0.155, 0.118),
            core::FAUCET => (0.125, 0.095),
            _ => (0.13, 0.10),
        };
        slice.mouth(radius, bore, 8);
    } else if material == core::VALVE {
        slice.valve(facing & 8 != 0);
    } else if material == core::FAUCET {
        slice.faucet(facing);
    } else {
        slice.pump();
    }
    subtract(visible, slice.cuts, tangents, origin)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice(material: u16, facing: u8, axis: usize, level: f32, own: bool) -> Vec<Vec<[f32; 3]>> {
        let tangents: Vec<_> = (0..3).filter(|i| *i != axis).collect();
        let mut normal = [0.; 3];
        normal[axis] = 1.;
        let face = [[0., 0.], [0., 1.], [1., 1.], [1., 0.]].map(|[u, v]| {
            let mut p = [0.; 3];
            p[axis] = level;
            p[tangents[0]] = u;
            p[tangents[1]] = v;
            p
        });
        fragments(
            vec![face.to_vec()],
            face,
            normal,
            [0.; 3],
            material,
            facing,
            own,
        )
    }

    fn area(polygons: &[Vec<[f32; 3]>], axis: usize) -> f64 {
        let tangents: Vec<_> = (0..3).filter(|i| *i != axis).collect();
        polygons
            .iter()
            .map(|polygon| {
                let p = |i: usize| {
                    DVec2::new(
                        f64::from(polygon[i][tangents[0]]),
                        f64::from(polygon[i][tangents[1]]),
                    )
                };
                (1..polygon.len() - 1)
                    .map(|i| (p(i) - p(0)).perp_dot(p(i + 1) - p(0)).abs() * 0.5)
                    .sum::<f64>()
            })
            .sum()
    }

    fn covered(polygons: &[Vec<[f32; 3]>], point: [f64; 3], axis: usize) -> bool {
        let tangents: Vec<_> = (0..3).filter(|i| *i != axis).collect();
        let point = DVec2::new(point[tangents[0]], point[tangents[1]]);
        polygons.iter().any(|polygon| {
            let p = |i: usize| {
                DVec2::new(
                    f64::from(polygon[i][tangents[0]]),
                    f64::from(polygon[i][tangents[1]]),
                )
            };
            // Clipping can retain subpixel f32 slivers. An unnormalised cross
            // product makes their tiny edges accept distant points on the
            // edge's extension, even far outside the polygon's bounds.
            let minimum = (0..polygon.len())
                .map(p)
                .fold(DVec2::splat(f64::INFINITY), DVec2::min);
            let maximum = (0..polygon.len())
                .map(p)
                .fold(DVec2::splat(f64::NEG_INFINITY), DVec2::max);
            if point.cmplt(minimum - DVec2::splat(1e-8)).any()
                || point.cmpgt(maximum + DVec2::splat(1e-8)).any()
            {
                return false;
            }
            let distances: Vec<_> = (0..polygon.len())
                .map(|i| {
                    let edge = p((i + 1) % polygon.len()) - p(i);
                    edge.perp_dot(point - p(i)) / edge.length()
                })
                .collect();
            distances.iter().all(|d| *d >= -1e-8) || distances.iter().all(|d| *d <= 1e-8)
        })
    }

    #[test]
    fn valve_and_pump_mouths_cut_the_actual_octagon_in_every_pose_instead_of_a_square() {
        for material in [core::VALVE, core::PUMP] {
            for facing in 0..if material == core::VALVE { 6 } else { 4 } {
                let axis = core::ports(material, facing).trailing_zeros() as usize / 2;
                let (radius, bore): (f64, f64) = if material == core::VALVE {
                    (0.155, 0.118)
                } else {
                    (0.13, 0.10)
                };
                for level in [0., 1.] {
                    for own in [true, false] {
                        let polygons = slice(material, facing, axis, level, own);
                        let removed = 4.
                            * (radius.powi(2) - if own { 0. } else { bore.powi(2) })
                            * (TAU / 8.).sin();
                        // Eight triangles, each with sin(PI/4) between its radii.
                        let expected = 1. - removed;
                        assert!(
                            (area(&polygons, axis) - expected).abs() < 1e-5,
                            "material={material} facing={facing} plane={level} own={own}"
                        );
                        let mut corner = [0.64; 3];
                        corner[axis] = f64::from(level);
                        assert!(covered(&polygons, corner, axis));
                        let mut center = [0.5; 3];
                        center[axis] = f64::from(level);
                        assert_eq!(covered(&polygons, center, axis), !own);
                    }
                }
            }
        }
    }

    #[test]
    fn the_valve_handwheel_hole_and_pump_drive_box_corners_remain_wet() {
        for facing in [0, 8] {
            let polygons = slice(core::VALVE, facing, 1, 0.83, true);
            assert!(covered(&polygons, [0.56, 0.83, 0.60], 1));
            assert!(!covered(&polygons, [0.5, 0.83, 0.5], 1));
        }
        let pump = slice(core::PUMP, 0, 1, 0.69, true);
        assert!(covered(&pump, [0.875, 0.69, 0.69], 1));
        assert!(!covered(&pump, [0.5, 0.69, 0.5], 1));
    }

    #[test]
    fn thinner_pump_and_faucet_bodies_keep_the_visible_space_inside_their_flange_rings_wet() {
        for material in [core::PUMP, core::FAUCET] {
            let polygons = slice(material, 0, 2, 0.055, true);
            assert!(covered(&polygons, [0.64, 0.5, 0.055], 2));
            assert!(!covered(&polygons, [0.50, 0.5, 0.055], 2));
            assert!(!covered(&polygons, [0.70, 0.5, 0.055], 2));
        }
    }
}
