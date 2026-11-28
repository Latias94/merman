use super::css_override::CssOverridePolicy;
use crate::Result;
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};
use merman_render_core::svg_css::apply_scoped_css;
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct ScopedCssPostprocessor {
    css: String,
    override_policy: CssOverridePolicy,
    merge_into_existing_style: bool,
}

impl ScopedCssPostprocessor {
    pub fn new(css: impl Into<String>) -> Self {
        Self {
            css: css.into(),
            override_policy: CssOverridePolicy::Preserve,
            merge_into_existing_style: false,
        }
    }

    pub fn with_override_policy(mut self, policy: CssOverridePolicy) -> Self {
        self.override_policy = policy;
        self
    }

    pub fn css(&self) -> &str {
        &self.css
    }

    pub fn override_policy(&self) -> CssOverridePolicy {
        self.override_policy
    }

    pub fn with_existing_style_merge(mut self) -> Self {
        self.merge_into_existing_style = true;
        self
    }
}

impl SvgPostprocessor for ScopedCssPostprocessor {
    fn name(&self) -> &'static str {
        "scoped-css"
    }

    fn scoped_css(&self) -> Option<&str> {
        Some(&self.css)
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        apply_scoped_css(
            svg,
            &self.css,
            ctx.svg_id(),
            self.override_policy,
            self.merge_into_existing_style,
            ctx.execution().work_meter(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;
    use crate::environment::RenderEnvironment;
    use crate::resources::{RenderResourcePolicy, ResourceLimitId, ResourceLimitPhase};
    use crate::svg::pipeline::{SvgPipeline, SvgPipelinePreset, SvgPostprocessMetadata};

    fn render_session() -> crate::environment::RenderSession {
        crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap()
    }

    #[test]
    fn scoped_css_injects_after_root_svg_tag_when_no_style_exists() {
        let svg = r#"<svg id="diagram"><rect class="node"/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                ".node rect, text.label { fill: red; }",
            ))
            .process_to_string(svg, &session)
            .unwrap();

        assert!(out.starts_with(r#"<svg id="diagram"><style"#));
        assert!(out.contains("#diagram .node rect, #diagram text.label { fill: red; }"));
    }

    #[test]
    fn scoped_css_injects_after_existing_style_for_cascade_order() {
        let svg =
            r#"<svg id="diagram"><style>#diagram .node rect { fill: red; }</style><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(".node rect { fill: green; }"))
            .process_to_string(svg, &session)
            .unwrap();

        let existing = out.find("fill: red").unwrap();
        let injected = out.find("fill: green").unwrap();
        assert!(
            existing < injected,
            "injected CSS should follow Mermaid CSS for cascade order: {out}"
        );
    }

    #[test]
    fn scoped_css_can_merge_into_mermaid_generated_stylesheet() {
        let svg = r#"<svg id="diagram"><style>#diagram{fill:red;}</style><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(
                ScopedCssPostprocessor::new(".node { fill: green; }").with_existing_style_merge(),
            )
            .process_to_string(svg, &session)
            .unwrap();

        assert_eq!(out.matches("<style").count(), 1);
        assert!(out.contains("#diagram{fill:red;}#diagram .node { fill: green; }</style>"));
        assert!(!out.contains("data-merman-postprocess"));
    }

    #[test]
    fn scoped_css_merge_targets_mermaids_first_global_stylesheet() {
        let svg = r#"<svg id="diagram"><style>.global{fill:red;}</style><g><style>.nested{fill:blue;}</style></g></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(
                ScopedCssPostprocessor::new(".node { fill: green; }").with_existing_style_merge(),
            )
            .process_to_string(svg, &session)
            .unwrap();

        assert!(out.contains(".global{fill:red;}#diagram .node { fill: green; }</style>"));
        assert!(out.contains("<style>.nested{fill:blue;}</style>"));
    }

    #[test]
    fn scoped_css_can_strip_existing_important_before_injection() {
        let svg = r#"<svg id="diagram"><style>.node{fill:red !important;}</style></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(
                ScopedCssPostprocessor::new(".node { fill: green; }")
                    .with_override_policy(CssOverridePolicy::StripExistingImportant),
            )
            .process_to_string(svg, &session)
            .unwrap();

        assert!(!out.contains("!important"));
        assert!(out.contains("#diagram .node { fill: green; }"));
    }

    #[test]
    fn scoped_css_matches_mermaid_ampersand_selector_namespace() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                ":not(&){background:green !important}",
            ))
            .process_to_string(svg, &session)
            .unwrap();

        assert!(out.contains("#diagram :not(#diagram) {background:green !important}"));
    }

    #[test]
    fn scoped_css_pipeline_preserves_xml_whitespace_and_removes_forbidden_scalars() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                "& \n\t \r \r\n + *{color: red;}",
            ))
            .process_to_string(svg, &session)
            .expect("XML-compatible CSS whitespace should remain injectable");
        assert!(out.contains("#diagram #diagram \n\t \r \r\n + * {color: red;}"));
        crate::xml_dom::Document::parse(&out).expect("injected CSS must remain valid XML");

        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                "& \n\t \r \u{000C} \r\n + *{color: red;}",
            ))
            .process_to_string(svg, &session)
            .expect("forbidden XML scalars are removed at the opaque output boundary");
        assert!(!out.contains('\u{000C}'));
        assert!(out.contains("color: red;"));
        crate::xml_dom::Document::parse(&out).expect("injected CSS must remain valid XML");
    }

    #[test]
    fn scoped_css_scopes_nested_grouping_at_rules_and_drops_unsupported_rules() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                "@layer theme; @import url('https://example.test/styles.css'); @media (max-width: 600px) { * { fill: red; } } @supports selector(h2 > p) { h2 > p { color: red; } }",
            ))
            .process_to_string(svg, &session)
            .unwrap();

        assert!(out.contains("@layer theme;"));
        assert!(!out.contains("@import"));
        assert!(out.contains("@media (max-width: 600px) {"));
        assert!(out.contains("#diagram * { fill: red; }"));
        assert!(out.contains("@supports selector(h2 > p) {"));
        assert!(out.contains("#diagram h2 > p { color: red; }"));
    }

    #[test]
    fn scoped_css_keeps_keyframes_unscoped_like_mermaid() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                "@keyframes dash { to { stroke-dashoffset: 1000; } } .edge { animation: dash 1s; }",
            ))
            .process_to_string(svg, &session)
            .unwrap();

        assert!(out.contains("@keyframes dash { to { stroke-dashoffset: 1000; } }"));
        assert!(out.contains("#diagram .edge { animation: dash 1s; }"));
    }

    #[test]
    fn scoped_css_decodes_mermaid_hash_placeholders_as_css_hashes() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(".node { fill: ﬂ°°123456¶ß }"))
            .process_to_string(svg, &session)
            .unwrap();

        assert!(out.contains("#diagram .node { fill: #123456; }"));
    }

    #[test]
    fn scoped_css_uses_css_tokens_for_braces_and_selector_commas() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                r#"/* { } */ svg:not([data-x="{"]) { content: "}"; opacity: 0 } .node:is(.a, .b) { fill: red }"#,
            ))
            .process_to_string(svg, &session)
            .unwrap();

        assert!(
            out.contains(r#"#diagram svg:not([data-x="{"]) { content: "}"; opacity: 0 }"#),
            "{out}"
        );
        assert!(
            out.contains("#diagram .node:is(.a, .b) { fill: red }"),
            "{out}"
        );
    }

    #[test]
    fn scoped_css_escapes_style_terminators_after_tokenized_rewrite() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                r#".node { content: "</style"; }"#,
            ))
            .process_to_string(svg, &session)
            .unwrap();

        assert!(out.contains(r#"content: "\3c /style";"#), "{out}");
        assert_eq!(out.matches("</style>").count(), 1, "{out}");
    }

    #[test]
    fn scoped_css_rejects_unclosed_rule_before_injection() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let error = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                r#".node { content: "{"; fill: red;"#,
            ))
            .process_to_string(svg, &session)
            .unwrap_err();

        assert!(error.to_string().contains("unclosed"), "{error}");
    }

    #[test]
    fn scoped_css_rejects_excessive_rule_nesting() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let css = format!(
            "{} .node {{ fill: red; }} {}",
            "@media all {".repeat(64),
            "}".repeat(64)
        );
        let error = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(css))
            .process_to_string(svg, &session)
            .unwrap_err();

        assert!(error.to_string().contains("nesting"), "{error}");
    }

    #[test]
    fn scoped_css_preflights_exact_projected_bytes_before_materialization() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let processor =
            ScopedCssPostprocessor::new(".node, .edge, .label { fill: red; stroke: blue; }");
        let metadata = SvgPostprocessMetadata::from_svg(svg);
        let unbounded_session = RenderEnvironment::deterministic()
            .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .begin_session()
            .unwrap();
        let unbounded_context = SvgPostprocessContext::new(
            SvgPipelinePreset::Parity,
            0,
            "scoped-css",
            &metadata,
            &unbounded_session,
        );
        let projected_bytes = processor
            .process(Cow::Borrowed(svg), &unbounded_context)
            .expect("unbounded scoped CSS should materialize")
            .len();

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, projected_bytes)
            .unwrap();
        let exact_session = RenderEnvironment::deterministic()
            .with_resource_policy(exact_policy)
            .begin_session()
            .unwrap();
        let exact_context = SvgPostprocessContext::new(
            SvgPipelinePreset::Parity,
            0,
            "scoped-css",
            &metadata,
            &exact_session,
        );
        assert_eq!(
            processor
                .process(Cow::Borrowed(svg), &exact_context)
                .expect("the exact SVG byte limit should admit scoped CSS")
                .len(),
            projected_bytes
        );

        let limited_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, projected_bytes - 1)
            .unwrap();
        let limited_session = RenderEnvironment::deterministic()
            .with_resource_policy(limited_policy)
            .begin_session()
            .unwrap();
        let limited_context = SvgPostprocessContext::new(
            SvgPipelinePreset::Parity,
            0,
            "scoped-css",
            &metadata,
            &limited_session,
        );
        let error = processor
            .process(Cow::Borrowed(svg), &limited_context)
            .unwrap_err();
        let Error::ResourceLimitExceeded(details) = error else {
            panic!("expected SVG byte resource rejection, got {error}");
        };
        assert_eq!(details.phase, ResourceLimitPhase::SvgPostprocess);
        assert_eq!(details.limit, "max_svg_bytes");
        assert_eq!(details.actual, projected_bytes);
        assert_eq!(details.max, projected_bytes - 1);
    }
}
