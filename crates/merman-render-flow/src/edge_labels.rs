//! Mermaid edge-label layers and complete SVG edge labels for the Flow families.

use merman_render_core::{Result, svg_output::SvgOutput};

pub mod svg;

const EMPTY_BACKGROUND: &str = r#"<g><rect class="background" style="stroke: none"/></g>"#;

/// Append the ordinary label layer using the caller's prepared edge occurrences.
///
/// An empty slice emits the self-closing group. The caller owns the sink checkpoint after
/// the layer tail, including the empty case used by Swimlane's synthetic labels.
pub fn write_edge_labels<S: SvgOutput, Edge: Copy>(
    out: &mut S,
    edges: &[Edge],
    html_labels: bool,
    merge_self_loop_segments: bool,
    checkpoint_emit: impl Fn() -> Result<()>,
    label_is_empty: impl Fn(Edge) -> bool,
    mut write_label: impl FnMut(&mut S, Edge) -> Result<()>,
) -> Result<()> {
    if edges.is_empty() {
        out.push_str(r#"<g class="edgeLabels"/>"#);
        return Ok(());
    }

    out.push_str(r#"<g class="edgeLabels">"#);
    if !html_labels && !merge_self_loop_segments {
        // Agentflow's unmerged Graphlib edges keep each empty background after its label.
        for &edge in edges {
            checkpoint_emit()?;
            write_label(out, edge)?;
            if label_is_empty(edge) {
                out.push_str(EMPTY_BACKGROUND);
                out.checkpoint()?;
            }
            out.checkpoint()?;
        }
    } else {
        if !html_labels {
            // SVG createText leaves the unused background wrappers ahead of the labels.
            for &edge in edges {
                checkpoint_emit()?;
                if label_is_empty(edge) {
                    out.push_str(EMPTY_BACKGROUND);
                    out.checkpoint()?;
                }
            }
        }
        // HTML keeps zero-sized labels in graph edge order and skips the background pass.
        for &edge in edges {
            checkpoint_emit()?;
            write_label(out, edge)?;
            out.checkpoint()?;
        }
    }
    out.push_str("</g>");
    Ok(())
}

/// Append the ELK label layer after the caller's entry Emit checkpoint.
///
/// A nonempty edge list uses a tag pair even when every label is empty. ELK checks the sink
/// immediately after either form of the group tail.
pub fn write_elk_edge_labels<S: SvgOutput, Edge: Copy>(
    out: &mut S,
    edges: &[Edge],
    checkpoint_emit: impl Fn() -> Result<()>,
    label_is_empty: impl Fn(Edge) -> bool,
    mut write_label: impl FnMut(&mut S, Edge) -> Result<()>,
) -> Result<()> {
    if edges.is_empty() {
        out.push_str(r#"<g class="edgeLabels"/>"#);
        return out.checkpoint();
    }

    out.push_str(r#"<g class="edgeLabels">"#);
    for &edge in edges {
        checkpoint_emit()?;
        if label_is_empty(edge) {
            continue;
        }
        write_label(out, edge)?;
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_render_core::{
        Error,
        resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId},
        svg_output::BoundedSvgOutput,
    };

    #[test]
    fn elk_checks_label_group_tails_and_ordinary_leaves_them_for_the_caller() {
        let cases: [(&[()], &str); 2] = [
            (&[], r#"<g class="edgeLabels"/>"#),
            (&[()], r#"<g class="edgeLabels"></g>"#),
        ];
        for (edges, expected) in cases {
            for elk in [false, true] {
                let limit = expected.len() - 1;
                let meter = OperationWorkMeter::new(
                    RenderResourcePolicy::unbounded_for_trusted_input()
                        .with_limit(ResourceLimitId::MaxSvgBytes, limit)
                        .unwrap(),
                );
                let mut out = BoundedSvgOutput::new(&meter);
                let error = if elk {
                    write_elk_edge_labels(
                        &mut out,
                        edges,
                        || Ok(()),
                        |()| true,
                        |_, ()| panic!("ELK skips empty labels"),
                    )
                    .expect_err("ELK checks the label group tail")
                } else {
                    write_edge_labels(
                        &mut out,
                        edges,
                        true,
                        true,
                        || Ok(()),
                        |()| panic!("HTML skips the empty-label background scan"),
                        |_, ()| Ok(()),
                    )
                    .expect("ordinary label groups leave the final check for the caller");
                    out.checkpoint()
                        .expect_err("the caller observes the rejected label group tail")
                };
                let Error::ResourceLimitExceeded(details) = error else {
                    panic!("expected the SVG byte ceiling, got {error}");
                };
                assert_eq!(details.limit, "max_svg_bytes");
                assert_eq!(details.max, limit);
            }
        }
    }
}
