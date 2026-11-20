//! Presentation changes follow replicated professions without changing NPC identity.
use super::{Kind, model};
use bevy::prelude::*;
use terra_core::creatures::Creature;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) struct Appearance(pub (Kind, u8));

impl Appearance {
    pub(super) fn of(animal: &Creature) -> Self {
        Self((
            animal.kind,
            match animal.kind {
                Kind::Horse | Kind::Butterfly | Kind::SpiritGloom | Kind::Skeleton => {
                    animal.variant
                }
                Kind::Villager => model::villager_variant(animal.variant, animal.sex()),
                Kind::Fish => u8::from(animal.variant >= 2),
                _ => 0,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creatures::{CreatureAvatar, Models, VillagerHand};
    use std::time::Instant;
    use terra_core::creatures::{CreatureBehavior, CreatureNeeds, CreatureSex, villagers};
    use terra_network::client::{CreatureTrack, ReplicatedCreatures};

    #[test]
    fn replicated_profession_changes_rebuild_the_avatar_and_tool_without_changing_sex_or_id() {
        for sex in [CreatureSex::Male, CreatureSex::Female] {
            let id = (1..100)
                .find(|id| CreatureSex::from_spawn_id(*id) == sex)
                .unwrap();
            let animal = Creature {
                id,
                kind: Kind::Villager,
                home: [0, 0],
                position: [0.5, 1.0, 0.5],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: 20.0,
                behavior: CreatureBehavior::Rest,
                timer: 0.0,
                age: 0.0,
                variant: villagers::UNEMPLOYED,
                needs: CreatureNeeds::full(Kind::Villager),
            };
            let mut replicas = ReplicatedCreatures::default();
            replicas.creatures.insert(
                id,
                CreatureTrack {
                    previous: animal.clone(),
                    current: animal.clone(),
                    received: Instant::now(),
                    interval: 0.1,
                    playback: Default::default(),
                },
            );
            let mut models = Models::default();
            for role in [villagers::UNEMPLOYED, villagers::LUMBERJACK] {
                let mut dressed = animal.clone();
                dressed.variant = role;
                models.parts.insert(
                    Appearance::of(&dressed).0,
                    vec![
                        (model::Part::Body, Vec3::ZERO, Handle::default()),
                        (model::Part::VillagerArm(1.0), Vec3::ZERO, Handle::default()),
                    ],
                );
            }
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .insert_resource(models)
                .insert_resource(replicas)
                .add_systems(Update, crate::creatures::render);
            app.update();
            let first = app
                .world_mut()
                .query_filtered::<Entity, With<CreatureAvatar>>()
                .single(app.world())
                .unwrap();
            let old_children = app.world().get::<Children>(first).unwrap().to_vec();
            assert_eq!(
                app.world_mut()
                    .query::<&VillagerHand>()
                    .iter(app.world())
                    .count(),
                0
            );
            app.world_mut()
                .resource_mut::<ReplicatedCreatures>()
                .creatures
                .get_mut(&id)
                .unwrap()
                .current
                .variant = villagers::LUMBERJACK;
            app.update();
            assert!(app.world().get_entity(first).is_err());
            assert!(
                old_children
                    .iter()
                    .all(|child| app.world().get_entity(*child).is_err())
            );
            let (avatar, appearance) = app
                .world_mut()
                .query::<(&CreatureAvatar, &Appearance)>()
                .single(app.world())
                .unwrap();
            assert_eq!(avatar.0, id);
            assert_eq!(
                appearance.0,
                (
                    Kind::Villager,
                    model::villager_variant(villagers::LUMBERJACK, sex)
                )
            );
            assert_eq!(
                app.world().resource::<ReplicatedCreatures>().creatures[&id]
                    .current
                    .sex(),
                sex
            );
            let hand = app
                .world_mut()
                .query::<&VillagerHand>()
                .single(app.world())
                .unwrap();
            assert_eq!(
                hand.item,
                terra_core::ItemKind::Tool(terra_core::ToolKind::IronAxe)
            );
        }
    }

    #[test]
    fn both_lumberjack_models_have_distinct_bounded_meshes_and_share_neither_gender_key() {
        use bevy::mesh::VertexAttributeValues;
        let mut fingerprints = std::collections::BTreeSet::new();
        for sex in [CreatureSex::Male, CreatureSex::Female] {
            let key = model::villager_variant(villagers::LUMBERJACK, sex);
            let parts = model::parts_variant(Kind::Villager, key).unwrap();
            assert_eq!(parts.len(), 6);
            assert!(
                parts
                    .iter()
                    .map(|(_, _, mesh)| mesh.count_vertices())
                    .sum::<usize>()
                    < 50000
            );
            let mut fingerprint = 14695981039346656037_u64;
            for (_, _, mesh) in parts {
                let Some(VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("Missing positions");
                };
                for value in positions.iter().flatten() {
                    assert!(value.is_finite());
                    fingerprint =
                        fingerprint.wrapping_mul(1099511628211) ^ u64::from(value.to_bits());
                }
            }
            assert!(fingerprints.insert(fingerprint));
        }
    }
}
