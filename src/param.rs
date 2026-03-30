/// Named, bounded parameters — the atoms of the control surface.
///
/// A Param is a named dimension with bounds and a driving Signal.
/// It's what knobs bind to, what automation drives, what Claude
/// can read and suggest changes to.

use crate::signal::Signal;

// ---------------------------------------------------------------------------
// Param
// ---------------------------------------------------------------------------

/// A named, bounded, semantic dimension.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub range: (f64, f64),
    pub default: f64,
    pub signal: Signal,
}

impl Param {
    /// Create a new param at its default value (driven by Const).
    pub fn new(name: &str, range: (f64, f64), default: f64) -> Self {
        Self {
            name: name.to_string(),
            range,
            default,
            signal: Signal::Const(default),
        }
    }

    /// Drive this param with a new signal.
    pub fn set(&mut self, signal: Signal) {
        self.signal = signal;
    }

    /// Reset to default (Const).
    pub fn reset(&mut self) {
        self.signal = Signal::Const(self.default);
    }

    /// Sample the param at a point in time, clamped to range.
    pub fn sample(&self, time: f64) -> f64 {
        let raw = self.signal.sample(time);
        raw.clamp(self.range.0, self.range.1)
    }

    /// Sample at N evenly-spaced points, clamped.
    pub fn sample_n(&self, start: f64, end: f64, n: usize) -> Vec<f64> {
        self.signal
            .sample_n(start, end, n)
            .into_iter()
            .map(|v| v.clamp(self.range.0, self.range.1))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ParamSet
// ---------------------------------------------------------------------------

/// A collection of named params — the control surface of a patch.
#[derive(Debug, Clone)]
pub struct ParamSet {
    params: Vec<Param>,
}

impl ParamSet {
    pub fn new() -> Self {
        Self { params: vec![] }
    }

    /// Add a param to the set.
    pub fn add(&mut self, param: Param) {
        self.params.push(param);
    }

    /// Get a param by name.
    pub fn get(&self, name: &str) -> Option<&Param> {
        self.params.iter().find(|p| p.name == name)
    }

    /// Get a mutable param by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Param> {
        self.params.iter_mut().find(|p| p.name == name)
    }

    /// Number of params.
    pub fn len(&self) -> usize {
        self.params.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }

    /// Iterate over all params.
    pub fn iter(&self) -> impl Iterator<Item = &Param> {
        self.params.iter()
    }

    /// List all param names.
    pub fn names(&self) -> Vec<&str> {
        self.params.iter().map(|p| p.name.as_str()).collect()
    }
}

impl Default for ParamSet {
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
    use crate::signal::WaveShape;

    const EPS: f64 = 1e-9;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    #[test]
    fn param_starts_at_default() {
        let p = Param::new("brightness", (0.0, 1.0), 0.5);
        assert!(approx(p.sample(0.0), 0.5));
        assert!(approx(p.sample(100.0), 0.5));
    }

    #[test]
    fn param_clamps_to_range() {
        let mut p = Param::new("cutoff", (0.0, 1.0), 0.5);
        p.set(Signal::Const(2.0));
        assert!(approx(p.sample(0.0), 1.0)); // clamped to max
        p.set(Signal::Const(-1.0));
        assert!(approx(p.sample(0.0), 0.0)); // clamped to min
    }

    #[test]
    fn param_driven_by_lfo() {
        let mut p = Param::new("filter", (0.0, 1.0), 0.5);
        // LFO centered at 0.5 with depth 0.3: range 0.2..0.8
        p.set(Signal::lfo(WaveShape::Sine, 1.0, 0.0, 0.5, 0.3));
        // At t=0.25 (sine peak): 0.5 + 0.3 = 0.8
        assert!(approx(p.sample(0.25), 0.8));
        // At t=0.75 (sine trough): 0.5 - 0.3 = 0.2
        assert!(approx(p.sample(0.75), 0.2));
    }

    #[test]
    fn param_lfo_clamped_when_exceeds_range() {
        let mut p = Param::new("gain", (0.0, 1.0), 0.5);
        // LFO with depth that exceeds range
        p.set(Signal::lfo(WaveShape::Sine, 1.0, 0.0, 0.5, 1.0));
        // At peak: 0.5 + 1.0 = 1.5 → clamped to 1.0
        assert!(approx(p.sample(0.25), 1.0));
        // At trough: 0.5 - 1.0 = -0.5 → clamped to 0.0
        assert!(approx(p.sample(0.75), 0.0));
    }

    #[test]
    fn param_reset_returns_to_default() {
        let mut p = Param::new("x", (0.0, 10.0), 5.0);
        p.set(Signal::Const(8.0));
        assert!(approx(p.sample(0.0), 8.0));
        p.reset();
        assert!(approx(p.sample(0.0), 5.0));
    }

    #[test]
    fn param_sample_n() {
        let p = Param::new("x", (0.0, 100.0), 50.0);
        let samples = p.sample_n(0.0, 1.0, 5);
        assert_eq!(samples.len(), 5);
        assert!(samples.iter().all(|&v| approx(v, 50.0)));
    }

    // --- ParamSet ------------------------------------------------------------

    #[test]
    fn paramset_add_and_get() {
        let mut ps = ParamSet::new();
        ps.add(Param::new("a", (0.0, 1.0), 0.5));
        ps.add(Param::new("b", (0.0, 10.0), 5.0));
        assert_eq!(ps.len(), 2);
        assert!(ps.get("a").is_some());
        assert!(ps.get("b").is_some());
        assert!(ps.get("c").is_none());
    }

    #[test]
    fn paramset_get_mut() {
        let mut ps = ParamSet::new();
        ps.add(Param::new("x", (0.0, 1.0), 0.5));
        ps.get_mut("x").unwrap().set(Signal::Const(0.9));
        assert!(approx(ps.get("x").unwrap().sample(0.0), 0.9));
    }

    #[test]
    fn paramset_names() {
        let mut ps = ParamSet::new();
        ps.add(Param::new("density", (0.0, 1.0), 0.3));
        ps.add(Param::new("brightness", (0.0, 1.0), 0.6));
        let names = ps.names();
        assert!(names.contains(&"density"));
        assert!(names.contains(&"brightness"));
    }

    #[test]
    fn paramset_empty() {
        let ps = ParamSet::new();
        assert!(ps.is_empty());
        assert_eq!(ps.len(), 0);
    }
}
