//! F68 road identities and the shared authored collision profiles.
//!
//! Covers, ramps and curbs keep their piece identity when mined. Reversible
//! workbench recipes recover their exact material volumes. All profiles remain
//! upright and rotate around Y with the same facing convention as other blocks.
use crate::block_shapes::ShapeBox;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

pub const ASPHALT: u16 = 431;
pub const COVER: u16 = 432;
pub const SLOPE: u16 = 433;
pub const ENTRY: u16 = 434;
pub const EXIT: u16 = 435;
pub const CURB: u16 = 436;
/// A path with a finite, drying tar film. Its state stores volume and drying age.
pub const WET_TAR: u16 = 437;
pub const DRIED_TAR: u16 = 438;
pub const TAR_MATERIALS: [u16; 2] = [WET_TAR, DRIED_TAR];
pub const TAR_BUCKET_UNITS: u8 = 8;
pub const TAR_FINE_UNITS: u8 = 64;
pub const TAR_RANGE: u64 = 4;

/// Equivalent liquid depth for a particle simulation over a whole path cell.
pub fn tar_fine_height(volume: u8) -> f32 {
    PATH_HEIGHT + f32::from(volume.clamp(1, 64)) / 1024.0
}
pub const PATH: u16 = 102;
pub const PATH_HEIGHT: f32 = 15.0 / 16.0;

pub const fn is_tar(material: u16) -> bool {
    matches!(material, WET_TAR | DRIED_TAR)
}

/// Low three bits encode 1..8 conserved units, upper five the drying stage.
pub const fn tar_amount(state: u8) -> u8 {
    (state & 7) + 1
}

pub const fn tar_stage(state: u8) -> u8 {
    state >> 3
}

pub const fn tar_state(amount: u8, stage: u8) -> u8 {
    let amount = if amount < 1 {
        1
    } else if amount > 8 {
        8
    } else {
        amount
    };
    let stage = if stage > 31 { 31 } else { stage };
    (stage << 3) | (amount - 1)
}

/// Height of the covered portion of a path for any encoded volume or drying stage.
pub const fn tar_height(_state: u8) -> f32 {
    1.0
}

/// The paid volume controls how far a wet front has travelled. Covered pavement
/// meets the surrounding full blocks at grade, including once it cures.
pub fn tar_coating(
    material: u16,
    volume: u8,
    mut neighbor: impl FnMut(usize) -> (Option<u16>, u8),
) -> ShapeBox {
    let mut film = ShapeBox {
        minimum: [0., f64::from(PATH_HEIGHT), 0.],
        maximum: [1.; 3],
        hinged: false,
    };
    if material == WET_TAR {
        let incoming = [0, 1, 4, 5]
            .into_iter()
            .filter_map(|side| {
                let (id, head) = neighbor(side);
                (id == Some(WET_TAR)).then_some((side, head))
            })
            .max_by_key(|&(_, head)| head);
        if let Some((side, head)) = incoming
            && neighbor(side ^ 1).0 == Some(PATH)
            && let Some(front) = crate::fluid_surface::Front::new(
                f32::from(volume) / 64.,
                f32::from(head) / 64.,
                usize::from(side >= 4),
                side % 2 == 0,
            )
        {
            let axis = side / 2;
            if front.positive {
                film.maximum[axis] = f64::from(front.length);
            } else {
                film.minimum[axis] = 1. - f64::from(front.length);
            }
        }
    }
    film
}

/// Natural grass, sand, gravel and earth, including regional soil variants.
pub const fn is_path_base(material: u16) -> bool {
    matches!(material, 1 | 2 | 4 | 42 | 306 | 307 | 400 | 401)
}
pub const MATERIALS: [u16; 6] = [ASPHALT, COVER, SLOPE, ENTRY, EXIT, CURB];

pub const fn is_shape(material: u16) -> bool {
    matches!(material, COVER..=CURB)
}

/// Ramps rise along the horizontal look direction, using the stair yaw
/// convention. Curbs are authored along X, so turn their length onto that same
/// road direction rather than placing a crossbar across the player's path.
pub fn placement_facing(material: u16, direction: [f64; 3]) -> u8 {
    let forward = (0..4)
        .max_by(|&a, &b| {
            let dot = |facing| {
                crate::construction::stair_basis(facing)[2]
                    .into_iter()
                    .zip(direction)
                    .map(|(axis, look)| f64::from(axis) * look)
                    .sum::<f64>()
            };
            dot(a).total_cmp(&dot(b))
        })
        .unwrap_or(0);
    if material == CURB {
        (forward + 1) % 4
    } else {
        forward
    }
}

pub fn boxes(material: u16) -> &'static [ShapeBox] {
    #[derive(Deserialize)]
    struct Definition {
        id: u16,
        collision_boxes: Vec<ShapeBox>,
    }
    static PROFILES: OnceLock<BTreeMap<u16, Vec<ShapeBox>>> = OnceLock::new();
    PROFILES
        .get_or_init(|| {
            [
                include_str!("../../../assets/terra/blocks/asphalt_cover.json"),
                include_str!("../../../assets/terra/blocks/asphalt_slope.json"),
                include_str!("../../../assets/terra/blocks/asphalt_entry.json"),
                include_str!("../../../assets/terra/blocks/asphalt_exit.json"),
                include_str!("../../../assets/terra/blocks/road_curb.json"),
            ]
            .into_iter()
            .map(|source| {
                let definition: Definition =
                    serde_json::from_str(source).expect("valid builtin road profile");
                assert!(
                    definition
                        .collision_boxes
                        .iter()
                        .all(|bounds| bounds.valid_for(definition.id))
                );
                (definition.id, definition.collision_boxes)
            })
            .collect()
        })
        .get(&material)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tar_states_preserve_every_volume_and_drying_stage_without_hovering() {
        for amount in 1..=8 {
            for stage in 0..=31 {
                let state = tar_state(amount, stage);
                assert_eq!(tar_amount(state), amount);
                assert_eq!(tar_stage(state), stage);
                assert_eq!(
                    tar_coating(DRIED_TAR, amount * 8, |_| (None, 0)).maximum,
                    [1.; 3]
                );
            }
        }
        assert_eq!(tar_state(0, 0), tar_state(1, 0));
        assert_eq!(tar_state(255, 255), tar_state(8, 31));
        for material in [1, 2, 4, 42, 306, 307, 400, 401] {
            assert!(is_path_base(material));
        }
        for material in [3, 5, 27, 49, 51, 101, 102, ASPHALT, WET_TAR, DRIED_TAR] {
            assert!(!is_path_base(material));
        }
    }

    #[test]
    fn tar_bucket_uses_paid_binder_and_an_empty_vessel_without_reordering_tools() {
        use crate::crafting::{CraftDestination, CraftError, CraftStation, RecipeBook, recipe_key};
        use crate::{Inventory, ItemKind, ToolKind};
        let book = RecipeBook::builtins();
        assert_eq!(book.error, None);
        let key = recipe_key("teramic::tar_bucket");
        let item = ItemKind::Tool(ToolKind::TarBucket);
        assert_eq!(ToolKind::GoldSpoon as u8, 40);
        assert_eq!(ToolKind::TarBucket as u8, 41);
        assert_eq!(
            serde_json::to_string(&item).unwrap(),
            r#"{"tool":"tar_bucket"}"#
        );
        assert_eq!(item.max_stack(), 1);
        let mut bag = Inventory::default();
        bag.insert(ItemKind::Bitumen, 4);
        assert_eq!(
            book.craft(
                &mut bag,
                key,
                1,
                CraftStation::Workbench,
                CraftDestination::Inventory
            ),
            Err(CraftError::MissingIngredients)
        );
        bag.insert(ItemKind::Tool(ToolKind::Bucket), 1);
        book.craft(
            &mut bag,
            key,
            1,
            CraftStation::Workbench,
            CraftDestination::Inventory,
        )
        .unwrap();
        assert_eq!(bag.count(item), 1);
        assert_eq!(bag.count(ItemKind::Bitumen), 0);
        assert_eq!(bag.count(ItemKind::Tool(ToolKind::Bucket)), 0);
        let roundtrip: Inventory =
            serde_json::from_str(&serde_json::to_string(&bag).unwrap()).unwrap();
        assert_eq!(roundtrip, bag);
    }

    #[test]
    fn ramp_placement_uses_stair_heading_and_curbs_run_along_it() {
        for direction in [
            [1.0, -0.4, 0.0],
            [-1.0, -0.4, 0.0],
            [0.0, -0.4, 1.0],
            [0.0, -0.4, -1.0],
            [1.0, -0.4, 1.0],
            [-1.0, -0.4, -1.0],
        ] {
            let stair = crate::construction::placement_facing(77, [0, 1, 0], direction);
            for material in [SLOPE, ENTRY, EXIT] {
                assert_eq!(placement_facing(material, direction), stair);
                for normal in [
                    [0, 1, 0],
                    [0, -1, 0],
                    [1, 0, 0],
                    [-1, 0, 0],
                    [0, 0, 1],
                    [0, 0, -1],
                ] {
                    assert_eq!(
                        crate::construction::placement_facing(material, normal, direction),
                        stair
                    );
                }
            }
            assert_eq!(placement_facing(CURB, direction), (stair + 1) % 4);
        }
    }

    #[test]
    fn survival_machine_chain_crafts_asphalt_and_recovers_each_piece_without_duplication() {
        use crate::crafting::{
            CraftDestination, CraftError, CraftGridCell, CraftStation, RecipeBook, RecipePattern,
            recipe_key,
        };
        use crate::{Inventory, ItemKind, ItemStack, MineralItem, crusher, furnace};
        let stack = |item, count| ItemStack { item, count };
        let mut crushed = Inventory {
            slots: vec![stack(ItemKind::Block(3), 1)],
            ..Default::default()
        };
        let mut crusher_state = crusher::CrusherState::default();
        for _ in 0..4 {
            crusher::advance(&mut crusher_state, &mut crushed, 1.0);
        }
        assert_eq!(
            crushed.slot(crusher::OUTPUT),
            Some(stack(ItemKind::RoadAggregate, 4))
        );
        assert_eq!(crusher_state.completed, 1);

        let mut heated = Inventory {
            slots: vec![
                stack(ItemKind::Dust(crate::DustKind::Coal), 1),
                stack(ItemKind::Mineral(MineralItem::Coal), 1),
                ItemStack::EMPTY,
            ],
            ..Default::default()
        };
        let mut furnace_state = furnace::FurnaceState::default();
        for _ in 0..8 {
            furnace::advance(&mut furnace_state, &mut heated, 1.0, 68);
        }
        assert_eq!(
            heated.slot(furnace::OUTPUT),
            Some(stack(ItemKind::Bitumen, 1))
        );
        assert_eq!(furnace_state.completed, 1);

        let mut player = Inventory::default();
        for output in [
            crushed.slot(crusher::OUTPUT).unwrap(),
            heated.slot(furnace::OUTPUT).unwrap(),
            stack(ItemKind::Block(4), 1),
        ] {
            assert_eq!(player.insert(output.item, output.count), 0);
        }
        let book = RecipeBook::builtins();
        assert!(book.error.is_none(), "{:?}", book.error);
        assert_eq!(
            book.craft(
                &mut player,
                recipe_key("teramic::asphalt"),
                1,
                CraftStation::Hand,
                CraftDestination::Inventory
            ),
            Err(CraftError::WrongStation)
        );
        book.craft(
            &mut player,
            recipe_key("teramic::asphalt"),
            1,
            CraftStation::Workbench,
            CraftDestination::Inventory,
        )
        .unwrap();
        assert_eq!(player.count(ItemKind::Block(ASPHALT)), 4);
        assert_eq!(player.count(ItemKind::RoadAggregate), 0);
        assert_eq!(player.count(ItemKind::Bitumen), 0);
        for (slug, material, base, units, pieces) in [
            ("asphalt_cover", COVER, ASPHALT, 1, 8),
            ("asphalt_slope", SLOPE, ASPHALT, 2, 4),
            ("asphalt_entry", ENTRY, ASPHALT, 5, 16),
            ("asphalt_exit", EXIT, ASPHALT, 3, 4),
            ("road_curb", CURB, 12, 3, 64),
        ] {
            let mut bag = Inventory::default();
            bag.insert(ItemKind::Block(base), units);
            let key = recipe_key(&format!("teramic::{slug}"));
            let RecipePattern::Shapeless { ingredients } = &book.get(key).unwrap().pattern else {
                panic!("road forms use the real shapeless workbench grid");
            };
            let mut cells = [None; 9];
            for (index, ingredient) in ingredients.iter().enumerate() {
                let slot = bag
                    .slots
                    .iter()
                    .position(|stack| stack.item == ingredient.item)
                    .unwrap() as u8;
                cells[index] = Some(CraftGridCell {
                    slot,
                    count: ingredient.count,
                });
            }
            assert_eq!(
                book.grid_match_reserved(&bag, &cells, 3, CraftStation::Workbench)
                    .map(|matched| matched.0),
                Some(key),
                "{slug} must remain distinguishable from every other road form"
            );
            assert!(book.craft_reserved_grid(
                &mut bag,
                &cells,
                3,
                CraftStation::Workbench,
                1,
                CraftDestination::Inventory
            ));
            assert_eq!(bag.count(ItemKind::Block(base)), 0);
            assert_eq!(bag.count(ItemKind::Block(material)), pieces);
            let saved = serde_json::to_string(&bag).unwrap();
            let mut bag: Inventory = serde_json::from_str(&saved).unwrap();
            book.craft(
                &mut bag,
                recipe_key(&format!("teramic::recover_{slug}")),
                1,
                CraftStation::Workbench,
                CraftDestination::Inventory,
            )
            .unwrap();
            assert_eq!(bag.count(ItemKind::Block(base)), units);
            assert_eq!(bag.count(ItemKind::Block(material)), 0);
            assert!(bag.valid());
        }
    }

    #[test]
    fn road_profiles_are_shared_bounded_and_rotatable() {
        for id in MATERIALS {
            let material = terra_voxel::MaterialId::from_raw(id).unwrap();
            assert!(material.is_solid());
            assert_eq!(material.is_opaque(), id == ASPHALT);
            assert!(crate::ItemKind::Block(id).valid());
            if id == ASPHALT {
                continue;
            }
            assert!(crate::construction::is_shape(id));
            assert!(!crate::construction::is_slab(id));
            assert!(!crate::construction::is_stair(id));
            assert!(!boxes(id).is_empty());
            for facing in 0..4 {
                let placed = crate::construction::placed_boxes(id, facing, [None; 6]);
                let expected: Vec<_> = boxes(id).iter().map(|b| b.rotated_y(facing)).collect();
                assert_eq!(placed, expected);
                assert!(placed.iter().all(|b| b.valid_for(id)));
                assert!(crate::construction::valid_facing(id, facing));
            }
            assert!(!crate::construction::valid_facing(id, 4));
        }
    }

    #[test]
    fn road_grades_join_cover_half_height_and_full_height_without_gaps() {
        assert_eq!(boxes(COVER)[0].maximum[1], 0.125);
        for (id, low, high) in [(SLOPE, 0.0, 1.0), (ENTRY, 0.125, 0.5), (EXIT, 0.5, 1.0)] {
            let boxes = boxes(id);
            assert_eq!(boxes.len(), 12);
            assert_eq!(boxes[0].minimum[2], 0.0);
            assert_eq!(boxes[11].maximum[2], 1.0);
            assert_eq!(boxes[11].maximum[1], high);
            assert!(boxes[0].maximum[1] - low <= 1.0 / 12.0 + 1e-9);
            for pair in boxes.windows(2) {
                assert_eq!(pair[0].maximum[2], pair[1].minimum[2]);
                assert!(pair[1].maximum[1] > pair[0].maximum[1]);
                assert!(pair[1].maximum[1] - pair[0].maximum[1] < 0.084);
            }
        }
        assert_eq!(boxes(CURB)[0].maximum, [1.0, 0.25, 0.1875]);
        assert_eq!(
            crate::construction::placement_facing(SLOPE, [0, -1, 0], [1.0, -0.3, 0.0]),
            1
        );
        assert_eq!(
            crate::construction::placement_facing(SLOPE, [1, 0, 0], [0.0, 0.0, -1.0]),
            2
        );
    }
}
