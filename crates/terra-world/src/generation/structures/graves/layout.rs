//! Revision-ten rooms, connected corridors and explicit stair flights.
use super::*;
use GraveRoomPurpose as P;

type RoomSpec = (P, [i64; 4], i64);

pub(in crate::generation::structures) fn adapted_layout(
    summary: &SiteSummary,
    initial_drop: i64,
) -> Option<GraveDungeonInfo> {
    let mut info = summary.grave_dungeon()?;
    let [x, y, z] = summary.center;
    info.entrance = [x, y, z - 12];
    info.route = vec![info.entrance];
    info.levels.clear();
    info.treasures.clear();
    let mut top = y;
    for index in 0..info.variant.levels() {
        let depth = if index == 0 {
            initial_drop
        } else {
            GRAVE_LEVEL_HEIGHT
        };
        let side = if index.is_multiple_of(2) { -8 } else { 8 };
        connect(&mut info.route, [x, top, z - 8]);
        connect(&mut info.route, [x + side, top, z - 8]);
        for step in 1..=depth {
            info.route.push([x + side, top - step, z - 8 + step]);
        }
        let floor = top - depth;
        let landing = [x + side, floor, z - 8 + depth];
        top = floor;
        info.levels.push(landing);
        let mirror = if (summary.id >> (index + 2)) & 1 == 0 {
            1
        } else {
            -1
        };
        let rooms: Vec<_> = specs(info.variant, index)
            .iter()
            .map(|&(purpose, [left, north, right, south], height)| {
                let (left, right) = if mirror > 0 {
                    (left, right)
                } else {
                    (-right, -left)
                };
                GraveRoom {
                    purpose,
                    bounds: [
                        [x + left, floor, z + north],
                        [x + right, floor + height - 1, z + south],
                    ],
                }
            })
            .collect();
        connect(&mut info.route, [x, floor, landing[2]]);
        connect(&mut info.route, [x, floor, z]);
        for room in &rooms {
            visit(&mut info.route, room.center(), [x, floor, z], landing);
        }
        let primary = &rooms[0];
        let center = primary.center();
        // Mirrored side passages can cross a fixed corner. Select a free
        // alcove from the actual route, then connect its interaction position.
        let treasure = (primary.bounds[0][2] + 1..primary.bounds[1][2])
            .rev()
            .flat_map(|pz| {
                [primary.bounds[1][0] - 1, primary.bounds[0][0] + 1]
                    .into_iter()
                    .map(move |px| [px, floor, pz])
            })
            .find(|p| {
                !info
                    .route
                    .iter()
                    .any(|r| r[1] == floor && (r[0] - p[0]).abs() <= 1 && (r[2] - p[2]).abs() <= 1)
            })?;
        visit(
            &mut info.route,
            [
                treasure[0] - 2 * (treasure[0] - x).signum(),
                floor,
                treasure[2],
            ],
            [x, floor, z],
            landing,
        );
        info.treasures.push(treasure);
        info.floors.push(GraveFloor {
            rooms: rooms.clone(),
            landing,
            view_eye: [center[0] + 2, floor + 2, primary.bounds[0][2] + 2],
            view_target: [center[0], floor + 1, primary.bounds[1][2] - 1],
        });
    }
    Some(info)
}

fn visit(route: &mut Vec<[i64; 3]>, point: [i64; 3], spine: [i64; 3], landing: [i64; 3]) {
    // A long first descent can pass above a lateral doorway. Route around
    // its lower landing instead of crossing beneath the last low treads.
    let side = landing[0] - spine[0];
    let crosses = (point[0] - spine[0]) * side > 0
        && (point[0] - spine[0]).abs() >= side.abs()
        && (landing[2] - 4..=landing[2] + 1).contains(&point[2]);
    let crossing_z = if crosses { landing[2] + 3 } else { point[2] };
    let junction = [spine[0], spine[1], crossing_z];
    let corner = [point[0], spine[1], crossing_z];
    for target in [junction, corner, point, corner, junction, spine] {
        connect(route, target);
    }
}

fn specs(variant: GraveVariant, level: u8) -> &'static [RoomSpec] {
    match (variant, level) {
        (GraveVariant::Knight, 0) => &[
            (P::GuardHall, [-6, 5, 6, 14], 5),
            (P::Armory, [-21, 3, -13, 11], 4),
            (P::Ossuary, [13, 10, 20, 17], 4),
        ],
        (GraveVariant::Knight, 1) => &[
            (P::FamilyCrypt, [-4, 6, 4, 19], 4),
            (P::Ossuary, [-22, 2, -13, 8], 4),
            (P::FamilyCrypt, [12, 6, 21, 12], 4),
            (P::FamilyCrypt, [-21, 14, -12, 21], 4),
        ],
        (GraveVariant::Knight, _) => &[
            (P::BurialHall, [-10, 5, 10, 22], 6),
            (P::Armory, [-22, 4, -15, 11], 4),
            (P::Ossuary, [15, 14, 22, 21], 4),
        ],
        (GraveVariant::Priestess, 0) => &[
            (P::ProcessionalHall, [-5, 4, 5, 22], 6),
            (P::Reliquary, [-21, 8, -12, 15], 4),
            (P::Reliquary, [12, 8, 21, 15], 4),
        ],
        (GraveVariant::Priestess, 1) => &[
            (P::Cloister, [-7, 5, 7, 15], 5),
            (P::Reliquary, [-22, 2, -14, 9], 4),
            (P::Reliquary, [14, 13, 22, 21], 4),
        ],
        (GraveVariant::Priestess, 2) => &[
            (P::Reliquary, [-8, 11, 8, 20], 5),
            (P::Sanctum, [-22, 3, -12, 10], 5),
            (P::Sanctum, [12, 3, 22, 10], 5),
        ],
        (GraveVariant::Priestess, _) => &[
            (P::Sanctum, [-10, 4, 10, 22], 6),
            (P::Reliquary, [-22, 5, -15, 12], 4),
        ],
        (GraveVariant::Scholar, 0) => &[
            (P::Library, [-9, 5, 9, 17], 5),
            (P::Study, [-22, 3, -15, 10], 4),
            (P::Study, [15, 11, 22, 18], 4),
        ],
        (GraveVariant::Scholar, 1) => &[
            (P::Archive, [-5, 7, 5, 21], 4),
            (P::Archive, [-22, 2, -12, 9], 4),
            (P::Archive, [12, 11, 22, 18], 4),
        ],
        (GraveVariant::Scholar, 2) => &[
            (P::Study, [-6, 10, 6, 18], 5),
            (P::Library, [-22, 2, -11, 8], 5),
            (P::Reliquary, [13, 8, 22, 16], 4),
            (P::Study, [-21, 14, -13, 21], 4),
        ],
        (GraveVariant::Scholar, _) => &[
            (P::CodexVault, [-10, 6, 10, 21], 6),
            (P::Archive, [-22, 4, -15, 11], 4),
        ],
    }
}
