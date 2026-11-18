/// GDI renders RGB into a DIB. Supply the premultiplied alpha expected by DWM.
/// A 1/255 background keeps the clear area clickable and draggable.
pub fn prepare(
    pixels: &mut [u8],
    width: i32,
    background: u32,
    background_alpha: u8,
    cover: Option<[i32; 4]>,
) {
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        let x = index as i32 % width;
        let y = index as i32 / width;
        let in_cover = cover.is_some_and(|[left, top, right, bottom]| {
            x >= left && x < right && y >= top && y < bottom
        });
        let rgb = (pixel[2] as u32) << 16 | (pixel[1] as u32) << 8 | pixel[0] as u32;
        if background_alpha < 255 && !in_cover && rgb == background {
            for channel in &mut pixel[..3] {
                *channel = ((*channel as u16 * background_alpha as u16 + 127) / 255) as u8;
            }
            pixel[3] = background_alpha;
        } else {
            pixel[3] = 255;
        }
    }
}

/// Antialias the floating silhouette and add a subtle one-pixel glass rim.
pub fn frame(pixels: &mut [u8], width: i32, height: i32, radius: f32, stroke: u32) {
    let half_w = width as f32 / 2.;
    let half_h = height as f32 / 2.;
    let radius = radius.min(half_w).min(half_h);
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        let x = (index as i32 % width) as f32 + 0.5;
        let y = (index as i32 / width) as f32 + 0.5;
        if x > radius + 2. && x < width as f32 - radius - 2. && y > 2. && y < height as f32 - 2. { continue; }
        let qx = (x - half_w).abs() - (half_w - radius);
        let qy = (y - half_h).abs() - (half_h - radius);
        let distance = (qx.max(0.).powi(2) + qy.max(0.).powi(2)).sqrt() + qx.max(qy).min(0.) - radius;
        let coverage = (0.5 - distance).clamp(0., 1.);
        if coverage == 0. { pixel.fill(0); continue; }
        let rim = (distance + 1.5).clamp(0., 1.) * 0.28;
        for (i, channel) in pixel[..3].iter_mut().enumerate() {
            let c = ((stroke >> (i * 8)) & 255) as f32;
            *channel = ((*channel as f32 * (1. - rim) + c * rim) * coverage).round() as u8;
        }
        pixel[3] = ((pixel[3] as f32 + (255 - pixel[3]) as f32 * rim) * coverage).round() as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transparent_background_preserves_content_and_album_pixels() {
        let mut pixels = [
            0x33, 0x22, 0x11, 0, 0x33, 0x22, 0x11, 0, 0x88, 0x99, 0xaa, 0,
        ];
        prepare(&mut pixels, 3, 0x112233, 1, Some([1, 0, 2, 1]));
        assert_eq!(&pixels[..4], &[0, 0, 0, 1]);
        assert_eq!(&pixels[4..8], &[0x33, 0x22, 0x11, 255]);
        assert_eq!(&pixels[8..], &[0x88, 0x99, 0xaa, 255]);
        prepare(&mut pixels, 3, 0x112233, 255, None);
        assert_eq!(pixels[3], 255);
    }
    #[test]
    fn glass_tint_uses_premultiplied_alpha() {
        let mut pixels = [100, 150, 200, 0];
        prepare(&mut pixels, 1, 0xc89664, 160, None);
        assert_eq!(pixels, [63, 94, 125, 160]);
    }
    #[test]
    fn floating_frame_has_clear_corners_and_keeps_the_center() {
        let mut pixels = vec![100; 40 * 32 * 4];
        frame(&mut pixels, 40, 32, 12., 0xffffff);
        assert_eq!(&pixels[..4], &[0, 0, 0, 0]);
        assert_eq!(&pixels[(16 * 40 + 20) * 4..(16 * 40 + 20) * 4 + 4], &[100; 4]);
        assert!(pixels[(20 * 4) + 3] > 100);
    }
}
