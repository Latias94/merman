# HPD-080 - Zed Theme Feedback And Flowchart Stack Audit

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## External Signal

- Zed PR: <https://github.com/zed-industries/zed/pull/57967>
- Reviewer color feedback:
  <https://github.com/zed-industries/zed/pull/57967#issuecomment-4598335939>
- Follow-up color cleanup:
  <https://github.com/zed-industries/zed/pull/57967#issuecomment-4599604388>
- Zed PR `58325`: <https://github.com/zed-industries/zed/pull/58325>
- Zed merman fork fix:
  <https://github.com/zed-industries/merman/commit/1c765dcca2ef5092fcde7bebe8374819563623ef>

## Theme Finding

The current 0.7 theme surface covers common product-neutral host needs:

- official Mermaid theme selection and `themeVariables` through Rust `with_site_config(...)` or
  binding `options_json.site_config`,
- Mermaid-owned `themeCSS`,
- host-owned scoped CSS with optional `!important` stripping,
- `resvg-safe` output,
- optional duplicate native/fallback label cleanup,
- optional root canvas background replacement.

Zed's exact background, edge-label, tag-label, and accent cleanup remains host palette policy. It
should not become default merman output unless a common product-neutral contract emerges.

## Stack Finding

The same Zed PR thread pointed at a separate blocker: Zed PR `58325` fixed a stack overflow in
deeply nested Flowchart subgraphs on their fork. The local 0.7 renderer still had the same class of
unbounded recursive traversal in Flowchart cluster handling.

## Change

- Converted Flowchart effective-dir parent traversal to an iterative memoized walk.
- Converted `extract_descendants(...)` to an explicit stack.
- Converted non-cluster anchor discovery to an explicit leaf traversal.
- Converted `copy_cluster(...)` to an explicit post-order stack while preserving owner-cluster
  parent assignment semantics.
- Added a 512 KB stack-thread regression covering 10,000 nested subgraphs across all four traversal
  seams.

This does not change the bounded Mermaid-style recursive cluster layout depth rule; it removes
unnecessary Rust call-stack dependence from ordinary graph tree traversals.

## Verification

- `gh pr view 57967 --repo zed-industries/zed --json title,url,commits,comments,reviews,files`
- `gh api repos/zed-industries/zed/commits/c85f29cd2e78ec8a68b20349606d8298eecf37bb --jq '.files[] | select(.filename|test("mermaid_render/src/postprocess")) | {filename, patch}'`
- `gh issue view 58325 --repo zed-industries/zed --json title,url,state,body,comments,labels`
- `gh api repos/zed-industries/merman/commits/1c765dcca2ef5092fcde7bebe8374819563623ef --jq '.files[] | {filename, patch}'`
- `npm run build:ts --prefix platforms/web`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-bindings-core`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render drop_native_duplicate_fallbacks root_background scoped_css`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render flowchart_cluster_traversals_handle_deep_subgraphs_with_small_stack`
- `cargo fmt --check -p merman-render`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test flowchart_layout_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test flowchart_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test zed_pr_57644_corpus`

## Residual

- Common theme needs are covered, but arbitrary element/inline-style palette rewriting stays a Rust
  custom-postprocessor or host-side responsibility.
- The deep-subgraph regression covers traversal stack safety, not performance tuning for enormous
  diagrams.
