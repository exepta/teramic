//! Optional real founding/save/reload verification; no fabricated client snapshots.
use bevy::prelude::*;
use std::{collections::BTreeMap, path::PathBuf};
use terra_core::kingdom::KingdomId;
use terra_network::client::{ClientSession, ReplicatedCreatures};

pub(crate) fn verify(world: &World, id: KingdomId, reload: bool) -> Result<bool, String> {
    if std::env::var("TERRA_KINGDOM_STARTERS").as_deref() != Ok("1") {
        return Ok(true);
    }
    let save = PathBuf::from(
        std::env::var_os("TERRA_KINGDOM_SAVE_DIR").ok_or("Missing isolated kingdom save path")?,
    );
    let config = &world.resource::<terra_core::GameConfig>().world;
    let journal = terra_storage::load_edits(&save, config.id)?;
    let game = journal.gameplay();
    let grant = game
        .kingdom_starters
        .grants
        .get(&id)
        .ok_or("Persisted founding receipt missing")?;
    if Some(grant.founder) != world.resource::<ClientSession>().uuid
        || game.kingdom_starters.grants.len() != 1
    {
        return Err("Saved starter ownership or number of founding receipts is incorrect".into());
    }
    let mut roles = BTreeMap::<u8, usize>::new();
    for id in grant.ids {
        let npc = game
            .creatures
            .animals
            .get(&id)
            .ok_or("Persisted starter NPC missing")?;
        if game.creatures.owners.get(&id) != Some(&grant.founder) || npc.health <= 0.0 {
            return Err("Persisted starter is dead or owned by another player".into());
        }
        *roles.entry(npc.variant).or_default() += 1;
    }
    if roles
        != [
            (10, 3),
            (12, 2),
            (11, 3),
            (terra_core::creatures::villagers::UNEMPLOYED, 10),
        ]
        .into_iter()
        .collect()
    {
        return Err(format!("Persisted starter role counts differ: {roles:?}"));
    }
    let replicas = world.resource::<ReplicatedCreatures>();
    if grant
        .ids
        .iter()
        .any(|id| !replicas.creatures.contains_key(id))
    {
        return Ok(false);
    }
    for id in grant.ids {
        if replicas.creatures[&id].current.variant != game.creatures.animals[&id].variant {
            return Err("Replicated starter role differs from authoritative saved NPC".into());
        }
    }
    let report_dir = PathBuf::from(
        std::env::var_os("TERRA_CAPTURE_DIR").ok_or("Missing acceptance output directory")?,
    );
    let report = report_dir.join("kingdom-starters.json");
    let identity = serde_json::json!({"kingdom": id, "founder": grant.founder, "ids": grant.ids, "roles": roles});
    if reload {
        let expected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&report).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if expected != identity {
            return Err("Reload changed starter IDs, owner, roles or number of grants".into());
        }
    } else {
        std::fs::create_dir_all(&report_dir).map_err(|e| e.to_string())?;
        std::fs::write(
            &report,
            serde_json::to_vec_pretty(&identity).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    info!(
        ?id,
        reload,
        count = grant.ids.len(),
        "Native F80 accepted: 18 persisted and replicated starters, exact roles and stable founding receipt"
    );
    Ok(true)
}
