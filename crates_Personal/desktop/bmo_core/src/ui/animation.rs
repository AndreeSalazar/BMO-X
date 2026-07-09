//! Animation system — tween interpolation with easing curves.
//!
//! Lightweight, no_std, no heap. Designed for window animations
//! (minimize/maximize/open/close), dock magnification, and UI transitions.
//!
//! ## Usage
//!
//! ```ignore
//! let mut tween = Tween::new(0.0, 1.0, 300, Easing::EaseInOutCubic);
//! while !tween.done() {
//!     let val = tween.tick(16); // 16ms = ~60fps
//!     window.opacity = val;
//! }
//! ```

/// Easing functions for natural-feeling animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    /// Linear: constant speed. Good for progress bars.
    Linear,
    /// Quadratic ease-in: fast start, slow end.
    EaseInQuad,
    /// Quadratic ease-out: slow start, fast end.
    EaseOutQuad,
    /// Cubic ease-in-out: natural acceleration + deceleration. Good for most animations.
    EaseInOutCubic,
    /// Bounce at the end. Good for notifications, dialog pop-in.
    EaseOutBounce,
    /// Spring-like overshoot. Good for dock icons, elastic UI.
    EaseOutBack,
    /// Instant (0ms). Used for state resets.
    None,
}

/// A single animation tween interpolating an f32 value.
#[derive(Debug, Clone)]
pub struct Tween {
    pub from: f32,
    pub to: f32,
    /// Duration in milliseconds.
    pub duration_ms: u32,
    elapsed_ms: u32,
    easing: Easing,
}

impl Tween {
    pub const fn new(from: f32, to: f32, duration_ms: u32, easing: Easing) -> Self {
        Self { from, to, duration_ms, elapsed_ms: 0, easing }
    }

    /// Advance the tween by `dt_ms` and return the current value.
    pub fn tick(&mut self, dt_ms: u32) -> f32 {
        self.elapsed_ms = (self.elapsed_ms + dt_ms).min(self.duration_ms);
        self.value()
    }

    /// Current value without advancing time.
    pub fn value(&self) -> f32 {
        if self.duration_ms == 0 || self.easing == Easing::None {
            return self.to;
        }
        let t = self.elapsed_ms as f32 / self.duration_ms as f32;
        let eased = apply_easing(t, self.easing);
        self.from + (self.to - self.from) * eased
    }

    pub fn done(&self) -> bool { self.elapsed_ms >= self.duration_ms }
    pub fn progress(&self) -> f32 {
        if self.duration_ms == 0 { return 1.0; }
        (self.elapsed_ms as f32 / self.duration_ms as f32).min(1.0)
    }

    /// Reset to play again from the beginning.
    pub fn reset(&mut self) { self.elapsed_ms = 0; }

    /// Reverse direction: swap from↔to and reset.
    pub fn reverse(&mut self) {
        core::mem::swap(&mut self.from, &mut self.to);
        self.elapsed_ms = 0;
    }
}

fn apply_easing(t: f32, e: Easing) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match e {
        Easing::Linear => t,
        Easing::EaseInQuad => t * t,
        Easing::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
        Easing::EaseInOutCubic => {
            if t < 0.5 { 4.0 * t * t * t } else { 1.0 - f32_pow(-2.0 * t + 2.0, 3) / 2.0 }
        }
        Easing::EaseOutBounce => ease_out_bounce(t),
        Easing::EaseOutBack => {
            let c = 1.70158;
            let t1 = t - 1.0;
            1.0 + (c + 1.0) * t1 * t1 * t1 + c * t1 * t1
        }
        Easing::None => 1.0,
    }
}

fn ease_out_bounce(t: f32) -> f32 {
    let n1 = 7.5625;
    let d1 = 2.75;
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t2 = t - 1.5 / d1;
        n1 * t2 * t2 + 0.75
    } else if t < 2.5 / d1 {
        let t2 = t - 2.25 / d1;
        n1 * t2 * t2 + 0.9375
    } else {
        let t2 = t - 2.625 / d1;
        n1 * t2 * t2 + 0.984375
    }
}

fn f32_pow(x: f32, n: u32) -> f32 {
    if n == 0 { return 1.0; }
    let mut r = x;
    for _ in 1..n { r *= x; }
    r
}

// ── Animation for multiple properties ─────────────────────────────

/// Animated properties for a window or UI element.
#[derive(Debug, Clone)]
pub struct AnimState {
    pub opacity: Tween,
    pub scale: Tween,
    pub position_y: Tween,
    pub active: bool,
}

impl AnimState {
    /// Open animation: fade in + slide up.
    pub fn open() -> Self {
        Self {
            opacity: Tween::new(0.0, 1.0, 250, Easing::EaseInOutCubic),
            scale: Tween::new(0.9, 1.0, 250, Easing::EaseOutBack),
            position_y: Tween::new(40.0, 0.0, 250, Easing::EaseInOutCubic),
            active: true,
        }
    }

    /// Close animation: fade out + slide down.
    pub fn close() -> Self {
        Self {
            opacity: Tween::new(1.0, 0.0, 200, Easing::EaseInQuad),
            scale: Tween::new(1.0, 0.9, 200, Easing::EaseInQuad),
            position_y: Tween::new(0.0, 40.0, 200, Easing::EaseInQuad),
            active: false,
        }
    }

    /// Minimize animation: shrink to dock.
    pub fn minimize() -> Self {
        Self {
            opacity: Tween::new(1.0, 0.0, 300, Easing::EaseInOutCubic),
            scale: Tween::new(1.0, 0.3, 300, Easing::EaseInOutCubic),
            position_y: Tween::new(0.0, 200.0, 300, Easing::EaseInOutCubic),
            active: true,
        }
    }

    /// Restore from minimize.
    pub fn restore() -> Self {
        Self {
            opacity: Tween::new(0.0, 1.0, 300, Easing::EaseInOutCubic),
            scale: Tween::new(0.3, 1.0, 300, Easing::EaseOutBack),
            position_y: Tween::new(200.0, 0.0, 300, Easing::EaseInOutCubic),
            active: true,
        }
    }

    /// Tick all animations by `dt_ms`.
    pub fn tick(&mut self, dt_ms: u32) {
        self.opacity.tick(dt_ms);
        self.scale.tick(dt_ms);
        self.position_y.tick(dt_ms);
    }

    pub fn done(&self) -> bool {
        self.opacity.done() && self.scale.done() && self.position_y.done()
    }

    /// Paused / idle state.
    pub fn idle() -> Self {
        Self {
            opacity: Tween::new(1.0, 1.0, 0, Easing::None),
            scale: Tween::new(1.0, 1.0, 0, Easing::None),
            position_y: Tween::new(0.0, 0.0, 0, Easing::None),
            active: true,
        }
    }

    pub fn is_idle(&self) -> bool {
        self.active && self.opacity.from == self.opacity.to
            && self.scale.from == self.scale.to
            && self.position_y.from == self.position_y.to
    }
}
