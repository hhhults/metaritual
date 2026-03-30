# metaritual: A Compositional Semantic Musical Instrument

*Design document — living spec for the DSL, algebra, and architecture.*
*Harper + Claude, March 2026*

---

## What this is

metaritual is a domain-specific language for describing, composing, and playing musical instruments as open systems. An instrument in metaritual is not a synthesizer or a sampler — it's a typed surface with a control manifold, where anything that produces values in the right shape can perform.

metaritual compiles to Ableton Live. Live is the rendering backend, not the ontology.

The name: a ritual is a structured enactment. A metaritual is a structure for generating rituals — a system that creates the conditions for transformative experience, every time different, every time alive.

## Architecture

```
┌─────────────────────────────────────────────────┐
│  metaritual DSL (Rust)                            │
│  Pure algebra. No Ableton dependency.           │
│  Patterns, signals, patches, theories, models.  │
└──────────────────────┬──────────────────────────┘
                       │ serialized IR (JSON/msgpack)
┌──────────────────────▼──────────────────────────┐
│  Compiler (Python, for now)                     │
│  Translates IR → ableton.py calls.              │
│  Uses existing Session/Track/Clip/Device API.   │
└──────────────────────┬──────────────────────────┘
                       │ OSC (ports 11000/11001)
┌──────────────────────▼──────────────────────────┐
│  AbletonOSC Remote Script (forked, Python)      │
│  browser.py, automation.py, return_track.py     │
└──────────────────────┬──────────────────────────┘
                       │ Live API
┌──────────────────────▼──────────────────────────┐
│  Ableton Live 11.3                              │
└─────────────────────────────────────────────────┘
```

Bidirectional path (future):

```
Ableton Live → state listener → Rust DSL state update
```

The key principle: the Rust layer knows nothing about Ableton. It produces abstract descriptions. The Python layer knows nothing about the algebra. It executes concrete actions. They meet at the IR boundary.

## Core types

The DSL has two fundamental *kinds* of data, bridged by explicit morphisms.

### Discrete: `Pattern`

A `Pattern` is a time structure carrying `f64` values. It is the combinatorial layer — the world of rhythm, melody, sequencing, arrangement.

```rust
enum Pattern {
    Empty,                                  // silence / rest
    Atom(f64, Time),                        // a single event: value + duration
    Seq(Vec<Pattern>),                      // concatenation in time
    Stack(Vec<Pattern>),                    // superposition (simultaneous)
    Transform(Box<Pattern>, Box<dyn Xform>), // lazy transformation
}
```

**Why `f64`, not generics.** A pattern carries floats. A pitch of 60.0 (MIDI C4), a velocity of 0.8, a filter cutoff of 0.55 — they're all just numbers. This means you can pipe any pattern into any slot without type gymnastics. If you need to distinguish pitch from velocity semantically, use naming at the Param level, not at the Pattern level. Start maximally flexible; add type distinctions only when they earn their keep in practice.

**Time is cycle-relative.** Following TidalCycles, time is expressed as a fraction of a cycle (0.0 to 1.0). An `Atom` with duration 0.25 takes up a quarter of a cycle. A `Seq` of four atoms of equal duration fills one cycle. When compiled to Ableton, cycle duration converts to beats via BPM. This keeps the pattern algebra independent of tempo.

```rust
// One cycle = one "bar" by default. BPM sets the clock.
type Time = f64;  // 0.0..1.0 within a cycle, can exceed 1.0 for multi-cycle
```

Pattern is **recursive and self-similar**. A rhythm is a pattern of 1.0s and 0.0s. A melody is a pattern of MIDI note numbers. A phrase is a pattern of patterns. An arrangement is a pattern of scenes. The algebra doesn't change when you zoom in or out.

**The embedded DSL.** metaritual uses an embedded Rust DSL: method chaining, operator overloads, named functions. This is designed to be natural for both Harper and Claude to write and reason about.

```rust
// Operator overloads for the core monoidal operations
impl Add for Pattern { /* concat: a + b */ }
impl BitOr for Pattern { /* stack: a | b */ }
impl Mul<usize> for Pattern { /* repeat: p * 4 */ }

// Method chaining for transforms (endomorphisms)
let p = euclidean(5, 8)
    .rotate(2)
    .degrade(0.3)
    .swing(0.08)
    .every(3, |p| p.rotate(1));
```

Operations on Pattern (all work at every scale):

- `a + b` — concat: `a` then `b`
- `a | b` — stack: `a` and `b` simultaneously
- `p * n` — repeat `n` times
- `.rotate(k)` — cycle the sequence by `k` positions
- `.retrograde()` — reverse in time
- `.shift(n)` — add `n` to all values (transpose when values are pitches)
- `.invert(axis)` — reflect values around axis
- `.stretch(factor)` — time scaling
- `.degrade(prob)` — randomly drop events
- `.swing(amount)` — shift off-beat events
- `.humanize(amount)` — add random timing jitter
- `.every(n, f)` — apply transform `f` every nth cycle

These compose. A chain of transforms is a chain of endomorphisms.

**Combining patterns.** Since patterns are just floats, "combining rhythm and pitch" is combining two patterns according to a strategy:

```rust
enum Combine {
    Zip,                        // pair them 1:1
    Cycle,                      // shorter one cycles to match longer
    Polymetric,                 // independent lengths, align at start
    Stochastic(Distribution),   // random pairing
}

// A rhythm (pattern of onsets) and a melody (pattern of pitches)
// become a pattern of (onset, pitch) pairs — rendered as Note events
fn combine(a: &Pattern, b: &Pattern, how: Combine) -> Vec<(f64, f64)>;
```

The `how` parameter is the morphism. Different choices produce different music from the same material.

### Continuous: `Signal`

A `Signal` is a trajectory through time — a function from time to value. Audio streams, automation curves, LFOs, envelope followers, control voltages.

```rust
struct Signal {
    kind: SignalKind,
    range: (f64, f64),      // bounds on value
    label: Option<String>,  // semantic name: "brightness", "density", etc.
}

enum SignalKind {
    Const(f64),                          // static value (a knob at rest)
    Curve(Vec<Breakpoint>),              // piecewise automation
    LFO { shape: WaveShape, rate: f64 }, // generated oscillation
    Audio(AudioSource),                  // actual audio stream
    Derived(Box<Signal>, SignalFn),      // transformed from another signal
    External(InputBinding),              // from outside: guitar, game, webcam, etc.
}
```

The crucial insight: **a parameter IS a signal, not a scalar.** Setting a knob to 0.7 is `Signal::Const(0.7)`. Drawing an automation curve is `Signal::Curve(...)`. An LFO is `Signal::LFO(...)`. A guitar envelope follower is `Signal::External(...)`. They all flow into the same slot.

This means automation is zero-cost conceptually — you don't "add automation to a parameter." A parameter was always a signal. You just decide what kind.

### Bridges between discrete and continuous

These are the morphisms that cross the boundary:

**`trigger`: (Pattern, Pattern, Source) → Signal**
A rhythm pattern, a pitch pattern, and a sound source produce an audio signal. The patterns provide discrete events; the source provides audio material. This is what Simpler/a sampler does.

**`render`: Pattern → Signal**
Evaluate a pattern into a continuous signal. A step sequence of parameter values becomes a `Signal::Curve`. The pattern's cycle-relative time gets converted to real time via BPM.

**`analyze`: Signal → Pattern or Signal → Signal**
Extract structure from audio. Onset detection gives a pattern (rhythm). Pitch tracking gives a pattern (melody). Envelope following gives a Signal. Feature extraction (brightness, noisiness) gives a Signal. This is how guitar input enters the discrete world.

**`resample`: Signal → Source**
The trace. Collapse audio output into a new atomic sample. This is the fold that lets the system eat itself.

### Source

The atom of sonic material.

```rust
enum Source {
    Sample(FilePath),           // a .wav file on disk
    LiveInput(InputChannel),    // guitar, mic, line in
    Synth(SynthParams),         // a synthesizer preset
    Resampled(PatchId),         // output of another patch, captured
}
```

Sources are what get attached to events. A Source doesn't know about time — it's material, not structure.

### Space

Spatial position as its own type, not just another parameter.

```rust
struct Space {
    pan: Signal,        // stereo position
    width: Signal,      // stereo spread
    depth: Signal,      // distance / reverb send
}
```

Space gets its own type because it maps into multiple output domains: stereo audio (pan/width), visual rendering (x/y position), physical space (speaker array). The same spatial description can drive all of them via different functors.

### Param

A named, bounded, semantic dimension in the control manifold.

```rust
struct Param {
    name: String,           // "density", "brightness", "attack"
    range: (f64, f64),      // valid bounds
    default: f64,           // rest position
    signal: Signal,         // what's currently driving it
}
```

A `Param` is the atom of the control surface. Params are what knobs bind to, what games drive, what I (Claude) can read and suggest changes to.

## Patch: the routing diagram

A `Patch` is a wiring diagram — a directed graph where nodes are processing stages and edges are signal flow.

```rust
struct Patch {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    exposed_params: Vec<ParamId>,   // the control surface
    inputs: Vec<InputPort>,          // where external signal enters
    outputs: Vec<OutputPort>,        // where signal leaves
}

enum Node {
    SourceNode(Source),
    PatternNode(Pattern),
    EffectNode(Effect),
    MixerNode(MixParams),
    AnalysisNode(AnalysisType),
    SplitNode,                      // one signal to many
    MergeNode,                      // many signals to one
}
```

A Patch is an **open system**. It has typed inputs and outputs — the interface. Two patches compose when their interfaces match: plug the output of one into the input of another.

The `exposed_params` are the control surface. They're what makes the patch *playable*. Everything that produces Signals (your hands on MIDI knobs, a game's state, a webcam feature extractor, me suggesting values) connects to the patch through these exposed params.

### Patch composition

Patches compose in several ways:

- **Series**: output of A feeds input of B.
- **Parallel**: A and B run simultaneously, outputs merge.
- **Feedback**: output of A routes back to input of A (the hot trace).
- **Substitution**: replace a node inside a patch with an entire sub-patch (operadic composition).

This is where the categorical structure earns its keep. Composition is typed — you can only connect things whose interfaces match. The DSL checks this statically.

## The Trace

Two kinds, both first-class:

### Hot trace (live resampling)

Audio-rate feedback inside Ableton. A track's input set to "Resampling" or to another track's output. The signal never leaves Live. Set up via routing in the Patch:

```rust
// In the Patch graph: output -> delay/effect -> input (feedback loop)
patch.add_feedback(output_node, effect_chain, input_node, feedback_gain);
```

Compiles to: create a return track, set up routing, arm recording. The degradation/accumulation happens continuously in real time.

### Cold trace (corpus resampling)

Discrete render-analyze-reingest cycle. Output gets rendered to audio, analyzed for features, chopped into grains, re-enters the system as new Source material in a corpus.

```rust
// Resample the output, analyze it, add to corpus
let new_source = resample(patch.output, duration);
corpus.ingest(new_source, analysis_features);
```

This is your existing `ResamplingPipeline` — self-feeding corpus, cross-feeding, multi-feature feed across generations. It stays, but now it's a named operation in the DSL rather than a Python script.

## Functors: interpreting into target spaces

The Patch's control manifold is abstract. **Functors** interpret it into concrete domains:

- **Sonic functor**: control state → Ableton actions (the compiler). This is the one that makes sound.
- **Visual functor**: control state → graphics (WebGL, p5.js, etc.). Same structure, different rendering.
- **Lighting functor**: control state → DMX values. Same control surface drives lights.
- **Data functor**: control state → logged values. For analysis, replay, machine learning.

These are automatically synchronized because they share the same source. The visualization isn't reactive FFT on the audio — it's a *sibling interpretation* of the same compositional structure.

## Bindings: who's performing

The control surface (exposed params) accepts input from anything that can produce signals:

```rust
enum Binding {
    Midi(MidiMapping),           // hardware knob, fader, pad
    OSC(OscAddress),             // external OSC source
    Audio(AnalysisFn),           // audio input → signal (envelope follower, etc.)
    GameState(StateExtractor),   // game engine state → signal
    Webcam(FeatureExtractor),    // camera feed → signal
    Claude(ParamSuggestion),    // me, suggesting values
    Internal(SignalId),          // another signal in the patch
}
```

Binding is hot-swappable. The patch doesn't know or care who's driving its params. This is what makes it a meta-instrument: define the surface once, perform from anywhere.

## Mapping to existing modules

| DSL concept | Existing module | Status |
|---|---|---|
| `Pattern` | `sequencing.py` — Pattern, Markov, Accumulator | Core algebra exists. Port to Rust with f64 values. |
| `Pattern` transforms | `base.py` — rotate, retrograde, invert, euclidean, degrade | Implemented. Port to Rust. |
| `Pattern` rhythm ops | `rhythm.py` + TidalCycles syntax | Partial. Tidal syntax parser exists. |
| `Source::Sample` | `freesound.py` + `/soundhunt` skill | Working. |
| `Source` loading | `/loadsample` skill, `browser.py` | Working. |
| Granular `Signal` | `granular.py` — GranularLayer, Simpler params | Working. Wraps multiple DSL concepts. |
| Concatenative `Signal` | `concatenative.py` — Corpus, LiveCorpus, mosaic | Working. Implements analysis + trigger. |
| `Signal::Curve` (automation) | `automation.py` — breakpoint envelopes | Working. Batched in groups of 60. |
| `Patch` routing | Return tracks, sends in `feedback.py` | Partial. Implicit in scripts, not declarative. |
| Hot trace | Live resampling routing | Not yet automated. Manual setup. |
| Cold trace | `resampling.py` — ResamplingPipeline | Working. Self-feed, cross-feed, multi-feature. |
| `analyze` | `analysis.py`, `concatenative.py` feature extraction | Working. brightness, loudness, noisiness, zcr, pitch. |
| `/compose` skill | Aesthetic thinking → Patch construction | Exists as prompt, not as DSL. |
| `/mix` skill | MixParams → EQ, compression, spatial | Exists as prompt. |
| `/master` skill | Mastering chain | Exists as prompt. |
| Curves & LFOs | `base.py` — sine, triangle, drunk, stochastic, envelope | Working. Port to Rust as SignalKind. |

## What a session looks like

A sketch of the workflow, not final syntax:

```
// 1. Harper describes a vibe in natural language
"I want something that sounds like sunlight through water.
 Bright granular textures, slow polymetric phasing, 
 the density controlled by my guitar's dynamics."

// 2. Claude + Harper design the patch together
//    (this conversation happens here, in claude.ai)

// 3. The design becomes a metaritual expression:

let water_grains = source::freesound("water bubbling dripping");
let glass_grains = source::freesound("glass resonance singing");

let rhythm = euclidean(5, 8)
    .every(3, |p| p.rotate(1))
    .swing(0.08);

let pitches = scale(Scale::Lydian, 60.0, 72.0)
    .markov(transition_matrix);

let events = combine(&rhythm, &pitches, Combine::Cycle);

let density = param("density", 0.0..1.0, 0.3);
let brightness = param("brightness", 0.0..1.0, 0.6);

let granular_water = granular(water_grains)
    .grain_size(density.signal.map(|d| lerp(20.0, 200.0, d)))
    .grain_density(density.signal)
    .filter_cutoff(brightness.signal);

let granular_glass = granular(glass_grains)
    .grain_size(Signal::lfo(Shape::Sine, 0.1))
    .transpose(Signal::drunk(0.0, 0.05));

let patch = Patch::new()
    .add(trigger(&events, &granular_water))
    .add(trigger(&events.rotate(3), &granular_glass))  // polymetric offset
    .effect(reverb(0.7, 4.0))
    .effect(delay(3.0/8.0, 0.4))
    .space(Space { pan: Signal::lfo(Shape::Sine, 0.03), width: 0.8, depth: 0.5 })
    .expose(density)      // on the control surface
    .expose(brightness)   // on the control surface
    .bind(density, Binding::Audio(envelope_follower(Input::Guitar)));

// 4. Compile and instantiate in Ableton
compile(&patch);  // → Python compiler → OSC → Ableton

// 5. Harper plays guitar. Guitar dynamics drive grain density.
//    Harper tweaks brightness knob. Claude watches the control surface
//    and the output summary (spectral centroid rising, onset density stable)
//    and suggests:
//    "try modulating brightness with a slow sine, 0.02 Hz?"
//    Harper agrees. Claude updates the binding:

patch.rebind(brightness, Binding::Internal(Signal::lfo(Shape::Sine, 0.02)));
recompile(&patch);  // hot-reload the automation
```

## Resolved decisions

**Syntax: embedded Rust DSL.** Method chaining, operator overloads (`+` for concat, `|` for stack, `*` for repeat), named functions for transforms. Designed to be natural for both Harper and Claude to write and reason about. No external parser, no macro magic. Just fluent Rust.

**Type system: `Pattern` carries `f64`.** Start untyped and maximally flexible. Semantic distinctions (pitch vs. velocity vs. cutoff) live at the Param level via naming, not at the Pattern level via types. Add newtypes later only if they earn their keep in practice.

**Time: cycle-relative.** Following TidalCycles, time is a fraction of a cycle (0.0 to 1.0). The compiler converts to beats/bars using the BPM setting at compilation time. The algebra never thinks in absolute time.

**Corpus: an effect via `&mut`.** The corpus is a mutable database of analyzed samples. Functions that modify it take `&mut Corpus`. Functions that read it take `&Corpus`. Rust's borrow checker enforces the discipline — no monads needed. The corpus lives outside the pure pattern algebra as a side-effecting resource, threaded explicitly through operations that need it.

**Live state: assume fully observable.** Design as though all Live state can be read back. Model the observation surface as a trait, so when reality forces a subset, we implement the trait with what's actually available. The DSL doesn't degrade — it just has less information.

**Compilation: full recompile first, partial later.** Start with full recompile on every change. Partial/hot update is an optimization. The IR should be *structured* so that partial recompilation is possible later (i.e., the IR is a diff-able tree, not a flat list of commands), but the first compiler can ignore this.

**Operadic structure: implicit, not named.** The self-similarity of Pattern and the substitution structure of Patches are operadic. We exploit this in the implementation (patterns of patterns compose by substitution, patches nest) without requiring users to know the word "operad." The algebra just works the way it should.

**What Claude sees.** During a session, Claude receives:
- The full control surface state (all exposed params and their current signal values).
- The Patch structure (what's connected to what).
- A summary of the output stream: peak/RMS levels, spectral centroid, onset density — enough to have a musical conversation about "it's getting muddy" or "the rhythm is too dense" without hearing the audio. The exact feature set is TBD but should be the same features the `analyze` bridge can extract.

## Remaining open questions

**Stochastic operations and determinism.** `degrade(0.3)` and `humanize(0.1)` involve randomness. Should patterns be deterministic (seeded PRNG, same seed → same output every time) or nondeterministic (different every cycle)? TidalCycles uses deterministic pseudorandom keyed by cycle number. This seems right — reproducibility matters, and you can always re-seed for variation.

**Pattern evaluation strategy.** Is a Pattern a *description* (lazy, evaluated at compile time to produce IR) or a *generator* (evaluated at runtime, potentially infinite)? For Ableton, clips are finite, so we need to evaluate patterns into concrete event lists. But for live performance and infinite cycling, we'd want lazy evaluation. Probably: patterns are descriptions, the compiler evaluates them for N cycles at compile time, and looping is handled by Ableton's clip loop.

**Integration with Max for Live.** For the hot trace and for richer bidirectional communication, a Max for Live device could complement abletonosc. Design the DSL so this is a transport option, not an architectural dependency.

**Multi-cycle patterns and arrangement.** If a Scene is a pattern of patterns, and an Arrangement is a pattern of scenes, how deep does the recursion go in practice? Do we need explicit "zoom levels" or does flat recursion suffice?

**Concurrency.** When Harper is playing guitar and Claude is suggesting param changes and the pattern is cycling, what's the concurrency model? Probably: the Rust DSL is single-threaded and produces snapshots, the Python compiler applies them to Live, and Live handles real-time concurrency natively. But this needs more thought.

## Testing

**Layer 1: Algebraic property tests (Rust, automated).**

The pure DSL layer is tested with standard Rust unit tests plus property-based testing via `proptest`. Generate random patterns, verify algebraic laws hold:

```rust
// Associativity of concat
assert_eq!((a + b) + c, a + (b + c));

// Rotate identity
assert_eq!(p.rotate(0), p);
assert_eq!(p.rotate(n).rotate(-n), p);

// Degrade(0.0) is identity, degrade(1.0) is empty
assert_eq!(p.degrade(0.0), p);
assert!(p.degrade(1.0).is_empty());

// Stack is commutative (up to event ordering)
assert_eq!((a | b).events().sorted(), (b | a).events().sorted());
```

These tests run fast, in CI, on every commit. They verify the algebra is sound.

**Layer 2: IR snapshot tests (Rust, automated).**

Compile DSL expressions to IR, snapshot the output, compare:

```rust
let ir = compile_to_ir(
    euclidean(5, 8).rotate(2).stretch(0.5)
);
expect_test::expect![[r#"
    { "events": [ ... ], "duration": 0.5, ... }
"#]].assert_eq(&ir.to_json());
```

Uses `expect-test` or `insta`. If the IR changes, the test fails. You review and accept or reject. This catches unintended regressions in compilation.

**Layer 3: Integration tests (Python + Ableton, manual/CI-with-Ableton).**

A test harness that compiles a known expression, sends it to Live via abletonosc, reads back state, checks structural properties:

```python
compile_and_send(euclidean(5, 8))
assert session.tracks_count() == 1
assert session.tracks[0].clips[0].note_count() == 5
assert session.tempo == 120.0
```

Requires Ableton running. Run manually during development, or in a CI environment with Ableton headless (if possible). Not every commit, but before releases.

**Layer 4: Sonic reference catalog (human-evaluated).**

A collection of small reference pieces (water_braid_mini, glass_voice_mini, etc.) that exercise specific features. After significant changes, compile them all, listen, verify they still sound right. This is the musical regression suite. Not automated — the ear is the test.

## Design principles

**river-shape not water.** The DSL describes the *shape* of the music — its structure, its topology, its control surface. The water (actual audio) flows through at runtime. We design the riverbed.

**spaciousness over speed.** Get the ontology right before optimizing. A wrong abstraction that runs fast is worse than a right abstraction that runs slow. We can always speed up later; we can't easily un-bake a bad model.

**the thing is the verb.** An instrument is not a noun (a fixed object). It's a verb (an ongoing process of shaping sound). The DSL describes *activities*, not *things*. A Pattern is not a list of notes — it's a *way of distributing events in time*.

**you don't have to earn it.** The system should be immediately playable at every stage of development. A single `Source` with a single `Param` is already an instrument. Complexity is always optional, never required.

## Implementation plan

**Phase 0: This document.** Iterate until it says what we mean.

**Phase 1: Pattern in Rust.** Port sequencing.py / base.py to Rust. Define the Pattern enum, operator overloads, transforms, concat/stack. Property-based tests for algebraic laws. This is self-contained and a good way to learn Rust.

**Phase 2: Signal and Param in Rust.** Define the Signal types, the Param model. Write the bridge operations (render, analyze). Corpus as `&mut` state. Still no Ableton connection.

**Phase 3: Patch in Rust.** Define the routing graph. Patch composition (series, parallel, feedback). Port compatibility checking. Exposed params as the control surface.

**Phase 4: IR and compiler.** Define the intermediate representation (what the Rust layer emits as JSON). Write the Python compiler that translates IR → ableton.py calls. Snapshot tests on IR. Integration tests against Live. At this point, we can play things.

**Phase 5: Bidirectional.** State observation from Live. The lens trait. Hot-reload of param bindings. Live state → Rust DSL state sync.

**Phase 6: Functors.** Visual rendering from the same control manifold. Claude's observation stream. The multi-output dream.

Each phase produces something usable. Phase 1 gives you a better pattern library. Phase 4 gives you a playable instrument. Phase 6 gives you the full vision.

---

*Last updated: March 10, 2026*
*This document lives in the musictools repo and evolves with the project.*
