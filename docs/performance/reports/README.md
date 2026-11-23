# Renderer benchmark results

These reports show how released renderers and identified local builds handle selected Mermaid inputs. Start with the coverage table, then find the operation that matches your use case. A warm Node call, a fresh CLI export and a native layout-work probe answer different questions. Each report names the builds, machine, sample unit and scope of the evidence.

| Measurement date | Report | Coverage |
| --- | --- | --- |
| 2026-11-23 | [Windows x64 renderer comparison](2026-11-23-windows-x64.md) · [Measurements](2026-11-23-windows-x64.json) | Merman, Pele and Mermaid v13 in Node, Merman CLI and mmdc file exports, browser Merman Web and Mermaid.js, and native Merman layout work. |

**Other benchmark results.** The earlier [native Rust comparison with mmdr](../renderer_comparison_2026-07-28_75c9fd156_vs_mmdr.md) uses a separate corpus and toolchain. The [alignment dashboard](../../alignment/STATUS.md) tracks Mermaid compatibility. For changes between Merman releases, see the [release comparison guide](../../release/RELEASE_REPORTS.md).

**For maintainers.** Put dated reader-facing Markdown here, beside its portable measurements in JSON. Keep raw bundles, SVGs, PNGs and logs under `target/bench` or in a linked CI artifact. Add a row above for each published snapshot. The [publication procedure](../RUNBOOK.md#publish-a-reader-report) covers collection, generation and verification.
