//! metaritual-live — hot-reload loop for metaritual examples.
//!
//! Watches source files for changes, recompiles on save, and
//! incrementally updates Ableton Live via OSC. Pattern changes
//! are near-instant (just rewriting clip notes); structural changes
//! (adding/removing tracks or effects) trigger a full rebuild.
//!
//!     cargo run --features play --bin live -- drift
//!     cargo run --features play --bin live -- pulse

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: cargo run --features play --bin live -- <example-name>");
        eprintln!();
        eprintln!("examples:");
        eprintln!("  cargo run --features play --bin live -- drift");
        eprintln!("  cargo run --features play --bin live -- pulse");
        eprintln!("  cargo run --features play --bin live -- scatter");
        eprintln!("  cargo run --features play --bin live -- twinkle");
        std::process::exit(1);
    }

    let example = &args[1];
    let example_path = format!("examples/{example}.rs");

    if !std::path::Path::new(&example_path).exists() {
        eprintln!("error: {example_path} not found");
        std::process::exit(1);
    }

    // Connect to Ableton
    eprintln!("connecting to Ableton Live...");
    let session = match ableton::Session::connect() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to connect: {e}");
            eprintln!("make sure Ableton Live is running with AbletonOSC");
            std::process::exit(1);
        }
    };

    if let Ok(v) = session.version() {
        eprintln!("connected to Ableton Live {}.{}", v.0, v.1);
    }

    let mut live = ableton::LiveSession::new(session);

    // Initial build + compile
    eprintln!("building {example}...");
    let ir = match build_and_capture(example) {
        Ok(ir) => ir,
        Err(e) => {
            eprintln!("build failed:\n{e}");
            std::process::exit(1);
        }
    };

    let patch = match ableton::IrPatch::from_json(&ir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("IR parse failed: {e}");
            std::process::exit(1);
        }
    };

    match live.update(patch) {
        Ok(result) => eprintln!("{}", result.summary()),
        Err(e) => {
            eprintln!("compile failed: {e}");
            std::process::exit(1);
        }
    }

    // Start playback
    if let Err(e) = live.play() {
        eprintln!("playback failed: {e}");
    }
    eprintln!("playing!");

    // Watch loop
    eprintln!();
    eprintln!("watching for changes... (ctrl-c to stop)");
    eprintln!("  edit {example_path} and save to hot-reload");
    eprintln!();

    let mut mtimes = snapshot_mtimes(example);

    loop {
        std::thread::sleep(Duration::from_millis(500));

        let new_mtimes = snapshot_mtimes(example);
        if new_mtimes == mtimes {
            continue;
        }
        mtimes = new_mtimes;

        eprint!("change detected, rebuilding {example}... ");

        match build_and_capture(example) {
            Ok(ir) => match ableton::IrPatch::from_json(&ir) {
                Ok(patch) => match live.update(patch) {
                    Ok(result) => eprintln!("{}", result.summary()),
                    Err(e) => eprintln!("update failed: {e}"),
                },
                Err(e) => eprintln!("IR parse failed: {e}"),
            },
            Err(e) => eprintln!("build failed:\n{e}"),
        }
    }
}

/// Build the example and capture its JSON IR output.
fn build_and_capture(example: &str) -> Result<String, String> {
    // Build without --features play so the example outputs JSON
    let build = Command::new("cargo")
        .args(["build", "--example", example])
        .output()
        .map_err(|e| format!("cargo build: {e}"))?;

    if !build.status.success() {
        let stderr = String::from_utf8_lossy(&build.stderr);
        return Err(stderr.to_string());
    }

    // Run the built binary (no --play flag -> JSON output to stdout)
    let bin_path = format!("target/debug/examples/{example}");
    let run = Command::new(&bin_path)
        .output()
        .map_err(|e| format!("run {bin_path}: {e}"))?;

    if !run.status.success() {
        let stderr = String::from_utf8_lossy(&run.stderr);
        return Err(format!("example failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&run.stdout).to_string();
    if stdout.trim().is_empty() {
        return Err("example produced no output (did it crash?)".to_string());
    }

    Ok(stdout)
}

/// Snapshot modification times for all watched source files.
fn snapshot_mtimes(example: &str) -> HashMap<PathBuf, SystemTime> {
    let mut mtimes = HashMap::new();

    // Watch the example file
    let example_path = PathBuf::from(format!("examples/{example}.rs"));
    add_mtime(&mut mtimes, &example_path);

    // Watch all src/*.rs files (library changes trigger rebuild too)
    if let Ok(entries) = std::fs::read_dir("src") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "rs") {
                add_mtime(&mut mtimes, &path);
            }
        }
    }

    mtimes
}

fn add_mtime(map: &mut HashMap<PathBuf, SystemTime>, path: &PathBuf) {
    if let Ok(meta) = std::fs::metadata(path) {
        if let Ok(mtime) = meta.modified() {
            map.insert(path.clone(), mtime);
        }
    }
}
