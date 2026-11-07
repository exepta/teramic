use super::*;
use terra_core::{
    ArmorItem, ArmorMaterial, ArmorPiece, EquipmentSlot, Inventory, ItemStack, ToolKind,
};
use terra_voxel::{BlockSection, SectionPos};
use terra_world::streaming::LoadedSection;

struct Arena {
    config: GameConfig,
    chunks: ChunkStore,
    journal: EditJournal,
    properties: terra_core::block_properties::BlockPropertiesCatalog,
    weather: terra_core::WorldWeather,
    positions: [Vec3; 2],
    blockers: Vec<Vec3>,
}
impl Arena {
    fn new() -> Self {
        let mut config = GameConfig::default();
        config.world.game_rules.pvp = true;
        let mut chunks = ChunkStore::default();
        let position = SectionPos::default();
        chunks.loaded.insert(
            position,
            LoadedSection {
                data: BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        Self {
            config,
            chunks,
            journal: EditJournal::default(),
            properties: Default::default(),
            weather: Default::default(),
            positions: [Vec3::new(8.5, 2.0, 8.5), Vec3::new(8.5, 2.0, 5.5)],
            blockers: Vec::new(),
        }
    }
    fn environment(&self) -> Environment<'_> {
        Environment {
            config: &self.config,
            chunks: &self.chunks,
            journal: &self.journal,
            properties: &self.properties,
            weather: &self.weather,
            positions: self.positions,
            other_players: &self.blockers,
        }
    }
}
fn players() -> (Peer, Peer) {
    let mut attacker = super::super::survival::tests::resting_peer(100.0);
    attacker.creature_action_at = Instant::now() - Duration::from_secs(1);
    let mut victim = super::super::survival::tests::resting_peer(50.0);
    victim.id = PlayerId(2);
    (attacker, victim)
}

#[test]
fn pvp_rule_and_shared_cooldown_gate_damage_and_stamina() {
    let mut arena = Arena::new();
    let (mut attacker, mut victim) = players();
    let stamina = attacker.vitals.stamina;
    let ready_at = attacker.creature_action_at;
    arena.config.world.game_rules.pvp = false;
    assert!(!strike(
        &mut attacker,
        &mut victim,
        [0.0; 2],
        arena.environment()
    ));
    assert_eq!(victim.vitals.health, 50.0);
    assert_eq!(victim.survival.damage_epoch, 0);
    assert_eq!(attacker.vitals.stamina, stamina);
    assert_eq!(attacker.creature_action_at, ready_at);
    arena.config.world.game_rules.pvp = true;
    assert!(strike(
        &mut attacker,
        &mut victim,
        [0.0; 2],
        arena.environment()
    ));
    assert_eq!(victim.vitals.health, 49.0);
    assert_eq!(victim.vitals.last_damage, Some(DamageCause::Player));
    assert_eq!(victim.survival.damage_epoch, 1);
    assert_eq!(attacker.vitals.stamina, stamina - 2.0);
    assert!(!strike(
        &mut attacker,
        &mut victim,
        [0.0; 2],
        arena.environment()
    ));
    assert_eq!(victim.vitals.health, 49.0);
    assert_eq!(attacker.vitals.stamina, stamina - 2.0);
    attacker.creature_action_at = Instant::now() - Duration::from_millis(500);
    assert!(strike(
        &mut attacker,
        &mut victim,
        [0.0; 2],
        arena.environment()
    ));
    assert_eq!(victim.vitals.health, 48.0);
    assert_eq!(victim.survival.damage_epoch, 2);
}

#[test]
fn melee_requires_range_aim_clear_solid_line_of_sight_and_loaded_cells() {
    for invalid in 0..4 {
        let mut arena = Arena::new();
        let (mut attacker, mut victim) = players();
        let mut look = [0.0; 2];
        match invalid {
            0 => arena.positions[1].z = 0.5,
            1 => look[0] = std::f32::consts::PI,
            2 => {
                arena
                    .chunks
                    .loaded
                    .get_mut(&SectionPos::default())
                    .unwrap()
                    .data
                    .set_block([8, 2, 7], 3)
                    .unwrap();
            }
            _ => arena.chunks.loaded.clear(),
        }
        let stamina = attacker.vitals.stamina;
        assert!(
            !strike(&mut attacker, &mut victim, look, arena.environment()),
            "invalid geometry case {invalid}"
        );
        assert_eq!(victim.vitals.health, 50.0);
        assert_eq!(victim.survival.damage_epoch, 0);
        assert_eq!(attacker.vitals.stamina, stamina);
    }
}

#[test]
fn survival_melee_needs_stamina_and_respects_armor_while_creative_attacks_are_free() {
    let arena = Arena::new();
    let (mut attacker, mut victim) = players();
    attacker.vitals.stamina = 1.0;
    assert!(!strike(
        &mut attacker,
        &mut victim,
        [0.0; 2],
        arena.environment()
    ));
    assert_eq!(victim.vitals.health, 50.0);
    attacker.vitals.stamina = 2.0;
    victim.equipment.set(
        EquipmentSlot::Chestplate,
        Some(ItemKind::Armor(ArmorItem {
            material: ArmorMaterial::Iron,
            piece: ArmorPiece::Chestplate,
        })),
    );
    victim.vitals.apply_equipment(victim.equipment);
    let reduced = 1.0 - victim.vitals.armor_points * 0.02;
    assert!(reduced < 1.0 && reduced > 0.0);
    assert!(strike(
        &mut attacker,
        &mut victim,
        [0.0; 2],
        arena.environment()
    ));
    assert!((victim.vitals.health - (50.0 - reduced)).abs() < 0.001);
    assert_eq!(attacker.vitals.stamina, 0.0);
    attacker.creature_action_at = Instant::now() - Duration::from_secs(1);
    attacker.mode = Some(GameMode::Creative);
    assert!(strike(
        &mut attacker,
        &mut victim,
        [0.0; 2],
        arena.environment()
    ));
    assert_eq!(attacker.vitals.stamina, 0.0);
    assert!((victim.vitals.health - (50.0 - reduced * 2.0)).abs() < 0.001);
}

#[test]
fn protected_modes_self_dead_unaccepted_and_unpermitted_players_cannot_take_melee_damage() {
    for immune in 0..10 {
        let arena = Arena::new();
        let (mut attacker, mut victim) = players();
        match immune {
            0 => victim.mode = Some(GameMode::Creative),
            1 => victim.mode = Some(GameMode::Spectator),
            2 => attacker.mode = Some(GameMode::Spectator),
            3 => victim.id = attacker.id,
            4 => attacker.vitals.health = 0.0,
            5 => victim.vitals.health = 0.0,
            6 => victim.survival.protection = 2.0,
            7 => attacker.permissions.build = false,
            8 => attacker.accepted = false,
            _ => victim.synced = false,
        }
        let health = victim.vitals.health;
        let stamina = attacker.vitals.stamina;
        assert!(
            !strike(&mut attacker, &mut victim, [0.0; 2], arena.environment()),
            "immunity case {immune}"
        );
        assert_eq!(victim.vitals.health, health);
        assert_eq!(victim.survival.damage_epoch, 0);
        assert_eq!(attacker.vitals.stamina, stamina);
    }
}

#[test]
fn held_tool_damage_can_kill_once_without_consuming_the_tool() {
    let mut arena = Arena::new();
    let (mut attacker, mut victim) = players();
    let tool = ItemStack {
        item: ItemKind::Tool(ToolKind::RubySword),
        count: 1,
    };
    arena.journal.gameplay_mut().inventories.insert(
        attacker.uuid,
        Inventory {
            slots: vec![tool],
            ..Default::default()
        },
    );
    attacker.held_slot = Some(0);
    victim.vitals.health = 5.0;
    assert!(strike(
        &mut attacker,
        &mut victim,
        [0.0; 2],
        arena.environment()
    ));
    assert!(victim.vitals.dead());
    assert_eq!(victim.vitals.health, 0.0);
    assert_eq!(victim.vitals.last_damage, Some(DamageCause::Player));
    assert_eq!(victim.survival.damage_epoch, 1);
    assert_eq!(
        arena.journal.gameplay().inventories[&attacker.uuid].slot(0),
        Some(tool)
    );
    attacker.creature_action_at = Instant::now() - Duration::from_secs(1);
    assert!(!strike(
        &mut attacker,
        &mut victim,
        [0.0; 2],
        arena.environment()
    ));
    assert_eq!(victim.survival.damage_epoch, 1);
}

#[test]
fn forged_farther_target_is_rejected_when_a_nearer_player_intercepts_the_ray() {
    let mut arena = Arena::new();
    let (mut attacker, mut victim) = players();
    arena.blockers.push(Vec3::new(8.5, 2.0, 7.0));
    assert!(!strike(
        &mut attacker,
        &mut victim,
        [0.0; 2],
        arena.environment()
    ));
    assert_eq!(victim.vitals.health, 50.0);
    assert_eq!(attacker.vitals.stamina, 100.0);
    arena.blockers[0].x += 2.0;
    assert!(strike(
        &mut attacker,
        &mut victim,
        [0.0; 2],
        arena.environment()
    ));
}
