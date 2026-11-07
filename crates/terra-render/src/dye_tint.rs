//! Picker RGB is sRGB; direct PBR vertex/material multipliers must be linear.
use terra_core::beds::DyeColor;

fn channel(srgb: f32) -> f32 {
    if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

pub(crate) fn rgb(rgb: [u8; 3]) -> [f32; 4] {
    let [r, g, b] = rgb.map(|value| channel(f32::from(value) / 255.0));
    [r, g, b, 1.0]
}

pub(crate) fn color(color: DyeColor) -> [f32; 4] {
    match color {
        DyeColor::Rgb(value) => rgb(value),
        _ => color.tint(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    #[test]
    fn picker_srgb_matches_neutral_pbr_linear_values_and_bevy_for_all_bytes() {
        assert_eq!(rgb([0; 3]), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(rgb([255; 3]), [1.0; 4]);
        let reference = [0.06301002, 0.10946171, 0.47932018, 1.0];
        for (actual, expected) in rgb([71, 93, 184]).into_iter().zip(reference) {
            assert!((actual - expected).abs() < 1e-7);
        }
        for value in 0..=255 {
            let expected = Color::srgb_u8(value, value, value)
                .to_linear()
                .to_f32_array();
            for (a, b) in rgb([value; 3]).into_iter().zip(expected) {
                assert!((a - b).abs() < 2e-7, "sRGB byte {value}: {a} != {b}");
            }
        }
        assert!((rgb([10; 3])[0] - 10.0 / 255.0 / 12.92).abs() < 1e-8);
        assert!((rgb([11; 3])[0] - ((11.0f32 / 255.0 + 0.055) / 1.055).powf(2.4)).abs() < 1e-8);
    }

    #[test]
    fn existing_named_palette_values_and_alpha_remain_bit_exact() {
        for dye in DyeColor::ALL {
            assert_eq!(color(dye), dye.tint());
        }
    }
}
