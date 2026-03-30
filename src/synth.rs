/// Synthesis presets — named recipes encoding parameter knowledge
/// for Ableton instruments.
///
/// Each function returns a `Source::Synth` with the right preset name
/// and parameters. Parameters use Ableton's internal names where reliable
/// (Operator) or index-as-string keys where names are fragile (Analog,
/// Collision, Wavetable).
///
/// These encode knowledge mined from hands-on parameter exploration.

use std::collections::HashMap;
use crate::source::Source;

fn src(preset: &str, label: &str, params: &[(&str, f64)]) -> Source {
    let p: HashMap<String, f64> = params.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    Source::Synth {
        preset: preset.to_string(),
        params: p,
        label: Some(label.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Bass
// ---------------------------------------------------------------------------

/// Liquid metal FM bass — Operator 4-op cascade with self-modulating feedback.
/// Bubbly, morphing, SAW II-era Aphex Twin quality.
pub fn liquid_metal() -> Source {
    src("Operator", "liquid_metal", &[
        ("Algorithm", 0.1),
        ("Osc-A Level", 1.0),
        ("Osc-A Wave", 0.0),
        ("A Coarse", 0.5),
        ("Osc-B On", 1.0),
        ("Osc-B Level", 0.55),
        ("Osc-B Wave", 0.0),
        ("B Coarse", 0.68),
        ("Osc-C On", 1.0),
        ("Osc-C Level", 0.4),
        ("Osc-C Wave", 0.0),
        ("C Coarse", 0.33),
        ("Osc-D On", 1.0),
        ("Osc-D Level", 0.5),
        ("Osc-D Wave", 0.0),
        ("D Coarse", 0.82),
        ("Osc-D Feedback", 0.5),
        ("Filter On", 1.0),
        ("Filter Freq", 0.45),
        ("Filter Res", 0.5),
        ("Ae Attack", 0.05),
        ("Ae Decay", 0.5),
        ("Ae Sustain", 0.45),
        ("Ae Release", 0.35),
        ("Pe Init", 0.58),
        ("Pe Peak", 0.53),
        ("Pe Time", 0.15),
    ])
}

/// Pure sine sub-bass — Analog, two octaves down, no filter.
pub fn sub_bass() -> Source {
    src("Analog", "sub_bass", &[
        ("38", 1.0),    // OSC1 On
        ("39", 0.0),    // Sine
        ("40", -2.0),   // Two octaves down
        ("105", 0.0),   // OSC2 Off
        ("34", 0.0),    // Noise Off
        ("53", 0.0),    // Filter Off
        ("89", 0.08),   // Attack gentle
        ("90", 0.7),    // Decay
        ("92", 0.8),    // Sustain high
        ("94", 0.5),    // Release
    ])
}

// ---------------------------------------------------------------------------
// Percussion
// ---------------------------------------------------------------------------

/// Synthesized kick — Analog sine with pitch envelope.
/// Strong pitch drop creates the thud.
pub fn synth_kick() -> Source {
    src("Analog", "synth_kick", &[
        ("38", 1.0),    // OSC1 On
        ("39", 0.0),    // Sine
        ("40", -1.0),   // Octave down
        ("49", 0.75),   // PEG Amount — strong pitch drop
        ("43", 0.12),   // PEG Time — fast
        ("105", 0.0),   // OSC2 Off
        ("34", 0.0),    // Noise Off
        ("53", 0.0),    // Filter Off
        ("89", 0.0),    // Attack instant
        ("90", 0.28),   // Decay
        ("92", 0.0),    // Sustain zero
        ("94", 0.12),   // Release short
    ])
}

/// Synthesized snare/perc — Analog noise + sine through resonant filter.
/// Filter resonance gives it a pitched quality.
pub fn synth_perc() -> Source {
    src("Analog", "synth_perc", &[
        ("38", 1.0),    // OSC1 On
        ("39", 0.0),    // Sine (for the ping)
        ("52", 0.3),    // OSC1 Level low
        ("34", 1.0),    // Noise On
        ("35", 0.55),   // Noise Color
        ("37", 0.65),   // Noise Level
        ("105", 0.0),   // OSC2 Off
        ("53", 1.0),    // Filter On
        ("54", 0.0),    // LP
        ("57", 0.5),    // Filter Freq
        ("59", 0.45),   // Resonance — pitched quality
        ("62", 0.7),    // Filter Env amount
        ("71", 0.12),   // FEG Decay short
        ("73", 0.0),    // FEG Sustain zero
        ("89", 0.0),    // Attack instant
        ("90", 0.18),   // Decay
        ("92", 0.0),    // Sustain zero
        ("94", 0.1),    // Release
    ])
}

// ---------------------------------------------------------------------------
// Lead
// ---------------------------------------------------------------------------

/// Crystalline FM lead — Operator with non-integer ratios for inharmonic sparkle.
/// Bell-like shimmer, good for high-register melodies.
pub fn crystalline() -> Source {
    src("Operator", "crystalline", &[
        ("Algorithm", 0.15),
        ("Osc-A Level", 0.85),
        ("Osc-A Wave", 0.0),
        ("Osc-B On", 1.0),
        ("Osc-B Level", 0.4),
        ("Osc-B Wave", 0.0),
        ("B Coarse", 0.75),     // non-integer = sparkle
        ("B Fine", 0.52),
        ("Osc-C On", 1.0),
        ("Osc-C Level", 0.25),
        ("Osc-C Wave", 0.0),
        ("C Coarse", 0.6),
        ("Filter On", 1.0),
        ("Filter Freq", 0.65),
        ("Filter Res", 0.25),
        ("Ae Attack", 0.08),
        ("Ae Decay", 0.55),
        ("Ae Sustain", 0.35),
        ("Ae Release", 0.5),
    ])
}

// ---------------------------------------------------------------------------
// Pad
// ---------------------------------------------------------------------------

/// Elastic glass pad — Collision physical modeling with two resonators.
/// Metallic, evolving timbre with beating from detuned resonators.
/// Inharmonics parameter makes it progressively more alien.
pub fn elastic_glass() -> Source {
    src("Collision", "elastic_glass", &[
        ("7", 1.0),     // Mallet On
        ("8", 0.5),     // Mallet Volume
        ("11", 0.6),    // Stiffness
        ("14", 0.2),    // Noise Amount
        ("32", 1.0),    // Res 1 On
        ("33", 2.0),    // Marimba
        ("34", 2.0),    // Quality max
        ("41", 0.9),    // Decay long
        ("45", 0.7),    // Material metallic
        ("52", 0.5),    // Brightness
        ("54", 0.45),   // Inharmonics
        ("56", 0.95),   // Opening
        ("64", 0.55),   // Volume
        ("65", 1.0),    // Res 2 On
        ("66", 1.0),    // Beam
        ("67", 2.0),    // Quality
        ("68", 5.0),    // Tune
        ("69", 0.2),    // Fine Tune — beating
        ("74", 0.85),   // Res 2 Decay
        ("78", 0.8),    // Res 2 Material
        ("87", 0.5),    // Res 2 Inharmonics
        ("97", 0.4),    // Res 2 Volume
        ("1", 1.0),     // Structure = parallel
    ])
}

// ---------------------------------------------------------------------------
// Texture
// ---------------------------------------------------------------------------

/// Subatomic texture — Wavetable with ultra-short envelope.
/// Designed for micro-notes (0.05 beat duration) that blur into timbre.
/// Pair with Spectral Time for full effect.
pub fn subatomic() -> Source {
    src("Wavetable", "subatomic", &[
        ("4", 0.5),     // Osc 1 Pos
        ("5", 0.6),     // Effect 1
        ("9", 1.0),     // Osc 2 On
        ("12", 0.3),    // Osc 2 Pos
        ("16", 0.5),    // Osc 2 Gain
        ("26", 0.6),    // Filter Freq
        ("27", 0.2),    // Filter Res
        ("39", 0.0),    // Attack instant
        ("40", 0.1),    // Decay very short
        ("45", 0.03),   // Sustain near zero
        ("41", 0.05),   // Release tiny
        ("89", 0.15),   // Slight unison
    ])
}

// ---------------------------------------------------------------------------
// Experimental
// ---------------------------------------------------------------------------

/// FM bubble — Operator with high feedback for SOPHIE-style liquid sounds.
/// More extreme than liquid_metal: higher feedback, wider FM depth.
pub fn fm_bubble() -> Source {
    src("Operator", "fm_bubble", &[
        ("Algorithm", 0.1),
        ("Osc-A Level", 1.0),
        ("Osc-A Wave", 0.0),
        ("Osc-B On", 1.0),
        ("Osc-B Level", 0.75),
        ("Osc-B Wave", 0.0),
        ("B Coarse", 0.6),
        ("Osc-C On", 1.0),
        ("Osc-C Level", 0.6),
        ("Osc-C Wave", 0.0),
        ("C Coarse", 0.45),
        ("Osc-D On", 1.0),
        ("Osc-D Level", 0.7),
        ("Osc-D Wave", 0.0),
        ("D Coarse", 0.5),
        ("Osc-D Feedback", 0.85),
        ("Filter On", 1.0),
        ("Filter Freq", 0.35),
        ("Filter Res", 0.6),
        ("Ae Attack", 0.0),
        ("Ae Decay", 0.4),
        ("Ae Sustain", 0.3),
        ("Ae Release", 0.25),
    ])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liquid_metal_is_operator() {
        match liquid_metal() {
            Source::Synth { preset, params, label, .. } => {
                assert_eq!(preset, "Operator");
                assert_eq!(label, Some("liquid_metal".to_string()));
                assert!(params.contains_key("Osc-D Feedback"));
                assert!(params.len() > 20);
            }
            _ => panic!("expected Synth"),
        }
    }

    #[test]
    fn synth_kick_is_analog() {
        match synth_kick() {
            Source::Synth { preset, params, .. } => {
                assert_eq!(preset, "Analog");
                // Uses index-based keys for Analog
                assert!(params.contains_key("49")); // PEG Amount
                assert!(params.contains_key("43")); // PEG Time
            }
            _ => panic!("expected Synth"),
        }
    }

    #[test]
    fn elastic_glass_is_collision() {
        match elastic_glass() {
            Source::Synth { preset, params, .. } => {
                assert_eq!(preset, "Collision");
                assert!(params.contains_key("54")); // Inharmonics
                assert!(params.contains_key("1"));  // Structure
            }
            _ => panic!("expected Synth"),
        }
    }

    #[test]
    fn all_presets_have_params() {
        let presets: Vec<Source> = vec![
            liquid_metal(), sub_bass(), synth_kick(), synth_perc(),
            crystalline(), elastic_glass(), subatomic(), fm_bubble(),
        ];
        for s in presets {
            match s {
                Source::Synth { params, .. } => assert!(!params.is_empty()),
                _ => panic!("expected Synth"),
            }
        }
    }
}
