use terra_voxel::meshing::BlockFace;

pub(super) fn split(face: BlockFace, tint: &crate::appearance::GroundTint) -> Vec<BlockFace> {
    let mut result = Vec::new();
    subdivide(face, &|point| tint.at(point, face.material), &mut result);
    result
}

// Greedy triangles interpolate linearly, unlike the bilinear climate field.
// Retain large rectangles unless their actual interior color would visibly pop.
fn subdivide(face: BlockFace, color: &impl Fn([f32; 3]) -> [f32; 3], result: &mut Vec<BlockFace>) {
    if face.width == 1 && face.height == 1 || error(face, color) <= 0.025 {
        result.push(face);
        return;
    }
    let mut a = face;
    let mut b = face;
    if face.width >= face.height {
        a.width /= 2;
        b.minimum[face.u_axis] += a.width;
        b.width -= a.width;
    } else {
        a.height /= 2;
        b.minimum[face.v_axis] += a.height;
        b.height -= a.height;
    }
    subdivide(a, color, result);
    subdivide(b, color, result);
}

fn error(face: BlockFace, color: &impl Fn([f32; 3]) -> [f32; 3]) -> f32 {
    let point = |u: f32, v: f32| {
        let mut p = face.minimum.map(|v| v as f32);
        p[1] += 1.0;
        p[face.u_axis] += u * face.width as f32;
        p[face.v_axis] += v * face.height as f32;
        color(p)
    };
    let [a, b, c, d] = [
        point(0.0, 0.0),
        point(1.0, 0.0),
        point(1.0, 1.0),
        point(0.0, 1.0),
    ];
    [0.25, 0.5, 0.75]
        .into_iter()
        .flat_map(|u| [0.25, 0.5, 0.75].into_iter().map(move |v| (u, v)))
        .map(|(u, v)| {
            let actual = point(u, v);
            (0..3)
                .map(|axis| {
                    let linear = if u >= v {
                        a[axis] * (1.0 - u) + b[axis] * (u - v) + c[axis] * v
                    } else {
                        a[axis] * (1.0 - v) + c[axis] * u + d[axis] * (v - u)
                    };
                    (linear - actual[axis]).abs()
                })
                .fold(0.0, f32::max)
        })
        .fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curved_color_fields_split_but_flat_and_linear_fields_stay_greedy() {
        let face = BlockFace {
            edge_mask: 15,
            occupied_neighbor: false,
            minimum: [0; 3],
            width: 16,
            height: 16,
            u_axis: 0,
            v_axis: 2,
            side: 3,
            material: 1,
        };
        for linear in [false, true] {
            let mut result = Vec::new();
            subdivide(
                face,
                &|p| [if linear { p[0] / 16.0 } else { 0.5 }; 3],
                &mut result,
            );
            assert_eq!(result.len(), 1);
        }
        let color = |p: [f32; 3]| {
            let [x, _, z] = p.map(|v| v / 16.0);
            [x * z + (1.0 - x) * (1.0 - z); 3]
        };
        let mut result = Vec::new();
        subdivide(face, &color, &mut result);
        assert!(result.len() > 1 && result.len() <= 64);
        assert_eq!(result.iter().map(|f| f.width * f.height).sum::<u32>(), 256);
        assert!(result.iter().all(|face| error(*face, &color) <= 0.025));
    }
}
