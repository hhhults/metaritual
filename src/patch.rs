/// Patch — the routing diagram.
///
/// A Patch is a directed graph where nodes are processing stages and
/// edges are signal flow. It's an open system with typed inputs and
/// outputs. Two patches compose when their interfaces match.
///
/// The `exposed_params` are the control surface — what makes the
/// patch playable. Everything that produces Signals connects through
/// these exposed params.

use crate::effect::{Effect, EffectChain};
use crate::param::{Param, ParamSet};
use crate::pattern::Pattern;
use crate::source::Source;
use crate::space::Space;

// ---------------------------------------------------------------------------
// Port types
// ---------------------------------------------------------------------------

/// The type of data flowing through a port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortType {
    /// Audio signal (continuous).
    Audio,
    /// Control signal (automation, LFO, etc.).
    Control,
    /// Discrete event stream (notes, triggers).
    Pattern,
}

/// A typed port on a node.
#[derive(Debug, Clone)]
pub struct Port {
    pub id: usize,
    pub port_type: PortType,
    pub label: String,
}

// ---------------------------------------------------------------------------
// Node
// ---------------------------------------------------------------------------

/// A processing stage in the patch graph.
#[derive(Debug, Clone)]
pub enum Node {
    /// A sound source (sample, synth, live input, resampled).
    SourceNode {
        source: Source,
        label: String,
    },
    /// A discrete pattern (rhythm, melody, etc.).
    PatternNode {
        pattern: Pattern,
        label: String,
    },
    /// An audio effect (reverb, delay, filter, etc.).
    EffectNode {
        effect: Effect,
        label: String,
    },
    /// An effect chain (multiple effects in series).
    ChainNode {
        chain: EffectChain,
        label: String,
    },
    /// A mixer — blends multiple inputs with gain control.
    MixerNode {
        channels: usize,
        label: String,
    },
    /// Splits one signal into multiple outputs.
    SplitNode {
        outputs: usize,
        label: String,
    },
    /// Merges multiple inputs into one output.
    MergeNode {
        inputs: usize,
        label: String,
    },
}

impl Node {
    /// Get the node's label.
    pub fn label(&self) -> &str {
        match self {
            Node::SourceNode { label, .. } => label,
            Node::PatternNode { label, .. } => label,
            Node::EffectNode { label, .. } => label,
            Node::ChainNode { label, .. } => label,
            Node::MixerNode { label, .. } => label,
            Node::SplitNode { label, .. } => label,
            Node::MergeNode { label, .. } => label,
        }
    }

    /// Input ports for this node type.
    pub fn input_ports(&self) -> Vec<Port> {
        match self {
            Node::SourceNode { .. } => {
                // Sources have no audio input (they generate sound)
                vec![]
            }
            Node::PatternNode { .. } => {
                // Pattern nodes have no input — they generate events
                vec![]
            }
            Node::EffectNode { .. } => {
                vec![Port {
                    id: 0,
                    port_type: PortType::Audio,
                    label: "in".to_string(),
                }]
            }
            Node::ChainNode { .. } => {
                vec![Port {
                    id: 0,
                    port_type: PortType::Audio,
                    label: "in".to_string(),
                }]
            }
            Node::MixerNode { channels, .. } => {
                (0..*channels)
                    .map(|i| Port {
                        id: i,
                        port_type: PortType::Audio,
                        label: format!("in_{}", i),
                    })
                    .collect()
            }
            Node::SplitNode { .. } => {
                vec![Port {
                    id: 0,
                    port_type: PortType::Audio,
                    label: "in".to_string(),
                }]
            }
            Node::MergeNode { inputs, .. } => {
                (0..*inputs)
                    .map(|i| Port {
                        id: i,
                        port_type: PortType::Audio,
                        label: format!("in_{}", i),
                    })
                    .collect()
            }
        }
    }

    /// Output ports for this node type.
    pub fn output_ports(&self) -> Vec<Port> {
        match self {
            Node::SourceNode { .. } => {
                vec![Port {
                    id: 0,
                    port_type: PortType::Audio,
                    label: "out".to_string(),
                }]
            }
            Node::PatternNode { .. } => {
                vec![Port {
                    id: 0,
                    port_type: PortType::Pattern,
                    label: "events".to_string(),
                }]
            }
            Node::EffectNode { .. } => {
                vec![Port {
                    id: 0,
                    port_type: PortType::Audio,
                    label: "out".to_string(),
                }]
            }
            Node::ChainNode { .. } => {
                vec![Port {
                    id: 0,
                    port_type: PortType::Audio,
                    label: "out".to_string(),
                }]
            }
            Node::MixerNode { .. } => {
                vec![Port {
                    id: 0,
                    port_type: PortType::Audio,
                    label: "out".to_string(),
                }]
            }
            Node::SplitNode { outputs, .. } => {
                (0..*outputs)
                    .map(|i| Port {
                        id: i,
                        port_type: PortType::Audio,
                        label: format!("out_{}", i),
                    })
                    .collect()
            }
            Node::MergeNode { .. } => {
                vec![Port {
                    id: 0,
                    port_type: PortType::Audio,
                    label: "out".to_string(),
                }]
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Edge
// ---------------------------------------------------------------------------

/// A connection between two ports in the graph.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub from_node: usize,
    pub from_port: usize,
    pub to_node: usize,
    pub to_port: usize,
}

// ---------------------------------------------------------------------------
// Validation errors
// ---------------------------------------------------------------------------

/// Errors that can occur when validating a patch.
#[derive(Debug, Clone, PartialEq)]
pub enum PatchError {
    /// An edge references a node that doesn't exist.
    DanglingNode { edge_index: usize, node_id: usize },
    /// An edge references a port that doesn't exist.
    DanglingPort {
        edge_index: usize,
        node_id: usize,
        port_id: usize,
    },
    /// Connected ports have incompatible types.
    PortTypeMismatch {
        edge_index: usize,
        from_type: PortType,
        to_type: PortType,
    },
    /// An edge creates a self-loop.
    SelfLoop { edge_index: usize, node_id: usize },
}

// ---------------------------------------------------------------------------
// Patch
// ---------------------------------------------------------------------------

/// A routing diagram — the core compositional unit.
#[derive(Debug, Clone)]
pub struct Patch {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    params: ParamSet,
    space: Space,
    label: String,
}

impl Patch {
    /// Create an empty patch.
    pub fn new(label: &str) -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            params: ParamSet::new(),
            space: Space::default(),
            label: label.to_string(),
        }
    }

    /// Add a node. Returns its index.
    pub fn add_node(&mut self, node: Node) -> usize {
        let id = self.nodes.len();
        self.nodes.push(node);
        id
    }

    /// Add a source node. Returns its index.
    pub fn add_source(&mut self, source: Source) -> usize {
        let label = source.label().to_string();
        self.add_node(Node::SourceNode { source, label })
    }

    /// Add a pattern node. Returns its index.
    pub fn add_pattern(&mut self, pattern: Pattern, label: &str) -> usize {
        self.add_node(Node::PatternNode {
            pattern,
            label: label.to_string(),
        })
    }

    /// Add an effect node. Returns its index.
    pub fn add_effect(&mut self, effect: Effect) -> usize {
        let label = effect.name.clone();
        self.add_node(Node::EffectNode { effect, label })
    }

    /// Add an effect chain node. Returns its index.
    pub fn add_chain(&mut self, chain: EffectChain, label: &str) -> usize {
        self.add_node(Node::ChainNode {
            chain,
            label: label.to_string(),
        })
    }

    /// Add a mixer node with N channels. Returns its index.
    pub fn add_mixer(&mut self, channels: usize, label: &str) -> usize {
        self.add_node(Node::MixerNode {
            channels,
            label: label.to_string(),
        })
    }

    /// Add a split node. Returns its index.
    pub fn add_split(&mut self, outputs: usize) -> usize {
        self.add_node(Node::SplitNode {
            outputs,
            label: "split".to_string(),
        })
    }

    /// Add a merge node. Returns its index.
    pub fn add_merge(&mut self, inputs: usize) -> usize {
        self.add_node(Node::MergeNode {
            inputs,
            label: "merge".to_string(),
        })
    }

    /// Connect two nodes. Returns the edge index.
    pub fn connect(
        &mut self,
        from_node: usize,
        from_port: usize,
        to_node: usize,
        to_port: usize,
    ) -> usize {
        let idx = self.edges.len();
        self.edges.push(Edge {
            from_node,
            from_port,
            to_node,
            to_port,
        });
        idx
    }

    /// Expose a parameter on the control surface.
    pub fn expose_param(&mut self, param: Param) {
        self.params.add(param);
    }

    /// Set the spatial configuration.
    pub fn set_space(&mut self, space: Space) {
        self.space = space;
    }

    // --- Accessors -----------------------------------------------------------

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn params(&self) -> &ParamSet {
        &self.params
    }

    pub fn params_mut(&mut self) -> &mut ParamSet {
        &mut self.params
    }

    pub fn space(&self) -> &Space {
        &self.space
    }

    pub fn get_node(&self, id: usize) -> Option<&Node> {
        self.nodes.get(id)
    }

    // --- Validation ----------------------------------------------------------

    /// Validate the patch graph. Returns a list of errors (empty = valid).
    pub fn validate(&self) -> Vec<PatchError> {
        let mut errors = Vec::new();

        for (ei, edge) in self.edges.iter().enumerate() {
            // Self-loop check
            if edge.from_node == edge.to_node {
                errors.push(PatchError::SelfLoop {
                    edge_index: ei,
                    node_id: edge.from_node,
                });
                continue;
            }

            // Dangling node checks
            let from_node = if edge.from_node < self.nodes.len() {
                &self.nodes[edge.from_node]
            } else {
                errors.push(PatchError::DanglingNode {
                    edge_index: ei,
                    node_id: edge.from_node,
                });
                continue;
            };

            let to_node = if edge.to_node < self.nodes.len() {
                &self.nodes[edge.to_node]
            } else {
                errors.push(PatchError::DanglingNode {
                    edge_index: ei,
                    node_id: edge.to_node,
                });
                continue;
            };

            // Dangling port checks
            let out_ports = from_node.output_ports();
            let from_port = out_ports.iter().find(|p| p.id == edge.from_port);
            if from_port.is_none() {
                errors.push(PatchError::DanglingPort {
                    edge_index: ei,
                    node_id: edge.from_node,
                    port_id: edge.from_port,
                });
                continue;
            }

            let in_ports = to_node.input_ports();
            let to_port = in_ports.iter().find(|p| p.id == edge.to_port);
            if to_port.is_none() {
                errors.push(PatchError::DanglingPort {
                    edge_index: ei,
                    node_id: edge.to_node,
                    port_id: edge.to_port,
                });
                continue;
            }

            // Port type compatibility
            let from_type = from_port.unwrap().port_type;
            let to_type = to_port.unwrap().port_type;
            if from_type != to_type {
                errors.push(PatchError::PortTypeMismatch {
                    edge_index: ei,
                    from_type,
                    to_type,
                });
            }
        }

        errors
    }

    /// Check if the patch is valid (no errors).
    pub fn is_valid(&self) -> bool {
        self.validate().is_empty()
    }
}

// ---------------------------------------------------------------------------
// Composition
// ---------------------------------------------------------------------------

/// Connect patch A's outputs to patch B's inputs in series.
///
/// Creates a new patch containing all nodes from both patches,
/// with edges connecting A's output nodes to B's input nodes.
/// A's audio outputs connect to B's audio inputs in order.
pub fn series(a: &Patch, b: &Patch, label: &str) -> Patch {
    let mut result = Patch::new(label);
    let a_offset = 0;
    let b_offset = a.nodes.len();

    // Copy all nodes
    for node in &a.nodes {
        result.nodes.push(node.clone());
    }
    for node in &b.nodes {
        result.nodes.push(node.clone());
    }

    // Copy edges from A (no offset needed)
    for edge in &a.edges {
        result.edges.push(edge.clone());
    }

    // Copy edges from B (offset node indices)
    for edge in &b.edges {
        result.edges.push(Edge {
            from_node: edge.from_node + b_offset,
            from_port: edge.from_port,
            to_node: edge.to_node + b_offset,
            to_port: edge.to_port,
        });
    }

    // Find A's terminal audio output nodes (nodes with audio outputs and no outgoing edges)
    let a_output_nodes: Vec<usize> = (a_offset..b_offset)
        .filter(|&i| {
            let has_audio_out = result.nodes[i]
                .output_ports()
                .iter()
                .any(|p| p.port_type == PortType::Audio);
            let has_outgoing = result
                .edges
                .iter()
                .any(|e| e.from_node == i);
            has_audio_out && !has_outgoing
        })
        .collect();

    // Find B's initial audio input nodes (nodes with audio inputs and no incoming edges)
    let b_input_nodes: Vec<usize> = (b_offset..result.nodes.len())
        .filter(|&i| {
            let has_audio_in = result.nodes[i]
                .input_ports()
                .iter()
                .any(|p| p.port_type == PortType::Audio);
            let has_incoming = result
                .edges
                .iter()
                .any(|e| e.to_node == i);
            has_audio_in && !has_incoming
        })
        .collect();

    // Connect A's outputs to B's inputs (pair 1:1, or first available)
    for (a_node, b_node) in a_output_nodes.iter().zip(b_input_nodes.iter()) {
        result.edges.push(Edge {
            from_node: *a_node,
            from_port: 0,
            to_node: *b_node,
            to_port: 0,
        });
    }

    result
}

/// Run patches A and B in parallel, merge their outputs.
///
/// Creates a new patch with a merge node collecting both patches'
/// audio outputs.
pub fn parallel(a: &Patch, b: &Patch, label: &str) -> Patch {
    let mut result = Patch::new(label);
    let a_offset = 0;
    let b_offset = a.nodes.len();

    // Copy all nodes
    for node in &a.nodes {
        result.nodes.push(node.clone());
    }
    for node in &b.nodes {
        result.nodes.push(node.clone());
    }

    // Copy edges from A
    for edge in &a.edges {
        result.edges.push(edge.clone());
    }

    // Copy edges from B (offset)
    for edge in &b.edges {
        result.edges.push(Edge {
            from_node: edge.from_node + b_offset,
            from_port: edge.from_port,
            to_node: edge.to_node + b_offset,
            to_port: edge.to_port,
        });
    }

    // Find terminal audio output nodes from both patches
    let a_outputs: Vec<usize> = (a_offset..b_offset)
        .filter(|&i| {
            let has_audio_out = result.nodes[i]
                .output_ports()
                .iter()
                .any(|p| p.port_type == PortType::Audio);
            let has_outgoing = result.edges.iter().any(|e| e.from_node == i);
            has_audio_out && !has_outgoing
        })
        .collect();

    let b_outputs: Vec<usize> = (b_offset..result.nodes.len())
        .filter(|&i| {
            let has_audio_out = result.nodes[i]
                .output_ports()
                .iter()
                .any(|p| p.port_type == PortType::Audio);
            let has_outgoing = result.edges.iter().any(|e| e.from_node == i);
            has_audio_out && !has_outgoing
        })
        .collect();

    let total_inputs = a_outputs.len() + b_outputs.len();
    if total_inputs > 0 {
        let merge_id = result.add_merge(total_inputs);

        for (i, &node_id) in a_outputs.iter().chain(b_outputs.iter()).enumerate() {
            result.edges.push(Edge {
                from_node: node_id,
                from_port: 0,
                to_node: merge_id,
                to_port: i,
            });
        }
    }

    result
}

/// Add a feedback path within a patch.
///
/// Connects `from_node`'s output back to `to_node`'s input,
/// creating a feedback loop. The edge is just added directly —
/// cycle detection is left to the compiler/runtime.
pub fn feedback(patch: &mut Patch, from_node: usize, to_node: usize) {
    patch.edges.push(Edge {
        from_node,
        from_port: 0,
        to_node,
        to_port: 0,
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::construct::euclidean;
    use crate::effect;

    #[test]
    fn empty_patch() {
        let p = Patch::new("test");
        assert_eq!(p.node_count(), 0);
        assert_eq!(p.edge_count(), 0);
        assert!(p.is_valid());
    }

    #[test]
    fn add_source_node() {
        let mut p = Patch::new("test");
        let id = p.add_source(Source::sample("kick.wav"));
        assert_eq!(p.node_count(), 1);
        assert_eq!(p.get_node(id).unwrap().label(), "kick.wav");
    }

    #[test]
    fn add_pattern_node() {
        let mut p = Patch::new("test");
        let pat = euclidean(3, 8);
        let id = p.add_pattern(pat, "rhythm");
        assert_eq!(p.get_node(id).unwrap().label(), "rhythm");
    }

    #[test]
    fn add_effect_node() {
        let mut p = Patch::new("test");
        let id = p.add_effect(effect::reverb(4.0, 0.5));
        assert_eq!(p.get_node(id).unwrap().label(), "Reverb");
    }

    #[test]
    fn connect_source_to_effect() {
        let mut p = Patch::new("test");
        let src = p.add_source(Source::sample("pad.wav"));
        let fx = p.add_effect(effect::reverb(3.0, 0.4));
        p.connect(src, 0, fx, 0);
        assert_eq!(p.edge_count(), 1);
        assert!(p.is_valid());
    }

    #[test]
    fn source_effect_chain() {
        let mut p = Patch::new("test");
        let src = p.add_source(Source::synth("Analog"));
        let filt = p.add_effect(effect::lowpass(2000.0, 0.5));
        let rev = p.add_effect(effect::reverb(3.0, 0.4));
        p.connect(src, 0, filt, 0);
        p.connect(filt, 0, rev, 0);
        assert_eq!(p.edge_count(), 2);
        assert!(p.is_valid());
    }

    #[test]
    fn mixer_node() {
        let mut p = Patch::new("test");
        let s1 = p.add_source(Source::sample("a.wav"));
        let s2 = p.add_source(Source::sample("b.wav"));
        let mix = p.add_mixer(2, "main mix");
        p.connect(s1, 0, mix, 0);
        p.connect(s2, 0, mix, 1);
        assert!(p.is_valid());
    }

    #[test]
    fn split_node() {
        let mut p = Patch::new("test");
        let src = p.add_source(Source::sample("vocal.wav"));
        let split = p.add_split(2);
        let fx1 = p.add_effect(effect::reverb(2.0, 0.5));
        let fx2 = p.add_effect(effect::delay(0.375, 0.3, 0.5));
        p.connect(src, 0, split, 0);
        p.connect(split, 0, fx1, 0);
        p.connect(split, 1, fx2, 0);
        assert!(p.is_valid());
    }

    #[test]
    fn expose_param() {
        let mut p = Patch::new("test");
        p.expose_param(Param::new("density", (0.0, 1.0), 0.5));
        p.expose_param(Param::new("brightness", (0.0, 1.0), 0.7));
        assert_eq!(p.params().len(), 2);
        assert!(p.params().get("density").is_some());
    }

    #[test]
    fn set_space() {
        let mut p = Patch::new("test");
        p.set_space(Space::panned(-0.5));
        let (pan, _, _) = p.space().sample(0.0);
        assert!((pan - -0.5).abs() < 1e-9);
    }

    // --- Validation errors ---------------------------------------------------

    #[test]
    fn validate_dangling_node() {
        let mut p = Patch::new("test");
        p.add_source(Source::sample("x.wav"));
        // Manually push an edge referencing a non-existent node
        p.edges.push(Edge {
            from_node: 0,
            from_port: 0,
            to_node: 99,
            to_port: 0,
        });
        let errors = p.validate();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], PatchError::DanglingNode { node_id: 99, .. }));
    }

    #[test]
    fn validate_dangling_port() {
        let mut p = Patch::new("test");
        let src = p.add_source(Source::sample("x.wav"));
        let fx = p.add_effect(effect::reverb(1.0, 0.5));
        // Port 5 doesn't exist on the source
        p.connect(src, 5, fx, 0);
        let errors = p.validate();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], PatchError::DanglingPort { port_id: 5, .. }));
    }

    #[test]
    fn validate_port_type_mismatch() {
        let mut p = Patch::new("test");
        // Pattern node outputs PortType::Pattern
        let pat = p.add_pattern(euclidean(3, 8), "rhythm");
        // Effect expects PortType::Audio input
        let fx = p.add_effect(effect::reverb(1.0, 0.5));
        p.connect(pat, 0, fx, 0);
        let errors = p.validate();
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            errors[0],
            PatchError::PortTypeMismatch {
                from_type: PortType::Pattern,
                to_type: PortType::Audio,
                ..
            }
        ));
    }

    #[test]
    fn validate_self_loop() {
        let mut p = Patch::new("test");
        let fx = p.add_effect(effect::reverb(1.0, 0.5));
        p.connect(fx, 0, fx, 0);
        let errors = p.validate();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], PatchError::SelfLoop { .. }));
    }

    // --- Composition ---------------------------------------------------------

    #[test]
    fn series_composition() {
        let mut a = Patch::new("source");
        a.add_source(Source::sample("pad.wav"));

        let mut b = Patch::new("effects");
        b.add_effect(effect::reverb(3.0, 0.5));

        let composed = series(&a, &b, "combined");
        assert_eq!(composed.node_count(), 2);
        // Should have one edge connecting source → reverb
        assert_eq!(composed.edge_count(), 1);
        assert!(composed.is_valid());
    }

    #[test]
    fn series_chain() {
        let mut a = Patch::new("source");
        a.add_source(Source::synth("Wavetable"));

        let mut b = Patch::new("filter");
        b.add_effect(effect::lowpass(2000.0, 0.5));

        let mut c = Patch::new("reverb");
        c.add_effect(effect::reverb(4.0, 0.3));

        let ab = series(&a, &b, "filtered");
        let abc = series(&ab, &c, "full chain");
        assert_eq!(abc.node_count(), 3);
        assert_eq!(abc.edge_count(), 2);
        assert!(abc.is_valid());
    }

    #[test]
    fn parallel_composition() {
        let mut a = Patch::new("kick");
        a.add_source(Source::sample("kick.wav"));

        let mut b = Patch::new("snare");
        b.add_source(Source::sample("snare.wav"));

        let composed = parallel(&a, &b, "drums");
        // 2 sources + 1 merge node
        assert_eq!(composed.node_count(), 3);
        // 2 edges: each source → merge
        assert_eq!(composed.edge_count(), 2);
        assert!(composed.is_valid());
    }

    #[test]
    fn parallel_then_series() {
        let mut kick = Patch::new("kick");
        kick.add_source(Source::sample("kick.wav"));

        let mut snare = Patch::new("snare");
        snare.add_source(Source::sample("snare.wav"));

        let drums = parallel(&kick, &snare, "drums");

        let mut bus_fx = Patch::new("bus");
        bus_fx.add_effect(effect::compressor(-15.0, 4.0));

        let composed = series(&drums, &bus_fx, "drum bus");
        // 2 sources + merge + compressor = 4
        assert_eq!(composed.node_count(), 4);
        assert!(composed.is_valid());
    }

    #[test]
    fn feedback_adds_edge() {
        let mut p = Patch::new("test");
        let src = p.add_source(Source::synth("Operator"));
        let dly = p.add_effect(effect::delay(0.375, 0.5, 0.3));
        p.connect(src, 0, dly, 0);
        feedback(&mut p, dly, src);
        // Note: feedback creates a type mismatch (effect audio out → source which has no input)
        // In practice, the compiler would handle this via Ableton routing
        assert_eq!(p.edge_count(), 2);
    }

    // --- Full patch example --------------------------------------------------

    #[test]
    fn design_doc_style_patch() {
        let mut p = Patch::new("water_braid");

        // Sources
        let water = p.add_source(Source::sample_labeled("water.wav", "water grains"));
        let glass = p.add_source(Source::sample_labeled("glass.wav", "glass grains"));

        // Patterns
        let _rhythm = p.add_pattern(
            euclidean(5, 8).rotate(2),
            "main rhythm",
        );

        // Effects
        let rev = p.add_effect(effect::reverb(4.0, 0.5));
        let dly = p.add_effect(effect::delay(0.375, 0.4, 0.3));

        // Mixer
        let mix = p.add_mixer(2, "submix");
        p.connect(water, 0, mix, 0);
        p.connect(glass, 0, mix, 1);
        p.connect(mix, 0, rev, 0);
        p.connect(rev, 0, dly, 0);

        // Control surface
        p.expose_param(Param::new("density", (0.0, 1.0), 0.3));
        p.expose_param(Param::new("brightness", (0.0, 1.0), 0.6));

        // Space
        p.set_space(
            Space::center()
                .with_pan(crate::signal::Signal::sine(0.03))
                .with_depth(crate::signal::Signal::Const(0.5)),
        );

        assert_eq!(p.node_count(), 6); // 2 sources + 1 pattern + 2 effects + 1 mixer
        assert_eq!(p.edge_count(), 4);
        assert_eq!(p.params().len(), 2);

        // Only audio edges should be valid (pattern node isn't connected to audio)
        assert!(p.is_valid());
    }
}
