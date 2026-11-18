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
}
