/// The core Pattern type — a recursive time structure carrying f64 values.
///
/// Patterns are the discrete, combinatorial layer of metaritual.
/// Time is cycle-relative (0.0 to 1.0). Values are untyped f64s —
/// pitch, velocity, cutoff, whatever the context demands.

use std::fmt;
use std::ops::{Add, BitOr, Mul};

/// A single event in a pattern.
#[derive(Debug, Clone)]
pub struct Event {
    pub value: f64,
    pub start: f64,
    pub duration: f64,
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        const EPS: f64 = 1e-9;
        (self.value - other.value).abs() < EPS
            && (self.start - other.start).abs() < EPS
            && (self.duration - other.duration).abs() < EPS
    }
}

/// The recursive pattern algebra.
#[derive(Debug, Clone)]
pub enum Pattern {
    /// Silence / zero-duration rest.
    Empty,
    /// A single event: (value, duration). NaN value = silent rest.
    Atom(f64, f64),
    /// Concatenation in time.
    Seq(Vec<Pattern>),
    /// Superposition (simultaneous).
    Stack(Vec<Pattern>),
}

// ---------------------------------------------------------------------------
// Core methods
// ---------------------------------------------------------------------------

impl Pattern {
    /// Construct a single-event atom.
    pub fn atom(value: f64, duration: f64) -> Self {
        Pattern::Atom(value, duration)
    }

    /// Zero-duration empty pattern.
    pub fn empty() -> Self {
        Pattern::Empty
    }

    /// A silent rest of `duration` cycle-time (no event emitted).
    pub fn silence(duration: f64) -> Self {
        Pattern::Atom(f64::NAN, duration)
    }

    /// Total duration in cycles.
    pub fn duration(&self) -> f64 {
        match self {
            Pattern::Empty => 0.0,
            Pattern::Atom(_, d) => *d,
            Pattern::Seq(ps) => ps.iter().map(|p| p.duration()).sum(),
            Pattern::Stack(ps) => ps.iter().map(|p| p.duration()).fold(0.0, f64::max),
        }
    }

    /// Flatten this pattern into a sorted list of events.
    ///
    /// - `Empty`  → nothing
    /// - `Atom(NaN, _)` → nothing (silent rest)
    /// - `Atom(v, d)` → single event at start=0.0
    /// - `Seq`  → events offset by cumulative duration of preceding children
    /// - `Stack` → all children merged without time offset (they carry their own starts)
    pub fn events(&self) -> Vec<Event> {
        let mut evs = self.events_offset(0.0);
        evs.sort_by(|a, b| {
            a.start
                .partial_cmp(&b.start)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    a.value
                        .partial_cmp(&b.value)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });
        evs
    }

    /// Internal helper: produce events with an added time offset.
    fn events_offset(&self, offset: f64) -> Vec<Event> {
        match self {
            Pattern::Empty => vec![],
            Pattern::Atom(v, d) => {
                if v.is_nan() {
                    vec![]
                } else {
                    vec![Event {
                        value: *v,
                        start: offset,
                        duration: *d,
                    }]
                }
            }
            Pattern::Seq(ps) => {
                let mut result = Vec::new();
                let mut cursor = offset;
                for p in ps {
                    result.extend(p.events_offset(cursor));
                    cursor += p.duration();
                }
                result
            }
            Pattern::Stack(ps) => {
                let mut result = Vec::new();
                for p in ps {
                    result.extend(p.events_offset(offset));
                }
                result
            }
        }
    }

    /// True if this pattern produces no events.
    pub fn is_empty(&self) -> bool {
        self.events().is_empty()
    }

    /// Number of events this pattern produces.
    pub fn len(&self) -> usize {
        self.events().len()
    }

    // -----------------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------------

    /// Build a Seq of equal-duration atoms filling one cycle.
    ///
    /// `seq(&[60.0, 64.0, 67.0])` → three atoms each with duration 1/3.
    pub fn seq(values: &[f64]) -> Pattern {
        if values.is_empty() {
            return Pattern::Empty;
        }
        let d = 1.0 / values.len() as f64;
        Pattern::Seq(values.iter().map(|&v| Pattern::Atom(v, d)).collect())
    }

    /// Build a Seq where every atom has the given explicit duration.
    pub fn seq_with_duration(values: &[f64], duration: f64) -> Pattern {
        if values.is_empty() {
            return Pattern::Empty;
        }
        Pattern::Seq(
            values
                .iter()
                .map(|&v| Pattern::Atom(v, duration))
                .collect(),
        )
    }
}

// ---------------------------------------------------------------------------
// PartialEq — compare by event lists (with epsilon)
// ---------------------------------------------------------------------------

impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.events() == other.events()
    }
}

// ---------------------------------------------------------------------------
// Operator overloads
// ---------------------------------------------------------------------------

/// Concatenation: `a + b` → Seq, with flat flattening of nested Seqs.
impl Add for Pattern {
    type Output = Pattern;

    fn add(self, rhs: Pattern) -> Pattern {
        let mut parts: Vec<Pattern> = Vec::new();

        // Flatten left operand
        match self {
            Pattern::Seq(ps) => parts.extend(ps),
            Pattern::Empty => {}
            other => parts.push(other),
        }

        // Flatten right operand
        match rhs {
            Pattern::Seq(ps) => parts.extend(ps),
            Pattern::Empty => {}
            other => parts.push(other),
        }

        if parts.is_empty() {
            Pattern::Empty
        } else {
            Pattern::Seq(parts)
        }
    }
}

/// Superposition: `a | b` → Stack, with flat flattening of nested Stacks.
impl BitOr for Pattern {
    type Output = Pattern;

    fn bitor(self, rhs: Pattern) -> Pattern {
        let mut parts: Vec<Pattern> = Vec::new();

        match self {
            Pattern::Stack(ps) => parts.extend(ps),
            Pattern::Empty => {}
            other => parts.push(other),
        }

        match rhs {
            Pattern::Stack(ps) => parts.extend(ps),
            Pattern::Empty => {}
            other => parts.push(other),
        }

        if parts.is_empty() {
            Pattern::Empty
        } else {
            Pattern::Stack(parts)
        }
    }
}

/// Repetition: `p * n` → Seq([p, p, ..., p]) n times.
impl Mul<usize> for Pattern {
    type Output = Pattern;

    fn mul(self, n: usize) -> Pattern {
        if n == 0 {
            return Pattern::Empty;
        }
        let parts: Vec<Pattern> = (0..n).map(|_| self.clone()).collect();
        Pattern::Seq(parts)
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Pattern::Empty => write!(f, "_"),
            Pattern::Atom(v, _) => {
                if v.is_nan() {
                    write!(f, "~")
                } else if v.fract() == 0.0 && v.abs() < 1e12 {
                    write!(f, "{}", *v as i64)
                } else {
                    write!(f, "{v}")
                }
            }
            Pattern::Seq(ps) => {
                write!(f, "[")?;
                for (i, p) in ps.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, "]")
            }
            Pattern::Stack(ps) => {
                write!(f, "{{")?;
                for (i, p) in ps.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, "}}")
            }
        }
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

    // --- Constructors -------------------------------------------------------

    #[test]
    fn atom_duration() {
        let p = Pattern::atom(60.0, 0.5);
        assert!(approx(p.duration(), 0.5));
    }

    #[test]
    fn empty_duration_is_zero() {
        assert!(approx(Pattern::empty().duration(), 0.0));
    }

    #[test]
    fn silence_duration() {
        let p = Pattern::silence(0.25);
        assert!(approx(p.duration(), 0.25));
        assert!(p.is_empty(), "silence should produce no events");
    }

    #[test]
    fn seq_equal_durations() {
        let p = Pattern::seq(&[60.0, 64.0, 67.0]);
        assert!(approx(p.duration(), 1.0));
        let evs = p.events();
        assert_eq!(evs.len(), 3);
        assert!(approx(evs[0].start, 0.0));
        assert!(approx(evs[1].start, 1.0 / 3.0));
        assert!(approx(evs[2].start, 2.0 / 3.0));
        for e in &evs {
            assert!(approx(e.duration, 1.0 / 3.0));
        }
    }

    #[test]
    fn seq_with_duration_custom() {
        let p = Pattern::seq_with_duration(&[1.0, 2.0], 0.5);
        assert!(approx(p.duration(), 1.0));
        let evs = p.events();
        assert_eq!(evs.len(), 2);
        assert!(approx(evs[0].duration, 0.5));
        assert!(approx(evs[1].duration, 0.5));
    }

    // --- events() -----------------------------------------------------------

    #[test]
    fn atom_events() {
        let p = Pattern::atom(72.0, 0.25);
        let evs = p.events();
        assert_eq!(evs.len(), 1);
        assert!(approx(evs[0].value, 72.0));
        assert!(approx(evs[0].start, 0.0));
        assert!(approx(evs[0].duration, 0.25));
    }

    #[test]
    fn empty_events() {
        assert!(Pattern::empty().events().is_empty());
    }

    #[test]
    fn seq_events_offsets() {
        let a = Pattern::atom(60.0, 0.5);
        let b = Pattern::atom(64.0, 0.5);
        let p = Pattern::Seq(vec![a, b]);
        let evs = p.events();
        assert_eq!(evs.len(), 2);
        assert!(approx(evs[0].start, 0.0));
        assert!(approx(evs[1].start, 0.5));
    }

    #[test]
    fn stack_events_same_start() {
        let a = Pattern::atom(60.0, 1.0);
        let b = Pattern::atom(64.0, 1.0);
        let p = Pattern::Stack(vec![a, b]);
        let evs = p.events();
        assert_eq!(evs.len(), 2);
        // Both start at 0.0
        for e in &evs {
            assert!(approx(e.start, 0.0));
        }
    }

    // --- Operators ----------------------------------------------------------

    #[test]
    fn add_flattens_seqs() {
        let a = Pattern::seq(&[60.0, 62.0]);
        let b = Pattern::seq(&[64.0, 67.0]);
        let c = a + b;
        match &c {
            Pattern::Seq(ps) => assert_eq!(ps.len(), 4, "should be flat Seq of 4 atoms"),
            other => panic!("expected Seq, got {:?}", other),
        }
    }

    #[test]
    fn add_associativity() {
        let a = Pattern::atom(60.0, 1.0);
        let b = Pattern::atom(64.0, 1.0);
        let c = Pattern::atom(67.0, 1.0);
        assert_eq!(
            (a.clone() + b.clone()) + c.clone(),
            a + (b + c),
            "concatenation should be associative by event equality"
        );
    }

    #[test]
    fn bitor_flattens_stacks() {
        let a = Pattern::Stack(vec![Pattern::atom(60.0, 1.0), Pattern::atom(62.0, 1.0)]);
        let b = Pattern::atom(64.0, 1.0);
        let c = a | b;
        match &c {
            Pattern::Stack(ps) => assert_eq!(ps.len(), 3, "should be flat Stack of 3"),
            other => panic!("expected Stack, got {:?}", other),
        }
    }

    #[test]
    fn mul_repeat() {
        let a = Pattern::atom(60.0, 0.5);
        let p = a * 3;
        assert!(approx(p.duration(), 1.5));
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn mul_zero() {
        let a = Pattern::atom(60.0, 1.0);
        let p = a * 0;
        assert!(p.is_empty());
    }

    // --- Display ------------------------------------------------------------

    #[test]
    fn display_empty() {
        assert_eq!(format!("{}", Pattern::Empty), "_");
    }

    #[test]
    fn display_atom_integer() {
        assert_eq!(format!("{}", Pattern::atom(60.0, 1.0)), "60");
    }

    #[test]
    fn display_silence() {
        assert_eq!(format!("{}", Pattern::silence(0.5)), "~");
    }

    #[test]
    fn display_seq() {
        let p = Pattern::seq(&[60.0, 64.0, 67.0]);
        assert_eq!(format!("{p}"), "[60 64 67]");
    }

    #[test]
    fn display_stack() {
        let a = Pattern::atom(60.0, 1.0);
        let b = Pattern::atom(64.0, 1.0);
        let p = a | b;
        assert_eq!(format!("{p}"), "{60, 64}");
    }

    #[test]
    fn display_nested() {
        let inner = Pattern::seq(&[60.0, 64.0]);
        let outer = Pattern::Seq(vec![inner, Pattern::atom(67.0, 0.5)]);
        assert_eq!(format!("{outer}"), "[[60 64] 67]");
    }

    // --- len / is_empty -----------------------------------------------------

    #[test]
    fn len_counts_events() {
        let p = Pattern::seq(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(p.len(), 4);
    }

    #[test]
    fn is_empty_true_for_empty() {
        assert!(Pattern::empty().is_empty());
    }

    #[test]
    fn is_empty_false_for_atom() {
        assert!(!Pattern::atom(60.0, 1.0).is_empty());
    }
}
