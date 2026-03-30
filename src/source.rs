/// Source — the atom of sonic material.
///
/// A Source doesn't know about time — it's material, not structure.
/// Sources get attached to events. The compiler maps them to Ableton
/// instruments (Simpler, synthesizers, audio inputs).

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Source
// ---------------------------------------------------------------------------

/// What produces sound.
#[derive(Debug, Clone)]
pub enum Source {
    /// A sample file on disk.
    Sample {
        path: String,
        label: Option<String>,
    },

    /// Live audio input (guitar, mic, line in).
    LiveInput {
        channel: usize, // 0-based input channel
        label: Option<String>,
    },

    /// A synthesizer with named parameters.
    Synth {
        preset: String,
        params: HashMap<String, f64>,
        label: Option<String>,
    },

    /// Resampled output from another patch.
    Resampled {
        patch_id: String,
        label: Option<String>,
    },
}

impl Source {
    /// Create a sample source.
    pub fn sample(path: &str) -> Self {
        Source::Sample {
            path: path.to_string(),
            label: None,
        }
    }

    /// Create a sample source with a label.
    pub fn sample_labeled(path: &str, label: &str) -> Self {
        Source::Sample {
            path: path.to_string(),
            label: Some(label.to_string()),
        }
    }

    /// Create a live input source.
    pub fn live_input(channel: usize) -> Self {
        Source::LiveInput {
            channel,
            label: None,
        }
    }

    /// Create a synth source with a preset name.
    pub fn synth(preset: &str) -> Self {
        Source::Synth {
            preset: preset.to_string(),
            params: HashMap::new(),
            label: None,
        }
    }

    /// Create a synth source with parameters.
    pub fn synth_with_params(preset: &str, params: HashMap<String, f64>) -> Self {
        Source::Synth {
            preset: preset.to_string(),
            params,
            label: None,
        }
    }

    /// Create a resampled source.
    pub fn resampled(patch_id: &str) -> Self {
        Source::Resampled {
            patch_id: patch_id.to_string(),
            label: None,
        }
    }

    /// Get the label, or a generated default.
    pub fn label(&self) -> &str {
        match self {
            Source::Sample { label, path, .. } => {
                label.as_deref().unwrap_or(path.as_str())
            }
            Source::LiveInput { label, .. } => {
                label.as_deref().unwrap_or("live-in")
            }
            Source::Synth { label, preset, .. } => {
                label.as_deref().unwrap_or(preset.as_str())
            }
            Source::Resampled { label, patch_id, .. } => {
                label.as_deref().unwrap_or(patch_id.as_str())
            }
        }
    }

    /// Number of audio output channels this source produces.
    pub fn output_channels(&self) -> usize {
        match self {
            Source::Sample { .. } => 2, // stereo by default
            Source::LiveInput { .. } => 1, // mono input
            Source::Synth { .. } => 2,
            Source::Resampled { .. } => 2,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_source() {
        let s = Source::sample("kick.wav");
        assert_eq!(s.label(), "kick.wav");
        assert_eq!(s.output_channels(), 2);
    }

    #[test]
    fn sample_with_label() {
        let s = Source::sample_labeled("samples/kick_909.wav", "909 Kick");
        assert_eq!(s.label(), "909 Kick");
    }

    #[test]
    fn live_input_source() {
        let s = Source::live_input(0);
        assert_eq!(s.output_channels(), 1);
        match &s {
            Source::LiveInput { channel, .. } => assert_eq!(*channel, 0),
            _ => panic!("expected LiveInput"),
        }
    }

    #[test]
    fn synth_source() {
        let s = Source::synth("Analog");
        assert_eq!(s.label(), "Analog");
        assert_eq!(s.output_channels(), 2);
    }

    #[test]
    fn synth_with_params() {
        let mut params = HashMap::new();
        params.insert("cutoff".to_string(), 0.7);
        params.insert("resonance".to_string(), 0.3);
        let s = Source::synth_with_params("Wavetable", params);
        match &s {
            Source::Synth { params, .. } => {
                assert_eq!(params.len(), 2);
                assert_eq!(params["cutoff"], 0.7);
            }
            _ => panic!("expected Synth"),
        }
    }

    #[test]
    fn resampled_source() {
        let s = Source::resampled("patch-001");
        assert_eq!(s.label(), "patch-001");
    }
}
