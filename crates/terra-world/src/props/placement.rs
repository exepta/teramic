//! Shared four-place ground layout for generated and player-planted vegetation.
use super::{PropKind, WorldProp};

pub const CAPACITY: usize = 4;
const HALF_WIDTH: f64 = 0.25;
const SLOT_ORDER: [usize; CAPACITY] = [0, 3, 1, 2];

pub fn support(prop: &WorldProp) -> [i64; 3] {
    if prop.kind == PropKind::AppleFruit {
        return [
            prop.position[0].floor() as i64,
            (prop.position[1] + 0.43).floor() as i64,
            prop.position[2].floor() as i64,
        ];
    }
    if prop.kind == PropKind::SpiderWeb {
        return super::webs::support(prop);
    }
    if prop.wall_attached() {
        let (sin, cos) = f64::from(prop.rotation).sin_cos();
        return [
            (prop.position[0] - sin * 0.64).floor() as i64,
            prop.position[1].floor() as i64,
            (prop.position[2] - cos * 0.64).floor() as i64,
        ];
    }
    [
        prop.position[0].floor() as i64,
        (prop.position[1] - 1.0).floor() as i64,
        prop.position[2].floor() as i64,
    ]
}

/// A ground anchor must have a complete top surface, not a fluid, thin mineral
/// shaft, or oriented furniture whose logical solid cell is mostly empty.
pub fn ground_support(material: u16) -> bool {
    terra_voxel::MaterialId::from_raw(material).is_some_and(|m| {
        m.is_solid()
            && !crate::segments::is_segment(material)
            && !matches!(material, 14 | 28 | 29 | 59..=100)
    })
}

/// Nonpersistent visibility/interaction validation against actual loaded cells.
/// `None` means incomplete streaming, not a reason to tombstone a saved prop.
pub fn supported(prop: &WorldProp, mut block: impl FnMut([i64; 3]) -> Option<u16>) -> Option<bool> {
    if prop.kind == PropKind::AppleFruit {
        return Some(true);
    }
    if prop.kind == PropKind::SpiderWeb {
        let mut unknown = false;
        let fits = super::webs::fits(prop, |cell| {
            let material = block(cell);
            unknown |= material.is_none();
            material
        });
        return (!unknown).then_some(fits);
    }
    let material = block(support(prop))?;
    let anchor = if prop.kind.sapling().is_some() {
        matches!(material, 1 | 2 | terra_core::farming::FARMLAND)
    } else if prop.wall_attached() {
        matches!(material, 6 | 10 | 36 | 134)
    } else {
        ground_support(material)
    };
    if !anchor {
        return Some(false);
    }
    let root = prop.position.map(|v| v.floor() as i64);
    let material = block(root)?;
    Some(matches!(material, 0 | 5 | 98) || prop.water_tolerant() && matches!(material, 27 | 50))
}

pub fn cost(kind: PropKind) -> usize {
    if kind.sapling().is_some()
        || kind.crop().is_some()
        || matches!(kind, PropKind::Scarecrow | PropKind::SpiderWeb)
    {
        CAPACITY
    } else if kind == PropKind::Grass {
        2
    } else {
        1
    }
}

pub fn align(prop: &mut WorldProp, support: [i64; 3], slot: usize) {
    if prop.kind == PropKind::SpiderWeb {
        return;
    }
    if prop.kind != PropKind::Scarecrow {
        prop.rotation = ((prop.id.0 >> 32) & 3) as f32 * std::f32::consts::FRAC_PI_2;
    }
    prop.position = [
        support[0] as f64 + 0.5,
        support[1] as f64 + 1.0,
        support[2] as f64 + 0.5,
    ];
    if prop.kind == PropKind::Grass
        || prop.kind.aquatic()
        || prop.kind.crop().is_some()
        || prop.kind == PropKind::Scarecrow
        || prop.kind.sapling().is_some()
    {
        return;
    }
    let variation = ((prop.id.0 ^ (prop.id.0 >> 23)) & 0xffff) as f32 / 65535.0;
    prop.scale = match prop.kind {
        PropKind::Flower => (0.65 + variation * 0.30) / 1.3,
        PropKind::Mushroom => (0.35 + variation * 0.20) / 1.3,
        _ => prop.scale.min(1.0),
    };
    prop.position[0] += if slot & 1 == 0 {
        -HALF_WIDTH
    } else {
        HALF_WIDTH
    };
    prop.position[2] += if slot & 2 == 0 {
        -HALF_WIDTH
    } else {
        HALF_WIDTH
    };
}

pub fn generated_slot(rank: usize) -> usize {
    SLOT_ORDER[rank.min(CAPACITY - 1)]
}

pub(crate) fn compatible(prop: &WorldProp, cell: [i64; 3]) -> bool {
    if prop.wall_attached() || matches!(prop.kind, PropKind::SpiderWeb | PropKind::AppleFruit) {
        return false;
    }
    if prop.kind.sapling().is_some()
        || prop.kind.crop().is_some()
        || prop.kind == PropKind::Scarecrow
    {
        return [0, 2]
            .into_iter()
            .all(|axis| (prop.position[axis] - cell[axis] as f64 - 0.5).abs() <= 0.001);
    }
    if prop.kind == PropKind::Grass {
        let turns = prop.rotation / std::f32::consts::FRAC_PI_2;
        return prop.scale <= 1.02
            && (turns - turns.round()).abs() <= 0.001
            && [0, 2]
                .into_iter()
                .all(|axis| (prop.position[axis] - cell[axis] as f64 - 0.5).abs() <= 0.001);
    }
    footprint_radius(prop) <= HALF_WIDTH + 0.00001
}

fn footprint_radius(prop: &WorldProp) -> f64 {
    // Measure locally so repeated reflow cannot drift through world-coordinate rounding.
    let mut local = *prop;
    local.position = [0.0; 3];
    let bounds = local.bounds();
    [0, 2].into_iter().fold(0.0_f64, |radius, axis| {
        radius.max(bounds[0][axis].abs()).max(bounds[1][axis].abs())
    })
}

pub fn next_slot(props: &[WorldProp], cell: [i64; 3], kind: PropKind) -> Option<usize> {
    if kind == PropKind::AppleFruit {
        return None;
    }
    let mut used = 0;
    let mut count = 0;
    for prop in props
        .iter()
        .filter(|p| !p.removed && !p.growing_grass() && support(p) == cell)
    {
        if !compatible(prop, cell) || (kind == PropKind::Grass && prop.kind == PropKind::Grass) {
            return None;
        }
        used += cost(prop.kind);
        count += 1;
    }
    (used + cost(kind) <= CAPACITY).then(|| generated_slot(count))
}

/// Derived positions depend only on the active IDs on this support, never packet order.
/// Fixed-size scratch storage keeps procedural per-block grouping allocation-free.
pub fn reflow_group<'a>(props: impl Iterator<Item = &'a mut WorldProp>) {
    let mut group: [Option<&mut WorldProp>; CAPACITY] = std::array::from_fn(|_| None);
    let mut count = 0;
    let mut used = 0;
    let mut grass = false;
    let mut cell = None;
    let mut radius = 0.0_f64;
    for prop in props.filter(|p| !p.removed && !p.growing_grass()) {
        let support = support(prop);
        if count == CAPACITY
            || cell.is_some_and(|cell| cell != support)
            || !compatible(prop, support)
        {
            return;
        }
        cell = Some(support);
        used += cost(prop.kind);
        if used > CAPACITY || (grass && prop.kind == PropKind::Grass) {
            return;
        }
        grass |= prop.kind == PropKind::Grass;
        if prop.kind != PropKind::Grass {
            radius = radius.max(footprint_radius(prop));
        }
        group[count] = Some(prop);
        count += 1;
    }
    let Some(cell) = cell else { return };
    group[..count].sort_unstable_by_key(|p| p.as_ref().map(|p| p.id));
    let companions = count - usize::from(grass);
    let spread = if grass {
        HALF_WIDTH
    } else {
        (radius + 0.012).min(HALF_WIDTH)
    };
    let mut rank = 0;
    for prop in group.into_iter().flatten() {
        let offset = if prop.kind == PropKind::Grass || companions == 1 && !grass {
            [0.0, 0.0]
        } else if grass {
            let sign = if rank == 0 { -1.0 } else { 1.0 };
            [sign * spread; 2]
        } else {
            match companions {
                2 => [if rank == 0 { -spread } else { spread }, 0.0],
                3 => [[-spread, -spread], [spread, -spread], [0.0, spread]][rank],
                _ => [
                    if rank & 1 == 0 { -spread } else { spread },
                    if rank & 2 == 0 { -spread } else { spread },
                ],
            }
        };
        prop.position = [
            cell[0] as f64 + 0.5 + offset[0],
            cell[1] as f64 + 1.0,
            cell[2] as f64 + 0.5 + offset[1],
        ];
        rank += usize::from(prop.kind != PropKind::Grass);
    }
}

/// Rebuild only logical presentation positions; identities and journal records stay intact.
pub fn reflow(props: &mut [WorldProp]) {
    props.sort_unstable_by_key(|prop| (support(prop), prop.id));
    let mut remaining = props;
    while let Some(first) = remaining.first() {
        let cell = support(first);
        let end = remaining.partition_point(|prop| support(prop) == cell);
        let (group, rest) = remaining.split_at_mut(end);
        reflow_group(group.iter_mut());
        remaining = rest;
    }
}
