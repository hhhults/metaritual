# metaritual

A compositional algebra for music. Pure Rust, no audio runtime.

metaritual gives you a small set of types and operations for building music programmatically — patterns, signals, harmony, synthesis routing — that compile to Ableton Live sessions via JSON IR.

## Core idea

Two kinds of musical data, bridged by explicit morphisms:

- **Pattern** (discrete) — events in time: notes, rhythms, triggers. Time is cycle-relative (0.0 to 1.0), independent of tempo.
- **Signal** (continuous) — trajectories through time: automation curves, LFOs, audio envelopes.

Everything else builds on these. Chords are patterns. Automation is a signal. A synth patch is a routing graph connecting patterns to signals to effects.

## Patterns

```rust
use metaritual::{pattern, construct, transform};

// From a scale
let p = construct::scale(60, construct::DORIAN, 2, 0.25);

// From euclidean rhythm
let p = construct::euclidean(7, 16, 60, 0.25);

// From a chord
let p = construct::chord(60, construct::MINOR_7, 4.0);

// Transform
let p = transform::rotate(&p, 3);
let p = transform::transpose(&p, 5);
let p = transform::degrade(&p, 0.2, 42);
let p = transform::swing(&p, 0.33, 0.5);

// Combine
let seq = pattern::concat(&p1, &p2);     // sequential
let stack = pattern::stack(&p1, &p2);     // simultaneous
let repeated = pattern::repeat(&p, 4);    // loop
```

NaN represents silence — `Atom(f64::NAN, duration)` is a rest that preserves rhythmic structure.

## Signals

```rust
use metaritual::signal::{Signal, WaveShape};

let lfo = Signal::lfo(WaveShape::Sine, 2.0, 0.2, 0.8);  // sine, 2 cycles, range 0.2-0.8
let curve = Signal::curve(vec![
    (0.0, 0.0), (0.3, 1.0), (0.7, 0.8), (1.0, 0.2)
]);
let constant = Signal::constant(0.5);

// Evaluate at any point in time
let value = lfo.sample(0.25);

// Arithmetic
let combined = &lfo + &curve;
let scaled = &lfo * &Signal::constant(0.5);
```

## Bridges

Explicit morphisms between discrete and continuous:

```rust
use metaritual::bridge;

// Pattern → Signal (extract pitch contour as automation curve)
let curve = bridge::render(&pattern, 256);

// Signal → Pattern (detect onsets, generate trigger events)
let triggers = bridge::onset_detect(&signal, 0.5, 0.25);

// Sample and hold (Signal → stepped Pattern)
let stepped = bridge::sample_and_hold(&signal, 16, 1.0);
```

## Harmony

Voice-leading graph with parsimonious operations:

```rust
use metaritual::harmony::{Chord, ChordKind, walk, path, orbit, PLR};

// Random walk through chord space
let chords = walk(
    Chord::new(0, ChordKind::Major7),
    8,     // steps
    1,     // max voice-leading distance
    42,    // seed
);

// Find smoothest path between two chords
let path = path(
    Chord::new(0, ChordKind::Major7),
    Chord::new(6, ChordKind::MinMaj7),
    true,  // smoothest
);

// Neo-Riemannian PLR orbit (triadic)
let orbit = orbit(
    Chord::new(0, ChordKind::Major),
    &[PLR::P, PLR::L, PLR::R],
);
```

## Voicing

```rust
use metaritual::voicing;

// Staggered bloom voicing
let p = voicing::bloom(60, &[0, 3, 7, 10], 0.1, 4.0);

// Drop-2 open voicing
let p = voicing::open(60, &[0, 4, 7, 11], 4.0);

// Wide orchestral spread
let p = voicing::wide(60, &[0, 3, 7, 10], 2, 4.0);

// Voice-led progression (minimal movement between chords)
let p = voicing::voice_led(&roots, &intervals, 48, 72, 4.0);
```

## Generative

```rust
use metaritual::markov::MarkovChain;
use metaritual::accumulator::Accumulator;

// Markov melody
let chain = MarkovChain::from_scale(60, construct::DORIAN, 2);
let melody = chain.generate(32, 0.25, 42);

// Accumulator — iterative pattern evolution
let evolved = Accumulator::new(seed_pattern)
    .push(|p| transform::rotate(p, 2))
    .push(|p| transform::transpose(p, 5))
    .push(|p| transform::degrade(p, 0.15, 0))
    .generate(12);  // 12 iterations of accumulated mutation
```

## Routing graphs

Patches describe how sources, effects, and spaces connect:

```rust
use metaritual::{patch::Patch, source::Source, effect, space::Space};

let mut patch = Patch::new();

let src = patch.add_source(Source::synth("Analog", params));
let fx1 = patch.add_effect(effect::reverb(0.6, 0.8));
let fx2 = patch.add_effect(effect::delay(0.375, 0.4));
let mix = patch.add_mixer();

patch.connect(src, fx1);
patch.connect(fx1, fx2);
patch.connect(fx2, mix);

// Compile to JSON IR
let ir = metaritual::ir::compile_patch(&patch, bpm);
println!("{}", serde_json::to_string_pretty(&ir).unwrap());
```

## Synthesis presets

```rust
use metaritual::synth;

synth::liquid_metal();    // Operator FM bass
synth::sub_bass();        // Analog sine sub
synth::crystalline();     // Non-integer ratio FM lead
synth::elastic_glass();   // Collision resonant pad
synth::subatomic();       // Wavetable micro-texture
synth::fm_bubble();       // High-feedback FM experiment
```

## Rhythm

```rust
use metaritual::rhythm;

let fill = rhythm::fill(60, 4, 0.5);          // accelerating fill
let cross = rhythm::cross_rhythm(3, 60);       // polyrhythmic
let roll = rhythm::roll(60, 8, 0.125, 0.5);   // drum roll with decay
```

## Testing

337 tests (321 unit + 16 property-based via proptest):

```bash
cargo test
```

Property tests verify algebraic invariants — rotation identity, retrograde involution, concatenation associativity.

## Architecture

```
metaritual (pure algebra, no I/O)
    ↓ JSON IR
ableton-rs (OSC transport)
    ↓ OSC
AbletonOSC (Remote Script)
    ↓
Ableton Live
```

metaritual has zero I/O dependencies by default. The optional `play` feature adds direct Ableton playback via [ableton-rs](https://github.com/hhhults/ableton-rs).

## Building

```bash
cargo build
cargo test

# With live playback support
cargo build --features play
```

## License

MIT
