//! Crisp procedural vector icons, brand emblem, and pixel-art player avatars via `egui::Painter`.

use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Stroke, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Grid,       // Instances / Library
    Layers,     // My Mods / Content
    Compass,    // Discover / Modrinth
    Sliders,    // Instance Settings
    Terminal,   // Console / Logs
    User,       // Accounts
    Play,       // Launch triangle
    Stop,       // Stop square
    Plus,       // Create / Add
    Search,     // Magnifying glass
    Close,      // X / Remove / Clear
    Check,      // Checkmark
    Folder,     // Folder
    ChevronLeft,
    ChevronRight,
    Download,   // Download arrow
    Refresh,    // Reload / Rescan
    Copy,       // Copy to clipboard
    External,   // External link
    Duplicate,  // Clone instance
    Alert,      // Warning / Error triangle or circle
    Trash,      // Delete bin
    Sparkle,    // Modrinth / Featured
    List,       // List view toggle
    Star,       // Pin / Favorite
}

/// Draws a crisp vector icon centered inside `rect` with `color`.
pub fn draw_icon(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height());
    let u = s * 0.5; // half-extent
    let stroke_w = (s * 0.11).clamp(1.25, 2.2);
    let st = Stroke::new(stroke_w, color);

    match icon {
        Icon::Grid => {
            let gap = (s * 0.10).max(1.5);
            let cell = (s * 0.34).max(3.0);
            let r = Rounding::same((cell * 0.28).max(1.0));
            for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let center = Pos2::new(
                    c.x + dx * (cell * 0.5 + gap * 0.5),
                    c.y + dy * (cell * 0.5 + gap * 0.5),
                );
                let cell_rect = Rect::from_center_size(center, Vec2::splat(cell));
                painter.rect_stroke(cell_rect, r, st);
            }
        }
        Icon::Layers => {
            // Stacked 3-layer diamond/cards icon
            let w = u * 0.82;
            let h = u * 0.32;
            let top_y = c.y - u * 0.28;
            let pts = [
                Pos2::new(c.x, top_y - h),
                Pos2::new(c.x + w, top_y),
                Pos2::new(c.x, top_y + h),
                Pos2::new(c.x - w, top_y),
            ];
            for i in 0..4 {
                painter.line_segment([pts[i], pts[(i + 1) % 4]], st);
            }
            for dy in [u * 0.34, u * 0.68] {
                let l = Pos2::new(c.x - w * 0.92, top_y + dy);
                let m = Pos2::new(c.x, top_y + h * 0.92 + dy);
                let r = Pos2::new(c.x + w * 0.92, top_y + dy);
                painter.line_segment([l, m], st);
                painter.line_segment([m, r], st);
            }
        }
        Icon::Compass => {
            let rad = u * 0.78;
            painter.circle_stroke(c, rad, st);
            let n = u * 0.42;
            let w = u * 0.18;
            let needle = vec![
                Pos2::new(c.x + n * 0.7, c.y - n * 0.7),
                Pos2::new(c.x + w * 0.7, c.y + w * 0.7),
                Pos2::new(c.x - n * 0.7, c.y + n * 0.7),
                Pos2::new(c.x - w * 0.7, c.y - w * 0.7),
            ];
            painter.add(egui::Shape::convex_polygon(needle, color, Stroke::NONE));
        }
        Icon::Sliders => {
            let x0 = c.x - u * 0.76;
            let x1 = c.x + u * 0.76;
            let rows = [
                (c.y - u * 0.50, c.x - u * 0.25),
                (c.y, c.x + u * 0.30),
                (c.y + u * 0.50, c.x - u * 0.10),
            ];
            let knob_r = (s * 0.12).max(2.0);
            for (y, kx) in rows {
                if kx - knob_r > x0 {
                    painter.line_segment([Pos2::new(x0, y), Pos2::new(kx - knob_r, y)], st);
                }
                if kx + knob_r < x1 {
                    painter.line_segment([Pos2::new(kx + knob_r, y), Pos2::new(x1, y)], st);
                }
                painter.circle_stroke(Pos2::new(kx, y), knob_r, st);
            }
        }
        Icon::Terminal => {
            let box_r = Rect::from_center_size(c, Vec2::new(s * 0.82, s * 0.68));
            painter.rect_stroke(box_r, Rounding::same((s * 0.14).max(2.0)), st);
            let p_x = box_r.left() + box_r.width() * 0.24;
            let p_y = box_r.center().y;
            let d = s * 0.12;
            painter.line_segment([Pos2::new(p_x, p_y - d), Pos2::new(p_x + d, p_y)], st);
            painter.line_segment([Pos2::new(p_x + d, p_y), Pos2::new(p_x, p_y + d)], st);
            painter.line_segment(
                [
                    Pos2::new(p_x + d * 1.9, p_y + d),
                    Pos2::new(box_r.right() - box_r.width() * 0.20, p_y + d),
                ],
                st,
            );
        }
        Icon::User => {
            let head_r = (s * 0.18).max(2.4);
            painter.circle_stroke(Pos2::new(c.x, c.y - u * 0.28), head_r, st);
            // Shoulders arc approximated cleanly
            let y0 = c.y + u * 0.64;
            let y1 = c.y + u * 0.22;
            let x_w = u * 0.64;
            let pts = [
                Pos2::new(c.x - x_w, y0),
                Pos2::new(c.x - x_w * 0.62, y1),
                Pos2::new(c.x, y1 - u * 0.08),
                Pos2::new(c.x + x_w * 0.62, y1),
                Pos2::new(c.x + x_w, y0),
            ];
            for w in pts.windows(2) {
                painter.line_segment([w[0], w[1]], st);
            }
        }
        Icon::Play => {
            let r = u * 0.62;
            let pts = vec![
                Pos2::new(c.x - r * 0.55, c.y - r * 0.85),
                Pos2::new(c.x + r * 0.95, c.y),
                Pos2::new(c.x - r * 0.55, c.y + r * 0.85),
            ];
            painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
        }
        Icon::Stop => {
            let sq = Rect::from_center_size(c, Vec2::splat(s * 0.48));
            painter.rect_filled(sq, Rounding::same((s * 0.08).max(1.5)), color);
        }
        Icon::Plus => {
            let r = u * 0.58;
            painter.line_segment([Pos2::new(c.x - r, c.y), Pos2::new(c.x + r, c.y)], st);
            painter.line_segment([Pos2::new(c.x, c.y - r), Pos2::new(c.x, c.y + r)], st);
        }
        Icon::Search => {
            let lens_c = Pos2::new(c.x - u * 0.16, c.y - u * 0.16);
            let lens_r = u * 0.46;
            painter.circle_stroke(lens_c, lens_r, st);
            let dir = Vec2::new(0.707, 0.707);
            painter.line_segment(
                [lens_c + dir * lens_r, lens_c + dir * (lens_r + u * 0.46)],
                Stroke::new(stroke_w * 1.15, color),
            );
        }
        Icon::Close => {
            let r = u * 0.48;
            painter.line_segment(
                [Pos2::new(c.x - r, c.y - r), Pos2::new(c.x + r, c.y + r)],
                st,
            );
            painter.line_segment(
                [Pos2::new(c.x + r, c.y - r), Pos2::new(c.x - r, c.y + r)],
                st,
            );
        }
        Icon::Check => {
            let p1 = Pos2::new(c.x - u * 0.52, c.y + u * 0.04);
            let p2 = Pos2::new(c.x - u * 0.12, c.y + u * 0.44);
            let p3 = Pos2::new(c.x + u * 0.58, c.y - u * 0.36);
            painter.line_segment([p1, p2], st);
            painter.line_segment([p2, p3], st);
        }
        Icon::Folder => {
            let w = s * 0.78;
            let h = s * 0.58;
            let left = c.x - w * 0.5;
            let top = c.y - h * 0.5;
            let body = Rect::from_min_size(Pos2::new(left, top + h * 0.18), Vec2::new(w, h * 0.82));
            painter.rect_stroke(body, Rounding::same((s * 0.10).max(1.5)), st);
            let tab = [
                Pos2::new(left + w * 0.06, top + h * 0.18),
                Pos2::new(left + w * 0.16, top),
                Pos2::new(left + w * 0.46, top),
                Pos2::new(left + w * 0.54, top + h * 0.18),
            ];
            for pair in tab.windows(2) {
                painter.line_segment([pair[0], pair[1]], st);
            }
        }
        Icon::ChevronLeft => {
            let r = u * 0.42;
            painter.line_segment(
                [Pos2::new(c.x + r * 0.45, c.y - r), Pos2::new(c.x - r * 0.55, c.y)],
                st,
            );
            painter.line_segment(
                [Pos2::new(c.x - r * 0.55, c.y), Pos2::new(c.x + r * 0.45, c.y + r)],
                st,
            );
        }
        Icon::ChevronRight => {
            let r = u * 0.42;
            painter.line_segment(
                [Pos2::new(c.x - r * 0.45, c.y - r), Pos2::new(c.x + r * 0.55, c.y)],
                st,
            );
            painter.line_segment(
                [Pos2::new(c.x + r * 0.55, c.y), Pos2::new(c.x - r * 0.45, c.y + r)],
                st,
            );
        }
        Icon::Download => {
            let top = c.y - u * 0.55;
            let tip = c.y + u * 0.25;
            painter.line_segment([Pos2::new(c.x, top), Pos2::new(c.x, tip)], st);
            let aw = u * 0.36;
            painter.line_segment([Pos2::new(c.x - aw, tip - aw), Pos2::new(c.x, tip)], st);
            painter.line_segment([Pos2::new(c.x + aw, tip - aw), Pos2::new(c.x, tip)], st);
            let tray_y = c.y + u * 0.58;
            let tw = u * 0.62;
            painter.line_segment(
                [Pos2::new(c.x - tw, tray_y - u * 0.16), Pos2::new(c.x - tw, tray_y)],
                st,
            );
            painter.line_segment(
                [Pos2::new(c.x - tw, tray_y), Pos2::new(c.x + tw, tray_y)],
                st,
            );
            painter.line_segment(
                [Pos2::new(c.x + tw, tray_y), Pos2::new(c.x + tw, tray_y - u * 0.16)],
                st,
            );
        }
        Icon::Refresh => {
            let rad = u * 0.58;
            let segments = 14;
            let start_ang = -0.35_f32;
            let end_ang = std::f32::consts::TAU - 1.15;
            let mut prev = Pos2::new(c.x + start_ang.cos() * rad, c.y + start_ang.sin() * rad);
            for i in 1..=segments {
                let t = i as f32 / segments as f32;
                let a = start_ang + (end_ang - start_ang) * t;
                let next = Pos2::new(c.x + a.cos() * rad, c.y + a.sin() * rad);
                painter.line_segment([prev, next], st);
                prev = next;
            }
            // Arrowhead at start_ang
            let tip = Pos2::new(c.x + start_ang.cos() * rad, c.y + start_ang.sin() * rad);
            painter.line_segment(
                [tip, Pos2::new(tip.x - u * 0.30, tip.y - u * 0.08)],
                st,
            );
            painter.line_segment(
                [tip, Pos2::new(tip.x + u * 0.04, tip.y - u * 0.32)],
                st,
            );
        }
        Icon::Copy => {
            let sz = s * 0.52;
            let r = Rounding::same((s * 0.09).max(1.5));
            let front = Rect::from_min_size(
                Pos2::new(c.x - u * 0.54, c.y - u * 0.24),
                Vec2::splat(sz),
            );
            painter.rect_stroke(front, r, st);
            let b_min = Pos2::new(c.x - u * 0.16, c.y - u * 0.58);
            let b_max = Pos2::new(b_min.x + sz, b_min.y + sz);
            painter.line_segment([b_min, Pos2::new(b_max.x, b_min.y)], st);
            painter.line_segment([Pos2::new(b_max.x, b_min.y), b_max], st);
            painter.line_segment([Pos2::new(b_max.x - sz * 0.22, b_max.y), b_max], st);
        }
        Icon::External => {
            let box_r = Rect::from_min_size(
                Pos2::new(c.x - u * 0.58, c.y - u * 0.30),
                Vec2::splat(s * 0.54),
            );
            painter.rect_stroke(box_r, Rounding::same((s * 0.08).max(1.5)), st);
            let tip = Pos2::new(c.x + u * 0.58, c.y - u * 0.58);
            painter.line_segment([Pos2::new(c.x - u * 0.04, c.y + u * 0.04), tip], st);
            painter.line_segment([Pos2::new(tip.x - u * 0.38, tip.y), tip], st);
            painter.line_segment([tip, Pos2::new(tip.x, tip.y + u * 0.38)], st);
        }
        Icon::Duplicate => {
            let sz = s * 0.52;
            let r = Rounding::same((s * 0.09).max(1.5));
            let back = Rect::from_min_size(
                Pos2::new(c.x - u * 0.56, c.y - u * 0.56),
                Vec2::splat(sz),
            );
            painter.rect_stroke(back, r, st);
            let front = Rect::from_min_size(
                Pos2::new(c.x - u * 0.08, c.y - u * 0.08),
                Vec2::splat(sz),
            );
            painter.rect_stroke(front, r, st);
        }
        Icon::Alert => {
            let top = Pos2::new(c.x, c.y - u * 0.64);
            let left = Pos2::new(c.x - u * 0.68, c.y + u * 0.56);
            let right = Pos2::new(c.x + u * 0.68, c.y + u * 0.56);
            painter.line_segment([top, right], st);
            painter.line_segment([right, left], st);
            painter.line_segment([left, top], st);
            painter.line_segment(
                [Pos2::new(c.x, c.y - u * 0.22), Pos2::new(c.x, c.y + u * 0.16)],
                st,
            );
            painter.circle_filled(Pos2::new(c.x, c.y + u * 0.36), (stroke_w * 0.65).max(1.0), color);
        }
        Icon::Trash => {
            let top_y = c.y - u * 0.42;
            let bot_y = c.y + u * 0.62;
            let w = u * 0.52;
            // Lid
            painter.line_segment(
                [Pos2::new(c.x - w * 1.2, top_y), Pos2::new(c.x + w * 1.2, top_y)],
                st,
            );
            painter.line_segment(
                [
                    Pos2::new(c.x - w * 0.4, top_y - u * 0.20),
                    Pos2::new(c.x + w * 0.4, top_y - u * 0.20),
                ],
                st,
            );
            // Can body
            let body = Rect::from_min_max(
                Pos2::new(c.x - w * 0.9, top_y),
                Pos2::new(c.x + w * 0.9, bot_y),
            );
            painter.rect_stroke(body, Rounding::same((s * 0.08).max(1.5)), st);
            painter.line_segment(
                [
                    Pos2::new(c.x - w * 0.32, top_y + u * 0.24),
                    Pos2::new(c.x - w * 0.32, bot_y - u * 0.18),
                ],
                st,
            );
            painter.line_segment(
                [
                    Pos2::new(c.x + w * 0.32, top_y + u * 0.24),
                    Pos2::new(c.x + w * 0.32, bot_y - u * 0.18),
                ],
                st,
            );
        }
        Icon::Sparkle => {
            let r = u * 0.72;
            let inner = u * 0.22;
            let pts = [
                Pos2::new(c.x, c.y - r),
                Pos2::new(c.x + inner, c.y - inner),
                Pos2::new(c.x + r, c.y),
                Pos2::new(c.x + inner, c.y + inner),
                Pos2::new(c.x, c.y + r),
                Pos2::new(c.x - inner, c.y + inner),
                Pos2::new(c.x - r, c.y),
                Pos2::new(c.x - inner, c.y - inner),
            ];
            for i in 0..8 {
                painter.line_segment([pts[i], pts[(i + 1) % 8]], st);
            }
        }
        Icon::List => {
            for dy in [-u * 0.42, 0.0, u * 0.42] {
                let y = c.y + dy;
                painter.circle_filled(
                    Pos2::new(c.x - u * 0.56, y),
                    (stroke_w * 0.72).max(1.1),
                    color,
                );
                painter.line_segment(
                    [Pos2::new(c.x - u * 0.24, y), Pos2::new(c.x + u * 0.64, y)],
                    st,
                );
            }
        }
        Icon::Star => {
            let outer = u * 0.72;
            let inner = u * 0.32;
            let mut pts = Vec::with_capacity(10);
            for i in 0..10 {
                let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * (std::f32::consts::PI / 5.0);
                let rad = if i % 2 == 0 { outer } else { inner };
                pts.push(Pos2::new(c.x + angle.cos() * rad, c.y + angle.sin() * rad));
            }
            for i in 0..10 {
                painter.line_segment([pts[i], pts[(i + 1) % 10]], st);
            }
        }
    }
}

/// Draws the Sunset Launcher vector emblem fallback: twilight squircle,
/// glowing sunset sun disc, and dark isometric Minecraft cube with amber rim-lighting.
pub fn draw_brand_logo(painter: &egui::Painter, rect: Rect) {
    let s = rect.width().min(rect.height());
    let rounding = Rounding::same((s * 0.26).clamp(6.0, 12.0));
    let sun_coral = Color32::from_rgb(245, 114, 62);
    let sun_gold = Color32::from_rgb(255, 198, 92);

    painter.rect_filled(
        rect.expand(2.0),
        Rounding::same((s * 0.30).clamp(7.0, 14.0)),
        Color32::from_rgba_unmultiplied(sun_coral.r(), sun_coral.g(), sun_coral.b(), 48),
    );
    painter.rect(
        rect,
        rounding,
        Color32::from_rgb(30, 21, 46),
        Stroke::new(1.25_f32, Color32::from_rgba_unmultiplied(sun_gold.r(), sun_gold.g(), sun_gold.b(), 190)),
    );

    let c = Pos2::new(rect.center().x, rect.center().y + s * 0.03);
    painter.circle_filled(Pos2::new(c.x, c.y - s * 0.07), s * 0.27, sun_gold);
    painter.circle_filled(Pos2::new(c.x, c.y - s * 0.04), s * 0.21, sun_coral);

    let r = s * 0.28;
    let dx = r * 0.866;
    let dy = r * 0.50;
    let top = Pos2::new(c.x, c.y - r);
    let tr = Pos2::new(c.x + dx, c.y - dy);
    let br = Pos2::new(c.x + dx, c.y + dy);
    let bot = Pos2::new(c.x, c.y + r);
    let bl = Pos2::new(c.x - dx, c.y + dy);
    let tl = Pos2::new(c.x - dx, c.y - dy);

    painter.add(egui::Shape::convex_polygon(
        vec![top, tr, c, tl],
        Color32::from_rgb(44, 34, 58),
        Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![tl, c, bot, bl],
        Color32::from_rgb(31, 24, 44),
        Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![c, tr, br, bot],
        Color32::from_rgb(20, 15, 31),
        Stroke::NONE,
    ));

    let rim = Stroke::new((s * 0.045).clamp(1.0, 1.8), sun_gold);
    painter.line_segment([tl, top], rim);
    painter.line_segment([top, tr], rim);
    painter.line_segment([tl, c], rim);
    painter.line_segment([tr, c], rim);
}

/// Decodes `logo.png` into a 64x64 RGBA `egui::IconData` for the OS window/taskbar.
pub fn build_window_icon() -> egui::IconData {
    const SIZE: u32 = 64;
    if let Ok(img) = image::load_from_memory(include_bytes!("../logo.png")) {
        let resized = img
            .resize_exact(SIZE, SIZE, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        return egui::IconData {
            rgba: resized.into_raw(),
            width: SIZE,
            height: SIZE,
        };
    }
    egui::IconData {
        rgba: vec![0u8; (SIZE * SIZE * 4) as usize],
        width: SIZE,
        height: SIZE,
    }
}

#[cfg(test)]
mod tests {
    use super::build_window_icon;

    #[test]
    fn test_build_window_icon_valid_rgba() {
        let icon = build_window_icon();
        assert_eq!(icon.width, 64);
        assert_eq!(icon.height, 64);
        assert_eq!(icon.rgba.len(), 64 * 64 * 4);
        let center_alpha = icon.rgba[(32 * 64 + 32) * 4 + 3];
        assert_eq!(center_alpha, 255);
        assert_eq!(icon.rgba[3], 0);
    }
}

/// Draws a deterministic stylized 8x8 Minecraft pixel-head avatar for an account username.
pub fn draw_player_pixel_avatar(
    painter: &egui::Painter,
    rect: Rect,
    username: &str,
    accent_tint: Color32,
) {
    let rounding = Rounding::same((rect.width() * 0.22).clamp(4.0, 8.0));
    // Deterministic hash from username
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in username.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }

    let skin_palettes = [
        (Color32::from_rgb(206, 156, 118), Color32::from_rgb(176, 126, 88)),
        (Color32::from_rgb(235, 194, 158), Color32::from_rgb(204, 162, 126)),
        (Color32::from_rgb(166, 112, 74), Color32::from_rgb(138, 88, 54)),
        (Color32::from_rgb(112, 72, 48), Color32::from_rgb(88, 54, 34)),
    ];
    let hair_palettes = [
        Color32::from_rgb(66, 46, 32),
        Color32::from_rgb(38, 40, 48),
        Color32::from_rgb(148, 86, 44),
        Color32::from_rgb(198, 158, 78),
        accent_tint,
    ];
    let eye_palettes = [
        Color32::from_rgb(56, 134, 214),
        Color32::from_rgb(27, 217, 106),
        Color32::from_rgb(140, 92, 200),
        Color32::from_rgb(88, 62, 42),
    ];

    let (skin, skin_shade) = skin_palettes[(hash as usize) % skin_palettes.len()];
    let hair = hair_palettes[((hash >> 8) as usize) % hair_palettes.len()];
    let eye = eye_palettes[((hash >> 16) as usize) % eye_palettes.len()];

    painter.rect_filled(rect, rounding, skin);

    let cell = rect.width() / 8.0;
    let draw_px = |gx: usize, gy: usize, col: Color32| {
        let r = Rect::from_min_size(
            Pos2::new(rect.left() + gx as f32 * cell, rect.top() + gy as f32 * cell),
            Vec2::splat(cell + 0.35),
        );
        painter.rect_filled(r, Rounding::ZERO, col);
    };

    // Hair top + sides
    for gx in 0..8 {
        draw_px(gx, 0, hair);
        draw_px(gx, 1, hair);
    }
    draw_px(0, 2, hair);
    draw_px(7, 2, hair);
    if (hash >> 24) & 1 == 1 {
        draw_px(1, 2, hair);
        draw_px(6, 2, hair);
    }

    // Subtle cheek/nose shading
    draw_px(3, 5, skin_shade);
    draw_px(4, 5, skin_shade);
    draw_px(3, 6, Color32::from_rgb(138, 84, 72));
    draw_px(4, 6, Color32::from_rgb(138, 84, 72));

    // Eyes (white sclera + colored iris)
    draw_px(1, 4, Color32::WHITE);
    draw_px(2, 4, eye);
    draw_px(5, 4, eye);
    draw_px(6, 4, Color32::WHITE);

    painter.rect_stroke(
        rect,
        rounding,
        Stroke::new(1.1_f32, Color32::from_rgba_unmultiplied(accent_tint.r(), accent_tint.g(), accent_tint.b(), 170)),
    );
}

