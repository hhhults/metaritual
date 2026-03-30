/// Property-based algebraic tests for the Pattern algebra.
///
/// These tests verify mathematical laws that should hold for all valid inputs:
/// associativity, identity, involution, commutativity (where applicable), and
/// composition laws for the various pattern transforms.

use metaritual::construct::euclidean;
use metaritual::pattern::{Event, Pattern};
use proptest::prelude::*;
use proptest::test_runner::Config as ProptestConfig;

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Generate a Pattern with 1–8 equal-duration events, values in 0.0..128.0.
fn arb_pattern() -> impl Strategy<Value = Pattern> {
    prop::collection::vec(0.0f64..128.0, 1..8)
        .prop_map(|values| Pattern::seq(&values))
}

// ---------------------------------------------------------------------------
// Helper: approximate event equality
// ---------------------------------------------------------------------------

fn events_approx_eq(a: &[Event], b: &[Event]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).all(|(ea, eb)| {
        (ea.value - eb.value).abs() < 1e-9
            && (ea.start - eb.start).abs() < 1e-9
            && (ea.duration - eb.duration).abs() < 1e-9
    })
}

fn events_sorted(evs: &[Event]) -> Vec<Event> {
    let mut v = evs.to_vec();
    v.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a.value
                    .partial_cmp(&b.value)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    v
}

// ---------------------------------------------------------------------------
// 1. Concat associativity: (a + b) + c  ≡  a + (b + c)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_concat_associativity(
        a in arb_pattern(),
        b in arb_pattern(),
        c in arb_pattern(),
    ) {
        let lhs = (a.clone() + b.clone()) + c.clone();
        let rhs = a + (b + c);
        prop_assert!(
            events_approx_eq(&lhs.events(), &rhs.events()),
            "concat not associative: lhs={:?}, rhs={:?}",
            lhs.events(),
            rhs.events()
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Concat identity: empty + p  ≡  p  and  p + empty  ≡  p
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_concat_left_identity(p in arb_pattern()) {
        let result = Pattern::empty() + p.clone();
        prop_assert!(
            events_approx_eq(&result.events(), &p.events()),
            "empty + p != p"
        );
    }

    #[test]
    fn prop_concat_right_identity(p in arb_pattern()) {
        let result = p.clone() + Pattern::empty();
        prop_assert!(
            events_approx_eq(&result.events(), &p.events()),
            "p + empty != p"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Stack commutativity: (a | b) ≡ (b | a)  [events sorted]
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_stack_commutativity(a in arb_pattern(), b in arb_pattern()) {
        let ab = events_sorted(&(a.clone() | b.clone()).events());
        let ba = events_sorted(&(b | a).events());
        prop_assert!(
            events_approx_eq(&ab, &ba),
            "stack not commutative by sorted events"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Repeat consistency: (p * n).len() == n * p.len()
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_repeat_event_count(p in arb_pattern(), n in 1usize..8) {
        let expected = n * p.len();
        let actual = (p * n).len();
        prop_assert_eq!(actual, expected, "p * n should have n * p.len() events");
    }
}

// ---------------------------------------------------------------------------
// 5. Rotate identity: p.rotate(0) ≡ p
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_rotate_identity(p in arb_pattern()) {
        let rotated = p.rotate(0);
        prop_assert!(
            events_approx_eq(&rotated.events(), &p.events()),
            "rotate(0) is not identity"
        );
    }
}

// ---------------------------------------------------------------------------
// 6. Rotate inverse: p.rotate(k).rotate(-k) ≡ p
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_rotate_inverse(p in arb_pattern(), k in -8i32..8) {
        let round_tripped = p.rotate(k).rotate(-k);
        prop_assert!(
            events_approx_eq(&round_tripped.events(), &p.events()),
            "rotate({k}).rotate({neg_k}) is not identity",
            neg_k = -k
        );
    }
}

// ---------------------------------------------------------------------------
// 7. Retrograde involution: p.retrograde().retrograde() ≡ p
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_retrograde_involution(p in arb_pattern()) {
        let double_retro = p.retrograde().retrograde();
        prop_assert!(
            events_approx_eq(&double_retro.events(), &p.events()),
            "retrograde().retrograde() is not identity"
        );
    }
}

// ---------------------------------------------------------------------------
// 8. Shift linearity: p.shift(a).shift(b) ≡ p.shift(a + b)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_shift_linearity(
        p in arb_pattern(),
        a in -100.0f64..100.0,
        b in -100.0f64..100.0,
    ) {
        let sequential = p.shift(a).shift(b);
        let combined = p.shift(a + b);
        prop_assert!(
            events_approx_eq(&sequential.events(), &combined.events()),
            "shift({a}).shift({b}) != shift({})",
            a + b
        );
    }
}

// ---------------------------------------------------------------------------
// 9. Invert involution: p.invert(x).invert(x) ≡ p
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_invert_involution(p in arb_pattern(), x in -100.0f64..100.0) {
        let double_inverted = p.invert(x).invert(x);
        prop_assert!(
            events_approx_eq(&double_inverted.events(), &p.events()),
            "invert({x}).invert({x}) is not identity"
        );
    }
}

// ---------------------------------------------------------------------------
// 10. Stretch identity: p.stretch(1.0) ≡ p
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_stretch_identity(p in arb_pattern()) {
        let stretched = p.stretch(1.0);
        prop_assert!(
            events_approx_eq(&stretched.events(), &p.events()),
            "stretch(1.0) is not identity"
        );
    }
}

// ---------------------------------------------------------------------------
// 11. Stretch composition: p.stretch(a).stretch(b) has same duration as p.stretch(a * b)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_stretch_composition(
        p in arb_pattern(),
        a in 0.1f64..10.0,
        b in 0.1f64..10.0,
    ) {
        let sequential_dur = p.stretch(a).stretch(b).duration();
        let combined_dur = p.stretch(a * b).duration();
        prop_assert!(
            (sequential_dur - combined_dur).abs() < 1e-9,
            "stretch({a}).stretch({b}) duration {sequential_dur} != stretch({}) duration {combined_dur}",
            a * b
        );
    }
}

// ---------------------------------------------------------------------------
// 12. Euclidean hit count: euclidean(k, n).events().len() == k
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn prop_euclidean_hit_count(
        steps in 1usize..17,
        hits_frac in 0usize..17,
    ) {
        // Clamp hits to [0, steps]
        let hits = hits_frac % (steps + 1);
        let p = euclidean(hits, steps);
        let actual = p.events().len();
        prop_assert_eq!(
            actual, hits,
            "euclidean({}, {}) produced {} hits, expected {}",
            hits, steps, actual, hits
        );
    }
}

// ---------------------------------------------------------------------------
// 13. Degrade bounds
//     degrade(0.0, seed).events() == p.events()
//     degrade(1.0, seed).events().is_empty()
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_degrade_zero_is_identity(p in arb_pattern(), seed in 0u64..u64::MAX) {
        let degraded = p.degrade(0.0, seed);
        prop_assert!(
            events_approx_eq(&degraded.events(), &p.events()),
            "degrade(0.0) is not identity"
        );
    }

    #[test]
    fn prop_degrade_one_drops_all(p in arb_pattern(), seed in 0u64..u64::MAX) {
        let degraded = p.degrade(1.0, seed);
        prop_assert!(
            degraded.events().is_empty(),
            "degrade(1.0) left {} events",
            degraded.events().len()
        );
    }
}
