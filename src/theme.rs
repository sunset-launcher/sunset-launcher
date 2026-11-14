//! Modrinth-inspired Obsidian & Emerald design system, animations, and custom widgets.

use crate::icons::{draw_icon, Icon};
use eframe::egui::{
    self, Color32, FontFamily, FontId, Margin, Pos2, Rect, Rounding, Sense, Stroke, TextStyle,
    Vec2,
};

// Surface hierarchy (deep obsidian -> elevated cards)
pub const BG_VOID: Color32 = Color32::from_rgb(18, 20, 24);         // #121418 main canvas
pub const BG_SIDEBAR: Color32 = Color32::from_rgb(24, 26, 32);      // #181A20 sidebar / chrome bars
pub const BG_ELEVATED: Color32 = Color32::from_rgb(30, 33, 40);     // #1E2128 modals / popovers
pub const BG_CARD: Color32 = Color32::from_rgb(36, 40, 48);         // #242830 cards
pub const BG_CARD_HOVER: Color32 = Color32::from_rgb(44, 49, 59);   // #2C313B card hover
pub const BG_INPUT: Color32 = Color32::from_rgb(15, 17, 21);        // #0F1115 inputs / tracks

pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(46, 51, 61);   // #2E333D
pub const BORDER_STRONG: Color32 = Color32::from_rgb(69, 76, 90);   // #454C5A

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(238, 242, 247);   // #EEF2F7
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(155, 163, 180); // #9BA3B4
pub const TEXT_MUTED: Color32 = Color32::from_rgb(103, 112, 130);     // #677082

static ACTIVE_ACCENT: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new((27 << 16) | (217 << 8) | 106);

/// Updates the active theme accent() color at runtime.
pub fn set_accent_rgb(rgb: [u8; 3]) {
    let packed = ((rgb[0] as u32) << 16) | ((rgb[1] as u32) << 8) | (rgb[2] as u32);
    ACTIVE_ACCENT.store(packed, std::sync::atomic::Ordering::Relaxed);
}

pub fn accent() -> Color32 {
    let packed = ACTIVE_ACCENT.load(std::sync::atomic::Ordering::Relaxed);
    Color32::from_rgb((packed >> 16) as u8, (packed >> 8) as u8, packed as u8)
}

pub fn accent_hover() -> Color32 {
    lerp_color(accent(), PURE_WHITE, 0.16)
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
    lerp_color(BG_VOID, accent(), 0.22)
}

pub fn bg_card_active() -> Color32 {
    lerp_color(BG_CARD, accent(), 0.11)
}

pub const DANGER: Color32 = Color32::from_rgb(229, 72, 77);         // #E5484D
pub const DANGER_HOVER: Color32 = Color32::from_rgb(242, 94, 99);
pub const WARNING: Color32 = Color32::from_rgb(245, 166, 35);       // #F5A623
pub const INFO: Color32 = Color32::from_rgb(76, 142, 219);          // #4C8EDB
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
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(13.5, FontFamily::Proportional),
    );
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

    let mut v = egui::Visuals::dark();
    v.dark_mode = true;
    v.override_text_color = None;

    v.panel_fill = BG_VOID;
    v.window_fill = BG_ELEVATED;
    v.window_rounding = Rounding::same(12.0);
    v.window_stroke = Stroke::new(1.0_f32, BORDER_STRONG);
    v.window_shadow = egui::epaint::Shadow {
        offset: Vec2::new(0.0, 10.0),
        blur: 24.0,
        spread: 1.0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 165),
    };
    v.popup_shadow = egui::epaint::Shadow {
        offset: Vec2::new(0.0, 6.0),
        blur: 16.0,
        spread: 0.0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 140),
    };
    v.extreme_bg_color = BG_INPUT;
    v.faint_bg_color = Color32::from_rgb(24, 27, 33);
    v.code_bg_color = BG_CARD;

    v.selection.bg_fill = accent_dim();
    v.selection.stroke = Stroke::new(1.0_f32, PURE_WHITE);

    v.widgets.noninteractive.bg_fill = BG_CARD;
    v.widgets.noninteractive.weak_bg_fill = BG_CARD;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    v.widgets.noninteractive.rounding = Rounding::same(8.0);

    v.widgets.inactive.bg_fill = Color32::from_rgb(28, 32, 40);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(32, 36, 45);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(56, 63, 76));
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    v.widgets.inactive.rounding = Rounding::same(8.0);

    v.widgets.hovered.bg_fill = Color32::from_rgb(42, 48, 59);
    v.widgets.hovered.weak_bg_fill = BG_CARD_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, accent());
    v.widgets.hovered.fg_stroke = Stroke::new(1.2_f32, PURE_WHITE);
    v.widgets.hovered.rounding = Rounding::same(8.0);

    v.widgets.active.bg_fill = accent_dim();
    v.widgets.active.weak_bg_fill = accent_dim();
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, accent());
    v.widgets.active.fg_stroke = Stroke::new(1.5_f32, PURE_WHITE);
    v.widgets.active.rounding = Rounding::same(8.0);

    v.widgets.open.bg_fill = BG_CARD_HOVER;
    v.widgets.open.weak_bg_fill = BG_CARD_HOVER;
    v.widgets.open.bg_stroke = Stroke::new(1.0_f32, accent());
    v.widgets.open.fg_stroke = Stroke::new(1.0_f32, PURE_WHITE);
    v.widgets.open.rounding = Rounding::same(8.0);

    v.slider_trailing_fill = true;

    style.visuals = v;
    ctx.set_style(style);
}

/// Draws a card frame with optional animated hover and emerald selection border.
pub fn card_frame(selected: bool, hover_t: f32) -> egui::Frame {
    let bg = if selected {
        lerp_color(bg_card_active(), Color32::from_rgb(33, 60, 47), hover_t)
    } else {
        lerp_color(BG_CARD, BG_CARD_HOVER, hover_t)
    };
    let border = if selected {
        accent()
    } else {
        lerp_color(BORDER_SUBTLE, BORDER_STRONG, hover_t)
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
    let font = FontId::new(if height <= 28.0 { 12.5 } else { 13.0 }, FontFamily::Proportional);
    let galley = if text.is_empty() {
        None
    } else {
        Some(
            ui.painter()
                .layout_no_wrap(text.to_string(), font, TEXT_PRIMARY),
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
    let content_w = (if has_icon { icon_sz } else { 0.0 })
        + gap
        + galley.as_ref().map_or(0.0, |g| g.size().x);
    let min_fit_w = (content_w + pad_x * 1.5).max(height);
    let desired_w = fixed_width
        .map(|w| w.max(min_fit_w))
        .unwrap_or_else(|| (content_w + pad_x * 2.0).max(height));
    let desired = Vec2::new(desired_w, height);

    let (rect, resp) = ui.allocate_exact_size(
        desired,
        if enabled { Sense::click() } else { Sense::hover() },
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

    if ui.is_rect_visible(rect) {
        let (base_bg, hover_bg, base_border, hover_border, base_fg, hover_fg, glow_col) =
            if !enabled {
                (
                    Color32::from_rgb(28, 30, 36),
                    Color32::from_rgb(28, 30, 36),
                    BORDER_SUBTLE,
                    BORDER_SUBTLE,
                    TEXT_MUTED,
                    TEXT_MUTED,
                    Color32::TRANSPARENT,
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
                        accent(),
                    ),
                    ButtonVariant::Secondary => (
                        BG_CARD,
                        BG_CARD_HOVER,
                        BORDER_SUBTLE,
                        BORDER_STRONG,
                        TEXT_PRIMARY,
                        PURE_WHITE,
                        BORDER_STRONG,
                    ),
                    ButtonVariant::Subtle => (
                        Color32::from_rgb(24, 27, 33),
                        BG_CARD_HOVER,
                        BORDER_SUBTLE,
                        BORDER_STRONG,
                        TEXT_SECONDARY,
                        PURE_WHITE,
                        BORDER_SUBTLE,
                    ),
                    ButtonVariant::Danger => (
                        Color32::from_rgb(46, 28, 32),
                        DANGER,
                        Color32::from_rgb(96, 46, 52),
                        DANGER_HOVER,
                        Color32::from_rgb(248, 136, 140),
                        PURE_WHITE,
                        DANGER,
                    ),
                }
            };

        let bg = lerp_color(base_bg, hover_bg, hover_t);
        let border = lerp_color(base_border, hover_border, hover_t);
        let fg = lerp_color(base_fg, hover_fg, hover_t);
        let draw_rect = rect.shrink(press_t * 1.0);
        let rounding = Rounding::same(7.5);
        let painter = ui.painter();

        if hover_t > 0.02 && matches!(variant, ButtonVariant::Primary | ButtonVariant::Danger) {
            painter.rect_stroke(
                draw_rect.expand(1.5 * hover_t),
                Rounding::same(9.0),
                Stroke::new(1.3_f32, with_alpha(glow_col, (hover_t * 72.0) as u8)),
            );
        }

        painter.rect_filled(draw_rect, rounding, bg);
        if enabled && draw_rect.width() > 14.0 {
            let bevel_alpha = match variant {
                ButtonVariant::Primary => 72,
                ButtonVariant::Secondary | ButtonVariant::Subtle => (18.0 + hover_t * 24.0) as u8,
                ButtonVariant::Danger => (20.0 + hover_t * 36.0) as u8,
            };
            painter.line_segment(
                [
                    Pos2::new(draw_rect.left() + 6.0, draw_rect.top() + 1.0),
                    Pos2::new(draw_rect.right() - 6.0, draw_rect.top() + 1.0),
                ],
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, bevel_alpha)),
            );
        }
        painter.rect_stroke(draw_rect, rounding, Stroke::new(1.0_f32, border));

        let clipped = painter.with_clip_rect(draw_rect.shrink(3.0));
        let mut cur_x = draw_rect.center().x - content_w * 0.5;
        if let Some(ic) = icon {
            let icon_rect = Rect::from_center_size(
                Pos2::new(cur_x + icon_sz * 0.5, draw_rect.center().y),
                Vec2::splat(icon_sz),
            );
            draw_icon(&clipped, icon_rect, ic, fg);
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
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), row_h),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_height(row_h);
            ui.set_max_height(row_h);
            ui.spacing_mut().interact_size.y = row_h;
            ui.spacing_mut().button_padding = Vec2::new(11.0, ((row_h - 15.0) * 0.5).max(2.0));
            add_contents(ui)
        },
    )
}

/// Wrapping horizontal row where every control aligns vertically to the first widget's centerline
/// and inherits identical row height (`interact_size.y = row_h`).
pub fn ui_wrap_row<R>(
    ui: &mut egui::Ui,
    row_h: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), row_h),
        egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true),
        |ui| {
            ui.spacing_mut().interact_size.y = row_h;
            ui.spacing_mut().button_padding = Vec2::new(11.0, ((row_h - 15.0) * 0.5).max(2.0));
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            add_contents(ui)
        },
    )
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
        .layout_no_wrap(text.to_string(), font, TEXT_PRIMARY);
    let icon_sz = 12.5;
    let gap = if icon.is_some() { 5.5 } else { 0.0 };
    let content_w = (if icon.is_some() { icon_sz } else { 0.0 }) + gap + galley.size().x;
    let row_h = ui.spacing().interact_size.y.max(28.0);
    let desired = Vec2::new((content_w + 22.0).max(46.0), row_h);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());

    let active_t = ui
        .ctx()
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

    if ui.is_rect_visible(rect) {
        let draw_rect = rect.shrink(press_t * 0.9);
        let idle_bg = lerp_color(Color32::from_rgb(26, 29, 35), BG_CARD_HOVER, hover_t);
        let bg = lerp_color(idle_bg, accent_dim(), active_t);
        let border = lerp_color(
            lerp_color(BORDER_SUBTLE, BORDER_STRONG, hover_t),
            accent(),
            active_t,
        );
        let fg = lerp_color(
            lerp_color(TEXT_SECONDARY, PURE_WHITE, hover_t),
            Color32::from_rgb(174, 252, 206),
            active_t,
        );
        let painter = ui.painter();
        painter.rect_filled(draw_rect, Rounding::same(7.5), bg);
        painter.rect_stroke(draw_rect, Rounding::same(7.5), Stroke::new(1.0_f32, border));

        if active_t > 0.03 {
            let bar_w = (draw_rect.width() - 14.0).max(8.0) * active_t;
            let bar_rect = Rect::from_center_size(
                Pos2::new(draw_rect.center().x, draw_rect.bottom() - 2.0),
                Vec2::new(bar_w, 2.0),
            );
            painter.rect_filled(bar_rect, Rounding::same(1.0), accent());
        }

        let mut cur_x = draw_rect.center().x - content_w * 0.5;
        let cy = draw_rect.center().y - active_t * 0.4;
        if let Some(ic) = icon {
            let ic_rect =
                Rect::from_center_size(Pos2::new(cur_x + icon_sz * 0.5, cy), Vec2::splat(icon_sz));
            draw_icon(painter, ic_rect, ic, fg);
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
        .layout_no_wrap(kind.label().to_string(), font, TEXT_PRIMARY);
    let dot_sz = 7.0;
    let gap = 6.0;
    let content_w = dot_sz + gap + galley.size().x;
    let row_h = ui.spacing().interact_size.y.max(28.0);
    let desired = Vec2::new((content_w + 22.0).max(54.0), row_h);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());

    let active_t = ui
        .ctx()
        .animate_bool_with_time(ui.make_persistent_id(("loader_pill_act", &id)), active, 0.16);
    let hover_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("loader_pill_hov", &id)),
        resp.hovered(),
        0.11,
    );
    if (active_t > 0.01 && active_t < 0.99) || (hover_t > 0.01 && hover_t < 0.99) {
        ui.ctx().request_repaint();
    }

    if ui.is_rect_visible(rect) {
        let idle_bg = lerp_color(Color32::from_rgb(26, 29, 35), BG_CARD_HOVER, hover_t);
        let bg = lerp_color(idle_bg, loader_bg, active_t);
        let border = lerp_color(
            lerp_color(BORDER_SUBTLE, BORDER_STRONG, hover_t),
            loader_border,
            active_t,
        );
        let fg = lerp_color(
            lerp_color(TEXT_SECONDARY, PURE_WHITE, hover_t),
            loader_fg,
            active_t,
        );
        let dot_col = lerp_color(loader_border, loader_fg, active_t * 0.45);
        let painter = ui.painter();
        painter.rect_filled(rect, Rounding::same(7.5), bg);
        painter.rect_stroke(rect, Rounding::same(7.5), Stroke::new(1.0_f32, border));

        if active_t > 0.03 {
            let bar_w = (rect.width() - 14.0).max(8.0) * active_t;
            let bar_rect = Rect::from_center_size(
                Pos2::new(rect.center().x, rect.bottom() - 2.0),
                Vec2::new(bar_w, 2.0),
            );
            painter.rect_filled(bar_rect, Rounding::same(1.0), loader_border);
        }

        let start_x = rect.center().x - content_w * 0.5;
        let cy = rect.center().y - active_t * 0.4;
        painter.circle_filled(Pos2::new(start_x + dot_sz * 0.5, cy), 3.2, dot_col);
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
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, TEXT_PRIMARY);
    let box_sz = 17.0;
    let gap = 7.0;
    let row_h = ui.spacing().interact_size.y.max(26.0);
    let desired = Vec2::new(box_sz + gap + galley.size().x + 4.0, row_h);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    let mut toggled = false;
    if resp.clicked() {
        *checked = !*checked;
        toggled = true;
    }

    let check_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("chk_val", &id_salt)),
        *checked,
        0.13,
    );
    let hover_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("chk_hov", &id_salt)),
        resp.hovered(),
        0.11,
    );
    if (check_t > 0.01 && check_t < 0.99) || (hover_t > 0.01 && hover_t < 0.99) {
        ui.ctx().request_repaint();
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let box_rect = Rect::from_center_size(
            Pos2::new(rect.left() + box_sz * 0.5 + 1.0, rect.center().y),
            Vec2::splat(box_sz),
        );
        let fill = lerp_color(
            lerp_color(BG_INPUT, BG_CARD_HOVER, hover_t * 0.6),
            accent(),
            check_t,
        );
        let border = lerp_color(
            lerp_color(BORDER_STRONG, PURE_WHITE, hover_t * 0.4),
            accent_hover(),
            check_t,
        );
        painter.rect_filled(box_rect, Rounding::same(4.5), fill);
        painter.rect_stroke(box_rect, Rounding::same(4.5), Stroke::new(1.1_f32, border));

        if check_t > 0.08 {
            draw_icon(
                painter,
                box_rect.shrink(3.0),
                Icon::Check,
                with_alpha(accent_dark(), (check_t * 255.0) as u8),
            );
        }

        let text_col = lerp_color(TEXT_SECONDARY, PURE_WHITE, check_t.max(hover_t * 0.8));
        painter.galley(
            Pos2::new(box_rect.right() + gap, rect.center().y - galley.size().y * 0.5),
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

    let border_col = lerp_color(BORDER_SUBTLE, accent(), focus_t);
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
                if focus_t > 0.05 {
                    ui.painter().rect_stroke(
                        box_rect.expand(1.4 * focus_t),
                        Rounding::same(9.0),
                        Stroke::new(1.0_f32, with_alpha(accent(), (focus_t * 50.0) as u8)),
                    );
                }
                ui.painter().rect(
                    box_rect,
                    Rounding::same(7.5),
                    BG_INPUT,
                    Stroke::new(1.0_f32 + focus_t * 0.25, border_col),
                );
            }
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(9.0);
            let edit = egui::TextEdit::singleline(text)
                .id_salt(&id_salt)
                .frame(false)
                .password(password)
                .hint_text(hint)
                .desired_width((total_w - 18.0).max(50.0));
            let r = ui.add(edit);
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

    let border_col = lerp_color(BORDER_SUBTLE, accent(), focus_t);
    let icon_col = lerp_color(TEXT_MUTED, accent(), focus_t);
    let mut now_focused = false;

    ui.allocate_ui_with_layout(
        Vec2::new(total_w, row_h),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_size(Vec2::new(total_w, row_h));
            ui.set_max_size(Vec2::new(total_w, row_h));
            let box_rect = ui.max_rect();
            if ui.is_rect_visible(box_rect) {
                if focus_t > 0.05 {
                    ui.painter().rect_stroke(
                        box_rect.expand(1.5 * focus_t),
                        Rounding::same(9.0),
                        Stroke::new(1.0_f32, with_alpha(accent(), (focus_t * 58.0) as u8)),
                    );
                }
                ui.painter().rect(
                    box_rect,
                    Rounding::same(7.5),
                    BG_INPUT,
                    Stroke::new(1.0_f32 + focus_t * 0.3, border_col),
                );
            }
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_space(2.0);
            let (mag_r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
            draw_icon(ui.painter(), mag_r, Icon::Search, icon_col);

            let edit_resp = ui.add(
                egui::TextEdit::singleline(text)
                    .id_salt(&id_salt)
                    .frame(false)
                    .hint_text(hint)
                    .desired_width(inner_edit_w),
            );
            now_focused = edit_resp.has_focus() || edit_resp.hovered();
            if edit_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submitted = true;
            }
            if has_clear {
                let (clr_r, clr_resp) =
                    ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
                let clr_col = if clr_resp.hovered() {
                    PURE_WHITE
                } else {
                    TEXT_SECONDARY
                };
                draw_icon(ui.painter(), clr_r.shrink(2.5), Icon::Close, clr_col);
                if clr_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if clr_resp.on_hover_text("Clear search").clicked() {
                    text.clear();
                    submitted = true;
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

/// Renders an inline semantic alert banner with a leading vector icon.
pub fn ui_alert_banner(ui: &mut egui::Ui, kind: AlertKind, text: &str, width: f32) {
    let (bg, border, fg, icon) = match kind {
        AlertKind::Info => (
            Color32::from_rgb(24, 34, 48),
            INFO,
            Color32::from_rgb(172, 212, 255),
            Icon::Sparkle,
        ),
        AlertKind::Success => (
            bg_card_active(),
            accent(),
            Color32::from_rgb(162, 248, 198),
            Icon::Check,
        ),
        AlertKind::Warning => (
            Color32::from_rgb(48, 38, 22),
            WARNING,
            Color32::from_rgb(252, 212, 134),
            Icon::Alert,
        ),
        AlertKind::Error => (
            Color32::from_rgb(50, 26, 30),
            DANGER,
            Color32::from_rgb(255, 168, 172),
            Icon::Alert,
        ),
    };
    let inner_w = (width - 24.0).max(80.0);
    egui::Frame::none()
        .fill(bg)
        .stroke(Stroke::new(1.0_f32, with_alpha(border, 180)))
        .rounding(Rounding::same(8.5))
        .inner_margin(Margin::symmetric(11.0, 8.0))
        .show(ui, |ui| {
            ui.set_min_width(inner_w);
            ui.set_max_width(inner_w);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let (ic_r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                draw_icon(ui.painter(), ic_r, icon, border);
                ui.add(egui::Label::new(egui::RichText::new(text).small().color(fg)).wrap());
            });
        });
}

/// Draws a section header label with subtle uppercase tracking inside settings/forms.
pub fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .size(10.5)
            .strong()
            .color(TEXT_MUTED),
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
    let hover_t = ui.ctx().animate_bool_with_time(id, prev_hover, 0.14);
    let select_t = ui
        .ctx()
        .animate_bool_with_time(id.with("sel"), selected, 0.16);
    if (hover_t > 0.01 && hover_t < 0.99) || (select_t > 0.01 && select_t < 0.99) {
        ui.ctx().request_repaint();
    }
    let resp = card_frame(selected, hover_t).show(ui, add_contents);
    let rect = resp.response.rect;
    let hovered_now = resp.response.hovered() || ui.rect_contains_pointer(rect);
    ui.ctx().data_mut(|d| d.insert_temp(id, hovered_now));

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let rim_alpha = (14.0 + hover_t * 24.0 + select_t * 28.0) as u8;
        if rect.width() > 20.0 {
            painter.line_segment(
                [
                    Pos2::new(rect.left() + 10.0, rect.top() + 1.0),
                    Pos2::new(rect.right() - 10.0, rect.top() + 1.0),
                ],
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, rim_alpha)),
            );
        }
        let bar_strength = select_t.max(hover_t * 0.55);
        if bar_strength > 0.04 && rect.height() >= 28.0 {
            let max_bar_h = (rect.height() - 14.0).clamp(10.0, 38.0);
            let bar_h = max_bar_h * bar_strength;
            let bar_col = lerp_color(BORDER_STRONG, accent(), select_t.max(hover_t * 0.7));
            let bar_rect = Rect::from_center_size(
                Pos2::new(rect.left() + 2.2, rect.center().y),
                Vec2::new(3.0, bar_h),
            );
            painter.rect_filled(bar_rect, Rounding::same(1.5), bar_col);
        }
    }
    resp
}

/// Renders the hero spotlight card with a subtle loader-tinted ambient corner glow.
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

    let bg = lerp_color(bg_card_active(), Color32::from_rgb(31, 58, 45), hover_t);
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

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_stroke(
            rect.expand(1.5),
            Rounding::same(13.5),
            Stroke::new(1.5_f32, with_alpha(loader_accent, 28)),
        );
        if rect.width() > 28.0 {
            painter.line_segment(
                [
                    Pos2::new(rect.left() + 14.0, rect.top() + 1.2),
                    Pos2::new(rect.right() - 14.0, rect.top() + 1.2),
                ],
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 58)),
            );
        }
        if rect.height() > 28.0 {
            let bar_h = (rect.height() - 22.0).max(16.0);
            let bar_rect = Rect::from_center_size(
                Pos2::new(rect.left() + 3.0, rect.center().y),
                Vec2::new(3.5, bar_h),
            );
            painter.rect_filled(bar_rect, Rounding::same(1.75), loader_accent);
        }
    }
    resp
}

/// Draws a pill badge (`Vanilla`, `Fabric`, `Microsoft`, `Installed`).
pub fn badge(ui: &mut egui::Ui, text: &str, highlighted: bool) {
    let (fill, border, fg) = if highlighted {
        (accent_dim(), accent(), Color32::from_rgb(148, 248, 190))
    } else {
        (Color32::from_rgb(30, 33, 40), BORDER_STRONG, TEXT_SECONDARY)
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
        (accent_dim(), accent(), Color32::from_rgb(148, 248, 190))
    } else {
        (Color32::from_rgb(30, 33, 40), BORDER_STRONG, TEXT_SECONDARY)
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
    match kind {
        LoaderKind::Vanilla => (
            Color32::from_rgb(24, 40, 58),
            Color32::from_rgb(76, 142, 219),
            Color32::from_rgb(162, 210, 255),
        ),
        LoaderKind::Fabric => (
            Color32::from_rgb(54, 45, 30),
            Color32::from_rgb(214, 174, 108),
            Color32::from_rgb(248, 218, 166),
        ),
        LoaderKind::Quilt => (
            Color32::from_rgb(47, 30, 62),
            Color32::from_rgb(176, 118, 224),
            Color32::from_rgb(222, 186, 252),
        ),
        LoaderKind::Forge => (
            Color32::from_rgb(58, 36, 24),
            Color32::from_rgb(228, 132, 64),
            Color32::from_rgb(252, 188, 138),
        ),
        LoaderKind::NeoForge => (
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

    let top_strip = Rect::from_min_size(
        Pos2::new(rect.left() + 3.0, rect.top() + 2.0),
        Vec2::new((rect.width() - 6.0).max(4.0), 2.0),
    );
    painter.rect_filled(top_strip, Rounding::same(1.0), with_alpha(loader_fg, 65));
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
        FontId::new((rect.height() * 0.44).clamp(12.0, 24.0), FontFamily::Proportional),
        if selected { PURE_WHITE } else { loader_fg },
    );

    if rect.width() >= 28.0 {
        let dot_center = Pos2::new(rect.right() - 5.5, rect.bottom() - 5.5);
        painter.circle_filled(dot_center, 4.6, BG_CARD);
        painter.circle_filled(
            dot_center,
            3.1,
            if is_running { accent() } else { loader_border },
        );
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
    painter.rect_filled(rect, Rounding::same(rounding), BG_INPUT);
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
        Stroke::new(1.0_f32, BORDER_SUBTLE),
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
        FontId::new((rect.height() * 0.42).clamp(12.0, 18.0), FontFamily::Proportional),
        TEXT_SECONDARY,
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
    ui.ctx().request_repaint();
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
        ctx.request_repaint();
    }

    let desired_w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(desired_w, 22.0), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();

    painter.rect_filled(rect, Rounding::same(6.5), BG_INPUT);
    painter.rect_stroke(rect, Rounding::same(6.5), Stroke::new(1.0_f32, BORDER_SUBTLE));

    let fill_w = (rect.width() * smooth).max(if active { 20.0 } else { 0.0 });
    if fill_w > 0.5 {
        let fill_rect =
            Rect::from_min_size(rect.min, Vec2::new(fill_w.min(rect.width()), rect.height()));
        painter.rect_filled(fill_rect, Rounding::same(6.5), accent());

        if active {
            let t = (ui.input(|i| i.time) as f32 * 1.4).fract();
            let sweep_x = fill_rect.left() + t * (fill_rect.width() + 60.0) - 30.0;
            let stripe = Rect::from_min_max(
                Pos2::new(sweep_x.max(fill_rect.left()), fill_rect.top()),
                Pos2::new((sweep_x + 28.0).min(fill_rect.right()), fill_rect.bottom()),
            );
            if stripe.width() > 1.0 {
                painter.rect_filled(
                    stripe,
                    Rounding::same(5.0),
                    Color32::from_rgba_unmultiplied(255, 255, 255, 80),
                );
            }
        }
    }

    let pct_text = format!("{}%", (smooth * 100.0).round() as u32);
    let text_color = if smooth > 0.48 { accent_dark() } else { TEXT_PRIMARY };
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
            TEXT_SECONDARY
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
pub fn draw_mini_gauge(
    ui: &mut egui::Ui,
    id_salt: &str,
    fraction: f32,
    width: f32,
    height: f32,
) {
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
    painter.rect_filled(rect, r, BG_INPUT);
    painter.rect_stroke(rect, r, Stroke::new(1.0_f32, BORDER_SUBTLE));
    let fill_w = (rect.width() * smooth).clamp(0.0, rect.width());
    if fill_w > 1.0 {
        let fill_rect = Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
        painter.rect_filled(fill_rect, r, accent());
    }
}

/// Visual RAM allocation meter showing Min (-Xms) and Max (-Xmx) out of 16 GB.
pub fn draw_ram_allocation_bar(
    ui: &mut egui::Ui,
    min_mb: u32,
    max_mb: u32,
    width: f32,
) {
    let cap = 16384.0_f32;
    let min_f = (min_mb as f32 / cap).clamp(0.0, 1.0);
    let max_f = (max_mb as f32 / cap).clamp(min_f, 1.0);
    let smooth_min = ui.ctx().animate_value_with_time(
        ui.make_persistent_id("ram_bar_min"),
        min_f,
        0.16,
    );
    let smooth_max = ui.ctx().animate_value_with_time(
        ui.make_persistent_id("ram_bar_max"),
        max_f,
        0.16,
    );
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
    painter.rect_stroke(rect, r, Stroke::new(1.0_f32, BORDER_STRONG));

    for gb in [4.0_f32, 8.0, 12.0] {
        let x = rect.left() + rect.width() * (gb / 16.0);
        painter.line_segment(
            [Pos2::new(x, rect.top() + 1.5), Pos2::new(x, rect.bottom() - 1.5)],
            Stroke::new(1.0_f32, BORDER_STRONG),
        );
    }

    let max_w = (rect.width() * smooth_max).clamp(0.0, rect.width());
    if max_w > 2.0 {
        let max_col = if max_mb >= 12288 { WARNING } else { accent_dim() };
        let max_rect = Rect::from_min_size(rect.min, Vec2::new(max_w, rect.height()));
        painter.rect_filled(max_rect, r, with_alpha(max_col, 220));
    }
    let min_w = (rect.width() * smooth_min).clamp(0.0, rect.width());
    if min_w > 2.0 {
        let min_rect = Rect::from_min_size(rect.min, Vec2::new(min_w, rect.height()));
        painter.rect_filled(min_rect, r, accent());
    }
}

/// Renders a high-contrast slider where the full track bar remains clearly visible on card surfaces.
pub fn ui_ram_slider(
    ui: &mut egui::Ui,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
    slider_width: f32,
) -> egui::Response {
    ui.scope(|ui| {
        ui.style_mut().spacing.slider_width = slider_width;
        ui.style_mut().spacing.slider_rail_height = 8.0;
        let visuals = ui.visuals_mut();
        visuals.slider_trailing_fill = true;
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(64, 71, 86);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.5_f32, accent());
        visuals.widgets.inactive.rounding = Rounding::same(4.0);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(82, 91, 108);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.5_f32, PURE_WHITE);
        visuals.widgets.active.bg_fill = accent();
        visuals.selection.bg_fill = Color32::from_rgb(27, 165, 86);
        ui.add(
            egui::Slider::new(value, range)
                .step_by(256.0)
                .suffix(" MB"),
        )
    })
    .inner
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
        let base_block = lerp_color(
            Color32::from_rgb(26, 29, 35),
            Color32::from_rgb(34, 38, 46),
            pulse,
        );

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
    let (rect, resp) = ui.allocate_exact_size(
        size,
        if enabled { Sense::click() } else { Sense::hover() },
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
    if (hover_t > 0.01 && hover_t < 0.99) || (press_t > 0.01 && press_t < 0.99) {
        ui.ctx().request_repaint();
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let time = ui.input(|i| i.time) as f32;

        let (bg, border, fg, halo_color) = if !enabled {
            (BG_CARD, BORDER_SUBTLE, TEXT_MUTED, accent())
        } else if is_stop_style {
            let bg = lerp_color(DANGER, DANGER_HOVER, hover_t);
            (bg, DANGER_HOVER, PURE_WHITE, DANGER)
        } else {
            let bg = lerp_color(accent(), accent_hover(), hover_t);
            (bg, accent_hover(), accent_dark(), accent())
        };

        if pulsing {
            let pulse = (time * 3.5).sin() * 0.5 + 0.5;
            let halo_alpha = (45.0 + 75.0 * pulse) as u8;
            painter.rect_stroke(
                rect.expand(2.2 + pulse * 1.4),
                Rounding::same(10.5),
                Stroke::new(1.5_f32, with_alpha(halo_color, halo_alpha)),
            );
            ui.ctx().request_repaint();
        } else if hover_t > 0.02 {
            let halo_alpha = (hover_t * 105.0) as u8;
            painter.rect_stroke(
                rect.expand(2.2 * hover_t),
                Rounding::same(10.5),
                Stroke::new(1.5_f32, with_alpha(halo_color, halo_alpha)),
            );
        }

        let draw_rect = rect.shrink(press_t * 1.1);
        painter.rect_filled(draw_rect, Rounding::same(8.5), bg);
        if enabled && draw_rect.width() > 20.0 {
            painter.line_segment(
                [
                    Pos2::new(draw_rect.left() + 8.0, draw_rect.top() + 1.2),
                    Pos2::new(draw_rect.right() - 8.0, draw_rect.top() + 1.2),
                ],
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 90)),
            );
        }
        painter.rect_stroke(draw_rect, Rounding::same(8.5), Stroke::new(1.2_f32, border));

        let galley = painter.layout_no_wrap(
            label.to_string(),
            FontId::new(14.0, FontFamily::Proportional),
            fg,
        );
        let icon_sz = 13.0;
        let gap = if icon.is_some() { 7.0 } else { 0.0 };
        let total_w = (if icon.is_some() { icon_sz } else { 0.0 }) + gap + galley.size().x;
        let mut x = draw_rect.center().x - total_w * 0.5;
        if let Some(ic) = icon {
            let ic_rect = Rect::from_center_size(
                Pos2::new(x + icon_sz * 0.5, draw_rect.center().y),
                Vec2::splat(icon_sz),
            );
            draw_icon(painter, ic_rect, ic, fg);
            x += icon_sz + gap;
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
    let desired = Vec2::new(38.0, 20.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    let anim_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("switch", id_salt)),
        on,
        0.15,
    );
    let hover_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("switch_hov", id_salt)),
        resp.hovered(),
        0.11,
    );
    if (anim_t > 0.01 && anim_t < 0.99) || (hover_t > 0.01 && hover_t < 0.99) {
        ui.ctx().request_repaint();
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let bg = lerp_color(
            lerp_color(BG_INPUT, BG_CARD_HOVER, hover_t * 0.5),
            lerp_color(accent(), accent_hover(), hover_t),
            anim_t,
        );
        let border = lerp_color(BORDER_STRONG, accent_hover(), anim_t.max(hover_t * 0.4));
        let rounding = Rounding::same(rect.height() * 0.5);
        if anim_t > 0.05 && hover_t > 0.05 {
            painter.rect_stroke(
                rect.expand(1.2 * hover_t),
                Rounding::same(rect.height() * 0.5 + 1.2),
                Stroke::new(1.0_f32, with_alpha(accent(), (hover_t * 65.0) as u8)),
            );
        }
        painter.rect_filled(rect, rounding, bg);
        painter.rect_stroke(rect, rounding, Stroke::new(1.0_f32, border));

        let r = rect.height() * (0.36 + hover_t * 0.03);
        let left_x = rect.left() + r + 3.0;
        let right_x = rect.right() - r - 3.0;
        let knob_x = left_x + (right_x - left_x) * anim_t;
        let knob_col = lerp_color(
            lerp_color(TEXT_SECONDARY, PURE_WHITE, hover_t),
            accent_dark(),
            anim_t,
        );
        painter.circle_filled(Pos2::new(knob_x, rect.center().y), r, knob_col);
    }

    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}
