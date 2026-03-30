/// Combining patterns — morphisms between pattern spaces.
///
/// Rhythm + melody → events. Different combination strategies
/// produce different music from the same material.

use crate::pattern::{Event, Pattern};

/// Strategy for combining two patterns.
pub enum Combine {
    /// Pair events 1:1 by index. Truncates to the shorter pattern.
    Zip,
    /// The shorter pattern's events cycle to match the longer.
    /// E.g. if `a` has 5 events and `b` has 3, `b` cycles: b0 b1 b2 b0 b1.
    Cycle,
    /// Both patterns keep independent timing. Each event from `a` is paired
    /// with the nearest-in-time event from `b`. Produces polymetric relationships.
    Polymetric,
}

/// Combine two patterns (e.g. rhythm + pitch) into paired events.
///
/// `a` typically carries rhythmic/onset information (value = 1.0 for a hit).
/// `b` typically carries pitch or another dimension.
///
/// Returns a `Vec<(f64, f64)>` of `(a_value, b_value)` pairs.
pub fn combine(a: &Pattern, b: &Pattern, how: Combine) -> Vec<(f64, f64)> {
    let a_events = a.events();
    let b_events = b.events();

    if a_events.is_empty() || b_events.is_empty() {
        return vec![];
    }

    match how {
        Combine::Zip => zip(&a_events, &b_events),
        Combine::Cycle => cycle(&a_events, &b_events),
        Combine::Polymetric => polymetric(&a_events, &b_events),
    }
}

// ---------------------------------------------------------------------------
// Strategy implementations
// ---------------------------------------------------------------------------

/// Pair events 1:1 by index; truncate to the shorter list.
fn zip(a: &[Event], b: &[Event]) -> Vec<(f64, f64)> {
    a.iter()
        .zip(b.iter())
        .map(|(ea, eb)| (ea.value, eb.value))
        .collect()
}

/// The shorter list cycles to match the length of the longer.
fn cycle(a: &[Event], b: &[Event]) -> Vec<(f64, f64)> {
    let len = a.len().max(b.len());
    (0..len)
        .map(|i| {
            let av = a[i % a.len()].value;
            let bv = b[i % b.len()].value;
            (av, bv)
        })
        .collect()
}

/// Each event from `a` is paired with the nearest-in-time event from `b`.
/// "Nearest" means the `b` event whose `start` time is closest to `a`'s `start`.
/// If `b` events share equal distance, the earlier one is preferred.
fn polymetric(a: &[Event], b: &[Event]) -> Vec<(f64, f64)> {
    a.iter()
        .map(|ea| {
            let nearest = b
                .iter()
                .min_by(|x, y| {
                    let dx = (x.start - ea.start).abs();
                    let dy = (y.start - ea.start).abs();
                    dx.partial_cmp(&dy).unwrap_or(std::cmp::Ordering::Equal)
                })
                .expect("b is non-empty; checked above");
            (ea.value, nearest.value)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::Pattern;

    // --- Zip -----------------------------------------------------------------

    #[test]
    fn zip_equal_lengths() {
        let a = Pattern::seq(&[1.0, 2.0, 3.0]);
        let b = Pattern::seq(&[10.0, 20.0, 30.0]);
        let pairs = combine(&a, &b, Combine::Zip);
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], (1.0, 10.0));
        assert_eq!(pairs[1], (2.0, 20.0));
        assert_eq!(pairs[2], (3.0, 30.0));
    }

    #[test]
    fn zip_truncates_to_shorter_a_shorter() {
        let a = Pattern::seq(&[1.0, 2.0]);
        let b = Pattern::seq(&[10.0, 20.0, 30.0]);
        let pairs = combine(&a, &b, Combine::Zip);
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn zip_truncates_to_shorter_b_shorter() {
        let a = Pattern::seq(&[1.0, 2.0, 3.0, 4.0]);
        let b = Pattern::seq(&[10.0, 20.0]);
        let pairs = combine(&a, &b, Combine::Zip);
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn zip_empty_a_returns_empty() {
        let a = Pattern::empty();
        let b = Pattern::seq(&[1.0, 2.0]);
        let pairs = combine(&a, &b, Combine::Zip);
        assert!(pairs.is_empty());
    }

    #[test]
    fn zip_empty_b_returns_empty() {
        let a = Pattern::seq(&[1.0, 2.0]);
        let b = Pattern::empty();
        let pairs = combine(&a, &b, Combine::Zip);
        assert!(pairs.is_empty());
    }

    // --- Cycle ---------------------------------------------------------------

    #[test]
    fn cycle_b_cycles_to_match_a() {
        // a has 5, b has 3 → b cycles: b0 b1 b2 b0 b1
        let a = Pattern::seq(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        let b = Pattern::seq(&[10.0, 20.0, 30.0]);
        let pairs = combine(&a, &b, Combine::Cycle);
        assert_eq!(pairs.len(), 5);
        let b_vals: Vec<f64> = pairs.iter().map(|(_, bv)| *bv).collect();
        assert_eq!(b_vals, vec![10.0, 20.0, 30.0, 10.0, 20.0]);
    }

    #[test]
    fn cycle_a_cycles_to_match_b() {
        // a has 2, b has 4 → a cycles: a0 a1 a0 a1
        let a = Pattern::seq(&[1.0, 2.0]);
        let b = Pattern::seq(&[10.0, 20.0, 30.0, 40.0]);
        let pairs = combine(&a, &b, Combine::Cycle);
        assert_eq!(pairs.len(), 4);
        let a_vals: Vec<f64> = pairs.iter().map(|(av, _)| *av).collect();
        assert_eq!(a_vals, vec![1.0, 2.0, 1.0, 2.0]);
    }

    #[test]
    fn cycle_equal_lengths_same_as_zip() {
        let a = Pattern::seq(&[1.0, 2.0, 3.0]);
        let b = Pattern::seq(&[10.0, 20.0, 30.0]);
        let zip_pairs = combine(&a, &b, Combine::Zip);
        let cycle_pairs = combine(&a, &b, Combine::Cycle);
        assert_eq!(zip_pairs, cycle_pairs);
    }

    #[test]
    fn cycle_produces_longer_length() {
        let a = Pattern::seq(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        let b = Pattern::seq(&[10.0]);
        let pairs = combine(&a, &b, Combine::Cycle);
        assert_eq!(pairs.len(), 5);
        // All b values should be 10.0
        assert!(pairs.iter().all(|(_, bv)| *bv == 10.0));
    }

    // --- Polymetric ----------------------------------------------------------

    #[test]
    fn polymetric_pairs_by_nearest_time() {
        // a: 4 events at 0, 0.25, 0.5, 0.75
        // b: 3 events at 0, 1/3, 2/3
        // nearest mapping:
        //   a@0.0   → b@0.0   (dist=0)
        //   a@0.25  → b@0.333 (dist=0.083) vs b@0.0 (dist=0.25) → b@1/3
        //   a@0.5   → b@0.333 (dist=0.167) vs b@0.667 (dist=0.167) → tie → earlier = b@1/3
        //   a@0.75  → b@0.667 (dist=0.083) → b@2/3
        let a = Pattern::seq(&[1.0, 2.0, 3.0, 4.0]); // durations 0.25 each
        let b = Pattern::seq(&[10.0, 20.0, 30.0]); // durations 1/3 each

        let pairs = combine(&a, &b, Combine::Polymetric);
        assert_eq!(pairs.len(), 4);

        // a-side values should come in order from a's events
        let a_vals: Vec<f64> = pairs.iter().map(|(av, _)| *av).collect();
        assert_eq!(a_vals, vec![1.0, 2.0, 3.0, 4.0]);

        // b-side: verify specific nearest matches
        assert_eq!(pairs[0].1, 10.0); // a@0.0 → b@0.0
        assert_eq!(pairs[1].1, 20.0); // a@0.25 → nearest is b@1/3
        assert_eq!(pairs[3].1, 30.0); // a@0.75 → b@2/3
    }

    #[test]
    fn polymetric_same_pattern_pairs_with_itself() {
        let a = Pattern::seq(&[60.0, 64.0, 67.0]);
        let b = Pattern::seq(&[60.0, 64.0, 67.0]);
        let pairs = combine(&a, &b, Combine::Polymetric);
        // Each event maps to itself
        assert_eq!(pairs.len(), 3);
        for (av, bv) in &pairs {
            assert_eq!(av, bv);
        }
    }
}
