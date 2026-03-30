"""IR dataclasses — Python mirror of the Rust IR types.

These are deserialized from the JSON that the Rust DSL emits.
"""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class IrBreakpoint:
    time: float
    value: float

    @classmethod
    def from_dict(cls, d: dict) -> IrBreakpoint:
        return cls(time=d["time"], value=d["value"])


@dataclass
class IrAutomation:
    param_name: str
    breakpoints: list[IrBreakpoint]

    @classmethod
    def from_dict(cls, d: dict) -> IrAutomation:
        return cls(
            param_name=d["param_name"],
            breakpoints=[IrBreakpoint.from_dict(bp) for bp in d["breakpoints"]],
        )


@dataclass
class IrParam:
    name: str
    value: float
    range: tuple[float, float]

    @classmethod
    def from_dict(cls, d: dict) -> IrParam:
        return cls(name=d["name"], value=d["value"], range=tuple(d["range"]))


@dataclass
class IrEvent:
    value: float
    start: float
    duration: float

    @classmethod
    def from_dict(cls, d: dict) -> IrEvent:
        return cls(value=d["value"], start=d["start"], duration=d["duration"])


@dataclass
class IrClip:
    events: list[IrEvent]
    length: float
    bpm: float

    @classmethod
    def from_dict(cls, d: dict) -> IrClip:
        return cls(
            events=[IrEvent.from_dict(e) for e in d["events"]],
            length=d["length"],
            bpm=d["bpm"],
        )


@dataclass
class IrEffect:
    name: str
    params: list[IrParam]
    automation: list[IrAutomation]

    @classmethod
    def from_dict(cls, d: dict) -> IrEffect:
        return cls(
            name=d["name"],
            params=[IrParam.from_dict(p) for p in d["params"]],
            automation=[IrAutomation.from_dict(a) for a in d["automation"]],
        )


@dataclass
class IrSource:
    """Deserialized from tagged enum: {"type": "sample", "path": ..., "label": ...}"""

    source_type: str  # "sample", "synth", "live_input", "resampled"
    path: str | None = None
    preset: str | None = None
    params: list[tuple[str, float]] = field(default_factory=list)
    channel: int | None = None
    patch_id: str | None = None
    label: str = ""

    @classmethod
    def from_dict(cls, d: dict) -> IrSource:
        st = d["type"]
        if st == "sample":
            return cls(source_type=st, path=d["path"], label=d["label"])
        elif st == "synth":
            return cls(
                source_type=st,
                preset=d["preset"],
                params=[(k, v) for k, v in d["params"]],
                label=d["label"],
            )
        elif st == "live_input":
            return cls(source_type=st, channel=d["channel"], label=d["label"])
        elif st == "resampled":
            return cls(source_type=st, patch_id=d["patch_id"], label=d["label"])
        else:
            raise ValueError(f"Unknown source type: {st}")


@dataclass
class IrNode:
    """Deserialized from tagged enum: {"type": "source", "id": 0, ...}"""

    node_type: str  # "source", "pattern", "effect", "chain", "mixer", "split", "merge"
    id: int
    label: str
    source: IrSource | None = None
    clip: IrClip | None = None
    effect: IrEffect | None = None
    effects: list[IrEffect] | None = None
    channels: int | None = None
    outputs: int | None = None
    inputs: int | None = None

    @classmethod
    def from_dict(cls, d: dict) -> IrNode:
        nt = d["type"]
        node = cls(node_type=nt, id=d["id"], label=d["label"])
        if nt == "source":
            node.source = IrSource.from_dict(d["source"])
        elif nt == "pattern":
            node.clip = IrClip.from_dict(d["clip"])
        elif nt == "effect":
            node.effect = IrEffect.from_dict(d["effect"])
        elif nt == "chain":
            node.effects = [IrEffect.from_dict(e) for e in d["effects"]]
        elif nt == "mixer":
            node.channels = d["channels"]
        elif nt == "split":
            node.outputs = d["outputs"]
        elif nt == "merge":
            node.inputs = d["inputs"]
        return node


@dataclass
class IrEdge:
    from_node: int
    from_port: int
    to_node: int
    to_port: int

    @classmethod
    def from_dict(cls, d: dict) -> IrEdge:
        return cls(
            from_node=d["from_node"],
            from_port=d["from_port"],
            to_node=d["to_node"],
            to_port=d["to_port"],
        )


@dataclass
class IrSpace:
    pan: IrAutomation
    width: IrAutomation
    depth: IrAutomation

    @classmethod
    def from_dict(cls, d: dict) -> IrSpace:
        return cls(
            pan=IrAutomation.from_dict(d["pan"]),
            width=IrAutomation.from_dict(d["width"]),
            depth=IrAutomation.from_dict(d["depth"]),
        )


@dataclass
class IrExposedParam:
    name: str
    range: tuple[float, float]
    default: float
    automation: IrAutomation

    @classmethod
    def from_dict(cls, d: dict) -> IrExposedParam:
        return cls(
            name=d["name"],
            range=tuple(d["range"]),
            default=d["default"],
            automation=IrAutomation.from_dict(d["automation"]),
        )


@dataclass
class IrPatch:
    label: str
    bpm: float
    nodes: list[IrNode]
    edges: list[IrEdge]
    space: IrSpace
    exposed_params: list[IrExposedParam]

    @classmethod
    def from_dict(cls, d: dict) -> IrPatch:
        return cls(
            label=d["label"],
            bpm=d["bpm"],
            nodes=[IrNode.from_dict(n) for n in d["nodes"]],
            edges=[IrEdge.from_dict(e) for e in d["edges"]],
            space=IrSpace.from_dict(d["space"]),
            exposed_params=[IrExposedParam.from_dict(p) for p in d["exposed_params"]],
        )

    @classmethod
    def from_json(cls, json_str: str) -> IrPatch:
        import json
        return cls.from_dict(json.loads(json_str))
