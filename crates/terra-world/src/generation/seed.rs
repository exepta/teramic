pub(crate) fn layer_seed(seed: u64, layer: u64) -> u32 {
    let mut x = seed.wrapping_add(layer.wrapping_mul(0x9e3779b97f4a7c15));
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    (x ^ (x >> 31)) as u32
}
