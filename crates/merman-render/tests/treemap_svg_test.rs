use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalPalette,
    Specified, ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, TypographySpec,
};
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMetrics, TextStyle};
use merman_render::{DiagramFamilyId, LayoutOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct CountingTreemapHost {
    calls: AtomicUsize,
    observations: Mutex<Vec<(String, TextStyle)>>,
}

impl CountingTreemapHost {
    fn width(&self, text: &str, style: &TextStyle) -> f64 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.observations
            .lock()
            .expect("text measurement recorder is not poisoned")
            .push((text.to_owned(), style.clone()));
        text.chars().count() as f64 * style.font_size.max(1.0)
    }

    fn reset(&self) {
        self.calls.store(0, Ordering::Relaxed);
        self.observations
            .lock()
            .expect("text measurement recorder is not poisoned")
            .clear();
    }

    fn observed_font_families(&self) -> Vec<Option<String>> {
        self.observations
            .lock()
            .expect("text measurement recorder is not poisoned")
            .iter()
            .map(|(_, style)| style.font_family.clone())
            .collect()
    }

    fn observed_styles(&self) -> Vec<TextStyle> {
        self.observations
            .lock()
            .expect("text measurement recorder is not poisoned")
            .iter()
            .map(|(_, style)| style.clone())
            .collect()
    }

    fn observed_styles_for_text(&self, text: &str) -> Vec<TextStyle> {
        self.observations
            .lock()
            .expect("text measurement recorder is not poisoned")
            .iter()
            .filter(|(observed_text, _)| observed_text == text)
            .map(|(_, style)| style.clone())
            .collect()
    }
}

impl HostTextMeasurer for CountingTreemapHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        let width = self.width(request.text, request.style);
        Ok(Some(match request.operation {
            TextMeasurementOperation::Measure | TextMeasurementOperation::Wrapped => {
                HostTextMeasurement::Metrics(TextMetrics {
                    width,
                    height: request.style.font_size.max(1.0),
                    line_count: 1,
                })
            }
            TextMeasurementOperation::ComputedLength => HostTextMeasurement::Length(width),
            TextMeasurementOperation::SimpleBBoxWidth => HostTextMeasurement::Length(width),
            TextMeasurementOperation::SimpleBBoxHeight => {
                HostTextMeasurement::Length(request.style.font_size.max(1.0))
            }
            _ => return Ok(None),
        }))
    }
}

fn counting_treemap_environment(host: Arc<CountingTreemapHost>) -> RenderEnvironment {
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.treemap-host").expect("valid profile id"),
        "1",
    )
    .expect("valid profile identity");
    RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::host_display(identity, host, TextMeasurementPhase::ALL),
    )
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn attr_f64(tag: &str, name: &str) -> Option<f64> {
    let needle = format!(r#"{name}=""#);
    let i = tag.find(&needle)? + needle.len();
    let rest = &tag[i..];
    let end = rest.find('"')?;
    rest[..end].parse::<f64>().ok()
}

fn font_size_px(tag: &str) -> Option<f64> {
    let (_, suffix) = tag.split_once("font-size:")?;
    let value = suffix.trim_start();
    let end = value.find("px")?;
    value[..end].trim().parse().ok()
}

fn css_declarations_for_selector_suffix<'a>(stylesheet: &'a str, suffix: &str) -> &'a str {
    stylesheet
        .split('}')
        .filter_map(|rule| rule.rsplit_once('{'))
        .find_map(|(selectors, declarations)| {
            selectors
                .split(',')
                .any(|selector| selector.trim().ends_with(suffix))
                .then_some(declarations)
        })
        .unwrap_or_else(|| panic!("missing CSS selector ending in `{suffix}`: {stylesheet}"))
}

fn text_tag_by_text<'a>(svg: &'a str, text: &str) -> &'a str {
    let needle = format!(">{text}</text>");
    let end = svg.find(&needle).expect("expected text tag") + needle.len();
    let start = svg[..end].rfind("<text").expect("expected text tag start");
    &svg[start..end]
}

fn text_tag_by_class_and_text<'a>(svg: &'a str, class_name: &str, text: &str) -> &'a str {
    let needle = format!(">{text}</text>");
    let mut offset = 0;
    while let Some(rel_end) = svg[offset..].find(&needle) {
        let end = offset + rel_end + needle.len();
        let start = svg[..end].rfind("<text").expect("expected text tag start");
        let tag = &svg[start..end];
        if tag.contains(&format!(r#"class="{class_name}""#)) {
            return tag;
        }
        offset = end;
    }
    panic!("expected text tag with class {class_name} and text {text}");
}

fn contains_default_text_fill(tag: &str) -> bool {
    tag.contains("fill:#333")
        || tag.contains("fill: #333")
        || tag.contains("fill:rgb(51, 51, 51)")
        || tag.contains("fill: rgb(51, 51, 51)")
}

fn render_treemap_svg_and_config_from_fixture(fixture: &str) -> (String, serde_json::Value) {
    let path = workspace_root()
        .join("fixtures")
        .join("treemap")
        .join(fixture);
    let text = std::fs::read_to_string(&path).expect("fixture");
    render_treemap_svg_and_config_from_source(&text)
}

fn render_treemap_svg_and_config_from_source(text: &str) -> (String, serde_json::Value) {
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let effective_config = parsed.metadata().effective_config.as_value().clone();

    let layout_options = LayoutOptions::default();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin render session");
    let artifact = family::prepare(parsed, &layout_options, session).expect("layout ok");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned();

    (svg, effective_config)
}

fn render_treemap_svg_from_fixture(fixture: &str) -> String {
    render_treemap_svg_and_config_from_fixture(fixture).0
}

fn render_treemap_svg_from_source(text: &str) -> String {
    render_treemap_svg_and_config_from_source(text).0
}

fn treemap_title_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Title,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::TREEMAP),
                ),
            ),
        )
        .expect("compile Treemap title fill theme")
}

fn treemap_text_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::TREEMAP),
                ),
            ),
        )
        .expect("compile Treemap text fill theme")
}

fn treemap_typography_theme(font_stack: FontStack) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(
                DiagramFamilyId::TREEMAP,
                ThemeTextStyle::default().with_font_stack(font_stack),
            ),
        ))
        .expect("compile Treemap typography theme")
}

fn render_treemap_with_theme(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> family::RenderedFamilySvg {
    try_render_treemap_with_theme_requirement(
        source,
        theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render themed Treemap")
}

fn try_render_treemap_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Treemap")
        .expect("detect themed Treemap");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Treemap session");
    family::prepare(parsed, &LayoutOptions::default(), session)?
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

fn try_render_treemap_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Treemap resource-bound session");
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse Treemap resource-bound fixture")
        .expect("detect Treemap resource-bound fixture");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn treemap_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "treemap\n\"Section\"\n  \"Alpha\": 4\n  \"Beta\": 2\n";
    let baseline = try_render_treemap_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Treemap baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Treemap fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Treemap SVG byte ceiling");
    let exact = try_render_treemap_svg_with_resource_policy(source, exact_policy)
        .expect("the exact Treemap family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Treemap SVG byte ceiling");
    let error = try_render_treemap_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the Treemap family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Treemap MaxSvgBytes rejection, got {error}");
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

fn deep_treemap_chain(depth: usize) -> String {
    let mut input = String::from("treemap\n");
    for level in 0..depth {
        input.push_str(&" ".repeat(level));
        input.push('"');
        input.push_str(&format!("section{level}"));
        input.push_str("\"\n");
    }
    input.push_str(&" ".repeat(depth));
    input.push_str("\"leaf\": 1\n");
    input
}

#[test]
fn treemap_frontmatter_title_renders_unless_the_body_overrides_it() {
    let frontmatter_svg = render_treemap_svg_from_source(
        r#"---
title: Frontmatter treemap
---
treemap-beta
"A": 1
"#,
    );
    assert!(
        frontmatter_svg.contains(r#"class="treemapTitle" text-anchor="middle" dominant-baseline="middle">Frontmatter treemap</text>"#),
        "frontmatter title should render when the Treemap body has none: {frontmatter_svg}"
    );
    assert!(frontmatter_svg.contains(r#"transform="translate(0, 30)" class="treemapContainer""#));

    let body_svg = render_treemap_svg_from_source(
        r#"---
title: Frontmatter treemap
---
treemap-beta
title Body treemap
"A": 1
"#,
    );
    assert!(body_svg.contains(">Body treemap</text>"));
    assert!(!body_svg.contains(">Frontmatter treemap</text>"));
}

#[test]
fn treemap_title_fill_reaches_terminal_css_and_title_text() {
    let cases = [
        (
            CanvasPaint::solid("#123456").expect("valid Treemap title color"),
            "#123456",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ];

    for (fill, expected_fill) in cases {
        let theme = treemap_title_fill_theme(fill);
        let rendered = render_treemap_with_theme(
            "treemap\ntitle Themed treemap\n\"Section\"\n  \"Leaf\": 1\n",
            &theme,
            Engine::new(),
        );
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid themed Treemap SVG");
        let titles = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("text") && node.attribute("class") == Some("treemapTitle")
            })
            .collect::<Vec<_>>();
        assert_eq!(titles.len(), 1);
        assert_eq!(titles[0].text(), Some("Themed treemap"));

        let style = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Treemap stylesheet");
        assert!(
            style.contains(&format!(".treemapTitle{{fill:{expected_fill};")),
            "typed title fill must own the final Treemap title rule: {style}"
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn treemap_text_fill_reaches_label_and_value_terminals() {
    let theme =
        treemap_text_fill_theme(CanvasPaint::solid("#123456").expect("valid Treemap text color"));
    let rendered = render_treemap_with_theme(
        "treemap\n\"Section\"\n  \"Leaf\": 12\n",
        &theme,
        Engine::new(),
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Treemap SVG");
    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Treemap stylesheet");
    assert!(stylesheet.contains(".treemapLabel{fill:#123456;"));
    assert!(stylesheet.contains(".treemapValue{fill:#123456;"));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn treemap_mixed_text_rule_does_not_certify_unsupported_typography() {
    let source = "treemap\n\"Section\"\n  \"Leaf\": 12\n";
    for font_stack in [false, true] {
        let mut patch =
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
        if font_stack {
            patch.typography.font_stack =
                Specified::Value(FontStack::single("UnimplementedRuleFace").unwrap());
        } else {
            patch.typography.font_size_px = Specified::Value(40.0);
        }
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(ThemeTarget::Text, patch).for_family(DiagramFamilyId::TREEMAP),
                )),
            )
            .unwrap();
        let rendered = try_render_treemap_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let label = text_tag_by_text(rendered.svg(), "Leaf");
        assert!(
            label.contains("#123456") || label.contains("rgb(18, 52, 86)"),
            "{label}"
        );
        assert_ne!(font_size_px(label), Some(40.0));
        assert!(!label.contains("UnimplementedRuleFace"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(
            evidence.applied_count(),
            0,
            "fill cannot certify the rule's font facet"
        );
        assert_eq!(evidence.theme_residual_count(), 1);
        let error = try_render_treemap_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .err()
        .expect("mixed rule must fail closed");
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::TREEMAP, 1))
        );
    }
}

#[test]
fn treemap_text_rule_accounts_only_its_winning_facets() {
    let mut mixed = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
    mixed.typography.font_size_px = Specified::Value(40.0);
    let mut later_font = ThemeStylePatch::default();
    later_font.typography.font_size_px = Specified::Value(50.0);
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(ThemeTarget::Text, mixed)
                            .for_family(DiagramFamilyId::TREEMAP),
                    )
                    .with_rule(
                        ThemeRule::new(ThemeTarget::Text, later_font)
                            .for_family(DiagramFamilyId::TREEMAP),
                    ),
            ),
        )
        .unwrap();
    let source = "treemap\n\"Section\"\n  \"Leaf\": 12\n";
    let rendered = try_render_treemap_with_theme_requirement(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(
        evidence.applied_count(),
        1,
        "the first rule's remaining winning fill is supported"
    );
    assert_eq!(
        evidence.theme_residual_count(),
        1,
        "only the later rule owns unsupported size"
    );

    let hidden = "---\nconfig:\n  treemap:\n    nodeWidth: 1\n    nodeHeight: 1\n---\ntreemap\ntitle Visible title\n\"Leaf\": 12\n";
    let rendered = render_treemap_with_theme(hidden, &theme, Engine::new());
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("treemapTitle"))
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(
        evidence.not_applicable_count(),
        2,
        "a title is not a Text terminal"
    );
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn treemap_label_hidden_after_fitting_does_not_prove_text_fill() {
    let theme = treemap_text_fill_theme(CanvasPaint::solid("#123456").unwrap());
    let source = "---\nconfig:\n  treemap:\n    nodeWidth: 8\n    nodeHeight: 20\n    showValues: false\n---\ntreemap\n\"A very long label that cannot fit at the minimum font size\": 12\n";
    let rendered = render_treemap_with_theme(source, &theme, Engine::new());
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let leaf_label = document
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("treemapLabel")
                && node.text() == Some("A very long label that cannot fit at the minimum font size")
        })
        .unwrap();
    assert!(
        leaf_label
            .attribute("style")
            .unwrap()
            .contains("display: none")
    );
    assert_eq!(
        font_size_px(text_tag_by_text(
            rendered.svg(),
            "A very long label that cannot fit at the minimum font size"
        )),
        Some(8.0)
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn treemap_receipts_exclude_hidden_values_and_empty_truncated_sections() {
    struct WideTextHost {
        every_text: bool,
    }
    impl HostTextMeasurer for WideTextHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            Ok(
                (request.operation == TextMeasurementOperation::ComputedLength
                    && (self.every_text || request.text == "12"))
                    .then_some(HostTextMeasurement::Length(1e8)),
            )
        }
    }
    let theme = treemap_text_fill_theme(CanvasPaint::solid("#123456").unwrap());
    for every_text in [false, true] {
        let (source, config) = if every_text {
            (
                "treemap\n\"Section\"\n  \"Leaf\": 12\n",
                serde_json::json!({"treemap": {"showValues": false}}),
            )
        } else {
            (
                "treemap\n\"Leaf\": 12\n",
                serde_json::json!({"treemap": {"labelColor": "#abcdef"}}),
            )
        };
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(config));
        let parsed = merman_render::__private::install_parse_compatibility(&theme, engine)
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.treemap-visibility").unwrap(),
            "1",
        )
        .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                identity,
                Arc::new(WideTextHost { every_text }),
                TextMeasurementPhase::ALL,
            ))
            .begin_session_with_theme(&theme)
            .unwrap();
        let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        if every_text {
            let sections: Vec<_> = document
                .descendants()
                .filter(|node| node.attribute("class") == Some("treemapSectionLabel"))
                .collect();
            assert!(!sections.is_empty());
            assert!(
                sections
                    .iter()
                    .all(|node| node.text().is_none_or(str::is_empty))
            );
        } else {
            let label = text_tag_by_text(rendered.svg(), "Leaf");
            assert!(!label.contains("display: none"));
            let value = text_tag_by_text(rendered.svg(), "12");
            assert!(value.contains("display: none"));
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0, "every_text={every_text}");
        assert_eq!(evidence.not_applicable_count(), 1);
    }
}

#[test]
fn treemap_text_fill_respects_independent_label_and_value_owners() {
    let theme =
        treemap_text_fill_theme(CanvasPaint::solid("#123456").expect("valid Treemap text color"));
    let cases = [
        (
            "label owner",
            serde_json::json!({
                "treemap": { "labelColor": "#fedcba" }
            }),
            "#fedcba",
            "#123456",
            1,
        ),
        (
            "value owner",
            serde_json::json!({
                "treemap": { "valueColor": "#abcdef" }
            }),
            "#123456",
            "#abcdef",
            1,
        ),
        (
            "both owners",
            serde_json::json!({
                "treemap": { "labelColor": "#fedcba", "valueColor": "#abcdef" }
            }),
            "#fedcba",
            "#abcdef",
            0,
        ),
    ];

    for (name, site_config, expected_label, expected_value, applied_count) in cases {
        let typed_cssom = "rgb(18, 52, 86)";
        let rendered = render_treemap_with_theme(
            "treemap\n\"Section\"\n  \"Leaf\": 12\n",
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(site_config)),
        );
        let document = roxmltree::Document::parse(rendered.svg())
            .unwrap_or_else(|error| panic!("{name}: valid Treemap SVG: {error}"));
        let stylesheet = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Treemap stylesheet");
        assert!(
            stylesheet.contains(&format!(".treemapLabel{{fill:{expected_label};")),
            "{name}: {stylesheet}"
        );
        assert!(
            stylesheet.contains(&format!(".treemapValue{{fill:{expected_value};")),
            "{name}: {stylesheet}"
        );
        let leaf_label = document
            .descendants()
            .find(|node| {
                node.has_tag_name("text")
                    && node.attribute("class") == Some("treemapLabel")
                    && node.text() == Some("Leaf")
            })
            .and_then(|node| node.attribute("style"))
            .expect("Treemap leaf label terminal");
        let leaf_value = document
            .descendants()
            .find(|node| {
                node.has_tag_name("text")
                    && node.attribute("class") == Some("treemapValue")
                    && node.text() == Some("12")
            })
            .and_then(|node| node.attribute("style"))
            .expect("Treemap leaf value terminal");
        if name == "label owner" {
            assert!(leaf_value.contains(typed_cssom), "{name}: {leaf_value}");
            assert!(!leaf_label.contains(typed_cssom), "{name}: {leaf_label}");
        } else if name == "value owner" {
            assert!(leaf_label.contains(typed_cssom), "{name}: {leaf_label}");
            assert!(!leaf_value.contains(typed_cssom), "{name}: {leaf_value}");
        } else {
            assert!(!leaf_label.contains(typed_cssom), "{name}: {leaf_label}");
            assert!(!leaf_value.contains(typed_cssom), "{name}: {leaf_value}");
        }

        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "{name}");
        assert_eq!(evidence.applied_count(), applied_count, "{name}");
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(applied_count == 0),
            "{name}"
        );
        assert_eq!(evidence.theme_residual_count(), 0, "{name}");
    }
}

#[test]
fn treemap_text_palette_uses_final_fill_ownership() {
    for (fill, source_owned, residual_count, not_applicable_count) in [
        (
            Some(Specified::Value(CanvasPaint::solid("#123456").unwrap())),
            false,
            0,
            1,
        ),
        // Clear shadows the palette but the Clear rule itself remains unsupported.
        (Some(Specified::Clear), false, 1, 1),
        (None, true, 0, 1),
        (None, false, 1, 0),
    ] {
        let mut styles = ThemeRuleSet::default().with_ordinal_palette(
            ThemeTarget::Text,
            OrdinalPalette::new([ThemeColorValue::parse("#abcdef").unwrap()]).unwrap(),
        );
        let case = format!("fill={fill:?}, source_owned={source_owned}");
        if let Some(fill) = fill {
            let mut patch = ThemeStylePatch::default();
            patch.paint.fill = fill;
            styles = styles.with_rule(ThemeRule::new(ThemeTarget::Text, patch));
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .unwrap();
        let source = if source_owned {
            "treemap\nclassDef owned color:#fedcba;\n\"Leaf\": 12:::owned\n"
        } else {
            "treemap\n\"Leaf\": 12\n"
        };
        let rendered = try_render_treemap_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(
            evidence.theme_residual_count(),
            residual_count,
            "{case}: {evidence:?}"
        );
        assert_eq!(
            evidence.not_applicable_count(),
            not_applicable_count,
            "{case}: {evidence:?}"
        );
        let strict = try_render_treemap_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
        );
        if residual_count == 0 {
            strict.expect("shadowed palette must not prevent portable rendering");
        } else {
            assert_eq!(
                strict.err().unwrap().unverified_family_theme(),
                Some((DiagramFamilyId::TREEMAP, 1))
            );
        }
    }
}

#[test]
fn treemap_text_palette_respects_visible_roles_and_config_owners() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_ordinal_palette(
                ThemeTarget::Text,
                OrdinalPalette::new([ThemeColorValue::parse("#abcdef").unwrap()]).unwrap(),
            )),
        )
        .unwrap();
    for (config, portable) in [
        (
            serde_json::json!({"treemap": {"nodeWidth": 1, "nodeHeight": 1}}),
            true,
        ),
        (
            serde_json::json!({"treemap": {"labelColor": "#123456", "showValues": false}}),
            true,
        ),
        (
            serde_json::json!({"treemap": {"labelColor": "#123456", "showValues": true}}),
            false,
        ),
        (
            serde_json::json!({"themeVariables": {"textColor": "#123456"}}),
            true,
        ),
    ] {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(config.clone()));
        let rendered = try_render_treemap_with_theme_requirement(
            "treemap\n\"Leaf\": 12\n",
            &theme,
            engine,
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(
            document
                .descendants()
                .any(|node| node.has_tag_name("text") && node.text() == Some("Leaf"))
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(portable),
            "{config}"
        );
        assert_eq!(
            evidence.theme_residual_count(),
            usize::from(!portable),
            "{config}"
        );
    }
}

#[test]
fn treemap_text_effect_binding_accounts_only_visible_terminals() {
    use merman_render::diagram_theme::{
        DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive,
    };
    let effects = DiagramEffectSet::default()
        .with_graph(
            EffectGraph::new(
                "blur",
                [EffectPrimitive::GaussianBlur {
                    input: EffectInput::SourceGraphic,
                    std_deviation: 1.0,
                }],
            )
            .unwrap(),
        )
        .unwrap()
        .with_binding(EffectBinding::new(ThemeTarget::Text, "blur").unwrap())
        .unwrap();
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_effects(effects))
        .unwrap();
    for (source, visible) in [
        ("treemap\n\"Leaf\": 12\n", true),
        (
            "---\nconfig:\n  treemap:\n    nodeWidth: 1\n    nodeHeight: 1\n---\ntreemap\n\"Leaf\": 12\n",
            false,
        ),
    ] {
        let rendered = try_render_treemap_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.not_applicable_count(), usize::from(!visible));
        assert_eq!(evidence.theme_residual_count(), usize::from(visible));
        let strict = try_render_treemap_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
        );
        if visible {
            assert_eq!(
                strict.err().unwrap().unverified_family_theme(),
                Some((DiagramFamilyId::TREEMAP, 1))
            );
        } else {
            strict.expect("hidden text has no active effect binding");
        }
    }
}

#[test]
fn treemap_source_text_colors_do_not_certify_typed_fill() {
    let theme = treemap_text_fill_theme(CanvasPaint::solid("#123456").unwrap());
    for (has_typed_leaf, style, color) in [
        (false, "color:#abcdef", "#abcdef"),
        (true, "color:#abcdef", "#abcdef"),
        (false, "color:var(--missing),color:#abcdef", "#abcdef"),
        (false, "color:#123456", "#123456"),
        (false, "color:none", "none"),
    ] {
        let source = format!(
            "treemap\nclassDef sourceColor {style};\n\"Section\":::sourceColor\n  \"Owned\": 12:::sourceColor\n{}",
            if has_typed_leaf {
                "  \"Typed\": 12\n"
            } else {
                ""
            },
        );
        let rendered = render_treemap_with_theme(&source, &theme, Engine::new());
        let owned = text_tag_by_text(rendered.svg(), "Owned");
        assert!(
            owned.contains(&format!("fill:{color} !important")),
            "{owned}"
        );
        if has_typed_leaf {
            let typed = text_tag_by_text(rendered.svg(), "Typed");
            assert!(
                typed.contains("#123456") || typed.contains("rgb(18, 52, 86)"),
                "{typed}"
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), usize::from(has_typed_leaf));
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(!has_typed_leaf)
        );
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn treemap_uncertain_source_fill_does_not_certify_a_mixed_surface() {
    let theme = treemap_text_fill_theme(CanvasPaint::solid("#123456").unwrap());
    for style in [
        "color:var(--missing-color)",
        "color:currentColor",
        "color:#abcdef !important",
        "font-family:\"color:face\",color:#abcdef",
    ] {
        let source = format!(
            "treemap\nclassDef uncertain {style};\n\"Uncertain\": 12:::uncertain\n\"Typed\": 12\n"
        );
        let rendered = try_render_treemap_with_theme_requirement(
            &source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0, "{style}");
        assert_eq!(evidence.theme_residual_count(), 1, "{style}");
        let error = try_render_treemap_with_theme_requirement(
            &source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .err()
        .expect("uncertain source paint must fail closed");
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::TREEMAP, 1))
        );
    }
}

#[test]
fn treemap_text_fill_is_not_applicable_without_visible_text_terminals() {
    let theme =
        treemap_text_fill_theme(CanvasPaint::solid("#123456").expect("valid Treemap text color"));
    let rendered = render_treemap_with_theme(
        r#"---
config:
  treemap:
    nodeWidth: 1
    nodeHeight: 1
---
treemap
"Leaf": 12
"#,
        &theme,
        Engine::new(),
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid tiny Treemap SVG");
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text")
            && node.attribute("class") == Some("treemapLabel")
            && node
                .attribute("style")
                .is_some_and(|style| style.contains("display: none"))
    }));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn treemap_explicit_title_color_owners_outrank_typed_fill() {
    let theme =
        treemap_title_fill_theme(CanvasPaint::solid("#123456").expect("valid Treemap title color"));
    let cases = [
        (
            concat!(
                "---\nconfig:\n  treemap:\n    titleColor: '#fedcba'\n---\n",
                "treemap\ntitle Source-owned title\n\"Leaf\": 1\n",
            ),
            Engine::new(),
        ),
        (
            "treemap\ntitle Site-owned title\n\"Leaf\": 1\n",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "titleColor": "#fedcba" }
            }))),
        ),
    ];

    for (source, engine) in cases {
        let rendered = render_treemap_with_theme(source, &theme, engine);
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid owner-precedence SVG");
        let style = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Treemap stylesheet");
        assert!(style.contains(".treemapTitle{fill:#fedcba;"), "{style}");

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn treemap_title_fill_is_not_applicable_without_a_title() {
    let theme =
        treemap_title_fill_theme(CanvasPaint::solid("#123456").expect("valid Treemap title color"));
    let rendered = render_treemap_with_theme("treemap\n\"Leaf\": 1\n", &theme, Engine::new());
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid titleless Treemap SVG");
    assert!(!document.descendants().any(|node| {
        node.has_tag_name("text") && node.attribute("class") == Some("treemapTitle")
    }));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn treemap_typed_font_stack_reaches_css_and_all_text_measurements() {
    let expected_font = "TreemapTyped, monospace";
    let theme = treemap_typography_theme(
        FontStack::new(["TreemapTyped", "monospace"]).expect("valid Treemap font stack"),
    );
    let source = r#"treemap
title Typed treemap
"Section"
  "A long leaf label": 4
  "Another leaf": 2
"#;
    let host = Arc::new(CountingTreemapHost::default());
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Treemap")
        .expect("detect themed Treemap");
    let session = counting_treemap_environment(Arc::clone(&host))
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Treemap session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed Treemap");
    host.reset();

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed Treemap");
    assert!(
        rendered
            .svg()
            .contains(&format!("font-family:{expected_font};")),
        "typed Treemap FontStack must reach the final stylesheet: {}",
        rendered.svg()
    );

    let observed_families = host.observed_font_families();
    assert!(
        !observed_families.is_empty(),
        "Treemap should measure visible text"
    );
    assert!(
        observed_families
            .iter()
            .all(|family| family.as_deref() == Some(expected_font)),
        "every Treemap text measurement must use the resolved typed font: {observed_families:?}"
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn treemap_classdef_typography_drives_measurement_and_final_leaf_styles() {
    let root_font = "TreemapRoot";
    let theme = treemap_typography_theme(
        FontStack::single(root_font).expect("valid Treemap root font stack"),
    );
    let source = r#"treemap
title Root title
classDef sectionFont font-family:SectionFace,font-size:24px,font-weight:normal;
classDef leafFont font-family:"Leaf\,Face"\,serif,font-size:18px,font-style:italic;
"Section":::sectionFont
  "Leaf": 42:::leafFont
"#;
    let host = Arc::new(CountingTreemapHost::default());
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse class-styled Treemap")
        .expect("detect class-styled Treemap");
    let session = counting_treemap_environment(Arc::clone(&host))
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Treemap session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare class-styled Treemap");
    host.reset();

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render class-styled Treemap");
    let styles = host.observed_styles();
    assert!(styles.iter().any(|style| {
        style.font_family.as_deref() == Some(root_font) && style.font_size == 14.0
    }));
    assert!(styles.iter().any(|style| {
        style.font_family.as_deref() == Some("SectionFace")
            && style.font_size == 24.0
            && style.font_weight.as_deref() == Some("normal")
    }));
    assert!(styles.iter().any(|style| {
        style.font_family.as_deref() == Some(r#""Leaf,Face",serif"#)
            && style.font_size == 18.0
            && style.font_style.as_deref() == Some("italic")
    }));

    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Treemap SVG");
    let section_style = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class") == Some("treemapSectionLabel")
                && node.text() == Some("Section")
        })
        .and_then(|node| node.attribute("style"))
        .expect("styled Treemap section label");
    assert!(
        section_style.contains("font-family:SectionFace !important"),
        "{section_style}"
    );
    assert!(
        section_style.contains("font-size:24px !important"),
        "{section_style}"
    );

    let leaf_style = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class") == Some("treemapLabel")
                && node.text() == Some("Leaf")
        })
        .and_then(|node| node.attribute("style"))
        .expect("styled Treemap leaf label");
    assert!(leaf_style.contains(r#"font-family:"Leaf,Face",serif !important"#));
    assert!(leaf_style.contains("font-style:italic !important"));
    assert_eq!(leaf_style.matches("font-size:").count(), 1, "{leaf_style}");
    assert!(
        !leaf_style.contains("font-size:18px !important"),
        "the final D3 font-size mutation must retire the authored leaf size: {leaf_style}"
    );

    let value_style = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class") == Some("treemapValue")
                && node.text() == Some("42")
        })
        .and_then(|node| node.attribute("style"))
        .expect("styled Treemap leaf value");
    assert!(value_style.contains(r#"font-family:"Leaf,Face",serif !important"#));
    assert_eq!(
        value_style.matches("font-size:").count(),
        1,
        "{value_style}"
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn treemap_title_font_size_config_drives_measurement_and_final_css() {
    let theme = treemap_typography_theme(
        FontStack::single("TreemapRoot").expect("valid Treemap root font stack"),
    );
    let source = r#"---
config:
  treemap:
    titleFontSize: 31.5px
---
treemap
title Config-sized title
"Leaf": 42
"#;
    let host = Arc::new(CountingTreemapHost::default());
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse config-sized Treemap title")
        .expect("detect config-sized Treemap title");
    let session = counting_treemap_environment(Arc::clone(&host))
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Treemap session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare config-sized Treemap title");
    host.reset();

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render config-sized Treemap title");
    let title_styles = host.observed_styles_for_text("Config-sized title");
    assert_eq!(title_styles.len(), 2, "title width and height measurements");
    assert!(
        title_styles.iter().all(|style| {
            style.font_family.as_deref() == Some("TreemapRoot") && style.font_size == 31.5
        }),
        "title measurements must use the final configured CSS size: {title_styles:?}"
    );

    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Treemap SVG");
    let style = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Treemap stylesheet");
    let title_rule = css_declarations_for_selector_suffix(style, ".treemapTitle");
    assert!(
        title_rule.contains("font-size:31.5px;"),
        "final Treemap title rule must preserve the measured size: {title_rule}"
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn treemap_unverified_source_fonts_fail_closed_for_portable_themes() {
    let theme = treemap_typography_theme(
        FontStack::single("TreemapTyped").expect("valid Treemap root font stack"),
    );
    let cases = [
        (
            "dynamic font",
            r#"treemap
classDef dynamicFont font-family:var(--treemap-font);
"Leaf": 42:::dynamicFont
"#,
        ),
        (
            "authored important",
            r#"treemap
classDef importantFont font-family:SourceFace !important;
"Leaf": 42:::importantFont
"#,
        ),
        (
            "measurement-affecting white space",
            r#"treemap
classDef preservedSpace white-space:pre;
"Leaf with spaces": 42:::preservedSpace
"#,
        ),
        (
            "dynamic title size",
            r#"---
config:
  treemap:
    titleFontSize: var(--treemap-title-size)
---
treemap
title Dynamic title
"Leaf": 42
"#,
        ),
    ];

    for (label, source) in cases {
        let rendered = try_render_treemap_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap_or_else(|error| panic!("BestEffort must preserve {label}: {error}"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "{label}");
        assert_eq!(evidence.applied_count(), 0, "{label}");
        assert_eq!(evidence.not_applicable_count(), 0, "{label}");
        assert_eq!(evidence.theme_residual_count(), 1, "{label}");

        let error = match try_render_treemap_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
        ) {
            Ok(_) => panic!("RequirePortable must reject {label}"),
            Err(error) => error,
        };
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::TREEMAP, 1)),
            "{label}"
        );
    }
}

#[test]
fn treemap_leaf_label_and_value_remain_visible_and_vertically_ordered() {
    let svg = render_treemap_svg_from_fixture("upstream_treemap_docs_basic_spec.mmd");

    let needle = ">Item A1</text>";
    let end = svg.find(needle).expect("expected Item A1 label");
    let tag = text_tag_by_text(&svg, "Item A1");

    assert!(tag.contains(r#"class="treemapLabel""#));
    assert!(!tag.contains("display: none"));
    assert_eq!(
        font_size_px(tag),
        Some(34.0),
        "Treemap leaf fitting must use Mermaid's getComputedTextLength semantics"
    );

    let rest = &svg[(end + needle.len())..];
    let value_class = rest
        .find(r#"class="treemapValue""#)
        .expect("expected value tag");
    let value_start = rest[..value_class]
        .rfind("<text")
        .expect("expected value tag start");
    let value_end_rel = rest[value_start..]
        .find("</text>")
        .expect("expected value end");
    let value_tag = &rest[value_start..(value_start + value_end_rel + "</text>".len())];
    let label_y = attr_f64(tag, "y").expect("label y");
    let value_y = attr_f64(value_tag, "y").expect("value y");
    assert!(!value_tag.contains("display: none"));
    assert!(value_y > label_y, "value must be placed below its label");
    assert!(
        font_size_px(value_tag).expect("value font size")
            <= font_size_px(tag).expect("label font size")
    );
}

#[test]
fn treemap_huge_source_font_finishes_height_fitting() {
    struct NarrowTextHost;
    impl HostTextMeasurer for NarrowTextHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            Ok(match request.operation {
                TextMeasurementOperation::ComputedLength => Some(HostTextMeasurement::Length(1.0)),
                _ => None,
            })
        }
    }
    let source = "treemap\nclassDef huge font-size:100000000000000000000px;\n\"Leaf\": 42:::huge\n";
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.treemap-narrow").unwrap(),
        "1",
    )
    .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::host_display(
            identity,
            Arc::new(NarrowTextHost),
            TextMeasurementPhase::ALL,
        ))
        .begin_session()
        .unwrap();
    let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let label = text_tag_by_text(rendered.svg(), "Leaf");
    assert!(
        font_size_px(label).is_some_and(|size| size >= 8.0 && size < 1000.0),
        "{label}"
    );
    assert!(!label.contains("display: none"));
}

#[test]
fn treemap_hierarchical_leaf_label_is_visible_with_positive_font_size() {
    let svg = render_treemap_svg_from_fixture("upstream_treemap_docs_hierarchical_spec.mmd");
    let tag = text_tag_by_text(&svg, "Accessories");

    assert!(!tag.contains("display: none"), "{tag}");
    assert!(font_size_px(tag).is_some_and(|size| size > 0.0), "{tag}");
}

#[test]
fn treemap_dark_complex_example_uses_readable_label_colors() {
    let (svg, effective_config) = render_treemap_svg_and_config_from_fixture(
        "upstream_cypress_treemap_spec_9_should_handle_a_complex_example_with_multiple_features_016.mmd",
    );
    let theme = effective_config
        .get("theme")
        .and_then(|v| v.as_str())
        .unwrap_or("<missing>");
    let label_text_color = effective_config
        .pointer("/themeVariables/labelTextColor")
        .and_then(|v| v.as_str())
        .unwrap_or("<missing>");
    let scale_label_color = effective_config
        .pointer("/themeVariables/scaleLabelColor")
        .and_then(|v| v.as_str())
        .unwrap_or("<missing>");

    let engineering_tag = text_tag_by_text(&svg, "Engineering");
    assert!(
        engineering_tag.contains("fill:lightgrey") || engineering_tag.contains("fill: lightgrey"),
        "expected Engineering section label to use lightgrey like upstream, got {engineering_tag}; theme={theme}; labelTextColor={label_text_color}; scaleLabelColor={scale_label_color}"
    );

    let frontend_tag = text_tag_by_text(&svg, "Frontend");
    assert!(
        frontend_tag.contains("fill:lightgrey") || frontend_tag.contains("fill: lightgrey"),
        "expected Frontend leaf label to use lightgrey like upstream, got {frontend_tag}"
    );
}

#[test]
fn treemap_single_leaf_label_uses_readable_fill_over_transparent_cell() {
    let svg = render_treemap_svg_from_source(
        r#"treemap
"Item" : 123.45
"#,
    );

    assert!(
        svg.contains(r#"class="treemapLeaf" fill="transparent""#),
        "expected single top-level leaf to preserve Mermaid's transparent cell fill: {svg}"
    );

    let label_tag = text_tag_by_text(&svg, "Item");
    assert!(
        contains_default_text_fill(label_tag),
        "single-leaf label must remain visible on the white root background: {label_tag}"
    );
    assert!(
        !label_tag.contains("fill:#ffffff")
            && !label_tag.contains("fill: #ffffff")
            && !label_tag.contains("fill: rgb(255, 255, 255)"),
        "single-leaf label should not keep upstream's white-on-transparent fill: {label_tag}"
    );

    let value_tag = text_tag_by_class_and_text(&svg, "treemapValue", "123.45");
    assert!(
        contains_default_text_fill(value_tag),
        "single-leaf value must remain visible on the white root background: {value_tag}"
    );
}

#[test]
fn treemap_classdef_bare_label_style_token_renders_error_like_mermaid_parser() {
    let source = r#"treemap
classDef c fill:#ff0000, stroke:rgb(1\,2\,3), color;
"Root":::c
  "Leaf": 1000.00:::c
"#;

    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            source,
            ParseOptions {
                suppress_errors: true,
            },
        )
        .expect("parse returns suppressed error")
        .expect("diagram detected");

    assert_eq!(parsed.metadata().diagram_type, "error");

    let layout_options = LayoutOptions::default();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin render session");
    let artifact = family::prepare(parsed, &layout_options, session).expect("layout ok");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned();

    assert!(
        svg.contains(r#"aria-roledescription="error""#) && svg.contains("Syntax error in text"),
        "expected Mermaid parser-compatible error SVG for invalid classDef style token: {svg}"
    );
}

#[test]
fn treemap_deep_chain_is_rejected_before_recursive_projection() {
    const DEPTH: usize = 1200;
    let source = deep_treemap_chain(DEPTH);

    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");

    let session = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
        .begin_session()
        .expect("begin render session");
    let error = match family::prepare(parsed, &LayoutOptions::default(), session) {
        Ok(_) => panic!("deep recursive Treemap model must be rejected before projection"),
        Err(error) => error,
    };
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected typed resource-limit error, got {error}");
    };

    assert_eq!(limit.limit, "typed_model_tree_depth");
    assert_eq!(limit.actual, DEPTH + 1);
    assert!(limit.actual > limit.max);
}

#[test]
fn treemap_svg_uses_the_session_measurement_route() {
    let source = r#"treemap
title Routed title
"Section"
  "Measured leaf alpha": 42
  "Measured leaf bravo": 42
  "Measured leaf charlie": 42
  "Measured leaf delta": 42
  "Measured leaf echo": 42
  "Measured leaf foxtrot": 42
  "Measured leaf golf": 42
  "Measured leaf hotel": 42
"#;
    let host = Arc::new(CountingTreemapHost::default());
    let session = counting_treemap_environment(Arc::clone(&host))
        .begin_session()
        .expect("begin render session");
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");
    host.calls.store(0, Ordering::Relaxed);

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render SVG");
    let completion = rendered.into_completion();
    assert_eq!(completion.report().family_id(), DiagramFamilyId::TREEMAP);
    let (host_svg, host_report) = completion.into_output_and_report();

    assert!(
        host.calls.load(Ordering::Relaxed) > 0,
        "Treemap must not bypass the session with a family-local deterministic measurer"
    );
    assert!(
        host_report
            .session_report()
            .measurement()
            .entries()
            .iter()
            .any(|entry| {
                entry.provenance().phase == TextMeasurementPhase::ComputedLength
                    && entry.provenance().operation == TextMeasurementOperation::ComputedLength
                    && entry.provenance().source
                        == merman_render::environment::TextMeasurementSource::Host
            }),
        "Treemap fitting checks must use the exact getComputedTextLength operation"
    );
    assert!(
        host_report
            .session_report()
            .measurement()
            .entries()
            .iter()
            .any(|entry| {
                entry.provenance().operation == TextMeasurementOperation::SimpleBBoxHeight
                    && entry.provenance().source
                        == merman_render::environment::TextMeasurementSource::Host
            }),
        "Treemap title bounds must route SVG bbox height through the session"
    );

    let parity_parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let parity_session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parity_artifact = family::prepare(parity_parsed, &LayoutOptions::default(), parity_session)
        .expect("parity layout");
    let parity_svg = parity_artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("parity SVG")
        .svg()
        .to_owned();
    assert_ne!(
        host_svg, parity_svg,
        "host metrics must change observable geometry"
    );
}
