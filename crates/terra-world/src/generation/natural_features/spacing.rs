//! Ecology eleven's sparse, jittered landmark sites. Pure integer coordinates
//! make the result independent of streaming order and cache contents.
use super::super::{Tile, ticket};

const REGION_TILES: i64 = 16;
const INSET_TILES: i64 = 5;
const JITTER_TILES: u32 = 6;

/// At most one candidate per 512 m square. The inset leaves at least 349 m
/// between final landmark centers, including their existing 3 m position jitter;
/// even the widest 26 m silhouettes retain more than 320 m of open ground.
pub(super) fn candidate(seed: u64, region: Tile) -> Tile {
    let hash = ticket(seed, region, 1911);
    [
        region[0] * REGION_TILES + INSET_TILES + i64::from(hash % JITTER_TILES),
        region[1] * REGION_TILES + INSET_TILES + i64::from((hash >> 12) % JITTER_TILES),
    ]
}

pub(super) fn eligible(seed: u64, tile: Tile) -> bool {
    let region = tile.map(|coordinate| coordinate.div_euclid(REGION_TILES));
    tile == candidate(seed, region)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn center(seed: u64, tile: Tile) -> Tile {
        let hash = ticket(seed, tile, 1901);
        [
            tile[0] * 32 + 14 + i64::from(hash % 4),
            tile[1] * 32 + 14 + i64::from((hash >> 5) % 4),
        ]
    }

    #[test]
    fn landmark_candidates_are_rare_in_measured_regions_and_stable_in_reverse_order() {
        let tiles: Vec<_> = (-128..128)
            .flat_map(|z| (-128..128).map(move |x| [x, z]))
            .collect();
        let selected: Vec<_> = tiles
            .iter()
            .copied()
            .filter(|&p| eligible(42042, p))
            .collect();
        assert_eq!(selected.len(), 256);
        let per_square_kilometre = selected.len() as f64 / (tiles.len() as f64 * 1024.0 / 1e6);
        assert!((3.7..3.9).contains(&per_square_kilometre));
        assert!(
            selected.len() * 200 < tiles.len(),
            "landmarks must be discoveries, not a tile carpet"
        );
        let reversed: Vec<_> = tiles
            .iter()
            .rev()
            .copied()
            .filter(|&p| eligible(42042, p))
            .collect();
        assert!(selected.iter().copied().eq(reversed.into_iter().rev()));
        let other_seed: Vec<_> = tiles
            .iter()
            .copied()
            .filter(|&p| eligible(42043, p))
            .collect();
        assert_eq!(selected.len(), other_seed.len());
        assert_ne!(selected, other_seed);
    }

    #[test]
    fn neighboring_negative_and_positive_regions_keep_three_hundred_meters_of_open_ground() {
        for seed in [0, 1, 42042, u64::MAX] {
            let points: Vec<_> = (-8..8)
                .flat_map(|z| (-8..8).map(move |x| [x, z]))
                .map(|region| {
                    let tile = candidate(seed, region);
                    assert_eq!(tile.map(|v| v.div_euclid(REGION_TILES)), region);
                    assert!(eligible(seed, tile));
                    center(seed, tile)
                })
                .collect();
            for (index, a) in points.iter().enumerate() {
                for b in &points[index + 1..] {
                    let distance_squared = (a[0] - b[0]).pow(2) + (a[1] - b[1]).pow(2);
                    assert!(
                        distance_squared >= 349_i64.pow(2),
                        "close landmarks: {a:?}/{b:?}"
                    );
                    assert!((distance_squared as f64).sqrt() - 26.0 > 320.0);
                }
            }
        }
    }
}
