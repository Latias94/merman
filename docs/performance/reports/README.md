# Renderer benchmark results

These reports show how released renderers and identified local builds handle selected Mermaid inputs. Start with the coverage table, then find the operation that matches your use case. A warm Node call, a fresh CLI export and a native layout-work probe answer different questions. Each report names the builds, machine, sample unit and scope of the evidence.

| Measurement date | Report | Coverage |
| --- | --- | --- |
| 2026-11-23 | [Windows x64 comparison](2026-11-23-windows-x64.md) · [Measurements](2026-11-23-windows-x64.json) · [Raw samples and outputs](https://raw.githubusercontent.com/Latias94/merman/d17b7feaf461775b4a27db6edf4b08b21fef12b3/windows-x64-20261123.zip) · [ZIP checksum](https://raw.githubusercontent.com/Latias94/merman/d17b7feaf461775b4a27db6edf4b08b21fef12b3/SHA256SUMS) | 12 Node timing scenarios with startup and peak RSS, a 25-chart CLI document, browser delivery and SVG return times, and 32 native layout stress controls. |

**Other benchmark results.** The earlier [native Rust comparison with mmdr](../renderer_comparison_2026-07-28_75c9fd156_vs_mmdr.md) uses a separate corpus and toolchain. The [alignment dashboard](../../alignment/STATUS.md) tracks Mermaid compatibility. For changes between Merman releases, see the [release comparison guide](../../release/RELEASE_REPORTS.md).

**For maintainers.** Put dated Markdown and its portable measurements here. Use `target/bench` for local collection. For a durable public snapshot, the `benchmark-data` branch holds a dated ZIP with the frozen inputs, samples, retained outputs, replay instructions and `SHA256SUMS`. Link to its full commit SHA. Manual CI runs can link to the corresponding Actions artifact. [Publication procedure](../RUNBOOK.md#publish-a-reader-report)
