use terra_core::block_shapes::ShapeBox;

const EPSILON: f32 = 0.00001;

/// Subtract only the solid portion touching this face. Water stays in its own
/// voxel; a nearby slab/stair must not hide the entire falling-water curtain.
pub(super) fn fragments(
    face: [[f32; 3]; 4],
    normal: [f32; 3],
    origin: [i64; 3],
    boxes: &[ShapeBox],
) -> Vec<Vec<[f32; 3]>> {
    let axis = normal.iter().position(|v| *v != 0.0).unwrap();
    let tangents: Vec<_> = (0..3).filter(|&i| i != axis).collect();
    let plane = face[0][axis];
    let mut visible = vec![face.to_vec()];
    for bounds in boxes {
        let minimum: [f32; 3] =
            std::array::from_fn(|i| (bounds.minimum[i] - origin[i] as f64) as f32);
        let maximum: [f32; 3] =
            std::array::from_fn(|i| (bounds.maximum[i] - origin[i] as f64) as f32);
        if plane < minimum[axis] - EPSILON || plane > maximum[axis] + EPSILON {
            continue;
        }
        let mut remaining = Vec::new();
        for polygon in visible {
            let mut inside = polygon;
            for (coordinate, boundary, keep_above) in [
                (tangents[0], minimum[tangents[0]], true),
                (tangents[0], maximum[tangents[0]], false),
                (tangents[1], minimum[tangents[1]], true),
                (tangents[1], maximum[tangents[1]], false),
            ] {
                if inside.len() < 3 {
                    break;
                }
                let outside = clip(&inside, coordinate, boundary, !keep_above);
                if has_area(&outside) {
                    remaining.push(outside);
                }
                inside = clip(&inside, coordinate, boundary, keep_above);
            }
        }
        visible = remaining;
        if visible.is_empty() {
            break;
        }
    }
    visible
}

fn clip(polygon: &[[f32; 3]], axis: usize, boundary: f32, keep_above: bool) -> Vec<[f32; 3]> {
    let mut output = Vec::with_capacity(polygon.len() + 1);
    let distance = |p: [f32; 3]| {
        if keep_above {
            p[axis] - boundary
        } else {
            boundary - p[axis]
        }
    };
    let mut previous = *polygon.last().expect("polygon has at least three vertices");
    let mut previous_distance = distance(previous);
    for &current in polygon {
        let current_distance = distance(current);
        if (current_distance >= 0.0) != (previous_distance >= 0.0) {
            let t = previous_distance / (previous_distance - current_distance);
            let mut point = std::array::from_fn(|i| previous[i] + t * (current[i] - previous[i]));
            point[axis] = boundary;
            push_distinct(&mut output, point);
        }
        if current_distance >= 0.0 {
            push_distinct(&mut output, current);
        }
        previous = current;
        previous_distance = current_distance;
    }
    if output.len() > 1 && same_point(output[0], *output.last().unwrap()) {
        output.pop();
    }
    output
}

fn same_point(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < EPSILON)
}

fn push_distinct(polygon: &mut Vec<[f32; 3]>, point: [f32; 3]) {
    if polygon.last().is_none_or(|&last| !same_point(last, point)) {
        polygon.push(point);
    }
}

fn has_area(polygon: &[[f32; 3]]) -> bool {
    polygon.len() >= 3
        && polygon[1..].windows(2).any(|pair| {
            let u: [f32; 3] = std::array::from_fn(|i| pair[0][i] - polygon[0][i]);
            let v: [f32; 3] = std::array::from_fn(|i| pair[1][i] - polygon[0][i]);
            let cross = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            cross.iter().map(|x| x * x).sum::<f32>() > EPSILON * EPSILON
        })
}
