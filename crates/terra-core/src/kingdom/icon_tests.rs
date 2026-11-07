use super::*;

#[test]
fn legacy_save_values_remain_readable_and_catalog_ids_are_stable() {
    for icon in KingdomIcon::ALL {
        let json = format!("\"{}\"", icon.key());
        assert_eq!(serde_json::from_str::<KingdomIcon>(&json).unwrap(), icon);
        assert_eq!(serde_json::to_string(&icon).unwrap(), json);
    }
    let mut names = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for icon in KingdomIcon::catalog() {
        assert!(icon.valid());
        assert!(names.insert(icon.key()));
        assert_eq!(KingdomIcon::from_key(icon.key()), Some(icon));
        assert!(icon.glyph_mask().unwrap().iter().any(|row| *row != 0));
        let json = serde_json::to_string(&icon).unwrap();
        assert_eq!(serde_json::from_str::<KingdomIcon>(&json).unwrap(), icon);
        if let KingdomIcon::Catalog(id) = icon {
            let expected = icon.key().bytes().fold(0xcbf29ce484222325_u64, |v, b| {
                (v ^ u64::from(b)).wrapping_mul(0x100000001b3)
            });
            assert_eq!(id, expected);
            assert!(ids.insert(id));
        }
    }
    assert!(names.len() > 500);
    assert!(!KingdomIcon::Catalog(0).valid());
}

#[test]
fn invalid_catalog_ids_are_rejected_before_registry_mutation() {
    let mut registry = KingdomRegistry::default();
    let owner = PlayerUuid::from_u128(900);
    let invalid = KingdomHeraldry {
        icon: KingdomIcon::Catalog(0),
        ..Default::default()
    };
    assert_eq!(
        registry.found_settlement(
            owner,
            "Owner",
            [0; 3],
            "Keep",
            SettlementKind::Kingdom,
            invalid
        ),
        Err(KingdomError::InvalidHeraldry)
    );
    assert_eq!(registry, KingdomRegistry::default());
    let id = registry
        .found_settlement(
            owner,
            "Owner",
            [0; 3],
            "Keep",
            SettlementKind::Kingdom,
            Default::default(),
        )
        .unwrap();
    let before = registry.clone();
    assert_eq!(
        registry.change_appearance(owner, id, invalid),
        Err(KingdomError::InvalidHeraldry)
    );
    assert_eq!(registry, before);
}
