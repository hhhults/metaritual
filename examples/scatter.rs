/// scatter — stochastic composition that sounds different every run.
///
/// Uses time-seeded randomness for non-deterministic output:
///   - Degraded euclidean rhythms (randomly dropping hits)
///   - Humanized timing and velocity
///   - Markov chain learned from a pentatonic scale then extended
///   - Swing applied to the rhythm
///
///     cargo run --example scatter
///     cargo run --features play --example scatter -- --play
///
/// Run it twice — it'll sound different each time.

use metaritual::construct::{self, MINOR_PENTATONIC};
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
    // Time-based seed for non-deterministic output
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    eprintln!("seed: {seed}");

    // -- Melody: markov chain from pentatonic scale --
    // Learn transitions from a seed phrase, then generate something new
    let seed_phrase = Pattern::Seq(vec![
        Pattern::Atom(60.0, 0.125), // C
        Pattern::Atom(63.0, 0.125), // Eb
        Pattern::Atom(65.0, 0.125), // F
        Pattern::Atom(67.0, 0.125), // G
        Pattern::Atom(70.0, 0.125), // Bb
        Pattern::Atom(72.0, 0.125), // C5
        Pattern::Atom(70.0, 0.125), // Bb
        Pattern::Atom(67.0, 0.125), // G
    ]);

    let chain = Markov::from_pattern(&seed_phrase);
    let melody = chain
        .generate(24, 1.0 / 24.0, Some(60.0), seed)
        .humanize(0.02, 0.15, seed + 1)
        .swing(0.15);

    // -- Rhythm: degraded euclidean --
    // Start with E(9,16), randomly drop 30% of hits
    let rhythm = construct::euclidean(9, 16)
        .degrade(0.3, seed + 2)
        .with_values(|v| if v == 1.0 { 36.0 } else { v }); // map hits to kick

    // A second rhythmic layer with different euclidean + degradation
    let rhythm2 = construct::euclidean_rotated(5, 12, 3)
        .degrade(0.2, seed + 3)
        .with_values(|v| if v == 1.0 { 42.0 } else { v }); // hi-hat

    // -- Bass: pentatonic scale shifted down, thinned --
    let bass_scale = construct::scale(36.0, MINOR_PENTATONIC, 1);
    let bass = bass_scale
        .stretch(2.0) // slow it down
        .degrade(0.15, seed + 4)
        .humanize(0.01, 0.1, seed + 5);

    // -- Build the patch --
    let mut p = Patch::new("scatter");

    let mel_src = p.add_source(Source::synth("Analog"));
    let perc_src = p.add_source(Source::synth("Analog"));
    let bass_src = p.add_source(Source::synth("Analog"));

    p.add_pattern(melody, "melody");
    p.add_pattern(
        Pattern::Stack(vec![rhythm, rhythm2]),
        "percussion",
    );
    p.add_pattern(bass, "bass");

    // Effects: saturator → reverb
    let sat = p.add_effect(effect::saturator(3.0, 0.6));
    let rev = p.add_effect(effect::reverb(4.0, 0.4));
    let dly = p.add_effect(effect::delay(0.333, 0.25, 0.15));

    // Routing
    let mix = p.add_mixer(3, "main");
    p.connect(mel_src, 0, mix, 0);
    p.connect(perc_src, 0, mix, 1);
    p.connect(bass_src, 0, mix, 2);
    p.connect(mix, 0, sat, 0);
    p.connect(sat, 0, rev, 0);
    p.connect(rev, 0, dly, 0);

    // Control surface
    p.expose_param(Param::new("chaos", (0.0, 1.0), 0.5));
    p.expose_param(Param::new("warmth", (0.0, 1.0), 0.6));

    // Spatial: LFO pan at different rates
    p.set_space(
        Space::center()
            .with_pan(Signal::sine(0.13))
            .with_depth(Signal::Const(0.3)),
    );

    // Output
    #[cfg(feature = "play")]
    {
        let config = metaritual::play::PlayConfig {
            bpm: 85.0,
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
        let ir = compile_patch(&p, 85.0, 1, 4.0, 16);
        println!("{}", ir.to_json_pretty());
    }
}
