use crate::Result;
use crate::svg::pipeline::SvgPostprocessExecution;
use merman_render_core::svg_css::lower_id_suffix_selectors;
use std::borrow::Cow;

pub(crate) fn apply_lower_id_suffix_selectors<'a>(
    svg: Cow<'a, str>,
    execution: SvgPostprocessExecution<'_>,
) -> Result<Cow<'a, str>> {
    lower_id_suffix_selectors(svg, execution.work_meter())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::RenderEnvironment;
    use crate::svg::pipeline::{ScopedCssPostprocessor, SvgPipeline};

    fn lower(svg: &str) -> String {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        apply_lower_id_suffix_selectors(Cow::Borrowed(svg), SvgPostprocessExecution::new(&session))
            .unwrap()
            .into_owned()
    }

    #[test]
    fn expands_actual_ids_and_keeps_selector_context_and_declarations() {
        let svg = r#"<svg><style>defs [id$='-arrowhead'] path,[id$='-missing'],.fallback{fill:transparent!important;stroke:red}</style><g id='b-arrowhead'/><g id='a-arrowhead'/></svg>"#;
        let out = lower(svg);
        let doc = crate::xml_dom::Document::parse(&out).unwrap();
        let css = doc
            .descendants()
            .find(|n| n.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(
            css,
            r#"defs [id="a-arrowhead"] path,defs [id="b-arrowhead"] path,.fallback{fill:transparent!important;stroke:red}"#
        );
        assert_eq!(lower(&out), out);
    }

    #[test]
    fn drops_only_unmatched_branches_including_empty_suffix() {
        let out = lower(
            r#"<svg><style>[id$='-missing']{fill:red}[id$=''],.aggregation{fill:transparent}</style></svg>"#,
        );
        assert_eq!(
            out,
            "<svg><style>.aggregation{fill:transparent}</style></svg>"
        );
    }

    #[test]
    fn preserves_other_selectors_and_does_not_rewrite_declaration_strings() {
        let svg = r#"<svg><style>[ID$='-end'],[data-id$='-end'],:is([id$='-end']),[id$='-end' i],[id$='-end'] [id$='-start']{content:'[id$="-end"]'}</style></svg>"#;
        assert_eq!(lower(svg), svg);
    }

    #[test]
    fn handles_cdata_entities_and_escaped_id_values() {
        let svg = r#"<svg><style><![CDATA[[id$='-end']{fill:red}]]></style><path id='a&quot;&amp;-end'/></svg>"#;
        let out = lower(svg);
        let doc = crate::xml_dom::Document::parse(&out).unwrap();
        let css = doc
            .descendants()
            .find(|n| n.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(css, r#"[id="a\"&-end"]{fill:red}"#);
    }

    #[test]
    fn only_resvg_safe_lowers_user_styles_after_scoping() {
        let svg = "<svg id='diagram'><path id='diagram-aggregationEnd'/></svg>";
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let css = r#"[id$="-aggregationEnd"]{fill:transparent!important}"#;
        let parity = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(css))
            .process_to_string(svg, &session)
            .unwrap();
        assert!(parity.contains("$="));
        let readable = SvgPipeline::readable()
            .with_postprocessor(ScopedCssPostprocessor::new(css))
            .process_to_string(svg, &session)
            .unwrap();
        assert!(readable.contains("$="));
        let safe = SvgPipeline::resvg_safe()
            .with_postprocessor(ScopedCssPostprocessor::new(css))
            .process_to_string(svg, &session)
            .unwrap();
        let doc = crate::xml_dom::Document::parse(&safe).unwrap();
        let css = doc
            .descendants()
            .find(|n| n.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert!(
            css.contains(r#"#diagram [id="diagram-aggregationEnd"]"#),
            "{css}"
        );
        assert!(!css.contains("$="));
    }
    #[test]
    fn resvg_safe_resolves_rebased_marker_ids() {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let svg = r#"<svg id="diagram"><style>#diagram [id$="-aggregationEnd"]{fill:transparent}</style><path id="diagram-aggregationEnd"/></svg>"#;
        let out = SvgPipeline::resvg_safe()
            .with_rebased_ids("embed")
            .process_to_string(svg, &session)
            .unwrap();
        let doc = crate::xml_dom::Document::parse(&out).unwrap();
        let id = doc
            .descendants()
            .find(|n| n.has_tag_name("path"))
            .unwrap()
            .attribute("id")
            .unwrap();
        let css = doc
            .descendants()
            .find(|n| n.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert!(css.contains(&format!("[id=\"{id}\"]")), "{css}");
        assert!(!css.contains("$="));
    }
}
