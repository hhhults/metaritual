"""CLI entry point — reads IR JSON and compiles to Ableton.

Usage:
    # From file
    metaritual-compile patch.json

    # From stdin (pipe from Rust)
    cargo run --example my_patch | metaritual-compile

    # Dry run (just parse and validate, no Ableton connection)
    metaritual-compile --dry-run patch.json
"""

from __future__ import annotations

import argparse
import json
import sys

from metaritual_compiler.ir import IrPatch


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Compile metaritual IR JSON to Ableton Live",
    )
    parser.add_argument(
        "ir_file",
        nargs="?",
        help="Path to IR JSON file (reads from stdin if omitted)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Parse and validate IR without connecting to Ableton",
    )
    parser.add_argument(
        "--pretty",
        action="store_true",
        help="Pretty-print the parsed IR and exit",
    )
    args = parser.parse_args()

    # Read IR JSON
    if args.ir_file:
        with open(args.ir_file) as f:
            data = json.load(f)
    else:
        data = json.load(sys.stdin)

    # Parse
    try:
        patch = IrPatch.from_dict(data)
    except (KeyError, ValueError) as e:
        print(f"Error parsing IR: {e}", file=sys.stderr)
        sys.exit(1)

    print(f"Parsed: {patch.label} ({len(patch.nodes)} nodes, {len(patch.edges)} edges, bpm={patch.bpm})")

    if args.pretty:
        print(json.dumps(data, indent=2))
        return

    if args.dry_run:
        # Validate structure
        source_count = sum(1 for n in patch.nodes if n.node_type == "source")
        pattern_count = sum(1 for n in patch.nodes if n.node_type == "pattern")
        effect_count = sum(1 for n in patch.nodes if n.node_type in ("effect", "chain"))
        print(f"  Sources: {source_count}")
        print(f"  Patterns: {pattern_count}")
        print(f"  Effects: {effect_count}")
        print(f"  Exposed params: {len(patch.exposed_params)}")
        print("Dry run complete — no Ableton connection made.")
        return

    # Compile to Ableton
    from metaritual_compiler.compiler import compile

    result = compile(patch)
    print(result.summary())


if __name__ == "__main__":
    main()
