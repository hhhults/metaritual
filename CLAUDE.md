# metaritual

A compositional semantic musical instrument DSL.
Rust algebra → JSON IR → Rust OSC client → Ableton Live.

Also supports: Rust → JSON IR → Python compiler → OSC → Ableton Live (legacy pipeline).

## Architecture

### Phase 1 — Discrete algebra
- `src/pattern.rs` — Core `Pattern` enum (`Empty`, `Atom`, `Seq`, `Stack`) with operators `+` (concat), `|` (stack), `*` (repeat)
- `src/transform.rs` — 11 transforms (shift, invert, retrograde, rotate, stretch, thin, degrade, swing, humanize, every, with_values)
- `src/combine.rs` — Zip, Cycle, Polymetric combination strategies
- `src/construct.rs` — Euclidean rhythms (Bjorklund), scales, chords
- `src/markov.rs` — Markov chain generation
- `src/accumulator.rs` — Iterative pattern evolution
- `src/ir.rs` — JSON IR compilation (Pattern -> IrClip/IrTrack/IrSession)

### Phase 2 — Continuous types
- `src/signal.rs` — Signal enum (Const, Curve, LFO, Sum, Product, Map) with `sample(time)` evaluation
- `src/param.rs` — Param (named, bounded, Signal-driven) and ParamSet (control surface)
- `src/bridge.rs` — render (Pattern→Signal), onset_detect (Signal→Pattern), sample_and_hold, quantize
- `src/corpus.rs` — Corpus (mutable grain database with nearest-neighbor query)

### Phase 3 — Routing
- `src/source.rs` — Source enum (Sample, LiveInput, Synth, Resampled)
- `src/effect.rs` — Effect with named params, common constructors (reverb, delay, filter, etc.), EffectChain
- `src/space.rs` — Space (pan, width, depth as Signals)
- `src/patch.rs` — Patch graph (nodes, edges, ports with type checking), composition (series, parallel, feedback)

### Phase 3.5 — Synthesis presets
- `src/synth.rs` — Named synthesis recipes as `Source::Synth` constructors
  - Bass: `liquid_metal()` (Operator FM), `sub_bass()` (Analog sine)
  - Percussion: `synth_kick()` (Analog pitch envelope), `synth_perc()` (Analog noise+filter)
  - Lead: `crystalline()` (Operator non-integer FM ratios)
  - Pad: `elastic_glass()` (Collision dual resonator)
  - Texture: `subatomic()` (Wavetable ultra-short envelope for micro-notes)
  - Experimental: `fm_bubble()` (Operator high-feedback SOPHIE-style)
- Params use Ableton names (Operator) or index-as-string keys (Analog/Collision/Wavetable)
- Compiler (`ableton-rs/src/compiler.rs`) now applies synth params after loading instrument

### Phase 3.6 — Rhythm generators & voicing
- `src/rhythm.rs` — Rhythm generator functions building on Pattern
  - `cross_rhythm(beats, value)` — polyrhythmic events (3-against-4, quintuplets)
  - `cross_euclidean(hits, beats, value)` — euclidean pattern at cross-rhythm subdivision
  - `fill(value, count, duration)` — accelerating fill (decreasing gaps)
  - `decel(value, count, duration)` — decelerating fill (increasing gaps)
  - `roll(value, count, duration)` — even repeated hits
  - `delayed(pattern, amount)` — prepend silence to offset a pattern
  - `every_other(pattern)` — replace odd events with silence
  - `polymetric(a_beats, a_value, b_beats, b_value)` — two cross-rhythms stacked
- `src/voicing.rs` — Chord voicing and voice leading
  - `bloom(root, intervals, spread, duration)` — staggered chord entries (flower-opening)
  - `open(root, intervals, duration)` — drop inner voices down octave (drop-2 style)
  - `wide(root, intervals, octave_span, duration)` — distribute across multiple octaves
  - `bloom_open(root, intervals, spread, duration)` — open voicing with stagger
  - `progression(chords, total_duration)` — sequential chord progression
  - `bloom_progression(chords, spread, total_duration)` — blooming chord progression
  - `voice_led(chords, total_duration)` — minimal voice movement between chords
  - Uses same interval convention as `construct.rs` (absolute offsets from root)

### Phase 4 — Full pipeline (Rust → JSON IR → Python → Ableton)
- `src/ir.rs` — Extended with `IrPatch`, `IrNode`, `IrEdge`, `IrSpace`, `IrExposedParam`, `compile_patch()`, `compile_signal()`
- `examples/water_braid.rs` — Standalone example that outputs IR JSON to stdout
- `compiler/` — Python uv project:
  - `compiler/src/metaritual_compiler/ir.py` — Python dataclasses mirroring all Rust IR types with `from_dict()` deserializers
  - `compiler/src/metaritual_compiler/compiler.py` — Compiles IrPatch → Ableton Live via ableton.py + OSC
  - `compiler/src/metaritual_compiler/cli.py` — CLI (`metaritual-compile`) reads IR JSON from file/stdin, supports `--dry-run` and `--pretty`
  - `compiler/tests/test_ir.py` — 25 tests including 7 cross-language round-trip tests (Rust→JSON→Python)

### Phase 5 — Direct Rust→Ableton playback
- `src/play.rs` — `play()` and `play_or_print()` functions (behind `play` feature flag)
- `Cargo.toml` — optional `ableton` dependency via `features = ["play"]`
- Usage: `cargo run --features play --example twinkle -- --play`
- Falls back to JSON IR output when `--play` is not passed

### Phase 6 — Generative composition examples
- `examples/drift.rs` — Ambient: Markov melody over pentatonic, accumulator bass, euclidean gating (72 BPM)
- `examples/pulse.rs` — Rhythmic: polymetric euclidean drums, accumulated Am7 arpeggio lead, chord progression (118 BPM)
- `examples/scatter.rs` — Stochastic: time-seeded randomness, degraded rhythms, humanized timing (85 BPM, different every run)

### Phase 7 — Live coding / hot-reload
- `src/bin/live.rs` — File watcher binary: watches examples + src, recompiles on save, incremental Ableton update
- Uses `LiveSession` from ableton-rs for incremental updates
- Pattern-only changes rewrite clips in-place (near-instant); structural changes do full teardown/rebuild
- Usage: `cargo run --features play --bin live -- drift`

### ableton-rs — Standalone Rust OSC client (`../ableton-rs/`)
- `src/osc.rs` — UDP/OSC transport with background receiver thread
- `src/session.rs` — Session (transport, tempo, tracks, browser)
- `src/track.rs` — Track (properties, clips, devices)
- `src/clip.rs` — Clip (MIDI notes, automation, properties)
- `src/device.rs` — Device (parameters)
- `src/compiler.rs` — Full IR→Ableton compiler (replaces Python compiler)
- `src/live.rs` — `LiveSession` with incremental update support (change detection, clips-only fast path)
- `src/bin/compile.rs` — CLI: `cargo run --bin compile` reads IR JSON from stdin

## Key design decisions

- **Cycle-relative time**: All timing is 0.0..1.0 within a cycle, converted to beats via BPM at IR compilation
- **NaN-as-silence**: `Atom(f64::NAN, duration)` represents rests; `events()` filters NaN but duration is preserved
- **Seq derives start times from cumulative duration**: Don't shift start times directly — adjust durations instead (e.g., swing)
- **Stochastic ops use `rand::rngs::SmallRng`** with explicit seeds for determinism
- **Patch is a typed graph**: Ports have types (Audio, Control, Pattern). Validation catches mismatches.
- **Corpus uses `&mut` semantics**: Rust's borrow checker enforces safe concurrent access.

## Build notes

- macOS sandbox SIGKILLs unsigned binaries. Fixed with `~/.cargo/bin/cc-sign.sh` linker wrapper (see `~/.cargo/config.toml`)
- Dependencies: `rand 0.9`, `serde 1` (with derive), `serde_json 1`, `proptest 1` (dev)

## Testing

- 323 Rust tests (307 unit + 15 proptest + 1 doctest), 25 Python tests, 11 ableton-rs tests — 359 total, zero warnings
- Rust: `cargo test`
- ableton-rs: `cd ../ableton-rs && cargo test`
- Python: `cd compiler && uv run pytest tests/ -v`
- End-to-end (Python): `cargo run --example water_braid | metaritual-compile --dry-run`
- End-to-end (Rust): `cargo run --example twinkle | (cd ../ableton-rs && cargo run --bin compile -- --dry-run)`
- Direct play: `cargo run --features play --example twinkle -- --play`
- Live coding: `cargo run --features play --bin live -- drift`
- **Never run `cargo test` in the background** when proptest/fuzzing tests are present — a hanging test can consume unbounded memory. Always use foreground with a timeout.
