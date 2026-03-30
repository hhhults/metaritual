# Phase 3: Patch in Rust — COMPLETE

Define the routing graph. Patch composition (series, parallel, feedback). Source and Effect types. Exposed params as the control surface. Space type.

**Status: All 6 milestones implemented. 43 new tests (281 total).**

## Milestones

### 1. Source (`src/source.rs`) — DONE

- [x] `Source` enum: Sample, LiveInput, Synth, Resampled
- [x] `SynthParams` via HashMap<String, f64>
- [x] Source metadata (label, output_channels)
- [x] Constructor helpers: `sample()`, `live_input()`, `synth()`, `resampled()`

### 2. Effect (`src/effect.rs`) — DONE

- [x] `Effect` struct with name, params
- [x] Common effect constructors: reverb, delay, lowpass, highpass, compressor, eq3, saturator
- [x] `EffectChain` — ordered list of effects
- [x] Builder pattern: `Effect::new("x").param(...).param(...)`

### 3. Space (`src/space.rs`) — DONE

- [x] `Space { pan, width, depth }` — all Signal-driven
- [x] `Space::center()`, `Space::panned()`
- [x] Builder: `.with_pan()`, `.with_width()`, `.with_depth()`
- [x] `Space::sample(time)` with clamping

### 4. Patch graph (`src/patch.rs`) — DONE

- [x] `PortType` enum: Audio, Control, Pattern
- [x] `Port { id, port_type, label }`
- [x] `Node` enum: SourceNode, PatternNode, EffectNode, ChainNode, MixerNode, SplitNode, MergeNode
- [x] Each node declares input/output ports
- [x] `Edge { from_node, from_port, to_node, to_port }`
- [x] `Patch { nodes, edges, params, space, label }`
- [x] Builder API: `add_source()`, `add_effect()`, `add_mixer()`, `connect()`
- [x] `expose_param()`, `set_space()`
- [x] Validation: dangling nodes, dangling ports, port type mismatches, self-loops

### 5. Patch composition — DONE

- [x] `series(a, b)` — connect a's outputs to b's inputs
- [x] `parallel(a, b)` — run side by side, merge outputs
- [x] `feedback(patch, from, to)` — add feedback edge

### 6. Tests — DONE

- [x] 6 source tests
- [x] 12 effect tests (constructors, chains, param mutation)
- [x] 6 space tests
- [x] 19 patch tests (building, validation, composition, full example)

## Test summary

| Module | Tests |
|---|---|
| source.rs | 6 |
| effect.rs | 12 |
| space.rs | 6 |
| patch.rs | 19 |
| **Phase 3 total** | **43** |
| **Cumulative** | **281** |

## What's next: Phase 4

IR and compiler. Extend IR to support Patch/Signal/Param types. Write the Python compiler that translates IR → ableton.py calls. Snapshot tests on IR. Integration tests against Live. At this point, we can play things.
