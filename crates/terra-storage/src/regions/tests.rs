use super::*;
use std::path::PathBuf;

#[test]
fn generated_structure_containers_keep_empty_and_removed_markers_across_real_save_reload() {
    use terra_world::{SectionGenerator, TerraGenerator, generation::structures::StructureKind};
    let root = root();
    let world = WorldId(122);
    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        id: world,
        structures_revision: 1,
        biomes: false,
        trees: false,
        caves: false,
        water: false,
        rivers: false,
        landscape_revision: 0,
        geology_revision: 0,
        ocean_revision: 0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        ..Default::default()
    })
    .unwrap();
    let site = (-3..=3)
        .flat_map(|z| (-3..=3).map(move |x| [x, z]))
        .filter_map(|p| terrain.structure_site(p))
        .find(|s| s.kind == StructureKind::Ruin)
        .unwrap();
    let chest = site.containers[0];
    let owner = terra_voxel::global_to_section(chest).0;
    let generator = SectionGenerator::new(terrain);
    let column = generator.prepare_column([owner.x, owner.z], false).unwrap();
    let objects = column.structure_objects(owner);
    let mut journal = EditJournal::default();
    assert!(!journal.initialize_structures(&objects).unwrap().is_empty());
    assert!(journal.container_at(chest).unwrap().occupied_slots() > 0);
    journal.set_container(chest, Inventory::default()).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.container_at(chest), Some(&Inventory::default()));
    assert!(loaded.initialize_structures(&objects).unwrap().is_empty());
    assert_eq!(loaded.container_at(chest), Some(&Inventory::default()));
    // server_furniture retires private contents before its block-edit commit;
    // record() is deliberately not a destructive inventory transaction.
    assert_eq!(
        loaded.remove_container(chest).unwrap(),
        Some(Inventory::default())
    );
    loaded.record(chest, 0).unwrap();
    save_edits(&root, world, &loaded).unwrap();
    let mut removed = load_edits(&root, world).unwrap();
    assert!(removed.initialize_structures(&objects).unwrap().is_empty());
    assert_eq!(removed.block(chest), Some(0));
    assert!(removed.container_at(chest).is_none());
    let section = generator
        .build(
            owner,
            removed.revision(owner),
            &removed.snapshot(owner).unwrap(),
            false,
        )
        .unwrap();
    assert_eq!(
        section
            .data
            .block(terra_voxel::global_to_section(chest).1)
            .unwrap(),
        0
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn generated_smith_furnaces_are_simulatable_persisted_and_not_reset_on_publication() {
    use terra_world::{edits::BlockObjectState, generation::structures::StructureObject};
    let root = root();
    let world = WorldId(122);
    let primary = [-17, 15, -1];
    let upper = [-17, 16, -1];
    let objects =
        [(primary, upper, false), (upper, primary, true)].map(|(position, peer, secondary)| {
            StructureObject {
                position,
                material: 59,
                state: Some(BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    ..Default::default()
                }),
                loot: None,
                crop: None,
            }
        });
    let mut journal = EditJournal::default();
    journal.initialize_structures(&objects).unwrap();
    assert!(journal.furnace_at(primary).is_some());
    assert!(journal.furnace_at(upper).is_none());
    let mut state = journal.furnace_at(primary).unwrap().clone();
    state.burn_remaining = 4.0;
    state.burn_total = 8.0;
    state.completed = 3;
    journal.set_furnace(primary, state.clone()).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.furnace_at(primary), Some(&state));
    assert!(loaded.initialize_structures(&objects).unwrap().is_empty());
    assert_eq!(loaded.furnace_at(primary), Some(&state));
    assert_eq!(loaded.object_state(primary).unwrap().peer, Some(upper));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn player_locations_roundtrip_per_uuid_and_world_without_terrain_edits() {
    let root = root();
    let other = root.join("other-world");
    let world = WorldId(120);
    let players = [PlayerUuid::new_v4(), PlayerUuid::new_v4()];
    let locations = [
        terra_core::PlayerLocation {
            position: [-16.25, -63.1, -32.75],
            look: [-1.2, 0.7],
            flying: false,
        },
        terra_core::PlayerLocation {
            position: [346.5, 220.5, -501.5],
            look: [2.0, -0.3],
            flying: true,
        },
    ];
    let mut journal = EditJournal::default();
    for (player, location) in players.into_iter().zip(locations) {
        assert!(journal.update_player_location(player, location).unwrap());
        assert!(!journal.update_player_location(player, location).unwrap());
    }
    assert_eq!(journal.player_state_revision(), 2);
    assert_eq!(journal.changed_sections().count(), 0);
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    for (player, location) in players.into_iter().zip(locations) {
        assert_eq!(loaded.player_location(player), Some(location));
        let bytes = read(&root.join(format!("players/{player}.data"))).unwrap();
        assert_eq!(bytes.get(12..16), Some(b"PLY8".as_slice()));
    }
    assert_eq!(
        load_edits(&other, WorldId(121))
            .unwrap()
            .player_location(players[0]),
        None
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_player_payload_has_no_location_and_invalid_new_payload_is_rejected() {
    let player = PlayerUuid::new_v4();
    let data = PlayerData {
        world: WorldId(120),
        player,
        inventory: Default::default(),
        stats: Default::default(),
        has_inventory: false,
        has_stats: false,
    };
    assert!(
        read_saved_player(&encode(b"PLY7", &data).unwrap())
            .unwrap()
            .1
            .is_none()
    );
    let invalid = terra_core::PlayerLocation {
        position: [f32::NAN, 0.0, 0.0],
        look: [0.0; 2],
        flying: false,
    };
    assert!(
        read_saved_player(
            &encode(
                b"PLY8",
                &LocatedPlayerData {
                    prior: data,
                    location: invalid
                }
            )
            .unwrap()
        )
        .is_err()
    );
    let mut journal = EditJournal::default();
    assert!(journal.update_player_location(player, invalid).is_err());
    assert!(
        journal
            .update_player_location(
                PlayerUuid::nil(),
                terra_core::PlayerLocation {
                    position: [0.0; 3],
                    look: [0.0; 2],
                    flying: false
                }
            )
            .is_err()
    );
    assert_eq!(journal.player_state_revision(), 0);
    assert_eq!(journal.player_locations().count(), 0);
}

#[test]
fn spanning_silk_geometry_identity_and_removal_survive_region_and_replica_reload() {
    use terra_world::props::{PropId, webs};
    let root = root();
    let world = WorldId(118);
    let prop = webs::spanning([-17, -9, 15], [1, 0, 0], 21, 13, PropId((1 << 60) + 118)).unwrap();
    let bounds = prop.bounds();
    let mut journal = EditJournal::default();
    assert!(journal.place_prop(prop).unwrap());
    save_edits(&root, world, &journal).unwrap();
    let mut loaded = load_edits(&root, world).unwrap();
    let delta = loaded.section_delta(prop.section);
    assert_eq!(delta.placed_props, vec![prop]);
    assert_eq!(delta.placed_props[0].bounds(), bounds);
    let mut replica = EditJournal::default();
    replica.import_delta(delta).unwrap();
    let mut rendered = Vec::new();
    replica.apply_prop_removals(prop.section, &mut rendered);
    assert_eq!(rendered, vec![prop]);
    assert!(loaded.remove_prop(prop.section, prop.id).unwrap());
    save_edits(&root, world, &loaded).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert!(loaded.prop_removed(prop.section, prop.id));
    loaded.apply_prop_removals(prop.section, &mut rendered);
    assert!(rendered.iter().all(|p| p.removed));
    fs::remove_dir_all(root).unwrap();
}

fn current_farming(bytes: &[u8]) -> FarmingRegion {
    decode::<TamingRegion>(b"RGA3", bytes).unwrap().prior
}

#[test]
fn natural_cave_stalker_without_surface_spawn_history_roundtrips_without_claiming_surface_cells() {
    use terra_core::creatures::{
        Creature, CreatureBehavior, CreatureKind, CreatureNeeds, stalker_id,
    };
    let root = root();
    let world = WorldId(117);
    let mut journal = EditJournal::default();
    let home = [-17, -1];
    let id = stalker_id(home).unwrap();
    let animal = Creature {
        id,
        kind: CreatureKind::Stalker,
        home,
        position: [-536.5, -15.0, -8.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: CreatureKind::Stalker.health(),
        behavior: CreatureBehavior::Rest,
        timer: 2.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::Stalker),
    };
    let sex = animal.sex();
    journal.gameplay_mut().creatures.animals.insert(id, animal);
    assert!(journal.gameplay().valid());
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    assert_eq!(loaded.gameplay().creatures.animals[&id].sex(), sex);
    assert!(loaded.gameplay().creatures.explored.is_empty());
    save_edits(&root, world, &loaded).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rga4_families_growth_and_global_birth_high_water_survive_cross_region_reload_and_deletion() {
    use terra_core::creatures::{
        Creature, CreatureBehavior, CreatureKind, CreatureNeeds,
        breeding::{self, AnimalFamily},
        taming::AnimalBond,
    };
    let root = root();
    let world = WorldId(115);
    let owner = PlayerUuid::new_v4();
    let mut journal = EditJournal::default();
    let parents = [(1 << 62) + 1, (1 << 62) + 2];
    for (n, id) in parents.into_iter().enumerate() {
        let home = if n == 0 { [-17, 0] } else { [1, 0] };
        let animal = Creature {
            id,
            kind: CreatureKind::Cow,
            home,
            position: [home[0] as f64 * 32.0 + 8.0, 3.0, 8.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: CreatureKind::Cow.health(),
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 300.0,
            variant: 0,
            needs: CreatureNeeds::full(CreatureKind::Cow),
        };
        let state = journal.gameplay_mut();
        state.creatures.animals.insert(id, animal);
        state.creatures.owners.insert(id, owner);
        state.animal_bonds.insert(id, AnimalBond::owned(owner));
        state.animal_families.insert(
            id,
            AnimalFamily {
                ready_age: 500.0,
                ..Default::default()
            },
        );
    }
    let state = journal.gameplay_mut();
    let id = breeding::next_birth_id(&state.creatures, state.next_animal_birth).unwrap();
    let mut child = state.creatures.animals[&parents[0]].clone();
    child.id = id;
    child.age = 599.5;
    child.position[0] += 2.0;
    state.creatures.animals.insert(id, child);
    state.creatures.owners.insert(id, owner);
    state.animal_bonds.insert(id, AnimalBond::owned(owner));
    state.animal_families.insert(
        id,
        AnimalFamily {
            parents: Some(parents),
            active_age: 700.0,
            ready_age: breeding::GROWTH_SECONDS,
            willing_until_age: 0.0,
        },
    );
    state.next_animal_birth = breeding::birth_serial(id).unwrap();
    assert!(state.valid());
    save_edits(&root, world, &journal).unwrap();
    assert_eq!(&read(&root.join(INDEX)).unwrap()[12..16], b"IDX7");
    assert_eq!(
        &read(&root.join(region_name([-2, 0]))).unwrap()[12..16],
        b"RGA4"
    );
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    for (&animal_id, animal) in &journal.gameplay().creatures.animals {
        assert_eq!(
            loaded.gameplay().creatures.animals[&animal_id].sex(),
            animal.sex()
        );
    }
    assert_eq!(loaded.gameplay().creatures.animals[&id].age, 599.5);
    assert_eq!(loaded.gameplay().animal_families[&id].active_age, 700.0);
    let state = loaded.gameplay_mut();
    state.creatures.animals.remove(&id);
    state.creatures.owners.remove(&id);
    state.animal_bonds.remove(&id);
    state.animal_families.remove(&id);
    save_edits(&root, world, &loaded).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    let state = loaded.gameplay();
    let next = breeding::next_birth_id(&state.creatures, state.next_animal_birth).unwrap();
    assert!(next > id);
    assert_eq!(state.next_animal_birth, 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rga3_taming_ownership_modes_and_spider_dens_roundtrip_and_old_spider_hunger_migrates_once() {
    use terra_core::creatures::{
        Creature, CreatureBehavior, CreatureKind, CreatureNeeds,
        taming::{AnimalBond, CompanionMode, SpiderDen},
    };
    let root = root();
    let world = WorldId(113);
    let player = PlayerUuid::new_v4();
    let id = terra_core::creatures::spawning::next_command_id(&Default::default()).unwrap();
    let mut journal = EditJournal::default();
    journal.gameplay_mut().creatures.animals.insert(
        id,
        Creature {
            id,
            kind: CreatureKind::MediumSpider,
            home: [0, 0],
            position: [8.0, -11.0, 8.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 10.0,
            behavior: CreatureBehavior::Rest,
            timer: 2.0,
            age: 12.0,
            variant: 0,
            needs: CreatureNeeds::full(CreatureKind::MediumSpider),
        },
    );
    journal.gameplay_mut().creatures.owners.insert(id, player);
    journal.gameplay_mut().animal_bonds.insert(
        id,
        AnimalBond {
            player,
            trust: 100,
            mode: CompanionMode::Follow,
            last_fed_age: Some(10.0),
        },
    );
    journal.gameplay_mut().spider_dens.insert(
        id,
        SpiderDen {
            anchor: [7, -12, 8],
            web_id: 113,
        },
    );
    save_edits(&root, world, &journal).unwrap();
    let current = read(&root.join(region_name([0, 0]))).unwrap();
    assert_eq!(current.get(12..16), Some(b"RGA3".as_slice()));
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    assert_eq!(
        loaded.gameplay().creatures.animals[&id].sex(),
        journal.gameplay().creatures.animals[&id].sex()
    );
    assert_eq!(loaded.gameplay().creatures.natural_count(), 0);
    let mut old: FarmingRegion = current_farming(&current);
    old.prior
        .prior
        .prior
        .creatures
        .animals
        .get_mut(&id)
        .unwrap()
        .needs
        .food = 0.0;
    fs::write(
        root.join(region_name([0, 0])),
        encode(b"RGA2", &old).unwrap(),
    )
    .unwrap();
    let mut migrated = load_edits(&root, world).unwrap();
    assert!(migrated.gameplay().animal_bonds.is_empty());
    assert_eq!(migrated.gameplay().creatures.owners.get(&id), Some(&player));
    assert_eq!(migrated.gameplay().creatures.animals[&id].needs.food, 50.0);
    migrated
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&id)
        .unwrap()
        .needs
        .food = 0.0;
    save_edits(&root, world, &migrated).unwrap();
    assert_eq!(
        load_edits(&root, world)
            .unwrap()
            .gameplay()
            .creatures
            .animals[&id]
            .needs
            .food,
        0.0
    );
    let valid = read(&root.join(region_name([0, 0]))).unwrap();
    let mut corrupt: TamingRegion = decode(b"RGA3", &valid).unwrap();
    corrupt.bonds.push((
        id,
        AnimalBond {
            player: PlayerUuid::new_v4(),
            trust: 100,
            mode: CompanionMode::Follow,
            last_fed_age: None,
        },
    ));
    fs::write(
        root.join(region_name([0, 0])),
        encode(b"RGA3", &corrupt).unwrap(),
    )
    .unwrap();
    assert!(
        load_edits(&root, world).is_err(),
        "different UUID must not acquire ownership through a bond record"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn extended_dye_and_tool_palettes_keep_every_legacy_postcard_discriminant() {
    #[derive(Serialize)]
    enum OldColor {
        Red,
        Yellow,
        Blue,
        Orange,
        Green,
        Purple,
        White,
    }
    #[derive(Serialize)]
    enum OldTool {
        WoodPickaxe,
        StonePickaxe,
        StoneAxe,
        StoneShovel,
        IronPickaxe,
        WoodAxe,
        WoodShovel,
        WoodSword,
        StoneSword,
    }
    use terra_core::{ToolKind, beds::DyeColor};
    for (old, color) in [
        (OldColor::Red, DyeColor::Red),
        (OldColor::Yellow, DyeColor::Yellow),
        (OldColor::Blue, DyeColor::Blue),
        (OldColor::Orange, DyeColor::Orange),
        (OldColor::Green, DyeColor::Green),
        (OldColor::Purple, DyeColor::Purple),
        (OldColor::White, DyeColor::White),
    ] {
        assert_eq!(
            decode::<DyeColor>(b"TEST", &encode(b"TEST", &old).unwrap()).unwrap(),
            color
        );
    }
    for (old, tool) in [
        (OldTool::WoodPickaxe, ToolKind::WoodPickaxe),
        (OldTool::StonePickaxe, ToolKind::StonePickaxe),
        (OldTool::StoneAxe, ToolKind::StoneAxe),
        (OldTool::StoneShovel, ToolKind::StoneShovel),
        (OldTool::IronPickaxe, ToolKind::IronPickaxe),
        (OldTool::WoodAxe, ToolKind::WoodAxe),
        (OldTool::WoodShovel, ToolKind::WoodShovel),
        (OldTool::WoodSword, ToolKind::WoodSword),
        (OldTool::StoneSword, ToolKind::StoneSword),
    ] {
        assert_eq!(
            decode::<ToolKind>(b"TEST", &encode(b"TEST", &old).unwrap()).unwrap(),
            tool
        );
    }
}

#[test]
fn every_new_color_and_tool_round_trips_existing_region_and_player_formats() {
    use terra_core::{
        ArmorMaterial, CapeItem, EquipmentSlot, ItemKind, ToolKind,
        beds::{DyeColor, bed_color, dyed_facing},
    };
    use terra_world::edits::BlockObjectState;
    let root = root();
    let world = WorldId(108);
    let mut journal = EditJournal::default();
    for (index, color) in DyeColor::ALL.into_iter().enumerate() {
        let anchor = [-16 - index as i64 * 4, -3, -17];
        let peer = [anchor[0] - 1, anchor[1], anchor[2]];
        let facing = dyed_facing(3, color);
        for (position, other, secondary) in [(anchor, peer, false), (peer, anchor, true)] {
            journal.record(position, 71).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        facing,
                        peer: Some(other),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        let player = PlayerUuid::from_u128(index as u128 + 1);
        let inventory = journal
            .gameplay_mut()
            .inventories
            .entry(player)
            .or_default();
        assert_eq!(inventory.insert(ItemKind::Dye(color), 1), 0);
        assert_eq!(
            inventory.insert(
                ItemKind::DyedBed {
                    material: 71,
                    color
                },
                1
            ),
            0
        );
        // Rotate the expanded catalogue across player files, retaining valid 27-slot bags.
        for tool in ToolKind::ALL.into_iter().cycle().skip(index * 25).take(25) {
            assert_eq!(inventory.insert(ItemKind::Tool(tool), 1), 0);
        }
        let mut stats = PlayerStats::default();
        assert!(stats.equipment.set(
            EquipmentSlot::Cape,
            Some(ItemKind::Cape(CapeItem {
                material: ArmorMaterial::Ruby,
                color
            }))
        ));
        stats.survival.apply_equipment(stats.equipment);
        assert!(journal.update_player_stats(player, stats));
    }
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    for (index, color) in DyeColor::ALL.into_iter().enumerate() {
        let anchor = [-16 - index as i64 * 4, -3, -17];
        let state = loaded.object_state(anchor).unwrap();
        assert_eq!(bed_color(71, state.facing), color);
        assert_eq!(state.facing % 4, 3);
        assert_eq!(
            loaded.object_state(state.peer.unwrap()).unwrap().facing,
            state.facing
        );
        let player = PlayerUuid::from_u128(index as u128 + 1);
        assert_eq!(
            loaded.gameplay().inventories[&player],
            journal.gameplay().inventories[&player]
        );
        assert_eq!(
            loaded.gameplay().player_stats[&player],
            journal.gameplay().player_stats[&player]
        );
        assert_eq!(
            &fs::read(root.join("players").join(format!("{player}.data"))).unwrap()[12..16],
            b"PLY7"
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gamerules_save_in_existing_world_metadata_for_both_world_layouts() {
    for library in [false, true] {
        let path = root();
        let mut config = terra_core::WorldConfig::default();
        let world = config.id;
        let entry = crate::WorldEntry {
            title: "Keep my title".into(),
            game_version: env!("CARGO_PKG_VERSION").into(),
            mode: terra_core::GameMode::Survival,
            last_played: Some(123),
            manifest: crate::WorldManifest::new(config.clone()),
        };
        if library {
            terra_core::save_toml(&entry, path.join("world.toml")).unwrap();
        } else {
            terra_core::save_toml(&entry.manifest, path.join("world.toml")).unwrap();
        }
        let mut clock = terra_core::WorldClock::default();
        for (rule, value) in [
            (terra_core::GameRule::SleepPercentage, 40.0),
            (terra_core::GameRule::SeasonSwapInterval, 1.0),
            (terra_core::GameRule::GameTimeSpeed, 0.5),
        ] {
            let settings =
                terra_core::WorldRuleSettings::set(&config, &mut clock, rule, value).unwrap();
            settings.apply(&mut config);
        }
        let mut journal = EditJournal::default();
        journal.clock = Some(clock);
        journal.world_rules = Some(terra_core::WorldRuleSettings::from_world(&config));
        journal.record([-1, 4, 1], 3).unwrap();
        save_edits(&path, world, &journal).unwrap();
        let text = fs::read_to_string(path.join("world.toml")).unwrap();
        let loaded = if library {
            let loaded: crate::WorldEntry = toml::from_str(&text).unwrap();
            assert_eq!(loaded.title, "Keep my title");
            assert_eq!(loaded.last_played, Some(123));
            loaded.manifest.world
        } else {
            toml::from_str::<crate::WorldManifest>(&text).unwrap().world
        };
        assert_eq!(loaded.game_rules, config.game_rules);
        assert_eq!(loaded.seasons, config.seasons);
        assert_eq!(load_edits(&path, world).unwrap().clock, Some(clock));
        if !library {
            assert_eq!(
                crate::DedicatedStore::resolve_world_config(
                    &path,
                    &terra_core::WorldConfig::default()
                )
                .unwrap()
                .game_rules,
                config.game_rules
            );
        }
        assert!(path.join("regions/r.-1.0.region").is_file());
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn mixed_slabs_and_eighth_snow_layers_roundtrip_without_changing_old_material_ids() {
    use terra_world::edits::BlockObjectState;
    let root = root();
    let world = WorldId(96);
    let mut journal = EditJournal::default();
    for (index, facing) in [0, 4, 5, 6, 7, 8].into_iter().enumerate() {
        let position = [-513 + index as i64, 15, -1];
        journal.record(position, 82).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    facing: terra_core::construction::merged_slab(82, facing, 85).unwrap(),
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    for state in 1..=8 {
        let position = [-513 + i64::from(state), 16, -1];
        journal
            .record(position, if state == 8 { 5 } else { 98 })
            .unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    facing: state,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    for (position, id, state) in journal.object_states() {
        assert_eq!(loaded.block(position), Some(id));
        assert_eq!(loaded.object_state(position), Some(state));
    }
    assert_eq!(loaded.object_states().count(), 14);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn wool_mixed_slab_primary_and_secondary_states_survive_journal_updates_region_and_replica_reload()
{
    use terra_world::edits::BlockObjectState;
    let root = root();
    let world = WorldId(122);
    let mut journal = EditJournal::default();
    let mut expected = Vec::new();
    let mut wool_states = std::collections::BTreeSet::new();
    for (first, second) in [(105, 82), (105, 85), (82, 105), (85, 105)] {
        for facing in [0, 4, 5, 6, 7, 8] {
            let position = [-513 + expected.len() as i64, 15, -1];
            let combined = terra_core::construction::merged_slab(first, facing, second).unwrap();
            if second == 105 {
                wool_states.insert(combined);
            }
            let state = BlockObjectState {
                facing: combined,
                ..Default::default()
            };
            journal.record(position, first).unwrap();
            journal.set_object_state(position, Some(state)).unwrap();
            // Recording an unchanged block must retain its form metadata. This
            // exercises the real journal gate, not just the facing-byte codec.
            journal.record(position, first).unwrap();
            assert_eq!(journal.object_state(position), Some(state));
            assert_eq!(
                terra_core::construction::slab_halves(first, combined).unwrap()[0],
                (first, facing)
            );
            expected.push((position, first, state));
        }
    }
    assert_eq!(wool_states, (58..=63).collect());
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    let mut replica = EditJournal::default();
    let owners: std::collections::BTreeSet<_> = expected
        .iter()
        .map(|(position, _, _)| terra_voxel::global_to_section(*position).0)
        .collect();
    for owner in owners {
        replica.import_delta(loaded.section_delta(owner)).unwrap();
    }
    for (position, material, state) in expected {
        for copy in [&loaded, &replica] {
            assert_eq!(copy.block(position), Some(material));
            assert_eq!(copy.object_state(position), Some(state));
            assert!(state.valid_for(position, material));
            assert_eq!(
                terra_core::construction::slab_halves(material, state.facing),
                terra_core::construction::slab_halves(
                    material,
                    copy.object_state(position).unwrap().facing
                )
            );
        }
    }
    assert_eq!(loaded.object_states().count(), 24);
    assert_eq!(replica.object_states().count(), 24);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn command_spawned_creatures_persist_without_natural_habitat_records() {
    use terra_core::creatures::*;
    let root = root();
    let world = WorldId(55);
    let mut journal = EditJournal::default();
    let id = spawning::next_command_id(&journal.gameplay().creatures).unwrap();
    let animal = Creature {
        id,
        home: [-17, 1],
        kind: CreatureKind::Fish,
        position: [-540.5, 3.0, 35.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: CreatureKind::Fish.health(),
        behavior: CreatureBehavior::Swim,
        timer: 1.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::Fish),
    };
    journal.gameplay_mut().creatures.animals.insert(id, animal);
    assert!(journal.gameplay().creatures.valid());
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().creatures, journal.gameplay().creatures);
    assert!(loaded.gameplay().creatures.explored.is_empty());
    assert_ne!(
        spawning::next_command_id(&loaded.gameplay().creatures),
        Some(id)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn settlement_residents_keep_identity_role_needs_sleep_pose_and_deaths_across_saves() {
    use terra_core::creatures::*;
    let root = root();
    let world = WorldId(126);
    let mut journal = EditJournal::default();
    for slot in 0..10 {
        let home = [-17, 19];
        let id = villagers::id(home, slot).unwrap();
        journal.gameplay_mut().creatures.animals.insert(
            id,
            Creature {
                id,
                home,
                kind: CreatureKind::Villager,
                position: [-540.1 + f64::from(slot), 3.79, 610.5],
                velocity: [0.0; 3],
                yaw: 1.57,
                health: if slot == 9 { 0.0 } else { 17.0 },
                behavior: if slot == 9 {
                    CreatureBehavior::Dead
                } else {
                    CreatureBehavior::Sleep
                },
                timer: 0.0,
                age: 917.5,
                variant: slot as u8,
                needs: CreatureNeeds {
                    food: 19.0,
                    water: 12.0,
                    ..CreatureNeeds::full(CreatureKind::Villager)
                },
            },
        );
    }
    assert!(journal.gameplay().creatures.valid());
    assert_eq!(journal.gameplay().creatures.natural_count(), 0);
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().creatures, journal.gameplay().creatures);
    assert!(loaded.gameplay().creatures.explored.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn creature_regions_preserve_crossed_boundaries_dead_identities_and_products() {
    use terra_core::creatures::*;
    let root = root();
    let world = WorldId(55);
    let mut journal = EditJournal::default();
    let home = [-17, 0];
    let id = creature_id(home, 0).unwrap();
    let mut animal = Creature {
        id,
        home,
        kind: CreatureKind::Sheep,
        position: [2.0, 3.0, -513.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: 7.0,
        behavior: CreatureBehavior::Graze,
        timer: 2.0,
        age: 45.0,
        variant: 2,
        needs: CreatureNeeds {
            food: 12.5,
            water: 8.0,
            ..CreatureNeeds::full(CreatureKind::Sheep)
        },
    };
    journal.gameplay_mut().creatures.explored.insert(home);
    journal
        .gameplay_mut()
        .creatures
        .animals
        .insert(id, animal.clone());
    assert_eq!(journal.gameplay().creatures.natural_count(), 1);
    journal
        .gameplay_mut()
        .creatures
        .owners
        .insert(id, PlayerUuid::new_v4());
    assert_eq!(journal.gameplay().creatures.natural_count(), 0);
    journal.gameplay_mut().drop_one(
        animal.position,
        terra_core::ItemKind::Animal(AnimalProduct::Wool),
    );
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    animal.health = 0.0;
    animal.behavior = CreatureBehavior::Dead;
    journal.gameplay_mut().creatures.animals.insert(id, animal);
    save_edits(&root, world, &journal).unwrap();
    assert_eq!(
        load_edits(&root, world).unwrap().gameplay(),
        journal.gameplay()
    );
    assert_eq!(
        &fs::read(root.join(region_name([-2, 0]))).unwrap()[12..16],
        b"RGA3"
    );
    fs::remove_dir_all(root).unwrap();
}

fn root() -> PathBuf {
    std::env::temp_dir().join(format!("terra-regions-{}", PlayerUuid::new_v4()))
}

#[test]
fn crops_seeds_and_grazed_forage_roundtrip_across_negative_section_and_region_seams() {
    use terra_core::{CropItem, CropKind, CropState, ItemKind};
    use terra_world::{
        farming::ForageRegrowth,
        props::{PropId, PropKind, WorldProp},
    };
    let root = root();
    let world = WorldId(109);
    let player = PlayerUuid::new_v4();
    let mut journal = EditJournal::default();
    for (index, kind) in CropKind::ALL.into_iter().enumerate() {
        let support = [-513 + index as i64 * 513, 15, -1];
        journal
            .record(support, terra_core::farming::FARMLAND)
            .unwrap();
        let state = CropState {
            kind,
            prop_id: 1090 + index as u64,
            progress_seconds: kind.definition().growth_seconds * 0.63,
            moisture: 0.71,
            growth_duration_seconds: kind.definition().growth_seconds,
        };
        let prop = WorldProp {
            id: PropId(state.prop_id),
            kind: kind.resource_prop().into(),
            position: [support[0] as f64 + 0.5, 16.0, -0.5],
            rotation: 0.0,
            scale: 1.0,
            variant: state.stage(),
            section: terra_voxel::global_to_section(support).0,
            removed: false,
        };
        assert!(journal.place_prop(prop).unwrap());
        assert!(journal.set_crop(support, state).unwrap());
        journal
            .gameplay_mut()
            .inventories
            .entry(player)
            .or_default()
            .insert(ItemKind::Crop(kind.seed()), 12);
        journal
            .gameplay_mut()
            .inventories
            .entry(player)
            .or_default()
            .insert(ItemKind::Crop(kind.produce()), 3);
    }
    let forage_support = [-513, 15, 2];
    let prop = WorldProp {
        id: PropId(10_901),
        kind: PropKind::Grass,
        position: [-512.5, 16.0, 2.5],
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section: terra_voxel::global_to_section(forage_support).0,
        removed: false,
    };
    journal.remove_prop(prop.section, prop.id).unwrap();
    journal
        .set_forage_regrowth(ForageRegrowth {
            prop,
            remaining_seconds: 123.5,
        })
        .unwrap();
    journal
        .gameplay_mut()
        .inventories
        .entry(player)
        .or_default()
        .insert(ItemKind::Crop(CropItem::Bread), 2);
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.crops().count(), 3);
    for (support, state) in journal.crops() {
        assert_eq!(loaded.crop_at(support), Some(state));
    }
    assert_eq!(
        loaded.forage_regrowth().copied().collect::<Vec<_>>(),
        journal.forage_regrowth().copied().collect::<Vec<_>>()
    );
    assert_eq!(loaded.gameplay(), journal.gameplay());
    assert_eq!(
        &read(&root.join(region_name([-2, -1]))).unwrap()[12..16],
        b"RGA3"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rga1_regions_gain_empty_farming_state_without_losing_prior_contents() {
    let root = root();
    let world = WorldId(1091);
    let mut journal = EditJournal::default();
    journal.record([2, 8, 3], 29).unwrap();
    let mut inventory = Inventory::default();
    inventory.insert(terra_core::ItemKind::Stick, 11);
    journal.set_container([2, 8, 3], inventory.clone()).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(region_name([0, 0]));
    let current = current_farming(&read(&path).unwrap());
    fs::write(&path, encode(b"RGA1", &current.prior).unwrap()).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.container_at([2, 8, 3]), Some(&inventory));
    assert_eq!(loaded.crops().count(), 0);
    assert_eq!(loaded.forage_regrowth().count(), 0);
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(&read(&path).unwrap()[12..16], b"RGA3");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_or_orphaned_crop_records_fail_closed_without_altering_the_saved_file() {
    use terra_core::{CropKind, CropState};
    use terra_world::props::{PropId, WorldProp};
    let root = root();
    let world = WorldId(1092);
    let support = [3, 8, 4];
    let mut journal = EditJournal::default();
    journal
        .record(support, terra_core::farming::FARMLAND)
        .unwrap();
    let state = CropState::new(CropKind::Wheat, 1_092);
    journal
        .place_prop(WorldProp {
            id: PropId(state.prop_id),
            kind: state.kind.resource_prop().into(),
            position: [3.5, 9.0, 4.5],
            rotation: 0.0,
            scale: 1.0,
            variant: 0,
            section: terra_voxel::global_to_section(support).0,
            removed: false,
        })
        .unwrap();
    journal.set_crop(support, state).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(region_name([0, 0]));
    let valid = read(&path).unwrap();
    for invalid in 0..6 {
        let mut data = current_farming(&valid);
        match invalid {
            0 => data.crops.push(data.crops[0]),
            1 => data.crops[0].1.moisture = f32::NAN,
            2 => data.crops[0].1.progress_seconds = 1_000_000.0,
            3 => data.crops[0].1.prop_id += 1,
            4 => data.crops[0].1.kind = CropKind::Lettuce,
            _ => data.crops[0].0 = [600, 8, 4],
        }
        let corrupted = encode(b"RGA2", &data).unwrap();
        fs::write(&path, &corrupted).unwrap();
        assert!(
            load_edits(&root, world).is_err(),
            "invalid crop case {invalid}"
        );
        assert_eq!(read(&path).unwrap(), corrupted);
    }
    fs::write(&path, valid).unwrap();
    assert_eq!(
        load_edits(&root, world).unwrap().crop_at(support),
        Some(&state)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn crusher_private_slots_progress_and_horizontal_seam_pair_round_trip() {
    use terra_core::{ItemKind, ItemStack, MineralItem, crusher};
    use terra_world::edits::BlockObjectState;
    let root = root();
    let world = WorldId(106);
    let anchor = [-1, 12, -1];
    let peer = [0, 12, -1];
    let mut journal = EditJournal::default();
    for (position, other, secondary) in [(anchor, peer, false), (peer, anchor, true)] {
        journal.record(position, 99).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    peer: Some(other),
                    secondary,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let mut state = crusher::CrusherState::default();
    let mut inventory = Inventory {
        slots: vec![ItemStack {
            item: ItemKind::Mineral(MineralItem::RawIron),
            count: 3,
        }],
        ..Default::default()
    };
    crusher::advance(&mut state, &mut inventory, 0.75);
    journal
        .commit_crusher(anchor, state.clone(), inventory.clone())
        .unwrap();
    let crank = [1, 12, -1];
    journal.record(crank, 100).unwrap();
    journal
        .set_object_state(
            crank,
            Some(BlockObjectState {
                peer: Some(anchor),
                ..Default::default()
            }),
        )
        .unwrap();
    journal.replace_crusher_drives([anchor].into_iter().collect());
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.container_at(anchor), Some(&inventory));
    assert_eq!(loaded.crusher_at(anchor), Some(&state));
    assert!(loaded.valid_crusher(anchor));
    assert_eq!(loaded.crank_anchor(crank), Some(anchor));
    assert!(!loaded.crusher_driven(anchor));
    assert_eq!(
        loaded
            .crushers_in_section(terra_voxel::global_to_section(anchor).0)
            .collect::<Vec<_>>(),
        vec![anchor]
    );
    assert!(loaded.container_at(peer).is_none());
    let mut continued = inventory;
    let mut reloaded = loaded.container_at(anchor).unwrap().clone();
    let mut continued_state = state;
    let mut reloaded_state = loaded.crusher_at(anchor).unwrap().clone();
    for _ in 0..5 {
        crusher::advance(&mut continued_state, &mut continued, 1.0);
        crusher::advance(&mut reloaded_state, &mut reloaded, 1.0);
    }
    assert_eq!(continued, reloaded);
    assert_eq!(continued_state, reloaded_state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rga0_furnaces_migrate_to_rga2_without_changing_prior_private_state() {
    let root = root();
    let world = WorldId(1061);
    let anchor = [5, 20, 3];
    let mut journal = furnace_journal(anchor);
    journal.set_furnace(anchor, Default::default()).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(region_name([0, 0]));
    let current: CrusherRegion = current_farming(&read(&path).unwrap()).prior;
    fs::write(&path, encode(b"RGA0", &current.prior).unwrap()).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.furnace_at(anchor), journal.furnace_at(anchor));
    assert_eq!(loaded.gameplay(), journal.gameplay());
    assert_eq!(loaded.crushers().count(), 0);
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(&read(&path).unwrap()[12..16], b"RGA3");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn dyed_bed_pairs_shapes_and_inventory_roundtrip_across_negative_region_boundaries() {
    use terra_core::{
        ItemKind,
        beds::{DyeColor, dyed_facing},
    };
    use terra_world::edits::BlockObjectState;
    let root = root();
    let world = WorldId(94);
    let mut journal = EditJournal::default();
    let a = [-513, 40, -1];
    let b = [-513, 40, 0];
    for (position, peer, secondary) in [(a, b, false), (b, a, true)] {
        journal.record(position, 71).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    facing: dyed_facing(0, DyeColor::Purple),
                    peer: Some(peer),
                    secondary,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let stair = [-512, 40, 0];
    journal.record(stair, 74).unwrap();
    journal
        .set_object_state(
            stair,
            Some(BlockObjectState {
                facing: 1,
                ..Default::default()
            }),
        )
        .unwrap();
    let player = PlayerUuid::new_v4();
    let bag = journal
        .gameplay_mut()
        .inventories
        .entry(player)
        .or_default();
    bag.insert(
        ItemKind::DyedBed {
            material: 71,
            color: DyeColor::Purple,
        },
        3,
    );
    bag.insert(ItemKind::Dye(DyeColor::Purple), 2);
    bag.insert(ItemKind::Block(97), 16);
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    for position in [a, b, stair] {
        assert_eq!(loaded.block(position), journal.block(position));
        assert_eq!(
            loaded.object_state(position),
            journal.object_state(position)
        );
    }
    assert_eq!(
        loaded.gameplay().inventories[&player],
        journal.gameplay().inventories[&player]
    );
    assert_eq!(
        &fs::read(root.join(region_name([-2, -1]))).unwrap()[12..16],
        b"RGA3"
    );
    fs::remove_dir_all(root).unwrap();
}

fn furnace_journal(position: [i64; 3]) -> EditJournal {
    let mut journal = EditJournal::default();
    let upper = [position[0], position[1] + 1, position[2]];
    for (cell, peer, secondary) in [(position, upper, false), (upper, position, true)] {
        journal.record(cell, 59).unwrap();
        journal
            .set_object_state(
                cell,
                Some(terra_world::edits::BlockObjectState {
                    facing: 2,
                    peer: Some(peer),
                    secondary,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    journal
}

#[test]
fn opening_states_and_all_adaptive_chimney_poses_survive_region_reload() {
    use terra_world::edits::BlockObjectState;
    let root = root();
    let world = WorldId(805);
    let mut journal = EditJournal::default();
    for id in 65..=70 {
        let position = [-513 + i64::from(id - 65) * 3, 15, -1];
        let peer = [position[0], 16, -1];
        let state = BlockObjectState {
            facing: 4 + (id % 4) as u8,
            peer: (id <= 67).then_some(peer),
            ..Default::default()
        };
        journal.record(position, id).unwrap();
        journal.set_object_state(position, Some(state)).unwrap();
        if id <= 67 {
            journal.record(peer, id).unwrap();
            journal
                .set_object_state(
                    peer,
                    Some(BlockObjectState {
                        peer: Some(position),
                        secondary: true,
                        ..state
                    }),
                )
                .unwrap();
        }
    }
    let poses = (8..23)
        .chain((40..55).filter(|&pose| terra_world::objects::chimney_ports(60, pose).is_some()));
    for facing in poses {
        let position = [-513, 30, -40 + i64::from(facing)];
        journal.record(position, 60).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    for (position, id, state) in journal.object_states() {
        assert_eq!(loaded.block(position), Some(id));
        assert_eq!(loaded.object_state(position), Some(state));
    }
    assert_eq!(loaded.object_states().count(), 36);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn furnace_progress_fuel_bonus_ordinal_and_room_smoke_roundtrip_exactly() {
    use terra_core::{BellowsKind, ItemKind, ItemStack, MineralItem, furnace};
    let root = root();
    let world = WorldId(804);
    let position = [-513, 15, -1];
    let mut journal = furnace_journal(position);
    let recipe = furnace::recipe_for(ItemKind::Mineral(MineralItem::RawIron)).unwrap();
    let inventory = Inventory {
        slots: vec![
            ItemStack {
                item: recipe.input,
                count: 4,
            },
            ItemStack {
                item: ItemKind::Mineral(MineralItem::Coal),
                count: 2,
            },
            ItemStack {
                item: recipe.output.item,
                count: 3,
            },
            ItemStack {
                item: ItemKind::Bellows(BellowsKind::Ruby),
                count: 1,
            },
        ],
        revision: 17,
        ..Default::default()
    };
    let state = furnace::FurnaceState {
        progress_seconds: 3.25,
        recipe: Some(recipe.id),
        burn_remaining: 19.625,
        burn_total: 30.0,
        completed: 41,
        smoke_credit: -0.375,
        outlet: [-513, 17, -1],
        vented: false,
        smoke_cells: vec![[-513, 17, -1], [-512, 17, -1]],
    };
    journal.set_container(position, inventory.clone()).unwrap();
    journal.set_furnace(position, state.clone()).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.furnace_at(position), Some(&state));
    assert_eq!(loaded.container_at(position), Some(&inventory));
    assert_eq!(loaded.gameplay().revision, journal.gameplay().revision);
    assert_eq!(
        loaded.object_state([position[0], position[1] + 1, position[2]]),
        journal.object_state([position[0], position[1] + 1, position[2]])
    );
    let mut continued_state = state.clone();
    let mut continued_inventory = inventory.clone();
    let mut reloaded_state = loaded.furnace_at(position).unwrap().clone();
    let mut reloaded_inventory = loaded.container_at(position).unwrap().clone();
    for _ in 0..12 {
        furnace::advance(&mut continued_state, &mut continued_inventory, 0.75, 99);
        furnace::advance(&mut reloaded_state, &mut reloaded_inventory, 0.75, 99);
    }
    assert_eq!(reloaded_state, continued_state);
    assert_eq!(reloaded_inventory, continued_inventory);
    assert!(reloaded_state.completed > state.completed);
    assert_eq!(
        &read(&root.join(region_name(region_at(position)))).unwrap()[12..16],
        b"RGA3"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn empty_furnace_state_roundtrips_without_a_materialized_container_record() {
    let root = root();
    let world = WorldId(805);
    let position = [5, 20, 3];
    let mut journal = furnace_journal(position);
    journal.set_furnace(position, Default::default()).unwrap();
    assert!(journal.container_at(position).is_none());
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert!(loaded.container_at(position).is_none());
    assert_eq!(loaded.furnace_at(position), journal.furnace_at(position));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reg9_regions_migrate_to_rga2_without_changing_prior_world_state() {
    let root = root();
    let world = WorldId(806);
    let position = [5, 20, 3];
    let mut journal = EditJournal::default();
    journal.record(position, 29).unwrap();
    let mut inventory = Inventory::default();
    inventory.insert(terra_core::ItemKind::Stick, 12);
    journal.set_container(position, inventory.clone()).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(region_name([0, 0]));
    let current: CrusherRegion = current_farming(&read(&path).unwrap()).prior;
    fs::write(&path, encode(b"REG9", &current.prior.prior).unwrap()).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.container_at(position), Some(&inventory));
    assert_eq!(loaded.gameplay(), journal.gameplay());
    assert_eq!(loaded.furnaces().count(), 0);
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(&read(&path).unwrap()[12..16], b"RGA3");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_duplicate_misplaced_and_orphaned_furnace_records_fail_closed() {
    let root = root();
    let world = WorldId(807);
    let position = [5, 20, 3];
    let mut journal = furnace_journal(position);
    journal.set_furnace(position, Default::default()).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(region_name([0, 0]));
    let valid = read(&path).unwrap();
    for invalid in 0..7 {
        let mut data: FurnaceRegion = current_farming(&valid).prior.prior;
        match invalid {
            0 => data.furnaces.push(data.furnaces[0].clone()),
            1 => data.furnaces[0].0 = [513, 20, 3],
            2 => data.furnaces[0].0 = [5, 22, 3],
            3 => data.furnaces[0].0 = [5, 21, 3],
            4 => data.furnaces[0].1.progress_seconds = f32::NAN,
            5 => data.furnaces[0].1.burn_remaining = 5.0,
            _ => data.furnaces[0].1.smoke_cells = vec![[5, 22, 3]; 2],
        }
        fs::write(&path, encode(b"RGA0", &data).unwrap()).unwrap();
        assert!(
            load_edits(&root, world).is_err(),
            "invalid furnace case {invalid}"
        );
    }
    fs::write(&path, valid).unwrap();
    assert_eq!(load_edits(&root, world).unwrap().furnaces().count(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cave_segments_cuts_waterlogged_sources_and_falling_crystals_roundtrip_in_existing_regions() {
    use terra_core::ItemKind;
    let root = root();
    let world = WorldId(800);
    let mut journal = EditJournal::default();
    for material in 50..=56 {
        journal
            .record([-513, material as i64 - 70, -17], material)
            .unwrap();
        journal.gameplay_mut().drop_one(
            [-512.5, material as f64 - 68.5, -16.5],
            ItemKind::Block(material),
        );
    }
    let p = [-514, -16, -17];
    let water = terra_world::water::FluidCell {
        amount: 8,
        source: true,
        flow: [0; 2],
    };
    journal.set_fluid(p, water).unwrap();
    journal.record(p, 50).unwrap();
    journal.record(p, 0).unwrap();
    let drop = &mut journal.gameplay_mut().drops[1];
    drop.velocity = [0.0, -8.0, 0.0];
    drop.rotation = [0.0, 0.0, 0.0, 1.0];
    drop.angular_velocity = [0.0; 3];
    drop.pickup_delay_seconds = 1.0;
    let before = journal.gameplay().clone();
    let removed_source = [-515, -16, -17];
    journal.record(removed_source, 0).unwrap();
    journal
        .set_fluid(removed_source, terra_world::water::FluidCell::default())
        .unwrap();
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.block(p), Some(0));
    assert_eq!(loaded.fluid(p), Some(water));
    assert_eq!(
        loaded.fluid(removed_source),
        Some(terra_world::water::FluidCell::default())
    );
    for material in 50..=56 {
        assert_eq!(
            loaded.block([-513, material as i64 - 70, -17]),
            Some(material)
        );
    }
    assert_eq!(loaded.gameplay(), &before);
    assert_eq!(
        &fs::read(root.join(region_name([-2, -1]))).unwrap()[12..16],
        b"RGA3"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ice_edits_and_inventory_stacks_roundtrip_with_legacy_block_ids() {
    let root = root();
    let world = WorldId(790);
    let player = PlayerUuid::new_v4();
    let mut journal = EditJournal::default();
    journal
        .record([-17, 8, -513], terra_voxel::MaterialId::Ice as u16)
        .unwrap();
    journal
        .record([0, 7, 0], terra_voxel::MaterialId::Stone as u16)
        .unwrap();
    journal
        .gameplay_mut()
        .inventories
        .entry(player)
        .or_default()
        .insert(terra_core::ItemKind::Block(49), 3);
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.block([-17, 8, -513]), Some(49));
    assert_eq!(loaded.block([0, 7, 0]), Some(3));
    assert_eq!(
        loaded.gameplay().inventories[&player].count(terra_core::ItemKind::Block(49)),
        3
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn all_prop_kinds_items_and_removal_tombstones_persist_in_binary_regions() {
    use terra_core::{ItemKind, ResourcePropKind};
    use terra_world::props::{PropId, PropKind, WorldProp};
    let root = root();
    let world = WorldId(780);
    let player = PlayerUuid::new_v4();
    let mut journal = EditJournal::default();
    let mut originals = Vec::new();
    for (index, kind) in PropKind::ALL.into_iter().enumerate() {
        let x = index as i64 - 7;
        let support = [x, 4, 0];
        let mut prop = WorldProp {
            id: PropId(500 + index as u64),
            kind,
            position: [x as f64 + 0.5, 5.0, 0.5],
            rotation: 0.0,
            scale: 1.0,
            variant: 0,
            section: terra_voxel::global_to_section(support).0,
            removed: false,
        };
        if kind == PropKind::SpiderWeb {
            prop = terra_world::props::webs::attached(support, [0, 1, 0], 5, prop.id).unwrap();
        }
        assert!(journal.place_prop(prop).unwrap());
        originals.push(prop);
        let item = ItemKind::Prop(ResourcePropKind::from(kind));
        assert_eq!(PropKind::from(ResourcePropKind::from(kind)), kind);
        journal
            .gameplay_mut()
            .inventories
            .entry(player)
            .or_default()
            .insert(item, 1);
        assert!(journal.gameplay_mut().drop_one(prop.position, item));
    }
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    for prop in &originals {
        let mut props = Vec::new();
        loaded.apply_prop_removals(prop.section, &mut props);
        assert!(props.iter().any(|p| p.id == prop.id && p.kind == prop.kind));
        assert!(journal.remove_prop(prop.section, prop.id).unwrap());
    }
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    for original in originals {
        assert!(loaded.prop_removed(original.section, original.id));
        let mut props = vec![original];
        loaded.apply_prop_removals(original.section, &mut props);
        assert!(props[0].removed);
        assert!(
            loaded
                .section_delta(original.section)
                .placed_props
                .iter()
                .all(|p| p.id != original.id)
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn all_aquatic_species_and_ink_hold_states_round_trip_in_existing_region_layout() {
    use terra_core::creatures::*;
    let root = root();
    let mut journal = EditJournal::default();
    for (index, kind) in CreatureKind::ALL
        .into_iter()
        .enumerate()
        .filter(|(_, k)| k.is_aquatic())
    {
        let home = [index as i64 - 16, 0];
        let id = creature_id(home, 0).unwrap();
        journal.gameplay_mut().creatures.explored.insert(home);
        journal.gameplay_mut().creatures.animals.insert(
            id,
            Creature {
                id,
                kind,
                home,
                position: [home[0] as f64 * 32.0, 5.0, 0.0],
                velocity: [0.1, 0.0, -0.2],
                yaw: 0.5,
                health: kind.health(),
                behavior: if kind == CreatureKind::Squid {
                    CreatureBehavior::Ink
                } else if kind.is_jelly() {
                    CreatureBehavior::Rest
                } else {
                    CreatureBehavior::Swim
                },
                timer: 0.6,
                age: 12.0,
                variant: 0,
                needs: CreatureNeeds::full(kind),
            },
        );
    }
    assert!(journal.gameplay().creatures.valid());
    save_edits(&root, WorldId(78), &journal).unwrap();
    let loaded = load_edits(&root, WorldId(78)).unwrap();
    assert_eq!(loaded.gameplay().creatures, journal.gameplay().creatures);
    for (id, animal) in &loaded.gameplay().creatures.animals {
        assert_eq!(
            animal.aquatic_scale(),
            journal.gameplay().creatures.animals[id].aquatic_scale()
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn spider_attachment_venom_species_and_silk_anchor_round_trip_without_a_new_save_layout() {
    use terra_core::creatures::*;
    let root = root();
    let mut journal = EditJournal::default();
    for (species, kind) in [
        CreatureKind::SmallSpider,
        CreatureKind::MediumSpider,
        CreatureKind::LargeSpider,
    ]
    .into_iter()
    .enumerate()
    {
        for (pose, behavior) in [
            CreatureBehavior::Crawl,
            CreatureBehavior::Climb,
            CreatureBehavior::Ceiling,
            CreatureBehavior::Descend,
        ]
        .into_iter()
        .enumerate()
        {
            let home = [-16 + species as i64, -8 + pose as i64];
            let id = spider_id(home, species as u8).unwrap();
            journal.gameplay_mut().creatures.animals.insert(
                id,
                Creature {
                    id,
                    kind,
                    home,
                    position: [-500.5, -30.0, -200.5],
                    velocity: [0.0, -0.55, 0.0],
                    yaw: std::f32::consts::FRAC_PI_2,
                    health: kind.health(),
                    behavior,
                    timer: 0.75,
                    age: 140.0,
                    variant: 2,
                    needs: CreatureNeeds {
                        goal: (behavior == CreatureBehavior::Descend).then_some(NeedGoal {
                            kind: NeedKind::Food,
                            position: [-501, -24, -201],
                        }),
                        ..CreatureNeeds::full(kind)
                    },
                },
            );
        }
    }
    assert!(journal.gameplay().creatures.valid());
    save_edits(&root, WorldId(111), &journal).unwrap();
    let loaded = load_edits(&root, WorldId(111)).unwrap();
    assert_eq!(loaded.gameplay().creatures, journal.gameplay().creatures);
    assert_eq!(loaded.gameplay().creatures.animals.len(), 12);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn bear_climb_and_hunger_round_trip_without_changing_region_layout() {
    use terra_core::creatures::*;
    let root = root();
    let mut journal = EditJournal::default();
    let home = [-2, 1];
    let id = creature_id(home, 0).unwrap();
    journal.gameplay_mut().creatures.explored.insert(home);
    journal.gameplay_mut().creatures.animals.insert(
        id,
        Creature {
            id,
            kind: CreatureKind::Bear,
            home,
            position: [-48.0, 24.5, 48.0],
            velocity: [0.0, -1.2, 0.0],
            yaw: 0.2,
            health: 24.0,
            behavior: CreatureBehavior::Climb,
            timer: 4.0,
            age: 139.0,
            variant: 2,
            needs: CreatureNeeds {
                food: 13.0,
                water: 11.0,
                search_in: 4.0,
                goal: Some(NeedGoal {
                    kind: NeedKind::Tree,
                    position: [-49, 21, 48],
                }),
            },
        },
    );
    assert!(journal.gameplay().creatures.valid());
    save_edits(&root, WorldId(67), &journal).unwrap();
    let loaded = load_edits(&root, WorldId(67)).unwrap();
    assert_eq!(loaded.gameplay().creatures, journal.gameplay().creatures);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reg8_adds_an_empty_ownership_map_without_losing_ecology() {
    let old = Reg8 {
        region: Region {
            world: WorldId(60),
            position: [0, 0],
            sections: vec![],
            drops: vec![],
            containers: vec![],
            furniture_order: vec![],
        },
        creatures: Population8 {
            animals: BTreeMap::new(),
            explored: BTreeSet::new(),
            ecology: terra_core::creatures::ecology::Ecology {
                seconds: 128.0,
                ..Default::default()
            },
        },
    };
    let migrated = read_creature_region(&encode(b"REG8", &old).unwrap()).unwrap();
    assert!(migrated.creatures.owners.is_empty());
    assert_eq!(migrated.creatures.ecology.seconds, 128.0);
    assert_eq!(migrated.region.world, WorldId(60));
}

#[test]
fn ecology_clock_uses_the_index_without_rewriting_unchanged_flower_regions() {
    let root = root();
    let world = WorldId(59);
    let mut journal = EditJournal::default();
    journal
        .gameplay_mut()
        .creatures
        .ecology
        .take_nectar(59, [-0.5, 2.0, -0.5], 50);
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(region_name([-1, -1]));
    let before = fs::metadata(&path).unwrap().modified().unwrap();
    for _ in 0..10 {
        journal.gameplay_mut().creatures.ecology.tick(0.1);
    }
    save_edits(&root, world, &journal).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
    assert_eq!(
        load_edits(&root, world).unwrap().gameplay(),
        journal.gameplay()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn idx2_calendar_upgrades_to_idx4_with_no_world_or_region_loss() {
    let root = root();
    let world = WorldId(59);
    let mut journal = EditJournal::default();
    journal.record([1, 2, 3], 3).unwrap();
    journal.clock = Some(terra_core::WorldClock::default());
    save_edits(&root, world, &journal).unwrap();
    let data = read_index(&root).unwrap();
    fs::write(
        root.join(INDEX),
        encode(
            b"IDX2",
            &Index2 {
                index: data.index,
                clock: data.clock,
            },
        )
        .unwrap(),
    )
    .unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.block([1, 2, 3]), Some(3));
    assert_eq!(loaded.clock, journal.clock);
    assert_eq!(loaded.gameplay().creatures.ecology.seconds, 0.0);
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(&fs::read(root.join(INDEX)).unwrap()[12..16], b"IDX6");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn bee_nest_torch_and_new_species_roundtrip_through_existing_region_format() {
    use terra_core::creatures::*;
    use terra_world::props::{PropId, PropKind, WorldProp};
    let root = root();
    let world = WorldId(58);
    let mut journal = EditJournal::default();
    journal.record([-1, 3, 2], 40).unwrap();
    journal
        .set_object_state(
            [-1, 3, 2],
            Some(terra_world::edits::BlockObjectState {
                facing: 4,
                ..Default::default()
            }),
        )
        .unwrap();
    let section = terra_voxel::SectionPos::new(-1, 0, 0);
    let nest = WorldProp {
        id: PropId(58),
        kind: PropKind::BeeNest,
        position: [-0.5, 4.0, 2.5],
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section,
        removed: false,
    };
    assert!(journal.place_prop(nest).unwrap());
    let home = [-1, 0];
    let id = bee_id(home, 0).unwrap();
    let bee = Creature {
        id,
        kind: CreatureKind::Bee,
        home,
        position: [-0.5, 4.8, 3.5],
        velocity: [0.1, 0.0, 0.0],
        yaw: 1.0,
        health: 2.0,
        behavior: CreatureBehavior::Fly,
        timer: 3.0,
        age: 5.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::Bee),
    };
    journal.gameplay_mut().creatures.explored.insert(home);
    journal.gameplay_mut().creatures.animals.insert(id, bee);
    {
        use terra_core::creatures::ecology::*;
        let eco = &mut journal.gameplay_mut().creatures.ecology;
        eco.seconds = 117.0;
        eco.take_nectar(123, [-0.5, 2.0, 2.5], 100);
        eco.nests.insert(
            nest.id.0,
            NestState {
                position: nest.position,
                nectar: 175,
                honey: 2,
                progress: 0.4,
            },
        );
        eco.bees.insert(
            id,
            BeeState {
                nest: nest.id.0,
                carried: 25,
                flower: Some(123),
                inside: false,
            },
        );
    }
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    assert_eq!(
        loaded.section_delta(section),
        journal.section_delta(section)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reg6_migrates_species_health_and_full_needs_without_resurrecting_dead_ids() {
    use creature_legacy::{PreviousCreature, PreviousCreatureRegion, PreviousPopulation};
    use terra_core::creatures::*;
    let world = WorldId(56);
    let home = [-1, 0];
    let id = creature_id(home, 0).unwrap();
    let dead_id = creature_id(home, 1).unwrap();
    let old = |id, health, behavior| PreviousCreature {
        id,
        kind: CreatureKind::Cow,
        home,
        position: [-8.0, 3.0, 8.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health,
        behavior,
        timer: 3.0,
        age: 9.0,
        variant: 1,
    };
    let bytes = encode(
        b"REG6",
        &PreviousCreatureRegion {
            region: Region {
                world,
                position: [-1, 0],
                sections: vec![],
                drops: vec![],
                containers: vec![],
                furniture_order: vec![],
            },
            creatures: PreviousPopulation {
                animals: BTreeMap::from([
                    (id, old(id, 10.0, CreatureBehavior::Wander)),
                    (dead_id, old(dead_id, 0.0, CreatureBehavior::Dead)),
                ]),
                explored: BTreeSet::from([home, [-2, 0]]),
            },
        },
    )
    .unwrap();
    let data = read_creature_region(&bytes).unwrap();
    assert_eq!(read_region(&bytes).unwrap().world, world);
    assert_eq!(data.creatures.animals[&id].health, 5.0);
    assert_eq!(
        data.creatures.animals[&id].needs,
        CreatureNeeds::full(CreatureKind::Cow)
    );
    assert_eq!(data.creatures.animals[&dead_id].health, 0.0);
    assert!(data.creatures.explored.contains(&home));
    assert!(!data.creatures.explored.contains(&[-2, 0]));
    assert!(data.creatures.valid());
    let round_trip = read_creature_region(&encode(b"REG9", &data).unwrap()).unwrap();
    assert_eq!(round_trip.creatures, data.creatures);
}

#[test]
fn reg7_migrates_without_losing_existing_animals_and_starts_empty_ecology() {
    let region = Region {
        world: WorldId(59),
        position: [0, 0],
        sections: vec![],
        drops: vec![],
        containers: vec![],
        furniture_order: vec![],
    };
    let previous = Reg7 {
        region,
        creatures: Population7 {
            animals: BTreeMap::new(),
            explored: BTreeSet::from([[0, 0]]),
        },
    };
    let current = read_creature_region(&encode(b"REG7", &previous).unwrap()).unwrap();
    assert!(current.creatures.explored.contains(&[0, 0]));
    assert_eq!(current.creatures.ecology, Default::default());
    assert_eq!(current.region.world, WorldId(59));
}

#[test]
fn appended_mineral_and_tool_variants_preserve_existing_binary_item_discriminants() {
    #[derive(Serialize)]
    enum BeforeMinerals {
        Block(u16),
        Prop(terra_core::ResourcePropKind),
        Tool(terra_core::ToolKind),
        Stick,
        WoodenBlock {
            material: u16,
            wood: terra_core::WoodSpecies,
        },
    }
    use terra_core::{ItemKind, ResourcePropKind, ToolKind, WoodSpecies};
    for (old, expected) in [
        (BeforeMinerals::Block(29), ItemKind::Block(29)),
        (
            BeforeMinerals::Prop(ResourcePropKind::Grass),
            ItemKind::Prop(ResourcePropKind::Grass),
        ),
        (
            BeforeMinerals::Tool(ToolKind::StonePickaxe),
            ItemKind::Tool(ToolKind::StonePickaxe),
        ),
        (BeforeMinerals::Stick, ItemKind::Stick),
        (
            BeforeMinerals::WoodenBlock {
                material: 29,
                wood: WoodSpecies::Birch,
            },
            ItemKind::for_wood(29, WoodSpecies::Birch),
        ),
    ] {
        let bytes = encode(b"TEST", &old).unwrap();
        assert_eq!(decode::<ItemKind>(b"TEST", &bytes).unwrap(), expected);
    }
}

#[test]
fn ore_blocks_resources_tools_chest_contents_and_loose_minerals_roundtrip_without_new_save_format()
{
    use terra_core::{ItemKind, MineralItem, OreKind, ToolKind};
    let root = root();
    let world = WorldId(73);
    let uuid = PlayerUuid::new_v4();
    let mut journal = EditJournal::default();
    let mut inventory = Inventory::default();
    for (index, ore) in OreKind::ALL.into_iter().enumerate() {
        journal
            .record([-1, -20 - index as i64, -1], ore.material())
            .unwrap();
        inventory.insert(ItemKind::Mineral(ore.resource()), 7);
        journal.gameplay_mut().drop_one(
            [index as f64, -10.0, 1.0],
            ItemKind::Mineral(ore.resource()),
        );
    }
    inventory.insert(ItemKind::Tool(ToolKind::IronPickaxe), 1);
    inventory.cursor = Some(terra_core::ItemStack {
        item: ItemKind::Mineral(MineralItem::IronIngot),
        count: 3,
    });
    journal
        .gameplay_mut()
        .inventories
        .insert(uuid, inventory.clone());
    journal.record([1, 20, 3], 29).unwrap();
    let mut chest = Inventory::default();
    chest.insert(ItemKind::Mineral(MineralItem::GoldIngot), 128);
    journal.set_container([1, 20, 3], chest.clone()).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().inventories[&uuid], inventory);
    assert_eq!(loaded.gameplay().drops, journal.gameplay().drops);
    assert_eq!(loaded.container_at([1, 20, 3]), Some(&chest));
    for (index, ore) in OreKind::ALL.into_iter().enumerate() {
        assert_eq!(
            loaded.block([-1, -20 - index as i64, -1]),
            Some(ore.material())
        );
    }
    let bytes = fs::read(root.join(format!("players/{uuid}.data"))).unwrap();
    assert_eq!(&bytes[12..16], b"PLY7");
    assert_eq!(
        &fs::read(root.join(region_name([-1, -1]))).unwrap()[12..16],
        b"RGA3"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn furniture_placement_order_and_workbench_chest_selection_survive_cross_region_reload() {
    let root = root();
    let world = WorldId(73);
    let anchor = [-1, 20, -1];
    let second = [0, 20, -1];
    let oldest = [0, 20, 0];
    let newer = [-2, 20, -1];
    let mut journal = EditJournal::default();
    for (position, peer, secondary) in [(anchor, second, false), (second, anchor, true)] {
        journal.record(position, 28).unwrap();
        journal
            .set_object_state(
                position,
                Some(terra_world::edits::BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    journal.record(oldest, 29).unwrap();
    journal.record(newer, 29).unwrap();
    let old_orders: BTreeMap<_, _> = journal.furniture_orders().collect();
    save_edits(&root, world, &journal).unwrap();
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(
        loaded.furniture_orders().collect::<BTreeMap<_, _>>(),
        old_orders
    );
    assert_eq!(loaded.workbench_chest(second), Some((anchor, oldest)));
    loaded.record(oldest, 0).unwrap();
    loaded.record(oldest, 29).unwrap();
    assert_eq!(loaded.workbench_chest(anchor), Some((anchor, newer)));
    let new_order = loaded
        .furniture_orders()
        .find(|(position, _)| *position == oldest)
        .unwrap()
        .1;
    assert!(new_order > *old_orders.values().max().unwrap());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reg4_container_regions_load_and_migrate_without_losing_contents() {
    let root = root();
    let world = WorldId(73);
    let position = [1, 20, 3];
    let mut journal = EditJournal::default();
    journal.record(position, 29).unwrap();
    let mut inventory = Inventory::default();
    inventory.insert(terra_core::ItemKind::Stick, 31);
    journal.set_container(position, inventory.clone()).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(region_name([0, 0]));
    let current = read_region(&read(&path).unwrap()).unwrap();
    let old = ContainerRegion {
        world: current.world,
        position: current.position,
        sections: current.sections,
        drops: current.drops,
        containers: current.containers,
    };
    fs::write(&path, encode(b"REG4", &old).unwrap()).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.container_at(position), Some(&inventory));
    assert_eq!(loaded.furniture_orders().count(), 0);
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(&read(&path).unwrap()[12..16], b"RGA3");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reg3_loads_without_containers_and_migrates_to_reg5_without_changing_wood() {
    let root = root();
    let world = WorldId(73);
    let position = [1, 20, 3];
    let mut journal = EditJournal::default();
    journal.record(position, 29).unwrap();
    journal
        .set_object_state(
            position,
            Some(terra_world::edits::BlockObjectState {
                wood: terra_core::WoodSpecies::Birch,
                ..Default::default()
            }),
        )
        .unwrap();
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(region_name([0, 0]));
    let current = read_region(&read(&path).unwrap()).unwrap();
    let old = WoodRegion {
        world: current.world,
        position: current.position,
        sections: current.sections,
        drops: current.drops,
    };
    fs::write(&path, encode(b"REG3", &old).unwrap()).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.containers().count(), 0);
    assert_eq!(
        loaded.object_state(position),
        journal.object_state(position)
    );
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(&read(&path).unwrap()[12..16], b"RGA3");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn independent_chest_halves_roundtrip_across_negative_region_boundaries() {
    let root = root();
    let world = WorldId(73);
    let mut journal = EditJournal::default();
    let positions = [[-1, 20, -1], [0, 20, -1]];
    for (index, position) in positions.into_iter().enumerate() {
        journal.record(position, 29).unwrap();
        let inventory = Inventory {
            slots: vec![terra_core::ItemStack {
                item: terra_core::ItemKind::Block(3),
                count: 7 + index as u16,
            }],
            revision: 41 + index as u64,
            cursor: None,
        };
        journal.set_container(position, inventory).unwrap();
    }
    save_edits(&root, world, &journal).unwrap();
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().revision, journal.gameplay().revision);
    assert_eq!(loaded.containers().count(), 2);
    for position in positions {
        assert_eq!(
            loaded.container_at(position),
            journal.container_at(position)
        );
    }
    loaded.remove_container(positions[0]).unwrap();
    loaded.record(positions[0], 0).unwrap();
    save_edits(&root, world, &loaded).unwrap();
    let reloaded = load_edits(&root, world).unwrap();
    assert!(reloaded.container_at(positions[0]).is_none());
    assert_eq!(
        reloaded.container_at(positions[1]),
        journal.container_at(positions[1])
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn corrupt_duplicate_misplaced_or_invalid_container_records_fail_closed() {
    let root = root();
    let world = WorldId(73);
    let position = [1, 20, 3];
    let mut journal = EditJournal::default();
    journal.record(position, 29).unwrap();
    journal
        .set_container(position, Inventory::default())
        .unwrap();
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(region_name([0, 0]));
    let valid = read(&path).unwrap();
    for case in 0..6 {
        let mut region = read_region(&valid).unwrap();
        match case {
            0 => region.containers.push(region.containers[0].clone()),
            1 => region.containers[0].0 = [513, 20, 3],
            2 => region.containers[0].0 = [1, 21, 3],
            3 => region.containers[0].1.slots = vec![terra_core::ItemStack::EMPTY; 19],
            4 => {
                region.containers[0].1.cursor = Some(terra_core::ItemStack {
                    item: terra_core::ItemKind::Stick,
                    count: 1,
                })
            }
            _ => {
                region.containers[0].1.slots = vec![terra_core::ItemStack {
                    item: terra_core::ItemKind::Stick,
                    count: 129,
                }]
            }
        }
        fs::write(&path, encode(b"REG5", &region).unwrap()).unwrap();
        assert!(load_edits(&root, world).is_err(), "case {case}");
    }
    fs::write(&path, valid).unwrap();
    assert_eq!(load_edits(&root, world).unwrap().containers().count(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn previous_oriented_regions_default_to_oak_without_changing_positions_or_ids() {
    let old = PreviousRegion {
        world: WorldId(73),
        position: [0, 0],
        drops: Vec::new(),
        sections: vec![PreviousSectionDelta {
            position: terra_voxel::SectionPos::default(),
            revision: 3,
            prop_epoch: 0,
            appearance_epoch: 0,
            fluid_epoch: 0,
            blocks: vec![([1, 2, 3], 29)],
            removed_props: Vec::new(),
            placed_props: Vec::new(),
            conditions: Vec::new(),
            fluids: Vec::new(),
            objects: vec![(
                [1, 2, 3],
                PreviousObjectState {
                    facing: 2,
                    peer: None,
                    secondary: false,
                },
            )],
        }],
    };
    let migrated = read_region(&encode(b"REG2", &old).unwrap()).unwrap();
    assert_eq!(
        migrated.sections[0].objects[0].1.wood,
        terra_core::WoodSpecies::Oak
    );
    assert_eq!(migrated.sections[0].objects[0].1.facing, 2);
    assert_eq!(migrated.sections[0].blocks, vec![([1, 2, 3], 29)]);
    let encoded = encode(b"REG5", &migrated).unwrap();
    assert_eq!(read_region(&encoded).unwrap().sections, migrated.sections);
}

#[test]
fn wood_variants_roundtrip_inventory_cursor_and_world_drops_without_changing_item_stack_layout() {
    let root = root();
    let world = WorldId(73);
    let player = PlayerUuid::new_v4();
    let birch = terra_core::ItemKind::for_wood(29, terra_core::WoodSpecies::Birch);
    let spruce = terra_core::ItemKind::for_wood(28, terra_core::WoodSpecies::Spruce);
    let mut journal = EditJournal::default();
    let mut inventory = Inventory::default();
    inventory.insert(birch, 7);
    inventory.cursor = Some(terra_core::ItemStack {
        item: spruce,
        count: 2,
    });
    journal
        .gameplay_mut()
        .inventories
        .insert(player, inventory.clone());
    assert!(journal.gameplay_mut().drop_one([1.5, 20.5, 3.5], birch));
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().inventories[&player], inventory);
    assert_eq!(loaded.gameplay().drops[0].stack.item, birch);
    assert_eq!(
        &fs::read(root.join(format!("players/{player}.data"))).unwrap()[12..16],
        b"PLY7"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_regions_migrate_and_oriented_pairs_roundtrip_across_region_boundaries() {
    let world = WorldId(73);
    let old = LegacyRegion {
        world,
        position: [0, 0],
        sections: vec![LegacySectionDelta {
            position: terra_voxel::SectionPos::default(),
            revision: 3,
            prop_epoch: 0,
            appearance_epoch: 0,
            fluid_epoch: 0,
            blocks: vec![([1, 2, 3], 29)],
            removed_props: Vec::new(),
            placed_props: Vec::new(),
            conditions: Vec::new(),
            fluids: Vec::new(),
        }],
        drops: Vec::new(),
    };
    let migrated = read_region(&encode(b"REGN", &old).unwrap()).unwrap();
    assert_eq!(migrated.sections[0].blocks, vec![([1, 2, 3], 29)]);
    assert!(migrated.sections[0].objects.is_empty());
    let root = root();
    let mut journal = EditJournal::default();
    let a = [-1, 20, -1];
    let b = [0, 20, -1];
    journal.record(a, 28).unwrap();
    journal.record(b, 28).unwrap();
    for (p, peer, secondary) in [(a, b, false), (b, a, true)] {
        journal
            .set_object_state(
                p,
                Some(terra_world::edits::BlockObjectState {
                    wood: terra_core::WoodSpecies::Spruce,
                    facing: 0,
                    peer: Some(peer),
                    secondary,
                }),
            )
            .unwrap();
    }
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.object_state(a), journal.object_state(a));
    assert_eq!(loaded.object_state(b), journal.object_state(b));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ply2_and_ply3_migrate_vitals_and_inventory_to_food_aware_ply4() {
    let mut inventory = Inventory::default();
    inventory.insert(
        terra_core::ItemKind::Animal(terra_core::creatures::AnimalProduct::CookedFish),
        17,
    );
    inventory.cursor = Some(terra_core::ItemStack {
        item: terra_core::ItemKind::Stick,
        count: 3,
    });
    inventory.revision = 91;
    let old = SurvivalPlayerData {
        world: WorldId(73),
        player: PlayerUuid::new_v4(),
        inventory,
        stats: SurvivalPlayerStats {
            blocks_broken: 12,
            blocks_placed: 7,
            props_harvested: 4,
            deaths: 3,
            survival: LegacySurvivalVitals {
                health: 43.0,
                stamina: 71.0,
                hunger: 21.0,
                oxygen: 95.0,
                exhausted: true,
                stamina_delay: 0.75,
                recovery_delay: 2.5,
                last_damage: Some(terra_core::DamageCause::Drowning),
            },
        },
        has_inventory: true,
        has_stats: true,
    };
    let mut current = read_player(&encode(b"PLY2", &old).unwrap()).unwrap();
    assert_eq!(current.stats.survival, old.stats.survival.into());
    assert_eq!(current.stats.deaths, 3);
    assert_eq!(current.stats.blocks_broken, 12);
    assert_eq!(current.inventory, old.inventory);
    assert_eq!(current.stats.game_mode, None);
    assert_eq!(current.stats.survival.saturation, 0.0);
    assert_eq!(current.stats.survival.regeneration_seconds, 0.0);
    current.stats.game_mode = Some(terra_core::GameMode::Creative);
    let mode_data = GameModePlayerData {
        world: old.world,
        player: old.player,
        inventory: old.inventory,
        stats: GameModePlayerStats {
            game_mode: current.stats.game_mode,
            blocks_broken: old.stats.blocks_broken,
            blocks_placed: old.stats.blocks_placed,
            props_harvested: old.stats.props_harvested,
            deaths: old.stats.deaths,
            survival: old.stats.survival,
        },
        has_inventory: old.has_inventory,
        has_stats: old.has_stats,
    };
    let migrated = read_player(&encode(b"PLY3", &mode_data).unwrap()).unwrap();
    assert_eq!(migrated.stats, current.stats);
    assert_eq!(migrated.inventory, current.inventory);
    let reloaded = read_player(&encode(b"PLY7", &current).unwrap()).unwrap();
    assert_eq!(reloaded.stats, current.stats);
    assert_eq!(reloaded.inventory, current.inventory);
}

#[test]
fn legacy_player_records_gain_full_vitals_and_new_vitals_roundtrip_without_losing_inventory() {
    let root = root();
    let world = WorldId(73);
    let player = PlayerUuid::new_v4();
    let mut journal = EditJournal::default();
    journal
        .gameplay_mut()
        .inventories
        .entry(player)
        .or_default()
        .insert(terra_core::ItemKind::Block(3), 27);
    let stats = PlayerStats {
        blocks_broken: 91,
        blocks_placed: 17,
        props_harvested: 12,
        ..Default::default()
    };
    journal.gameplay_mut().player_stats.insert(player, stats);
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(format!("players/{player}.data"));
    let legacy = LegacyPlayerData {
        world,
        player,
        inventory: journal.gameplay().inventories[&player].clone(),
        stats: LegacyPlayerStats {
            blocks_broken: 91,
            blocks_placed: 17,
            props_harvested: 12,
        },
        has_inventory: true,
        has_stats: true,
    };
    fs::write(&path, encode(b"PLYR", &legacy).unwrap()).unwrap();
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().player_stats[&player], stats);
    let mut changed = stats;
    changed.deaths = 4;
    changed.survival = terra_core::SurvivalVitals {
        health: 29.5,
        stamina: 13.25,
        hunger: 42.0,
        oxygen: 16.0,
        exhausted: true,
        stamina_delay: 0.8,
        recovery_delay: 3.5,
        last_damage: Some(terra_core::DamageCause::Drowning),
        saturation: 17.5,
        regeneration_seconds: 2.5,
        regeneration_rate: 0.5,
        effects: Default::default(),
        ..Default::default()
    };
    assert!(loaded.update_player_stats(player, changed));
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(&fs::read(&path).unwrap()[12..16], b"PLY7");
    let reloaded = load_edits(&root, world).unwrap();
    assert_eq!(reloaded.gameplay().player_stats[&player], changed);
    assert_eq!(reloaded.gameplay().inventories[&player], legacy.inventory);
    changed.survival.health = f32::NAN;
    assert!(!loaded.update_player_stats(player, changed));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn food_player_v4_migrates_fractional_regeneration_without_reading_effect_bytes() {
    let old = FoodPlayerData {
        world: WorldId(104),
        player: PlayerUuid::new_v4(),
        inventory: Inventory::default(),
        stats: FoodPlayerStats {
            game_mode: Some(terra_core::GameMode::Survival),
            blocks_broken: 16,
            blocks_placed: 8,
            props_harvested: 3,
            deaths: 2,
            survival: FoodSurvivalVitals {
                health: 41.0,
                stamina: 67.0,
                hunger: 44.0,
                oxygen: 81.0,
                exhausted: false,
                stamina_delay: 0.2,
                recovery_delay: 1.5,
                last_damage: Some(terra_core::DamageCause::Smoke),
                saturation: 13.0,
                regeneration_seconds: 3.25,
                regeneration_rate: 0.5,
            },
        },
        has_inventory: true,
        has_stats: true,
    };
    let bytes = encode(b"PLY4", &old).unwrap();
    let loaded = read_player(&bytes).unwrap();
    assert_eq!(loaded.world, old.world);
    assert_eq!(loaded.player, old.player);
    assert_eq!(loaded.inventory, old.inventory);
    assert_eq!(loaded.stats.survival, old.stats.survival.into());
    assert_eq!(loaded.stats.blocks_broken, 16);
    assert_eq!(loaded.stats.deaths, 2);
    assert_eq!(loaded.stats.survival.effects.iter().count(), 0);
    assert_eq!(loaded.stats.survival.active_effects().count(), 1);
    assert!(loaded.stats.survival.valid());
}

#[test]
fn status_effect_player_v5_preserves_timed_effects_and_shield_but_resamples_smoke() {
    let root = root();
    let world = WorldId(104);
    let player = PlayerUuid::new_v4();
    let mut journal = EditJournal::default();
    let mut stats = PlayerStats::default();
    for kind in terra_core::EffectKind::ALL {
        stats.survival.effects.apply(kind, 9.75, 1);
    }
    stats.survival.effects.absorb_damage(7.5);
    stats.survival.effects.set_smoke_exposure(true);
    journal.gameplay_mut().player_stats.insert(player, stats);
    journal
        .gameplay_mut()
        .inventories
        .entry(player)
        .or_default()
        .insert(terra_core::ItemKind::Block(3), 17);
    save_edits(&root, world, &journal).unwrap();
    assert!(
        journal.gameplay().player_stats[&player]
            .survival
            .effects
            .smoke_exposed()
    );
    let bytes = fs::read(root.join(format!("players/{player}.data"))).unwrap();
    assert_eq!(&bytes[12..16], b"PLY7");
    let loaded = load_edits(&root, world).unwrap();
    let actual = loaded.gameplay().player_stats[&player];
    stats.survival.effects.set_smoke_exposure(false);
    assert_eq!(actual, stats);
    assert_eq!(actual.survival.effects.shield_points(), 12.5);
    assert_eq!(
        actual.survival.effects.iter().count(),
        terra_core::effects::EFFECT_COUNT
    );
    assert_eq!(
        loaded.gameplay().inventories[&player],
        journal.gameplay().inventories[&player]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn player_v5_loading_caps_shield_charge_to_the_current_definition() {
    let mut vitals = terra_core::SurvivalVitals::default();
    vitals
        .effects
        .apply(terra_core::EffectKind::ShieldPower, 10.0, 1);
    let mut text = toml::Value::try_from(vitals).unwrap();
    text["effects"]["shield"] = toml::Value::Float(100.0);
    let invalid: terra_core::SurvivalVitals = text.try_into().unwrap();
    assert!(!invalid.valid());
    let data = PlayerData {
        world: WorldId(104),
        player: PlayerUuid::new_v4(),
        inventory: Inventory::default(),
        stats: PlayerStats {
            survival: invalid,
            ..Default::default()
        },
        has_inventory: false,
        has_stats: true,
    };
    let loaded = read_player(&encode(b"PLY7", &data).unwrap()).unwrap();
    assert_eq!(loaded.stats.survival.effects.shield_points(), 20.0);
    assert!(loaded.stats.survival.valid());
}

#[test]
fn status_player_v5_migrates_effects_without_losing_stats_or_inventories() {
    let mut effects = terra_core::StatusEffects::default();
    effects.apply(terra_core::EffectKind::Poison, 7.25, 1);
    effects.apply(terra_core::EffectKind::ShieldPower, 12.5, 1);
    effects.absorb_damage(3.0);
    let old = EffectsPlayerData {
        world: WorldId(105),
        player: PlayerUuid::new_v4(),
        inventory: Inventory::default(),
        stats: EffectsPlayerStats {
            game_mode: Some(terra_core::GameMode::Survival),
            blocks_broken: 83,
            blocks_placed: 17,
            props_harvested: 6,
            deaths: 2,
            survival: EffectsSurvivalVitals {
                health: 67.0,
                stamina: 44.0,
                hunger: 51.0,
                oxygen: 93.0,
                exhausted: false,
                stamina_delay: 0.2,
                recovery_delay: 3.0,
                last_damage: Some(terra_core::DamageCause::Poison),
                saturation: 7.0,
                regeneration_seconds: 3.5,
                regeneration_rate: 0.5,
                effects,
            },
        },
        has_inventory: true,
        has_stats: true,
    };
    let loaded = read_player(&encode(b"PLY5", &old).unwrap()).unwrap();
    assert_eq!(loaded.stats.blocks_broken, 83);
    assert_eq!(loaded.stats.survival, old.stats.survival.into());
    assert_eq!(
        loaded.stats.equipment,
        terra_core::EquipmentSlots::default()
    );
    assert_eq!(loaded.stats.survival.effects.shield_points(), 17.0);
    assert_eq!(loaded.inventory, old.inventory);
}

#[test]
fn equipment_player_v6_roundtrips_all_nine_slots_with_server_derived_attributes() {
    use terra_core::{AccessoryKind, ArmorItem, ArmorMaterial, ArmorPiece, EquipmentSlots};
    let root = root();
    let world = WorldId(105);
    let player = PlayerUuid::new_v4();
    let mut gear = EquipmentSlots::default();
    for piece in ArmorPiece::ALL {
        gear.set(
            piece.slot(),
            Some(terra_core::ItemKind::Armor(ArmorItem {
                material: ArmorMaterial::Ruby,
                piece,
            })),
        );
    }
    for kind in AccessoryKind::ALL {
        gear.set(kind.slot(), Some(terra_core::ItemKind::Accessory(kind)));
    }
    let mut stats = PlayerStats {
        equipment: gear,
        ..Default::default()
    };
    stats.survival.apply_equipment(gear);
    stats.survival.health = 123.5;
    stats.survival.stamina = 118.0;
    let mut journal = EditJournal::default();
    journal.gameplay_mut().player_stats.insert(player, stats);
    journal
        .gameplay_mut()
        .inventories
        .entry(player)
        .or_default()
        .insert(
            terra_core::ItemKind::Mineral(terra_core::MineralItem::Diamond),
            17,
        );
    save_edits(&root, world, &journal).unwrap();
    let bytes = fs::read(root.join(format!("players/{player}.data"))).unwrap();
    assert_eq!(&bytes[12..16], b"PLY7");
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().player_stats[&player], stats);
    assert_eq!(
        loaded.gameplay().inventories[&player],
        journal.gameplay().inventories[&player]
    );
    assert_eq!(stats.survival.max_health, 145.0);
    assert_eq!(stats.survival.max_stamina, 140.0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn equipment_player_v6_rejects_wrong_slot_items_and_rederives_forged_bonuses() {
    let mut data = PlayerData {
        world: WorldId(105),
        player: PlayerUuid::new_v4(),
        inventory: Inventory::default(),
        stats: PlayerStats::default(),
        has_inventory: false,
        has_stats: true,
    };
    data.stats.equipment.slots[0] = terra_core::ItemStack {
        item: terra_core::ItemKind::Block(3),
        count: 1,
    };
    assert!(read_player(&encode(b"PLY7", &data).unwrap()).is_err());
    data.stats.equipment = Default::default();
    data.stats.survival.max_health = 300.0;
    data.stats.survival.max_stamina = 300.0;
    data.stats.survival.armor_points = 40.0;
    data.stats.survival.health = 200.0;
    let loaded = read_player(&encode(b"PLY7", &data).unwrap()).unwrap();
    assert_eq!(loaded.stats.survival.max_health, 100.0);
    assert_eq!(loaded.stats.survival.health, 100.0);
    assert_eq!(loaded.stats.survival.armor_points, 0.0);
}

#[test]
fn player_v6_nine_slot_layout_migrates_without_losing_existing_armor_or_effects() {
    use terra_core::{ArmorItem, ArmorMaterial, ArmorPiece, EquipmentSlot};
    let mut gear = terra_core::EquipmentSlots::default();
    for piece in ArmorPiece::ALL {
        gear.set(
            piece.slot(),
            Some(terra_core::ItemKind::Armor(ArmorItem {
                material: ArmorMaterial::Diamond,
                piece,
            })),
        );
    }
    let mut survival = terra_core::SurvivalVitals::default();
    survival.apply_equipment(gear);
    survival.health = 89.0;
    survival
        .effects
        .apply(terra_core::EffectKind::Speed, 15.0, 1);
    let mut slots = [terra_core::ItemStack::EMPTY; 9];
    slots.copy_from_slice(&gear.slots[..9]);
    let old = EquipmentPlayerData {
        world: WorldId(106),
        player: PlayerUuid::new_v4(),
        inventory: Inventory::default(),
        stats: EquipmentPlayerStats {
            game_mode: Some(terra_core::GameMode::Survival),
            blocks_broken: 9,
            blocks_placed: 4,
            props_harvested: 2,
            deaths: 1,
            survival,
            equipment: LegacyEquipmentSlots { slots },
        },
        has_inventory: true,
        has_stats: true,
    };
    let current = read_player(&encode(b"PLY6", &old).unwrap()).unwrap();
    assert_eq!(current.stats.equipment, gear);
    assert_eq!(current.stats.survival, survival);
    assert_eq!(current.stats.equipment.get(EquipmentSlot::Cape), None);
    assert_eq!(current.stats.blocks_broken, 9);
    assert_eq!(current.inventory, old.inventory);
}

#[test]
fn player_v7_preserves_dyed_cape_material_and_rederives_its_attributes() {
    use terra_core::{ArmorMaterial, CapeItem, EquipmentSlot, beds::DyeColor};
    let mut data = PlayerData {
        world: WorldId(106),
        player: PlayerUuid::new_v4(),
        inventory: Inventory::default(),
        stats: PlayerStats::default(),
        has_inventory: true,
        has_stats: true,
    };
    let cape = terra_core::ItemKind::Cape(CapeItem {
        material: ArmorMaterial::Ruby,
        color: DyeColor::Purple,
    });
    data.stats.equipment.set(EquipmentSlot::Cape, Some(cape));
    data.stats.survival.apply_equipment(data.stats.equipment);
    data.stats.survival.health = 83.0;
    let current = read_player(&encode(b"PLY7", &data).unwrap()).unwrap();
    assert_eq!(current.stats, data.stats);
    assert_eq!(current.stats.equipment.get(EquipmentSlot::Cape), Some(cape));
    assert!(current.stats.survival.max_health > 100.0);
}

#[test]
fn the_same_player_has_independent_survival_and_inventory_in_each_world() {
    let root = root();
    let player = PlayerUuid::new_v4();
    for (name, world, health, count) in [
        ("first", WorldId(41), 24.0, 7),
        ("second", WorldId(42), 83.0, 19),
    ] {
        let mut journal = EditJournal::default();
        journal.update_player_stats(
            player,
            PlayerStats {
                survival: terra_core::SurvivalVitals {
                    health,
                    hunger: 61.0,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        journal
            .gameplay_mut()
            .inventories
            .entry(player)
            .or_default()
            .insert(terra_core::ItemKind::Block(3), count);
        save_edits(&root.join(name), world, &journal).unwrap();
    }
    let first = load_edits(&root.join("first"), WorldId(41)).unwrap();
    let second = load_edits(&root.join("second"), WorldId(42)).unwrap();
    assert_eq!(first.gameplay().player_stats[&player].survival.health, 24.0);
    assert_eq!(
        second.gameplay().player_stats[&player].survival.health,
        83.0
    );
    assert_eq!(
        first.gameplay().inventories[&player].count(terra_core::ItemKind::Block(3)),
        7
    );
    assert_eq!(
        second.gameplay().inventories[&player].count(terra_core::ItemKind::Block(3)),
        19
    );
    assert!(load_edits(&root.join("first"), WorldId(42)).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn old_index_upgrades_and_calendar_roundtrips_without_touching_regions() {
    let root = root();
    let mut journal = EditJournal::default();
    journal.record([-20, 4, 1], 3).unwrap();
    save_edits(&root, WorldId(1), &journal).unwrap();
    let index = read_index(&root).unwrap().index;
    fs::write(root.join(INDEX), encode(b"INDX", &index).unwrap()).unwrap();
    let mut loaded = load_edits(&root, WorldId(1)).unwrap();
    assert!(loaded.clock.is_none());
    let region = root.join("regions/r.-1.0.region");
    let before = fs::metadata(&region).unwrap().modified().unwrap();
    let clock = terra_core::WorldClock {
        day: 37,
        phase: 0.9,
        paused: true,
        ..Default::default()
    };
    loaded.clock = Some(clock);
    save_edits(&root, WorldId(1), &loaded).unwrap();
    let reloaded = load_edits(&root, WorldId(1)).unwrap();
    assert_eq!(reloaded.clock, Some(clock));
    let seasons = terra_core::SeasonConfig {
        winter_days: 14,
        start_season: terra_core::Season::Summer,
        ..Default::default()
    };
    assert_eq!(
        reloaded.clock.unwrap().season(seasons),
        clock.season(seasons)
    );
    assert_eq!(reloaded.block([-20, 4, 1]), Some(3));
    assert_eq!(fs::metadata(&region).unwrap().modified().unwrap(), before);
    assert_eq!(&fs::read(root.join(INDEX)).unwrap()[12..16], b"IDX6");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn weather_roundtrips_with_surface_snow_and_upgrades_previous_calendar_index() {
    let root = root();
    let mut journal = EditJournal::default();
    let mut weather = terra_core::WorldWeather::new(77);
    weather.advance(700.0, 77);
    journal.weather = Some(weather);
    journal
        .set_surface_condition(
            [-17, 81, 15],
            terra_world::environment::SurfaceCondition {
                snow: 0.72,
                wetness: 0.3,
                ..Default::default()
            },
        )
        .unwrap();
    save_edits(&root, WorldId(1), &journal).unwrap();
    let loaded = load_edits(&root, WorldId(1)).unwrap();
    assert_eq!(loaded.weather, Some(weather));
    assert_eq!(loaded.surface_condition([-17, 81, 15]).snow, 0.72);
    let timed = read_index(&root).unwrap();
    fs::write(
        root.join(INDEX),
        encode(
            b"IDX4",
            &Index4 {
                index: timed.index,
                clock: timed.clock,
                ecology_seconds: timed.ecology_seconds,
                weather: Some(Weather4 {
                    sequence: weather.sequence,
                    elapsed_seconds: weather.elapsed_seconds,
                    elapsed_total: weather.elapsed_total,
                    duration_seconds: weather.duration_seconds,
                    transition_seconds: weather.transition_seconds,
                    from_intensity: weather.from_intensity,
                    target_intensity: weather.target_intensity,
                    from_wind: weather.from_wind,
                    target_wind: weather.target_wind,
                    wetness: weather.wetness,
                }),
            },
        )
        .unwrap(),
    )
    .unwrap();
    let migrated = load_edits(&root, WorldId(1)).unwrap();
    assert_eq!(migrated.weather, Some(weather));
    assert_eq!(migrated.surface_condition([-17, 81, 15]).snow, 0.72);
    let timed = read_index(&root).unwrap();
    fs::write(
        root.join(INDEX),
        encode(
            b"IDX3",
            &Index3 {
                index: timed.index,
                clock: timed.clock,
                ecology_seconds: 123.0,
            },
        )
        .unwrap(),
    )
    .unwrap();
    let previous = read_index(&root).unwrap();
    assert!(previous.weather.is_none());
    assert_eq!(previous.ecology_seconds, 123.0);
    assert_eq!(
        load_edits(&root, WorldId(1))
            .unwrap()
            .surface_condition([-17, 81, 15])
            .snow,
        0.72
    );
    journal.weather.as_mut().unwrap().wetness = f32::NAN;
    assert!(save_edits(&root, WorldId(1), &journal).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn weather_index5_migrates_and_index6_preserves_snow_and_shared_lightning() {
    let root = root();
    let mut journal = EditJournal::default();
    let mut weather = terra_core::WorldWeather::new(74);
    weather.set_pattern(terra_core::WeatherPattern::Snow);
    weather.advance(8.0, 74);
    journal.weather = Some(weather);
    save_edits(&root, WorldId(1), &journal).unwrap();
    let timed = read_index(&root).unwrap();
    fs::write(
        root.join(INDEX),
        encode(
            b"IDX5",
            &Index5 {
                index: timed.index,
                clock: timed.clock,
                ecology_seconds: timed.ecology_seconds,
                weather: Some(Weather5 {
                    prior: Weather4 {
                        sequence: weather.sequence,
                        elapsed_seconds: weather.elapsed_seconds,
                        elapsed_total: weather.elapsed_total,
                        duration_seconds: weather.duration_seconds,
                        transition_seconds: weather.transition_seconds,
                        from_intensity: weather.from_intensity,
                        target_intensity: weather.target_intensity,
                        from_wind: weather.from_wind,
                        target_wind: weather.target_wind,
                        wetness: weather.wetness,
                    },
                    pattern: weather.pattern,
                }),
            },
        )
        .unwrap(),
    )
    .unwrap();
    let mut migrated = load_edits(&root, WorldId(1)).unwrap();
    assert_eq!(migrated.weather, Some(weather));
    weather.set_pattern(terra_core::WeatherPattern::Blizzard);
    weather.advance(8.0, 74);
    weather.latest_lightning = Some(terra_core::LightningEvent {
        id: 113,
        at: weather.elapsed_total,
        position: [-15.5, 82.02, 4.5],
        ground_strike: true,
    });
    migrated.weather = Some(weather);
    save_edits(&root, WorldId(1), &migrated).unwrap();
    assert_eq!(&fs::read(root.join(INDEX)).unwrap()[12..16], b"IDX6");
    assert_eq!(
        load_edits(&root, WorldId(1)).unwrap().weather,
        Some(weather)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn binary_regions_separate_negative_columns_water_and_uuid_players_and_stats() {
    let root = root();
    let world = WorldId(7);
    let id = PlayerUuid::new_v4();
    let mut journal = EditJournal::default();
    for p in [[-513, -17, -1], [-1, 2, 513], [512, 30, 512]] {
        journal.record(p, 3).unwrap();
    }
    journal.record([-512, 20, -512], 27).unwrap();
    journal
        .gameplay_mut()
        .inventories
        .entry(id)
        .or_default()
        .insert(terra_core::ItemKind::Block(3), 128);
    journal.gameplay_mut().player_stats.insert(
        id,
        PlayerStats {
            blocks_broken: 23,
            blocks_placed: 9,
            props_harvested: 7,
            ..Default::default()
        },
    );
    journal
        .gameplay_mut()
        .drop_one([-513.5, 1.0, 513.5], terra_core::ItemKind::Block(4));
    save_edits(&root, world, &journal).unwrap();
    assert!(!root.join("edits.toml").exists());
    assert!(root.join("regions/r.-2.-1.region").is_file());
    assert!(root.join(format!("players/{id}.data")).is_file());
    assert_eq!(&fs::read(root.join(INDEX)).unwrap()[..8], MAGIC);
    let loaded = load_edits(&root, world).unwrap();
    for p in journal.changed_sections() {
        assert_eq!(loaded.section_delta(p), journal.section_delta(p));
    }
    assert_eq!(loaded.gameplay(), journal.gameplay());
    assert!(load_edits(&root, WorldId(8)).is_err());
    assert!(save_edits(&root, WorldId(8), &journal).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn corrupt_or_truncated_files_fail_without_falling_back_to_legacy() {
    let root = root();
    let mut journal = EditJournal::default();
    journal.record([0; 3], 3).unwrap();
    save_edits(&root, WorldId(1), &journal).unwrap();
    let path = root.join("regions/r.0.0.region");
    let original = fs::read(&path).unwrap();
    let mut damaged = original.clone();
    damaged[32] ^= 1;
    fs::write(&path, damaged).unwrap();
    assert!(load_edits(&root, WorldId(1)).is_err());
    fs::write(&path, &original[..20]).unwrap();
    assert!(load_edits(&root, WorldId(1)).is_err());
    fs::write(&path, original).unwrap();
    assert_eq!(
        load_edits(&root, WorldId(1)).unwrap().block([0; 3]),
        Some(3)
    );
    fs::remove_file(root.join(INDEX)).unwrap();
    assert!(load_edits(&root, WorldId(1)).is_err());
    assert!(save_edits(&root, WorldId(1), &EditJournal::default()).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unchanged_regions_are_not_rewritten_and_drained_fluid_is_not_resurrected() {
    let root = root();
    let mut journal = EditJournal::default();
    journal.record([-1, 20, -1], 27).unwrap();
    save_edits(&root, WorldId(1), &journal).unwrap();
    let path = root.join("regions/r.-1.-1.region");
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    save_edits(&root, WorldId(1), &journal).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    journal.record([-1, 20, -1], 3).unwrap();
    save_edits(&root, WorldId(1), &journal).unwrap();
    let loaded = load_edits(&root, WorldId(1)).unwrap();
    assert!(loaded.fluid([-1, 20, -1]).is_none());
    assert_eq!(loaded.block([-1, 20, -1]), Some(3));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn interrupted_committed_batch_is_replayed_and_uncommitted_batch_is_discarded() {
    let root = root();
    let journal = EditJournal::default();
    save_edits(&root, WorldId(1), &journal).unwrap();
    let pending = root.join(TRANSACTION);
    directory(&pending).unwrap();
    write(&pending.join("incomplete.tmp"), b"partial").unwrap();
    assert!(load_edits(&root, WorldId(1)).is_ok());
    assert!(!pending.exists());
    directory(&pending).unwrap();
    let index = Index {
        world: WorldId(1),
        regions: vec![],
        players: vec![],
        next_drop: 9,
        revision: 13,
    };
    let bytes = encode(b"INDX", &index).unwrap();
    write(&pending.join(INDEX), &bytes).unwrap();
    write(
        &pending.join("commit.data"),
        &encode(
            b"CMIT",
            &Commit {
                files: vec![(INDEX.into(), checksum(&bytes))],
            },
        )
        .unwrap(),
    )
    .unwrap();
    let loaded = load_edits(&root, WorldId(1)).unwrap();
    assert_eq!(loaded.gameplay().next_drop, 9);
    assert_eq!(loaded.gameplay().revision, 13);
    assert!(!pending.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn transaction_paths_and_overlapping_writers_are_rejected() {
    let root = root();
    let lock = directory_lock(&root, ".write.lock").unwrap();
    assert!(load_edits(&root, WorldId(1)).is_err());
    drop(lock);
    let pending = root.join(TRANSACTION);
    directory(&pending).unwrap();
    write(
        &pending.join("commit.data"),
        &encode(
            b"CMIT",
            &Commit {
                files: vec![("../outside".into(), 0), (INDEX.into(), 0)],
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert!(load_edits(&root, WorldId(1)).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn partially_installed_transaction_finishes_player_and_index_together() {
    let root = root();
    let id = PlayerUuid::new_v4();
    let world = WorldId(1);
    save_edits(&root, world, &EditJournal::default()).unwrap();
    let pending = root.join(TRANSACTION);
    let data = PlayerData {
        world,
        player: id,
        inventory: Inventory::default(),
        stats: PlayerStats {
            blocks_broken: 19,
            ..Default::default()
        },
        has_inventory: true,
        has_stats: true,
    };
    let player = format!("players/{id}.data");
    let player_bytes = encode(b"PLY7", &data).unwrap();
    let index_bytes = encode(
        b"INDX",
        &Index {
            world,
            regions: vec![],
            players: vec![id],
            next_drop: 0,
            revision: 0,
        },
    )
    .unwrap();
    write(&pending.join(&player), &player_bytes).unwrap();
    write(&pending.join(INDEX), &index_bytes).unwrap();
    write(
        &pending.join("commit.data"),
        &encode(
            b"CMIT",
            &Commit {
                files: vec![
                    (player.clone(), checksum(&player_bytes)),
                    (INDEX.into(), checksum(&index_bytes)),
                ],
            },
        )
        .unwrap(),
    )
    .unwrap();
    fs::rename(pending.join(&player), root.join(&player)).unwrap();
    assert_eq!(
        load_edits(&root, world).unwrap().gameplay().player_stats[&id].blocks_broken,
        19
    );
    assert!(!pending.exists());
    fs::remove_dir_all(root).unwrap();
}
