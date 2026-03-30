/// Euclidean rhythm constructors and scale/chord builders.
///
/// Rhythms via the Bjorklund algorithm. Scales and chords as Pattern
/// structures ready to be combined with rhythm patterns.

use crate::pattern::Pattern;

// ---------------------------------------------------------------------------
// Euclidean rhythms
// ---------------------------------------------------------------------------

/// Distribute `hits` pulses as evenly as possible across `steps` via Bjorklund.
///
/// Returns a `Seq` of `steps` atoms, each with duration `1.0 / steps`.
/// Hit atoms carry value `1.0`; rests carry `f64::NAN` (silent).
pub fn euclidean(hits: usize, steps: usize) -> Pattern {
    if steps == 0 {
        return Pattern::empty();
    }
    let hits = hits.min(steps);

    // Build bool sequence via Bjorklund
    let bools = bjorklund(hits, steps);

    let dur = 1.0 / steps as f64;
    Pattern::Seq(
        bools
            .into_iter()
            .map(|h| {
                if h {
                    Pattern::Atom(1.0, dur)
                } else {
                    Pattern::Atom(f64::NAN, dur)
                }
            })
            .collect(),
    )
}

/// Same as `euclidean`, but the pattern is rotated left by `rotation` steps.
pub fn euclidean_rotated(hits: usize, steps: usize, rotation: usize) -> Pattern {
    if steps == 0 {
        return Pattern::empty();
    }
    let hits = hits.min(steps);
    let mut bools = bjorklund(hits, steps);
    if steps > 0 {
        let rot = rotation % steps;
        bools.rotate_left(rot);
    }
    let dur = 1.0 / steps as f64;
    Pattern::Seq(
        bools
            .into_iter()
            .map(|h| {
                if h {
                    Pattern::Atom(1.0, dur)
                } else {
                    Pattern::Atom(f64::NAN, dur)
                }
            })
            .collect(),
    )
}

/// Bjorklund algorithm — distributes `hits` across `steps` as evenly as possible.
/// Returns a Vec<bool> of length `steps`.
pub fn bjorklund(hits: usize, steps: usize) -> Vec<bool> {
    if hits == 0 {
        return vec![false; steps];
    }
    if hits >= steps {
        return vec![true; steps];
    }

    // Track count/remainder numerically (standard Bresenham-style Bjorklund).
    // Build groups: `hits` × [true], `(steps - hits)` × [false].
    let mut groups: Vec<Vec<bool>> = Vec::with_capacity(steps);
    for _ in 0..hits {
        groups.push(vec![true]);
    }
    for _ in 0..(steps - hits) {
        groups.push(vec![false]);
    }

    let mut count = hits;
    let mut remainder = steps - hits;

    while remainder > 1 {
        let pairs = count.min(remainder);
        let n = groups.len();

        let mut new_groups: Vec<Vec<bool>> = Vec::with_capacity(n - pairs);
        for i in 0..pairs {
            let mut merged = groups[i].clone();
            merged.extend_from_slice(&groups[n - pairs + i]);
            new_groups.push(merged);
        }
        for i in pairs..(n - pairs) {
            new_groups.push(groups[i].clone());
        }
        groups = new_groups;

        let new_remainder = count.abs_diff(remainder);
        count = pairs;
        remainder = new_remainder;
    }

    groups.into_iter().flatten().collect()
}

// ---------------------------------------------------------------------------
// Scale constants
// ---------------------------------------------------------------------------

pub const MAJOR: &[u8] = &[0, 2, 4, 5, 7, 9, 11];
pub const MINOR: &[u8] = &[0, 2, 3, 5, 7, 8, 10];
pub const DORIAN: &[u8] = &[0, 2, 3, 5, 7, 9, 10];
pub const PHRYGIAN: &[u8] = &[0, 1, 3, 5, 7, 8, 10];
pub const LYDIAN: &[u8] = &[0, 2, 4, 6, 7, 9, 11];
pub const MIXOLYDIAN: &[u8] = &[0, 2, 4, 5, 7, 9, 10];
pub const PENTATONIC: &[u8] = &[0, 2, 4, 7, 9];
pub const MINOR_PENTATONIC: &[u8] = &[0, 3, 5, 7, 10];
pub const BLUES: &[u8] = &[0, 3, 5, 6, 7, 10];
pub const WHOLE_TONE: &[u8] = &[0, 2, 4, 6, 8, 10];
pub const CHROMATIC: &[u8] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

// ---------------------------------------------------------------------------
// Chord interval constants
// ---------------------------------------------------------------------------

pub const MAJOR_TRIAD: &[u8] = &[0, 4, 7];
pub const MINOR_TRIAD: &[u8] = &[0, 3, 7];
pub const DIM: &[u8] = &[0, 3, 6];
pub const AUG: &[u8] = &[0, 4, 8];
pub const MAJOR_7: &[u8] = &[0, 4, 7, 11];
pub const MINOR_7: &[u8] = &[0, 3, 7, 10];
pub const DOM_7: &[u8] = &[0, 4, 7, 10];
pub const SUS2: &[u8] = &[0, 2, 7];
pub const SUS4: &[u8] = &[0, 5, 7];
pub const ADD9: &[u8] = &[0, 2, 4, 7];

// ---------------------------------------------------------------------------
// Scale and chord builders
// ---------------------------------------------------------------------------

/// Build a sequential scale Pattern spanning `octaves` octaves.
///
/// `root` is a MIDI note number. `intervals` gives semitone offsets within
/// one octave. All notes get equal duration; total duration = 1.0 cycle.
///
/// Example: `scale(60.0, MAJOR, 1)` → 7-note C major starting at MIDI 60,
/// each note lasting 1/7 of a cycle.
pub fn scale(root: f64, intervals: &[u8], octaves: usize) -> Pattern {
    if intervals.is_empty() || octaves == 0 {
        return Pattern::empty();
    }

    let mut notes: Vec<f64> = Vec::new();
    for oct in 0..octaves {
        for &interval in intervals {
            notes.push(root + (oct as f64 * 12.0) + interval as f64);
        }
    }

    let dur = 1.0 / notes.len() as f64;
    Pattern::Seq(notes.into_iter().map(|n| Pattern::Atom(n, dur)).collect())
}

/// Build a chord as a stacked Pattern (all notes simultaneous).
///
/// `root` is a MIDI note number. `intervals` gives semitone offsets.
/// Each note atom gets the given `duration`.
pub fn chord(root: f64, intervals: &[u8], duration: f64) -> Pattern {
    if intervals.is_empty() {
        return Pattern::empty();
    }
    Pattern::Stack(
        intervals
            .iter()
            .map(|&i| Pattern::Atom(root + i as f64, duration))
            .collect(),
    )
}

/// Build a chord of stacked perfect fourths (5 semitones) from `root`.
pub fn stacked_fourths(root: f64, count: usize) -> Pattern {
    if count == 0 {
        return Pattern::empty();
    }
    let intervals: Vec<u8> = (0..count).map(|i| (i * 5) as u8).collect();
    chord(root, &intervals, 1.0)
}

/// Build a chord of stacked perfect fifths (7 semitones) from `root`.
pub fn stacked_fifths(root: f64, count: usize) -> Pattern {
    if count == 0 {
        return Pattern::empty();
    }
    let intervals: Vec<u8> = (0..count).map(|i| (i * 7) as u8).collect();
    chord(root, &intervals, 1.0)
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

    // --- Euclidean -----------------------------------------------------------

    #[test]
    fn euclidean_hit_and_step_count() {
        let p = euclidean(3, 8);
        // Must produce exactly 8 atoms total
        let all_atoms = match &p {
            Pattern::Seq(ps) => ps.len(),
            _ => panic!("expected Seq"),
        };
        assert_eq!(all_atoms, 8);
        // Exactly 3 non-NaN (sounding) events
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn euclidean_all_hits() {
        let p = euclidean(5, 5);
        assert_eq!(p.len(), 5);
        // All events should be sounding
        let evs = p.events();
        assert!(evs.iter().all(|e| !e.value.is_nan()));
    }

    #[test]
    fn euclidean_no_hits() {
        let p = euclidean(0, 8);
        assert_eq!(p.len(), 0);
        // But still 8 atoms in the Seq (all silent)
        let all_atoms = match &p {
            Pattern::Seq(ps) => ps.len(),
            _ => panic!("expected Seq"),
        };
        assert_eq!(all_atoms, 8);
    }

    #[test]
    fn euclidean_duration_sums_to_one() {
        let p = euclidean(3, 8);
        assert!(approx(p.duration(), 1.0));
    }

    #[test]
    fn euclidean_3_8_known_pattern() {
        // E(3,8) = [1,0,0,1,0,0,1,0] — the standard tresillo
        let p = euclidean(3, 8);
        let bools: Vec<bool> = match &p {
            Pattern::Seq(ps) => ps
                .iter()
                .map(|atom| match atom {
                    Pattern::Atom(v, _) => !v.is_nan(),
                    _ => false,
                })
                .collect(),
            _ => panic!("expected Seq"),
        };
        assert_eq!(bools, vec![true, false, false, true, false, false, true, false]);
    }

    #[test]
    fn euclidean_rotated_shifts_pattern() {
        let p0 = euclidean(3, 8);
        let p1 = euclidean_rotated(3, 8, 3);
        // They should have the same number of hits but different positions
        assert_eq!(p0.len(), p1.len());
        let evs0 = p0.events();
        let evs1 = p1.events();
        // Starts should differ
        let starts0: Vec<f64> = evs0.iter().map(|e| e.start).collect();
        let starts1: Vec<f64> = evs1.iter().map(|e| e.start).collect();
        assert_ne!(starts0, starts1);
    }

    #[test]
    fn euclidean_zero_steps_is_empty() {
        let p = euclidean(3, 0);
        assert!(p.is_empty());
    }

    #[test]
    fn euclidean_hits_clamped_to_steps() {
        // hits > steps is clamped to all-hits
        let p = euclidean(10, 4);
        assert_eq!(p.len(), 4);
    }

    // --- Scale ---------------------------------------------------------------

    #[test]
    fn scale_major_one_octave() {
        let p = scale(60.0, MAJOR, 1);
        let evs = p.events();
        assert_eq!(evs.len(), 7);
        // First note is root
        assert!(approx(evs[0].value, 60.0));
        // All durations equal 1/7
        for e in &evs {
            assert!(approx(e.duration, 1.0 / 7.0));
        }
    }

    #[test]
    fn scale_major_notes_correct() {
        let p = scale(60.0, MAJOR, 1);
        let values: Vec<f64> = p.events().iter().map(|e| e.value).collect();
        let expected: Vec<f64> = MAJOR.iter().map(|&i| 60.0 + i as f64).collect();
        for (v, e) in values.iter().zip(expected.iter()) {
            assert!(approx(*v, *e));
        }
    }

    #[test]
    fn scale_two_octaves() {
        let p = scale(60.0, MAJOR, 2);
        assert_eq!(p.events().len(), 14);
        let evs = p.events();
        // 8th note should be root + 12
        assert!(approx(evs[7].value, 72.0));
    }

    #[test]
    fn scale_duration_is_one_cycle() {
        let p = scale(60.0, PENTATONIC, 1);
        assert!(approx(p.duration(), 1.0));
    }

    #[test]
    fn scale_empty_intervals_returns_empty() {
        let p = scale(60.0, &[], 1);
        assert!(p.is_empty());
    }

    // --- Chord ---------------------------------------------------------------

    #[test]
    fn chord_major_triad() {
        let p = chord(60.0, MAJOR_TRIAD, 1.0);
        let evs = p.events();
        assert_eq!(evs.len(), 3);
        // All simultaneous
        for e in &evs {
            assert!(approx(e.start, 0.0));
        }
    }

    #[test]
    fn chord_correct_pitches() {
        let p = chord(60.0, MAJOR_TRIAD, 1.0);
        let mut values: Vec<f64> = p.events().iter().map(|e| e.value).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!(approx(values[0], 60.0));
        assert!(approx(values[1], 64.0));
        assert!(approx(values[2], 67.0));
    }

    #[test]
    fn chord_duration_applied() {
        let p = chord(60.0, MINOR_TRIAD, 0.5);
        let evs = p.events();
        for e in &evs {
            assert!(approx(e.duration, 0.5));
        }
    }

    // --- Stacked intervals ---------------------------------------------------

    #[test]
    fn stacked_fourths_count() {
        let p = stacked_fourths(60.0, 4);
        assert_eq!(p.events().len(), 4);
        let mut values: Vec<f64> = p.events().iter().map(|e| e.value).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // 60, 65, 70, 75
        assert!(approx(values[0], 60.0));
        assert!(approx(values[1], 65.0));
        assert!(approx(values[2], 70.0));
        assert!(approx(values[3], 75.0));
    }

    #[test]
    fn stacked_fifths_count() {
        let p = stacked_fifths(60.0, 3);
        assert_eq!(p.events().len(), 3);
        let mut values: Vec<f64> = p.events().iter().map(|e| e.value).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // 60, 67, 74
        assert!(approx(values[0], 60.0));
        assert!(approx(values[1], 67.0));
        assert!(approx(values[2], 74.0));
    }
}
