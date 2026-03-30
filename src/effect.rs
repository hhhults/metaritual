/// Effect — audio processing stages in the signal chain.
///
/// Effects are descriptive, not functional. They describe what the
/// compiler should instantiate in Ableton (reverb, delay, EQ, etc.).
/// Parameters are named and bounded, ready to be driven by Signals.

use crate::param::Param;

// ---------------------------------------------------------------------------
// Effect
// ---------------------------------------------------------------------------

/// A single audio effect with named parameters.
#[derive(Debug, Clone)]
pub struct Effect {
    pub name: String,
    pub params: Vec<Param>,
}

impl Effect {
    /// Create an effect with the given name and no params.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            params: Vec::new(),
        }
    }

    /// Add a parameter to this effect.
    pub fn param(mut self, p: Param) -> Self {
        self.params.push(p);
        self
    }

    /// Get a parameter by name.
    pub fn get_param(&self, name: &str) -> Option<&Param> {
        self.params.iter().find(|p| p.name == name)
    }

    /// Get a mutable parameter by name.
    pub fn get_param_mut(&mut self, name: &str) -> Option<&mut Param> {
        self.params.iter_mut().find(|p| p.name == name)
    }
}

// ---------------------------------------------------------------------------
// Common effect constructors
// ---------------------------------------------------------------------------

/// Reverb with decay and mix controls.
pub fn reverb(decay: f64, mix: f64) -> Effect {
    Effect::new("Reverb")
        .param(Param::new("decay", (0.0, 60.0), decay))
        .param(Param::new("mix", (0.0, 1.0), mix))
        .param(Param::new("size", (0.0, 1.0), 0.5))
        .param(Param::new("damping", (0.0, 1.0), 0.5))
}

/// Delay with time (in beats), feedback, and mix.
pub fn delay(time: f64, feedback: f64, mix: f64) -> Effect {
    Effect::new("Delay")
        .param(Param::new("time", (0.0, 16.0), time))
        .param(Param::new("feedback", (0.0, 1.0), feedback))
        .param(Param::new("mix", (0.0, 1.0), mix))
}

/// Low-pass filter with cutoff and resonance.
pub fn lowpass(cutoff: f64, resonance: f64) -> Effect {
    Effect::new("LowPass")
        .param(Param::new("cutoff", (20.0, 20000.0), cutoff))
        .param(Param::new("resonance", (0.0, 1.0), resonance))
}

/// High-pass filter with cutoff and resonance.
pub fn highpass(cutoff: f64, resonance: f64) -> Effect {
    Effect::new("HighPass")
        .param(Param::new("cutoff", (20.0, 20000.0), cutoff))
        .param(Param::new("resonance", (0.0, 1.0), resonance))
}

/// Compressor with threshold, ratio, attack, release.
pub fn compressor(threshold: f64, ratio: f64) -> Effect {
    Effect::new("Compressor")
        .param(Param::new("threshold", (-60.0, 0.0), threshold))
        .param(Param::new("ratio", (1.0, 20.0), ratio))
        .param(Param::new("attack", (0.01, 100.0), 10.0))
        .param(Param::new("release", (1.0, 1000.0), 100.0))
}

/// 3-band EQ with low, mid, high gain.
pub fn eq3(low: f64, mid: f64, high: f64) -> Effect {
    Effect::new("EQ3")
        .param(Param::new("low", (-15.0, 15.0), low))
        .param(Param::new("mid", (-15.0, 15.0), mid))
        .param(Param::new("high", (-15.0, 15.0), high))
}

/// Saturator/distortion with drive and mix.
pub fn saturator(drive: f64, mix: f64) -> Effect {
    Effect::new("Saturator")
        .param(Param::new("drive", (0.0, 1.0), drive))
        .param(Param::new("mix", (0.0, 1.0), mix))
}

// ---------------------------------------------------------------------------
// EffectChain
// ---------------------------------------------------------------------------

/// An ordered sequence of effects (series processing).
#[derive(Debug, Clone)]
pub struct EffectChain {
    effects: Vec<Effect>,
}

impl EffectChain {
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
        }
    }

    /// Append an effect to the chain.
    pub fn add(mut self, effect: Effect) -> Self {
        self.effects.push(effect);
        self
    }

    /// Number of effects in the chain.
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Get an effect by index.
    pub fn get(&self, index: usize) -> Option<&Effect> {
        self.effects.get(index)
    }

    /// Get a mutable effect by index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Effect> {
        self.effects.get_mut(index)
    }

    /// Iterate over effects in order.
    pub fn iter(&self) -> impl Iterator<Item = &Effect> {
        self.effects.iter()
    }

    /// Get an effect by name (first match).
    pub fn by_name(&self, name: &str) -> Option<&Effect> {
        self.effects.iter().find(|e| e.name == name)
    }
}

impl Default for EffectChain {
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

    #[test]
    fn reverb_has_params() {
        let r = reverb(4.0, 0.5);
        assert_eq!(r.name, "Reverb");
        assert_eq!(r.params.len(), 4);
        assert!(approx(r.get_param("decay").unwrap().sample(0.0), 4.0));
        assert!(approx(r.get_param("mix").unwrap().sample(0.0), 0.5));
    }

    #[test]
    fn delay_has_params() {
        let d = delay(0.375, 0.4, 0.3);
        assert_eq!(d.name, "Delay");
        assert!(approx(d.get_param("time").unwrap().sample(0.0), 0.375));
        assert!(approx(d.get_param("feedback").unwrap().sample(0.0), 0.4));
    }

    #[test]
    fn lowpass_cutoff() {
        let f = lowpass(1000.0, 0.5);
        assert!(approx(f.get_param("cutoff").unwrap().sample(0.0), 1000.0));
    }

    #[test]
    fn compressor_params() {
        let c = compressor(-20.0, 4.0);
        assert!(approx(c.get_param("threshold").unwrap().sample(0.0), -20.0));
        assert!(approx(c.get_param("ratio").unwrap().sample(0.0), 4.0));
    }

    #[test]
    fn eq3_params() {
        let e = eq3(3.0, 0.0, -2.0);
        assert!(approx(e.get_param("low").unwrap().sample(0.0), 3.0));
        assert!(approx(e.get_param("mid").unwrap().sample(0.0), 0.0));
        assert!(approx(e.get_param("high").unwrap().sample(0.0), -2.0));
    }

    #[test]
    fn saturator_params() {
        let s = saturator(0.7, 1.0);
        assert!(approx(s.get_param("drive").unwrap().sample(0.0), 0.7));
    }

    #[test]
    fn effect_builder() {
        let e = Effect::new("Custom")
            .param(Param::new("amount", (0.0, 1.0), 0.5))
            .param(Param::new("tone", (0.0, 1.0), 0.3));
        assert_eq!(e.params.len(), 2);
    }

    #[test]
    fn effect_get_param_mut() {
        let mut e = reverb(4.0, 0.5);
        e.get_param_mut("mix")
            .unwrap()
            .set(crate::signal::Signal::Const(0.8));
        assert!(approx(e.get_param("mix").unwrap().sample(0.0), 0.8));
    }

    // --- EffectChain ---------------------------------------------------------

    #[test]
    fn chain_builds_in_order() {
        let chain = EffectChain::new()
            .add(lowpass(5000.0, 0.3))
            .add(reverb(3.0, 0.4))
            .add(compressor(-10.0, 3.0));
        assert_eq!(chain.len(), 3);
        assert_eq!(chain.get(0).unwrap().name, "LowPass");
        assert_eq!(chain.get(1).unwrap().name, "Reverb");
        assert_eq!(chain.get(2).unwrap().name, "Compressor");
    }

    #[test]
    fn chain_by_name() {
        let chain = EffectChain::new()
            .add(delay(0.5, 0.3, 0.5))
            .add(reverb(2.0, 0.6));
        assert!(chain.by_name("Reverb").is_some());
        assert!(chain.by_name("Chorus").is_none());
    }

    #[test]
    fn chain_empty() {
        let chain = EffectChain::new();
        assert!(chain.is_empty());
    }
}
