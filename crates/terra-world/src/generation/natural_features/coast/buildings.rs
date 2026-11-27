//! Ground survey, foundations, moorings and walkable coastal buildings.
use super::{Cell, CoastalLandmarkKind as Kind, CoastalLandmarkSite, Habitat, M, StructureObject};
use crate::edits::BlockObjectState;
use std::collections::BTreeMap;
use terra_core::{Inventory, ItemKind};

mod harbor;

pub(super) fn build(
    h: &mut Habitat<'_>,
    site: CoastalLandmarkSite,
    blocks: &mut BTreeMap<Cell, u16>,
    objects: &mut Vec<StructureObject>,
) -> bool {
    let mut ground = [[0; 31]; 31];
    for z in -15..=15 {
        for x in -15..=15 {
            let [wx, _, wz] = site.world(x, 0, z);
            let col = h.column([wx, wz]);
            if col.lava_level.is_some() {
                return false;
            }
            ground[(z + 15) as usize][(x + 15) as usize] =
                (col.height - 0.5).floor() as i64 - site.center[1];
        }
    }
    let floor = |x: i64, z: i64| ground[(z + 15) as usize][(x + 15) as usize];
    let mut harbor_plan = None;
    match site.kind {
        Kind::Lighthouse => {
            for (x, z) in [(-4, 4), (4, 4), (-4, -3), (4, -3), (-11, 4), (-11, 10)] {
                if !(-5..=5).contains(&floor(x, z)) {
                    return false;
                }
            }
        }
        Kind::FishingVillage => {
            harbor_plan = harbor::survey(h, site, &ground);
            if harbor_plan.is_none() {
                return false;
            }
        }
        Kind::Shipwreck => {
            if floor(0, 8) > 5 || floor(0, -8) > 5 {
                return false;
            }
        }
        Kind::TidalCave => {
            let [wx, _, wz] = site.world(0, 0, 5);
            if h.column([wx, wz]).river_cavity.is_none() || floor(0, 6) < 6 {
                return false;
            }
        }
    }
    let mut s = SiteFrame {
        site,
        ground,
        blocks,
        objects,
    };
    match site.kind {
        Kind::Lighthouse => s.lighthouse(),
        Kind::FishingVillage => s.fishing_village(harbor_plan.expect("surveyed coastal plots")),
        Kind::Shipwreck => s.wreck(h.terrain.config().sea_level.floor() as i64 - site.center[1]),
        Kind::TidalCave => s.cave(),
    }
    // Natural fittings carry independent orientation and inventory records.
    // Remove a previous fitting when a foundation or doorway replaced its cell.
    s.objects
        .retain(|o| s.blocks.get(&o.position).is_none_or(|id| *id == o.material));
    true
}

struct SiteFrame<'a> {
    site: CoastalLandmarkSite,
    ground: [[i64; 31]; 31],
    blocks: &'a mut BTreeMap<Cell, u16>,
    objects: &'a mut Vec<StructureObject>,
}
impl SiteFrame<'_> {
    fn floor(&self, x: i64, z: i64) -> i64 {
        self.ground[(z + 15) as usize][(x + 15) as usize]
    }
    fn put(&mut self, x: i64, y: i64, z: i64, m: M) {
        debug_assert!(x.abs() <= 15 && z.abs() <= 15);
        self.blocks.insert(self.site.world(x, y, z), m as u16);
    }
    fn shaft(&mut self, x: i64, z: i64, from: i64, to: i64, m: M) {
        for y in from..=to {
            self.put(x, y, z, m);
        }
    }
    fn seat(&mut self, x: i64, z: i64, y: i64, m: M) {
        self.shaft(x, z, self.floor(x, z).min(y) - 1, y - 1, M::CoastalGranite);
        self.put(x, y, z, m);
    }
    fn prop(&mut self, x: i64, y: i64, z: i64, m: M, turn: u8) {
        self.put(x, y, z, m);
        self.objects.push(StructureObject {
            position: self.site.world(x, y, z),
            material: m as u16,
            state: Some(BlockObjectState {
                facing: (turn + 4 - self.site.turn) % 4,
                ..Default::default()
            }),
            loot: None,
            crop: None,
        });
    }
    fn locker(&mut self, x: i64, y: i64, z: i64) {
        let mut loot = Inventory::default();
        for (m, n) in [(M::ClamShell, 4), (M::FishingNet, 1), (M::Lantern, 1)] {
            loot.insert(ItemKind::Block(m as u16), n);
        }
        self.prop(x, y, z, M::Chest, 2);
        self.objects.last_mut().unwrap().loot = Some(loot);
    }
    fn cottage(&mut self, cx: i64, cz: i64, y: i64, roof: M) {
        for z in -3_i64..=3 {
            for x in -3_i64..=3 {
                let (wx, wz) = (cx + x, cz + z);
                self.seat(wx, wz, y, M::WeatheredWood);
                self.shaft(wx, wz, y + 1, y + 8, M::Air);
                if x.abs() == 3 || z.abs() == 3 {
                    self.shaft(wx, wz, y + 1, y + 3, M::WhitewashedPlaster);
                    self.put(wx, y + 4, wz, M::HarborTimber);
                    if x.abs() == 3 && z.abs() == 3 {
                        self.shaft(wx, wz, y + 1, y + 4, M::HarborTimber);
                    } else if x == 0 && z == -3 {
                        self.shaft(wx, wz, y + 1, y + 3, M::Air);
                    } else if (x.abs() == 2 && z.abs() == 3) || (x.abs() == 3 && z == 0) {
                        self.put(wx, y + 2, wz, M::Glass);
                    }
                }
            }
        }
        for z in -4_i64..=4 {
            for x in -4_i64..=4 {
                let roof_y = y + 4 + (4 - x.abs()) / 2;
                if z.abs() == 3 && x.abs() <= 2 {
                    self.shaft(cx + x, cz + z, y + 4, roof_y - 1, M::WhitewashedPlaster);
                }
                self.put(cx + x, roof_y, cz + z, roof);
            }
        }
        self.shaft(cx + 2, cz + 2, y + 1, y + 8, M::SmoothStone);
        self.put(cx - 2, y + 1, cz + 1, M::OakSlab);
        self.put(cx + 2, y + 4, cz - 4, M::Lantern);
        self.seat(cx - 2, cz - 4, y, M::BeachShingle);
        self.prop(cx - 2, y + 1, cz - 4, M::HarborSign, 2);
    }
    fn lighthouse(&mut self) {
        let radius = |y: i64| {
            if y < 4 {
                4.4
            } else if y < 12 {
                3.7
            } else {
                3.2
            }
        };
        for z in -5_i64..=5 {
            for x in -5_i64..=5 {
                let d = (x as f64).hypot(z as f64);
                if d <= 4.6 {
                    self.seat(x, z, 0, M::SaltStainedStone);
                    self.shaft(x, z, 1, self.floor(x, z).max(24), M::Air);
                }
                for y in 1..=16 {
                    let r = radius(y);
                    if d <= r && d > r - 1.25 {
                        if x == 0 && z < 0 && y <= 3 {
                            continue;
                        }
                        let m = if y < 3 {
                            M::CoastalGranite
                        } else if (6..=7).contains(&y) || (12..=13).contains(&y) {
                            M::LighthouseStripe
                        } else if (x == 0 || z == 0) && [5, 10, 15].contains(&y) {
                            M::Glass
                        } else {
                            M::WhitewashedPlaster
                        };
                        self.put(x, y, z, m);
                    }
                }
                if d <= 4.4 {
                    if !(x == 0 && z == -2) {
                        self.put(x, 17, z, M::SmoothStone);
                    }
                    if d > 3.5 {
                        self.put(x, 18, z, M::SpruceFence);
                    }
                }
                if (2.0..3.15).contains(&d) {
                    for y in 18..=21 {
                        self.put(
                            x,
                            y,
                            z,
                            if x.abs() == 2 && z.abs() == 2 {
                                M::HarborTimber
                            } else {
                                M::Glass
                            },
                        );
                    }
                }
                for y in 22..=24 {
                    if d <= 4.0 - (y - 22) as f64 * 1.25 {
                        self.put(x, y, z, M::HarborRoofTiles);
                    }
                }
            }
        }
        self.shaft(0, 0, 1, 17, M::HarborTimber);
        let stair = [
            (0, -2),
            (1, -2),
            (2, -2),
            (2, -1),
            (2, 0),
            (2, 1),
            (2, 2),
            (1, 2),
            (0, 2),
            (-1, 2),
            (-2, 2),
            (-2, 1),
            (-2, 0),
            (-2, -1),
            (-2, -2),
            (-1, -2),
        ];
        for y in 0..=17 {
            let (x, z) = stair[(y % 16) as usize];
            self.put(x, y, z, M::SprucePlanks);
        }
        self.put(0, 18, 0, M::SmoothStone);
        self.put(0, 19, 0, M::BeaconLens);
        self.put(0, 25, 0, M::StoneFence);
        self.cottage(-9, 7, 1, M::HarborRoofTiles);
        for z in -10..=-4 {
            let y = self.floor(0, z).clamp(-2, 0);
            for x in -1..=1 {
                self.seat(x, z, y, M::BeachShingle);
                self.shaft(x, z, y + 1, y + 3, M::Air);
            }
        }
        self.locker(-9, 2, 9);
    }
    fn wreck(&mut self, waterline: i64) {
        // A grounded stern and a raised, listing bow keep the broken frames
        // visible even when the keel rests six metres below the surface.
        let lift = waterline.clamp(2, 8);
        for z in -12_i64..=12 {
            let width = match z.abs() {
                10.. => 1,
                8..=9 => 2,
                6..=7 => 3,
                _ => 4,
            };
            let keel = 1 + (7 - z).clamp(0, 16) * lift / 16 + i64::from(z > 8);
            self.shaft(0, z, self.floor(0, z).min(keel), keel, M::HarborTimber);
            for x in -width..=width {
                if x < 0 || z < -4 || (z + x) % 5 != 0 {
                    self.put(x, keel + x.abs() / 2, z, M::WeatheredWood);
                }
            }
            for side in [-1, 1] {
                let footing = keel + width / 2;
                let rib = keel + width + 1 + i64::from(side < 0) * 2;
                let x = side * width;
                if z.rem_euclid(3) == 0 {
                    for k in 1..=width {
                        self.put(side * k, keel + k - 1, z, M::HarborTimber);
                    }
                    self.shaft(x, z, footing, rib, M::HarborTimber);
                } else if side < 0 || !(-2..=6).contains(&z) {
                    // Intact lower strakes hold the bow together. Missing upper
                    // strakes form a real breach on the seaward side.
                    self.shaft(x, z, footing, rib - 2, M::SprucePlanks);
                    if z < -1 || (z + side).rem_euclid(5) < 3 {
                        self.put(x, rib - 1, z, M::WeatheredWood);
                    }
                }
            }
        }
        let mast_top = 14 + lift / 2;
        for y in 2..=mast_top {
            self.put((y - 2) / 4, y, -3 + (y - 2) / 5, M::HarborTimber);
        }
        let yard = mast_top - 2;
        let yard_x = (yard - 2) / 4;
        let yard_z = -3 + (yard - 2) / 5;
        for x in -4_i64..=4 {
            let y = yard - (x + 4) / 5;
            self.put(yard_x + x, y, yard_z, M::HarborTimber);
            if matches!(x, -3 | -2 | 0 | 2 | 3) {
                self.put(yard_x + x, y - 1, yard_z, M::WoolSlab);
            }
            if matches!(x, -3 | 2) {
                self.put(yard_x + x, y - 2, yard_z, M::FishingNet);
            }
        }
        for (x, y, z, m) in [
            (-3, 4, -4, M::FishingNet),
            (2, 3, 7, M::BarnacledBarrel),
            (1, 3, 3, M::WreckTimbers),
            (-1, 4, -1, M::LobsterPot),
        ] {
            self.put(x, y, z, m);
        }
        self.put(-5, self.floor(-5, -4) + 1, -4, M::ShipAnchor);
        self.locker(-1, 3, 6);
    }
    fn cave(&mut self) {
        // A narrow pier skirts the natural tidal channel. Leave the arched stone
        // ceiling and the water through its centre untouched.
        for z in -11..=5 {
            let x = 3;
            for dx in 0..=1 {
                self.put(x + dx, 1, z, M::WeatheredWood);
                self.shaft(x + dx, z, 2, 4, M::Air);
            }
            if (z + 11) % 4 == 0 {
                self.shaft(x, z, self.floor(x, z).min(-3), 1, M::HarborTimber);
                self.put(x, 2, z, M::MooringPost);
            }
        }
        self.put(4, 2, 3, M::Lantern);
        self.put(4, 2, -4, M::BarnacledBarrel);
        self.put(3, 2, -1, M::LobsterPot);
        self.prop(4, 4, 2, M::FishingNet, 1);
        self.locker(4, 2, 5);
    }
}
