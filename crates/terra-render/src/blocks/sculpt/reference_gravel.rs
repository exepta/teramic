//! Fracture lines traced in the orthographic 384 px gravel tile. The crown,
//! its bevel and the backing all use the same face projection.
use super::patch::FacePatch;
use crate::blocks::PrimitiveTemplate;
use bevy::math::Vec2;

type Outline = &'static [[u16; 2]];

// Outline and interior cap anchor in gravel.png pixels, then height in millimetres.
// Each anchor sees every segment of its outline, including reentrant fractures.
// Retaining the partial stones at the margins keeps the block densely packed.
const STONES: &[(Outline, [u16; 2], u8)] = &[
    (
        &[[0, 0], [95, 0], [99, 43], [52, 58], [0, 73]],
        [31, 31],
        48,
    ),
    (
        &[[95, 0], [198, 0], [203, 49], [177, 55], [139, 44], [99, 43]],
        [143, 22],
        28,
    ),
    (
        &[
            [198, 0],
            [328, 0],
            [313, 47],
            [281, 80],
            [249, 82],
            [203, 49],
        ],
        [261, 41],
        58,
    ),
    (
        &[
            [328, 0],
            [384, 0],
            [384, 69],
            [365, 70],
            [325, 63],
            [281, 80],
            [313, 47],
        ],
        [363, 26],
        23,
    ),
    (
        &[[0, 73], [52, 58], [52, 105], [59, 140], [55, 174], [0, 164]],
        [26, 108],
        54,
    ),
    (
        &[
            [52, 58],
            [99, 43],
            [139, 44],
            [126, 88],
            [100, 100],
            [102, 141],
            [96, 173],
            [55, 174],
            [59, 140],
            [52, 105],
        ],
        [82, 89],
        31,
    ),
    (
        &[
            [139, 44],
            [177, 55],
            [203, 49],
            [249, 82],
            [246, 129],
            [182, 131],
            [142, 142],
            [102, 141],
            [100, 100],
            [126, 88],
        ],
        [192, 94],
        38,
    ),
    (
        &[
            [102, 141],
            [142, 142],
            [182, 131],
            [246, 129],
            [252, 130],
            [245, 193],
            [217, 186],
            [174, 190],
            [123, 192],
            [96, 173],
        ],
        [219, 165],
        22,
    ),
    (
        &[
            [249, 82],
            [281, 80],
            [325, 63],
            [346, 68],
            [329, 128],
            [306, 136],
            [252, 130],
            [246, 129],
        ],
        [303, 107],
        63,
    ),
    (
        &[
            [346, 68],
            [365, 70],
            [384, 69],
            [384, 183],
            [337, 199],
            [316, 182],
            [306, 136],
            [329, 128],
        ],
        [352, 161],
        36,
    ),
    (
        &[
            [252, 130],
            [306, 136],
            [316, 182],
            [337, 199],
            [319, 221],
            [277, 228],
            [245, 193],
        ],
        [287, 196],
        18,
    ),
    (
        &[
            [0, 164],
            [55, 174],
            [96, 173],
            [123, 192],
            [123, 278],
            [126, 311],
            [96, 321],
            [46, 292],
            [22, 249],
            [0, 252],
        ],
        [43, 210],
        37,
    ),
    (
        &[
            [123, 192],
            [174, 190],
            [217, 186],
            [245, 193],
            [252, 223],
            [246, 289],
            [219, 297],
            [167, 300],
            [123, 278],
        ],
        [190, 244],
        69,
    ),
    (
        &[
            [245, 193],
            [277, 228],
            [319, 221],
            [337, 199],
            [339, 252],
            [330, 292],
            [279, 303],
            [246, 289],
            [252, 223],
        ],
        [297, 279],
        24,
    ),
    (
        &[[337, 199], [384, 183], [384, 250], [373, 267], [339, 252]],
        [361, 216],
        55,
    ),
    (
        &[
            [0, 252],
            [22, 249],
            [46, 292],
            [96, 321],
            [80, 384],
            [0, 384],
        ],
        [36, 348],
        26,
    ),
    (
        &[
            [96, 321],
            [126, 311],
            [123, 278],
            [167, 300],
            [219, 297],
            [204, 344],
            [195, 384],
            [80, 384],
        ],
        [166, 341],
        51,
    ),
    (
        &[
            [219, 297],
            [246, 289],
            [279, 303],
            [330, 292],
            [339, 252],
            [373, 267],
            [384, 250],
            [384, 384],
            [195, 384],
            [204, 344],
        ],
        [344, 358],
        34,
    ),
];

pub(super) fn face(side: usize, variant: usize) -> Vec<PrimitiveTemplate> {
    let mut mesh = FacePatch::new(side, variant);
    for (index, &(survey, anchor, height_mm)) in STONES.iter().enumerate() {
        let points: Vec<_> = survey
            .iter()
            .map(|&[x, y]| Vec2::new(f32::from(x), f32::from(y)) / 384.)
            .collect();
        let center = Vec2::new(f32::from(anchor[0]), f32::from(anchor[1])) / 384.;
        let variation = (index * 3 + mesh.variant + side) % 4;
        let height = (f32::from(height_mm) + [-6., 4., -2., 6.][variation]) / 1000.;
        let tilt = [
            Vec2::new(-0.017, 0.009),
            Vec2::new(0.011, -0.018),
            Vec2::new(0.019, 0.012),
            Vec2::new(-0.009, -0.014),
        ][variation];
        let mut rim = Vec::with_capacity(points.len() * 2);
        for (i, &p) in points.iter().enumerate() {
            let before = points[(i + points.len() - 1) % points.len()] - p;
            let after = points[(i + 1) % points.len()] - p;
            let b = before.normalize();
            let a = after.normalize();
            // A single broken facet across an exposed corner; almost straight
            // junctions in the survey and reentrant corners keep their outline.
            if a.perp_dot(b) > 0.32 {
                let chip = (0.009 + 0.003 * ((index + i + variation) % 4) as f32)
                    .min(before.length().min(after.length()) * 0.23);
                rim.extend([p + b * chip, p + a * chip * 0.73]);
            } else {
                rim.push(p);
            }
        }
        let rings: Vec<_> = rim
            .iter()
            .map(|&p| {
                let inward = (center - p).normalize();
                let h = (height + (p - center).dot(tilt)).clamp(0.012, 0.075);
                let wall = p + inward * 0.0028;
                let shoulder = p + inward * 0.0055;
                let crown = p + inward * (0.009 + 0.11 * h);
                (
                    [wall.x, wall.y, 0.],
                    [shoulder.x, shoulder.y, -h * 0.73],
                    [crown.x, crown.y, -h],
                )
            })
            .collect();
        for i in 0..rings.len() {
            let (a, b, c) = rings[i];
            let (next_a, next_b, next_c) = rings[(i + 1) % rings.len()];
            if height > 0.040 {
                mesh.surface_polygon(side, [a, next_a, next_b, b]);
                mesh.surface_polygon(side, [b, next_b, next_c, c]);
            } else {
                mesh.surface_polygon(side, [a, next_a, next_c, c]);
            }
            mesh.surface_polygon(side, [[center.x, center.y, -height], c, next_c]);
        }
    }
    mesh.meshes
}
