/// Corpus — a mutable database of analyzed audio grains.
///
/// The corpus is the material reservoir. Functions that modify it take
/// `&mut Corpus`; functions that read it take `&Corpus`. Rust's borrow
/// checker enforces the discipline.
///
/// This module defines the data structures and query operations.
/// Actual audio analysis (extracting features from WAV files) is
/// deferred to the Python compiler layer. The Rust side manages
/// the feature vectors and nearest-neighbor lookups.

use crate::pattern::Pattern;

// ---------------------------------------------------------------------------
// Feature vector
// ---------------------------------------------------------------------------

/// Fixed-dimension feature descriptor for a grain.
///
/// Features are normalized to 0.0..1.0 for consistent distance calculations.
/// Standard features: brightness, loudness, noisiness, pitch, duration.
#[derive(Debug, Clone, PartialEq)]
pub struct Features {
    values: Vec<f64>,
}

impl Features {
    /// Create a feature vector from raw values.
    pub fn new(values: Vec<f64>) -> Self {
        Self { values }
    }

    /// Number of dimensions.
    pub fn dim(&self) -> usize {
        self.values.len()
    }

    /// Euclidean distance to another feature vector.
    ///
    /// Panics if dimensions don't match.
    pub fn distance(&self, other: &Features) -> f64 {
        assert_eq!(
            self.values.len(),
            other.values.len(),
            "feature dimension mismatch: {} vs {}",
            self.values.len(),
            other.values.len()
        );
        self.values
            .iter()
            .zip(other.values.iter())
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f64>()
            .sqrt()
    }

    /// Get the value at a given dimension.
    pub fn get(&self, dim: usize) -> f64 {
        self.values[dim]
    }

    /// Raw values slice.
    pub fn as_slice(&self) -> &[f64] {
        &self.values
    }
}

// ---------------------------------------------------------------------------
// Grain
// ---------------------------------------------------------------------------

/// A grain — an analyzed audio fragment in the corpus.
#[derive(Debug, Clone)]
pub struct Grain {
    pub id: usize,
    pub source: String,     // file path or source identifier
    pub start: f64,         // start time in source (seconds)
    pub duration: f64,      // grain duration (seconds)
    pub features: Features, // extracted feature vector
}

// ---------------------------------------------------------------------------
// Corpus
// ---------------------------------------------------------------------------

/// A mutable database of analyzed grains.
///
/// The corpus is threaded through operations via `&mut` — Rust's
/// borrow checker ensures safe concurrent access patterns.
#[derive(Debug, Clone)]
pub struct Corpus {
    grains: Vec<Grain>,
    next_id: usize,
}

impl Corpus {
    /// Create an empty corpus.
    pub fn new() -> Self {
        Self {
            grains: Vec::new(),
            next_id: 0,
        }
    }

    /// Number of grains in the corpus.
    pub fn len(&self) -> usize {
        self.grains.len()
    }

    /// Whether the corpus is empty.
    pub fn is_empty(&self) -> bool {
        self.grains.is_empty()
    }

    /// Ingest a new grain into the corpus.
    /// Returns the assigned grain ID.
    pub fn ingest(&mut self, source: &str, start: f64, duration: f64, features: Features) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.grains.push(Grain {
            id,
            source: source.to_string(),
            start,
            duration,
            features,
        });
        id
    }

    /// Get a grain by ID.
    pub fn get(&self, id: usize) -> Option<&Grain> {
        self.grains.iter().find(|g| g.id == id)
    }

    /// Find the N nearest grains to a target feature vector.
    ///
    /// Returns grains sorted by distance (closest first).
    pub fn query(&self, target: &Features, n: usize) -> Vec<&Grain> {
        let mut scored: Vec<(&Grain, f64)> = self
            .grains
            .iter()
            .map(|g| (g, g.features.distance(target)))
            .collect();

        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        scored.into_iter().take(n).map(|(g, _)| g).collect()
    }

    /// Query the corpus with a sequence of target features, building
    /// a Pattern from the nearest grain for each target.
    ///
    /// Each matched grain becomes an Atom with its MIDI-range value
    /// (derived from the first feature dimension, scaled to 0..127).
    /// Duration is evenly distributed across the pattern.
    pub fn query_pattern(&self, targets: &[Features], _grain_duration: f64) -> Pattern {
        if targets.is_empty() || self.is_empty() {
            return Pattern::empty();
        }

        let dur_per = 1.0 / targets.len() as f64;
        Pattern::Seq(
            targets
                .iter()
                .map(|target| {
                    let nearest = self.query(target, 1);
                    if let Some(grain) = nearest.first() {
                        // Use the first feature as value, scaled to MIDI range
                        let value = grain.features.get(0) * 127.0;
                        Pattern::Atom(value, dur_per)
                    } else {
                        Pattern::Atom(f64::NAN, dur_per)
                    }
                })
                .collect(),
        )
    }

    /// Remove a grain by ID. Returns true if found and removed.
    pub fn remove(&mut self, id: usize) -> bool {
        let before = self.grains.len();
        self.grains.retain(|g| g.id != id);
        self.grains.len() < before
    }

    /// All grains, for iteration.
    pub fn grains(&self) -> &[Grain] {
        &self.grains
    }

    /// Remove all grains from a given source.
    pub fn remove_source(&mut self, source: &str) {
        self.grains.retain(|g| g.source != source);
    }

    /// Clear the entire corpus.
    pub fn clear(&mut self) {
        self.grains.clear();
    }
}

impl Default for Corpus {
    fn default() -> Self {
        Self::new()
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

    fn feat(vals: &[f64]) -> Features {
        Features::new(vals.to_vec())
    }

    // --- Features ------------------------------------------------------------

    #[test]
    fn feature_distance_identical_is_zero() {
        let a = feat(&[0.5, 0.5, 0.5]);
        assert!(approx(a.distance(&a), 0.0));
    }

    #[test]
    fn feature_distance_unit() {
        let a = feat(&[0.0]);
        let b = feat(&[1.0]);
        assert!(approx(a.distance(&b), 1.0));
    }

    #[test]
    fn feature_distance_2d() {
        let a = feat(&[0.0, 0.0]);
        let b = feat(&[3.0, 4.0]);
        assert!(approx(a.distance(&b), 5.0)); // 3-4-5 triangle
    }

    #[test]
    #[should_panic(expected = "dimension mismatch")]
    fn feature_distance_mismatch_panics() {
        let a = feat(&[1.0, 2.0]);
        let b = feat(&[1.0]);
        a.distance(&b);
    }

    #[test]
    fn feature_dim() {
        let f = feat(&[0.0, 0.5, 1.0]);
        assert_eq!(f.dim(), 3);
    }

    // --- Corpus --------------------------------------------------------------

    #[test]
    fn corpus_starts_empty() {
        let c = Corpus::new();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn corpus_ingest_and_get() {
        let mut c = Corpus::new();
        let id = c.ingest("test.wav", 0.0, 0.5, feat(&[0.5, 0.3]));
        assert_eq!(c.len(), 1);
        let grain = c.get(id).unwrap();
        assert_eq!(grain.source, "test.wav");
        assert!(approx(grain.start, 0.0));
        assert!(approx(grain.duration, 0.5));
    }

    #[test]
    fn corpus_assigns_unique_ids() {
        let mut c = Corpus::new();
        let id1 = c.ingest("a.wav", 0.0, 1.0, feat(&[0.0]));
        let id2 = c.ingest("b.wav", 0.0, 1.0, feat(&[1.0]));
        assert_ne!(id1, id2);
    }

    #[test]
    fn corpus_query_nearest() {
        let mut c = Corpus::new();
        c.ingest("low.wav", 0.0, 1.0, feat(&[0.1]));
        c.ingest("mid.wav", 0.0, 1.0, feat(&[0.5]));
        c.ingest("high.wav", 0.0, 1.0, feat(&[0.9]));

        let target = feat(&[0.45]);
        let results = c.query(&target, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "mid.wav"); // 0.5 is closest to 0.45
    }

    #[test]
    fn corpus_query_returns_sorted() {
        let mut c = Corpus::new();
        c.ingest("far.wav", 0.0, 1.0, feat(&[1.0]));
        c.ingest("close.wav", 0.0, 1.0, feat(&[0.1]));
        c.ingest("closer.wav", 0.0, 1.0, feat(&[0.05]));

        let target = feat(&[0.0]);
        let results = c.query(&target, 3);
        assert_eq!(results[0].source, "closer.wav");
        assert_eq!(results[1].source, "close.wav");
        assert_eq!(results[2].source, "far.wav");
    }

    #[test]
    fn corpus_query_n_larger_than_corpus() {
        let mut c = Corpus::new();
        c.ingest("only.wav", 0.0, 1.0, feat(&[0.5]));

        let results = c.query(&feat(&[0.0]), 10);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn corpus_remove() {
        let mut c = Corpus::new();
        let id = c.ingest("x.wav", 0.0, 1.0, feat(&[0.0]));
        assert_eq!(c.len(), 1);
        assert!(c.remove(id));
        assert_eq!(c.len(), 0);
        assert!(!c.remove(id)); // already removed
    }

    #[test]
    fn corpus_remove_source() {
        let mut c = Corpus::new();
        c.ingest("a.wav", 0.0, 1.0, feat(&[0.0]));
        c.ingest("a.wav", 1.0, 1.0, feat(&[0.5]));
        c.ingest("b.wav", 0.0, 1.0, feat(&[1.0]));
        assert_eq!(c.len(), 3);
        c.remove_source("a.wav");
        assert_eq!(c.len(), 1);
        assert_eq!(c.grains()[0].source, "b.wav");
    }

    #[test]
    fn corpus_clear() {
        let mut c = Corpus::new();
        c.ingest("x.wav", 0.0, 1.0, feat(&[0.0]));
        c.ingest("y.wav", 0.0, 1.0, feat(&[1.0]));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn corpus_query_pattern_builds_pattern() {
        let mut c = Corpus::new();
        // Grains with known first feature
        c.ingest("low.wav", 0.0, 0.1, feat(&[0.2, 0.0]));
        c.ingest("high.wav", 0.0, 0.1, feat(&[0.8, 0.0]));

        let targets = vec![feat(&[0.1, 0.0]), feat(&[0.9, 0.0])];
        let p = c.query_pattern(&targets, 0.1);
        let evs = p.events();
        assert_eq!(evs.len(), 2);
        // First should match "low" (0.2 * 127 ≈ 25.4)
        assert!(approx(evs[0].value, 0.2 * 127.0));
        // Second should match "high" (0.8 * 127 ≈ 101.6)
        assert!(approx(evs[1].value, 0.8 * 127.0));
    }

    #[test]
    fn corpus_query_pattern_empty_corpus() {
        let c = Corpus::new();
        let p = c.query_pattern(&[feat(&[0.5])], 0.1);
        assert!(p.is_empty());
    }

    #[test]
    fn corpus_query_pattern_empty_targets() {
        let mut c = Corpus::new();
        c.ingest("x.wav", 0.0, 1.0, feat(&[0.5]));
        let p = c.query_pattern(&[], 0.1);
        assert!(p.is_empty());
    }

    #[test]
    fn corpus_multidimensional_query() {
        let mut c = Corpus::new();
        // 3D features: [brightness, loudness, noisiness]
        c.ingest("bright_loud.wav", 0.0, 1.0, feat(&[0.9, 0.8, 0.1]));
        c.ingest("dark_quiet.wav", 0.0, 1.0, feat(&[0.1, 0.2, 0.1]));
        c.ingest("noisy.wav", 0.0, 1.0, feat(&[0.5, 0.5, 0.9]));

        // Target: bright and loud
        let target = feat(&[0.8, 0.9, 0.2]);
        let results = c.query(&target, 1);
        assert_eq!(results[0].source, "bright_loud.wav");

        // Target: noisy
        let target = feat(&[0.4, 0.4, 0.85]);
        let results = c.query(&target, 1);
        assert_eq!(results[0].source, "noisy.wav");
    }
}
