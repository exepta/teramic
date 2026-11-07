//! Live realm identity on soldier tabards, shared with settlement banners and map claims.
use super::{ReplicatedCreatures, model};
use bevy::prelude::*;
use std::collections::BTreeMap;
use terra_core::kingdom::{KingdomHeraldry, KingdomRegistry, SettlementDomain};
use terra_network::client::ClientSession;
use terra_world::streaming::ChunkStore;

#[derive(Component)]
pub(super) struct Heraldry(pub u64);
#[derive(Resource, Default)]
pub(super) struct Uniforms {
    next: f64,
    meshes: BTreeMap<KingdomHeraldry, Handle<Mesh>>,
}

fn style(
    registry: &KingdomRegistry,
    domains: &BTreeMap<[i64; 2], SettlementDomain>,
    creature: &terra_core::creatures::Creature,
) -> KingdomHeraldry {
    let p = creature.position.map(|v| v.floor() as i64);
    domains
        .get(&creature.home)
        .and_then(|d| registry.domain_heraldry(d, p))
        .or_else(|| registry.region_at(p).map(|r| r.heraldry))
        .unwrap_or_default()
}

pub(super) fn update(
    time: Res<Time>,
    mut uniforms: ResMut<Uniforms>,
    chunks: Option<Res<ChunkStore>>,
    session: Option<Res<ClientSession>>,
    animals: Option<Res<ReplicatedCreatures>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut parts: Query<(&Heraldry, &mut Mesh3d)>,
) {
    if parts.is_empty() || time.elapsed_secs_f64() < uniforms.next {
        return;
    }
    uniforms.next = time.elapsed_secs_f64() + 0.25;
    let Some((chunks, session, animals)) = chunks
        .zip(session)
        .zip(animals)
        .map(|((a, b), c)| (a, b, c))
    else {
        return;
    };
    let mut domains = BTreeMap::new();
    for site in chunks.village_settlements() {
        domains
            .entry([site.center[0].div_euclid(32), site.center[2].div_euclid(32)])
            .or_insert_with(|| site.domain());
    }
    if uniforms.meshes.len() > 128 {
        uniforms.meshes.clear();
    }
    for (identity, mut target) in &mut parts {
        let Some(creature) = animals.creatures.get(&identity.0).map(|t| &t.current) else {
            continue;
        };
        let heraldry = style(&session.kingdoms, &domains, creature);
        let source = if let Some(handle) = uniforms.meshes.get(&heraldry) {
            handle.clone()
        } else {
            let Ok(mesh) = model::soldier::tabard(heraldry) else {
                continue;
            };
            let handle = meshes.add(mesh);
            uniforms.meshes.insert(heraldry, handle.clone());
            handle
        };
        if target.0 != source {
            target.0 = source;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{
        PlayerUuid,
        creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds},
        kingdom::{BotKingdom, DomainKind},
    };
    #[test]
    fn garrison_color_follows_captured_center_and_live_changes_then_returns_to_native_realm() {
        let d = SettlementDomain {
            id: 15,
            center: [0, 20, 0],
            bounds: [[-24, 10, -24], [40, 40, 40]],
            kind: DomainKind::Kingdom,
            native: Some(BotKingdom::Petora),
        };
        let domains = BTreeMap::from([([0, 0], d)]);
        let creature = Creature {
            id: terra_core::creatures::villagers::id([0, 0], 143).unwrap(),
            kind: CreatureKind::Villager,
            home: [0, 0],
            position: [25.0, 20.0, 25.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 20.0,
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 12,
            needs: CreatureNeeds::full(CreatureKind::Villager),
        };
        let mut r = KingdomRegistry::default();
        assert_eq!(
            style(&r, &domains, &creature),
            BotKingdom::Petora.heraldry()
        );
        let owner = PlayerUuid::from_u128(72);
        let id = r.found(owner, d.center, "Westwatch").unwrap();
        r.change_appearance(owner, id, BotKingdom::Helteria.heraldry())
            .unwrap();
        assert_eq!(
            style(&r, &domains, &creature),
            BotKingdom::Helteria.heraldry()
        );
        r.dissolve(owner, id).unwrap();
        assert_eq!(
            style(&r, &domains, &creature),
            BotKingdom::Petora.heraldry()
        );
    }
}
