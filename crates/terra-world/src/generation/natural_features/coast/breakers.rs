//! Isolated, wave-cut granite on the shallow shelf off a misty shore.
//! The opening is aligned with the shore; fishing lanes stay clear around landmarks.
use super::{BTreeMap, Cell, Habitat, M, TILE_SIZE, Tile, ticket};

#[derive(Debug, Clone, Copy)]
struct Reef {
    along_x: bool,
    arch: bool,
    seed: u32,
    center: Tile,
    sea: i64,
}

impl Reef {
    fn at(self, u: i64, v: i64) -> Tile {
        if self.along_x {
            [self.center[0] + u, self.center[1] + v]
        } else {
            [self.center[0] + v, self.center[1] + u]
        }
    }
}

pub(super) fn build(h: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) -> bool {
    let Some(reef) = plan(h, tile) else {
        return false;
    };
    // Other natural features have first claim on their mineral beds and spans.
    if cells.iter().any(|(p, m)| {
        (p[0] - reef.center[0]).abs() <= 11
            && (p[2] - reef.center[1]).abs() <= 11
            && p[1] >= reef.sea - 1
            && *m != M::Air as u16
    }) {
        return false;
    }
    let span = 5 + i64::from((reef.seed >> 9) % 3);
    let crest = 8 + i64::from((reef.seed >> 15) % 5);
    for u in -span - 3..=span + 3 {
        for v in -5i64..=5 {
            let [x, z] = reef.at(u, v);
            let c = h.column([x, z]);
            let floor = (c.height - 0.5).floor() as i64;
            let r = ticket(reef.seed as u64, [x, z], 0x8383);
            let jag = f64::from(r % 100) / 100. - 0.5;
            let u = u as f64;
            let v = v as f64;
            let shoulder = if reef.arch {
                span as f64
            } else {
                span as f64 * 0.59
            };
            let axis = if u < 0. { -shoulder } else { shoulder };
            let radius = if u < 0. { 2.9 } else { 2.3 };
            let distance = ((u - axis).powi(2) + (v * 1.10).powi(2)).sqrt();
            let taper = (1. - distance / (radius + 0.5 + jag * 0.4)).clamp(0., 1.);
            // Batter and salt-notched edges, with a gently leaning crest on the taller stack.
            let mut top = if taper > 0. {
                reef.sea
                    + ((if u < 0. { crest } else { crest - 2 }) as f64 * taper.sqrt() + jag).round()
                        as i64
            } else {
                floor - 1
            };
            let mut bottom = floor;
            // A thick natural lintel grows from both shoulders. The broad curved
            // soffit directly faces the beach and stays open down to the sea floor.
            if reef.arch && u.abs() <= shoulder && v.abs() <= 2.0 + (u.abs() / shoulder) * 0.55 {
                let t = (u / shoulder).abs().min(1.);
                let underside =
                    reef.sea + 1 + ((1. - t * t).sqrt() * (crest - 4) as f64).round() as i64;
                top = top.max(
                    underside
                        + 2
                        + (v.abs() < 1.) as i64
                        + ((u + v).round() as i64).rem_euclid(3) / 2,
                );
                if taper < 0.28 {
                    bottom = underside;
                }
            }
            if top < bottom {
                continue;
            }
            for y in bottom..=top {
                let bed = (y + u as i64 / 4 + v as i64 / 3).rem_euclid(8);
                let material = if y == top && y > reef.sea + 3 && r % 8 < 3 {
                    M::LichenCoastalStone
                } else if y <= reef.sea + 2 {
                    M::TidewornRock
                } else if bed == 1 {
                    M::SaltStainedStone
                } else {
                    M::CoastalGranite
                };
                cells.insert([x, y, z], material as u16);
            }
        }
    }
    true
}

fn plan(h: &mut Habitat<'_>, tile: Tile) -> Option<Reef> {
    let cfg = h.terrain.config();
    let seed = cfg.seed.0;
    let sector = tile.map(|v| v.div_euclid(3));
    let draw = ticket(seed, sector, 0x5ea5_7ac5);
    if tile.map(|v| v.rem_euclid(3)) != [i64::from(draw % 3), i64::from((draw >> 9) % 3)] {
        return None;
    }
    let center = tile.map(|v| v * TILE_SIZE + 16);
    let c = h.column(center);
    let sea = cfg.sea_level.floor() as i64;
    if c.biome_kind(cfg.sea_level) != terra_core::BiomeKind::Ocean
        || !(2.5..=14.).contains(&(cfg.sea_level - c.height))
        || h.reserved(center, 14)
    {
        return None;
    }
    // A nearby dry salt meadow or cliff, not just water of the correct depth.
    let mut shore = None;
    'rings: for n in [19, 32, 49, 68, 86] {
        for [dx, dz] in [
            [1, 0],
            [0, 1],
            [-1, 0],
            [0, -1],
            [1, 1],
            [-1, 1],
            [1, -1],
            [-1, -1],
        ] {
            let (x, z) = (
                (center[0] + n * dx) as f64 + 0.5,
                (center[1] + n * dz) as f64 + 0.5,
            );
            let b = h.terrain.climate_biome_at(x, z).weights;
            if b.coastal_land_fraction() > 0.55
                && b.ocean < 0.6
                && h.terrain.surface_height(x, z) > cfg.sea_level + 2.
            {
                shore = Some([dx, dz]);
                break 'rings;
            }
        }
    }
    let direction = shore?;
    // Keep the site and the first seaward approach clear of both piers and wrecks.
    for dz in -1..=1 {
        for dx in -1..=1 {
            if (dx == 0 || dz == 0)
                && h.terrain
                    .coastal_plan([tile[0] + dx, tile[1] + dz])
                    .is_some()
            {
                return None;
            }
        }
    }
    let reef = Reef {
        center,
        along_x: direction[1] != 0,
        sea,
        arch: !(draw >> 7).is_multiple_of(3),
        seed: draw,
    };
    for (u, v) in [(-9, 0), (9, 0), (0, -4), (0, 4), (0, 0)] {
        let c = h.column(reef.at(u, v));
        if c.water_level
            .is_none_or(|w| (w - cfg.sea_level).abs() > 0.15)
            || c.height > cfg.sea_level - 1.
            || c.height < cfg.sea_level - 18.
        {
            return None;
        }
    }
    Some(reef)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SectionGenerator, TerraGenerator};
    use std::collections::BTreeSet;
    use terra_voxel::global_to_section;

    #[test]
    fn breakers_rise_from_the_seabed_and_waves_pass_under_the_stone_door() {
        let terrain = TerraGenerator::new(super::super::tests::config()).unwrap();
        let g = SectionGenerator::new(terrain.clone());
        for (tile, door) in [([1, -2], true), ([15, -9], false)] {
            let mut habitat = Habitat {
                terrain: &terrain,
                trees: vec![],
                sites: vec![],
                columns: Default::default(),
            };
            let reef = plan(&mut habitat, tile).unwrap();
            assert_eq!(reef.arch, door);
            assert!(terrain.coastal_landmark_at(tile).is_none());
            let patch = terrain.natural_patch(tile);
            let material = |id: u16| matches!(id,774..=776|779..=780);
            let mut rock: BTreeSet<_> = patch
                .cells
                .iter()
                .filter_map(|(p, id)| material(*id).then_some(*p))
                .collect();
            let mut waveground = Vec::new();
            for p in &rock {
                let floor = (habitat.column([p[0], p[2]]).height - 0.5).floor() as i64;
                if p[1] == floor {
                    waveground.push(*p);
                }
            }
            assert!(rock.len() > 100 && waveground.len() > 10);
            while let Some(p) = waveground.pop() {
                if !rock.remove(&p) {
                    continue;
                }
                for axis in 0..3 {
                    for step in [-1, 1] {
                        let mut n = p;
                        n[axis] += step;
                        waveground.push(n);
                    }
                }
            }
            assert!(
                rock.is_empty(),
                "visible rock has no face-connected route to its own seabed"
            );
            // The cap belongs to a streamed, individually mineable terrain cell.
            let [x, z] = reef.at(0, 0);
            let cap =
                (reef.sea + 2..reef.sea + 18).find(|y| material(g.block_at([x, *y, z]).unwrap()));
            assert_eq!(cap.is_some(), door);
            if let Some(y) = cap {
                let pos = [x, y, z];
                let (section, local) = global_to_section(pos);
                assert_eq!(
                    g.generate(section).unwrap().block(local).unwrap(),
                    g.block_at(pos).unwrap()
                );
            }
            for v in -4..=4 {
                let [x, z] = reef.at(0, v);
                assert_eq!(g.block_at([x, reef.sea + 1, z]).unwrap(), M::Air as u16);
                assert!(
                    terrain
                        .surface_column(x as f64 + 0.5, z as f64 + 0.5)
                        .water_columns()
                        .at(reef.sea - 1)
                        .is_some()
                );
            }
        }
    }
}
