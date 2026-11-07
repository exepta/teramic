//! Loaded hearths share fixed processing/replication budgets, never terrain rebuilds.
use super::*;
use terra_core::{GameMode, ItemKind, brewing, campfire, effects::EffectKind};
use terra_world::hearths;

type Cell = [i64; 3];
const PROCESS_LIMIT: usize = 64;
const MAX_CATCHUP: f64 = 8.0;

#[derive(Default)]
pub(super) struct Schedule {
    elapsed: f64,
    next_send: f64,
    cursor: Option<Cell>,
    accounted: HashMap<Cell, f64>,
    weather: terra_world::weather::SurfaceWeatherProbe,
    sent: HashMap<PlayerId, (u32, Vec<CauldronVisual>, Vec<CampfireVisual>)>,
}

fn due(schedule: &mut Schedule, positions: &[Cell], now: f64, elapsed: f64) -> Vec<(Cell, f64)> {
    schedule
        .accounted
        .retain(|position, _| positions.binary_search(position).is_ok());
    let start = schedule.cursor.map_or(0, |cursor| {
        positions.partition_point(|position| *position <= cursor)
    }) % positions.len().max(1);
    let mut ready = Vec::with_capacity(positions.len().min(PROCESS_LIMIT));
    for offset in 0..positions.len().min(PROCESS_LIMIT) {
        let position = positions[(start + offset) % positions.len()];
        let previous = schedule
            .accounted
            .insert(position, now)
            .unwrap_or(now - elapsed);
        ready.push((position, (now - previous).clamp(0.0, MAX_CATCHUP)));
        schedule.cursor = Some(position);
    }
    ready
}

fn process(
    position: Cell,
    elapsed: f64,
    rain: bool,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
) -> Result<(), String> {
    if !elapsed.is_finite() || elapsed <= 0.0 {
        return Ok(());
    }
    if (chunks.block_at(position) == Some(campfire::MATERIAL)
        || journal.has_attached_campfire(position))
        && !hearths::loaded_fire(chunks, position)
    {
        return Ok(());
    }
    if chunks.block_at(position) == Some(campfire::MATERIAL)
        || journal.has_attached_campfire(position)
    {
        let mut inventory = journal
            .hearth_inventory_at(position)
            .cloned()
            .unwrap_or_default();
        let Some(mut state) = journal.hearth_at(position).cloned() else {
            return Ok(());
        };
        if !hearths::can_light(chunks, journal, position) || rain {
            if state.lit {
                state.lit = false;
                state.burn_remaining = 0.0;
                state.burn_total = 0.0;
            }
        } else {
            let mut remaining = elapsed.min(MAX_CATCHUP) as f32;
            for _ in 0..8 {
                if remaining <= 0.0 {
                    break;
                }
                campfire::advance(&mut state, &mut inventory, remaining.min(1.0));
                remaining -= 1.0;
            }
        }
        journal.commit_hearth(position, state, inventory)?;
    }
    if chunks.block_at(position) == Some(brewing::MATERIAL) {
        let mut inventory = journal.container_at(position).cloned().unwrap_or_default();
        let Some(mut state) = journal.cauldron_at(position).cloned() else {
            return Ok(());
        };
        let heated = hearths::heated_cauldron(chunks, journal, position);
        let mut remaining = elapsed.min(MAX_CATCHUP) as f32;
        for _ in 0..8 {
            if remaining <= 0.0 {
                break;
            }
            brewing::advance(&mut state, &mut inventory, heated, remaining.min(1.0));
            remaining -= 1.0;
        }
        journal.commit_cauldron(position, state, inventory)?;
    }
    Ok(())
}

fn visuals(
    position: [f64; 3],
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> (Vec<CauldronVisual>, Vec<CampfireVisual>) {
    let distance = |cell: Cell| -> f64 {
        (0..3)
            .map(|axis| (cell[axis] as f64 + 0.5 - position[axis]).powi(2))
            .sum()
    };
    let mut cauldrons: Vec<_> = journal
        .cauldrons()
        .filter(|(p, _)| {
            chunks.block_at(*p) == Some(brewing::MATERIAL) && distance(*p) <= 128.0_f64.powi(2)
        })
        .collect();
    cauldrons.sort_by(|a, b| distance(a.0).total_cmp(&distance(b.0)).then(a.0.cmp(&b.0)));
    let cauldrons = cauldrons
        .into_iter()
        .take(64)
        .map(|(position, state)| {
            let inventory = journal.container_at(position).cloned().unwrap_or_default();
            let view = state.view(
                hearths::heated_cauldron(chunks, journal, position),
                &inventory,
            );
            let spoon = inventory.slot(brewing::SPOON).and_then(|stack| {
                if let ItemKind::Tool(tool) = stack.item {
                    Some(tool)
                } else {
                    None
                }
            });
            CauldronVisual {
                position,
                water: state.water,
                brewing: view.working,
                heated: view.heated,
                recipe: state.recipe,
                spoon,
            }
        })
        .collect();
    let mut fires: Vec<_> = journal
        .hearths()
        .filter(|(p, _)| {
            (chunks.block_at(*p) == Some(campfire::MATERIAL) || journal.has_attached_campfire(*p))
                && distance(*p) <= 128.0_f64.powi(2)
        })
        .map(|(p, state)| CampfireVisual {
            position: p,
            lit: state.lit,
        })
        .collect();
    fires.sort_by(|a, b| {
        distance(a.position)
            .total_cmp(&distance(b.position))
            .then(a.position.cmp(&b.position))
    });
    fires.truncate(64);
    (cauldrons, fires)
}

pub(super) fn tick(
    mut world: ControlWorld,
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
    let elapsed = std::mem::take(&mut schedule.elapsed).min(MAX_CATCHUP);
    let now = time.elapsed_secs_f64();
    let journal = world.journal.bypass_change_detection();
    let mut positions: Vec<_> = journal
        .hearths()
        .map(|(p, _)| p)
        .chain(journal.cauldrons().map(|(p, _)| p))
        .filter(|p| match world.chunks.block_at(*p) {
            Some(brewing::MATERIAL) => {
                !journal.has_attached_campfire(*p) || hearths::loaded_fire(&world.chunks, *p)
            }
            Some(campfire::MATERIAL) => hearths::loaded_fire(&world.chunks, *p),
            _ => false,
        })
        .collect();
    positions.sort_unstable();
    positions.dedup();
    for (position, dt) in due(&mut schedule, &positions, now, elapsed) {
        let rain = world.chunks.block_at(position) == Some(campfire::MATERIAL)
            && schedule
                .weather
                .sample(
                    &world.weather,
                    &world.terrain.0,
                    &world.chunks,
                    position.map(|value| value as f64 + 0.5),
                    *world.clock,
                )
                .rain
                > 0.2
            && hearths::rain_exposed(&world.chunks, journal, position);
        if let Err(error) = process(position, dt, rain, &world.chunks, journal) {
            debug!(%error, ?position, "Hearth update deferred");
        }
    }
    let fires: Vec<_> = journal
        .hearths()
        .filter(|(p, state)| {
            state.lit
                && (world.chunks.block_at(*p) == Some(campfire::MATERIAL)
                    || journal.has_attached_campfire(*p))
        })
        .map(|(p, _)| p)
        .collect();
    let send = now >= schedule.next_send;
    if send {
        schedule.next_send = now + 0.25;
        schedule
            .sent
            .retain(|id, _| peers.iter().any(|(peer, _)| peer.synced && peer.id == *id));
    }
    for (mut peer, mut sender) in &mut peers {
        if !peer.synced {
            continue;
        }
        let Ok(body) = world.bodies.get(peer.body) else {
            continue;
        };
        let position = body.translation.to_array().map(f64::from);
        if peer.game_mode(&world.config) == GameMode::Survival
            && !peer.vitals.dead()
            && peer.chat.teleport.is_none()
        {
            let mut warmth = 0.0_f32;
            let mut burning = false;
            for &source in &fires {
                if (source[0] as f64 + 0.5 - position[0]).abs() > hearths::HEAT_RADIUS
                    || (source[2] as f64 + 0.5 - position[2]).abs() > hearths::HEAT_RADIUS
                {
                    continue;
                }
                warmth = warmth.max(hearths::heat_at(
                    &world.chunks,
                    journal,
                    &world.block_properties,
                    source,
                    position,
                ));
                burning |= hearths::contacts_hearth_flame(
                    journal,
                    source,
                    position,
                    [
                        f64::from(terra_physics::character::PLAYER_RADIUS),
                        f64::from(terra_physics::character::PLAYER_HALF_HEIGHT),
                        f64::from(terra_physics::character::PLAYER_RADIUS),
                    ],
                );
            }
            if warmth > 0.05 {
                peer.vitals
                    .effects
                    .apply(EffectKind::FreezeResistance, 0.75, 1);
            }
            if burning {
                peer.vitals.effects.apply(EffectKind::OnFire, 3.0, 1);
            }
        }
        if send {
            let (cauldrons, fires) = visuals(position, &world.chunks, journal);
            if !schedule
                .sent
                .get(&peer.id)
                .is_some_and(|(generation, old_cauldron, old_fire)| {
                    *generation == peer.generation
                        && old_cauldron == &cauldrons
                        && old_fire == &fires
                })
            {
                sender.send::<ControlChannel>(ServerControl::Cauldrons(cauldrons.clone()));
                sender.send::<ControlChannel>(ServerControl::Campfires(fires.clone()));
                schedule
                    .sent
                    .insert(peer.id, (peer.generation, cauldrons, fires));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{Inventory, ItemStack, ToolKind};
    use terra_voxel::{BlockSection, SectionPos, global_to_section};
    use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

    fn fixture() -> (ChunkStore, EditJournal, Cell, Cell) {
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            SectionPos::default(),
            LoadedSection {
                data: BlockSection::empty(SectionPos::default()).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut journal = EditJournal::default();
        let fire = [8, 4, 8];
        let kettle = [8, 5, 8];
        for (position, material) in [
            ([8, 3, 8], 3),
            (fire, campfire::MATERIAL),
            (kettle, brewing::MATERIAL),
        ] {
            let (owner, local) = global_to_section(position);
            chunks
                .loaded
                .get_mut(&owner)
                .unwrap()
                .data
                .set_block(local, material)
                .unwrap();
            journal.record(position, material).unwrap();
            if material != 3 {
                journal
                    .set_object_state(position, Some(BlockObjectState::default()))
                    .unwrap();
            }
        }
        journal.set_campfire(fire, Default::default()).unwrap();
        journal
            .set_cauldron(
                kettle,
                brewing::CauldronState {
                    recipe: Some(6),
                    ..Default::default()
                },
            )
            .unwrap();
        let mut fire_bag = Inventory {
            slots: vec![ItemStack::EMPTY; campfire::SLOTS],
            ..Default::default()
        };
        fire_bag.slots[campfire::INPUT] = ItemStack {
            item: campfire::recipes()[0].input,
            count: 1,
        };
        fire_bag.slots[campfire::FUEL] = ItemStack {
            item: ItemKind::Stick,
            count: 64,
        };
        journal.set_container(fire, fire_bag).unwrap();
        let mut kettle_bag = Inventory {
            slots: vec![ItemStack::EMPTY; brewing::SLOTS],
            ..Default::default()
        };
        for (slot, stack) in brewing::recipe(6).unwrap().inputs.iter().enumerate() {
            kettle_bag.slots[slot] = *stack;
        }
        kettle_bag.slots[brewing::WATER] = ItemStack {
            item: ItemKind::Tool(ToolKind::WaterBucket),
            count: 1,
        };
        kettle_bag.slots[brewing::BOTTLES] = ItemStack {
            item: ItemKind::GlassBottle,
            count: 1,
        };
        kettle_bag.slots[brewing::SPOON] = ItemStack {
            item: ItemKind::Tool(ToolKind::GoldSpoon),
            count: 1,
        };
        journal.set_container(kettle, kettle_bag).unwrap();
        (chunks, journal, fire, kettle)
    }

    #[test]
    fn real_loaded_hearths_cook_and_brew_without_touching_mesh_revisions() {
        let (chunks, mut journal, fire, kettle) = fixture();
        let section = global_to_section(fire).0;
        let epoch = journal.mesh_epoch(section);
        process(kettle, 0.5, false, &chunks, &mut journal).unwrap();
        assert_eq!(journal.cauldron_at(kettle).unwrap().water, 3);
        assert_eq!(journal.cauldron_at(kettle).unwrap().progress_seconds, 0.0);
        let mut state = journal.campfire_at(fire).unwrap().clone();
        let mut inventory = journal.container_at(fire).unwrap().clone();
        assert!(campfire::set_lit(&mut state, &mut inventory, true));
        journal.commit_campfire(fire, state, inventory).unwrap();
        for _ in 0..500 {
            process(fire, 0.1, false, &chunks, &mut journal).unwrap();
            process(kettle, 0.1, false, &chunks, &mut journal).unwrap();
        }
        assert_eq!(
            journal.container_at(kettle).unwrap().slot(brewing::OUTPUT),
            Some(brewing::recipe(6).unwrap().output())
        );
        assert_eq!(journal.cauldron_at(kettle).unwrap().water, 2);
        assert_eq!(
            journal.container_at(fire).unwrap().slot(campfire::OUTPUT),
            Some(campfire::recipes()[0].output)
        );
        assert_eq!(journal.mesh_epoch(section), epoch);
        let (cauldrons, fires) = visuals([8.5, 5.0, 10.0], &chunks, &journal);
        assert_eq!(cauldrons.len(), 1);
        assert_eq!(fires.len(), 1);
        assert!(cauldrons.iter().all(CauldronVisual::valid));
        assert!(fires.iter().all(CampfireVisual::valid));
    }

    #[test]
    fn one_cell_compound_burns_cooks_and_heats_its_separate_kettle_without_public_mesh_churn() {
        let (mut chunks, mut journal, host, old_kettle) = fixture();
        let mut fire_state = journal.campfire_at(host).unwrap().clone();
        let mut fire_inventory = journal.container_at(host).unwrap().clone();
        assert!(campfire::set_lit(
            &mut fire_state,
            &mut fire_inventory,
            true
        ));
        let kettle_inventory = journal.container_at(old_kettle).unwrap().clone();
        let (section, local) = global_to_section(host);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, brewing::MATERIAL)
            .unwrap();
        journal.record(host, brewing::MATERIAL).unwrap();
        journal
            .set_object_state(
                host,
                Some(BlockObjectState {
                    facing: brewing::ATTACHED_FIRE,
                    ..Default::default()
                }),
            )
            .unwrap();
        journal
            .commit_cauldron(
                host,
                brewing::CauldronState {
                    recipe: Some(6),
                    ..Default::default()
                },
                kettle_inventory,
            )
            .unwrap();
        journal
            .commit_attached_campfire(
                host,
                campfire::AttachedCampfire {
                    state: fire_state,
                    inventory: fire_inventory,
                },
            )
            .unwrap();
        assert!(hearths::heated_cauldron(&chunks, &journal, host));
        let epoch = journal.mesh_epoch(section);
        let delta = journal.section_delta(section);
        for _ in 0..500 {
            process(host, 0.1, false, &chunks, &mut journal).unwrap();
        }
        assert_eq!(
            journal.container_at(host).unwrap().slot(brewing::OUTPUT),
            Some(brewing::recipe(6).unwrap().output())
        );
        let fire = journal.attached_campfire_at(host).unwrap();
        assert_eq!(
            fire.inventory.slot(campfire::OUTPUT),
            Some(campfire::recipes()[0].output)
        );
        assert!(fire.inventory.slot(campfire::FUEL).unwrap().count < 63);
        assert!(fire.state.lit);
        assert_eq!(journal.mesh_epoch(section), epoch);
        assert_eq!(journal.section_delta(section), delta);
        let (cauldrons, fires) = visuals(host.map(|value| value as f64 + 0.5), &chunks, &journal);
        assert!(
            cauldrons
                .iter()
                .any(|view| view.position == host && view.heated && view.valid())
        );
        assert!(
            fires
                .iter()
                .any(|view| view.position == host && view.lit && view.valid())
        );
        let paused = journal.attached_campfire_at(host).unwrap().clone();
        let removed = chunks.loaded.remove(&section).unwrap();
        process(host, 8.0, false, &chunks, &mut journal).unwrap();
        assert_eq!(journal.attached_campfire_at(host), Some(&paused));
        chunks.loaded.insert(section, removed);
        process(host, 0.1, false, &chunks, &mut journal).unwrap();
        let resumed = journal.attached_campfire_at(host).unwrap();
        let fuel_seconds = f64::from(
            terra_core::furnace::fuel_seconds(paused.inventory.slot(campfire::FUEL).unwrap().item)
                .unwrap(),
        );
        let energy = |fire: &campfire::AttachedCampfire| {
            f64::from(fire.state.burn_remaining)
                + f64::from(
                    fire.inventory
                        .slot(campfire::FUEL)
                        .map_or(0, |fuel| fuel.count),
                ) * fuel_seconds
        };
        // A refill raises the current timer; total queued plus burning fuel must still decrease.
        let consumed = energy(&paused) - energy(resumed);
        assert!(
            (consumed - 0.1).abs() < 0.00001,
            "resumed fuel seconds consumed: {consumed}"
        );
        assert!(resumed.state.lit && resumed.valid());
        assert_eq!(resumed.state.completed, paused.state.completed);
        assert_eq!(
            resumed.inventory.slot(campfire::INPUT),
            paused.inventory.slot(campfire::INPUT)
        );
        assert_eq!(
            resumed.inventory.slot(campfire::OUTPUT),
            paused.inventory.slot(campfire::OUTPUT)
        );
    }

    #[test]
    fn rain_and_removed_support_extinguish_without_refunding_spent_fuel() {
        for rain in [false, true] {
            let (mut chunks, mut journal, fire, _) = fixture();
            let mut state = journal.campfire_at(fire).unwrap().clone();
            let mut inventory = journal.container_at(fire).unwrap().clone();
            assert!(campfire::set_lit(&mut state, &mut inventory, true));
            let fuel = inventory.slot(campfire::FUEL);
            journal.commit_campfire(fire, state, inventory).unwrap();
            if !rain {
                chunks
                    .loaded
                    .get_mut(&SectionPos::default())
                    .unwrap()
                    .data
                    .set_block([8, 3, 8], 0)
                    .unwrap();
            }
            process(fire, 0.1, rain, &chunks, &mut journal).unwrap();
            assert!(!journal.campfire_at(fire).unwrap().lit);
            assert_eq!(
                journal.container_at(fire).unwrap().slot(campfire::FUEL),
                fuel
            );
        }
    }

    #[test]
    fn a_missing_neighbor_section_pauses_a_saved_fire_instead_of_extinguishing_it() {
        let (mut chunks, mut journal, _, _) = fixture();
        let fire = [15, 4, 8];
        let neighboring = SectionPos { x: 1, y: 0, z: 0 };
        let neighbor = LoadedSection {
            data: BlockSection::empty(neighboring).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        };
        chunks.loaded.insert(neighboring, neighbor);
        for (position, material) in [([15, 3, 8], 3), (fire, campfire::MATERIAL)] {
            let (owner, local) = global_to_section(position);
            chunks
                .loaded
                .get_mut(&owner)
                .unwrap()
                .data
                .set_block(local, material)
                .unwrap();
            journal.record(position, material).unwrap();
        }
        journal
            .set_object_state(fire, Some(BlockObjectState::default()))
            .unwrap();
        let state = campfire::CampfireState {
            lit: true,
            burn_remaining: 15.0,
            burn_total: 20.0,
            ..Default::default()
        };
        journal.set_campfire(fire, state.clone()).unwrap();
        assert!(hearths::can_light(&chunks, &journal, fire));
        let neighbor = chunks.loaded.remove(&neighboring).unwrap();
        let inventory = journal.container_at(fire).cloned();
        let revision = journal.gameplay().revision;
        process(fire, 8.0, false, &chunks, &mut journal).unwrap();
        assert_eq!(journal.campfire_at(fire), Some(&state));
        assert_eq!(journal.container_at(fire), inventory.as_ref());
        assert_eq!(journal.gameplay().revision, revision);
        chunks.loaded.insert(neighboring, neighbor);
        process(fire, 0.25, false, &chunks, &mut journal).unwrap();
        let resumed = journal.campfire_at(fire).unwrap();
        assert!(resumed.lit);
        assert_eq!(resumed.burn_remaining, 14.75);
    }

    #[test]
    fn fair_jobs_and_unloaded_accounting_are_bounded_and_do_not_catch_up_offline_time() {
        let positions: Vec<_> = (0..130).map(|x| [x, 4, 8]).collect();
        let mut schedule = Schedule::default();
        let mut seen = std::collections::BTreeSet::new();
        for step in 0..3 {
            let ready = due(&mut schedule, &positions, 1.0 + step as f64 * 0.1, 0.1);
            assert_eq!(ready.len(), PROCESS_LIMIT);
            for (position, elapsed) in ready {
                seen.insert(position);
                assert!(elapsed <= 0.31);
            }
        }
        assert_eq!(seen.len(), positions.len());
        assert!(due(&mut schedule, &[], 50.0, 0.1).is_empty());
        assert!(schedule.accounted.is_empty());
        for (_, dt) in due(&mut schedule, &positions, 1000.0, 0.1) {
            assert!((dt - 0.1).abs() < 0.00001);
        }
    }
}
