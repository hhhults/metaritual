/// Intermediate representation — the boundary between Rust and Python.
///
/// The IR is a JSON-serializable description of what to instantiate in Ableton.
/// The Rust DSL compiles Pattern → IR. The Python compiler reads IR → ableton.py calls.

use serde::{Deserialize, Serialize};

use crate::effect::EffectChain;
use crate::patch::{Node, Patch};
use crate::pattern::Pattern;
use crate::signal::Signal;

// ---------------------------------------------------------------------------
// IR types
// ---------------------------------------------------------------------------

/// A single note/event in the IR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IrEvent {
    pub value: f64,
    pub start: f64,    // in beats
    pub duration: f64, // in beats
}

/// The compiled output of a pattern — one clip's worth of events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IrClip {
    pub events: Vec<IrEvent>,
    pub length: f64, // total length in beats
    pub bpm: f64,
}

/// A named track containing clips.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IrTrack {
    pub name: String,
    pub clips: Vec<IrClip>,
}

/// Top-level IR — a session containing tracks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IrSession {
    pub bpm: f64,
    pub tracks: Vec<IrTrack>,
}

// ---------------------------------------------------------------------------
// Patch-level IR types
// ---------------------------------------------------------------------------

/// A time-value breakpoint for automation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IrBreakpoint {
    pub time: f64,
    pub value: f64,
}

/// A parameter automation curve (sampled from a Signal).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IrAutomation {
    pub param_name: String,
    pub breakpoints: Vec<IrBreakpoint>,
}

/// A static parameter value with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IrParam {
    pub name: String,
    pub value: f64,
    pub range: (f64, f64),
}

/// A sound source in the IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IrSource {
    Sample { path: String, label: String },
    Synth { preset: String, params: Vec<(String, f64)>, label: String },
    LiveInput { channel: usize, label: String },
    Resampled { patch_id: String, label: String },
}

/// An effect in the IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IrEffect {
    pub name: String,
    pub params: Vec<IrParam>,
    pub automation: Vec<IrAutomation>,
}

/// A node in the compiled patch graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IrNode {
    Source { id: usize, source: IrSource, label: String },
    Pattern { id: usize, clip: IrClip, label: String },
    Effect { id: usize, effect: IrEffect, label: String },
    Chain { id: usize, effects: Vec<IrEffect>, label: String },
    Mixer { id: usize, channels: usize, label: String },
    Split { id: usize, outputs: usize, label: String },
    Merge { id: usize, inputs: usize, label: String },
}

/// An edge in the compiled patch graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IrEdge {
    pub from_node: usize,
    pub from_port: usize,
    pub to_node: usize,
    pub to_port: usize,
}

/// Spatial configuration in the IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IrSpace {
    pub pan: IrAutomation,
    pub width: IrAutomation,
    pub depth: IrAutomation,
}

/// An exposed parameter on the control surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IrExposedParam {
    pub name: String,
    pub range: (f64, f64),
    pub default: f64,
    pub automation: IrAutomation,
}

/// A compiled Patch — the full routing diagram as IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IrPatch {
    pub label: String,
    pub bpm: f64,
    pub nodes: Vec<IrNode>,
    pub edges: Vec<IrEdge>,
    pub space: IrSpace,
    pub exposed_params: Vec<IrExposedParam>,
}

// ---------------------------------------------------------------------------
// Compilation: Pattern → IR
// ---------------------------------------------------------------------------

/// Compile a Pattern into IR.
///
/// - `cycles`: how many cycles to evaluate (the pattern repeats).
/// - `bpm`: tempo for metadata (doesn't affect beat calculation directly).
/// - `beats_per_cycle`: how many beats in one cycle (4.0 for 4/4 time).
///
/// Cycle-relative times are converted to beats: `beat = cycle_time * beats_per_cycle`.
/// NaN-valued events (rests/silences) are filtered out.
pub fn compile(pattern: &Pattern, cycles: usize, bpm: f64, beats_per_cycle: f64) -> IrClip {
    if cycles == 0 {
        return IrClip {
            events: vec![],
            length: 0.0,
            bpm,
        };
    }

    let cycle_dur = pattern.duration();
    let mut all_events = Vec::new();

    for c in 0..cycles {
        let offset = c as f64 * cycle_dur;
        for e in pattern.events() {
            all_events.push(IrEvent {
                value: e.value,
                start: (e.start + offset) * beats_per_cycle,
                duration: e.duration * beats_per_cycle,
            });
        }
    }

    let length = cycles as f64 * cycle_dur * beats_per_cycle;

    IrClip {
        events: all_events,
        length,
        bpm,
    }
}

// ---------------------------------------------------------------------------
// Compilation: Signal → IrAutomation
// ---------------------------------------------------------------------------

/// Compile a Signal into an IrAutomation by sampling it to breakpoints.
///
/// For constant signals, produces just two breakpoints (start and end).
/// For dynamic signals, samples at `resolution` points per beat.
pub fn compile_signal(
    signal: &Signal,
    name: &str,
    total_beats: f64,
    resolution: usize,
) -> IrAutomation {
    let breakpoints = if signal.is_const() {
        let v = signal.sample(0.0);
        vec![
            IrBreakpoint { time: 0.0, value: v },
            IrBreakpoint { time: total_beats, value: v },
        ]
    } else {
        let n = ((total_beats * resolution as f64) as usize).max(2);
        let step = total_beats / (n - 1) as f64;
        (0..n)
            .map(|i| {
                let t = i as f64 * step;
                IrBreakpoint {
                    time: t,
                    value: signal.sample(t),
                }
            })
            .collect()
    };

    IrAutomation {
        param_name: name.to_string(),
        breakpoints,
    }
}

// ---------------------------------------------------------------------------
// Compilation: Effect → IrEffect
// ---------------------------------------------------------------------------

fn compile_effect(
    effect: &crate::effect::Effect,
    total_beats: f64,
    resolution: usize,
) -> IrEffect {
    let mut params = Vec::new();
    let mut automation = Vec::new();

    for p in &effect.params {
        params.push(IrParam {
            name: p.name.clone(),
            value: p.sample(0.0),
            range: p.range,
        });

        if !p.signal.is_const() {
            automation.push(compile_signal(&p.signal, &p.name, total_beats, resolution));
        }
    }

    IrEffect {
        name: effect.name.clone(),
        params,
        automation,
    }
}

fn compile_chain(
    chain: &EffectChain,
    total_beats: f64,
    resolution: usize,
) -> Vec<IrEffect> {
    chain.iter().map(|e| compile_effect(e, total_beats, resolution)).collect()
}

// ---------------------------------------------------------------------------
// Compilation: Patch → IrPatch
// ---------------------------------------------------------------------------

/// Compile a Patch into an IrPatch.
///
/// - `bpm`: tempo metadata.
/// - `cycles`: how many cycles to evaluate pattern nodes for.
/// - `beats_per_cycle`: beats per cycle (4.0 for 4/4).
/// - `automation_resolution`: breakpoints per beat for Signal sampling.
pub fn compile_patch(
    patch: &Patch,
    bpm: f64,
    cycles: usize,
    beats_per_cycle: f64,
    automation_resolution: usize,
) -> IrPatch {
    let total_beats = cycles as f64 * beats_per_cycle;

    // Compile nodes
    let nodes: Vec<IrNode> = patch
        .nodes()
        .iter()
        .enumerate()
        .map(|(id, node)| match node {
            Node::SourceNode { source, label } => {
                let ir_source = match source {
                    crate::source::Source::Sample { path, label: l } => IrSource::Sample {
                        path: path.clone(),
                        label: l.as_deref().unwrap_or(path).to_string(),
                    },
                    crate::source::Source::LiveInput { channel, label: l } => IrSource::LiveInput {
                        channel: *channel,
                        label: l.as_deref().unwrap_or("live-in").to_string(),
                    },
                    crate::source::Source::Synth { preset, params, label: l } => IrSource::Synth {
                        preset: preset.clone(),
                        params: params.iter().map(|(k, v)| (k.clone(), *v)).collect(),
                        label: l.as_deref().unwrap_or(preset).to_string(),
                    },
                    crate::source::Source::Resampled { patch_id, label: l } => {
                        IrSource::Resampled {
                            patch_id: patch_id.clone(),
                            label: l.as_deref().unwrap_or(patch_id).to_string(),
                        }
                    }
                };
                IrNode::Source {
                    id,
                    source: ir_source,
                    label: label.clone(),
                }
            }
            Node::PatternNode { pattern, label } => {
                let clip = compile(pattern, cycles, bpm, beats_per_cycle);
                IrNode::Pattern {
                    id,
                    clip,
                    label: label.clone(),
                }
            }
            Node::EffectNode { effect, label } => {
                let ir_effect = compile_effect(effect, total_beats, automation_resolution);
                IrNode::Effect {
                    id,
                    effect: ir_effect,
                    label: label.clone(),
                }
            }
            Node::ChainNode { chain, label } => {
                let effects = compile_chain(chain, total_beats, automation_resolution);
                IrNode::Chain {
                    id,
                    effects,
                    label: label.clone(),
                }
            }
            Node::MixerNode { channels, label } => IrNode::Mixer {
                id,
                channels: *channels,
                label: label.clone(),
            },
            Node::SplitNode { outputs, label } => IrNode::Split {
                id,
                outputs: *outputs,
                label: label.clone(),
            },
            Node::MergeNode { inputs, label } => IrNode::Merge {
                id,
                inputs: *inputs,
                label: label.clone(),
            },
        })
        .collect();

    // Compile edges
    let edges: Vec<IrEdge> = patch
        .edges()
        .iter()
        .map(|e| IrEdge {
            from_node: e.from_node,
            from_port: e.from_port,
            to_node: e.to_node,
            to_port: e.to_port,
        })
        .collect();

    // Compile space
    let space = IrSpace {
        pan: compile_signal(
            &patch.space().pan,
            "pan",
            total_beats,
            automation_resolution,
        ),
        width: compile_signal(
            &patch.space().width,
            "width",
            total_beats,
            automation_resolution,
        ),
        depth: compile_signal(
            &patch.space().depth,
            "depth",
            total_beats,
            automation_resolution,
        ),
    };

    // Compile exposed params
    let exposed_params: Vec<IrExposedParam> = patch
        .params()
        .iter()
        .map(|p| IrExposedParam {
            name: p.name.clone(),
            range: p.range,
            default: p.default,
            automation: compile_signal(
                &p.signal,
                &p.name,
                total_beats,
                automation_resolution,
            ),
        })
        .collect();

    IrPatch {
        label: patch.label().to_string(),
        bpm,
        nodes,
        edges,
        space,
        exposed_params,
    }
}

// ---------------------------------------------------------------------------
// JSON serialization (via serde)
// ---------------------------------------------------------------------------

impl IrEvent {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }
}

impl IrClip {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }
}

impl IrTrack {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }
}

impl IrSession {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }
}

impl IrPatch {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::Pattern;

    const EPS: f64 = 1e-9;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    // --- compile basics ---

    #[test]
    fn compile_basic_event_count() {
        let p = Pattern::seq(&[60.0, 64.0, 67.0]);
        let clip = compile(&p, 1, 120.0, 4.0);
        assert_eq!(clip.events.len(), 3);
    }

    #[test]
    fn compile_beat_positions() {
        // 3 events in 1 cycle, 4 beats per cycle
        // event starts: 0.0, 1/3, 2/3 in cycles → 0.0, 4/3, 8/3 in beats
        let p = Pattern::seq(&[60.0, 64.0, 67.0]);
        let clip = compile(&p, 1, 120.0, 4.0);
        assert!(approx(clip.events[0].start, 0.0));
        assert!(approx(clip.events[1].start, 4.0 / 3.0));
        assert!(approx(clip.events[2].start, 8.0 / 3.0));
    }

    #[test]
    fn compile_beat_durations() {
        let p = Pattern::seq(&[60.0, 64.0, 67.0]);
        let clip = compile(&p, 1, 120.0, 4.0);
        // Each atom has duration 1/3 cycle = 4/3 beats
        for e in &clip.events {
            assert!(approx(e.duration, 4.0 / 3.0));
        }
    }

    #[test]
    fn compile_total_length() {
        let p = Pattern::seq(&[60.0, 64.0]);
        let clip = compile(&p, 1, 120.0, 4.0);
        assert!(approx(clip.length, 4.0)); // 1 cycle * 4 beats
    }

    #[test]
    fn compile_multi_cycle() {
        let p = Pattern::seq(&[60.0, 64.0]);
        let clip = compile(&p, 4, 120.0, 4.0);
        assert_eq!(clip.events.len(), 8); // 2 events * 4 cycles
        assert!(approx(clip.length, 16.0)); // 4 cycles * 4 beats
    }

    #[test]
    fn compile_multi_cycle_offsets() {
        let p = Pattern::seq(&[60.0, 64.0]); // each 0.5 cycle
        let clip = compile(&p, 2, 120.0, 4.0);
        // Cycle 0: 0.0, 2.0 beats; Cycle 1: 4.0, 6.0 beats
        assert!(approx(clip.events[0].start, 0.0));
        assert!(approx(clip.events[1].start, 2.0));
        assert!(approx(clip.events[2].start, 4.0));
        assert!(approx(clip.events[3].start, 6.0));
    }

    #[test]
    fn compile_filters_nan() {
        // Pattern with silence (NaN atoms)
        let p = Pattern::Seq(vec![
            Pattern::Atom(60.0, 0.5),
            Pattern::silence(0.25),
            Pattern::Atom(67.0, 0.25),
        ]);
        let clip = compile(&p, 1, 120.0, 4.0);
        assert_eq!(clip.events.len(), 2); // only non-NaN events
        assert!(approx(clip.events[0].value, 60.0));
        assert!(approx(clip.events[1].value, 67.0));
    }

    #[test]
    fn compile_empty_pattern() {
        let p = Pattern::empty();
        let clip = compile(&p, 1, 120.0, 4.0);
        assert!(clip.events.is_empty());
        assert!(approx(clip.length, 0.0));
    }

    #[test]
    fn compile_zero_cycles() {
        let p = Pattern::seq(&[60.0, 64.0]);
        let clip = compile(&p, 0, 120.0, 4.0);
        assert!(clip.events.is_empty());
        assert!(approx(clip.length, 0.0));
    }

    #[test]
    fn compile_preserves_bpm() {
        let p = Pattern::seq(&[60.0]);
        let clip = compile(&p, 1, 140.0, 4.0);
        assert!(approx(clip.bpm, 140.0));
    }

    // --- JSON serialization ---

    #[test]
    fn ir_event_to_json() {
        let e = IrEvent { value: 60.0, start: 0.0, duration: 1.0 };
        let json = e.to_json();
        // serde_json compact format: no spaces after colons
        assert!(json.contains("\"value\":60.0"));
        assert!(json.contains("\"start\":0.0"));
        assert!(json.contains("\"duration\":1.0"));
    }

    #[test]
    fn ir_clip_to_json() {
        let clip = IrClip {
            events: vec![
                IrEvent { value: 60.0, start: 0.0, duration: 2.0 },
                IrEvent { value: 64.0, start: 2.0, duration: 2.0 },
            ],
            length: 4.0,
            bpm: 120.0,
        };
        let json = clip.to_json();
        assert!(json.contains("\"events\":"));
        assert!(json.contains("\"length\":4.0"));
        assert!(json.contains("\"bpm\":120.0"));
        assert!(json.contains("60.0"));
        assert!(json.contains("64.0"));
    }

    #[test]
    fn ir_session_to_json() {
        let session = IrSession {
            bpm: 120.0,
            tracks: vec![
                IrTrack {
                    name: "melody".to_string(),
                    clips: vec![IrClip {
                        events: vec![IrEvent { value: 60.0, start: 0.0, duration: 4.0 }],
                        length: 4.0,
                        bpm: 120.0,
                    }],
                },
                IrTrack {
                    name: "bass".to_string(),
                    clips: vec![],
                },
            ],
        };
        let json = session.to_json();
        assert!(json.contains("\"bpm\":120.0"));
        assert!(json.contains("\"melody\""));
        assert!(json.contains("\"bass\""));
    }

    #[test]
    fn ir_track_string_escaping() {
        let track = IrTrack {
            name: "track \"one\"".to_string(),
            clips: vec![],
        };
        let json = track.to_json();
        // serde_json escapes embedded quotes correctly
        assert!(json.contains("track \\\"one\\\""));
    }

    #[test]
    fn compile_and_serialize_round_trip() {
        let p = Pattern::seq(&[60.0, 64.0, 67.0, 72.0]);
        let clip = compile(&p, 2, 120.0, 4.0);
        let json = clip.to_json();
        // Should have 8 events serialized
        assert_eq!(json.matches("\"value\":").count(), 8);
        assert!(json.contains("\"length\":8.0"));
    }

    // --- serde round-trip ---

    #[test]
    fn ir_event_serde_round_trip() {
        let original = IrEvent { value: 60.0, start: 1.5, duration: 0.75 };
        let json = original.to_json();
        let decoded: IrEvent = serde_json::from_str(&json).unwrap();
        assert!(approx(decoded.value, original.value));
        assert!(approx(decoded.start, original.start));
        assert!(approx(decoded.duration, original.duration));
    }

    #[test]
    fn ir_clip_serde_round_trip() {
        let original = IrClip {
            events: vec![
                IrEvent { value: 60.0, start: 0.0, duration: 2.0 },
                IrEvent { value: 67.0, start: 2.0, duration: 2.0 },
            ],
            length: 4.0,
            bpm: 130.0,
        };
        let json = original.to_json();
        let decoded: IrClip = serde_json::from_str(&json).unwrap();
        assert!(approx(decoded.bpm, original.bpm));
        assert!(approx(decoded.length, original.length));
        assert_eq!(decoded.events.len(), original.events.len());
        for (a, b) in decoded.events.iter().zip(original.events.iter()) {
            assert!(approx(a.value, b.value));
            assert!(approx(a.start, b.start));
            assert!(approx(a.duration, b.duration));
        }
    }

    #[test]
    fn ir_track_serde_round_trip() {
        let original = IrTrack {
            name: "lead synth".to_string(),
            clips: vec![IrClip {
                events: vec![IrEvent { value: 72.0, start: 0.0, duration: 1.0 }],
                length: 4.0,
                bpm: 120.0,
            }],
        };
        let json = original.to_json();
        let decoded: IrTrack = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, original.name);
        assert_eq!(decoded.clips.len(), original.clips.len());
        assert!(approx(decoded.clips[0].bpm, original.clips[0].bpm));
    }

    #[test]
    fn ir_session_serde_round_trip() {
        let original = IrSession {
            bpm: 98.0,
            tracks: vec![
                IrTrack {
                    name: "kick".to_string(),
                    clips: vec![IrClip {
                        events: vec![IrEvent { value: 36.0, start: 0.0, duration: 0.5 }],
                        length: 4.0,
                        bpm: 98.0,
                    }],
                },
                IrTrack {
                    name: "pad".to_string(),
                    clips: vec![],
                },
            ],
        };
        let json = original.to_json();
        let decoded: IrSession = serde_json::from_str(&json).unwrap();
        assert!(approx(decoded.bpm, original.bpm));
        assert_eq!(decoded.tracks.len(), original.tracks.len());
        assert_eq!(decoded.tracks[0].name, original.tracks[0].name);
        assert_eq!(decoded.tracks[1].name, original.tracks[1].name);
        assert_eq!(decoded.tracks[0].clips[0].events[0].value, 36.0);
    }

    #[test]
    fn to_json_pretty_is_multiline() {
        let e = IrEvent { value: 60.0, start: 0.0, duration: 1.0 };
        let pretty = e.to_json_pretty();
        assert!(pretty.contains('\n'));
        assert!(pretty.contains("  ")); // indented
    }

    // --- compile_signal ---

    #[test]
    fn compile_signal_const() {
        let s = crate::signal::Signal::constant(0.5);
        let auto = compile_signal(&s, "test", 4.0, 16);
        assert_eq!(auto.breakpoints.len(), 2);
        assert!(approx(auto.breakpoints[0].value, 0.5));
        assert!(approx(auto.breakpoints[1].value, 0.5));
        assert!(approx(auto.breakpoints[0].time, 0.0));
        assert!(approx(auto.breakpoints[1].time, 4.0));
    }

    #[test]
    fn compile_signal_lfo() {
        let s = crate::signal::Signal::sine(1.0);
        let auto = compile_signal(&s, "pan", 4.0, 4);
        // 4 beats * 4 resolution = 16 breakpoints
        assert_eq!(auto.breakpoints.len(), 16);
        assert_eq!(auto.param_name, "pan");
    }

    #[test]
    fn compile_signal_preserves_name() {
        let s = crate::signal::Signal::constant(1.0);
        let auto = compile_signal(&s, "brightness", 4.0, 16);
        assert_eq!(auto.param_name, "brightness");
    }

    // --- compile_patch ---

    #[test]
    fn compile_patch_minimal() {
        let mut p = crate::patch::Patch::new("simple");
        p.add_source(crate::source::Source::sample("kick.wav"));
        let ir = compile_patch(&p, 120.0, 1, 4.0, 16);
        assert_eq!(ir.label, "simple");
        assert!(approx(ir.bpm, 120.0));
        assert_eq!(ir.nodes.len(), 1);
        assert!(ir.edges.is_empty());
    }

    #[test]
    fn compile_patch_source_and_effect() {
        let mut p = crate::patch::Patch::new("filtered");
        let src = p.add_source(crate::source::Source::synth("Analog"));
        let fx = p.add_effect(crate::effect::lowpass(2000.0, 0.5));
        p.connect(src, 0, fx, 0);
        let ir = compile_patch(&p, 120.0, 1, 4.0, 16);
        assert_eq!(ir.nodes.len(), 2);
        assert_eq!(ir.edges.len(), 1);
    }

    #[test]
    fn compile_patch_with_pattern() {
        let mut p = crate::patch::Patch::new("rhythmic");
        p.add_source(crate::source::Source::sample("pad.wav"));
        p.add_pattern(Pattern::seq(&[60.0, 64.0, 67.0]), "melody");
        let ir = compile_patch(&p, 120.0, 2, 4.0, 16);
        // Find the pattern node and check it has events
        let pat_node = ir.nodes.iter().find(|n| matches!(n, IrNode::Pattern { .. }));
        assert!(pat_node.is_some());
        if let IrNode::Pattern { clip, .. } = pat_node.unwrap() {
            assert_eq!(clip.events.len(), 6); // 3 events * 2 cycles
        }
    }

    #[test]
    fn compile_patch_with_automation() {
        let mut p = crate::patch::Patch::new("automated");
        let src = p.add_source(crate::source::Source::sample("pad.wav"));
        let mut fx = crate::effect::lowpass(2000.0, 0.5);
        fx.get_param_mut("cutoff")
            .unwrap()
            .set(crate::signal::Signal::sine(0.25).scale(5000.0, 5000.0));
        let fx_id = p.add_effect(fx);
        p.connect(src, 0, fx_id, 0);
        let ir = compile_patch(&p, 120.0, 1, 4.0, 4);

        // The effect node should have automation for cutoff
        let fx_node = ir.nodes.iter().find(|n| matches!(n, IrNode::Effect { .. }));
        if let Some(IrNode::Effect { effect, .. }) = fx_node {
            assert!(!effect.automation.is_empty());
            let cutoff_auto = effect.automation.iter().find(|a| a.param_name == "cutoff");
            assert!(cutoff_auto.is_some());
            assert!(cutoff_auto.unwrap().breakpoints.len() > 2);
        } else {
            panic!("expected effect node");
        }
    }

    #[test]
    fn compile_patch_exposed_params() {
        let mut p = crate::patch::Patch::new("controlled");
        p.add_source(crate::source::Source::sample("x.wav"));
        p.expose_param(crate::param::Param::new("density", (0.0, 1.0), 0.5));
        let ir = compile_patch(&p, 120.0, 1, 4.0, 16);
        assert_eq!(ir.exposed_params.len(), 1);
        assert_eq!(ir.exposed_params[0].name, "density");
        assert!(approx(ir.exposed_params[0].default, 0.5));
    }

    #[test]
    fn compile_patch_space() {
        let mut p = crate::patch::Patch::new("spatial");
        p.add_source(crate::source::Source::sample("x.wav"));
        p.set_space(
            crate::space::Space::center()
                .with_pan(crate::signal::Signal::sine(0.5)),
        );
        let ir = compile_patch(&p, 120.0, 1, 4.0, 4);
        // Pan should have dynamic automation (more than 2 breakpoints)
        assert!(ir.space.pan.breakpoints.len() > 2);
        // Width and depth are const (2 breakpoints each)
        assert_eq!(ir.space.width.breakpoints.len(), 2);
        assert_eq!(ir.space.depth.breakpoints.len(), 2);
    }

    #[test]
    fn compile_patch_serde_round_trip() {
        let mut p = crate::patch::Patch::new("roundtrip");
        let src = p.add_source(crate::source::Source::sample("test.wav"));
        let fx = p.add_effect(crate::effect::reverb(3.0, 0.5));
        p.connect(src, 0, fx, 0);
        p.expose_param(crate::param::Param::new("mix", (0.0, 1.0), 0.5));

        let ir = compile_patch(&p, 120.0, 1, 4.0, 16);
        let json = ir.to_json();
        let decoded: IrPatch = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.label, "roundtrip");
        assert!(approx(decoded.bpm, 120.0));
        assert_eq!(decoded.nodes.len(), 2);
        assert_eq!(decoded.edges.len(), 1);
        assert_eq!(decoded.exposed_params.len(), 1);
    }

    #[test]
    fn compile_patch_full_example() {
        use crate::construct::euclidean;

        let mut p = crate::patch::Patch::new("water_braid");
        let water = p.add_source(
            crate::source::Source::sample_labeled("water.wav", "water grains"),
        );
        let glass = p.add_source(
            crate::source::Source::sample_labeled("glass.wav", "glass grains"),
        );
        let _rhythm = p.add_pattern(euclidean(5, 8).rotate(2), "main rhythm");
        let rev = p.add_effect(crate::effect::reverb(4.0, 0.5));
        let dly = p.add_effect(crate::effect::delay(0.375, 0.4, 0.3));
        let mix = p.add_mixer(2, "submix");
        p.connect(water, 0, mix, 0);
        p.connect(glass, 0, mix, 1);
        p.connect(mix, 0, rev, 0);
        p.connect(rev, 0, dly, 0);
        p.expose_param(crate::param::Param::new("density", (0.0, 1.0), 0.3));
        p.expose_param(crate::param::Param::new("brightness", (0.0, 1.0), 0.6));
        p.set_space(
            crate::space::Space::center()
                .with_pan(crate::signal::Signal::sine(0.03))
                .with_depth(crate::signal::Signal::Const(0.5)),
        );

        let ir = compile_patch(&p, 120.0, 4, 4.0, 16);

        // Verify structure
        assert_eq!(ir.label, "water_braid");
        assert_eq!(ir.nodes.len(), 6);
        assert_eq!(ir.edges.len(), 4);
        assert_eq!(ir.exposed_params.len(), 2);

        // JSON round-trip
        let json = ir.to_json();
        let decoded: IrPatch = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.nodes.len(), 6);
        assert_eq!(decoded.edges.len(), 4);

        // Pretty print should be valid JSON
        let pretty = ir.to_json_pretty();
        let decoded2: IrPatch = serde_json::from_str(&pretty).unwrap();
        assert_eq!(decoded2.label, "water_braid");
    }
}
