use super::*;
use terra_core::{ItemKind, ItemStack, ResourcePropKind, TreeSpecies, TreeStage};
use terra_world::edits::EditJournal;
#[test]
fn udp_sapling_placement_consumes_one_then_growth_and_late_join_keep_species_and_exact_tree_ownership()
 {
    let (mut host, mut a, mut b) = pair();
    let state = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(state.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let ground_y =
        (state.position[1] - terra_physics::character::PLAYER_HALF_HEIGHT).floor() as i64 - 1;
    // Real generated crowns can reach beyond the original one-section vertical
    // fixture. Load those sections through streaming, never assume unknown air.
    {
        let mut settings = host
            .world_mut()
            .resource_mut::<terra_world::streaming::StreamingSettings>();
        settings.radius = settings.radius.max(3);
        settings.vertical_radius = 3;
    }
    wait(&mut host, &mut [&mut a, &mut b], |host, _| {
        let chunks = host.world().resource::<ChunkStore>();
        (-1..=1).all(|x| {
            (0..=2).all(|z| {
                ((ground_y + 1).div_euclid(16)..=(ground_y + 41).div_euclid(16)).all(|y| {
                    chunks
                        .loaded
                        .contains_key(&terra_voxel::SectionPos::new(x, y, z))
                })
            })
        })
    });
    let chunks = host.world().resource::<ChunkStore>();
    let journal = host.world().resource::<EditJournal>();
    let mut chosen = None;
    'search: for x in 2..=14 {
        for z in 16..=30 {
            let support = [x, ground_y, z];
            if !terra_world::saplings::planting_environment_valid(
                chunks,
                journal,
                support,
                TreeSpecies::Oak,
            ) {
                continue;
            }
            if chunks
                .loaded
                .values()
                .flat_map(|s| &s.props)
                .any(|p| !p.removed && terra_world::props::placement::support(p) == support)
            {
                continue;
            }
            let root = [x, ground_y + 1, z];
            let epoch = journal.prop_epoch(terra_voxel::global_to_section(support).0);
            let seed =
                terra_world::saplings::placement_seed(support, ResourcePropKind::OakSapling, epoch);
            let (wood, leaves) = terra_world::saplings::shape(TreeSpecies::Oak, seed);
            let cells = wood
                .into_iter()
                .chain(leaves)
                .map(|offset| std::array::from_fn(|axis| root[axis] + i64::from(offset[axis])))
                .collect::<std::collections::HashSet<_>>();
            if cells.iter().any(|p| chunks.block_at(*p) != Some(0)) {
                continue;
            }
            if chunks
                .loaded
                .values()
                .flat_map(|s| &s.props)
                .filter(|p| {
                    !p.removed
                        && !(matches!(
                            p.kind,
                            terra_world::props::PropKind::Grass
                                | terra_world::props::PropKind::Flower
                        ) && !p.growing_grass()
                            && !journal
                                .placed_props(p.section)
                                .any(|placed| placed.id == p.id))
                })
                .any(|p| {
                    let bounds = p.bounds();
                    let low = [root[0] - 16, root[1], root[2] - 16];
                    let high = [root[0] + 17, root[1] + 41, root[2] + 17];
                    if (0..3)
                        .any(|a| bounds[1][a] <= low[a] as f64 || bounds[0][a] >= high[a] as f64)
                    {
                        return false;
                    }
                    cells.iter().any(|cell| {
                        (0..3).all(|a| {
                            bounds[1][a] > cell[a] as f64 + 0.001
                                && bounds[0][a] < cell[a] as f64 + 0.999
                        })
                    })
                })
            {
                continue;
            }
            let target = Vec3::new(x as f32 + 0.5, ground_y as f32 + 1., z as f32 + 0.5);
            let d = (target - eye).normalize();
            let (_, hit) = terra_world::targeting::pick_world(
                chunks,
                eye.to_array().map(f64::from),
                d.to_array().map(f64::from),
            );
            if matches!(hit,Some(terra_world::targeting::WorldTarget::Block(h)) if h.position==support && h.normal==[0,1,0])
            {
                chosen = Some((support, [(-d.x).atan2(-d.z), d.y.asin()]));
                break 'search;
            }
        }
    }
    let (support, look) = chosen.expect("real visible empty sapling soil");
    // Exercise an actual UDP placement over unmodified procedural ground cover.
    // The same grass would previously fail target occlusion/capacity checks.
    let natural_cover = terra_world::props::WorldProp {
        id: terra_world::props::PropId(0xf202_0001),
        kind: terra_world::props::PropKind::Grass,
        position: [
            support[0] as f64 + 0.5,
            support[1] as f64 + 1.,
            support[2] as f64 + 0.5,
        ],
        rotation: 0.,
        scale: 1.,
        variant: 0,
        section: terra_voxel::global_to_section(support).0,
        removed: false,
    };
    host.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&natural_cover.section)
        .unwrap()
        .props
        .push(natural_cover);
    let uuid = a.world().resource::<client::ClientSession>().uuid.unwrap();
    let item = ItemKind::Prop(ResourcePropKind::OakSapling);
    {
        let mut j = host.world_mut().resource_mut::<EditJournal>();
        assert!(
            j.gameplay_mut()
                .inventories
                .entry(uuid)
                .or_default()
                .creative_set(8, ItemStack { item, count: 2 })
        );
        j.gameplay_mut().revision += 1;
    }
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = terra_core::GameMode::Survival;
    wait(&mut host, &mut [&mut a, &mut b], |_, c| {
        c[0].world()
            .resource::<client::ClientSession>()
            .inventory
            .count(item)
            == 2
    });
    a.world_mut().resource_mut::<client::ClientSession>().look = look;
    a.world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(8);
    a.world_mut().write_message(client::PropPlaceRequest {
        support,
        kind: ResourcePropKind::OakSapling,
    });
    wait(&mut host, &mut [&mut a, &mut b], |_, c| {
        c.iter().all(|c| {
            c.world()
                .resource::<client::ClientSession>()
                .trees
                .trees
                .len()
                == 1
        })
    });
    assert!(
        host.world()
            .resource::<EditJournal>()
            .prop_removed(natural_cover.section, natural_cover.id)
    );
    assert!(
        host.world().resource::<ChunkStore>().loaded[&natural_cover.section]
            .props
            .iter()
            .find(|p| p.id == natural_cover.id)
            .unwrap()
            .removed
    );
    let id = *host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trees
        .trees
        .keys()
        .next()
        .unwrap();
    wait(&mut host, &mut [&mut a, &mut b], |_, c| {
        c[0].world()
            .resource::<client::ClientSession>()
            .inventory
            .count(item)
            == 1
    });
    for app in [&a, &b] {
        let t = app
            .world()
            .resource::<client::ClientSession>()
            .trees
            .get(id)
            .unwrap();
        assert_eq!(t.species, TreeSpecies::Oak);
        assert_eq!(t.stage, TreeStage::Sapling);
        assert_eq!(t.planted_by, Some(uuid));
    }
    // Intermediate confirmed progress is visible, while sub-percent updates
    // stay inside the same transport bucket instead of resending geometry.
    host.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trees
        .get_mut(id)
        .unwrap()
        .growth_seconds = 19.;
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ClientSession>()
                .trees
                .get(id)
                .is_some_and(|tree| tree.growth_seconds == 18.)
        })
    });
    let revision = a.world().resource::<client::ClientSession>().trees.revision;
    host.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trees
        .get_mut(id)
        .unwrap()
        .growth_seconds = 20.;
    for _ in 0..10 {
        host.update();
        a.update();
        b.update();
    }
    assert_eq!(
        a.world().resource::<client::ClientSession>().trees.revision,
        revision
    );
    host.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trees
        .get_mut(id)
        .unwrap()
        .growth_seconds = 27.1;
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ClientSession>()
                .trees
                .get(id)
                .is_some_and(|tree| tree.growth_seconds == 27.)
        })
    });
    let planted = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trees
        .get(id)
        .unwrap()
        .clone();
    let (wood, leaves) = terra_world::saplings::shape(planted.species, planted.seed);
    assert!(
        wood.len() + leaves.len() > 100,
        "fixture must grow the genuine generated crown"
    );
    assert!(
        wood.iter().chain(&leaves).all(|offset| host
            .world()
            .resource::<ChunkStore>()
            .block_at(planted.global(*offset))
            == Some(0)),
        "the confirmed seed's full tree needs loaded empty cells"
    );
    host.world_mut()
        .resource_scope(|w, mut journal: Mut<EditJournal>| {
            journal
                .gameplay_mut()
                .trees
                .get_mut(id)
                .unwrap()
                .growth_seconds = 900.;
            let mut chunks = w.resource_mut::<ChunkStore>();
            assert!(
                terra_world::saplings::grow(&mut chunks, &mut journal, id).unwrap(),
                "confirmed generator tree must grow atomically"
            );
        });
    wait(&mut host, &mut [&mut a, &mut b], |_, c| {
        c.iter().all(|a| {
            a.world()
                .resource::<client::ClientSession>()
                .trees
                .get(id)
                .is_some_and(|t| t.stage == TreeStage::Generated)
        })
    });
    let expected = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trees
        .get(id)
        .unwrap()
        .clone();
    let token = host
        .world()
        .resource::<server::ServerRuntime>()
        .issue_token(777)
        .unwrap();
    let mut late = client(token);
    wait(&mut host, &mut [&mut a, &mut b, &mut late], |_, c| {
        c[2].world().resource::<client::ClientSession>().ready
            && c[2]
                .world()
                .resource::<client::ClientSession>()
                .trees
                .get(id)
                == Some(&expected)
    });
    assert_eq!(
        late.world()
            .resource::<EditJournal>()
            .gameplay()
            .trees
            .get(id),
        Some(&expected)
    );
    for p in &expected.wood {
        assert_eq!(
            host.world()
                .resource::<EditJournal>()
                .block(expected.global(*p)),
            Some(expected.species.wood())
        );
    }
    let removed = expected.global(expected.wood[0]);
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
                let properties =
                    world.resource::<terra_core::block_properties::BlockPropertiesCatalog>();
                assert!(
                    terra_world::streaming::commit_block_edit(
                        &mut chunks,
                        &mut journal,
                        &terra_world::streaming::BlockEditRequest {
                            position: removed,
                            expected_material: expected.species.wood(),
                            material: 0
                        },
                        properties
                    )
                    .unwrap()
                );
            });
        });
    let partial = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trees
        .get(id)
        .unwrap()
        .clone();
    assert_eq!(partial.wood.len() + 1, expected.wood.len());
    wait(&mut host, &mut [&mut a, &mut b, &mut late], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ClientSession>()
                .trees
                .get(id)
                == Some(&partial)
        })
    });
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
                let properties =
                    world.resource::<terra_core::block_properties::BlockPropertiesCatalog>();
                assert!(
                    terra_world::streaming::commit_block_edit(
                        &mut chunks,
                        &mut journal,
                        &terra_world::streaming::BlockEditRequest {
                            position: expected.root,
                            expected_material: expected.species.wood(),
                            material: 0
                        },
                        properties
                    )
                    .unwrap()
                );
            });
        });
    let rest = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trees
        .get(id)
        .unwrap()
        .clone();
    assert!(rest.stage.is_generated());
    assert!(!rest.wood.is_empty());
    wait(&mut host, &mut [&mut a, &mut b, &mut late], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ClientSession>()
                .trees
                .get(id)
                == Some(&rest)
        })
    });
}
#[test]
fn internal_derived_fruit_is_never_a_grantable_or_placeable_inventory_item() {
    assert!(!ItemKind::Prop(ResourcePropKind::AppleFruit).valid());
    assert!(!ResourcePropKind::ALL.contains(&ResourcePropKind::AppleFruit));
    for species in TreeSpecies::ALL {
        assert!(ItemKind::Prop(species.sapling()).valid());
    }
}

#[test]
fn ripe_partly_harvested_tree_control_snapshot_keeps_exact_picked_slots_and_last_harvest_day() {
    use terra_core::{FruitPhase, FruitState, TreeRecord};
    let mut expected = terra_core::TreeRegistry::default();
    assert!(expected.insert(TreeRecord {
        id: 7,
        root: [8, 16, 24],
        species: TreeSpecies::Apple,
        seed: 7,
        planted_by: None,
        stage: TreeStage::Mature,
        growth_seconds: 0.,
        bounds: None,
        wood: vec![],
        leaves: vec![],
        fruit: Some(FruitState {
            phase: FruitPhase::Ripe,
            remaining: 5,
            picked: 0b00100101,
            cycle_day: 10,
            last_harvest_day: Some(12),
            revision: 9
        }),
        revision: 12
    }));
    let wire = protocol::session::ServerControl::Trees(expected.clone());
    let encoded = postcard::to_allocvec(&wire).unwrap();
    let protocol::session::ServerControl::Trees(decoded) = postcard::from_bytes(&encoded).unwrap()
    else {
        panic!("wrong tree control variant")
    };
    assert!(decoded.valid());
    assert_eq!(decoded, expected);
    let fruit = decoded.get(7).unwrap().fruit.unwrap();
    assert_eq!(fruit.picked, 0b00100101);
    assert_eq!(fruit.remaining, 5);
    assert_eq!(fruit.last_harvest_day, Some(12));
}

#[test]
fn udp_large_tree_patches_accumulate_interest_and_preserve_generated_felled_rests() {
    let (mut host, mut a, mut b) = pair();
    let state = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let root = state.position.map(|value| value.floor() as i64);
    let mut expected = std::collections::BTreeMap::new();
    for id in 1..=3 {
        let mut cells = (0..20_000)
            .map(|n| {
                [
                    (n % 32) as i8 - 16,
                    (n / 1024) as i8,
                    (n / 32 % 32) as i8 - 16,
                ]
            })
            .collect::<Vec<_>>();
        let leaves = cells.split_off(4096);
        let wood = cells;
        assert_eq!((wood.len(), leaves.len()), (4096, 15_904));
        let tree = terra_core::TreeRecord {
            id,
            root: [root[0] + id as i64, root[1], root[2]],
            species: TreeSpecies::Oak,
            seed: id,
            planted_by: Some(terra_core::PlayerUuid::from_u128(1)),
            stage: TreeStage::Generated,
            growth_seconds: 900.,
            wood,
            leaves,
            bounds: None,
            fruit: None,
            revision: 1,
        }
        .with_bounds();
        assert!(tree.valid());
        expected.insert(id, tree);
    }
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        for tree in expected.values() {
            assert!(journal.gameplay_mut().trees.insert(tree.clone()));
        }
    }
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            let session = client.world().resource::<client::ClientSession>();
            session.trees.trees == expected && session.trees.revision >= 4
        })
    });
    let id = 1;
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let tree = journal.gameplay_mut().trees.get_mut(id).unwrap();
        tree.leaves.remove(99);
        tree.revision += 1;
        tree.refresh_bounds();
        expected.insert(id, tree.clone());
    }
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ClientSession>()
                .trees
                .trees
                == expected
        })
    });
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let tree = journal.gameplay_mut().trees.get_mut(id).unwrap();
        tree.wood.clear();
        tree.stage = TreeStage::GeneratedFelled;
        tree.fruit = None;
        tree.refresh_bounds();
        tree.revision += 1;
        let tree = journal.gameplay().trees.get(id).unwrap();
        assert_eq!(tree.stage, TreeStage::GeneratedFelled);
        assert!(!tree.leaves.is_empty());
        expected.insert(id, tree.clone());
    }
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ClientSession>()
                .trees
                .trees
                == expected
        })
    });
}
