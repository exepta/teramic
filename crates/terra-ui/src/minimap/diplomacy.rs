//! Cached claim polygons over the existing map; never requests world chunks.
use super::{Entry, Key, raster::Projection};
use std::collections::{BTreeMap, HashMap};
use terra_core::kingdom::{ClaimKind, KingdomRegistry, SettlementDomain};
use terra_world::cartography::MapLayer;

pub(super) struct RasterSource<'a> {
    pub registry: &'a KingdomRegistry,
    pub selected: Option<[i64; 2]>,
    pub domains: &'a [SettlementDomain],
    pub draft: &'a BTreeMap<[i64; 2], ClaimKind>,
}

#[derive(Default)]
pub(super) struct ClaimRasterCache {
    key: Option<CacheKey>,
    pixels: Vec<u8>,
}

#[derive(PartialEq)]
struct CacheKey {
    registry: u64,
    terrain: u64,
    selected: Option<[i64; 2]>,
    draft: Vec<([i64; 2], ClaimKind)>,
    domains: Vec<SettlementDomain>,
    projection: [[u64; 2]; 3],
    size: usize,
    layer: MapLayer,
    level: i64,
    relief: bool,
    three_dimensional: bool,
    aspect: u32,
}

impl ClaimRasterCache {
    pub(super) fn pixels(
        &mut self,
        source: RasterSource<'_>,
        projection: &Projection,
        entries: &HashMap<Key, Entry>,
        layer: MapLayer,
        level: i64,
        terrain_revision: u64,
    ) -> &[u8] {
        let size = projection.size();
        let key = CacheKey {
            registry: source.registry.revision,
            domains: source.domains.to_vec(),
            terrain: terrain_revision,
            selected: source.selected,
            draft: source.draft.iter().map(|(p, k)| (*p, *k)).collect(),
            projection: [[0.0, 0.0], [size as f32, 0.0], [0.0, size as f32]]
                .map(|p| projection.world_at(p).map(f64::to_bits)),
            size,
            layer,
            level,
            relief: projection.relief_enabled(),
            three_dimensional: projection.is_three_dimensional(),
            aspect: projection.aspect().to_bits(),
        };
        if self.key.as_ref() != Some(&key) {
            self.pixels = paint(&source, projection, entries, layer, level);
            self.key = Some(key);
        }
        &self.pixels
    }
}

fn height(entries: &HashMap<Key, Entry>, layer: MapLayer, x: i64, z: i64, level: i64) -> f32 {
    entries
        .get(&([x.div_euclid(16), z.div_euclid(16)], layer))
        .and_then(|entry| entry.height_at((z.rem_euclid(16) * 16 + x.rem_euclid(16)) as usize))
        .map_or(level as f32, f32::from)
}

fn paint(
    source: &RasterSource<'_>,
    projection: &Projection,
    entries: &HashMap<Key, Entry>,
    layer: MapLayer,
    level: i64,
) -> Vec<u8> {
    let size = projection.size();
    let mut pixels = vec![0; size * size * 4];
    // Generated realm territory is the base layer. Explicit claims from every
    // player realm are drawn above it and remain authoritative locally.
    for domain in source.domains.iter().filter(|d| d.valid()) {
        // Direct claims are drawn afterwards, so resolve the inherited color
        // once per settlement rather than scanning the registry per chunk.
        let Some(style) = source.registry.domain_heraldry(domain, domain.center) else {
            continue;
        };
        let [r, g, b] = style.color;
        let visible = visible_chunks(
            projection,
            level,
            [
                domain.bounds[0][1] as f32 - 128.0,
                domain.bounds[1][1] as f32 + 128.0,
            ],
        );
        let low = terra_core::kingdom::claim_chunk(domain.bounds[0]);
        let high = terra_core::kingdom::claim_chunk(domain.bounds[1]);
        for z in low[1].max(visible[0][1])..=high[1].min(visible[1][1]) {
            for x in low[0].max(visible[0][0])..=high[0].min(visible[1][0]) {
                fill_quad(
                    &mut pixels,
                    size,
                    chunk_polygon([x, z], projection, entries, layer, level),
                    [r, g, b, 88],
                );
            }
        }
    }
    // North-to-south order matches the existing orthographic relief projection.
    let mut claims: Vec<_> = source.registry.claims.iter().collect();
    claims.sort_unstable_by_key(|claim| (claim.chunk[1], claim.chunk[0]));
    for claim in claims {
        let Some(kingdom) = source.registry.get(claim.kingdom) else {
            continue;
        };
        let polygon = chunk_polygon(claim.chunk, projection, entries, layer, level);
        let [r, g, b] = kingdom.heraldry.color;
        let protected = claim.kind == ClaimKind::ProtectedBuilding;
        let color = if protected {
            [r, g, b, 112]
        } else {
            [
                r.saturating_add((255 - r) / 3),
                g.saturating_add((255 - g) / 3),
                b.saturating_add((255 - b) / 3),
                57,
            ]
        };
        fill_quad(&mut pixels, size, polygon, color);
    }
    // Empty selected columns need a visible cursor too.
    if let Some(chunk) = source.selected.filter(|p| {
        source.registry.claim_at(*p).is_none()
            && !source.domains.iter().any(|domain| {
                domain.contains_chunk(*p)
                    && source
                        .registry
                        .domain_heraldry(domain, domain.center)
                        .is_some()
            })
    }) {
        let polygon = chunk_polygon(chunk, projection, entries, layer, level);
        fill_quad(&mut pixels, size, polygon, [230, 205, 146, 22]);
    }
    // The uncommitted selection remains visibly distinct from saved ownership.
    for (&chunk, &kind) in source.draft {
        let polygon = chunk_polygon(chunk, projection, entries, layer, level);
        let color = match kind {
            ClaimKind::ProtectedBuilding => [93, 180, 125, 130],
            ClaimKind::RealmTerritory => [100, 164, 220, 100],
        };
        fill_quad(&mut pixels, size, polygon, color);
    }
    pixels
}

fn visible_chunks(projection: &Projection, level: i64, heights: [f32; 2]) -> [[i64; 2]; 2] {
    let n = projection.size() as f32;
    let mut bounds = [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]];
    for height in heights {
        for p in [[0.0, 0.0], [n, 0.0], [0.0, n], [n, n]] {
            let world = projection.world_at_height(p, height, level);
            for (axis, v) in world.into_iter().enumerate() {
                bounds[0][axis] = bounds[0][axis].min(v);
                bounds[1][axis] = bounds[1][axis].max(v);
            }
        }
    }
    [
        bounds[0].map(|v| (v.floor() as i64).div_euclid(16) - 1),
        bounds[1].map(|v| (v.ceil() as i64).div_euclid(16) + 1),
    ]
}

fn chunk_polygon(
    chunk: [i64; 2],
    projection: &Projection,
    entries: &HashMap<Key, Entry>,
    layer: MapLayer,
    level: i64,
) -> [[f32; 2]; 4] {
    let [x, z] = chunk.map(|v| v * 16);
    [[x, z], [x + 16, z], [x + 16, z + 16], [x, z + 16]].map(|[x, z]| {
        projection.project(
            [x as f32, height(entries, layer, x, z, level), z as f32],
            level,
        )
    })
}

fn fill_quad(pixels: &mut [u8], size: usize, polygon: [[f32; 2]; 4], color: [u8; 4]) {
    let min = std::array::from_fn::<_, 2, _>(|axis| {
        polygon
            .iter()
            .map(|p| p[axis])
            .fold(f32::INFINITY, f32::min)
    });
    let max = std::array::from_fn::<_, 2, _>(|axis| {
        polygon
            .iter()
            .map(|p| p[axis])
            .fold(f32::NEG_INFINITY, f32::max)
    });
    if max[0] < 0.0 || max[1] < 0.0 || min[0] >= size as f32 || min[1] >= size as f32 {
        return;
    }
    for y in min[1].floor().max(0.0) as usize..=(max[1].ceil().min(size as f32 - 1.0) as usize) {
        for x in min[0].floor().max(0.0) as usize..=(max[0].ceil().min(size as f32 - 1.0) as usize)
        {
            let point = [x as f32 + 0.5, y as f32 + 0.5];
            if !inside_triangle(point, polygon[0], polygon[1], polygon[2])
                && !inside_triangle(point, polygon[0], polygon[2], polygon[3])
            {
                continue;
            }
            pixels[(y * size + x) * 4..(y * size + x) * 4 + 4].copy_from_slice(&color);
        }
    }
}

fn inside_triangle(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let cross = |a: [f32; 2], b: [f32; 2], p: [f32; 2]| {
        (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
    };
    let d = [cross(a, b, p), cross(b, c, p), cross(c, a, p)];
    d.iter().all(|v| *v >= -0.001) || d.iter().all(|v| *v <= 0.001)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn polygons_keep_distinct_transparency_and_clip_at_all_viewport_edges() {
        let mut pixels = vec![0; 32 * 32 * 4];
        fill_quad(
            &mut pixels,
            32,
            [[-8.0, -8.0], [16.0, -8.0], [16.0, 16.0], [-8.0, 16.0]],
            [40, 80, 150, 112],
        );
        assert_eq!(
            &pixels[(8 * 32 + 8) * 4..(8 * 32 + 8) * 4 + 4],
            &[40, 80, 150, 112]
        );
        assert_eq!(pixels[(24 * 32 + 24) * 4 + 3], 0);
        fill_quad(
            &mut pixels,
            32,
            [[16.0, 16.0], [40.0, 16.0], [40.0, 40.0], [16.0, 40.0]],
            [110, 160, 200, 57],
        );
        assert_eq!(pixels[(24 * 32 + 24) * 4 + 3], 57);
        assert_eq!(
            &pixels[(17 * 32 + 17) * 4..(17 * 32 + 17) * 4 + 4],
            &[110, 160, 200, 57]
        );
    }

    #[test]
    fn actual_claims_and_live_color_changes_render_without_loading_any_world_chunk() {
        use terra_core::{
            PlayerUuid,
            kingdom::{ClaimActor, KingdomHeraldry, KingdomIcon},
        };
        let owner = PlayerUuid::from_u128(56);
        let mut registry = KingdomRegistry::default();
        let id = registry.found(owner, [0, 0, 0], "Oakwatch").unwrap();
        registry
            .claim(
                id,
                [1, 0],
                ClaimKind::RealmTerritory,
                ClaimActor::Player(owner),
            )
            .unwrap();
        registry
            .change_appearance(
                owner,
                id,
                KingdomHeraldry {
                    color: [30, 80, 150],
                    icon: KingdomIcon::Tree,
                },
            )
            .unwrap();
        let projection = Projection::at([16.0, 8.0], 64, 0.0, false, false, 64, false);
        let entries = HashMap::new();
        let mut cache = ClaimRasterCache::default();
        let sample = |bytes: &[u8], x: usize, y: usize| {
            <[u8; 4]>::try_from(&bytes[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4]).unwrap()
        };
        let pixels = cache.pixels(
            RasterSource {
                domains: &[],
                draft: &BTreeMap::new(),
                registry: &registry,
                selected: None,
            },
            &projection,
            &entries,
            MapLayer::Surface,
            0,
            1,
        );
        assert_eq!(sample(pixels, 24, 32), [30, 80, 150, 112]);
        assert_eq!(sample(pixels, 40, 32)[3], 57);
        assert_eq!(sample(pixels, 4, 4)[3], 0);
        registry
            .change_appearance(
                owner,
                id,
                KingdomHeraldry {
                    color: [190, 35, 60],
                    icon: KingdomIcon::Star,
                },
            )
            .unwrap();
        let pixels = cache.pixels(
            RasterSource {
                domains: &[],
                draft: &BTreeMap::new(),
                registry: &registry,
                selected: None,
            },
            &projection,
            &entries,
            MapLayer::Surface,
            0,
            1,
        );
        assert_eq!(sample(pixels, 24, 32), [190, 35, 60, 112]);
        registry.dissolve(owner, id).unwrap();
        assert!(
            cache
                .pixels(
                    RasterSource {
                        domains: &[],
                        draft: &BTreeMap::new(),
                        registry: &registry,
                        selected: None
                    },
                    &projection,
                    &entries,
                    MapLayer::Surface,
                    0,
                    1
                )
                .iter()
                .all(|v| *v == 0)
        );
        assert!(entries.is_empty());
    }
    #[test]
    fn npc_territory_and_all_player_claims_keep_live_realm_colors_without_borders() {
        use terra_core::{
            PlayerUuid,
            kingdom::{BotKingdom, DomainKind, KingdomHeraldry, KingdomIcon},
        };
        let mut registry = KingdomRegistry::default();
        let owner = PlayerUuid::from_u128(1);
        let other = PlayerUuid::from_u128(2);
        let a = registry.found(owner, [0, 20, 0], "Northwatch").unwrap();
        let b = registry.found(other, [32, 20, 0], "Eastbank").unwrap();
        let blue = KingdomHeraldry {
            color: [25, 90, 210],
            icon: KingdomIcon::Anchor,
        };
        let orange = KingdomHeraldry {
            color: [210, 90, 25],
            icon: KingdomIcon::Crown,
        };
        registry.change_appearance(owner, a, blue).unwrap();
        registry.change_appearance(other, b, orange).unwrap();
        let domain = SettlementDomain {
            id: 12,
            center: [-24, 20, 8],
            bounds: [[-32, 10, 0], [-1, 40, 31]],
            kind: DomainKind::Village,
            native: Some(BotKingdom::Petora),
        };
        let projection = Projection::at([8.0, 8.0], 128, 0.0, false, false, 128, false);
        let entries = HashMap::new();
        let draft = BTreeMap::new();
        let mut cache = ClaimRasterCache::default();
        let sample = |p: &[u8], x: usize, y: usize| {
            <[u8; 4]>::try_from(&p[(y * 128 + x) * 4..(y * 128 + x) * 4 + 4]).unwrap()
        };
        let p = cache.pixels(
            RasterSource {
                registry: &registry,
                domains: &[domain],
                selected: Some([-1, 0]),
                draft: &draft,
            },
            &projection,
            &entries,
            MapLayer::Surface,
            0,
            1,
        );
        let native = BotKingdom::Petora.heraldry().color;
        assert_eq!(sample(p, 32, 64), [native[0], native[1], native[2], 88]);
        assert_eq!(
            sample(p, 48, 64),
            [native[0], native[1], native[2], 88],
            "Selecting an NPC chunk must retain its kingdom color"
        );
        assert_eq!(sample(p, 64, 64), [25, 90, 210, 112]);
        assert_eq!(
            sample(p, 96, 64),
            [210, 90, 25, 112],
            "Other players' claims must be shown too"
        );
        assert_eq!(
            sample(p, 56, 56),
            [25, 90, 210, 112],
            "Chunk edges keep the fill color instead of white outlines"
        );
        let conqueror = PlayerUuid::from_u128(3);
        let c = registry
            .found(conqueror, domain.center, "Westhold")
            .unwrap();
        let captured = KingdomHeraldry {
            color: [30, 180, 95],
            icon: KingdomIcon::Tree,
        };
        registry.change_appearance(conqueror, c, captured).unwrap();
        let p = cache.pixels(
            RasterSource {
                registry: &registry,
                domains: &[domain],
                selected: None,
                draft: &draft,
            },
            &projection,
            &entries,
            MapLayer::Surface,
            0,
            1,
        );
        assert_eq!(
            sample(p, 48, 64),
            [30, 180, 95, 88],
            "Capturing the center recolors the remainder of the village"
        );
        registry.dissolve(conqueror, c).unwrap();
        let p = cache.pixels(
            RasterSource {
                registry: &registry,
                domains: &[domain],
                selected: None,
                draft: &draft,
            },
            &projection,
            &entries,
            MapLayer::Surface,
            0,
            1,
        );
        assert_eq!(sample(p, 48, 64), [native[0], native[1], native[2], 88]);
        assert_eq!(sample(p, 96, 64), [210, 90, 25, 112]);
        assert!(entries.is_empty());
    }
}
