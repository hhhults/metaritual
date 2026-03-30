/// First-order Markov chain for generating melodic and rhythmic sequences.
///
/// States are f64 values (pitch, velocity, ratio — whatever the context
/// demands). Because f64 is not Hash, the transition matrix is stored
/// internally as `HashMap<u64, …>` keyed on `f64::to_bits()`.
/// All public constructors accept plain `f64` keys via slices/vecs of tuples.
///
/// # Quick start
///
/// ```rust
/// use metaritual::markov::Markov;
///
/// // 60 always goes to 64; 64 goes to 60 (70%) or 67 (30%).
/// let markov = Markov::new(
///     vec![60.0, 64.0, 67.0],
///     vec![
///         (60.0, vec![(64.0, 1.0)]),
///         (64.0, vec![(60.0, 0.7), (67.0, 0.3)]),
///     ],
/// );
/// let pattern = markov.generate(8, 0.25, Some(60.0), 42);
/// assert_eq!(pattern.len(), 8);
/// ```

use std::collections::HashMap;

use crate::pattern::Pattern;
use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;

// ---------------------------------------------------------------------------
// Markov struct
// ---------------------------------------------------------------------------

/// A first-order Markov chain over f64 states.
pub struct Markov {
    /// Ordered list of all known states.
    states: Vec<f64>,
    /// Transition map: state bits → [(next_state, cumulative_probability)].
    ///
    /// Probabilities are stored in *cumulative* form so that `step` only
    /// needs a single linear scan and one comparison, not a running sum.
    matrix: HashMap<u64, Vec<(f64, f64)>>,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Convert a raw (state, probability) list into a cumulative distribution.
///
/// `[(a, 0.3), (b, 0.5), (c, 0.2)]` → `[(a, 0.3), (b, 0.8), (c, 1.0)]`
fn to_cumulative(raw: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    let mut acc = 0.0_f64;
    raw.into_iter()
        .map(|(state, p)| {
            acc += p;
            (state, acc)
        })
        .collect()
}

/// Normalize a weight slice so that its elements sum to 1.0.
/// Returns all-zeros if the total is zero.
fn normalize_weights(weights: &[f64]) -> Vec<f64> {
    let total: f64 = weights.iter().sum();
    if total == 0.0 {
        return vec![0.0; weights.len()];
    }
    weights.iter().map(|&w| w / total).collect()
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

impl Markov {
    /// Build from explicit states and a transition list.
    ///
    /// `transitions` is a list of `(from_state, Vec<(next_state, probability)>)` pairs.
    /// Probabilities for each source state should sum to 1.0; if they don't,
    /// the last bucket will be under/over-represented.
    pub fn new(states: Vec<f64>, transitions: Vec<(f64, Vec<(f64, f64)>)>) -> Self {
        let matrix = transitions
            .into_iter()
            .map(|(from, raw)| (from.to_bits(), to_cumulative(raw)))
            .collect();

        Markov { states, matrix }
    }

    /// Learn a Markov chain from an existing Pattern's event values.
    ///
    /// Events are sorted by start time (the Pattern API guarantees this).
    /// Consecutive event values become state → next-state transitions.
    /// An empty or single-event pattern produces a chain with no transitions.
    pub fn from_pattern(pattern: &Pattern) -> Self {
        let events = pattern.events();

        // Collect unique states (preserving first-seen order).
        let mut states: Vec<f64> = Vec::new();
        let mut seen: HashMap<u64, ()> = HashMap::new();
        for e in &events {
            if seen.insert(e.value.to_bits(), ()).is_none() {
                states.push(e.value);
            }
        }

        // Map bit-key back to f64 value.
        let mut bit_to_value: HashMap<u64, f64> = HashMap::new();
        for e in &events {
            bit_to_value.insert(e.value.to_bits(), e.value);
        }

        // Count transitions: raw_counts[from_bits][to_bits] = count.
        let mut raw_counts: HashMap<u64, HashMap<u64, u64>> = HashMap::new();
        for window in events.windows(2) {
            let from = window[0].value;
            let to = window[1].value;
            *raw_counts
                .entry(from.to_bits())
                .or_default()
                .entry(to.to_bits())
                .or_insert(0) += 1;
        }

        // Convert counts → normalised probabilities → cumulative.
        let mut matrix: HashMap<u64, Vec<(f64, f64)>> = HashMap::new();
        for (from_bits, to_counts) in raw_counts {
            let total: u64 = to_counts.values().sum();
            let raw: Vec<(f64, f64)> = to_counts
                .into_iter()
                .map(|(to_bits, count)| {
                    let to_val = bit_to_value[&to_bits];
                    let prob = count as f64 / total as f64;
                    (to_val, prob)
                })
                .collect();
            matrix.insert(from_bits, to_cumulative(raw));
        }

        Markov { states, matrix }
    }

    /// Build from states and a weight matrix.
    ///
    /// `weights` is a list of `(from_state, row)` pairs where `row` contains
    /// one non-negative weight per state in `states` order. Weights are
    /// auto-normalised to probabilities.
    ///
    /// # Panics
    ///
    /// Panics if any weight row has a different length than `states`.
    pub fn from_weights(states: Vec<f64>, weights: Vec<(f64, Vec<f64>)>) -> Self {
        let n = states.len();
        let mut matrix: HashMap<u64, Vec<(f64, f64)>> = HashMap::new();

        for (from, raw_weights) in &weights {
            assert_eq!(
                raw_weights.len(),
                n,
                "weight row for state {from} has length {}, expected {n}",
                raw_weights.len()
            );
            let probs = normalize_weights(raw_weights);
            // Zip with states, skipping zero-weight entries.
            let raw: Vec<(f64, f64)> = states
                .iter()
                .zip(probs.iter())
                .filter(|(_, p)| **p > 0.0)
                .map(|(&s, &p)| (s, p))
                .collect();
            matrix.insert(from.to_bits(), to_cumulative(raw));
        }

        Markov { states, matrix }
    }
}

// ---------------------------------------------------------------------------
// Methods
// ---------------------------------------------------------------------------

impl Markov {
    /// Advance one step from `current` using `rand_val` ∈ [0.0, 1.0).
    ///
    /// Scans the cumulative distribution for `current` and returns the first
    /// bucket whose cumulative probability exceeds `rand_val`. If `current`
    /// has no outgoing transitions, returns `current` unchanged.
    fn step(&self, current: f64, rand_val: f64) -> f64 {
        let key = current.to_bits();
        match self.matrix.get(&key) {
            None => current,
            Some(cumulative) => {
                for &(next, cum_prob) in cumulative {
                    if rand_val < cum_prob {
                        return next;
                    }
                }
                // Floating-point rounding edge: rand_val ≈ 1.0 — last bucket.
                cumulative.last().map(|&(s, _)| s).unwrap_or(current)
            }
        }
    }

    /// Generate a `Pattern` of `length` events each with the given `duration`.
    ///
    /// - `start_state`: begin the walk here; if `None`, uses `self.states[0]`.
    ///   If `states` is empty, returns `Pattern::Empty`.
    /// - `seed`: deterministic PRNG seed — same seed always yields same output.
    pub fn generate(
        &self,
        length: usize,
        duration: f64,
        start_state: Option<f64>,
        seed: u64,
    ) -> Pattern {
        if length == 0 || self.states.is_empty() {
            return Pattern::empty();
        }

        let first = match start_state {
            Some(s) => s,
            None => self.states[0],
        };

        let mut rng = SmallRng::seed_from_u64(seed);
        let mut atoms: Vec<Pattern> = Vec::with_capacity(length);
        let mut current = first;

        for _ in 0..length {
            atoms.push(Pattern::atom(current, duration));
            current = self.step(current, rng.random::<f64>());
        }

        Pattern::Seq(atoms)
    }

    /// All known states, in the order they were registered.
    pub fn states(&self) -> &[f64] {
        &self.states
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};
    use rand::rngs::SmallRng;

    const EPS: f64 = 1e-9;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    // -----------------------------------------------------------------------
    // from_pattern
    // -----------------------------------------------------------------------

    #[test]
    fn from_pattern_learns_transitions() {
        // A → B → A → B → A. Expected: A→B=1.0, B→A=1.0.
        let p = Pattern::seq(&[60.0, 64.0, 60.0, 64.0, 60.0]);
        let chain = Markov::from_pattern(&p);

        // From 60 → should always go to 64.
        let next = chain.step(60.0, 0.5);
        assert!(approx(next, 64.0), "60→64 expected, got {next}");

        // From 64 → should always go to 60.
        let next = chain.step(64.0, 0.5);
        assert!(approx(next, 60.0), "64→60 expected, got {next}");
    }

    #[test]
    fn from_pattern_captures_state_set() {
        let p = Pattern::seq(&[60.0, 64.0, 67.0, 60.0]);
        let chain = Markov::from_pattern(&p);
        assert!(chain.states.contains(&60.0));
        assert!(chain.states.contains(&64.0));
        assert!(chain.states.contains(&67.0));
    }

    #[test]
    fn from_pattern_single_event_no_transitions() {
        let p = Pattern::atom(60.0, 1.0);
        let chain = Markov::from_pattern(&p);
        // No transitions — step should return current unchanged.
        let next = chain.step(60.0, 0.5);
        assert!(approx(next, 60.0));
    }

    #[test]
    fn from_pattern_empty_pattern() {
        let p = Pattern::empty();
        let chain = Markov::from_pattern(&p);
        assert!(chain.states.is_empty());
        assert!(chain.matrix.is_empty());
    }

    #[test]
    fn from_pattern_transition_probabilities_near_half() {
        // 60 → 64 twice, 60 → 67 twice → each ≈ 0.5.
        let p = Pattern::seq(&[60.0, 64.0, 60.0, 67.0, 60.0]);
        let chain = Markov::from_pattern(&p);

        let mut count_64 = 0usize;
        let mut count_67 = 0usize;
        let n = 10_000usize;
        let mut rng = SmallRng::seed_from_u64(99);
        for _ in 0..n {
            let next = chain.step(60.0, rng.random::<f64>());
            if approx(next, 64.0) {
                count_64 += 1;
            } else if approx(next, 67.0) {
                count_67 += 1;
            }
        }
        let ratio_64 = count_64 as f64 / n as f64;
        let ratio_67 = count_67 as f64 / n as f64;
        assert!(
            (ratio_64 - 0.5).abs() < 0.05,
            "60→64 ratio = {ratio_64:.3}, expected ~0.5"
        );
        assert!(
            (ratio_67 - 0.5).abs() < 0.05,
            "60→67 ratio = {ratio_67:.3}, expected ~0.5"
        );
    }

    // -----------------------------------------------------------------------
    // from_weights
    // -----------------------------------------------------------------------

    #[test]
    fn from_weights_normalizes_correctly() {
        let states = vec![60.0, 64.0, 67.0];
        // Raw weights [1, 3, 0] → probs [0.25, 0.75, 0.0].
        let weights = vec![
            (60.0_f64, vec![1.0, 3.0, 0.0]),
            (64.0_f64, vec![0.0, 0.0, 1.0]),
            (67.0_f64, vec![1.0, 1.0, 1.0]),
        ];

        let chain = Markov::from_weights(states, weights);

        // rand_val = 0.24 → lands in 60's first bucket (cum = 0.25) → 60.
        let next = chain.step(60.0, 0.24);
        assert!(approx(next, 60.0), "expected 60, got {next}");

        // rand_val = 0.26 → lands in 64's bucket (cum = 1.0) → 64.
        let next = chain.step(60.0, 0.26);
        assert!(approx(next, 64.0), "expected 64, got {next}");

        // 64 → 67 always.
        let next = chain.step(64.0, 0.99);
        assert!(approx(next, 67.0), "expected 67, got {next}");
    }

    #[test]
    fn from_weights_equal_weights_uniform_distribution() {
        let states = vec![60.0, 64.0, 67.0];
        let weights = vec![(60.0_f64, vec![1.0, 1.0, 1.0])];
        let chain = Markov::from_weights(states, weights);

        let mut counts = [0usize; 3];
        let values = [60.0, 64.0, 67.0];
        let n = 9_000usize;
        let mut rng = SmallRng::seed_from_u64(7);
        for _ in 0..n {
            let next = chain.step(60.0, rng.random::<f64>());
            for (i, &v) in values.iter().enumerate() {
                if approx(next, v) {
                    counts[i] += 1;
                }
            }
        }
        for (i, &c) in counts.iter().enumerate() {
            let ratio = c as f64 / n as f64;
            assert!(
                (ratio - 1.0 / 3.0).abs() < 0.04,
                "state {} ratio = {ratio:.3}, expected ~0.333",
                values[i]
            );
        }
    }

    #[test]
    #[should_panic]
    fn from_weights_wrong_length_panics() {
        let states = vec![60.0, 64.0];
        let weights = vec![(60.0_f64, vec![1.0])]; // wrong length
        let _ = Markov::from_weights(states, weights);
    }

    // -----------------------------------------------------------------------
    // generate
    // -----------------------------------------------------------------------

    #[test]
    fn generate_correct_event_count() {
        let chain = Markov::new(
            vec![60.0, 64.0, 67.0],
            vec![
                (60.0, vec![(64.0, 1.0)]),
                (64.0, vec![(67.0, 1.0)]),
                (67.0, vec![(60.0, 1.0)]),
            ],
        );
        let pattern = chain.generate(16, 0.25, Some(60.0), 1);
        assert_eq!(pattern.len(), 16);
    }

    #[test]
    fn generate_zero_length_returns_empty() {
        let chain = Markov::new(vec![60.0], vec![]);
        assert!(chain.generate(0, 0.25, None, 1).is_empty());
    }

    #[test]
    fn generate_no_states_returns_empty() {
        let chain = Markov::new(vec![], vec![]);
        assert!(chain.generate(4, 0.25, None, 1).is_empty());
    }

    #[test]
    fn generate_uses_first_state_when_start_none() {
        let chain = Markov::new(
            vec![60.0, 64.0],
            vec![(60.0, vec![(64.0, 1.0)]), (64.0, vec![(60.0, 1.0)])],
        );
        let pattern = chain.generate(4, 0.25, None, 1);
        let events = pattern.events();
        assert!(
            approx(events[0].value, 60.0),
            "first event should be 60.0, got {}",
            events[0].value
        );
    }

    #[test]
    fn generate_deterministic_same_seed() {
        let p = Pattern::seq(&[60.0, 64.0, 67.0, 60.0, 64.0]);
        let chain = Markov::from_pattern(&p);

        let a = chain.generate(12, 0.25, Some(60.0), 42);
        let b = chain.generate(12, 0.25, Some(60.0), 42);

        let evs_a = a.events();
        let evs_b = b.events();
        assert_eq!(evs_a.len(), evs_b.len());
        for (ea, eb) in evs_a.iter().zip(evs_b.iter()) {
            assert!(
                approx(ea.value, eb.value),
                "same seed must give same values: {} vs {}",
                ea.value,
                eb.value
            );
        }
    }

    #[test]
    fn generate_different_seeds_different_output() {
        // Need a chain with branching transitions (not deterministic)
        let chain = Markov::new(
            vec![1.0, 2.0, 3.0],
            vec![
                (1.0, vec![(1.0, 0.5), (2.0, 0.3), (3.0, 0.2)]),
                (2.0, vec![(1.0, 0.3), (2.0, 0.4), (3.0, 0.3)]),
                (3.0, vec![(1.0, 0.2), (2.0, 0.3), (3.0, 0.5)]),
            ],
        );

        let a = chain.generate(20, 0.25, Some(1.0), 1);
        let b = chain.generate(20, 0.25, Some(1.0), 99999);

        let values_a: Vec<u64> = a.events().iter().map(|e| e.value.to_bits()).collect();
        let values_b: Vec<u64> = b.events().iter().map(|e| e.value.to_bits()).collect();
        assert_ne!(values_a, values_b, "different seeds should differ");
    }

    #[test]
    fn generate_all_events_have_correct_duration() {
        let chain = Markov::new(
            vec![60.0, 64.0],
            vec![(60.0, vec![(64.0, 1.0)]), (64.0, vec![(60.0, 1.0)])],
        );
        let pattern = chain.generate(8, 0.125, Some(60.0), 5);
        for e in pattern.events() {
            assert!(
                approx(e.duration, 0.125),
                "expected duration 0.125, got {}",
                e.duration
            );
        }
    }

    #[test]
    fn generate_values_are_valid_states() {
        let states = vec![60.0, 64.0, 67.0];
        let chain = Markov::new(
            states.clone(),
            vec![
                (60.0, vec![(64.0, 0.5), (67.0, 0.5)]),
                (64.0, vec![(60.0, 0.5), (67.0, 0.5)]),
                (67.0, vec![(60.0, 1.0)]),
            ],
        );
        let pattern = chain.generate(50, 0.25, Some(60.0), 77);
        for e in pattern.events() {
            let is_valid = states.iter().any(|&s| approx(s, e.value));
            assert!(is_valid, "unexpected state value {}", e.value);
        }
    }

    // -----------------------------------------------------------------------
    // step — edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn step_no_transition_returns_current() {
        // 64 has no outgoing transitions → step should return 64 unchanged.
        let chain = Markov::new(
            vec![60.0, 64.0],
            vec![(60.0, vec![(64.0, 1.0)])],
        );
        let next = chain.step(64.0, 0.5);
        assert!(approx(next, 64.0));
    }

    #[test]
    fn step_single_state_always_self_loops() {
        let chain = Markov::new(
            vec![60.0],
            vec![(60.0, vec![(60.0, 1.0)])],
        );
        let mut rng = SmallRng::seed_from_u64(3);
        for _ in 0..100 {
            let next = chain.step(60.0, rng.random::<f64>());
            assert!(approx(next, 60.0), "single state must self-loop");
        }
    }

    #[test]
    fn step_high_rand_val_hits_last_bucket() {
        // rand_val close to 1.0 should fall through to the last bucket.
        let chain = Markov::new(
            vec![60.0, 64.0],
            vec![(60.0, vec![(64.0, 1.0)])],
        );
        let next = chain.step(60.0, 0.9999999999);
        assert!(approx(next, 64.0));
    }

    // -----------------------------------------------------------------------
    // step — statistical distribution test
    // -----------------------------------------------------------------------

    #[test]
    fn step_follows_transition_probabilities() {
        // 60 → 64 (30%), 60 → 67 (70%).
        let chain = Markov::new(
            vec![60.0, 64.0, 67.0],
            vec![(60.0, vec![(64.0, 0.3), (67.0, 0.7)])],
        );

        let n = 20_000usize;
        let mut count_64 = 0usize;
        let mut count_67 = 0usize;
        let mut rng = SmallRng::seed_from_u64(55);
        for _ in 0..n {
            let next = chain.step(60.0, rng.random::<f64>());
            if approx(next, 64.0) {
                count_64 += 1;
            } else if approx(next, 67.0) {
                count_67 += 1;
            }
        }
        let r64 = count_64 as f64 / n as f64;
        let r67 = count_67 as f64 / n as f64;
        assert!(
            (r64 - 0.3).abs() < 0.02,
            "60→64 rate = {r64:.3}, expected ~0.3"
        );
        assert!(
            (r67 - 0.7).abs() < 0.02,
            "60→67 rate = {r67:.3}, expected ~0.7"
        );
    }
}
