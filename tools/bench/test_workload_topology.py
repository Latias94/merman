import re
import unittest
from graphlib import CycleError, TopologicalSorter

if __package__:
    from .workload_topology import topology_source
else:
    from workload_topology import topology_source


class TopologySourceTests(unittest.TestCase):
    def test_graphs_have_the_declared_size_and_topology(self) -> None:
        for nodes in (50, 100, 250, 500):
            for shape in ("chain", "fan-in", "fan-out", "ternary", "fork-join"):
                with self.subTest(nodes=nodes, shape=shape):
                    source, count = topology_source(shape, nodes)
                    self.assertEqual((source, count), topology_source(shape, nodes))
                    self.assertTrue(source.endswith("\n"))
                    header, *lines = source.removesuffix("\n").split("\n")
                    self.assertEqual(header, "flowchart TD")
                    incoming = [set() for _ in range(nodes)]
                    outgoing = [set() for _ in range(nodes)]
                    for line in lines:
                        match = re.fullmatch(r"N(\d+) --> N(\d+)", line)
                        self.assertIsNotNone(match, line)
                        a, b = map(int, match.groups())
                        self.assertLess(max(a, b), nodes)
                        self.assertNotIn(b, outgoing[a])
                        outgoing[a].add(b)
                        incoming[b].add(a)
                    self.assertEqual(len(lines), count)
                    self.assertTrue(all(ins or outs for ins, outs in zip(incoming, outgoing)))

                    if shape == "fork-join":
                        self.assertEqual(count, 2 * (nodes - 2))
                        branches = set(range(1, nodes - 1))
                        self.assertEqual(outgoing[0], branches)
                        self.assertEqual(incoming[-1], branches)
                        for branch in branches:
                            self.assertEqual(incoming[branch], {0})
                            self.assertEqual(outgoing[branch], {nodes - 1})
                    else:
                        self.assertEqual(count, nodes - 1)
                        for i in range(1, nodes):
                            if shape == "fan-in":
                                self.assertEqual(outgoing[i], {0})
                            elif shape == "fan-out":
                                self.assertEqual(incoming[i], {0})
                            elif shape == "chain":
                                self.assertEqual(incoming[i], {i - 1})
                            else:
                                self.assertEqual(incoming[i], {(i - 1) // 3})

    def test_invalid_inputs_name_the_offending_parameter(self) -> None:
        with self.assertRaisesRegex(ValueError, "shape.*tree"):
            topology_source("tree", 50)
        for nodes in (0, 49, 501, True, 50.0, "50"):
            with self.subTest(nodes=nodes), self.assertRaisesRegex(ValueError, "node count"):
                topology_source("chain", nodes)
        with self.assertRaisesRegex(ValueError, "SCC"):
            topology_source("chain", 50, "edges-1")
        for bad in ("nodes-2", "edges-0", "declarations-1-extra"):
            with self.subTest(bad=bad), self.assertRaisesRegex(ValueError, "statement order"):
                topology_source("scc", 75, bad)

    def test_statement_order_permutations_preserve_each_labeled_directed_graph(self) -> None:
        for shape in ("scc", "scc-dag"):
            for n in (75, 100):
                original, edge_count = topology_source(shape, n)
                reference = original.splitlines()
                seen = {original}
                for axis in ("declarations", "edges"):
                    for permutation in ("reverse", "1", "2", "3"):
                        with self.subTest(shape=shape, n=n, axis=axis, permutation=permutation):
                            text, count = topology_source(shape, n, f"{axis}-{permutation}")
                            self.assertEqual((len(text.encode()), count), (len(original.encode()), edge_count))
                            self.assertNotIn(text, seen)
                            seen.add(text)
                            lines = text.splitlines()
                            self.assertEqual(lines[0], reference[0])
                            changed, fixed = slice(1, n + 1), slice(n + 1, None)
                            if axis == "edges":
                                changed, fixed = fixed, changed
                            self.assertEqual(lines[fixed], reference[fixed])
                            self.assertCountEqual(lines[changed], reference[changed])
                            if permutation == "reverse":
                                self.assertEqual(lines[changed], reference[changed][::-1])

    def test_scc_control_reverses_only_the_closing_edges(self) -> None:
        for nodes in (25, 50, 75, 100):
            with self.subTest(nodes=nodes):
                cyclic, acyclic = (topology_source(shape, nodes) for shape in ("scc", "scc-dag"))
                self.assertEqual((cyclic[1], len(cyclic[0])), (acyclic[1], len(acyclic[0])))
                parsed = []
                for source, count in (cyclic, acyclic):
                    lines = source.splitlines()
                    self.assertEqual(lines[1:nodes + 1], [f"  C{i}[C{i}]" for i in range(nodes)])
                    edges = [tuple(map(int, re.fullmatch(r"  C(\d+)-->C(\d+)", line).groups()))
                             for line in lines[nodes + 1:]]
                    self.assertEqual(len(edges), count)
                    self.assertEqual(len(set(edges)), 2 * nodes - 5)
                    parsed.append(edges)
                changes = [(a, b) for a, b in zip(*parsed, strict=True) if a != b]
                self.assertEqual(changes, [((i + 4, i), (i, i + 4)) for i in range(0, nodes, 5)])
                for i in range(nodes):
                    if i % 5 < 4:
                        self.assertIn((i, i + 1), parsed[0])
                    if i + 5 < nodes:
                        self.assertIn((i, i + 5), parsed[0])
                dag, cycles = TopologicalSorter(), TopologicalSorter()
                for edge, control in zip(*parsed, strict=True):
                    cycles.add(edge[1], edge[0])
                    dag.add(control[1], control[0])
                self.assertEqual(len(tuple(dag.static_order())), nodes)
                with self.assertRaises(CycleError):
                    cycles.prepare()


if __name__ == "__main__":
    unittest.main()
