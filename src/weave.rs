//! Weave — global compositional structure.
//!
//! A Weave is a diagram of named strands (voices with lifecycles) and their
//! interactions (crossings) over time. All time values are normalized to
//! [0.0, 1.0] relative to the weave's duration. Compiles to a sorted event
//! timeline for execution.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Envelope ───────────────────────────────────────────────────────────────

/// Activation lifecycle for a strand.
///
/// Times are normalized [0.0, 1.0] relative to the weave duration.
/// The strand is silent before `enter`, ramps up during `attack`,
/// holds during `sustain`, fades during `release`, then silent again.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Envelope {
    /// When the strand begins (0.0–1.0).
    pub enter: f64,
    /// Duration of the attack ramp (proportion of weave).
    pub attack: f64,
    /// Duration of the sustain plateau (proportion of weave).
    pub sustain: f64,
    /// Duration of the release fade (proportion of weave).
    pub release: f64,
}

impl Envelope {
    /// When the strand fully exits (clamped to 1.0).
    pub fn exit(&self) -> f64 {
        (self.enter + self.attack + self.sustain + self.release).min(1.0)
    }

    /// Is this strand sounding at time t?
    pub fn active(&self, t: f64) -> bool {
        t >= self.enter && t < self.exit()
    }

    /// Activation level at time t (0.0 = silent, 1.0 = full).
    pub fn level(&self, t: f64) -> f64 {
        if t < self.enter {
            return 0.0;
        }
        let rel = t - self.enter;
        if rel < self.attack {
            // Attack ramp: 0 → 1
            if self.attack == 0.0 {
                1.0
            } else {
                rel / self.attack
            }
        } else if rel < self.attack + self.sustain {
            // Sustain plateau
            1.0
        } else if rel < self.attack + self.sustain + self.release {
            // Release fade: 1 → 0
            if self.release == 0.0 {
                0.0
            } else {
                1.0 - (rel - self.attack - self.sustain) / self.release
            }
        } else {
            0.0
        }
    }

    /// Phase name at time t.
    pub fn phase(&self, t: f64) -> &str {
        if t < self.enter {
            return "silent";
        }
        let rel = t - self.enter;
        if rel < self.attack {
            "attack"
        } else if rel < self.attack + self.sustain {
            "sustain"
        } else if rel < self.attack + self.sustain + self.release {
            "release"
        } else {
            "silent"
        }
    }
}

// ─── Trajectory ─────────────────────────────────────────────────────────────

/// How a parameter evolves over normalized time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Trajectory {
    /// Fixed value throughout.
    Constant { value: f64 },
    /// Linear ramp from one value to another.
    Ramp { from: f64, to: f64 },
    /// Brownian motion with deterministic seed.
    Drunk {
        range: (f64, f64),
        step: f64,
        seed: u64,
    },
    /// Sinusoidal oscillation.
    Sine {
        range: (f64, f64),
        cycles: f64,
    },
    /// Piecewise linear curve defined by breakpoints (t, value).
    Curve { points: Vec<(f64, f64)> },
    /// Follow another strand's parameter (resolved externally).
    Follow {
        strand: String,
        param: String,
        lag: f64,
    },
}

impl Trajectory {
    /// Evaluate the trajectory at normalized time t (0.0–1.0).
    pub fn eval(&self, t: f64) -> f64 {
        match self {
            Trajectory::Constant { value } => *value,

            Trajectory::Ramp { from, to } => {
                let t_clamped = t.clamp(0.0, 1.0);
                from + (to - from) * t_clamped
            }

            Trajectory::Drunk {
                range,
                step,
                seed,
            } => {
                // Deterministic drunk walk: discretize time into steps,
                // iterate from midpoint using seed-derived pseudo-random choices.
                let mid = (range.0 + range.1) / 2.0;
                let t_clamped = t.clamp(0.0, 1.0);
                // Number of steps based on step size as fraction of range
                let range_size = (range.1 - range.0).abs();
                if range_size == 0.0 {
                    return mid;
                }
                let n_steps = (t_clamped / step.max(0.001)).floor() as u64;
                let mut value = mid;
                let mut hash = *seed;
                for _ in 0..n_steps {
                    // Simple deterministic hash (splitmix64-style)
                    hash = hash.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    let direction = if (hash >> 32) & 1 == 0 { 1.0 } else { -1.0 };
                    let step_size = step * range_size;
                    value = (value + direction * step_size).clamp(range.0, range.1);
                }
                value
            }

            Trajectory::Sine { range, cycles } => {
                let t_clamped = t.clamp(0.0, 1.0);
                let phase = t_clamped * cycles * 2.0 * std::f64::consts::PI;
                let unit = (phase.sin() + 1.0) / 2.0; // 0..1
                range.0 + (range.1 - range.0) * unit
            }

            Trajectory::Curve { points } => {
                if points.is_empty() {
                    return 0.0;
                }
                if points.len() == 1 {
                    return points[0].1;
                }
                let t_clamped = t.clamp(0.0, 1.0);
                // Before first point
                if t_clamped <= points[0].0 {
                    return points[0].1;
                }
                // After last point
                if t_clamped >= points[points.len() - 1].0 {
                    return points[points.len() - 1].1;
                }
                // Linear interpolation between surrounding breakpoints
                for i in 0..points.len() - 1 {
                    let (t0, v0) = points[i];
                    let (t1, v1) = points[i + 1];
                    if t_clamped >= t0 && t_clamped <= t1 {
                        if (t1 - t0).abs() < f64::EPSILON {
                            return v0;
                        }
                        let frac = (t_clamped - t0) / (t1 - t0);
                        return v0 + (v1 - v0) * frac;
                    }
                }
                points[points.len() - 1].1
            }

            Trajectory::Follow { .. } => {
                // Follow requires external resolution; return midpoint as placeholder.
                0.5
            }
        }
    }
}

// ─── Strand ─────────────────────────────────────────────────────────────────

/// A named voice with lifecycle and parameter trajectories.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Strand {
    pub name: String,
    pub envelope: Envelope,
    #[serde(default)]
    pub trajectories: HashMap<String, Trajectory>,
}

// ─── CrossingKind ───────────────────────────────────────────────────────────

/// How strands interact at a crossing point.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CrossingKind {
    /// Material transfers from one strand to another.
    Handoff,
    /// Roles swap between strands.
    Crossfade,
    /// Rhythms synchronize.
    Entrain,
    /// Shared material diverges.
    Diverge,
    /// Different material converges.
    Converge,
    /// One strand echoes another with delay.
    Echo,
}

// ─── Crossing ───────────────────────────────────────────────────────────────

/// An interaction between strands at a time window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Crossing {
    pub kind: CrossingKind,
    pub strands: Vec<String>,
    pub window: (f64, f64),
}

// ─── Movement ───────────────────────────────────────────────────────────────

/// An arc segment — a named section of the compositional trajectory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Movement {
    pub name: String,
    pub span: (f64, f64),
    pub energy: Trajectory,
}

// ─── WeaveAction ────────────────────────────────────────────────────────────

/// Event types emitted by compiling a Weave.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum WeaveAction {
    StrandEnter { strand: String },
    StrandFull { strand: String },
    StrandRelease { strand: String },
    StrandExit { strand: String },
    CrossingBegin { kind: CrossingKind, strands: Vec<String> },
    CrossingEnd { kind: CrossingKind, strands: Vec<String> },
    Movement { name: String },
}

// ─── WeaveEvent ─────────────────────────────────────────────────────────────

/// A compiled event at a specific normalized time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WeaveEvent {
    pub t: f64,
    pub action: WeaveAction,
}

// ─── Weave ──────────────────────────────────────────────────────────────────

/// Top-level compositional structure — a diagram of strands and crossings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Weave {
    pub duration_bars: u32,
    #[serde(default)]
    pub arc: Vec<Movement>,
    pub strands: Vec<Strand>,
    #[serde(default)]
    pub crossings: Vec<Crossing>,
}

impl Weave {
    /// Which strands are active at normalized time t.
    pub fn active_strands(&self, t: f64) -> Vec<&Strand> {
        self.strands
            .iter()
            .filter(|s| s.envelope.active(t))
            .collect()
    }

    /// Which arc movement contains normalized time t.
    pub fn current_movement(&self, t: f64) -> Option<&Movement> {
        self.arc.iter().find(|m| t >= m.span.0 && t < m.span.1)
    }

    /// Crossings active at normalized time t.
    pub fn active_crossings(&self, t: f64) -> Vec<&Crossing> {
        self.crossings
            .iter()
            .filter(|c| t >= c.window.0 && t < c.window.1)
            .collect()
    }

    /// Compile the weave into a sorted event timeline.
    pub fn compile(&self) -> Vec<WeaveEvent> {
        let mut events = Vec::new();

        // Strand lifecycle events
        for s in &self.strands {
            let env = &s.envelope;
            events.push(WeaveEvent {
                t: env.enter,
                action: WeaveAction::StrandEnter {
                    strand: s.name.clone(),
                },
            });
            if env.attack > 0.0 {
                let full_t = (env.enter + env.attack).min(1.0);
                events.push(WeaveEvent {
                    t: full_t,
                    action: WeaveAction::StrandFull {
                        strand: s.name.clone(),
                    },
                });
            } else {
                // Instant attack: full immediately
                events.push(WeaveEvent {
                    t: env.enter,
                    action: WeaveAction::StrandFull {
                        strand: s.name.clone(),
                    },
                });
            }
            let release_t = (env.enter + env.attack + env.sustain).min(1.0);
            if env.release > 0.0 {
                events.push(WeaveEvent {
                    t: release_t,
                    action: WeaveAction::StrandRelease {
                        strand: s.name.clone(),
                    },
                });
            }
            events.push(WeaveEvent {
                t: env.exit(),
                action: WeaveAction::StrandExit {
                    strand: s.name.clone(),
                },
            });
        }

        // Crossing events
        for c in &self.crossings {
            events.push(WeaveEvent {
                t: c.window.0,
                action: WeaveAction::CrossingBegin {
                    kind: c.kind.clone(),
                    strands: c.strands.clone(),
                },
            });
            events.push(WeaveEvent {
                t: c.window.1,
                action: WeaveAction::CrossingEnd {
                    kind: c.kind.clone(),
                    strands: c.strands.clone(),
                },
            });
        }

        // Movement events
        for m in &self.arc {
            events.push(WeaveEvent {
                t: m.span.0,
                action: WeaveAction::Movement {
                    name: m.name.clone(),
                },
            });
        }

        // Sort by time, stable
        events.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
        events
    }

    /// First event after normalized time t.
    pub fn next_event(&self, t: f64) -> Option<WeaveEvent> {
        let events = self.compile();
        events.into_iter().find(|e| e.t > t)
    }

    /// All events in the window [t, t + window].
    pub fn upcoming(&self, t: f64, window: f64) -> Vec<WeaveEvent> {
        let events = self.compile();
        events
            .into_iter()
            .filter(|e| e.t >= t && e.t <= t + window)
            .collect()
    }

    /// Sequential composition: self followed by other with optional overlap.
    ///
    /// Rescales self to [0, split] and other to [split - overlap_norm, 1.0],
    /// where the overlap region allows crossfading between the two weaves.
    pub fn compose(self, other: Weave, overlap: f64) -> Weave {
        let total_bars = self.duration_bars + other.duration_bars;
        let self_frac = self.duration_bars as f64 / total_bars as f64;
        let other_frac = other.duration_bars as f64 / total_bars as f64;
        let overlap_norm = overlap.min(self_frac.min(other_frac));

        let split = self_frac;
        let other_start = split - overlap_norm;

        // Rescale self strands to [0, split]
        let mut strands: Vec<Strand> = self
            .strands
            .into_iter()
            .map(|s| Strand {
                name: s.name,
                envelope: rescale_envelope(&s.envelope, 0.0, split),
                trajectories: s.trajectories,
            })
            .collect();

        // Rescale other strands to [other_start, 1.0]
        let other_strands: Vec<Strand> = other
            .strands
            .into_iter()
            .map(|s| Strand {
                name: s.name,
                envelope: rescale_envelope(&s.envelope, other_start, 1.0),
                trajectories: s.trajectories,
            })
            .collect();
        strands.extend(other_strands);

        // Rescale self arc
        let mut arc: Vec<Movement> = self
            .arc
            .into_iter()
            .map(|m| Movement {
                name: m.name,
                span: (m.span.0 * split, m.span.1 * split),
                energy: m.energy,
            })
            .collect();

        // Rescale other arc
        let other_range = 1.0 - other_start;
        let other_arc: Vec<Movement> = other
            .arc
            .into_iter()
            .map(|m| Movement {
                name: m.name,
                span: (
                    other_start + m.span.0 * other_range,
                    other_start + m.span.1 * other_range,
                ),
                energy: m.energy,
            })
            .collect();
        arc.extend(other_arc);

        // Rescale self crossings
        let mut crossings: Vec<Crossing> = self
            .crossings
            .into_iter()
            .map(|c| Crossing {
                kind: c.kind,
                strands: c.strands,
                window: (c.window.0 * split, c.window.1 * split),
            })
            .collect();

        // Rescale other crossings
        let other_crossings: Vec<Crossing> = other
            .crossings
            .into_iter()
            .map(|c| Crossing {
                kind: c.kind,
                strands: c.strands,
                window: (
                    other_start + c.window.0 * other_range,
                    other_start + c.window.1 * other_range,
                ),
            })
            .collect();
        crossings.extend(other_crossings);

        Weave {
            duration_bars: total_bars,
            arc,
            strands,
            crossings,
        }
    }
}

/// Rescale an envelope from [0,1] to [start, end].
fn rescale_envelope(env: &Envelope, start: f64, end: f64) -> Envelope {
    let range = end - start;
    Envelope {
        enter: start + env.enter * range,
        attack: env.attack * range,
        sustain: env.sustain * range,
        release: env.release * range,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_lifecycle() {
        let env = Envelope {
            enter: 0.2,
            attack: 0.1,
            sustain: 0.4,
            release: 0.1,
        };

        // exit = 0.2 + 0.1 + 0.4 + 0.1 = 0.8
        assert!((env.exit() - 0.8).abs() < 1e-10);

        // Before enter: silent
        assert!((env.level(0.1) - 0.0).abs() < 1e-10);
        assert_eq!(env.phase(0.1), "silent");

        // Mid-attack: t=0.25, rel=0.05, 0.05/0.1 = 0.5
        assert!((env.level(0.25) - 0.5).abs() < 1e-10);
        assert_eq!(env.phase(0.25), "attack");

        // Sustain: t=0.5, rel=0.3, attack=0.1 -> in sustain
        assert!((env.level(0.5) - 1.0).abs() < 1e-10);
        assert_eq!(env.phase(0.5), "sustain");

        // Release phase: release starts at 0.2+0.1+0.4=0.7, ends at 0.8
        assert_eq!(env.phase(0.75), "release");
        // level at 0.75: rel=0.55, past sustain by 0.05, release=0.1, so 1.0 - 0.05/0.1 = 0.5
        assert!((env.level(0.75) - 0.5).abs() < 1e-10, "level at 0.75: {}", env.level(0.75));

        // After exit: silent
        assert!((env.level(0.85) - 0.0).abs() < 1e-10);

        // Active checks
        assert!(!env.active(0.1));
        assert!(env.active(0.25));
        assert!(env.active(0.5));
        assert!(!env.active(0.85));
    }

    #[test]
    fn trajectory_ramp() {
        let traj = Trajectory::Ramp {
            from: 0.0,
            to: 1.0,
        };
        assert!((traj.eval(0.0) - 0.0).abs() < 1e-10);
        assert!((traj.eval(0.5) - 0.5).abs() < 1e-10);
        assert!((traj.eval(1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn trajectory_sine() {
        let traj = Trajectory::Sine {
            range: (0.0, 1.0),
            cycles: 1.0,
        };
        // t=0: sin(0) = 0 -> unit = 0.5
        assert!((traj.eval(0.0) - 0.5).abs() < 1e-10);
        // t=0.25: sin(π/2) = 1 -> unit = 1.0 -> value = 1.0
        assert!((traj.eval(0.25) - 1.0).abs() < 1e-10);
        // t=0.5: sin(π) ≈ 0 -> unit = 0.5
        assert!((traj.eval(0.5) - 0.5).abs() < 1e-6);
        // t=0.75: sin(3π/2) = -1 -> unit = 0.0 -> value = 0.0
        assert!((traj.eval(0.75) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn trajectory_curve() {
        let traj = Trajectory::Curve {
            points: vec![(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)],
        };
        assert!((traj.eval(0.0) - 0.0).abs() < 1e-10);
        assert!((traj.eval(0.25) - 0.5).abs() < 1e-10);
        assert!((traj.eval(0.5) - 1.0).abs() < 1e-10);
        assert!((traj.eval(0.75) - 0.5).abs() < 1e-10);
        assert!((traj.eval(1.0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn trajectory_constant() {
        let traj = Trajectory::Constant { value: 0.42 };
        assert!((traj.eval(0.0) - 0.42).abs() < 1e-10);
        assert!((traj.eval(0.5) - 0.42).abs() < 1e-10);
        assert!((traj.eval(1.0) - 0.42).abs() < 1e-10);
    }

    #[test]
    fn trajectory_drunk_deterministic() {
        let traj = Trajectory::Drunk {
            range: (0.0, 1.0),
            step: 0.1,
            seed: 42,
        };
        // Same seed and time should produce same result
        let v1 = traj.eval(0.5);
        let v2 = traj.eval(0.5);
        assert!((v1 - v2).abs() < 1e-10);
        // Should be within range
        assert!(v1 >= 0.0 && v1 <= 1.0);
    }

    fn make_test_strand(name: &str, enter: f64, attack: f64, sustain: f64, release: f64) -> Strand {
        Strand {
            name: name.to_string(),
            envelope: Envelope {
                enter,
                attack,
                sustain,
                release,
            },
            trajectories: HashMap::new(),
        }
    }

    #[test]
    fn weave_active_strands() {
        let weave = Weave {
            duration_bars: 32,
            arc: vec![],
            strands: vec![
                make_test_strand("pad", 0.0, 0.1, 0.6, 0.1),     // 0.0–0.8
                make_test_strand("lead", 0.3, 0.05, 0.3, 0.05),  // 0.3–0.7
                make_test_strand("bass", 0.6, 0.05, 0.2, 0.05),  // 0.6–0.9
            ],
            crossings: vec![],
        };

        // t=0.05: only pad
        let active = weave.active_strands(0.05);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "pad");

        // t=0.5: pad + lead
        let active = weave.active_strands(0.5);
        assert_eq!(active.len(), 2);
        let names: Vec<&str> = active.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"pad"));
        assert!(names.contains(&"lead"));

        // t=0.65: pad + lead + bass
        let active = weave.active_strands(0.65);
        assert_eq!(active.len(), 3);

        // t=0.85: only bass
        let active = weave.active_strands(0.85);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "bass");

        // t=0.95: nobody
        let active = weave.active_strands(0.95);
        assert_eq!(active.len(), 0);
    }

    #[test]
    fn weave_compile() {
        let weave = Weave {
            duration_bars: 16,
            arc: vec![Movement {
                name: "intro".to_string(),
                span: (0.0, 0.5),
                energy: Trajectory::Ramp {
                    from: 0.0,
                    to: 1.0,
                },
            }],
            strands: vec![make_test_strand("pad", 0.0, 0.1, 0.3, 0.1)],
            crossings: vec![Crossing {
                kind: CrossingKind::Handoff,
                strands: vec!["pad".to_string(), "lead".to_string()],
                window: (0.4, 0.6),
            }],
        };

        let events = weave.compile();

        // Should have: StrandEnter, StrandFull, StrandRelease, StrandExit,
        //              CrossingBegin, CrossingEnd, Movement = 7
        assert_eq!(events.len(), 7);

        // First event should be at t=0.0 (StrandEnter or Movement)
        assert!((events[0].t - 0.0).abs() < 1e-10);

        // Events should be sorted
        for i in 1..events.len() {
            assert!(events[i].t >= events[i - 1].t);
        }

        // Verify specific events exist
        let has_enter = events
            .iter()
            .any(|e| matches!(&e.action, WeaveAction::StrandEnter { strand } if strand == "pad"));
        assert!(has_enter);

        let has_crossing_begin = events
            .iter()
            .any(|e| matches!(&e.action, WeaveAction::CrossingBegin { kind: CrossingKind::Handoff, .. }));
        assert!(has_crossing_begin);

        let has_movement = events
            .iter()
            .any(|e| matches!(&e.action, WeaveAction::Movement { name } if name == "intro"));
        assert!(has_movement);
    }

    #[test]
    fn weave_compose() {
        let w1 = Weave {
            duration_bars: 16,
            arc: vec![Movement {
                name: "intro".to_string(),
                span: (0.0, 1.0),
                energy: Trajectory::Constant { value: 0.5 },
            }],
            strands: vec![make_test_strand("pad", 0.0, 0.1, 0.7, 0.2)],
            crossings: vec![],
        };

        let w2 = Weave {
            duration_bars: 16,
            arc: vec![Movement {
                name: "outro".to_string(),
                span: (0.0, 1.0),
                energy: Trajectory::Constant { value: 0.3 },
            }],
            strands: vec![make_test_strand("lead", 0.0, 0.1, 0.7, 0.2)],
            crossings: vec![],
        };

        let composed = w1.compose(w2, 0.1);

        // Duration should sum
        assert_eq!(composed.duration_bars, 32);

        // Should have both strands
        assert_eq!(composed.strands.len(), 2);
        let names: Vec<&str> = composed.strands.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"pad"));
        assert!(names.contains(&"lead"));

        // Should have both arc movements
        assert_eq!(composed.arc.len(), 2);
        let arc_names: Vec<&str> = composed.arc.iter().map(|m| m.name.as_str()).collect();
        assert!(arc_names.contains(&"intro"));
        assert!(arc_names.contains(&"outro"));

        // Pad strand should be in first half (rescaled to [0, 0.5])
        let pad = composed.strands.iter().find(|s| s.name == "pad").unwrap();
        assert!(pad.envelope.enter < 0.5);

        // Lead strand should start in second half region
        let lead = composed.strands.iter().find(|s| s.name == "lead").unwrap();
        assert!(lead.envelope.enter >= 0.4); // starts at other_start = 0.5 - 0.1 = 0.4
    }

    #[test]
    fn weave_serde_roundtrip() {
        let weave = Weave {
            duration_bars: 32,
            arc: vec![
                Movement {
                    name: "build".to_string(),
                    span: (0.0, 0.5),
                    energy: Trajectory::Ramp {
                        from: 0.0,
                        to: 1.0,
                    },
                },
                Movement {
                    name: "peak".to_string(),
                    span: (0.5, 1.0),
                    energy: Trajectory::Constant { value: 1.0 },
                },
            ],
            strands: vec![
                Strand {
                    name: "pad".to_string(),
                    envelope: Envelope {
                        enter: 0.0,
                        attack: 0.1,
                        sustain: 0.6,
                        release: 0.2,
                    },
                    trajectories: {
                        let mut m = HashMap::new();
                        m.insert(
                            "filter".to_string(),
                            Trajectory::Drunk {
                                range: (0.2, 0.8),
                                step: 0.05,
                                seed: 42,
                            },
                        );
                        m.insert(
                            "volume".to_string(),
                            Trajectory::Sine {
                                range: (0.3, 0.7),
                                cycles: 4.0,
                            },
                        );
                        m
                    },
                },
                Strand {
                    name: "lead".to_string(),
                    envelope: Envelope {
                        enter: 0.3,
                        attack: 0.05,
                        sustain: 0.4,
                        release: 0.1,
                    },
                    trajectories: {
                        let mut m = HashMap::new();
                        m.insert(
                            "pitch".to_string(),
                            Trajectory::Curve {
                                points: vec![(0.0, 60.0), (0.5, 72.0), (1.0, 60.0)],
                            },
                        );
                        m
                    },
                },
            ],
            crossings: vec![Crossing {
                kind: CrossingKind::Crossfade,
                strands: vec!["pad".to_string(), "lead".to_string()],
                window: (0.25, 0.35),
            }],
        };

        let json = serde_json::to_string_pretty(&weave).expect("serialize");
        let roundtripped: Weave = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(weave, roundtripped);
    }

    #[test]
    fn weave_next_event_and_upcoming() {
        let weave = Weave {
            duration_bars: 16,
            arc: vec![],
            strands: vec![
                make_test_strand("a", 0.0, 0.0, 0.4, 0.1),
                make_test_strand("b", 0.5, 0.0, 0.3, 0.1),
            ],
            crossings: vec![],
        };

        // Next event after t=0.3 should be strand a's release or exit
        let next = weave.next_event(0.3);
        assert!(next.is_some());
        assert!(next.unwrap().t > 0.3);

        // Upcoming in [0.0, 0.1] should catch strand a's enter and full
        let upcoming = weave.upcoming(0.0, 0.1);
        assert!(!upcoming.is_empty());
        assert!(upcoming.iter().all(|e| e.t >= 0.0 && e.t <= 0.1));
    }

    #[test]
    fn weave_current_movement() {
        let weave = Weave {
            duration_bars: 32,
            arc: vec![
                Movement {
                    name: "intro".to_string(),
                    span: (0.0, 0.25),
                    energy: Trajectory::Constant { value: 0.3 },
                },
                Movement {
                    name: "build".to_string(),
                    span: (0.25, 0.75),
                    energy: Trajectory::Ramp { from: 0.3, to: 1.0 },
                },
                Movement {
                    name: "outro".to_string(),
                    span: (0.75, 1.0),
                    energy: Trajectory::Ramp { from: 1.0, to: 0.0 },
                },
            ],
            strands: vec![],
            crossings: vec![],
        };

        assert_eq!(weave.current_movement(0.1).unwrap().name, "intro");
        assert_eq!(weave.current_movement(0.5).unwrap().name, "build");
        assert_eq!(weave.current_movement(0.9).unwrap().name, "outro");
        assert!(weave.current_movement(1.0).is_none());
    }

    #[test]
    fn weave_active_crossings() {
        let weave = Weave {
            duration_bars: 16,
            arc: vec![],
            strands: vec![],
            crossings: vec![
                Crossing {
                    kind: CrossingKind::Handoff,
                    strands: vec!["a".to_string(), "b".to_string()],
                    window: (0.2, 0.4),
                },
                Crossing {
                    kind: CrossingKind::Entrain,
                    strands: vec!["b".to_string(), "c".to_string()],
                    window: (0.5, 0.7),
                },
            ],
        };

        assert_eq!(weave.active_crossings(0.3).len(), 1);
        assert_eq!(weave.active_crossings(0.3)[0].kind, CrossingKind::Handoff);
        assert_eq!(weave.active_crossings(0.6).len(), 1);
        assert_eq!(weave.active_crossings(0.45).len(), 0);
    }
}
