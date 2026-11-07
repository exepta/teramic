//! Live territorial transactions and immutable native-domain validation.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use terra_core::kingdom::{KingdomRegistry, SettlementDomain, SettlementKind};

pub(super) fn plan(
    action: &KingdomAction,
    peer: &Peer,
    world: &ControlWorld,
    actor: Option<ClaimActor>,
    domains: &[SettlementDomain],
) -> Result<Option<(KingdomRegistry, KingdomOutcome)>, KingdomError> {
    let mut registry = world.journal.gameplay().kingdoms.clone();
    let rules = &world.config.world.kingdom_rules;
    let profile = world
        .identities
        .0
        .get(&peer.id)
        .map_or("Player", |profile| profile.name.as_str());
    let outcome = match action {
        KingdomAction::Found { position, name } => KingdomOutcome::Founded {
            id: registry.found_settlement_with_rules(
                peer.uuid,
                profile,
                *position,
                name,
                SettlementKind::Kingdom,
                Default::default(),
                rules,
                domains,
            )?,
        },
        KingdomAction::FoundSettlement {
            position,
            name,
            kind,
            heraldry,
        } => KingdomOutcome::Founded {
            id: registry.found_settlement_with_rules(
                peer.uuid, profile, *position, name, *kind, *heraldry, rules, domains,
            )?,
        },
        KingdomAction::ClaimBatch { id, claims } => KingdomOutcome::ClaimsApplied {
            id: *id,
            count: registry.claim_batch_with_rules(
                *id,
                claims,
                actor.ok_or(KingdomError::InvalidIdentity)?,
                rules,
                domains,
            )?,
        },
        KingdomAction::ClaimChunk { id, chunk, kind }
        | KingdomAction::ClaimBotChunk {
            id, chunk, kind, ..
        } => {
            registry.claim_with_rules(
                *id,
                *chunk,
                *kind,
                actor.ok_or(KingdomError::InvalidIdentity)?,
                rules,
                domains,
            )?;
            KingdomOutcome::Claimed {
                id: *id,
                chunk: *chunk,
                kind: *kind,
            }
        }
        KingdomAction::ReleaseChunk { id, chunk }
        | KingdomAction::ReleaseBotChunk { id, chunk, .. } => {
            registry.release_with_rules(peer.uuid, *id, *chunk, rules)?;
            KingdomOutcome::Released {
                id: *id,
                chunk: *chunk,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some((registry, outcome)))
}

pub(super) fn domains(
    action: &KingdomAction,
    peer: &Peer,
    world: &ControlWorld,
    actor: Option<ClaimActor>,
) -> Result<Vec<SettlementDomain>, KingdomError> {
    let (kind, points) = match action {
        KingdomAction::Found { position, .. } => {
            (SettlementKind::Kingdom, vec![claim_chunk(*position)])
        }
        KingdomAction::FoundSettlement { position, kind, .. } => {
            (*kind, vec![claim_chunk(*position)])
        }
        KingdomAction::ClaimChunk { id, chunk, .. }
        | KingdomAction::ClaimBotChunk { id, chunk, .. } => (
            world
                .journal
                .gameplay()
                .kingdoms
                .get(*id)
                .ok_or(KingdomError::UnknownKingdom)?
                .kind,
            vec![*chunk],
        ),
        KingdomAction::ClaimBatch { id, claims } => (
            world
                .journal
                .gameplay()
                .kingdoms
                .get(*id)
                .ok_or(KingdomError::UnknownKingdom)?
                .kind,
            claims.iter().map(|(chunk, _)| *chunk).collect(),
        ),
        _ => return Ok(vec![]),
    };
    let rules = &world.config.world.kingdom_rules;
    let frozen = world.terrain.0.config();
    let native_royal = frozen.structures_revision >= 6
        && (if frozen.structures_revision == 6 {
            frozen.structures.villages
        } else {
            frozen.structures.kingdoms || frozen.structures.outposts
        });
    let royal = matches!(kind, SettlementKind::Kingdom | SettlementKind::Outpost)
        && rules.realm_min_border_gap > 0
        && native_royal;
    let city = kind == SettlementKind::City && rules.city_min_build_gap > 0;
    if !royal && !city {
        return Ok(vec![]);
    }
    let mut result = world
        .chunks
        .village_settlements()
        .map(|site| site.domain())
        .map(|domain| (domain.id, domain))
        .collect::<BTreeMap<_, _>>();
    plan(
        action,
        peer,
        world,
        actor,
        &result.values().copied().collect::<Vec<_>>(),
    )?;
    let mut regions = BTreeSet::new();
    if royal {
        // A historical site spans at most 512 blocks. Its center can be outside
        // the requested claim's region, so include the complete bounded halo.
        let reach = i64::from(rules.realm_min_border_gap) + 33;
        let size = terra_world::generation::structures::REGION_SIZE;
        let mut queries = BTreeSet::new();
        for chunk in &points {
            let low = chunk.map(|value| ((value - reach) * 16).div_euclid(size));
            let high = chunk.map(|value| ((value + reach) * 16 + 15).div_euclid(size));
            queries.insert((low, high));
        }
        for (low, high) in queries {
            for region in world
                .terrain
                .0
                .royal_site_candidates_in_bounds(low, high, 1024)
                .map_err(|()| KingdomError::CapacityReached)?
            {
                regions.insert(region);
                if regions.len() > 1024 {
                    return Err(KingdomError::CapacityReached);
                }
            }
        }
    }

    if city
        && let Some(parent) = world
            .journal
            .gameplay()
            .kingdoms
            .active_for_member(peer.uuid)
    {
        let reach = i64::from(rules.city_min_build_gap) + 33;
        for claim in world
            .journal
            .gameplay()
            .kingdoms
            .claims
            .iter()
            .filter(|claim| world.journal.gameplay().kingdoms.realm_id(claim.kingdom) == parent.id)
        {
            if points.iter().any(|point| {
                (0..2).all(|axis| point[axis].abs_diff(claim.chunk[axis]) <= reach as u64)
            }) {
                regions.insert(claim.chunk.map(|value| {
                    (value * 16).div_euclid(terra_world::generation::structures::REGION_SIZE)
                }));
                if regions.len() > 1024 {
                    return Err(KingdomError::CapacityReached);
                }
            }
        }
    }
    // The query uses each world's frozen structure revision/settings. It never
    // stamps blocks, changes generation or depends on current client loading.
    for region in regions {
        if let Some(domain) = world
            .terrain
            .0
            .structure_site(region)
            .and_then(|site| site.domain())
        {
            result.insert(domain.id, domain);
            // Fail as soon as one real historical domain disproves the command;
            // large custom buffers must not author every site in their halo.
            plan(
                action,
                peer,
                world,
                actor,
                &result.values().copied().collect::<Vec<_>>(),
            )?;
        }
    }
    Ok(result.into_values().collect())
}
