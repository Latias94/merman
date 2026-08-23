# ASCII Semantic-Depth Benchmark Projection

Status: current projection; the canonical evidence record is [ASCII_U25_CLOSEOUT_RECEIPT.md](../rendering/ASCII_U25_CLOSEOUT_RECEIPT.md).

## Source and contract

- Product source: `861fc0ba...`
- Product tree: `fc552d5c...`
- Entry point: `Renderer::render(RenderRequest::ascii(...))`
- Harness: `tools/bench/compare_self.py` schema v2
- Corpus: `tools/bench/ascii_corpus.json`
- Durable evidence: Markdown projections under `docs/performance/evidence/ascii-u25/861fc0ba3/`
- Raw schema-v2 JSON: build artifacts under `target/performance/u25/`; they are identified by SHA-256 in the canonical receipt and are not committed.

The 50 microsecond value is a preregistered materiality boundary for this low-latency lane. It is not a repository-wide hard pass/fail threshold; other benchmark lanes may use a profile-specific formula or structural objective.

## Current disposition

| Lane | Result | Maintainer disposition |
| --- | --- | --- |
| `sequence_medium` | Relative interval inconclusive; absolute interval `41.10..44.85 us` | Accepted below this lane's materiality boundary; relative slowdown is not claimed to be zero. |
| `class_medium` | Relative interval inconclusive; absolute interval `32.53..35.91 us` | Accepted below this lane's materiality boundary; relative slowdown is not claimed to be zero. |
| `flowchart_large` | A/A calibration inconclusive | Accepted as insufficient calibration evidence. |
| `sequence_mermaid_api_large` | A/A calibration inconclusive | Accepted as insufficient calibration evidence. |
| `class_large` | A/A calibration inconclusive | Accepted as insufficient calibration evidence. |
| `er_large` | Same-source A/A stability observed | Retained as supporting evidence; this is not a cross-version product comparison. |
| `xychart_large` | Same-source A/A stability observed | Retained as supporting evidence; this is not a cross-version product comparison. |

All comparable rows had zero contract failures and matched output identities. Changed-output baseline rows are excluded from causal performance claims. No universal performance pass is asserted.

For exact commands, environment, hashes, sample configuration, and raw-artifact sizes, see the canonical receipt.
