//! Logical vegetation shared by headless generation, rendering and interaction.
use terra_voxel::SectionPos;
mod picking;
pub mod placement;
#[cfg(test)]
mod placement_tests;
pub mod seasonal;
pub mod visual;
pub mod webs;
#[derive(Debug, Clone, Copy)]
pub enum PropPose {
    Rest,
    Wind(f32),
    WeatherWind {
        time: f32,
        frame: [f32; 4],
    },
    Swept,
    WeatherSwept {
        frame: [f32; 4],
    },
    Seasonal {
        time: Option<f32>,
        frame: [f32; 4],
        swept: bool,
        season: terra_core::SeasonSample,
    },
}

impl PropPose {
    pub fn with_season(self, season: terra_core::SeasonSample) -> Self {
        let (time, frame, swept) = match self {
            Self::Rest => (None, [0.86, 0.51, 1.0, 1.0], false),
            Self::Wind(time) => (Some(time), [0.86, 0.51, 1.0, 1.0], false),
            Self::WeatherWind { time, frame } => (Some(time), frame, false),
            Self::Swept => (None, [0.86, 0.51, 1.0, 1.0], true),
            Self::WeatherSwept { frame } => (None, frame, true),
            Self::Seasonal {
                time, frame, swept, ..
            } => (time, frame, swept),
        };
        Self::Seasonal {
            time,
            frame,
            swept,
            season,
        }
    }
}

pub const WIND_AMPLITUDE: f32 = 0.026;
pub const WIND_FLUTTER: f32 = 0.007;
/// Upper bound shared by weather shaders, render bounds and authority picking.
pub const MAX_WEATHER_WIND: f32 = 4.0;

/// Same quantized frame is used by shaders, selection outlines and picking.
pub fn weather_wind_frame(wind: [f32; 2]) -> [f32; 4] {
    let speed = wind[0].hypot(wind[1]);
    let direction = if speed > f32::EPSILON {
        wind.map(|v| v / speed)
    } else {
        [0.0; 2]
    };
    [
        direction[0],
        direction[1],
        (0.18 + speed * speed * 0.065).clamp(0.18, MAX_WEATHER_WIND),
        1.0,
    ]
    .map(|v| (v * 128.0).round() / 128.0)
}

pub(super) fn wind_displacement(
    position: [f64; 3],
    time: f32,
    weights: [f32; 2],
    frame: [f32; 4],
) -> [f64; 2] {
    let [x, y, z] = position.map(|v| v as f32);
    if weights[1] < 0.0 {
        let pulse = (x * 0.21 + z * 0.18 - y * 0.45 + time * 0.85).sin();
        let flutter = (x * 0.8 - z * 0.6 + y * 0.7 + time * 1.4).sin();
        let sway =
            (pulse * WIND_AMPLITUDE + flutter * WIND_FLUTTER) * weights[0] * weights[1].abs();
        return [f64::from(sway * 0.86), f64::from(sway * 0.51)];
    }
    let strength = frame[2];
    let storm = ((strength - 1.0) / 3.0).clamp(0.0, 1.0);
    let front = (x * 0.025 + z * 0.018 - time * 1.15).sin();
    let base = (x * 0.33 + z * 0.27 + time * 1.05).sin();
    let gust = base + (0.62 + front * 0.38 - base) * storm;
    let flutter = (x * 1.17 - z * 0.91 + time * 2.1).sin();
    let sway =
        (gust * WIND_AMPLITUDE + flutter * WIND_FLUTTER) * weights[0] * weights[1] * strength;
    [f64::from(sway * frame[0]), f64::from(sway * frame[1])]
}
pub fn wind_parameters(kind: PropKind, variant: u8, height: f32, scale: f32) -> [f32; 2] {
    let (range, flexibility) = match kind {
        PropKind::Grass => (0.53, 1.0),
        PropKind::Bush if variant >= 128 => (0.65, 0.18),
        PropKind::Bush => (0.75, 0.65),
        PropKind::Flower => (0.60, 0.55),
        PropKind::Fern => (0.45, 0.7),
        PropKind::Wheat => (1.3, 0.5),
        PropKind::Corn => (2.0, 0.35),
        PropKind::Lettuce => (0.5, 0.15),
        PropKind::SmallPlant if variant < 128 => (0.9, 0.8),
        PropKind::Seagrass => (3.35, -7.5),
        PropKind::SeaAnemone => (0.50, -0.9),
        _ => return [0.0; 2],
    };
    [
        (height / range).clamp(0.0, 1.0).powi(2),
        flexibility * scale,
    ]
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct PropId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PropKind {
    Grass,
    Flower,
    Bush,
    Fern,
    Mushroom,
    SmallPlant,
    BeeNest,
    Seagrass,
    SeaCucumber,
    BranchingCoral,
    TubeCoral,
    PlateCoral,
    SeaAnemone,
    SmallStones,
    Wheat,
    Corn,
    Lettuce,
    Scarecrow,
    SpiderWeb,
}

impl PropKind {
    pub const ALL: [Self; 19] = [
        Self::Grass,
        Self::Flower,
        Self::Bush,
        Self::Fern,
        Self::Mushroom,
        Self::SmallPlant,
        Self::BeeNest,
        Self::Seagrass,
        Self::SeaCucumber,
        Self::BranchingCoral,
        Self::TubeCoral,
        Self::PlateCoral,
        Self::SeaAnemone,
        Self::SmallStones,
        Self::Wheat,
        Self::Corn,
        Self::Lettuce,
        Self::Scarecrow,
        Self::SpiderWeb,
    ];
    pub fn crop(self) -> Option<terra_core::farming::CropKind> {
        use terra_core::farming::CropKind;
        match self {
            Self::Wheat => Some(CropKind::Wheat),
            Self::Corn => Some(CropKind::Corn),
            Self::Lettuce => Some(CropKind::Lettuce),
            _ => None,
        }
    }
    pub fn aquatic(self) -> bool {
        matches!(
            self,
            Self::Seagrass
                | Self::SeaCucumber
                | Self::BranchingCoral
                | Self::TubeCoral
                | Self::PlateCoral
                | Self::SeaAnemone
                | Self::SmallStones
        )
    }
}

impl From<terra_core::ResourcePropKind> for PropKind {
    fn from(kind: terra_core::ResourcePropKind) -> Self {
        Self::ALL[kind as usize]
    }
}
impl From<PropKind> for terra_core::ResourcePropKind {
    fn from(kind: PropKind) -> Self {
        Self::ALL[kind as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorldProp {
    pub id: PropId,
    pub kind: PropKind,
    /// Template origin in world coordinates: ground top or an attached trunk face.
    pub position: [f64; 3],
    pub rotation: f32,
    pub scale: f32,
    pub variant: u8,
    /// Owner is the supporting block's section, including at vertical boundaries.
    pub section: SectionPos,
    pub removed: bool,
}

impl WorldProp {
    pub fn water_tolerant(&self) -> bool {
        self.kind.aquatic() || self.kind == PropKind::SmallPlant
    }
    /// High mushroom variants grow from a trunk face rather than a ground cell.
    pub fn wall_attached(&self) -> bool {
        matches!(self.kind, PropKind::BeeNest | PropKind::Mushroom) && self.variant >= 128
    }

    pub fn growing_grass(&self) -> bool {
        self.kind == PropKind::Grass && (64..=71).contains(&self.variant)
    }

    /// A pre-existing tuft already shows the growth; retain only its saved timer.
    pub fn hidden_growth(&self) -> bool {
        self.kind == PropKind::Grass && (68..=71).contains(&self.variant)
    }

    pub fn template_variant(&self) -> u8 {
        if self.kind == PropKind::SpiderWeb {
            self.variant
        } else if self.kind.aquatic() {
            self.variant % 8
        } else if self.growing_grass() {
            0
        } else {
            self.variant % 8 + if self.variant >= 128 { 128 } else { 0 }
        }
    }

    pub fn visit_selection_edges(&self, visit: impl FnMut([[f64; 3]; 2])) {
        picking::edges(self, PropPose::Rest, visit);
    }
    pub fn visit_animated_edges(&self, time: f32, visit: impl FnMut([[f64; 3]; 2])) {
        picking::edges(self, PropPose::Wind(time), visit);
    }
    pub fn visit_weather_edges(
        &self,
        time: f32,
        frame: [f32; 4],
        visit: impl FnMut([[f64; 3]; 2]),
    ) {
        picking::edges(self, PropPose::WeatherWind { time, frame }, visit);
    }
    /// Presentation scale is deliberately separate from persisted generation data.
    pub fn visual_scale(&self) -> f32 {
        let scale = self.scale * 1.3;
        match self.kind {
            kind if matches!(
                kind,
                PropKind::BeeNest | PropKind::Scarecrow | PropKind::SpiderWeb
            ) || kind.aquatic()
                || kind.crop().is_some() =>
            {
                self.scale
            }
            PropKind::Flower => scale.clamp(0.65, 0.95),
            PropKind::Mushroom => scale.clamp(0.35, 0.55),
            _ => scale,
        }
    }

    pub fn valid(&self) -> bool {
        let support = placement::support(self);
        self.position
            .iter()
            .all(|p| p.is_finite() && p.abs() <= 1_000_000.0)
            && self.scale.is_finite()
            && (0.1..=2.0).contains(&self.scale)
            && self.rotation.is_finite()
            && self.rotation.abs() <= std::f32::consts::TAU
            && (self.kind.crop().is_none() || self.variant <= 7)
            && (self.kind != PropKind::Scarecrow || self.variant <= 3)
            && (self.kind != PropKind::SpiderWeb || usize::from(self.variant) < webs::VARIANTS)
            && (!webs::is_construction(self)
                || ((0.6975..=2.0).contains(&self.scale) && {
                    let turn = self.rotation / std::f32::consts::FRAC_PI_2;
                    (turn - turn.round()).abs() < 0.001
                }))
            && terra_voxel::global_to_section(support).0 == self.section
    }
    /// Conservative bounds shared with procedural visual templates.
    pub fn bounds(&self) -> [[f64; 3]; 2] {
        self.bounds_at_scale(self.visual_scale())
    }

    /// Keep deterministic anchor jitter unchanged when presentation size changes.
    pub(crate) fn placement_bounds(&self) -> [[f64; 3]; 2] {
        self.bounds_at_scale(self.scale)
    }

    fn bounds_at_scale(&self, scale: f32) -> [[f64; 3]; 2] {
        if self.kind == PropKind::Grass {
            let radius = 0.4 * f64::from(scale);
            let [x, y, z] = self.position;
            return [
                [x - radius, y, z - radius],
                [x + radius, y + 0.65 * f64::from(scale), z + radius],
            ];
        }
        let variant = self.template_variant();
        let [minimum, maximum] = visual::template_bounds(self.kind, variant)
            .unwrap_or([[-1.5, 0.0, -1.5], [1.5, 3.0, 1.5]])
            .map(|point| point.map(|value| f64::from(value) * f64::from(scale)));
        let center = std::array::from_fn::<_, 3, _>(|axis| (minimum[axis] + maximum[axis]) * 0.5);
        let half = std::array::from_fn::<_, 3, _>(|axis| (maximum[axis] - minimum[axis]) * 0.5);
        let (sin, cos) = f64::from(self.rotation).sin_cos();
        let x = self.position[0] + cos * center[0] + sin * center[2];
        let z = self.position[2] - sin * center[0] + cos * center[2];
        let half_x = cos.abs() * half[0] + sin.abs() * half[2];
        let half_z = sin.abs() * half[0] + cos.abs() * half[2];
        [
            [x - half_x, self.position[1] + minimum[1], z - half_z],
            [x + half_x, self.position[1] + maximum[1], z + half_z],
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PropHit {
    pub prop: WorldProp,
    pub distance: f64,
}

/// Call with candidates from the sections intersecting the short interaction ray.
pub fn raycast_props<'a>(
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
    props: impl IntoIterator<Item = &'a WorldProp>,
) -> Option<PropHit> {
    raycast_props_with_pose(origin, direction, reach, props, PropPose::Rest)
}
pub fn raycast_props_with_pose<'a>(
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
    props: impl IntoIterator<Item = &'a WorldProp>,
    pose: PropPose,
) -> Option<PropHit> {
    raycast_props_with_lifecycle(origin, direction, reach, props, pose, None)
}

pub(crate) fn raycast_props_with_lifecycle<'a>(
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
    props: impl IntoIterator<Item = &'a WorldProp>,
    pose: PropPose,
    flowers: Option<&seasonal::FlowerLifecycle>,
) -> Option<PropHit> {
    if origin.iter().chain(&direction).any(|v| !v.is_finite())
        || !reach.is_finite()
        || !(0.0..=128.0).contains(&reach)
    {
        return None;
    }
    let length = direction.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !length.is_finite() || length <= f64::EPSILON {
        return None;
    }
    let direction = direction.map(|v| v / length);
    let mut closest: Option<PropHit> = None;
    let season = match pose {
        PropPose::Seasonal { season, .. } => season,
        _ => terra_core::WorldClock::default().season(terra_core::SeasonConfig::default()),
    };
    for prop in props {
        let coverage = flowers.map_or_else(
            || match pose {
                PropPose::Seasonal { season, .. } => seasonal::flower_coverage(prop, season),
                _ => 1.0,
            },
            |flowers| flowers.coverage(prop, season),
        );
        if prop.removed || prop.growing_grass() || coverage <= 0.0 {
            continue;
        }
        let [mut minimum, mut maximum] = prop.bounds_at_scale(prop.visual_scale() * coverage);
        if !matches!(pose, PropPose::Rest) {
            let strength = match pose {
                PropPose::WeatherWind { frame, .. }
                | PropPose::WeatherSwept { frame }
                | PropPose::Seasonal { frame, .. } => frame[2].clamp(0.0, MAX_WEATHER_WIND),
                PropPose::Wind(_) | PropPose::Swept => 1.0,
                _ => MAX_WEATHER_WIND,
            };
            let [_, flexibility] = wind_parameters(
                prop.kind,
                prop.variant,
                10.0,
                prop.visual_scale() * coverage,
            );
            let gain = if flexibility < 0.0 { 1.0 } else { strength };
            let margin = f64::from((WIND_AMPLITUDE + WIND_FLUTTER) * flexibility.abs() * gain);
            for axis in [0, 2] {
                minimum[axis] -= margin;
                maximum[axis] += margin;
            }
        }
        let mut near: f64 = 0.0;
        let mut far = closest.map_or(reach, |hit| hit.distance);
        for axis in 0..3 {
            if direction[axis].abs() < 1e-12 {
                if origin[axis] < minimum[axis] || origin[axis] > maximum[axis] {
                    far = -1.0;
                    break;
                }
                continue;
            }
            let first = (minimum[axis] - origin[axis]) / direction[axis];
            let second = (maximum[axis] - origin[axis]) / direction[axis];
            near = near.max(first.min(second));
            far = far.min(first.max(second));
        }
        let precise = (near <= far)
            .then(|| {
                if coverage == 1.0 {
                    picking::hit(prop, origin, direction, far, pose)
                } else {
                    picking::hit_scaled(prop, origin, direction, far, pose, coverage)
                }
            })
            .flatten();
        if let Some(distance) = precise
            && closest.is_none_or(|hit| {
                distance < hit.distance || (distance == hit.distance && prop.id < hit.prop.id)
            })
        {
            closest = Some(PropHit {
                prop: *prop,
                distance,
            });
        }
    }
    closest
}

/// Stable visual thinning, shared with picking; never changes logical prop state.
pub fn visible_at_density(prop: &WorldProp, density: f32) -> bool {
    density >= 1.0 || (density > 0.0 && ((prop.id.0 % 10_000) as f32) < density * 10_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prop(id: u64, x: f64) -> WorldProp {
        WorldProp {
            id: PropId(id),
            kind: PropKind::Bush,
            position: [x, 16.0, -0.5],
            rotation: 0.0,
            scale: 1.0,
            variant: 0,
            section: SectionPos::new(-1, 0, -1),
            removed: false,
        }
    }

    #[test]
    fn picking_selects_the_nearest_individual_and_skips_removed_props() {
        let mut props = [prop(1, -1.5), prop(2, -3.5)];
        let ray =
            |props: &[WorldProp]| raycast_props([0.0, 16.4, -0.5], [-1.0, 0.0, 0.0], 8.0, props);
        assert_eq!(ray(&props).unwrap().prop.id, PropId(1));
        props[0].removed = true;
        assert_eq!(ray(&props).unwrap().prop.id, PropId(2));
        assert!(raycast_props([0.0, 16.4, -0.5], [-1.0, 0.0, 0.0], 1.0, &props).is_none());
    }

    #[test]
    fn invalid_rays_and_parallel_misses_do_not_pick_props() {
        let props = [prop(1, -1.5)];
        assert!(raycast_props([0.0; 3], [0.0; 3], 8.0, &props).is_none());
        assert!(raycast_props([f64::NAN; 3], [1.0; 3], 8.0, &props).is_none());
        assert!(raycast_props([0.0, 16.4, 1.0], [-1.0, 0.0, 0.0], 8.0, &props).is_none());
    }

    #[test]
    fn density_thinning_is_stable_nested_and_never_removes_logical_props() {
        let props: Vec<_> = (0..10_000).map(|id| prop(id, -1.5)).collect();
        for density in [0.0, 0.25, 0.5, 1.0] {
            assert_eq!(
                props
                    .iter()
                    .filter(|p| visible_at_density(p, density))
                    .count(),
                (density * 10_000.0) as usize
            );
        }
        for prop in &props {
            assert!(!visible_at_density(prop, 0.25) || visible_at_density(prop, 0.5));
            assert!(!prop.removed);
        }
    }

    #[test]
    fn presentation_sizing_keeps_saved_props_and_anchors_unchanged() {
        for kind in [
            PropKind::Grass,
            PropKind::Flower,
            PropKind::Bush,
            PropKind::Fern,
            PropKind::Mushroom,
            PropKind::SmallPlant,
        ] {
            let plant = WorldProp {
                kind,
                ..prop(71, -1.5)
            };
            let [old_low, old_high] = plant.placement_bounds();
            let [low, high] = plant.bounds();
            let factor = f64::from(plant.visual_scale() / plant.scale);
            for axis in 0..3 {
                assert!(
                    ((high[axis] - low[axis]) / (old_high[axis] - old_low[axis]) - factor).abs()
                        < 1e-6
                );
            }
            assert_eq!(plant.scale, 1.0);
            assert_eq!(plant.position, [-1.5, 16.0, -0.5]);
        }
    }

    #[test]
    fn saved_flower_and_mushroom_sizes_obey_current_presentation_limits() {
        for (kind, minimum, maximum) in [
            (PropKind::Flower, 0.65, 0.95),
            (PropKind::Mushroom, 0.35, 0.55),
        ] {
            for scale in [0.1, 0.2, 0.35, 0.5, 1.0, 2.0] {
                let plant = WorldProp {
                    kind,
                    scale,
                    ..prop(71, -1.5)
                };
                assert!((minimum..=maximum).contains(&plant.visual_scale()));
                let [low, high] = plant.bounds();
                assert!(high[1] - low[1] <= f64::from(maximum) + 1e-6);
                assert!(high[0] - low[0] <= 0.50001);
                assert!(high[2] - low[2] <= 0.50001);
                assert_eq!(plant.scale, scale);
            }
        }
    }
}
