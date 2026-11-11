//! Inspect saved train clearance without opening a renderer or changing a world.
use std::{collections::BTreeSet, path::PathBuf};
use terra_core::trains::CouplerEnd;
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().ok_or("world-library directory required")?);
    let id = terra_core::WorldId(args.next().ok_or("world id required")?.parse()?);
    let inspect_builder = args.any(|arg| arg == "--builder");
    let library = terra_storage::WorldLibrary { root };
    let entry = library.load(id)?;
    let directory = library.path(id);
    let journal: EditJournal = terra_storage::load_edits(&directory, id)?;
    let generator =
        terra_world::SectionGenerator::new(terra_world::TerraGenerator::new(entry.manifest.world)?);
    let mut owners = BTreeSet::new();
    for car in journal.gameplay().trains.vehicles.values() {
        let bounds = car.model_bounds();
        let margin = if inspect_builder { 16 } else { 2 };
        let low = terra_voxel::global_to_section(bounds[0].map(|v| v.floor() as i64 - margin)).0;
        let high = terra_voxel::global_to_section(std::array::from_fn(|axis| {
            bounds[1][axis].ceil() as i64
                + if inspect_builder && axis == 1 {
                    32
                } else {
                    margin
                }
        }))
        .0;
        for x in low.x..=high.x {
            for y in low.y..=high.y {
                for z in low.z..=high.z {
                    owners.insert(terra_voxel::SectionPos { x, y, z });
                }
            }
        }
    }
    let mut chunks = ChunkStore::default();
    for owner in owners {
        let generated = generator.build(
            owner,
            journal.revision(owner),
            &journal.snapshot(owner)?,
            false,
        )?;
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data: generated.data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
    }
    let properties = Default::default();
    if let (Some(first), Some(second)) = (
        journal.gameplay().trains.vehicles.get(&1),
        journal.gameplay().trains.vehicles.get(&2),
    ) {
        let mut close = first.clone();
        let a = close.coupler_point(CouplerEnd::Rear).unwrap();
        let b = second.coupler_point(CouplerEnd::Front).unwrap();
        let direction = close.coupler_direction(CouplerEnd::Rear).unwrap();
        for axis in 0..3 {
            close.motion.translation[axis] += b[axis] - a[axis] - direction[axis];
        }
        let mut close_journal = journal.clone();
        close_journal
            .gameplay_mut()
            .trains
            .vehicles
            .insert(1, close.clone());
        for index in 0..close.parts.len() {
            if let Some((feet, yaw)) = close.seat_pose(index) {
                println!(
                    "nearby-cars coupling={:?} exit={:?}",
                    close.coupling_distance(CouplerEnd::Rear, second, CouplerEnd::Front),
                    terra_physics::safe_train_exit(
                        close.id,
                        feet,
                        yaw,
                        &chunks,
                        &close_journal,
                        generator.terrain(),
                        &properties
                    )
                );
            }
        }
    }
    for car in journal.gameplay().trains.vehicles.values() {
        println!(
            "CAR {} {:?} parts={} bounds={:?} motion={:?}",
            car.id,
            car.kind,
            car.parts.len(),
            car.model_bounds(),
            car.motion
        );
        println!(
            "clear={:?}",
            terra_world::train_motion::clear(&chunks, &journal, &properties, car)
        );
        if let Ok(network) = terra_world::rails::network(&journal) {
            if inspect_builder
                && let Some((module, _)) = car
                    .parts
                    .iter()
                    .enumerate()
                    .find(|(_, part)| part.material == terra_core::train_parts::RAIL_BUILDER)
            {
                println!(
                    "builder-clearance={:?}",
                    terra_world::rails::builder::plan_clearance(&chunks, &journal, car)
                        .map(|plan| (plan.layout().edits.len(), plan.drops().len()))
                );
                let front = car.world_point(car.parts[module].offset.map(|v| f64::from(v) + 0.5));
                println!("builder-module={:?} position={front:?}", car.parts[module]);
                for piece in network.records() {
                    for port in piece.ports() {
                        let endpoint = terra_core::rails::Endpoint {
                            anchor: piece.anchor,
                            port: port.index,
                        };
                        if network.connected_endpoint(endpoint).is_some()
                            || port
                                .position
                                .position()
                                .iter()
                                .zip(front)
                                .map(|(a, b)| (a - b).powi(2))
                                .sum::<f64>()
                                > 256.0
                        {
                            continue;
                        }
                        println!(
                            "builder-end={endpoint:?} port={port:?} in_reach={:?} plan={:?}",
                            terra_world::rails::builder::extension_in_reach(car, port),
                            terra_world::rails::builder::plan_for_vehicle(
                                &chunks,
                                &journal,
                                port,
                                Default::default(),
                                car,
                            )
                            .map(|plan| (
                                plan.layout().edits.len(),
                                plan.tracks(),
                                plan.ballast()
                            ))
                        );
                    }
                }
            }
            for throttle in [-1, 1] {
                let mut moving = car.clone();
                moving.motion.throttle = throttle;
                moving.motion.brake = false;
                for step in 0..60 {
                    match terra_world::train_motion::advance(
                        &chunks,
                        &journal,
                        &network,
                        &moving,
                        1.0 / 60.0,
                    ) {
                        Ok(next) => {
                            let clear = terra_world::train_motion::clear_swept(
                                &chunks,
                                &journal,
                                &properties,
                                &moving,
                                &next,
                            )
                            .map(|_| ());
                            if clear.is_err() || step == 59 {
                                println!(
                                    "advance throttle={throttle} step={step} delta={} clear={clear:?} reason={:?}",
                                    next.motion.wheel_distance_m - car.motion.wheel_distance_m,
                                    next.motion.stop_reason
                                );
                            }
                            moving = next;
                            if clear.is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            println!("advance throttle={throttle} step={step} error={error:?}");
                            break;
                        }
                    }
                }
            }
        }
        for index in 0..car.parts.len() {
            if let Some((feet, yaw)) = car.seat_pose(index) {
                let low = [feet[0] - 0.32, feet[1], feet[2] - 0.32];
                let high = [feet[0] + 0.32, feet[1] + 1.8, feet[2] + 0.32];
                for x in low[0].floor() as i64..=high[0].floor() as i64 {
                    for y in low[1].floor() as i64..=high[1].floor() as i64 {
                        for z in low[2].floor() as i64..=high[2].floor() as i64 {
                            if terra_world::interaction_shapes::space_intersects(
                                &chunks,
                                &journal,
                                &properties,
                                [x, y, z],
                                low,
                                high,
                            ) {
                                println!(
                                    "seat={index} initial terrain intersection cell={:?}",
                                    [x, y, z]
                                );
                            }
                        }
                    }
                }
                println!(
                    "exit part={index} feet={feet:?} target={:?}",
                    terra_physics::safe_train_exit(
                        car.id,
                        feet,
                        yaw,
                        &chunks,
                        &journal,
                        generator.terrain(),
                        &properties
                    )
                );
            }
        }
        let mut probes = 0;
        let mut obstructions = 0;
        for index in 0..car.parts.len() {
            for shape in car.transformed_shapes(index) {
                for x in shape.minimum[0].floor() as i64..=(shape.maximum[0] - 1e-7).floor() as i64
                {
                    for y in
                        shape.minimum[1].floor() as i64..=(shape.maximum[1] - 1e-7).floor() as i64
                    {
                        for z in shape.minimum[2].floor() as i64
                            ..=(shape.maximum[2] - 1e-7).floor() as i64
                        {
                            probes += 1;
                            let cell = [x, y, z];
                            let material = chunks.block_at(cell).unwrap_or(0);
                            if material != 0
                                && material != 27
                                && !terra_core::rails::is_rail(material)
                                && terra_world::interaction_shapes::space_intersects(
                                    &chunks,
                                    &journal,
                                    &properties,
                                    cell,
                                    shape.minimum,
                                    shape.maximum,
                                )
                            {
                                obstructions += 1;
                                if obstructions <= 16 {
                                    println!(
                                        "obstruction part={} material={} cell={cell:?} terrain={material}",
                                        index, car.parts[index].material
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("probes={probes} obstructions={obstructions}");
        for other in journal
            .gameplay()
            .trains
            .vehicles
            .values()
            .filter(|other| other.id > car.id)
        {
            for a in [CouplerEnd::Front, CouplerEnd::Rear] {
                for b in [CouplerEnd::Front, CouplerEnd::Rear] {
                    println!(
                        "pair {}:{a:?} {:?} to {}:{b:?} {:?} compatible={:?}",
                        car.id,
                        car.coupler_point(a),
                        other.id,
                        other.coupler_point(b),
                        car.coupling_distance(a, other, b)
                    );
                }
            }
        }
    }
    Ok(())
}
