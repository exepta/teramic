//! Loaded-only mechanical simulation; all network and item work has a tick budget.
use super::*;
use std::collections::{BTreeMap, HashSet};
use terra_core::kinetic::{self, Cell, KineticState, Node};

const NODE_BUDGET: usize = 1024;
#[derive(Default)]
pub(super) struct Runtime {
    transport: terra_world::conveyors::Transport,
}

#[cfg(test)]
fn wheel_rpm(
    position: Cell,
    facing: u8,
    terrain: &terra_world::TerraGenerator,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> i16 {
    wheel_drive(position, facing, terrain, chunks, journal).rpm as i16
}

fn node_at(
    position: Cell,
    terrain: &terra_world::TerraGenerator,
    chunks: &ChunkStore,
    journal: &EditJournal,
    cranks: &HashSet<Cell>,
    steam: &BTreeMap<Cell, terra_world::train_services::MechanicalDrive>,
) -> Option<Node> {
    let material = chunks.block_at(position)?;
    kinetic::part(material)?;
    let state = journal.object_state(position)?;
    if !state.valid_for(position, material) {
        return None;
    }
    if material == 99 {
        let anchor = journal.crusher_primary(position)?;
        if !super::crushers::loaded(anchor, chunks, journal) {
            return None;
        }
    }
    if material == 100 {
        let anchor = journal.crank_anchor(position)?;
        if chunks.block_at(anchor) != journal.block(anchor) {
            return None;
        }
        if journal.valid_crusher(anchor) && !super::crushers::loaded(anchor, chunks, journal) {
            return None;
        }
    }
    let drive = if material == 100 && cranks.contains(&position) {
        kinetic::SourceDrive {
            rpm: 30.0,
            capacity: 12800,
        }
    } else if material == kinetic::WATER_WHEEL {
        wheel_drive(position, state.facing, terrain, chunks, journal)
    } else if let Some(drive) = steam.get(&position) {
        kinetic::SourceDrive {
            rpm: drive.rpm,
            capacity: drive.capacity,
        }
    } else {
        kinetic::SourceDrive::default()
    };
    let belt = if matches!(material, 144..=146 | 148) {
        let segment = terra_world::conveyors::segment_at(journal, position);
        if material == kinetic::CONVEYOR && segment.is_none() {
            return None;
        }
        if let Some(segment) = segment {
            if !segment.span.loaded(chunks, journal) {
                return None;
            }
            Some(segment)
        } else {
            None
        }
    } else {
        None
    };
    Some(Node {
        position,
        material,
        facing: state.facing,
        secondary: state.secondary
            || material == kinetic::CONVEYOR
                && belt.is_some_and(|s| s.span.linked && position != s.span.controller),
        source_rpm: drive.rpm,
        source_capacity: drive.capacity,
        belt_load: matches!(material, 144..=146)
            && belt
                .is_some_and(|segment| segment.span.linked && segment.span.controller == position),
        links: belt.map_or([None; 2], |s| s.span.links(position)),
    })
}

fn wheel_drive(
    position: Cell,
    facing: u8,
    terrain: &terra_world::TerraGenerator,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> kinetic::SourceDrive {
    let axis = kinetic::axis(facing);
    if axis[1] != 0 {
        return Default::default();
    }
    let tangent = [-axis[2], 0, axis[0]];
    let mut torques = [0.0_f32; 3];
    let mut strengths = [0.0_f32; 3];
    // Sample the wetted rim faces, not beyond the wheel's voxel footprint.
    // The axial faces sit just outside its conservative cross-shaped collider.
    let contact_radius = f64::from(kinetic::radial_extent(kinetic::WATER_WHEEL).unwrap()) - 0.01;
    for segment in 0..8 {
        let angle = f64::from(segment) * std::f64::consts::TAU / 8.0;
        for axial in [-0.40, 0.40] {
            let radius: [f64; 3] = std::array::from_fn(|i| {
                f64::from(tangent[i] as i32) * angle.sin() * contact_radius
                    - if i == 1 {
                        angle.cos() * contact_radius
                    } else {
                        0.0
                    }
            });
            let point = std::array::from_fn(|i| {
                position[i] as f64 + 0.5 + radius[i] + axis[i] as f64 * axial
            });
            let motion = terra_world::water::wheel_motion_at(terrain, chunks, journal, point);
            let Some(kind) = motion.kind else { continue };
            let cross = [
                radius[1] * f64::from(motion.velocity[2])
                    - radius[2] * f64::from(motion.velocity[1]),
                radius[2] * f64::from(motion.velocity[0])
                    - radius[0] * f64::from(motion.velocity[2]),
                radius[0] * f64::from(motion.velocity[1])
                    - radius[1] * f64::from(motion.velocity[0]),
            ];
            let torque =
                (0..3).map(|i| cross[i] * axis[i] as f64).sum::<f64>() as f32 * motion.immersion;
            let index = match kind {
                terra_world::water::WaterDriveKind::Current => 0,
                terra_world::water::WaterDriveKind::Fall => 1,
                terra_world::water::WaterDriveKind::Ocean => 2,
            };
            torques[index] += torque;
            strengths[index] = strengths[index].max(motion.strength);
        }
    }
    let total: f32 = torques.iter().sum();
    if total.abs() < 0.1 {
        return Default::default();
    }
    let dominant = (0..3)
        .max_by(|&a, &b| torques[a].abs().total_cmp(&torques[b].abs()))
        .unwrap();
    kinetic::SourceDrive {
        rpm: kinetic::WATER_WHEEL_RPM * total.signum(),
        capacity: (f32::from(kinetic::WATER_WHEEL_CAPACITY) * strengths[dominant]).round() as u16,
    }
}

fn loaded_seeds(chunks: &ChunkStore, journal: &EditJournal) -> Vec<Cell> {
    if !journal.has_kinetics() {
        return Vec::new();
    }
    let mut sections: Vec<_> = chunks
        .loaded
        .keys()
        .copied()
        .filter(|section| journal.kinetics_in_section(*section).next().is_some())
        .collect();
    sections.sort_unstable();
    // Stable admission prevents a limited simulation budget from alternately
    // powering and stopping valid networks. Unloaded history is never visited.
    sections
        .into_iter()
        .flat_map(|section| journal.kinetics_in_section(section))
        .take(NODE_BUDGET)
        .collect()
}

pub(super) fn advance(
    _runtime: &mut Runtime,
    terrain: &terra_world::TerraGenerator,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    cranks: &HashSet<Cell>,
) {
    let steam = terra_world::train_services::mechanical_drives(chunks, journal);
    let seeds = loaded_seeds(chunks, journal);
    let mut budget = NODE_BUDGET;
    let mut states = BTreeMap::<Cell, KineticState>::new();
    let mut cached = HashMap::new();
    for seed in seeds {
        if budget == 0 {
            break;
        }
        if states.contains_key(&seed) {
            continue;
        }
        let solved = kinetic::solve_component(seed, &mut budget, |p| {
            *cached
                .entry(p)
                .or_insert_with(|| node_at(p, terrain, chunks, journal, cranks, &steam))
        });
        for (node, state) in solved {
            states.insert(node.position, state);
        }
    }
    let drives = states
        .iter()
        .filter_map(|(position, state)| {
            state
                .running()
                .then(|| journal.crusher_primary(*position))
                .flatten()
        })
        .collect();
    journal.replace_kinetic_states(states);
    journal.replace_crusher_drives(drives);
}

pub(super) fn visuals(center: [f64; 3], journal: &EditJournal) -> Vec<KineticVisual> {
    let mut states: Vec<_> = journal
        .kinetic_states()
        .filter_map(|(position, state)| {
            let distance = (0..3)
                .map(|i| (position[i] as f64 + 0.5 - center[i]).powi(2))
                .sum::<f64>();
            (distance <= 64.0 * 64.0).then_some((
                distance,
                KineticVisual {
                    position,
                    rpm: state.rpm,
                    overloaded: state.overloaded,
                    stress: state.stress,
                    capacity: state.capacity,
                    available_capacity: state.available_capacity,
                },
            ))
        })
        .collect();
    states.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.position.cmp(&b.1.position)));
    states.into_iter().take(256).map(|(_, v)| v).collect()
}

pub(super) fn transport(
    runtime: &mut Runtime,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    elapsed: f32,
) {
    terra_world::conveyors::transport(&mut runtime.transport, chunks, journal, properties, elapsed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{Inventory, ItemKind, ItemStack, MineralItem, WorldConfig};
    use terra_voxel::{BlockSection, SectionPos, global_to_section};
    use terra_world::{edits::BlockObjectState, streaming::LoadedSection, water::FluidCell};

    fn terrain() -> terra_world::TerraGenerator {
        terra_world::TerraGenerator::new(WorldConfig {
            water: true,
            rivers: false,
            sea_level: -100.0,
            ..Default::default()
        })
        .unwrap()
    }
    fn fixture() -> (ChunkStore, EditJournal) {
        let mut chunks = ChunkStore::default();
        for x in 0..=1 {
            let position = SectionPos { x, y: 0, z: 0 };
            chunks.loaded.insert(
                position,
                LoadedSection {
                    data: BlockSection::empty(position).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
        }
        (chunks, EditJournal::default())
    }
    fn place(
        chunks: &mut ChunkStore,
        journal: &mut EditJournal,
        p: Cell,
        material: u16,
        state: BlockObjectState,
    ) {
        let (section, local) = global_to_section(p);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, material)
            .unwrap();
        journal.record(p, material).unwrap();
        journal.set_object_state(p, Some(state)).unwrap();
    }
    #[test]
    fn crank_powers_existing_crusher_across_sections_and_eviction_stops_without_catchup() {
        let (mut chunks, mut journal) = fixture();
        place(
            &mut chunks,
            &mut journal,
            [13, 3, 4],
            100,
            BlockObjectState {
                facing: 2,
                peer: Some([14, 3, 4]),
                ..Default::default()
            },
        );
        for x in [14, 15] {
            place(
                &mut chunks,
                &mut journal,
                [x, 3, 4],
                kinetic::IRON_SHAFT,
                Default::default(),
            );
        }
        let anchor = [16, 3, 4];
        let partner = [17, 3, 4];
        place(
            &mut chunks,
            &mut journal,
            anchor,
            99,
            BlockObjectState {
                peer: Some(partner),
                ..Default::default()
            },
        );
        place(
            &mut chunks,
            &mut journal,
            partner,
            99,
            BlockObjectState {
                peer: Some(anchor),
                secondary: true,
                ..Default::default()
            },
        );
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
        let mut runtime = Runtime::default();
        let cranks = HashSet::from([[13, 3, 4]]);
        let terrain = terrain();
        advance(&mut runtime, &terrain, &chunks, &mut journal, &cranks);
        assert!(journal.crusher_driven(anchor));
        assert_eq!(journal.kinetic_state(anchor).rpm, -30.0);
        super::super::crushers::process(anchor, 0.5, &chunks, &mut journal).unwrap();
        let progress = journal.crusher_at(anchor).unwrap().progress_seconds;
        assert!(progress > 0.0);
        let source = chunks.loaded.remove(&SectionPos::default()).unwrap();
        advance(&mut runtime, &terrain, &chunks, &mut journal, &cranks);
        assert!(!journal.crusher_driven(anchor));
        super::super::crushers::process(anchor, 0.5, &chunks, &mut journal).unwrap();
        assert_eq!(
            journal.crusher_at(anchor).unwrap().progress_seconds,
            progress
        );
        chunks.loaded.insert(SectionPos::default(), source);
        advance(&mut runtime, &terrain, &chunks, &mut journal, &cranks);
        assert!(journal.crusher_driven(anchor));
        assert_eq!(
            journal.crusher_at(anchor).unwrap().progress_seconds,
            progress
        );
        advance(
            &mut runtime,
            &terrain,
            &chunks,
            &mut journal,
            &HashSet::new(),
        );
        assert!(!journal.crusher_driven(anchor));
    }

    #[test]
    fn unloaded_placement_history_does_not_pulse_loaded_power_or_delay_source_invalidation() {
        let (mut chunks, mut journal) = fixture();
        let shaft = [6, 3, 4];
        let crank = [5, 3, 4];
        place(
            &mut chunks,
            &mut journal,
            shaft,
            kinetic::IRON_SHAFT,
            Default::default(),
        );
        place(
            &mut chunks,
            &mut journal,
            crank,
            100,
            BlockObjectState {
                facing: 2,
                peer: Some(shaft),
                ..Default::default()
            },
        );
        for index in 0..2048 {
            let p = [-64 + index % 16, index / 16 % 16, index / 256];
            journal.record(p, kinetic::IRON_SHAFT).unwrap();
            journal
                .set_object_state(p, Some(Default::default()))
                .unwrap();
        }
        assert_eq!(loaded_seeds(&chunks, &journal), [crank, shaft]);
        let mut runtime = Runtime::default();
        let terrain = terrain();
        let cranks = HashSet::from([crank]);
        for _ in 0..20 {
            advance(&mut runtime, &terrain, &chunks, &mut journal, &cranks);
            assert_eq!(journal.kinetic_state(shaft).rpm, -30.0);
        }
        advance(
            &mut runtime,
            &terrain,
            &chunks,
            &mut journal,
            &HashSet::new(),
        );
        assert!(!journal.kinetic_state(shaft).running());
        advance(&mut runtime, &terrain, &chunks, &mut journal, &cranks);
        assert!(journal.kinetic_state(shaft).running());
        let source = chunks.loaded.remove(&SectionPos::default()).unwrap();
        advance(&mut runtime, &terrain, &chunks, &mut journal, &cranks);
        assert!(!journal.kinetic_state(shaft).running());
        chunks.loaded.insert(SectionPos::default(), source);
        advance(&mut runtime, &terrain, &chunks, &mut journal, &cranks);
        assert!(journal.kinetic_state(shaft).running());
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block(global_to_section(crank).1, 0)
            .unwrap();
        journal.record(crank, 0).unwrap();
        advance(&mut runtime, &terrain, &chunks, &mut journal, &cranks);
        assert!(!journal.kinetic_state(shaft).running());
    }
    #[test]
    fn waterwheel_requires_real_loaded_flow_and_reverses_with_it() {
        let (mut chunks, mut journal) = fixture();
        let terrain = terrain();
        let position = [7, 3, 7];
        place(
            &mut chunks,
            &mut journal,
            position,
            kinetic::WATER_WHEEL,
            Default::default(),
        );
        let anchor = [5, 3, 7];
        let partner = [6, 3, 7];
        place(
            &mut chunks,
            &mut journal,
            anchor,
            99,
            BlockObjectState {
                peer: Some(partner),
                ..Default::default()
            },
        );
        place(
            &mut chunks,
            &mut journal,
            partner,
            99,
            BlockObjectState {
                peer: Some(anchor),
                secondary: true,
                ..Default::default()
            },
        );
        journal.set_crusher(anchor, Default::default()).unwrap();
        journal
            .set_container(
                anchor,
                Inventory {
                    slots: vec![ItemStack {
                        item: ItemKind::Mineral(MineralItem::Coal),
                        count: 1,
                    }],
                    ..Default::default()
                },
            )
            .unwrap();
        let water = [7, 2, 7];
        // A solid bottom suppresses gravity flow, isolating the horizontal current.
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([7, 1, 7], 3)
            .unwrap();
        journal
            .set_fluid(
                water,
                FluidCell {
                    amount: 8,
                    source: true,
                    flow: [0, 0],
                },
            )
            .unwrap();
        assert_eq!(wheel_rpm(position, 0, &terrain, &chunks, &journal), 0);
        journal
            .set_fluid(
                water,
                FluidCell {
                    amount: 8,
                    source: false,
                    flow: [0, 1],
                },
            )
            .unwrap();
        let positive = wheel_rpm(position, 0, &terrain, &chunks, &journal);
        assert_ne!(positive, 0);
        assert_eq!(
            wheel_drive(position, 0, &terrain, &chunks, &journal).capacity,
            kinetic::WATER_WHEEL_CAPACITY
        );
        let storm = terra_core::WorldWeather {
            from_wind: [6.6, 0.0],
            target_wind: [6.6, 0.0],
            ..Default::default()
        };
        journal.weather = Some(storm);
        assert_eq!(
            wheel_drive(position, 0, &terrain, &chunks, &journal).capacity,
            23040
        );
        journal.weather = None;
        advance(
            &mut Runtime::default(),
            &terrain,
            &chunks,
            &mut journal,
            &HashSet::new(),
        );
        assert!(journal.crusher_driven(anchor));
        super::super::crushers::process(anchor, 0.25, &chunks, &mut journal).unwrap();
        assert!(journal.crusher_at(anchor).unwrap().progress_seconds > 0.0);
        journal
            .set_fluid(
                water,
                FluidCell {
                    amount: 8,
                    source: false,
                    flow: [0, -1],
                },
            )
            .unwrap();
        assert_eq!(
            wheel_rpm(position, 0, &terrain, &chunks, &journal),
            -positive
        );
        journal.set_fluid(water, FluidCell::default()).unwrap();
        assert_eq!(wheel_rpm(position, 0, &terrain, &chunks, &journal), 0);
        chunks.loaded.clear();
        assert_eq!(wheel_rpm(position, 0, &terrain, &chunks, &journal), 0);
    }

    #[test]
    fn three_waterwheels_and_adjacent_short_crusher_belt_keep_their_real_port_directions() {
        use terra_world::conveyors;
        let (mut chunks, mut journal) = fixture();
        let terrain = terrain();
        for p in [[6, 6, 3], [6, 6, 7], [6, 6, 11], [7, 6, 4], [7, 6, 6]] {
            place(
                &mut chunks,
                &mut journal,
                p,
                kinetic::IRON_SHAFT,
                Default::default(),
            );
        }
        let mut spans = Vec::new();
        for (start, end) in [([6, 6, 3], [6, 6, 11]), ([7, 6, 4], [7, 6, 6])] {
            let plan =
                conveyors::layout(&chunks, &journal, start, end, ItemKind::Block(148)).unwrap();
            assert!(conveyors::commit(&mut chunks, &mut journal, &plan).unwrap());
            spans.push(conveyors::span_at(&journal, start).unwrap());
        }
        for (p, material) in [
            ([7, 6, 3], kinetic::IRON_SHAFT),
            ([8, 6, 3], kinetic::IRON_GEAR),
            ([8, 6, 4], kinetic::IRON_GEAR),
        ] {
            place(&mut chunks, &mut journal, p, material, Default::default());
        }
        let crusher = [8, 6, 6];
        let secondary = [9, 6, 6];
        for (p, peer, is_secondary) in [(crusher, secondary, false), (secondary, crusher, true)] {
            place(
                &mut chunks,
                &mut journal,
                p,
                99,
                BlockObjectState {
                    peer: Some(peer),
                    secondary: is_secondary,
                    ..Default::default()
                },
            );
        }
        journal.set_crusher(crusher, Default::default()).unwrap();
        journal
            .set_container(crusher, Inventory::default())
            .unwrap();
        for (z, facing) in [(3, 2), (7, 0), (11, 2)] {
            place(
                &mut chunks,
                &mut journal,
                [5, 6, z],
                kinetic::WATER_WHEEL,
                BlockObjectState {
                    facing,
                    ..Default::default()
                },
            );
            chunks
                .loaded
                .get_mut(&SectionPos::default())
                .unwrap()
                .data
                .set_block([5, 4, z as usize], 3)
                .unwrap();
            journal
                .set_fluid(
                    [5, 5, z],
                    FluidCell {
                        amount: 8,
                        source: false,
                        flow: [0, 1],
                    },
                )
                .unwrap();
            assert_eq!(
                wheel_drive([5, 6, z], facing, &terrain, &chunks, &journal).rpm,
                if facing == 2 {
                    kinetic::WATER_WHEEL_RPM
                } else {
                    -kinetic::WATER_WHEEL_RPM
                }
            );
        }
        let mut runtime = Runtime::default();
        for target in [
            None,
            Some(spans[0].controller),
            Some(spans[1].end),
            Some(spans[1].controller),
        ] {
            if let Some(p) = target {
                let plan = conveyors::dye_layout(
                    &chunks,
                    &journal,
                    p,
                    if p == spans[1].controller {
                        terra_core::beds::DyeColor::Purple
                    } else {
                        terra_core::beds::DyeColor::LightBlue
                    },
                )
                .unwrap();
                assert!(conveyors::commit(&mut chunks, &mut journal, &plan).unwrap());
            }
            advance(
                &mut runtime,
                &terrain,
                &chunks,
                &mut journal,
                &HashSet::new(),
            );
            assert_eq!(
                journal.kinetic_state(spans[0].start).rpm,
                -kinetic::WATER_WHEEL_RPM
            );
            assert_eq!(journal.kinetic_state(crusher).rpm, kinetic::WATER_WHEEL_RPM);
            assert!(journal.crusher_driven(crusher));
            assert_eq!(journal.kinetic_states().count(), 20);
            assert!(journal.kinetic_states().all(|(_, state)| state.running()));
        }
    }

    #[test]
    fn real_waterwheel_drives_a_geared_crusher_but_still_stops_when_power_is_exhausted() {
        let (mut chunks, mut journal) = fixture();
        let terrain = terrain();
        let wheel = [10, 4, 10];
        for (position, material) in [
            (wheel, 147),
            ([9, 4, 10], 151),
            ([9, 5, 11], 143),
            ([8, 5, 11], 146),
        ] {
            place(
                &mut chunks,
                &mut journal,
                position,
                material,
                Default::default(),
            );
        }
        let anchor = [6, 5, 11];
        let secondary = [7, 5, 11];
        for (position, peer, is_secondary) in
            [(anchor, secondary, false), (secondary, anchor, true)]
        {
            place(
                &mut chunks,
                &mut journal,
                position,
                99,
                BlockObjectState {
                    peer: Some(peer),
                    secondary: is_secondary,
                    ..Default::default()
                },
            );
        }
        journal.set_crusher(anchor, Default::default()).unwrap();
        journal
            .set_container(
                anchor,
                Inventory {
                    slots: vec![ItemStack {
                        item: ItemKind::Mineral(MineralItem::Coal),
                        count: 1,
                    }],
                    ..Default::default()
                },
            )
            .unwrap();
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([10, 2, 10], 3)
            .unwrap();
        journal
            .set_fluid(
                [10, 3, 10],
                FluidCell {
                    amount: 8,
                    source: false,
                    flow: [0, 1],
                },
            )
            .unwrap();
        let drive = wheel_drive(wheel, 0, &terrain, &chunks, &journal);
        assert_eq!(drive.capacity, kinetic::WATER_WHEEL_CAPACITY);
        assert_eq!(drive.rpm.abs(), kinetic::WATER_WHEEL_RPM);
        advance(
            &mut Runtime::default(),
            &terrain,
            &chunks,
            &mut journal,
            &HashSet::new(),
        );
        assert!(journal.crusher_driven(anchor));
        assert_eq!(
            journal.kinetic_state(anchor).rpm.abs(),
            2.0 * kinetic::WATER_WHEEL_RPM
        );
        super::super::crushers::process(anchor, 0.25, &chunks, &mut journal).unwrap();
        assert!(journal.crusher_at(anchor).unwrap().progress_seconds > 0.0);
        // Slower wheel RPM reduces each directly connected machine's demand.
        // Eight additional machines still exceed the preserved 128-unit supply.
        for x in (11..27).step_by(2) {
            let primary = [x, 4, 10];
            let secondary = [x + 1, 4, 10];
            for (position, peer, is_secondary) in
                [(primary, secondary, false), (secondary, primary, true)]
            {
                place(
                    &mut chunks,
                    &mut journal,
                    position,
                    99,
                    BlockObjectState {
                        peer: Some(peer),
                        secondary: is_secondary,
                        ..Default::default()
                    },
                );
            }
            journal.set_crusher(primary, Default::default()).unwrap();
            journal
                .set_container(primary, Inventory::default())
                .unwrap();
        }
        advance(
            &mut Runtime::default(),
            &terrain,
            &chunks,
            &mut journal,
            &HashSet::new(),
        );
        let state = journal.kinetic_state(wheel);
        assert!(state.stress > state.capacity);
        assert!(state.overloaded);
        assert_eq!(journal.kinetic_state(anchor).rpm, 0.0);
        assert!(!journal.crusher_driven(anchor));
        journal
            .set_fluid(
                [10, 3, 10],
                FluidCell {
                    amount: 8,
                    source: true,
                    flow: [0, 0],
                },
            )
            .unwrap();
        assert_eq!(
            wheel_drive(wheel, 0, &terrain, &chunks, &journal),
            kinetic::SourceDrive::default()
        );
    }

    #[test]
    fn waterwheel_side_falls_reverse_but_centered_falls_and_dry_clearance_do_not_drive() {
        let terrain = terrain();
        for facing in 0..4 {
            let (mut chunks, mut journal) = fixture();
            let position = [7, 3, 7];
            place(
                &mut chunks,
                &mut journal,
                position,
                kinetic::WATER_WHEEL,
                BlockObjectState {
                    facing,
                    ..Default::default()
                },
            );
            let axis = kinetic::axis(facing);
            let tangent = [-axis[2], 0, axis[0]];
            let positive = kinetic::offset(position, tangent);
            let negative = kinetic::offset(position, tangent.map(|v| -v));
            let still = FluidCell {
                amount: 8,
                source: true,
                flow: [0, 0],
            };
            let falling = FluidCell {
                source: false,
                ..still
            };
            journal.set_fluid(position, still).unwrap();
            assert_eq!(wheel_rpm(position, facing, &terrain, &chunks, &journal), 0);
            journal.set_fluid(positive, falling).unwrap();
            assert!(terra_world::water::falling_fluid_at(
                &terrain, &chunks, &journal, positive
            ));
            let forward = wheel_rpm(position, facing, &terrain, &chunks, &journal);
            assert_ne!(forward, 0);
            assert_eq!(
                wheel_drive(position, facing, &terrain, &chunks, &journal).capacity,
                23040
            );
            journal.set_fluid(positive, FluidCell::default()).unwrap();
            journal.set_fluid(negative, falling).unwrap();
            assert_eq!(
                wheel_rpm(position, facing, &terrain, &chunks, &journal),
                -forward
            );
            journal.set_fluid(positive, falling).unwrap();
            assert_eq!(wheel_rpm(position, facing, &terrain, &chunks, &journal), 0);
            for p in [positive, negative] {
                journal.set_fluid(p, still).unwrap();
            }
            assert_eq!(wheel_rpm(position, facing, &terrain, &chunks, &journal), 0);
            for p in [positive, negative] {
                journal.set_fluid(p, FluidCell::default()).unwrap();
            }
            journal.set_fluid(position, falling).unwrap();
            assert!(terra_world::water::falling_fluid_at(
                &terrain, &chunks, &journal, position
            ));
            assert_eq!(wheel_rpm(position, facing, &terrain, &chunks, &journal), 0);
            journal.set_fluid(position, FluidCell::default()).unwrap();
            journal.set_fluid(positive, FluidCell::default()).unwrap();
            journal
                .set_fluid(kinetic::offset(positive, tangent), falling)
                .unwrap();
            assert_eq!(wheel_rpm(position, facing, &terrain, &chunks, &journal), 0);
        }
    }

    #[test]
    fn waterwheel_side_contact_requires_the_neighbor_fluid_section_to_be_loaded() {
        let (mut chunks, mut journal) = fixture();
        let terrain = terrain();
        let position = [7, 3, 15];
        let edge = [7, 3, 16];
        place(
            &mut chunks,
            &mut journal,
            position,
            kinetic::WATER_WHEEL,
            Default::default(),
        );
        journal
            .set_fluid(
                position,
                FluidCell {
                    amount: 8,
                    source: true,
                    flow: [0, 0],
                },
            )
            .unwrap();
        journal
            .set_fluid(
                edge,
                FluidCell {
                    amount: 8,
                    source: false,
                    flow: [0, 0],
                },
            )
            .unwrap();
        assert_eq!(wheel_rpm(position, 0, &terrain, &chunks, &journal), 0);
        let owner = global_to_section(edge).0;
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data: BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        assert_ne!(wheel_rpm(position, 0, &terrain, &chunks, &journal), 0);
        chunks.loaded.remove(&owner);
        assert_eq!(wheel_rpm(position, 0, &terrain, &chunks, &journal), 0);
    }
    #[test]
    fn conveyor_moves_existing_stack_once_and_stops_for_missing_power_and_walls() {
        let (mut chunks, mut journal) = fixture();
        let belt = [5, 1, 5];
        place(
            &mut chunks,
            &mut journal,
            belt,
            kinetic::CONVEYOR,
            Default::default(),
        );
        let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
        journal.replace_kinetic_states(BTreeMap::from([(
            belt,
            KineticState {
                rpm: 30.0,
                overloaded: false,
                ..Default::default()
            },
        )]));
        journal.gameplay_mut().drop_stack(
            [5.5, 1.825, 5.5],
            ItemStack {
                item: ItemKind::Block(3),
                count: 17,
            },
            [0.0; 3],
            0.0,
        );
        let mut settled = journal.gameplay().drops[0].clone();
        for _ in 0..20 {
            super::super::gameplay::step_drop_with_context(
                &mut settled,
                &chunks,
                &journal,
                &properties,
                0.02,
            );
        }
        let support = 1.0 + f64::from(kinetic::CONVEYOR_TOP) + 0.12;
        assert!((support - 0.01..=support + 0.09).contains(&settled.position[1]));
        journal.gameplay_mut().drops[0] = settled;
        let original = journal.gameplay().drops[0].clone();
        let mut runtime = Runtime::default();
        transport(&mut runtime, &chunks, &mut journal, &properties, 0.1);
        let moved = journal.gameplay().drops[0].clone();
        assert!(moved.position[2] > original.position[2]);
        assert_eq!((moved.id, moved.stack), (original.id, original.stack));
        assert_eq!(journal.gameplay().drops.len(), 1);
        journal.replace_kinetic_states(Default::default());
        transport(&mut runtime, &chunks, &mut journal, &properties, 0.1);
        assert_eq!(journal.gameplay().drops[0].position, moved.position);
        journal.replace_kinetic_states(BTreeMap::from([(
            belt,
            KineticState {
                rpm: 30.0,
                overloaded: false,
                ..Default::default()
            },
        )]));
        journal.gameplay_mut().drops[0].position = [5.5, support + 0.01, 5.85];
        chunks
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([5, 1, 6], 3)
            .unwrap();
        transport(&mut runtime, &chunks, &mut journal, &properties, 0.1);
        assert_eq!(
            journal.gameplay().drops[0].position,
            [5.5, support + 0.01, 5.85]
        );
    }

    #[test]
    fn authoritative_reduction_and_speedup_drive_a_seam_crusher_with_the_128_stress_crank() {
        let (mut chunks, mut journal) = fixture();
        let terrain = terrain();
        let crank = [13, 5, 4];
        let input = [14, 5, 4];
        let output = [14, 6, 5];
        let anchor = [15, 6, 5];
        let partner = [16, 6, 5];
        place(
            &mut chunks,
            &mut journal,
            input,
            kinetic::IRON_GEAR,
            Default::default(),
        );
        place(
            &mut chunks,
            &mut journal,
            output,
            kinetic::IRON_LARGE_GEAR,
            Default::default(),
        );
        place(
            &mut chunks,
            &mut journal,
            crank,
            100,
            BlockObjectState {
                facing: 2,
                peer: Some(input),
                ..Default::default()
            },
        );
        place(
            &mut chunks,
            &mut journal,
            anchor,
            99,
            BlockObjectState {
                peer: Some(partner),
                ..Default::default()
            },
        );
        place(
            &mut chunks,
            &mut journal,
            partner,
            99,
            BlockObjectState {
                peer: Some(anchor),
                secondary: true,
                ..Default::default()
            },
        );
        journal.set_crusher(anchor, Default::default()).unwrap();
        let mut runtime = Runtime::default();
        let cranks = HashSet::from([crank]);
        advance(&mut runtime, &terrain, &chunks, &mut journal, &cranks);
        assert!(journal.crusher_driven(anchor));
        assert_eq!(journal.kinetic_state(anchor).rpm, 15.0);
        let section = global_to_section(partner).0;
        let secondary = chunks.loaded.remove(&section).unwrap();
        advance(&mut runtime, &terrain, &chunks, &mut journal, &cranks);
        assert!(!journal.crusher_driven(anchor));
        chunks.loaded.insert(section, secondary);
        place(
            &mut chunks,
            &mut journal,
            input,
            kinetic::IRON_LARGE_GEAR,
            Default::default(),
        );
        place(
            &mut chunks,
            &mut journal,
            output,
            kinetic::IRON_GEAR,
            Default::default(),
        );
        advance(&mut runtime, &terrain, &chunks, &mut journal, &cranks);
        assert!(journal.crusher_driven(anchor));
        assert_eq!(journal.kinetic_state(anchor).rpm, 60.0);
        assert!(!journal.kinetic_state(anchor).overloaded);
    }

    #[test]
    fn unrelated_off_belt_drops_do_not_consume_transport_budget_or_reduce_belt_speed() {
        let (mut chunks, mut journal) = fixture();
        let belts = [[5, 1, 5], [5, 1, 6]];
        for p in belts {
            place(
                &mut chunks,
                &mut journal,
                p,
                kinetic::CONVEYOR,
                Default::default(),
            );
        }
        journal.replace_kinetic_states(
            belts
                .into_iter()
                .map(|p| {
                    (
                        p,
                        KineticState {
                            rpm: 30.0,
                            overloaded: false,
                            ..Default::default()
                        },
                    )
                })
                .collect(),
        );
        let stack = ItemStack {
            item: ItemKind::Block(3),
            count: 1,
        };
        for _ in 0..1024 {
            assert!(
                journal
                    .gameplay_mut()
                    .drop_stack([2.5, 1.4, 2.5], stack, [0.0; 3], 0.0)
            );
        }
        assert!(journal.gameplay_mut().drop_stack(
            [5.5, 1.0 + f64::from(kinetic::CONVEYOR_TOP) + 0.15, 5.2],
            stack,
            [0.0; 3],
            0.0,
        ));
        let target_id = journal.gameplay().drops.last().unwrap().id;
        let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
        let mut runtime = Runtime::default();
        for _ in 0..10 {
            transport(&mut runtime, &chunks, &mut journal, &properties, 0.1);
        }
        let target = journal.gameplay().drops.last().unwrap();
        assert_eq!((target.id, target.stack), (target_id, stack));
        assert!((target.position[2] - 6.2).abs() < 0.000001);
        assert_eq!(journal.gameplay().drops.len(), 1025);
        assert_eq!(journal.gameplay().drops[0].position, [2.5, 1.4, 2.5]);
    }
    #[test]
    fn steam_dock_drives_a_real_loaded_pump_and_stops_when_the_boiler_is_empty() {
        use terra_core::{
            PlayerUuid,
            train_power::SteamState,
            trains::{TrainPart, Vehicle, VehicleKind},
        };
        let (mut chunks, mut journal) = fixture();
        let mut parts =
            terra_core::train_blueprint::template([8, 2, 4], 0, VehicleKind::Locomotive)
                .into_iter()
                .map(|p| (p.cell.map(|v| v as i16), (p.material, p.facing)))
                .collect::<BTreeMap<_, _>>();
        for z in 4..10 {
            parts.insert([8, 4, z], (219, 0));
        }
        for (cell, material) in [
            ([8, 3, 7], 218),
            ([8, 4, 10], 229),
            ([9, 4, 10], 230),
            ([8, 3, 8], 254),
        ] {
            parts.insert(cell, (material, 0));
        }
        let parts = parts
            .into_iter()
            .map(|(offset, (material, facing))| TrainPart {
                offset,
                material,
                facing,
                wood: Default::default(),
            })
            .collect::<Vec<_>>();
        let cargo = parts
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(p.material, 29 | 216 | 218))
            .map(|(index, _)| (index as u16, Inventory::default()))
            .collect();
        let car = Vehicle {
            id: 1,
            owner: PlayerUuid::from_u128(1),
            name: String::new(),
            kind: VehicleKind::Locomotive,
            anchor: [0; 3],
            parts,
            cargo,
            blocks: Default::default(),
            payloads: Default::default(),
            couplings: [None; 2],
            revision: 0,
            motion: Default::default(),
            burn_seconds: 0.0,
            steam: SteamState {
                water: 1000.0,
                pressure_bar: 9.0,
            },
        };
        assert!(car.valid());
        assert!(terra_core::train_power::steam_installed(&car.parts));
        journal.gameplay_mut().trains.insert(car).unwrap();
        for (cell, material) in [
            ([12, 2, 6], 217),
            ([13, 2, 6], kinetic::IRON_SHAFT),
            ([14, 2, 6], terra_core::plumbing::PUMP),
        ] {
            place(
                &mut chunks,
                &mut journal,
                cell,
                material,
                Default::default(),
            );
        }
        let mut runtime = Runtime::default();
        advance(
            &mut runtime,
            &terrain(),
            &chunks,
            &mut journal,
            &HashSet::new(),
        );
        let pump = journal.kinetic_state([14, 2, 6]);
        assert!(pump.running() && pump.stress > 0.0, "{pump:?}");
        let before = journal.gameplay().trains.vehicles[&1].steam;
        terra_world::train_services::advance(&mut Default::default(), &chunks, &mut journal, 0.2)
            .unwrap();
        assert!(journal.gameplay().trains.vehicles[&1].steam.water < before.water);
        journal
            .gameplay_mut()
            .trains
            .vehicles
            .get_mut(&1)
            .unwrap()
            .steam
            .water = 0.0;
        advance(
            &mut runtime,
            &terrain(),
            &chunks,
            &mut journal,
            &HashSet::new(),
        );
        assert!(!journal.kinetic_state([14, 2, 6]).running());
    }

    #[test]
    fn malformed_attachments_and_visual_states_cannot_create_authoritative_power() {
        let (mut chunks, mut journal) = fixture();
        let terrain = terrain();
        place(
            &mut chunks,
            &mut journal,
            [5, 3, 5],
            kinetic::IRON_SHAFT,
            Default::default(),
        );
        place(
            &mut chunks,
            &mut journal,
            [7, 3, 5],
            100,
            BlockObjectState {
                peer: Some([5, 3, 5]),
                ..Default::default()
            },
        );
        assert_eq!(journal.crank_anchor([7, 3, 5]), None);
        let mut runtime = Runtime::default();
        advance(
            &mut runtime,
            &terrain,
            &chunks,
            &mut journal,
            &HashSet::from([[7, 3, 5], [5, 3, 5]]),
        );
        assert!(!journal.kinetic_state([5, 3, 5]).running());
        assert!(
            !KineticVisual {
                position: [0; 3],
                rpm: 257.0,
                overloaded: false,
                ..Default::default()
            }
            .valid()
        );
        assert!(
            !KineticVisual {
                position: [0; 3],
                rpm: 30.0,
                overloaded: true,
                ..Default::default()
            }
            .valid()
        );
        assert!(
            !KineticVisual {
                position: [1_000_001, 0, 0],
                rpm: 0.0,
                overloaded: false,
                ..Default::default()
            }
            .valid()
        );
    }
}
