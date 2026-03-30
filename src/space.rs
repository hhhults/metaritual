/// Space — spatial position as its own type.
///
/// Space gets its own type because it maps into multiple output domains:
/// stereo audio (pan/width), visual rendering (x/y), physical space
/// (speaker array). The same spatial description can drive all of them.

use crate::signal::Signal;

// ---------------------------------------------------------------------------
// Space
// ---------------------------------------------------------------------------

/// Spatial position driven by signals.
#[derive(Debug, Clone)]
pub struct Space {
    /// Stereo position: -1.0 (full left) to 1.0 (full right).
    pub pan: Signal,
    /// Stereo spread: 0.0 (mono) to 1.0 (full stereo).
    pub width: Signal,
    /// Distance / reverb send: 0.0 (close/dry) to 1.0 (far/wet).
    pub depth: Signal,
}

impl Space {
    /// Create a centered, full-width, close space.
    pub fn center() -> Self {
        Self {
            pan: Signal::Const(0.0),
            width: Signal::Const(1.0),
            depth: Signal::Const(0.0),
        }
    }

    /// Create a space at a fixed pan position.
    pub fn panned(pan: f64) -> Self {
        Self {
            pan: Signal::Const(pan),
            width: Signal::Const(1.0),
            depth: Signal::Const(0.0),
        }
    }

    /// Set pan to a signal.
    pub fn with_pan(mut self, pan: Signal) -> Self {
        self.pan = pan;
        self
    }

    /// Set width to a signal.
    pub fn with_width(mut self, width: Signal) -> Self {
        self.width = width;
        self
    }

    /// Set depth to a signal.
    pub fn with_depth(mut self, depth: Signal) -> Self {
        self.depth = depth;
        self
    }

    /// Sample all spatial dimensions at a point in time.
    pub fn sample(&self, time: f64) -> (f64, f64, f64) {
        (
            self.pan.sample(time).clamp(-1.0, 1.0),
            self.width.sample(time).clamp(0.0, 1.0),
            self.depth.sample(time).clamp(0.0, 1.0),
        )
    }
}

impl Default for Space {
    fn default() -> Self {
        Self::center()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::WaveShape;

    const EPS: f64 = 1e-9;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    #[test]
    fn center_is_default() {
        let s = Space::center();
        let (pan, width, depth) = s.sample(0.0);
        assert!(approx(pan, 0.0));
        assert!(approx(width, 1.0));
        assert!(approx(depth, 0.0));
    }

    #[test]
    fn panned_left() {
        let s = Space::panned(-1.0);
        let (pan, _, _) = s.sample(0.0);
        assert!(approx(pan, -1.0));
    }

    #[test]
    fn panned_right() {
        let s = Space::panned(1.0);
        let (pan, _, _) = s.sample(0.0);
        assert!(approx(pan, 1.0));
    }

    #[test]
    fn pan_lfo() {
        let s = Space::center()
            .with_pan(Signal::sine(1.0));
        // At t=0.25 (sine peak): pan = 1.0
        let (pan, _, _) = s.sample(0.25);
        assert!(approx(pan, 1.0));
        // At t=0.75 (sine trough): pan = -1.0
        let (pan, _, _) = s.sample(0.75);
        assert!(approx(pan, -1.0));
    }

    #[test]
    fn depth_with_signal() {
        let s = Space::center()
            .with_depth(Signal::lfo(WaveShape::Triangle, 0.5, 0.0, 0.5, 0.5));
        let (_, _, depth) = s.sample(0.0);
        assert!(approx(depth, 0.5)); // triangle starts at center
    }

    #[test]
    fn values_are_clamped() {
        let s = Space::center()
            .with_pan(Signal::Const(5.0))   // exceeds range
            .with_width(Signal::Const(-1.0)) // below range
            .with_depth(Signal::Const(2.0)); // exceeds range
        let (pan, width, depth) = s.sample(0.0);
        assert!(approx(pan, 1.0));
        assert!(approx(width, 0.0));
        assert!(approx(depth, 1.0));
    }
}
