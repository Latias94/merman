//! Property-local ownership and measurement evidence for retained XHTML labels.

use super::FlowchartCompiledStyles;
use crate::flowchart::FlowchartSourceFacetStatus;
use crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt;
use crate::text::VisibleTextStyleFacts;

pub(in crate::svg::parity::flowchart) fn html_label_typography_receipt(
    receipt: FlowchartNodeLabelEmissionReceipt,
    styles: &FlowchartCompiledStyles,
    html: &str,
) -> FlowchartNodeLabelEmissionReceipt {
    let facts = VisibleTextStyleFacts::from_xhtml_fragment(html);
    let inherited_stack = facts
        .inherited_font_family_run_count()
        .saturating_sub(intrinsic_monospace_family_runs(html));
    let visible = facts.parse_valid() && facts.has_visible_runs();
    let stack_source = if visible && inherited_stack == 0 {
        if facts.unverified_font_family_run_count() == 0 {
            // Intrinsic monospace families are shared by HTML measurement and the native cascade.
            FlowchartSourceFacetStatus::Admitted
        } else {
            FlowchartSourceFacetStatus::Unverified
        }
    } else {
        FlowchartSourceFacetStatus::Absent
    };
    let size_transport_verified = styles.font_size_source.as_ref().is_none_or(|source| {
        // The HTML shell repeats this declaration. Only an absolute source winner is
        // idempotent across those ancestors; relative sizes retain their source residual.
        source.admitted
            && crate::mermaid_style::parse_svg_number_or_px(source.prepared.value()).is_some()
    });
    let size_source =
        if !size_transport_verified || (visible && facts.inherited_font_size_run_count() == 0) {
            FlowchartSourceFacetStatus::Unverified
        } else {
            FlowchartSourceFacetStatus::Absent
        };
    let stack_verified = receipt
        .prepared_font_stack_reach()
        .is_some_and(|reach| reach.is_verified())
        && facts.parse_valid()
        && facts.unverified_font_family_run_count() == 0;
    let size_verified = receipt
        .prepared_font_size_reach()
        .is_some_and(|reach| reach.is_verified())
        && facts.parse_valid()
        && facts.unverified_font_size_run_count() == 0
        && size_transport_verified;
    receipt
        .with_prepared_font_reaches(
            receipt.typography_applicable(),
            stack_verified,
            size_verified,
        )
        .with_local_font_source_ownership(stack_source, size_source)
}

/// Count runs that inherit the outer family except for intrinsic HTML monospace.
/// Other descendant declarations remain owned by the shared visible-run facts.
fn intrinsic_monospace_family_runs(html: &str) -> usize {
    if ![b"<code".as_slice(), b"<kbd", b"<samp", b"<tt"]
        .iter()
        .any(|tag| super::contains_ascii_case_insensitive(html, tag))
    {
        return 0;
    }
    let wrapped = format!("<merman-label-fonts>{html}</merman-label-fonts>");
    let Ok(document) = crate::xml_dom::Document::parse(&wrapped) else {
        return 0;
    };
    let mut parents = Vec::<(crate::xml_dom::NodeId, (bool, bool))>::new();
    let mut monospace_runs = 0;
    for node in document.root_element().descendants() {
        let parent = node.parent().map(|parent| parent.id());
        while parents.last().map(|(id, _)| *id) != parent && !parents.is_empty() {
            parents.pop();
        }
        let inherited = parents.last().map_or((true, true), |(_, owners)| *owners);
        if node.is_element() {
            let has_class = node
                .attribute("class")
                .is_some_and(|class| !class.trim().is_empty());
            let has_face = node.tag_name().name().eq_ignore_ascii_case("font")
                && node
                    .attribute("face")
                    .is_some_and(|face| !face.trim().is_empty());
            let local = (
                inherited.0 && !has_class && !has_face,
                inherited.1
                    && !has_class
                    && !has_face
                    && crate::text::xhtml_font_default(node.tag_name().name(), "font-family")
                        != Some("monospace"),
            );
            let owners = match node
                .attribute("style")
                .and_then(crate::mermaid_style::css_font_family_override)
            {
                None => local,
                Some(override_)
                    if override_.ownership()
                        == crate::mermaid_style::CssFontFamilyOwnership::Inherited
                        && (!has_class || override_.important()) =>
                {
                    inherited
                }
                Some(_) => (false, false),
            };
            parents.push((node.id(), owners));
        } else if node.is_text()
            && inherited.0
            && !inherited.1
            && !node
                .parent()
                .is_some_and(|parent| parent.tag_name().name().eq_ignore_ascii_case("style"))
            && node.text().is_some_and(|text| {
                !crate::text::trim_html_collapsible_ascii_whitespace(text).is_empty()
            })
        {
            monospace_runs += 1;
        }
    }
    monospace_runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intrinsic_monospace_ownership_keeps_author_overrides_and_mixed_runs() {
        for tag in ["code", "kbd", "samp", "tt"] {
            assert_eq!(
                intrinsic_monospace_family_runs(&format!("<span>A</span><{tag}>B</{tag}>")),
                1,
                "{tag}"
            );
            assert_eq!(
                intrinsic_monospace_family_runs(&format!(
                    "<{tag} style='font-family:inherit'>B</{tag}>"
                )),
                0,
                "{tag}"
            );
            assert_eq!(
                intrinsic_monospace_family_runs(&format!(
                    "<{tag} style='font-family:serif'>B</{tag}>"
                )),
                0,
                "{tag}"
            );
        }
    }
}
