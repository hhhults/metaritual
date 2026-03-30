/// pulse — minimal rhythmic piece using euclidean patterns and layered accumulation.
///
/// Polymetric rhythms that phase in and out:
///   - Kick: E(4,16) — four-on-the-floor
///   - Hi-hat: E(7,16) — syncopated
///   - Snare: E(3,8) — tresillo on the backbeat
///   - Bass: E(5,16) with pitch from minor scale
///   - Lead: accumulated melody that shifts each bar
///
///     cargo run --example pulse
///     cargo run --features play --example pulse -- --play

use metaritual::accumulator::Accumulator;
use metaritual::construct::{self, MINOR, chord, MINOR_7};
use metaritual::effect;
#[cfg(not(feature = "play"))]
use metaritual::ir::compile_patch;
use metaritual::param::Param;
use metaritual::patch::Patch;
use metaritual::pattern::Pattern;
use metaritual::signal::Signal;
use metaritual::source::Source;
use metaritual::space::Space;

fn main() {
    // -- Drums --
    // Kick: MIDI 36, four-on-the-floor
    let kick = construct::euclidean(4, 16)
        .with_values(|v| if v == 1.0 { 36.0 } else { v });

    // Hi-hat: MIDI 42, syncopated 7-in-16
    let hihat = construct::euclidean_rotated(7, 16, 1)
        .with_values(|v| if v == 1.0 { 42.0 } else { v });

    // Snare: MIDI 38, tresillo
    let snare = construct::euclidean_rotated(3, 8, 1)
        .with_values(|v| if v == 1.0 { 38.0 } else { v });

    // Combine drums into one pattern (stacked = simultaneous)
    let drums = Pattern::Stack(vec![kick, hihat, snare]);

    // -- Bass: euclidean rhythm × A minor scale --
    let bass_rhythm = construct::euclidean(5, 16);
    let bass_notes = construct::scale(33.0, MINOR, 1); // A1 minor
    let pairs = metaritual::combine::combine(
        &bass_rhythm,
        &bass_notes,
        metaritual::combine::Combine::Cycle,
    );
    let bass_dur = 1.0 / pairs.len().max(1) as f64;
    let bass = Pattern::Seq(
        pairs.iter().map(|&(_r, pitch)| Pattern::Atom(pitch, bass_dur)).collect(),
    );

    // -- Lead: accumulator-driven melody --
    // Seed: Am7 arpeggio
    let lead_seed = Pattern::Seq(vec![
        Pattern::Atom(69.0, 0.125), // A4
        Pattern::Atom(72.0, 0.125), // C5
        Pattern::Atom(76.0, 0.125), // E5
        Pattern::Atom(79.0, 0.125), // G5
    ]);

    let lead = Accumulator::new(
        lead_seed,
        vec![
            Box::new(|p: &Pattern| p.rotate(1)),              // rotate
            Box::new(|p: &Pattern| p.shift(2.0)),             // up whole step
            Box::new(|p: &Pattern| p.retrograde()),            // reverse
            Box::new(|p: &Pattern| p.invert(74.0).shift(-2.0)), // mirror + down
            Box::new(|p: &Pattern| p.thin(2)),                 // keep every other
            Box::new(|p: &Pattern| p.rotate(-1).shift(-2.0)),  // rotate back + down
        ],
    )
    .generate(8); // 8 iterations of evolving melody

    // -- Chords: Am7 → Dm7 → G7 → Cmaj7 --
    let chord_prog = Pattern::Seq(vec![
        chord(57.0, MINOR_7, 0.25),                            // Am7
        chord(62.0, MINOR_7, 0.25),                            // Dm7
        chord(55.0, &[0, 4, 7, 10], 0.25),                    // G7
        chord(60.0, &[0, 4, 7, 11], 0.25),                    // Cmaj7
    ]);

    // -- Build the patch --
    let mut p = Patch::new("pulse");

    let drum_src = p.add_source(Source::synth("Analog"));
    let bass_src = p.add_source(Source::synth("Analog"));
    let lead_src = p.add_source(Source::synth("Analog"));
    let pad_src = p.add_source(Source::synth("Analog"));

    p.add_pattern(drums, "drums");
    p.add_pattern(bass, "bass");
    p.add_pattern(lead, "lead");
    p.add_pattern(chord_prog, "chords");

    // Effects
    let comp = p.add_effect(effect::compressor(-12.0, 4.0));
    let rev = p.add_effect(effect::reverb(2.0, 0.2));
    let dly = p.add_effect(effect::delay(0.25, 0.3, 0.15));

    // Routing
    let mix = p.add_mixer(4, "main");
    p.connect(drum_src, 0, mix, 0);
    p.connect(bass_src, 0, mix, 1);
    p.connect(lead_src, 0, mix, 2);
    p.connect(pad_src, 0, mix, 3);
    p.connect(mix, 0, comp, 0);
    p.connect(comp, 0, rev, 0);
    p.connect(rev, 0, dly, 0);

    // Control surface
    p.expose_param(Param::new("drive", (0.0, 1.0), 0.3));
    p.expose_param(Param::new("space", (0.0, 1.0), 0.2));

    // Spatial: subtle movement
    p.set_space(
        Space::center()
            .with_pan(Signal::triangle(0.25))
            .with_depth(Signal::Const(0.15)),
    );

    // Output
    #[cfg(feature = "play")]
    {
        let config = metaritual::play::PlayConfig {
            bpm: 118.0,
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
        let ir = compile_patch(&p, 118.0, 1, 4.0, 16);
        println!("{}", ir.to_json_pretty());
    }
}
