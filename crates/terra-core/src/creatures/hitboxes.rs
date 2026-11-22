//! Anatomical picking volumes, separate from conservative locomotion clearance.
use super::{Creature, CreatureBehavior, CreatureKind as Kind};

mod dragon;

const CAPACITY: usize = 48;
const IDENTITY: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

#[derive(Debug, Clone, Copy)]
pub struct Hitbox {
    pub center: [f64; 3],
    pub size: [f64; 3],
    /// Local X/Y/Z basis vectors in world space.
    pub axes: [[f64; 3]; 3],
}
impl Hitbox {
    const EMPTY: Self = Self {
        center: [0.0; 3],
        size: [0.0; 3],
        axes: IDENTITY,
    };

    pub fn corners(self) -> [[f64; 3]; 8] {
        std::array::from_fn(|corner| {
            let offset = std::array::from_fn(|axis| {
                self.size[axis] * if corner & (1 << axis) == 0 { -0.5 } else { 0.5 }
            });
            let delta = transform(self.axes, offset);
            std::array::from_fn(|i| self.center[i] + delta[i])
        })
    }

    pub fn ray_distance(self, origin: [f64; 3], direction: [f64; 3], reach: f64) -> Option<f64> {
        if !reach.is_finite()
            || reach < 0.0
            || !origin.iter().chain(direction.iter()).all(|v| v.is_finite())
        {
            return None;
        }
        let delta = std::array::from_fn::<_, 3, _>(|i| origin[i] - self.center[i]);
        let mut near: f64 = 0.0;
        let mut far = reach;
        for axis in 0..3 {
            let origin = dot(delta, self.axes[axis]);
            let direction = dot(direction, self.axes[axis]);
            let half = self.size[axis] * 0.5;
            if direction.abs() < 1e-9 {
                if origin.abs() > half {
                    return None;
                }
            } else {
                let a = (-half - origin) / direction;
                let b = (half - origin) / direction;
                near = near.max(a.min(b));
                far = far.min(a.max(b));
                if near > far {
                    return None;
                }
            }
        }
        Some(near)
    }
}

pub struct Hitboxes {
    boxes: [Hitbox; CAPACITY],
    count: usize,
    origin: [f64; 3],
    scale: [f64; 3],
    axes: [[f64; 3]; 3],
    pivot: [f64; 3],
}
impl Hitboxes {
    pub fn iter(&self) -> impl Iterator<Item = &Hitbox> {
        self.boxes[..self.count].iter()
    }
    pub fn ray_distance(&self, origin: [f64; 3], direction: [f64; 3], reach: f64) -> Option<f64> {
        self.iter()
            .filter_map(|b| b.ray_distance(origin, direction, reach))
            .min_by(f64::total_cmp)
    }
    fn add(&mut self, center: [f64; 3], size: [f64; 3]) {
        self.oriented(center, size, IDENTITY);
    }
    fn oriented(&mut self, center: [f64; 3], size: [f64; 3], axes: [[f64; 3]; 3]) {
        assert!(self.count < CAPACITY);
        let delta = transform(
            self.axes,
            std::array::from_fn(|i| center[i] * self.scale[i] - self.pivot[i]),
        );
        // Scale each axis before normalizing: custom species dimensions may
        // be nonuniform, but the segment envelopes must remain orthogonal.
        let scaled_y = std::array::from_fn(|i| axes[1][i] * self.scale[i]);
        let length_y = dot(scaled_y, scaled_y).sqrt();
        let local_y = scaled_y.map(|v| v / length_y);
        let (local_x, local_z) = perpendicular(local_y);
        let segment = axes != IDENTITY;
        let local = if segment {
            [local_x, local_y, local_z]
        } else {
            axes
        };
        let size = if segment {
            let radius_scale = self.scale.into_iter().fold(0.0_f64, f64::max);
            [
                size[0] * radius_scale,
                size[1] * length_y,
                size[2] * radius_scale,
            ]
        } else {
            std::array::from_fn(|i| size[i] * self.scale[i])
        };
        self.boxes[self.count] = Hitbox {
            center: std::array::from_fn(|i| self.origin[i] + delta[i]),
            size,
            axes: local.map(|axis| transform(self.axes, axis)),
        };
        self.count += 1;
    }
    fn segment(&mut self, start: [f64; 3], end: [f64; 3], width: f64) {
        let delta = std::array::from_fn(|i| end[i] - start[i]);
        let length = dot(delta, delta).sqrt();
        let y = delta.map(|v| v / length);
        let (x, z) = perpendicular(y);
        self.oriented(
            std::array::from_fn(|i| (start[i] + end[i]) * 0.5),
            [width, length + width * 0.25, width],
            [x, y, z],
        );
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn perpendicular(y: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let x = cross(
        if y[1].abs() < 0.9 {
            [0.0, 1.0, 0.0]
        } else {
            [0.0, 0.0, 1.0]
        },
        y,
    );
    let length = dot(x, x).sqrt();
    let x = x.map(|v| v / length);
    (x, cross(x, y))
}
fn transform(axes: [[f64; 3]; 3], point: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| (0..3).map(|axis| axes[axis][i] * point[axis]).sum())
}

/// Dimensions of the shared authored meshes, before configurable species scale.
pub fn model_reference_size(kind: Kind) -> [f64; 3] {
    match kind {
        Kind::Sheep => [0.85, 1.25, 1.2],
        Kind::Cow => [0.95, 1.65, 1.5],
        Kind::Pig => [0.8, 1.0, 1.15],
        Kind::Fish => [0.3, 0.35, 0.7],
        Kind::Bee => [0.095, 0.095, 0.115],
        // Keep the original sculpture reference: changing the authored mob
        // size then scales both rendered wings and anatomical targets by 50%.
        Kind::Butterfly => [0.22, 0.22, 0.30],
        Kind::Bear => [1.0, 1.3, 1.8],
        Kind::Horse => [0.595, 1.95, 2.15],
        Kind::Squid => [0.95, 0.95, 2.0],
        Kind::MoonJelly
        | Kind::FireJelly
        | Kind::VoidJelly
        | Kind::SunJelly
        | Kind::CrystalJelly => [0.35, 0.95, 0.35],
        _ => kind.size(),
    }
}

pub fn for_creature(a: &Creature) -> Hitboxes {
    let reference = model_reference_size(a.kind);
    let maximum = a.kind.size();
    let individual = a.aquatic_scale();
    let scale = std::array::from_fn(|i| {
        maximum[i] / reference[i] * f64::from(individual[i]) * a.growth_scale()
    });
    let (sin, cos) = f64::from(a.yaw).sin_cos();
    let yaw = [[cos, 0.0, -sin], [0.0, 1.0, 0.0], [sin, 0.0, cos]];
    let attachment = if a.kind == Kind::Villager && a.behavior == CreatureBehavior::Sleep {
        [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]]
    } else if a.kind.is_spider() {
        match a.behavior {
            CreatureBehavior::Climb => [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]],
            CreatureBehavior::Ceiling => [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]],
            _ => IDENTITY,
        }
    } else {
        IDENTITY
    };
    let mut result = Hitboxes {
        boxes: [Hitbox::EMPTY; CAPACITY],
        count: 0,
        origin: if a.kind.is_spider() {
            super::spiders::visual_center(a)
        } else {
            a.position
        },
        scale,
        axes: attachment.map(|axis| transform(yaw, axis)),
        pivot: if a.kind.is_spider() {
            super::spiders::model_pivot(a.kind).map(|v| v * a.growth_scale())
        } else {
            [0.0; 3]
        },
    };
    match a.kind {
        Kind::DarkDragon | Kind::EarthDragon | Kind::WaterDragon => dragon::add(&mut result, a),
        Kind::SandWorm => {
            if matches!(
                a.behavior,
                CreatureBehavior::Burrowed | CreatureBehavior::Tremor
            ) {
                // Underground anatomy is not a visible or hittable surface decoration.
            } else {
                for index in 0..=super::sandworm::HEAD_SEGMENT {
                    let pose = super::sandworm::pose(a, index);
                    if !super::sandworm::anatomy_visible(a.behavior, pose, index) {
                        continue;
                    }
                    let (sin, cos) = f64::from(pose.pitch).sin_cos();
                    let axes = [[1.0, 0.0, 0.0], [0.0, cos, sin], [0.0, -sin, cos]];
                    result.oriented(
                        super::sandworm::target_center(pose, index).map(f64::from),
                        super::sandworm::target_size(index).map(f64::from),
                        axes,
                    );
                }
            }
        }

        Kind::Butterfly => {
            result.add([0.0, 0.14, -0.02], [0.14, 0.25, 0.30]);
            for side in [-1.0, 1.0] {
                result.add([side * 0.25, 0.14, 0.0], [0.45, 0.12, 0.52]);
            }
        }
        Kind::Skeleton => {
            result.add([0.0, 1.20, 0.0], [0.47, 0.46, 0.34]);
            result.add([0.0, 1.69, 0.0], [0.48, 0.47, 0.42]);
            result.add([0.0, 0.83, 0.0], [0.43, 0.22, 0.25]);
            for side in [-1.0, 1.0] {
                result.segment([side * 0.29, 1.39, 0.0], [side * 0.335, 0.68, -0.05], 0.17);
                result.segment([side * 0.125, 0.74, 0.0], [side * 0.16, 0.05, -0.04], 0.17);
            }
        }
        Kind::BloodStalker => {
            result.add([0.0, 0.50, 0.22], [0.61, 0.42, 0.72]);
            result.add([0.0, 0.50, -0.25], [0.48, 0.30, 0.32]);
            for side in [-1.0, 1.0] {
                for index in 0..4 {
                    let z = -0.23 + f64::from(index) * 0.19;
                    let root = [side * 0.23, 0.48, z];
                    let knee = [side * 0.59, 0.50, z + (f64::from(index) - 1.5) * 0.13];
                    let foot = [side * 0.74, 0.01, z + (f64::from(index) - 1.5) * 0.20];
                    result.segment(root, knee, 0.11);
                    result.segment(knee, foot, 0.09);
                }
            }
        }
        Kind::Raven => {
            result.add([0.0, 0.23, 0.04], [0.30, 0.36, 0.42]);
            result.add([0.0, 0.41, -0.12], [0.22, 0.21, 0.24]);
            for side in [-1.0, 1.0] {
                result.add([side * 0.25, 0.30, 0.12], [0.35, 0.15, 0.40]);
            }
        }
        Kind::SpiritFox => {
            result.add([0.0, 0.45, 0.07], [0.46, 0.53, 0.79]);
            result.add([0.0, 0.79, -0.34], [0.48, 0.40, 0.42]);
            for side in [-1.0, 1.0] {
                for front in [-1.0, 1.0] {
                    result.add([side * 0.16, 0.16, front * 0.26], [0.16, 0.32, 0.22]);
                }
            }
        }
        Kind::Villager => {
            result.add([0.0, 1.13, 0.0], [0.43, 0.54, 0.29]);
            result.add([0.0, 1.57, -0.015], [0.36, 0.37, 0.33]);
            for side in [-1.0, 1.0] {
                result.add([side * 0.30, 1.08, 0.0], [0.18, 0.54, 0.22]);
                result.add([side * 0.12, 0.36, 0.0], [0.21, 0.72, 0.28]);
            }
        }
        Kind::Horse => {
            result.add([0.0, 1.035, 0.0], [0.515, 0.65, 1.30]);
            result.add([0.0, 1.40, -0.58], [0.39, 0.58, 0.43]);
            result.add([0.0, 1.57, -0.65], [0.33, 0.39, 0.39]);
            result.add([0.0, 1.43, -0.94], [0.28, 0.25, 0.39]);
            for side in [-1.0, 1.0] {
                result.add([side * 0.10, 1.815, -0.60], [0.095, 0.24, 0.10]);
                for z in [-0.46, 0.46] {
                    result.add([side * 0.175, 0.34, z], [0.22, 0.68, 0.23]);
                }
            }
        }
        Kind::Sheep | Kind::Cow | Kind::Pig => {
            let (width, height, length, leg, head) = match a.kind {
                Kind::Sheep => (0.85, 0.70, 1.02, 0.42, [0.0, 1.05, -0.56]),
                Kind::Cow => (0.85, 0.76, 1.30, 0.62, [0.0, 1.23, -0.75]),
                _ => (0.72, 0.55, 0.98, 0.32, [0.0, 0.74, -0.56]),
            };
            result.add([0.0, leg + height * 0.5, 0.0], [width, height, length]);
            result.add(head, [0.52, 0.53, 0.59]);
            for x in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    result.add(
                        [x * width * 0.34, leg * 0.5, z * length * 0.32],
                        [0.20, leg, 0.23],
                    );
                }
            }
        }
        Kind::Bear => {
            result.add([0.0, 0.79, 0.12], [0.94, 0.88, 1.36]);
            result.add([0.0, 0.94, -0.65], [0.64, 0.52, 0.73]);
            for side in [-1.0, 1.0] {
                for z in [-0.37, 0.48] {
                    result.add([side * 0.31, 0.27, z], [0.29, 0.54, 0.32]);
                }
            }
        }
        Kind::Stalker => {
            result.add([0.0, 1.62, 0.19], [0.99, 1.15, 0.61]);
            result.add([0.0, 2.19, -0.31], [0.64, 0.58, 0.70]);
            for side in [-1.0, 1.0] {
                result.segment([side * 0.51, 2.06, 0.03], [side * 0.80, 1.17, -0.16], 0.34);
                result.segment([side * 0.80, 1.17, -0.16], [side * 1.01, 0.52, -0.45], 0.25);
                result.add([side * 1.04, 0.30, -0.53], [0.42, 0.48, 0.35]);
                result.segment([side * 0.29, 1.24, 0.13], [side * 0.46, 0.62, 0.34], 0.30);
                result.segment([side * 0.46, 0.62, 0.34], [side * 0.38, 0.10, -0.10], 0.23);
            }
        }
        Kind::SmallSpider | Kind::MediumSpider | Kind::LargeSpider => {
            let [w, h, l] = maximum;
            result.add([0.0, h * 0.85, 0.0], [w, h, l]);
            result.add([0.0, h * 0.66, -l * 0.50], [w * 0.65, h * 0.32, l * 0.46]);
            for side in [-1.0, 1.0] {
                for index in 0..4 {
                    let fan = (f64::from(index) - 1.5) * l * 0.20;
                    let origin = [
                        side * w * 0.32,
                        h * 0.71,
                        (f64::from(index) - 1.5) * l * 0.19,
                    ];
                    let knee = [
                        origin[0] + side * w * 0.49,
                        origin[1] + h * (0.18 + f64::from(index) * 0.018),
                        origin[2] + fan,
                    ];
                    let foot = [origin[0] + side * w * 0.87, 0.0, origin[2] + fan * 1.60];
                    result.segment(origin, knee, w * 0.13);
                    result.segment(knee, foot, w * 0.13);
                }
            }
        }
        _ => result.add([0.0, reference[1] * 0.5, 0.0], reference),
    }
    result
}

#[cfg(test)]
mod tests;
