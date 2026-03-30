/// Bridge operations between discrete (Pattern) and continuous (Signal).
///
/// These morphisms cross the boundary:
/// - `render`: Pattern → Signal — step sequence becomes a curve
/// - `onset_detect`: Signal → Pattern — threshold crossings become events
/// - `sample_and_hold`: sample a signal at pattern event times

use crate::pattern::Pattern;
use crate::signal::{Breakpoint, Signal};

// ---------------------------------------------------------------------------
// render: Pattern → Signal
// ---------------------------------------------------------------------------

/// Convert a pattern to a step-function Signal (sample-and-hold curve).
///
/// Each event's value is held for its duration. The resulting curve is
/// a series of breakpoints creating a step function.
///
/// `beats_per_cycle` converts cycle-relative time to beat time.
pub fn render(pattern: &Pattern, beats_per_cycle: f64) -> Signal {
    let events = pattern.events();
    if events.is_empty() {
        return Signal::Const(0.0);
    }

    let mut breakpoints = Vec::with_capacity(events.len() * 2);
    let tiny = 1e-12; // sub-sample offset for step transitions

    for e in &events {
        let start = e.start * beats_per_cycle;
        let end = (e.start + e.duration) * beats_per_cycle;

        // Step function: jump to value just after start, hold until end
        if breakpoints.is_empty() || start > 0.0 {
            breakpoints.push(Breakpoint::new(start, e.value));
        } else {
            // First event at t=0: just set the value
            breakpoints.push(Breakpoint::new(0.0, e.value));
        }
        // Hold value until just before the next event
        breakpoints.push(Breakpoint::new(end - tiny, e.value));
    }

    Signal::curve(breakpoints)
}

/// Convert a pattern to a linearly-interpolated Signal.
///
/// Each event places a breakpoint at its midpoint (start + duration/2).
/// Values are linearly interpolated between event midpoints.
pub fn render_smooth(pattern: &Pattern, beats_per_cycle: f64) -> Signal {
    let events = pattern.events();
    if events.is_empty() {
        return Signal::Const(0.0);
    }

    let breakpoints: Vec<Breakpoint> = events
        .iter()
        .map(|e| {
            let mid = (e.start + e.duration / 2.0) * beats_per_cycle;
            Breakpoint::new(mid, e.value)
        })
        .collect();

    Signal::curve(breakpoints)
}

// ---------------------------------------------------------------------------
// onset_detect: Signal → Pattern
// ---------------------------------------------------------------------------

/// Detect upward threshold crossings in a signal, producing a rhythm pattern.
///
/// Samples the signal at `resolution` evenly-spaced points over [0, duration).
/// Each upward crossing of `threshold` becomes a hit (value 1.0).
/// Returns a pattern with duration normalized to 1.0 cycle.
pub fn onset_detect(signal: &Signal, duration: f64, threshold: f64, resolution: usize) -> Pattern {
    if resolution < 2 {
        return Pattern::empty();
    }

    let step = duration / resolution as f64;
    let samples = signal.sample_n(0.0, duration - step, resolution);

    let mut onsets: Vec<f64> = Vec::new(); // times of crossings

    for i in 1..samples.len() {
        if samples[i - 1] < threshold && samples[i] >= threshold {
            onsets.push(i as f64 * step);
        }
    }

    if onsets.is_empty() {
        return Pattern::empty();
    }

    // Build a pattern from onset times
    let atom_dur = 1.0 / onsets.len() as f64;
    Pattern::Seq(
        onsets
            .into_iter()
            .map(|t| {
                // Normalize time to 0..1 cycle
                let normalized_start = t / duration;
                // Store the onset time as the value (preserves timing info)
                Pattern::Atom(normalized_start, atom_dur)
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// sample_and_hold: (Signal, Pattern) → Pattern
// ---------------------------------------------------------------------------

/// Sample a signal at each event's start time, producing a new pattern
/// with the signal's values but the original pattern's timing.
///
/// This is how a continuous parameter (e.g., an LFO) gets "quantized"
/// to a pattern's rhythmic grid.
pub fn sample_and_hold(signal: &Signal, pattern: &Pattern, beats_per_cycle: f64) -> Pattern {
    let events = pattern.events();
    if events.is_empty() {
        return Pattern::empty();
    }

    Pattern::Seq(
        events
            .iter()
            .map(|e| {
                let t = e.start * beats_per_cycle;
                let value = signal.sample(t);
                Pattern::Atom(value, e.duration)
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// quantize: Signal → Pattern (regular sampling)
// ---------------------------------------------------------------------------

/// Sample a signal at regular intervals, producing an evenly-spaced pattern.
///
/// `steps` determines how many samples per cycle.
pub fn quantize(signal: &Signal, steps: usize, beats_per_cycle: f64) -> Pattern {
    if steps == 0 {
        return Pattern::empty();
    }

    let dur = 1.0 / steps as f64;
    Pattern::Seq(
        (0..steps)
            .map(|i| {
                let t = i as f64 * dur * beats_per_cycle;
                let value = signal.sample(t);
                Pattern::Atom(value, dur)
            })
            .collect(),
    )
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

    // --- render --------------------------------------------------------------

    #[test]
    fn render_basic_step_function() {
        let p = Pattern::seq(&[60.0, 64.0, 67.0]);
        let s = render(&p, 4.0);
        // At t=0: first event value = 60.0
        assert!(approx(s.sample(0.0), 60.0));
    }

    #[test]
    fn render_second_event() {
        let p = Pattern::seq(&[60.0, 64.0]);
        let s = render(&p, 4.0);
        // Second event starts at 0.5 cycles = 2.0 beats
        // Just inside the second step
        assert!(approx(s.sample(2.0), 64.0));
    }

    #[test]
    fn render_empty_is_const_zero() {
        let p = Pattern::empty();
        let s = render(&p, 4.0);
        assert!(approx(s.sample(0.0), 0.0));
        assert!(approx(s.sample(1.0), 0.0));
    }

    #[test]
    fn render_smooth_interpolates() {
        let p = Pattern::seq(&[0.0, 10.0]);
        let s = render_smooth(&p, 4.0);
        // Midpoints: first at 0.25*4=1.0, second at 0.75*4=3.0
        assert!(approx(s.sample(1.0), 0.0));
        assert!(approx(s.sample(3.0), 10.0));
        // Between: should interpolate
        assert!(approx(s.sample(2.0), 5.0));
    }

    // --- onset_detect --------------------------------------------------------

    #[test]
    fn onset_detect_sine_crossing() {
        // A sine wave with 2 cycles: crosses 0 going up at t≈0.5 and t≈1.5
        // (it starts at 0, goes positive, comes back through 0 at t=0.5 going negative,
        //  then crosses 0 going up at t=1.0)
        let s = Signal::sine(1.0);
        // Over 2 full cycles, sine crosses threshold=0.0 going up twice
        // (at t≈1.0 and at t≈2.0, since at t=0 it starts AT 0 not crossing from below)
        let p = onset_detect(&s, 2.0, 0.0, 2000);
        assert!(p.len() >= 1);
    }

    #[test]
    fn onset_detect_no_crossings() {
        let s = Signal::constant(5.0);
        let p = onset_detect(&s, 1.0, 10.0, 100);
        // Constant below threshold — no crossings
        assert!(p.is_empty());
    }

    #[test]
    fn onset_detect_square_wave() {
        // Square wave at 2 Hz over 2 seconds: starts high, goes low at t=0.25,
        // crosses back up at t=0.5, down at t=0.75, up at t=1.0, etc.
        // Over 2 seconds we get crossings at t=0.5, t=1.0, t=1.5
        let s = Signal::square(2.0);
        let p = onset_detect(&s, 2.0, 0.0, 2000);
        assert!(p.len() >= 2);
    }

    // --- sample_and_hold -----------------------------------------------------

    #[test]
    fn sample_and_hold_captures_signal_values() {
        let rhythm = Pattern::seq(&[1.0, 1.0, 1.0, 1.0]); // 4 equal events
        let lfo = Signal::lfo(WaveShape::Saw, 1.0, 0.0, 60.0, 12.0);
        let result = sample_and_hold(&lfo, &rhythm, 1.0);
        let evs = result.events();
        assert_eq!(evs.len(), 4);
        // Each event should have the LFO value at its start time
        for (i, e) in evs.iter().enumerate() {
            let expected = lfo.sample(i as f64 * 0.25);
            assert!(approx(e.value, expected));
        }
    }

    #[test]
    fn sample_and_hold_preserves_timing() {
        let rhythm = Pattern::seq(&[1.0, 1.0]);
        let signal = Signal::constant(42.0);
        let result = sample_and_hold(&signal, &rhythm, 1.0);
        let evs = result.events();
        assert_eq!(evs.len(), 2);
        assert!(approx(evs[0].duration, 0.5));
        assert!(approx(evs[1].duration, 0.5));
    }

    #[test]
    fn sample_and_hold_empty_pattern() {
        let signal = Signal::constant(42.0);
        let result = sample_and_hold(&signal, &Pattern::empty(), 1.0);
        assert!(result.is_empty());
    }

    // --- quantize ------------------------------------------------------------

    #[test]
    fn quantize_samples_evenly() {
        let s = Signal::constant(5.0);
        let p = quantize(&s, 4, 1.0);
        let evs = p.events();
        assert_eq!(evs.len(), 4);
        for e in &evs {
            assert!(approx(e.value, 5.0));
            assert!(approx(e.duration, 0.25));
        }
    }

    #[test]
    fn quantize_captures_lfo() {
        let s = Signal::sine(1.0);
        let p = quantize(&s, 8, 1.0);
        let evs = p.events();
        assert_eq!(evs.len(), 8);
        // First sample at t=0: sin(0) = 0
        assert!(approx(evs[0].value, 0.0));
    }

    #[test]
    fn quantize_zero_steps() {
        let s = Signal::constant(1.0);
        let p = quantize(&s, 0, 1.0);
        assert!(p.is_empty());
    }

    #[test]
    fn quantize_duration_sums_to_one() {
        let s = Signal::sine(1.0);
        let p = quantize(&s, 16, 1.0);
        assert!(approx(p.duration(), 1.0));
    }
}
