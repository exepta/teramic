//! Reject clear-color dithering and partially missing model rows in native captures.
pub(super) fn has_six_heads(data: &[u8], width: usize, height: usize) -> bool {
    if width < 12
        || height < 8
        || width.checked_mul(height).and_then(|n| n.checked_mul(4)) != Some(data.len())
    {
        return false;
    }
    let background = &data[..3];
    let mut foreground = [0usize; 6];
    let mut sampled = [0usize; 6];
    for y in (0..height).step_by(4) {
        for x in (0..width).step_by(4) {
            let cell = (y * 2 / height) * 3 + x * 3 / width;
            sampled[cell] += 1;
            let p = (y * width + x) * 4;
            if data[p..p + 3]
                .iter()
                .zip(background)
                .any(|(a, b)| a.abs_diff(*b) > 24)
            {
                foreground[cell] += 1;
            }
        }
    }
    // Each head must fill at least 2% of its own tile. A single bright object
    // or 1-LSB tonemapping noise cannot validate the entire 3x2 gallery.
    foreground
        .into_iter()
        .zip(sampled)
        .all(|(visible, samples)| visible * 50 >= samples && visible > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dithered_clear_color_and_a_missing_head_are_rejected() {
        let (w, h) = (192, 108);
        let mut data = vec![0; w * h * 4];
        for (i, p) in data.chunks_exact_mut(4).enumerate() {
            p.copy_from_slice(&[18 + (i % 3) as u8, 24, 29, 255]);
        }
        assert!(!has_six_heads(&data, w, h));
        for row in 0..2 {
            for column in 0..3 {
                if row == 1 && column == 2 {
                    continue;
                }
                for y in row * 54 + 12..row * 54 + 40 {
                    for x in column * 64 + 16..column * 64 + 48 {
                        data[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&[150, 100, 70]);
                    }
                }
            }
        }
        assert!(!has_six_heads(&data, w, h));
        for y in 66..94 {
            for x in 144..176 {
                data[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&[150, 100, 70]);
            }
        }
        assert!(has_six_heads(&data, w, h));
        assert!(!has_six_heads(&data[..16], w, h));
    }
}
