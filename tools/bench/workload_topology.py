"""Deterministic, equal-node Flowchart inputs for the public workload suite."""

SHAPES = ("chain", "fan-in", "fan-out", "ternary", "fork-join")
SIZES = (50, 100, 250, 500)


def topology_source(shape: str, nodes: int) -> tuple[str, int]:
    """Return LF-terminated Mermaid source and the number of directed edges."""
    if shape not in SHAPES:
        raise ValueError(f"unknown topology shape {shape!r}; expected one of {SHAPES}")
    if not isinstance(nodes, int) or nodes not in SIZES:
        raise ValueError(f"invalid topology node count {nodes!r}; expected one of {SIZES}")

    if shape == "chain":
        edges = [(i - 1, i) for i in range(1, nodes)]
    elif shape == "fan-in":
        edges = [(i, 0) for i in range(1, nodes)]
    elif shape == "fan-out":
        edges = [(0, i) for i in range(1, nodes)]
    elif shape == "ternary":
        edges = [((i - 1) // 3, i) for i in range(1, nodes)]
    else:
        # Each intermediate node makes one independent path from the source to the sink.
        edges = []
        for i in range(1, nodes - 1):
            edges.extend(((0, i), (i, nodes - 1)))

    source = "flowchart TD\n" + "".join(f"N{a} --> N{b}\n" for a, b in edges)
    return source, len(edges)
