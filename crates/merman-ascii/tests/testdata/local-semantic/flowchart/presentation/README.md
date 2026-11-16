# Terminal presentation corpus

These original inputs establish information and readability requirements for terminal graph
planning. They are not copied reference fixtures or exact-output oracles. Preserve the existing
upstream fixture provenance independently.

The renderer may simplify outlines, share compatible trunks, or move full labels into uniquely
keyed details. Evaluate the complete emitted document. A shared arrow is acceptable when the
relationships remain clear; a compact drawing that implies false relationships is not.

| Input | Information that readers must retain |
| --- | --- |
| `chain.mmd` | Read, Parse, Validate, Plan, and Render form one ordered chain. |
| `five-edge.mmd` | The five direct edges are A→B, A→C, B→C, B→D, and D→C. Reachability alone is insufficient. |
| `fork-merge.mmd` | Start branches to Left and Right; each branch continues to Finish. |
| `fan-out-12-lr.mmd` | Dispatcher sends to each of twelve distinct jobs. A long individual wire per target is unnecessary. |
| `fan-out-16-td.mmd` | Dispatcher sends to all sixteen jobs. A width fallback must remain readable. |
| `fan-in-12-lr.mmd` | Each of twelve distinct sources sends to Sink. |
| `labeled-branches.mmd` | Valid leads to Execute, invalid to Reject, and retry to Queue; each continues to Report. Retain the decision/type cue. |
| `feedback.mmd` | Validate retries Parse; Write failure returns to Input. Do not imply the two other source/target pairings. |
| `group-member-connection.mmd` | East contains Ingress and Worker; West contains Broker and Store; Worker sends jobs to Broker. |
| `group-nonterminal-member.mmd` | The same groups instead have Ingress sending jobs to Store. A frame-level substitute must not make this look like the preceding input. |
| `long-labels.mmd` | Preserve the complete two branch conditions, node identities, and final merge; full labels may use a nearby key. |
| `distinct-suffixes.mmd` | The targets are PRODUCTION and STAGING. Truncating the shared prefix into two indistinguishable names fails. |
| `cjk.mmd` | 校验结果 branches through 通过 to 写入队列 and through 失败 to 返回错误; preserve terminal-cell alignment. |

Compare equivalent requests with pinned Grok terminal Mermaid and mermaid-ascii on their supported
subset. Record native unbounded output and an explicit 80-column request; native overflow behavior
may differ and must stay visible. Inspect total rows, long tracing spans, ambiguous crossings, and
detail lookups alongside width. Dimensions and exit status alone do not establish a quality pass.

The corpus is the initial comparison subset, not a completed replacement for State, Swimlane,
marker-kind, all-direction, generated, or resource-boundary acceptance. Existing tests continue to
own those contracts. Local adapters, binary hashes, raw captures, and comparison galleries belong
under ignored `target/`; release checks must not require local reference checkouts.
