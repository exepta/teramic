//! A fishing village follows the surveyed bank. Local -Z points out to sea.
use super::{CoastalLandmarkSite, Habitat, M, SiteFrame};
use crate::generation::natural_features::ticket;
use std::collections::{BTreeMap, BinaryHeap};

#[derive(Clone, Copy)]
struct Plot {
    x: i64,
    z: i64,
    floor: i64,
    style: u32,
}

pub(super) struct Harbor {
    houses: Vec<Plot>,
    jetty: i64,
    berth: i64,
    cutter: bool,
    second_berth: Option<i64>,
}

fn plots(seed: u32, floor: &[[i64; 31]; 31]) -> Vec<Plot> {
    let ground = |x: i64, z: i64| floor[(z + 15) as usize][(x + 15) as usize];
    let mut lots = Vec::new();
    for z in [1, 11] {
        for x in [-12, -4, 4, 12] {
            let heights = [
                ground(x, z),
                ground(x - 2, z - 2),
                ground(x + 2, z - 2),
                ground(x - 2, z + 2),
                ground(x + 2, z + 2),
            ];
            let low = *heights.iter().min().unwrap();
            let high = *heights.iter().max().unwrap();
            if low < -3 || high > 14 || high - low > 7 {
                continue;
            }
            lots.push(Plot {
                x,
                z,
                floor: ground(x, z).max(high - 2).max(1),
                style: ticket(u64::from(seed), [x, z], 0xf157),
            });
        }
    }
    lots.sort_by_key(|p| p.style);
    // A small hamlet should present more than one frontage from the water.
    if seed.is_multiple_of(7)
        && lots.len() > 2
        && lots[0].x == lots[1].x
        && let Some(other) = lots.iter().position(|p| p.x != lots[0].x)
    {
        lots.swap(1, other);
    }
    lots.truncate(2 + (seed % 7) as usize);
    lots
}

pub(super) fn survey(
    h: &mut Habitat<'_>,
    site: CoastalLandmarkSite,
    ground: &[[i64; 31]; 31],
) -> Option<Harbor> {
    let seed = ticket(
        h.terrain.config().seed.0,
        [site.center[0], site.center[2]],
        0xc057,
    );
    let houses = plots(seed, ground);
    if houses.len() < 2 {
        return None;
    }
    let mut wet = |x, z| {
        let [wx, _, wz] = site.world(x, 0, z);
        h.column([wx, wz])
            .water_columns()
            .at(site.center[1] - 1)
            .is_some()
    };
    let mut skiff_only = None;
    for jetty in [1, -3, 5, -7, 9] {
        if !wet(jetty, -13) || !wet(jetty, -14) {
            continue;
        }
        for offset in [4, -4] {
            let berth = jetty + offset;
            if ![-12, -10, -8].into_iter().all(|z| wet(berth, z)) {
                continue;
            }
            let cutter =
                (berth - 2..=berth + 2).all(|x| [-13, -11, -9, -7].into_iter().all(|z| wet(x, z)));
            let other = jetty - offset;
            let second_berth = (houses.len() >= 4
                && [-13, -11, -9].into_iter().all(|z| wet(other, z)))
            .then_some(other);
            if cutter {
                return Some(Harbor {
                    houses,
                    jetty,
                    berth,
                    cutter,
                    second_berth,
                });
            }
            skiff_only.get_or_insert((jetty, berth));
        }
    }
    skiff_only.map(|(jetty, berth)| Harbor {
        houses,
        jetty,
        berth,
        cutter: false,
        second_berth: None,
    })
}

fn grade_boardwalk(deck: &mut BTreeMap<(i64, i64), i64>) {
    let mut queue: BinaryHeap<_> = deck.iter().map(|(&pos, &y)| (y, pos)).collect();
    while let Some((y, (x, z))) = queue.pop() {
        if deck[&(x, z)] != y {
            continue;
        }
        for pos in [(x - 1, z), (x + 1, z), (x, z - 1), (x, z + 1)] {
            if let Some(next) = deck.get_mut(&pos)
                && *next < y - 1
            {
                *next = y - 1;
                queue.push((*next, pos));
            }
        }
    }
}

impl SiteFrame<'_> {
    fn net_yard(&self, harbor: &Harbor) -> i64 {
        // The platform can hug either side of a sloping cove. Keep its
        // seaward edge clear of both hulls and close to the working pier.
        (-12_i64..=12)
            .filter(|&x| (x - harbor.jetty).abs() >= 5)
            .min_by_key(|&x| {
                let mut low = i64::MAX;
                let mut high = i64::MIN;
                for dx in -3..=3 {
                    for z in -7..=-4 {
                        let y = self.floor(x + dx, z);
                        low = low.min(y);
                        high = high.max(y);
                    }
                }
                let clash = (x - harbor.berth).abs() <= 4
                    || harbor.second_berth.is_some_and(|b| (x - b).abs() <= 4);
                (
                    clash,
                    3 * (high - low) + 2 * (high - 1).abs() + (x - harbor.jetty).abs(),
                )
            })
            .expect("the jetty leaves room for a net platform")
    }

    pub(super) fn fishing_village(&mut self, mut harbor: Harbor) {
        let j = harbor.jetty;
        let yard = self.net_yard(&harbor);
        let mut deck = BTreeMap::new();
        let mut add = |x, z| {
            deck.insert((x, z), self.floor(x, z).max(1));
        };
        for z in -14..=-3 {
            for x in j - 1..=j + 1 {
                add(x, z);
            }
        }
        for x in yard - 3..=yard + 3 {
            for z in -7..=-3 {
                add(x, z);
            }
        }
        let inland = harbor.houses.iter().any(|p| p.z == 11);
        for z in -3..=if inland { 6 } else { -3 } {
            add(0, z);
        }
        for (lane, row) in [(-3, 1), (6, 11)] {
            let mut left = if row == 1 { j.min(yard).min(0) } else { 0 };
            let mut right = if row == 1 { j.max(yard).max(0) } else { 0 };
            for p in harbor.houses.iter().filter(|p| p.z == row) {
                left = left.min(p.x);
                right = right.max(p.x);
                for z in lane..=p.z - 3 {
                    add(p.x, z);
                }
            }
            if row == 1 || inland {
                for x in left..=right {
                    for z in lane - 1..=lane {
                        add(x, z);
                    }
                }
            }
        }
        for p in &harbor.houses {
            deck.insert((p.x, p.z - 3), p.floor.max(self.floor(p.x, p.z - 3)));
        }
        // Lift low neighbours into steps. A level yard is a single landing;
        // finish grading from its highest edge after grading the approaches.
        grade_boardwalk(&mut deck);
        let yard_level = deck
            .iter()
            .filter(|&(&(x, z), _)| (x - yard).abs() <= 3 && (-7..=-4).contains(&z))
            .map(|(_, &y)| y)
            .max()
            .expect("surveyed net platform");
        for x in yard - 3..=yard + 3 {
            for z in -7..=-4 {
                deck.insert((x, z), yard_level);
            }
        }
        grade_boardwalk(&mut deck);
        for (&(x, z), &y) in &deck {
            let floor = self.floor(x, z);
            if floor >= y - 1 {
                self.seat(x, z, y, M::SprucePlanks);
            } else {
                self.put(x, y, z, M::SprucePlanks);
                if (x == j - 1 || x == j + 1) && (z + 14) % 4 == 0
                    || (x - yard).abs() == 3 && (z == -7 || z == -4)
                    || (x + 2 * z).rem_euclid(5) == 0
                {
                    self.shaft(x, z, floor, y - 1, M::Wood);
                }
            }
            self.shaft(x, z, y + 1, y + 3, M::Air);
        }
        for p in &mut harbor.houses {
            p.floor = deck[&(p.x, p.z - 3)];
            self.fishers_house(*p);
        }
        for z in [-14, -10, -6] {
            for x in [j - 1, j + 1] {
                self.put(x, deck[&(x, z)] + 1, z, M::MooringPost);
            }
        }
        self.put(j + 1, deck[&(j + 1, -14)] + 2, -14, M::Lantern);
        self.prop(j - 1, deck[&(j - 1, -9)] + 1, -9, M::LobsterPot, 1);
        for x in [yard - 3, yard + 2] {
            self.shaft(x, -6, yard_level + 1, yard_level + 2, M::Wood);
        }
        for x in yard - 3..=yard + 2 {
            self.put(x, yard_level + 3, -6, M::SpruceSlab);
        }
        for x in yard - 2..=yard + 1 {
            self.put(x, yard_level + 1, -6, M::SprucePlanks);
            self.prop(x, yard_level + 2, -6, M::FishingNet, 0);
        }
        self.prop(yard - 2, yard_level + 1, -5, M::SaltCrate, 0);
        self.prop(yard - 1, yard_level + 1, -5, M::SaltCrate, 1);
        self.put(yard + 1, yard_level + 1, -7, M::BarnacledBarrel);
        self.locker(yard + 2, yard_level + 1, -5);
        self.boat(harbor.berth, -10, harbor.cutter);
        if let Some(other) = harbor.second_berth {
            self.boat(other, -11, false);
        }
    }

    fn fishers_house(&mut self, p: Plot) {
        let Plot {
            x: cx,
            z: cz,
            floor: y,
            style,
        } = p;
        let roof = if style % 5 < 2 {
            M::HarborRoofTiles
        } else {
            M::BlueSlateShingles
        };
        let siding = if style & 4 == 0 {
            M::SprucePlanks
        } else {
            M::WhitewashedPlaster
        };
        let tall = i64::from(style & 16 != 0);
        let eave = y + 4 + tall;
        for dx in -2_i64..=2 {
            for dz in -2_i64..=2 {
                let (x, z) = (cx + dx, cz + dz);
                self.seat(x, z, y, M::SprucePlanks);
                self.shaft(x, z, y + 1, self.floor(x, z).max(eave + 2), M::Air);
                if dx.abs() == 2 || dz.abs() == 2 {
                    self.shaft(x, z, y + 1, eave - 1, siding);
                    self.put(x, eave, z, M::Wood);
                    if dx.abs() == 2 && dz.abs() == 2 {
                        self.shaft(x, z, y + 1, eave, M::Wood);
                    } else if dx == 0 && dz == -2 {
                        self.shaft(x, z, y + 1, y + 2, M::Air);
                    } else if (dx.abs() == 2 && dz == 0)
                        || (dz.abs() == 2 && dx != 0)
                        || (dz == 2 && dx == 0)
                    {
                        self.put(x, y + 2, z, M::Glass);
                        if dx.abs() == 2 {
                            self.put(x + dx.signum(), y + 1, z, M::SpruceSlab);
                        }
                    }
                }
            }
        }
        let crosswise = style & 32 != 0;
        for dx in -3_i64..=3 {
            for dz in -3_i64..=3 {
                let (across, along) = if crosswise { (dz, dx) } else { (dx, dz) };
                let pitch = 3 - across.abs();
                let roof_y = eave
                    + if style & 64 != 0 {
                        pitch
                    } else {
                        (pitch + 1) / 2
                    };
                if along.abs() == 2 {
                    self.shaft(cx + dx, cz + dz, eave, roof_y - 1, siding);
                }
                if along.abs() == 2 && across == 0 {
                    self.put(cx + dx, eave + 1, cz + dz, M::Glass);
                }
                self.put(cx + dx, roof_y, cz + dz, roof);
                if across == 0 {
                    self.put(
                        cx + dx,
                        roof_y + 1,
                        cz + dz,
                        if roof == M::BlueSlateShingles {
                            M::StoneSlab
                        } else {
                            M::SpruceSlab
                        },
                    );
                }
            }
        }
        // The covered porch, bracket and sign face the raised boardwalk.
        for dx in -1..=1 {
            self.put(cx + dx, y + 4, cz - 3, M::SpruceSlab);
        }
        if tall != 0 {
            self.shaft(cx + 1, cz - 3, y + 2, y + 3, M::OakFence);
        }
        let flue = cx + if style & 1 == 0 { 1 } else { -1 };
        let chimney_top = eave + if style & 64 != 0 { 4 } else { 3 };
        self.shaft(flue, cz + 1, y + 1, chimney_top - 1, M::CoastalGranite);
        self.put(flue, chimney_top, cz + 1, M::StoneSlab);
        self.put(cx - 1, y + 1, cz, M::SpruceSlab);
        for x in cx - 1..=cx + 1 {
            self.seat(x, cz - 3, y, M::SprucePlanks);
        }
        self.prop(cx + 1, y + 1, cz - 3, M::HarborSign, 2);
        self.put(cx - 1, y + 3, cz - 3, M::Lantern);
        if style & 8 != 0 {
            self.prop(cx + 1, y + 1, cz, M::SaltCrate, 1);
        } else {
            self.put(cx + 1, y + 1, cz, M::BarnacledBarrel);
        }
    }

    fn boat(&mut self, x: i64, z: i64, cutter: bool) {
        let length: i64 = if cutter { 4 } else { 3 };
        for dz in -length..=length {
            let width = if dz.abs() == length {
                0
            } else if cutter && dz.abs() <= 2 {
                2
            } else {
                1
            };
            self.put(x, -1, z + dz, M::Wood);
            for dx in -width..=width {
                self.put(x + dx, 0, z + dz, M::SprucePlanks);
                if dx.abs() == width {
                    self.put(
                        x + dx,
                        1,
                        z + dz,
                        if cutter { M::BlueWool } else { M::OakSlab },
                    );
                    if cutter {
                        self.put(x + dx, 2, z + dz, M::SpruceSlab);
                    }
                }
            }
            if !cutter && (dz == -1 || dz == 2) {
                self.put(x, 1, z + dz, M::SpruceSlab);
            }
        }
        if cutter {
            for dx in -1..=1 {
                for dz in 0..=2 {
                    if dx != 0 || dz == 2 {
                        self.put(x + dx, 1, z + dz, M::WhitewashedPlaster);
                        self.put(x + dx, 2, z + dz, M::Glass);
                    }
                    self.put(x + dx, 3, z + dz, M::SpruceSlab);
                }
            }
            self.shaft(x, z - 2, 1, 5, M::OakFence);
            self.put(x, 6, z - 2, M::Lantern);
            self.put(x + 1, 4, z - 2, M::SpruceSlab);
            self.prop(x + 1, 3, z - 2, M::FishingNet, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn village_sizes_cover_every_house_count_with_disjoint_plots_and_walking_lanes() {
        let ground = [[1; 31]; 31];
        for count in 2..=8 {
            let houses = plots(count - 2, &ground);
            assert_eq!(houses.len(), count as usize);
            for (i, p) in houses.iter().enumerate() {
                assert!(p.x.abs() + 3 <= 15 && p.z + 3 <= 15);
                assert!(
                    houses[..i]
                        .iter()
                        .all(|q| (q.x - p.x).abs() > 6 || (q.z - p.z).abs() > 6)
                );
            }
        }
    }
    #[test]
    fn surveyed_jetties_reach_every_door_and_stores_above_the_bank() {
        use super::super::super::tests::config;
        use crate::{SectionGenerator, TerraGenerator};
        use std::collections::{BTreeSet, VecDeque};

        let terrain = TerraGenerator::new(config()).unwrap();
        let sections = SectionGenerator::new(terrain.clone());
        let mut roof_tiles = BTreeSet::new();
        let mut thresholds = BTreeSet::new();
        for (tile, count) in [
            ([-51, -65], 2),
            ([-30, -39], 3),
            ([73, -86], 4),
            ([-50, 55], 5),
            ([103, -74], 6),
            ([-167, 147], 7),
            ([144, 135], 8),
        ] {
            let site = terrain
                .coastal_landmark_at(tile)
                .unwrap_or_else(|| panic!("missing fishing village at tile {tile:?}"));
            let patch = terrain.natural_patch(tile);
            let label = site.center;
            let blocks: BTreeMap<_, _> = patch.cells.iter().copied().collect();
            for &m in blocks.values() {
                if m == M::HarborRoofTiles as u16 || m == M::BlueSlateShingles as u16 {
                    roof_tiles.insert(m);
                }
            }
            let block = |p| sections.block_at(p).unwrap();
            let lantern = (-9..=9)
                .find(|j| blocks.get(&site.world(j + 1, 3, -14)) == Some(&(M::Lantern as u16)))
                .unwrap_or_else(|| panic!("missing jetty-head lantern at village {label:?}"));
            let start = site.world(lantern, 1, -14);
            let mut seen = BTreeSet::from([start]);
            let mut queue = VecDeque::from([start]);
            while let Some([x, y, z]) = queue.pop_front() {
                for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    for dy in [-1, 0, 1] {
                        let pos = [x + dx, y + dy, z + dz];
                        if !seen.contains(&pos)
                            && blocks.get(&pos) == Some(&(M::SprucePlanks as u16))
                            && [1, 2]
                                .into_iter()
                                .all(|up| block([pos[0], pos[1] + up, pos[2]]) == M::Air as u16)
                        {
                            seen.insert(pos);
                            queue.push_back(pos);
                        }
                    }
                }
            }
            let mut doors = Vec::new();
            for x in [-12, -4, 4, 12] {
                for z in [1, 11] {
                    let door = (1..=20)
                        .map(|y| site.world(x, y, z - 2))
                        .find(|&[a, b, c]| {
                            blocks.get(&[a, b, c]) == Some(&(M::SprucePlanks as u16))
                                && block([a, b + 1, c]) == M::Air as u16
                                && block([a, b + 2, c]) == M::Air as u16
                        });
                    if let Some(door) = door {
                        assert!(
                            seen.contains(&door),
                            "unreachable door {door:?} at {label:?}"
                        );
                        thresholds.insert(door[1]);
                        doors.push(x);
                    }
                }
            }
            assert_eq!(doors.len(), count, "houses at {label:?}");
            if count == 2 {
                assert_ne!(doors[0], doors[1], "both houses hidden in one frontage");
            }
            for &[x, y, z] in &seen {
                let natural =
                    (terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5) - 0.5).floor() as i64;
                // The interiors of fisher cottages can have a stone retaining wall.
                let in_house = [1, 11].into_iter().any(|cz| {
                    [-12, -4, 4, 12].into_iter().any(|cx| {
                        let [px, _, pz] = site.world(cx, 0, cz);
                        (px - x).abs() <= 2 && (pz - z).abs() <= 2
                    })
                });
                if !in_house {
                    assert!(
                        y >= natural,
                        "path cut into bank at {x},{y},{z}, surface {natural}"
                    );
                }
            }
            let nets: Vec<_> = (-15..=15)
                .flat_map(|x| (2..=18).map(move |y| site.world(x, y, -6)))
                .filter(|p| blocks.get(p) == Some(&(M::FishingNet as u16)))
                .collect();
            assert_eq!(nets.len(), 4, "incomplete drying rack at {label:?}");
            for &[x, y, z] in &nets {
                let surface =
                    (terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5) - 0.5).floor() as i64;
                assert_eq!(y, nets[0][1], "uneven drying rack at {x},{y},{z}");
                assert!(
                    y - 2 >= surface,
                    "net platform at {x},{},{z} lies below ground {surface}",
                    y - 2
                );
            }
            let chest = patch
                .objects
                .iter()
                .find(|o| o.loot.is_some() && o.material == M::Chest as u16)
                .unwrap_or_else(|| panic!("no stocked locker at village {label:?}"))
                .position;
            assert!(
                seen.iter()
                    .any(|p| (p[0] - chest[0]).abs() + (p[2] - chest[2]).abs() == 1
                        && p[1] + 1 == chest[1]),
                "locker at {chest:?} unreachable from jetty at {start:?}"
            );
        }
        assert_eq!(roof_tiles.len(), 2);
        assert!(thresholds.len() >= 3);
    }
}
