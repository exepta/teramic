use super::*;
use terra_core::{
    ItemKind, WoodSpecies,
    logistics::{DistributorConfig, FilterConfig},
};
use terra_world::edits::BlockObjectState;

#[test]
fn rga9_preserves_three_exact_filters_and_legacy_regions_across_reload_and_removal() {
    let root = std::env::temp_dir().join(format!("terra-distributors-{}", PlayerUuid::new_v4()));
    let world = WorldId(1620);
    let cell = [-17, 15, -1];
    let tunnel = [512, 16, 0];
    let rgb = ItemKind::TintedBlock {
        material: 148,
        wood: WoodSpecies::Cherry,
        color: [71, 93, 184],
    };
    let config = DistributorConfig {
        filters: [
            FilterConfig {
                item: Some(rgb),
                allowed: true,
            },
            FilterConfig {
                item: Some(ItemKind::Block(1)),
                allowed: false,
            },
            FilterConfig::default(),
        ],
    };
    let mut journal = EditJournal::default();
    for (position, material) in [(cell, 160), (tunnel, 159)] {
        journal.record(position, material).unwrap();
        journal
            .set_object_state(position, Some(BlockObjectState::default()))
            .unwrap();
    }
    journal.set_distributor(cell, config).unwrap();
    journal
        .set_logistics_filter(tunnel, config.filters[0])
        .unwrap();
    for _ in 0..3 {
        save_edits(&root, world, &journal).unwrap();
        assert_eq!(
            read(&root.join(region_name([-1, -1]))).unwrap().get(12..16),
            Some(b"RGA9".as_slice())
        );
        assert_eq!(
            read(&root.join(region_name([1, 0]))).unwrap().get(12..16),
            Some(b"RGA7".as_slice())
        );
        let loaded = load_edits(&root, world).unwrap();
        assert_eq!(loaded.distributor_at(cell), Some(&config));
        assert_eq!(loaded.logistics_filter_at(tunnel), Some(&config.filters[0]));
        assert!(loaded.container_at(cell).is_none());
        assert_eq!(
            loaded.gameplay().inventories,
            journal.gameplay().inventories
        );
        journal = loaded;
    }
    journal.record(cell, 0).unwrap();
    assert!(journal.distributor_at(cell).is_none());
    save_edits(&root, world, &journal).unwrap();
    assert!(
        load_edits(&root, world)
            .unwrap()
            .distributor_at(cell)
            .is_none()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rga9_rejects_orphan_and_invalid_filters_without_partial_import() {
    let root = std::env::temp_dir().join(format!(
        "terra-invalid-distributors-{}",
        PlayerUuid::new_v4()
    ));
    let world = WorldId(1621);
    let cell = [-1, 4, -1];
    let mut journal = EditJournal::default();
    journal.record(cell, 160).unwrap();
    journal
        .set_object_state(cell, Some(BlockObjectState::default()))
        .unwrap();
    journal.set_distributor(cell, Default::default()).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(region_name([-1, -1]));
    let bytes = read(&path).unwrap();
    for mutation in 0..3 {
        let mut data: DistributorRegion = decode(b"RGA9", &bytes).unwrap();
        match mutation {
            0 => data.distributors[0].0 = [-2, 4, -1],
            1 => data.distributors.push(data.distributors[0]),
            _ => data.distributors[0].1.filters[2].item = Some(ItemKind::Block(u16::MAX)),
        }
        std::fs::write(&path, encode(b"RGA9", &data).unwrap()).unwrap();
        assert!(load_edits(&root, world).is_err());
    }
    std::fs::remove_dir_all(root).unwrap();
}
