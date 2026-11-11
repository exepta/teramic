//! Loaded furnaces share a bounded fair scheduler and one non-stacking smoke field.
use super::*;
use terra_core::{
    GameMode, Inventory,
    furnace::{self, FurnaceState},
};

type Cell = [i64; 3];
const PROCESS_LIMIT: usize = 64;
const MAX_CATCHUP: f64 = 8.0;
const VISUAL_RADIUS: f64 = 192.0;

#[derive(Default)]
pub(super) struct Schedule {
    elapsed: f64,
    next_send: f64,
    cursor: Option<Cell>,
    accounted: HashMap<Cell, f64>,
    smoke_pending: HashMap<Cell, SmokePending>,
    smoke: SmokeField,
    sent: HashMap<PlayerId, (u32, Vec<FurnaceVisual>)>,
}

#[derive(Default)]
struct SmokePending {
    burning: f32,
    cooling: f32,
}

#[derive(Default)]
struct SmokeField {
    sources: HashMap<Cell, Vec<Cell>>,
    cells: HashMap<Cell, usize>,
}
impl SmokeField {
    fn remove(&mut self, source: Cell) {
        if let Some(previous) = self.sources.remove(&source) {
            for cell in previous {
                if let Some(count) = self.cells.get_mut(&cell) {
                    *count -= 1;
                    if *count == 0 {
                        self.cells.remove(&cell);
                    }
                }
            }
        }
    }

    fn set(&mut self, source: Cell, cells: &[Cell]) {
        if self.sources.get(&source).is_some_and(|old| old == cells) {
            return;
        }
        self.remove(source);
        if !cells.is_empty() {
            for cell in cells {
                *self.cells.entry(*cell).or_default() += 1;
            }
            self.sources.insert(source, cells.to_vec());
        }
    }

    fn intersects(&self, center: [f64; 3], half: [f64; 3]) -> bool {
        if self.cells.is_empty()
            || !center.iter().all(|v| v.is_finite())
            || !half
                .iter()
                .all(|v| v.is_finite() && (0.0..=16.0).contains(v))
        {
            return false;
        }
        let low: Cell = std::array::from_fn(|axis| (center[axis] - half[axis]).floor() as i64);
        let high: Cell =
            std::array::from_fn(|axis| (center[axis] + half[axis] - 0.000001).floor() as i64);
        for x in low[0]..=high[0] {
            for y in low[1]..=high[1] {
                for z in low[2]..=high[2] {
                    if self.cells.contains_key(&[x, y, z]) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

pub(super) fn loaded(anchor: Cell, chunks: &ChunkStore, journal: &EditJournal) -> bool {
    let upper = [anchor[0], anchor[1] + 1, anchor[2]];
    chunks.block_at(anchor) == Some(59)
        && chunks.block_at(upper) == Some(59)
        && journal.object_state(anchor).is_some_and(|state| {
            !state.secondary
                && state.peer == Some(upper)
                && journal.object_state(upper).is_some_and(|other| {
                    other.secondary && other.peer == Some(anchor) && other.facing == state.facing
                })
        })
}

fn active(state: &FurnaceState, inventory: Option<&Inventory>) -> bool {
    state.burn_remaining > 0.0
        || !state.smoke_cells.is_empty()
        || inventory.is_some_and(|inventory| {
            inventory
                .slot(furnace::INPUT)
                .is_some_and(|stack| furnace::recipe_for(stack.item).is_some())
                && inventory
                    .slot(furnace::FUEL)
                    .is_some_and(|stack| furnace::fuel_seconds(stack.item).is_some())
        })
}

fn position_seed(seed: u64, position: Cell) -> u64 {
    position.into_iter().fold(seed, |value, axis| {
        value.rotate_left(21) ^ (axis as u64).wrapping_mul(0x9e3779b97f4a7c15)
    })
}

#[cfg(test)]
fn process(
    position: Cell,
    elapsed: f64,
    seed: u64,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
) -> Result<(), String> {
    process_with_smoke(
        position,
        elapsed,
        seed,
        chunks,
        journal,
        &mut SmokePending::default(),
    )
}

fn process_with_smoke(
    position: Cell,
    elapsed: f64,
    seed: u64,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    smoke_pending: &mut SmokePending,
) -> Result<(), String> {
    if !loaded(position, chunks, journal)
        || !elapsed.is_finite()
        || elapsed <= 0.0
        || journal.gameplay().revision > u64::MAX - 2
    {
        return Ok(());
    }
    let Some(mut state) = journal.furnace_at(position).cloned() else {
        return Ok(());
    };
    let mut inventory = journal.container_at(position).cloned().unwrap_or_default();
    let before_state = state.clone();
    let before_inventory = inventory.clone();
    let elapsed = elapsed.min(MAX_CATCHUP);
    let mut remaining = elapsed;
    let mut burning_total = 0.0;
    while remaining > 0.000001 {
        let dt = remaining.min(1.0) as f32;
        let before_burn = state.burn_remaining;
        let before_fuel = inventory.slot(furnace::FUEL);
        furnace::advance(
            &mut state,
            &mut inventory,
            dt,
            position_seed(seed, position),
        );
        let spent_fuel = before_fuel.map_or(0.0, |before| {
            let count = inventory.slot(furnace::FUEL).map_or(0, |after| after.count);
            f32::from(before.count.saturating_sub(count))
                * furnace::fuel_seconds(before.item).unwrap_or(0.0)
        });
        let burned = (before_burn + spent_fuel - state.burn_remaining).clamp(0.0, dt);
        let burning = if burned + 0.0001 >= dt { dt } else { burned };
        burning_total += burning;
        remaining -= f64::from(dt);
    }
    if state.outlet == [0; 3] {
        state.outlet = [position[0], position[1] + 2, position[2]];
    }
    smoke_pending.burning += burning_total;
    smoke_pending.cooling += elapsed as f32 - burning_total;
    // Cooking/UI progresses at 10 Hz; bounded room discovery runs at most 1 Hz.
    let smoke_due = smoke_pending.burning + smoke_pending.cooling >= 0.99999;
    if smoke_due && smoke_pending.burning > 0.0 {
        terra_world::furnaces::update_smoke(
            position,
            &mut state,
            true,
            smoke_pending.burning,
            chunks,
            journal,
        );
    }
    if smoke_due && smoke_pending.cooling > 0.0 {
        terra_world::furnaces::update_smoke(
            position,
            &mut state,
            false,
            smoke_pending.cooling,
            chunks,
            journal,
        );
    }
    if smoke_due {
        *smoke_pending = SmokePending::default();
    }
    if !state.valid() || !journal.valid_container(position, &inventory) {
        return Err("Invalid furnace result".into());
    }
    if inventory != before_inventory {
        journal.set_container(position, inventory)?;
    }
    if state != before_state {
        journal.set_furnace(position, state)?;
    }
    Ok(())
}

fn due(schedule: &mut Schedule, positions: &[Cell], now: f64, elapsed: f64) -> Vec<(Cell, f64)> {
    schedule
        .accounted
        .retain(|position, _| positions.binary_search(position).is_ok());
    schedule
        .smoke_pending
        .retain(|position, _| positions.binary_search(position).is_ok());
    for position in positions {
        schedule.accounted.entry(*position).or_insert(now - elapsed);
    }
    let removed: Vec<_> = schedule
        .smoke
        .sources
        .keys()
        .filter(|position| positions.binary_search(position).is_err())
        .copied()
        .collect();
    for position in removed {
        schedule.smoke.remove(position);
    }
    let start = schedule
        .cursor
        .map_or(0, |cursor| positions.partition_point(|p| *p <= cursor));
    let selected: Vec<_> = positions
        .iter()
        .cycle()
        .skip(start)
        .take(positions.len().min(PROCESS_LIMIT))
        .map(|&position| {
            let accounted = schedule.accounted.entry(position).or_insert(now);
            let elapsed = (now - *accounted).clamp(0.0, MAX_CATCHUP);
            *accounted += elapsed;
            (position, elapsed)
        })
        .collect();
    if let Some((position, _)) = selected.last() {
        schedule.cursor = Some(*position);
    }
    selected
}

fn visuals(position: [f64; 3], chunks: &ChunkStore, journal: &EditJournal) -> Vec<FurnaceVisual> {
    let distance = |cell: Cell| {
        (0..3)
            .map(|axis| (cell[axis] as f64 + 0.5 - position[axis]).powi(2))
            .sum::<f64>()
    };
    let mut nearby: Vec<_> = journal
        .furnaces()
        .filter(|(anchor, state)| {
            (state.burn_remaining > 0.0 || !state.smoke_cells.is_empty())
                && loaded(*anchor, chunks, journal)
        })
        .filter_map(|(anchor, state)| {
            let nearest = state
                .smoke_cells
                .iter()
                .copied()
                .map(distance)
                .fold(distance(anchor), f64::min);
            (nearest <= VISUAL_RADIUS.powi(2)).then_some((nearest, anchor, state))
        })
        .collect();
    nearby.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    nearby.truncate(PROCESS_LIMIT);
    let mut cells = Vec::new();
    for (source, (_, _, state)) in nearby.iter().enumerate() {
        for &cell in &state.smoke_cells {
            let distance = distance(cell);
            if distance <= VISUAL_RADIUS.powi(2) {
                cells.push((distance, cell, source));
            }
        }
    }
    cells.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    // Shared cells consume one network slot, owned by the nearest selected source.
    cells.dedup_by(|a, b| a.1 == b.1);
    cells.truncate(furnace::MAX_SMOKE_CELLS);
    let mut snapshots: Vec<_> = nearby
        .into_iter()
        .map(|(_, position, state)| FurnaceVisual {
            position,
            lit: state.burn_remaining > 0.0,
            outlet: state.outlet,
            smoke_cells: Vec::new(),
        })
        .collect();
    for (_, cell, source) in cells {
        snapshots[source].smoke_cells.push(cell);
    }
    snapshots
}

fn hurt_player(peer: &mut Peer, elapsed: f64, mode: GameMode) -> bool {
    let previous = peer.survival.damage_epoch;
    survival::environmental_damage(
        peer,
        (elapsed * 2.0) as f32,
        mode,
        terra_core::DamageCause::Smoke,
    );
    peer.survival.damage_epoch != previous
}

pub(super) fn tick(
    world: ControlWorld,
    time: Res<Time>,
    mut schedule: Local<Schedule>,
    mut peers: Query<(&mut Peer, &mut MessageSender<ServerControl>), With<Connected>>,
) {
    if !peers.iter().any(|(peer, _)| peer.synced) {
        *schedule = Schedule::default();
        return;
    }
    schedule.elapsed += f64::from(time.delta_secs());
    if schedule.elapsed < 0.1 {
        return;
    }
    let elapsed = std::mem::take(&mut schedule.elapsed);
    let now = time.elapsed_secs_f64();
    let ControlWorld {
        config,
        chunks,
        mut journal,
        bodies,
        ..
    } = world;
    // Furnace inventories, vitals and smoke are metadata, never terrain mesh edits.
    let journal = journal.bypass_change_detection();
    let mut positions: Vec<_> = journal
        .furnaces()
        .filter(|(position, state)| {
            active(state, journal.container_at(*position)) && loaded(*position, &chunks, journal)
        })
        .map(|(position, _)| position)
        .collect();
    positions.sort_unstable();
    for (position, dt) in due(&mut schedule, &positions, now, elapsed) {
        if let Err(error) = process_with_smoke(
            position,
            dt,
            config.world.seed.0,
            &chunks,
            journal,
            schedule.smoke_pending.entry(position).or_default(),
        ) {
            tracing::warn!(%error, ?position, "Furnace could not be advanced");
        }
        if let Some(state) = journal.furnace_at(position) {
            schedule.smoke.set(position, &state.smoke_cells);
        }
    }
    let send = now >= schedule.next_send;
    if send {
        schedule.next_send += (((now - schedule.next_send) / 0.25).floor() + 1.0) * 0.25;
        schedule
            .sent
            .retain(|id, _| peers.iter().any(|(peer, _)| peer.synced && peer.id == *id));
    }
    for (mut peer, mut sender) in &mut peers {
        if !peer.synced {
            continue;
        }
        let Ok(body) = bodies.get(peer.body) else {
            continue;
        };
        let position = body.translation.to_array().map(f64::from);
        let exposed = !peer.vitals.dead()
            && peer.chat.teleport.is_none()
            && chunks
                .block_at(position.map(|axis| axis.floor() as i64))
                .is_some()
            && schedule.smoke.intersects(
                position,
                [
                    f64::from(terra_physics::character::PLAYER_RADIUS),
                    f64::from(terra_physics::character::PLAYER_HALF_HEIGHT),
                    f64::from(terra_physics::character::PLAYER_RADIUS),
                ],
            );
        let mode = peer.game_mode(&config);
        peer.vitals
            .effects
            .set_smoke_exposure(exposed && mode == GameMode::Survival);
        if exposed && hurt_player(&mut peer, elapsed, mode) {
            consumption::cancel(&mut peer, &mut sender);
        }
        if send {
            let visible = visuals(position, &chunks, journal);
            if !schedule
                .sent
                .get(&peer.id)
                .is_some_and(|(generation, previous)| {
                    *generation == peer.generation && *previous == visible
                })
            {
                sender.send::<ControlChannel>(ServerControl::Furnaces(visible.clone()));
                schedule.sent.insert(peer.id, (peer.generation, visible));
            }
        }
    }
    if schedule.smoke.cells.is_empty() {
        return;
    }
    let affected: Vec<_> = journal
        .gameplay()
        .creatures
        .animals
        .values()
        .filter(|animal| {
            let size = animal.size();
            animal.health > 0.0
                && chunks
                    .block_at(animal.position.map(|v| v.floor() as i64))
                    .is_some()
                && schedule.smoke.intersects(
                    [
                        animal.position[0],
                        animal.position[1] + size[1] * 0.5,
                        animal.position[2],
                    ],
                    size.map(|axis| axis * 0.5),
                )
        })
        .map(|animal| animal.id)
        .collect();
    if !affected.is_empty() {
        let Some(revision) = journal.gameplay().revision.checked_add(1) else {
            return;
        };
        journal.gameplay_mut().revision = revision;
    }
    for id in affected {
        let mut drop = None;
        if let Some(animal) = journal.gameplay_mut().creatures.animals.get_mut(&id) {
            animal.health = (animal.health - (elapsed * 2.0) as f32).max(0.0);
            if animal.health == 0.0 {
                animal.behavior = terra_core::creatures::CreatureBehavior::Dead;
                animal.timer = 5.0;
                drop = animal.product().map(|product| (animal.position, product));
            }
        }
        if let Some((position, product)) = drop {
            // One death transition, using the existing world-drop capacity policy.
            journal
                .gameplay_mut()
                .drop_one(position, terra_core::ItemKind::Animal(product));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn fixture() -> (Cell, ChunkStore, EditJournal) {
        use terra_voxel::{BlockSection, SectionPos};
        use terra_world::{edits::BlockObjectState, streaming::LoadedSection};
        let section = SectionPos::default();
        let mut data = BlockSection::empty(section).unwrap();
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    if !(1..=7).contains(&x) || !(1..=4).contains(&y) || !(1..=7).contains(&z) {
                        data.set_block([x, y, z], 3).unwrap();
                    }
                }
            }
        }
        let position = [4, 1, 4];
        let upper = [4, 2, 4];
        let mut journal = EditJournal::default();
        for (cell, peer, secondary) in [(position, upper, false), (upper, position, true)] {
            data.set_block(cell.map(|axis| axis as usize), 59).unwrap();
            journal.record(cell, 59).unwrap();
            journal
                .set_object_state(
                    cell,
                    Some(BlockObjectState {
                        peer: Some(peer),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        journal
            .set_furnace(position, FurnaceState::default())
            .unwrap();
        journal
            .set_container(
                position,
                Inventory {
                    slots: vec![
                        terra_core::ItemStack {
                            item: terra_core::ItemKind::Animal(
                                terra_core::creatures::AnimalProduct::Fish,
                            ),
                            count: 3,
                        },
                        terra_core::ItemStack {
                            item: terra_core::ItemKind::Mineral(terra_core::MineralItem::Coal),
                            count: 2,
                        },
                        terra_core::ItemStack::EMPTY,
                        terra_core::ItemStack::EMPTY,
                    ],
                    ..Default::default()
                },
            )
            .unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        (position, chunks, journal)
    }

    #[test]
    fn smoke_probes_are_decoupled_from_the_ten_hz_cooking_updates() {
        let (position, chunks, mut journal) = fixture();
        let mut pending = SmokePending::default();
        for _ in 0..9 {
            process_with_smoke(position, 0.1, 7, &chunks, &mut journal, &mut pending).unwrap();
            assert!(journal.furnace_at(position).unwrap().smoke_cells.is_empty());
        }
        assert!(journal.furnace_at(position).unwrap().progress_seconds > 0.8);
        process_with_smoke(position, 0.1, 7, &chunks, &mut journal, &mut pending).unwrap();
        assert_eq!(journal.furnace_at(position).unwrap().smoke_cells.len(), 1);
        assert_eq!(pending.burning, 0.0);
        assert_eq!(pending.cooling, 0.0);
    }

    #[test]
    fn server_furnace_commits_food_and_smoke_without_terrain_edits_and_pauses_unloaded_halves() {
        let (position, mut chunks, mut journal) = fixture();
        let section = terra_voxel::global_to_section(position).0;
        let terrain = journal.section_delta(section);
        let epoch = journal.mesh_epoch(section);
        process(position, 3.0, 7, &chunks, &mut journal).unwrap();
        assert_eq!(journal.furnace_at(position).unwrap().smoke_cells.len(), 3);
        assert_eq!(
            journal
                .container_at(position)
                .unwrap()
                .slot(furnace::FUEL)
                .unwrap()
                .count,
            1
        );
        assert!(
            journal
                .container_at(position)
                .unwrap()
                .slot(furnace::OUTPUT)
                .is_none()
        );
        let previous = (
            journal.furnace_at(position).cloned(),
            journal.container_at(position).cloned(),
        );
        let loaded = chunks.loaded.remove(&section).unwrap();
        process(position, 8.0, 7, &chunks, &mut journal).unwrap();
        assert_eq!(
            previous,
            (
                journal.furnace_at(position).cloned(),
                journal.container_at(position).cloned()
            )
        );
        chunks.loaded.insert(section, loaded);
        process(position, 3.0, 7, &chunks, &mut journal).unwrap();
        assert_eq!(journal.furnace_at(position).unwrap().completed, 1);
        assert_eq!(journal.furnace_at(position).unwrap().smoke_cells.len(), 6);
        assert_eq!(
            journal
                .container_at(position)
                .unwrap()
                .slot(furnace::INPUT)
                .unwrap()
                .count,
            2
        );
        assert_eq!(
            journal
                .container_at(position)
                .unwrap()
                .slot(furnace::OUTPUT)
                .unwrap()
                .count,
            1
        );
        assert_eq!(journal.section_delta(section), terrain);
        assert_eq!(journal.mesh_epoch(section), epoch);
        let visual = visuals([4.5, 2.0, 4.5], &chunks, &journal);
        assert_eq!(visual.len(), 1);
        assert_eq!(visual[0].position, position);
        assert_eq!(visual[0].outlet, [4, 3, 4]);
        assert_eq!(visual[0].smoke_cells.len(), 6);
        assert!(visual[0].lit && visual[0].valid());
        assert!(visuals([500.0; 3], &chunks, &journal).is_empty());
    }

    #[test]
    fn furnace_visuals_select_globally_nearest_unique_smoke_across_stations() {
        use terra_world::edits::BlockObjectState;
        let (first, mut chunks, mut journal) = fixture();
        let second = [6, 1, 6];
        let upper = [6, 2, 6];
        for (position, peer, secondary) in [(second, upper, false), (upper, second, true)] {
            let (section, local) = terra_voxel::global_to_section(position);
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block(local, 59)
                .unwrap();
            journal.record(position, 59).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        peer: Some(peer),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        let near: Vec<_> = (0..200)
            .map(|index| [index % 10, (index / 10) % 4 + 1, index / 40])
            .collect();
        let far: Vec<_> = (0..200)
            .map(|index| [40 + index % 10, 3 + index / 10, 0])
            .collect();
        journal
            .set_furnace(
                first,
                FurnaceState {
                    smoke_cells: far.iter().chain(&near[..20]).copied().collect(),
                    ..Default::default()
                },
            )
            .unwrap();
        journal
            .set_furnace(
                second,
                FurnaceState {
                    smoke_cells: near.clone(),
                    ..Default::default()
                },
            )
            .unwrap();
        let snapshots = visuals([4.5, 1.5, 4.5], &chunks, &journal);
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].position, first);
        let sent: Vec<_> = snapshots
            .iter()
            .flat_map(|snapshot| snapshot.smoke_cells.iter().copied())
            .collect();
        let unique: HashSet<_> = sent.iter().copied().collect();
        assert_eq!(sent.len(), furnace::MAX_SMOKE_CELLS);
        assert_eq!(unique.len(), sent.len());
        assert!(near.iter().all(|cell| unique.contains(cell)));
        assert_eq!(far.iter().filter(|cell| unique.contains(*cell)).count(), 56);
        assert!(snapshots.iter().all(FurnaceVisual::valid));
    }

    #[test]
    fn nearby_smoke_is_visible_even_when_its_loaded_furnace_is_outside_anchor_radius() {
        let (anchor, chunks, mut journal) = fixture();
        let cell = [400, 1, 4];
        journal
            .set_furnace(
                anchor,
                FurnaceState {
                    smoke_cells: vec![cell],
                    ..Default::default()
                },
            )
            .unwrap();
        let snapshots = visuals([400.5, 1.5, 4.5], &chunks, &journal);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].position, anchor);
        assert_eq!(snapshots[0].smoke_cells, vec![cell]);
        assert!(snapshots[0].valid());
    }

    #[test]
    fn smoke_damage_is_two_per_second_and_uses_survival_damage_and_death_rules() {
        let now = Instant::now();
        let mut peer = Peer {
            farming: Default::default(),
            crank: Default::default(),
            chest_cursor: None,
            creature_action_at: now,
            container: None,
            held_slot: None,
            mode: Some(GameMode::Survival),
            generation: 0,
            chat: Default::default(),
            uuid: terra_core::PlayerUuid::new_v4(),
            gender: Default::default(),
            appearance: Default::default(),
            mining: None,
            consumption: None,
            gameplay_sent: None,
            permissions: terra_core::PlayerPermissions {
                game_mode: false,
                build: true,
                flight: false,
                environment: false,
            },
            id: PlayerId(1),
            transport_id: 1,
            body: Entity::PLACEHOLDER,
            accepted: true,
            synced: true,
            view: 1,
            known: Default::default(),
            inputs: Default::default(),
            received: 0,
            ack: 0,
            look: [0.0; 2],
            last_input: now,
            joined: now,
            last_request: 0,
            edits_left: 20.0,
            rate_time: now,
            spawned: true,
            restored_spawn: false,
            restore_location: None,
            last_movement: Default::default(),
            vitals: Default::default(),
            equipment: Default::default(),
            survival: Default::default(),
            stats_sent: None,
        };
        peer.vitals.effects.set_smoke_exposure(true);
        for _ in 0..10 {
            assert!(hurt_player(&mut peer, 0.1, GameMode::Survival));
            peer.vitals.advance_life(false, 0.1);
        }
        assert!((peer.vitals.health - 98.0).abs() < 0.0001);
        assert_eq!(peer.survival.damage_epoch, 10);
        assert_eq!(
            peer.vitals.last_damage,
            Some(terra_core::DamageCause::Smoke)
        );
        assert!(peer.vitals.effects.contains(terra_core::EffectKind::Poison));
        assert!(
            peer.vitals
                .effects
                .timed(terra_core::EffectKind::Poison)
                .is_none()
        );
        peer.vitals.effects.set_smoke_exposure(false);
        assert!(!peer.vitals.effects.contains(terra_core::EffectKind::Poison));
        for mode in [GameMode::Creative, GameMode::Spectator] {
            let health = peer.vitals.health;
            assert!(!hurt_player(&mut peer, 1.0, mode));
            assert_eq!(peer.vitals.health, health);
        }
        peer.vitals.health = 1.0;
        assert!(hurt_player(&mut peer, 1.0, GameMode::Survival));
        assert!(peer.vitals.dead());
        assert!(!hurt_player(&mut peer, 1.0, GameMode::Survival));
    }

    #[test]
    fn smoke_fields_cover_whole_bodies_and_overlapping_sources_never_stack_damage() {
        let mut field = SmokeField::default();
        field.set([0; 3], &[[1, 2, 3], [-1, 0, -1]]);
        field.set([5; 3], &[[1, 2, 3]]);
        assert!(field.intersects([0.9, 2.9, 3.4], [0.32, 0.9, 0.32]));
        assert!(field.intersects([-0.1, 0.5, -0.1], [0.1; 3]));
        assert!(!field.intersects([2.5, 2.5, 3.5], [0.5; 3]));
        field.remove([0; 3]);
        assert!(field.intersects([1.5, 2.5, 3.5], [0.1; 3]));
        assert!(!field.intersects([-0.1, 0.5, -0.1], [0.1; 3]));
        field.remove([5; 3]);
        assert!(field.cells.is_empty() && field.sources.is_empty());
    }

    #[test]
    fn furnace_round_robin_preserves_elapsed_time_above_the_per_pass_limit() {
        let positions: Vec<_> = (0..160).map(|x| [x, 0, 0]).collect();
        let mut schedule = Schedule::default();
        let mut total = HashMap::<_, f64>::new();
        for tick in 1..=60 {
            let work = due(&mut schedule, &positions, f64::from(tick) * 0.1, 0.1);
            assert_eq!(work.len(), PROCESS_LIMIT);
            assert_eq!(
                work.iter().map(|(p, _)| p).collect::<HashSet<_>>().len(),
                PROCESS_LIMIT
            );
            for (position, dt) in work {
                *total.entry(position).or_default() += dt;
            }
        }
        assert_eq!(total.len(), positions.len());
        assert!(total.values().all(|dt| (5.7..=6.00001).contains(dt)));
        let removed = positions[0];
        schedule.smoke.set(removed, &[[0, 2, 0]]);
        due(&mut schedule, &positions[1..], 6.1, 0.1);
        assert!(!schedule.accounted.contains_key(&removed));
        assert!(schedule.smoke.cells.is_empty());
        let fresh = due(&mut schedule, &[removed], 20.0, 0.1);
        assert!((fresh[0].1 - 0.1).abs() < 0.00001);
    }
}
