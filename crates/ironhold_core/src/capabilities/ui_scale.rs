//! Reference-resolution UI scale (`ProjectConfig.ui_scale`).
//!
//! A project that sets `ui_scale: (reference: (1280.0, 720.0), ...)` authors its UI at that
//! resolution; this module keeps Bevy's global [`UiScale`] equal to "how much bigger or smaller
//! the real window is than the reference" so every fixed-pixel UI value (and every text size)
//! scales with it. A project with no `ui_scale` block keeps `UiScale` at exactly 1.0.
//!
//! Presentation only: nothing here is read by simulation, runs in `FixedUpdate`, or is a replay
//! input. See `planning/features/responsive_ui.md` (Phase 2).

use bevy::prelude::*;
use bevy::ui::UiScale;
use bevy::window::PrimaryWindow;

use crate::schema::project::{ProjectConfig, ProjectConfigHandle, UiScaleConfig, UiScaleFit};

/// Logical window size, or `None` when it carries no information.
///
/// A minimized PC window can report 0x0 (winit sends `Resized` 0x0 and Bevy writes it through),
/// and a hidden web host can report a 0px canvas. Dividing by it would compute a scale of 0 and
/// clamp to the floor, re-laying out the whole UI, so every consumer treats a degenerate size as
/// "no information": keep the last applied value and write nothing.
pub fn valid_window_size(width: f32, height: f32) -> Option<(f32, f32)> {
    (width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0).then_some((width, height))
}

/// The scale to apply for one window size, plus whether the `min_scale` floor had to raise it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiScaleResult {
    /// Final value written to [`UiScale`]: snapped down, then clamped to `[min_scale, max_scale]`.
    pub applied: f32,
    /// What the window actually needs, snapped down, before clamping (for display).
    pub needed: f32,
    /// What the window actually needs, unsnapped. The floor only makes the UI overflow when this is
    /// below `min_scale` (comparing the snapped value would warn falsely when `min_scale` is not a
    /// multiple of `snap`: a needed 0.65 snaps to 0.5, is raised to a 0.6 floor, and still fits).
    pub raw: f32,
}

impl UiScaleResult {
    /// `true` when the `min_scale` floor raised the scale, so the UI overflows the window.
    pub fn floored(&self, cfg: &UiScaleConfig) -> bool {
        self.raw < cfg.min_scale
    }
}

/// Scale for a (valid, see [`valid_window_size`]) window size under `cfg`.
///
/// The raw scale is `window / reference` per the `fit` mode, **snapped down** to a multiple of
/// `snap` (never to the nearest step: 0.88 -> 1.0 would be bigger than the window allows and
/// overflow it; 0.88 -> 0.75 fits), then clamped to `[min_scale, max_scale]`. Snapping exists
/// because a fractional scale blurs small text and multiplies glyph-atlas variants; 0.0 disables it.
pub fn compute_ui_scale(cfg: &UiScaleConfig, width: f32, height: f32) -> UiScaleResult {
    let (ref_w, ref_h) = cfg.reference;
    let raw = match cfg.fit {
        UiScaleFit::Fit => (width / ref_w).min(height / ref_h),
        UiScaleFit::FitWidth => width / ref_w,
        UiScaleFit::FitHeight => height / ref_h,
    };
    // The small epsilon keeps an exact multiple (e.g. 0.75 / 0.25 = 2.9999998) from dropping a step.
    let needed = if cfg.snap > 0.0 { ((raw / cfg.snap) + 1e-4).floor() * cfg.snap } else { raw };
    // `max().min()`, not `f32::clamp`: `clamp` panics when min > max or either bound is NaN, and the
    // runtime only logs a failed `ProjectConfig::validate()` (it stays in `Assets`, and a hot-reload is
    // never re-validated), so this runs every frame against whatever config is loaded.
    let applied = needed.max(cfg.min_scale).min(cfg.max_scale);
    UiScaleResult { applied, needed, raw }
}

/// Keeps [`UiScale`] equal to the configured reference scale for the current window.
///
/// Polled every frame instead of reacting to `WindowResized`: the work is a handful of float
/// operations, and polling also covers a project load, a hot-reloaded project and the first frame
/// with no event at all. The write is guarded (`UiScale` change = a full UI relayout), and a
/// project without a `ui_scale` block pulls the scale back to exactly 1.0 (hot-reload case).
pub fn ui_scale_system(
    handle: Option<Res<ProjectConfigHandle>>,
    configs: Option<Res<Assets<ProjectConfig>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Option<ResMut<UiScale>>,
    mut was_floored: Local<bool>,
) {
    let (Some(handle), Some(configs), Some(mut ui_scale)) = (handle, configs, ui_scale) else { return };
    let Some(config) = configs.get(&handle.0) else { return };

    let target = match &config.ui_scale {
        None => {
            *was_floored = false;
            1.0
        }
        // An invalid block is treated as off. The runtime only logs `ProjectConfig::validate()` at load
        // (and never re-validates a hot-reload), so the config this system reads can be anything.
        Some(cfg) if cfg.validate().is_err() => {
            *was_floored = false;
            1.0
        }
        Some(cfg) => {
            let Ok(window) = windows.single() else { return };
            let Some((w, h)) = valid_window_size(window.width(), window.height()) else { return };
            let result = compute_ui_scale(cfg, w, h);
            // Warn once when the floor starts raising the scale (not on every frame of a resize
            // drag): the UI will overflow the window, and nothing else says why.
            let floored = result.floored(cfg);
            if floored && !*was_floored {
                warn!(
                    "ui_scale: window {w:.0}x{h:.0} needs scale {:.2} but ui_scale.min_scale is {:.2}; \
                     the UI will overflow the window (lower ui_scale.min_scale; portrait layouts are not supported yet)",
                    result.raw, cfg.min_scale
                );
            }
            *was_floored = floored;
            result.applied
        }
    };

    if (ui_scale.0 - target).abs() > f32::EPSILON {
        ui_scale.0 = target;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(reference: (f32, f32), fit: UiScaleFit) -> UiScaleConfig {
        UiScaleConfig { reference, fit, min_scale: 0.5, max_scale: 2.0, snap: 0.25 }
    }

    #[test]
    fn fit_uses_the_smaller_axis() {
        let c = cfg((1280.0, 720.0), UiScaleFit::Fit);
        assert_eq!(compute_ui_scale(&c, 1920.0, 1080.0).applied, 1.5);
        // 2560x720: width would allow 2.0 but the height only 1.0.
        assert_eq!(compute_ui_scale(&c, 2560.0, 720.0).applied, 1.0);
    }

    #[test]
    fn fit_width_and_fit_height_use_one_axis() {
        assert_eq!(compute_ui_scale(&cfg((1280.0, 720.0), UiScaleFit::FitWidth), 2560.0, 720.0).applied, 2.0);
        assert_eq!(compute_ui_scale(&cfg((1280.0, 720.0), UiScaleFit::FitHeight), 2560.0, 720.0).applied, 1.0);
    }

    #[test]
    fn snaps_down_never_up() {
        let mut c = cfg((1000.0, 1000.0), UiScaleFit::Fit);
        // raw 0.88: nearest step would be 1.0 (bigger than the window), down gives 0.75.
        assert_eq!(compute_ui_scale(&c, 880.0, 880.0).applied, 0.75);
        // raw 0.65 -> 0.5, not 0.75.
        assert_eq!(compute_ui_scale(&c, 650.0, 650.0).applied, 0.5);
        // An exact multiple must not drop a step (float error: 0.75 / 0.25).
        assert_eq!(compute_ui_scale(&c, 750.0, 750.0).applied, 0.75);
        c.snap = 0.0;
        assert!((compute_ui_scale(&c, 880.0, 880.0).applied - 0.88).abs() < 1e-6, "snap 0 disables snapping");
    }

    #[test]
    fn clamps_to_min_and_max() {
        let c = cfg((1280.0, 720.0), UiScaleFit::Fit);
        assert_eq!(compute_ui_scale(&c, 3840.0, 2160.0).applied, 2.0);
        let small = compute_ui_scale(&c, 390.0, 844.0);
        assert_eq!(small.applied, 0.5, "floored to min_scale");
        assert_eq!(small.needed, 0.25, "390/1280 = 0.30 snaps down to 0.25");
        assert!(small.floored(&c));
        assert!(!compute_ui_scale(&c, 1280.0, 720.0).floored(&c));
    }

    #[test]
    fn degenerate_window_sizes_carry_no_information() {
        assert_eq!(valid_window_size(1280.0, 720.0), Some((1280.0, 720.0)));
        for (w, h) in [(0.0, 0.0), (0.0, 720.0), (1280.0, 0.0), (-1.0, 720.0), (f32::NAN, 720.0), (1280.0, f32::INFINITY)] {
            assert_eq!(valid_window_size(w, h), None, "({w}, {h})");
        }
    }
}
