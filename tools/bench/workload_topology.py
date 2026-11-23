"""Deterministic, equal-node Flowchart inputs for the public workload suite."""

from random import Random

SHAPES = ("chain", "fan-in", "fan-out", "ternary", "fork-join")
SIZES = (50, 100, 250, 500)
SCC_SHAPES = ("scc", "scc-dag")
SCC_SIZES = (25, 50, 75, 100)


def topology_source(shape: str, nodes: int, order: str = "canonical") -> tuple[str, int]:
    """Return LF-terminated Mermaid source and the number of directed edges."""
    if shape not in SHAPES + SCC_SHAPES:
        raise ValueError(f"unknown topology shape {shape!r}; expected one of {SHAPES + SCC_SHAPES}")
    sizes = SCC_SIZES if shape in SCC_SHAPES else SIZES
    if type(nodes) is not int or nodes not in sizes:
        raise ValueError(f"invalid topology node count {nodes!r}; expected one of {sizes}")

    if shape in SCC_SHAPES:
        # The orientation control reverses just the closing edge of each five-node cycle.
        declarations = [f"  C{i}[C{i}]" for i in range(nodes)]
        edges = []
        for node in range(nodes):
            start = node - node % 5
            if node == start + 4:
                a, b = (node, start) if shape == "scc" else (start, node)
            else:
                a, b = node, node + 1
            edges.append(f"  C{a}-->C{b}")
            if node + 5 < nodes:
                edges.append(f"  C{node}-->C{node + 5}")
        if order != "canonical":
            axis, _, permutation = order.partition("-")
            if axis not in ("declarations", "edges") or permutation not in ("reverse", "1", "2", "3"):
                raise ValueError(f"unknown SCC statement order {order!r}")
            statements = declarations if axis == "declarations" else edges
            if permutation == "reverse":
                statements.reverse()
            else:
                Random(int(permutation)).shuffle(statements)
        return "\n".join(["flowchart TD", *declarations, *edges, ""]), len(edges)

    if order != "canonical":
        raise ValueError(f"statement order variants require an SCC topology, got {shape!r}")
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
