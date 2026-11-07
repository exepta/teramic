//! Mechanical stations advance only while their complete saved footprint is loaded.
use super::*;
use terra_core::{Inventory, crusher};
type Cell = [i64; 3];
const PROCESS_LIMIT: usize = 32;
const DRIVE_LEASE: Duration = Duration::from_millis(350);

#[derive(Default)]
pub(super) struct CrankPeer {
    lease: Option<Lease>,
    last_intent: Option<Instant>,
}
#[derive(Clone, Copy)]
struct Lease {
    position: Cell,
    anchor: Cell,
    refreshed: Instant,
}
fn lease_current(lease: Lease, now: Instant) -> bool {
    now.saturating_duration_since(lease.refreshed) <= DRIVE_LEASE
}

fn crank_available(
    position: Cell,
    look: [f32; 2],
    peer: &Peer,
    world: &ControlWorld,
) -> Option<Cell> {
    if !peer.synced
        || peer.vitals.dead()
        || peer.chat.teleport.is_some()
        || !peer.permissions.build
        || peer.game_mode(&world.config) == terra_core::GameMode::Spectator
        || world
            .sleep
            .as_ref()
            .is_some_and(|sleep| sleep.is_sleeping(peer.id))
        || !look.iter().all(|value| value.is_finite())
        || look[0].abs() > std::f32::consts::TAU + 0.001
        || look[1].abs() > 1.501
        || world.chunks.block_at(position) != Some(100)
    {
        return None;
    }
    let anchor = world.journal.crank_anchor(position)?;
    if !super::kingdoms::permission_allowed(
        peer.uuid,
        terra_core::kingdom::KingdomPermission::Inventories,
        position,
        world,
    ) || !super::kingdoms::permission_allowed(
        peer.uuid,
        terra_core::kingdom::KingdomPermission::Inventories,
        anchor,
        world,
    ) {
        return None;
    }
    if world.journal.valid_crusher(anchor) && !loaded(anchor, &world.chunks, &world.journal)
        || world.chunks.block_at(anchor) != world.journal.block(anchor)
    {
        return None;
    }
    let body = world.bodies.get(peer.body).ok()?;
    let eye = body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let (_, selected) = terra_world::targeting::pick_world_with_context(
        &world.chunks,
        &world.journal,
        &world.block_properties,
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
        terra_world::props::PropPose::Rest.with_season(
            world
                .journal
                .clock
                .unwrap_or_default()
                .season(world.config.world.seasons),
        ),
    );
    matches!(selected,Some(terra_world::targeting::WorldTarget::Block(hit)) if hit.position==position && hit.distance<=6.0).then_some(anchor)
}

pub(super) fn intent(
    position: Option<Cell>,
    look: [f32; 2],
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &ControlWorld,
) {
    let Some(position) = position else {
        peer.crank.lease = None;
        return;
    };
    if !super::kingdoms::permission_allowed(
        peer.uuid,
        terra_core::kingdom::KingdomPermission::Inventories,
        position,
        world,
    ) {
        peer.crank.lease = None;
        return;
    }
    let now = Instant::now();
    if peer
        .crank
        .last_intent
        .is_some_and(|at| now.duration_since(at) < Duration::from_millis(50))
    {
        return;
    }
    peer.crank.last_intent = Some(now);
    if let Some(anchor) = crank_available(position, look, peer, world) {
        if peer.crank.lease.is_none() {
            consumption::cancel(peer, sender);
            peer.mining = None;
            sender.send::<ControlChannel>(ServerControl::Mining(None));
        }
        peer.look = look;
        peer.crank.lease = Some(Lease {
            position,
            anchor,
            refreshed: now,
        });
    } else {
        peer.crank.lease = None;
    }
}

fn active_drives(
    world: &ControlWorld,
    peers: &mut Query<(&mut Peer, &mut MessageSender<ServerControl>), With<Connected>>,
    now: Instant,
) -> std::collections::HashSet<Cell> {
    let mut active = std::collections::HashSet::new();
    for (mut peer, _) in peers.iter_mut() {
        if let Some(lease) = peer.crank.lease {
            if lease_current(lease, now)
                && crank_available(lease.position, peer.look, &peer, world) == Some(lease.anchor)
            {
                active.insert(lease.position);
            } else {
                peer.crank.lease = None;
            }
        }
    }
    active
}

#[derive(Default)]
pub(super) struct Schedule {
    elapsed: f32,
    accounted: HashMap<Cell, f64>,
    cursor: Option<Cell>,
    sent: HashMap<PlayerId, (u32, Vec<CrusherVisual>)>,
    kinetic_sent: HashMap<PlayerId, (u32, Vec<KineticVisual>)>,
    kinetics: super::kinetics::Runtime,
    logistics: super::logistics::Runtime,
}

pub(super) fn loaded(position: Cell, chunks: &ChunkStore, journal: &EditJournal) -> bool {
    terra_world::crushers::loaded(position, chunks, journal)
}

pub(super) fn process(
    position: Cell,
    elapsed: f32,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
) -> Result<(), String> {
    if !loaded(position, chunks, journal) {
        return Ok(());
    }
    if !journal.crusher_driven(position) {
        return Ok(());
    }
    let Some(mut state) = journal.crusher_at(position).cloned() else {
        return Ok(());
    };
    let mut inventory = journal.container_at(position).cloned().unwrap_or_default();
    let rpm = journal.kinetic_state(position).rpm.abs();
    let elapsed = if rpm == 0.0 {
        elapsed
    } else {
        elapsed * rpm / 30.0
    };
    if crusher::advance(&mut state, &mut inventory, elapsed) {
        journal.commit_crusher(position, state, inventory)?;
    }
    Ok(())
}

fn due(schedule: &mut Schedule, positions: &[Cell], now: f64, elapsed: f32) -> Vec<(Cell, f32)> {
    schedule
        .accounted
        .retain(|position, _| positions.binary_search(position).is_ok());
    if positions.is_empty() {
        schedule.cursor = None;
        return Vec::new();
    }
    let start = schedule.cursor.map_or(0, |cursor| {
        positions.partition_point(|position| *position <= cursor)
    }) % positions.len();
    let mut result = Vec::new();
    for offset in 0..positions.len().min(PROCESS_LIMIT) {
        let position = positions[(start + offset) % positions.len()];
        let previous = schedule
            .accounted
            .insert(position, now)
            .unwrap_or(now - f64::from(elapsed));
        result.push((position, (now - previous).clamp(0.0, 1.0) as f32));
        schedule.cursor = Some(position);
    }
    result
}

fn visuals(center: [f64; 3], chunks: &ChunkStore, journal: &EditJournal) -> Vec<CrusherVisual> {
    let mut stations: Vec<_> = chunks
        .loaded
        .keys()
        .flat_map(|section| journal.crushers_in_section(*section))
        .filter_map(|position| {
            let state = journal.crusher_at(position)?;
            let distance = (0..3)
                .map(|axis| (position[axis] as f64 + 0.5 - center[axis]).powi(2))
                .sum::<f64>();
            (distance <= 64.0 * 64.0 && loaded(position, chunks, journal)).then(|| {
                let empty = Inventory::default();
                let view = state.view(journal.container_at(position).unwrap_or(&empty));
                (
                    distance,
                    CrusherVisual {
                        position,
                        working: journal.crusher_driven(position) && view.working,
                        progress: (view.progress * 32.0).round() / 32.0,
                        manual_active: journal.crusher_driven(position),
                    },
                )
            })
        })
        .collect();
    stations.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.position.cmp(&b.1.position)));
    stations
        .into_iter()
        .take(64)
        .map(|(_, visual)| visual)
        .collect()
}

fn process_mixer(
    position: Cell,
    elapsed: f32,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
) -> Result<(), String> {
    let drive = journal.kinetic_state(position);
    if chunks.block_at(position) != Some(terra_core::mixer::MATERIAL) || !drive.running() {
        return Ok(());
    }
    let Some(mut state) = journal.mixer_at(position).cloned() else {
        return Ok(());
    };
    let mut inventory = journal.container_at(position).cloned().unwrap_or_default();
    if terra_core::mixer::advance(&mut state, &mut inventory, elapsed * drive.rpm.abs() / 30.0) {
        journal.commit_mixer(position, state, inventory)?;
    }
    Ok(())
}

pub(super) fn tick(
    mut world: ControlWorld,
    time: Res<Time>,
    mut schedule: Local<Schedule>,
    mut peers: Query<(&mut Peer, &mut MessageSender<ServerControl>), With<Connected>>,
) {
    if !peers.iter().any(|(peer, _)| peer.synced) {
        world
            .journal
            .bypass_change_detection()
            .replace_crusher_drives(Default::default());
        world
            .journal
            .bypass_change_detection()
            .replace_kinetic_states(Default::default());
        *schedule = Schedule::default();
        return;
    }
    schedule.elapsed += time.delta_secs();
    if schedule.elapsed < 0.1 {
        return;
    }
    let elapsed = std::mem::take(&mut schedule.elapsed).min(1.0);
    let now = time.elapsed_secs_f64();
    let active = active_drives(&world, &mut peers, Instant::now());
    let ControlWorld {
        chunks,
        mut journal,
        bodies,
        terrain,
        block_properties,
        ..
    } = world;
    let journal = journal.bypass_change_detection();
    super::kinetics::advance(
        &mut schedule.kinetics,
        &terrain.0,
        &chunks,
        journal,
        &active,
    );
    super::kinetics::transport(
        &mut schedule.kinetics,
        &chunks,
        journal,
        &block_properties,
        elapsed,
    );
    super::logistics::advance(
        &mut schedule.logistics,
        &chunks,
        journal,
        &block_properties,
        elapsed,
    );
    let mut positions: Vec<_> = journal
        .kinetic_states()
        .filter_map(|(position, state)| {
            (state.running()
                && matches!(journal.block(position), Some(99 | 156))
                && journal.object_state(position).is_some_and(|s| !s.secondary))
            .then_some(position)
        })
        .filter(|position| {
            if journal.block(*position) == Some(156) {
                // Full pigment fitting belongs after the shared 32-station budget.
                return journal.mixer_at(*position).is_some()
                    && journal.container_at(*position).is_some_and(|inventory| {
                        (0..terra_core::mixer::INPUT_SLOTS)
                            .any(|slot| inventory.slot(slot).is_some())
                    });
            }
            let Some(state) = journal.crusher_at(*position) else {
                return false;
            };
            journal.crusher_driven(*position)
                && loaded(*position, &chunks, journal)
                && (state.can_process(
                    journal
                        .container_at(*position)
                        .unwrap_or(&Inventory::default()),
                ) || state.recipe.is_some()
                    && journal
                        .container_at(*position)
                        .is_none_or(|inventory| inventory.slot(0).is_none()))
        })
        .collect();
    positions.sort_unstable();
    for (position, dt) in due(&mut schedule, &positions, now, elapsed) {
        let result = if journal.block(position) == Some(156) {
            process_mixer(position, dt, &chunks, journal)
        } else {
            process(position, dt, &chunks, journal)
        };
        if let Err(error) = result {
            tracing::warn!(%error, ?position, "Crusher could not be advanced");
        }
    }
    schedule
        .sent
        .retain(|id, _| peers.iter().any(|(peer, _)| peer.synced && peer.id == *id));
    schedule
        .kinetic_sent
        .retain(|id, _| peers.iter().any(|(peer, _)| peer.synced && peer.id == *id));
    for (peer, mut sender) in &mut peers {
        if !peer.synced {
            continue;
        }
        let Ok(body) = bodies.get(peer.body) else {
            continue;
        };
        let visible = visuals(body.translation.to_array().map(f64::from), &chunks, journal);
        let kinetics =
            super::kinetics::visuals(body.translation.to_array().map(f64::from), journal);
        if !schedule
            .kinetic_sent
            .get(&peer.id)
            .is_some_and(|(generation, previous)| {
                *generation == peer.generation && *previous == kinetics
            })
        {
            sender.send::<ControlChannel>(ServerControl::Kinetics(kinetics.clone()));
            schedule
                .kinetic_sent
                .insert(peer.id, (peer.generation, kinetics));
        }
        if !schedule
            .sent
            .get(&peer.id)
            .is_some_and(|(generation, previous)| {
                *generation == peer.generation && *previous == visible
            })
        {
            sender.send::<ControlChannel>(ServerControl::Crushers(visible.clone()));
            schedule.sent.insert(peer.id, (peer.generation, visible));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{ItemKind, ItemStack, MineralItem};
    use terra_world::{edits::BlockObjectState, streaming::LoadedSection};
    fn fixture() -> (Cell, ChunkStore, EditJournal) {
        let anchor = [4, 1, 4];
        let peer = [5, 1, 4];
        let section = terra_voxel::SectionPos::default();
        let mut data = terra_voxel::BlockSection::empty(section).unwrap();
        let mut journal = EditJournal::default();
        for (position, other, secondary) in [(anchor, peer, false), (peer, anchor, true)] {
            data.set_block(position.map(|axis| axis as usize), 99)
                .unwrap();
            journal.record(position, 99).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        peer: Some(other),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        journal.set_crusher(anchor, Default::default()).unwrap();
        journal
            .set_container(
                anchor,
                Inventory {
                    slots: vec![ItemStack {
                        item: ItemKind::Mineral(MineralItem::Coal),
                        count: 2,
                    }],
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
        (anchor, chunks, journal)
    }
    #[test]
    fn mixer_requires_loaded_actual_kinetic_drive_and_commits_one_exact_rgb_batch() {
        use terra_core::{kinetic, mixer};
        let (anchor, mut chunks, mut journal) = fixture();
        let section = terra_voxel::global_to_section(anchor).0;
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(terra_voxel::global_to_section(anchor).1, mixer::MATERIAL)
            .unwrap();
        journal.remove_crusher(anchor);
        journal.record(anchor, mixer::MATERIAL).unwrap();
        journal
            .set_object_state(anchor, Some(BlockObjectState::default()))
            .unwrap();
        let target = [41, 176, 221];
        let recipe = mixer::recipe(target);
        let mut inventory = Inventory::default();
        inventory.slots.resize(mixer::SLOTS, ItemStack::EMPTY);
        for (slot, stack) in recipe.inputs.iter().enumerate() {
            inventory.slots[slot] = *stack;
        }
        journal
            .commit_mixer(
                anchor,
                mixer::MixerState {
                    target,
                    ..Default::default()
                },
                inventory.clone(),
            )
            .unwrap();
        for _ in 0..10 {
            process_mixer(anchor, 1.0, &chunks, &mut journal).unwrap();
        }
        assert_eq!(journal.mixer_at(anchor).unwrap().progress_seconds, 0.0);
        assert_eq!(journal.container_at(anchor), Some(&inventory));
        let source = kinetic::Node {
            position: [anchor[0] - 1, anchor[1], anchor[2]],
            material: 100,
            facing: 0,
            secondary: false,
            source_rpm: 30.0,
            source_capacity: 12800,
            belt_load: false,
            links: [None; 2],
        };
        let machine = kinetic::Node {
            position: anchor,
            material: 156,
            source_rpm: 0.0,
            source_capacity: 0,
            belt_load: false,
            ..source
        };
        let nodes = [source, machine];
        let solved = kinetic::solve_component(anchor, &mut 256, |p| {
            nodes.iter().find(|n| n.position == p).copied()
        });
        journal.replace_kinetic_states(solved.into_iter().map(|(n, s)| (n.position, s)).collect());
        assert!(journal.kinetic_state(anchor).running());
        let epoch = journal.mesh_epoch(section);
        for _ in 0..4 {
            process_mixer(anchor, 1.0, &chunks, &mut journal).unwrap();
        }
        assert_eq!(
            journal.container_at(anchor).unwrap().slot(mixer::OUTPUT),
            Some(recipe.output)
        );
        assert_eq!(journal.mixer_at(anchor).unwrap().completed, 1);
        assert_eq!(journal.mesh_epoch(section), epoch);
        let before = journal.mixer_at(anchor).cloned();
        chunks.loaded.clear();
        process_mixer(anchor, 1.0, &chunks, &mut journal).unwrap();
        assert_eq!(journal.mixer_at(anchor).cloned(), before);
    }
    #[test]
    fn loaded_crusher_conserves_material_without_remeshing_and_unloaded_time_does_not_advance() {
        let (anchor, mut chunks, mut journal) = fixture();
        let section = terra_voxel::global_to_section(anchor).0;
        let epoch = journal.mesh_epoch(section);
        let delta = journal.section_delta(section);
        let before = journal.container_at(anchor).cloned();
        for _ in 0..40 {
            process(anchor, 1.0, &chunks, &mut journal).unwrap();
        }
        assert_eq!(journal.container_at(anchor).cloned(), before);
        assert_eq!(journal.crusher_at(anchor).unwrap().progress_seconds, 0.0);
        journal.replace_crusher_drives([anchor].into_iter().collect());
        for _ in 0..4 {
            process(anchor, 1.0, &chunks, &mut journal).unwrap();
        }
        let inventory = journal.container_at(anchor).unwrap();
        assert_eq!(inventory.slot(0).unwrap().count, 1);
        assert_eq!(
            inventory.slot(1).unwrap().item,
            ItemKind::Dust(crusher::DustKind::Coal)
        );
        assert_eq!(journal.crusher_at(anchor).unwrap().completed, 1);
        let before = (
            journal.crusher_at(anchor).cloned(),
            journal.container_at(anchor).cloned(),
        );
        chunks.loaded.clear();
        process(anchor, 60.0, &chunks, &mut journal).unwrap();
        assert_eq!(
            (
                journal.crusher_at(anchor).cloned(),
                journal.container_at(anchor).cloned()
            ),
            before
        );
        assert_eq!(journal.mesh_epoch(section), epoch);
        assert_eq!(journal.section_delta(section), delta);
    }
    #[test]
    fn fair_budget_is_bounded_and_unloaded_stations_leave_no_accounting_cache() {
        let positions: Vec<_> = (0..100).map(|x| [x, 0, 0]).collect();
        let mut schedule = Schedule::default();
        let mut visited = std::collections::HashSet::new();
        for tick in 1..=4 {
            let selected = due(&mut schedule, &positions, tick as f64 * 0.125, 0.125);
            assert_eq!(selected.len(), PROCESS_LIMIT);
            for (position, elapsed) in selected {
                assert!(elapsed <= 1.0);
                visited.insert(position);
            }
        }
        assert_eq!(visited.len(), 100);
        assert!(due(&mut schedule, &[], 900.0, 0.125).is_empty());
        assert!(schedule.accounted.is_empty() && schedule.cursor.is_none());
        assert_eq!(due(&mut schedule, &positions, 901.0, 0.125)[0].1, 0.125);
    }
    #[test]
    fn visual_snapshots_include_idle_loaded_machines_but_not_private_contents_or_unloaded_regions()
    {
        let (anchor, mut chunks, mut journal) = fixture();
        journal.set_container(anchor, Inventory::default()).unwrap();
        let snapshots = visuals([4.0, 2.0, 4.0], &chunks, &journal);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].position, anchor);
        assert!(!snapshots[0].working && snapshots[0].valid());
        assert!(visuals([200.0; 3], &chunks, &journal).is_empty());
        chunks.loaded.clear();
        assert!(visuals([4.0, 2.0, 4.0], &chunks, &journal).is_empty());
    }
    #[test]
    fn metadata_revision_overflow_leaves_input_and_processing_state_unchanged() {
        let (anchor, chunks, mut journal) = fixture();
        journal.replace_crusher_drives([anchor].into_iter().collect());
        journal.gameplay_mut().revision = u64::MAX;
        let before = (
            journal.crusher_at(anchor).cloned(),
            journal.container_at(anchor).cloned(),
        );
        assert!(process(anchor, 1.0, &chunks, &mut journal).is_err());
        assert_eq!(
            (
                journal.crusher_at(anchor).cloned(),
                journal.container_at(anchor).cloned()
            ),
            before
        );
    }
    #[test]
    fn an_empty_machine_can_be_turned_without_falsely_processing_or_persisting_its_drive() {
        let (anchor, chunks, mut journal) = fixture();
        journal.set_container(anchor, Inventory::default()).unwrap();
        journal.replace_crusher_drives([anchor].into_iter().collect());
        let before = (
            journal.crusher_at(anchor).cloned(),
            journal.gameplay().revision,
        );
        process(anchor, 1.0, &chunks, &mut journal).unwrap();
        assert_eq!(
            (
                journal.crusher_at(anchor).cloned(),
                journal.gameplay().revision
            ),
            before
        );
        let visual = visuals([4.0, 2.0, 4.0], &chunks, &journal);
        assert!(visual[0].manual_active && !visual[0].working);
        journal.replace_crusher_drives(Default::default());
        assert!(!visuals([4.0, 2.0, 4.0], &chunks, &journal)[0].manual_active);
    }
    #[test]
    fn drive_lease_expires_without_pulses_and_never_accumulates_offline_work() {
        let now = Instant::now();
        let lease = Lease {
            position: [6, 1, 4],
            anchor: [4, 1, 4],
            refreshed: now,
        };
        assert!(lease_current(lease, now + Duration::from_millis(349)));
        assert!(!lease_current(lease, now + Duration::from_millis(351)));
        assert!(!lease_current(lease, now + Duration::from_secs(60)));
    }
}
