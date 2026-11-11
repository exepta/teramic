use super::*;

fn fixture() -> (World, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "terra-character-ui-{}",
        terra_core::PlayerUuid::new_v4()
    ));
    let configs = root.join("configs");
    let profile = PlayerProfile::default();
    let selected = PlayerProfile::selected_path(&configs).unwrap();
    assert!(
        selected.starts_with(&configs),
        "Tests must never write an overridden real profile path"
    );
    profile.save(selected).unwrap();
    let mut world = World::new();
    world.insert_resource(profile);
    world.insert_resource(MenuState {
        page: MenuPage::Main,
        profile_loaded: true,
        ..default()
    });
    world.insert_resource(MenuPaths {
        configs,
        worlds: terra_storage::WorldLibrary {
            root: root.join("worlds"),
        },
    });
    world.insert_resource(ServerMenu {
        selected: Some(11),
        list: terra_core::config::servers::ServerList {
            servers: vec![terra_core::config::servers::ServerEntry {
                id: 11,
                name: "Workshop".into(),
                address: "127.0.0.1".into(),
                port: 5000,
                token_file: default(),
            }],
        },
        ..default()
    });
    world.init_resource::<Messages<SessionRequest>>();
    world.init_resource::<CharacterView>();
    (world, root)
}

#[test]
fn four_tabs_independent_eyes_and_colored_highlights_preserve_saved_identity() {
    let (mut world, root) = fixture();
    let original = world.resource::<PlayerProfile>().clone();
    command(&mut world, "character-open").unwrap();
    for tab in ["body", "face", "clothing", "hair"] {
        command(&mut world, &format!("character-tab-{tab}")).unwrap();
        assert_eq!(world.resource::<CharacterDraft>().tab, tab);
    }
    for obsolete in ["colors", "accessories"] {
        command(&mut world, &format!("character-tab-{obsolete}")).unwrap();
        assert_eq!(world.resource::<CharacterDraft>().tab, "hair");
    }
    command(&mut world, "character-female").unwrap();
    command(&mut world, "character-hair-13").unwrap();
    command(&mut world, "character-highlights").unwrap();
    change(&mut world, "character-highlight-color", "#c93b96");
    change(&mut world, "character-eye-color", "#286fbb");
    assert_eq!(
        world
            .resource::<CharacterDraft>()
            .selection
            .appearance
            .resolved_right_eye_color(),
        original.appearance.eye_color
    );
    change(&mut world, "character-right-eye-color", "#8d642e");
    synchronize(&mut world);
    let appearance = world.resource::<CharacterPreviewSelection>().appearance;
    assert_eq!(appearance.hairstyle, 13);
    assert!(appearance.hair_highlights);
    assert_eq!(appearance.hair_highlight_color, [201, 59, 150]);
    assert_eq!(appearance.eye_color, [40, 111, 187]);
    assert_eq!(appearance.resolved_right_eye_color(), [141, 100, 46]);
    assert_eq!(world.resource::<CharacterView>().styles.len(), 14);
    command(&mut world, "character-save").unwrap();
    let saved = PlayerProfile::load(root.join("configs/profile.toml")).unwrap();
    assert_eq!(saved.appearance, appearance);
    assert_eq!(saved.uuid, original.uuid);
    command(&mut world, "character-open").unwrap();
    command(&mut world, "character-male").unwrap();
    synchronize(&mut world);
    assert_eq!(world.resource::<CharacterView>().styles.len(), 12);
    assert_eq!(
        world
            .resource::<CharacterPreviewSelection>()
            .appearance
            .hairstyle,
        11
    );
    command(&mut world, "character-highlights").unwrap();
    command(&mut world, "character-cancel").unwrap();
    assert_eq!(
        PlayerProfile::load(root.join("configs/profile.toml")).unwrap(),
        saved
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn garment_materials_preview_save_cancel_and_server_override_are_independent() {
    let (mut world, root) = fixture();
    let original = world.resource::<PlayerProfile>().clone();
    command(&mut world, "character-open").unwrap();
    for (part, material) in [
        ("shirt", "leather"),
        ("pants", "wool"),
        ("boots", "canvas"),
        ("cape", "linen"),
    ] {
        assert!(change(
            &mut world,
            &format!("character-{part}-material"),
            material
        ));
    }
    synchronize(&mut world);
    let selected = world.resource::<CharacterPreviewSelection>().appearance;
    assert_eq!(selected.shirt_material, ClothingMaterial::Leather);
    assert_eq!(selected.pants_material, ClothingMaterial::Wool);
    assert_eq!(selected.boots_material, ClothingMaterial::Canvas);
    assert_eq!(selected.cape_material, ClothingMaterial::Linen);
    assert_eq!(selected.shirt_color, original.appearance.shirt_color);
    assert_eq!(world.resource::<PlayerProfile>(), &original);
    change(&mut world, "character-shirt-material", "steel");
    assert_eq!(
        world.resource::<CharacterDraft>().selection.appearance,
        selected
    );
    assert!(!world.resource::<CharacterDraft>().error.is_empty());
    command(&mut world, "character-reset").unwrap();
    assert_eq!(
        world.resource::<CharacterDraft>().selection.appearance,
        original.appearance
    );
    change(&mut world, "character-shirt-material", "wool");
    command(&mut world, "character-cancel").unwrap();
    assert_eq!(
        PlayerProfile::load(root.join("configs/profile.toml")).unwrap(),
        original
    );
    command(&mut world, "character-open").unwrap();
    change(&mut world, "character-shirt-material", "canvas");
    command(&mut world, "character-save").unwrap();
    let local = PlayerProfile::load(root.join("configs/profile.toml")).unwrap();
    assert_eq!(local.appearance.shirt_material, ClothingMaterial::Canvas);
    world.resource_mut::<MenuState>().page = MenuPage::Servers;
    command(&mut world, "server-character").unwrap();
    change(&mut world, "character-shirt-material", "leather");
    command(&mut world, "character-save").unwrap();
    let remote = prepare_character_profile(&mut world, &SessionRequest::JoinServer(11))
        .unwrap()
        .unwrap();
    assert_eq!(remote.appearance.shirt_material, ClothingMaterial::Leather);
    assert_eq!(world.resource::<PlayerProfile>(), &local);
    assert_eq!(remote.uuid, original.uuid);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn main_editor_previews_then_commits_colors_and_hairstyle_to_the_selected_local_profile() {
    let (mut world, root) = fixture();
    let original = world.resource::<PlayerProfile>().clone();
    assert!(command(&mut world, "character-open").unwrap());
    command(&mut world, "character-female").unwrap();
    command(&mut world, "character-hair-9").unwrap();
    assert!(change(&mut world, "character-hair-color", "#221133"));
    assert!(change(&mut world, "character-shirt-color", "#669922"));
    assert!(change(&mut world, "character-pants-color", "#776655"));
    synchronize(&mut world);
    assert_eq!(world.resource::<CharacterView>().styles.len(), 14);
    assert!(world.resource::<CharacterView>().styles[9].selected);
    assert_eq!(
        world
            .resource::<CharacterPreviewSelection>()
            .appearance
            .hair_color,
        [34, 17, 51]
    );
    assert_eq!(
        world.resource::<PlayerProfile>(),
        &original,
        "preview cannot mutate the saved default"
    );
    command(&mut world, "character-save").unwrap();
    let saved = world.resource::<PlayerProfile>();
    assert_eq!(saved.uuid, original.uuid);
    assert_eq!(saved.name, original.name);
    assert_eq!(saved.gender, PlayerGender::Female);
    assert_eq!(saved.appearance.hairstyle, 9);
    assert_eq!(saved.appearance.hair_color, [34, 17, 51]);
    assert_eq!(saved.appearance.shirt_color, [102, 153, 34]);
    assert_eq!(saved.appearance.pants_color, [119, 102, 85]);
    assert_eq!(
        PlayerProfile::load(root.join("configs/profile.toml")).unwrap(),
        *saved
    );
    assert_eq!(world.resource::<MenuState>().dialog, MenuDialog::None);
    assert!(world.resource::<Messages<SessionRequest>>().is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn first_join_waits_for_confirmation_cancel_does_not_join_and_override_restores_cleanly() {
    let (mut world, root) = fixture();
    let local = world.resource::<PlayerProfile>().clone();
    let request = SessionRequest::JoinServer(11);
    assert!(
        prepare_character_profile(&mut world, &request)
            .unwrap()
            .is_none()
    );
    assert_eq!(world.resource::<MenuState>().dialog, MenuDialog::Character);
    assert_eq!(
        world.resource::<CharacterDraft>().selection,
        CharacterSelection::from(&local)
    );
    change(&mut world, "character-shirt-color", "#112233");
    command(&mut world, "character-cancel").unwrap();
    assert!(!root.join("configs/server-characters.toml").exists());
    assert!(world.resource::<Messages<SessionRequest>>().is_empty());
    assert_eq!(world.resource::<PlayerProfile>(), &local);

    assert!(
        prepare_character_profile(&mut world, &request)
            .unwrap()
            .is_none()
    );
    command(&mut world, "character-female").unwrap();
    command(&mut world, "character-hair-8").unwrap();
    change(&mut world, "character-shirt-color", "#112233");
    command(&mut world, "character-save").unwrap();
    let queued: Vec<_> = world
        .resource_mut::<Messages<SessionRequest>>()
        .drain()
        .collect();
    assert!(matches!(
        queued.as_slice(),
        [SessionRequest::JoinServer(11)]
    ));
    assert_eq!(world.resource::<PlayerProfile>(), &local);
    assert_eq!(
        PlayerProfile::load(root.join("configs/profile.toml")).unwrap(),
        local
    );
    let remote = prepare_character_profile(&mut world, &request)
        .unwrap()
        .unwrap();
    assert_eq!(remote.uuid, local.uuid);
    assert_eq!(remote.name, local.name);
    assert_eq!(remote.appearance.shirt_color, [17, 34, 51]);
    assert_eq!(remote.appearance.hairstyle, 8);
    assert_eq!(remote.gender, PlayerGender::Female);
    activate_character_profile(&mut world, remote.clone());
    assert_eq!(world.resource::<PlayerProfile>(), &remote);
    restore_character_profile(&mut world);
    assert_eq!(world.resource::<PlayerProfile>(), &local);
    restore_character_profile(&mut world);
    assert_eq!(world.resource::<PlayerProfile>(), &local);
    assert_eq!(
        prepare_character_profile(
            &mut world,
            &SessionRequest::JoinAddress("127.0.0.1:5000".parse().unwrap())
        )
        .unwrap(),
        Some(remote)
    );
    assert!(
        prepare_character_profile(
            &mut world,
            &SessionRequest::JoinAddress("127.0.0.1:5001".parse().unwrap())
        )
        .unwrap()
        .is_none()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn server_list_edit_is_independent_and_invalid_files_are_never_overwritten() {
    let (mut world, root) = fixture();
    let local = world.resource::<PlayerProfile>().clone();
    world.resource_mut::<MenuState>().page = MenuPage::Servers;
    command(&mut world, "server-character").unwrap();
    change(&mut world, "character-pants-color", "#440088");
    command(&mut world, "character-save").unwrap();
    assert!(world.resource::<Messages<SessionRequest>>().is_empty());
    assert_eq!(world.resource::<PlayerProfile>(), &local);
    let saved = ServerCharacterProfiles::load(root.join("configs/server-characters.toml")).unwrap();
    assert_eq!(
        saved
            .get(local.uuid, "127.0.0.1", 5000)
            .unwrap()
            .unwrap()
            .appearance
            .pants_color,
        [68, 0, 136]
    );
    assert_eq!(saved.get(local.uuid, "127.0.0.1", 5001).unwrap(), None);
    std::fs::write(
        root.join("configs/server-characters.toml"),
        "not valid toml",
    )
    .unwrap();
    assert!(command(&mut world, "server-character").is_err());
    assert_eq!(
        std::fs::read_to_string(root.join("configs/server-characters.toml")).unwrap(),
        "not valid toml"
    );
    assert_eq!(world.resource::<PlayerProfile>(), &local);
    world.resource_mut::<MenuState>().profile_loaded = false;
    world.resource_mut::<MenuState>().page = MenuPage::Main;
    assert!(command(&mut world, "character-open").is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_save_keeps_the_draft_and_never_changes_the_active_character() {
    let (mut world, root) = fixture();
    let local = world.resource::<PlayerProfile>().clone();
    world.resource_mut::<MenuState>().page = MenuPage::Servers;
    command(&mut world, "server-character").unwrap();
    change(&mut world, "character-hair-color", "#c08020");
    std::fs::create_dir(root.join("configs/server-characters.toml")).unwrap();
    command(&mut world, "character-save").unwrap();
    assert_eq!(world.resource::<MenuState>().dialog, MenuDialog::Character);
    assert!(!world.resource::<CharacterDraft>().error.is_empty());
    assert_eq!(world.resource::<PlayerProfile>(), &local);
    assert!(world.resource::<Messages<SessionRequest>>().is_empty());
    std::fs::remove_dir_all(root).unwrap();
}
