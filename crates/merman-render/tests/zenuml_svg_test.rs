use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::presentation::{HostTheme, HostThemePreset, Presentation, ThemeRole};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_zenuml(source: &str, presentation: Presentation) -> String {
    let parsed = presentation
        .resolve()
        .materialize_engine(Engine::new())
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

fn css_rule<'a>(svg: &'a str, class: &str) -> &'a str {
    let prefix = format!("#zenuml .{class}");
    svg.split_once(&prefix)
        .unwrap_or_else(|| panic!("missing {class} rule"))
        .1
        .split_once('{')
        .unwrap()
        .1
        .split_once('}')
        .unwrap()
        .0
}

fn has_class(document: &roxmltree::Document<'_>, class: &str) -> bool {
    document.descendants().any(|node| {
        node.attribute("class")
            .is_some_and(|classes| classes.split_whitespace().any(|value| value == class))
    })
}

const ZENUML_PAINT: &str = r#"zenuml
title Theme coverage
group Business { @Actor <<Client>> Alice @Database Bob }
@Starter(Alice)
// Important comment
Bob.call() {
  if(ready) {
    Bob.self()
  } else {
    Alice->Bob: Retry
  }
  return result
}
== Phase two ==
while(active) {
  Alice->Bob: Continue
}
new Service
Bob-->Alice: Reply
"#;

#[test]
fn zenuml_plain_mermaid_dark_and_empty_presentation_keep_plugin_defaults() {
    let source = "zenuml\nAlice->Bob: Hello\nBob-->Alice: Reply\n";
    let baseline = render_zenuml(source, Presentation::new());
    assert_eq!(
        baseline,
        render_zenuml(source, Presentation::new().with_theme(HostTheme::new()))
    );
    let dark = format!("---\nconfig:\n  theme: dark\n  fontFamily: UnrelatedFont\n---\n{source}");
    assert_eq!(baseline, render_zenuml(&dark, Presentation::new()));
    assert_eq!(
        css_rule(&baseline, "participant-box"),
        "fill:#fff;stroke:#666;stroke-width:2"
    );
    assert_eq!(
        css_rule(&baseline, "message-label"),
        "font-family:Helvetica,Verdana,serif;font-size:14px;fill:#222"
    );
}

#[test]
fn zenuml_presentation_roles_cover_painted_groups_fragments_notes_occurrences_and_arrows() {
    for preset in [HostThemePreset::EditorLight, HostThemePreset::EditorDark] {
        let theme = HostTheme::from_preset(preset)
            .try_with_font_family("Inter, sans-serif")
            .unwrap();
        let role = |role| theme.role(role).unwrap().to_string();
        let canvas = role(ThemeRole::Canvas);
        let text = role(ThemeRole::Text);
        let line = role(ThemeRole::Line);
        let border = role(ThemeRole::Border);
        let actor_background = role(ThemeRole::ActorBackground);
        let actor_text = role(ThemeRole::ActorText);
        let note_background = role(ThemeRole::NoteBackground);
        let note_text = role(ThemeRole::NoteText);
        let activation_background = role(ThemeRole::ActivationBackground);
        let svg = render_zenuml(ZENUML_PAINT, Presentation::new().with_theme(theme));
        let document = roxmltree::Document::parse(&svg).unwrap();
        for class in [
            "frame-border-inner",
            "participant-group",
            "group-title-bg",
            "participant-box",
            "participant-icon",
            "stereotype-label",
            "message-line",
            "self-call",
            "return-line",
            "occurrence",
            "fragment-border",
            "fragment-header",
            "fragment-icon",
            "fragment-section-label",
            "divider-bg",
            "comment-text",
            "creation",
        ] {
            assert!(has_class(&document, class), "missing painted {class}");
        }
        for (class, declaration) in [
            ("frame-border-inner", format!("fill:{canvas}")),
            ("frame-border-outer", format!("fill:{border}")),
            ("participant-box", format!("fill:{actor_background}")),
            ("participant-label", format!("fill:{actor_text}")),
            ("message-label", format!("fill:{text}")),
            ("message-line", format!("stroke:{line}")),
            ("return-line", format!("stroke:{line}")),
            ("occurrence", format!("fill:{activation_background}")),
            ("fragment-label", format!("fill:{text}")),
            ("fragment-icon", format!("color:{text}")),
            ("divider-bg", format!("fill:{note_background}")),
            ("divider-label", format!("fill:{note_text}")),
        ] {
            assert!(
                css_rule(&svg, class).contains(&declaration),
                "{class}: {declaration}"
            );
        }
        assert!(css_rule(&svg, "group-title-text").contains("font-family:Inter,sans-serif"));
        let stereotype = document
            .descendants()
            .find(|node| node.attribute("class") == Some("stereotype-label"))
            .unwrap();
        assert_eq!(
            stereotype.attribute("style"),
            Some(format!("fill:{actor_text};").as_str())
        );
        let self_call = document
            .descendants()
            .find(|node| node.attribute("class") == Some("message self-call"))
            .unwrap();
        assert!(
            self_call
                .descendants()
                .filter(|node| node.has_tag_name("path"))
                .all(|node| node.attribute("stroke") == Some(line.as_str()))
        );
        let arrow = document
            .descendants()
            .find(|node| {
                node.attribute("class")
                    .is_some_and(|classes| classes.starts_with("arrow-head"))
                    && node.has_tag_name("svg")
            })
            .unwrap();
        assert!(
            arrow
                .descendants()
                .filter(|node| node.has_tag_name("path"))
                .all(|node| node.attribute("stroke") == Some(line.as_str()))
        );
        let loop_icon = document
            .descendants()
            .find(|node| node.attribute("data-icon") == Some("loop"))
            .unwrap();
        assert_eq!(loop_icon.attribute("fill"), Some(text.as_str()));
    }
}

#[test]
fn zenuml_presentation_keeps_source_styles_and_explicit_nested_frontmatter_overrides() {
    let source = r#"---
config:
  themeVariables:
    zenuml:
      lineColor: '#112233'
      fontFamily: 'Georgia, serif'
---
zenuml
@Actor Alice #FFEBE6
Bob
// <red> (bold) Important
Alice->Bob: Hello
"#;
    let svg = render_zenuml(
        source,
        Presentation::new().with_theme(HostTheme::from_preset(HostThemePreset::EditorDark)),
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let participant = document
        .descendants()
        .find(|node| node.attribute("data-participant") == Some("Alice") && node.has_tag_name("g"))
        .unwrap();
    let rect = participant
        .children()
        .find(|node| node.has_tag_name("rect"))
        .unwrap();
    assert_eq!(rect.attribute("style"), Some("fill:#FFEBE6;"));
    let comment = document
        .descendants()
        .find(|node| node.attribute("class") == Some("comment-text"))
        .unwrap();
    assert_eq!(comment.attribute("style"), Some("fill:red"));
    let label = document
        .descendants()
        .find(|node| node.attribute("class") == Some("message-label"))
        .unwrap();
    assert_eq!(label.attribute("style"), Some("font-weight:bold"));
    assert!(css_rule(&svg, "message-line").contains("stroke:#112233"));
    assert!(css_rule(&svg, "participant-label").contains("font-family:Georgia,serif"));
}

#[test]
fn zenuml_presentation_font_sizes_reserve_participant_and_fragment_title_rows() {
    for size in [14.0, 18.0, 24.0] {
        let theme = HostTheme::from_preset(HostThemePreset::EditorDark)
            .try_with_font_family("Inter, sans-serif")
            .unwrap()
            .try_with_font_size(format!("{size}px"))
            .unwrap();
        let svg = render_zenuml(ZENUML_PAINT, Presentation::new().with_theme(theme));
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert!(css_rule(&svg, "participant-label").contains(&format!("font-size:{size}px")));
        let scale = size / 16.0;
        let participant = document
            .descendants()
            .find(|node| {
                node.attribute("data-participant") == Some("Alice") && node.has_tag_name("g")
            })
            .unwrap();
        let rect = participant
            .children()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        let height = rect.attribute("height").unwrap().parse::<f64>().unwrap();
        assert!(
            height >= 2.0 * size,
            "two participant rows must fit at {size}px"
        );
        let stereotype = participant
            .children()
            .find(|node| node.attribute("class") == Some("stereotype-label"))
            .unwrap();
        let label = participant
            .children()
            .find(|node| node.attribute("class") == Some("participant-label"))
            .unwrap();
        let separation = label.attribute("y").unwrap().parse::<f64>().unwrap()
            - stereotype.attribute("y").unwrap().parse::<f64>().unwrap();
        assert!((separation - size).abs() < 1e-9);
        let rect_top = rect.attribute("y").unwrap().parse::<f64>().unwrap();
        assert!(
            stereotype.attribute("y").unwrap().parse::<f64>().unwrap() - size / 2.0 >= rect_top
        );
        assert!(
            label.attribute("y").unwrap().parse::<f64>().unwrap() + size / 2.0 <= rect_top + height
        );
        for header in document
            .descendants()
            .filter(|node| node.attribute("class") == Some("fragment-header"))
        {
            assert_eq!(
                header.attribute("height").unwrap().parse::<f64>().unwrap(),
                25.0 * scale
            );
            assert!(25.0 * scale > 14.0 * scale);
        }
    }
}

#[test]
fn zenuml_partial_roles_select_readable_actor_text_and_keep_unrelated_plugin_defaults() {
    let theme = HostTheme::new()
        .try_with_role(ThemeRole::Canvas, "#ffffff")
        .unwrap()
        .try_with_role(ThemeRole::Surface, "#111111")
        .unwrap()
        .try_with_role(ThemeRole::SurfaceMuted, "#eeeeee")
        .unwrap()
        .try_with_series_palette(["#ff0000", "#00ff00"])
        .unwrap();
    let svg = render_zenuml(ZENUML_PAINT, Presentation::new().with_theme(theme));
    assert!(css_rule(&svg, "participant-box").contains("fill:#111111"));
    assert!(css_rule(&svg, "participant-label").contains("fill:#ffffff"));
    assert!(css_rule(&svg, "message-label").contains("fill:#000000"));
    assert!(css_rule(&svg, "group-title-text").contains("fill:#ffffff"));
    assert!(css_rule(&svg, "fragment-header").contains("fill:#eeeeee"));
    assert!(css_rule(&svg, "fragment-label").contains("fill:#000000"));
    assert!(css_rule(&svg, "fragment-condition").contains("fill:#000000"));
    assert!(css_rule(&svg, "fragment-icon").contains("color:#000000"));
    assert!(css_rule(&svg, "participant-label").contains("font-family:Helvetica,Verdana,serif"));
    assert!(css_rule(&svg, "message-line").contains("stroke:#000"));
}

#[test]
fn zenuml_canvas_only_presentation_preserves_unmodified_actor_note_and_group_text() {
    let theme = HostTheme::new()
        .try_with_role(ThemeRole::Canvas, "#111111")
        .unwrap();
    let svg = render_zenuml(ZENUML_PAINT, Presentation::new().with_theme(theme));
    let document = roxmltree::Document::parse(&svg).unwrap();
    for (class, declaration) in [
        ("frame-border-inner", "fill:#111111"),
        ("participant-box", "fill:#fff"),
        ("participant-label", "fill:#222"),
        ("group-title-bg", "fill:#fff"),
        ("group-title-text", "fill:#222"),
        ("divider-bg", "fill:#fff5ad"),
        ("divider-label", "fill:#333"),
        ("message-label", "fill:#ffffff"),
    ] {
        assert!(
            css_rule(&svg, class).contains(declaration),
            "{class}: {declaration}"
        );
    }
    let stereotype = document
        .descendants()
        .find(|node| node.attribute("class") == Some("stereotype-label"))
        .unwrap();
    assert_eq!(stereotype.attribute("style"), Some("fill:#222;"));
}

#[test]
fn zenuml_small_presentation_font_keeps_icons_inside_their_reserved_rows() {
    let source = format!("{ZENUML_PAINT}\nBob->Bob: Async self\nAlice->Bob: Final\n");
    let theme = HostTheme::new().try_with_font_size("8px").unwrap();
    let svg = render_zenuml(&source, Presentation::new().with_theme(theme));
    let document = roxmltree::Document::parse(&svg).unwrap();
    let coordinate = |node: roxmltree::Node<'_, '_>, attribute| {
        node.attribute(attribute).unwrap().parse::<f64>().unwrap()
    };
    for fragment in document.descendants().filter(|node| {
        node.attribute("class")
            .is_some_and(|classes| classes.starts_with("fragment fragment-"))
    }) {
        let header = fragment
            .children()
            .find(|node| node.attribute("class") == Some("fragment-header"))
            .unwrap();
        let icon = fragment
            .children()
            .find(|node| node.attribute("class") == Some("fragment-icon"))
            .unwrap();
        let label = fragment
            .children()
            .find(|node| node.attribute("class") == Some("fragment-label"))
            .unwrap();
        assert_eq!(coordinate(icon, "width"), 10.0);
        assert_eq!(coordinate(icon, "height"), 12.0);
        assert!(
            coordinate(icon, "y") + coordinate(icon, "height")
                <= coordinate(header, "y") + coordinate(header, "height")
        );
        assert!(coordinate(icon, "x") + coordinate(icon, "width") < coordinate(label, "x"));
    }
    let final_message = document
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("message")
                && node.children().any(|text| {
                    text.attribute("class") == Some("message-label") && text.text() == Some("Final")
                })
        })
        .unwrap();
    let next_line = final_message
        .children()
        .find(|node| node.attribute("class") == Some("message-line"))
        .unwrap();
    for call in document
        .descendants()
        .filter(|node| node.attribute("class") == Some("message self-call"))
    {
        let icon = call
            .children()
            .find(|node| node.has_tag_name("svg"))
            .unwrap();
        assert_eq!(coordinate(icon, "width"), 15.0);
        assert_eq!(coordinate(icon, "height"), 12.0);
        assert_eq!(icon.attribute("viewBox"), Some("0 0 30 24"));
        assert!(coordinate(icon, "y") + coordinate(icon, "height") < coordinate(next_line, "y1"));
    }
    for participant in document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("participant"))
    {
        if let Some(icon) = participant
            .children()
            .find(|node| node.attribute("class") == Some("participant-icon"))
        {
            let rect = participant
                .children()
                .find(|node| node.attribute("class") == Some("participant-box"))
                .unwrap();
            let transform = icon.attribute("transform").unwrap();
            let translation = transform
                .split_once("translate(")
                .unwrap()
                .1
                .split_once(')')
                .unwrap()
                .0;
            let icon_y = translation
                .split(',')
                .nth(1)
                .unwrap()
                .trim()
                .parse::<f64>()
                .unwrap();
            assert!(icon_y >= coordinate(rect, "y"));
            assert!(icon_y + 12.0 <= coordinate(rect, "y") + coordinate(rect, "height"));
        }
    }
}
