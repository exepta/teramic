//! Stable material identities for the authored castle construction family.
pub mod connections;
pub const BERGFRIED: u16 = 173;
pub const STONE_WALL: u16 = 174;
pub const BERGFRIED_WALL: u16 = 175;
pub const CRACKED_WALL: u16 = 176;
pub const OVERGROWN_WALL: u16 = 177;
pub const MOSS_WALL: u16 = 178;
pub const IRON_WALL: u16 = 179;
pub const BATTLEMENT_STRAIGHT: u16 = 180;
pub const BATTLEMENT_CORNER: u16 = 181;
pub const BATTLEMENT_PILLAR: u16 = 182;
pub const ARCH_LEFT: u16 = 183;
pub const ARCH_RIGHT: u16 = 184;
pub const ARROW_SLIT: u16 = 185;
pub const WINDOW: u16 = 186;
pub const SANDSTONE_WALL: u16 = 187;
pub const DESERT_WALL: u16 = 188;
pub const SNOW_WALL: u16 = 189;
pub const BLACKSTONE_WALL: u16 = 190;
pub const ROYAL_WALL: u16 = 191;
pub const TIMBER_WALL: u16 = 192;
pub const FIRST: u16 = BERGFRIED;
pub const LAST: u16 = TIMBER_WALL;
pub const BLOCKS: [u16; 20] = [
    BERGFRIED,
    STONE_WALL,
    BERGFRIED_WALL,
    CRACKED_WALL,
    OVERGROWN_WALL,
    MOSS_WALL,
    IRON_WALL,
    BATTLEMENT_STRAIGHT,
    BATTLEMENT_CORNER,
    BATTLEMENT_PILLAR,
    ARCH_LEFT,
    ARCH_RIGHT,
    ARROW_SLIT,
    WINDOW,
    SANDSTONE_WALL,
    DESERT_WALL,
    SNOW_WALL,
    BLACKSTONE_WALL,
    ROYAL_WALL,
    TIMBER_WALL,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastleShape {
    Keep,
    Wall,
    Crown,
    BattlementStraight,
    BattlementCorner,
    BattlementPillar,
    ArchLeft,
    ArchRight,
    ArrowSlit,
    Window,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastleFinish {
    Stone,
    Cracked,
    Overgrown,
    Moss,
    Iron,
    Sandstone,
    Desert,
    Snow,
    Blackstone,
    Royal,
    Timber,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CastleBlock {
    pub shape: CastleShape,
    pub finish: CastleFinish,
}

pub const fn is_castle(material: u16) -> bool {
    material >= FIRST && material <= LAST
}

/// ID181 remains readable in worlds and carried legacy stacks, but the adaptive
/// wall now supplies its corners without a second craftable/catalog item.
pub const fn retired_item(material: u16) -> bool {
    material == BATTLEMENT_CORNER
}
pub const fn item_material(material: u16) -> u16 {
    if retired_item(material) {
        BATTLEMENT_STRAIGHT
    } else {
        material
    }
}

/// Profiles with a wall plane that should continue unchanged when their edge is clicked.
pub const fn has_wall_plane(material: u16) -> bool {
    matches!(
        material,
        BERGFRIED_WALL
            | BATTLEMENT_STRAIGHT
            | BATTLEMENT_CORNER
            | BATTLEMENT_PILLAR
            | ARCH_LEFT
            | ARCH_RIGHT
            | ARROW_SLIT
            | WINDOW
    )
}

/// Keep walls upright on vertical supports. Unlike a stair's tread, a wall's
/// front, not its bottom, meets the clicked wall. Floor/ceiling placement uses
/// the view direction, with a stable yaw when looking straight up or down.
pub fn placement_facing(normal: [i8; 3], direction: [f64; 3]) -> u8 {
    let group = if normal == [0, -1, 0] { 4 } else { 0 };
    let target = if normal[1] == 0 && normal != [0; 3] {
        normal.map(f64::from)
    } else {
        direction
    };
    let mut best = group;
    let mut score = f64::NEG_INFINITY;
    for facing in group..group + 4 {
        let dot = crate::construction::stair_basis(facing)[2]
            .into_iter()
            .zip(target)
            .map(|(axis, target)| f64::from(axis) * target)
            .sum::<f64>();
        if dot > score + 0.000001 {
            best = facing;
            score = dot;
        }
    }
    best
}

/// Twenty-four proper rotations, identical to the existing stair placement basis.
pub fn point(facing: u8, point: [f64; 3]) -> [f64; 3] {
    let basis = crate::construction::stair_basis(facing);
    std::array::from_fn(|axis| {
        0.5 + (0..3)
            .map(|i| (point[i] - 0.5) * f64::from(basis[i][axis]))
            .sum::<f64>()
    })
}

pub fn rotated_box(
    source: crate::block_shapes::ShapeBox,
    facing: u8,
) -> crate::block_shapes::ShapeBox {
    let mut result = crate::block_shapes::ShapeBox {
        minimum: [f64::INFINITY; 3],
        maximum: [f64::NEG_INFINITY; 3],
        hinged: false,
    };
    for corner in 0..8 {
        let p = point(
            facing,
            std::array::from_fn(|axis| {
                if corner & (1 << axis) == 0 {
                    source.minimum[axis]
                } else {
                    source.maximum[axis]
                }
            }),
        );
        for (axis, coordinate) in p.into_iter().enumerate() {
            result.minimum[axis] = result.minimum[axis].min(coordinate);
            result.maximum[axis] = result.maximum[axis].max(coordinate);
        }
    }
    result
}

pub fn boxes(material: u16) -> &'static [crate::block_shapes::ShapeBox] {
    use std::sync::OnceLock;
    if !is_castle(material) {
        return &[];
    }
    static BOXES: [OnceLock<Vec<crate::block_shapes::ShapeBox>>; 20] =
        [const { OnceLock::new() }; 20];
    const SOURCES: [&str; 20] = [
        include_str!("../../../assets/terra/blocks/bergfried.json"),
        include_str!("../../../assets/terra/blocks/castle_stone_wall.json"),
        include_str!("../../../assets/terra/blocks/bergfried_wall.json"),
        include_str!("../../../assets/terra/blocks/castle_cracked_wall.json"),
        include_str!("../../../assets/terra/blocks/castle_overgrown_wall.json"),
        include_str!("../../../assets/terra/blocks/castle_moss_wall.json"),
        include_str!("../../../assets/terra/blocks/castle_iron_wall.json"),
        include_str!("../../../assets/terra/blocks/castle_battlement_straight.json"),
        include_str!("../../../assets/terra/blocks/castle_battlement_corner.json"),
        include_str!("../../../assets/terra/blocks/castle_battlement_pillar.json"),
        include_str!("../../../assets/terra/blocks/castle_arch_left.json"),
        include_str!("../../../assets/terra/blocks/castle_arch_right.json"),
        include_str!("../../../assets/terra/blocks/castle_arrow_slit.json"),
        include_str!("../../../assets/terra/blocks/castle_window.json"),
        include_str!("../../../assets/terra/blocks/castle_sandstone_wall.json"),
        include_str!("../../../assets/terra/blocks/castle_desert_wall.json"),
        include_str!("../../../assets/terra/blocks/castle_snow_wall.json"),
        include_str!("../../../assets/terra/blocks/castle_blackstone_wall.json"),
        include_str!("../../../assets/terra/blocks/castle_royal_wall.json"),
        include_str!("../../../assets/terra/blocks/castle_timber_wall.json"),
    ];
    #[derive(serde::Deserialize)]
    struct Definition {
        id: u16,
        collision_boxes: Vec<crate::block_shapes::ShapeBox>,
    }
    let index = usize::from(material - FIRST);
    BOXES[index].get_or_init(|| {
        let definition: Definition =
            serde_json::from_str(SOURCES[index]).expect("Embedded castle definition");
        assert_eq!(definition.id, material);
        assert!(!definition.collision_boxes.is_empty() && definition.collision_boxes.len() <= 128);
        assert!(
            definition
                .collision_boxes
                .iter()
                .all(|bounds| bounds.valid_for(material))
        );
        definition.collision_boxes
    })
}

pub const fn definition(material: u16) -> Option<CastleBlock> {
    use CastleFinish::*;
    use CastleShape::*;
    let (shape, finish) = match material {
        BERGFRIED => (Keep, Royal),
        STONE_WALL => (Wall, Stone),
        BERGFRIED_WALL => (Crown, Stone),
        CRACKED_WALL => (Wall, Cracked),
        OVERGROWN_WALL => (Wall, Overgrown),
        MOSS_WALL => (Wall, Moss),
        IRON_WALL => (Wall, Iron),
        BATTLEMENT_STRAIGHT => (BattlementStraight, Stone),
        BATTLEMENT_CORNER => (BattlementCorner, Stone),
        BATTLEMENT_PILLAR => (BattlementPillar, Stone),
        ARCH_LEFT => (ArchLeft, Stone),
        ARCH_RIGHT => (ArchRight, Stone),
        ARROW_SLIT => (ArrowSlit, Stone),
        WINDOW => (Window, Stone),
        SANDSTONE_WALL => (Wall, Sandstone),
        DESERT_WALL => (Wall, Desert),
        SNOW_WALL => (Wall, Snow),
        BLACKSTONE_WALL => (Wall, Blackstone),
        ROYAL_WALL => (Wall, Royal),
        TIMBER_WALL => (Wall, Timber),
        _ => return None,
    };
    Some(CastleBlock { shape, finish })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn castle_identities_and_embedded_mechanics_are_complete_and_stable() {
        assert_eq!(BERGFRIED, terra_voxel::MaterialId::Bergfried as u16);
        assert_eq!(terra_voxel::MaterialId::GlassPipe as u16, 172);
        for material in BLOCKS {
            assert!(definition(material).is_some());
            let id = terra_voxel::MaterialId::from_raw(material).unwrap();
            assert!(id.is_solid() && !id.is_opaque());
            assert!(!boxes(material).is_empty());
            assert!(
                boxes(material)
                    .iter()
                    .all(|bounds| bounds.valid_for(material))
            );
        }
    }

    #[test]
    fn castle_side_placement_faces_the_support_and_keeps_the_stone_upright() {
        for normal in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            for direction in [
                [0.0, 0.0, 1.0],
                [1.0, -0.3, 0.0],
                [0.0, 0.9, -1.0],
                [-1.0, 0.0, 0.0],
            ] {
                let basis = crate::construction::stair_basis(placement_facing(normal, direction));
                assert_eq!(basis[1], [0, 1, 0]);
                assert_eq!(basis[2], normal);
            }
        }
    }

    #[test]
    fn castle_floor_and_ceiling_placement_have_stable_view_relative_yaw() {
        for normal in [[0, 1, 0], [0, -1, 0]] {
            for direction in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
                let facing = placement_facing(normal, direction.map(f64::from));
                let basis = crate::construction::stair_basis(facing);
                assert_eq!(basis[1], normal);
                assert_eq!(basis[2], direction);
            }
        }
        assert_eq!(placement_facing([0, 1, 0], [0.0, -1.0, 0.0]), 0);
        assert_eq!(placement_facing([0, -1, 0], [0.0, 1.0, 0.0]), 4);
    }

    #[test]
    fn arch_halves_are_full_depth_and_keep_their_actual_opening_in_all_stair_poses() {
        for (material, opening_x, pier_x) in [(ARCH_LEFT, 0.90, 0.10), (ARCH_RIGHT, 0.10, 0.90)] {
            let source = boxes(material);
            assert!(
                source
                    .iter()
                    .all(|bounds| bounds.minimum[2] == 0.0 && bounds.maximum[2] == 1.0)
            );
            for facing in 0..24 {
                let ray = |x| {
                    let origin = point(facing, [x, 0.45, -1.0]);
                    let end = point(facing, [x, 0.45, 0.0]);
                    let direction = std::array::from_fn(|axis| end[axis] - origin[axis]);
                    source.iter().any(|bounds| {
                        rotated_box(*bounds, facing)
                            .ray(origin, direction, 3.0)
                            .is_some()
                    })
                };
                assert!(!ray(opening_x), "arch opening blocked: {material}/{facing}");
                assert!(ray(pier_x), "arch pier missing: {material}/{facing}");
            }
        }
    }
}
