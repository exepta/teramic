//! Surface-attached steering uses the same edited voxel shapes as other animals.
use super::{Behavior, Creature, CreaturePopulation, Habitat, Kind, distance, random};
use terra_core::creatures::intentions::IntentReason;
use terra_core::creatures::{CreatureNeeds, NeedGoal, NeedKind, spider_id};
pub mod cycle;
mod surfaces;
pub(super) use surfaces::Route as SurfaceRoute;

pub fn pursuit_speed(kind: Kind) -> f64 {
    kind.speed() * 1.35 * 1.6
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bite {
    pub player: usize,
    pub damage: f32,
    pub poison_level: u8,
    pub poison_seconds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WebRequest {
    pub anchor: [i64; 3],
    pub face: [i8; 3],
    pub variant: u8,
    pub id: u64,
    /// Zero keeps a legacy attachment; 8..21 is the distance between real rock anchors.
    pub separation: u8,
}

impl WebRequest {
    pub fn prop(self) -> Option<crate::props::WorldProp> {
        if self.separation == 0 {
            crate::props::webs::attached(
                self.anchor,
                self.face,
                self.variant,
                crate::props::PropId(self.id),
            )
        } else {
            crate::props::webs::spanning(
                self.anchor,
                self.face,
                self.separation,
                self.variant,
                crate::props::PropId(self.id),
            )
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Outcome {
    pub bite: Option<Bite>,
    pub web: Option<WebRequest>,
}

pub fn in_cave(a: &Creature, h: &Habitat) -> bool {
    let surface = h.terrain.surface_height(a.position[0], a.position[2]);
    a.position[1] + a.size()[1] < surface - 3.0
}

pub fn den_request(a: &Creature, h: &Habitat) -> Option<WebRequest> {
    let anchor = contact(h, a)?;
    let face = terra_core::creatures::spiders::surface_normal(a);
    let mut request = WebRequest {
        anchor,
        face,
        variant: if face[1] < 0 {
            2
        } else if face[1] > 0 {
            5
        } else {
            0
        },
        id: random(a.id, 0x0044_454e) | (1 << 60),
        separation: 0,
    };
    if face[1] == 0
        && let Some(prop) = crate::props::webs::find_spanning(
            anchor,
            face,
            crate::props::PropId(request.id),
            |position| web_block(h, position),
        )
    {
        request.variant = prop.variant;
        request.separation = (prop.scale * crate::props::webs::TEMPLATE_SPAN + 1.024).round() as u8;
    }
    Some(request)
}

fn web_block(h: &Habitat, position: [i64; 3]) -> Option<u16> {
    if h.terrain.config().bounded_world && !(-128..=356).contains(&position[1]) {
        return None;
    }
    h.block(position).map(|block| {
        if block == 0 && h.wet(position.map(|v| v as f64 + 0.01)) {
            terra_voxel::MaterialId::FluidWater as u16
        } else {
            block
        }
    })
}

/// A removed den advances its persistent identity once, without scanning tombstones.
pub fn replacement_den_id(initial: u64, previous: u64) -> u64 {
    let next = random(initial, previous ^ 0x5245_4255_494c_4400) | (1 << 60);
    if next == previous { next ^ 1 } else { next }
}

fn den_fits(request: WebRequest, h: &Habitat) -> bool {
    request.prop().is_some_and(|prop| {
        crate::props::webs::fits_in_world(&prop, h.terrain.config().bounded_world, |position| {
            web_block(h, position)
        })
    })
}

fn den_at(
    a: &Creature,
    h: &Habitat,
    anchor: [i64; 3],
    normal: [i8; 3],
    owned: bool,
) -> Option<Creature> {
    let center = terra_core::creatures::spiders::visual_center(a);
    let mut probe = a.clone();
    for axis in 0..3 {
        if normal[axis] == 0 {
            probe.position[axis] += anchor[axis] as f64 + 0.5 - center[axis];
        }
    }
    attach_with_depth(h, &probe, anchor, normal, owned)
}

fn den_goal(a: &Creature, h: &Habitat, owned: bool) -> Option<[i64; 3]> {
    let origin = contact(h, a)?;
    let normal = terra_core::creatures::spiders::surface_normal(a);
    let axis = normal.iter().position(|v| *v != 0)?;
    let tangents = match axis {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    };
    // Keep the original planar probes, then consider nearby raised/recessed
    // rock. All 64 candidates share the existing eight-search server budget.
    for depth in [0, -1, 1] {
        for radius in 1..=if depth == 0 { 4 } else { 2 } {
            for [left, right] in [
                [1, 0],
                [0, 1],
                [-1, 0],
                [0, -1],
                [1, 1],
                [-1, 1],
                [-1, -1],
                [1, -1],
            ] {
                let mut anchor = origin;
                anchor[axis] += depth;
                anchor[tangents[0]] += left * radius;
                anchor[tangents[1]] += right * radius;
                if let Some(candidate) = den_at(a, h, anchor, normal, owned)
                    && let Some(request) = den_request(&candidate, h)
                    && den_fits(request, h)
                {
                    return Some(
                        terra_core::creatures::spiders::visual_center(&candidate)
                            .map(|v| v.floor() as i64),
                    );
                }
            }
        }
    }
    None
}

/// Search a bounded local shell for a reachable wall face, rather than dropping
/// a tiny floor patch while an entire dry chamber can hold a real construction.
pub fn construction_goal(a: &Creature, h: &Habitat, owned: bool) -> Option<[i64; 3]> {
    if !in_cave(a, h) || contact(h, a).is_none() {
        return None;
    }
    let center = terra_core::creatures::spiders::visual_center(a);
    let origin = center.map(|v| v.floor() as i64);
    let mut candidates = 0;
    for height in [4, 6, 2, 0] {
        for direction in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            for distance in 1..=10 {
                let mut anchor = std::array::from_fn(|i| origin[i] + direction[i] * distance);
                anchor[1] += height;
                let Some(block) = h.block(anchor) else { break };
                if block == 0 {
                    continue;
                }
                let normal = direction.map(|v| -v as i8);
                if let Some(candidate) = den_at(a, h, anchor, normal, owned) {
                    candidates += 1;
                    if let Some(request) = den_request(&candidate, h)
                        && request.separation != 0
                    {
                        return Some(
                            terra_core::creatures::spiders::visual_center(&candidate)
                                .map(|v| v.floor() as i64),
                        );
                    }
                }
                if candidates >= 6 {
                    return None;
                }
                break;
            }
        }
    }
    None
}

/// Before acquiring a den, spiders still move and obey real loaded surface contact.
/// No player combat or food search occurs until the server commits an actual web.
pub fn prepare_den(
    a: &mut Creature,
    h: &Habitat,
    dt: f32,
    search: bool,
    navigation: &mut super::navigation::Navigation,
) -> Option<WebRequest> {
    prepare_den_with_access(a, h, dt, search, navigation, false)
}

pub fn prepare_owned_den(
    a: &mut Creature,
    h: &Habitat,
    dt: f32,
    search: bool,
    navigation: &mut super::navigation::Navigation,
) -> Option<WebRequest> {
    prepare_den_with_access(a, h, dt, search, navigation, true)
}

fn prepare_den_with_access(
    a: &mut Creature,
    h: &Habitat,
    dt: f32,
    search: bool,
    navigation: &mut super::navigation::Navigation,
    owned: bool,
) -> Option<WebRequest> {
    if !dt.is_finite()
        || !(0.0..=0.25).contains(&dt)
        || dt == 0.0
        || a.health <= 0.0
        || h.block(a.position.map(|v| v.floor() as i64)).is_none()
    {
        return None;
    }
    a.age += f64::from(dt);
    a.timer = (a.timer - dt).max(0.0);
    a.needs.search_in = (a.needs.search_in - dt).max(0.0);
    a.needs.food = (a.needs.food - dt * 0.016).max(0.0);
    a.needs.water = 0.0;
    if a.needs.food == 0.0 {
        a.health = (a.health - dt * 0.025).max(0.0);
    }
    if a.health == 0.0 {
        a.behavior = Behavior::Dead;
        a.velocity = [0.0; 3];
        a.needs.goal = None;
        return None;
    }
    if submersion_damage(a, h, dt) {
        return None;
    }
    if a.behavior == Behavior::Descend {
        falling(a, h, dt, true);
        return None;
    }
    if contact(h, a).is_none() {
        detach_for_gravity(a, h);
        navigation.forget(a.id);
        navigation.intend(a, IntentReason::Unsupported, None);
        falling(a, h, dt, false);
        return None;
    }
    if search {
        let request =
            den_request(a, h).filter(|request| request.separation != 0 || den_fits(*request, h));
        if let Some(request) = request.filter(|request| request.separation != 0) {
            a.velocity = [0.0; 3];
            a.timer = 0.0;
            a.needs.search_in = 1.0;
            a.needs.goal = None;
            navigation.forget(a.id);
            navigation.intend(a, IntentReason::Surface, Some(a.position));
            return Some(request);
        }
        let reached = a.needs.goal.is_some_and(|goal| {
            distance(
                terra_core::creatures::spiders::visual_center(a),
                goal.position.map(|v| v as f64 + 0.5),
            ) < 0.8_f64.powi(2)
        });
        if a.timer <= 0.0
            || a.needs
                .goal
                .is_none_or(|goal| goal.kind != NeedKind::Shelter)
            || reached
        {
            let construction = construction_goal(a, h, owned);
            if construction.is_none()
                && let Some(request) = request
            {
                a.velocity = [0.0; 3];
                a.timer = 0.0;
                a.needs.search_in = 1.0;
                a.needs.goal = None;
                navigation.forget(a.id);
                navigation.intend(a, IntentReason::Surface, Some(a.position));
                return Some(request);
            }
            a.needs.goal = construction
                .or_else(|| den_goal(a, h, owned))
                .map(|position| NeedGoal {
                    kind: NeedKind::Shelter,
                    position,
                });
            a.timer = 4.0;
        }
        if a.needs.goal.is_none()
            && let Some(request) = request
        {
            a.velocity = [0.0; 3];
            a.timer = 0.0;
            a.needs.search_in = 1.0;
            navigation.forget(a.id);
            navigation.intend(a, IntentReason::Surface, Some(a.position));
            return Some(request);
        }
        a.needs.search_in = 1.0;
    }
    let center = terra_core::creatures::spiders::visual_center(a);
    let normal = terra_core::creatures::spiders::surface_normal(a);
    let seed = random(a.id, (a.age / 4.0) as u64);
    let yaw = (seed % 628) as f64 * 0.01;
    let destination = a
        .needs
        .goal
        .filter(|goal| goal.kind == NeedKind::Shelter)
        .map_or_else(
            || {
                if normal[1] == 0 {
                    [
                        center[0],
                        center[1] + if seed.is_multiple_of(2) { 3.0 } else { -3.0 },
                        center[2],
                    ]
                } else {
                    [
                        center[0] - yaw.sin() * 4.0,
                        center[1],
                        center[2] - yaw.cos() * 4.0,
                    ]
                }
            },
            |goal| goal.position.map(|v| v as f64 + 0.5),
        );
    navigation.intend(a, IntentReason::Surface, Some(destination));
    if normal[1] > 0 {
        a.behavior = Behavior::Crawl;
    }
    surfaces::advance(
        a,
        h,
        destination,
        (dt, a.kind.speed(), owned),
        search,
        Some(navigation),
    );
    None
}

pub(super) fn submersion_damage(a: &mut Creature, h: &Habitat, dt: f32) -> bool {
    if h.wet(terra_core::creatures::spiders::visual_center(a)) {
        a.health = (a.health - dt * 0.5).max(0.0);
        if a.health == 0.0 {
            a.behavior = Behavior::Dead;
            a.velocity = [0.0; 3];
            a.needs.goal = None;
            return true;
        }
    }
    false
}

fn clear(h: &Habitat, a: &Creature) -> bool {
    let yaw = if a.behavior == Behavior::Climb {
        0.0
    } else {
        f64::from(a.yaw)
    };
    h.clear_swimmer_rotated(a.position, a.size(), yaw)
}

fn within_roaming_depth(a: &Creature) -> bool {
    a.kind != Kind::LargeSpider
        || terra_core::creatures::spawning::is_command_id(a.id)
        || a.position[1] + a.size()[1] <= -22.0
}

fn shape_contains(h: &Habitat, point: [f64; 3]) -> Option<[i64; 3]> {
    let cell = point.map(|v| v.floor() as i64);
    let block = h.block(cell)?;
    if !terra_voxel::MaterialId::from_raw(block).is_some_and(|b| b.is_solid()) {
        return None;
    }
    let mut contains = false;
    let shaped = h.boxes(cell, |shape| {
        contains |= (0..3).all(|i| point[i] >= shape.minimum[i] && point[i] < shape.maximum[i]);
    });
    (!shaped || contains).then_some(cell)
}

fn contact(h: &Habitat, a: &Creature) -> Option<[i64; 3]> {
    let normal = terra_core::creatures::spiders::surface_normal(a);
    let size = a.size();
    let center = terra_core::creatures::spiders::visual_center(a);
    let axis = normal.iter().position(|v| *v != 0)?;
    let mut point = center;
    point[axis] -= f64::from(normal[axis]) * (size[axis] * 0.5 + 0.055);
    if let Some(cell) = shape_contains(h, point) {
        return Some(cell);
    }
    // Broad legs can retain a real ledge contact while the body crosses its edge.
    for tangent in 0..3 {
        if tangent == axis {
            continue;
        }
        for sign in [-1.0, 1.0] {
            let mut probe = point;
            probe[tangent] += size[tangent] * 0.35 * sign;
            if let Some(cell) = shape_contains(h, probe) {
                return Some(cell);
            }
        }
    }
    // An outer foot can touch a terrace although the center and axial probes
    // are above air. Rotate floor footprints with the actual occupied box.
    let axes = tangent_axes(normal);
    for [u, v] in [[-1.0, -1.0], [-1.0, 1.0], [1.0, -1.0], [1.0, 1.0]] {
        let mut probe = point;
        if axis == 1 {
            let (sin, cos) = f64::from(a.yaw).sin_cos();
            let x = u * size[0] * 0.47;
            let z = v * size[2] * 0.47;
            probe[0] += x * cos + z * sin;
            probe[2] += -x * sin + z * cos;
        } else {
            probe[axes[0]] += u * size[axes[0]] * 0.47;
            probe[axes[1]] += v * size[axes[1]] * 0.47;
        }
        if let Some(cell) = shape_contains(h, probe) {
            return Some(cell);
        }
    }
    if normal[1] > 0 {
        let (sin, cos) = f64::from(a.yaw).sin_cos();
        let half = [
            (size[0] * cos.abs() + size[2] * sin.abs()) * 0.5,
            (size[0] * sin.abs() + size[2] * cos.abs()) * 0.5,
        ];
        let lower = [
            a.position[0] - half[0],
            a.position[1] - 0.075,
            a.position[2] - half[1],
        ];
        let upper = [
            a.position[0] + half[0] - 0.000001,
            a.position[1] - 0.015,
            a.position[2] + half[1] - 0.000001,
        ];
        let min = lower.map(|v| v.floor() as i64);
        let max = upper.map(|v| v.floor() as i64);
        let pose = (
            [a.position[0], lower[1], a.position[2]],
            [size[0], upper[1] - lower[1], size[2]],
            f64::from(a.yaw),
        );
        for x in min[0]..=max[0] {
            for z in min[2]..=max[2] {
                for y in min[1]..=max[1] {
                    let cell = [x, y, z];
                    if h.block(cell).is_some() && h.obstructs(cell, lower, upper, pose) {
                        return Some(cell);
                    }
                }
            }
        }
    }
    None
}

fn attach(h: &Habitat, a: &Creature, anchor: [i64; 3], normal: [i8; 3]) -> Option<Creature> {
    attach_with_depth(h, a, anchor, normal, false)
}

fn attach_with_depth(
    h: &Habitat,
    a: &Creature,
    anchor: [i64; 3],
    normal: [i8; 3],
    owned: bool,
) -> Option<Creature> {
    let block = h.block(anchor)?;
    if !terra_voxel::MaterialId::from_raw(block).is_some_and(|b| b.is_solid()) {
        return None;
    }
    let axis = normal.iter().position(|v| *v != 0)?;
    let mut next = a.clone();
    next.behavior = if normal[1] < 0 {
        Behavior::Ceiling
    } else if normal[1] > 0 {
        Behavior::Crawl
    } else {
        Behavior::Climb
    };
    if axis != 1 {
        next.yaw = f32::from(normal[0])
            .atan2(f32::from(normal[2]))
            .rem_euclid(std::f32::consts::TAU);
    }
    let size = next.size();
    let center = terra_core::creatures::spiders::visual_center(a);
    let mut result = None;
    let mut best_distance = f64::INFINITY;
    let mut visit = |shape: terra_core::block_shapes::ShapeBox| {
        let face = if normal[axis] > 0 {
            shape.maximum[axis]
        } else {
            shape.minimum[axis]
        };
        let mut at = center;
        at[axis] = face + f64::from(normal[axis]) * (size[axis] * 0.5 + 0.015);
        let old_normal = terra_core::creatures::spiders::surface_normal(a);
        if axis == 1 && old_normal[1] == 0 {
            for side in [0, 2] {
                if old_normal[side] == 0 {
                    continue;
                }
                if normal[1] > 0 {
                    at[side] = (shape.minimum[side] + shape.maximum[side]) * 0.5;
                } else {
                    at[side] += f64::from(old_normal[side])
                        * ((size[side] - a.size()[side]) * 0.5).max(0.0);
                }
            }
        }
        if axis != 1 && terra_core::creatures::spiders::surface_normal(a)[1] > 0 {
            at[1] = at[1].max(a.position[1] + size[1] * 0.5 + 0.015);
        }
        next.position = [at[0], at[1] - size[1] * 0.5, at[2]];
        if (owned || within_roaming_depth(&next))
            && clear(h, &next)
            && contact(h, &next).is_some()
            && distance(at, center) < best_distance
        {
            best_distance = distance(at, center);
            result = Some(next.clone());
        }
    };
    if !h.boxes(anchor, &mut visit) {
        visit(terra_core::block_shapes::ShapeBox {
            minimum: anchor.map(|v| v as f64),
            maximum: anchor.map(|v| v as f64 + 1.0),
            hinged: false,
        });
    }
    result
}

fn line_of_sight(h: &Habitat, from: [f64; 3], to: [f64; 3]) -> bool {
    super::perception::visible(h, from, to)
}

fn corner_goal(h: &Habitat, a: &Creature) -> Option<[i64; 3]> {
    let here = a.position.map(|v| v.floor() as i64);
    let mut best = None;
    for dx in -4_i64..=4 {
        for dz in -4_i64..=4 {
            if dx * dx + dz * dz > 25 {
                continue;
            }
            let cell = [here[0] + dx, here[1], here[2] + dz];
            if h.block(cell) != Some(0) {
                continue;
            }
            let walls = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]
                .into_iter()
                .filter(|d| {
                    shape_contains(h, std::array::from_fn(|i| (cell[i] + d[i]) as f64 + 0.5))
                        .is_some()
                })
                .count();
            if walls == 0 {
                continue;
            }
            let variety = random(
                a.id ^ (cell[0] as u64).rotate_left(17) ^ cell[2] as u64,
                (a.age / 20.0) as u64,
            ) % 23;
            let score = walls as i64 * 32 - dx * dx - dz * dz + variety as i64;
            if best.is_none_or(|(_, old)| score > old) {
                best = Some((cell, score));
            }
        }
    }
    best.map(|(cell, _)| cell)
}

/// Only loaded, opaque cover counts. Thick natural roofs are preferred over
/// temporary single-block shade; missing sections never imply a safe refuge.
fn shelter(h: &Habitat, a: &Creature) -> Option<bool> {
    if a.position[1] < -22.0
        || !clear(h, a)
        || h.wet(terra_core::creatures::spiders::visual_center(a))
    {
        return None;
    }
    let center = terra_core::creatures::spiders::visual_center(a);
    let top = (a.position[1] + a.size()[1] + 0.05).ceil() as i64;
    let x = center[0].floor() as i64;
    let z = center[2].floor() as i64;
    for y in top..top + 16 {
        let material = h.block([x, y, z])?;
        if matches!(material, 7..=9 | 132 | 133 | 310..=312 | 15 | 80 | 88 | 96) {
            continue;
        }
        if terra_voxel::MaterialId::from_raw(material).is_some_and(|m| m.is_solid()) {
            return Some((1..=2).all(|dy| {
                h.block([x, y + dy, z]).is_some_and(|m| {
                    !matches!(m, 7..=9 | 132 | 133 | 310..=312 | 15 | 80 | 88 | 96)
                        && terra_voxel::MaterialId::from_raw(m).is_some_and(|m| m.is_solid())
                })
            }));
        }
    }
    None
}

fn shelter_goal(h: &Habitat, a: &Creature) -> Option<[i64; 3]> {
    let mut candidate = a.clone();
    candidate.behavior = Behavior::Crawl;
    let mut best: Option<([i64; 3], f64)> = None;
    let mut tested = 0;
    for ring in 0..=4 {
        for [dx, dz] in [
            [1, 0],
            [0, 1],
            [-1, 0],
            [0, -1],
            [1, 1],
            [-1, 1],
            [-1, -1],
            [1, -1],
        ] {
            if ring == 0 && [dx, dz] != [1, 0] {
                continue;
            }
            for drop in (0..=20).step_by(2) {
                tested += 1;
                if tested > 256 {
                    return best.map(|(position, _)| position);
                }
                let probe = [
                    a.position[0] + f64::from(dx * ring * 2),
                    a.position[1] - f64::from(drop),
                    a.position[2] + f64::from(dz * ring * 2),
                ];
                if probe[1] < -22.0 {
                    continue;
                }
                let Some(ground) = h.ground_rotated(probe, a.kind, f64::from(a.yaw)) else {
                    continue;
                };
                candidate.position = ground;
                let Some(cave_roof) = shelter(h, &candidate) else {
                    continue;
                };
                if contact(h, &candidate).is_none() {
                    continue;
                }
                let score = distance(a.position, ground) + if cave_roof { 0.0 } else { 400.0 };
                if best.is_none_or(|(_, old)| score < old) {
                    best = Some((ground.map(|v| v.floor() as i64), score));
                }
            }
        }
    }
    best.map(|(position, _)| position)
}

fn shelter_position(goal: NeedGoal) -> [f64; 3] {
    [
        goal.position[0] as f64 + 0.5,
        goal.position[1] as f64 + 0.025,
        goal.position[2] as f64 + 0.5,
    ]
}

fn retreat(
    a: &mut Creature,
    h: &Habitat,
    dt: f32,
    search: bool,
    mut navigation: Option<&mut super::navigation::Navigation>,
) {
    if let Some(goal) = a.needs.goal.filter(|goal| goal.kind == NeedKind::Shelter) {
        let mut candidate = a.clone();
        candidate.position = shelter_position(goal);
        candidate.behavior = Behavior::Crawl;
        if shelter(h, &candidate).is_none() {
            a.needs.goal = None;
            a.needs.search_in = 0.0;
            if let Some(nav) = navigation.as_deref_mut() {
                nav.forget(a.id);
            }
        }
    }
    if search && a.needs.search_in <= 0.0 {
        a.needs.goal = shelter_goal(h, a).map(|position| NeedGoal {
            kind: NeedKind::Shelter,
            position,
        });
        a.needs.search_in = 8.0 + (a.id % 6) as f32;
    }
    if shelter(h, a).is_some() {
        // Reaching cover is not an instruction to remain AFK all day. Keep
        // short staggered pauses, then explore the cave instead of targeting
        // this same shelter cell forever.
        a.needs.goal = None;
        if (a.age + (a.id % 17) as f64).rem_euclid(12.0) < 3.0 {
            if let Some(nav) = navigation.as_deref_mut() {
                nav.forget(a.id);
                nav.intend(a, IntentReason::DayRefuge, None);
            }
            if !matches!(a.behavior, Behavior::Climb | Behavior::Ceiling) {
                a.behavior = Behavior::Rest;
            }
            a.velocity = [0.0; 3];
            return;
        }
    }
    let center = terra_core::creatures::spiders::visual_center(a);
    let seed = random(a.id, (a.age / 6.0) as u64);
    let yaw = (seed % 628) as f64 * 0.01;
    let mut destination = a
        .needs
        .goal
        .filter(|goal| goal.kind == NeedKind::Shelter)
        .map_or(
            [
                center[0] - yaw.sin() * 4.0,
                center[1],
                center[2] - yaw.cos() * 4.0,
            ],
            shelter_position,
        );
    if a.behavior == Behavior::Climb && (destination[1] - center[1]).abs() < 0.65 {
        destination[1] = center[1] + 3.0;
    }
    if let Some(nav) = navigation.as_deref_mut() {
        nav.intend(
            a,
            if a.needs.goal.is_some() {
                IntentReason::DayRefuge
            } else {
                IntentReason::Explore
            },
            Some(destination),
        );
    }
    let ground = !matches!(a.behavior, Behavior::Climb | Behavior::Ceiling);
    if ground {
        a.behavior = Behavior::Crawl;
    }
    if !surfaces::advance(
        a,
        h,
        destination,
        (dt, a.kind.speed(), false),
        search,
        navigation.as_deref_mut(),
    ) && ground
    {
        // A disconnected room below this floor is not a reachable refuge.
        // Keep the search cooldown and explore instead of snapping back to it.
        if search {
            a.needs.goal = None;
            if let Some(nav) = navigation {
                nav.forget(a.id);
            }
        }
        move_surface(
            a,
            h,
            [
                center[0] - yaw.sin() * 4.0,
                center[1],
                center[2] - yaw.cos() * 4.0,
            ],
            dt,
            a.kind.speed(),
        );
    }
}

fn falling(a: &mut Creature, h: &Habitat, dt: f32, silk: bool) {
    let before = a.position;
    a.velocity[1] = if silk {
        -0.55
    } else {
        (a.velocity[1] - 12.0 * dt).max(-9.0)
    };
    let dy = f64::from(a.velocity[1] * dt);
    let count = (dy.abs() / 0.12).ceil().max(1.0) as usize;
    for _ in 0..count {
        let mut next = a.clone();
        next.position[1] += dy / count as f64;
        if clear(h, &next) {
            a.position = next.position;
        } else {
            let below = [
                a.position[0].floor() as i64,
                (a.position[1] - 0.15).floor() as i64,
                a.position[2].floor() as i64,
            ];
            let anchor = contact(h, a).unwrap_or(below);
            if let Some(next) = attach_with_depth(h, a, anchor, [0, 1, 0], true) {
                a.position = next.position;
                a.behavior = Behavior::Crawl;
                a.needs.goal = None;
            } else if silk {
                // A newly blocked silk line must not retain a permanent
                // suspended state. Ordinary gravity/surface navigation resumes.
                a.behavior = Behavior::Crawl;
                a.needs.goal = None;
            }
            break;
        }
        if contact(h, a).is_some() {
            if !matches!(a.behavior, Behavior::Climb | Behavior::Ceiling) {
                a.behavior = Behavior::Crawl;
            }
            a.needs.goal = None;
            break;
        }
    }
    a.velocity = std::array::from_fn(|i| ((a.position[i] - before[i]) / f64::from(dt)) as f32);
}

fn detach_for_gravity(a: &mut Creature, h: &Habitat) {
    let center = terra_core::creatures::spiders::visual_center(a);
    let mut floor = a.clone();
    floor.behavior = Behavior::Crawl;
    floor.position[1] = center[1] - floor.size()[1] * 0.5;
    // Losing a wall does not permit a wider floor-oriented body to overlap
    // adjacent rock. Keep the previous orientation while gravity clears it.
    if clear(h, &floor) {
        *a = floor;
    }
    a.needs.goal = None;
}

fn move_surface(a: &mut Creature, h: &Habitat, destination: [f64; 3], dt: f32, speed: f64) -> bool {
    move_surface_with_depth(a, h, destination, dt, speed, false)
}

fn move_surface_with_depth(
    a: &mut Creature,
    h: &Habitat,
    destination: [f64; 3],
    dt: f32,
    speed: f64,
    owned: bool,
) -> bool {
    // Attachment pivots alter the model base, not locomotion speed.
    a.velocity = [0.0; 3];
    if !dt.is_finite()
        || dt <= 0.0
        || dt > 0.25
        || !speed.is_finite()
        || speed <= 0.0
        || !super::navigation::valid_point(a.position)
        || !super::navigation::valid_point(destination)
    {
        return false;
    }
    let speed = speed * a.growth_scale();
    let steps = (speed * f64::from(dt) / 0.12).ceil().clamp(1.0, 32.0) as usize;
    let mut moved = false;
    for _ in 0..steps {
        let surface = terra_core::creatures::spiders::surface_normal(a);
        if !move_surface_step(a, h, destination, dt / steps as f32, speed, owned) {
            break;
        }
        moved = true;
        // A route edge is steered in its original tangent plane. Preserve the
        // real corner contact as a node; otherwise remaining substeps can chase
        // that old floor-height target back off the wall within the same edge.
        if terra_core::creatures::spiders::surface_normal(a) != surface {
            break;
        }
    }
    moved
}

pub(super) fn companion_move(
    a: &mut Creature,
    h: &Habitat,
    destination: [f64; 3],
    dt: f32,
    moving: bool,
    owned: bool,
) {
    if a.behavior == Behavior::Descend {
        falling(a, h, dt, true);
        return;
    }
    if contact(h, a).is_none() {
        detach_for_gravity(a, h);
        falling(a, h, dt, false);
        return;
    }
    if moving {
        if a.behavior == Behavior::Ceiling && destination[1] < a.position[1] - 1.0 {
            let anchor = contact(h, a);
            let center = terra_core::creatures::spiders::visual_center(a);
            a.position[1] = center[1]
                - terra_core::creatures::spiders::floor_size(a.kind)[1] * a.growth_scale() * 0.5;
            a.behavior = Behavior::Descend;
            a.needs.goal = anchor.map(|position| NeedGoal {
                kind: NeedKind::Food,
                position,
            });
            falling(a, h, dt, true);
            return;
        }
        if !matches!(a.behavior, Behavior::Climb | Behavior::Ceiling) {
            a.behavior = Behavior::Crawl;
        }
        move_surface_with_depth(a, h, destination, dt, a.kind.speed() * 1.35, owned);
    } else {
        a.velocity = [0.0; 3];
        if !matches!(a.behavior, Behavior::Climb | Behavior::Ceiling) {
            a.behavior = Behavior::Rest;
        }
    }
}

pub(super) fn hunt_surface(
    a: &mut Creature,
    h: &Habitat,
    destination: [f64; 3],
    dt: f32,
    search: bool,
    navigation: &mut super::navigation::Navigation,
    owned: bool,
) {
    if a.behavior == Behavior::Descend
        || contact(h, a).is_none()
        || a.behavior == Behavior::Ceiling && destination[1] < a.position[1] - 1.0
    {
        companion_move(a, h, destination, dt, true, owned);
        return;
    }
    surfaces::advance(
        a,
        h,
        destination,
        (dt, pursuit_speed(a.kind), owned),
        search,
        Some(navigation),
    );
}

fn move_surface_step(
    a: &mut Creature,
    h: &Habitat,
    destination: [f64; 3],
    dt: f32,
    speed: f64,
    owned: bool,
) -> bool {
    let normal = terra_core::creatures::spiders::surface_normal(a);
    let center = terra_core::creatures::spiders::visual_center(a);
    let mut direction = std::array::from_fn::<_, 3, _>(|i| destination[i] - center[i]);
    for i in 0..3 {
        if normal[i] != 0 {
            direction[i] = 0.0;
        }
    }
    if !owned
        && a.kind == Kind::LargeSpider
        && !terra_core::creatures::spawning::is_command_id(a.id)
        && normal[1] == 0
        && a.position[1] + a.size()[1] > -23.0
        && direction[1] > 0.0
    {
        direction[1] = -3.0;
    }
    let length = direction.iter().map(|v| v * v).sum::<f64>().sqrt();
    if length < 0.05 {
        return false;
    }
    let travel = (speed * f64::from(dt)).min(length);
    let unit = direction.map(|v| v / length);
    let n = normal.map(f64::from);
    let across = [
        n[1] * unit[2] - n[2] * unit[1],
        n[2] * unit[0] - n[0] * unit[2],
        n[0] * unit[1] - n[1] * unit[0],
    ];
    for attempt in 0..7 {
        let mut next = a.clone();
        let step = if attempt < 5 {
            let (sin, cos) = [0.0_f64, 0.7, -0.7, 1.4, -1.4][attempt].sin_cos();
            std::array::from_fn::<_, 3, _>(|i| unit[i] * cos + across[i] * sin)
        } else {
            let axis = tangent_axes(normal)[attempt - 5];
            let mut cardinal = [0.0; 3];
            cardinal[axis] = if unit[axis] < 0.0 { -1.0 } else { 1.0 };
            cardinal
        };
        for (i, value) in step.iter().enumerate() {
            next.position[i] += value * travel;
        }
        if !owned && !within_roaming_depth(&next) {
            continue;
        }
        if normal[1] != 0 {
            next.yaw = (-step[0]).atan2(-step[2]).rem_euclid(std::f64::consts::TAU) as f32;
        }
        if clear(h, &next) {
            if contact(h, &next).is_some() {
                next.velocity = step.map(|v| (v * travel / f64::from(dt)) as f32);
                *a = next;
                return true;
            }
            if let Some(anchor) = contact(h, a) {
                for axis in tangent_axes(normal) {
                    if step[axis].abs() < 0.2 {
                        continue;
                    }
                    let mut face = [0; 3];
                    face[axis] = if step[axis] > 0.0 { 1 } else { -1 };
                    if let Some(mut wrapped) = turn_surface(h, a, anchor, face, true, owned) {
                        wrapped.velocity = step.map(|v| (v * travel / f64::from(dt)) as f32);
                        *a = wrapped;
                        return true;
                    }
                }
            }
        }
        let size = a.size();
        let reach = travel + directional_extent(&next, step) + 0.06;
        let old_axis = normal.iter().position(|v| *v != 0).unwrap_or(1);
        for level in [0.0, -0.35, 0.35] {
            let mut probe = std::array::from_fn::<_, 3, _>(|i| center[i] + step[i] * reach);
            probe[old_axis] += size[old_axis] * level;
            if let Some(anchor) = shape_contains(h, probe) {
                let axis = tangent_axes(normal)
                    .into_iter()
                    .max_by(|a, b| step[*a].abs().total_cmp(&step[*b].abs()))
                    .unwrap_or(0);
                let mut face = [0; 3];
                face[axis] = if step[axis] > 0.0 { -1 } else { 1 };
                if let Some(mut turned) = turn_surface(h, a, anchor, face, false, owned) {
                    turned.velocity = step.map(|v| (v * travel / f64::from(dt)) as f32);
                    *a = turned;
                    return true;
                }
            }
        }
    }
    false
}

fn tangent_axes(normal: [i8; 3]) -> [usize; 2] {
    match normal.iter().position(|v| *v != 0).unwrap_or(1) {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    }
}

/// Floor and ceiling bodies are yaw-rotated boxes; wall bodies already encode
/// their attachment rotation in occupied_size. Probes and pivots must use the
/// very same support extent as the collision test, especially for large spiders.
fn directional_extent(a: &Creature, direction: [f64; 3]) -> f64 {
    let size = a.size();
    let local = if terra_core::creatures::spiders::surface_normal(a)[1] != 0 {
        let (sin, cos) = f64::from(a.yaw).sin_cos();
        [
            direction[0] * cos - direction[2] * sin,
            direction[1],
            direction[0] * sin + direction[2] * cos,
        ]
    } else {
        direction
    };
    (0..3).map(|i| local[i].abs() * size[i] * 0.5).sum()
}

fn support_plane(h: &Habitat, a: &Creature) -> Option<(f64, f64)> {
    let normal = terra_core::creatures::spiders::surface_normal(a);
    let axis = normal.iter().position(|v| *v != 0)?;
    let anchor = contact(h, a)?;
    let center = terra_core::creatures::spiders::visual_center(a);
    let mut best: Option<(f64, f64)> = None;
    let mut visit = |shape: terra_core::block_shapes::ShapeBox| {
        let plane = if normal[axis] > 0 {
            shape.maximum[axis]
        } else {
            shape.minimum[axis]
        };
        if best.is_none_or(|(old, _)| (center[axis] - plane).abs() < (center[axis] - old).abs()) {
            best = Some((plane, shape.maximum[axis] - shape.minimum[axis]));
        }
    };
    if !h.boxes(anchor, &mut visit) {
        visit(terra_core::block_shapes::ShapeBox {
            minimum: anchor.map(|v| v as f64),
            maximum: anchor.map(|v| v as f64 + 1.0),
            hinged: false,
        });
    }
    best
}

fn turn_surface(
    h: &Habitat,
    a: &Creature,
    anchor: [i64; 3],
    face: [i8; 3],
    convex: bool,
    owned: bool,
) -> Option<Creature> {
    let old = terra_core::creatures::spiders::surface_normal(a);
    let axis = old.iter().position(|v| *v != 0)?;
    if face[axis] != 0 {
        return None;
    }
    let (plane, _) = support_plane(h, a)?;
    let mut probe = a.clone();
    probe.behavior = if face[1] > 0 {
        Behavior::Crawl
    } else if face[1] < 0 {
        Behavior::Ceiling
    } else {
        Behavior::Climb
    };
    if face[1] == 0 {
        probe.yaw = f32::from(face[0])
            .atan2(f32::from(face[2]))
            .rem_euclid(std::f32::consts::TAU);
    }
    let size = probe.size();
    let mut center = terra_core::creatures::spiders::visual_center(a);
    if !convex {
        let mut outward = [0.0; 3];
        outward[axis] = 1.0;
        center[axis] = plane + f64::from(old[axis]) * (directional_extent(&probe, outward) + 0.015);
    }
    // Keep the center beside the edge instead of snapping it inside the old
    // support voxel. Broad legs can reach the terrace without intersecting the
    // next stair. Only two small, swept contact corrections are permitted.
    for offset in [0.0, 0.06, 0.12] {
        if !convex && offset != 0.0 {
            break;
        }
        let mut adjusted = center;
        adjusted[axis] -= f64::from(old[axis]) * offset * a.growth_scale();
        probe.position = [adjusted[0], adjusted[1] - size[1] * 0.5, adjusted[2]];
        let Some(next) = attach_with_depth(h, &probe, anchor, face, owned) else {
            continue;
        };
        let mut sweep = next.clone();
        if (0..=2).all(|step| {
            sweep.position[axis] = next.position[axis]
                + f64::from(old[axis]) * offset * a.growth_scale() * f64::from(step) * 0.5;
            clear(h, &sweep)
        }) {
            return Some(next);
        }
    }
    None
}

/// A bounded real-voxel update; only a granted search scans nearby hiding corners.
pub fn step(
    a: &mut Creature,
    h: &Habitat,
    players: &[[f64; 3]],
    dt: f32,
    search: bool,
    night: bool,
) -> Outcome {
    step_with_navigation(a, h, players, dt, search, night, None)
}

pub fn step_with_navigation(
    a: &mut Creature,
    h: &Habitat,
    players: &[[f64; 3]],
    dt: f32,
    search: bool,
    night: bool,
    navigation: Option<&mut super::navigation::Navigation>,
) -> Outcome {
    step_with_targets(a, h, Some(players), dt, search, night, navigation)
}

/// Pets keep autonomous cave activity, but can never receive a player target.
pub fn step_owned(
    a: &mut Creature,
    h: &Habitat,
    dt: f32,
    search: bool,
    night: bool,
    navigation: Option<&mut super::navigation::Navigation>,
) -> Outcome {
    step_with_targets(a, h, None, dt, search, night, navigation)
}

fn step_with_targets(
    a: &mut Creature,
    h: &Habitat,
    players: Option<&[[f64; 3]]>,
    dt: f32,
    search: bool,
    night: bool,
    mut navigation: Option<&mut super::navigation::Navigation>,
) -> Outcome {
    let owned = players.is_none();
    let players = players.unwrap_or_default();
    let mut outcome = Outcome::default();
    if !a.kind.is_spider()
        || a.health <= 0.0
        || !dt.is_finite()
        || !(0.0..=0.25).contains(&dt)
        || dt == 0.0
        || h.block(a.position.map(|v| v.floor() as i64)).is_none()
    {
        return outcome;
    }
    a.age += f64::from(dt);
    a.needs.food = (a.needs.food
        - dt * (0.012 + a.velocity[0].hypot(a.velocity[2]).min(3.0) * 0.003))
        .max(0.0);
    a.needs.water = 0.0;
    if a.needs.food == 0.0 {
        a.health = (a.health - dt * 0.025).max(0.0);
        if a.health == 0.0 {
            a.behavior = Behavior::Dead;
            return outcome;
        }
    }
    a.timer = (a.timer - dt).max(0.0);
    a.needs.search_in = (a.needs.search_in - dt).max(0.0);
    let retreating = !owned && a.kind == Kind::MediumSpider && !night;
    if a.needs.goal.is_some_and(|goal| {
        retreating && goal.kind != NeedKind::Shelter && a.behavior != Behavior::Descend
            || !retreating && goal.kind == NeedKind::Shelter
    }) {
        a.needs.goal = None;
        a.needs.search_in = 0.0;
        if let Some(nav) = navigation.as_deref_mut() {
            nav.forget(a.id);
        }
    }
    if submersion_damage(a, h, dt) {
        return outcome;
    }
    if a.behavior == Behavior::Descend {
        if let Some(nav) = navigation.as_deref_mut() {
            nav.intend(
                a,
                IntentReason::Silk,
                a.needs
                    .goal
                    .map(|goal| goal.position.map(|v| v as f64 + 0.5)),
            );
        }
        falling(a, h, dt, true);
        return outcome;
    }
    if contact(h, a).is_none() {
        detach_for_gravity(a, h);
        if let Some(nav) = navigation.as_deref_mut() {
            nav.forget(a.id);
            nav.intend(a, IntentReason::Unsupported, None);
        }
        falling(a, h, dt, false);
        return outcome;
    }
    let center = terra_core::creatures::spiders::visual_center(a);
    a.velocity = [0.0; 3];
    if let Some(player) = players
        .iter()
        .filter(|p| super::navigation::valid_point(**p))
        .min_by(|a, b| distance(**a, center).total_cmp(&distance(**b, center)))
        && ((a.behavior == Behavior::Flee && a.health < a.kind.health() * 0.4 && a.timer > 0.0)
            || a.kind == Kind::SmallSpider
                && a.health == a.kind.health()
                && distance(*player, center) < 1.1_f64.powi(2))
    {
        let destination = [
            center[0] + (center[0] - player[0]) * 4.0,
            center[1],
            center[2] + (center[2] - player[2]) * 4.0,
        ];
        a.behavior = if matches!(a.behavior, Behavior::Climb | Behavior::Ceiling) {
            a.behavior
        } else {
            Behavior::Flee
        };
        if a.health < a.kind.health() * 0.4 {
            a.timer = a.timer.max(1.0);
        }
        a.needs.search_in = a.needs.search_in.max(1.0);
        if let Some(nav) = navigation.as_deref_mut() {
            nav.ground_unblocked(a.id);
            nav.intend(a, IntentReason::Threat, Some(destination));
        }
        surfaces::advance(
            a,
            h,
            destination,
            (dt, pursuit_speed(a.kind), owned),
            search,
            navigation,
        );
        return outcome;
    }
    let combat = terra_core::creatures::definitions::get(a.kind).combat;
    let active = a.kind != Kind::SmallSpider || a.health < a.kind.health();
    let target = if active {
        combat.and_then(|combat| {
            players
                .iter()
                .enumerate()
                .filter(|(_, player)| {
                    super::navigation::valid_point(**player)
                        && distance(**player, center) < combat.aggro_radius.powi(2)
                })
                .min_by(|(_, left), (_, right)| {
                    distance(**left, center).total_cmp(&distance(**right, center))
                })
                .filter(|(_, player)| line_of_sight(h, center, **player))
                .map(|(index, player)| (index, player, combat))
        })
    } else {
        None
    };
    if let Some((index, player, combat)) = target {
        // Combat performs no resource scan, including while attached to a wall.
        a.needs.search_in = a.needs.search_in.max(1.0);
        if let Some(nav) = navigation.as_deref_mut() {
            nav.ground_unblocked(a.id);
        }
        let reach = (combat.reach + a.kind.size()[2] * 0.3) * a.growth_scale();
        let stalking = night
            && distance(*player, center) > 7.0_f64.powi(2)
            && (a.age + (a.id % 11) as f64).rem_euclid(12.0) < 1.2;
        if let Some(nav) = navigation.as_deref_mut() {
            nav.intend(
                a,
                if distance(*player, center) <= reach.powi(2) {
                    IntentReason::Attack
                } else if stalking {
                    IntentReason::Stalk
                } else {
                    IntentReason::Hunt
                },
                Some(*player),
            );
        }
        if distance(*player, center) <= reach.powi(2) {
            if a.timer <= 0.0 {
                let (poison_level, poison_seconds) =
                    terra_core::creatures::spiders::venom(a.kind).unwrap_or((1, 1.0));
                outcome.bite = Some(Bite {
                    player: index,
                    damage: combat.damage * a.growth_scale().powi(2) as f32,
                    poison_level,
                    poison_seconds,
                });
                a.timer = combat.cooldown;
            }
            if !matches!(a.behavior, Behavior::Climb | Behavior::Ceiling) {
                a.behavior = Behavior::Attack;
            }
            a.velocity = [0.0; 3];
            return outcome;
        }
        if a.behavior == Behavior::Ceiling
            && player[1] < center[1] - 1.0
            && distance([player[0], center[1], player[2]], center) < 2.5_f64.powi(2)
        {
            let anchor = contact(h, a);
            let floor_height =
                terra_core::creatures::spiders::floor_size(a.kind)[1] * a.growth_scale();
            a.position[1] = center[1] - floor_height * 0.5;
            a.behavior = Behavior::Descend;
            a.needs.goal = anchor.map(|position| NeedGoal {
                kind: NeedKind::Food,
                position,
            });
            falling(a, h, dt, true);
            return outcome;
        }
        let mut destination = *player;
        if a.behavior == Behavior::Climb {
            // Ascend first when a wall blocks a horizontal chase.
            if (player[1] - center[1]).abs() < 0.6 {
                destination[1] = center[1] + 3.0;
            }
        } else if a.behavior != Behavior::Ceiling {
            a.behavior = if stalking {
                Behavior::Crawl
            } else {
                Behavior::Hunt
            };
        }
        surfaces::advance(
            a,
            h,
            destination,
            (
                dt,
                pursuit_speed(a.kind) * if stalking { 0.55 } else { 1.0 },
                owned,
            ),
            search,
            navigation,
        );
    } else {
        if !retreating
            && a.behavior == Behavior::Ceiling
            && a.timer <= 0.0
            && random(a.id, (a.age / 14.0) as u64).is_multiple_of(5)
            && let Some(anchor) = contact(h, a)
        {
            a.behavior = Behavior::Descend;
            a.needs.goal = Some(NeedGoal {
                kind: NeedKind::Food,
                position: anchor,
            });
            a.timer = 3.0;
            if let Some(nav) = navigation.as_deref_mut() {
                nav.intend(a, IntentReason::Silk, Some(anchor.map(|v| v as f64 + 0.5)));
            }
            falling(a, h, dt, true);
            return outcome;
        }
        if retreating {
            retreat(a, h, dt, search, navigation);
        } else {
            if search && a.needs.search_in <= 0.0 {
                a.needs.goal = corner_goal(h, a).map(|position| NeedGoal {
                    kind: NeedKind::Food,
                    position,
                });
                a.needs.search_in = 8.0 + (a.id % 6) as f32;
            }
            let seed = random(a.id, (a.age / 4.0) as u64);
            let hidden = a
                .needs
                .goal
                .is_some_and(|goal| distance(goal.position.map(|v| v as f64 + 0.5), center) < 1.0);
            if hidden && a.timer <= 0.0 {
                a.needs.goal = None;
                a.needs.search_in = 0.0;
            }
            let resting = hidden && a.timer > 0.0 || seed.is_multiple_of(5);
            if resting && !matches!(a.behavior, Behavior::Climb | Behavior::Ceiling) {
                if a.behavior != Behavior::Rest {
                    a.timer = 2.0 + (a.id % 30) as f32 * 0.1;
                }
                a.behavior = Behavior::Rest;
                if let Some(nav) = navigation.as_deref_mut() {
                    nav.intend(
                        a,
                        IntentReason::Corner,
                        a.needs.goal.map(|g| g.position.map(|v| v as f64 + 0.5)),
                    );
                }
            } else {
                let normal = terra_core::creatures::spiders::surface_normal(a);
                let destination = a.needs.goal.map_or_else(
                    || {
                        if normal[1] == 0 {
                            [
                                center[0],
                                center[1]
                                    + if (seed >> 4).is_multiple_of(3) {
                                        -3.0
                                    } else {
                                        4.0
                                    },
                                center[2],
                            ]
                        } else {
                            let yaw = (seed % 628) as f64 * 0.01;
                            [
                                center[0] - yaw.sin() * 4.0,
                                center[1],
                                center[2] - yaw.cos() * 4.0,
                            ]
                        }
                    },
                    |goal| goal.position.map(|v| v as f64 + 0.5),
                );
                if normal[1] > 0 {
                    a.behavior = Behavior::Crawl;
                }
                if let Some(nav) = navigation.as_deref_mut() {
                    nav.intend(
                        a,
                        if a.needs.goal.is_some() {
                            IntentReason::Corner
                        } else {
                            IntentReason::Surface
                        },
                        Some(destination),
                    );
                }
                surfaces::advance(
                    a,
                    h,
                    destination,
                    (dt, a.kind.speed() * 0.65, owned),
                    search,
                    navigation,
                );
            }
        }
    }
    outcome
}

/// Loaded surface and cave habitats share fixed per-cell IDs; reload never rerolls deaths.
pub fn spawn(
    population: &mut CreaturePopulation,
    h: &Habitat,
    home: [i64; 2],
    night: bool,
    player_y: f64,
    available: usize,
) -> usize {
    let mut spawned = 0;
    let seed = h.terrain.config().seed.0;
    for (slot, kind) in [Kind::SmallSpider, Kind::MediumSpider, Kind::LargeSpider]
        .into_iter()
        .enumerate()
    {
        if spawned >= available || population.animals.len() >= terra_core::creatures::MAX_CREATURES
        {
            break;
        }
        let Some(id) = spider_id(home, slot as u8) else {
            continue;
        };
        if population.animals.contains_key(&id) {
            continue;
        }
        let hash = random(id, seed);
        let definition = terra_core::creatures::definitions::get(kind);
        let surface_gate = hash.is_multiple_of(if kind == Kind::SmallSpider { 3 } else { 5 });
        let cave_gate = kind != Kind::SmallSpider && definition.permits_cave_spawn(hash, 5);
        if definition
            .spawn
            .as_ref()
            .is_some_and(|spawn| !spawn.enabled)
            || !surface_gate && !cave_gate
        {
            continue;
        }
        let mut sections: Vec<_> = h
            .chunks
            .loaded
            .keys()
            .filter(|p| p.x.div_euclid(2) == home[0] && p.z.div_euclid(2) == home[1])
            .copied()
            .collect();
        sections.sort_unstable_by(|a, b| {
            ((a.y * 16) as f64 - player_y)
                .abs()
                .total_cmp(&((b.y * 16) as f64 - player_y).abs())
                .then(a.y.cmp(&b.y))
                .then(a.z.cmp(&b.z))
                .then(a.x.cmp(&b.x))
        });
        let mut animal = Creature {
            id,
            kind,
            home,
            position: [0.0; 3],
            velocity: [0.0; 3],
            yaw: (hash % 628) as f32 * 0.01,
            health: kind.health(),
            behavior: Behavior::Crawl,
            timer: 1.5,
            age: 0.0,
            variant: ((hash >> 10) % 4) as u8,
            needs: CreatureNeeds::full(kind),
        };
        'search: for section in sections.into_iter().take(12) {
            for probe in 0..8_u64 {
                let value = random(hash, probe ^ (section.y as u64).wrapping_mul(31));
                let x = section.x * 16 + (value % 16) as i64;
                let z = section.z * 16 + ((value >> 8) % 16) as i64;
                let column = h.terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
                if !definition
                    .permits_spawn_biome(super::biome_slug(column.biome.weights.dominant()))
                {
                    continue;
                }
                let surface = column.height;
                for offset in 0..16_i64 {
                    let y = section.y * 16 + offset;
                    let position = [x as f64 + 0.5, y as f64 + 1.015, z as f64 + 0.5];
                    let underground = position[1] + 5.0 < surface;
                    if !(if underground { cave_gate } else { surface_gate }) {
                        continue;
                    }
                    if kind == Kind::LargeSpider
                        && (position[1] + animal.size()[1] > -22.0 || !underground)
                        || kind == Kind::MediumSpider
                            && (position[1] < -22.0 || !night && !underground)
                        || kind == Kind::SmallSpider && underground
                    {
                        continue;
                    }
                    if !h.solid([x, y, z]) || h.block([x, y + 1, z]) != Some(0) {
                        continue;
                    }
                    animal.position = position;
                    if !clear(h, &animal)
                        || contact(h, &animal).is_none()
                        || h.wet([
                            position[0],
                            position[1] + animal.size()[1] * 0.5,
                            position[2],
                        ])
                    {
                        continue;
                    }
                    if population
                        .animals
                        .values()
                        .filter(|a| a.health > 0.0)
                        .any(|a| {
                            distance(a.position, position)
                                < (a.size()[0].max(a.size()[2]) * 0.5
                                    + animal.size()[0].max(animal.size()[2]) * 0.5
                                    + 0.5)
                                    .powi(2)
                        })
                    {
                        continue;
                    }
                    // Small woodland spiders may start on an actual tree face.
                    if kind == Kind::SmallSpider && (hash >> 16).is_multiple_of(2) {
                        for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
                            let trunk = [x + d[0], y + 1, z + d[2]];
                            if matches!(h.block(trunk), Some(6 | 36)) {
                                let face = d.map(|v| -v as i8);
                                if let Some(next) = attach(h, &animal, trunk, face) {
                                    animal = next;
                                    break;
                                }
                            }
                        }
                    }
                    population.animals.insert(id, animal);
                    spawned += 1;
                    break 'search;
                }
            }
        }
    }
    spawned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        TerraGenerator,
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection},
    };
    use terra_voxel::{BlockSection, SectionPos};

    fn fixture(seed: u64) -> (TerraGenerator, ChunkStore, EditJournal) {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            seed: terra_core::WorldSeed(seed),
            base_height: 3.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            caves: false,
            trees: false,
            biomes: false,
            rivers: false,
            water: false,
            ..Default::default()
        })
        .unwrap();
        let mut chunks = ChunkStore::default();
        for y in -2..=0 {
            for x in -2..=1 {
                for z in -2..=1 {
                    let p = SectionPos::new(x, y, z);
                    let mut data = BlockSection::empty(p).unwrap();
                    for dx in 0..16 {
                        for dz in 0..16 {
                            for floor in [-30_i64, -12, 2] {
                                if floor.div_euclid(16) == y {
                                    data.set_block([dx, floor.rem_euclid(16) as usize, dz], 3)
                                        .unwrap();
                                }
                            }
                        }
                    }
                    chunks.loaded.insert(
                        p,
                        LoadedSection {
                            data,
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: 0,
                            water: None,
                            props: vec![],
                        },
                    );
                }
            }
        }
        (terrain, chunks, EditJournal::default())
    }

    fn spider(kind: Kind) -> Creature {
        Creature {
            id: spider_id([0, 0], 0).unwrap(),
            kind,
            home: [0, 0],
            position: [7.5, 3.015, 7.5],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: kind.health(),
            behavior: Behavior::Crawl,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        }
    }

    #[test]
    fn removed_wall_support_rebases_about_the_real_center_before_gravity_without_clipping() {
        for kind in [Kind::SmallSpider, Kind::MediumSpider, Kind::LargeSpider] {
            let (terrain, chunks, mut journal) = fixture(121);
            journal.record([7, 5, 5], 3).unwrap();
            let habitat = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            let mut a = spider(kind);
            a.position = [7.5, 5.0, 7.5];
            a = attach_with_depth(&habitat, &a, [7, 5, 5], [0, 0, 1], true).unwrap();
            assert_eq!(a.behavior, Behavior::Climb);
            let center = terra_core::creatures::spiders::visual_center(&a);
            // A neighboring wall must never be crossed by the reorientation.
            let original = a.clone();
            detach_for_gravity(&mut a, &habitat);
            assert!(clear(&habitat, &a));
            if kind == Kind::LargeSpider {
                assert_eq!(a.behavior, Behavior::Climb);
            }
            a = original;
            journal.record([7, 5, 5], 0).unwrap();
            let habitat = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            assert!(contact(&habitat, &a).is_none());
            companion_move(&mut a, &habitat, [0.0; 3], 0.1, false, true);
            let after = terra_core::creatures::spiders::visual_center(&a);
            assert_eq!(a.behavior, Behavior::Crawl);
            assert!(
                (after[0] - center[0]).abs() < 0.00001 && (after[2] - center[2]).abs() < 0.00001
            );
            assert!(
                (after[1] - center[1] + 0.12).abs() < 0.00001,
                "{kind:?}: {center:?} -> {after:?}"
            );
            assert!(clear(&habitat, &a) && a.valid());
        }
    }

    #[test]
    fn invalid_surface_targets_fail_closed_and_do_not_poison_the_next_valid_move() {
        let (terrain, chunks, journal) = fixture(122);
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut a = spider(Kind::SmallSpider);
        let position = a.position;
        let mut nav = super::super::navigation::Navigation::default();
        for bad in [[f64::NAN; 3], [f64::INFINITY; 3], [f64::MAX; 3]] {
            assert!(!line_of_sight(&habitat, a.position, bad));
            assert!(!surfaces::advance(
                &mut a,
                &habitat,
                bad,
                (0.1, 1.0, true),
                true,
                Some(&mut nav)
            ));
            assert_eq!(a.position, position);
            assert!(a.valid());
        }
        let center = terra_core::creatures::spiders::visual_center(&a);
        assert!(move_surface_with_depth(
            &mut a,
            &habitat,
            [center[0] + 1.0, center[1], center[2]],
            0.1,
            1.0,
            true
        ));
        assert!(a.position[0] > position[0] && a.valid());
    }

    #[test]
    fn spiders_seek_real_upper_wall_anchors_and_build_spanning_dens_instead_of_floor_patches() {
        let (terrain, chunks, mut journal) = fixture(118);
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            base_height: 30.0,
            ..terrain.config().clone()
        })
        .unwrap();
        for z in 2..=13 {
            for y in 3..=13 {
                journal.record([2, y, z], 3).unwrap();
                journal.record([14, y, z], 3).unwrap();
            }
            for x in 2..=14 {
                journal.record([x, 13, z], 3).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        for kind in [Kind::SmallSpider, Kind::MediumSpider, Kind::LargeSpider] {
            let mut a = spider(kind);
            let start = a.position;
            assert!(construction_goal(&a, &habitat, true).is_some(), "{kind:?}");
            let mut nav = super::super::navigation::Navigation::default();
            let mut built = None;
            for tick in 0..500 {
                if let Some(request) =
                    prepare_owned_den(&mut a, &habitat, 0.1, tick % 10 == 0, &mut nav)
                {
                    built = Some(request);
                    break;
                }
                assert!(a.valid() && clear(&habitat, &a));
            }
            let built = built.unwrap_or_else(|| panic!("{kind:?}: {a:?}"));
            assert!(built.separation >= 8, "{kind:?}: {built:?}");
            assert!(distance(start, a.position) > 4.0);
            assert!(den_fits(built, &habitat));
            assert!(crate::props::webs::is_construction(&built.prop().unwrap()));
        }
    }

    #[test]
    fn surface_route_edges_preserve_wall_contact_instead_of_reversing_to_the_floor_mid_edge() {
        let (terrain, chunks, mut journal) = fixture(118);
        for z in 2..=13 {
            for y in 3..=13 {
                journal.record([14, y, z], 3).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        for kind in [Kind::SmallSpider, Kind::MediumSpider, Kind::LargeSpider] {
            let mut a = spider(kind);
            a.yaw = std::f32::consts::PI * 1.5;
            a.position[0] =
                14.0 - terra_core::creatures::spiders::floor_size(kind)[2] * 0.5 - 0.055;
            assert!(clear(&habitat, &a));
            let before = terra_core::creatures::spiders::visual_center(&a);
            assert!(move_surface_with_depth(
                &mut a,
                &habitat,
                [before[0] + 4.0, before[1], before[2]],
                0.25,
                2.4,
                true
            ));
            assert_eq!(
                terra_core::creatures::spiders::surface_normal(&a),
                [-1, 0, 0],
                "{kind:?}: {a:?}"
            );
            assert!(a.valid() && clear(&habitat, &a));
            assert_eq!(contact(&habitat, &a).unwrap()[0], 14);
            let wall = terra_core::creatures::spiders::visual_center(&a);
            assert!(move_surface_with_depth(
                &mut a,
                &habitat,
                [wall[0], wall[1] + 4.0, wall[2]],
                0.25,
                2.4,
                true
            ));
            assert!(terra_core::creatures::spiders::visual_center(&a)[1] > wall[1] + 0.4);
            assert!(a.valid() && clear(&habitat, &a));
            assert_eq!(
                terra_core::creatures::spiders::surface_normal(&a),
                [-1, 0, 0]
            );
        }
    }

    #[test]
    fn missing_den_relocates_from_invalid_contact_and_only_spins_at_a_real_fit() {
        let (terrain, mut chunks, journal) = fixture(113);
        let p = terra_voxel::global_to_section([6, 2, 6]);
        chunks
            .loaded
            .get_mut(&p.0)
            .unwrap()
            .data
            .set_block(p.1, 0)
            .unwrap();
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut a = spider(Kind::MediumSpider);
        assert!(contact(&habitat, &a).is_some());
        assert!(!den_fits(den_request(&a, &habitat).unwrap(), &habitat));
        let start = a.position;
        let mut navigation = super::super::navigation::Navigation::default();
        let mut built = None;
        for tick in 0..80 {
            navigation.begin_tick(a.id);
            let search = tick == 0 || a.needs.search_in <= 0.1;
            if let Some(request) = prepare_den(&mut a, &habitat, 0.1, search, &mut navigation) {
                assert!(search);
                assert!(den_fits(request, &habitat));
                built = Some(request);
                break;
            }
            assert!(contact(&habitat, &a).is_some());
            assert!(!matches!(a.behavior, Behavior::Hunt | Behavior::Attack));
            assert!(a.valid());
        }
        assert!(
            built.is_some(),
            "spider must reach a fitting nearby anchor: {:?}",
            a.position
        );
        assert!(
            distance(start, a.position) > 0.25,
            "no stationary retry loop"
        );
        let intent = navigation.inspection(&a);
        assert_eq!(intent.reason, IntentReason::Surface);
    }

    #[test]
    fn missing_den_without_search_budget_explores_but_cannot_spin_or_query_a_route() {
        let (terrain, chunks, journal) = fixture(114);
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut a = spider(Kind::MediumSpider);
        let start = a.position;
        let mut navigation = super::super::navigation::Navigation::default();
        for _ in 0..20 {
            navigation.begin_tick(a.id);
            assert!(prepare_den(&mut a, &habitat, 0.1, false, &mut navigation).is_none());
            assert!(a.valid());
            assert!(contact(&habitat, &a).is_some());
        }
        assert!(distance(start, a.position) > 0.25);
        assert!(navigation.inspection(&a).path.is_empty());
    }

    #[test]
    fn den_search_reaches_a_raised_cave_ledge_instead_of_requiring_a_flat_floor() {
        let (terrain, chunks, mut journal) = fixture(115);
        for x in 0..16 {
            for z in 0..16 {
                if !(x <= 7 && z == 7) {
                    journal.record([x, 2, z], 0).unwrap();
                }
                if (8..=14).contains(&x) && (5..=9).contains(&z) {
                    journal.record([x, 3, z], 3).unwrap();
                }
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut animal = spider(Kind::MediumSpider);
        animal.position[0] = 7.0;
        assert!(clear(&habitat, &animal));
        assert!(!den_fits(den_request(&animal, &habitat).unwrap(), &habitat));
        let target =
            den_goal(&animal, &habitat, false).expect("adjacent raised ledge is a valid web site");
        assert_eq!(target[1], 4);
        let start = animal.position;
        let mut navigation = super::super::navigation::Navigation::default();
        let mut built = false;
        for tick in 0..160 {
            navigation.begin_tick(animal.id);
            let search = tick == 0 || animal.needs.search_in <= 0.1;
            if let Some(web) = prepare_den(&mut animal, &habitat, 0.1, search, &mut navigation) {
                assert!(den_fits(web, &habitat));
                assert_eq!(web.anchor[1], 3);
                built = true;
                break;
            }
            assert!(animal.valid());
            assert!(clear(&habitat, &animal));
            assert!(contact(&habitat, &animal).is_some());
        }
        assert!(
            built,
            "must physically climb to the reachable web site: {:?}",
            animal.position
        );
        assert!(distance(start, animal.position) > 1.0);
    }

    #[test]
    fn sheltered_daytime_medium_spiders_pause_then_explore_instead_of_remaining_afk() {
        let (terrain, chunks, journal) = fixture(116);
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut animal = spider(Kind::MediumSpider);
        animal.position[1] = -10.985;
        animal.age = 36.5 - (animal.id % 17) as f64;
        assert!(in_cave(&animal, &habitat));
        assert!(shelter(&habitat, &animal).is_some());
        let mut navigation = super::super::navigation::Navigation::default();
        let mut moving = 0;
        let mut resting = 0;
        for tick in 0..160 {
            let before = animal.position;
            navigation.begin_tick(animal.id);
            step_with_navigation(
                &mut animal,
                &habitat,
                &[],
                0.1,
                tick % 10 == 0,
                false,
                Some(&mut navigation),
            );
            moving += usize::from(distance(before, animal.position) > 0.0001);
            resting += usize::from(animal.behavior == Behavior::Rest);
            assert!(animal.valid());
            assert!(contact(&habitat, &animal).is_some());
            assert!(shelter(&habitat, &animal).is_some());
            assert!(!matches!(
                animal.behavior,
                Behavior::Attack | Behavior::Hunt
            ));
        }
        assert!(
            moving > 40,
            "a sheltered wild spider must not wait for an owner: {moving}"
        );
        assert!(resting > 0, "short natural hiding pauses are retained");
    }

    #[test]
    fn owned_large_spiders_build_and_explore_above_wild_depth_without_player_aggro() {
        let (terrain, chunks, journal) = fixture(117);
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut animal = spider(Kind::LargeSpider);
        let mut navigation = super::super::navigation::Navigation::default();
        let web = prepare_owned_den(&mut animal, &habitat, 0.1, true, &mut navigation).unwrap();
        assert!(den_fits(web, &habitat));
        let start = animal.position;
        let mut displacement = 0.0;
        for tick in 0..100 {
            let before = animal.position;
            navigation.begin_tick(animal.id);
            let result = step_owned(
                &mut animal,
                &habitat,
                0.1,
                tick % 10 == 0,
                false,
                Some(&mut navigation),
            );
            assert!(result.bite.is_none());
            assert!(animal.valid());
            assert!(clear(&habitat, &animal));
            assert!(contact(&habitat, &animal).is_some());
            assert!(!matches!(
                animal.behavior,
                Behavior::Attack | Behavior::Hunt
            ));
            displacement += distance(before, animal.position).sqrt();
        }
        assert!(
            displacement > 1.0,
            "owned spider must not be frozen by the wild Y guard: {start:?} -> {:?}",
            animal.position
        );
        assert_eq!(animal.needs.water, 0.0);
    }

    #[test]
    fn juvenile_spider_surface_speed_is_scaled_once_by_its_actual_growth() {
        let (terrain, chunks, journal) = fixture(118);
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut adult = spider(Kind::MediumSpider);
        let mut baby = adult.clone();
        baby.id = terra_core::creatures::breeding::next_birth_id(&CreaturePopulation::default(), 0)
            .unwrap();
        let before = adult.position;
        let target = [12.5, 3.4, 7.5];
        move_surface_with_depth(&mut adult, &habitat, target, 0.1, 1.0, true);
        move_surface_with_depth(&mut baby, &habitat, target, 0.1, 1.0, true);
        let normal_distance = distance(before, adult.position).sqrt();
        let baby_distance = distance(before, baby.position).sqrt();
        assert!((normal_distance - 0.1).abs() < 0.001);
        assert!((baby_distance / normal_distance - baby.growth_scale()).abs() < 0.001);
        assert!(baby.valid());
        assert!(contact(&habitat, &baby).is_some());
    }

    #[test]
    fn juvenile_spider_cannot_consume_food_at_adult_attack_reach() {
        use super::super::spider_hunting;
        use terra_core::creatures::AnimalProduct;
        use terra_core::{GameplayState, ItemKind, ItemStack};
        let (terrain, chunks, journal) = fixture(119);
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut adult = spider(Kind::MediumSpider);
        adult.needs.food = 35.0;
        let mut baby = adult.clone();
        baby.id = terra_core::creatures::breeding::next_birth_id(&CreaturePopulation::default(), 0)
            .unwrap();
        let mut gameplay = GameplayState::default();
        assert!(gameplay.drop_stack(
            [8.5, 3.35, 7.5],
            ItemStack {
                item: ItemKind::Animal(AnimalProduct::Beef),
                count: 1
            },
            [0.0; 3],
            0.0
        ));
        let index = spider_hunting::FoodIndex::new(std::iter::empty(), gameplay.drops.iter());
        let mut navigation = super::super::navigation::Navigation::default();
        assert!(matches!(
            spider_hunting::step_owned(
                &mut adult,
                &habitat,
                &index,
                [7, 2, 7],
                0.1,
                true,
                &mut navigation
            ),
            Some(Some(_))
        ));
        assert_eq!(
            spider_hunting::step_owned(
                &mut baby,
                &habitat,
                &index,
                [7, 2, 7],
                0.1,
                true,
                &mut navigation
            ),
            Some(None)
        );
        let mut population = CreaturePopulation::default();
        population.animals.insert(baby.id, baby.clone());
        let meal = spider_hunting::Meal::Drop(gameplay.drops[0].id);
        assert!(!spider_hunting::eat(
            &mut population,
            &mut gameplay,
            &habitat,
            baby.id,
            meal
        ));
        assert_eq!(gameplay.drops[0].stack.count, 1);
    }

    #[test]
    fn hunting_speed_is_sixty_percent_faster_in_real_displacement_and_stays_swept() {
        let (terrain, chunks, journal) = fixture(31);
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut a = spider(Kind::MediumSpider);
        a.age = 7.0;
        let before = a.position;
        step(&mut a, &habitat, &[[12.0, 3.4, 7.5]], 0.1, false, true);
        let moved = distance(before, a.position).sqrt();
        assert!(
            (moved - Kind::MediumSpider.speed() * 1.35 * 1.6 * 0.1).abs() < 0.005,
            "{moved}"
        );
        assert_eq!(a.behavior, Behavior::Hunt);
        a.behavior = Behavior::Flee;
        a.health = a.kind.health() * 0.3;
        a.timer = 2.0;
        let before = a.position;
        step(&mut a, &habitat, &[[12.0, 3.4, 7.5]], 0.1, false, true);
        assert_eq!(a.behavior, Behavior::Flee);
        assert!(a.position[0] < before[0]);
        assert!((distance(before, a.position).sqrt() - pursuit_speed(a.kind) * 0.1).abs() < 0.005);
        assert_eq!(
            pursuit_speed(Kind::LargeSpider),
            Kind::LargeSpider.speed() * 1.35 * 1.6
        );
    }

    #[test]
    fn slower_floor_stalking_is_crawl_with_a_real_stalk_intention_not_hunt() {
        let (terrain, chunks, journal) = fixture(33);
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut a = spider(Kind::MediumSpider);
        a.age = 12.1 - (a.id % 11) as f64;
        let target = [16.0, 3.4, 7.5];
        let before = a.position;
        let mut nav = super::super::navigation::Navigation::default();
        step_with_navigation(
            &mut a,
            &habitat,
            &[target],
            0.1,
            false,
            true,
            Some(&mut nav),
        );
        assert_eq!(a.behavior, Behavior::Crawl);
        let intent = nav.inspection(&a);
        assert_eq!(intent.reason, IntentReason::Stalk);
        assert_eq!(intent.destination, Some(target));
        let actual = distance(before, a.position).sqrt();
        assert!((actual - pursuit_speed(a.kind) * 0.55 * 0.1).abs() < 0.005);
        assert!(actual < pursuit_speed(a.kind) * 0.1);
    }

    #[test]
    fn spider_decisions_report_real_target_and_unattached_fall_reason() {
        let (terrain, chunks, journal) = fixture(32);
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut a = spider(Kind::MediumSpider);
        let target = [12.0, 3.4, 7.5];
        let mut nav = super::super::navigation::Navigation::default();
        nav.begin_tick(a.id);
        step_with_navigation(
            &mut a,
            &habitat,
            &[target],
            0.1,
            false,
            true,
            Some(&mut nav),
        );
        let intent = nav.inspection(&a);
        assert_eq!(intent.destination, Some(target));
        assert!(matches!(
            intent.reason,
            IntentReason::Hunt | IntentReason::Stalk
        ));
        assert!(intent.valid());
        a.position[1] = 6.0;
        nav.begin_tick(a.id);
        step_with_navigation(&mut a, &habitat, &[], 0.1, false, true, Some(&mut nav));
        assert_eq!(nav.inspection(&a).reason, IntentReason::Unsupported);
        assert!(a.velocity[1] < 0.0);
    }

    #[test]
    fn neutral_spiders_only_retaliate_and_every_bite_obeys_reach_los_and_cooldown() {
        let (terrain, chunks, mut journal) = fixture(7);
        for kind in [Kind::SmallSpider, Kind::MediumSpider, Kind::LargeSpider] {
            let mut animal = spider(kind);
            let player = [7.5, 3.5, 7.05];
            let habitat = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            let first = step(&mut animal, &habitat, &[player], 0.1, false, true);
            if kind == Kind::SmallSpider {
                assert!(first.bite.is_none());
                animal.health -= 1.0;
            } else {
                assert!(first.bite.is_some());
                animal.timer = 0.0;
            }
            let bite = step(&mut animal, &habitat, &[player], 0.1, false, true)
                .bite
                .unwrap();
            assert_eq!(
                (bite.poison_level, bite.poison_seconds),
                terra_core::creatures::spiders::venom(kind).unwrap()
            );
            if kind == Kind::LargeSpider {
                assert_eq!(bite.damage, 5.5);
            }
            assert!(
                step(&mut animal, &habitat, &[player], 0.1, false, true)
                    .bite
                    .is_none()
            );
            animal.timer = 0.0;
            assert!(
                step(
                    &mut animal,
                    &habitat,
                    &[[20.0, 3.5, 20.0]],
                    0.1,
                    false,
                    true
                )
                .bite
                .is_none()
            );
            assert!(animal.valid());
        }
        for y in 3..=6 {
            journal.record([7, y, 6], 3).unwrap();
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut animal = spider(Kind::LargeSpider);
        assert!(
            step(&mut animal, &habitat, &[[7.5, 3.5, 5.7]], 0.1, false, true)
                .bite
                .is_none()
        );
    }

    #[test]
    fn real_wall_and_ceiling_attachment_stays_clear_and_detaches_if_support_is_removed() {
        let (terrain, chunks, mut journal) = fixture(4);
        for y in 3..=8 {
            for x in 4..=10 {
                journal.record([x, y, 5], 3).unwrap();
            }
        }
        for x in 4..=10 {
            for z in 5..=10 {
                journal.record([x, 8, z], 3).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut animal = spider(Kind::MediumSpider);
        animal.position = [7.5, 3.015, 6.6];
        animal = attach(&habitat, &animal, [7, 3, 5], [0, 0, 1]).unwrap();
        assert_eq!(animal.behavior, Behavior::Climb);
        let before = animal.position;
        for _ in 0..20 {
            let center = terra_core::creatures::spiders::visual_center(&animal);
            assert!(move_surface(
                &mut animal,
                &habitat,
                [center[0], center[1] + 2.0, center[2]],
                0.1,
                0.65
            ));
            assert!(clear(&habitat, &animal));
            assert!(contact(&habitat, &animal).is_some());
        }
        assert!(animal.position[1] > before[1] + 1.0);
        animal.position = [7.5, 6.0, 7.5];
        animal = attach(&habitat, &animal, [7, 8, 7], [0, -1, 0]).unwrap();
        assert_eq!(animal.behavior, Behavior::Ceiling);
        let before = animal.position;
        assert!(move_surface(
            &mut animal,
            &habitat,
            [8.5, before[1], 7.5],
            0.1,
            0.65
        ));
        assert!(animal.position[0] > before[0]);
        for x in 4..=10 {
            for z in 5..=10 {
                journal.record([x, 8, z], 0).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let before = animal.position[1];
        step(&mut animal, &habitat, &[], 0.1, false, true);
        assert_eq!(animal.behavior, Behavior::Crawl);
        assert!(animal.position[1] < before);
        assert!(animal.valid());
    }

    #[test]
    fn spider_physically_climbs_a_stepped_cave_wall_then_traverses_the_ceiling() {
        let (terrain, chunks, mut journal) = fixture(116);
        for x in 3..=14 {
            for y in 3..=12 {
                let z = if y < 6 {
                    5
                } else if y < 9 {
                    4
                } else {
                    3
                };
                journal.record([x, y, z], 3).unwrap();
            }
            for z in 3..=14 {
                journal.record([x, 12, z], 3).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        for kind in [Kind::SmallSpider, Kind::MediumSpider, Kind::LargeSpider] {
            let mut animal = spider(kind);
            animal.position = [7.5, 3.015, 7.5];
            let start = animal.position;
            let mut navigation = super::super::navigation::Navigation::default();
            let mut saw_wall = false;
            for tick in 0_usize..500 {
                animal.age += 0.1;
                let search =
                    tick.is_multiple_of(10) || navigation.needs_search(animal.id, animal.age);
                surfaces::advance(
                    &mut animal,
                    &habitat,
                    [7.5, 13.0, 3.5],
                    (0.1, 1.1, true),
                    search,
                    Some(&mut navigation),
                );
                saw_wall |= animal.behavior == Behavior::Climb;
                assert!(
                    clear(&habitat, &animal),
                    "body crossed cave stone: {:?}",
                    animal.position
                );
                assert!(
                    contact(&habitat, &animal).is_some(),
                    "lost real attachment: {:?}",
                    animal.position
                );
                if animal.behavior == Behavior::Ceiling {
                    break;
                }
            }
            assert!(saw_wall);
            assert_eq!(
                animal.behavior,
                Behavior::Ceiling,
                "spider must turn onto the ceiling, not stay crawling at {:?}",
                animal.position
            );
            assert!(animal.position[1] > start[1] + 7.0);
            let before = terra_core::creatures::spiders::visual_center(&animal);
            // Cross beyond the lower terraces before descending to the base floor.
            let target = [before[0] + 2.0, before[1], before[2] + 4.0];
            for _ in 0..40 {
                animal.age += 0.1;
                surfaces::advance(
                    &mut animal,
                    &habitat,
                    target,
                    (0.1, 1.1, true),
                    false,
                    Some(&mut navigation),
                );
                assert_eq!(animal.behavior, Behavior::Ceiling);
                assert!(clear(&habitat, &animal));
                assert!(contact(&habitat, &animal).is_some());
            }
            assert!(
                distance(
                    before,
                    terra_core::creatures::spiders::visual_center(&animal)
                ) > 1.0
            );
            let silk_anchor = contact(&habitat, &animal).unwrap();
            let center = terra_core::creatures::spiders::visual_center(&animal);
            companion_move(
                &mut animal,
                &habitat,
                [center[0], 3.5, center[2]],
                0.1,
                true,
                true,
            );
            assert_eq!(animal.behavior, Behavior::Descend);
            assert_eq!(animal.needs.goal.unwrap().position, silk_anchor);
            for _ in 0..220 {
                if animal.behavior != Behavior::Descend {
                    break;
                }
                falling(&mut animal, &habitat, 0.1, true);
                assert!(clear(&habitat, &animal));
                assert!(animal.valid());
            }
            assert_eq!(
                animal.behavior,
                Behavior::Crawl,
                "{kind:?} stuck at {:?}",
                animal.position
            );
            assert!(
                animal.position[1] < 3.1,
                "{kind:?} landed before the clear shaft: {:?}",
                animal.position
            );
            assert!(contact(&habitat, &animal).is_some());
            assert!(animal.needs.goal.is_none());
        }
    }

    #[test]
    fn every_spider_size_climbs_quarter_turned_corners_with_the_same_real_body_clearance() {
        let (terrain, chunks, mut journal) = fixture(119);
        for u in 3..=14 {
            for y in 3..=10 {
                journal.record([u, y, 5], 3).unwrap();
                journal.record([5, y, u], 3).unwrap();
            }
            for v in 3..=14 {
                journal.record([u, 11, v], 3).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        for kind in [Kind::SmallSpider, Kind::MediumSpider, Kind::LargeSpider] {
            let mut endpoints = Vec::new();
            for axis in [2, 0] {
                let mut animal = spider(kind);
                animal.position = [9.0, 3.015, 9.0];
                animal.yaw = if axis == 0 {
                    std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                };
                let mut target = [9.0, 12.0, 9.0];
                target[axis] = 4.5;
                let mut previous_normal = terra_core::creatures::spiders::surface_normal(&animal);
                let mut changes = 0;
                let mut climbed = false;
                for _ in 0..500 {
                    let previous = terra_core::creatures::spiders::visual_center(&animal);
                    assert!(
                        move_surface_with_depth(&mut animal, &habitat, target, 0.05, 1.1, true),
                        "{kind:?} axis {axis} stalled at {animal:?}"
                    );
                    let normal = terra_core::creatures::spiders::surface_normal(&animal);
                    changes += usize::from(normal != previous_normal);
                    climbed |= animal.behavior == Behavior::Climb;
                    assert!(clear(&habitat, &animal));
                    assert!(contact(&habitat, &animal).is_some());
                    assert!(animal.valid());
                    if normal == previous_normal {
                        assert!(
                            distance(
                                previous,
                                terra_core::creatures::spiders::visual_center(&animal)
                            )
                            .sqrt()
                                < 0.057
                        );
                    }
                    previous_normal = normal;
                    if animal.behavior == Behavior::Ceiling {
                        break;
                    }
                }
                assert!(climbed);
                assert_eq!(
                    animal.behavior,
                    Behavior::Ceiling,
                    "{kind:?} axis {axis}: {animal:?}"
                );
                assert_eq!(
                    changes, 2,
                    "a flat corner must not oscillate between attachments"
                );
                let center = terra_core::creatures::spiders::visual_center(&animal);
                let mut direction = [0.0; 3];
                direction[axis] = 1.0;
                assert!(
                    (center[axis] - (6.015 + directional_extent(&animal, direction))).abs() < 0.001
                );
                endpoints.push(center);
                // This is actual ceiling displacement, not just a pose change.
                let mut across = center;
                across[axis] += 2.0;
                for _ in 0..20 {
                    assert!(move_surface_with_depth(
                        &mut animal,
                        &habitat,
                        across,
                        0.05,
                        1.1,
                        true
                    ));
                    assert_eq!(animal.behavior, Behavior::Ceiling);
                    assert!(clear(&habitat, &animal) && contact(&habitat, &animal).is_some());
                }
                assert!(
                    distance(
                        center,
                        terra_core::creatures::spiders::visual_center(&animal)
                    ) > 1.0
                );
            }
            let mirrored = [endpoints[1][2], endpoints[1][1], endpoints[1][0]];
            assert!(
                distance(endpoints[0], mirrored) < 0.000001,
                "{kind:?}: {endpoints:?}"
            );
        }
    }

    #[test]
    fn spider_wall_steering_can_detour_laterally_instead_of_repeating_the_same_upward_probe() {
        let (terrain, chunks, mut journal) = fixture(117);
        for x in 3..=14 {
            for y in 3..=10 {
                journal.record([x, y, 5], 3).unwrap();
            }
        }
        for x in 7..=8 {
            for y in 5..=6 {
                journal.record([x, y, 6], 3).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut animal = spider(Kind::MediumSpider);
        animal.position = [7.5, 3.015, 7.0];
        animal = attach(&habitat, &animal, [7, 3, 5], [0, 0, 1]).unwrap();
        let start = animal.position;
        let mut navigation = super::super::navigation::Navigation::default();
        super::super::navigation::testing::reset_path_calls();
        for tick in 0_usize..150 {
            animal.age += 0.1;
            surfaces::advance(
                &mut animal,
                &habitat,
                [7.5, 9.0, 6.35],
                (0.1, 1.0, true),
                tick.is_multiple_of(10),
                Some(&mut navigation),
            );
            assert!(clear(&habitat, &animal));
            assert!(contact(&habitat, &animal).is_some());
            if terra_core::creatures::spiders::visual_center(&animal)[1] > 8.0 {
                break;
            }
        }
        assert!(
            animal.position[1] > start[1] + 3.0,
            "real movement must clear the wall obstruction: {:?}",
            animal.position
        );
        let searches = super::super::navigation::testing::path_calls();
        for _ in 0..8 {
            animal.age += 0.1;
            surfaces::advance(
                &mut animal,
                &habitat,
                [7.5, 9.0, 6.35],
                (0.1, 1.0, true),
                false,
                Some(&mut navigation),
            );
        }
        assert_eq!(
            super::super::navigation::testing::path_calls(),
            searches,
            "cache reuse cannot bypass the shared search grant"
        );
        navigation.forget(animal.id);
        assert!(!navigation.needs_search(animal.id, animal.age + 10.0));
    }

    #[test]
    fn silk_descent_requires_a_real_ceiling_and_lands_on_solid_floor() {
        let (terrain, chunks, mut journal) = fixture(8);
        for x in 4..=10 {
            for z in 4..=10 {
                journal.record([x, 8, z], 3).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut animal = spider(Kind::MediumSpider);
        animal.position = [7.5, 6.0, 7.5];
        animal = attach(&habitat, &animal, [7, 8, 7], [0, -1, 0]).unwrap();
        step(&mut animal, &habitat, &[[7.5, 3.6, 7.5]], 0.1, false, true);
        assert_eq!(animal.behavior, Behavior::Descend);
        assert!(animal.needs.goal.is_some());
        for _ in 0..120 {
            if animal.behavior != Behavior::Descend {
                break;
            }
            step(&mut animal, &habitat, &[], 0.1, false, true);
            assert!(clear(&habitat, &animal));
            assert!(animal.valid());
        }
        assert_eq!(animal.behavior, Behavior::Crawl);
        assert!((animal.position[1] - 3.0).abs() < 0.1);
        assert!(animal.needs.goal.is_none());
    }

    #[test]
    fn silk_lands_on_real_off_center_terraces_and_releases_removed_support_for_every_size() {
        let (terrain, chunks, mut journal) = fixture(119);
        journal.record([7, 5, 7], 3).unwrap();
        for kind in [Kind::SmallSpider, Kind::MediumSpider, Kind::LargeSpider] {
            let habitat = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            let mut animal = spider(kind);
            animal.position = [8.05, 6.2, 8.05];
            animal.behavior = Behavior::Descend;
            animal.needs.goal = Some(NeedGoal {
                kind: NeedKind::Food,
                position: [8, 12, 8],
            });
            assert!(clear(&habitat, &animal));
            for _ in 0..20 {
                if animal.behavior != Behavior::Descend {
                    break;
                }
                falling(&mut animal, &habitat, 0.1, true);
                assert!(clear(&habitat, &animal));
            }
            assert_eq!(
                animal.behavior,
                Behavior::Crawl,
                "{kind:?} never landed at {:?}",
                animal.position
            );
            assert!((animal.position[1] - 6.015).abs() < 0.08);
            assert_eq!(contact(&habitat, &animal), Some([7, 5, 7]));
            assert_eq!(
                habitat.block([8, 5, 8]),
                Some(0),
                "body center really is above air"
            );
            assert!(animal.needs.goal.is_none());
            let mut removed = journal.clone();
            removed.record([7, 5, 7], 0).unwrap();
            let habitat = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &removed,
            };
            assert!(contact(&habitat, &animal).is_none());
            let before = animal.position[1];
            falling(&mut animal, &habitat, 0.1, false);
            assert!(
                animal.position[1] < before,
                "{kind:?} cannot retain an air floor after support removal"
            );
        }
    }

    #[test]
    fn sixty_hz_large_spider_pivot_changes_keep_locomotion_and_saved_gameplay_valid() {
        let (terrain, chunks, mut journal) = fixture(121);
        for x in 3..=14 {
            for y in 3..=12 {
                journal.record([x, y, 5], 3).unwrap();
            }
        }
        for x in 3..=14 {
            for z in 5..=14 {
                journal.record([x, 12, z], 3).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut animal = spider(Kind::LargeSpider);
        animal = attach_with_depth(&habitat, &animal, [7, 3, 5], [0, 0, 1], true).unwrap();
        let dt = 1.0 / 60.0;
        let mut saw_large_base_change = false;
        for _ in 0..2_000 {
            let before = animal.position;
            let old_normal = terra_core::creatures::spiders::surface_normal(&animal);
            companion_move(&mut animal, &habitat, [7.5, 13.0, 5.5], dt, true, true);
            let base_speed = ((animal.position[1] - before[1]) / f64::from(dt)).abs();
            if old_normal != terra_core::creatures::spiders::surface_normal(&animal) {
                saw_large_base_change |= base_speed > 32.0;
            }
            assert!(
                animal.valid(),
                "kinematic pivot is not physical velocity: {animal:?}"
            );
            let speed = animal.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
            assert!(f64::from(speed) <= animal.kind.speed() * 1.35 + 0.001);
            let mut state = terra_core::GameplayState::default();
            state.creatures.animals.insert(animal.id, animal.clone());
            assert!(state.valid());
            if animal.behavior == Behavior::Ceiling {
                break;
            }
        }
        assert_eq!(animal.behavior, Behavior::Ceiling);
        assert!(
            saw_large_base_change,
            "fixture must exercise the original >32m/s model-base offset"
        );
    }

    #[test]
    fn newly_blocked_silk_aborts_its_anchor_instead_of_preserving_a_stuck_descend_state() {
        let (terrain, chunks, mut journal) = fixture(120);
        journal.record([7, 7, 7], 3).unwrap();
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut animal = spider(Kind::MediumSpider);
        animal.position = [7.5, 7.0, 7.5];
        animal.behavior = Behavior::Descend;
        animal.needs.goal = Some(NeedGoal {
            kind: NeedKind::Food,
            position: [7, 12, 7],
        });
        falling(&mut animal, &habitat, 0.1, true);
        assert_eq!(animal.behavior, Behavior::Crawl);
        assert!(animal.needs.goal.is_none());
        assert!(contact(&habitat, &animal).is_none());
        journal.record([7, 7, 7], 0).unwrap();
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let before = animal.position[1];
        falling(&mut animal, &habitat, 0.1, false);
        assert!(animal.position[1] < before);
    }

    #[test]
    fn medium_spider_retreats_to_a_real_day_refuge_and_releases_it_at_night() {
        let (terrain, chunks, mut journal) = fixture(11);
        for x in 10..=14 {
            for z in 5..=10 {
                for y in 6..=8 {
                    journal.record([x, y, z], 3).unwrap();
                }
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut animal = spider(Kind::MediumSpider);
        let start = animal.position;
        let mut navigation = super::super::navigation::Navigation::default();
        for tick in 0..100 {
            step_with_navigation(
                &mut animal,
                &habitat,
                &[],
                0.1,
                tick % 10 == 0,
                false,
                Some(&mut navigation),
            );
            assert!(animal.valid());
            if shelter(&habitat, &animal).is_some() {
                break;
            }
        }
        assert!(distance(start, animal.position) > 1.0);
        assert!(shelter(&habitat, &animal).is_some());
        assert_eq!(animal.needs.goal.unwrap().kind, NeedKind::Shelter);
        animal.age = 36.5 - (animal.id % 17) as f64;
        step_with_navigation(
            &mut animal,
            &habitat,
            &[],
            0.1,
            false,
            false,
            Some(&mut navigation),
        );
        assert_eq!(animal.behavior, Behavior::Rest);
        step_with_navigation(
            &mut animal,
            &habitat,
            &[],
            0.1,
            false,
            true,
            Some(&mut navigation),
        );
        assert!(
            animal
                .needs
                .goal
                .is_none_or(|goal| goal.kind != NeedKind::Shelter)
        );
    }

    #[test]
    fn daylight_does_not_freeze_exposed_spiders_or_keep_a_removed_shelter() {
        let (terrain, chunks, mut journal) = fixture(12);
        let mut animal = spider(Kind::MediumSpider);
        let start = animal.position;
        {
            let habitat = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            for tick in 0..60 {
                step(&mut animal, &habitat, &[], 0.1, tick % 10 == 0, false);
                assert!(animal.valid());
                if tick == 0 {
                    assert!(
                        animal.needs.goal.is_none(),
                        "Disconnected lower chamber must not stay a movement target"
                    );
                    assert!(
                        animal.needs.search_in > 1.0,
                        "Failed refuge searches retain their bounded retry interval"
                    );
                }
            }
        }
        assert!(distance(start, animal.position) > 0.5);
        let roof = [
            animal.position[0].floor() as i64,
            6,
            animal.position[2].floor() as i64,
        ];
        journal.record(roof, 3).unwrap();
        animal.needs.goal = Some(NeedGoal {
            kind: NeedKind::Shelter,
            position: animal.position.map(|v| v.floor() as i64),
        });
        animal.needs.search_in = 10.0;
        let before = animal.position;
        journal.record(roof, 0).unwrap();
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        for _ in 0..20 {
            step(&mut animal, &habitat, &[], 0.1, false, false);
        }
        assert!(animal.needs.goal.is_none());
        assert!(distance(before, animal.position) > 0.25);
    }

    #[test]
    fn natural_large_spiders_turn_down_before_the_upper_depth_boundary_without_an_air_floor() {
        let (terrain, chunks, mut journal) = fixture(13);
        for y in -30..=-18 {
            for x in 4..=11 {
                journal.record([x, y, 5], 3).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut animal = spider(Kind::LargeSpider);
        animal.position = [7.5, -28.985, 7.0];
        animal = attach(&habitat, &animal, [7, -29, 5], [0, 0, 1]).unwrap();
        let start = animal.position[1];
        for _ in 0..120 {
            let center = terra_core::creatures::spiders::visual_center(&animal);
            move_surface(
                &mut animal,
                &habitat,
                [center[0], center[1] + 4.0, center[2]],
                0.1,
                1.1,
            );
            assert!(animal.position[1] + animal.size()[1] <= -22.0);
            assert!(clear(&habitat, &animal));
            assert!(contact(&habitat, &animal).is_some());
        }
        assert!(animal.position[1] > start + 1.0);
        assert_eq!(animal.behavior, Behavior::Climb);
        // Gravity is not constrained by the roaming guard and still needs real support.
        let mut falling_spider = spider(Kind::LargeSpider);
        falling_spider.position = [12.5, -23.0, 12.5];
        let before = falling_spider.position[1];
        step(&mut falling_spider, &habitat, &[], 0.1, false, true);
        assert!(falling_spider.position[1] < before);
    }

    #[test]
    fn owner_driven_large_spiders_climb_above_the_wild_depth_guard_with_real_wall_contact() {
        let (terrain, chunks, mut journal) = fixture(113);
        for y in -30..=-18 {
            for x in 4..=11 {
                journal.record([x, y, 5], 3).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut animal = spider(Kind::LargeSpider);
        animal.position = [7.5, -28.985, 7.0];
        animal = attach(&habitat, &animal, [7, -29, 5], [0, 0, 1]).unwrap();
        let center = terra_core::creatures::spiders::visual_center(&animal);
        let target = [center[0], -18.5, center[2]];
        for _ in 0..100 {
            companion_move(&mut animal, &habitat, target, 0.1, true, true);
            assert!(clear(&habitat, &animal));
            assert!(contact(&habitat, &animal).is_some());
        }
        assert!(animal.position[1] + animal.size()[1] > -22.0);
        assert_eq!(animal.behavior, Behavior::Climb);
    }

    #[test]
    fn finished_refuges_night_release_and_attached_combat_discard_stale_navigation_searches() {
        let (terrain, chunks, mut journal) = fixture(19);
        for x in 5..=10 {
            for z in 5..=10 {
                journal.record([x, 8, z], 3).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        for case in 0..3 {
            let mut animal = spider(Kind::MediumSpider);
            let mut navigation = super::super::navigation::Navigation::default();
            assert!(
                super::super::navigation::ground_waypoint(
                    &animal,
                    &habitat,
                    [11.5, 3.0, 7.5],
                    true,
                    Some(&mut navigation)
                )
                .is_some()
            );
            animal.age = 10.0;
            // A soft cache expiry does not queue another search while a valid
            // route remains. Validate the genuinely unmoved/stalled route first
            // without granting any A* budget, leaving a real pending retry.
            assert!(
                super::super::navigation::ground_waypoint(
                    &animal,
                    &habitat,
                    [11.5, 3.0, 7.5],
                    false,
                    Some(&mut navigation),
                )
                .is_none()
            );
            assert!(navigation.needs_search(animal.id, animal.age));
            animal.needs.goal = Some(NeedGoal {
                kind: NeedKind::Shelter,
                position: [7, 3, 7],
            });
            animal.needs.search_in = 10.0;
            if case == 0 {
                animal.age = 36.5 - (animal.id % 17) as f64;
                step_with_navigation(
                    &mut animal,
                    &habitat,
                    &[],
                    0.1,
                    false,
                    false,
                    Some(&mut navigation),
                );
                assert_eq!(animal.behavior, Behavior::Rest);
            } else if case == 1 {
                step_with_navigation(
                    &mut animal,
                    &habitat,
                    &[],
                    0.1,
                    false,
                    true,
                    Some(&mut navigation),
                );
                assert!(animal.needs.goal.is_none());
            } else {
                animal.position = [7.5, 6.0, 7.5];
                animal = attach(&habitat, &animal, [7, 8, 7], [0, -1, 0]).unwrap();
                animal.needs.goal = Some(NeedGoal {
                    kind: NeedKind::Food,
                    position: [7, 8, 7],
                });
                animal.needs.search_in = 0.0;
                for _ in 0..20 {
                    step_with_navigation(
                        &mut animal,
                        &habitat,
                        &[[7.5, 6.8, 7.5]],
                        0.1,
                        false,
                        true,
                        Some(&mut navigation),
                    );
                    assert_eq!(animal.behavior, Behavior::Ceiling);
                    assert!(animal.needs.search_in >= 1.0);
                }
            }
            assert!(!navigation.needs_search(animal.id, animal.age));
            assert!(!navigation.following_ground_route(animal.id));
            assert!(animal.valid());
        }
    }

    #[test]
    fn natural_spiders_are_seed_stable_capped_and_depth_filtered_even_at_negative_coordinates() {
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..24 {
            let (terrain, chunks, journal) = fixture(seed);
            let habitat = Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            };
            let home = [-1, -1];
            let mut population = CreaturePopulation::default();
            spawn(&mut population, &habitat, home, false, -12.0, 3);
            let first = population.clone();
            spawn(&mut population, &habitat, home, false, -12.0, 3);
            assert_eq!(first, population);
            let mut replica = CreaturePopulation::default();
            spawn(&mut replica, &habitat, home, false, -12.0, 3);
            assert_eq!(first, replica);
            assert!(population.valid());
            assert!(population.animals.len() <= 3);
            for animal in population.animals.values() {
                seen.insert(animal.kind);
                match animal.kind {
                    Kind::SmallSpider => assert!(animal.position[1] > 0.0),
                    Kind::MediumSpider => assert!((-22.0..0.0).contains(&animal.position[1])),
                    Kind::LargeSpider => assert!(animal.position[1] < -22.0),
                    _ => unreachable!(),
                }
                assert!(clear(&habitat, animal));
                assert!(contact(&habitat, animal).is_some());
            }
            let mut limited = CreaturePopulation::default();
            assert_eq!(spawn(&mut limited, &habitat, home, true, 3.0, 0), 0);
            spawn(&mut limited, &habitat, home, true, 3.0, 1);
            assert!(limited.animals.len() <= 1);
        }
        assert_eq!(seen.len(), 3);
    }
}
