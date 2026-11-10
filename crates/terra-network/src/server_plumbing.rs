//! Bounded authoritative water transfers and change-only nearby presentation.
use super::*;

#[derive(Default)]
pub(super) struct Schedule {
    elapsed: f32,
    next_send: f64,
    runtime: terra_world::plumbing::Runtime,
    rain: terra_world::plumbing::RainRuntime,
    trains: terra_world::train_services::Runtime,
    sent: HashMap<PlayerId, (u32, Vec<PlumbingVisual>)>,
    sent_infusion: HashMap<PlayerId, (u32, Vec<terra_core::infusion::HoldVisual>)>,
}

fn infusion_visuals(
    position: [f64; 3],
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Vec<terra_core::infusion::HoldVisual> {
    let _scope = terra_core::profiling::ScopeTimer::new("server.infusion_visuals");
    let mut nearby = Vec::new();
    for section in chunks.loaded.keys() {
        for cell in journal.holds_in_section(*section) {
            let distance = (0..3)
                .map(|i| (cell[i] as f64 + 0.5 - position[i]).powi(2))
                .sum::<f64>();
            if distance <= 128.0_f64.powi(2)
                && terra_world::conveyors::infusion_loaded(cell, chunks, journal)
            {
                nearby.push((distance, cell));
            }
        }
    }
    let order = |a: &(f64, [i64; 3]), b: &(f64, [i64; 3])| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1));
    if nearby.len() > terra_core::infusion::MAX_HOLD_VISUALS {
        nearby.select_nth_unstable_by(terra_core::infusion::MAX_HOLD_VISUALS, order);
        nearby.truncate(terra_core::infusion::MAX_HOLD_VISUALS);
    }
    nearby.sort_unstable_by(order);
    nearby
        .into_iter()
        .filter_map(|(_, cell)| {
            journal
                .infusion_view(cell)
                .or_else(|| terra_world::conveyors::infusion_geometry(cell, journal))
        })
        .filter(|view| view.valid())
        .collect()
}

fn visuals(position: [f64; 3], chunks: &ChunkStore, journal: &EditJournal) -> Vec<PlumbingVisual> {
    let distance = |cell: [i64; 3]| {
        (0..3)
            .map(|i| (cell[i] as f64 + 0.5 - position[i]).powi(2))
            .sum::<f64>()
    };
    let mut nearby: Vec<_> = journal
        .plumbing_positions()
        .filter_map(|cell| {
            let d = distance(cell);
            (d <= 128.0_f64.powi(2)
                && chunks
                    .block_at(cell)
                    .is_some_and(terra_core::plumbing::is_part))
            .then_some((d, cell))
        })
        .collect();
    let order = |a: &(f64, [i64; 3]), b: &(f64, [i64; 3])| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1));
    if nearby.len() > 256 {
        nearby.select_nth_unstable_by(256, order);
        nearby.truncate(256);
    }
    nearby.sort_unstable_by(order);
    nearby
        .into_iter()
        .filter_map(|(_, position)| {
            let state = terra_world::plumbing::visual_data(chunks, journal, position)?;
            let view = PlumbingVisual {
                position,
                amount: state.amount,
                capacity: state.capacity,
                open: state.open,
                flowing: state.flowing,
                supplied: state.supplied,
                pump_rpm: state.pump_rpm,
                basin: state.basin,
                intake: state.intake,
            };
            view.valid().then_some(view)
        })
        .collect()
}

pub(super) fn tick(
    mut world: ControlWorld,
    time: Res<Time>,
    mut schedule: Local<Schedule>,
    mut peers: Query<(&Peer, &mut MessageSender<ServerControl>), With<Connected>>,
) {
    if !peers.iter().any(|(peer, _)| peer.synced) {
        *schedule = Schedule::default();
        return;
    }
    schedule.elapsed += time.delta_secs().min(0.1);
    if schedule.elapsed < 0.1 {
        return;
    }
    let elapsed = std::mem::take(&mut schedule.elapsed).min(0.25);
    if let Err(error) = terra_world::plumbing::advance(
        &mut schedule.runtime,
        &mut world.chunks,
        world.journal.bypass_change_detection(),
        &world.terrain.0,
        elapsed,
    ) {
        debug!(%error, "Water network update deferred");
    }
    let weather = *world.weather;
    let clock = *world.clock;
    if let Err(error) = terra_world::plumbing::collect_rain(
        &mut schedule.rain,
        &world.chunks,
        world.journal.bypass_change_detection(),
        &world.terrain.0,
        &weather,
        clock,
        elapsed,
    ) {
        debug!(%error, "Basin rain collection deferred");
    }
    if let Err(error) = terra_world::train_services::advance(
        &mut schedule.trains,
        &world.chunks,
        world.journal.bypass_change_detection(),
        elapsed,
    ) {
        debug!(%error,"Train service update deferred");
    }
    let now = time.elapsed_secs_f64();
    if now < schedule.next_send {
        return;
    }
    schedule.next_send = now + 0.1;
    schedule
        .sent
        .retain(|id, _| peers.iter().any(|(peer, _)| peer.synced && peer.id == *id));
    schedule
        .sent_infusion
        .retain(|id, _| peers.iter().any(|(peer, _)| peer.synced && peer.id == *id));
    for (peer, mut sender) in &mut peers {
        if !peer.synced {
            continue;
        }
        let Ok(body) = world.bodies.get(peer.body) else {
            continue;
        };
        let next = visuals(
            body.translation.to_array().map(f64::from),
            &world.chunks,
            &world.journal,
        );
        if !schedule
            .sent
            .get(&peer.id)
            .is_some_and(|(generation, previous)| {
                *generation == peer.generation && previous == &next
            })
        {
            sender.send::<ControlChannel>(ServerControl::Plumbing(next.clone()));
            schedule.sent.insert(peer.id, (peer.generation, next));
        }
        let infusion = infusion_visuals(
            body.translation.to_array().map(f64::from),
            &world.chunks,
            &world.journal,
        );
        if !schedule
            .sent_infusion
            .get(&peer.id)
            .is_some_and(|(generation, previous)| {
                *generation == peer.generation && previous == &infusion
            })
        {
            sender.send::<ControlChannel>(ServerControl::Infusion(infusion.clone()));
            schedule
                .sent_infusion
                .insert(peer.id, (peer.generation, infusion));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::{BlockSection, SectionPos};
    use terra_world::streaming::LoadedSection;

    #[test]
    fn hold_snapshots_include_idle_connected_geometry_are_nearest_bounded_and_skip_unloaded() {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let owner = SectionPos::default();
        let mut data = BlockSection::empty(owner).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                let p = [x, 4, z];
                data.set_block(p.map(|v| v as usize), terra_core::infusion::HOLD_BLOCK)
                    .unwrap();
                journal.record(p, terra_core::infusion::HOLD_BLOCK).unwrap();
                journal
                    .set_object_state(p, Some(Default::default()))
                    .unwrap();
            }
        }
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let views = infusion_visuals([0.5, 4.5, 0.5], &chunks, &journal);
        assert_eq!(views.len(), terra_core::infusion::MAX_HOLD_VISUALS);
        assert_eq!(views[0].position, [0, 4, 0]);
        assert!(
            views
                .iter()
                .all(|view| view.valid() && view.phase == terra_core::infusion::HoldPhase::Idle)
        );
        chunks.loaded.clear();
        assert!(infusion_visuals([0.; 3], &chunks, &journal).is_empty());
    }

    #[test]
    fn visual_snapshots_are_bounded_nearest_first_and_skip_unloaded_nodes() {
        let mut chunks = ChunkStore::default();
        let owner = SectionPos::default();
        let mut data = BlockSection::empty(owner).unwrap();
        let mut journal = EditJournal::default();
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..2 {
                    let p = [x, y, z];
                    data.set_block(p.map(|v| v as usize), 165).unwrap();
                    journal.record(p, 165).unwrap();
                    journal
                        .set_object_state(p, Some(Default::default()))
                        .unwrap();
                    journal.set_plumbing(p, Default::default()).unwrap();
                }
            }
        }
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let next = visuals([0.5, 0.5, 0.5], &chunks, &journal);
        assert_eq!(next.len(), 256);
        assert_eq!(next[0].position, [0, 0, 0]);
        assert!(next.iter().all(PlumbingVisual::valid));
        chunks.loaded.clear();
        assert!(visuals([0.5, 0.5, 0.5], &chunks, &journal).is_empty());
    }
}
