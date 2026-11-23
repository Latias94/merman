import re
import unittest

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


if __name__ == "__main__":
    unittest.main()
