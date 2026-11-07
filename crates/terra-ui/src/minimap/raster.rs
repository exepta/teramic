use super::{Entry, Key, markers as glyphs};
use bevy::prelude::*;
use std::collections::HashMap;
use terra_core::{
    MapPalette, MinimapConfig,
    creatures::{CreatureBehavior, CreatureKind},
};
use terra_network::client::{ReplicatedCreatures, ReplicatedPlayers};
use terra_world::cartography::{DetailTile, MapLayer, MapRelief, MapSample, MapTile};

#[derive(Clone, Debug)]
pub(super) struct Projection {
    center: [f64; 2],
    rotation: f32,
    sine: f64,
    cosine: f64,
    pixels_per_meter: f32,
    relief: bool,
    size: usize,
    circular: bool,
    aspect: f32,
    three_dimensional: bool,
}
impl Projection {
    pub fn relief_enabled(&self) -> bool {
        self.relief
    }
    #[cfg(test)]
    pub fn new(center: [i64; 2], span: i64, yaw: f32, rotate: bool, relief: bool) -> Self {
        Self::at(
            center.map(|p| p as f64),
            span,
            yaw,
            rotate,
            relief,
            super::SIZE,
            true,
        )
    }
    pub fn at(
        center: [f64; 2],
        span: i64,
        yaw: f32,
        rotate: bool,
        relief: bool,
        size: usize,
        circular: bool,
    ) -> Self {
        let rotation = if rotate { -yaw } else { 0.0 };
        let (sine, cosine) = f64::from(rotation).sin_cos();
        let size = size.max(1);
        Self {
            center,
            rotation,
            sine,
            cosine,
            pixels_per_meter: size as f32 * if circular { 0.81 } else { 1.0 } / span.max(1) as f32,
            relief,
            size,
            circular,
            aspect: 1.0,
            three_dimensional: false,
        }
    }
    pub fn with_aspect(mut self, aspect: f32) -> Self {
        self.aspect = if aspect.is_finite() {
            aspect.clamp(0.2, 5.0)
        } else {
            1.0
        };
        self
    }
    pub fn three_dimensional(mut self, enabled: bool) -> Self {
        self.three_dimensional = enabled;
        self.relief |= enabled;
        self
    }
    pub fn is_three_dimensional(&self) -> bool {
        self.three_dimensional
    }
    pub fn aspect(&self) -> f32 {
        self.aspect
    }
    pub fn span(&self) -> f32 {
        self.size as f32 / self.pixels_per_meter
    }
    pub fn size(&self) -> usize {
        self.size
    }
    pub fn is_circular(&self) -> bool {
        self.circular
    }
    pub fn cached(&self) -> Self {
        let mut projection = self.clone();
        projection.rotation = 0.0;
        projection.sine = 0.0;
        projection.cosine = 1.0;
        let scale = [
            self.pixels_per_meter,
            self.pixels_per_meter * self.tilt() / self.aspect,
        ];
        projection.center = std::array::from_fn(|axis| {
            let scale = f64::from(scale[axis]);
            (self.center[axis] * scale).round() / scale
        });
        projection
    }
    pub fn world_at(&self, pixel: [f32; 2]) -> [f64; 2] {
        let dx = f64::from(pixel[0] - self.size as f32 * 0.5) / f64::from(self.pixels_per_meter);
        let dz = f64::from(pixel[1] - self.size as f32 * 0.5)
            / f64::from(self.pixels_per_meter * self.tilt() / self.aspect);
        [
            self.center[0] + dx * self.cosine - dz * self.sine,
            self.center[1] + dx * self.sine + dz * self.cosine,
        ]
    }
    pub fn world_at_height(&self, pixel: [f32; 2], height: f32, level: i64) -> [f64; 2] {
        let offset = self.relief_offset(height - level as f32);
        self.world_at([pixel[0] - offset[0], pixel[1] - offset[1]])
    }
    #[cfg(test)]
    fn world(&self, x: f32, y: f32) -> [i64; 2] {
        self.world_at([x, y]).map(|p| p.floor() as i64)
    }
    fn tilt(&self) -> f32 {
        if self.three_dimensional {
            0.60
        } else if self.relief {
            0.88
        } else {
            1.0
        }
    }
    fn lift(&self, height: f32) -> f32 {
        if self.three_dimensional {
            height * self.pixels_per_meter * 0.80
        } else if self.relief {
            height * self.pixels_per_meter * 0.23
        } else {
            0.0
        }
    }
    fn relief_offset(&self, height: f32) -> [f32; 2] {
        let lift = self.lift(height);
        // Relief points geographically north and scales in world units, so
        // cached terrain and live markers share the same affine projection.
        [
            -lift * self.sine as f32 / self.tilt(),
            -lift * self.cosine as f32 / self.aspect,
        ]
    }
    fn project_world(&self, position: [f64; 2], height: f64, level: i64) -> [f32; 2] {
        let dx = position[0] - self.center[0];
        let dz = position[1] - self.center[1];
        let offset = self.relief_offset((height - level as f64) as f32);
        [
            self.size as f32 * 0.5
                + ((dx * self.cosine + dz * self.sine) * f64::from(self.pixels_per_meter)) as f32
                + offset[0],
            self.size as f32 * 0.5
                + ((-dx * self.sine + dz * self.cosine)
                    * f64::from(self.pixels_per_meter * self.tilt() / self.aspect))
                    as f32
                + offset[1],
        ]
    }
    pub fn project(&self, position: [f32; 3], level: i64) -> [f32; 2] {
        self.project_world(
            [f64::from(position[0]), f64::from(position[2])],
            f64::from(position[1]),
            level,
        )
    }
    pub fn player_transform(&self, position: [f32; 3], level: i64, yaw: f32) -> Vec4 {
        let point = self.project(position, level);
        let (sine, cosine) = (yaw + self.rotation).sin_cos();
        Vec4::new(point[0], point[1], sine, cosine)
    }
    fn point(&self, position: [f32; 3], level: i64) -> [i32; 2] {
        self.project(position, level).map(|v| v.round() as i32)
    }
    fn contains(&self, point: [f32; 2], margin: f32) -> bool {
        let half = self.size as f32 * 0.5;
        if self.circular {
            (point[0] - half).powi(2) + (point[1] - half).powi(2)
                <= (self.radius() - margin).max(0.0).powi(2)
        } else {
            point
                .iter()
                .all(|p| *p >= margin && *p < self.size as f32 - margin)
        }
    }
    fn radius(&self) -> f32 {
        self.size as f32 * 0.338
    }
    /// Rows mapping current normalized UV into the cached terrain texture.
    /// The shader uses dot(row.xyz, vec3(uv, 1)); row.w is reserved.
    pub fn uv_transform_from(
        &self,
        reference: &Projection,
        level: i64,
        reference_level: i64,
    ) -> [Vec4; 2] {
        let map = |pixel| {
            let p = reference.project_world(self.world_at(pixel), level as f64, reference_level);
            Vec2::from_array(p) / reference.size as f32
        };
        let origin = map([0.0, 0.0]);
        let x = map([self.size as f32, 0.0]) - origin;
        let y = map([0.0, self.size as f32]) - origin;
        [
            Vec4::new(x.x, y.x, origin.x, 0.0),
            Vec4::new(x.y, y.y, origin.y, 0.0),
        ]
    }
    pub fn cardinal_style(&self, angle: f32) -> String {
        // The painted diamonds are not on a perfect circle (the south socket
        // sits higher). Rotate their actual texture landmarks with the frame.
        let radius = if (angle - std::f32::consts::PI).abs() < 0.01 {
            37.5
        } else if angle.abs() < 0.01 {
            41.0
        } else {
            40.5
        };
        let angle = angle - self.rotation;
        format!(
            "left: {:.2}%; top: {:.2}%;",
            50.0 + angle.sin() * radius,
            50.0 - angle.cos() * radius
        )
    }
    pub fn rotation(&self) -> f32 {
        self.rotation
    }
}

pub(super) struct Marker {
    pub player: bool,
    position: [f32; 3],
    kind: CreatureKind,
    variant: u8,
    hostile: bool,
}
pub(super) fn marker_visible(
    marker: &Marker,
    entries: &HashMap<Key, Entry>,
    layer: MapLayer,
    chunks: &terra_world::streaming::ChunkStore,
) -> bool {
    // Replicated players are already server-authorized. Exploration cache IO
    // must not hide a connected peer while its detailed tile is still pending.
    if marker.player {
        return true;
    }
    let [x, feet, z] = marker.position.map(|v| v.floor() as i64);
    // A live loaded animal must not wait for asynchronous exploration-cache IO.
    // Keep unloaded regions out, but use every existing height source, not only
    // the short-lived detailed tiles that are evicted during normal mapping.
    if chunks.block_at([x, feet, z]).is_none() && chunks.block_at([x, feet + 1, z]).is_none() {
        return false;
    }
    let floor = entries
        .get(&([x.div_euclid(16), z.div_euclid(16)], layer))
        .and_then(|entry| entry.height_at((z.rem_euclid(16) * 16 + x.rem_euclid(16)) as usize))
        .map(i64::from)
        .or_else(|| match layer {
            MapLayer::Surface => chunks.surface_column_at(x, z).map(|column| {
                column
                    .height
                    .max(column.water_level.unwrap_or(column.height))
                    .floor() as i64
            }),
            MapLayer::Cave { level } => Some(i64::from(level)),
        })
        .unwrap_or(feet + 40);
    if floor.abs_diff(feet) > 128 {
        return false;
    }
    // Leaves and water do not hide wildlife; a solid roof separates cave storeys.
    // Missing vertically streamed sections are not evidence of a solid roof.
    (feet.min(floor) + 1..feet.max(floor)).all(|y| {
        !chunks
            .block_at([x, y, z])
            .and_then(terra_voxel::MaterialId::from_raw)
            .is_some_and(terra_voxel::MaterialId::is_opaque)
    })
}

#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct PlayerLabel {
    pub id: String,
    pub name: String,
    pub style: String,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn player_labels(
    players: Option<&ReplicatedPlayers>,
    social: Option<&terra_network::client::SocialState>,
    projection: &Projection,
    level: i64,
    cave: bool,
    enabled: bool,
    size: Vec2,
) -> Vec<PlayerLabel> {
    if !enabled {
        return Vec::new();
    }
    let mut labels: Vec<_> = players
        .into_iter()
        .flat_map(|players| &players.players)
        .filter_map(|(id, track)| {
            let position = track.position().to_array();
            let pixel = projection.project(position, level);
            if !projection.contains(pixel, 12.0)
                || cave && (position[1] - level as f32).abs() > 24.0
            {
                return None;
            }
            let pixel = Vec2::from_array(pixel) * size / projection.size() as f32;
            Some(PlayerLabel {
                id: id.0.to_string(),
                name: social
                    .and_then(|social| social.names.get(id))
                    .cloned()
                    .unwrap_or_else(|| format!("Player {}", id.0)),
                style: format!("left: {:.2}px; top: {:.2}px;", pixel.x, pixel.y - 11.0),
            })
        })
        .collect();
    labels.sort_by(|a, b| a.id.cmp(&b.id));
    labels.truncate(128);
    labels
}
pub(super) fn markers(
    creatures: Option<&ReplicatedCreatures>,
    players: Option<&ReplicatedPlayers>,
    projection: &Projection,
    level: i64,
    cave: bool,
    settings: &MinimapConfig,
) -> Vec<Marker> {
    let mut out = Vec::new();
    let visible = |position: [f32; 3]| {
        projection.contains(
            projection.project(position, level),
            7.0 * settings.icon_scale,
        ) && (!cave || (position[1] - level as f32).abs() <= 20.0)
    };
    if settings.mobs
        && let Some(creatures) = creatures
    {
        for track in creatures.creatures.values() {
            let animal = &track.current;
            let position = track.position().to_array();
            if animal.health <= 0.0
                || matches!(
                    animal.behavior,
                    CreatureBehavior::Dead | CreatureBehavior::InNest
                )
                || !visible(position)
            {
                continue;
            }
            out.push(Marker {
                player: false,
                position,
                kind: animal.kind,
                variant: animal.variant,
                hostile: creatures
                    .bonds
                    .get(&animal.id)
                    .is_none_or(|bond| bond.owner.is_none())
                    && (animal.kind.hostile_to_players()
                        || matches!(
                            animal.behavior,
                            CreatureBehavior::Attack | CreatureBehavior::Hunt
                        )),
            });
        }
    }
    if settings.players
        && let Some(players) = players
    {
        for track in players.players.values() {
            let position = track.position().to_array();
            if visible(position) {
                out.push(Marker {
                    player: true,
                    position,
                    kind: CreatureKind::Sheep,
                    variant: 0,
                    hostile: false,
                });
            }
        }
    }
    out
}

pub(super) fn albedo(sample: &MapSample, palette: &MapPalette, layer: MapLayer) -> [f32; 3] {
    let mut rgb = palette
        .colors
        .get(usize::from(sample.material))
        .copied()
        .unwrap_or([0.16, 0.18, 0.16]);
    if sample.water_depth > 0 || matches!(sample.material, 14 | 27) {
        rgb = std::array::from_fn(|a| {
            sample
                .biome_weights
                .iter()
                .zip(palette.water_colors)
                .map(|(w, c)| w * c[a])
                .sum()
        });
        let attenuation = 1.0 - (f32::from(sample.water_depth).min(18.0) * 0.022);
        return rgb.map(|v| v * attenuation);
    }
    if matches!(sample.material, 1 | 4 | 5) {
        let grass: [f32; 3] = std::array::from_fn(|a| {
            sample
                .biome_weights
                .iter()
                .zip(palette.biome_colors)
                .map(|(w, c)| w * c[a])
                .sum()
        });
        if layer == MapLayer::Surface
            && sample.material == 1
            && palette.authored_grass
            && let Some(top) = palette.surface_grass
        {
            let tint = palette.authored_grass_tint(grass);
            return std::array::from_fn(|axis| top[axis] * tint[axis]);
        }
        let turf = palette.colors.get(1).copied().unwrap_or([0.38; 3]);
        let sand = palette.colors.get(4).copied().unwrap_or([0.72, 0.60, 0.34]);
        let snow = palette.colors.get(5).copied().unwrap_or([0.94, 0.97, 1.0]);
        let shore = sample.shore.clamp(0.0, 1.0);
        let aridity = sample.biome_weights[3] + sample.biome_weights[6];
        let snow_cover =
            sample.snow.clamp(0.0, 1.0) * (1.0 - ((aridity - 0.35) / 0.30).clamp(0.0, 1.0));
        let blend = match sample.material {
            4 => (shore / 0.35).clamp(0.0, 1.0),
            5 => (snow_cover / 0.35).clamp(0.0, 1.0),
            _ => 1.0,
        };
        for a in 0..3 {
            let natural = turf[a] * grass[a];
            let natural = natural + (sand[a] - natural) * shore;
            let natural = natural + (snow[a] - natural) * snow_cover;
            rgb[a] += (natural - rgb[a]) * blend;
        }
    }
    rgb
}
fn srgb(rgb: [f32; 3]) -> [u8; 4] {
    let c = Color::linear_rgb(rgb[0], rgb[1], rgb[2]).to_srgba();
    [byte(c.red), byte(c.green), byte(c.blue), 255]
}
fn byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}
pub(super) fn tile(detail: &DetailTile, palette: &MapPalette) -> MapTile {
    let mut tile = MapTile {
        coordinate: detail.coordinate,
        pixels: [0; 1024],
        relief: Some(Box::new(MapRelief {
            heights: [MapRelief::UNKNOWN; 256],
            water: [0; 32],
        })),
    };
    for (i, sample) in detail.cells.iter().enumerate() {
        if sample.known {
            tile.pixels[i * 4..i * 4 + 4].copy_from_slice(&srgb(albedo(
                sample,
                palette,
                detail.layer,
            )));
            if let Some(relief) = tile.relief.as_mut() {
                relief.heights[i] = sample.height;
                if sample.water_depth > 0 {
                    relief.water[i / 8] |= 1 << (i % 8);
                }
            }
        }
    }
    tile
}

#[derive(Clone, Copy)]
struct Pixel {
    color: [u8; 4],
    height: i16,
    known: bool,
    water: bool,
}
fn sample(
    entries: &HashMap<Key, Entry>,
    layer: MapLayer,
    p: [i64; 2],
    palette: &MapPalette,
    level: i64,
) -> Pixel {
    let entry = entries.get(&(p.map(|v| v.div_euclid(16)), layer));
    let offset = (p[1].rem_euclid(16) * 16 + p[0].rem_euclid(16)) as usize;
    let color = entry
        .and_then(|e| e.tile.as_ref())
        .map(|tile| {
            let p = &tile.pixels[offset * 4..offset * 4 + 4];
            [p[0], p[1], p[2], p[3]]
        })
        .or_else(|| {
            let overview = entry.and_then(|e| e.overview.as_ref())?;
            let offset = ((p[1].rem_euclid(16) / 4) * 4 + p[0].rem_euclid(16) / 4) as usize * 4;
            Some([
                overview.pixels[offset],
                overview.pixels[offset + 1],
                overview.pixels[offset + 2],
                overview.pixels[offset + 3],
            ])
        });
    if let Some(sample) = entry
        .and_then(|e| e.detail.as_ref())
        .map(|d| d.cells[offset])
        .filter(|s| s.known)
    {
        return Pixel {
            color: color.unwrap_or_else(|| srgb(albedo(&sample, palette, layer))),
            height: sample.height,
            known: sample.known,
            water: sample.water_depth > 0,
        };
    }
    let relief = entry
        .and_then(|e| e.tile.as_ref())
        .and_then(|tile| tile.relief.as_ref());
    Pixel {
        color: color.unwrap_or([24, 29, 28, 255]),
        height: entry
            .and_then(|entry| entry.height_at(offset))
            .unwrap_or(level as i16),
        known: color.is_some_and(|p| p[3] != 0),
        water: relief.map_or_else(
            || {
                entry
                    .and_then(|entry| entry.overview.as_ref())
                    .is_some_and(|overview| {
                        overview.water & (1 << ((offset / 16 / 4) * 4 + offset % 16 / 4)) != 0
                    })
            },
            |r| r.is_water(offset),
        ),
    }
}
fn shade(color: [u8; 4], factor: f32) -> [u8; 4] {
    [
        byte(f32::from(color[0]) / 255.0 * factor),
        byte(f32::from(color[1]) / 255.0 * factor),
        byte(f32::from(color[2]) / 255.0 * factor),
        255,
    ]
}
fn put(pixels: &mut [u8], size: usize, x: i32, y: i32, color: [u8; 4]) {
    if (0..size as i32).contains(&x) && (0..size as i32).contains(&y) {
        let i = (y as usize * size + x as usize) * 4;
        pixels[i..i + 4].copy_from_slice(&color);
    }
}

fn geometry_bounds(
    entries: &HashMap<Key, Entry>,
    layer: MapLayer,
    projection: &Projection,
    level: i64,
) -> ([i64; 2], [i64; 2]) {
    let mut minimum = level as i16;
    let mut maximum = level as i16;
    if projection.relief {
        for ((_, entry_layer), entry) in entries {
            if *entry_layer != layer {
                continue;
            }
            let mut include = |height: i16| {
                if height != MapRelief::UNKNOWN {
                    minimum = minimum.min(height);
                    maximum = maximum.max(height);
                }
            };
            if let Some(relief) = entry.tile.as_ref().and_then(|tile| tile.relief.as_ref()) {
                for &height in &relief.heights {
                    include(height);
                }
            } else if let Some(detail) = &entry.detail {
                for cell in detail.cells.iter().filter(|cell| cell.known) {
                    include(cell.height);
                }
            } else if let Some(overview) = &entry.overview {
                for &height in &overview.heights {
                    include(height);
                }
            }
        }
    }
    let size = projection.size as f32;
    let corners = [[0.0, 0.0], [size, 0.0], [0.0, size], [size, size]];
    let mut low = [f64::INFINITY; 2];
    let mut high = [f64::NEG_INFINITY; 2];
    for corner in corners {
        for height in [minimum, maximum] {
            let world = projection.world_at_height(corner, f32::from(height), level);
            for axis in 0..2 {
                low[axis] = low[axis].min(world[axis]);
                high[axis] = high[axis].max(world[axis]);
            }
        }
    }
    (
        low.map(|v| v.floor() as i64 - 2),
        high.map(|v| v.ceil() as i64 + 2),
    )
}

pub(super) fn terrain(
    entries: &HashMap<Key, Entry>,
    layer: MapLayer,
    palette: &MapPalette,
    projection: &Projection,
    level: i64,
) -> Vec<u8> {
    let size = projection.size;
    let mut pixels = [20, 26, 25, 255].repeat(size * size);
    let (low, high) = geometry_bounds(entries, layer, projection, level);
    let step = (projection.span() / size as f32).ceil().max(1.0) as i64;
    let low = low.map(|v| v.div_euclid(step) * step);
    let high = high.map(|v| v.div_euclid(step) * step + step);
    let mut rectangle = |a: [f32; 2], b: [f32; 2], color: [u8; 4]| {
        let min = Vec2::from_array(a)
            .min(Vec2::from_array(b))
            .floor()
            .as_ivec2();
        let max = Vec2::from_array(a)
            .max(Vec2::from_array(b))
            .ceil()
            .as_ivec2();
        for y in min.y.max(0)..max.y.min(size as i32) {
            for x in min.x.max(0)..max.x.min(size as i32) {
                let offset = (y as usize * size + x as usize) * 4;
                pixels[offset..offset + 4].copy_from_slice(&color);
            }
        }
    };
    // Rasterize complete top faces, not translated individual pixels. The
    // former splat path left seams whenever adjacent heights changed and
    // clipped all relief to a 32 m band around the player.
    for z in (low[1]..high[1]).step_by(step as usize) {
        for x in (low[0]..high[0]).step_by(step as usize) {
            let p = [x, z];
            let cell = sample(entries, layer, p, palette, level);
            if !cell.known {
                continue;
            }
            let south = sample(entries, layer, [x, z + step], palette, level);
            let mut slope = 1.0;
            if projection.relief && !cell.water {
                let west = sample(entries, layer, [x - step, z], palette, level);
                let north = sample(entries, layer, [x, z - step], palette, level);
                let dx = if west.known {
                    f32::from(cell.height - west.height) / step as f32
                } else {
                    0.0
                };
                let dz = if north.known {
                    f32::from(cell.height - north.height) / step as f32
                } else {
                    0.0
                };
                slope = (1.0 + dx.clamp(-3.0, 3.0) * 0.065 + dz.clamp(-3.0, 3.0) * 0.085)
                    .clamp(0.65, 1.22);
            }
            let grain = ((x.wrapping_mul(73856093) ^ z.wrapping_mul(19349663)) & 15) as f32 / 15.0;
            let color = shade(cell.color, slope * (0.97 + grain * 0.06));
            let top = projection.project_world([x as f64, z as f64], f64::from(cell.height), level);
            let front = projection.project_world(
                [(x + step) as f64, (z + step) as f64],
                f64::from(cell.height),
                level,
            );
            rectangle(top, front, color);
            if projection.relief && south.known && cell.height > south.height && !cell.water {
                let bottom = projection.project_world(
                    [(x + step) as f64, (z + step) as f64],
                    f64::from(south.height),
                    level,
                );
                let edge = projection.project_world(
                    [x as f64, (z + step) as f64],
                    f64::from(cell.height),
                    level,
                );
                rectangle(edge, bottom, shade(color, 0.66));
            }
        }
    }
    pixels
}

fn glyph(
    pixels: &mut [u8],
    projection: &Projection,
    center: [i32; 2],
    sprite: &[u8; glyphs::ICON_SIZE * glyphs::ICON_SIZE * 4],
    scale: f32,
) {
    let width = (glyphs::ICON_SIZE as f32 * scale).round() as i32;
    let height = (width as f32 / projection.aspect).round().max(1.0) as i32;
    for y in 0..height {
        for x in 0..width {
            let index = ((y as usize * glyphs::ICON_SIZE / height as usize) * glyphs::ICON_SIZE
                + x as usize * glyphs::ICON_SIZE / width as usize)
                * 4;
            let color = [
                sprite[index],
                sprite[index + 1],
                sprite[index + 2],
                sprite[index + 3],
            ];
            if color[3] == 0 {
                continue;
            }
            let point = [center[0] + x - width / 2, center[1] + y - height / 2];
            if projection.contains(point.map(|v| v as f32), 0.0) {
                put(pixels, projection.size, point[0], point[1], color);
            }
        }
    }
}

#[cfg(test)]
pub(super) fn overlay(
    projection: &Projection,
    level: i64,
    yaw: f32,
    markers: &[Marker],
    icon_scale: f32,
) -> Vec<u8> {
    overlay_at(
        projection,
        level,
        yaw,
        markers,
        icon_scale,
        [
            projection.center[0] as f32,
            level as f32,
            projection.center[1] as f32,
        ],
    )
}

#[cfg(test)]
pub(super) fn overlay_at(
    projection: &Projection,
    level: i64,
    yaw: f32,
    markers: &[Marker],
    icon_scale: f32,
    player_position: [f32; 3],
) -> Vec<u8> {
    let mut pixels = marker_overlay(projection, level, markers, icon_scale);
    let size = projection.size;
    let arrow_yaw = yaw + projection.rotation;
    let (s, c) = arrow_yaw.sin_cos();
    let center = projection.point(player_position, level);
    for y in -13..=13 {
        for x in -13..=13 {
            let point = [center[0] + x, center[1] + y];
            if !projection.contains(point.map(|v| v as f32), 0.0) {
                continue;
            }
            let d = (x * x + y * y) as f32;
            if (140.0..=165.0).contains(&d) {
                put(&mut pixels, size, point[0], point[1], [224, 222, 166, 220]);
            }
            let forward = -(x as f32) * s - (y as f32) * c;
            let side = x as f32 * c - y as f32 * s;
            if (-6.0..=10.0).contains(&forward) && side.abs() <= (10.0 - forward) * 0.48 {
                let edge = side.abs() > (10.0 - forward) * 0.48 - 1.6 || forward < -4.0;
                put(
                    &mut pixels,
                    size,
                    point[0],
                    point[1],
                    if edge {
                        [13, 27, 34, 255]
                    } else {
                        [82, 221, 255, 255]
                    },
                );
            }
        }
    }
    pixels
}

#[cfg(test)]
pub(super) fn marker_overlay(
    projection: &Projection,
    level: i64,
    markers: &[Marker],
    icon_scale: f32,
) -> Vec<u8> {
    marker_overlay_with_underlay(projection, level, markers, icon_scale, None)
}

pub(super) fn marker_overlay_with_underlay(
    projection: &Projection,
    level: i64,
    markers: &[Marker],
    icon_scale: f32,
    underlay: Option<&[u8]>,
) -> Vec<u8> {
    let size = projection.size;
    let mut pixels = underlay
        .filter(|p| p.len() == size * size * 4)
        .map_or_else(|| vec![0; size * size * 4], <[u8]>::to_vec);
    let icon_scale = if icon_scale.is_finite() {
        icon_scale.clamp(1.0, 2.5)
    } else {
        1.0
    };
    for marker in markers {
        let center = projection.point(marker.position, level);
        if marker.player {
            glyph(&mut pixels, projection, center, &glyphs::player_icon(), 1.0);
        } else {
            glyph(
                &mut pixels,
                projection,
                center,
                &glyphs::creature_icon(marker.kind, marker.variant, marker.hostile),
                icon_scale,
            );
        }
    }
    pixels
}

#[cfg(test)]
fn render(
    entries: &HashMap<Key, Entry>,
    layer: MapLayer,
    palette: &MapPalette,
    projection: &Projection,
    level: i64,
    yaw: f32,
    markers: &[Marker],
) -> Vec<u8> {
    let mut pixels = terrain(entries, layer, palette, projection, level);
    let overlay = overlay(projection, level, yaw, markers, 1.0);
    for (base, icon) in pixels.chunks_exact_mut(4).zip(overlay.chunks_exact(4)) {
        if icon[3] != 0 {
            base.copy_from_slice(icon);
        }
    }
    if projection.circular {
        for y in 0..projection.size {
            for x in 0..projection.size {
                if !projection.contains([x as f32, y as f32], 0.0) {
                    pixels[(y * projection.size + x) * 4..(y * projection.size + x) * 4 + 4]
                        .fill(0);
                }
            }
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minimap::SIZE;

    fn loaded_air(position: terra_voxel::SectionPos) -> terra_world::streaming::LoadedSection {
        terra_world::streaming::LoadedSection {
            data: terra_voxel::BlockSection::empty(position).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        }
    }

    #[test]
    fn live_wildlife_does_not_depend_on_detail_tile_io_and_known_roofs_still_separate_storeys() {
        use terra_voxel::global_to_section;
        for location in [[0, 2, 0], [-1, 2, -1], [16, 18, -17]] {
            let (owner, cell) = global_to_section(location);
            let mut chunks = terra_world::streaming::ChunkStore::default();
            chunks.loaded.insert(owner, loaded_air(owner));
            let marker = Marker {
                player: false,
                position: [
                    location[0] as f32 + 0.5,
                    location[1] as f32,
                    location[2] as f32 + 0.5,
                ],
                kind: CreatureKind::Cow,
                variant: 0,
                hostile: false,
            };
            let mut entries = HashMap::new();
            assert!(marker_visible(
                &marker,
                &entries,
                MapLayer::Surface,
                &chunks
            ));
            entries.insert(
                ([owner.x, owner.z], MapLayer::Surface),
                Entry {
                    tile: None,
                    detail: None,
                    overview: Some(std::sync::Arc::new(super::super::Overview {
                        pixels: [0; 64],
                        heights: [location[1] as i16 + 5; 16],
                        water: 0,
                    })),
                    source_revision: None,
                    palette_fingerprint: MapPalette::default().fingerprint(),
                    used: 0,
                },
            );
            assert!(marker_visible(
                &marker,
                &entries,
                MapLayer::Surface,
                &chunks
            ));
            let roof = [location[0], location[1] + 3, location[2]];
            let (roof_owner, roof_cell) = global_to_section(roof);
            chunks
                .loaded
                .entry(roof_owner)
                .or_insert_with(|| loaded_air(roof_owner))
                .data
                .set_block(roof_cell, 3)
                .unwrap();
            assert!(!marker_visible(
                &marker,
                &entries,
                MapLayer::Surface,
                &chunks
            ));
            chunks
                .loaded
                .get_mut(&roof_owner)
                .unwrap()
                .data
                .set_block(roof_cell, 7)
                .unwrap();
            assert!(marker_visible(
                &marker,
                &entries,
                MapLayer::Surface,
                &chunks
            ));
            chunks
                .loaded
                .get_mut(&owner)
                .unwrap()
                .data
                .set_block(cell, 27)
                .unwrap();
            assert!(marker_visible(
                &marker,
                &entries,
                MapLayer::Surface,
                &chunks
            ));
            chunks.loaded.clear();
            assert!(!marker_visible(
                &marker,
                &entries,
                MapLayer::Surface,
                &chunks
            ));
        }
    }

    #[test]
    fn deep_water_mobs_remain_visible_but_a_known_rock_roof_is_not_water() {
        use terra_voxel::{SectionPos, global_to_section};
        let mut chunks = terra_world::streaming::ChunkStore::default();
        let location = [-1, -55, -1];
        let (owner, local) = global_to_section(location);
        let mut section = loaded_air(owner);
        section.data.set_block(local, 27).unwrap();
        chunks.loaded.insert(owner, section);
        let layer = MapLayer::Surface;
        let marker = Marker {
            player: false,
            position: [-0.5, -55.0, -0.5],
            kind: CreatureKind::ReefShark,
            variant: 0,
            hostile: true,
        };
        let entries = HashMap::from([(
            ([-1, -1], layer),
            Entry {
                tile: None,
                detail: None,
                overview: Some(std::sync::Arc::new(super::super::Overview {
                    pixels: [0; 64],
                    heights: [0; 16],
                    water: u16::MAX,
                })),
                source_revision: None,
                palette_fingerprint: MapPalette::default().fingerprint(),
                used: 0,
            },
        )]);
        assert!(marker_visible(&marker, &entries, layer, &chunks));
        let roof = SectionPos::new(-1, -2, -1);
        let mut rock = loaded_air(roof);
        rock.data.set_block([15, 12, 15], 3).unwrap();
        chunks.loaded.insert(roof, rock);
        assert!(!marker_visible(&marker, &entries, layer, &chunks));
    }

    #[test]
    fn real_creature_snapshots_update_icons_without_tile_repaints_and_keep_tame_hostiles_friendly()
    {
        use terra_core::creatures::{
            Creature, CreatureNeeds,
            taming::{CompanionMode, CompanionView},
        };
        use terra_network::client::CreatureTrack;
        let mut animals = ReplicatedCreatures::default();
        for (index, kind) in [
            CreatureKind::Cow,
            CreatureKind::Fish,
            CreatureKind::SmallSpider,
            CreatureKind::MediumSpider,
            CreatureKind::Stalker,
            CreatureKind::Bee,
        ]
        .into_iter()
        .enumerate()
        {
            let animal = Creature {
                id: index as u64 + 1,
                home: [-1, -1],
                kind,
                position: [-8.0 + index as f64 * 3.0, 2.0, -8.0],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: kind.health(),
                behavior: if kind == CreatureKind::Bee {
                    CreatureBehavior::InNest
                } else {
                    CreatureBehavior::Rest
                },
                timer: 0.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(kind),
            };
            animals.creatures.insert(
                animal.id,
                CreatureTrack {
                    previous: animal.clone(),
                    current: animal,
                    interval: 0.1,
                    received: std::time::Instant::now(),
                    playback: Default::default(),
                },
            );
        }
        let owner = terra_core::PlayerUuid::from_bytes([1; 16]);
        animals.bonds.insert(
            4,
            CompanionView {
                id: 4,
                owner: Some(owner),
                player: owner,
                trust: 100,
                mode: CompanionMode::Wait,
            },
        );
        let settings = MinimapConfig::default();
        let projection = Projection::at([0.0, -8.0], 96, 0.0, false, true, 320, true);
        let first = markers(Some(&animals), None, &projection, 2, false, &settings);
        assert_eq!(first.len(), 5);
        assert!(
            first
                .iter()
                .find(|marker| marker.kind == CreatureKind::Stalker)
                .unwrap()
                .hostile
        );
        assert!(
            !first
                .iter()
                .find(|marker| marker.kind == CreatureKind::MediumSpider)
                .unwrap()
                .hostile
        );
        let before = marker_overlay(&projection, 2, &first, 1.0);
        let moved = animals.creatures.get_mut(&1).unwrap();
        moved.current.position[2] += 12.0;
        moved.previous = moved.current.clone();
        let after = marker_overlay(
            &projection,
            2,
            &markers(Some(&animals), None, &projection, 2, false, &settings),
            1.0,
        );
        assert_ne!(
            before, after,
            "live snapshots repaint the mob overlay independently of terrain cache"
        );
        assert!(before.chunks_exact(4).filter(|pixel| pixel[3] != 0).count() > 100);
        let mut hidden = settings;
        hidden.mobs = false;
        assert!(markers(Some(&animals), None, &projection, 2, false, &hidden).is_empty());
    }

    #[test]
    fn peers_keep_icons_and_names_before_detail_io_and_in_live_3d() {
        use terra_network::{
            client::{RemoteTrack, SocialState},
            protocol::session::PlayerState,
        };
        let state = PlayerState {
            player: terra_core::PlayerId(9),
            position: [-27.0, 7.0, -31.0],
            ..default()
        };
        let mut players = ReplicatedPlayers::default();
        players.players.insert(
            state.player,
            RemoteTrack {
                previous: state,
                current: state,
                received: std::time::Instant::now(),
                interval: 0.05,
            },
        );
        let social = SocialState {
            names: HashMap::from([(state.player, "Mira".into())]),
            ..default()
        };
        for three in [false, true] {
            let projection = Projection::at([-24.0, -24.0], 96, 0.0, false, true, 320, !three)
                .three_dimensional(three);
            let markers = markers(
                None,
                Some(&players),
                &projection,
                4,
                false,
                &MinimapConfig::default(),
            );
            assert_eq!(markers.len(), 1);
            assert!(marker_visible(
                &markers[0],
                &HashMap::new(),
                MapLayer::Surface,
                &Default::default()
            ));
            let labels = player_labels(
                Some(&players),
                Some(&social),
                &projection,
                4,
                false,
                true,
                Vec2::splat(320.0),
            );
            assert_eq!(labels.len(), 1);
            assert_eq!(labels[0].name, "Mira");
            assert!(labels[0].style.contains("left:"));
            assert!(
                player_labels(
                    Some(&players),
                    Some(&social),
                    &projection,
                    -40,
                    true,
                    true,
                    Vec2::splat(320.0)
                )
                .is_empty()
            );
        }
    }

    #[test]
    fn evicting_live_detail_keeps_identical_persistent_relief_and_water() {
        let layer = MapLayer::Surface;
        let mut detail = DetailTile {
            coordinate: [-1, -1],
            layer,
            revision: 2,
            cells: [MapSample {
                known: true,
                material: 3,
                height: 70,
                ..default()
            }; 256],
        };
        detail.cells[7].height = 86;
        detail.cells[7].water_depth = 2;
        let palette = MapPalette::default();
        let tile = tile(&detail, &palette);
        let mut entries = HashMap::from([(
            ([-1, -1], layer),
            Entry {
                tile: Some(std::sync::Arc::new(tile)),
                detail: Some(std::sync::Arc::new(detail)),
                overview: None,
                source_revision: Some(2),
                palette_fingerprint: palette.fingerprint(),
                used: 1,
            },
        )]);
        let projection = Projection::at([-8.0, -8.0], 24, 0.0, false, true, 64, false);
        let live = terrain(&entries, layer, &palette, &projection, 70);
        entries.get_mut(&([-1, -1], layer)).unwrap().detail = None;
        assert_eq!(terrain(&entries, layer, &palette, &projection, 70), live);
        assert_eq!(sample(&entries, layer, [-9, -16], &palette, 0).height, 86);
        assert!(sample(&entries, layer, [-9, -16], &palette, 0).water);
    }

    #[test]
    fn distant_overview_terrain_retains_water_and_height_without_live_sections() {
        let layer = MapLayer::Surface;
        let mut relief = MapRelief {
            heights: [142; 256],
            water: [255; 32],
        };
        relief.heights[0] = 142;
        let tile = MapTile {
            coordinate: [25, -30],
            pixels: [140; 1024],
            relief: Some(Box::new(relief)),
        };
        let entries = HashMap::from([(
            (tile.coordinate, layer),
            Entry {
                overview: Some(std::sync::Arc::new(super::super::overview(&tile))),
                tile: None,
                detail: None,
                source_revision: None,
                palette_fingerprint: MapPalette::default().fingerprint(),
                used: 0,
            },
        )]);
        let cell = sample(&entries, layer, [400, -480], &MapPalette::default(), 80);
        assert!(cell.known && cell.water);
        assert_eq!(cell.height, 142);
        let projection = Projection::new([400, -480], 192, 0.0, false, true);
        let low = projection.project([400.0, 140.0, -480.0], 80);
        let high = projection.project([400.0, 160.0, -480.0], 80);
        assert!(
            high[1] < low[1] - 5.0,
            "relief must not become flat outside the former +/-32 m height band"
        );
    }

    #[test]
    fn rectangular_3d_uses_real_unclamped_elevation_and_preserves_reprojection() {
        let projection = Projection::at([8.0, 8.0], 32, 0.0, false, true, 640, false)
            .with_aspect(0.5)
            .three_dimensional(true);
        assert_eq!(projection.world_at([0.0, 320.0]), [-8.0, 8.0]);
        let ground = projection.project([8.0, 80.0, 8.0], 80);
        let high = projection.project([8.0, 180.0, 8.0], 80);
        assert!((ground[1] - high[1] - 3200.0).abs() < 0.01);
        let current = Projection::at([9.25, 7.7], 32, 0.0, false, true, 640, false)
            .with_aspect(0.5)
            .three_dimensional(true);
        let rows = current.uv_transform_from(&projection, 80, 80);
        for height in [-120.0, 80.0, 350.0] {
            let current_uv = Vec2::from_array(current.project([10.0, height, 9.0], 80)) / 640.0;
            let point = current_uv.extend(1.0).extend(0.0);
            let actual = Vec2::new(rows[0].dot(point), rows[1].dot(point));
            let expected = Vec2::from_array(projection.project([10.0, height, 9.0], 80)) / 640.0;
            assert!((actual - expected).length() < 0.00001);
        }
    }

    #[test]
    fn cached_relief_reprojects_real_heights_across_fast_rotation_zoom_and_translation() {
        let reference =
            Projection::at([-140.25, -67.125], 384, 0.0, false, true, 320, true).cached();
        for (center, span, yaw) in [
            ([-132.6, -71.625], 192, 1.3),
            ([-142.25, -61.0], 96, -2.8),
            ([-139.0, -68.5], 768, 3.1),
        ] {
            let current = Projection::at(center, span, yaw, true, true, 320, true);
            let rows = current.uv_transform_from(&reference, 80, 80);
            for height in [-120.0, 62.0, 80.0, 102.0, 350.0] {
                let world = [-137.5, -65.25];
                let screen = Vec2::from_array(current.project_world(world, height, 80)) / 320.0;
                let actual = Vec2::new(
                    rows[0].dot(screen.extend(1.0).extend(0.0)),
                    rows[1].dot(screen.extend(1.0).extend(0.0)),
                );
                let expected = Vec2::from_array(reference.project_world(world, height, 80)) / 320.0;
                assert!(
                    (actual - expected).length() < 0.000_002,
                    "height {height}, yaw {yaw}"
                );
            }
            let cached = current.cached();
            assert_eq!(cached.rotation, 0.0);
            assert!(
                (cached.center[0] * f64::from(cached.pixels_per_meter))
                    .fract()
                    .abs()
                    < 0.000_001
            );
        }
    }

    #[test]
    fn changing_follow_ground_datum_keeps_arrow_terrain_and_cached_icons_geographically_aligned() {
        let reference =
            Projection::at([-140.25, -67.125], 384, 0.0, false, true, 320, true).cached();
        for (center, level) in [
            ([-132.6, -71.625], -32),
            ([-0.2, 16.25], 104),
            ([32.125, -64.5], 200),
        ] {
            for yaw in [-2.8, 0.0, 1.3, 3.1] {
                let current = Projection::at(center, 192, yaw, true, true, 320, true);
                let rows = current.uv_transform_from(&reference, level, 80);
                let ground = [center[0] as f32, level as f32, center[1] as f32];
                let arrow = current.player_transform(ground, level, yaw);
                assert!((arrow.xy() - Vec2::splat(160.0)).length() < 0.0001);
                for (world, height) in [
                    (center, level as f64),
                    ([-137.5, -65.25], -120.0),
                    ([12.25, -0.125], 350.0),
                ] {
                    let point =
                        Vec2::from_array(current.project_world(world, height, level)) / 320.0;
                    let actual = Vec2::new(
                        rows[0].dot(point.extend(1.0).extend(0.0)),
                        rows[1].dot(point.extend(1.0).extend(0.0)),
                    );
                    let expected =
                        Vec2::from_array(reference.project_world(world, height, 80)) / 320.0;
                    assert!(
                        actual.distance(expected) < 0.00001,
                        "level={level}, yaw={yaw}"
                    );
                }
            }
        }
    }

    #[test]
    fn reprojection_tracks_fractional_centers_rotation_scale_and_height() {
        let reference = Projection::at([-140.25, -67.125], 384, 0.7, true, true, 320, true);
        for (rotate, relief, circular, size) in [
            (false, false, false, 640),
            (true, true, false, 640),
            (true, true, true, 320),
        ] {
            let projection =
                Projection::at([-132.6, -71.625], 192, 1.3, rotate, relief, size, circular);
            let rows = projection.uv_transform_from(&reference, 42, 38);
            for uv in [
                Vec2::ZERO,
                Vec2::ONE,
                Vec2::new(0.41, 0.76),
                Vec2::new(-0.2, 1.1),
            ] {
                let world = projection.world_at((uv * size as f32).to_array());
                let expected = Vec2::from_array(reference.project_world(world, 42.0, 38)) / 320.0;
                let point = uv.extend(1.0).extend(0.0);
                let actual = Vec2::new(rows[0].dot(point), rows[1].dot(point));
                assert!((expected - actual).length() < 0.000_002);
            }
            let center = projection.project_world([-132.6, -71.625], 42.0, 42);
            assert_eq!(center, [size as f32 * 0.5; 2]);
        }
        let identity = reference.uv_transform_from(&reference, 38, 38);
        assert!((identity[0] - Vec4::new(1.0, 0.0, 0.0, 0.0)).length() < 0.000_001);
        assert!((identity[1] - Vec4::new(0.0, 1.0, 0.0, 0.0)).length() < 0.000_001);
    }

    #[test]
    fn cache_covers_rectangle_while_only_circular_overlays_clip_their_edges() {
        let empty = HashMap::new();
        let palette = MapPalette::default();
        for circular in [false, true] {
            let projection = Projection::at([0.0; 2], 100, 0.0, false, false, 640, circular);
            assert_eq!(projection.size(), 640);
            assert_eq!(projection.is_circular(), circular);
            let terrain = terrain(&empty, MapLayer::Surface, &palette, &projection, 0);
            assert_eq!(terrain.len(), 640 * 640 * 4);
            assert!(terrain.chunks_exact(4).all(|p| p[3] == 255));
            let marker = Marker {
                player: false,
                position: [-42.0, 0.0, -42.0],
                kind: CreatureKind::Cow,
                variant: 0,
                hostile: false,
            };
            let overlay = overlay(&projection, 0, 0.0, &[marker], 2.5);
            let colored = overlay.chunks_exact(4).filter(|p| p[3] != 0).count();
            if circular {
                assert!(
                    colored < 400,
                    "Corner marker must not leak through compass frame"
                );
            } else {
                assert!(
                    colored > 500,
                    "Rectangular FullMap must retain corner markers"
                );
            }
            assert_eq!(&overlay[..4], &[0; 4]);
        }
    }

    #[test]
    fn icon_scale_enlarges_animals_without_changing_players_or_navigation_arrow() {
        let projection = Projection::at([0.0; 2], 100, 0.0, false, false, 320, false);
        let markers = [
            Marker {
                player: false,
                position: [20.0, 0.0, 0.0],
                kind: CreatureKind::Cow,
                variant: 0,
                hostile: false,
            },
            Marker {
                player: true,
                position: [-20.0, 0.0, 0.0],
                kind: CreatureKind::Sheep,
                variant: 0,
                hostile: false,
            },
        ];
        let small = overlay(&projection, 0, 0.0, &markers, 1.0);
        let large = overlay(&projection, 0, 0.0, &markers, 2.5);
        let region = |pixels: &[u8], left: usize, right: usize| {
            (140..180)
                .flat_map(|y| (left..right).map(move |x| (y * 320 + x) * 4))
                .filter(|index| pixels[*index + 3] != 0)
                .count()
        };
        assert!(region(&large, 200, 245) > region(&small, 200, 245) * 4);
        for y in 140..180 {
            for x in 70..180 {
                let i = (y * 320 + x) * 4;
                assert_eq!(&small[i..i + 4], &large[i..i + 4]);
            }
        }
    }
    #[test]
    fn colors_follow_actual_palette_and_different_materials() {
        let mut palette = MapPalette {
            colors: vec![[0.3, 0.3, 0.3]; 44],
            biome_colors: [[0.2, 0.6, 0.1]; 8],
            ..Default::default()
        };
        let sample = MapSample {
            material: 1,
            known: true,
            ..Default::default()
        };
        let grass = albedo(&sample, &palette, MapLayer::Surface);
        assert!(grass[1] > grass[0] && grass[1] > grass[2]);
        palette.colors[3] = [0.8, 0.1, 0.2];
        assert_eq!(
            albedo(
                &MapSample {
                    material: 3,
                    ..sample
                },
                &palette,
                MapLayer::Surface
            ),
            [0.8, 0.1, 0.2]
        );
        assert_ne!(
            grass,
            albedo(
                &MapSample {
                    material: 3,
                    ..sample
                },
                &palette,
                MapLayer::Surface
            )
        );
    }

    fn authored_palette() -> MapPalette {
        let mut palette = MapPalette {
            colors: vec![[0.15, 0.09, 0.03]; 44],
            biome_colors: [[0.28, 0.40, 0.08]; 8],
            authored_grass: true,
            surface_grass: Some([0.28, 0.46, 0.08]),
            ..Default::default()
        };
        palette.biome_colors[2] = [0.2, 0.5, 0.1];
        palette.biome_colors[3] = [0.8, 0.6, 0.05];
        palette
    }

    #[test]
    fn authored_surface_grass_uses_the_painted_top_and_relative_biome_color() {
        let palette = authored_palette();
        let forest = MapSample {
            material: 1,
            biome_weights: [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            shore: 0.8,
            snow: 0.9,
            known: true,
            ..Default::default()
        };
        // Authored world grass uses its own top, including at transition bands.
        assert_eq!(
            albedo(&forest, &palette, MapLayer::Surface),
            palette.surface_grass.unwrap()
        );
        let blend = MapSample {
            biome_weights: [0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0],
            ..forest
        };
        for (actual, expected) in albedo(&blend, &palette, MapLayer::Surface)
            .into_iter()
            .zip([0.448, 0.506, 0.06])
        {
            assert!((actual - expected).abs() < 1e-6);
        }
    }

    #[test]
    fn surface_grass_palette_does_not_brighten_caves_or_other_materials() {
        let authored = authored_palette();
        let neutral = MapPalette {
            authored_grass: false,
            surface_grass: None,
            ..authored.clone()
        };
        for material in [1, 3, 4, 5, 7, 14, 27] {
            let sample = MapSample {
                material,
                shore: 0.6,
                snow: 0.2,
                known: true,
                ..Default::default()
            };
            let cave = MapLayer::Cave { level: -32 };
            assert_eq!(
                albedo(&sample, &authored, cave),
                albedo(&sample, &neutral, cave)
            );
            if material != 1 {
                assert_eq!(
                    albedo(&sample, &authored, MapLayer::Surface),
                    albedo(&sample, &neutral, MapLayer::Surface)
                );
            }
        }
        let grass = MapSample {
            material: 1,
            ..Default::default()
        };
        for (actual, expected) in albedo(&grass, &authored, MapLayer::Cave { level: -32 })
            .into_iter()
            .zip([0.15 * 0.28, 0.09 * 0.40, 0.03 * 0.08])
        {
            assert!((actual - expected).abs() < 1e-7);
        }
    }

    #[test]
    fn baked_tiles_and_live_fallbacks_respect_the_surface_and_cave_palette() {
        let palette = authored_palette();
        let mut colors = Vec::new();
        for layer in [MapLayer::Surface, MapLayer::Cave { level: -32 }] {
            let mut detail = DetailTile {
                coordinate: [0, 0],
                layer,
                revision: 0,
                cells: [MapSample::default(); 256],
            };
            detail.cells[0] = MapSample {
                material: 1,
                known: true,
                ..Default::default()
            };
            let baked = tile(&detail, &palette);
            let entries = HashMap::from([(
                ([0, 0], layer),
                Entry {
                    tile: None,
                    detail: Some(std::sync::Arc::new(detail)),
                    overview: None,
                    source_revision: None,
                    palette_fingerprint: palette.fingerprint(),
                    used: 0,
                },
            )]);
            let live = sample(&entries, layer, [0, 0], &palette, -32);
            assert_eq!(&baked.pixels[..4], &live.color);
            colors.push(live.color);
        }
        assert!(colors[0][1] > colors[1][1] * 2);
    }

    #[test]
    fn rotation_projection_relief_and_circle_are_consistent() {
        let p = Projection::new([-8, -8], 192, 0.0, false, true);
        assert_eq!(p.world(160.0, 160.0), [-8, -8]);
        let a = p.point([-8.0, 0.0, -28.0], 0);
        assert!(a[1] < 160);
        let r = Projection::new([-8, -8], 192, std::f32::consts::FRAC_PI_2, true, true);
        let b = r.point([-28.0, 0.0, -8.0], 0);
        assert!(b[1] < 160);
        assert!(p.point([-8.0, 10.0, -8.0], 0)[1] < 160);
        let image = render(
            &HashMap::new(),
            MapLayer::Surface,
            &MapPalette::default(),
            &p,
            0,
            0.0,
            &[],
        );
        assert_eq!(&image[..4], &[0; 4]);
        assert_eq!(image.len(), SIZE * SIZE * 4);
    }
    #[test]
    fn markers_do_not_cross_solid_roofs_but_can_show_below_canopies() {
        use terra_voxel::{BlockSection, SectionPos};
        use terra_world::streaming::{ChunkStore, LoadedSection};
        let layer = MapLayer::Surface;
        let mut detail = DetailTile {
            coordinate: [0, 0],
            layer,
            revision: 0,
            cells: [MapSample::default(); 256],
        };
        detail.cells[0] = MapSample {
            known: true,
            height: 8,
            material: 7,
            ..Default::default()
        };
        let mut entries = HashMap::new();
        entries.insert(
            ([0, 0], layer),
            Entry {
                tile: None,
                detail: Some(std::sync::Arc::new(detail)),
                overview: None,
                source_revision: None,
                palette_fingerprint: MapPalette::default().fingerprint(),
                used: 0,
            },
        );
        let position = SectionPos::new(0, 0, 0);
        let mut data = BlockSection::empty(position).unwrap();
        data.set_block([0, 5, 0], 3).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            position,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let marker = Marker {
            player: false,
            position: [0.5, 2.0, 0.5],
            kind: CreatureKind::Cow,
            variant: 0,
            hostile: false,
        };
        assert!(!marker_visible(&marker, &entries, layer, &chunks));
        chunks
            .loaded
            .get_mut(&position)
            .unwrap()
            .data
            .set_block([0, 5, 0], 7)
            .unwrap();
        assert!(marker_visible(&marker, &entries, layer, &chunks));
        chunks.loaded.clear();
        assert!(!marker_visible(&marker, &entries, layer, &chunks));
    }
}
