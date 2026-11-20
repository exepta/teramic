use super::*;
use terra_core::{
    Inventory, ItemKind, ToolKind, WorldConfig,
    creatures::CreatureNeeds,
    kingdom::KingdomRole,
    npc_jobs::{WorkArea, WorkKind, WorkOrder, WorkerState},
};
use terra_voxel::{BlockSection, SectionPos};
use terra_world::streaming::LoadedSection;

const NPC: u64 = 7001;
fn fixture() -> (
    ChunkStore,
    EditJournal,
    terra_world::TerraGenerator,
    PlayerUuid,
    PlayerUuid,
) {
    let mut data = BlockSection::empty(SectionPos::default()).unwrap();
    for x in 0..16 {
        for z in 0..16 {
            data.set_block([x, 0, z], 3).unwrap();
        }
    }
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        SectionPos::default(),
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    let king = PlayerUuid::from_u128(11);
    let owner = PlayerUuid::from_u128(12);
    let mut journal = EditJournal::default();
    let realm = journal
        .gameplay_mut()
        .kingdoms
        .found(king, [2, 1, 2], "Defense")
        .unwrap();
    journal.gameplay_mut().kingdoms.join(owner, realm).unwrap();
    journal
        .gameplay_mut()
        .kingdoms
        .set_role(king, realm, owner, KingdomRole::Member)
        .unwrap();
    journal.gameplay_mut().creatures.animals.insert(
        NPC,
        Creature {
            id: NPC,
            kind: CreatureKind::Villager,
            home: [0, 0],
            position: [8.5, 1.0, 8.5],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 20.0,
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: terra_core::creatures::villagers::LUMBERJACK,
            needs: CreatureNeeds::full(CreatureKind::Villager),
        },
    );
    journal.gameplay_mut().creatures.owners.insert(NPC, owner);
    journal.gameplay_mut().jobs.next_id = 2;
    journal.gameplay_mut().jobs.orders.insert(
        1,
        WorkOrder {
            id: 1,
            npc: NPC,
            owner,
            kind: WorkKind::Lumberjack,
            area: WorkArea::new([1, 1, 1], [14, 4, 14]).unwrap(),
            storage: [2, 1, 3],
            priority: 5,
            enabled: true,
            revision: 7,
        },
    );
    let mut worker = WorkerState::default();
    assert_eq!(worker.insert(ItemKind::Tool(ToolKind::RubyAxe), 1), 0);
    journal.gameplay_mut().jobs.workers.insert(NPC, worker);
    let mut building_bag = Inventory::default();
    assert_eq!(building_bag.insert(ItemKind::Block(16), 12), 0);
    journal
        .gameplay_mut()
        .buildings
        .inventories
        .insert(NPC, building_bag);
    let terrain = terra_world::TerraGenerator::new(WorldConfig {
        trees: false,
        water: false,
        caves: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    (chunks, journal, terrain, king, owner)
}
fn player(position: [f64; 3]) -> Player {
    Player {
        position,
        vulnerable: true,
        protected: false,
    }
}

#[test]
fn only_the_actual_realm_sovereign_is_exempt_including_when_the_worker_is_owned_by_a_member() {
    let (_chunks, mut journal, _terrain, king, owner) = fixture();
    let mut defenses = Defenses::default();
    defenses.provoke(NPC, king, journal.gameplay());
    assert!(!defenses.contains(NPC));
    defenses.provoke(NPC, owner, journal.gameplay());
    assert!(
        defenses.contains(NPC),
        "being this NPC's owner is not royal immunity"
    );
    defenses.entries.clear();
    let foreign_king = PlayerUuid::from_u128(99);
    journal
        .gameplay_mut()
        .kingdoms
        .found(foreign_king, [80, 1, 80], "Other realm")
        .unwrap();
    defenses.provoke(NPC, foreign_king, journal.gameplay());
    assert!(
        defenses.contains(NPC),
        "another kingdom's founder is still an attacker"
    );
}

#[test]
fn real_retaliation_obeys_its_own_cooldown_and_repeated_hits_do_not_permanently_stun_it_or_lose_work_supplies()
 {
    let (chunks, mut journal, terrain, _king, owner) = fixture();
    let jobs = journal.gameplay().jobs.clone();
    let supplies = journal.gameplay().buildings.clone();
    let mut defenses = Defenses::default();
    let players = BTreeMap::from([(owner, player([8.5, 1.9, 6.5]))]);
    let mut hits = 0;
    for tick in 0..20 {
        if tick % 4 == 0 {
            defenses.provoke(NPC, owner, journal.gameplay());
        }
        let effects = defenses.advance(&chunks, &mut journal, &terrain, &players, 0.1);
        assert!(effects.len() <= 1);
        hits += effects.len();
        if tick < 4 {
            assert_eq!(hits, 0, "the initial reaction has a cooldown");
        }
    }
    assert_eq!(
        hits, 2,
        "melee must retain its 900ms cooldown while being attacked"
    );
    assert_eq!(journal.gameplay().jobs, jobs);
    assert_eq!(journal.gameplay().buildings, supplies);
    assert!(journal.gameplay().creatures.animals[&NPC].age >= 1.9);
}

#[test]
fn defensive_pursuit_physically_walks_and_ends_on_logout_death_or_excessive_distance() {
    for stop in 0..3 {
        let (chunks, mut journal, terrain, _king, owner) = fixture();
        let jobs = journal.gameplay().jobs.clone();
        let mut defenses = Defenses::default();
        defenses.provoke(NPC, owner, journal.gameplay());
        let mut players = BTreeMap::from([(owner, player([13.5, 1.9, 8.5]))]);
        let initial = journal.gameplay().creatures.animals[&NPC].position;
        for _ in 0..12 {
            let before = journal.gameplay().creatures.animals[&NPC].position;
            defenses.advance(&chunks, &mut journal, &terrain, &players, 0.1);
            let npc = &journal.gameplay().creatures.animals[&NPC];
            assert!(
                (npc.position[0] - before[0]).hypot(npc.position[2] - before[2]) <= 0.14,
                "no teleport"
            );
            assert!(
                Habitat {
                    chunks: &chunks,
                    journal: &journal,
                    terrain: &terrain
                }
                .clear_rotated(npc.position, npc.size(), false, f64::from(npc.yaw))
            );
        }
        assert!(distance(initial, journal.gameplay().creatures.animals[&NPC].position) > 1.0);
        match stop {
            0 => players.clear(),
            1 => players.get_mut(&owner).unwrap().vulnerable = false,
            _ => players.get_mut(&owner).unwrap().position = [100.0, 1.9, 8.5],
        }
        assert!(
            defenses
                .advance(&chunks, &mut journal, &terrain, &players, 0.1)
                .is_empty()
        );
        assert!(!defenses.contains(NPC));
        assert_eq!(journal.gameplay().jobs, jobs);
        assert_eq!(
            journal.gameplay().creatures.animals[&NPC].behavior,
            CreatureBehavior::Rest
        );
    }
}

#[test]
fn retaliation_cannot_hit_through_solid_or_unloaded_terrain_and_stops_an_infeasible_pursuit() {
    for unloaded in [false, true] {
        let (mut chunks, mut journal, terrain, _king, owner) = fixture();
        if unloaded {
            chunks.loaded.clear();
        } else {
            let data = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
            for x in 0..16 {
                for y in 1..5 {
                    data.set_block([x, y, 7], 3).unwrap();
                }
            }
        }
        let mut defenses = Defenses::default();
        defenses.provoke(NPC, owner, journal.gameplay());
        let players = BTreeMap::from([(owner, player([8.5, 1.9, 6.5]))]);
        for _ in 0..60 {
            assert!(
                defenses
                    .advance(&chunks, &mut journal, &terrain, &players, 0.1)
                    .is_empty()
            );
        }
        assert!(!defenses.contains(NPC));
    }
}

#[test]
fn protected_attacker_is_still_confronted_but_never_takes_damage_until_protection_expires() {
    let (chunks, mut journal, terrain, _king, owner) = fixture();
    let mut defenses = Defenses::default();
    defenses.provoke(NPC, owner, journal.gameplay());
    let mut target = player([8.5, 1.9, 6.5]);
    target.protected = true;
    let mut players = BTreeMap::from([(owner, target)]);
    for _ in 0..10 {
        assert!(
            defenses
                .advance(&chunks, &mut journal, &terrain, &players, 0.1)
                .is_empty()
        );
        assert!(defenses.contains(NPC));
    }
    players.get_mut(&owner).unwrap().protected = false;
    assert_eq!(
        defenses.advance(&chunks, &mut journal, &terrain, &players, 0.1),
        vec![(owner, 2.0)]
    );
}
