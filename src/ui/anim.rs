//! Procedural 60fps animation engine: easing curves, click/press states, icon micro-animations,
//! and subtle foreground interaction feedback for Sunset Launcher.

use crate::icons::Icon;
use eframe::egui::{self, Color32, Id, LayerId, Order, Pos2, Rect, Rounding, Stroke, Vec2};
use std::f32::consts::{PI, TAU};

// ─────────────────────────────────────────────────────────────────────────────
// Easing & timing curves
// ─────────────────────────────────────────────────────────────────────────────

/// Smooth cubic ease-out (`1 - (1 - t)^3`), ideal for hover & spatial slides.
#[inline]
pub fn ease_out_cubic(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

/// Snappy quintic ease-out (`1 - (1 - t)^5`), great for ripples & fast reveals.
#[inline]
pub fn ease_out_quint(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u * u * u
}

/// Exponential ease-out (`1 - 2^(-10 t)`), ultra-responsive initial velocity.
#[inline]
#[allow(dead_code)]
pub fn ease_out_expo(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 {
        1.0
    } else {
        1.0 - 2.0_f32.powf(-10.0 * t)
    }
}

/// Symmetric cubic ease-in-out for reversible transitions.
#[inline]
#[allow(dead_code)]
pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let f = -2.0 * t + 2.0;
        1.0 - (f * f * f) * 0.5
    }
}

/// Spring-like overshoot ease-out (`c1 = 1.70158`).
#[inline]
pub fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.70158_f32;
    let c3 = c1 + 1.0;
    let u = t - 1.0;
    1.0 + c3 * u * u * u + c1 * u * u
}

/// Elastic spring impulse (`0 -> ~1.08 -> 1.0`), ideal for scale pop on click/check.
#[inline]
#[allow(dead_code)]
pub fn spring_pop(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.0 || t >= 1.0 {
        return t;
    }
    1.0 - (-6.5 * t).exp() * (t * PI * 2.5).cos()
}

/// Smooth 0..1->0 click impulse wave (peaks at `t ≈ 0.28`, decays smoothly to `0.0` at `t = 1.0`).
#[inline]
pub fn impulse_wave(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let h = 4.5 * t;
    (h * (1.0 - t) * (-2.2 * t).exp() * 2.15).clamp(0.0, 1.0)
}

/// Continuous 0..1 sine wave oscillation at `hz` frequency.
#[inline]
pub fn pulse_01(time: f64, hz: f32) -> f32 {
    (((time * (hz as f64) * std::f64::consts::TAU).sin() * 0.5 + 0.5) as f32).clamp(0.0, 1.0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Persistent per-widget click / timestamp tracking
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct ClickStamp {
    time: f64,
    pos: Pos2,
}

/// Records a click timestamp and pointer position (falling back to `rect.center()`) when `clicked` is true,
/// and returns normalized progress `(t in 0.0..=1.0, click_origin)` while the click animation
/// is active within `duration` seconds. Automatically schedules repaints while active.
pub fn track_click_anim(
    ctx: &egui::Context,
    id: Id,
    rect: Rect,
    clicked: bool,
    pointer_pos: Option<Pos2>,
    duration: f32,
) -> Option<(f32, Pos2)> {
    let now = ctx.input(|i| i.time);
    let stamp_id = id.with("click_stamp_v2");
    if clicked {
        let pos = pointer_pos.unwrap_or_else(|| rect.center());
        ctx.data_mut(|d| d.insert_temp(stamp_id, ClickStamp { time: now, pos }));
        ctx.request_repaint();
        return Some((0.001, pos));
    }

    let stamp: Option<ClickStamp> = ctx.data_mut(|d| d.get_temp(stamp_id));
    if let Some(st) = stamp {
        let elapsed = (now - st.time) as f32;
        let dur = duration.max(0.05);
        if elapsed >= 0.0 && elapsed < dur {
            ctx.request_repaint();
            return Some(((elapsed / dur).clamp(0.0, 1.0), st.pos));
        }
    }
    None
}

/// Tracks how many seconds have elapsed since `id` was last clicked (up to 10s).
/// Useful for temporary button state swaps (e.g. Copy icon morphing into Check icon for 0.85s).
pub fn click_age_secs(ctx: &egui::Context, id: Id) -> Option<f32> {
    let now = ctx.input(|i| i.time);
    let stamp_id = id.with("click_stamp_v2");
    let stamp: Option<ClickStamp> = ctx.data_mut(|d| d.get_temp(stamp_id));
    stamp.and_then(|st| {
        let age = (now - st.time) as f32;
        if (0.0..=10.0).contains(&age) {
            Some(age)
        } else {
            None
        }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Clipped surface effects (ripples & hover shimmer sweeps)
// ─────────────────────────────────────────────────────────────────────────────

/// Paints a radial shockwave + soft fill flash clipped inside `rect` during a click animation.
pub fn paint_click_ripple(
    painter: &egui::Painter,
    rect: Rect,
    rounding: Rounding,
    origin: Pos2,
    t: f32,
    color: Color32,
    outer_bloom: bool,
) {
    let t = t.clamp(0.0, 1.0);
    let eased = ease_out_quint(t);
    let inv = 1.0 - t;
    let origin = Pos2::new(
        origin.x.clamp(rect.left(), rect.right()),
        origin.y.clamp(rect.top(), rect.bottom()),
    );
    let max_alpha = 64.0_f32;

    let clipped = painter.with_clip_rect(rect);
    let max_r = (rect.width().hypot(rect.height()) * 0.65).max(20.0);
    let r = 4.0 + eased * max_r;

    let fill_a = ((inv * inv) * (max_alpha * 0.45)).round() as u8;
    if fill_a > 1 {
        clipped.circle_filled(
            origin,
            r,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), fill_a),
        );
    }

    let ring_a = ((inv * (1.0 - eased * 0.55)) * max_alpha).round() as u8;
    if ring_a > 1 {
        clipped.circle_stroke(
            origin,
            r * 0.92,
            Stroke::new(
                1.35 * inv + 0.45,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), ring_a),
            ),
        );
    }

    if outer_bloom {
        let border_a = ((1.0 - (t * 1.6).min(1.0)).powi(2) * (max_alpha * 0.65)).round() as u8;
        if border_a > 1 {
            painter.rect_stroke(
                rect.expand(eased * 2.2),
                rounding,
                Stroke::new(
                    1.15_f32,
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), border_a),
                ),
            );
        }
    }
}

/// Paints a subtle moving diagonal sheen across action buttons when hovered.
pub fn paint_shimmer_sweep(
    painter: &egui::Painter,
    rect: Rect,
    rounding: Rounding,
    progress_01: f32,
    color: Color32,
    max_alpha: u8,
) {
    if max_alpha <= 1 {
        return;
    }
    let p = ease_out_cubic(progress_01.clamp(0.0, 1.0));
    let clipped = painter.with_clip_rect(rect);
    let band_w = (rect.width() * 0.28).clamp(14.0, 48.0);
    let x_center = (rect.left() - band_w * 0.5) + p * (rect.width() + band_w);
    let band = Rect::from_min_max(
        Pos2::new(x_center - band_w * 0.5, rect.top()),
        Pos2::new(x_center + band_w * 0.5, rect.bottom()),
    );
    clipped.rect_filled(
        band,
        rounding,
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), max_alpha),
    );
}

/// Paints a decaying fade-in veil on top of a transitioned panel/sub-tab so that
/// content appears to smoothly cross-fade as well as slide.
pub fn paint_fade_veil(
    painter: &egui::Painter,
    rect: Rect,
    bg_color: Color32,
    progress_01: f32,
    max_alpha: f32,
) {
    let p = progress_01.clamp(0.0, 1.0);
    if p >= 0.99 {
        return;
    }
    let inv = 1.0 - ease_out_cubic(p);
    let a = (inv * inv * max_alpha).round().clamp(0.0, 255.0) as u8;
    if a > 1 {
        painter.rect_filled(
            rect,
            Rounding::ZERO,
            Color32::from_rgba_unmultiplied(bg_color.r(), bg_color.g(), bg_color.b(), a),
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Semantic icon micro-animation transforms
// ─────────────────────────────────────────────────────────────────────────────

/// Transform applied to vector icons inside buttons, tabs, and action controls.
#[derive(Debug, Clone, Copy)]
pub struct IconAnim {
    pub offset: Vec2,
    pub scale: f32,
    pub rotation: f32,
}

impl Default for IconAnim {
    fn default() -> Self {
        Self {
            offset: Vec2::ZERO,
            scale: 1.0,
            rotation: 0.0,
        }
    }
}

/// Computes a semantic, icon-specific animation transform driven by hover (`0..1`),
/// press (`0..1`), and click impulse (`Option<0..1>`).
pub fn compute_icon_anim(icon: Icon, hover_t: f32, press_t: f32, click_t: Option<f32>) -> IconAnim {
    let hover_eased = ease_out_cubic(hover_t);
    let click_p = click_t.unwrap_or(1.0);
    let impulse = click_t.map(impulse_wave).unwrap_or(0.0);

    let mut offset = Vec2::ZERO;
    let mut scale = 1.0 + hover_eased * 0.07 - press_t * 0.08 + impulse * 0.14;
    let mut rotation = 0.0_f32;

    match icon {
        Icon::Refresh => {
            let click_spin = click_t.map(|t| ease_out_cubic(t) * TAU).unwrap_or(0.0);
            rotation = hover_eased * 0.55 + click_spin;
        }
        Icon::Sliders => {
            let click_turn = click_t
                .map(|t| ease_out_back(t) * (TAU / 3.0))
                .unwrap_or(0.0);
            rotation = hover_eased * 0.35 + click_turn;
        }
        Icon::Plus => {
            let click_turn = click_t
                .map(|t| ease_out_back(t) * (PI * 0.5))
                .unwrap_or(0.0);
            rotation = hover_eased * (PI * 0.25) + click_turn;
            scale += impulse * 0.08;
        }
        Icon::Play => {
            offset.x = hover_eased * 1.4 + impulse * 2.4 - press_t * 0.6;
            scale += hover_eased * 0.04;
        }
        Icon::Stop => {
            scale = 1.0 - press_t * 0.12 + hover_eased * 0.04 - impulse * 0.08;
        }
        Icon::Download => {
            offset.y = hover_eased * 1.1 + impulse * 2.2;
        }
        Icon::External => {
            offset.x = (hover_eased * 1.1 + impulse * 1.8) * 0.8;
            offset.y = -(hover_eased * 1.2 + impulse * 2.2);
        }
        Icon::Trash => {
            rotation = (hover_eased * -0.14) + ((click_p * PI * 4.0).sin() * impulse * 0.22);
            offset.y = -impulse * 1.2;
        }
        Icon::Star => {
            rotation = hover_eased * 0.18 + impulse * 0.35;
            scale += impulse * 0.12;
        }
        Icon::Copy | Icon::Duplicate => {
            offset.y = -impulse * 1.8;
            scale += impulse * 0.1;
        }
        Icon::Check => {
            if click_t.is_some() {
                scale = 0.82 + 0.26 * ease_out_back(click_p);
            }
        }
        Icon::Search => {
            rotation = -hover_eased * 0.14 + impulse * 0.18;
            scale += hover_eased * 0.05;
        }
        Icon::Folder => {
            offset.y = -hover_eased * 0.8 - impulse * 1.4;
            rotation = -impulse * 0.08;
        }
        Icon::Compass => {
            rotation = hover_eased * 0.22 + impulse * 0.45;
        }
        Icon::ChevronLeft => {
            offset.x = -(hover_eased * 1.1 + impulse * 1.8);
        }
        Icon::ChevronRight => {
            offset.x = hover_eased * 1.1 + impulse * 1.8;
        }
        Icon::Close => {
            rotation = hover_eased * (PI * 0.25) + impulse * (PI * 0.25);
        }
        Icon::Sparkle => {
            scale += impulse * 0.12;
            rotation = hover_eased * 0.18 + impulse * 0.35;
        }
        Icon::Terminal
        | Icon::User
        | Icon::Layers
        | Icon::Grid
        | Icon::Server
        | Icon::List
        | Icon::Alert => {
            offset.y = -hover_eased * 0.6 - impulse * 1.1;
        }
    }

    IconAnim {
        offset,
        scale: scale.clamp(0.72, 1.38),
        rotation,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Subtle foreground action feedback (`ActionFx`)
// ─────────────────────────────────────────────────────────────────────────────

const MAX_RINGS: usize = 24;

#[derive(Clone)]
struct FxRing {
    pos: Pos2,
    color: Color32,
    max_radius: f32,
    start_time: f64,
    duration: f32,
}

#[derive(Default, Clone)]
struct ActionFxState {
    rings: Vec<FxRing>,
    screen_veil_start: Option<f64>,
}

impl ActionFxState {
    fn prune_expired(&mut self, now: f64) {
        self.rings
            .retain(|r| (now - r.start_time) < r.duration as f64 && (now - r.start_time) >= -0.01);
        if let Some(t0) = self.screen_veil_start {
            if (now - t0) >= 0.20 {
                self.screen_veil_start = None;
            }
        }
    }

    fn is_active(&self) -> bool {
        !self.rings.is_empty() || self.screen_veil_start.is_some()
    }

    fn push_ring(&mut self, ring: FxRing) {
        if self.rings.len() >= MAX_RINGS {
            self.rings.remove(0);
        }
        self.rings.push(ring);
    }
}

fn fx_state_id() -> Id {
    Id::new("sunset_global_action_fx_v1")
}

fn with_fx_state<R>(ctx: &egui::Context, f: impl FnOnce(&mut ActionFxState, f64) -> R) -> R {
    let now = ctx.input(|i| i.time);
    let id = fx_state_id();
    let mut state: ActionFxState = ctx.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    state.prune_expired(now);
    let out = f(&mut state, now);
    ctx.data_mut(|d| d.insert_temp(id, state));
    out
}

/// Returns the current pointer interact position, or falls back to the screen center.
pub fn pointer_or_center(ctx: &egui::Context) -> Pos2 {
    ctx.input(|i| i.pointer.interact_pos().or_else(|| i.pointer.latest_pos()))
        .unwrap_or_else(|| ctx.screen_rect().center())
}

/// Emits a subtle expanding shockwave ring at `pos` (in screen coordinates).
pub fn emit_ring(ctx: &egui::Context, pos: Pos2, color: Color32, max_radius: f32) {
    with_fx_state(ctx, |st, now| {
        st.push_ring(FxRing {
            pos,
            color,
            max_radius: max_radius.clamp(12.0, 24.0),
            start_time: now,
            duration: 0.30,
        });
    });
    ctx.request_repaint();
}

/// Emits a subtle ring pulse at `pos` for primary confirmations.
pub fn emit_burst(ctx: &egui::Context, pos: Pos2, color: Color32, _count: usize) {
    with_fx_state(ctx, |st, now| {
        st.push_ring(FxRing {
            pos,
            color,
            max_radius: 22.0,
            start_time: now,
            duration: 0.32,
        });
    });
    ctx.request_repaint();
}

/// Kept as a clean no-op so action sites rely on in-place widget morphs and toasts without floating clutter.
pub fn emit_floating_badge(
    _ctx: &egui::Context,
    _pos: Pos2,
    _icon: Option<Icon>,
    _text: impl Into<String>,
    _color: Color32,
) {
}

/// Kept as a clean no-op so window borders never flash on clicks.
pub fn emit_edge_pulse(_ctx: &egui::Context, _color: Color32) {}

/// Triggers a smooth 0.18s full-window crossfade veil (used on language or font switch).
pub fn trigger_screen_veil(ctx: &egui::Context) {
    with_fx_state(ctx, |st, now| {
        st.screen_veil_start = Some(now);
    });
    ctx.request_repaint();
}

/// Renders active foreground pulse rings and language/font crossfade veil.
/// Call once at the end of `McLauncherApp::update`.
pub fn render_action_fx(ctx: &egui::Context) {
    let (state, now) = with_fx_state(ctx, |st, now| (st.clone(), now));
    if !state.is_active() {
        return;
    }
    ctx.request_repaint();

    let painter = ctx.layer_painter(LayerId::new(
        Order::Tooltip,
        Id::new("sunset_action_fx_layer"),
    ));
    let screen = ctx.screen_rect();

    // 1. Full-window crossfade veil (language / font switch)
    if let Some(t0) = state.screen_veil_start {
        let p = ((now - t0) as f32 / 0.18).clamp(0.0, 1.0);
        paint_fade_veil(&painter, screen, crate::theme::bg_void(), p, 90.0);
    }

    // 2. Subtle expanding pulse rings
    for ring in &state.rings {
        let t = ((now - ring.start_time) as f32 / ring.duration).clamp(0.0, 1.0);
        let eased = ease_out_quint(t);
        let inv = 1.0 - t;
        let r = 4.0 + eased * ring.max_radius;
        let stroke_a = (inv * inv * 95.0).round() as u8;
        if stroke_a > 1 {
            painter.circle_stroke(
                ring.pos,
                r,
                Stroke::new(
                    1.25 * inv + 0.35,
                    Color32::from_rgba_unmultiplied(
                        ring.color.r(),
                        ring.color.g(),
                        ring.color.b(),
                        stroke_a,
                    ),
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_curves_hit_endpoints_and_back_overshoots() {
        assert!((ease_out_cubic(0.0) - 0.0).abs() < 1e-5);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < 1e-5);
        assert!((ease_out_quint(0.0) - 0.0).abs() < 1e-5);
        assert!((ease_out_quint(1.0) - 1.0).abs() < 1e-5);
        assert!((ease_out_expo(0.0) - 0.0).abs() < 1e-5);
        assert!((ease_out_expo(1.0) - 1.0).abs() < 1e-5);
        assert!((ease_in_out_cubic(0.5) - 0.5).abs() < 1e-5);
        assert!(
            ease_out_back(0.65) > 1.0,
            "ease_out_back must overshoot 1.0"
        );
        assert!((ease_out_back(1.0) - 1.0).abs() < 1e-5);
        assert!(impulse_wave(0.28) > 0.5);
        assert!(impulse_wave(1.0) < 0.15);
    }

    #[test]
    fn icon_anim_produces_distinct_curves_for_key_actions() {
        let refresh_click = compute_icon_anim(Icon::Refresh, 1.0, 0.0, Some(0.5));
        assert!(refresh_click.rotation > 1.5, "Refresh should spin on click");

        let play_hover = compute_icon_anim(Icon::Play, 1.0, 0.0, None);
        assert!(
            play_hover.offset.x > 0.5,
            "Play icon should nudge right on hover"
        );

        let trash_click = compute_icon_anim(Icon::Trash, 1.0, 0.0, Some(0.25));
        assert!(trash_click.scale > 1.0);
    }

    #[test]
    fn action_fx_state_caps_and_pruning_work() {
        let mut st = ActionFxState::default();
        for i in 0..40 {
            st.push_ring(FxRing {
                pos: Pos2::new(i as f32, 0.0),
                color: Color32::WHITE,
                max_radius: 20.0,
                start_time: i as f64 * 0.1,
                duration: 0.4,
            });
        }
        assert_eq!(st.rings.len(), MAX_RINGS);

        st.screen_veil_start = Some(0.0);
        assert!(st.is_active());

        st.prune_expired(100.0);
        assert!(!st.is_active());
    }
}
