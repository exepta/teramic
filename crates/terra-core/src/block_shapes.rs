//! Authored model volumes, exported alongside render geometry from the same boxes.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShapeBox {
    pub minimum: [f64; 3],
    pub maximum: [f64; 3],
    #[serde(default)]
    pub hinged: bool,
}
impl ShapeBox {
    /// Authored unit-cell boxes use the same clockwise cardinal yaw everywhere,
    /// including pure world/persistence builds without the runtime ECS.
    pub fn rotated_y(self, facing: u8) -> Self {
        let [x, y, z] = self.minimum;
        let [u, v, w] = self.maximum;
        let (minimum, maximum) = match facing % 4 {
            0 => ([x, y, z], [u, v, w]),
            1 => ([z, y, 1.0 - u], [w, v, 1.0 - x]),
            2 => ([1.0 - u, y, 1.0 - w], [1.0 - x, v, 1.0 - z]),
            _ => ([1.0 - w, y, x], [1.0 - z, v, u]),
        };
        Self {
            minimum,
            maximum,
            ..self
        }
    }

    pub fn valid(&self) -> bool {
        self.valid_for(0)
    }
    pub fn valid_for(&self, material: u16) -> bool {
        (0..3).all(|axis| {
            self.minimum[axis].is_finite()
                && self.maximum[axis].is_finite()
                && self.minimum[axis] >= -0.01
                && self.maximum[axis]
                    <= if material == 99 && axis == 1 {
                        1.25
                    } else if matches!(material, 59 | 65..=67 | 139) && axis == 1
                        || matches!(material, 71..=73) && axis == 2
                    {
                        2.01
                    } else {
                        1.01
                    }
                && self.minimum[axis] < self.maximum[axis]
        })
    }
    pub fn intersects(&self, low: [f64; 3], high: [f64; 3]) -> bool {
        (0..3).all(|axis| self.minimum[axis] < high[axis] && self.maximum[axis] > low[axis])
    }
    pub fn ray(
        &self,
        origin: [f64; 3],
        direction: [f64; 3],
        reach: f64,
    ) -> Option<(f64, [i32; 3])> {
        let mut near: f64 = 0.0;
        let mut far = reach;
        let mut normal = [0; 3];
        for axis in 0..3 {
            if direction[axis].abs() < 1.0e-12 {
                if origin[axis] < self.minimum[axis] || origin[axis] > self.maximum[axis] {
                    return None;
                }
                continue;
            }
            let a = (self.minimum[axis] - origin[axis]) / direction[axis];
            let b = (self.maximum[axis] - origin[axis]) / direction[axis];
            if a.min(b) > near {
                near = a.min(b);
                normal = [0; 3];
                normal[axis] = if direction[axis] > 0.0 { -1 } else { 1 };
            }
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
        (near <= reach && far >= 0.0).then_some((near, normal))
    }
}

pub fn builtin_boxes(material: u16) -> &'static [ShapeBox] {
    if material == 139 {
        return builtin_boxes(65);
    }
    if material == 140 {
        return builtin_boxes(68);
    }
    static SNOW: [ShapeBox; 1] = [ShapeBox {
        minimum: [0.0; 3],
        maximum: [1.0, 0.125, 1.0],
        hinged: false,
    }];
    static PATH: [ShapeBox; 1] = [ShapeBox {
        minimum: [0.0; 3],
        maximum: [1.0, 0.9375, 1.0],
        hinged: false,
    }];
    if material == 102 {
        return &PATH;
    }
    if material == 98 {
        return &SNOW;
    }
    if crate::textiles::is_carpet(material) {
        return &SNOW;
    }
    if crate::construction::is_shape(material) {
        return crate::construction::boxes(material);
    }
    use std::sync::OnceLock;
    #[derive(Deserialize)]
    struct Definition {
        collision_boxes: Vec<ShapeBox>,
    }
    fn read(source: &str, material: u16) -> Vec<ShapeBox> {
        serde_json::from_str::<Definition>(source)
            .ok()
            .map(|d| d.collision_boxes)
            .filter(|boxes| boxes.len() <= 128 && boxes.iter().all(|b| b.valid_for(material)))
            .unwrap_or_default()
    }
    static BENCH: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static CHEST: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static TORCH: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static FURNACE: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static CRUSHER: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static CRANK: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static STRAIGHT: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static ELBOW: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static HORIZONTAL: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static BEND_UP: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static BEND_DOWN: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static OAK_DOOR: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static BIRCH_DOOR: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static SPRUCE_DOOR: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static OAK_WINDOW: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static BIRCH_WINDOW: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static SPRUCE_WINDOW: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static BED: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    static POT: OnceLock<Vec<ShapeBox>> = OnceLock::new();
    match material {
        131 => POT.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/flower_pot.json"),
                131,
            )
        }),
        100 => CRANK.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/hand_crank.json"),
                100,
            )
        }),
        99 => CRUSHER.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/crusher.json"),
                99,
            )
        }),
        71..=73 => BED.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/oak_bed.json"),
                71,
            )
        }),
        28 => BENCH.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/workbench.json"),
                28,
            )
        }),
        29 => {
            CHEST.get_or_init(|| read(include_str!("../../../assets/terra/blocks/chest.json"), 29))
        }
        40 => {
            TORCH.get_or_init(|| read(include_str!("../../../assets/terra/blocks/torch.json"), 40))
        }
        59 => FURNACE.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/furnace.json"),
                59,
            )
        }),
        60 => STRAIGHT.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/chimney_straight.json"),
                60,
            )
        }),
        61 => ELBOW.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/chimney_elbow.json"),
                61,
            )
        }),
        62 => HORIZONTAL.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/chimney_horizontal.json"),
                62,
            )
        }),
        63 => BEND_UP.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/chimney_bend_up.json"),
                63,
            )
        }),
        64 => BEND_DOWN.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/chimney_bend_down.json"),
                64,
            )
        }),
        65 => OAK_DOOR.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/oak_door.json"),
                65,
            )
        }),
        66 => BIRCH_DOOR.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/birch_door.json"),
                66,
            )
        }),
        67 => SPRUCE_DOOR.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/spruce_door.json"),
                67,
            )
        }),
        68 => OAK_WINDOW.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/oak_window.json"),
                68,
            )
        }),
        69 => BIRCH_WINDOW.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/birch_window.json"),
                69,
            )
        }),
        70 => SPRUCE_WINDOW.get_or_init(|| {
            read(
                include_str!("../../../assets/terra/blocks/spruce_window.json"),
                70,
            )
        }),
        _ => &[],
    }
}

/// Leaf pose shared with the visual hinge. World orientation is applied afterwards.
pub fn hinge_point([x, y, z]: [f64; 3], openness: f64) -> [f64; 3] {
    let (s, c) = (-std::f64::consts::FRAC_PI_2 * openness.clamp(0.0, 1.0)).sin_cos();
    [
        0.15 + (x - 0.15) * c + (z - 0.5) * s,
        y,
        0.5 - (x - 0.15) * s + (z - 0.5) * c,
    ]
}

/// Shared authored-model pose. Mounts 4..7 face out from a supporting wall;
/// the persisted object layout remains compatible with earlier furniture states.
pub fn torch_point(facing: u8, point: [f64; 3]) -> [f64; 3] {
    if facing < 4 {
        return point;
    }
    let v = torch_vector(facing, [point[0] - 0.5, point[1], point[2] - 0.5]);
    let n = torch_normal(facing);
    [
        v[0] + 0.5 - n[0] * 0.455,
        v[1] + 0.13,
        v[2] + 0.5 - n[2] * 0.455,
    ]
}
pub fn torch_normal(facing: u8) -> [f64; 3] {
    match facing {
        4 => [1.0, 0.0, 0.0],
        5 => [-1.0, 0.0, 0.0],
        6 => [0.0, 0.0, 1.0],
        7 => [0.0, 0.0, -1.0],
        _ => [0.0, 1.0, 0.0],
    }
}
pub fn torch_vector(facing: u8, [x, y, z]: [f64; 3]) -> [f64; 3] {
    if facing < 4 {
        return [x, y, z];
    }
    let n = torch_normal(facing);
    let (s, c) = 25.0_f64.to_radians().sin_cos();
    let outward = x * c + y * s;
    [
        n[0] * outward + n[2] * z,
        y * c - x * s,
        n[2] * outward - n[0] * z,
    ]
}

#[cfg(test)]
mod tests;
