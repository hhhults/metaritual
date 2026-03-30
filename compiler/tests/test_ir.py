"""Tests for IR deserialization — verify Python can parse Rust-generated JSON."""

import json
import subprocess
import sys
from pathlib import Path

import pytest

from metaritual_compiler.ir import (
    IrAutomation,
    IrBreakpoint,
    IrClip,
    IrEdge,
    IrEffect,
    IrEvent,
    IrExposedParam,
    IrNode,
    IrParam,
    IrPatch,
    IrSource,
    IrSpace,
)


# ---------------------------------------------------------------------------
# Fixtures: known IR JSON structures
# ---------------------------------------------------------------------------

MINIMAL_PATCH = {
    "label": "test",
    "bpm": 120.0,
    "nodes": [
        {
            "type": "source",
            "id": 0,
            "source": {"type": "sample", "path": "kick.wav", "label": "kick.wav"},
            "label": "kick.wav",
        }
    ],
    "edges": [],
    "space": {
        "pan": {"param_name": "pan", "breakpoints": [{"time": 0.0, "value": 0.0}, {"time": 4.0, "value": 0.0}]},
        "width": {"param_name": "width", "breakpoints": [{"time": 0.0, "value": 1.0}, {"time": 4.0, "value": 1.0}]},
        "depth": {"param_name": "depth", "breakpoints": [{"time": 0.0, "value": 0.0}, {"time": 4.0, "value": 0.0}]},
    },
    "exposed_params": [],
}

FULL_PATCH = {
    "label": "water_braid",
    "bpm": 120.0,
    "nodes": [
        {
            "type": "source",
            "id": 0,
            "source": {"type": "sample", "path": "water.wav", "label": "water grains"},
            "label": "water grains",
        },
        {
            "type": "source",
            "id": 1,
            "source": {"type": "synth", "preset": "Analog", "params": [["cutoff", 0.7]], "label": "Analog"},
            "label": "synth pad",
        },
        {
            "type": "pattern",
            "id": 2,
            "clip": {
                "events": [
                    {"value": 60.0, "start": 0.0, "duration": 1.0},
                    {"value": 64.0, "start": 1.0, "duration": 1.0},
                    {"value": 67.0, "start": 2.0, "duration": 1.0},
                ],
                "length": 4.0,
                "bpm": 120.0,
            },
            "label": "melody",
        },
        {
            "type": "effect",
            "id": 3,
            "effect": {
                "name": "Reverb",
                "params": [
                    {"name": "decay", "value": 3.0, "range": [0.0, 60.0]},
                    {"name": "mix", "value": 0.5, "range": [0.0, 1.0]},
                ],
                "automation": [
                    {
                        "param_name": "mix",
                        "breakpoints": [
                            {"time": 0.0, "value": 0.3},
                            {"time": 2.0, "value": 0.7},
                            {"time": 4.0, "value": 0.3},
                        ],
                    }
                ],
            },
            "label": "Reverb",
        },
        {
            "type": "mixer",
            "id": 4,
            "channels": 2,
            "label": "submix",
        },
    ],
    "edges": [
        {"from_node": 0, "from_port": 0, "to_node": 4, "to_port": 0},
        {"from_node": 1, "from_port": 0, "to_node": 4, "to_port": 1},
        {"from_node": 4, "from_port": 0, "to_node": 3, "to_port": 0},
    ],
    "space": {
        "pan": {"param_name": "pan", "breakpoints": [{"time": 0.0, "value": -0.5}, {"time": 4.0, "value": 0.5}]},
        "width": {"param_name": "width", "breakpoints": [{"time": 0.0, "value": 1.0}, {"time": 4.0, "value": 1.0}]},
        "depth": {"param_name": "depth", "breakpoints": [{"time": 0.0, "value": 0.3}, {"time": 4.0, "value": 0.3}]},
    },
    "exposed_params": [
        {
            "name": "density",
            "range": [0.0, 1.0],
            "default": 0.5,
            "automation": {"param_name": "density", "breakpoints": [{"time": 0.0, "value": 0.5}, {"time": 4.0, "value": 0.5}]},
        }
    ],
}


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


class TestIrBreakpoint:
    def test_from_dict(self):
        bp = IrBreakpoint.from_dict({"time": 1.5, "value": 0.7})
        assert bp.time == 1.5
        assert bp.value == 0.7


class TestIrAutomation:
    def test_from_dict(self):
        auto = IrAutomation.from_dict({
            "param_name": "cutoff",
            "breakpoints": [
                {"time": 0.0, "value": 0.0},
                {"time": 1.0, "value": 1.0},
            ],
        })
        assert auto.param_name == "cutoff"
        assert len(auto.breakpoints) == 2


class TestIrParam:
    def test_from_dict(self):
        p = IrParam.from_dict({"name": "decay", "value": 3.0, "range": [0.0, 60.0]})
        assert p.name == "decay"
        assert p.value == 3.0
        assert p.range == (0.0, 60.0)


class TestIrEvent:
    def test_from_dict(self):
        e = IrEvent.from_dict({"value": 60.0, "start": 0.0, "duration": 0.5})
        assert e.value == 60.0


class TestIrClip:
    def test_from_dict(self):
        c = IrClip.from_dict({
            "events": [{"value": 60.0, "start": 0.0, "duration": 1.0}],
            "length": 4.0,
            "bpm": 120.0,
        })
        assert len(c.events) == 1
        assert c.length == 4.0


class TestIrSource:
    def test_sample(self):
        s = IrSource.from_dict({"type": "sample", "path": "kick.wav", "label": "kick"})
        assert s.source_type == "sample"
        assert s.path == "kick.wav"

    def test_synth(self):
        s = IrSource.from_dict({"type": "synth", "preset": "Analog", "params": [["cutoff", 0.5]], "label": "Analog"})
        assert s.source_type == "synth"
        assert s.preset == "Analog"
        assert s.params == [("cutoff", 0.5)]

    def test_live_input(self):
        s = IrSource.from_dict({"type": "live_input", "channel": 1, "label": "guitar"})
        assert s.source_type == "live_input"
        assert s.channel == 1

    def test_resampled(self):
        s = IrSource.from_dict({"type": "resampled", "patch_id": "p1", "label": "resamp"})
        assert s.source_type == "resampled"
        assert s.patch_id == "p1"


class TestIrNode:
    def test_source_node(self):
        n = IrNode.from_dict({
            "type": "source",
            "id": 0,
            "source": {"type": "sample", "path": "x.wav", "label": "x"},
            "label": "source",
        })
        assert n.node_type == "source"
        assert n.source is not None
        assert n.source.path == "x.wav"

    def test_pattern_node(self):
        n = IrNode.from_dict({
            "type": "pattern",
            "id": 1,
            "clip": {"events": [], "length": 4.0, "bpm": 120.0},
            "label": "rhythm",
        })
        assert n.node_type == "pattern"
        assert n.clip is not None

    def test_effect_node(self):
        n = IrNode.from_dict({
            "type": "effect",
            "id": 2,
            "effect": {"name": "Reverb", "params": [], "automation": []},
            "label": "reverb",
        })
        assert n.node_type == "effect"
        assert n.effect.name == "Reverb"

    def test_mixer_node(self):
        n = IrNode.from_dict({"type": "mixer", "id": 3, "channels": 2, "label": "mix"})
        assert n.node_type == "mixer"
        assert n.channels == 2


class TestIrPatch:
    def test_minimal(self):
        patch = IrPatch.from_dict(MINIMAL_PATCH)
        assert patch.label == "test"
        assert patch.bpm == 120.0
        assert len(patch.nodes) == 1
        assert len(patch.edges) == 0

    def test_full(self):
        patch = IrPatch.from_dict(FULL_PATCH)
        assert patch.label == "water_braid"
        assert len(patch.nodes) == 5
        assert len(patch.edges) == 3
        assert len(patch.exposed_params) == 1
        assert patch.exposed_params[0].name == "density"

    def test_from_json(self):
        json_str = json.dumps(MINIMAL_PATCH)
        patch = IrPatch.from_json(json_str)
        assert patch.label == "test"

    def test_space(self):
        patch = IrPatch.from_dict(FULL_PATCH)
        assert patch.space.pan.param_name == "pan"
        assert len(patch.space.pan.breakpoints) == 2
        assert patch.space.pan.breakpoints[0].value == -0.5

    def test_effect_automation(self):
        patch = IrPatch.from_dict(FULL_PATCH)
        effect_nodes = [n for n in patch.nodes if n.node_type == "effect"]
        assert len(effect_nodes) == 1
        fx = effect_nodes[0].effect
        assert fx.name == "Reverb"
        assert len(fx.automation) == 1
        assert fx.automation[0].param_name == "mix"
        assert len(fx.automation[0].breakpoints) == 3


class TestRustIrRoundTrip:
    """Test that the Python IR can parse JSON generated by Rust's compile_patch().

    Runs `cargo run --example water_braid` to generate IR JSON, then
    parses it in Python. Skipped if cargo is not available or the build fails.
    """

    @pytest.fixture
    def rust_project_root(self):
        return Path(__file__).resolve().parent.parent.parent

    @pytest.fixture
    def rust_ir_json(self, rust_project_root):
        """Run the Rust example and return the parsed JSON dict."""
        try:
            result = subprocess.run(
                ["cargo", "run", "--example", "water_braid"],
                capture_output=True,
                text=True,
                cwd=str(rust_project_root),
                timeout=60,
            )
        except (FileNotFoundError, subprocess.TimeoutExpired):
            pytest.skip("cargo not available or build timed out")
        if result.returncode != 0:
            pytest.skip(f"cargo build failed: {result.stderr[:200]}")
        return json.loads(result.stdout)

    def test_round_trip_parses(self, rust_ir_json):
        """Rust-generated JSON deserializes into Python IrPatch."""
        patch = IrPatch.from_dict(rust_ir_json)
        assert patch.label == "water_braid"
        assert patch.bpm == 120.0

    def test_round_trip_nodes(self, rust_ir_json):
        """Correct number and types of nodes."""
        patch = IrPatch.from_dict(rust_ir_json)
        assert len(patch.nodes) == 6
        types = [n.node_type for n in patch.nodes]
        assert types.count("source") == 2
        assert types.count("pattern") == 1
        assert types.count("effect") == 2
        assert types.count("mixer") == 1

    def test_round_trip_edges(self, rust_ir_json):
        """Correct number of edges."""
        patch = IrPatch.from_dict(rust_ir_json)
        assert len(patch.edges) == 4

    def test_round_trip_exposed_params(self, rust_ir_json):
        """Exposed params survive the round trip."""
        patch = IrPatch.from_dict(rust_ir_json)
        assert len(patch.exposed_params) == 2
        names = {ep.name for ep in patch.exposed_params}
        assert names == {"density", "brightness"}

    def test_round_trip_space(self, rust_ir_json):
        """Space automation is preserved."""
        patch = IrPatch.from_dict(rust_ir_json)
        # Pan is a sine LFO — should have many breakpoints
        assert len(patch.space.pan.breakpoints) > 2
        # Width and depth are constant — 2 breakpoints each
        assert len(patch.space.width.breakpoints) == 2
        assert len(patch.space.depth.breakpoints) == 2

    def test_round_trip_effect_params(self, rust_ir_json):
        """Effect parameters and automation survive."""
        patch = IrPatch.from_dict(rust_ir_json)
        effect_nodes = [n for n in patch.nodes if n.node_type == "effect"]
        assert len(effect_nodes) == 2
        names = {n.effect.name for n in effect_nodes}
        assert "Reverb" in names
        assert "Delay" in names

    def test_round_trip_pattern_events(self, rust_ir_json):
        """Pattern clip events survive the round trip."""
        patch = IrPatch.from_dict(rust_ir_json)
        pat_nodes = [n for n in patch.nodes if n.node_type == "pattern"]
        assert len(pat_nodes) == 1
        clip = pat_nodes[0].clip
        assert clip is not None
        # euclidean(5, 8).rotate(2) over 4 cycles = 5*4 = 20 events
        assert len(clip.events) == 20
        assert clip.bpm == 120.0
