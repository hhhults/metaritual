/// Continuous signal types — trajectories through time.
///
/// A Signal is a function from time to value. It can be a constant,
/// a piecewise linear curve (automation), an LFO, or a derived
/// (transformed) signal. Signals are the continuous counterpart
/// to Pattern's discrete events.

use std::f64::consts::PI;
use std::fmt;
use std::ops::{Add, Mul};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Wave shapes
// ---------------------------------------------------------------------------

/// Oscillator wave shapes for LFOs and signal generation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WaveShape {
    Sine,
    Triangle,
    Square,
    Saw,
}

impl WaveShape {
    /// Evaluate the wave shape at a given phase (0.0..1.0).
    /// Output is in -1.0..1.0.
    pub fn eval(&self, phase: f64) -> f64 {
        let p = phase.rem_euclid(1.0);
        match self {
            WaveShape::Sine => (p * 2.0 * PI).sin(),
            WaveShape::Triangle => {
                if p < 0.25 {
                    p * 4.0
                } else if p < 0.75 {
                    2.0 - p * 4.0
                } else {
                    p * 4.0 - 4.0
                }
            }
            WaveShape::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            WaveShape::Saw => 2.0 * p - 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Breakpoint
// ---------------------------------------------------------------------------

/// A point on a piecewise-linear curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Breakpoint {
    pub time: f64,
    pub value: f64,
}

impl Breakpoint {
    pub fn new(time: f64, value: f64) -> Self {
        Self { time, value }
    }
}

// ---------------------------------------------------------------------------
// Signal
// ---------------------------------------------------------------------------

/// A continuous signal — a function from time to value.
#[derive(Clone)]
pub enum Signal {
    /// Static value (a knob at rest).
    Const(f64),

    /// Piecewise-linear automation curve.
    /// Breakpoints must be sorted by time. Values are linearly interpolated.
    /// Before the first breakpoint: first breakpoint's value.
    /// After the last breakpoint: last breakpoint's value.
    Curve(Vec<Breakpoint>),

    /// Low-frequency oscillator.
    LFO {
        shape: WaveShape,
        rate: f64,   // cycles per unit time
        phase: f64,  // initial phase offset (0.0..1.0)
        center: f64, // center value
        depth: f64,  // amplitude (output ranges center ± depth)
    },

    /// Sum of two signals.
    Sum(Box<Signal>, Box<Signal>),

    /// Product of two signals.
    Product(Box<Signal>, Box<Signal>),

    /// Mapped through a closure. Stores a label for Debug.
    Map(Box<Signal>, Arc<dyn Fn(f64) -> f64 + Send + Sync>, String),
}

impl fmt::Debug for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Signal::Const(v) => write!(f, "Const({v})"),
            Signal::Curve(bp) => f.debug_tuple("Curve").field(bp).finish(),
            Signal::LFO { shape, rate, phase, center, depth } => {
                f.debug_struct("LFO")
                    .field("shape", shape)
                    .field("rate", rate)
                    .field("phase", phase)
                    .field("center", center)
                    .field("depth", depth)
                    .finish()
            }
            Signal::Sum(a, b) => f.debug_tuple("Sum").field(a).field(b).finish(),
            Signal::Product(a, b) => f.debug_tuple("Product").field(a).field(b).finish(),
            Signal::Map(inner, _, label) => {
                f.debug_tuple("Map").field(inner).field(label).finish()
            }
        }
    }
}

impl Signal {
    // --- Constructors --------------------------------------------------------

    /// Constant signal.
    pub fn constant(value: f64) -> Self {
        Signal::Const(value)
    }

    /// Piecewise-linear curve from breakpoints.
    /// Breakpoints are sorted by time automatically.
    pub fn curve(mut breakpoints: Vec<Breakpoint>) -> Self {
        breakpoints.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        Signal::Curve(breakpoints)
    }

    /// Sine LFO with given rate, centered at 0.0 with depth 1.0.
    pub fn sine(rate: f64) -> Self {
        Signal::LFO {
            shape: WaveShape::Sine,
            rate,
            phase: 0.0,
            center: 0.0,
            depth: 1.0,
        }
    }

    /// Triangle LFO.
    pub fn triangle(rate: f64) -> Self {
        Signal::LFO {
            shape: WaveShape::Triangle,
            rate,
            phase: 0.0,
            center: 0.0,
            depth: 1.0,
        }
    }

    /// Square LFO.
    pub fn square(rate: f64) -> Self {
        Signal::LFO {
            shape: WaveShape::Square,
            rate,
            phase: 0.0,
            center: 0.0,
            depth: 1.0,
        }
    }

    /// Saw LFO.
    pub fn saw(rate: f64) -> Self {
        Signal::LFO {
            shape: WaveShape::Saw,
            rate,
            phase: 0.0,
            center: 0.0,
            depth: 1.0,
        }
    }

    /// LFO with full control.
    pub fn lfo(shape: WaveShape, rate: f64, phase: f64, center: f64, depth: f64) -> Self {
        Signal::LFO {
            shape,
            rate,
            phase,
            center,
            depth,
        }
    }

    // --- Evaluation ----------------------------------------------------------

    /// Evaluate the signal at a point in time.
    pub fn sample(&self, time: f64) -> f64 {
        match self {
            Signal::Const(v) => *v,

            Signal::Curve(breakpoints) => {
                if breakpoints.is_empty() {
                    return 0.0;
                }
                if breakpoints.len() == 1 {
                    return breakpoints[0].value;
                }

                // Before first breakpoint
                if time <= breakpoints[0].time {
                    return breakpoints[0].value;
                }
                // After last breakpoint
                if time >= breakpoints[breakpoints.len() - 1].time {
                    return breakpoints[breakpoints.len() - 1].value;
                }

                // Find the two surrounding breakpoints and interpolate
                for i in 0..breakpoints.len() - 1 {
                    let a = &breakpoints[i];
                    let b = &breakpoints[i + 1];
                    if time >= a.time && time <= b.time {
                        let dt = b.time - a.time;
                        if dt.abs() < 1e-15 {
                            return b.value;
                        }
                        let t = (time - a.time) / dt;
                        return a.value + t * (b.value - a.value);
                    }
                }

                // Shouldn't reach here, but just in case
                breakpoints[breakpoints.len() - 1].value
            }

            Signal::LFO {
                shape,
                rate,
                phase,
                center,
                depth,
            } => {
                let p = time * rate + phase;
                center + depth * shape.eval(p)
            }

            Signal::Sum(a, b) => a.sample(time) + b.sample(time),
            Signal::Product(a, b) => a.sample(time) * b.sample(time),
            Signal::Map(inner, f, _) => f(inner.sample(time)),
        }
    }

    /// Evaluate the signal at N evenly-spaced points in [start, end].
    pub fn sample_n(&self, start: f64, end: f64, n: usize) -> Vec<f64> {
        if n == 0 {
            return vec![];
        }
        if n == 1 {
            return vec![self.sample(start)];
        }
        let step = (end - start) / (n - 1) as f64;
        (0..n).map(|i| self.sample(start + i as f64 * step)).collect()
    }

    // --- Transforms ----------------------------------------------------------

    /// Apply a closure to the output of this signal.
    pub fn map(self, f: impl Fn(f64) -> f64 + Send + Sync + 'static, label: &str) -> Self {
        Signal::Map(Box::new(self), Arc::new(f), label.to_string())
    }

    /// Whether this signal is a constant value.
    pub fn is_const(&self) -> bool {
        matches!(self, Signal::Const(_))
    }

    /// Scale the output: `value * scale + offset`.
    pub fn scale(self, scale: f64, offset: f64) -> Self {
        // We can express this as: self * Const(scale) + Const(offset)
        Signal::Sum(
            Box::new(Signal::Product(
                Box::new(self),
                Box::new(Signal::Const(scale)),
            )),
            Box::new(Signal::Const(offset)),
        )
    }
}

// ---------------------------------------------------------------------------
// Operator overloads
// ---------------------------------------------------------------------------

impl Add for Signal {
    type Output = Signal;
    fn add(self, rhs: Signal) -> Signal {
        Signal::Sum(Box::new(self), Box::new(rhs))
    }
}

impl Mul for Signal {
    type Output = Signal;
    fn mul(self, rhs: Signal) -> Signal {
        Signal::Product(Box::new(self), Box::new(rhs))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    // --- Const ---------------------------------------------------------------

    #[test]
    fn const_always_returns_value() {
        let s = Signal::constant(42.0);
        assert!(approx(s.sample(0.0), 42.0));
        assert!(approx(s.sample(100.0), 42.0));
        assert!(approx(s.sample(-5.0), 42.0));
    }

    // --- Curve ---------------------------------------------------------------

    #[test]
    fn curve_single_breakpoint() {
        let s = Signal::curve(vec![Breakpoint::new(0.0, 5.0)]);
        assert!(approx(s.sample(0.0), 5.0));
        assert!(approx(s.sample(10.0), 5.0));
    }

    #[test]
    fn curve_interpolates_linearly() {
        let s = Signal::curve(vec![
            Breakpoint::new(0.0, 0.0),
            Breakpoint::new(1.0, 10.0),
        ]);
        assert!(approx(s.sample(0.0), 0.0));
        assert!(approx(s.sample(0.5), 5.0));
        assert!(approx(s.sample(1.0), 10.0));
    }

    #[test]
    fn curve_holds_before_first() {
        let s = Signal::curve(vec![
            Breakpoint::new(1.0, 5.0),
            Breakpoint::new(2.0, 10.0),
        ]);
        assert!(approx(s.sample(0.0), 5.0));
        assert!(approx(s.sample(-1.0), 5.0));
    }

    #[test]
    fn curve_holds_after_last() {
        let s = Signal::curve(vec![
            Breakpoint::new(0.0, 0.0),
            Breakpoint::new(1.0, 10.0),
        ]);
        assert!(approx(s.sample(2.0), 10.0));
        assert!(approx(s.sample(100.0), 10.0));
    }

    #[test]
    fn curve_multi_segment() {
        let s = Signal::curve(vec![
            Breakpoint::new(0.0, 0.0),
            Breakpoint::new(1.0, 10.0),
            Breakpoint::new(2.0, 5.0),
        ]);
        assert!(approx(s.sample(0.5), 5.0));
        assert!(approx(s.sample(1.5), 7.5));
    }

    #[test]
    fn curve_sorts_breakpoints() {
        let s = Signal::curve(vec![
            Breakpoint::new(1.0, 10.0),
            Breakpoint::new(0.0, 0.0),
        ]);
        assert!(approx(s.sample(0.5), 5.0));
    }

    #[test]
    fn curve_empty_returns_zero() {
        let s = Signal::Curve(vec![]);
        assert!(approx(s.sample(0.0), 0.0));
    }

    // --- LFO -----------------------------------------------------------------

    #[test]
    fn sine_at_zero_is_center() {
        let s = Signal::sine(1.0);
        assert!(approx(s.sample(0.0), 0.0));
    }

    #[test]
    fn sine_quarter_cycle_is_peak() {
        let s = Signal::sine(1.0);
        assert!(approx(s.sample(0.25), 1.0));
    }

    #[test]
    fn sine_half_cycle_returns_to_zero() {
        let s = Signal::sine(1.0);
        assert!(s.sample(0.5).abs() < 1e-9);
    }

    #[test]
    fn sine_period() {
        let s = Signal::sine(1.0);
        assert!(approx(s.sample(0.0), s.sample(1.0)));
        assert!(approx(s.sample(0.3), s.sample(1.3)));
    }

    #[test]
    fn lfo_center_and_depth() {
        let s = Signal::lfo(WaveShape::Sine, 1.0, 0.0, 5.0, 3.0);
        // At peak (t=0.25): center + depth = 8.0
        assert!(approx(s.sample(0.25), 8.0));
        // At trough (t=0.75): center - depth = 2.0
        assert!(approx(s.sample(0.75), 2.0));
    }

    #[test]
    fn lfo_phase_offset() {
        let s = Signal::lfo(WaveShape::Sine, 1.0, 0.25, 0.0, 1.0);
        // phase=0.25 means at t=0 we're at the peak
        assert!(approx(s.sample(0.0), 1.0));
    }

    #[test]
    fn triangle_peaks_at_quarter() {
        let s = Signal::triangle(1.0);
        assert!(approx(s.sample(0.25), 1.0));
    }

    #[test]
    fn triangle_trough_at_three_quarter() {
        let s = Signal::triangle(1.0);
        assert!(approx(s.sample(0.75), -1.0));
    }

    #[test]
    fn square_high_then_low() {
        let s = Signal::square(1.0);
        assert!(approx(s.sample(0.1), 1.0));
        assert!(approx(s.sample(0.6), -1.0));
    }

    #[test]
    fn saw_ramps_linearly() {
        let s = Signal::saw(1.0);
        assert!(approx(s.sample(0.0), -1.0));
        assert!(approx(s.sample(0.5), 0.0));
        // Just before 1.0
        assert!((s.sample(0.999) - 1.0).abs() < 0.01);
    }

    #[test]
    fn lfo_rate_scales_frequency() {
        let s = Signal::sine(2.0); // 2 cycles per unit time
        // At t=0.125, phase = 0.25 → peak
        assert!(approx(s.sample(0.125), 1.0));
    }

    // --- Arithmetic ----------------------------------------------------------

    #[test]
    fn sum_adds_signals() {
        let a = Signal::constant(3.0);
        let b = Signal::constant(4.0);
        let s = a + b;
        assert!(approx(s.sample(0.0), 7.0));
    }

    #[test]
    fn product_multiplies_signals() {
        let a = Signal::constant(3.0);
        let b = Signal::constant(4.0);
        let s = a * b;
        assert!(approx(s.sample(0.0), 12.0));
    }

    #[test]
    fn sum_with_lfo() {
        let base = Signal::constant(60.0);
        let vibrato = Signal::lfo(WaveShape::Sine, 5.0, 0.0, 0.0, 2.0);
        let s = base + vibrato;
        assert!(approx(s.sample(0.0), 60.0)); // sine at 0 = 0
        assert!(approx(s.sample(0.05), 62.0)); // sine at peak
    }

    #[test]
    fn product_ring_modulation() {
        let a = Signal::sine(1.0);
        let b = Signal::sine(2.0);
        let s = a * b;
        // At t=0, both sine(0) = 0, so product = 0
        assert!(approx(s.sample(0.0), 0.0));
    }

    // --- Scale ---------------------------------------------------------------

    #[test]
    fn scale_transforms_output() {
        let s = Signal::constant(1.0).scale(10.0, 5.0);
        assert!(approx(s.sample(0.0), 15.0)); // 1.0 * 10.0 + 5.0
    }

    // --- sample_n ------------------------------------------------------------

    #[test]
    fn sample_n_returns_correct_count() {
        let s = Signal::constant(1.0);
        assert_eq!(s.sample_n(0.0, 1.0, 10).len(), 10);
    }

    #[test]
    fn sample_n_zero_returns_empty() {
        let s = Signal::constant(1.0);
        assert!(s.sample_n(0.0, 1.0, 0).is_empty());
    }

    #[test]
    fn sample_n_one_returns_start() {
        let s = Signal::curve(vec![
            Breakpoint::new(0.0, 0.0),
            Breakpoint::new(1.0, 10.0),
        ]);
        let result = s.sample_n(0.0, 1.0, 1);
        assert_eq!(result.len(), 1);
        assert!(approx(result[0], 0.0));
    }

    #[test]
    fn sample_n_evenly_spaced() {
        let s = Signal::curve(vec![
            Breakpoint::new(0.0, 0.0),
            Breakpoint::new(1.0, 10.0),
        ]);
        let result = s.sample_n(0.0, 1.0, 5);
        assert!(approx(result[0], 0.0));
        assert!(approx(result[1], 2.5));
        assert!(approx(result[2], 5.0));
        assert!(approx(result[3], 7.5));
        assert!(approx(result[4], 10.0));
    }

    // --- Map -----------------------------------------------------------------

    #[test]
    fn map_applies_function() {
        let s = Signal::constant(4.0).map(f64::sqrt, "sqrt");
        assert!(approx(s.sample(0.0), 2.0));
    }

    #[test]
    fn map_captures_environment() {
        let scale = 10.0;
        let offset = 5.0;
        let s = Signal::constant(3.0).map(move |v| v * scale + offset, "affine");
        assert!(approx(s.sample(0.0), 35.0)); // 3.0 * 10.0 + 5.0
    }

    #[test]
    fn map_closure_clones_correctly() {
        let threshold = 0.5;
        let s = Signal::sine(1.0).map(move |v| if v > threshold { 1.0 } else { 0.0 }, "gate");
        let s2 = s.clone();
        // Both should produce the same output
        assert!(approx(s.sample(0.25), s2.sample(0.25)));
    }

    #[test]
    fn is_const() {
        assert!(Signal::constant(1.0).is_const());
        assert!(!Signal::sine(1.0).is_const());
    }

    // --- WaveShape eval directly ---------------------------------------------

    #[test]
    fn wave_shape_sine_range() {
        for i in 0..100 {
            let p = i as f64 / 100.0;
            let v = WaveShape::Sine.eval(p);
            assert!(v >= -1.0 - EPS && v <= 1.0 + EPS);
        }
    }

    #[test]
    fn wave_shape_triangle_range() {
        for i in 0..100 {
            let p = i as f64 / 100.0;
            let v = WaveShape::Triangle.eval(p);
            assert!(v >= -1.0 - EPS && v <= 1.0 + EPS);
        }
    }

    #[test]
    fn wave_shape_square_only_two_values() {
        for i in 0..100 {
            let p = i as f64 / 100.0;
            let v = WaveShape::Square.eval(p);
            assert!(approx(v, 1.0) || approx(v, -1.0));
        }
    }

    #[test]
    fn wave_shape_saw_range() {
        for i in 0..100 {
            let p = i as f64 / 100.0;
            let v = WaveShape::Saw.eval(p);
            assert!(v >= -1.0 - EPS && v <= 1.0 + EPS);
        }
    }
}
