use crate::{battery::Battery, media::Music, settings::Settings};
use std::sync::Arc;
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*};

pub fn color(rgb: u32) -> COLORREF {
    ((rgb & 0xff) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 0xff)
}
pub fn rgb(c: COLORREF) -> u32 {
    color(c)
}
pub fn blend(a: u32, b: u32, t: u32) -> u32 {
    let mut c = 0;
    for shift in [0, 8, 16] {
        let x = (a >> shift) & 255;
        let y = (b >> shift) & 255;
        c |= ((x * (255 - t) + y * t) / 255) << shift;
    }
    c
}
pub fn light(rgb: u32) -> bool {
    ((rgb >> 16) & 255) * 299 + ((rgb >> 8) & 255) * 587 + (rgb & 255) * 114 > 150_000
}
pub fn hit(x: i32, width: i32, player: bool) -> u32 {
    if x >= width - 30 {
        4
    } else if player && x >= width - 180 && x < width - 96 {
        if x < width - 155 {
            1
        } else if x < width - 124 {
            2
        } else {
            3
        }
    } else {
        0
    }
}
pub struct Cache {
    pub font: HFONT,
    pub small: HFONT,
    pub cover_key: Option<Arc<Vec<u8>>>,
    pub cover: Option<Vec<u8>>,
    pub scale: i32,
}
impl Cache {
    pub fn new() -> Self {
        Self {
            font: std::ptr::null_mut(),
            small: std::ptr::null_mut(),
            cover_key: None,
            cover: None,
            scale: 0,
        }
    }
    pub unsafe fn prepare(&mut self, dpi: i32, music: &Music) {
        unsafe {
            if self.scale != dpi {
                if !self.font.is_null() {
                    DeleteObject(self.font);
                }
                if !self.small.is_null() {
                    DeleteObject(self.small);
                }
                let face = crate::wide("Segoe UI");
                self.font = CreateFontW(
                    -12 * dpi / 96,
                    0,
                    0,
                    0,
                    600,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    0,
                    0,
                    CLEARTYPE_QUALITY as u32,
                    0,
                    face.as_ptr(),
                );
                self.small = CreateFontW(
                    -11 * dpi / 96,
                    0,
                    0,
                    0,
                    400,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    0,
                    0,
                    CLEARTYPE_QUALITY as u32,
                    0,
                    face.as_ptr(),
                );
                self.scale = dpi;
                self.cover_key = None;
                self.cover = None;
            }
            let same = match (&self.cover_key, &music.cover) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            };
            if !same {
                self.cover = None;
                if let Some(bytes) = &music.cover {
                    let decoded = image::ImageReader::new(std::io::Cursor::new(bytes.as_slice()))
                        .with_guessed_format()
                        .ok()
                        .and_then(|mut reader| {
                            let mut limits = image::Limits::default();
                            limits.max_image_width = Some(2048);
                            limits.max_image_height = Some(2048);
                            limits.max_alloc = Some(16 * 1024 * 1024);
                            reader.limits(limits);
                            reader.decode().ok()
                        });
                    if let Some(decoded) = decoded {
                        let side = (38 * dpi / 96).max(1) as u32;
                        let image = decoded
                            .resize_to_fill(side, side, image::imageops::FilterType::Triangle)
                            .to_rgba8();
                        let mut pixels = image.into_raw();
                        for p in pixels.chunks_exact_mut(4) {
                            p.swap(0, 2);
                        }
                        self.cover = Some(pixels);
                    }
                }
                self.cover_key = music.cover.clone();
            }
        }
    }
}
impl Drop for Cache {
    fn drop(&mut self) {
        unsafe {
            if !self.font.is_null() {
                DeleteObject(self.font);
            }
            if !self.small.is_null() {
                DeleteObject(self.small);
            }
        }
    }
}
unsafe fn fill(dc: HDC, r: RECT, c: u32) {
    unsafe {
        let b = CreateSolidBrush(color(c));
        FillRect(dc, &r, b);
        DeleteObject(b);
    }
}
unsafe fn text(dc: HDC, s: &str, mut r: RECT, font: HFONT, c: u32) {
    label(dc, s, &mut r, font, c, DT_LEFT);
}
unsafe fn centered(dc: HDC, s: &str, mut r: RECT, font: HFONT, c: u32) {
    label(dc, s, &mut r, font, c, DT_CENTER);
}
unsafe fn label(dc: HDC, s: &str, r: &mut RECT, font: HFONT, c: u32, alignment: u32) {
    unsafe {
        let previous = SelectObject(dc, font);
        SetTextColor(dc, color(c));
        SetBkMode(dc, TRANSPARENT as i32);
        let w = crate::wide(s);
        DrawTextW(
            dc,
            w.as_ptr(),
            w.len() as i32 - 1,
            r,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX | alignment,
        );
        SelectObject(dc, previous);
    }
}
pub unsafe fn paint(
    dc: HDC,
    width: i32,
    height: i32,
    dpi: i32,
    settings: &Settings,
    music: &Music,
    battery: &Battery,
    background: u32,
    hover: u32,
    cache: &mut Cache,
) {
    unsafe {
        cache.prepare(dpi, music);
        let scale = dpi as f64 / 96.;
        let p = |n: i32| (n as f64 * scale).round() as i32;
        let r = |l, t, w, h| RECT {
            left: p(l),
            top: p(t),
            right: p(l + w),
            bottom: p(t + h),
        };
        let w = (width as f64 / scale).round() as i32;
        let h = (height as f64 / scale).round() as i32;
        let center = h / 2;
        let bg = settings.background.unwrap_or(background);
        let foreground = if light(bg) { 0x202020 } else { 0xecf5fa };
        let muted = blend(bg, foreground, 165);
        let icon = settings.icon_color;
        fill(
            dc,
            RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            },
            bg,
        );
        let pen = CreatePen(PS_SOLID, p(1).max(1), color(icon));
        let old_pen = SelectObject(dc, pen);
        let empty_brush = GetStockObject(NULL_BRUSH);
        let old_brush = SelectObject(dc, empty_brush);
        if settings.player_visible(music.available) {
            let side = if settings.floating { 38 } else { 28 };
            let top = center - side / 2;
            if let Some(pixels) = &cache.cover {
                let cover_size = p(38);
                let mut info: BITMAPINFO = std::mem::zeroed();
                info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                info.bmiHeader.biWidth = cover_size;
                info.bmiHeader.biHeight = -cover_size;
                info.bmiHeader.biPlanes = 1;
                info.bmiHeader.biBitCount = 32;
                info.bmiHeader.biCompression = BI_RGB;
                StretchDIBits(
                    dc,
                    p(10),
                    p(top),
                    p(side),
                    p(side),
                    0,
                    0,
                    cover_size,
                    cover_size,
                    pixels.as_ptr().cast(),
                    &info,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                );
            } else {
                fill(dc, r(10, top, side, side), blend(bg, foreground, 16));
                centered(dc, "♫", r(10, top, side, side), cache.font, icon);
            }
            let title_x = if settings.floating { 58 } else { 50 };
            text(
                dc,
                &music.title,
                r(title_x, center - 16, w - 180 - title_x - 5, 17),
                cache.font,
                foreground,
            );
            text(
                dc,
                &music.artist,
                r(title_x, center + 1, w - 180 - title_x - 5, 15),
                cache.small,
                muted,
            );
            for action in 1..=3 {
                let x = w - 180
                    + match action {
                        1 => 0,
                        2 => 25,
                        _ => 56,
                    };
                let bw = if action == 2 { 31 } else { 25 };
                if hover == action || action == 2 {
                    let b = CreateSolidBrush(color(blend(
                        bg,
                        foreground,
                        if hover == action { 32 } else { 18 },
                    )));
                    SelectObject(dc, b);
                    SelectObject(dc, GetStockObject(NULL_PEN));
                    RoundRect(
                        dc,
                        p(x),
                        p(center - 16),
                        p(x + bw),
                        p(center + 16),
                        p(7),
                        p(7),
                    );
                    SelectObject(dc, empty_brush);
                    SelectObject(dc, pen);
                    DeleteObject(b);
                }
                let mid = x + bw / 2;
                let c = if music.available {
                    icon
                } else {
                    blend(bg, icon, 65)
                };
                let b = CreateSolidBrush(color(c));
                SelectObject(dc, b);
                SelectObject(dc, GetStockObject(NULL_PEN));
                if action == 2 && music.playing {
                    fill(dc, r(mid - 5, center - 6, 3, 12), c);
                    fill(dc, r(mid + 2, center - 6, 3, 12), c);
                } else {
                    let points = if action == 1 {
                        [
                            POINT {
                                x: p(mid + 4),
                                y: p(center - 5),
                            },
                            POINT {
                                x: p(mid + 4),
                                y: p(center + 5),
                            },
                            POINT {
                                x: p(mid - 3),
                                y: p(center),
                            },
                        ]
                    } else {
                        [
                            POINT {
                                x: p(mid - 3),
                                y: p(center - 5),
                            },
                            POINT {
                                x: p(mid - 3),
                                y: p(center + 5),
                            },
                            POINT {
                                x: p(mid + 4),
                                y: p(center),
                            },
                        ]
                    };
                    Polygon(dc, points.as_ptr(), 3);
                    if action != 2 {
                        fill(
                            dc,
                            r(
                                if action == 1 { mid - 5 } else { mid + 5 },
                                center - 5,
                                1,
                                10,
                            ),
                            c,
                        );
                    }
                }
                SelectObject(dc, empty_brush);
                SelectObject(dc, pen);
                DeleteObject(b);
            }
            fill(dc, r(w - 96, center - 14, 1, 28), blend(bg, foreground, 38));
            fill(dc, r(13, h - 2, w - 26, 2), blend(bg, foreground, 25));
            fill(
                dc,
                r(13, h - 2, ((w - 26) as f64 * music.progress) as i32, 2),
                settings.progress_color,
            );
        }
        let mouse_x = w - 67;
        let mouse_y = center - 17;
        RoundRect(
            dc,
            p(mouse_x - 6),
            p(mouse_y),
            p(mouse_x + 6),
            p(mouse_y + 17),
            p(10),
            p(10),
        );
        MoveToEx(dc, p(mouse_x), p(mouse_y), std::ptr::null_mut());
        LineTo(dc, p(mouse_x), p(mouse_y + 8));
        MoveToEx(dc, p(mouse_x - 6), p(mouse_y + 8), std::ptr::null_mut());
        LineTo(dc, p(mouse_x + 6), p(mouse_y + 8));
        SelectObject(dc, old_brush);
        let label = battery
            .percent
            .map(|v| format!("{v}%"))
            .unwrap_or_else(|| "—".into());
        let battery_color = if battery.percent.is_some_and(|p| p <= 20) || battery.stale {
            0xffb67b
        } else {
            foreground
        };
        centered(
            dc,
            &label,
            r(mouse_x - 24, center + 2, 48, 16),
            cache.small,
            battery_color,
        );
        if battery.charging {
            centered(
                dc,
                "⚡",
                r(mouse_x + 19, center + 2, 12, 16),
                cache.small,
                icon,
            );
        }
        if hover == 4 {
            fill(
                dc,
                r(w - 30, center - 16, 25, 32),
                blend(bg, foreground, 28),
            );
        }
        MoveToEx(dc, p(w - 21), p(center - 2), std::ptr::null_mut());
        LineTo(dc, p(w - 17), p(center + 2));
        LineTo(dc, p(w - 13), p(center - 2));
        SelectObject(dc, old_pen);
        DeleteObject(pen);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_hit_targets() {
        assert_eq!(hit(400, 420, true), 4);
        assert_eq!(hit(280, 420, true), 2);
        assert_eq!(hit(80, 100, false), 4);
        assert_eq!(hit(20, 100, false), 0);
    }
    #[test]
    fn colors_round_trip() {
        assert_eq!(rgb(color(0x85e4cf)), 0x85e4cf);
    }
}
