/// Accumulator — ecstatic computation via iterative pattern evolution.
///
/// The Accumulator treats a seed Pattern as DNA that mutates through a cycle of
/// transforms. Each iteration takes the *previous iteration's result* and applies
/// the next transform in the cycle. The final output is the temporal concatenation
/// of every iteration, including the unmodified seed at iteration 0.
///
/// Example: seed `[1 2 3]`, transforms `[rotate(1), shift(10)]`:
///   - Iter 0: `[1 2 3]`           (seed, untransformed)
///   - Iter 1: rotate(1) → `[3 1 2]`
///   - Iter 2: shift(10) → `[13 11 12]`
///   - Iter 3: rotate(1) → `[12 13 11]`
///   - Result: `[1 2 3] + [3 1 2] + [13 11 12] + [12 13 11]`

use crate::pattern::Pattern;

// ---------------------------------------------------------------------------
// Accumulator
// ---------------------------------------------------------------------------

pub struct Accumulator {
    seed: Pattern,
    transforms: Vec<Box<dyn Fn(&Pattern) -> Pattern>>,
}

impl Accumulator {
    /// Create an Accumulator from a seed pattern and a list of transform functions.
    pub fn new(seed: Pattern, transforms: Vec<Box<dyn Fn(&Pattern) -> Pattern>>) -> Self {
        Accumulator { seed, transforms }
    }

    /// Generate by iteratively transforming the seed.
    ///
    /// - Iteration 0: the seed, unchanged.
    /// - Iteration i (i >= 1): `transforms[(i-1) % len](result_of_iter_(i-1))`.
    ///
    /// Returns the temporal concatenation of all iterations via the `+` operator.
    /// If `iterations == 0`, returns `Pattern::Empty`.
    /// If `transforms` is empty, every iteration is the seed unchanged.
    pub fn generate(&self, iterations: usize) -> Pattern {
        if iterations == 0 {
            return Pattern::empty();
        }

        let mut result = Pattern::empty();
        let mut current = self.seed.clone();

        for i in 0..iterations {
            if i > 0 {
                if !self.transforms.is_empty() {
                    let tf = &self.transforms[(i - 1) % self.transforms.len()];
                    current = tf(&current);
                }
                // If transforms is empty, current stays as the seed each time.
            }
            result = result + current.clone();
        }

        result
    }

    /// Generate multiple voices with offset starting rotations.
    ///
    /// Each voice `v` (0-indexed) starts with the seed rotated by
    /// `v * (seed_len / voices)` positions, where `seed_len = seed.len()`.
    /// All voices run through the same transform cycle independently.
    ///
    /// `voice_offset` shifts each voice's events forward in time by
    /// `v * voice_offset` beats (i.e. the voice pattern's start times are
    /// displaced). The returned patterns carry raw event times starting at
    /// 0.0; callers handle the actual time displacement by reading
    /// `voice_offset` from the returned tuple or by the convention that
    /// voice `v` is offset by `v * voice_offset` when scheduling.
    ///
    /// Actually: the returned patterns have their event start times shifted
    /// by `v * voice_offset` so they can be stacked directly as a Stack.
    ///
    /// Returns a `Vec<Pattern>` with one element per voice.
    pub fn generate_layered(
        &self,
        iterations: usize,
        voices: usize,
        voice_offset: f64,
    ) -> Vec<Pattern> {
        if voices == 0 {
            return vec![];
        }

        let seed_len = self.seed.len();

        (0..voices)
            .map(|v| {
                // Rotate the seed by v * (seed_len / voices) positions.
                let rotation = if seed_len > 0 {
                    (v * seed_len / voices) as i32
                } else {
                    0
                };
                let rotated_seed = self.seed.rotate(rotation);

                // Build a per-voice accumulator with the same transforms.
                // We can't clone the boxed fns, so we inline the generation
                // logic directly (mirrors generate() but with a different seed).
                let voice_pattern = if iterations == 0 {
                    Pattern::empty()
                } else {
                    let mut result = Pattern::empty();
                    let mut current = rotated_seed;

                    for i in 0..iterations {
                        if i > 0 && !self.transforms.is_empty() {
                            let tf = &self.transforms[(i - 1) % self.transforms.len()];
                            current = tf(&current);
                        }
                        result = result + current.clone();
                    }
                    result
                };

                // Apply time offset for this voice.
                if voice_offset == 0.0 || v == 0 {
                    voice_pattern
                } else {
                    let offset = v as f64 * voice_offset;
                    voice_pattern.time_shift(offset)
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Pattern::time_shift — shift all event start times by a constant offset.
// ---------------------------------------------------------------------------
//
// This is a private helper used only by generate_layered. It is not part of
// the public transform API in transform.rs; it belongs here so accumulator.rs
// is self-contained.

impl Pattern {
    /// Return a new pattern where every event's start time is displaced by
    /// `offset`. Durations and values are unchanged. The structural
    /// representation is flattened to a Seq of Atoms (same as other transforms).
    ///
    /// This is intentionally package-private (module-level, not `pub`) — it
    /// is an internal aid for `generate_layered`.
    pub(crate) fn time_shift(&self, offset: f64) -> Pattern {
        let evs = self.events();
        if evs.is_empty() {
            return Pattern::empty();
        }
        // We need to rebuild as a flat Seq, preserving gaps between events.
        // Because events() filters out NaN (silence) atoms, we may lose rests.
        // For the purposes of generate_layered this is acceptable: the offset
        // simply moves the sounding events forward in time.
        let mut sorted = evs;
        sorted.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());

        let atoms: Vec<Pattern> = sorted
            .into_iter()
            .map(|e| {
                // We can't easily re-insert the silence prefix here without
                // re-examining the original structure. Instead we embed start
                // time information by prepending a silence atom if needed.
                // But from_events-style rebuild loses starts, so we use a
                // Stack trick: wrap each atom in a Seq with an appropriate
                // leading silence.
                //
                // Simpler and correct: build a Seq where each element is a
                // Seq([silence(start+offset), atom(value, dur)]) — but that
                // changes the pattern structure in a weird way.
                //
                // The cleanest approach: produce a flat Seq of atoms positioned
                // by inserting silence gaps between them.
                let _ = e; // placeholder — handled below
                Pattern::empty()
            })
            .collect();
        let _ = atoms;

        // Redo correctly: rebuild as flat Seq with silence gaps.
        let evs2 = {
            let mut e = self.events();
            e.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
            e
        };

        let mut parts: Vec<Pattern> = Vec::new();
        // Leading silence for the offset itself.
        if offset > 0.0 {
            parts.push(Pattern::silence(offset));
        }
        // Gaps between events (preserving relative spacing).
        let mut cursor = 0.0_f64;
        for e in &evs2 {
            if e.start > cursor + 1e-12 {
                parts.push(Pattern::silence(e.start - cursor));
            }
            parts.push(Pattern::Atom(e.value, e.duration));
            cursor = e.start + e.duration;
        }

        Pattern::Seq(parts)
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

    /// Extract values in start-time order.
    fn values(p: &Pattern) -> Vec<f64> {
        let mut evs = p.events();
        evs.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
        evs.iter().map(|e| e.value).collect()
    }

    /// A simple 3-note seed: [1, 2, 3], each duration 1/3.
    fn seed3() -> Pattern {
        Pattern::seq(&[1.0, 2.0, 3.0])
    }

    // ------------------------------------------------------------------
    // generate — basic shape
    // ------------------------------------------------------------------

    #[test]
    fn generate_zero_returns_empty() {
        let acc = Accumulator::new(seed3(), vec![]);
        let result = acc.generate(0);
        assert!(result.is_empty());
    }

    #[test]
    fn generate_one_returns_seed() {
        let acc = Accumulator::new(seed3(), vec![]);
        let result = acc.generate(1);
        // Should equal the seed in events
        assert_eq!(values(&result), values(&seed3()));
    }

    #[test]
    fn generate_one_duration_equals_seed_duration() {
        let acc = Accumulator::new(seed3(), vec![]);
        let result = acc.generate(1);
        assert!(approx(result.duration(), seed3().duration()));
    }

    // ------------------------------------------------------------------
    // generate — total duration = n * seed.duration()
    // ------------------------------------------------------------------

    #[test]
    fn generate_n_total_duration() {
        // With transforms that preserve duration (rotate), total duration == n * seed.dur
        let acc = Accumulator::new(
            seed3(),
            vec![Box::new(|p: &Pattern| p.rotate(1))],
        );
        for n in [1, 2, 3, 5] {
            let result = acc.generate(n);
            let expected = seed3().duration() * n as f64;
            assert!(
                approx(result.duration(), expected),
                "generate({n}): expected duration {expected}, got {}",
                result.duration()
            );
        }
    }

    // ------------------------------------------------------------------
    // generate — total event count
    // ------------------------------------------------------------------

    #[test]
    fn generate_n_event_count_with_identity_transforms() {
        // rotate preserves event count, so n * seed.len() events total.
        let seed = seed3();
        let seed_len = seed.len();
        let acc = Accumulator::new(
            seed,
            vec![Box::new(|p: &Pattern| p.rotate(1))],
        );
        for n in [1, 2, 4] {
            let result = acc.generate(n);
            assert_eq!(
                result.len(),
                seed_len * n,
                "generate({n}): expected {} events, got {}",
                seed_len * n,
                result.len()
            );
        }
    }

    // ------------------------------------------------------------------
    // generate — transform cycling
    // ------------------------------------------------------------------

    #[test]
    fn transform_cycling_two_transforms() {
        // Two transforms: shift(+10), shift(+100).
        // Iter 0: [1, 2, 3]
        // Iter 1: shift(+10) → [11, 12, 13]
        // Iter 2: shift(+100) → [111, 112, 113]
        // Iter 3: shift(+10) → [121, 122, 123]  (wraps back to transform 0)
        let acc = Accumulator::new(
            seed3(),
            vec![
                Box::new(|p: &Pattern| p.shift(10.0)),
                Box::new(|p: &Pattern| p.shift(100.0)),
            ],
        );
        let result = acc.generate(4);
        let evs: Vec<f64> = values(&result);

        // Iter 0: [1, 2, 3]
        assert!(approx(evs[0], 1.0));
        assert!(approx(evs[1], 2.0));
        assert!(approx(evs[2], 3.0));

        // Iter 1 (transform 0 = shift+10 applied to iter 0): [11, 12, 13]
        assert!(approx(evs[3], 11.0));
        assert!(approx(evs[4], 12.0));
        assert!(approx(evs[5], 13.0));

        // Iter 2 (transform 1 = shift+100 applied to iter 1): [111, 112, 113]
        assert!(approx(evs[6], 111.0));
        assert!(approx(evs[7], 112.0));
        assert!(approx(evs[8], 113.0));

        // Iter 3 (transform 0 again = shift+10 applied to iter 2): [121, 122, 123]
        assert!(approx(evs[9], 121.0));
        assert!(approx(evs[10], 122.0));
        assert!(approx(evs[11], 123.0));
    }

    // ------------------------------------------------------------------
    // generate — each iteration feeds from the previous result (not seed)
    // ------------------------------------------------------------------

    #[test]
    fn each_iteration_feeds_from_previous() {
        // One transform: rotate(1).
        // Iter 0: [1, 2, 3]
        // Iter 1: rotate(1) on [1,2,3] → [3, 1, 2]
        // Iter 2: rotate(1) on [3,1,2] → [2, 3, 1]  ← must feed from iter 1, not seed
        let acc = Accumulator::new(
            seed3(),
            vec![Box::new(|p: &Pattern| p.rotate(1))],
        );
        let result = acc.generate(3);
        let evs = values(&result);

        // Iter 0
        assert_eq!(evs[0..3], [1.0, 2.0, 3.0]);
        // Iter 1
        assert_eq!(evs[3..6], [3.0, 1.0, 2.0]);
        // Iter 2 — must come from rotating iter 1, not the seed
        assert_eq!(evs[6..9], [2.0, 3.0, 1.0]);
    }

    // ------------------------------------------------------------------
    // generate — design doc example
    // ------------------------------------------------------------------

    #[test]
    fn design_doc_example() {
        // seed [1 2 3], transforms [rotate(1), shift(10)]
        // Iter 0: [1 2 3]
        // Iter 1: rotate(1)([1 2 3]) → [3 1 2]
        // Iter 2: shift(10)([3 1 2]) → [13 11 12]
        // Iter 3: rotate(1)([13 11 12]) → [12 13 11]
        let acc = Accumulator::new(
            seed3(),
            vec![
                Box::new(|p: &Pattern| p.rotate(1)),
                Box::new(|p: &Pattern| p.shift(10.0)),
            ],
        );
        let result = acc.generate(4);
        let evs = values(&result);

        assert_eq!(evs[0..3], [1.0, 2.0, 3.0]);
        assert_eq!(evs[3..6], [3.0, 1.0, 2.0]);
        assert_eq!(evs[6..9], [13.0, 11.0, 12.0]);
        assert_eq!(evs[9..12], [12.0, 13.0, 11.0]);
    }

    // ------------------------------------------------------------------
    // generate_layered — voice count
    // ------------------------------------------------------------------

    #[test]
    fn layered_produces_correct_voice_count() {
        let acc = Accumulator::new(
            seed3(),
            vec![Box::new(|p: &Pattern| p.rotate(1))],
        );
        for voices in [1, 2, 3, 4] {
            let layers = acc.generate_layered(2, voices, 0.0);
            assert_eq!(layers.len(), voices, "expected {voices} voices");
        }
    }

    #[test]
    fn layered_zero_voices_returns_empty_vec() {
        let acc = Accumulator::new(seed3(), vec![]);
        let layers = acc.generate_layered(3, 0, 0.0);
        assert!(layers.is_empty());
    }

    // ------------------------------------------------------------------
    // generate_layered — voice 0 matches generate()
    // ------------------------------------------------------------------

    #[test]
    fn layered_voice_zero_matches_generate() {
        // With 1 voice and no offset, voice 0 should be identical to generate().
        let acc = Accumulator::new(
            seed3(),
            vec![Box::new(|p: &Pattern| p.rotate(1))],
        );
        let single = acc.generate(3);
        let layers = acc.generate_layered(3, 1, 0.0);
        assert_eq!(values(&layers[0]), values(&single));
    }

    // ------------------------------------------------------------------
    // generate_layered — voices have different rotations
    // ------------------------------------------------------------------

    #[test]
    fn layered_voices_have_different_rotations() {
        // With a 3-note seed and 3 voices:
        //   Voice 0: rotate(0) seed → [1,2,3]
        //   Voice 1: rotate(1) seed → [3,1,2]
        //   Voice 2: rotate(2) seed → [2,3,1]
        // (rotation = v * seed_len / voices = v * 3/3 = v)
        let acc = Accumulator::new(seed3(), vec![]);
        let layers = acc.generate_layered(1, 3, 0.0);

        // Voice 0: seed unchanged
        assert_eq!(values(&layers[0]), vec![1.0, 2.0, 3.0]);
        // Voice 1: rotated by 1 position right → [3,1,2]
        assert_eq!(values(&layers[1]), vec![3.0, 1.0, 2.0]);
        // Voice 2: rotated by 2 positions right → [2,3,1]
        assert_eq!(values(&layers[2]), vec![2.0, 3.0, 1.0]);
    }

    // ------------------------------------------------------------------
    // generate_layered — voice_offset shifts start times
    // ------------------------------------------------------------------

    #[test]
    fn layered_voice_offset_shifts_start_times() {
        // 2 voices, voice_offset = 0.5.
        // Voice 0: events start at their natural times (no offset).
        // Voice 1: events start 0.5 beats later than voice 0's times.
        let acc = Accumulator::new(seed3(), vec![]);
        let layers = acc.generate_layered(1, 2, 0.5);

        let evs0: Vec<f64> = {
            let mut e = layers[0].events();
            e.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
            e.iter().map(|ev| ev.start).collect()
        };
        let evs1: Vec<f64> = {
            let mut e = layers[1].events();
            e.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
            e.iter().map(|ev| ev.start).collect()
        };

        // Voice 1 starts should be voice 0 starts + 0.5
        for (s0, s1) in evs0.iter().zip(evs1.iter()) {
            assert!(
                approx(*s1, *s0 + 0.5),
                "voice 1 start {s1} != voice 0 start {s0} + 0.5"
            );
        }
    }

    #[test]
    fn layered_zero_offset_voice_times_match() {
        // With voice_offset=0.0, all voices should have the same event start times
        // (they may differ in values due to rotation, but timing is the same).
        let acc = Accumulator::new(seed3(), vec![]);
        let layers = acc.generate_layered(1, 3, 0.0);

        let starts0: Vec<f64> = {
            let mut e = layers[0].events();
            e.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
            e.iter().map(|ev| ev.start).collect()
        };

        for v in 1..layers.len() {
            let starts_v: Vec<f64> = {
                let mut e = layers[v].events();
                e.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
                e.iter().map(|ev| ev.start).collect()
            };
            for (s0, sv) in starts0.iter().zip(starts_v.iter()) {
                assert!(
                    approx(*s0, *sv),
                    "voice {v} start {sv} != voice 0 start {s0} with zero offset"
                );
            }
        }
    }

    // ------------------------------------------------------------------
    // generate — no-transform case produces n copies of seed
    // ------------------------------------------------------------------

    #[test]
    fn no_transforms_repeats_seed() {
        let acc = Accumulator::new(seed3(), vec![]);
        let result = acc.generate(3);
        let evs = values(&result);
        // Should be [1,2,3,1,2,3,1,2,3]
        assert_eq!(evs, vec![1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0, 3.0]);
    }
}
