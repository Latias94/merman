use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, OrdinalSelector,
    ThemeMaterializer, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_theme_contract::{ThemeColorTokenV1, ThemeDefinitionV1, ThemeTokensV1};
use regex::Regex;
use serde_json::json;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn edge_labels_group(svg: &str) -> &str {
    let start = svg
        .find(r#"<g class="edgeLabels">"#)
        .expect("expected edgeLabels group");
    let end = svg[start..]
        .find(r#"<g class="nodes">"#)
        .map(|idx| start + idx)
        .expect("expected nodes group after edgeLabels");
    &svg[start..end]
}

fn render_er_svg_from_text(text: &str, options: &SvgRenderOptions) -> String {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");

    artifact
        .render_svg(options, &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned()
}

fn try_render_er_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin ER resource-bound session");
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ER resource-bound fixture")
        .expect("detect ER resource-bound fixture");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

fn er_entity_paint_theme(fill: CanvasPaint, stroke: Option<CanvasPaint>) -> DiagramTheme {
    let mut style = ThemeStylePatch::default().with_fill(fill);
    if let Some(stroke) = stroke {
        style = style.with_stroke(stroke);
    }
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Entity, style)),
        ))
        .expect("compile ER entity fill theme")
}

fn prepare_er_family_with_theme_and_engine(
    text: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> family::FamilyRenderArtifact {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(text, ParseOptions::strict())
        .expect("parse themed ER diagram")
        .expect("detect themed ER diagram");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable ER session");
    family::prepare(parsed, &LayoutOptions::default(), session).expect("prepare themed ER artifact")
}

fn render_er_svg_from_text_with_theme(text: &str, theme: &DiagramTheme) -> String {
    prepare_er_family_with_theme_and_engine(text, theme, Engine::new())
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed ER SVG")
        .svg()
        .to_owned()
}

fn render_er_svg_from_text_with_theme_and_engine(
    text: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> String {
    prepare_er_family_with_theme_and_engine(text, theme, engine)
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed ER SVG")
        .svg()
        .to_owned()
}

#[test]
fn er_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "erDiagram\n  CUSTOMER ||--o{ ORDER : places\n";
    let baseline = try_render_er_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded ER baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "ER fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact ER SVG byte ceiling");
    let exact = try_render_er_svg_with_resource_policy(source, exact_policy)
        .expect("the exact ER family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact ER SVG byte ceiling");
    let error = try_render_er_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the ER family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected ER MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == below_exact
    }));
}

fn entity_transform(svg: &str, entity_id: &str) -> (f64, f64) {
    let re = Regex::new(&format!(
        r#"id="merman-{}"[^>]*transform="translate\(([^,]+), ([^)]+)\)""#,
        regex::escape(entity_id)
    ))
    .expect("entity transform regex");
    let captures = re
        .captures(svg)
        .unwrap_or_else(|| panic!("missing transform for {entity_id}: {svg}"));
    (
        captures[1].parse().expect("entity x"),
        captures[2].parse().expect("entity y"),
    )
}

fn root_view_box(svg: &str) -> [f64; 4] {
    let re = Regex::new(r#"\bviewBox="([^\"]+)""#).expect("viewBox regex");
    let captures = re.captures(svg).expect("root viewBox");
    let values = captures[1]
        .split_ascii_whitespace()
        .map(|value| value.parse::<f64>().expect("viewBox number"))
        .collect::<Vec<_>>();
    values.try_into().expect("four viewBox numbers")
}

#[test]
fn er_svg_renders_entities_and_relationships() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join("upstream_attributes_styles_classes.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let svg = render_er_svg_from_text(&text, &SvgRenderOptions::default());

    assert!(svg.contains(r#"id="merman-entity-BOOK-0""#));
    assert!(svg.contains(r#"data-look="classic""#));
    assert!(svg.contains(r#"id="merman-id_entity-BOOK-0_entity-PAGE-1_0""#));
    assert!(svg.contains(r#"id="merman-drop-shadow""#));
    assert!(svg.contains("relationshipLine"));
    assert!(
        Regex::new(
            r#"<path[^>]*class="[^"]*relationshipLine[^"]*" style="undefined;;;undefined"[^>]*>"#
        )
        .expect("relationship path regex")
        .is_match(&svg),
        "relationship paths should preserve Mermaid's exact empty pathStyle serialization"
    );
    assert!(svg.contains("relationshipLabelBox"));
    assert!(
        svg.contains("marker") && svg.contains("merman_er-zeroOrMoreStart"),
        "expected Mermaid-like marker ids"
    );
    assert!(
        {
            let path_re = Regex::new(r#"<path[^>]*relationshipLine[^>]*>"#).expect("regex");
            let d_re = Regex::new(r#"\bd="[^"]*C"#).expect("regex");
            path_re.find_iter(&svg).any(|m| d_re.is_match(m.as_str()))
        },
        "expected curveBasis cubic bezier commands in relationship paths"
    );
    assert!(
        svg.contains("color: rgb(255, 255, 255) !important;"),
        "expected classDef text color to use the ER HTML label CSSOM path"
    );
}

#[test]
fn er_svg_recursive_relationship_keeps_three_segments_and_one_label() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join(
            "upstream_cypress_erdiagram_spec_should_render_an_er_diagram_with_a_recursive_relationship_002.mmd",
        );
    let text = std::fs::read_to_string(path).expect("fixture");

    let svg = render_er_svg_from_text(&text, &SvgRenderOptions::default());

    for edge_id in [
        "entity-CUSTOMER-0-cyclic-special-1",
        "entity-CUSTOMER-0-cyclic-special-mid",
        "entity-CUSTOMER-0-cyclic-special-2",
    ] {
        assert!(
            svg.contains(&format!(r#"data-id="{edge_id}""#)),
            "missing recursive ER edge segment {edge_id}: {svg}"
        );
    }
    assert_eq!(
        svg.matches(">refers<").count(),
        1,
        "only the middle recursive segment should own the relationship label: {svg}"
    );
}

#[test]
fn er_svg_uses_configured_look_in_dom_attributes() {
    let text = r#"%%{init: {"look": "neo"}}%%
erDiagram
  CUSTOMER ||--o{ ORDER : places
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    assert!(
        svg.contains(r#"data-look="neo""#),
        "expected ER SVG to propagate configured look: {svg}"
    );
    assert!(
        !svg.contains(r#"data-look="classic""#),
        "configured ER look must not leave classic DOM attributes: {svg}"
    );
}

#[test]
fn er_svg_renders_diagram_title_and_viewbox_includes_it() {
    let text = r#"---
title: Diagram Title
---
erDiagram
  A ||--o{ B : has
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    assert!(svg.contains(r#"class="erDiagramTitleText""#));
    assert!(svg.contains(">Diagram Title<"));
    assert!(svg.contains("viewBox="));
}

#[test]
fn er_svg_title_expands_negative_viewbox_without_rebasing_graph_content() {
    let untitled = r#"erDiagram
  A ||--o{ B : has
"#;
    let titled = r#"---
title: A deliberately wide diagram title
---
erDiagram
  A ||--o{ B : has
"#;

    let untitled_svg = render_er_svg_from_text(untitled, &SvgRenderOptions::default());
    let titled_svg = render_er_svg_from_text(titled, &SvgRenderOptions::default());

    assert_eq!(
        entity_transform(&untitled_svg, "entity-A-0"),
        entity_transform(&titled_svg, "entity-A-0")
    );
    assert!(root_view_box(&titled_svg)[1] < 0.0);
}

#[test]
fn er_svg_forest_theme_renders_root_gradient() {
    let text = r#"---
config:
  theme: forest
---
erDiagram
  A ||--|| B : owns
"#;

    let svg = render_er_svg_from_text(
        text,
        &SvgRenderOptions {
            diagram_id: Some("er_theme_gradient".to_string()),
            ..SvgRenderOptions::default()
        },
    );

    assert!(
        svg.contains(r#"<linearGradient id="er_theme_gradient-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%">"#),
        "expected Mermaid 11.15 ER root gradient element: {svg}"
    );
}

#[test]
fn er_svg_relationship_labels_follow_root_htmllabels_not_flowchart_htmllabels() {
    let text = r#"%%{init: {"htmlLabels": true, "flowchart": {"htmlLabels": false}}}%%
erDiagram
  A ||--|| B : owns
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    let edge_labels = edge_labels_group(&svg);
    assert!(svg.contains(r#"class="nodeLabel markdown-node-label""#));
    assert!(
        edge_labels.contains(r#"class="labelBkg""#)
            && edge_labels.contains(r#"<foreignObject width=""#),
        "expected ER relationship labels to keep HTML foreignObject output when root htmlLabels=true"
    );
}

#[test]
fn er_svg_relationship_labels_follow_flowchart_htmllabels_when_root_unset() {
    let text = r#"%%{init: {"flowchart": {"htmlLabels": false}}}%%
erDiagram
  A ||--|| B : owns
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    let edge_labels = edge_labels_group(&svg);
    assert!(
        edge_labels.contains(r#"<rect class="background""#)
            && edge_labels.contains(">owns</tspan>")
            && edge_labels.contains(r#"text-anchor="middle""#)
            && !edge_labels.contains("<foreignObject"),
        "expected ER relationship labels to switch to SVG text when flowchart htmlLabels=false and root htmlLabels is unset"
    );
}

#[test]
fn er_static_entity_paint_reaches_plain_and_attribute_shells() {
    let source = r#"erDiagram
  PLAIN
  TABLE {
    string id PK
  }
  PLAIN ||--|| TABLE : owns
"#;
    let theme = er_entity_paint_theme(
        CanvasPaint::solid("#123456").expect("valid entity fill"),
        Some(CanvasPaint::solid("#654321").expect("valid entity stroke")),
    );
    let svg = render_er_svg_from_text_with_theme(source, &theme);

    let plain_start = svg
        .find(r#"id="merman-entity-PLAIN-0""#)
        .expect("plain entity group");
    let plain_end = svg[plain_start..]
        .find("</g>")
        .map(|offset| plain_start + offset)
        .expect("plain entity group end");
    let plain = &svg[plain_start..plain_end];
    assert!(plain.contains(r#"class="basic label-container""#));
    assert!(plain.contains("fill:#123456"), "{plain}");
    assert!(plain.contains("stroke:#654321"), "{plain}");

    let table_start = svg
        .find(r#"id="merman-entity-TABLE-1""#)
        .expect("attribute entity group");
    let table_end = svg[table_start..]
        .find("</g>")
        .map(|offset| table_start + offset)
        .expect("attribute entity group end");
    let table = &svg[table_start..table_end];
    assert!(table.contains(r#"class="outer-path""#), "{table}");
    assert!(table.contains("fill:#123456"), "{table}");
    assert!(table.contains("stroke:#654321"), "{table}");
}

#[test]
fn er_tokens_only_definition_reaches_the_model_owned_entity_surface() {
    let definition = ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_color(ThemeColorTokenV1::Surface, "#123456")
            .with_color(ThemeColorTokenV1::Border, "#654321"),
    );
    let materialized = ThemeMaterializer::new()
        .materialize_theme(&definition)
        .expect("materialize tokens-only ER theme");
    let theme = DiagramThemeCompiler::new()
        .compile_spec_wire(materialized.into_spec())
        .expect("compile tokens-only ER theme");

    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync("erDiagram\n  CUSTOMER\n", ParseOptions::strict())
        .expect("parse tokens-only ER diagram")
        .expect("detect tokens-only ER diagram");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort)
        .begin_session_with_theme(&theme)
        .expect("begin tokens-only ER session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare tokens-only ER artifact");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render tokens-only ER SVG");
    let entity_start = rendered
        .svg()
        .find(r#"id="merman-entity-CUSTOMER-0""#)
        .expect("ER entity group");
    let entity_end = rendered.svg()[entity_start..]
        .find("</g>")
        .map(|offset| entity_start + offset)
        .expect("ER entity group end");
    let entity = &rendered.svg()[entity_start..entity_end];

    assert!(entity.contains("fill:#123456"), "tokens.surface: {entity}");
    assert!(entity.contains("stroke:#654321"), "tokens.border: {entity}");
    assert!(
        merman_render::__private::family_evidence(rendered.into_completion().report())
            .applied_count()
            > 0,
        "the Entity adapter must own at least one terminal mechanism"
    );
}

#[test]
fn er_source_entity_styles_outrank_typed_entity_paint() {
    let source = r#"erDiagram
  BARE
  ASSIGNED
  INLINE
  classDef accent fill:#00aa00,stroke:#00bb00
  class ASSIGNED accent
  style INLINE fill:#0000aa,stroke:#0000bb
"#;
    let theme = er_entity_paint_theme(
        CanvasPaint::solid("#123456").expect("valid entity fill"),
        Some(CanvasPaint::solid("#654321").expect("valid entity stroke")),
    );
    let svg = render_er_svg_from_text_with_theme(source, &theme);

    let entity_fragment = |id: &str| {
        let start = svg
            .find(&format!(r#"id="merman-entity-{id}-"#))
            .unwrap_or_else(|| panic!("missing ER entity {id}: {svg}"));
        let end = svg[start..]
            .find(r#"class="node"#)
            .and_then(|_| svg[start..].find("</g>"))
            .map(|offset| start + offset)
            .expect("entity fragment end");
        &svg[start..end]
    };

    let bare = entity_fragment("BARE");
    assert!(bare.contains("fill:#123456"), "{bare}");
    assert!(bare.contains("stroke:#654321"), "{bare}");
    let assigned = entity_fragment("ASSIGNED");
    assert!(assigned.contains("fill:#00aa00"), "{assigned}");
    assert!(assigned.contains("stroke:#00bb00"), "{assigned}");
    let inline = entity_fragment("INLINE");
    assert!(inline.contains("fill:#0000aa"), "{inline}");
    assert!(inline.contains("stroke:#0000bb"), "{inline}");

    let default_svg = render_er_svg_from_text_with_theme(
        "erDiagram\n  DEFAULT\n  classDef default fill:#aa0000,stroke:#bb0000\n",
        &theme,
    );
    assert!(default_svg.contains("fill:#aa0000"), "{default_svg}");
    assert!(default_svg.contains("stroke:#bb0000"), "{default_svg}");
}

#[test]
fn er_site_theme_variable_ownership_controls_typed_entity_paint() {
    let source = "erDiagram\n  PLAIN\n  TABLE {\n    string id PK\n  }\n";
    let theme = er_entity_paint_theme(
        CanvasPaint::solid("#123456").expect("valid entity fill"),
        Some(CanvasPaint::solid("#654321").expect("valid entity stroke")),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "themeVariables": {
            "mainBkg": "#112233",
            "nodeBorder": "#445566"
        }
    })));
    let svg = render_er_svg_from_text_with_theme_and_engine(source, &theme, engine);

    assert!(svg.contains("#112233"), "{svg}");
    assert!(svg.contains("#445566"), "{svg}");
    assert!(
        !svg.contains("#123456"),
        "site fill must own ER entity fill: {svg}"
    );
    assert!(
        !svg.contains("#654321"),
        "site stroke must own ER entity stroke: {svg}"
    );

    let default_primary_border_svg = render_er_svg_from_text_with_theme_and_engine(
        source,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "theme": "default",
            "themeVariables": {
                "primaryBorderColor": "#abcdef"
            }
        }))),
    );
    assert!(
        default_primary_border_svg.contains("#654321"),
        "default primaryBorderColor is not the terminal ER entity stroke owner: {default_primary_border_svg}"
    );

    let base_primary_svg = render_er_svg_from_text_with_theme_and_engine(
        source,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "theme": "base",
            "themeVariables": {
                "primaryColor": "#fff4dd"
            }
        }))),
    );
    assert!(
        !base_primary_svg.contains("#654321"),
        "base primaryColor-derived nodeBorder must own ER entity stroke: {base_primary_svg}"
    );
}

#[test]
fn er_entity_ordinal_paint_fails_closed() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Entity,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").expect("valid entity fill")),
                    )
                    .with_ordinal(OrdinalSelector::exact(1).expect("valid ER entity ordinal")),
                ),
            ),
        )
        .expect("compile ordinal ER entity theme");
    let error = match prepare_er_family_with_theme_and_engine(
        "erDiagram\n  PLAIN\n",
        &theme,
        Engine::new(),
    )
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("ER entity ordinal paint must remain unverified"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::ER, 1))
    );
}

#[test]
fn er_entity_paint_is_not_applicable_without_entities() {
    let theme = er_entity_paint_theme(
        CanvasPaint::solid("#123456").expect("valid entity fill"),
        Some(CanvasPaint::solid("#654321").expect("valid entity stroke")),
    );
    let _ = render_er_svg_from_text_with_theme("erDiagram\n", &theme);
}
