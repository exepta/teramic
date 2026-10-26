use super::*;

struct Placed<'a, 'b> {
    builder: &'a mut Builder<'b>,
    plot: &'a Plot,
}
impl Placed<'_, '_> {
    fn set(&mut self, p: [i64; 3], m: u16) {
        self.builder.set(self.plot.point(p[0], p[1], p[2]), m);
    }
    fn fill(&mut self, lo: [i64; 3], hi: [i64; 3], m: u16) {
        for z in lo[2]..=hi[2] {
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    self.set([x, y, z], m);
                }
            }
        }
    }
    fn state(&mut self, p: [i64; 3], m: u16, facing: u8) {
        // A pot's state byte is its contents, not a cardinal orientation.
        let facing =
            if terra_core::construction::is_slab(m) || m == terra_core::flower_pot::MATERIAL {
                facing
            } else {
                (facing + self.plot.facing) % 4
            };
        self.builder
            .state(self.plot.point(p[0], p[1], p[2]), m, facing);
    }
    fn paired(&mut self, a: [i64; 3], b: [i64; 3], m: u16) {
        self.paired_facing(a, b, m, 0);
    }
    fn paired_facing(&mut self, a: [i64; 3], b: [i64; 3], m: u16, facing: u8) {
        let a = self.plot.point(a[0], a[1], a[2]);
        let b = self.plot.point(b[0], b[1], b[2]);
        for (p, peer, secondary) in [(a, b, false), (b, a, true)] {
            self.builder.object(
                p,
                m,
                Some(BlockObjectState {
                    facing: (self.plot.facing + facing) % 4,
                    peer: Some(peer),
                    secondary,
                    ..Default::default()
                }),
                None,
            );
        }
    }
}

pub(super) fn build(builder: &mut Builder<'_>, plot: &Plot) {
    let furnished = builder.terrain.config().structures_revision >= 3;
    let populated = builder.terrain.config().structures_revision >= 4;
    let style = builder.summary.style;
    let (stone, wall, wood, door, window, _, fence) = builder.palette();
    let (w, d, h) = (plot.width, plot.depth, plot.height);
    let roof_stone = matches!(
        plot.role,
        BuildingRole::Watch | BuildingRole::Smith | BuildingRole::Police
    ) && style != VillageStyle::Desert;
    let (stairs, slab) = if roof_stone {
        (77, 85)
    } else if style == VillageStyle::Highland {
        (76, 84)
    } else {
        (74, 82)
    };
    let mut model = Placed { builder, plot };
    let clear_height = h
        + w / 2
        + if populated && !matches!(plot.role, BuildingRole::Church | BuildingRole::Watch) {
            3
        } else {
            9
        };
    model.fill([-1, 0, -3], [w + 1, clear_height, d + 1], 0);
    for z in 0..=d {
        for x in 0..=w {
            let p = plot.point(x, 0, z);
            let natural = model.builder.ground(p[0], p[2]);
            model.builder.fill(
                [p[0], natural.min(plot.floor - 1) - 1, p[2]],
                [p[0], plot.floor - 1, p[2]],
                stone,
            );
        }
    }
    model.fill([0, -1, 0], [w, -1, d], wood);
    model.fill([0, 0, 0], [w, h - 1, d], wall);
    model.fill([1, 0, 1], [w - 1, h - 1, d - 1], 0);
    for x in [0, w] {
        for z in [0, d] {
            model.fill(
                [x, 0, z],
                [x, h, z],
                if style == VillageStyle::Desert { 11 } else { 6 },
            );
        }
    }
    for y in [0, h - 1] {
        model.fill([0, y, 0], [w, y, 0], wood);
        model.fill([0, y, d], [w, y, d], wood);
    }
    if style == VillageStyle::Desert {
        model.fill([-1, h, -1], [w + 1, h, d + 1], 11);
        for x in -1..=w + 1 {
            for z in [-1, d + 1] {
                model.state([x, h + 1, z], 87, 0);
            }
        }
        for z in 0..=d {
            for x in [-1, w + 1] {
                model.state([x, h + 1, z], 87, 0);
            }
        }
        for x in (1..w).step_by(3) {
            model.fill([x, 1, 0], [x, h - 2, 0], 19);
        }
    } else {
        for x in -1..=w + 1 {
            let rise = (x + 1).min(w + 1 - x).max(0);
            let y = h + rise;
            for z in -1..=d + 1 {
                if x * 2 == w {
                    model.state([x, y, z], slab, 0);
                } else {
                    model.state([x, y, z], stairs, if x * 2 < w { 1 } else { 3 });
                }
            }
            if (0..=w).contains(&x) {
                for z in [0, d] {
                    model.fill([x, h, z], [x, y - 1, z], wall);
                }
            }
        }
        for z in -1..=d + 1 {
            for x in [-1, w + 1] {
                model.state([x, h - 1, z], slab, 4);
            }
        }
        if matches!(plot.role, BuildingRole::Church | BuildingRole::Medic) {
            for x in 1..w {
                let rise = (x + 1).min(w + 1 - x);
                model.set([x, h + rise - 1, d / 2], 19);
            }
        }
        for z in [0, d] {
            model.fill([0, h, z], [w, h, z], wood);
            for x in 0..=w {
                let rise = (x + 1).min(w + 1 - x);
                model.set([x, h + rise - 1, z], wood);
            }
            for x in [2, w / 2, w - 2] {
                let rise = (x + 1).min(w + 1 - x);
                model.fill([x, h, z], [x, h + rise - 1, z], wood);
            }
            let middle = w / 2;
            let clearance = (middle + 1).min(w + 1 - middle);
            if clearance >= 4 {
                let height = if clearance >= 5 { 2 } else { 1 };
                model.fill([middle - 1, h, z], [middle + 1, h + height + 1, z], wood);
                for y in 1..=height {
                    model.state([middle, h + y, z], window, 0);
                }
            }
        }
    }
    let door_x = w / 2;
    model.paired([door_x, 0, 0], [door_x, 1, 0], door);
    // A three-cell porch keeps the door's approach open; posts stay at its sides.
    for z in -3..=-1 {
        for x in door_x - 2..=door_x + 2 {
            let p = plot.point(x, -1, z);
            let natural = model.builder.ground(p[0], p[2]);
            model
                .builder
                .fill([p[0], natural.min(p[1]) - 1, p[2]], p, stone);
            model.state([x, -1, z], slab, 4);
        }
    }
    for x in [door_x - 2, door_x + 2] {
        model.fill([x, 0, -3], [x, 2, -3], fence);
        model.state([x, 3, -3], slab, 0);
        for z in -2..=-1 {
            model.set([x, 0, z], fence);
        }
    }
    if !matches!(plot.role, BuildingRole::Watch) {
        for x in door_x - 2..=door_x + 2 {
            for z in -3..=-1 {
                model.state([x, 3 + (z + 3) / 2, z], stairs, 0);
            }
        }
    }
    for x in [2, w - 2] {
        if (x - door_x).abs() < 2 {
            continue;
        }
        for z in [0, d] {
            model.state([x, 2, z], window, 0);
            for dx in [-1, 1] {
                model.fill([x + dx, 1, z], [x + dx, 3, z], wood);
            }
            model.state([x, 1, z], 105, 4);
            model.state([x, 3, z], slab, 0);
        }
    }
    for z in [2, d - 2] {
        for x in [0, w] {
            model.state([x, 2, z], window, 1);
            model.state([x, 1, z], 105, 4);
            model.state([x, 3, z], slab, 0);
        }
    }
    let (chest_x, chest_z) = if matches!(plot.role, BuildingRole::Church | BuildingRole::Watch) {
        (2, 2)
    } else {
        (w - 2, d - 2)
    };
    let chest = plot.point(chest_x, 0, chest_z);
    model.builder.object(
        chest,
        29,
        Some(BlockObjectState {
            // Authored chests have their latch at +Z; turn it into the room.
            facing: (plot.facing + if furnished && chest_z > d / 2 { 2 } else { 0 }) % 4,
            ..Default::default()
        }),
        Some(loot::inventory(plot.role, plot.variant)),
    );
    model.builder.summary.containers.push(chest);
    for x in [1, w - 1] {
        model.set([x, 0, 1], 40);
    }
    match plot.role {
        BuildingRole::House | BuildingRole::Medic | BuildingRole::Police => {
            model.paired(
                [2, 0, d - 3],
                [2, 0, d - 2],
                if style == VillageStyle::Highland {
                    73
                } else {
                    71
                },
            );
            for x in w - 4..=w - 2 {
                model.state([x, 0, d / 2], 105, 0);
            }
            if plot.role == BuildingRole::Medic {
                for dx in -1..=1 {
                    model.set([door_x + dx, h - 2, -1], 19);
                }
                model.fill([door_x, h - 3, -1], [door_x, h - 1, -1], 19);
            }
        }
        BuildingRole::Smith => {
            model.paired_facing(
                [2, 0, d - 2],
                [2, 1, d - 2],
                59,
                if furnished { 2 } else { 0 },
            );
            for y in 2..=h + w / 2 + 3 {
                model.state([2, y, d - 2], 60, 0);
            }
            if furnished {
                // A rotated two-cell bench must also swap its primary anchor.
                model.paired_facing([w / 2 + 1, 0, d - 2], [w / 2, 0, d - 2], 28, 2);
            } else {
                model.paired([w / 2, 0, d - 2], [w / 2 + 1, 0, d - 2], 28);
            }
            for z in 2..=d - 2 {
                model.set([1, 0, z], 43);
                model.state([1, 1, z], 85, 0);
            }
        }
        BuildingRole::Church | BuildingRole::Watch => {
            tower(&mut model, stone, wood, window, stairs, fence);
            if plot.role == BuildingRole::Church {
                model.fill([3, 0, d - 3], [w - 4, 1, d - 2], WOOL);
                model.state([door_x, 0, d - 4], 104, 0);
                for z in (3..d - 4).step_by(3) {
                    for x in [2, w - 2] {
                        model.state([x, 0, z], 82, 0);
                    }
                }
            }
        }
        _ => {}
    }
    if populated {
        let material = if style == VillageStyle::Highland {
            73
        } else {
            71
        };
        let additional = match plot.role {
            BuildingRole::House | BuildingRole::Medic | BuildingRole::Police => Some([3, 0, d - 3]),
            BuildingRole::Church => Some([1, 0, d - 3]),
            BuildingRole::Smith => Some([w - 2, 0, d - 4]),
            _ => None,
        };
        if let Some(foot) = additional {
            model.paired(foot, [foot[0], foot[1], foot[2] + 1], material);
        }
    }
    if furnished {
        furnishings(&mut model, style);
    }
    // Low planters flank the porch rather than obstruct its middle opening.
    for x in [1, w - 1] {
        if (x - door_x).abs() > 2 {
            model.set([x, 0, -1], 43);
            model.set([x, 1, -1], 7);
        }
    }
    let [low, high] = plot.bounds();
    model.builder.summary.buildings.push(BuildingSummary {
        role: plot.role,
        entrance: plot.point(door_x, 0, 0),
        bounds: [
            [low[0] - 1, plot.floor - 2, low[1] - 1],
            [high[0] + 1, plot.floor + h + w / 2 + 10, high[1] + 1],
        ],
    });
}

fn furnishings(model: &mut Placed<'_, '_>, style: VillageStyle) {
    use terra_core::{ItemKind, beds::DyeColor, flower_pot, textiles};
    let plot = model.plot;
    let (w, d) = (plot.width, plot.depth);
    let colors = match style {
        VillageStyle::Temperate => [DyeColor::Red, DyeColor::Green],
        VillageStyle::Desert => [DyeColor::Orange, DyeColor::LightBlue],
        VillageStyle::Highland => [DyeColor::Blue, DyeColor::White],
    };
    let carpet = textiles::dyed(
        textiles::WHITE_CARPET,
        colors[plot.variant as usize % colors.len()],
    )
    .unwrap();
    // Keep a two-cell entrance aisle and every machine/bed interaction clear.
    let (left, right, near, far) = match plot.role {
        BuildingRole::House | BuildingRole::Medic => (w / 2 - 1, w / 2 + 1, 2, d / 2 - 1),
        BuildingRole::Church => (w / 2, w / 2, 2, d - 5),
        BuildingRole::Smith | BuildingRole::Watch | BuildingRole::Police => {
            (w / 2, w / 2 + 1, 2, 3)
        }
        _ => return,
    };
    for z in near..=far {
        for x in left..=right {
            model.set([x, 0, z], carpet);
        }
    }
    // A solid ledge supports each pot. Contents use the normal persisted state,
    // so taking a generated flower or breaking its pot uses ordinary gameplay.
    let flower = match style {
        VillageStyle::Temperate => DyeColor::Red,
        VillageStyle::Desert => DyeColor::Yellow,
        VillageStyle::Highland => DyeColor::White,
    };
    for (x, item) in [
        (w - 2, ItemKind::Flower(flower)),
        (
            2,
            if plot.role == BuildingRole::Medic {
                ItemKind::Prop(terra_core::ResourcePropKind::Mushroom)
            } else {
                ItemKind::Flower(DyeColor::Purple)
            },
        ),
    ] {
        let z = if x == 2 { d / 2 } else { 2 };
        model.set([x, 0, z], 43);
        model.state(
            [x, 1, z],
            flower_pot::MATERIAL,
            flower_pot::state(item).unwrap(),
        );
    }
}

fn tower(model: &mut Placed<'_, '_>, stone: u16, wood: u16, window: u16, stairs: u16, fence: u16) {
    let p = model.plot;
    let x = p.width - 4;
    let z = p.depth - 4;
    let base = p.height;
    let top = base + 6;
    model.fill([x, base, z], [x + 3, top, z + 3], stone);
    model.fill([x + 1, 0, z + 1], [x + 2, top, z + 2], 0);
    for [px, pz, facing] in [
        [x + 1, z, 0],
        [x + 1, z + 3, 0],
        [x, z + 1, 1],
        [x + 3, z + 1, 1],
    ] {
        model.state([px, top - 1, pz], window, facing as u8);
    }
    for level in 0..top {
        let (dx, dz, facing) = spiral(level);
        model.state([x + 1 + dx, level, z + 1 + dz], 77, facing);
    }
    model.fill([x + 1, 0, z - 1], [x + 2, 2, z], 0);
    if p.role == BuildingRole::Watch {
        model.fill([x - 1, top + 1, z - 1], [x + 4, top + 1, z + 4], wood);
        model.fill([x + 1, top + 1, z + 1], [x + 2, top + 1, z + 2], 0);
        for level in top..=top + 1 {
            let (dx, dz, facing) = spiral(level);
            model.state([x + 1 + dx, level, z + 1 + dz], 77, facing);
        }
        for px in x - 1..=x + 4 {
            for pz in [z - 1, z + 4] {
                model.set([px, top + 2, pz], if px % 2 == 0 { stone } else { fence });
            }
        }
        for pz in z..=z + 3 {
            for px in [x - 1, x + 4] {
                model.set([px, top + 2, pz], fence);
            }
        }
    } else {
        for px in x - 1..=x + 4 {
            for pz in z - 1..=z + 4 {
                model.state([px, top + 1, pz], stairs, if px < x + 2 { 1 } else { 3 });
            }
        }
        model.fill([x + 1, top + 2, z + 1], [x + 2, top + 3, z + 2], 19);
        model.fill([x + 1, top + 4, z + 1], [x + 1, top + 6, z + 1], wood);
        model.fill([x, top + 5, z + 1], [x + 2, top + 5, z + 1], wood);
    }
}

fn spiral(level: i64) -> (i64, i64, u8) {
    match level % 4 {
        0 => (0, 0, 1),
        1 => (1, 0, 0),
        2 => (1, 1, 3),
        _ => (0, 1, 2),
    }
}

pub(super) fn plaza(
    builder: &mut Builder<'_>,
    center: [i64; 2],
    layout: Layout,
    roads: &paths::Routes,
) {
    if layout == Layout::Courtyard {
        for u in -15..=15 {
            for v in [-14, 14] {
                fence(builder, [center[0] + u, center[1] + v], roads);
            }
        }
        for v in -13..=13 {
            for u in [-15, 15] {
                fence(builder, [center[0] + u, center[1] + v], roads);
            }
        }
    }
    for [u, v] in [[8, -8], [-9, 6], [10, 7], [-9, -7]] {
        let x = center[0] + u;
        let z = center[1] + v;
        if (-2..=2).any(|dx| (-2..=2).any(|dz| roads.walk.contains(&[x + dx, z + dz]))) {
            continue;
        }
        let floor = builder.ground(x, z);
        builder.fill([x - 2, floor - 1, z - 1], [x + 2, floor - 1, z + 1], 43);
        for dx in [-2, 2] {
            for dz in [-1, 1] {
                builder.fill([x + dx, floor, z + dz], [x + dx, floor + 2, z + dz], 90);
            }
        }
        for dx in -2..=2 {
            for dz in -1..=1 {
                builder.state(
                    [x + dx, floor + 3, z + dz],
                    if dx % 2 == 0 { 105 } else { 82 },
                    0,
                );
            }
        }
        builder.object(
            [x, floor, z],
            29,
            Some(BlockObjectState::default()),
            Some(loot::inventory(
                BuildingRole::House,
                builder.seed ^ u as u32,
            )),
        );
        builder.summary.containers.push([x, floor, z]);
    }
    for [u, v] in [[-6, -4], [6, 4], [-5, 10], [5, -11]] {
        let p = [center[0] + u, center[1] + v];
        if roads.walk.contains(&p) {
            continue;
        }
        let base = builder.ground(p[0], p[1]);
        builder.fill([p[0], base, p[1]], [p[0], base + 2, p[1]], 90);
        builder.set([p[0], base + 3, p[1]], 40);
    }
}
fn fence(builder: &mut Builder<'_>, p: [i64; 2], roads: &paths::Routes) {
    if (-1..=1).any(|dx| (-1..=1).any(|dz| roads.walk.contains(&[p[0] + dx, p[1] + dz]))) {
        return;
    }
    let y = builder.ground(p[0], p[1]);
    builder.set([p[0], y, p[1]], 90);
}
