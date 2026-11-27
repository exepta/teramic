//! Shared authored collision volumes for Blood Forest props.
use crate::block_shapes::ShapeBox;
use serde::Deserialize;
use std::sync::OnceLock;
pub const fn is_prop(material: u16) -> bool {
    matches!(material, 412..=421 | 496..=503)
}
pub fn boxes(material: u16) -> &'static [ShapeBox] {
    if !is_prop(material) {
        return &[];
    }
    #[derive(Deserialize)]
    struct Definition {
        #[serde(default)]
        collision_boxes: Vec<ShapeBox>,
    }
    static SHAPES: OnceLock<[Vec<ShapeBox>; 18]> = OnceLock::new();
    let sources = [
        include_str!("../../../assets/terra/blocks/blood_mushrooms.json"),
        include_str!("../../../assets/terra/blocks/blood_thorn_bush.json"),
        include_str!("../../../assets/terra/blocks/blood_flower.json"),
        include_str!("../../../assets/terra/blocks/hanging_blood_vines.json"),
        include_str!("../../../assets/terra/blocks/blood_roots.json"),
        include_str!("../../../assets/terra/blocks/blood_bones.json"),
        include_str!("../../../assets/terra/blocks/blood_fern.json"),
        include_str!("../../../assets/terra/blocks/giant_blood_mushroom.json"),
        include_str!("../../../assets/terra/blocks/blood_ruined_arch.json"),
        include_str!("../../../assets/terra/blocks/blood_crystal_pillar.json"),
        include_str!("../../../assets/terra/blocks/torn_blood_banner.json"),
        include_str!("../../../assets/terra/blocks/standing_blood_lantern.json"),
        include_str!("../../../assets/terra/blocks/blood_brazier.json"),
        include_str!("../../../assets/terra/blocks/weathered_ribcage.json"),
        include_str!("../../../assets/terra/blocks/bone_stake.json"),
        include_str!("../../../assets/terra/blocks/blood_crystal_growth.json"),
        include_str!("../../../assets/terra/blocks/hanging_blood_lantern.json"),
        include_str!("../../../assets/terra/blocks/fallen_blood_branches.json"),
    ];
    &SHAPES.get_or_init(|| {
        sources.map(|text| {
            serde_json::from_str::<Definition>(text)
                .expect("valid built-in Blood Forest shapes")
                .collision_boxes
        })
    })[usize::from(if material >= 496 {
        material - 496 + 10
    } else {
        material - 412
    })]
}
