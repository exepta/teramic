//! Live rail-bed speed limits; terrain edits never leave a stale speed bonus.
use super::{Cell, complete, owner_at};
use crate::{edits::EditJournal, streaming::ChunkStore};
use terra_core::rails::Route;

pub const BALLAST_SPEED_MULTIPLIER: f64 = 1.5;

fn ballast_floor(journal: &EditJournal, cell: Cell, material: u16) -> bool {
    use terra_core::construction;
    if !super::placement::supports_floor(journal, cell, material) {
        return false;
    }
    if construction::is_slab(material) {
        let facing = journal.object_state(cell).unwrap_or_default().facing;
        // A mixed slab's stored base material may belong to its lower half.
        // Every real top-face patch must be ballast, including vertical splits.
        let mut top = false;
        for (material, shape) in construction::slab_parts(material, facing) {
            if shape.maximum[1] >= 1.0 - 1e-9 {
                top = true;
                if !construction::is_ballast(material) {
                    return false;
                }
            }
        }
        top
    } else {
        construction::is_ballast(material)
    }
}

/// A module receives its bonus only when every actual bearing has a complete
/// ballast top face. Missing terrain or incomplete bedding never adds speed.
pub fn speed_multiplier(chunks: &ChunkStore, journal: &EditJournal, rail: Cell) -> f64 {
    let Some(piece) = owner_at(journal, rail).filter(|piece| complete(journal, *piece)) else {
        return 1.0;
    };
    let supports = piece.supports();
    if !supports.is_empty()
        && supports.iter().all(|point| {
            let cell =
                std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis]));
            chunks
                .block_at(cell)
                .or_else(|| journal.block(cell))
                .is_some_and(|material| ballast_floor(journal, cell, material))
        })
    {
        BALLAST_SPEED_MULTIPLIER
    } else {
        1.0
    }
}

/// Apply one live rail-bed modifier to an unmodified positive finite limit.
/// A missing or incomplete module cannot supply an authoritative speed limit.
pub fn speed_limit(
    chunks: &ChunkStore,
    journal: &EditJournal,
    rail: Cell,
    base_speed: f64,
) -> Option<f64> {
    if !base_speed.is_finite() || base_speed <= 0.0 {
        return None;
    }
    owner_at(journal, rail).filter(|piece| complete(journal, *piece))?;
    let speed = base_speed * speed_multiplier(chunks, journal, rail);
    (speed.is_finite() && speed > 0.0).then_some(speed)
}

/// Evaluate live active route legs without changing geometric distance or
/// fabricating movement for the future train system. Stale switch paths fail closed.
pub fn travel_seconds(
    chunks: &ChunkStore,
    journal: &EditJournal,
    route: &Route,
    base_speed: f64,
) -> Option<f64> {
    if !base_speed.is_finite()
        || base_speed <= 0.0
        || route.legs.len() > terra_core::rails::MAX_ROUTE_VISITS
    {
        return None;
    }
    let mut seconds = 0.0;
    for leg in &route.legs {
        let piece = owner_at(journal, leg.anchor).filter(|piece| complete(journal, *piece))?;
        let path = piece
            .active_paths()
            .into_iter()
            .find(|path| path.index == leg.path)?;
        seconds += path.length() / speed_limit(chunks, journal, leg.anchor, base_speed)?;
    }
    seconds.is_finite().then_some(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{edits::BlockObjectState, rails, streaming::LoadedSection};
    use terra_core::{
        construction,
        rails::{Endpoint, Piece, STRAIGHT, SWITCH_LEFT},
    };
    use terra_voxel::{BlockSection, global_to_section};

    fn set(chunks: &mut ChunkStore, cell: Cell, material: u16) {
        let (owner, local) = global_to_section(cell);
        chunks
            .loaded
            .entry(owner)
            .or_insert_with(|| LoadedSection {
                data: BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            })
            .data
            .set_block(local, material)
            .unwrap();
    }
    fn floor(
        chunks: &mut ChunkStore,
        journal: &mut EditJournal,
        cell: Cell,
        material: u16,
        facing: u8,
    ) {
        set(chunks, cell, material);
        journal.record(cell, material).unwrap();
        journal
            .set_object_state(
                cell,
                construction::is_shape(material).then_some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    fn install(piece: Piece, material: u16, facing: u8) -> (ChunkStore, EditJournal, Vec<Cell>) {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for cell in rails::cells(piece) {
            set(&mut chunks, cell, 0);
        }
        let bearings = piece
            .supports()
            .iter()
            .map(|point| {
                std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis]))
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        assert!(!bearings.is_empty());
        for &cell in &bearings {
            floor(&mut chunks, &mut journal, cell, material, facing);
        }
        let plan = rails::plan(
            &chunks,
            &journal,
            piece.anchor,
            piece.material,
            piece.encoded_facing(),
        )
        .unwrap();
        assert!(rails::commit(&mut chunks, &mut journal, &plan).unwrap());
        assert_eq!(rails::owner_at(&journal, piece.anchor), Some(piece));
        assert!(rails::complete(&journal, piece));
        (chunks, journal, bearings)
    }
    fn route(journal: &EditJournal, piece: Piece, port: u8) -> Route {
        rails::network(journal)
            .unwrap()
            .route(
                Endpoint {
                    anchor: piece.anchor,
                    port: 0,
                },
                Endpoint {
                    anchor: piece.anchor,
                    port,
                },
                64,
            )
            .unwrap()
    }

    #[test]
    fn real_gravel_and_all_bearing_ballast_forms_apply_one_live_fifty_percent_bonus() {
        for (material, facing) in [(42, 0), (206, 0), (207, 4), (208, 4)] {
            let piece = Piece::new([-17, 16, -17], STRAIGHT, 0).unwrap();
            let (mut chunks, mut journal, bearings) = install(piece, material, facing);
            let route = route(&journal, piece, 1);
            let geometric_length = route.length();
            assert_eq!(speed_multiplier(&chunks, &journal, piece.anchor), 1.5);
            assert_eq!(
                speed_limit(&chunks, &journal, piece.anchor, 8.0),
                Some(12.0)
            );
            assert!(
                (travel_seconds(&chunks, &journal, &route, 8.0).unwrap() - 4.0 / 12.0).abs() < 1e-9
            );
            assert_eq!(route.length(), geometric_length);
            let secondary = rails::cells(piece)
                .into_iter()
                .find(|cell| *cell != piece.anchor)
                .unwrap();
            assert_eq!(speed_limit(&chunks, &journal, secondary, 8.0), Some(12.0));
            floor(&mut chunks, &mut journal, bearings[0], 3, 0);
            assert_eq!(speed_multiplier(&chunks, &journal, piece.anchor), 1.0);
            assert_eq!(speed_limit(&chunks, &journal, piece.anchor, 8.0), Some(8.0));
            assert!(
                (travel_seconds(&chunks, &journal, &route, 8.0).unwrap() - 4.0 / 8.0).abs() < 1e-9
            );
        }
    }

    #[test]
    fn actual_top_material_and_support_pose_control_the_bonus_without_a_cached_guess() {
        let piece = Piece::new([3, 1, 3], STRAIGHT, 0).unwrap();
        let (mut chunks, mut journal, bearings) = install(piece, 206, 0);
        for (material, facing, expected) in [
            (0, 0, 1.0),
            (207, 0, 1.0),
            (207, 8, 1.0),
            (207, 4, 1.5),
            (208, 0, 1.0),
            (208, 5, 1.0),
            (208, 4, 1.5),
            (208, construction::DOUBLE_SLAB, 1.5),
            (208, construction::merged_slab(208, 0, 85).unwrap(), 1.0),
            (85, construction::merged_slab(85, 0, 208).unwrap(), 1.5),
            (208, construction::merged_slab(208, 5, 85).unwrap(), 1.0),
            (208, construction::merged_slab(208, 5, 208).unwrap(), 1.5),
        ] {
            floor(&mut chunks, &mut journal, bearings[0], material, facing);
            assert_eq!(
                speed_multiplier(&chunks, &journal, piece.anchor),
                expected,
                "{material}/{facing}"
            );
        }
        let unknown = global_to_section(bearings[0]).0;
        chunks.loaded.remove(&unknown);
        // This support has no saved stone after removal: absence cannot infer ballast.
        journal.record(bearings[0], 0).unwrap();
        assert_eq!(speed_multiplier(&chunks, &journal, piece.anchor), 1.0);
    }

    #[test]
    fn incomplete_modules_invalid_limits_and_inactive_switch_routes_fail_closed() {
        let piece = Piece::new([-17, 16, -17], SWITCH_LEFT, 0).unwrap();
        let (mut chunks, mut journal, _) = install(piece, 206, 0);
        let main = route(&journal, piece, 1);
        assert!((travel_seconds(&chunks, &journal, &main, 8.0).unwrap() - 8.0 / 12.0).abs() < 1e-9);
        for speed in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::MAX] {
            assert_eq!(speed_limit(&chunks, &journal, piece.anchor, speed), None);
            assert_eq!(travel_seconds(&chunks, &journal, &main, speed), None);
        }
        assert_eq!(speed_limit(&chunks, &journal, [500; 3], 8.0), None);
        let toggle = rails::toggle_switch_plan(&chunks, &journal, piece.anchor).unwrap();
        assert!(rails::commit(&mut chunks, &mut journal, &toggle).unwrap());
        assert_eq!(travel_seconds(&chunks, &journal, &main, 8.0), None);
        let switched = Piece::new(piece.anchor, SWITCH_LEFT, 4).unwrap();
        let branch = route(&journal, switched, 2);
        let length = switched.active_paths()[0].length();
        assert!(
            (travel_seconds(&chunks, &journal, &branch, 8.0).unwrap() - length / 12.0).abs() < 1e-9
        );
        let lost = rails::cells(piece)
            .into_iter()
            .find(|cell| *cell != piece.anchor)
            .unwrap();
        set(&mut chunks, lost, 0);
        journal.record(lost, 0).unwrap();
        journal.set_object_state(lost, None).unwrap();
        assert_eq!(speed_multiplier(&chunks, &journal, piece.anchor), 1.0);
        assert_eq!(speed_limit(&chunks, &journal, piece.anchor, 8.0), None);
        assert_eq!(travel_seconds(&chunks, &journal, &branch, 8.0), None);
    }
}
