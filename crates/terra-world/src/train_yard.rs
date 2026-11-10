//! Shared, bounded assembly-berth discovery for the workshop and its track markings.
use crate::{edits::EditJournal, streaming::ChunkStore};
use terra_core::{
    train_blueprint::{self, TrainBlueprint},
    trains::VehicleKind,
};
/// Search only the bounded station neighborhood, and validate both wheelsets
/// against the same complete rails that the assembly authority accepts.
pub fn blueprint(
    chunks: &ChunkStore,
    journal: &EditJournal,
    previous: Option<&TrainBlueprint>,
    station: [i64; 3],
    kind: VehicleKind,
    wheelbase: u8,
    width: u8,
) -> Option<TrainBlueprint> {
    if let Some(previous) = previous.filter(|guide| guide.station == Some(station))
        && let Some(chassis) = previous.cells.iter().find(|part| part.material == 210)
        && train_blueprint::building_template_with_width(
            chassis.cell,
            chassis.facing,
            kind,
            wheelbase,
            width,
        )
        .iter()
        .filter(|part| part.material == 211)
        .all(|part| {
            crate::trains::bogie_supported_with_facing(chunks, journal, part.cell, part.facing)
        })
    {
        return Some(TrainBlueprint {
            station: Some(station),
            kind,
            cells: train_blueprint::building_template_with_width(
                chassis.cell,
                chassis.facing,
                kind,
                wheelbase,
                width,
            ),
        });
    }
    let mut candidates = Vec::new();
    let mut considered = std::collections::BTreeSet::new();
    let mut candidate = |chassis, facing, distance| {
        if !considered.insert((chassis, facing)) {
            return;
        }
        let skeleton = train_blueprint::template_with_wheelbase(chassis, facing, kind, wheelbase);
        if skeleton.is_empty() {
            return;
        }
        if skeleton
            .iter()
            .filter(|part| part.material == 211)
            .all(|part| {
                crate::trains::bogie_supported_with_facing(chunks, journal, part.cell, part.facing)
            })
        {
            let matched = skeleton
                .iter()
                .filter(|part| {
                    train_blueprint::matches(
                        **part,
                        chunks.block_at(part.cell),
                        journal
                            .object_state(part.cell)
                            .map_or(0, |state| state.facing),
                    )
                })
                .count();
            candidates.push((
                chunks.block_at(chassis) != Some(210),
                std::cmp::Reverse(matched),
                distance,
                facing,
                train_blueprint::building_template_with_width(
                    chassis, facing, kind, wheelbase, width,
                ),
            ));
        }
    };
    let mut owners = std::collections::BTreeSet::new();
    for x in -8_i64..=8 {
        for y in -4_i64..=4 {
            for z in -8_i64..=8 {
                let cell = [station[0] + x, station[1] + y, station[2] + z];
                if chunks.block_at(cell) == Some(210) {
                    for facing in 0..4 {
                        candidate(cell, facing, x * x + y * y + z * z);
                    }
                }
                if !chunks
                    .block_at(cell)
                    .is_some_and(terra_core::rails::is_rail)
                {
                    continue;
                }
                let Some(piece) = crate::rails::owner_at(journal, cell) else {
                    continue;
                };
                if !owners.insert(piece.anchor) || !crate::rails::complete(journal, piece) {
                    continue;
                }
                // A drawn module may contain a straight berth followed by a bend.
                // Select local axle positions; never reject its entire owner curve.
                for path in piece.active_paths() {
                    let length = path.length();
                    let steps = (length * 2.0).ceil() as usize;
                    for step in 0..=steps {
                        let sample =
                            path.sample_distance(length * step as f64 / steps.max(1) as f64);
                        if sample.tangent[1].abs() > 0.02
                            || !crate::rails::local_loaded(chunks, piece, sample.position)
                        {
                            continue;
                        }
                        let along_z = sample.tangent[2].abs() >= sample.tangent[0].abs();
                        let facing = if along_z {
                            if sample.tangent[2] >= 0.0 { 0 } else { 2 }
                        } else if sample.tangent[0] >= 0.0 {
                            1
                        } else {
                            3
                        };
                        let rail = sample.position.map(|v| v.floor() as i64);
                        if (0..3).any(|i| rail[i].abs_diff(station[i]) > if i == 1 { 4 } else { 8 })
                        {
                            continue;
                        }
                        for facing in [facing, (facing + 2) % 4] {
                            let chassis = [rail[0], rail[1] + 2, rail[2]];
                            candidate(
                                chassis,
                                facing,
                                (0..3).map(|i| (chassis[i] - station[i]).pow(2)).sum(),
                            );
                        }
                    }
                }
            }
        }
    }
    candidates.sort_by_key(|(empty, matched, distance, facing, cells)| {
        (*empty, *matched, *distance, *facing, cells[0].cell)
    });
    candidates
        .into_iter()
        .next()
        .map(|(_, _, _, _, cells)| TrainBlueprint {
            station: Some(station),
            kind,
            cells,
        })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct YardTrack {
    pub path: terra_core::rails::Path,
    pub start: f64,
    pub end: f64,
}
/// Choose the nearest loaded track locally; its canonical owner may be far away.
pub fn track(chunks: &ChunkStore, journal: &EditJournal, station: [i64; 3]) -> Option<YardTrack> {
    let mut owners = std::collections::BTreeSet::new();
    let mut best: Option<(f64, YardTrack)> = None;
    for x in -8..=8 {
        for y in -3..=3 {
            for z in -8..=8 {
                let cell = [station[0] + x, station[1] + y, station[2] + z];
                if !chunks
                    .block_at(cell)
                    .is_some_and(terra_core::rails::is_rail)
                {
                    continue;
                }
                let Some(piece) = crate::rails::owner_at(journal, cell) else {
                    continue;
                };
                if !owners.insert(piece.anchor) || !crate::rails::complete(journal, piece) {
                    continue;
                }
                for path in piece.active_paths() {
                    let length = path.length();
                    let steps = (length * 2.0).ceil() as usize;
                    for step in 0..=steps {
                        let distance = length * step as f64 / steps.max(1) as f64;
                        let sample = path.sample_distance(distance);
                        if sample.tangent[1].abs() > 0.02
                            || !crate::rails::local_loaded(chunks, piece, sample.position)
                        {
                            continue;
                        }
                        let squared = (0..3)
                            .map(|i| (sample.position[i] - station[i] as f64 - 0.5).powi(2))
                            .sum::<f64>();
                        if squared <= 64.0 && best.as_ref().is_none_or(|(old, _)| squared < *old) {
                            best = Some((
                                squared,
                                YardTrack {
                                    path,
                                    start: (distance - 12.0).max(0.0),
                                    end: (distance + 12.0).min(length),
                                },
                            ));
                        }
                    }
                }
            }
        }
    }
    best.map(|(_, track)| track)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{edits::BlockObjectState, streaming::LoadedSection};
    use terra_core::{
        PlayerUuid,
        rails::{CurveSpec, Piece},
    };
    use terra_voxel::{BlockSection, global_to_section};
    fn block(chunks: &mut ChunkStore, cell: [i64; 3], material: u16) {
        let (section, local) = global_to_section(cell);
        chunks
            .loaded
            .entry(section)
            .or_insert_with(|| LoadedSection {
                data: BlockSection::empty(section).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            })
            .data
            .set_block(local, material)
            .unwrap();
    }
    #[test]
    fn an_extended_curve_offers_its_nearby_straight_berth_and_a_real_build_plan() {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for x in -23..=-8 {
            for y in -1..=10 {
                for z in -23..=36 {
                    block(&mut chunks, [x, y, z], 0);
                }
            }
        }
        let piece = Piece::new_curve(
            [-17, 0, -17],
            CurveSpec::new([[8, 2, 0], [8, 2, 256], [8, 2, 512], [40, 2, 768]]).unwrap(),
        )
        .unwrap();
        for cell in crate::rails::cells(piece) {
            block(&mut chunks, cell, piece.material);
            journal.record(cell, piece.material).unwrap();
            journal
                .set_object_state(cell, Some(crate::rails::object_state(piece, cell)))
                .unwrap();
        }
        let station = [-14, 0, -15];
        block(&mut chunks, station, 217);
        journal.record(station, 217).unwrap();
        journal
            .set_object_state(station, Some(BlockObjectState::default()))
            .unwrap();
        let guide = blueprint(
            &chunks,
            &journal,
            None,
            station,
            VehicleKind::Locomotive,
            8,
            3,
        )
        .expect("a distant owner and later bend must not hide the nearby usable berth");
        let chassis = guide
            .cells
            .iter()
            .find(|part| part.material == 210)
            .unwrap();
        let plan = crate::trains::plan_blueprint(
            &chunks,
            &journal,
            crate::trains::BuildRequest {
                actor: PlayerUuid::from_u128(1),
                station,
                chassis: chassis.cell,
                facing: chassis.facing,
                kind: VehicleKind::Locomotive,
                wheelbase: 8,
                width: 3,
                creative: true,
            },
        )
        .unwrap();
        assert!(!plan.cells.is_empty());
        let marked = track(&chunks, &journal, station).unwrap();
        assert_eq!(marked.path.piece.anchor, piece.anchor);
        block(&mut chunks, station, 0);
        assert!(crate::train_services::docks(&chunks, &journal).is_empty());
    }
}
