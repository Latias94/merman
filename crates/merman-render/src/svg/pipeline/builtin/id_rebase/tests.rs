use super::*;
use crate::Error;
use crate::environment::RenderEnvironment;
use crate::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use crate::svg::pipeline::{SvgPipeline, SvgPipelinePreset, SvgPostprocessMetadata};

fn rebase(svg: &str) -> String {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    SvgPipeline::parity()
        .with_postprocessor(RebaseSvgIdsPostprocessor::new("fragment-light"))
        .process_to_string(svg, &session)
        .unwrap()
}

#[test]
fn rebases_ids_and_every_supported_local_reference_shape() {
    let svg = r##"<svg id="root" aria-labelledby="title desc" xmlns:xlink="http://www.w3.org/1999/xlink"><style>#root #shape{fill:url(#paint)}#fff{stroke:#fff}.optional{fill:url(#missing-gradient)}</style><title id="title">T</title><desc id="desc">D</desc><defs><linearGradient id="paint"/><path id="shape"/><path id="fff"/></defs><use href="#shape" xlink:href="#shape"/><animate id="pulse" begin="shape.begin+1s; pulse.end"/><path style="fill:url('#paint')" marker-end="url(#shape)"/></svg>"##;
    let output = rebase(svg);

    for expected in [
        r#"id="fragment-light-root""#,
        r#"aria-labelledby="fragment-light-title fragment-light-desc""#,
        "#fragment-light-root #fragment-light-shape",
        "#fragment-light-fff{stroke:#fff}",
        "url(#fragment-light-paint)",
        "url(#fragment-light-missing-gradient)",
        r##"href="#fragment-light-shape""##,
        "fragment-light-shape.begin+1s;fragment-light-pulse.end",
    ] {
        assert!(output.contains(expected), "missing {expected:?}: {output}");
    }
    crate::xml_dom::Document::parse(&output).expect("rebased SVG XML");
}

#[test]
fn rebases_quoted_urls_without_discarding_whitespace_or_comments() {
    let svg = r##"<svg><style>.spaced{fill:url( "#paint" )}.commented{stroke:u\72l( "#paint" /* trailing */)}</style><defs><linearGradient id="paint"/></defs></svg>"##;
    let output = rebase(svg);

    assert!(
        output.contains("url( &quot;#fragment-light-paint&quot; )"),
        "{output}"
    );
    assert!(
        output.contains("u\\72l( &quot;#fragment-light-paint&quot; /* trailing */)"),
        "{output}"
    );
}

#[test]
fn rebases_css_tokens_across_xml_text_cdata_and_comment_boundaries() {
    let svg = r##"<svg><style>#no<![CDATA[de]]><!-- not CSS text -->{fill:url(#pa<![CDATA[int]]>)}</style><defs><linearGradient id="paint"/></defs><path id="node"/></svg>"##;
    let output = rebase(svg);

    assert!(
        output.contains("#fragment-light-node{fill:url(#fragment-light-paint)}"),
        "{output}"
    );
    assert!(!output.contains("not CSS text"), "{output}");
    assert!(!output.contains("<![CDATA["), "{output}");
}

#[test]
fn preserves_referenced_whitespace_in_attributes_and_style_text() {
    let svg = r##"<svg id="root" data-note="&#13;&#10;&#9;"><style>#root{fill:red}&#13;&#10;&#9;</style></svg>"##;
    let output = rebase(svg);
    let document = crate::xml_dom::Document::parse(&output).expect("rebased SVG XML");

    assert_eq!(
        document.root_element().attribute("data-note"),
        Some("\r\n\t")
    );
    assert_eq!(
        document
            .root_element()
            .first_element_child()
            .unwrap()
            .text(),
        Some("#fragment-light-root{fill:red}\r\n\t")
    );
}

#[test]
fn duplicate_ids_fail_closed() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let error = SvgPipeline::parity()
        .with_postprocessor(RebaseSvgIdsPostprocessor::new("fragment"))
        .process_to_string(r#"<svg><g id="same"/><path id="same"/></svg>"#, &session)
        .unwrap_err();

    assert!(error.to_string().contains("duplicate SVG id"), "{error}");
}

#[test]
fn projected_n_minus_one_returns_the_exact_svg_byte_limit_error() {
    let svg = r##"<svg id="root" aria-labelledby="paint shape" data-note="&#13;&#10;&#9;"><style>#root[aria-labelledby="paint shape"] #shape{fill:url(#paint);content:"&lt;&amp;"}&#13;&#10;&#9;</style><style><![CDATA[#shape{stroke:url(#paint)}]]></style><defs><linearGradient id="paint"/></defs><path id="shape" fill="url(#paint)"/><use href="#shape"/></svg>"##;
    let processor = RebaseSvgIdsPostprocessor::new("fragment-with-a-long-scope");
    let unbounded_session = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
        .begin_session()
        .unwrap();
    let metadata = SvgPostprocessMetadata::from_svg(svg);
    let unbounded_context = SvgPostprocessContext::new(
        SvgPipelinePreset::Parity,
        0,
        "rebase-svg-ids",
        &metadata,
        &unbounded_session,
    );
    let projected_bytes = processor
        .process(Cow::Borrowed(svg), &unbounded_context)
        .expect("unbounded ID rebase should materialize")
        .len();
    assert!(projected_bytes > svg.len());

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
        "rebase-svg-ids",
        &metadata,
        &exact_session,
    );
    let exact = processor
        .process(Cow::Borrowed(svg), &exact_context)
        .expect("the exact SVG byte limit should admit ID rebasing");
    assert_eq!(exact.len(), projected_bytes);

    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, projected_bytes - 1)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .unwrap();
    let context = SvgPostprocessContext::new(
        SvgPipelinePreset::Parity,
        0,
        "rebase-svg-ids",
        &metadata,
        &session,
    );
    let error = processor.process(Cow::Borrowed(svg), &context).unwrap_err();

    let Error::ResourceLimitExceeded(details) = error else {
        panic!("expected SVG byte resource rejection, got {error}");
    };
    assert_eq!(details.cause, ResourceLimitCause::Ceiling);
    assert_eq!(details.phase, ResourceLimitPhase::SvgPostprocess);
    assert_eq!(details.limit, "max_svg_bytes");
    assert_eq!(details.actual, projected_bytes);
    assert_eq!(details.max, projected_bytes - 1);
}

#[test]
fn large_prefix_style_projection_matches_output_and_rejects_n_minus_one() {
    let prefix = "p".repeat(256 * 1024);
    let svg = r##"<svg><style>#node{fill:url(#paint)}</style><style><![CDATA[#node{stroke:url(#paint)}]]></style><defs><linearGradient id="paint"/></defs><path id="node"/></svg>"##;
    let processor = RebaseSvgIdsPostprocessor::new(prefix.clone());
    let metadata = SvgPostprocessMetadata::from_svg(svg);
    let unbounded_session = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
        .begin_session()
        .unwrap();
    let unbounded_context = SvgPostprocessContext::new(
        SvgPipelinePreset::Parity,
        0,
        "rebase-svg-ids",
        &metadata,
        &unbounded_session,
    );
    let expected = processor
        .process(Cow::Borrowed(svg), &unbounded_context)
        .expect("unbounded ID rebase should materialize");
    let projected_bytes = expected.len();
    assert!(projected_bytes > 1024 * 1024);

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
        "rebase-svg-ids",
        &metadata,
        &exact_session,
    );
    let exact = processor
        .process(Cow::Borrowed(svg), &exact_context)
        .expect("the exact SVG byte limit should admit the expanded styles");
    assert_eq!(exact, expected);

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
        "rebase-svg-ids",
        &metadata,
        &limited_session,
    );
    let error = processor
        .process(Cow::Borrowed(svg), &limited_context)
        .expect_err("N - 1 bytes must reject before rebased SVG materialization");

    let Error::ResourceLimitExceeded(details) = error else {
        panic!("expected SVG byte resource rejection, got {error}");
    };
    assert_eq!(details.cause, ResourceLimitCause::Ceiling);
    assert_eq!(details.phase, ResourceLimitPhase::SvgPostprocess);
    assert_eq!(details.limit, "max_svg_bytes");
    assert_eq!(details.actual, projected_bytes);
    assert_eq!(details.max, projected_bytes - 1);
}

#[test]
fn streaming_element_limit_preserves_exact_and_n_minus_one_boundaries() {
    let svg = r#"<svg id="root"><g id="first"/><g id="second"/></svg>"#;
    let processor = RebaseSvgIdsPostprocessor::new("fragment-with-a-long-scope");
    let metadata = SvgPostprocessMetadata::from_svg(svg);

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgElements, 3)
        .unwrap();
    let exact_session = RenderEnvironment::deterministic()
        .with_resource_policy(exact_policy)
        .begin_session()
        .unwrap();
    let exact_context = SvgPostprocessContext::new(
        SvgPipelinePreset::Parity,
        0,
        "rebase-svg-ids",
        &metadata,
        &exact_session,
    );
    processor
        .process(Cow::Borrowed(svg), &exact_context)
        .expect("the exact element limit should admit ID rebasing");

    let limited_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgElements, 2)
        .unwrap();
    let limited_session = RenderEnvironment::deterministic()
        .with_resource_policy(limited_policy)
        .begin_session()
        .unwrap();
    let limited_context = SvgPostprocessContext::new(
        SvgPipelinePreset::Parity,
        0,
        "rebase-svg-ids",
        &metadata,
        &limited_session,
    );
    let error = processor
        .process(Cow::Borrowed(svg), &limited_context)
        .expect_err("N - 1 elements must fail during streaming collection");

    let Error::ResourceLimitExceeded(details) = error else {
        panic!("expected SVG element resource rejection, got {error}");
    };
    assert_eq!(details.cause, ResourceLimitCause::Ceiling);
    assert_eq!(details.phase, ResourceLimitPhase::SvgPostprocess);
    assert_eq!(details.limit, "max_svg_elements");
    assert_eq!(details.actual, 3);
    assert_eq!(details.max, 2);
}

#[test]
fn rebases_group_rule_selectors_without_rewriting_prelude_or_value_colors() {
    let svg = r##"<svg><style>@media (prefers-color-scheme:dark){#root{fill:#fff}}@supports (color:#fff){#root{stroke:#fff}}</style><g id="root"/><g id="fff"/></svg>"##;
    let output = rebase(svg);

    assert!(
        output.contains("@media (prefers-color-scheme:dark){#fragment-light-root{fill:#fff}}"),
        "{output}"
    );
    assert!(
        output.contains("@supports (color:#fff){#fragment-light-root{stroke:#fff}}"),
        "{output}"
    );
}

#[test]
fn rebases_escaped_urls_selector_preludes_attribute_selectors_and_smil_events() {
    let svg = r##"<svg><style>@scope (#root){[href="#shape"],[id="shape"]{fill:u\72l("#paint")}}@supports selector(#root){#root{stroke:#fff}}</style><defs><linearGradient id="paint"/><path id="shape"/><g id="root"/><g id="fff"/></defs><path fill="u\72l(#paint)"/><animate begin="shape.click;shape.repeat(2);shape.begin"/></svg>"##;
    let output = rebase(svg);

    for expected in [
        "@scope (#fragment-light-root)",
        "@supports selector(#fragment-light-root)",
        "[href=&quot;#fragment-light-shape&quot;]",
        "[id=&quot;fragment-light-shape&quot;]",
        "#fragment-light-root{stroke:#fff}",
        "#fragment-light-paint",
        "fragment-light-shape.click;fragment-light-shape.repeat(2);fragment-light-shape.begin",
    ] {
        assert!(output.contains(expected), "missing {expected:?}: {output}");
    }
    assert!(!output.contains("#fragment-light-fff}"), "{output}");
    crate::xml_dom::Document::parse(&output).expect("rebased SVG XML");
}

#[test]
fn leaves_non_css_prose_attributes_opaque() {
    let svg = r#"<svg id="root" aria-label="prose https://example.test/url(foo&amp;bar)"><g id="shape"/></svg>"#;
    let output = rebase(svg);

    assert!(
        output.contains(r#"aria-label="prose https://example.test/url(foo&amp;bar)""#),
        "{output}"
    );
}

#[test]
fn rebases_all_aria_idrefs_in_attributes_and_selectors() {
    let svg = r##"<svg id="root" aria-activedescendant="shape" aria-controls="shape" aria-details="shape" aria-errormessage="shape" aria-flowto="shape" aria-owns="shape"><style>#root [aria-controls="shape"],#root [aria-owns="shape"],#root [id$="-arrowhead"],#root [id*="arrow"],#root [id^="shape"]{fill:red}</style><path id="shape"/><path id="shape-arrowhead"/></svg>"##;
    let output = rebase(svg);

    for attribute in [
        "aria-activedescendant",
        "aria-controls",
        "aria-details",
        "aria-errormessage",
        "aria-flowto",
        "aria-owns",
    ] {
        assert!(
            output.contains(&format!(r#"{attribute}="fragment-light-shape""#)),
            "{attribute}: {output}"
        );
    }
    assert!(
        output.contains("[aria-controls=&quot;fragment-light-shape&quot;]"),
        "{output}"
    );
    assert!(
        output.contains("[aria-owns=&quot;fragment-light-shape&quot;]"),
        "{output}"
    );
    assert!(output.contains("[id$=&quot;-arrowhead&quot;]"), "{output}");
    assert!(output.contains("[id*=&quot;arrow&quot;]"), "{output}");
    assert!(
        output.contains("[id^=&quot;fragment-light-shape&quot;]"),
        "{output}"
    );
}
