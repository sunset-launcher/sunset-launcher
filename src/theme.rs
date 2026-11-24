//! Modrinth-inspired Obsidian & Emerald design system, animations, and custom widgets.

use crate::icons::{draw_check_animated, draw_icon_animated, Icon};
use crate::ui::anim::{
    compute_icon_anim, ease_out_back, ease_out_cubic, paint_click_ripple, paint_shimmer_sweep,
    pulse_01, track_click_anim,
};
use eframe::egui::{
    self, Color32, FontFamily, FontId, Margin, Pos2, Rect, Rounding, Sense, Stroke, TextStyle, Vec2,
};

// Dynamic theme palette slots (updated via `set_theme_palette`)
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

const fn pack_rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

fn unpack_rgb(packed: u32) -> Color32 {
    Color32::from_rgb((packed >> 16) as u8, (packed >> 8) as u8, packed as u8)
}

fn unpack_rgb_arr(packed: u32) -> [u8; 3] {
    [(packed >> 16) as u8, (packed >> 8) as u8, packed as u8]
}

fn store_rgb(slot: &AtomicU32, rgb: [u8; 3]) {
    slot.store(pack_rgb(rgb[0], rgb[1], rgb[2]), Ordering::Relaxed);
}

fn apply_palette_slots(p: &crate::models::CustomPalette) {
    store_rgb(&PAL_BG_VOID, p.bg_void);
    store_rgb(&PAL_BG_SIDEBAR, p.bg_sidebar);
    store_rgb(&PAL_BG_ELEVATED, p.bg_elevated);
    store_rgb(&PAL_BG_CARD, p.bg_card);
    store_rgb(&PAL_BG_CARD_HOVER, p.bg_card_hover);
    store_rgb(&PAL_BG_INPUT, p.bg_input);
    store_rgb(&PAL_BORDER_SUBTLE, p.border_subtle);
    store_rgb(&PAL_BORDER_STRONG, p.border_strong);
    store_rgb(&PAL_TEXT_PRIMARY, p.text_primary);
    store_rgb(&PAL_TEXT_SECONDARY, p.text_secondary);
    store_rgb(&PAL_TEXT_MUTED, p.text_muted);
    store_rgb(&ACTIVE_ACCENT, p.accent);
}

fn lerp_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    [
        (a[0] as f32 * inv + b[0] as f32 * t).round() as u8,
        (a[1] as f32 * inv + b[1] as f32 * t).round() as u8,
        (a[2] as f32 * inv + b[2] as f32 * t).round() as u8,
    ]
}

fn lerp_palette(
    a: &crate::models::CustomPalette,
    b: &crate::models::CustomPalette,
    t: f32,
) -> crate::models::CustomPalette {
    crate::models::CustomPalette {
        bg_void: lerp_rgb(a.bg_void, b.bg_void, t),
        bg_sidebar: lerp_rgb(a.bg_sidebar, b.bg_sidebar, t),
        bg_elevated: lerp_rgb(a.bg_elevated, b.bg_elevated, t),
        bg_card: lerp_rgb(a.bg_card, b.bg_card, t),
        bg_card_hover: lerp_rgb(a.bg_card_hover, b.bg_card_hover, t),
        bg_input: lerp_rgb(a.bg_input, b.bg_input, t),
        border_subtle: lerp_rgb(a.border_subtle, b.border_subtle, t),
        border_strong: lerp_rgb(a.border_strong, b.border_strong, t),
        text_primary: lerp_rgb(a.text_primary, b.text_primary, t),
        text_secondary: lerp_rgb(a.text_secondary, b.text_secondary, t),
        text_muted: lerp_rgb(a.text_muted, b.text_muted, t),
        accent: lerp_rgb(a.accent, b.accent, t),
    }
}

struct PaletteTransition {
    from: crate::models::CustomPalette,
    to: crate::models::CustomPalette,
    start_time: f64,
    duration: f32,
}

static PALETTE_TRANSITION: OnceLock<Mutex<Option<PaletteTransition>>> = OnceLock::new();

fn palette_transition_lock() -> &'static Mutex<Option<PaletteTransition>> {
    PALETTE_TRANSITION.get_or_init(|| Mutex::new(None))
}

static PAL_BG_VOID: AtomicU32 = AtomicU32::new(pack_rgb(18, 15, 24));
static PAL_BG_SIDEBAR: AtomicU32 = AtomicU32::new(pack_rgb(24, 19, 32));
static PAL_BG_ELEVATED: AtomicU32 = AtomicU32::new(pack_rgb(31, 24, 41));
static PAL_BG_CARD: AtomicU32 = AtomicU32::new(pack_rgb(38, 29, 49));
static PAL_BG_CARD_HOVER: AtomicU32 = AtomicU32::new(pack_rgb(48, 37, 61));
static PAL_BG_INPUT: AtomicU32 = AtomicU32::new(pack_rgb(14, 11, 20));
static PAL_BORDER_SUBTLE: AtomicU32 = AtomicU32::new(pack_rgb(56, 44, 70));
static PAL_BORDER_STRONG: AtomicU32 = AtomicU32::new(pack_rgb(84, 64, 102));
static PAL_TEXT_PRIMARY: AtomicU32 = AtomicU32::new(pack_rgb(247, 240, 236));
static PAL_TEXT_SECONDARY: AtomicU32 = AtomicU32::new(pack_rgb(188, 169, 178));
static PAL_TEXT_MUTED: AtomicU32 = AtomicU32::new(pack_rgb(130, 112, 128));
static ACTIVE_ACCENT: AtomicU32 = AtomicU32::new(pack_rgb(255, 128, 66));

pub fn bg_void() -> Color32 {
    unpack_rgb(PAL_BG_VOID.load(Ordering::Relaxed))
}
pub fn bg_sidebar() -> Color32 {
    unpack_rgb(PAL_BG_SIDEBAR.load(Ordering::Relaxed))
}
pub fn bg_elevated() -> Color32 {
    unpack_rgb(PAL_BG_ELEVATED.load(Ordering::Relaxed))
}
pub fn bg_card() -> Color32 {
    unpack_rgb(PAL_BG_CARD.load(Ordering::Relaxed))
}
pub fn bg_card_hover() -> Color32 {
    unpack_rgb(PAL_BG_CARD_HOVER.load(Ordering::Relaxed))
}
pub fn bg_input() -> Color32 {
    unpack_rgb(PAL_BG_INPUT.load(Ordering::Relaxed))
}
pub fn border_subtle() -> Color32 {
    unpack_rgb(PAL_BORDER_SUBTLE.load(Ordering::Relaxed))
}
pub fn border_strong() -> Color32 {
    unpack_rgb(PAL_BORDER_STRONG.load(Ordering::Relaxed))
}
pub fn text_primary() -> Color32 {
    unpack_rgb(PAL_TEXT_PRIMARY.load(Ordering::Relaxed))
}
pub fn text_secondary() -> Color32 {
    unpack_rgb(PAL_TEXT_SECONDARY.load(Ordering::Relaxed))
}
pub fn text_muted() -> Color32 {
    unpack_rgb(PAL_TEXT_MUTED.load(Ordering::Relaxed))
}

/// Reads the active 12 theme palette colors.
pub fn current_theme_palette() -> crate::models::CustomPalette {
    crate::models::CustomPalette {
        bg_void: unpack_rgb_arr(PAL_BG_VOID.load(Ordering::Relaxed)),
        bg_sidebar: unpack_rgb_arr(PAL_BG_SIDEBAR.load(Ordering::Relaxed)),
        bg_elevated: unpack_rgb_arr(PAL_BG_ELEVATED.load(Ordering::Relaxed)),
        bg_card: unpack_rgb_arr(PAL_BG_CARD.load(Ordering::Relaxed)),
        bg_card_hover: unpack_rgb_arr(PAL_BG_CARD_HOVER.load(Ordering::Relaxed)),
        bg_input: unpack_rgb_arr(PAL_BG_INPUT.load(Ordering::Relaxed)),
        border_subtle: unpack_rgb_arr(PAL_BORDER_SUBTLE.load(Ordering::Relaxed)),
        border_strong: unpack_rgb_arr(PAL_BORDER_STRONG.load(Ordering::Relaxed)),
        text_primary: unpack_rgb_arr(PAL_TEXT_PRIMARY.load(Ordering::Relaxed)),
        text_secondary: unpack_rgb_arr(PAL_TEXT_SECONDARY.load(Ordering::Relaxed)),
        text_muted: unpack_rgb_arr(PAL_TEXT_MUTED.load(Ordering::Relaxed)),
        accent: unpack_rgb_arr(ACTIVE_ACCENT.load(Ordering::Relaxed)),
    }
}

/// Updates all 12 theme palette colors at runtime immediately (clearing any in-flight fade).
pub fn set_theme_palette(p: &crate::models::CustomPalette) {
    if let Ok(mut guard) = palette_transition_lock().lock() {
        *guard = None;
    }
    apply_palette_slots(p);
}

/// Starts a smooth 0.28s crossfade from the current palette to `target`.
pub fn transition_theme_palette(ctx: &egui::Context, target: &crate::models::CustomPalette) {
    let from = current_theme_palette();
    let now = ctx.input(|i| i.time);
    if let Ok(mut guard) = palette_transition_lock().lock() {
        *guard = Some(PaletteTransition {
            from,
            to: *target,
            start_time: now,
            duration: 0.28,
        });
    }
    ctx.request_repaint();
}

/// Advances any active theme palette crossfade and reapplies visuals cleanly.
pub fn tick_theme_transition(ctx: &egui::Context) {
    let snapshot = if let Ok(mut guard) = palette_transition_lock().lock() {
        let Some(tr) = guard.as_ref() else {
            return;
        };
        let now = ctx.input(|i| i.time);
        let raw = ((now - tr.start_time) as f32 / tr.duration.max(0.01)).clamp(0.0, 1.0);
        let eased = ease_out_cubic(raw);
        let blended = lerp_palette(&tr.from, &tr.to, eased);
        let done = raw >= 1.0;
        if done {
            *guard = None;
        }
        Some((blended, !done))
    } else {
        None
    };

    if let Some((blended, still_running)) = snapshot {
        apply_palette_slots(&blended);
        apply_modrinth_theme(ctx);
        if still_running {
            ctx.request_repaint();
        }
    }
}

pub fn is_light_theme() -> bool {
    let c = bg_void();
    0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32 > 128.0
}

pub fn accent() -> Color32 {
    unpack_rgb(ACTIVE_ACCENT.load(Ordering::Relaxed))
}

pub fn accent_hover() -> Color32 {
    let target = if is_light_theme() {
        Color32::BLACK
    } else {
        PURE_WHITE
    };
    lerp_color(accent(), target, 0.16)
}

pub fn accent_dark() -> Color32 {
    let c = accent();
    let lum = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
    if lum > 140.0 {
        Color32::from_rgb(7, 20, 14)
    } else {
        PURE_WHITE
    }
}

pub fn accent_dim() -> Color32 {
    lerp_color(bg_void(), accent(), 0.22)
}

pub fn bg_card_active() -> Color32 {
    lerp_color(bg_card(), accent(), 0.11)
}

pub fn accent_text() -> Color32 {
    if is_light_theme() {
        lerp_color(accent(), Color32::BLACK, 0.45)
    } else {
        lerp_color(accent(), PURE_WHITE, 0.58)
    }
}

pub const DANGER: Color32 = Color32::from_rgb(229, 72, 77); // #E5484D
pub const DANGER_HOVER: Color32 = Color32::from_rgb(242, 94, 99);
pub const WARNING: Color32 = Color32::from_rgb(245, 166, 35); // #F5A623
pub const INFO: Color32 = Color32::from_rgb(76, 142, 219); // #4C8EDB
pub const PURE_WHITE: Color32 = Color32::from_rgb(255, 255, 255);

/// Linearly interpolates between two colors.
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    Color32::from_rgba_unmultiplied(
        (a.r() as f32 * inv + b.r() as f32 * t).round() as u8,
        (a.g() as f32 * inv + b.g() as f32 * t).round() as u8,
        (a.b() as f32 * inv + b.b() as f32 * t).round() as u8,
        (a.a() as f32 * inv + b.a() as f32 * t).round() as u8,
    )
}

/// Returns a copy of `col` with a scaled alpha channel.
pub fn with_alpha(col: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), alpha)
}

/// Applies the Obsidian & Emerald theme globally to `egui::Context`.
pub fn apply_modrinth_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.animation_time = 0.14;

    style.spacing.item_spacing = Vec2::new(8.0, 7.0);
    style.spacing.button_padding = Vec2::new(11.0, 6.0);
    style.spacing.interact_size.y = 26.0;
    style.spacing.window_margin = Margin::same(16.0);
    style.spacing.indent = 16.0;
    style.spacing.menu_margin = Margin::same(6.0);
    style.spacing.scroll.floating = true;
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.bar_inner_margin = 2.0;

    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(20.0, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(13.5, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(12.0, FontFamily::Monospace),
    );

    let light = is_light_theme();
    let mut v = if light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    v.dark_mode = !light;
    v.override_text_color = None;

    v.panel_fill = bg_void();
    v.window_fill = bg_elevated();
    v.window_rounding = Rounding::same(12.0);
    v.window_stroke = Stroke::new(1.0_f32, border_strong());
    let shadow_alpha = if light { 35 } else { 165 };
    v.window_shadow = egui::epaint::Shadow {
        offset: Vec2::new(0.0, 10.0),
        blur: 24.0,
        spread: 1.0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, shadow_alpha),
    };
    v.popup_shadow = egui::epaint::Shadow {
        offset: Vec2::new(0.0, 6.0),
        blur: 16.0,
        spread: 0.0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, shadow_alpha.saturating_sub(25)),
    };
    v.extreme_bg_color = bg_input();
    v.faint_bg_color = lerp_color(bg_void(), bg_card(), 0.35);
    v.code_bg_color = bg_card();

    v.selection.bg_fill = accent_dim();
    v.selection.stroke = Stroke::new(1.0_f32, text_primary());

    v.widgets.noninteractive.bg_fill = bg_card();
    v.widgets.noninteractive.weak_bg_fill = bg_card();
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border_subtle());
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, text_primary());
    v.widgets.noninteractive.rounding = Rounding::same(8.0);

    v.widgets.inactive.bg_fill = lerp_color(bg_card(), bg_input(), 0.35);
    v.widgets.inactive.weak_bg_fill = bg_card();
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, border_subtle());
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, text_primary());
    v.widgets.inactive.rounding = Rounding::same(8.0);

    v.widgets.hovered.bg_fill = bg_card_hover();
    v.widgets.hovered.weak_bg_fill = bg_card_hover();
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, accent());
    v.widgets.hovered.fg_stroke = Stroke::new(1.2_f32, text_primary());
    v.widgets.hovered.rounding = Rounding::same(8.0);

    v.widgets.active.bg_fill = accent_dim();
    v.widgets.active.weak_bg_fill = accent_dim();
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, accent());
    v.widgets.active.fg_stroke = Stroke::new(1.5_f32, text_primary());
    v.widgets.active.rounding = Rounding::same(8.0);

    v.widgets.open.bg_fill = bg_card_hover();
    v.widgets.open.weak_bg_fill = bg_card_hover();
    v.widgets.open.bg_stroke = Stroke::new(1.0_f32, accent());
    v.widgets.open.fg_stroke = Stroke::new(1.0_f32, text_primary());
    v.widgets.open.rounding = Rounding::same(8.0);

    v.slider_trailing_fill = true;

    style.visuals = v;
    ctx.set_style(style);
}

/// Draws a card frame with optional animated hover and accent selection border.
pub fn card_frame(selected: bool, hover_t: f32) -> egui::Frame {
    let bg = if selected {
        lerp_color(
            bg_card_active(),
            lerp_color(bg_card(), accent(), 0.18),
            hover_t,
        )
    } else {
        lerp_color(bg_card(), bg_card_hover(), hover_t)
    };
    let border = if selected {
        accent()
    } else {
        lerp_color(border_subtle(), border_strong(), hover_t)
    };
    egui::Frame::none()
        .fill(bg)
        .stroke(Stroke::new(1.0_f32, border))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::same(11.0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Subtle,
    Danger,
}

/// Renders an animated button with optional leading vector icon.
pub fn ui_button_icon(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    icon: Option<Icon>,
    text: &str,
    variant: ButtonVariant,
    enabled: bool,
    height: f32,
) -> egui::Response {
    render_button_impl(ui, id, icon, text, variant, enabled, None, height)
}

/// Renders an animated button with exact fixed width and height so stacked list/card CTAs align.
pub fn ui_button_sized(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    icon: Option<Icon>,
    text: &str,
    variant: ButtonVariant,
    enabled: bool,
    size: Vec2,
) -> egui::Response {
    render_button_impl(ui, id, icon, text, variant, enabled, Some(size.x), size.y)
}

#[allow(clippy::too_many_arguments)]
fn render_button_impl(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    icon: Option<Icon>,
    text: &str,
    variant: ButtonVariant,
    enabled: bool,
    fixed_width: Option<f32>,
    height: f32,
) -> egui::Response {
    let font = FontId::new(
        if height <= 28.0 { 12.5 } else { 13.0 },
        FontFamily::Proportional,
    );
    let galley = if text.is_empty() {
        None
    } else {
        Some(
            ui.painter()
                .layout_no_wrap(text.to_string(), font, text_primary()),
        )
    };
    let icon_sz = (height * 0.48).clamp(11.0, 15.0);
    let has_icon = icon.is_some();
    let has_text = galley.is_some();
    let gap = if has_icon && has_text { 6.0 } else { 0.0 };
    let pad_x = if !has_text {
        0.0
    } else if height <= 28.0 {
        11.0
    } else {
        14.0
    };
    let content_w =
        (if has_icon { icon_sz } else { 0.0 }) + gap + galley.as_ref().map_or(0.0, |g| g.size().x);
    let min_fit_w = (content_w + pad_x * 1.5).max(height);
    let desired_w = fixed_width
        .map(|w| w.max(min_fit_w))
        .unwrap_or_else(|| (content_w + pad_x * 2.0).max(height));
    let desired = Vec2::new(desired_w, height);

    let btn_id = ui.make_persistent_id(("btn_base", &id));
    let (rect, resp) = ui.allocate_exact_size(
        desired,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let hover_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("btn_hover", &id)),
        enabled && resp.hovered(),
        0.12,
    );
    let press_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("btn_press", &id)),
        enabled && resp.is_pointer_button_down_on(),
        0.07,
    );
    if (hover_t > 0.01 && hover_t < 0.99) || (press_t > 0.01 && press_t < 0.99) {
        ui.ctx().request_repaint();
    }
    let click_info = track_click_anim(
        ui.ctx(),
        btn_id,
        rect,
        enabled && resp.clicked(),
        resp.interact_pointer_pos(),
        0.42,
    );
    let click_t = click_info.map(|(t, _)| t);

    let copy_morph_t = if icon == Some(Icon::Copy) {
        crate::ui::anim::click_age_secs(ui.ctx(), btn_id).and_then(|age| {
            if age < 0.75 {
                ui.ctx().request_repaint();
                Some(age)
            } else {
                None
            }
        })
    } else {
        None
    };

    if ui.is_rect_visible(rect) {
        let (base_bg, hover_bg, base_border, hover_border, base_fg, hover_fg) = if !enabled {
            let dis_bg = lerp_color(bg_void(), bg_card(), 0.45);
            (
                dis_bg,
                dis_bg,
                border_subtle(),
                border_subtle(),
                text_muted(),
                text_muted(),
            )
        } else {
            match variant {
                ButtonVariant::Primary => (
                    accent(),
                    accent_hover(),
                    accent(),
                    accent_hover(),
                    accent_dark(),
                    accent_dark(),
                ),
                ButtonVariant::Secondary => (
                    bg_card(),
                    bg_card_hover(),
                    border_subtle(),
                    border_strong(),
                    text_primary(),
                    text_primary(),
                ),
                ButtonVariant::Subtle => (
                    bg_elevated(),
                    bg_card_hover(),
                    border_subtle(),
                    border_strong(),
                    text_secondary(),
                    text_primary(),
                ),
                ButtonVariant::Danger => {
                    if is_light_theme() {
                        (
                            lerp_color(bg_card(), DANGER, 0.12),
                            DANGER,
                            lerp_color(border_subtle(), DANGER, 0.55),
                            DANGER_HOVER,
                            Color32::from_rgb(178, 30, 38),
                            PURE_WHITE,
                        )
                    } else {
                        (
                            Color32::from_rgb(46, 28, 32),
                            DANGER,
                            Color32::from_rgb(96, 46, 52),
                            DANGER_HOVER,
                            Color32::from_rgb(248, 136, 140),
                            PURE_WHITE,
                        )
                    }
                }
            }
        };

        let bg = lerp_color(base_bg, hover_bg, hover_t);
        let border = lerp_color(base_border, hover_border, hover_t);
        let fg = lerp_color(base_fg, hover_fg, hover_t);
        let release_pop = click_t.map_or(0.0, |t| {
            ((1.0 - t) * std::f32::consts::PI).sin().max(0.0) * 0.55
        });
        let draw_rect = rect.shrink(press_t * 1.1).expand(release_pop);
        let rounding = Rounding::same(7.5);
        let painter = ui.painter();

        painter.rect_filled(draw_rect, rounding, bg);
        if enabled && hover_t > 0.01 && hover_t < 0.99 {
            let max_a = match variant {
                ButtonVariant::Primary => 42.0,
                _ => 22.0,
            };
            let sheen_alpha = ((hover_t * std::f32::consts::PI).sin() * max_a).round() as u8;
            paint_shimmer_sweep(
                painter,
                draw_rect,
                rounding,
                hover_t,
                PURE_WHITE,
                sheen_alpha,
            );
        }
        if enabled && draw_rect.width() > 14.0 {
            let bevel_alpha = match variant {
                ButtonVariant::Primary => (72.0 + hover_t * 36.0) as u8,
                ButtonVariant::Secondary | ButtonVariant::Subtle => (18.0 + hover_t * 28.0) as u8,
                ButtonVariant::Danger => (20.0 + hover_t * 40.0) as u8,
            };
            painter.line_segment(
                [
                    Pos2::new(draw_rect.left() + 6.0, draw_rect.top() + 1.0),
                    Pos2::new(draw_rect.right() - 6.0, draw_rect.top() + 1.0),
                ],
                Stroke::new(
                    1.0_f32,
                    Color32::from_rgba_unmultiplied(255, 255, 255, bevel_alpha),
                ),
            );
        }
        painter.rect_stroke(draw_rect, rounding, Stroke::new(1.0_f32, border));

        if let Some((ct, origin)) = click_info {
            let ripple_col = match variant {
                ButtonVariant::Primary => accent_dark(),
                ButtonVariant::Danger => PURE_WHITE,
                ButtonVariant::Secondary | ButtonVariant::Subtle => accent(),
            };
            paint_click_ripple(painter, draw_rect, rounding, origin, ct, ripple_col, true);
        }

        let clipped = painter.with_clip_rect(draw_rect.shrink(3.0));
        let mut cur_x = draw_rect.center().x - content_w * 0.5;
        if let Some(ic) = icon {
            let icon_rect = Rect::from_center_size(
                Pos2::new(cur_x + icon_sz * 0.5, draw_rect.center().y),
                Vec2::splat(icon_sz),
            );
            if let Some(age) = copy_morph_t {
                let draw_p = (age / 0.22).clamp(0.0, 1.0);
                let check_col = match variant {
                    ButtonVariant::Primary | ButtonVariant::Danger => fg,
                    _ => accent(),
                };
                draw_check_animated(&clipped, icon_rect, check_col, draw_p);
            } else {
                let ic_anim = compute_icon_anim(ic, hover_t, press_t, click_t);
                draw_icon_animated(&clipped, icon_rect, ic, fg, ic_anim);
            }
            cur_x += icon_sz + gap;
        }
        if let Some(g) = galley {
            clipped.galley(
                Pos2::new(cur_x, draw_rect.center().y - g.size().y * 0.5),
                g,
                fg,
            );
        }
    }

    if enabled && resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Renders a standard animated button without a leading icon.
pub fn ui_button(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    text: &str,
    variant: ButtonVariant,
    enabled: bool,
    height: f32,
) -> egui::Response {
    ui_button_icon(ui, id, None, text, variant, enabled, height)
}

/// Renders an animated square vector icon button.
pub fn ui_icon_button(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    icon: Icon,
    tooltip: &str,
    variant: ButtonVariant,
    enabled: bool,
    size: f32,
) -> egui::Response {
    ui_button_icon(ui, id, Some(icon), "", variant, enabled, size).on_hover_text(tooltip)
}

/// Horizontal row with fixed height where left and right widgets align to the exact same centerline.
pub fn ui_toolbar_row<R>(
    ui: &mut egui::Ui,
    row_h: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let avail_w = ui.available_width().max(20.0);
    let box_h = row_h + 2.0;
    ui.allocate_ui_with_layout(
        Vec2::new(avail_w, box_h),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_max_width(avail_w);
            ui.set_min_height(box_h);
            let r = ui.max_rect();
            let clip = ui.clip_rect().intersect(egui::Rect::from_min_max(
                Pos2::new(r.left() - 4.0, r.top() - 4.0),
                Pos2::new(r.right() + 2.0, r.bottom() + 4.0),
            ));
            ui.set_clip_rect(clip);
            ui.spacing_mut().interact_size.y = row_h;
            ui.spacing_mut().button_padding = Vec2::new(11.0, ((row_h - 15.0) * 0.5).max(2.0));
            add_contents(ui)
        },
    )
}

/// Multi-line wrapping horizontal row that adapts button padding and spacing when narrow
/// and wraps controls onto subsequent lines within `ui.available_width()` without clipping.
pub fn ui_wrap_row<R>(
    ui: &mut egui::Ui,
    row_h: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let avail_w = ui.available_width().max(60.0);
    let pad_x = if avail_w < 420.0 {
        7.0
    } else if avail_w < 620.0 {
        9.0
    } else {
        11.0
    };
    let gap_x = if avail_w < 460.0 { 5.0 } else { 6.0 };
    ui.scope(|ui| {
        ui.set_max_width(avail_w);
        ui.spacing_mut().interact_size.y = row_h;
        ui.spacing_mut().button_padding = Vec2::new(pad_x, ((row_h - 15.0) * 0.5).max(2.0));
        ui.spacing_mut().item_spacing = Vec2::new(gap_x, 6.0);
        ui.horizontal_wrapped(add_contents).inner
    })
}

/// Multi-line wrapping horizontal row where controls align vertically on each line
/// and wrap cleanly onto subsequent lines within `ui.available_width()` without
/// stretching surrounding cards or clipping buttons.
pub fn ui_flow_row<R>(
    ui: &mut egui::Ui,
    row_h: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let avail_w = ui.available_width().max(60.0);
    ui.scope(|ui| {
        ui.set_max_width(avail_w);
        ui.spacing_mut().interact_size.y = row_h;
        ui.spacing_mut().button_padding = Vec2::new(10.0, ((row_h - 15.0) * 0.5).max(2.0));
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        ui.horizontal_wrapped(add_contents).inner
    })
}

/// Renders a filter/option pill tab with optional vector icon and animated bottom emerald bar.
pub fn ui_pill_tab_icon(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    icon: Option<Icon>,
    text: &str,
    active: bool,
) -> egui::Response {
    let font = FontId::new(12.5, FontFamily::Proportional);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, text_primary());
    let icon_sz = 12.5;
    let gap = if icon.is_some() { 5.5 } else { 0.0 };
    let content_w = (if icon.is_some() { icon_sz } else { 0.0 }) + gap + galley.size().x;
    let row_h = ui.spacing().interact_size.y.max(28.0);
    let desired = Vec2::new((content_w + 22.0).max(46.0), row_h);
    let pill_id = ui.make_persistent_id(("pill_tab_base", &id));
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());

    let active_t =
        ui.ctx()
            .animate_bool_with_time(ui.make_persistent_id(("pill_act", &id)), active, 0.16);
    let hover_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("pill_hov", &id)),
        resp.hovered(),
        0.11,
    );
    let press_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("pill_prs", &id)),
        resp.is_pointer_button_down_on(),
        0.07,
    );
    if (active_t > 0.01 && active_t < 0.99)
        || (hover_t > 0.01 && hover_t < 0.99)
        || (press_t > 0.01 && press_t < 0.99)
    {
        ui.ctx().request_repaint();
    }
    let click_info = track_click_anim(
        ui.ctx(),
        pill_id,
        rect,
        resp.clicked(),
        resp.interact_pointer_pos(),
        0.38,
    );
    let click_t = click_info.map(|(t, _)| t);

    if ui.is_rect_visible(rect) {
        let draw_rect = rect.shrink(press_t * 0.9);
        let idle_bg = lerp_color(bg_elevated(), bg_card_hover(), hover_t);
        let bg = lerp_color(idle_bg, accent_dim(), active_t);
        let border = lerp_color(
            lerp_color(border_subtle(), border_strong(), hover_t),
            accent(),
            active_t,
        );
        let fg = lerp_color(
            lerp_color(text_secondary(), text_primary(), hover_t),
            accent_text(),
            active_t,
        );
        let painter = ui.painter();
        painter.rect_filled(draw_rect, Rounding::same(7.5), bg);
        painter.rect_stroke(draw_rect, Rounding::same(7.5), Stroke::new(1.0_f32, border));

        if let Some((ct, origin)) = click_info {
            paint_click_ripple(
                painter,
                draw_rect,
                Rounding::same(7.5),
                origin,
                ct,
                accent(),
                true,
            );
        }

        if active_t > 0.03 {
            let spring_w = ease_out_back(active_t).min(1.06);
            let bar_w =
                ((draw_rect.width() - 14.0).max(8.0) * spring_w).min(draw_rect.width() - 6.0);
            let bar_rect = Rect::from_center_size(
                Pos2::new(draw_rect.center().x, draw_rect.bottom() - 2.0),
                Vec2::new(bar_w, 2.0),
            );
            painter.rect_filled(
                bar_rect,
                Rounding::same(1.0),
                with_alpha(accent(), (active_t * 255.0) as u8),
            );
        }

        let mut cur_x = draw_rect.center().x - content_w * 0.5;
        let cy = draw_rect.center().y - active_t * 0.5;
        if let Some(ic) = icon {
            let ic_rect =
                Rect::from_center_size(Pos2::new(cur_x + icon_sz * 0.5, cy), Vec2::splat(icon_sz));
            let ic_anim = compute_icon_anim(ic, hover_t.max(active_t * 0.35), press_t, click_t);
            draw_icon_animated(painter, ic_rect, ic, fg, ic_anim);
            cur_x += icon_sz + gap;
        }
        painter.galley(Pos2::new(cur_x, cy - galley.size().y * 0.5), galley, fg);
    }

    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

pub fn ui_pill_tab(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    text: &str,
    active: bool,
) -> egui::Response {
    ui_pill_tab_icon(ui, id, None, text, active)
}

/// Renders a loader selector pill tinted with each loader's signature accent() colors.
#[allow(dead_code)]
pub fn ui_loader_pill(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    kind: crate::models::LoaderKind,
    active: bool,
) -> egui::Response {
    let (loader_bg, loader_border, loader_fg) = loader_colors(kind);
    let font = FontId::new(12.5, FontFamily::Proportional);
    let galley = ui
        .painter()
        .layout_no_wrap(kind.label().to_string(), font, text_primary());
    let dot_sz = 7.0;
    let gap = 6.0;
    let content_w = dot_sz + gap + galley.size().x;
    let row_h = ui.spacing().interact_size.y.max(28.0);
    let desired = Vec2::new((content_w + 22.0).max(54.0), row_h);
    let lpill_id = ui.make_persistent_id(("loader_pill_base", &id));
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());

    let active_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("loader_pill_act", &id)),
        active,
        0.16,
    );
    let hover_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("loader_pill_hov", &id)),
        resp.hovered(),
        0.11,
    );
    let press_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("loader_pill_prs", &id)),
        resp.is_pointer_button_down_on(),
        0.07,
    );
    if (active_t > 0.01 && active_t < 0.99)
        || (hover_t > 0.01 && hover_t < 0.99)
        || (press_t > 0.01 && press_t < 0.99)
    {
        ui.ctx().request_repaint();
    }
    let click_info = track_click_anim(
        ui.ctx(),
        lpill_id,
        rect,
        resp.clicked(),
        resp.interact_pointer_pos(),
        0.38,
    );

    if ui.is_rect_visible(rect) {
        let draw_rect = rect.shrink(press_t * 0.9);
        let idle_bg = lerp_color(bg_elevated(), bg_card_hover(), hover_t);
        let bg = lerp_color(idle_bg, loader_bg, active_t);
        let border = lerp_color(
            lerp_color(border_subtle(), border_strong(), hover_t),
            loader_border,
            active_t,
        );
        let fg = lerp_color(
            lerp_color(text_secondary(), text_primary(), hover_t),
            loader_fg,
            active_t,
        );
        let dot_col = lerp_color(loader_border, loader_fg, active_t * 0.45);
        let painter = ui.painter();
        painter.rect_filled(draw_rect, Rounding::same(7.5), bg);
        painter.rect_stroke(draw_rect, Rounding::same(7.5), Stroke::new(1.0_f32, border));

        if let Some((ct, origin)) = click_info {
            paint_click_ripple(
                painter,
                draw_rect,
                Rounding::same(7.5),
                origin,
                ct,
                loader_border,
                true,
            );
        }

        if active_t > 0.03 {
            let spring_w = ease_out_back(active_t).min(1.06);
            let bar_w =
                ((draw_rect.width() - 14.0).max(8.0) * spring_w).min(draw_rect.width() - 6.0);
            let bar_rect = Rect::from_center_size(
                Pos2::new(draw_rect.center().x, draw_rect.bottom() - 2.0),
                Vec2::new(bar_w, 2.0),
            );
            painter.rect_filled(
                bar_rect,
                Rounding::same(1.0),
                with_alpha(loader_border, (active_t * 255.0) as u8),
            );
        }

        let start_x = draw_rect.center().x - content_w * 0.5;
        let cy = draw_rect.center().y - active_t * 0.5;
        let dot_c = Pos2::new(start_x + dot_sz * 0.5, cy);
        if active_t > 0.05 && active_t < 0.95 {
            let ring_r = 3.5 + (1.0 - active_t) * 5.0;
            painter.circle_stroke(
                dot_c,
                ring_r,
                Stroke::new(
                    1.1_f32,
                    with_alpha(loader_border, (active_t * (1.0 - active_t) * 600.0) as u8),
                ),
            );
        }
        let dot_pop = click_info.map_or(0.0, |(t, _)| (t * std::f32::consts::PI).sin() * 1.1);
        painter.circle_filled(dot_c, 3.2 + dot_pop, dot_col);
        painter.galley(
            Pos2::new(start_x + dot_sz + gap, cy - galley.size().y * 0.5),
            galley,
            fg,
        );
    }

    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Custom animated emerald checkbox with vector checkmark. Returns true when toggled.
pub fn ui_checkbox(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    checked: &mut bool,
    label: &str,
) -> bool {
    let font = FontId::new(12.5, FontFamily::Proportional);
    let box_sz = 17.0;
    let gap = 7.0;
    let avail_bound = if ui.layout().main_dir().is_vertical() {
        ui.available_width()
    } else {
        ui.max_rect().width()
    };
    let max_text_w = if avail_bound.is_finite() && avail_bound > box_sz + gap + 24.0 {
        (avail_bound - box_sz - gap - 6.0).max(40.0)
    } else {
        f32::INFINITY
    };
    let galley = ui
        .painter()
        .layout(label.to_string(), font, text_primary(), max_text_w);
    let row_h = ui
        .spacing()
        .interact_size
        .y
        .max(26.0)
        .max(galley.size().y + 6.0);
    let desired = Vec2::new(box_sz + gap + galley.size().x + 4.0, row_h);
    let chk_id = ui.make_persistent_id(("chk_base", &id_salt));
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    let mut toggled = false;
    if resp.clicked() {
        *checked = !*checked;
        toggled = true;
    }

    let check_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("chk_val", &id_salt)),
        *checked,
        0.15,
    );
    let hover_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("chk_hov", &id_salt)),
        resp.hovered(),
        0.11,
    );
    let press_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("chk_prs", &id_salt)),
        resp.is_pointer_button_down_on(),
        0.07,
    );
    if (check_t > 0.01 && check_t < 0.99)
        || (hover_t > 0.01 && hover_t < 0.99)
        || (press_t > 0.01 && press_t < 0.99)
    {
        ui.ctx().request_repaint();
    }
    let click_info = track_click_anim(
        ui.ctx(),
        chk_id,
        rect,
        toggled,
        resp.interact_pointer_pos(),
        0.36,
    );

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let pop = click_info.map_or(0.0, |(t, _)| (t * std::f32::consts::PI).sin() * 0.14);
        let box_scale = (1.0 - press_t * 0.08 + pop).max(0.8);
        let multiline = galley.rows.len() > 1;
        let box_cy = if multiline {
            let first_row_h = galley.rows.first().map_or(box_sz, |r| r.rect.height());
            rect.top() + (first_row_h * 0.5 + 3.0).max(box_sz * 0.5 + 2.0)
        } else {
            rect.center().y
        };
        let box_rect = Rect::from_center_size(
            Pos2::new(rect.left() + box_sz * 0.5 + 1.0, box_cy),
            Vec2::splat(box_sz * box_scale),
        );
        let fill = lerp_color(
            lerp_color(bg_input(), bg_card_hover(), hover_t * 0.6),
            accent(),
            check_t,
        );
        let border = lerp_color(
            lerp_color(border_strong(), text_primary(), hover_t * 0.4),
            accent_hover(),
            check_t,
        );
        painter.rect_filled(box_rect, Rounding::same(4.5), fill);
        painter.rect_stroke(box_rect, Rounding::same(4.5), Stroke::new(1.1_f32, border));

        if let Some((ct, _)) = click_info {
            let eased = crate::ui::anim::ease_out_quint(ct);
            let ring_a = ((1.0 - ct) * (1.0 - ct) * 165.0).round() as u8;
            if ring_a > 1 {
                let ring_col = if *checked { accent() } else { border_strong() };
                painter.rect_stroke(
                    box_rect.expand(eased * 4.5),
                    Rounding::same(4.5 + eased * 1.5),
                    Stroke::new(1.3_f32, with_alpha(ring_col, ring_a)),
                );
            }
        }

        if check_t > 0.04 {
            draw_check_animated(
                painter,
                box_rect.shrink(3.0),
                with_alpha(accent_dark(), (check_t.clamp(0.0, 1.0) * 255.0) as u8),
                ease_out_cubic(check_t),
            );
        }

        let text_col = lerp_color(text_secondary(), text_primary(), check_t.max(hover_t * 0.8));
        let text_y = if multiline {
            rect.top() + 3.0
        } else {
            rect.center().y - galley.size().y * 0.5
        };
        painter.galley(
            Pos2::new(rect.left() + box_sz + 1.0 + gap, text_y),
            galley,
            text_col,
        );
    }

    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    toggled
}

/// Framed singleline text input with smooth emerald focus glow.
pub fn ui_text_input(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    text: &mut String,
    hint: &str,
    width: f32,
    password: bool,
) -> egui::Response {
    let focus_state_id = ui.make_persistent_id(("txt_foc_state", &id_salt));
    let type_pulse_id = ui.make_persistent_id(("txt_type_pulse", &id_salt));
    let prev_focused = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(focus_state_id))
        .unwrap_or(false);
    let focus_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("txt_foc_anim", &id_salt)),
        prev_focused,
        0.15,
    );
    if focus_t > 0.01 && focus_t < 0.99 {
        ui.ctx().request_repaint();
    }
    let last_type_t = ui.ctx().data(|d| d.get_temp::<f64>(type_pulse_id));
    let type_age = last_type_t.map(|t| (ui.input(|i| i.time) - t) as f32);
    let type_pulse = type_age
        .filter(|&age| (0.0..0.22).contains(&age))
        .map(|age| {
            ui.ctx().request_repaint();
            1.0 - (age / 0.22)
        })
        .unwrap_or(0.0);

    let border_col = lerp_color(
        lerp_color(border_subtle(), accent(), focus_t),
        accent_hover(),
        type_pulse * 0.65,
    );
    let total_w = width.max(80.0);
    let row_h = ui.spacing().interact_size.y.max(28.0);
    let mut now_focused = false;
    let mut inner_resp: Option<egui::Response> = None;

    let box_resp = ui.allocate_ui_with_layout(
        Vec2::new(total_w, row_h),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_size(Vec2::new(total_w, row_h));
            ui.set_max_size(Vec2::new(total_w, row_h));
            let box_rect = ui.max_rect();
            if ui.is_rect_visible(box_rect) {
                let painter = ui.painter();
                if focus_t > 0.03 || type_pulse > 0.02 {
                    let glow_a = ((focus_t * 55.0) + type_pulse * 75.0).clamp(0.0, 140.0) as u8;
                    painter.rect_stroke(
                        box_rect.expand(1.5),
                        Rounding::same(9.0),
                        Stroke::new(1.5_f32, with_alpha(accent(), glow_a)),
                    );
                }
                painter.rect(
                    box_rect,
                    Rounding::same(7.5),
                    bg_input(),
                    Stroke::new(1.0_f32 + focus_t * 0.5, border_col),
                );
                if focus_t > 0.04 {
                    let bar_w = (box_rect.width() - 16.0).max(8.0) * ease_out_cubic(focus_t);
                    let bar_r = Rect::from_center_size(
                        Pos2::new(box_rect.center().x, box_rect.bottom() - 1.5),
                        Vec2::new(bar_w, 1.8),
                    );
                    painter.rect_filled(
                        bar_r,
                        Rounding::same(0.9),
                        with_alpha(accent(), (focus_t * 215.0) as u8),
                    );
                }
            }
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(8.0);
            let edit = egui::TextEdit::singleline(text)
                .id_salt(&id_salt)
                .frame(false)
                .password(password)
                .hint_text(hint)
                .desired_width((total_w - 16.0).max(50.0));
            let r = ui.add(edit);
            if r.changed() {
                let now = ui.input(|i| i.time);
                ui.ctx().data_mut(|d| d.insert_temp(type_pulse_id, now));
                ui.ctx().request_repaint();
            }
            now_focused = r.has_focus() || r.hovered();
            inner_resp = Some(r);
        },
    );

    ui.ctx()
        .data_mut(|d| d.insert_temp(focus_state_id, now_focused));

    inner_resp.unwrap_or(box_resp.response)
}

/// Renders a Modrinth-style search input with vector magnifier, animated focus glow, and vector clear (`X`) button.
/// Returns `true` if Enter was pressed in the input or the clear button was clicked.
pub fn ui_search_input(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    text: &mut String,
    hint: &str,
    width: f32,
) -> bool {
    let mut submitted = false;
    let total_w = width.max(96.0);
    let row_h = ui.spacing().interact_size.y.max(28.0);
    let has_clear = !text.is_empty();
    let inner_edit_w = (total_w - if has_clear { 58.0 } else { 36.0 }).max(48.0);

    let focus_state_id = ui.make_persistent_id(("search_foc_state", &id_salt));
    let type_pulse_id = ui.make_persistent_id(("search_type_pulse", &id_salt));
    let submit_pulse_id = ui.make_persistent_id(("search_submit_pulse", &id_salt));
    let prev_focused = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(focus_state_id))
        .unwrap_or(false);
    let focus_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("search_foc_anim", &id_salt)),
        prev_focused,
        0.15,
    );
    if focus_t > 0.01 && focus_t < 0.99 {
        ui.ctx().request_repaint();
    }

    let now = ui.input(|i| i.time);
    let type_pulse = ui
        .ctx()
        .data(|d| d.get_temp::<f64>(type_pulse_id))
        .map(|t| (now - t) as f32)
        .filter(|&age| (0.0..0.22).contains(&age))
        .map(|age| {
            ui.ctx().request_repaint();
            1.0 - age / 0.22
        })
        .unwrap_or(0.0);
    let submit_pulse = ui
        .ctx()
        .data(|d| d.get_temp::<f64>(submit_pulse_id))
        .map(|t| (now - t) as f32)
        .filter(|&age| (0.0..0.32).contains(&age))
        .map(|age| {
            ui.ctx().request_repaint();
            age / 0.32
        });

    let border_col = lerp_color(
        lerp_color(border_subtle(), accent(), focus_t),
        accent_hover(),
        type_pulse * 0.65,
    );
    let icon_col = lerp_color(text_muted(), accent(), focus_t.max(type_pulse));
    let mut now_focused = false;

    ui.allocate_ui_with_layout(
        Vec2::new(total_w, row_h),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_size(Vec2::new(total_w, row_h));
            ui.set_max_size(Vec2::new(total_w, row_h));
            let box_rect = ui.max_rect();
            if ui.is_rect_visible(box_rect) {
                let painter = ui.painter();
                if focus_t > 0.03 || type_pulse > 0.02 {
                    let glow_a = ((focus_t * 55.0) + type_pulse * 75.0).clamp(0.0, 140.0) as u8;
                    painter.rect_stroke(
                        box_rect.expand(1.5),
                        Rounding::same(9.0),
                        Stroke::new(1.5_f32, with_alpha(accent(), glow_a)),
                    );
                }
                painter.rect(
                    box_rect,
                    Rounding::same(7.5),
                    bg_input(),
                    Stroke::new(1.0_f32 + focus_t * 0.5, border_col),
                );
                if focus_t > 0.04 {
                    let bar_w = (box_rect.width() - 16.0).max(8.0) * ease_out_cubic(focus_t);
                    let bar_r = Rect::from_center_size(
                        Pos2::new(box_rect.center().x, box_rect.bottom() - 1.5),
                        Vec2::new(bar_w, 1.8),
                    );
                    painter.rect_filled(
                        bar_r,
                        Rounding::same(0.9),
                        with_alpha(accent(), (focus_t * 215.0) as u8),
                    );
                }
                if let Some(sp) = submit_pulse {
                    paint_click_ripple(
                        painter,
                        box_rect,
                        Rounding::same(7.5),
                        box_rect.center(),
                        sp,
                        accent(),
                        true,
                    );
                }
            }
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_space(2.0);
            let (mag_r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
            let mag_anim = compute_icon_anim(
                Icon::Search,
                focus_t * 0.8 + type_pulse * 0.4,
                0.0,
                submit_pulse,
            );
            draw_icon_animated(ui.painter(), mag_r, Icon::Search, icon_col, mag_anim);

            let edit_resp = ui.add(
                egui::TextEdit::singleline(text)
                    .id_salt(&id_salt)
                    .frame(false)
                    .hint_text(hint)
                    .desired_width(inner_edit_w),
            );
            if edit_resp.changed() {
                ui.ctx().data_mut(|d| d.insert_temp(type_pulse_id, now));
                ui.ctx().request_repaint();
            }
            now_focused = edit_resp.has_focus() || edit_resp.hovered();
            if edit_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submitted = true;
                ui.ctx().data_mut(|d| d.insert_temp(submit_pulse_id, now));
                ui.ctx().request_repaint();
            }
            if has_clear {
                let clr_id = ui.make_persistent_id(("search_clr_btn", &id_salt));
                let (clr_r, clr_resp) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
                let clr_hov =
                    ui.ctx()
                        .animate_bool_with_time(clr_id.with("hov"), clr_resp.hovered(), 0.10);
                if clr_hov > 0.01 && clr_hov < 0.99 {
                    ui.ctx().request_repaint();
                }
                let clr_col = lerp_color(text_secondary(), PURE_WHITE, clr_hov);
                let clr_anim = compute_icon_anim(Icon::Close, clr_hov, 0.0, None);
                draw_icon_animated(
                    ui.painter(),
                    clr_r.shrink(2.5),
                    Icon::Close,
                    clr_col,
                    clr_anim,
                );
                if clr_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if clr_resp.on_hover_text("Clear search").clicked() {
                    text.clear();
                    submitted = true;
                    ui.ctx().data_mut(|d| d.insert_temp(submit_pulse_id, now));
                    ui.ctx().request_repaint();
                }
            }
        },
    );

    ui.ctx()
        .data_mut(|d| d.insert_temp(focus_state_id, now_focused));

    submitted
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    Info,
    Success,
    Warning,
    Error,
}

/// Renders an inline semantic alert banner with entrance slide and leading vector icon.
pub fn ui_alert_banner(ui: &mut egui::Ui, kind: AlertKind, text: &str, width: f32) {
    let enter_id = ui.make_persistent_id(("alert_banner_enter", kind as u8, text));
    let now = ui.input(|i| i.time);
    let enter_start = ui.ctx().data_mut(|d| {
        if let Some(t0) = d.get_temp::<f64>(enter_id) {
            t0
        } else {
            d.insert_temp(enter_id, now);
            now
        }
    });
    let enter_raw = ((now - enter_start) as f32 / 0.24).clamp(0.0, 1.0);
    let enter_eased = ease_out_cubic(enter_raw);
    if enter_raw < 1.0 {
        ui.ctx().request_repaint();
    }

    let light = is_light_theme();
    let (bg, border, fg, icon) = match kind {
        AlertKind::Info => (
            if light {
                lerp_color(bg_card(), INFO, 0.14)
            } else {
                Color32::from_rgb(24, 34, 48)
            },
            INFO,
            if light {
                lerp_color(INFO, Color32::BLACK, 0.42)
            } else {
                Color32::from_rgb(172, 212, 255)
            },
            Icon::Sparkle,
        ),
        AlertKind::Success => (bg_card_active(), accent(), accent_text(), Icon::Check),
        AlertKind::Warning => (
            if light {
                lerp_color(bg_card(), WARNING, 0.14)
            } else {
                Color32::from_rgb(48, 38, 22)
            },
            WARNING,
            if light {
                lerp_color(WARNING, Color32::BLACK, 0.45)
            } else {
                Color32::from_rgb(252, 212, 134)
            },
            Icon::Alert,
        ),
        AlertKind::Error => (
            if light {
                lerp_color(bg_card(), DANGER, 0.13)
            } else {
                Color32::from_rgb(50, 26, 30)
            },
            DANGER,
            if light {
                lerp_color(DANGER, Color32::BLACK, 0.35)
            } else {
                Color32::from_rgb(255, 168, 172)
            },
            Icon::Alert,
        ),
    };
    let inner_w = (width - 24.0).max(80.0);
    let alpha_scale = (0.35 + enter_eased * 0.65).clamp(0.0, 1.0);
    let border_alpha = ((180.0 + (1.0 - enter_eased) * 75.0) * alpha_scale) as u8;
    let frame_resp = egui::Frame::none()
        .fill(with_alpha(bg, (255.0 * alpha_scale) as u8))
        .stroke(Stroke::new(
            1.0_f32 + (1.0 - enter_eased) * 0.8,
            with_alpha(border, border_alpha),
        ))
        .rounding(Rounding::same(8.5))
        .inner_margin(Margin::symmetric(11.0, 8.0))
        .show(ui, |ui| {
            ui.set_min_width(inner_w);
            ui.set_max_width(inner_w);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let (ic_r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                let ic_scale = 0.85 + 0.15 * ease_out_back(enter_raw).min(1.15);
                let slide_y = (1.0 - enter_eased) * -4.0;
                draw_icon_animated(
                    ui.painter(),
                    ic_r,
                    icon,
                    with_alpha(border, (255.0 * alpha_scale) as u8),
                    crate::ui::anim::IconAnim {
                        offset: Vec2::new(0.0, slide_y),
                        scale: ic_scale,
                        rotation: 0.0,
                    },
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(text)
                            .small()
                            .color(with_alpha(fg, (255.0 * alpha_scale) as u8)),
                    )
                    .wrap(),
                );
            });
        });
    if enter_raw < 0.99 {
        let r = frame_resp.response.rect;
        let pulse_r = Rect::from_min_size(
            r.min + Vec2::new(3.0, 5.0),
            Vec2::new(3.0, (r.height() - 10.0).max(6.0)),
        );
        ui.painter().rect_filled(
            pulse_r,
            Rounding::same(1.5),
            with_alpha(border, ((1.0 - enter_raw) * 230.0) as u8),
        );
    }
}

/// Draws a section header label with subtle uppercase tracking inside settings/forms.
pub fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .size(10.5)
            .strong()
            .color(text_muted()),
    );
}

/// Renders a card with persistent smooth hover + selection animation,
/// top specular rim, and an animated left accent() indicator bar.
pub fn animated_card<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    selected: bool,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let prev_hover = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let prev_sel_id = id.with("prev_sel");
    let flash_id = id.with("sel_flash_t");
    let prev_selected = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(prev_sel_id))
        .unwrap_or(selected);
    let now = ui.input(|i| i.time);
    if !prev_selected && selected {
        ui.ctx().data_mut(|d| d.insert_temp(flash_id, now));
        ui.ctx().request_repaint();
    }
    ui.ctx().data_mut(|d| d.insert_temp(prev_sel_id, selected));

    let hover_t = ui.ctx().animate_bool_with_time(id, prev_hover, 0.14);
    let select_t = ui
        .ctx()
        .animate_bool_with_time(id.with("sel"), selected, 0.16);
    if (hover_t > 0.01 && hover_t < 0.99) || (select_t > 0.01 && select_t < 0.99) {
        ui.ctx().request_repaint();
    }

    let flash_t = ui
        .ctx()
        .data(|d| d.get_temp::<f64>(flash_id))
        .map(|t0| (now - t0) as f32)
        .filter(|&age| (0.0..0.36).contains(&age))
        .map(|age| {
            ui.ctx().request_repaint();
            age / 0.36
        });

    let resp = card_frame(selected, hover_t).show(ui, add_contents);
    let rect = resp.response.rect;
    let hovered_now = resp.response.hovered() || ui.rect_contains_pointer(rect);
    ui.ctx().data_mut(|d| d.insert_temp(id, hovered_now));

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if hover_t > 0.03 && rect.width() > 28.0 {
            let rim_a = (hover_t * if is_light_theme() { 45.0 } else { 32.0 }) as u8;
            painter.line_segment(
                [
                    Pos2::new(rect.left() + 10.0, rect.top() + 1.0),
                    Pos2::new(rect.right() - 10.0, rect.top() + 1.0),
                ],
                Stroke::new(1.0_f32, with_alpha(PURE_WHITE, rim_a)),
            );
        }
        let bar_strength = ease_out_cubic(select_t.max(hover_t * 0.55));
        if bar_strength > 0.04 && rect.height() >= 28.0 {
            let max_bar_h = (rect.height() - 14.0).clamp(10.0, 38.0);
            let bar_h = max_bar_h * bar_strength;
            let bar_col = lerp_color(border_strong(), accent(), select_t.max(hover_t * 0.7));
            let bar_rect = Rect::from_center_size(
                Pos2::new(rect.left() + 2.2, rect.center().y),
                Vec2::new(3.0, bar_h),
            );
            painter.rect_filled(bar_rect, Rounding::same(1.5), bar_col);
        }
        if let Some(ft) = flash_t {
            let inv = 1.0 - ft;
            let flash_a = (inv * inv * 135.0) as u8;
            if flash_a > 1 {
                painter.rect_stroke(
                    rect.expand(ease_out_cubic(ft) * 3.0),
                    Rounding::same(11.0),
                    Stroke::new(1.4_f32, with_alpha(accent(), flash_a)),
                );
                paint_shimmer_sweep(
                    painter,
                    rect,
                    Rounding::same(10.0),
                    ft,
                    accent(),
                    (inv * 28.0) as u8,
                );
            }
        }
    }
    resp
}

/// Renders the hero spotlight card with a subtle loader-tinted ambient corner glow.
#[allow(dead_code)]
pub fn hero_spotlight_card<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    loader_accent: Color32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let prev_hover = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let hover_t = ui.ctx().animate_bool_with_time(id, prev_hover, 0.16);
    if hover_t > 0.01 && hover_t < 0.99 {
        ui.ctx().request_repaint();
    }

    let bg = lerp_color(
        bg_card_active(),
        lerp_color(bg_card(), accent(), 0.18),
        hover_t,
    );
    let border = lerp_color(with_alpha(accent(), 185), accent_hover(), hover_t);

    let resp = egui::Frame::none()
        .fill(bg)
        .stroke(Stroke::new(1.1_f32, border))
        .rounding(Rounding::same(12.0))
        .inner_margin(Margin::same(14.0))
        .show(ui, add_contents);

    let rect = resp.response.rect;
    let hovered_now = resp.response.hovered() || ui.rect_contains_pointer(rect);
    ui.ctx().data_mut(|d| d.insert_temp(id, hovered_now));

    if ui.is_rect_visible(rect) && rect.height() > 28.0 {
        let painter = ui.painter();
        let bar_h = (rect.height() - 22.0).max(16.0);
        let bar_rect = Rect::from_center_size(
            Pos2::new(rect.left() + 3.0, rect.center().y),
            Vec2::new(3.5, bar_h),
        );
        painter.rect_filled(bar_rect, Rounding::same(1.75), loader_accent);
    }
    resp
}

/// Draws a pill badge (`Vanilla`, `Fabric`, `Microsoft`, `Installed`).
pub fn badge(ui: &mut egui::Ui, text: &str, highlighted: bool) {
    let (fill, border, fg) = if highlighted {
        (accent_dim(), accent(), accent_text())
    } else {
        (bg_elevated(), border_subtle(), text_secondary())
    };
    egui::Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, border))
        .rounding(Rounding::same(5.5))
        .inner_margin(Margin::symmetric(7.0, 2.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(10.5).strong().color(fg));
        });
}

/// Draws an exact-height pill badge (used when aligning badges inline with 28–34px controls).
pub fn badge_sized(
    ui: &mut egui::Ui,
    text: &str,
    highlighted: bool,
    height: f32,
    max_width: Option<f32>,
) -> egui::Response {
    let (fill, border, fg) = if highlighted {
        (accent_dim(), accent(), accent_text())
    } else {
        (bg_elevated(), border_subtle(), text_secondary())
    };
    draw_sized_pill_badge(ui, text, fill, border, fg, height, max_width)
}

/// Draws a color-coded pill badge with an exact height matching adjacent buttons/inputs.
pub fn loader_badge_sized(
    ui: &mut egui::Ui,
    kind: crate::models::LoaderKind,
    text: &str,
    height: f32,
) -> egui::Response {
    let (fill, border, fg) = loader_colors(kind);
    draw_sized_pill_badge(ui, text, fill, border, fg, height, None)
}

fn draw_sized_pill_badge(
    ui: &mut egui::Ui,
    text: &str,
    fill: Color32,
    border: Color32,
    fg: Color32,
    height: f32,
    max_width: Option<f32>,
) -> egui::Response {
    let font_sz = if height >= 28.0 { 12.0 } else { 10.5 };
    let pad_x = if height >= 28.0 { 10.0 } else { 7.0 };
    let font = FontId::new(font_sz, FontFamily::Proportional);
    let full_galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font.clone(), fg);
    let desired_w = (full_galley.size().x + pad_x * 2.0).max(height);
    let w = max_width.map_or(desired_w, |mw| desired_w.min(mw));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, height), Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let rounding = Rounding::same(if height >= 26.0 { 7.5 } else { 5.5 });
        painter.rect(rect, rounding, fill, Stroke::new(1.0_f32, border));
        let inner_w = (rect.width() - pad_x * 2.0).max(8.0);
        let galley = if full_galley.size().x <= inner_w {
            full_galley
        } else {
            let mut clipped = String::new();
            for ch in text.chars() {
                let mut cand = clipped.clone();
                cand.push(ch);
                cand.push('…');
                let g = painter.layout_no_wrap(cand, font.clone(), fg);
                if g.size().x > inner_w {
                    break;
                }
                clipped.push(ch);
            }
            clipped.push('…');
            painter.layout_no_wrap(clipped, font, fg)
        };
        painter.galley(
            Pos2::new(
                rect.center().x - galley.size().x * 0.5,
                rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            fg,
        );
    }
    resp
}

/// Returns signature `(bg_fill, border_color, text_color)` for each mod loader kind.
pub fn loader_colors(kind: crate::models::LoaderKind) -> (Color32, Color32, Color32) {
    use crate::models::LoaderKind;
    let light = is_light_theme();
    let adapt = |dark_bg: Color32, border: Color32, dark_fg: Color32| {
        if light {
            (
                lerp_color(bg_card(), border, 0.14),
                border,
                lerp_color(border, Color32::BLACK, 0.42),
            )
        } else {
            (dark_bg, border, dark_fg)
        }
    };
    match kind {
        LoaderKind::Vanilla => adapt(
            Color32::from_rgb(24, 40, 58),
            Color32::from_rgb(76, 142, 219),
            Color32::from_rgb(162, 210, 255),
        ),
        LoaderKind::Fabric => adapt(
            Color32::from_rgb(54, 45, 30),
            Color32::from_rgb(214, 174, 108),
            Color32::from_rgb(248, 218, 166),
        ),
        LoaderKind::Quilt => adapt(
            Color32::from_rgb(47, 30, 62),
            Color32::from_rgb(176, 118, 224),
            Color32::from_rgb(222, 186, 252),
        ),
        LoaderKind::Forge => adapt(
            Color32::from_rgb(58, 36, 24),
            Color32::from_rgb(228, 132, 64),
            Color32::from_rgb(252, 188, 138),
        ),
        LoaderKind::NeoForge => adapt(
            Color32::from_rgb(60, 31, 27),
            Color32::from_rgb(242, 104, 78),
            Color32::from_rgb(255, 176, 158),
        ),
    }
}

/// Draws a color-coded pill badge for a specific `LoaderKind`.
pub fn loader_badge(ui: &mut egui::Ui, kind: crate::models::LoaderKind, text: &str) {
    let (fill, border, fg) = loader_colors(kind);
    egui::Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, border))
        .rounding(Rounding::same(5.5))
        .inner_margin(Margin::symmetric(7.0, 2.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(10.5).strong().color(fg));
        });
}

/// Returns signature `(bg_fill, border_color, text_color)` for each `ServerSoftware`.
pub fn server_software_colors(
    software: crate::models::ServerSoftware,
) -> (Color32, Color32, Color32) {
    use crate::models::{LoaderKind, ServerSoftware};
    let light = is_light_theme();
    let adapt = |dark_bg: Color32, border: Color32, dark_fg: Color32| {
        if light {
            (
                lerp_color(bg_card(), border, 0.14),
                border,
                lerp_color(border, Color32::BLACK, 0.42),
            )
        } else {
            (dark_bg, border, dark_fg)
        }
    };
    match software {
        ServerSoftware::Vanilla => loader_colors(LoaderKind::Vanilla),
        ServerSoftware::Fabric => loader_colors(LoaderKind::Fabric),
        ServerSoftware::Quilt => loader_colors(LoaderKind::Quilt),
        ServerSoftware::Forge => loader_colors(LoaderKind::Forge),
        ServerSoftware::NeoForge => loader_colors(LoaderKind::NeoForge),
        ServerSoftware::Paper => adapt(
            Color32::from_rgb(24, 46, 58),
            Color32::from_rgb(72, 184, 218),
            Color32::from_rgb(168, 232, 252),
        ),
        ServerSoftware::Purpur => adapt(
            Color32::from_rgb(48, 28, 60),
            Color32::from_rgb(184, 108, 224),
            Color32::from_rgb(232, 188, 255),
        ),
        ServerSoftware::Folia => adapt(
            Color32::from_rgb(24, 52, 40),
            Color32::from_rgb(64, 198, 134),
            Color32::from_rgb(164, 244, 204),
        ),
        ServerSoftware::Arclight => adapt(
            Color32::from_rgb(56, 42, 22),
            Color32::from_rgb(232, 168, 60),
            Color32::from_rgb(255, 220, 150),
        ),
    }
}

/// Draws a color-coded pill badge for a specific `ServerSoftware`.
pub fn server_software_badge(ui: &mut egui::Ui, software: crate::models::ServerSoftware) {
    let (fill, border, fg) = server_software_colors(software);
    egui::Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, border))
        .rounding(Rounding::same(5.5))
        .inner_margin(Margin::symmetric(7.0, 2.0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(software.label())
                    .size(10.5)
                    .strong()
                    .color(fg),
            );
        });
}

/// Draws a stylized server emblem with software-tinted colors and running indicator.
pub fn draw_server_avatar(
    painter: &egui::Painter,
    rect: Rect,
    name: &str,
    software: crate::models::ServerSoftware,
    selected: bool,
    is_running: bool,
) {
    let (sw_bg, sw_border, sw_fg) = server_software_colors(software);
    let bg = if selected {
        lerp_color(sw_bg, accent_dim(), 0.48)
    } else {
        sw_bg
    };
    let border = if is_running || selected {
        accent()
    } else {
        sw_border
    };
    let rounding = Rounding::same((rect.width() * 0.22).clamp(6.0, 12.0));
    painter.rect_filled(rect, rounding, bg);
    painter.rect_stroke(rect, rounding, Stroke::new(1.2_f32, border));

    let initial = name
        .chars()
        .find(|c| c.is_alphanumeric())
        .or_else(|| name.chars().next())
        .unwrap_or('S')
        .to_uppercase()
        .to_string();
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initial,
        FontId::new(
            (rect.height() * 0.44).clamp(12.0, 24.0),
            FontFamily::Proportional,
        ),
        if selected { accent_text() } else { sw_fg },
    );

    if is_running && rect.width() >= 28.0 {
        let dot_center = Pos2::new(rect.right() - 5.5, rect.bottom() - 5.5);
        painter.circle_filled(dot_center, 4.6, bg_card());
        painter.circle_filled(dot_center, 3.1, accent());
    }
}

/// Draws a stylized instance emblem with loader-tinted accent() and running indicator.
pub fn draw_instance_avatar(
    painter: &egui::Painter,
    rect: Rect,
    name: &str,
    kind: crate::models::LoaderKind,
    selected: bool,
    is_running: bool,
) {
    let (loader_bg, loader_border, loader_fg) = loader_colors(kind);
    let bg = if selected {
        lerp_color(loader_bg, accent_dim(), 0.48)
    } else {
        loader_bg
    };
    let border = if is_running || selected {
        accent()
    } else {
        loader_border
    };
    let rounding = Rounding::same((rect.width() * 0.22).clamp(6.0, 12.0));
    painter.rect_filled(rect, rounding, bg);
    painter.rect_stroke(rect, rounding, Stroke::new(1.2_f32, border));

    let initial = name
        .chars()
        .find(|c| c.is_alphanumeric())
        .or_else(|| name.chars().next())
        .unwrap_or('M')
        .to_uppercase()
        .to_string();
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initial,
        FontId::new(
            (rect.height() * 0.44).clamp(12.0, 24.0),
            FontFamily::Proportional,
        ),
        if selected { accent_text() } else { loader_fg },
    );

    if is_running && rect.width() >= 28.0 {
        let dot_center = Pos2::new(rect.right() - 5.5, rect.bottom() - 5.5);
        painter.circle_filled(dot_center, 4.6, bg_card());
        painter.circle_filled(dot_center, 3.1, accent());
    }
}

/// Draws an icon placeholder box with an optional animated shimmer while downloading.
pub fn draw_icon_placeholder(
    ui: &mut egui::Ui,
    rect: Rect,
    rounding: f32,
    title: &str,
    loading: bool,
) {
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(rounding), bg_input());
    if loading {
        let t = (ui.input(|i| i.time) as f32 * 1.35).fract();
        let band_w = (rect.width() * 0.55).max(10.0);
        let sweep_x = rect.left() + t * (rect.width() + band_w) - band_w * 0.5;
        let shimmer_rect = Rect::from_min_max(
            Pos2::new(sweep_x.max(rect.left()), rect.top()),
            Pos2::new((sweep_x + band_w * 0.5).min(rect.right()), rect.bottom()),
        );
        if shimmer_rect.width() > 1.0 {
            painter.rect_filled(
                shimmer_rect,
                Rounding::same(rounding),
                Color32::from_rgba_unmultiplied(47, 54, 64, 150),
            );
        }
        ui.ctx().request_repaint();
    }
    painter.rect_stroke(
        rect,
        Rounding::same(rounding),
        Stroke::new(1.0_f32, border_subtle()),
    );
    let initial = title
        .chars()
        .find(|c| c.is_alphanumeric())
        .or_else(|| title.chars().next())
        .unwrap_or('M')
        .to_uppercase()
        .to_string();
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initial,
        FontId::new(
            (rect.height() * 0.42).clamp(12.0, 18.0),
            FontFamily::Proportional,
        ),
        text_secondary(),
    );
}

/// Draws an emerald solid pill badge (used for device codes or primary tags) with dark readable text.
pub fn solid_accent_badge(ui: &mut egui::Ui, text: &str) {
    egui::Frame::none()
        .fill(accent())
        .stroke(Stroke::new(1.0_f32, accent()))
        .rounding(Rounding::same(6.0))
        .inner_margin(Margin::symmetric(10.0, 4.0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .monospace()
                    .size(14.0)
                    .strong()
                    .color(accent_dark()),
            );
        });
}

/// Draws an animated Modrinth-green 4-square orbital loader.
pub fn mono_spinner(ui: &mut egui::Ui, size: f32) {
    mono_spinner_colored(ui, size, accent());
}

/// Draws an orbital 4-square spinner with a custom color.
pub fn mono_spinner_colored(ui: &mut egui::Ui, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let time = ui.input(|i| i.time) as f32;
    let painter = ui.painter();
    let center = rect.center();
    let r = size * 0.34;
    let sq = (size * 0.22).max(2.6);

    for idx in 0..4 {
        let phase = time * 4.2 + idx as f32 * std::f32::consts::FRAC_PI_2;
        let pos = Pos2::new(center.x + phase.cos() * r, center.y + phase.sin() * r);
        let pulse = 0.55 + 0.45 * ((time * 5.0 - idx as f32 * 0.9).sin() * 0.5 + 0.5);
        let alpha = (pulse * 255.0) as u8;
        painter.rect_filled(
            Rect::from_center_size(pos, Vec2::splat(sq * (0.75 + 0.25 * pulse))),
            Rounding::same(1.5),
            with_alpha(color, alpha),
        );
    }
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(33));
}

/// Draws an animated emerald progress bar with smooth interpolation, shimmer sweep, and % indicator.
pub fn animated_progress_bar(
    ui: &mut egui::Ui,
    id_salt: &str,
    target_fraction: f32,
    label: &str,
    active: bool,
) {
    let ctx = ui.ctx();
    let clamped = target_fraction.clamp(0.0, 1.0);
    let smooth = ctx.animate_value_with_time(ui.make_persistent_id(id_salt), clamped, 0.18);
    if (smooth - clamped).abs() > 0.002 || active {
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }

    let desired_w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(desired_w, 22.0), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();

    painter.rect_filled(rect, Rounding::same(6.5), bg_input());
    painter.rect_stroke(
        rect,
        Rounding::same(6.5),
        Stroke::new(1.0_f32, border_subtle()),
    );

    let fill_w = (rect.width() * smooth).max(if active { 20.0 } else { 0.0 });
    if fill_w > 0.5 {
        let fill_rect =
            Rect::from_min_size(rect.min, Vec2::new(fill_w.min(rect.width()), rect.height()));
        painter.rect_filled(fill_rect, Rounding::same(6.5), accent());
        if active && fill_rect.width() > 10.0 {
            let phase = (ui.input(|i| i.time) as f32 * 0.95).fract();
            paint_shimmer_sweep(
                painter,
                fill_rect,
                Rounding::same(6.5),
                phase,
                PURE_WHITE,
                72,
            );
            let tip_rect = Rect::from_min_max(
                Pos2::new(
                    (fill_rect.right() - 3.5).max(fill_rect.left()),
                    fill_rect.top() + 1.5,
                ),
                Pos2::new(fill_rect.right(), fill_rect.bottom() - 1.5),
            );
            painter.rect_filled(tip_rect, Rounding::same(2.0), with_alpha(PURE_WHITE, 145));
        } else if !active && smooth >= 0.985 {
            let p = pulse_01(ui.input(|i| i.time), 5.0);
            painter.rect_stroke(
                rect,
                Rounding::same(6.5),
                Stroke::new(
                    1.2_f32,
                    with_alpha(accent_hover(), (110.0 + p * 85.0) as u8),
                ),
            );
        }
    }

    let pct_text = format!("{}%", (smooth * 100.0).round() as u32);
    let text_color = if smooth > 0.48 {
        accent_dark()
    } else {
        text_primary()
    };
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::new(11.5, FontFamily::Proportional),
        text_color,
    );
    if rect.width() >= 180.0 {
        let pct_color = if smooth > 0.92 {
            accent_dark()
        } else {
            text_secondary()
        };
        painter.text(
            Pos2::new(rect.right() - 10.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            pct_text,
            FontId::new(11.0, FontFamily::Monospace),
            pct_color,
        );
    }
}

/// Draws a compact animated horizontal ratio/gauge bar.
#[allow(dead_code)]
pub fn draw_mini_gauge(ui: &mut egui::Ui, id_salt: &str, fraction: f32, width: f32, height: f32) {
    let smooth = ui.ctx().animate_value_with_time(
        ui.make_persistent_id(("mini_gauge", id_salt)),
        fraction.clamp(0.0, 1.0),
        0.2,
    );
    if (smooth - fraction.clamp(0.0, 1.0)).abs() > 0.003 {
        ui.ctx().request_repaint();
    }
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let r = Rounding::same(height * 0.5);
    let painter = ui.painter();
    painter.rect_filled(rect, r, bg_input());
    painter.rect_stroke(rect, r, Stroke::new(1.0_f32, border_subtle()));
    let fill_w = (rect.width() * smooth).clamp(0.0, rect.width());
    if fill_w > 1.0 {
        let fill_rect = Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
        painter.rect_filled(fill_rect, r, accent());
        if (smooth - fraction.clamp(0.0, 1.0)).abs() > 0.005 {
            let cap = Rect::from_min_max(
                Pos2::new(
                    (fill_rect.right() - 3.0).max(fill_rect.left()),
                    fill_rect.top(),
                ),
                Pos2::new(fill_rect.right(), fill_rect.bottom()),
            );
            painter.rect_filled(cap, r, with_alpha(PURE_WHITE, 165));
        }
    }
}

/// Visual RAM allocation meter showing Min (-Xms) and Max (-Xmx) up to 16 GB or 32 GB.
pub fn draw_ram_allocation_bar(ui: &mut egui::Ui, min_mb: u32, max_mb: u32, width: f32) {
    let cap = if max_mb > 16384 {
        32768.0_f32
    } else {
        16384.0_f32
    };
    let min_f = (min_mb as f32 / cap).clamp(0.0, 1.0);
    let max_f = (max_mb as f32 / cap).clamp(min_f, 1.0);
    let smooth_min =
        ui.ctx()
            .animate_value_with_time(ui.make_persistent_id("ram_bar_min"), min_f, 0.16);
    let smooth_max =
        ui.ctx()
            .animate_value_with_time(ui.make_persistent_id("ram_bar_max"), max_f, 0.16);
    if (smooth_min - min_f).abs() > 0.003 || (smooth_max - max_f).abs() > 0.003 {
        ui.ctx().request_repaint();
    }

    let (rect, _) = ui.allocate_exact_size(Vec2::new(width.max(80.0), 10.0), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    let r = Rounding::same(5.0);
    painter.rect_filled(rect, r, Color32::from_rgb(24, 28, 35));
    painter.rect_stroke(rect, r, Stroke::new(1.0_f32, border_strong()));

    for frac in [0.25_f32, 0.5, 0.75] {
        let x = rect.left() + rect.width() * frac;
        painter.line_segment(
            [
                Pos2::new(x, rect.top() + 1.5),
                Pos2::new(x, rect.bottom() - 1.5),
            ],
            Stroke::new(1.0_f32, border_strong()),
        );
    }

    let max_w = (rect.width() * smooth_max).clamp(0.0, rect.width());
    if max_w > 2.0 {
        let max_col = if max_mb >= 12288 {
            WARNING
        } else {
            accent_dim()
        };
        let max_rect = Rect::from_min_size(rect.min, Vec2::new(max_w, rect.height()));
        painter.rect_filled(max_rect, r, with_alpha(max_col, 220));
        if (smooth_max - max_f).abs() > 0.005 {
            let cap = Rect::from_min_max(
                Pos2::new(
                    (max_rect.right() - 3.0).max(max_rect.left()),
                    max_rect.top(),
                ),
                Pos2::new(max_rect.right(), max_rect.bottom()),
            );
            painter.rect_filled(cap, r, with_alpha(PURE_WHITE, 140));
        }
    }
    let min_w = (rect.width() * smooth_min).clamp(0.0, rect.width());
    if min_w > 2.0 {
        let min_rect = Rect::from_min_size(rect.min, Vec2::new(min_w, rect.height()));
        painter.rect_filled(min_rect, r, accent());
        if (smooth_min - min_f).abs() > 0.005 {
            let cap = Rect::from_min_max(
                Pos2::new(
                    (min_rect.right() - 3.0).max(min_rect.left()),
                    min_rect.top(),
                ),
                Pos2::new(min_rect.right(), min_rect.bottom()),
            );
            painter.rect_filled(cap, r, with_alpha(PURE_WHITE, 175));
        }
    }
}

/// Renders a high-contrast integer slider with customizable step, suffix, and change glow.
pub fn ui_int_slider(
    ui: &mut egui::Ui,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
    step: f64,
    suffix: &str,
    slider_width: f32,
) -> egui::Response {
    let resp = ui
        .scope(|ui| {
            ui.style_mut().spacing.slider_width = slider_width;
            ui.style_mut().spacing.slider_rail_height = 8.0;
            let visuals = ui.visuals_mut();
            visuals.slider_trailing_fill = true;
            visuals.widgets.inactive.bg_fill = border_subtle();
            visuals.widgets.inactive.fg_stroke = Stroke::new(1.5_f32, accent());
            visuals.widgets.inactive.rounding = Rounding::same(4.0);
            visuals.widgets.hovered.bg_fill = border_strong();
            visuals.widgets.hovered.fg_stroke = Stroke::new(1.5_f32, text_primary());
            visuals.widgets.active.bg_fill = accent();
            visuals.selection.bg_fill = accent();
            let mut slider = egui::Slider::new(value, range).step_by(step);
            if !suffix.is_empty() {
                slider = slider.suffix(suffix);
            }
            ui.add(slider)
        })
        .inner;
    let pulse_id = resp.id.with("slider_change_t");
    let now = ui.input(|i| i.time);
    if resp.changed() {
        ui.ctx().data_mut(|d| d.insert_temp(pulse_id, now));
        ui.ctx().request_repaint();
    }
    if let Some(t0) = ui.ctx().data(|d| d.get_temp::<f64>(pulse_id)) {
        let age = (now - t0) as f32;
        if (0.0..0.32).contains(&age) {
            ui.ctx().request_repaint();
            let frac = age / 0.32;
            let alpha = ((1.0 - frac) * 140.0) as u8;
            ui.painter().rect_stroke(
                resp.rect.expand(ease_out_cubic(frac) * 2.5),
                Rounding::same(6.0),
                Stroke::new(1.2_f32, with_alpha(accent(), alpha)),
            );
        }
    }
    resp
}

/// Renders a high-contrast slider where the full track bar remains clearly visible on card surfaces.
pub fn ui_ram_slider(
    ui: &mut egui::Ui,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
    slider_width: f32,
) -> egui::Response {
    ui_int_slider(ui, value, range, 256.0, " MB", slider_width)
}

/// Draws an animated skeleton shimmer card while Modrinth search results are loading.
pub fn draw_skeleton_card(ui: &mut egui::Ui, inner_w: f32) {
    card_frame(false, 0.0).show(ui, |ui| {
        ui.set_min_width(inner_w);
        ui.set_max_width(inner_w);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(inner_w, 68.0), Sense::hover());
        if !ui.is_rect_visible(rect) {
            return;
        }
        let painter = ui.painter();
        let t = (ui.input(|i| i.time) as f32 * 1.25).fract();
        let pulse = (ui.input(|i| i.time) as f32 * 3.2).sin() * 0.5 + 0.5;
        let base_block = lerp_color(bg_elevated(), bg_card_hover(), pulse);

        let icon_r = Rect::from_min_size(rect.min, Vec2::splat(44.0));
        painter.rect_filled(icon_r, Rounding::same(9.0), base_block);

        let title_r = Rect::from_min_size(
            Pos2::new(rect.left() + 56.0, rect.top() + 4.0),
            Vec2::new((inner_w * 0.38).clamp(80.0, 220.0), 14.0),
        );
        painter.rect_filled(title_r, Rounding::same(4.0), base_block);

        let sub_r = Rect::from_min_size(
            Pos2::new(rect.left() + 56.0, rect.top() + 24.0),
            Vec2::new((inner_w * 0.24).clamp(60.0, 150.0), 10.0),
        );
        painter.rect_filled(sub_r, Rounding::same(4.0), base_block);

        let desc_r = Rect::from_min_size(
            Pos2::new(rect.left(), rect.top() + 52.0),
            Vec2::new((inner_w * 0.80).max(100.0), 11.0),
        );
        painter.rect_filled(desc_r, Rounding::same(4.0), base_block);

        let sweep_w = 70.0;
        let sweep_x = rect.left() + t * (rect.width() + sweep_w) - sweep_w * 0.5;
        let shimmer = Rect::from_min_max(
            Pos2::new(sweep_x.max(rect.left()), rect.top()),
            Pos2::new((sweep_x + sweep_w * 0.5).min(rect.right()), rect.bottom()),
        );
        if shimmer.width() > 2.0 {
            painter.rect_filled(
                shimmer,
                Rounding::same(6.0),
                Color32::from_rgba_unmultiplied(255, 255, 255, 14),
            );
        }
        ui.ctx().request_repaint();
    });
}

/// Primary Modrinth-style CTA button ("PLAY" / "LOADING..." / "STOP")
/// with crisp vector Play/Stop icon, hover glow, press physics, and pulse halo.
pub fn primary_action_button(
    ui: &mut egui::Ui,
    id_salt: &str,
    label: &str,
    icon: Option<Icon>,
    enabled: bool,
    is_stop_style: bool,
    pulsing: bool,
    size: Vec2,
) -> egui::Response {
    let btn_id = ui.make_persistent_id(("primary_cta", id_salt));
    let (rect, resp) = ui.allocate_exact_size(
        size,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let hover_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(id_salt),
        enabled && resp.hovered(),
        0.14,
    );
    let press_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id((id_salt, "press")),
        enabled && resp.is_pointer_button_down_on(),
        0.07,
    );
    if (hover_t > 0.01 && hover_t < 0.99) || (press_t > 0.01 && press_t < 0.99) || pulsing {
        ui.ctx().request_repaint();
    }
    let click_info = track_click_anim(
        ui.ctx(),
        btn_id,
        rect,
        enabled && resp.clicked(),
        resp.interact_pointer_pos(),
        0.48,
    );
    let click_t = click_info.map(|(t, _)| t);

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let time = ui.input(|i| i.time);

        let (bg, border, fg) = if !enabled {
            if pulsing {
                (
                    lerp_color(bg_card(), accent_dim(), 0.35 + pulse_01(time, 3.6) * 0.25),
                    with_alpha(accent(), 145),
                    text_primary(),
                )
            } else {
                (bg_card(), border_subtle(), text_muted())
            }
        } else if is_stop_style {
            let bg = lerp_color(DANGER, DANGER_HOVER, hover_t);
            (bg, DANGER_HOVER, PURE_WHITE)
        } else {
            let bg = lerp_color(accent(), accent_hover(), hover_t);
            (bg, accent_hover(), accent_dark())
        };

        let pop = click_t.map_or(0.0, |t| {
            ((1.0 - t) * std::f32::consts::PI).sin().max(0.0) * 0.8
        });
        let draw_rect = rect.shrink(press_t * 1.1).expand(pop);
        let rounding = Rounding::same(8.5);

        if pulsing {
            let wave = (time as f32 * 0.95).fract();
            let halo_col = if is_stop_style { DANGER } else { accent() };
            let expand = wave * 7.0;
            let halo_alpha = ((1.0 - wave) * (1.0 - wave) * 125.0).round() as u8;
            if halo_alpha > 1 {
                painter.rect_stroke(
                    draw_rect.expand(expand),
                    Rounding::same(8.5 + expand * 0.4),
                    Stroke::new(1.5_f32, with_alpha(halo_col, halo_alpha)),
                );
            }
        } else if enabled && hover_t > 0.05 {
            let glow_col = if is_stop_style { DANGER } else { accent() };
            painter.rect_stroke(
                draw_rect.expand(hover_t * 2.5),
                Rounding::same(10.0),
                Stroke::new(1.3_f32, with_alpha(glow_col, (hover_t * 95.0) as u8)),
            );
        }

        painter.rect_filled(draw_rect, rounding, bg);
        if !enabled && pulsing {
            paint_shimmer_sweep(
                painter,
                draw_rect,
                rounding,
                (time as f32 * 1.1).fract(),
                PURE_WHITE,
                52,
            );
        } else if enabled && hover_t > 0.01 && hover_t < 0.99 {
            let sheen_alpha = ((hover_t * std::f32::consts::PI).sin() * 48.0).round() as u8;
            paint_shimmer_sweep(
                painter,
                draw_rect,
                rounding,
                hover_t,
                PURE_WHITE,
                sheen_alpha,
            );
        }

        if enabled && draw_rect.width() > 20.0 {
            painter.line_segment(
                [
                    Pos2::new(draw_rect.left() + 8.0, draw_rect.top() + 1.2),
                    Pos2::new(draw_rect.right() - 8.0, draw_rect.top() + 1.2),
                ],
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 90)),
            );
        }
        painter.rect_stroke(draw_rect, rounding, Stroke::new(1.2_f32, border));

        if let Some((ct, origin)) = click_info {
            let ripple_col = if is_stop_style {
                PURE_WHITE
            } else {
                accent_dark()
            };
            paint_click_ripple(painter, draw_rect, rounding, origin, ct, ripple_col, true);
        }

        let galley = painter.layout_no_wrap(
            label.to_string(),
            FontId::new(14.0, FontFamily::Proportional),
            fg,
        );
        let show_loading_dots = !enabled && pulsing && icon.is_none();
        let icon_sz = 13.0;
        let dots_w = 18.0;
        let gap = if icon.is_some() || show_loading_dots {
            7.0
        } else {
            0.0
        };
        let lead_w = if icon.is_some() {
            icon_sz
        } else if show_loading_dots {
            dots_w
        } else {
            0.0
        };
        let total_w = lead_w + gap + galley.size().x;
        let mut x = draw_rect.center().x - total_w * 0.5;
        if let Some(ic) = icon {
            let ic_rect = Rect::from_center_size(
                Pos2::new(x + icon_sz * 0.5, draw_rect.center().y),
                Vec2::splat(icon_sz),
            );
            let ic_anim = compute_icon_anim(ic, hover_t, press_t, click_t);
            draw_icon_animated(painter, ic_rect, ic, fg, ic_anim);
            x += icon_sz + gap;
        } else if show_loading_dots {
            for d_idx in 0..3 {
                let phase = (time as f32 * 5.2 - d_idx as f32 * 0.7).sin() * 0.5 + 0.5;
                let dot_pos = Pos2::new(
                    x + 3.0 + d_idx as f32 * 6.0,
                    draw_rect.center().y - phase * 2.5 + 1.0,
                );
                painter.circle_filled(
                    dot_pos,
                    1.8 + phase * 0.6,
                    with_alpha(accent(), (140.0 + phase * 115.0) as u8),
                );
            }
            x += dots_w + gap;
        }
        painter.galley(
            Pos2::new(x, draw_rect.center().y - galley.size().y * 0.5),
            galley,
            fg,
        );
    }

    if enabled && resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Animated Modrinth-style pill switch for enabling/disabling installed mods & packs.
pub fn mod_toggle_switch(ui: &mut egui::Ui, id_salt: &str, on: bool) -> egui::Response {
    let sw_id = ui.make_persistent_id(("switch_base", id_salt));
    let desired = Vec2::new(38.0, 20.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    let anim_t =
        ui.ctx()
            .animate_bool_with_time(ui.make_persistent_id(("switch", id_salt)), on, 0.16);
    let hover_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("switch_hov", id_salt)),
        resp.hovered(),
        0.11,
    );
    let press_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("switch_prs", id_salt)),
        resp.is_pointer_button_down_on(),
        0.07,
    );
    if (anim_t > 0.01 && anim_t < 0.99)
        || (hover_t > 0.01 && hover_t < 0.99)
        || (press_t > 0.01 && press_t < 0.99)
    {
        ui.ctx().request_repaint();
    }
    let click_info = track_click_anim(
        ui.ctx(),
        sw_id,
        rect,
        resp.clicked(),
        resp.interact_pointer_pos(),
        0.38,
    );

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let draw_rect = rect.shrink(press_t * 0.7);
        let eased_t = ease_out_cubic(anim_t);
        let bg = lerp_color(
            lerp_color(bg_input(), bg_card_hover(), hover_t * 0.5),
            lerp_color(accent(), accent_hover(), hover_t),
            eased_t,
        );
        let border = lerp_color(border_strong(), accent_hover(), eased_t.max(hover_t * 0.4));
        let rounding = Rounding::same(draw_rect.height() * 0.5);
        painter.rect_filled(draw_rect, rounding, bg);
        painter.rect_stroke(draw_rect, rounding, Stroke::new(1.0_f32, border));

        if let Some((ct, _)) = click_info {
            let eased_c = crate::ui::anim::ease_out_quint(ct);
            let ring_a = ((1.0 - ct) * (1.0 - ct) * 165.0).round() as u8;
            if ring_a > 1 {
                let ring_col = if on { accent() } else { border_strong() };
                painter.rect_stroke(
                    draw_rect.expand(eased_c * 4.5),
                    Rounding::same((draw_rect.height() + eased_c * 9.0) * 0.5),
                    Stroke::new(1.3_f32, with_alpha(ring_col, ring_a)),
                );
            }
        }

        let r = draw_rect.height() * (0.36 + hover_t * 0.03);
        let left_x = draw_rect.left() + r + 3.0;
        let right_x = draw_rect.right() - r - 3.0;
        let knob_x = left_x + (right_x - left_x) * eased_t;
        let stretch = (anim_t * std::f32::consts::PI).sin().max(0.0) * 4.2;
        let knob_col = lerp_color(
            lerp_color(text_secondary(), text_primary(), hover_t),
            accent_dark(),
            eased_t,
        );
        let knob_rect = Rect::from_center_size(
            Pos2::new(knob_x, draw_rect.center().y),
            Vec2::new(r * 2.0 + stretch, (r * 2.0 - stretch * 0.22).max(4.0)),
        );
        painter.rect_filled(
            knob_rect,
            Rounding::same(knob_rect.height() * 0.5),
            knob_col,
        );
    }

    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Draws a live breathing radar pulse indicator dot for active instances/servers/status.
pub fn draw_running_pulse_dot(
    ui: &egui::Ui,
    center: Pos2,
    radius: f32,
    color: Color32,
    bg: Color32,
) {
    let time = ui.input(|i| i.time);
    let wave = (time as f32 * 0.95).fract();
    let painter = ui.painter();
    let ring_r = radius + wave * (radius * 1.45);
    let ring_a = ((1.0 - wave) * (1.0 - wave) * 185.0).round() as u8;
    if ring_a > 2 {
        painter.circle_stroke(
            center,
            ring_r,
            Stroke::new(1.2_f32, with_alpha(color, ring_a)),
        );
    }
    painter.circle_filled(center, radius + 1.4, bg);
    painter.circle_filled(center, radius, color);
    ui.ctx().request_repaint();
}

/// Draws a custom image avatar for an instance with rounded corners, border, and running indicator.
pub fn draw_instance_image_avatar(
    painter: &egui::Painter,
    rect: Rect,
    tex_id: egui::TextureId,
    selected: bool,
    is_running: bool,
) {
    let rounding = Rounding::same((rect.width() * 0.22).clamp(6.0, 12.0));
    painter.image(
        tex_id,
        rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    let border = if is_running || selected {
        accent()
    } else {
        border_subtle()
    };
    painter.rect_stroke(rect, rounding, Stroke::new(1.2_f32, border));

    if rect.width() >= 28.0 {
        let dot_center = Pos2::new(rect.right() - 5.5, rect.bottom() - 5.5);
        let dot_col = if is_running {
            accent()
        } else if selected {
            accent_hover()
        } else {
            border_strong()
        };
        painter.circle_filled(dot_center, 3.8, bg_void());
        painter.circle_filled(dot_center, 2.5, dot_col);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemFont {
    pub name: String,
    pub path: String,
}

/// Scans standard OS font directories for `.ttf` and `.otf` UI fonts.
pub fn discover_system_fonts() -> Vec<SystemFont> {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    #[cfg(target_os = "linux")]
    {
        dirs.push(std::path::PathBuf::from("/usr/share/fonts"));
        dirs.push(std::path::PathBuf::from("/usr/local/share/fonts"));
        if let Some(home) = dirs::home_dir() {
            dirs.push(home.join(".local/share/fonts"));
            dirs.push(home.join(".fonts"));
        }
    }
    #[cfg(target_os = "windows")]
    {
        dirs.push(std::path::PathBuf::from(r"C:\Windows\Fonts"));
        if let Some(local) = dirs::data_local_dir() {
            dirs.push(local.join("Microsoft/Windows/Fonts"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        dirs.push(std::path::PathBuf::from("/System/Library/Fonts"));
        dirs.push(std::path::PathBuf::from("/Library/Fonts"));
        if let Some(home) = dirs::home_dir() {
            dirs.push(home.join("Library/Fonts"));
        }
    }

    let mut found: Vec<SystemFont> = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    fn walk(
        dir: &std::path::Path,
        depth: usize,
        out: &mut Vec<SystemFont>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        if depth > 3 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, depth + 1, out, seen);
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase());
            if !matches!(ext.as_deref(), Some("ttf" | "otf")) {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let low = stem.to_ascii_lowercase();
            if [
                "emoji",
                "math",
                "symbol",
                "dingbat",
                "wingding",
                "webding",
                "marlett",
                "powerline",
            ]
            .iter()
            .any(|bad| low.contains(bad))
            {
                continue;
            }
            let clean_name = stem.replace('_', " ");
            let key = clean_name.to_ascii_lowercase();
            if seen.insert(key) {
                out.push(SystemFont {
                    name: clean_name,
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
    }

    for d in dirs {
        walk(&d, 0, &mut found, &mut seen_names);
    }
    found.sort_by_key(|a| a.name.to_ascii_lowercase());
    found
}

/// Applies the selected `.ttf`/`.otf` font (or resets to egui's built-in default when empty).
pub fn apply_ui_font(ctx: &egui::Context, font_path: &str) {
    let mut fonts = egui::FontDefinitions::default();
    let trimmed = font_path.trim();
    if !trimmed.is_empty() {
        if let Ok(bytes) = std::fs::read(trimmed) {
            fonts.font_data.insert(
                "custom_ui_font".to_owned(),
                egui::FontData::from_owned(bytes),
            );
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "custom_ui_font".to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;
    use crate::models::{CustomPalette, ThemePreset};

    #[test]
    fn palette_lerp_and_roundtrip_work() {
        let a = ThemePreset::Sunset.palette();
        let b = ThemePreset::Emerald.palette();
        set_theme_palette(&a);
        assert_eq!(current_theme_palette(), a);

        let mid = lerp_palette(&a, &b, 0.5);
        assert_ne!(mid.accent, a.accent);
        assert_ne!(mid.accent, b.accent);
        assert_eq!(lerp_palette(&a, &b, 0.0), a);
        assert_eq!(lerp_palette(&a, &b, 1.0), b);

        // Restore default Sunset palette for other tests
        set_theme_palette(&CustomPalette::default());
    }

    #[test]
    fn flow_row_wraps_presets_inside_card_without_overflow() {
        let ctx = egui::Context::default();
        set_theme_palette(&CustomPalette::default());

        let mut last_metrics: Option<(
            Rect,
            f32,
            Vec<Rect>,
            Rect,
            f32,
            Vec<Rect>,
            Rect,
            f32,
            Vec<Rect>,
        )> = None;

        for _ in 0..2 {
            let mut raw_input = egui::RawInput::default();
            raw_input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 900.0)));
            let mut m = None;
            let _ = ctx.run(raw_input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    // Case 1: 760px card (wide settings window where 10 presets + Reset > 760px)
                    let mut p1_rects = Vec::new();
                    let mut inner_l1 = 0.0;
                    let card1 = card_frame(false, 0.0).show(ui, |ui| {
                        let card_w = 760.0;
                        ui.set_min_width(card_w);
                        ui.set_max_width(card_w);
                        inner_l1 = ui.max_rect().left();
                        ui_flow_row(ui, 30.0, |ui| {
                            for preset in ThemePreset::ALL {
                                let r = ui_pill_tab(
                                    ui,
                                    ("t1", preset.label()),
                                    preset.label(),
                                    preset == ThemePreset::Sunset,
                                );
                                p1_rects.push(r.rect);
                            }
                            let reset_r = ui_button_icon(
                                ui,
                                "t1_reset",
                                Some(Icon::Refresh),
                                "Reset Palette",
                                ButtonVariant::Subtle,
                                true,
                                28.0,
                            );
                            p1_rects.push(reset_r.rect);
                        });
                    });

                    // Case 2: 420px narrow card
                    let mut p2_rects = Vec::new();
                    let mut inner_l2 = 0.0;
                    let card2 = card_frame(false, 0.0).show(ui, |ui| {
                        let card_w = 420.0;
                        ui.set_min_width(card_w);
                        ui.set_max_width(card_w);
                        inner_l2 = ui.max_rect().left();
                        ui_flow_row(ui, 30.0, |ui| {
                            for preset in ThemePreset::ALL {
                                let r = ui_pill_tab(
                                    ui,
                                    ("t2", preset.label()),
                                    preset.label(),
                                    preset == ThemePreset::Sunset,
                                );
                                p2_rects.push(r.rect);
                            }
                            let reset_r = ui_button_icon(
                                ui,
                                "t2_reset",
                                Some(Icon::Refresh),
                                "Reset Palette",
                                ButtonVariant::Subtle,
                                true,
                                28.0,
                            );
                            p2_rects.push(reset_r.rect);
                        });
                    });

                    // Case 3: 500px card with 9 Language::ALL pills
                    let mut p3_rects = Vec::new();
                    let mut inner_l3 = 0.0;
                    let card3 = card_frame(false, 0.0).show(ui, |ui| {
                        let card_w = 500.0;
                        ui.set_min_width(card_w);
                        ui.set_max_width(card_w);
                        inner_l3 = ui.max_rect().left();
                        ui_flow_row(ui, 30.0, |ui| {
                            for opt in Language::ALL {
                                let r = ui_pill_tab(
                                    ui,
                                    ("t3", opt.code()),
                                    opt.label(),
                                    opt == Language::En,
                                );
                                p3_rects.push(r.rect);
                            }
                        });
                    });

                    m = Some((
                        card1.response.rect,
                        inner_l1,
                        p1_rects,
                        card2.response.rect,
                        inner_l2,
                        p2_rects,
                        card3.response.rect,
                        inner_l3,
                        p3_rects,
                    ));
                });
            });
            last_metrics = m;
        }

        let (c1, l1, r1, c2, l2, r2, c3, l3, r3) = last_metrics.expect("layout frame ran");

        for (name, card_rect, card_w, inner_left, rects) in [
            ("presets_760", c1, 760.0_f32, l1, r1),
            ("presets_420", c2, 420.0_f32, l2, r2),
            ("langs_500", c3, 500.0_f32, l3, r3),
        ] {
            assert!(
                card_rect.width() <= card_w + 22.0 + 1.0,
                "{name}: card stretched to {}, expected <= {}",
                card_rect.width(),
                card_w + 23.0
            );
            assert!(
                card_rect.height() >= 30.0 * 2.0 + 6.0 + 22.0 - 1.0,
                "{name}: card height {} indicates row did not wrap",
                card_rect.height()
            );
            let first_row_bottom = rects[0].bottom();
            let mut wrapped_count = 0usize;
            for r in &rects {
                assert!(
                    r.right() <= inner_left + card_w + 1.0,
                    "{name}: child rect right {} exceeds inner right {}",
                    r.right(),
                    inner_left + card_w + 1.0
                );
                if r.top() > rects[0].top() + 5.0 {
                    wrapped_count += 1;
                    assert!(
                        r.top() >= first_row_bottom + 4.0,
                        "{name}: wrapped item top {} overlaps first row bottom {}",
                        r.top(),
                        first_row_bottom
                    );
                }
            }
            assert!(
                wrapped_count >= 1,
                "{name}: expected at least 1 wrapped item"
            );
        }
    }

    #[test]
    fn palette_grid_and_modal_row_stay_within_width() {
        let ctx = egui::Context::default();
        set_theme_palette(&CustomPalette::default());

        let mut raw_input = egui::RawInput::default();
        raw_input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 900.0)));

        let mut grid_card_rect = Rect::NOTHING;
        let mut cmd_row_rect = Rect::NOTHING;
        let mut cmd_label_rect = Rect::NOTHING;

        for _ in 0..2 {
            let _ = ctx.run(raw_input.clone(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    // 1. 760px card with 3-col full_palette_grid using Russian long labels
                    let mut dummy = [255_u8, 128, 66];
                    let card_w = 760.0_f32;
                    let card = card_frame(false, 0.0).show(ui, |ui| {
                        ui.set_min_width(card_w);
                        ui.set_max_width(card_w);
                        let cols = 3usize;
                        let cell_w = ((card_w - 4.0 - 12.0 * (cols.saturating_sub(1) as f32))
                            / cols as f32)
                            .clamp(140.0, card_w);
                        egui::Grid::new("test_palette_grid")
                            .num_columns(cols)
                            .spacing([12.0, 8.0])
                            .show(ui, |ui| {
                                for idx in 0..3 {
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(cell_w, 26.0),
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            ui.set_max_width(cell_w);
                                            let _ = ui.color_edit_button_srgb(&mut dummy);
                                            ui.add_space(4.0);
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        egui::RichText::new("#FF8042")
                                                            .monospace()
                                                            .size(10.5),
                                                    );
                                                    ui.with_layout(
                                                        egui::Layout::left_to_right(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            ui.add(
                                                                egui::Label::new(
                                                                    egui::RichText::new(
                                                                        "Очень длинная русская подпись карточки (навед.)",
                                                                    )
                                                                    .small(),
                                                                )
                                                                .truncate(),
                                                            );
                                                        },
                                                    );
                                                },
                                            );
                                        },
                                    );
                                    if (idx + 1) % cols == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });
                    grid_card_rect = card.response.rect;

                    // 2. Command palette category row at 540px with 4 pills + right_to_left label
                    let scope_resp = ui.scope(|ui| {
                        ui.set_max_width(540.0);
                        ui_flow_row(ui, 26.0, |ui| {
                            for cat in ["All", "Menus", "Instances", "Actions"] {
                                let _ = ui_pill_tab(ui, ("cmd", cat), cat, cat == "All");
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    cmd_label_rect = ui.label("24 found").rect;
                                },
                            );
                        });
                    });
                    cmd_row_rect = scope_resp.response.rect;
                });
            });
        }

        assert!(
            grid_card_rect.width() <= 760.0 + 22.0 + 1.0,
            "palette grid stretched card to {}",
            grid_card_rect.width()
        );
        assert!(
            cmd_row_rect.width() <= 541.0,
            "cmd row exceeded 540px: {}",
            cmd_row_rect.width()
        );
        assert!(
            cmd_label_rect.right() <= cmd_row_rect.right() + 1.0,
            "cmd label clipped: {} > {}",
            cmd_label_rect.right(),
            cmd_row_rect.right()
        );
    }
}
