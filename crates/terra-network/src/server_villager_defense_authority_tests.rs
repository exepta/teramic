use super::*;
use crate::server::{creatures, villager_defense::Defenses};
use terra_core::{
    creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds, villagers},
    kingdom::KingdomRole,
};

#[test]
fn only_a_landed_authorized_player_hit_triggers_defense_and_a_true_king_is_exempt() {
    for case in 0..4 {
        let (mut app, mut peer, _) = fixture(ResourcePropKind::SmallStones, false);
        app.init_resource::<Defenses>();
        let npc = 7701;
        let is_king = case == 1;
        peer.creature_action_at = std::time::Instant::now() - std::time::Duration::from_secs(1);
        peer.held_slot = None;
        if case == 3 {
            peer.permissions.build = false;
        }
        let position = Vec3::new(4.5, 4.0 + PLAYER_HALF_HEIGHT, 8.5);
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = position;
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            let king = if is_king {
                peer.uuid
            } else {
                PlayerUuid::from_u128(999)
            };
            let realm = journal
                .gameplay_mut()
                .kingdoms
                .found(king, [12, 4, 12], "Real defense")
                .unwrap();
            if !is_king {
                journal
                    .gameplay_mut()
                    .kingdoms
                    .join(peer.uuid, realm)
                    .unwrap();
                journal
                    .gameplay_mut()
                    .kingdoms
                    .set_role(king, realm, peer.uuid, KingdomRole::Member)
                    .unwrap();
            }
            journal.gameplay_mut().creatures.animals.insert(
                npc,
                Creature {
                    id: npc,
                    kind: CreatureKind::Villager,
                    home: [0, 0],
                    position: [4.5, 4.0, 5.5],
                    velocity: [0.0; 3],
                    yaw: 0.0,
                    health: 20.0,
                    behavior: CreatureBehavior::Rest,
                    timer: 0.0,
                    age: 0.0,
                    variant: villagers::UNEMPLOYED,
                    needs: CreatureNeeds::full(CreatureKind::Villager),
                },
            );
            journal
                .gameplay_mut()
                .creatures
                .owners
                .insert(npc, peer.uuid);
        }
        if case == 2 {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            let data = &mut chunks
                .loaded
                .get_mut(&terra_voxel::SectionPos::default())
                .unwrap()
                .data;
            for y in 4..8 {
                data.set_block([4, y, 7], 3).unwrap();
            }
        }
        let eye = position + Vec3::Y * PLAYER_EYE_OFFSET;
        let target = Vec3::new(4.5, 4.9, 5.5);
        let offset = target - eye;
        let look = [0.0, offset.y.atan2(offset.x.hypot(offset.z))];
        {
            let mut state = SystemState::<ControlWorld>::new(app.world_mut());
            let mut world = state.get_mut(app.world_mut()).unwrap();
            creatures::interact(
                npc,
                false,
                look,
                &mut peer,
                &mut lightyear::prelude::MessageSender::default(),
                &mut world,
            );
        }
        let hit = case < 2;
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .gameplay()
                .creatures
                .animals[&npc]
                .health,
            if hit { 19.0 } else { 20.0 },
            "case{case}"
        );
        assert_eq!(
            app.world().resource::<Defenses>().contains(npc),
            hit && !is_king,
            "case{case}"
        );
    }
}
