# Phase 2: Signal and Param in Rust — COMPLETE

Define continuous types, bridge operations (render, analyze), corpus as `&mut` state. Still no Ableton connection.

**Status: All 6 milestones implemented. 75 new tests (238 total).**

## Milestones

### 1. Signal types (`src/signal.rs`) — DONE

- [x] `WaveShape` enum: Sine, Triangle, Square, Saw
- [x] `Breakpoint { time: f64, value: f64 }` for piecewise curves
- [x] `Signal` enum: Const, Curve, LFO, Sum, Product, Map
- [x] `Signal::sample(time: f64) -> f64` — evaluate at a point in time
- [x] `Signal::sample_n(start, end, n) -> Vec<f64>` — evaluate at N points
- [x] Signal arithmetic: `Add`, `Mul` for signal combination
- [x] Constructor helpers: `Signal::sine(rate)`, `Signal::triangle(rate)`, etc.
- [x] `Signal::lfo(shape, rate, phase, center, depth)` — full control
- [x] `Signal::scale(scale, offset)` — affine transform
- [x] `Signal::map(fn, label)` — arbitrary function mapping

### 2. Param (`src/param.rs`) — DONE

- [x] `Param { name, range, default, signal }` — named bounded dimension
- [x] `Param::new(name, range, default)` — starts as Const(default)
- [x] `Param::set(signal)` — drive with any signal
- [x] `Param::reset()` — return to default
- [x] `Param::sample(time)` — delegates to signal, clamps to range
- [x] `Param::sample_n()` — batch sampling, clamped
- [x] `ParamSet` — collection of named params (the control surface)

### 3. Bridge: render (`src/bridge.rs`) — DONE

- [x] `render(pattern, beats_per_cycle) -> Signal` — Pattern → step-function curve
- [x] `render_smooth(pattern, beats_per_cycle) -> Signal` — Pattern → interpolated curve

### 4. Bridge: analyze (`src/bridge.rs`) — DONE

- [x] `onset_detect(signal, duration, threshold, resolution) -> Pattern` — threshold crossing → rhythm
- [x] `sample_and_hold(signal, pattern, beats_per_cycle) -> Pattern` — sample signal at event times
- [x] `quantize(signal, steps, beats_per_cycle) -> Pattern` — regular sampling to pattern

### 5. Corpus (`src/corpus.rs`) — DONE

- [x] `Features` — arbitrary-dimension feature vector with Euclidean distance
- [x] `Grain { id, source, start, duration, features }` — analyzed audio fragment
- [x] `Corpus` with `&mut` semantics
- [x] `corpus.ingest()` — add new material with auto-ID
- [x] `corpus.get(id)` — lookup by ID
- [x] `corpus.query(target, n) -> Vec<&Grain>` — nearest-neighbor lookup
- [x] `corpus.query_pattern(targets, grain_duration) -> Pattern` — build pattern from matches
- [x] `corpus.remove(id)`, `corpus.remove_source()`, `corpus.clear()`

### 6. Tests — DONE

- [x] 35 signal tests (const, curve interpolation, LFO wave shapes, arithmetic, map)
- [x] 10 param tests (clamping, delegation, ParamSet)
- [x] 14 bridge tests (render, onset detection, sample_and_hold, quantize)
- [x] 16 corpus tests (ingest, query, nearest-neighbor, multidimensional)

## Test summary

| Module | Tests |
|---|---|
| signal.rs | 35 |
| param.rs | 10 |
| bridge.rs | 14 |
| corpus.rs | 16 |
| **Phase 2 total** | **75** |
| **Cumulative** | **238** |

## What's next: Phase 3

Patch in Rust. Define the routing graph. Patch composition (series, parallel, feedback). Port compatibility checking. Exposed params as the control surface.
