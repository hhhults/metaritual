/// water_braid — the design-doc example as a standalone binary.
///
/// Outputs IR JSON to stdout, ready for piping:
///
///     cargo run --example water_braid | metaritual-compile --dry-run
///     cargo run --example water_braid > patch.json

use metaritual::construct::euclidean;
use metaritual::effect;
use metaritual::ir::compile_patch;
use metaritual::param::Param;
use metaritual::patch::Patch;
use metaritual::signal::Signal;
use metaritual::source::Source;
use metaritual::space::Space;

fn main() {
    let mut p = Patch::new("water_braid");

    // Sources
    let water = p.add_source(Source::sample_labeled("water.wav", "water grains"));
    let glass = p.add_source(Source::sample_labeled("glass.wav", "glass grains"));

    // Patterns
    p.add_pattern(euclidean(5, 8).rotate(2), "main rhythm");

    // Effects
    let rev = p.add_effect(effect::reverb(4.0, 0.5));
    let dly = p.add_effect(effect::delay(0.375, 0.4, 0.3));

    // Routing
    let mix = p.add_mixer(2, "submix");
    p.connect(water, 0, mix, 0);
    p.connect(glass, 0, mix, 1);
    p.connect(mix, 0, rev, 0);
    p.connect(rev, 0, dly, 0);

    // Control surface
    p.expose_param(Param::new("density", (0.0, 1.0), 0.3));
    p.expose_param(Param::new("brightness", (0.0, 1.0), 0.6));

    // Space
    p.set_space(
        Space::center()
            .with_pan(Signal::sine(0.03))
            .with_depth(Signal::Const(0.5)),
    );

    // Compile and emit JSON
    let ir = compile_patch(&p, 120.0, 4, 4.0, 16);
    println!("{}", ir.to_json_pretty());
}
