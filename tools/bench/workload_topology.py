"""Flowchart topology inputs and structural observations for the public workload suite."""

from random import Random

SHAPES = ("chain", "fan-in", "fan-out", "ternary", "fork-join")
SIZES = (50, 100, 250, 500)
SCC_SHAPES = ("scc", "scc-dag")
SCC_SIZES = (25, 50, 75, 100)


def flowchart_layout_inventory(document):
    """Read the public write_layout_json v1 envelope and retain directed occurrences."""
    if document.get("schema_version") != 1:
        raise ValueError("expected schema_version 1 from the public layout writer")
    if (document.get("family_id") != "flowchart"
            or document.get("diagram_type") not in ("flowchart", "flowchart-v2", "flowchart-elk")):
        raise ValueError("expected a Flowchart family_id and diagram_type in layout JSON")
    graph = document.get("layout", {}).get("FlowchartV2")
    if not graph:
        raise ValueError("comparison requires a Flowchart layout")
    nodes = sorted(n["id"] for n in graph["nodes"] if not n["is_cluster"])
    groups = sorted(cluster["id"] for cluster in graph["clusters"])
    group_ids = set(groups)
    owners = set(nodes) | group_ids
    if any(n["id"] not in group_ids for n in graph["nodes"] if n["is_cluster"]):
        raise ValueError("invalid cluster proxy in Flowchart layout")
    edges = sorted([e["id"], e["from"], e["to"]] for e in graph["edges"])
    if not owners or len(nodes) + len(groups) != len(owners) or any(
        a not in owners or b not in owners for _, a, b in edges
    ):
        raise ValueError("invalid node, group, or edge endpoint in Flowchart layout")
    return dict(nodes=nodes, edges=edges, **({"groups": groups} if groups else {}))


def flowchart_svg_inventory(root):
    """Keep painted occurrences and their text, including repeated public edge IDs."""
    nodes, groups, edges, edge_labels = [], [], [], []
    for element in root.iter():
        kind, owner = element.get("data-et"), element.get("data-id")
        if kind == "edge":
            dom_id = element.get("id")
            if not owner or not dom_id or not element.get("d", "").strip():
                raise ValueError("Flowchart edge lacks an ID or painted path")
            edges.append([owner, dom_id, bool(element.get("marker-start")), bool(element.get("marker-end"))])
        elif kind == "cluster":
            if not owner:
                raise ValueError("Flowchart group lacks an ID")
            titles = [" ".join("".join(title.itertext()).split()) for title in element
                      if "cluster-label" in title.get("class", "").split()]
            groups.append([owner, titles])
        elif kind == "node" or (owner and "label" in element.get("class", "").split()):
            if not owner:
                raise ValueError("Flowchart node lacks an ID")
            texts = [" ".join("".join(label.itertext()).split()) for label in element.iter()
                     if label.tag.rsplit("}", 1)[-1] in ("foreignObject", "text")]
            labels = [text for text in texts if text]
            if kind == "node":
                nodes.append([owner, labels])
            elif labels:
                edge_labels.append([owner, labels])
    if len({edge[1] for edge in edges}) != len(edges):
        raise ValueError("Flowchart edge occurrences share an SVG ID")
    return dict(nodes=sorted(nodes), edges=sorted(edges), edge_labels=sorted(edge_labels),
                **({"groups": sorted(groups)} if groups else {}))


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
