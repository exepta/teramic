use super::*;
use crate::generation::structures::{GraveDungeonInfo, GraveRoom, GraveRoomPurpose as P};

pub(super) fn dungeon(b: &mut Builder<'_>, info: &GraveDungeonInfo) {
    let wall = match info.variant {
        GraveVariant::Knight => BRICK,
        GraveVariant::Priestess => 340,
        GraveVariant::Scholar => STONE,
    };
    for floor in &info.floors {
        for room in &floor.rooms {
            room_shell(b, room, wall);
        }
    }
    // Narrow, individually lined tunnels only. Natural rock remains untouched
    // between rooms, instead of filling their combined bounding box with stone.
    for p in &info.route {
        if p[1] >= b.summary.center[1] - 3 {
            continue;
        }
        b.fill(
            [p[0] - 2, p[1] - 1, p[2] - 1],
            [p[0] + 2, p[1] + 3, p[2] + 1],
            wall,
        );
    }
    // Restore rooms after tunnel lining crosses their open interiors.
    for floor in &info.floors {
        for room in &floor.rooms {
            b.fill(room.bounds[0], room.bounds[1], 0);
        }
    }
    for p in &info.route {
        for dx in -1..=1 {
            b.set([p[0] + dx, p[1] - 1, p[2]], DARK);
            b.fill([p[0] + dx, p[1], p[2]], [p[0] + dx, p[1] + 2, p[2]], 0);
        }
    }
    for pair in info.route.windows(2) {
        if pair[1][1] < pair[0][1] {
            let p = pair[0];
            for dx in -1..=1 {
                b.state([p[0] + dx, p[1] - 1, p[2]], 77, 2);
            }
        }
    }
    for (i, floor) in info.floors.iter().enumerate() {
        for room in &floor.rooms {
            decorate(b, room, info);
        }
        let treasure = info.treasures[i];
        b.object(
            treasure,
            377,
            Some(BlockObjectState::default()),
            Some(super::super::super::super::loot::inventory(
                BuildingRole::Ruin,
                b.seed.wrapping_add(i as u32 + 1),
            )),
        );
        b.summary.containers.push(treasure);
    }
}

fn room_shell(b: &mut Builder<'_>, room: &GraveRoom, wall: u16) {
    let [low, high] = room.bounds;
    b.fill(
        [low[0] - 1, low[1] - 1, low[2] - 1],
        [high[0] + 1, low[1] - 1, high[2] + 1],
        DARK,
    );
    b.fill(
        [low[0] - 1, high[1] + 1, low[2] - 1],
        [high[0] + 1, high[1] + 1, high[2] + 1],
        wall,
    );
    for x in [low[0] - 1, high[0] + 1] {
        b.fill([x, low[1], low[2] - 1], [x, high[1], high[2] + 1], wall);
    }
    for z in [low[2] - 1, high[2] + 1] {
        b.fill([low[0], low[1], z], [high[0], high[1], z], wall);
    }
    b.fill(low, high, 0);
}

fn decorate(b: &mut Builder<'_>, room: &GraveRoom, info: &GraveDungeonInfo) {
    let [low, high] = room.bounds;
    let reserved = |p: [i64; 3]| {
        info.route.iter().any(|r| {
            (r[0] - p[0]).abs() <= 1 && (r[2] - p[2]).abs() <= 1 && (r[1] - p[1]).abs() <= 2
        }) || info.treasures.contains(&p)
            || info
                .floors
                .iter()
                .any(|floor| floor.view_eye[0] == p[0] && floor.view_eye[2] == p[2])
    };
    let furnishing = match room.purpose {
        P::Library | P::Archive | P::Study | P::CodexVault => 378,
        P::GuardHall | P::Armory => 359,
        P::Ossuary => 375,
        P::FamilyCrypt | P::BurialHall => 367,
        P::ProcessionalHall | P::Cloister => 376,
        P::Reliquary | P::Sanctum => 395,
    };
    for x in (low[0] + 1..high[0]).step_by(4) {
        for z in [low[2] + 1, high[2] - 1] {
            let p = [x, low[1], z];
            if !reserved(p) {
                b.set(p, furnishing);
            }
        }
    }
    for x in [low[0], high[0]] {
        for z in [low[2], high[2]] {
            for y in low[1]..=high[1] {
                let p = [x, y, z];
                if !reserved(p) {
                    b.set(p, if y == high[1] { CAPITAL } else { COLUMN });
                }
            }
            let lamp = [x, high[1] - 1, z];
            if !reserved(lamp) {
                b.set(lamp, LANTERN);
            }
        }
    }
    let center = room.center();
    let altar = [center[0], low[1], high[2]];
    if !reserved(altar) {
        b.set(
            altar,
            match room.purpose {
                P::BurialHall => 368,
                P::Sanctum => 373,
                P::CodexVault => 378,
                _ => CANDLES,
            },
        );
    }
}

pub(super) fn mausoleum(b: &mut Builder<'_>, variant: GraveVariant) {
    let [x, y, z] = b.summary.center;
    let wall = match variant {
        GraveVariant::Knight => BRICK,
        GraveVariant::Priestess => 340,
        GraveVariant::Scholar => STONE,
    };
    b.fill([x - 11, y, z - 12], [x + 11, y + 7, z + 4], wall);
    b.fill([x - 10, y, z - 11], [x + 10, y + 6, z + 3], 0);
    b.fill([x - 2, y, z - 12], [x + 2, y + 4, z - 12], 0);
    for dx in -3..=3 {
        b.set([x + dx, y + 5, z - 12], ARCH);
    }
    for dx in [-10, -5, 5, 10] {
        b.fill([x + dx, y, z - 12], [x + dx, y + 6, z - 12], COLUMN);
        b.set([x + dx, y + 7, z - 12], CAPITAL);
        b.set([x + dx, y + 4, z - 13], LANTERN);
    }
    for dx in -12_i64..=12 {
        let roof = y + 8 + (12 - dx.abs()) / 2;
        b.fill([x + dx, roof, z - 13], [x + dx, roof, z + 5], DARK);
        for edge in [-12, 4] {
            b.fill(
                [x + dx, y + 7, z + edge],
                [x + dx, roof - 1, z + edge],
                wall,
            );
        }
    }
    b.set([x, y + 15, z - 12], 365);
}
