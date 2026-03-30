/// Rhythm generators — patterns encoding complex rhythmic structures.
///
/// These build on the `Pattern` type and complement the existing
/// `euclidean()` and `swing()` functions. They encode the rhythmic
/// knowledge mined from hands-on composition: cross-rhythms,
/// accelerating fills, rolls, and delays.

use crate::pattern::Pattern;

/// Generate a cross-rhythm: `beats` evenly spaced events in one cycle.
///
/// This creates polyrhythmic tension against a 4/4 grid.
/// `cross_rhythm(3, 60.0)` = 3 events per cycle = 3-against-4 polyrhythm.
/// `cross_rhythm(5, 60.0)` = 5 events per cycle = quintuplet feel.
///
/// The total duration is 1.0 (one cycle).
pub fn cross_rhythm(beats: usize, value: f64) -> Pattern {
    if beats == 0 {
        return Pattern::Empty;
    }
    let dur = 1.0 / beats as f64;
    Pattern::Seq((0..beats).map(|_| Pattern::Atom(value, dur)).collect())
}

/// Generate a cross-rhythm with rests — hit pattern over a cross-grid.
///
/// Like `cross_rhythm` but only some of the `beats` positions have notes.
/// `hits` must be <= `beats`.
///
/// Uses the same Bjorklund distribution as `euclidean` but at a different
/// subdivision, creating polyrhythmic euclidean patterns.
pub fn cross_euclidean(hits: usize, beats: usize, value: f64) -> Pattern {
    if beats == 0 {
        return Pattern::Empty;
    }
    let hits = hits.min(beats);
    let dur = 1.0 / beats as f64;
    let grid = crate::construct::bjorklund(hits, beats);
    Pattern::Seq(
        grid.into_iter()
            .map(|hit| {
                if hit {
                    Pattern::Atom(value, dur)
                } else {
                    Pattern::Atom(f64::NAN, dur)
                }
            })
            .collect(),
    )
}

/// Accelerating fill leading to a target position.
///
/// `count` notes packed into `duration` cycles, with decreasing gaps
/// so they accelerate toward the end. The first gap is widest, the
/// last is narrowest — like a snare roll building to a drop.
///
/// Returns a pattern of total duration `duration`.
pub fn fill(value: f64, count: usize, duration: f64) -> Pattern {
    if count == 0 {
        return Pattern::Empty;
    }
    if count == 1 {
        return Pattern::Atom(value, duration);
    }
    // Weights: note i gets weight (count - i), so early notes are longer
    let weights: Vec<f64> = (0..count).map(|i| (count - i) as f64).collect();
    let total_weight: f64 = weights.iter().sum();
    Pattern::Seq(
        weights
            .iter()
            .map(|w| Pattern::Atom(value, w / total_weight * duration))
            .collect(),
    )
}

/// Decelerating fill — notes spread out over time.
///
/// Opposite of `fill`: starts dense and ends sparse.
/// Like a ball bouncing with decreasing energy.
pub fn decel(value: f64, count: usize, duration: f64) -> Pattern {
    if count == 0 {
        return Pattern::Empty;
    }
    if count == 1 {
        return Pattern::Atom(value, duration);
    }
    let weights: Vec<f64> = (0..count).map(|i| (i + 1) as f64).collect();
    let total_weight: f64 = weights.iter().sum();
    Pattern::Seq(
        weights
            .iter()
            .map(|w| Pattern::Atom(value, w / total_weight * duration))
            .collect(),
    )
}

/// Even roll — `count` equally spaced hits over `duration` cycles.
///
/// `roll(60.0, 16, 0.5)` = 16 even 32nd notes over half a cycle.
/// Use for snare rolls, tremolo, or rapid repetition.
pub fn roll(value: f64, count: usize, duration: f64) -> Pattern {
    if count == 0 {
        return Pattern::Empty;
    }
    let dur = duration / count as f64;
    Pattern::Seq((0..count).map(|_| Pattern::Atom(value, dur)).collect())
}

/// Delay a pattern by prepending silence.
///
/// The total duration increases by `amount`.
/// Use this to offset a pattern within a larger structure —
/// e.g., a snare displaced by a 16th note.
pub fn delayed(pattern: &Pattern, amount: f64) -> Pattern {
    if amount <= 0.0 {
        return pattern.clone();
    }
    Pattern::Seq(vec![
        Pattern::Atom(f64::NAN, amount),
        pattern.clone(),
    ])
}

/// Interleave a pattern with rests to create a sparse feel.
///
/// Every other event is replaced with silence. The timing grid
/// is preserved, but half the hits are missing. Stack the result
/// with the original and use different values for a ghosted feel.
pub fn every_other(pattern: &Pattern) -> Pattern {
    let events = pattern.events();
    let dur = pattern.duration();
    if events.is_empty() {
        return pattern.clone();
    }

    // Rebuild as flat Seq, replacing odd events with silence
    let mut atoms = Vec::new();
    let mut prev_end = 0.0;
    for (i, e) in events.iter().enumerate() {
        // Gap before this event
        if e.start > prev_end + 1e-10 {
            atoms.push(Pattern::Atom(f64::NAN, e.start - prev_end));
        }
        if i % 2 == 0 {
            atoms.push(Pattern::Atom(e.value, e.duration));
        } else {
            atoms.push(Pattern::Atom(f64::NAN, e.duration));
        }
        prev_end = e.start + e.duration;
    }
    // Trailing silence
    if prev_end < dur - 1e-10 {
        atoms.push(Pattern::Atom(f64::NAN, dur - prev_end));
    }
    Pattern::Seq(atoms)
}

/// Combine two rhythmic patterns at different rates — explicit polymetric.
///
/// `a_beats` events and `b_beats` events occupy the same cycle,
/// using `a_value` and `b_value` respectively. Creates a composite
/// pattern with both layers stacked.
pub fn polymetric(a_beats: usize, a_value: f64, b_beats: usize, b_value: f64) -> Pattern {
    Pattern::Stack(vec![
        cross_rhythm(a_beats, a_value),
        cross_rhythm(b_beats, b_value),
    ])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_rhythm_3_produces_3_events() {
        let p = cross_rhythm(3, 60.0);
        assert_eq!(p.events().len(), 3);
        let dur = p.duration();
        assert!((dur - 1.0).abs() < 1e-10);
    }

    #[test]
    fn cross_rhythm_0_is_empty() {
        assert!(cross_rhythm(0, 60.0).events().is_empty());
    }

    #[test]
    fn cross_rhythm_events_evenly_spaced() {
        let events = cross_rhythm(4, 60.0).events();
        let gap = 1.0 / 4.0;
        for (i, e) in events.iter().enumerate() {
            assert!((e.start - i as f64 * gap).abs() < 1e-10);
            assert!((e.duration - gap).abs() < 1e-10);
        }
    }

    #[test]
    fn fill_accelerates() {
        let events = fill(60.0, 4, 1.0).events();
        assert_eq!(events.len(), 4);
        // Each event should have shorter duration than the previous
        for i in 1..events.len() {
            assert!(events[i].duration < events[i - 1].duration);
        }
    }

    #[test]
    fn fill_total_duration() {
        let p = fill(60.0, 8, 0.5);
        assert!((p.duration() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn fill_single_note() {
        let p = fill(60.0, 1, 0.25);
        assert_eq!(p.events().len(), 1);
        assert!((p.duration() - 0.25).abs() < 1e-10);
    }

    #[test]
    fn decel_decelerates() {
        let events = decel(60.0, 4, 1.0).events();
        assert_eq!(events.len(), 4);
        for i in 1..events.len() {
            assert!(events[i].duration > events[i - 1].duration);
        }
    }

    #[test]
    fn roll_produces_even_notes() {
        let events = roll(60.0, 8, 1.0).events();
        assert_eq!(events.len(), 8);
        let dur = events[0].duration;
        for e in &events {
            assert!((e.duration - dur).abs() < 1e-10);
        }
    }

    #[test]
    fn delayed_shifts_events() {
        let p = Pattern::Atom(60.0, 0.5);
        let d = delayed(&p, 0.25);
        let events = d.events();
        assert_eq!(events.len(), 1);
        assert!((events[0].start - 0.25).abs() < 1e-10);
        assert!((d.duration() - 0.75).abs() < 1e-10);
    }

    #[test]
    fn delayed_zero_is_identity() {
        let p = Pattern::Atom(60.0, 0.5);
        let d = delayed(&p, 0.0);
        assert_eq!(d.events().len(), 1);
        assert!((d.events()[0].start).abs() < 1e-10);
    }

    #[test]
    fn cross_euclidean_3_8() {
        let p = cross_euclidean(3, 8, 60.0);
        let events = p.events();
        assert_eq!(events.len(), 3);
        assert!((p.duration() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn polymetric_combines_two_layers() {
        let p = polymetric(3, 60.0, 4, 72.0);
        let events = p.events();
        assert_eq!(events.len(), 7); // 3 + 4
    }

    #[test]
    fn every_other_halves_events() {
        let p = roll(60.0, 8, 1.0);
        let sparse = every_other(&p);
        assert_eq!(sparse.events().len(), 4);
        assert!((sparse.duration() - 1.0).abs() < 1e-10);
    }
}
