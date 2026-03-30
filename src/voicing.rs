/// Voicing — chord construction with musical voice leading.
///
/// Builds on the `chord()` function from `construct.rs` with
/// techniques for wide voicings, staggered entries (bloom),
/// chord progressions, and smooth voice leading.
///
/// All timing is cycle-relative (0.0..1.0).
///
/// `intervals` uses the same convention as `construct.rs`:
/// absolute semitone offsets from root (e.g., MINOR_7 = [0, 3, 7, 10]).

use crate::pattern::Pattern;

/// Build pitch list from root + absolute offsets.
fn pitches_from(root: u8, intervals: &[u8]) -> Vec<f64> {
    intervals.iter().map(|&i| root as f64 + i as f64).collect()
}

/// Bloom: stagger chord entries so voices open like a flower.
///
/// Each voice enters `spread` cycles after the previous one.
/// The root sounds first, then each upper voice follows.
/// Creates a "growing" or "breathing" quality.
///
/// `bloom(58, &[0, 3, 7, 10], 0.02, 1.0)` = Bbm7 chord where
/// the third enters 0.02 after root, fifth at 0.04, seventh at 0.06.
pub fn bloom(root: u8, intervals: &[u8], spread: f64, duration: f64) -> Pattern {
    let pitches = pitches_from(root, intervals);
    if pitches.is_empty() {
        return Pattern::Empty;
    }

    let voices: Vec<Pattern> = pitches
        .iter()
        .enumerate()
        .map(|(i, &pitch)| {
            let delay = i as f64 * spread;
            let note_dur = (duration - delay).max(0.01);
            if delay < 1e-10 {
                Pattern::Atom(pitch, note_dur)
            } else {
                Pattern::Seq(vec![
                    Pattern::Atom(f64::NAN, delay),
                    Pattern::Atom(pitch, note_dur),
                ])
            }
        })
        .collect();

    Pattern::Stack(voices)
}

/// Open voicing: drop every other voice down an octave.
///
/// Takes a close-position chord and creates a wider spread by
/// lowering alternate voices by 12 semitones.
/// Drops voices at even indices > 0 (i.e., indices 2, 4, ...).
/// This keeps root in place and spreads inner voices.
pub fn open(root: u8, intervals: &[u8], duration: f64) -> Pattern {
    let mut pitches = pitches_from(root, intervals);

    // Drop every other inner voice down an octave
    for i in (2..pitches.len()).step_by(2) {
        pitches[i] -= 12.0;
    }

    Pattern::Stack(
        pitches
            .iter()
            .map(|&pitch| Pattern::Atom(pitch, duration))
            .collect(),
    )
}

/// Wide voicing: spread chord across multiple octaves.
///
/// Distributes chord tones across `octave_span` octaves by
/// assigning each voice to an octave based on its index.
/// Creates a very spacious, orchestral spread.
pub fn wide(root: u8, intervals: &[u8], octave_span: usize, duration: f64) -> Pattern {
    let mut pitches = pitches_from(root, intervals);

    let n = pitches.len();
    if n > 1 && octave_span > 1 {
        for i in 0..n {
            let target_octave = (i * octave_span) / n;
            pitches[i] += (target_octave as f64) * 12.0;
        }
    }

    Pattern::Stack(
        pitches
            .iter()
            .map(|&pitch| Pattern::Atom(pitch, duration))
            .collect(),
    )
}

/// Bloomed open voicing: wide spread with staggered entries.
///
/// Combines `open` voicing with `bloom` timing. The voices are
/// spread across octaves AND enter one at a time.
pub fn bloom_open(root: u8, intervals: &[u8], spread: f64, duration: f64) -> Pattern {
    let mut pitches = pitches_from(root, intervals);

    // Drop every other inner voice
    for i in (2..pitches.len()).step_by(2) {
        pitches[i] -= 12.0;
    }

    // Stagger entries
    let voices: Vec<Pattern> = pitches
        .iter()
        .enumerate()
        .map(|(i, &pitch)| {
            let delay = i as f64 * spread;
            let note_dur = (duration - delay).max(0.01);
            if delay < 1e-10 {
                Pattern::Atom(pitch, note_dur)
            } else {
                Pattern::Seq(vec![
                    Pattern::Atom(f64::NAN, delay),
                    Pattern::Atom(pitch, note_dur),
                ])
            }
        })
        .collect();

    Pattern::Stack(voices)
}

/// Chord progression: sequence of chords with equal durations.
///
/// Each chord is a simultaneous `Stack`. The total pattern
/// is a `Seq` of these stacks.
///
/// ```ignore
/// use metaritual::construct::{MINOR_7, MAJOR_7};
/// let prog = progression(&[
///     (58, MINOR_7),    // Bbm7
///     (61, MAJOR_7),    // Dbmaj7
///     (56, MINOR_7),    // Abm7
/// ], 3.0);
/// // Three chords, each 1.0 cycle, total 3.0 cycles
/// ```
pub fn progression(chords: &[(u8, &[u8])], total_duration: f64) -> Pattern {
    if chords.is_empty() {
        return Pattern::Empty;
    }
    let chord_dur = total_duration / chords.len() as f64;
    Pattern::Seq(
        chords
            .iter()
            .map(|(root, intervals)| {
                let pitches = pitches_from(*root, intervals);
                Pattern::Stack(
                    pitches
                        .iter()
                        .map(|&pitch| Pattern::Atom(pitch, chord_dur))
                        .collect(),
                )
            })
            .collect(),
    )
}

/// Bloomed chord progression: sequence of blooming chords.
///
/// Like `progression` but each chord blooms open with staggered entries.
pub fn bloom_progression(
    chords: &[(u8, &[u8])],
    spread: f64,
    total_duration: f64,
) -> Pattern {
    if chords.is_empty() {
        return Pattern::Empty;
    }
    let chord_dur = total_duration / chords.len() as f64;
    Pattern::Seq(
        chords
            .iter()
            .map(|(root, intervals)| bloom(*root, intervals, spread, chord_dur))
            .collect(),
    )
}

/// Voice-led progression: chords connected by minimal voice movement.
///
/// For each pair of adjacent chords, finds the octave transposition
/// of each pitch that minimizes distance to the previous chord's center.
/// This creates smooth, connected harmony rather than jumping between
/// root positions.
pub fn voice_led(chords: &[(u8, &[u8])], total_duration: f64) -> Pattern {
    if chords.is_empty() {
        return Pattern::Empty;
    }
    let chord_dur = total_duration / chords.len() as f64;

    let mut voiced: Vec<Vec<f64>> = Vec::new();

    for (i, (root, intervals)) in chords.iter().enumerate() {
        let mut pitches = pitches_from(*root, intervals);

        if i > 0 {
            let prev = &voiced[i - 1];
            let prev_center: f64 = prev.iter().sum::<f64>() / prev.len() as f64;

            for pitch in &mut pitches {
                let base = *pitch % 12.0;
                let mut best = *pitch;
                let mut best_dist = (best - prev_center).abs();
                for oct in -2..=3i32 {
                    let candidate = base + ((*root as f64 / 12.0).floor() + oct as f64) * 12.0;
                    let dist = (candidate - prev_center).abs();
                    if dist < best_dist {
                        best = candidate;
                        best_dist = dist;
                    }
                }
                *pitch = best;
            }
        }

        voiced.push(pitches);
    }

    Pattern::Seq(
        voiced
            .iter()
            .map(|pitches| {
                Pattern::Stack(
                    pitches
                        .iter()
                        .map(|&pitch| Pattern::Atom(pitch, chord_dur))
                        .collect(),
                )
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
    use crate::construct::{MINOR_TRIAD, MINOR_7, MAJOR_7};

    #[test]
    fn bloom_produces_staggered_events() {
        let p = bloom(60, MINOR_TRIAD, 0.1, 1.0); // Cm
        let events = p.events();
        assert_eq!(events.len(), 3);
        // Root at t=0, third at t=0.1, fifth at t=0.2
        assert!((events[0].start).abs() < 1e-10);
        assert!((events[1].start - 0.1).abs() < 1e-10);
        assert!((events[2].start - 0.2).abs() < 1e-10);
    }

    #[test]
    fn bloom_pitches_correct() {
        let p = bloom(60, MINOR_TRIAD, 0.05, 1.0);
        let events = p.events();
        assert_eq!(events[0].value, 60.0); // C
        assert_eq!(events[1].value, 63.0); // Eb
        assert_eq!(events[2].value, 67.0); // G
    }

    #[test]
    fn bloom_zero_spread_is_chord() {
        let p = bloom(60, MINOR_TRIAD, 0.0, 1.0);
        let events = p.events();
        assert_eq!(events.len(), 3);
        for e in &events {
            assert!((e.start).abs() < 1e-10);
        }
    }

    #[test]
    fn open_drops_inner_voices() {
        let p = open(60, MINOR_7, 1.0); // Cm7 = [60, 63, 67, 70]
        let events = p.events();
        assert_eq!(events.len(), 4);
        let mut pitches: Vec<f64> = events.iter().map(|e| e.value).collect();
        pitches.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // Index 0: 60 (kept), index 1: 63 (kept),
        // index 2: 67-12=55 (dropped), index 3: 70 (kept)
        assert!(pitches.contains(&60.0));
        assert!(pitches.contains(&63.0));
        assert!(pitches.contains(&55.0)); // dropped G
        assert!(pitches.contains(&70.0));
    }

    #[test]
    fn wide_spreads_across_octaves() {
        let close = crate::construct::chord(60.0, MINOR_TRIAD, 1.0);
        let w = wide(60, MINOR_TRIAD, 2, 1.0);
        let close_events = close.events();
        let wide_events = w.events();
        assert_eq!(close_events.len(), wide_events.len());
        // Wide voicing should span more than close
        let close_range = close_events.iter().map(|e| e.value).fold(0.0f64, f64::max)
            - close_events.iter().map(|e| e.value).fold(f64::MAX, f64::min);
        let wide_range = wide_events.iter().map(|e| e.value).fold(0.0f64, f64::max)
            - wide_events.iter().map(|e| e.value).fold(f64::MAX, f64::min);
        assert!(wide_range >= close_range);
    }

    #[test]
    fn progression_correct_count() {
        let prog = progression(&[
            (58, MINOR_7),
            (61, MAJOR_7),
            (56, MINOR_7),
        ], 3.0);
        let events = prog.events();
        // 3 chords × 4 notes = 12 events
        assert_eq!(events.len(), 12);
        assert!((prog.duration() - 3.0).abs() < 1e-10);
    }

    #[test]
    fn progression_chords_sequential() {
        let prog = progression(&[
            (60, MINOR_TRIAD),
            (65, MINOR_TRIAD),
        ], 2.0);
        let events = prog.events();
        // First chord at t=0, second at t=1.0
        let first: Vec<_> = events.iter().filter(|e| e.start < 0.5).collect();
        let second: Vec<_> = events.iter().filter(|e| e.start >= 0.5).collect();
        assert_eq!(first.len(), 3);
        assert_eq!(second.len(), 3);
    }

    #[test]
    fn bloom_progression_produces_staggered_chords() {
        let prog = bloom_progression(
            &[(60, MINOR_TRIAD), (65, MINOR_TRIAD)],
            0.05,
            2.0,
        );
        let events = prog.events();
        assert_eq!(events.len(), 6);
        // First chord's notes should be staggered
        assert!((events[0].start).abs() < 1e-10);
        assert!((events[1].start - 0.05).abs() < 1e-10);
    }

    #[test]
    fn voice_led_stays_close() {
        let prog = voice_led(&[
            (48, MINOR_7),  // Cm7 low
            (65, MINOR_7),  // Fm7 high root — voice leading should bring it close
        ], 2.0);
        let events = prog.events();
        let all_pitches: Vec<f64> = events.iter().map(|e| e.value).collect();
        let min = all_pitches.iter().cloned().fold(f64::MAX, f64::min);
        let max = all_pitches.iter().cloned().fold(f64::MIN, f64::max);
        // Should span less than 2 octaves even though roots are far apart
        assert!(max - min < 24.0);
    }

    #[test]
    fn bloom_open_combines_both() {
        let p = bloom_open(60, MINOR_7, 0.05, 1.0);
        let events = p.events();
        assert_eq!(events.len(), 4);
        // Should be staggered
        assert!(events[1].start > events[0].start);
        // Should have dropped voices
        let pitches: Vec<f64> = events.iter().map(|e| e.value).collect();
        assert!(pitches.iter().any(|&p| p < 60.0)); // something dropped below root
    }

    #[test]
    fn empty_progression() {
        let p = progression(&[], 1.0);
        assert!(p.events().is_empty());
    }
}
