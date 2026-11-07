//! Explored settlement metadata survives chunk eviction beside the map tile book.
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, futures_lite::future},
};
use std::{collections::BTreeMap, path::PathBuf};
use terra_core::{GameConfig, kingdom::SettlementDomain};
use terra_network::client::ClientSession;
use terra_storage::maps::{MapIdentity, domains};
use terra_world::streaming::ChunkStore;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SettlementMapSet;
#[derive(Resource, Default)]
pub struct SettlementMap {
    identity: Option<MapIdentity>,
    root: PathBuf,
    known: BTreeMap<u64, SettlementDomain>,
    revision: u64,
    load: Option<Task<Result<Vec<SettlementDomain>, String>>>,
    save: Option<Task<Result<(), String>>>,
    loaded: bool,
    dirty: bool,
    next: f64,
}
impl SettlementMap {
    pub fn domains(&self) -> impl Iterator<Item = &SettlementDomain> {
        self.known.values()
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn remember(&mut self, domain: SettlementDomain) -> bool {
        if !domain.valid()
            || self.known.get(&domain.id) == Some(&domain)
            || (!self.known.contains_key(&domain.id) && self.known.len() >= domains::MAX_DOMAINS)
        {
            return false;
        }
        self.known.insert(domain.id, domain);
        self.revision = self.revision.wrapping_add(1);
        self.dirty = true;
        true
    }
    fn retire(&mut self) {
        if self.identity.is_none() {
            return;
        }
        let pending = self.save.take();
        let loading = self.load.take();
        if !self.dirty && pending.is_none() {
            return;
        }
        let root = self.root.clone();
        let live = self.known.clone();
        IoTaskPool::get()
            .spawn(async move {
                if let Err(error) = finish_write(root, pending, loading, live).await {
                    warn!(%error,"Could not flush previous world's settlement metadata");
                }
            })
            .detach();
    }
    fn merge_saved(&mut self, saved: Vec<SettlementDomain>) {
        for domain in saved {
            if !self.known.contains_key(&domain.id) {
                self.remember(domain);
            }
        }
    }
}
/// Finish the older write before replacing it with the latest in-memory book.
/// A pending load contributes only absent IDs; current streamed metadata wins.
async fn finish_write(
    root: PathBuf,
    pending: Option<Task<Result<(), String>>>,
    loading: Option<Task<Result<Vec<SettlementDomain>, String>>>,
    live: BTreeMap<u64, SettlementDomain>,
) -> Result<(), String> {
    let mut known = BTreeMap::new();
    if let Some(loading) = loading
        && let Ok(saved) = loading.await
    {
        known.extend(saved.into_iter().map(|d| (d.id, d)));
    }
    known.extend(live);
    if let Some(pending) = pending {
        let _ = pending.await;
    }
    domains::save(&root, &known.into_values().collect::<Vec<_>>())
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<SettlementMap>().add_systems(
        Update,
        update
            .in_set(SettlementMapSet)
            .after(terra_world::streaming::StreamingSet::Receive),
    );
}
fn update(
    session: Option<Res<ClientSession>>,
    config: Option<Res<GameConfig>>,
    chunks: Option<Res<ChunkStore>>,
    paths: Option<Res<crate::menu::MenuPaths>>,
    time: Option<Res<Time>>,
    mut book: ResMut<SettlementMap>,
) {
    let identity = session
        .as_ref()
        .filter(|s| s.ready)
        .and_then(|s| s.uuid)
        .zip(config.as_ref())
        .map(|(player, config)| MapIdentity {
            world: config.world.id,
            seed: config.world.seed,
            generator: config.world.generator_version,
            player,
        });
    if book.identity != identity {
        book.retire();
        let revision = book.revision.wrapping_add(1);
        *book = SettlementMap {
            identity: identity.clone(),
            revision,
            ..default()
        };
        if let Some(identity) = identity {
            let configs = paths
                .as_ref()
                .map_or_else(terra_core::menu_config_directory, |p| p.configs.clone());
            book.root = identity.directory(&configs.join("maps"));
            let root = book.root.clone();
            book.load = Some(IoTaskPool::get().spawn(async move { domains::load(&root) }));
        }
    }
    if book.identity.is_none() {
        return;
    }
    if let Some(result) = book
        .load
        .as_mut()
        .and_then(|job| future::block_on(future::poll_once(job)))
    {
        book.load = None;
        book.loaded = true;
        match result {
            Ok(saved) => book.merge_saved(saved),
            Err(error) => warn!(%error,"Could not load settlement exploration metadata"),
        }
    }
    if let Some(result) = book
        .save
        .as_mut()
        .and_then(|job| future::block_on(future::poll_once(job)))
    {
        book.save = None;
        if let Err(error) = result {
            book.dirty = true;
            warn!(%error,"Could not save settlement exploration metadata");
        }
    }
    let now = time.as_ref().map_or(0.0, |t| t.elapsed_secs_f64());
    if now >= book.next {
        book.next = now + 1.0;
        if let Some(chunks) = chunks {
            for site in chunks.village_settlements() {
                book.remember(site.domain());
            }
        }
        if book.loaded && book.dirty && book.save.is_none() {
            let root = book.root.clone();
            let saved = book.known.values().copied().collect::<Vec<_>>();
            book.save = Some(IoTaskPool::get().spawn(async move { domains::save(&root, &saved) }));
            book.dirty = false;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::kingdom::{BotKingdom, DomainKind};
    #[test]
    fn remembered_domains_survive_eviction_and_saved_data_never_overwrites_live_metadata() {
        let mut book = SettlementMap::default();
        let old = SettlementDomain {
            id: 1,
            center: [0, 10, 0],
            bounds: [[-8, 0, -8], [8, 20, 8]],
            kind: DomainKind::Village,
            native: Some(BotKingdom::Lutera),
        };
        assert!(book.remember(old));
        let revision = book.revision();
        assert!(!book.remember(old));
        assert_eq!(book.revision(), revision);
        let live = SettlementDomain {
            native: Some(BotKingdom::Petora),
            ..old
        };
        assert!(book.remember(live));
        book.merge_saved(vec![old]);
        assert_eq!(book.domains().copied().collect::<Vec<_>>(), vec![live]);
        // No reference to ChunkStore remains: unloading cells cannot lose ownership.
        assert_eq!(book.domains().count(), 1);
    }
    #[test]
    fn disconnect_flush_waits_for_old_write_and_persists_latest_identity_snapshot() {
        let root =
            std::env::temp_dir().join(format!("terra-domain-disconnect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let old = SettlementDomain {
            id: 5,
            center: [0, 10, 0],
            bounds: [[-8, 0, -8], [8, 20, 8]],
            kind: DomainKind::Village,
            native: Some(BotKingdom::Lutera),
        };
        let latest = SettlementDomain {
            native: Some(BotKingdom::Petora),
            ..old
        };
        let pool = IoTaskPool::get_or_init(bevy::tasks::TaskPool::new);
        let first = root.clone();
        let pending = pool.spawn(async move { domains::save(&first, &[old]) });
        let mut app = App::new();
        install(&mut app);
        *app.world_mut().resource_mut::<SettlementMap>() = SettlementMap {
            identity: Some(MapIdentity {
                world: terra_core::WorldId::default(),
                seed: terra_core::WorldSeed::default(),
                generator: 1,
                player: terra_core::PlayerUuid::from_u128(5),
            }),
            root: root.clone(),
            known: BTreeMap::from([(latest.id, latest)]),
            loaded: true,
            dirty: true,
            save: Some(pending),
            ..default()
        };
        // Losing the ready session changes identity to None and clears the UI,
        // while the old world's latest book is flushed after its pending write.
        app.update();
        assert_eq!(app.world().resource::<SettlementMap>().domains().count(), 0);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if domains::load(&root).ok() == Some(vec![latest]) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Disconnect lost the latest explored settlement"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
