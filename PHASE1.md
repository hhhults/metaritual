# Phase 1: Pattern in Rust — COMPLETE

Port the discrete algebra from `techniques/base.py` and `techniques/sequencing.py` into the Rust DSL core. No Ableton dependency. No Python compiler. Just the pure algebra with tests.

**Status: All 9 milestones implemented. 146 tests passing.**

## What you can write now

```rust
use metaritual::pattern::Pattern;
use metaritual::construct::*;
use metaritual::combine::{combine, Combine};

let p = euclidean(5, 8)
    .rotate(2)
    .degrade(0.3, 42)
    .swing(0.08)
    .every(3, |p| p.rotate(1));

let melody = Pattern::seq(&[60.0, 64.0, 67.0, 72.0])
    .retrograde()
    .shift(12.0);

let combined = combine(&p, &melody, Combine::Cycle);

// Compile to IR (JSON for the Python compiler)
let clip = metaritual::ir::compile(&p, 4, 120.0, 4.0);
println!("{}", clip.to_json());
```

## Milestones

### 1. Pattern core (`src/pattern.rs`) — DONE

- [x] `Pattern::Empty`, `Atom(f64, f64)`, `Seq(Vec<Pattern>)`, `Stack(Vec<Pattern>)`
- [x] `Pattern::duration()` — total cycle time
- [x] `Pattern::events()` — flatten to `Vec<Event>` with absolute start times
- [x] `Pattern::is_empty()`, `Pattern::len()`
- [x] Constructors: `atom()`, `seq()`, `seq_with_duration()`, `silence()`
- [x] `Display` impl — `_`, `~`, `[a b c]`, `{a, b, c}`
- [x] `PartialEq` by event list comparison (epsilon-aware)

### 2. Operators (`src/pattern.rs`) — DONE

- [x] `Add` — concat with Seq flattening
- [x] `BitOr` — stack with Stack flattening
- [x] `Mul<usize>` — repeat n times

### 3. Transforms (`src/transform.rs`) — DONE

**Deterministic:**
- [x] `.rotate(k)` — cycle values, keep timing
- [x] `.retrograde()` — reverse in time
- [x] `.shift(n)` — transpose
- [x] `.invert(axis)` — reflect around axis
- [x] `.stretch(factor)` — time scaling
- [x] `.thin(keep_every)` — keep every Nth event
- [x] `.every(n, f)` — apply f every nth cycle
- [x] `.with_values(f)` — general value mapping

**Stochastic (uses internal xorshift64 PRNG, no `rand` dep):**
- [x] `.degrade(probability, seed)` — randomly drop events
- [x] `.swing(amount)` — off-beat delay
- [x] `.humanize(timing, velocity, seed)` — random jitter

### 4. Constructors (`src/construct.rs`) — DONE

- [x] `euclidean(hits, steps)` — Bjorklund algorithm
- [x] `euclidean_rotated(hits, steps, rotation)`
- [x] `scale(root, intervals, octaves)` — sequential scale walk
- [x] `chord(root, intervals, duration)` — simultaneous stack
- [x] `stacked_fourths(root, count)`, `stacked_fifths(root, count)`
- [x] 11 scale constants: MAJOR, MINOR, DORIAN, PHRYGIAN, LYDIAN, MIXOLYDIAN, PENTATONIC, MINOR_PENTATONIC, BLUES, WHOLE_TONE, CHROMATIC
- [x] 10 chord constants: MAJOR_TRIAD, MINOR_TRIAD, DIM, AUG, MAJOR_7, MINOR_7, DOM_7, SUS2, SUS4, ADD9

### 5. Combine (`src/combine.rs`) — DONE

- [x] `Combine::Zip` — pair 1:1, truncate to shorter
- [x] `Combine::Cycle` — shorter cycles to match longer
- [x] `Combine::Polymetric` — nearest-in-time pairing

### 6. Markov (`src/markov.rs`) — DONE

- [x] `Markov::new(states, transitions)` — explicit transitions
- [x] `Markov::from_pattern(pattern)` — learn from events
- [x] `Markov::from_weights(states, weights)` — auto-normalize
- [x] `.generate(length, duration, start_state, seed)` → `Pattern`

### 7. Accumulator (`src/accumulator.rs`) — DONE

- [x] `Accumulator::new(seed, transforms)` — boxed closures
- [x] `.generate(iterations)` → concatenated evolution
- [x] `.generate_layered(iterations, voices, voice_offset)` → `Vec<Pattern>`

### 8. Algebraic tests — DONE (proptest)

15 property-based tests via proptest, plus inline unit tests:
- [x] Associativity of concat
- [x] Rotate identity + inverse
- [x] Retrograde involution
- [x] Shift linearity
- [x] Invert involution
- [x] Stretch identity
- [x] Euclidean hit count
- [x] Degrade bounds (0.0 = identity, 1.0 = empty)

### 9. IR serialization (`src/ir.rs`) — DONE

- [x] `IrEvent`, `IrClip`, `IrTrack`, `IrSession` types
- [x] `compile(pattern, cycles, bpm, beats_per_cycle)` → `IrClip`
- [x] `.to_json()` on all IR types (manual, no serde dep)
- [x] Cycle-to-beat conversion, NaN filtering, multi-cycle

## Test summary

| Module | Tests |
|---|---|
| pattern.rs | 22 |
| transform.rs | 35 |
| construct.rs | 18 |
| combine.rs | 11 |
| markov.rs | 22 |
| accumulator.rs | 15 |
| rng.rs | 4 |
| ir.rs | 15 |
| **Total** | **146** |

## Files

```
src/
  lib.rs          — module declarations
  pattern.rs      — Pattern enum, Event, operators, Display, PartialEq
  transform.rs    — 11 transforms (8 deterministic + 3 stochastic)
  construct.rs    — euclidean, scale/chord builders, constants
  combine.rs      — Zip/Cycle/Polymetric combination
  markov.rs       — first-order Markov chain generation
  accumulator.rs  — iterative pattern evolution (ecstatic computation)
  rng.rs          — xorshift64 PRNG (internal, no rand dep)
  ir.rs           — IR types + JSON serialization + compile()
```

## Notes

- No external dependencies (rand/serde had SIGKILL on macOS build scripts). Internal xorshift64 PRNG and manual JSON serialization used instead. Can swap in rand/serde later.
- `cargo test` binary needs ad-hoc code signing on macOS: `codesign -s - target/debug/deps/metaritual-*`
- Silence/rest represented as `Atom(f64::NAN, duration)` — filtered by `events()`, preserved structurally.
- `from_events` helper rebuilds flat Seq from event list (loses NaN atoms). `thin` and `degrade` build Seq directly to preserve rests.

## What's next: Phase 2

Signal and Param in Rust. Define continuous types, the bridge operations (render, analyze), corpus as `&mut` state. Still no Ableton connection.
