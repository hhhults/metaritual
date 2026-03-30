/// drift — generative ambient piece using markov chains and accumulators.
///
/// A slowly evolving texture built from:
///   - Markov-chain melody wandering through a pentatonic scale
///   - Accumulator-driven bass that gradually shifts and inverts
///   - Euclidean rhythm gating the melody
///   - Deep reverb and gentle delay
///
///     cargo run --example drift
///     cargo run --features play --example drift -- --play

use metaritual::accumulator::Accumulator;
use metaritual::construct::{self, PENTATONIC};
use metaritual::effect;
#[cfg(not(feature = "play"))]
use metaritual::ir::compile_patch;
use metaritual::markov::Markov;
use metaritual::param::Param;
use metaritual::patch::Patch;
use metaritual::pattern::Pattern;
use metaritual::signal::Signal;
use metaritual::source::Source;
use metaritual::space::Space;

fn main() {
    // -- Melody: Markov chain wandering through D pentatonic --
    // D3 pentatonic: D E G A B → MIDI 50 52 55 57 59
    let scale_notes: Vec<f64> = PENTATONIC
        .iter()
        .flat_map(|&interval| {
            // Two octaves: D3 and D4
            vec![50.0 + interval as f64, 62.0 + interval as f64]
        })
        .collect();

    // Build transition weights: prefer stepwise motion, allow occasional leaps
    let n = scale_notes.len();
    let weights: Vec<(f64, Vec<f64>)> = scale_notes
        .iter()
        .enumerate()
        .map(|(i, &note)| {
            let row: Vec<f64> = (0..n)
                .map(|j| {
                    let dist = (i as i32 - j as i32).unsigned_abs() as f64;
                    match dist as u32 {
                        0 => 0.1, // stay (rare)
                        1 => 3.0, // step (common)
                        2 => 1.5, // skip
                        3 => 0.5, // leap
                        _ => 0.2, // distant leap (rare)
                    }
                })
                .collect();
            (note, row)
        })
        .collect();

    let melody_chain = Markov::from_weights(scale_notes, weights);
    let melody = melody_chain.generate(32, 1.0 / 32.0, Some(55.0), 7);

    // Gate the melody with a euclidean rhythm: 7 hits in 16 steps
    // combine() returns (rhythm_val, melody_val) pairs — take the melody pitch
    // where the rhythm hits, building a new pattern
    let rhythm = construct::euclidean(7, 16);
    let pairs = metaritual::combine::combine(
        &rhythm,
        &melody,
        metaritual::combine::Combine::Cycle,
    );
    let dur = 1.0 / pairs.len().max(1) as f64;
    let gated_melody = Pattern::Seq(
        pairs.iter().map(|&(_r, pitch)| Pattern::Atom(pitch, dur)).collect(),
    );

    // -- Bass: accumulator evolving a simple motif --
    // Seed: D2 whole notes with fifths
    let bass_seed = Pattern::Seq(vec![
        Pattern::Atom(38.0, 0.25), // D2
        Pattern::Atom(45.0, 0.25), // A2
        Pattern::Atom(43.0, 0.25), // G2
        Pattern::Atom(38.0, 0.25), // D2
    ]);

    let bass = Accumulator::new(
        bass_seed,
        vec![
            Box::new(|p: &Pattern| p.invert(42.0)),     // reflect around F#2
            Box::new(|p: &Pattern| p.shift(2.0)),        // up a whole step
            Box::new(|p: &Pattern| p.rotate(1)),         // rotate
            Box::new(|p: &Pattern| p.invert(42.0).shift(-2.0)), // reflect + down
        ],
    )
    .generate(4); // 4 iterations = 16 bass notes total

    // -- Pad: sustained chord tones --
    let pad = Pattern::Stack(vec![
        Pattern::Atom(50.0, 1.0), // D3
        Pattern::Atom(55.0, 1.0), // G3
        Pattern::Atom(57.0, 1.0), // A3
        Pattern::Atom(62.0, 1.0), // D4
    ]);

    // -- Build the patch --
    let mut p = Patch::new("drift");

    let lead = p.add_source(Source::synth("Analog"));
    let bass_src = p.add_source(Source::synth("Analog"));
    let pad_src = p.add_source(Source::synth("Analog"));

    p.add_pattern(gated_melody, "melody");
    p.add_pattern(bass, "bass");
    p.add_pattern(pad, "pad");

    // Effects: deep reverb + delay
    let rev = p.add_effect(effect::reverb(6.0, 0.5));
    let dly = p.add_effect(effect::delay(0.375, 0.35, 0.2));

    // Routing: all → mixer → reverb → delay
    let mix = p.add_mixer(3, "main");
    p.connect(lead, 0, mix, 0);
    p.connect(bass_src, 0, mix, 1);
    p.connect(pad_src, 0, mix, 2);
    p.connect(mix, 0, rev, 0);
    p.connect(rev, 0, dly, 0);

    // Control surface
    p.expose_param(Param::new("reverb_depth", (0.0, 1.0), 0.5));
    p.expose_param(Param::new("density", (0.0, 1.0), 0.4));

    // Spatial: slow sine panning, moderate depth
    p.set_space(
        Space::center()
            .with_pan(Signal::sine(0.05)) // very slow pan
            .with_depth(Signal::Const(0.4)),
    );

    // Output
    #[cfg(feature = "play")]
    {
        let config = metaritual::play::PlayConfig {
            bpm: 72.0,
            cycles: 1,
            beats_per_cycle: 4.0,
            automation_resolution: 16,
            autoplay: true,
        };
        metaritual::play::play_or_print(&p, &config);
        return;
    }

    #[cfg(not(feature = "play"))]
    {
        let ir = compile_patch(&p, 72.0, 1, 4.0, 16);
        println!("{}", ir.to_json_pretty());
    }
}
