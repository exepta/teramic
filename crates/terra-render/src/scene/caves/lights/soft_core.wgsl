// Reserved exact radius marker. Keep Bevy's physical radius and stock light
// attenuation unchanged for every source that did not explicitly opt in.
fn terra_physical_light_radius(encoded_radius: f32) -> f32 {
    return select(encoded_radius, 0.38, encoded_radius == 128.375);
}

fn terra_torch_attenuation(distance_squared: f32, inverse_range_squared: f32, encoded_radius: f32) -> f32 {
    if encoded_radius == 128.375 {
        // Finite 4 m soft core controls the close hotspot independently of the
        // useful 18 m spill. The usual 28 m smooth cutoff and shadows remain.
        let spill_gain = 1.0 + 0.2 * smoothstep(1.0, 4.0, sqrt(distance_squared));
        return spill_gain * getRangeFalloff(distance_squared, inverse_range_squared) / (distance_squared + 16.0);
    }
    return getDistanceAttenuation(distance_squared, inverse_range_squared);
}
