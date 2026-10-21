use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::{
    LayoutOptions,
    environment::RenderEnvironment,
    family,
    presentation::{HostTheme, HostThemePreset, Presentation, ThemeRole},
    svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions},
};
use std::collections::BTreeMap;

const PAINT: &str =
    "zenuml\ntitle Map\nAlice->Bob: Hello\nloop Repeat {\nBob->Alice: Reply\n}\n=== Note ===\n";

fn render(source: &str, theme: Option<HostTheme>, site: serde_json::Value) -> String {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(site));
    let engine = if let Some(theme) = theme {
        Presentation::new()
            .with_theme(theme)
            .resolve()
            .materialize_engine(engine)
    } else {
        engine
    };
    render_engine(source, engine)
}

fn render_engine(source: &str, engine: Engine) -> String {
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned()
}

fn rule<'a>(svg: &'a str, class: &str) -> &'a str {
    svg.split_once(&format!("#zenuml .{class}"))
        .unwrap_or_else(|| panic!("missing {class}"))
        .1
        .split_once('{')
        .unwrap()
        .1
        .split_once('}')
        .unwrap()
        .0
}

#[test]
fn zenuml_quoted_host_fonts_preserve_parsed_stylesheet_and_native_paint() {
    for preset in [HostThemePreset::EditorLight, HostThemePreset::EditorDark] {
        for native in [false, true] {
            let theme = HostTheme::from_preset(preset);
            let actor_fill = theme.role(ThemeRole::ActorBackground).unwrap().to_owned();
            let actor_text = theme.role(ThemeRole::ActorText).unwrap().to_owned();
            let engine = Presentation::new()
                .with_theme(theme)
                .resolve()
                .materialize_engine(Engine::new());
            let parsed = engine
                .parse_diagram_for_render_model_sync(PAINT, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let rendered =
                family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
                    .unwrap()
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap();
            let svg = if native {
                rendered
                    .finalize_resvg(&SvgPipeline::resvg_safe())
                    .unwrap()
                    .svg()
                    .as_str()
                    .to_owned()
            } else {
                rendered.svg().to_owned()
            };
            let document = roxmltree::Document::parse(&svg).unwrap();
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap();
            let rules = parse_stylesheet(css);
            for class in [
                "frame-title",
                "participant-label",
                "message-label",
                "group-title-text",
            ] {
                let declarations = &rules[&format!("#zenuml .{class}")];
                assert_eq!(
                    declarations["font-family"],
                    r#"Inter,ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif"#,
                    "{preset:?}/native={native}/{class}"
                );
            }
            assert_eq!(rules["#zenuml .participant-box"]["fill"], actor_fill);
            assert_eq!(rules["#zenuml .participant-label"]["fill"], actor_text);
            assert_eq!(rules["#zenuml .participant-label"]["font-size"], "14px");
        }
    }
}

fn parse_stylesheet(css: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    use cssparser::{Delimiter, ParseError, Parser, Token};

    let mut parser = Parser::new(css);
    let mut rules = BTreeMap::new();
    while !parser.is_exhausted() {
        let selector = parser
            .parse_until_before(Delimiter::CurlyBracketBlock, |input| {
                let start = input.position();
                while input.next().is_ok() {}
                Ok::<_, ParseError<()>>(input.slice_from(start).trim().to_owned())
            })
            .unwrap();
        parser.expect_curly_bracket_block().unwrap();
        let declarations = parser
            .parse_nested_block(|input| {
                let mut declarations = BTreeMap::new();
                while !input.is_exhausted() {
                    let name = input.expect_ident_cloned()?.to_string();
                    input.expect_colon()?;
                    let value = input.parse_until_before(Delimiter::Semicolon, |input| {
                        let start = input.position();
                        while let Ok(token) = input.next() {
                            assert!(!matches!(token, Token::BadString(_) | Token::BadUrl(_)));
                        }
                        Ok::<_, ParseError<()>>(input.slice_from(start).trim().to_owned())
                    })?;
                    assert!(declarations.insert(name, value).is_none());
                    let _ = input.try_parse(|input| input.expect_semicolon());
                }
                Ok::<_, ParseError<()>>(declarations)
            })
            .unwrap();
        assert!(rules.insert(selector, declarations).is_none());
    }
    rules
}

#[test]
fn zenuml_host_presets_style_frame_text_actor_message_and_note_surfaces() {
    for preset in HostThemePreset::ALL {
        let theme = HostTheme::from_preset(preset);
        let values = [
            ("frame-border-inner", "fill", ThemeRole::Canvas),
            ("frame-title", "fill", ThemeRole::Text),
            ("participant-box", "fill", ThemeRole::ActorBackground),
            ("participant-label", "fill", ThemeRole::ActorText),
            ("message-label", "fill", ThemeRole::Text),
            ("message-line", "stroke", ThemeRole::Line),
            ("lifeline", "stroke", ThemeRole::Border),
            ("divider-bg", "fill", ThemeRole::NoteBackground),
            ("divider-label", "fill", ThemeRole::NoteText),
        ]
        .map(|(class, property, role)| {
            (class, format!("{property}:{}", theme.role(role).unwrap()))
        });
        let svg = render(PAINT, Some(theme), serde_json::json!({}));
        roxmltree::Document::parse(&svg).unwrap();
        for (class, paint) in values {
            assert!(
                rule(&svg, class).contains(&paint),
                "{} {class}: {paint}; {}",
                preset.id(),
                rule(&svg, class)
            );
        }
    }
}

#[test]
fn zenuml_namespace_source_colors_and_fonts_override_host_property_defaults() {
    let svg = render(
        PAINT,
        Some(HostTheme::from_preset(HostThemePreset::EditorDark)),
        serde_json::json!({"themeVariables":{"zenuml":{"textColor":"#123456","actorBackgroundColor":"#abcdef","backgroundColor":"#f0f1f2","noteBackgroundColor":"#fefefe","noteBorderColor":"#456789","noteTextColor":"#234567","fontFamily":"Inter, sans-serif","fontSize":"24px"}}}),
    );
    for (class, paint) in [
        ("frame-title", "fill:#123456"),
        ("message-label", "fill:#123456"),
        ("participant-box", "fill:#abcdef"),
        ("participant-box", "stroke:#475569"),
        ("frame-border-inner", "fill:#f0f1f2"),
        ("divider-bg", "fill:#fefefe"),
        ("divider-bg", "stroke:#456789"),
        ("divider-label", "fill:#234567"),
        ("participant-label", "font-size:24px"),
        ("message-label", "font-size:21px"),
    ] {
        assert!(
            rule(&svg, class).contains(paint),
            "{class}: {paint}; {}",
            rule(&svg, class)
        );
    }
    assert!(rule(&svg, "participant-label").contains("font-family:Inter,sans-serif"));
    let document = roxmltree::Document::parse(&svg).unwrap();
    let actor = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect") && node.attribute("class") == Some("participant-box")
        })
        .unwrap();
    assert!(actor.attribute("height").unwrap().parse::<f64>().unwrap() >= 58.0);
}

#[test]
fn zenuml_without_host_keeps_companion_frame_and_text_defaults() {
    let svg = render(PAINT, None, serde_json::json!({}));
    assert_eq!(rule(&svg, "frame-border-outer"), "fill:#666");
    assert_eq!(
        rule(&svg, "frame-title"),
        "font-family:Helvetica,Verdana,serif;font-size:16px;font-weight:600;fill:#222"
    );
    assert_eq!(
        rule(&svg, "participant-box"),
        "fill:#fff;stroke:#666;stroke-width:2"
    );
    assert_eq!(
        rule(&svg, "message-label"),
        "font-family:Helvetica,Verdana,serif;font-size:14px;fill:#222"
    );
}

#[test]
fn zenuml_font_only_presentation_changes_typography_without_enabling_host_paint() {
    let theme = HostTheme::new()
        .try_with_font_family("Inter, sans-serif")
        .unwrap()
        .try_with_font_size("24px")
        .unwrap();
    let svg = render(PAINT, Some(theme), serde_json::json!({}));
    assert_eq!(rule(&svg, "frame-border-outer"), "fill:#666");
    assert_eq!(
        rule(&svg, "participant-box"),
        "fill:#fff;stroke:#666;stroke-width:2"
    );
    assert!(rule(&svg, "participant-label").contains("font-size:24px"));
    assert!(rule(&svg, "participant-label").contains("font-family:Inter,sans-serif"));
    assert!(rule(&svg, "message-label").contains("font-size:21px;fill:#222"));
    let document = roxmltree::Document::parse(&svg).unwrap();
    let actor = document
        .descendants()
        .find(|node| node.attribute("class") == Some("participant-box"))
        .unwrap();
    assert!(actor.attribute("height").unwrap().parse::<f64>().unwrap() >= 58.0);
}

#[test]
fn zenuml_profile_only_presentation_keeps_companion_paint_and_typography() {
    let presentation = Presentation::new()
        .with_profile(merman_render::presentation::PresentationProfile::MermanModern)
        .resolve();
    let svg = render_engine(PAINT, presentation.materialize_engine(Engine::new()));
    assert_eq!(rule(&svg, "frame-border-outer"), "fill:#666");
    assert_eq!(
        rule(&svg, "participant-box"),
        "fill:#fff;stroke:#666;stroke-width:2"
    );
    assert_eq!(
        rule(&svg, "participant-label"),
        "font-family:Helvetica,Verdana,serif;font-size:16px;fill:#222"
    );
}
