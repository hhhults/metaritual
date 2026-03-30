/// Pattern transforms — endomorphisms on Pattern.
///
/// Each transform takes a Pattern and returns a new Pattern.
/// They compose via method chaining.
///
/// The strategy is uniform: `events()` → transform event list → rebuild via `from_events`.
/// This means all transforms operate on the flat, evaluated representation and produce
/// a flat Seq of Atoms. Structural nesting (Seq-of-Seqs, Stack) is not preserved — the
/// output is always a normalised sequence. This is intentional: transforms are a
/// compositional layer *above* structure.

use crate::pattern::{Event, Pattern};
use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Rebuild a Pattern from a flat event list.
///
/// Each event becomes an `Atom(value, duration)`. Events must be non-overlapping
/// and cover contiguous time; gaps produce `Empty` atoms of the appropriate width.
/// Returns `Pattern::Empty` when the slice is empty.
///
/// This is intentionally a free function (not a method) because it is an internal
/// reconstruction primitive, not part of the public combinator API.
fn from_events(events: &[Event]) -> Pattern {
    if events.is_empty() {
        return Pattern::Empty;
    }
    // Build a Seq whose children are Atoms in start-time order.
    // We do not attempt to reconstruct nested structure — just a flat sequence.
    let mut sorted = events.to_vec();
    sorted.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());

    let atoms: Vec<Pattern> = sorted
        .iter()
        .map(|e| Pattern::Atom(e.value, e.duration))
        .collect();

    match atoms.len() {
        1 => atoms.into_iter().next().unwrap(),
        _ => Pattern::Seq(atoms),
    }
}

/// Collect events sorted by start time.
fn sorted_events(p: &Pattern) -> Vec<Event> {
    let mut evs = p.events();
    evs.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
    evs
}

// ---------------------------------------------------------------------------
// Transforms
// ---------------------------------------------------------------------------

impl Pattern {
    // ------------------------------------------------------------------
    // 1. rotate(k)
    // ------------------------------------------------------------------
    /// Rotate the value sequence by `k` positions, keeping timing intact.
    ///
    /// Positive `k` shifts values to the right (the last value wraps to the
    /// front). Negative `k` shifts left.
    ///
    /// ```text
    /// values [a, b, c, d], k=1  →  [d, a, b, c]
    /// values [a, b, c, d], k=-1 →  [b, c, d, a]
    /// ```
    ///
    /// Invariants:
    /// - `p.rotate(0)` is identity (same values, same timing)
    /// - `p.rotate(n).rotate(-n)` restores original values
    pub fn rotate(&self, k: i32) -> Pattern {
        let mut evs = sorted_events(self);
        if evs.is_empty() {
            return Pattern::Empty;
        }
        let n = evs.len();
        let values: Vec<f64> = evs.iter().map(|e| e.value).collect();

        // Normalise k into [0, n)
        let k = ((k % n as i32) + n as i32) as usize % n;

        // Right-rotate: element at position i comes from position (i + n - k) % n
        let rotated_values: Vec<f64> = (0..n).map(|i| values[(i + n - k) % n]).collect();

        for (e, v) in evs.iter_mut().zip(rotated_values) {
            e.value = v;
        }
        from_events(&evs)
    }

    // ------------------------------------------------------------------
    // 2. retrograde()
    // ------------------------------------------------------------------
    /// Reverse the order of events in time, preserving total duration and
    /// the relative duration shape.
    ///
    /// The event that was first in time becomes last; the event that was last
    /// becomes first. Start times are recalculated from zero using the reversed
    /// duration sequence.
    ///
    /// Invariant: `p.retrograde().retrograde()` produces the same values in the
    /// same order.
    pub fn retrograde(&self) -> Pattern {
        let evs = sorted_events(self);
        if evs.is_empty() {
            return Pattern::Empty;
        }

        // Reverse the duration+value sequence; recalculate starts from 0.
        let reversed: Vec<(f64, f64)> = evs
            .iter()
            .rev()
            .map(|e| (e.value, e.duration))
            .collect();

        let mut cursor = 0.0_f64;
        let new_events: Vec<Event> = reversed
            .into_iter()
            .map(|(value, duration)| {
                let e = Event { value, start: cursor, duration };
                cursor += duration;
                e
            })
            .collect();

        from_events(&new_events)
    }

    // ------------------------------------------------------------------
    // 3. shift(n)
    // ------------------------------------------------------------------
    /// Add `n` to every value.  When values represent pitch (semitones),
    /// this is transposition.
    ///
    /// Invariant: `p.shift(a).shift(b)` == `p.shift(a + b)`
    pub fn shift(&self, n: f64) -> Pattern {
        self.with_values(|v| v + n)
    }

    // ------------------------------------------------------------------
    // 4. invert(axis)
    // ------------------------------------------------------------------
    /// Reflect all values around `axis`: `new = 2 * axis - old`.
    ///
    /// Invariant: `p.invert(x).invert(x)` is identity.
    pub fn invert(&self, axis: f64) -> Pattern {
        self.with_values(|v| 2.0 * axis - v)
    }

    // ------------------------------------------------------------------
    // 5. stretch(factor)
    // ------------------------------------------------------------------
    /// Scale all event durations and start times by `factor`.
    ///
    /// Invariants:
    /// - `p.stretch(1.0)` is identity
    /// - `p.stretch(2.0)` doubles the total duration of the pattern
    pub fn stretch(&self, factor: f64) -> Pattern {
        let evs = sorted_events(self);
        if evs.is_empty() {
            return Pattern::Empty;
        }
        let scaled: Vec<Event> = evs
            .into_iter()
            .map(|e| Event {
                value: e.value,
                start: e.start * factor,
                duration: e.duration * factor,
            })
            .collect();
        from_events(&scaled)
    }

    // ------------------------------------------------------------------
    // 6. thin(keep_every)
    // ------------------------------------------------------------------
    /// Keep every `keep_every`-th event; replace the rest with silence
    /// (NaN-valued atoms) so the timing grid is preserved.
    ///
    /// Index counting starts at 0:
    /// - `thin(1)` keeps all events (identity)
    /// - `thin(2)` keeps indices 0, 2, 4, …
    ///
    /// Silence is represented as `f64::NAN`, which downstream consumers
    /// (e.g. the OSC emitter) should treat as a rest.
    ///
    /// Panics if `keep_every == 0`.
    pub fn thin(&self, keep_every: usize) -> Pattern {
        assert!(keep_every > 0, "thin: keep_every must be >= 1");
        let evs = sorted_events(self);
        if evs.is_empty() {
            return Pattern::Empty;
        }
        // Build directly as Atoms to preserve NaN-valued silences
        // (from_events → events() would filter them out).
        let atoms: Vec<Pattern> = evs
            .into_iter()
            .enumerate()
            .map(|(i, e)| {
                if i % keep_every == 0 {
                    Pattern::Atom(e.value, e.duration)
                } else {
                    Pattern::Atom(f64::NAN, e.duration)
                }
            })
            .collect();
        Pattern::Seq(atoms)
    }

    // ------------------------------------------------------------------
    // 7. every(n, f)
    // ------------------------------------------------------------------
    /// Apply transform `f` on every `n`th repetition of the pattern.
    ///
    /// Returns a `Seq` of `n` copies.  Copy `i` (0-indexed) has `f` applied
    /// when `i % n == 0`; the rest are untransformed.  Each copy is
    /// time-shifted so copies are concatenated end-to-end.
    ///
    /// Example: `p.every(3, |p| p.rotate(1))` → a 3-cycle sequence where
    /// cycle 0 is rotated, cycles 1 and 2 are unchanged.
    pub fn every(&self, n: usize, f: impl Fn(&Pattern) -> Pattern) -> Pattern {
        assert!(n > 0, "every: n must be >= 1");
        if n == 1 {
            return f(self);
        }

        let base_dur = self.duration();
        let mut result_events: Vec<Event> = Vec::new();

        for i in 0..n {
            let copy = if i % n == 0 { f(self) } else { self.clone() };
            let offset = base_dur * i as f64;
            // Time-shift this copy's events by the accumulated offset.
            let copy_evs = copy.events();
            for e in copy_evs {
                result_events.push(Event {
                    value: e.value,
                    start: e.start + offset,
                    duration: e.duration,
                });
            }
        }

        from_events(&result_events)
    }

    // ------------------------------------------------------------------
    // 8. with_values(f)
    // ------------------------------------------------------------------
    /// Map a function over every event value.  This is the general form
    /// that `shift` and `invert` are specialisations of.
    pub fn with_values(&self, f: impl Fn(f64) -> f64) -> Pattern {
        let evs = sorted_events(self);
        if evs.is_empty() {
            return Pattern::Empty;
        }
        let mapped: Vec<Event> = evs
            .into_iter()
            .map(|e| Event { value: f(e.value), start: e.start, duration: e.duration })
            .collect();
        from_events(&mapped)
    }

    // ------------------------------------------------------------------
    // 9. degrade(probability, seed)
    // ------------------------------------------------------------------
    /// Randomly drop events with given probability, using a seeded PRNG.
    ///
    /// - `probability` in \[0.0, 1.0\]: 0.0 = keep all, 1.0 = drop all.
    /// - Dropped events become NaN-valued atoms to preserve timing.
    ///
    /// Invariants:
    /// - `p.degrade(0.0, s)` is identity
    /// - `p.degrade(1.0, s).events()` is empty
    /// - Same seed → same result
    pub fn degrade(&self, probability: f64, seed: u64) -> Pattern {
        let evs = sorted_events(self);
        if evs.is_empty() {
            return Pattern::Empty;
        }
        let mut rng = SmallRng::seed_from_u64(seed);
        let atoms: Vec<Pattern> = evs
            .into_iter()
            .map(|e| {
                if rng.random::<f64>() < probability {
                    Pattern::Atom(f64::NAN, e.duration)
                } else {
                    Pattern::Atom(e.value, e.duration)
                }
            })
            .collect();
        Pattern::Seq(atoms)
    }

    // ------------------------------------------------------------------
    // 10. swing(amount)
    // ------------------------------------------------------------------
    /// Create swing feel by delaying off-beat events.
    ///
    /// - `amount` controls how much: 0.0 = straight, ~0.33 = triplet swing.
    /// - "Off-beat" = events at odd indices (1, 3, 5, …).
    /// - On-beat events get longer (duration + shift), off-beat events get
    ///   shorter (duration - shift) and start later. Total time is preserved.
    ///
    /// Invariant: `p.swing(0.0)` is identity.
    pub fn swing(&self, amount: f64) -> Pattern {
        let evs = sorted_events(self);
        if evs.is_empty() {
            return Pattern::Empty;
        }
        // Swing works in pairs: (on-beat, off-beat).
        // On-beat stretches by +shift, off-beat shrinks by -shift.
        // This delays the off-beat start while preserving total duration.
        let atoms: Vec<Pattern> = evs
            .iter()
            .enumerate()
            .map(|(i, e)| {
                if i % 2 == 0 {
                    // On-beat: look ahead to see if there's an off-beat partner
                    let shift = if i + 1 < evs.len() {
                        amount * evs[i + 1].duration
                    } else {
                        0.0
                    };
                    Pattern::Atom(e.value, e.duration + shift)
                } else {
                    // Off-beat: shortened by shift amount
                    let shift = amount * e.duration;
                    Pattern::Atom(e.value, (e.duration - shift).max(0.001))
                }
            })
            .collect();
        Pattern::Seq(atoms)
    }

    // ------------------------------------------------------------------
    // 11. humanize(timing, velocity, seed)
    // ------------------------------------------------------------------
    /// Add subtle random variations to timing and values.
    ///
    /// - `timing`: max random offset on start times (±timing).
    /// - `velocity`: max random offset on values (±velocity).
    /// - Start times clamped to >= 0.0.
    /// - Same seed → same result.
    pub fn humanize(&self, timing: f64, velocity: f64, seed: u64) -> Pattern {
        let evs = sorted_events(self);
        if evs.is_empty() {
            return Pattern::Empty;
        }
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut humanized: Vec<Event> = evs
            .into_iter()
            .map(|e| {
                let t_offset = (rng.random::<f64>() * 2.0 - 1.0) * timing;
                let v_offset = (rng.random::<f64>() * 2.0 - 1.0) * velocity;
                Event {
                    value: e.value + v_offset,
                    start: (e.start + t_offset).max(0.0),
                    duration: e.duration,
                }
            })
            .collect();
        humanized.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
        from_events(&humanized)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::pattern::Pattern;

    /// Tolerance for float comparisons.
    const EPS: f64 = 1e-10;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    /// Extract values from a pattern in start-time order.
    fn values(p: &Pattern) -> Vec<f64> {
        let mut evs = p.events();
        evs.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
        evs.iter().map(|e| e.value).collect()
    }

    /// Extract (start, duration) pairs from a pattern in start-time order.
    fn timing(p: &Pattern) -> Vec<(f64, f64)> {
        let mut evs = p.events();
        evs.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
        evs.iter().map(|e| (e.start, e.duration)).collect()
    }

    /// Build a simple 4-event pattern with values 1..=4, each 0.25 cycles.
    fn p4() -> Pattern {
        Pattern::Seq(vec![
            Pattern::Atom(1.0, 0.25),
            Pattern::Atom(2.0, 0.25),
            Pattern::Atom(3.0, 0.25),
            Pattern::Atom(4.0, 0.25),
        ])
    }

    /// Build a pattern with unequal durations so timing tests are meaningful.
    fn p_unequal() -> Pattern {
        Pattern::Seq(vec![
            Pattern::Atom(10.0, 0.5),
            Pattern::Atom(20.0, 0.25),
            Pattern::Atom(30.0, 0.125),
            Pattern::Atom(40.0, 0.125),
        ])
    }

    // ------------------------------------------------------------------
    // rotate
    // ------------------------------------------------------------------

    #[test]
    fn rotate_zero_is_identity() {
        let p = p4();
        let r = p.rotate(0);
        assert_eq!(values(&p), values(&r));
        assert_eq!(timing(&p), timing(&r));
    }

    #[test]
    fn rotate_round_trip() {
        let p = p4();
        let orig = values(&p);
        for k in [-4, -3, -2, -1, 0, 1, 2, 3, 4] {
            let round_tripped = values(&p.rotate(k).rotate(-k));
            assert_eq!(orig, round_tripped, "rotate({k}).rotate({}) failed", -k);
        }
    }

    #[test]
    fn rotate_positive_shifts_right() {
        // [1,2,3,4] rotated right by 1 → [4,1,2,3]
        let p = p4();
        let r = p.rotate(1);
        assert_eq!(values(&r), vec![4.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn rotate_negative_shifts_left() {
        // [1,2,3,4] rotated left by 1 → [2,3,4,1]
        let p = p4();
        let r = p.rotate(-1);
        assert_eq!(values(&r), vec![2.0, 3.0, 4.0, 1.0]);
    }

    #[test]
    fn rotate_preserves_timing() {
        // Rotating values must not change start/duration of any slot.
        let p = p4();
        let t_before = timing(&p);
        let t_after = timing(&p.rotate(2));
        assert_eq!(t_before, t_after);
    }

    // ------------------------------------------------------------------
    // retrograde
    // ------------------------------------------------------------------

    #[test]
    fn retrograde_double_is_identity() {
        let p = p4();
        let orig_vals = values(&p);
        let round = values(&p.retrograde().retrograde());
        assert_eq!(orig_vals, round);
    }

    #[test]
    fn retrograde_reverses_values() {
        let p = p4();
        let r = p.retrograde();
        assert_eq!(values(&r), vec![4.0, 3.0, 2.0, 1.0]);
    }

    #[test]
    fn retrograde_preserves_total_duration() {
        let p = p_unequal();
        let r = p.retrograde();
        let orig_dur: f64 = p.events().iter().map(|e| e.duration).sum();
        let retro_dur: f64 = r.events().iter().map(|e| e.duration).sum();
        assert!(approx_eq(orig_dur, retro_dur));
    }

    #[test]
    fn retrograde_reverses_duration_order() {
        // Unequal durations: [0.5, 0.25, 0.125, 0.125]
        // Retrograded:       [0.125, 0.125, 0.25, 0.5]
        let p = p_unequal();
        let r = p.retrograde();
        let t = timing(&r);
        // Check durations are reversed
        let durs: Vec<f64> = t.iter().map(|(_, d)| *d).collect();
        assert!(approx_eq(durs[0], 0.125));
        assert!(approx_eq(durs[1], 0.125));
        assert!(approx_eq(durs[2], 0.25));
        assert!(approx_eq(durs[3], 0.5));
        // Check starts are recalculated from 0
        assert!(approx_eq(t[0].0, 0.0));
        assert!(approx_eq(t[1].0, 0.125));
        assert!(approx_eq(t[2].0, 0.25));
        assert!(approx_eq(t[3].0, 0.5));
    }

    // ------------------------------------------------------------------
    // shift
    // ------------------------------------------------------------------

    #[test]
    fn shift_adds_to_all_values() {
        let p = p4();
        let shifted = p.shift(10.0);
        assert_eq!(values(&shifted), vec![11.0, 12.0, 13.0, 14.0]);
    }

    #[test]
    fn shift_compose() {
        // p.shift(a).shift(b) == p.shift(a + b)
        let p = p4();
        let ab = p.shift(3.0).shift(7.0);
        let combined = p.shift(10.0);
        let v_ab = values(&ab);
        let v_c = values(&combined);
        for (a, b) in v_ab.iter().zip(v_c.iter()) {
            assert!(approx_eq(*a, *b));
        }
    }

    #[test]
    fn shift_zero_is_identity() {
        let p = p4();
        assert_eq!(values(&p), values(&p.shift(0.0)));
    }

    // ------------------------------------------------------------------
    // invert
    // ------------------------------------------------------------------

    #[test]
    fn invert_double_is_identity() {
        let p = p4();
        let orig = values(&p);
        let dbl = values(&p.invert(5.0).invert(5.0));
        for (a, b) in orig.iter().zip(dbl.iter()) {
            assert!(approx_eq(*a, *b));
        }
    }

    #[test]
    fn invert_reflects_around_axis() {
        // values [1,2,3,4], axis=2.5 → [4,3,2,1]
        let p = p4();
        let r = p.invert(2.5);
        assert_eq!(values(&r), vec![4.0, 3.0, 2.0, 1.0]);
    }

    // ------------------------------------------------------------------
    // stretch
    // ------------------------------------------------------------------

    #[test]
    fn stretch_one_is_identity() {
        let p = p4();
        let stretched = p.stretch(1.0);
        let t_before = timing(&p);
        let t_after = timing(&stretched);
        for ((s1, d1), (s2, d2)) in t_before.iter().zip(t_after.iter()) {
            assert!(approx_eq(*s1, *s2));
            assert!(approx_eq(*d1, *d2));
        }
        assert_eq!(values(&p), values(&stretched));
    }

    #[test]
    fn stretch_doubles_duration() {
        let p = p4();
        let orig_dur = p.duration();
        let stretched = p.stretch(2.0);
        // Total duration should double
        let new_dur: f64 = stretched.events().iter().map(|e| e.duration).sum();
        assert!(approx_eq(new_dur, orig_dur * 2.0));
    }

    #[test]
    fn stretch_scales_start_times() {
        let p = p4();
        let r = p.stretch(3.0);
        let t = timing(&r);
        // First event starts at 0, each subsequent at +0.75 (0.25 * 3)
        assert!(approx_eq(t[0].0, 0.0));
        assert!(approx_eq(t[1].0, 0.75));
        assert!(approx_eq(t[2].0, 1.5));
        assert!(approx_eq(t[3].0, 2.25));
    }

    // ------------------------------------------------------------------
    // thin
    // ------------------------------------------------------------------

    #[test]
    fn thin_one_is_identity() {
        let p = p4();
        let t = p.thin(1);
        // All values should be preserved
        assert_eq!(values(&p), values(&t));
    }

    #[test]
    fn thin_two_silences_odd_indices() {
        let p = p4();
        let t = p.thin(2);
        // Inspect structural atoms directly (events() filters NaN)
        match &t {
            Pattern::Seq(atoms) => {
                assert_eq!(atoms.len(), 4);
                // Index 0, 2: kept values
                match &atoms[0] {
                    Pattern::Atom(v, _) => assert!(approx_eq(*v, 1.0)),
                    _ => panic!("expected Atom"),
                }
                // Index 1: silenced (NaN)
                match &atoms[1] {
                    Pattern::Atom(v, _) => assert!(v.is_nan()),
                    _ => panic!("expected Atom"),
                }
                match &atoms[2] {
                    Pattern::Atom(v, _) => assert!(approx_eq(*v, 3.0)),
                    _ => panic!("expected Atom"),
                }
                match &atoms[3] {
                    Pattern::Atom(v, _) => assert!(v.is_nan()),
                    _ => panic!("expected Atom"),
                }
            }
            _ => panic!("expected Seq"),
        }
    }

    #[test]
    fn thin_preserves_timing() {
        let p = p4();
        let t = p.thin(2);
        // Total duration preserved (NaN atoms still contribute duration)
        assert!(approx_eq(p.duration(), t.duration()));
        // The 2 non-silent events should have correct start times
        let evs = t.events(); // only non-NaN events
        assert_eq!(evs.len(), 2);
        assert!(approx_eq(evs[0].start, 0.0));   // index 0
        assert!(approx_eq(evs[1].start, 0.5));   // index 2
    }

    #[test]
    #[should_panic(expected = "thin: keep_every must be >= 1")]
    fn thin_zero_panics() {
        let p = p4();
        let _ = p.thin(0);
    }

    // ------------------------------------------------------------------
    // every
    // ------------------------------------------------------------------

    #[test]
    fn every_produces_n_cycles() {
        let p = p4();
        let n = 3usize;
        let result = p.every(n, |pat| pat.rotate(1));
        // Should have n * events_per_cycle events total
        let event_count = result.events().len();
        assert_eq!(event_count, n * p.events().len());
    }

    #[test]
    fn every_total_duration_is_n_times_base() {
        let p = p4();
        let n = 4usize;
        let base_dur = p.duration();
        let result = p.every(n, |pat| pat.shift(1.0));
        let total_dur: f64 = result.events().iter().map(|e| e.duration).sum();
        // n copies of base_dur worth of events
        assert!(approx_eq(total_dur, base_dur * n as f64));
    }

    #[test]
    fn every_applies_transform_on_cycle_zero() {
        // p.every(3, |p| p.shift(100)) — cycle 0 gets shifted, 1 and 2 don't.
        let p = p4();
        let result = p.every(3, |pat| pat.shift(100.0));
        let evs = {
            let mut e = result.events();
            e.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
            e
        };
        let base_dur = p.duration(); // 1.0

        // Cycle 0 (start in [0, base_dur)): values should be shifted by 100
        let cycle0: Vec<f64> = evs
            .iter()
            .filter(|e| e.start < base_dur)
            .map(|e| e.value)
            .collect();
        for v in &cycle0 {
            assert!(*v > 50.0, "cycle 0 should have shifted values, got {v}");
        }

        // Cycle 1 (start in [base_dur, 2*base_dur)): values should be unshifted
        let cycle1: Vec<f64> = evs
            .iter()
            .filter(|e| e.start >= base_dur && e.start < 2.0 * base_dur)
            .map(|e| e.value)
            .collect();
        for v in &cycle1 {
            assert!(*v < 50.0, "cycle 1 should have unshifted values, got {v}");
        }
    }

    #[test]
    fn every_one_applies_always() {
        // every(1, f) is equivalent to f(p)
        let p = p4();
        let via_every = p.every(1, |pat| pat.shift(5.0));
        let direct = p.shift(5.0);
        let v_every = values(&via_every);
        let v_direct = values(&direct);
        for (a, b) in v_every.iter().zip(v_direct.iter()) {
            assert!(approx_eq(*a, *b));
        }
    }

    // ------------------------------------------------------------------
    // with_values
    // ------------------------------------------------------------------

    #[test]
    fn with_values_maps_all() {
        let p = p4();
        let doubled = p.with_values(|v| v * 2.0);
        assert_eq!(values(&doubled), vec![2.0, 4.0, 6.0, 8.0]);
    }

    #[test]
    fn with_values_identity_fn_is_identity() {
        let p = p4();
        let same = p.with_values(|v| v);
        assert_eq!(values(&p), values(&same));
    }

    // ------------------------------------------------------------------
    // degrade
    // ------------------------------------------------------------------

    #[test]
    fn degrade_zero_is_identity() {
        let p = p4();
        let d = p.degrade(0.0, 42);
        assert_eq!(values(&p), values(&d));
    }

    #[test]
    fn degrade_one_drops_all() {
        let p = p4();
        let d = p.degrade(1.0, 42);
        assert!(d.events().is_empty());
    }

    #[test]
    fn degrade_preserves_total_duration() {
        let p = p4();
        let d = p.degrade(0.5, 42);
        assert!(approx_eq(p.duration(), d.duration()));
    }

    #[test]
    fn degrade_deterministic() {
        let p = p4();
        let a = p.degrade(0.5, 123);
        let b = p.degrade(0.5, 123);
        assert_eq!(values(&a), values(&b));
    }

    #[test]
    fn degrade_reduces_event_count() {
        // With 100 events and prob 0.5, should drop some
        let big = Pattern::seq(&(0..100).map(|i| i as f64).collect::<Vec<_>>());
        let d = big.degrade(0.5, 7);
        assert!(d.events().len() < 100);
        assert!(d.events().len() > 0);
    }

    // ------------------------------------------------------------------
    // swing
    // ------------------------------------------------------------------

    #[test]
    fn swing_zero_is_identity() {
        let p = p4();
        let s = p.swing(0.0);
        let t_before = timing(&p);
        let t_after = timing(&s);
        for ((s1, d1), (s2, d2)) in t_before.iter().zip(t_after.iter()) {
            assert!(approx_eq(*s1, *s2));
            assert!(approx_eq(*d1, *d2));
        }
    }

    #[test]
    fn swing_shifts_offbeats() {
        let p = p4(); // values 1,2,3,4 each dur 0.25
        let s = p.swing(0.33);
        let evs = s.events();
        let find = |val: f64| evs.iter().find(|e| approx_eq(e.value, val)).unwrap();
        // On-beat events start at 0.0 but are longer (absorb swing delay)
        assert!(approx_eq(find(1.0).start, 0.0));
        assert!(find(1.0).duration > 0.25); // stretched
        // Off-beat events start later than original 0.25
        assert!(find(2.0).start > 0.25);
        assert!(find(2.0).duration < 0.25); // shortened
        // Total duration preserved
        assert!(approx_eq(s.duration(), p.duration()));
    }

    #[test]
    fn swing_preserves_event_count() {
        let p = p4();
        let s = p.swing(0.2);
        assert_eq!(p.events().len(), s.events().len());
    }

    #[test]
    fn swing_preserves_values() {
        let p = p4();
        let s = p.swing(0.33);
        assert_eq!(values(&p), values(&s));
    }

    // ------------------------------------------------------------------
    // humanize
    // ------------------------------------------------------------------

    #[test]
    fn humanize_zero_is_identity() {
        let p = p4();
        let h = p.humanize(0.0, 0.0, 42);
        let v_orig = values(&p);
        let v_hum = values(&h);
        for (a, b) in v_orig.iter().zip(v_hum.iter()) {
            assert!(approx_eq(*a, *b));
        }
    }

    #[test]
    fn humanize_deterministic() {
        let p = p4();
        let a = p.humanize(0.01, 2.0, 555);
        let b = p.humanize(0.01, 2.0, 555);
        let va = values(&a);
        let vb = values(&b);
        for (x, y) in va.iter().zip(vb.iter()) {
            assert!(approx_eq(*x, *y));
        }
    }

    #[test]
    fn humanize_timing_stays_non_negative() {
        let p = p4();
        let h = p.humanize(1.0, 0.0, 42); // huge timing jitter
        for e in h.events() {
            assert!(e.start >= 0.0, "start time went negative: {}", e.start);
        }
    }

    #[test]
    fn humanize_preserves_event_count() {
        let p = p4();
        let h = p.humanize(0.01, 1.0, 42);
        assert_eq!(p.events().len(), h.events().len());
    }
}
