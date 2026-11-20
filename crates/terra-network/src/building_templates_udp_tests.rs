use super::*;
use crate::protocol::building_jobs::{BuildingAction, BuildingSnapshot};
use terra_core::{
    PlayerLocation, PlayerProfile,
    building_jobs::{BuildArea, BuildError, HOUSE_TEMPLATE},
    kingdom::{ClaimActor, ClaimKind, KingdomId},
};
use terra_world::edits::EditJournal;

fn command(
    host: &mut App,
    clients: &mut [&mut App],
    actor: usize,
    action: BuildingAction,
) -> Option<BuildError> {
    let request = clients[actor]
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .make_building_request(action)
        .unwrap();
    let nonce = request.0.request;
    clients[actor].world_mut().write_message(request);
    wait(host, clients, |_, clients| {
        clients[actor]
            .world()
            .resource::<client::ClientSession>()
            .last_building_result
            .as_ref()
            .is_some_and(|result| result.request == nonce)
    });
    clients[actor]
        .world()
        .resource::<client::ClientSession>()
        .last_building_result
        .as_ref()
        .unwrap()
        .error
}

fn host() -> App {
    let mut app = base(
        WorldMode::DedicatedServer,
        connection::NetworkRole::DedicatedServer,
    );
    let config = {
        let mut config = app.world_mut().resource_mut::<GameConfig>();
        config.world.base_height = 11.0;
        config.world.sea_level = 0.0;
        config.world.rivers = false;
        config.world.structures = terra_core::StructureSettings {
            villages: false,
            ruins: false,
            mines: false,
            kingdoms: false,
            outposts: false,
        };
        config.server.natural_mob_limit = Some(0);
        config.world.clone()
    };
    app.world_mut()
        .resource_mut::<terra_world::streaming::WorldTerrain>()
        .0 = terra_world::TerraGenerator::new(config).unwrap();
    app.insert_resource(server::ServerEndpoint {
        bind: ([127, 0, 0, 1], 0).into(),
        advertised: None,
    });
    app.finish();
    app.cleanup();
    app.update();
    assert!(
        app.world()
            .resource::<server::ServerRuntime>()
            .error
            .is_none()
    );
    app
}

fn snapshot(app: &App) -> &BuildingSnapshot {
    &app.world().resource::<client::ClientSession>().buildings
}

#[test]
fn real_udp_inspect_material_preview_selected_house_and_auto_selection_use_server_json_and_reject_forged_ids()
 {
    let mut host = host();
    let owner = PlayerProfile::default();
    let visitor = PlayerProfile::default();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        for (profile, position) in [(&owner, [8.5, 12.92, 9.5]), (&visitor, [11.5, 12.92, 9.5])] {
            journal
                .update_player_location(
                    profile.uuid,
                    PlayerLocation {
                        position,
                        look: [0.0; 2],
                        flying: false,
                    },
                )
                .unwrap();
        }
    }
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut a = profile_client(address, owner.clone());
    let mut b = profile_client(address, visitor);
    wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
        clients
            .iter()
            .all(|app| app.world().resource::<client::ClientSession>().ready)
            && [[1, 9, 17], [13, 21, 30]].into_iter().all(|cell| {
                host.world()
                    .resource::<ChunkStore>()
                    .block_at(cell)
                    .is_some()
            })
    });
    let keep = [8, 12, 8];
    let kingdom: KingdomId = {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        journal.record(keep, terra_core::castle::BERGFRIED).unwrap();
        let registry = &mut journal.gameplay_mut().kingdoms;
        let id = registry
            .found(owner.uuid, keep, "Template authority")
            .unwrap();
        registry
            .claim(
                id,
                [0, 1],
                ClaimKind::ProtectedBuilding,
                ClaimActor::Player(owner.uuid),
            )
            .unwrap();
        id
    };
    assert_eq!(
        command(
            &mut host,
            &mut [&mut a, &mut b],
            0,
            BuildingAction::Inspect { kingdom }
        ),
        None
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        snapshot(clients[0]).template("spruce-cottage-v1").is_some()
    });
    let catalog = terra_world::building_jobs::template_catalog().unwrap();
    let advertised = snapshot(&a);
    assert!(advertised.catalog_error.is_none());
    assert_eq!(advertised.templates, catalog.summaries());
    assert_eq!(
        advertised.settlements[&kingdom].selected_template, HOUSE_TEMPLATE,
        "old/default towns keep the original house"
    );
    let permanent = catalog.get("spruce-cottage-v1").unwrap().materials.clone();
    assert_eq!(
        advertised.template("spruce-cottage-v1").unwrap().materials,
        permanent
    );
    assert_ne!(
        advertised.template(HOUSE_TEMPLATE).unwrap().materials,
        permanent,
        "the real cottage has its own authored inputs"
    );
    assert_eq!(
        command(
            &mut host,
            &mut [&mut a, &mut b],
            1,
            BuildingAction::Inspect { kingdom }
        ),
        Some(BuildError::Protected)
    );
    assert!(snapshot(&b).templates.is_empty());
    let storage = BuildArea::new([12, 12, 12], [14, 14, 14]).unwrap();
    assert_eq!(
        command(
            &mut host,
            &mut [&mut a, &mut b],
            0,
            BuildingAction::SetStorage {
                kingdom,
                area: storage
            }
        ),
        None
    );
    let residential = BuildArea::new([1, 12, 17], [14, 22, 30]).unwrap();
    assert_eq!(
        command(
            &mut host,
            &mut [&mut a, &mut b],
            0,
            BuildingAction::SetResidential {
                kingdom,
                area: Some(residential),
                template: "spruce-cottage-v1".into()
            }
        ),
        None
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        snapshot(clients[0])
            .settlements
            .get(&kingdom)
            .is_some_and(|site| {
                site.residential == Some(residential)
                    && site.selected_template == "spruce-cottage-v1"
            })
    });
    let origin = [8, 12, 24];
    assert_eq!(
        command(
            &mut host,
            &mut [&mut a, &mut b],
            0,
            BuildingAction::QueueHouse {
                kingdom,
                origin,
                facing: 1,
                template: "spruce-cottage-v1".into()
            }
        ),
        None
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        snapshot(clients[0])
            .jobs
            .values()
            .any(|job| job.origin == origin && job.template == "spruce-cottage-v1")
    });
    let job = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .buildings
        .jobs
        .values()
        .next()
        .unwrap()
        .clone();
    assert_eq!(job.template, "spruce-cottage-v1");
    assert_eq!(job.facing, 1);
    for authored in catalog
        .get("spruce-cottage-v1")
        .unwrap()
        .placed(origin, 1)
        .unwrap()
    {
        assert!(
            job.steps.iter().any(|step| step == &authored),
            "queued steps come from selected server JSON"
        );
    }
    let paid = job.required_materials();
    assert!(
        permanent.iter().all(|input| paid
            .iter()
            .any(|cost| cost.item == input.item && cost.count >= input.count)),
        "site scaffolds supplement the permanent preview; costs are never client supplied"
    );
    let next = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .buildings
        .next_id;
    for template in ["missing-template-v1", "../../world.toml"] {
        assert_eq!(
            command(
                &mut host,
                &mut [&mut a, &mut b],
                0,
                BuildingAction::QueueHouse {
                    kingdom,
                    origin,
                    facing: 0,
                    template: template.into()
                }
            ),
            Some(BuildError::InvalidTemplate)
        );
        assert_eq!(
            command(
                &mut host,
                &mut [&mut a, &mut b],
                0,
                BuildingAction::SetResidential {
                    kingdom,
                    area: Some(residential),
                    template: template.into()
                }
            ),
            Some(BuildError::InvalidTemplate)
        );
    }
    let game = host.world().resource::<EditJournal>().gameplay();
    assert_eq!(game.buildings.next_id, next);
    assert_eq!(game.buildings.jobs.len(), 1);
    assert_eq!(
        game.buildings.settlements[&kingdom].selected_template,
        "spruce-cottage-v1"
    );
    assert_eq!(
        game.buildings.settlements[&kingdom].residential,
        Some(residential)
    );
    live_stock_preview_and_reload(&mut host, &mut a, &mut b, kingdom, owner.uuid);
}

/// The single existing catalog UDP test owns the live asset mutation, avoiding
/// races with another exact-catalog assertion in this same test process.
fn live_stock_preview_and_reload(
    host: &mut App,
    a: &mut App,
    b: &mut App,
    kingdom: KingdomId,
    owner: terra_core::PlayerUuid,
) {
    use terra_core::{Inventory, ItemKind, ItemStack, kingdom::KingdomRole};
    let visitor = b.world().resource::<client::ClientSession>().uuid.unwrap();
    assert_eq!(
        command(
            host,
            &mut [&mut *a, &mut *b],
            1,
            BuildingAction::ReloadCatalog { kingdom }
        ),
        Some(BuildError::Protected)
    );
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        journal
            .gameplay_mut()
            .kingdoms
            .join(visitor, kingdom)
            .unwrap();
        journal
            .gameplay_mut()
            .kingdoms
            .set_role(owner, kingdom, visitor, KingdomRole::Member)
            .unwrap();
    }
    assert_eq!(
        command(
            host,
            &mut [&mut *a, &mut *b],
            1,
            BuildingAction::ReloadCatalog { kingdom }
        ),
        Some(BuildError::Protected)
    );
    let position = [12, 12, 12];
    let props = host
        .world()
        .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .clone();
    host.world_mut()
        .resource_scope(|world, mut chunks: Mut<ChunkStore>| {
            let mut journal = world.resource_mut::<EditJournal>();
            let old = chunks.block_at(position).unwrap();
            assert!(
                terra_world::streaming::commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &terra_world::streaming::BlockEditRequest {
                        position,
                        expected_material: old,
                        material: 29
                    },
                    &props
                )
                .unwrap()
            );
            journal
                .set_container(
                    position,
                    Inventory {
                        slots: vec![ItemStack {
                            item: ItemKind::Block(16),
                            count: 21,
                        }],
                        ..Default::default()
                    },
                )
                .unwrap();
        });
    assert_eq!(
        command(
            host,
            &mut [&mut *a, &mut *b],
            0,
            BuildingAction::Preview {
                kingdom,
                template: HOUSE_TEMPLATE.into()
            }
        ),
        None
    );
    wait(host, &mut [&mut *a, &mut *b], |_, clients| {
        snapshot(clients[0])
            .preview
            .as_ref()
            .is_some_and(|p| p.id == HOUSE_TEMPLATE)
    });
    assert!(snapshot(a).preview.as_ref().unwrap().valid());
    wait(host, &mut [&mut *a, &mut *b], |_, clients| {
        snapshot(clients[0]).stock.count(ItemKind::Block(16)) == 21
    });
    assert_eq!(snapshot(a).stock_kingdom, Some(kingdom));
    assert!(snapshot(a).stock.complete);
    assert_eq!(
        command(
            host,
            &mut [&mut *a, &mut *b],
            1,
            BuildingAction::Inspect { kingdom }
        ),
        None
    );
    wait(host, &mut [&mut *a, &mut *b], |_, clients| {
        snapshot(clients[1]).stock.count(ItemKind::Block(16)) == 21
    });
    assert_eq!(snapshot(b).stock_kingdom, Some(kingdom));

    struct LiveAsset(std::path::PathBuf);
    impl Drop for LiveAsset {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
            let _ = terra_world::building_jobs::reload_template_catalog();
        }
    }
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let folder = format!("reload-test-{}-{unique}", std::process::id());
    let base = terra_core::asset_directory()
        .join("buildings")
        .join(&folder);
    std::fs::create_dir(&base).unwrap();
    let _cleanup = LiveAsset(base.clone());
    let nested = base.join("Wohnhäuser");
    std::fs::create_dir_all(&nested).unwrap();
    let catalog = terra_world::building_jobs::template_catalog().unwrap();
    let mut source = catalog.get("spruce-cottage-v1").unwrap().clone();
    source.id = format!("reload-cottage-{}-{unique}", std::process::id());
    source.display_name = "Reload cottage".into();
    std::fs::write(
        nested.join("new.json"),
        serde_json::to_vec(&source).unwrap(),
    )
    .unwrap();
    let reload_started = std::time::Instant::now();
    assert_eq!(
        command(
            host,
            &mut [&mut *a, &mut *b],
            0,
            BuildingAction::ReloadCatalog { kingdom }
        ),
        None
    );
    wait(host, &mut [&mut *a, &mut *b], |_, clients| {
        snapshot(clients[0]).template(&source.id).is_some()
    });
    let expected_path = format!("{folder}/Wohnhäuser");
    assert_eq!(
        snapshot(a).template(&source.id).unwrap().category_path,
        expected_path
    );
    let again = command(
        host,
        &mut [&mut *a, &mut *b],
        0,
        BuildingAction::ReloadCatalog { kingdom },
    );
    assert!(
        again == Some(BuildError::Busy)
            || reload_started.elapsed() >= std::time::Duration::from_secs(2) && again.is_none()
    );
    assert_eq!(
        command(
            host,
            &mut [&mut *a, &mut *b],
            1,
            BuildingAction::Inspect { kingdom }
        ),
        None
    );
    wait(host, &mut [&mut *a, &mut *b], |_, clients| {
        snapshot(clients[1]).template(&source.id).is_some()
    });
    assert_eq!(
        snapshot(b).template(&source.id).unwrap().category_path,
        expected_path
    );
    assert_eq!(
        command(
            host,
            &mut [&mut *a, &mut *b],
            1,
            BuildingAction::Preview {
                kingdom,
                template: source.id.clone()
            }
        ),
        None
    );
    wait(host, &mut [&mut *a, &mut *b], |_, clients| {
        snapshot(clients[1])
            .preview
            .as_ref()
            .is_some_and(|p| p.id == source.id)
    });
    assert_eq!(
        snapshot(b).preview.as_ref().unwrap().cells.len(),
        source.steps.iter().map(|s| s.cells.len()).sum::<usize>()
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .buildings
            .jobs
            .len(),
        1,
        "reload never replans existing work"
    );
}
