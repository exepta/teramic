//! Authored grave furniture. Pure simulation and rendering share the same volumes.
use crate::block_shapes::ShapeBox;
use serde::Deserialize;
use std::sync::OnceLock;

pub const ORNATE_CHEST: u16 = 377;
pub const fn is_prop(material: u16) -> bool {
    matches!(material, 344..=399)
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
    const SOURCES: [&str; 56] = [
        include_str!("../../../assets/terra/blocks/grave_stone_slab.json"),
        include_str!("../../../assets/terra/blocks/ornate_grave_slab.json"),
        include_str!("../../../assets/terra/blocks/mosaic_grave_slab.json"),
        include_str!("../../../assets/terra/blocks/cracked_grave_slab.json"),
        include_str!("../../../assets/terra/blocks/inscribed_grave_slab.json"),
        include_str!("../../../assets/terra/blocks/crypt_column.json"),
        include_str!("../../../assets/terra/blocks/ornate_crypt_column.json"),
        include_str!("../../../assets/terra/blocks/crypt_capital.json"),
        include_str!("../../../assets/terra/blocks/crypt_column_base.json"),
        include_str!("../../../assets/terra/blocks/crypt_arch.json"),
        include_str!("../../../assets/terra/blocks/mossy_crypt_arch.json"),
        include_str!("../../../assets/terra/blocks/grave_relief_wall.json"),
        include_str!("../../../assets/terra/blocks/skull_relief.json"),
        include_str!("../../../assets/terra/blocks/cross_relief.json"),
        include_str!("../../../assets/terra/blocks/crypt_wall_niche.json"),
        include_str!("../../../assets/terra/blocks/crypt_heraldic_banner.json"),
        include_str!("../../../assets/terra/blocks/crypt_torn_banner.json"),
        include_str!("../../../assets/terra/blocks/crypt_curtain.json"),
        include_str!("../../../assets/terra/blocks/gravestone_small.json"),
        include_str!("../../../assets/terra/blocks/gravestone_medium.json"),
        include_str!("../../../assets/terra/blocks/gravestone_large.json"),
        include_str!("../../../assets/terra/blocks/grave_cross.json"),
        include_str!("../../../assets/terra/blocks/bordered_grave.json"),
        include_str!("../../../assets/terra/blocks/sarcophagus_closed.json"),
        include_str!("../../../assets/terra/blocks/sarcophagus_open.json"),
        include_str!("../../../assets/terra/blocks/grave_lantern_post.json"),
        include_str!("../../../assets/terra/blocks/crypt_wall_torch.json"),
        include_str!("../../../assets/terra/blocks/crypt_candles.json"),
        include_str!("../../../assets/terra/blocks/crypt_chandelier.json"),
        include_str!("../../../assets/terra/blocks/soul_brazier.json"),
        include_str!("../../../assets/terra/blocks/skeleton_remains.json"),
        include_str!("../../../assets/terra/blocks/bone_pile.json"),
        include_str!("../../../assets/terra/blocks/grave_urn.json"),
        include_str!("../../../assets/terra/blocks/ornate_crypt_chest.json"),
        include_str!("../../../assets/terra/blocks/crypt_books.json"),
        include_str!("../../../assets/terra/blocks/grave_skull.json"),
        include_str!("../../../assets/terra/blocks/crypt_chain.json"),
        include_str!("../../../assets/terra/blocks/dead_grass.json"),
        include_str!("../../../assets/terra/blocks/thorn_bush.json"),
        include_str!("../../../assets/terra/blocks/hanging_ivy.json"),
        include_str!("../../../assets/terra/blocks/grave_moss.json"),
        include_str!("../../../assets/terra/blocks/dark_mushrooms.json"),
        include_str!("../../../assets/terra/blocks/glowing_purple_mushrooms.json"),
        include_str!("../../../assets/terra/blocks/blossoming_vines.json"),
        include_str!("../../../assets/terra/blocks/crypt_pressure_plate.json"),
        include_str!("../../../assets/terra/blocks/crypt_lever.json"),
        include_str!("../../../assets/terra/blocks/crypt_wall_switch.json"),
        include_str!("../../../assets/terra/blocks/crypt_portcullis.json"),
        include_str!("../../../assets/terra/blocks/locked_crypt_gate.json"),
        include_str!("../../../assets/terra/blocks/crypt_spike_trap.json"),
        include_str!("../../../assets/terra/blocks/crypt_arrow_trap.json"),
        include_str!("../../../assets/terra/blocks/soul_crystal_small.json"),
        include_str!("../../../assets/terra/blocks/soul_crystal_large.json"),
        include_str!("../../../assets/terra/blocks/active_rune_block.json"),
        include_str!("../../../assets/terra/blocks/inactive_rune_block.json"),
        include_str!("../../../assets/terra/blocks/mystic_mist_block.json"),
    ];
    static PROFILES: [OnceLock<Vec<ShapeBox>>; 56] = [const { OnceLock::new() }; 56];
    let index = usize::from(material - 344);
    PROFILES[index].get_or_init(|| {
        serde_json::from_str::<Definition>(SOURCES[index])
            .ok()
            .map(|definition| definition.collision_boxes)
            .filter(|boxes| {
                boxes.len() <= 128 && boxes.iter().all(|shape| shape.valid_for(material))
            })
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_grave_profiles_are_valid_and_storage_is_real() {
        let loaded = crate::block_properties::BlockPropertiesCatalog::load(
            &crate::asset_directory().join("terra/blocks"),
        )
        .unwrap();
        for id in 344..=399 {
            assert_eq!(boxes(id).len(), loaded.shape(id).len(), "material {id}");
            assert!(boxes(id).iter().all(|shape| shape.valid_for(id)));
        }
        assert!(crate::profession_stations::is_depot(ORNATE_CHEST));
        assert!(crate::profession_stations::supports_work(
            ORNATE_CHEST,
            crate::npc_jobs::WorkKind::Collector
        ));
        assert!(!crate::profession_stations::is_station(ORNATE_CHEST));
        assert!(!boxes(ORNATE_CHEST).is_empty());
    }
}
