"""Compiler — translates IrPatch into Ableton Live actions.

Given an IrPatch (deserialized from JSON), this module creates tracks,
loads instruments and effects, writes clips with notes, and applies
automation. Communicates via OSC through ableton.py.
"""

from __future__ import annotations

import sys
from dataclasses import dataclass, field
from pathlib import Path

from metaritual_compiler.ir import IrPatch, IrNode, IrEdge, IrEffect, IrAutomation

# Import ableton.py from the musictools root
_musictools_root = Path(__file__).resolve().parent.parent.parent.parent.parent
sys.path.insert(0, str(_musictools_root))
import ableton  # noqa: E402


# ---------------------------------------------------------------------------
# Effect name mapping: IR name → Ableton browser name
# ---------------------------------------------------------------------------

EFFECT_MAP: dict[str, str] = {
    "Reverb": "Reverb",
    "Delay": "Delay",
    "LowPass": "Auto Filter",
    "HighPass": "Auto Filter",
    "Compressor": "Compressor",
    "EQ3": "EQ Three",
    "Saturator": "Saturator",
}


# ---------------------------------------------------------------------------
# Compile result
# ---------------------------------------------------------------------------

@dataclass
class CompileResult:
    """Summary of what was created in Ableton."""
    tracks_created: list[str] = field(default_factory=list)
    clips_created: int = 0
    effects_loaded: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)

    def summary(self) -> str:
        lines = [f"Compiled: {len(self.tracks_created)} tracks, {self.clips_created} clips, {len(self.effects_loaded)} effects"]
        for t in self.tracks_created:
            lines.append(f"  track: {t}")
        for e in self.errors:
            lines.append(f"  error: {e}")
        return "\n".join(lines)


# ---------------------------------------------------------------------------
# Graph analysis helpers
# ---------------------------------------------------------------------------

def _build_adjacency(patch: IrPatch) -> dict[int, list[int]]:
    """Build adjacency list: node_id → [downstream node_ids]."""
    adj: dict[int, list[int]] = {}
    for node in patch.nodes:
        adj[node.id] = []
    for edge in patch.edges:
        adj.setdefault(edge.from_node, []).append(edge.to_node)
    return adj


def _find_effect_chain(patch: IrPatch, source_id: int) -> list[IrNode]:
    """Walk edges from a source node, collecting effect nodes in order."""
    adj = _build_adjacency(patch)
    nodes_by_id = {n.id: n for n in patch.nodes}
    chain = []
    current = source_id

    while True:
        downstream = adj.get(current, [])
        # Find the next effect/chain node
        next_effect = None
        for nid in downstream:
            node = nodes_by_id.get(nid)
            if node and node.node_type in ("effect", "chain"):
                next_effect = node
                break
            elif node and node.node_type == "mixer":
                # Walk through mixer to find effects after it
                mixer_downstream = adj.get(nid, [])
                for mid in mixer_downstream:
                    mnode = nodes_by_id.get(mid)
                    if mnode and mnode.node_type in ("effect", "chain"):
                        next_effect = mnode
                        break
                if next_effect:
                    break

        if next_effect is None:
            break
        chain.append(next_effect)
        current = next_effect.id

    return chain


def _find_pattern_nodes(patch: IrPatch) -> list[IrNode]:
    """Find all pattern nodes in the patch."""
    return [n for n in patch.nodes if n.node_type == "pattern"]


def _find_source_nodes(patch: IrPatch) -> list[IrNode]:
    """Find all source nodes in the patch."""
    return [n for n in patch.nodes if n.node_type == "source"]


# ---------------------------------------------------------------------------
# Core compiler
# ---------------------------------------------------------------------------

def compile(patch: IrPatch, session: ableton.Session | None = None) -> CompileResult:
    """Compile an IrPatch into a live Ableton session.

    If session is None, creates a new Session connection.

    Strategy: one Ableton track per source node. Effects following
    a source (via edges) become devices on that track. Pattern nodes
    create clips on the first source track.
    """
    if session is None:
        session = ableton.Session()

    result = CompileResult()

    # Set tempo
    session.tempo = patch.bpm

    source_nodes = _find_source_nodes(patch)
    pattern_nodes = _find_pattern_nodes(patch)

    # Create a track for each source
    track_map: dict[int, int] = {}  # source node id → Ableton track index

    for snode in source_nodes:
        source = snode.source

        if source.source_type == "live_input":
            track = session.create_audio_track()
            track.name = snode.label
            if source.channel is not None:
                # Audio input routing — would need specific routing setup
                pass
        else:
            track = session.create_midi_track()
            track.name = snode.label

            if source.source_type == "sample":
                loaded = track.load_sample(source.path)
                if loaded is None:
                    result.errors.append(f"Failed to load sample: {source.path}")

            elif source.source_type == "synth":
                track_idx = session.tracks().index(track) if track in session.tracks() else -1
                if track_idx >= 0:
                    loaded = session.load_instrument(track_idx, source.preset)
                    if loaded is None:
                        result.errors.append(f"Failed to load instrument: {source.preset}")

        result.tracks_created.append(snode.label)
        track_map[snode.id] = len(session.tracks()) - 1  # last created track

    # Load effects onto each source's track
    for snode in source_nodes:
        track_idx = track_map.get(snode.id)
        if track_idx is None:
            continue

        effects_chain = _find_effect_chain(patch, snode.id)
        for enode in effects_chain:
            if enode.node_type == "effect" and enode.effect:
                _load_effect(session, track_idx, enode.effect, result)
            elif enode.node_type == "chain" and enode.effects:
                for eff in enode.effects:
                    _load_effect(session, track_idx, eff, result)

    # Create clips from pattern nodes
    # Distribute patterns across source tracks: pattern i → source track i
    if pattern_nodes and source_nodes:
        for i, pnode in enumerate(pattern_nodes):
            if pnode.clip:
                # Map pattern i to source i (wrapping if more patterns than sources)
                source_idx = i % len(source_nodes)
                track_idx = track_map.get(source_nodes[source_idx].id)
                if track_idx is not None:
                    _create_clip(session, track_idx, 0, pnode, result)

    # Apply space (pan) to first track
    if source_nodes and track_map:
        first_track_idx = track_map.get(source_nodes[0].id)
        if first_track_idx is not None:
            track = session.track(first_track_idx)
            # Use first breakpoint's value for static pan
            if patch.space.pan.breakpoints:
                pan_val = patch.space.pan.breakpoints[0].value
                track.panning = pan_val

    return result


def _load_effect(
    session: ableton.Session,
    track_idx: int,
    ir_effect: IrEffect,
    result: CompileResult,
) -> None:
    """Load an effect onto a track and set its parameters."""
    ableton_name = EFFECT_MAP.get(ir_effect.name, ir_effect.name)
    loaded = session.load_effect(track_idx, ableton_name)
    if loaded is None:
        result.errors.append(f"Failed to load effect: {ir_effect.name} (as {ableton_name})")
        return

    result.effects_loaded.append(ir_effect.name)

    # Set static parameters
    track = session.track(track_idx)
    devices = track.devices()
    if not devices:
        return

    device = devices[-1]  # last loaded device

    for param in ir_effect.params:
        try:
            device.set_param_by_name(param.name, param.value)
        except Exception:
            # Parameter name might not match Ableton's naming
            pass


def _create_clip(
    session: ableton.Session,
    track_idx: int,
    slot_idx: int,
    pattern_node: IrNode,
    result: CompileResult,
) -> None:
    """Create a clip from a pattern node's IrClip."""
    clip_data = pattern_node.clip
    if not clip_data or not clip_data.events:
        return

    track = session.track(track_idx)
    clip = track.create_clip(slot_idx, length=clip_data.length)
    clip.name = pattern_node.label
    clip.looping = True

    # Convert IrEvents to note tuples: (pitch, start, duration, velocity)
    notes = []
    for event in clip_data.events:
        pitch = int(round(event.value))
        pitch = max(0, min(127, pitch))  # clamp to MIDI range
        velocity = 100  # default velocity
        notes.append((pitch, event.start, event.duration, velocity))

    if notes:
        clip.add_notes(notes)

    result.clips_created += 1


def _apply_automation(
    clip,
    device_idx: int,
    automation: IrAutomation,
    param_idx: int,
) -> None:
    """Write automation breakpoints to a clip."""
    if len(automation.breakpoints) < 2:
        return

    points = [(bp.time, bp.value) for bp in automation.breakpoints]
    clip.automate_smooth(device_idx, param_idx, points)
