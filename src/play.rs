/// Direct Rust→Ableton playback — no Python, no pipes.
///
/// Compiles a Patch to IR, then sends it directly to Ableton via OSC.
/// Requires the `play` feature: `cargo run --features play --example twinkle`

use crate::ir::compile_patch;
use crate::patch::Patch;

/// Configuration for playback.
pub struct PlayConfig {
    pub bpm: f32,
    pub cycles: usize,
    pub beats_per_cycle: f64,
    pub automation_resolution: usize,
    /// If true, fire all clips and start playing after compilation.
    pub autoplay: bool,
}

impl Default for PlayConfig {
    fn default() -> Self {
        Self {
            bpm: 120.0,
            cycles: 1,
            beats_per_cycle: 4.0,
            automation_resolution: 16,
            autoplay: true,
        }
    }
}

impl PlayConfig {
    pub fn new(bpm: f32) -> Self {
        Self { bpm, ..Default::default() }
    }
}

/// Play a Patch directly in Ableton Live.
///
/// Connects to AbletonOSC, compiles the patch to IR, then sends all the
/// OSC messages to create tracks, load instruments/effects, write clips.
///
/// Returns the compile result summary.
pub fn play(patch: &Patch, config: &PlayConfig) -> Result<String, String> {
    let ir = compile_patch(
        patch,
        config.bpm as f64,
        config.cycles,
        config.beats_per_cycle,
        config.automation_resolution,
    );

    let json = ir.to_json();
    let ir_patch = ableton::IrPatch::from_json(&json)?;

    let session = ableton::Session::connect().map_err(|e| format!("connect: {e}"))?;

    eprintln!("Connected to Ableton Live {:?}", session.version().map_err(|e| format!("{e}"))?);

    let result = ableton::compiler::compile(&ir_patch, &session)
        .map_err(|e| format!("compile: {e}"))?;

    let summary = result.summary();
    eprintln!("{summary}");

    if config.autoplay {
        // Fire all clips in slot 0
        if let Ok(tracks) = session.tracks() {
            for track in &tracks {
                if track.has_clip(0).unwrap_or(false) {
                    let clip = track.clip(0);
                    let _ = clip.fire();
                }
            }
        }
        let _ = session.play();
        eprintln!("Playing!");
    }

    Ok(summary)
}

/// Check if `--play` was passed on the command line.
pub fn should_play() -> bool {
    std::env::args().any(|a| a == "--play")
}

/// Convenience: compile a patch and either play it or print IR JSON.
///
/// If `--play` is on the command line, plays directly in Ableton.
/// Otherwise, prints the IR JSON to stdout (the original pipeline behavior).
pub fn play_or_print(patch: &Patch, config: &PlayConfig) {
    if should_play() {
        match play(patch, config) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        let ir = compile_patch(
            patch,
            config.bpm as f64,
            config.cycles,
            config.beats_per_cycle,
            config.automation_resolution,
        );
        println!("{}", ir.to_json_pretty());
    }
}
