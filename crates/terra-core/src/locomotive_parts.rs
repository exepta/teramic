//! Complete BR52-inspired native block palette and its functional identities.
use crate::block_shapes::ShapeBox;
use serde::Deserialize;
use std::sync::OnceLock;

pub const FIRST: u16 = 218;
pub const LAST: u16 = 294;
pub const ENGINE: u16 = 218;
pub const WATER_TANK: u16 = 254;
pub const THROTTLE_PIVOT: [f32; 3] = [0.72, 0.57, 0.25];
pub const COUNT: usize = (LAST - FIRST + 1) as usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Engine,
    Body,
    Wheel,
    Axle,
    Rod,
    Structure,
    Valve,
    Gauge,
    Tank,
    Headlight,
}
#[derive(Clone, Copy, Debug)]
pub struct Definition {
    pub id: u16,
    pub slug: &'static str,
    pub name: &'static str,
    pub kind: Kind,
}
pub const DEFINITIONS: [Definition; COUNT] = [
    Definition {
        id: 218,
        slug: "train_fuel_engine",
        name: "Brennstoffmotor",
        kind: Kind::Engine,
    },
    Definition {
        id: 219,
        slug: "train_boiler_straight",
        name: "Lok-Kessel gerade",
        kind: Kind::Body,
    },
    Definition {
        id: 220,
        slug: "train_boiler_curved",
        name: "Lok-Kessel gebogen",
        kind: Kind::Body,
    },
    Definition {
        id: 221,
        slug: "train_boiler_end",
        name: "Lok-Kessel Abschluss",
        kind: Kind::Body,
    },
    Definition {
        id: 222,
        slug: "train_smokebox",
        name: "Rauchkammer",
        kind: Kind::Body,
    },
    Definition {
        id: 223,
        slug: "train_smokebox_door",
        name: "Rauchkammertür 52 4867",
        kind: Kind::Body,
    },
    Definition {
        id: 224,
        slug: "train_chimney",
        name: "Lok-Schornstein",
        kind: Kind::Body,
    },
    Definition {
        id: 225,
        slug: "train_steam_dome",
        name: "Dampfdom",
        kind: Kind::Body,
    },
    Definition {
        id: 226,
        slug: "train_sand_dome",
        name: "Sanddom",
        kind: Kind::Body,
    },
    Definition {
        id: 227,
        slug: "train_brass_pipe_straight",
        name: "Messingrohr gerade",
        kind: Kind::Body,
    },
    Definition {
        id: 228,
        slug: "train_brass_pipe_bend",
        name: "Messingrohr Bogen",
        kind: Kind::Body,
    },
    Definition {
        id: 229,
        slug: "train_steam_valve",
        name: "Lok-Dampfventil",
        kind: Kind::Valve,
    },
    Definition {
        id: 230,
        slug: "train_pressure_gauge",
        name: "Lok-Druckanzeige",
        kind: Kind::Gauge,
    },
    Definition {
        id: 231,
        slug: "train_pressure_unit",
        name: "Lok-Druckeinheit",
        kind: Kind::Gauge,
    },
    Definition {
        id: 232,
        slug: "train_wheel_large",
        name: "Treibrad groß",
        kind: Kind::Wheel,
    },
    Definition {
        id: 233,
        slug: "train_wheel_medium",
        name: "Lokrad mittel",
        kind: Kind::Wheel,
    },
    Definition {
        id: 234,
        slug: "train_wheel_small",
        name: "Laufrad klein",
        kind: Kind::Wheel,
    },
    Definition {
        id: 235,
        slug: "train_axle",
        name: "Lok-Achse",
        kind: Kind::Axle,
    },
    Definition {
        id: 236,
        slug: "train_connecting_rod_long",
        name: "Kuppelstange lang",
        kind: Kind::Rod,
    },
    Definition {
        id: 237,
        slug: "train_connecting_rod_short",
        name: "Kuppelstange kurz",
        kind: Kind::Rod,
    },
    Definition {
        id: 238,
        slug: "train_rod_joint",
        name: "Gestänge-Gelenk",
        kind: Kind::Rod,
    },
    Definition {
        id: 239,
        slug: "train_piston_rod",
        name: "Treibstange",
        kind: Kind::Rod,
    },
    Definition {
        id: 240,
        slug: "train_crosshead",
        name: "Kreuzkopf",
        kind: Kind::Rod,
    },
    Definition {
        id: 241,
        slug: "train_main_frame",
        name: "Lok-Hauptrahmen",
        kind: Kind::Structure,
    },
    Definition {
        id: 242,
        slug: "train_red_frame",
        name: "Lokrahmen rot",
        kind: Kind::Structure,
    },
    Definition {
        id: 243,
        slug: "train_buffer_beam",
        name: "Pufferbohle",
        kind: Kind::Structure,
    },
    Definition {
        id: 244,
        slug: "train_buffer",
        name: "Lok-Puffer",
        kind: Kind::Body,
    },
    Definition {
        id: 245,
        slug: "train_cab_wall_side",
        name: "Führerhaus-Seitenwand",
        kind: Kind::Body,
    },
    Definition {
        id: 246,
        slug: "train_cab_wall_front",
        name: "Führerhaus-Frontwand",
        kind: Kind::Body,
    },
    Definition {
        id: 247,
        slug: "train_cab_window",
        name: "Führerhaus-Fenster",
        kind: Kind::Body,
    },
    Definition {
        id: 248,
        slug: "train_cab_door",
        name: "Führerhaus-Tür",
        kind: Kind::Body,
    },
    Definition {
        id: 249,
        slug: "train_roof_curved",
        name: "Lokdach gebogen",
        kind: Kind::Body,
    },
    Definition {
        id: 250,
        slug: "train_roof_flat",
        name: "Lokdach flach",
        kind: Kind::Body,
    },
    Definition {
        id: 251,
        slug: "train_roof_vent",
        name: "Lok-Dachlüfter",
        kind: Kind::Body,
    },
    Definition {
        id: 252,
        slug: "train_tender_wall",
        name: "Tenderwand",
        kind: Kind::Body,
    },
    Definition {
        id: 253,
        slug: "train_tender_front",
        name: "Tenderfront",
        kind: Kind::Body,
    },
    Definition {
        id: 254,
        slug: "train_water_tank",
        name: "Tender-Wassertank",
        kind: Kind::Tank,
    },
    Definition {
        id: 255,
        slug: "train_coal_load",
        name: "Tender-Kohleladung",
        kind: Kind::Body,
    },
    Definition {
        id: 256,
        slug: "train_ladder_side",
        name: "Lokleiter Seite",
        kind: Kind::Body,
    },
    Definition {
        id: 257,
        slug: "train_ladder_front",
        name: "Lokleiter Front",
        kind: Kind::Body,
    },
    Definition {
        id: 258,
        slug: "train_railing",
        name: "Lok-Geländer",
        kind: Kind::Body,
    },
    Definition {
        id: 259,
        slug: "train_handrail",
        name: "Lok-Handlauf",
        kind: Kind::Body,
    },
    Definition {
        id: 260,
        slug: "train_handrail_post",
        name: "Handlauf-Stütze",
        kind: Kind::Body,
    },
    Definition {
        id: 261,
        slug: "train_rivet_plate",
        name: "Lok-Nietenplatte",
        kind: Kind::Body,
    },
    Definition {
        id: 262,
        slug: "train_bolt",
        name: "Lok-Schraubenblock",
        kind: Kind::Body,
    },
    Definition {
        id: 263,
        slug: "train_hatch",
        name: "Lok-Luke",
        kind: Kind::Body,
    },
    Definition {
        id: 264,
        slug: "train_headlight_front",
        name: "Lok-Scheinwerfer Front",
        kind: Kind::Headlight,
    },
    Definition {
        id: 265,
        slug: "train_headlight_side",
        name: "Lok-Scheinwerfer Seite",
        kind: Kind::Headlight,
    },
    Definition {
        id: 266,
        slug: "train_number_plate",
        name: "Lok-Nummernschild 52 4867",
        kind: Kind::Body,
    },
    Definition {
        id: 267,
        slug: "train_toolbox",
        name: "Lok-Werkzeugkasten",
        kind: Kind::Body,
    },
    Definition {
        id: 268,
        slug: "train_wooden_plank",
        name: "Lok-Holzplanken",
        kind: Kind::Body,
    },
    Definition {
        id: 269,
        slug: "train_coal_piece",
        name: "Lok-Kohlestücke",
        kind: Kind::Body,
    },
    Definition {
        id: 270,
        slug: "train_black_iron_block",
        name: "Schwarzes Eisen Block",
        kind: Kind::Structure,
    },
    Definition {
        id: 271,
        slug: "train_black_iron_slab",
        name: "Schwarzes Eisen Slab",
        kind: Kind::Structure,
    },
    Definition {
        id: 272,
        slug: "train_black_iron_stairs",
        name: "Schwarzes Eisen Treppe",
        kind: Kind::Structure,
    },
    Definition {
        id: 273,
        slug: "train_black_iron_inner_corner",
        name: "Schwarzes Eisen Innenecke",
        kind: Kind::Structure,
    },
    Definition {
        id: 274,
        slug: "train_black_iron_outer_corner",
        name: "Schwarzes Eisen Außenecke",
        kind: Kind::Structure,
    },
    Definition {
        id: 275,
        slug: "train_black_iron_slope",
        name: "Schwarzes Eisen Schräge",
        kind: Kind::Structure,
    },
    Definition {
        id: 276,
        slug: "train_dark_steel_block",
        name: "Dunkler Stahl Block",
        kind: Kind::Structure,
    },
    Definition {
        id: 277,
        slug: "train_dark_steel_slab",
        name: "Dunkler Stahl Slab",
        kind: Kind::Structure,
    },
    Definition {
        id: 278,
        slug: "train_dark_steel_stairs",
        name: "Dunkler Stahl Treppe",
        kind: Kind::Structure,
    },
    Definition {
        id: 279,
        slug: "train_dark_steel_inner_corner",
        name: "Dunkler Stahl Innenecke",
        kind: Kind::Structure,
    },
    Definition {
        id: 280,
        slug: "train_dark_steel_outer_corner",
        name: "Dunkler Stahl Außenecke",
        kind: Kind::Structure,
    },
    Definition {
        id: 281,
        slug: "train_dark_steel_slope",
        name: "Dunkler Stahl Schräge",
        kind: Kind::Structure,
    },
    Definition {
        id: 282,
        slug: "train_sheet_metal_block",
        name: "Lok-Blech Block",
        kind: Kind::Structure,
    },
    Definition {
        id: 283,
        slug: "train_sheet_metal_slab",
        name: "Lok-Blech Slab",
        kind: Kind::Structure,
    },
    Definition {
        id: 284,
        slug: "train_sheet_metal_stairs",
        name: "Lok-Blech Treppe",
        kind: Kind::Structure,
    },
    Definition {
        id: 285,
        slug: "train_sheet_metal_inner_corner",
        name: "Lok-Blech Innenecke",
        kind: Kind::Structure,
    },
    Definition {
        id: 286,
        slug: "train_sheet_metal_outer_corner",
        name: "Lok-Blech Außenecke",
        kind: Kind::Structure,
    },
    Definition {
        id: 287,
        slug: "train_sheet_metal_slope",
        name: "Lok-Blech Schräge",
        kind: Kind::Structure,
    },
    Definition {
        id: 288,
        slug: "train_grate_block",
        name: "Lok-Gitter Block",
        kind: Kind::Structure,
    },
    Definition {
        id: 289,
        slug: "train_grate_slab",
        name: "Lok-Gitter Slab",
        kind: Kind::Structure,
    },
    Definition {
        id: 290,
        slug: "train_grate_stairs",
        name: "Lok-Gitter Treppe",
        kind: Kind::Structure,
    },
    Definition {
        id: 291,
        slug: "train_grate_inner_corner",
        name: "Lok-Gitter Innenecke",
        kind: Kind::Structure,
    },
    Definition {
        id: 292,
        slug: "train_grate_outer_corner",
        name: "Lok-Gitter Außenecke",
        kind: Kind::Structure,
    },
    Definition {
        id: 293,
        slug: "train_grate_slope",
        name: "Lok-Gitter Schräge",
        kind: Kind::Structure,
    },
    Definition {
        id: 294,
        slug: "train_ceiling_light",
        name: "Wagen-Deckenleuchte",
        kind: Kind::Headlight,
    },
];
const SOURCES: [&str; COUNT] = [
    include_str!("../../../assets/terra/blocks/train_fuel_engine.json"),
    include_str!("../../../assets/terra/blocks/train_boiler_straight.json"),
    include_str!("../../../assets/terra/blocks/train_boiler_curved.json"),
    include_str!("../../../assets/terra/blocks/train_boiler_end.json"),
    include_str!("../../../assets/terra/blocks/train_smokebox.json"),
    include_str!("../../../assets/terra/blocks/train_smokebox_door.json"),
    include_str!("../../../assets/terra/blocks/train_chimney.json"),
    include_str!("../../../assets/terra/blocks/train_steam_dome.json"),
    include_str!("../../../assets/terra/blocks/train_sand_dome.json"),
    include_str!("../../../assets/terra/blocks/train_brass_pipe_straight.json"),
    include_str!("../../../assets/terra/blocks/train_brass_pipe_bend.json"),
    include_str!("../../../assets/terra/blocks/train_steam_valve.json"),
    include_str!("../../../assets/terra/blocks/train_pressure_gauge.json"),
    include_str!("../../../assets/terra/blocks/train_pressure_unit.json"),
    include_str!("../../../assets/terra/blocks/train_wheel_large.json"),
    include_str!("../../../assets/terra/blocks/train_wheel_medium.json"),
    include_str!("../../../assets/terra/blocks/train_wheel_small.json"),
    include_str!("../../../assets/terra/blocks/train_axle.json"),
    include_str!("../../../assets/terra/blocks/train_connecting_rod_long.json"),
    include_str!("../../../assets/terra/blocks/train_connecting_rod_short.json"),
    include_str!("../../../assets/terra/blocks/train_rod_joint.json"),
    include_str!("../../../assets/terra/blocks/train_piston_rod.json"),
    include_str!("../../../assets/terra/blocks/train_crosshead.json"),
    include_str!("../../../assets/terra/blocks/train_main_frame.json"),
    include_str!("../../../assets/terra/blocks/train_red_frame.json"),
    include_str!("../../../assets/terra/blocks/train_buffer_beam.json"),
    include_str!("../../../assets/terra/blocks/train_buffer.json"),
    include_str!("../../../assets/terra/blocks/train_cab_wall_side.json"),
    include_str!("../../../assets/terra/blocks/train_cab_wall_front.json"),
    include_str!("../../../assets/terra/blocks/train_cab_window.json"),
    include_str!("../../../assets/terra/blocks/train_cab_door.json"),
    include_str!("../../../assets/terra/blocks/train_roof_curved.json"),
    include_str!("../../../assets/terra/blocks/train_roof_flat.json"),
    include_str!("../../../assets/terra/blocks/train_roof_vent.json"),
    include_str!("../../../assets/terra/blocks/train_tender_wall.json"),
    include_str!("../../../assets/terra/blocks/train_tender_front.json"),
    include_str!("../../../assets/terra/blocks/train_water_tank.json"),
    include_str!("../../../assets/terra/blocks/train_coal_load.json"),
    include_str!("../../../assets/terra/blocks/train_ladder_side.json"),
    include_str!("../../../assets/terra/blocks/train_ladder_front.json"),
    include_str!("../../../assets/terra/blocks/train_railing.json"),
    include_str!("../../../assets/terra/blocks/train_handrail.json"),
    include_str!("../../../assets/terra/blocks/train_handrail_post.json"),
    include_str!("../../../assets/terra/blocks/train_rivet_plate.json"),
    include_str!("../../../assets/terra/blocks/train_bolt.json"),
    include_str!("../../../assets/terra/blocks/train_hatch.json"),
    include_str!("../../../assets/terra/blocks/train_headlight_front.json"),
    include_str!("../../../assets/terra/blocks/train_headlight_side.json"),
    include_str!("../../../assets/terra/blocks/train_number_plate.json"),
    include_str!("../../../assets/terra/blocks/train_toolbox.json"),
    include_str!("../../../assets/terra/blocks/train_wooden_plank.json"),
    include_str!("../../../assets/terra/blocks/train_coal_piece.json"),
    include_str!("../../../assets/terra/blocks/train_black_iron_block.json"),
    include_str!("../../../assets/terra/blocks/train_black_iron_slab.json"),
    include_str!("../../../assets/terra/blocks/train_black_iron_stairs.json"),
    include_str!("../../../assets/terra/blocks/train_black_iron_inner_corner.json"),
    include_str!("../../../assets/terra/blocks/train_black_iron_outer_corner.json"),
    include_str!("../../../assets/terra/blocks/train_black_iron_slope.json"),
    include_str!("../../../assets/terra/blocks/train_dark_steel_block.json"),
    include_str!("../../../assets/terra/blocks/train_dark_steel_slab.json"),
    include_str!("../../../assets/terra/blocks/train_dark_steel_stairs.json"),
    include_str!("../../../assets/terra/blocks/train_dark_steel_inner_corner.json"),
    include_str!("../../../assets/terra/blocks/train_dark_steel_outer_corner.json"),
    include_str!("../../../assets/terra/blocks/train_dark_steel_slope.json"),
    include_str!("../../../assets/terra/blocks/train_sheet_metal_block.json"),
    include_str!("../../../assets/terra/blocks/train_sheet_metal_slab.json"),
    include_str!("../../../assets/terra/blocks/train_sheet_metal_stairs.json"),
    include_str!("../../../assets/terra/blocks/train_sheet_metal_inner_corner.json"),
    include_str!("../../../assets/terra/blocks/train_sheet_metal_outer_corner.json"),
    include_str!("../../../assets/terra/blocks/train_sheet_metal_slope.json"),
    include_str!("../../../assets/terra/blocks/train_grate_block.json"),
    include_str!("../../../assets/terra/blocks/train_grate_slab.json"),
    include_str!("../../../assets/terra/blocks/train_grate_stairs.json"),
    include_str!("../../../assets/terra/blocks/train_grate_inner_corner.json"),
    include_str!("../../../assets/terra/blocks/train_grate_outer_corner.json"),
    include_str!("../../../assets/terra/blocks/train_grate_slope.json"),
    include_str!("../../../assets/terra/blocks/train_ceiling_light.json"),
];
pub const fn is_part(material: u16) -> bool {
    material >= FIRST && material <= LAST
}
pub const fn definition(material: u16) -> Option<&'static Definition> {
    if is_part(material) {
        Some(&DEFINITIONS[(material - FIRST) as usize])
    } else {
        None
    }
}
pub const fn is_metal_shape(material: u16) -> bool {
    material >= 270 && material <= 293
}
pub const fn is_slab(material: u16) -> bool {
    is_metal_shape(material) && (material - 270) % 6 == 1
}
pub const fn is_stair(material: u16) -> bool {
    is_metal_shape(material) && (material - 270) % 6 == 2
}
pub fn is_animated(material: u16) -> bool {
    matches!(material, 211 | 232..=234 | 236..=240)
}
pub const fn wheel_radius(material: u16) -> Option<f64> {
    match material {
        211 => Some(0.45),
        232 => Some(0.85),
        233 => Some(0.38),
        234 => Some(0.28),
        _ => None,
    }
}
pub const fn supports_wood(material: u16) -> bool {
    matches!(material, 267 | 268)
}
pub const fn is_full_support(material: u16) -> bool {
    matches!(material, 210 | 212 | 241 | 242 | 254 | 270 | 276 | 282)
}
pub fn valid_facing(material: u16, facing: u8) -> bool {
    if crate::construction::is_shape(material) {
        crate::construction::valid_facing(material, facing)
    } else {
        facing
            < if matches!(material, 219..=223 | 247 | 248) {
                8
            } else {
                4
            }
    }
}
/// Preserve authored lattice bars while applying the shared slab/stair pose.
pub fn grate_boxes(material: u16, facing: u8) -> Option<Vec<ShapeBox>> {
    if let Some(halves) = crate::construction::slab_halves(material, facing)
        && halves.iter().any(|(id, _)| *id == 289)
    {
        return Some(
            halves
                .into_iter()
                .flat_map(|(id, pose)| {
                    grate_boxes(id, pose)
                        .unwrap_or_else(|| vec![crate::construction::slab_bounds(pose)])
                })
                .collect(),
        );
    }
    if !(288..=293).contains(&material) {
        return None;
    }
    let basis = if material == 290 {
        crate::construction::stair_basis(facing)
    } else if material == 289 {
        match facing {
            4 => [[1, 0, 0], [0, -1, 0], [0, 0, -1]],
            5 => [[0, 1, 0], [1, 0, 0], [0, 0, -1]],
            6 => [[0, -1, 0], [-1, 0, 0], [0, 0, -1]],
            7 => [[1, 0, 0], [0, 0, 1], [0, -1, 0]],
            8 => [[1, 0, 0], [0, 0, -1], [0, 1, 0]],
            _ => [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        }
    } else {
        crate::construction::stair_basis(facing % 4)
    };
    Some(
        base_boxes(material)
            .iter()
            .map(|bounds| {
                let mut transformed = ShapeBox {
                    minimum: [f64::INFINITY; 3],
                    maximum: [f64::NEG_INFINITY; 3],
                    hinged: false,
                };
                for corner in 0..8 {
                    let p: [f64; 3] = std::array::from_fn(|axis| {
                        if corner & (1 << axis) == 0 {
                            bounds.minimum[axis] - 0.5
                        } else {
                            bounds.maximum[axis] - 0.5
                        }
                    });
                    for (axis, (minimum, maximum)) in transformed
                        .minimum
                        .iter_mut()
                        .zip(&mut transformed.maximum)
                        .enumerate()
                    {
                        let value = 0.5
                            + p.iter()
                                .zip(basis)
                                .map(|(coordinate, basis)| coordinate * f64::from(basis[axis]))
                                .sum::<f64>();
                        *minimum = minimum.min(value);
                        *maximum = maximum.max(value);
                    }
                }
                transformed
            })
            .collect(),
    )
}
pub fn base_boxes(material: u16) -> &'static [ShapeBox] {
    static PROFILES: [OnceLock<Vec<ShapeBox>>; COUNT] = [const { OnceLock::new() }; COUNT];
    #[derive(Deserialize)]
    struct Shape {
        collision_boxes: Vec<ShapeBox>,
    }
    if !is_part(material) {
        return &[];
    }
    let index = usize::from(material - FIRST);
    PROFILES[index].get_or_init(|| {
        serde_json::from_str::<Shape>(SOURCES[index])
            .expect("valid authored locomotive profile")
            .collision_boxes
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_complete_palette_has_distinct_authored_assets_and_finite_collision() {
        let mut names = std::collections::BTreeSet::new();
        for (index, definition) in DEFINITIONS.iter().enumerate() {
            assert_eq!(definition.id, FIRST + index as u16);
            assert!(names.insert(definition.slug));
            let boxes = base_boxes(definition.id);
            assert!(
                !boxes.is_empty() && boxes.len() <= 128,
                "{}: {} boxes",
                definition.slug,
                boxes.len()
            );
            assert!(
                boxes.iter().all(|bounds| bounds.valid_for(definition.id)),
                "{}",
                definition.slug
            );
            let model = crate::asset_directory()
                .join(format!("terra/blocks/models/{}.gltf", definition.slug));
            assert!(model.is_file(), "{}", model.display());
            assert!(
                crate::asset_directory()
                    .join(format!("crafting/{}.json", definition.slug))
                    .is_file()
            );
        }
        for kind in [
            Kind::Engine,
            Kind::Wheel,
            Kind::Rod,
            Kind::Structure,
            Kind::Valve,
            Kind::Gauge,
            Kind::Tank,
            Kind::Headlight,
        ] {
            assert!(DEFINITIONS.iter().any(|definition| definition.kind == kind));
        }
        assert!(
            DEFINITIONS
                .iter()
                .filter(|definition| (270..=293).contains(&definition.id))
                .count()
                == 24
        );
    }
    #[test]
    fn every_component_is_craftable_from_the_embedded_catalog() {
        let recipes = crate::crafting::RecipeBook::builtins();
        for definition in DEFINITIONS {
            let key = crate::crafting::recipe_key(&format!("teramic::{}", definition.slug));
            assert!(recipes.get(key).is_some(), "{}", definition.slug);
        }
    }
    #[test]
    fn boilers_and_roofs_have_supports_at_their_owner_cell_floor() {
        for material in [219, 220, 221, 222, 223, 249, 250, 251] {
            assert!(
                base_boxes(material)
                    .iter()
                    .any(|bounds| bounds.minimum[1] == 0.0)
            );
        }
    }
    #[test]
    fn piston_and_crosshead_collars_share_their_rigid_body_motion() {
        for (slug, expected) in [
            ("train_piston_rod", "train_rod_piston"),
            ("train_crosshead", "train_rod_crosshead"),
        ] {
            let source: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(
                    crate::asset_directory().join(format!("terra/blocks/models/{slug}.gltf")),
                )
                .unwrap(),
            )
            .unwrap();
            let names: std::collections::BTreeSet<_> = source["materials"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|material| material["name"].as_str())
                .filter(|name| name.starts_with("train_rod_"))
                .collect();
            assert_eq!(names, [expected].into());
        }
    }
    #[test]
    fn mixed_grate_slabs_keep_both_authored_halves_and_the_open_lattice() {
        for first in [82, 289] {
            for second in [82, 289] {
                if first != 289 && second != 289 {
                    continue;
                }
                for facing in [0, 4, 5, 6, 7, 8] {
                    let merged = crate::construction::merged_slab(first, facing, second).unwrap();
                    let boxes = grate_boxes(first, merged).unwrap();
                    let ordinary_halves = usize::from(first != 289) + usize::from(second != 289);
                    assert_eq!(
                        boxes.len(),
                        (2 - ordinary_halves) * base_boxes(289).len() + ordinary_halves
                    );
                    assert!(boxes.iter().all(|bounds| bounds.valid_for(289)));
                    assert_eq!(
                        boxes,
                        crate::construction::placed_boxes(first, merged, [None; 6])
                    );
                }
            }
        }
        assert_eq!(
            grate_boxes(289, 9).unwrap().len(),
            base_boxes(289).len() * 2
        );
    }
}
