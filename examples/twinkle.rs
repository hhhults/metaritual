/// twinkle — "Twinkle Twinkle Little Star" as a metaritual patch.
///
/// Demonstrates melody with varying note durations, a simple effect
/// chain, and gentle spatial movement.
///
///     cargo run --example twinkle | metaritual-compile
///     cargo run --example twinkle | metaritual-compile --dry-run
///     cargo run --features play --example twinkle -- --play

use metaritual::effect;
#[cfg(not(feature = "play"))]
use metaritual::ir::compile_patch;
use metaritual::param::Param;
use metaritual::patch::Patch;
use metaritual::pattern::Pattern;
use metaritual::signal::Signal;
use metaritual::source::Source;
use metaritual::space::Space;

/// Build a note: MIDI pitch, duration in fractions of a cycle.
fn n(pitch: f64, dur: f64) -> Pattern {
    Pattern::Atom(pitch, dur)
}

fn main() {
    // Twinkle Twinkle Little Star — 2 phrases, each 1 cycle.
    //
    // In 4/4 at 100 BPM, 1 cycle = 4 beats.
    // Quarter note = 0.25 cycle, half note = 0.5 cycle.
    //
    // Phrase 1: C C G G | A A G~
    // Phrase 2: F F E E | D D C~

    let q = 0.125; // eighth note (1/8 of cycle)
    let h = 0.25;  // quarter note (1/4 of cycle)

    // Full melody as a single pattern (2 cycles)
    let melody = Pattern::Seq(vec![
        // Phrase 1: "Twinkle twinkle little star"
        //  C  C  G  G  A  A  G-
        n(60.0, h), n(60.0, h), n(67.0, h), n(67.0, h),
        n(69.0, h), n(69.0, h), n(67.0, h * 2.0),
        // Phrase 2: "How I wonder what you are"
        //  F  F  E  E  D  D  C-
        n(65.0, h), n(65.0, h), n(64.0, h), n(64.0, h),
        n(62.0, h), n(62.0, h), n(60.0, h * 2.0),
        // Phrase 3: "Up above the world so high"
        //  G  G  F  F  E  E  D-
        n(67.0, h), n(67.0, h), n(65.0, h), n(65.0, h),
        n(64.0, h), n(64.0, h), n(62.0, h * 2.0),
        // Phrase 4: "Like a diamond in the sky"
        //  G  G  F  F  E  E  D-
        n(67.0, h), n(67.0, h), n(65.0, h), n(65.0, h),
        n(64.0, h), n(64.0, h), n(62.0, h * 2.0),
        // Phrase 5 (reprise): "Twinkle twinkle little star"
        n(60.0, h), n(60.0, h), n(67.0, h), n(67.0, h),
        n(69.0, h), n(69.0, h), n(67.0, h * 2.0),
        // Phrase 6 (reprise): "How I wonder what you are"
        n(65.0, h), n(65.0, h), n(64.0, h), n(64.0, h),
        n(62.0, h), n(62.0, h), n(60.0, h * 2.0),
    ]);

    // Also add a simple bass line — root notes, whole notes
    let bass = Pattern::Seq(vec![
        // Under phrase 1: C, G
        n(48.0, 0.5), n(43.0, 0.5),
        // Under phrase 2: F, C
        n(41.0, 0.5), n(36.0, 0.5),
        // Under phrase 3: G, F
        n(43.0, 0.5), n(41.0, 0.5),
        // Under phrase 4: G, F
        n(43.0, 0.5), n(41.0, 0.5),
        // Under phrase 5: C, G
        n(48.0, 0.5), n(43.0, 0.5),
        // Under phrase 6: F, C
        n(41.0, 0.5), n(36.0, 0.5),
    ]);

    let _ = q; // eighth notes available for ornamentation

    // Build the patch
    let mut p = Patch::new("twinkle");

    // Sources
    let piano = p.add_source(Source::synth("Analog"));
    let pad = p.add_source(Source::synth("Analog"));

    // Patterns
    p.add_pattern(melody, "melody");
    p.add_pattern(bass, "bass");

    // Effects — gentle reverb and a touch of delay
    let rev = p.add_effect(effect::reverb(2.5, 0.3));
    let dly = p.add_effect(effect::delay(0.5, 0.2, 0.15));

    // Routing: both sources → mixer → reverb → delay
    let mix = p.add_mixer(2, "main mix");
    p.connect(piano, 0, mix, 0);
    p.connect(pad, 0, mix, 1);
    p.connect(mix, 0, rev, 0);
    p.connect(rev, 0, dly, 0);

    // Control surface
    p.expose_param(Param::new("reverb_mix", (0.0, 1.0), 0.3));
    p.expose_param(Param::new("brightness", (0.0, 1.0), 0.7));

    // Gentle stereo sway
    p.set_space(
        Space::center()
            .with_pan(Signal::sine(0.1))
            .with_depth(Signal::Const(0.2)),
    );

    // Compile: 100 BPM, the melody is 6 phrases of 2 bars each.
    // Total duration of melody pattern = 6.0 cycles (sum of all atom durations).
    // With beats_per_cycle = 4.0 and 1 cycle pass, we get the full piece.
    #[cfg(feature = "play")]
    {
        let config = metaritual::play::PlayConfig::new(100.0);
        metaritual::play::play_or_print(&p, &config);
        return;
    }

    #[cfg(not(feature = "play"))]
    {
        let ir = compile_patch(&p, 100.0, 1, 4.0, 16);
        println!("{}", ir.to_json_pretty());
    }
}
