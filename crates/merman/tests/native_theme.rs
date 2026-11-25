#[cfg(all(feature = "png", feature = "native-fonts", feature = "diagram-pie"))]
#[test]
fn direct_png_uses_the_matte_to_pair_automatic_ink_with_translucent_pie_slices() {
    use merman::svg::export::{NativeFontContext, NativeFontLimits, RasterOptions};
    use merman::theme::{ThemeCompiler, ThemeSelection};
    use merman::{OperationControl, PngRequest, RenderOutput, RenderRequest, Renderer, SvgRequest};

    let fonts = NativeFontContext::from_font_bytes(
        [include_bytes!("../../merman-export/tests/fonts/Cousine-Regular.ttf").to_vec()],
        NativeFontLimits::default(),
        &OperationControl::new(),
    )
    .unwrap();
    let source = "pie\n  \"One\": 1\n  \"Two\": 1\n";
    let render = |mode: &str| {
        let document = format!(
            r##"{{
            "schema_version":2,"extends":"studio",
            "roles":{{"canvas":"#00000000"}},
            "typography":{{"sans":["Cousine","sans-serif"]}},
            "form":{{"chart":{{"pie_opacity":0.65}}}},
            "appearances":{{"light":{{"data":{{
                "fill":["#777777","#777777","#777777","#777777"],"fill_ink":"{mode}"
            }}}}}}
        }}"##
        );
        let theme = ThemeCompiler::new()
            .compile_json(document.as_bytes(), None)
            .unwrap();
        let RenderOutput::Png(Some(png)) = Renderer::new()
            .render(RenderRequest::png(
                source,
                OperationControl::new(),
                PngRequest {
                    svg: SvgRequest::default().with_native_theme(ThemeSelection::compiled(theme)),
                    options: RasterOptions::default()
                        .with_matte("black")
                        .with_font_context(fonts.clone()),
                },
            ))
            .unwrap()
        else {
            panic!("expected a rendered PNG")
        };
        png.bytes().to_vec()
    };

    assert_eq!(render("auto"), render("#F8FAFC"));
    assert_ne!(render("#F8FAFC"), render("#0F172A"));
}

#[cfg(all(feature = "svg", feature = "diagram-info"))]
#[test]
fn native_svg_theme_obeys_source_precedence_and_survives_prepared_source_reuse() {
    use merman::theme::{BuiltinPreset, ThemePrecedence, ThemeSelection};
    use merman::{OperationControl, PrepareSourceRequest, RenderRequest, Renderer, SvgRequest};

    let renderer = Renderer::new();
    let source = "---\nconfig:\n  info:\n    theme: forest\n---\ninfo\n";
    let request = || RenderRequest::svg(source, OperationControl::new(), SvgRequest::default());
    let native = ThemeSelection::preset(BuiltinPreset::Blueprint, None);
    let parity = renderer.render_svg_data(request()).unwrap().unwrap();
    let source_first = renderer
        .render_svg_data(
            request().with_native_theme(native.clone().with_precedence(ThemePrecedence::Source)),
        )
        .unwrap()
        .unwrap();
    assert_eq!(source_first, parity);
    let host_first = renderer
        .render_svg_data(request().with_native_theme(native.clone()))
        .unwrap()
        .unwrap();
    assert!(host_first.contains("#0E3558"));
    assert!(host_first.contains("IBM Plex Mono"));

    let prepared = renderer
        .prepare_source(PrepareSourceRequest::new(source, OperationControl::new()))
        .unwrap()
        .unwrap();
    let reused = renderer
        .render_prepared_svg_data(
            &prepared,
            SvgRequest::default().with_native_theme(native),
            OperationControl::new(),
        )
        .unwrap();
    assert_eq!(reused, host_first);
}

#[cfg(all(feature = "ascii", feature = "diagram-flowchart"))]
#[test]
fn native_theme_reaches_a_truecolor_terminal_render() {
    use merman::ascii::AsciiColorMode;
    use merman::theme::{BuiltinPreset, ThemeCompiler, ThemeSelection};
    use merman::{AsciiRequest, OperationControl, RenderOutput, RenderRequest, Renderer};

    let mut request = AsciiRequest::default();
    request.options.color_mode = AsciiColorMode::TrueColor;
    let render = |theme| {
        let RenderOutput::Ascii(Some(output)) = Renderer::new()
            .render(
                RenderRequest::ascii(
                    "flowchart LR\nA --> B",
                    OperationControl::new(),
                    request.clone(),
                )
                .with_native_theme(theme),
            )
            .unwrap()
        else {
            panic!("expected terminal output");
        };
        output.text
    };
    let builtin = render(ThemeSelection::preset(BuiltinPreset::Cyberpunk, None));
    assert!(builtin.contains("\u{1b}[38;2;41;243;236m"));

    let custom = ThemeCompiler::new()
        .compile_json(
            br##"{
        "schema_version":2,"extends":"cyberpunk","overrides":[
          {"families":["flowchart"],"roles":{"stroke.connector":"#ED1208"}}
        ]
    }"##,
            None,
        )
        .unwrap();
    let themed = render(ThemeSelection::compiled(custom));
    assert!(themed.contains("\u{1b}[38;2;237;18;8m"));
}

#[cfg(all(feature = "svg", feature = "diagram-flowchart"))]
#[test]
fn native_flowchart_title_uses_theme_ink_and_title_typography() {
    use merman::theme::{ThemeCompiler, ThemeSelection};
    use merman::{OperationControl, RenderRequest, Renderer, SvgRequest};

    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
        "schema_version":2,"extends":"studio",
        "roles":{"text.title":"#243648"},
        "typography":{"serif":["Title Display","serif"],"title_family":"serif",
                      "base_size_px":24,"title_weight":650}
    }"##,
            None,
        )
        .unwrap();
    let source = "---\ntitle: A long title with its own typography\n---\nflowchart LR\nA --> B";
    let renderer = Renderer::new();
    let render = |request| {
        renderer
            .render_svg_data(RenderRequest::svg(source, OperationControl::new(), request))
            .unwrap()
            .unwrap()
    };
    let classic = render(SvgRequest::default());
    let themed = render(SvgRequest::default().with_native_theme(ThemeSelection::compiled(theme)));
    let doc = roxmltree::Document::parse(&themed).unwrap();
    let css = doc
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains(".flowchartTitleText{text-anchor:middle;font-size:30px;fill:#243648;font-family:\"Title Display\", serif;font-weight:650;}"));
    let classic_doc = roxmltree::Document::parse(&classic).unwrap();
    let width = |document: &roxmltree::Document| {
        document
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_ascii_whitespace()
            .nth(2)
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    assert!(width(&doc) > width(&classic_doc));
}

#[cfg(all(feature = "svg", feature = "diagram-flowchart"))]
#[test]
fn native_flowchart_preserves_local_style_and_paints_container_edge_labels() {
    use merman::theme::{Appearance, BuiltinPreset, Color, ThemeMode, ThemeSelection};
    use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

    let source = "flowchart LR\nsubgraph Outer\nA[Draft] -->|review| B{Approved}\nend\nstyle B fill:#AA1122\nB --> C((Done))";
    let RenderOutput::Svg(Some(output)) = Renderer::new()
        .render(RenderRequest::svg(
            source,
            OperationControl::new(),
            SvgRequest::default()
                .with_background(Color::TRANSPARENT)
                .with_native_theme(ThemeSelection::preset(BuiltinPreset::Blueprint, None)),
        ))
        .unwrap()
    else {
        panic!("expected SVG output");
    };
    let document = roxmltree::Document::parse(output.svg()).unwrap();
    let backplate = document
        .descendants()
        .find(|node| node.attribute("class") == Some("merman-edge-label-background"))
        .unwrap();
    assert_eq!(backplate.attribute("fill"), Some("#103B61"));
    assert!(output.svg().contains("#AA1122"));
    assert!(!output.svg().contains("fill=\"#0E3558\""));
    let outcome = output.theme_outcome();
    assert_eq!(outcome.mode, ThemeMode::MermanNative);
    assert_eq!(outcome.appearance, Some(Appearance::Dark));
    assert_eq!(outcome.preset_id, Some(BuiltinPreset::Blueprint));
}

#[cfg(all(feature = "svg", feature = "diagram-xychart"))]
#[test]
fn native_xychart_uses_chart_polish_and_the_shared_canvas() {
    use merman::theme::{BuiltinPreset, Color, ThemeSelection};
    use merman::{OperationControl, RenderRequest, Renderer, SvgRequest};

    let source = "xychart\n  title \"Traffic\"\n  x-axis [A, B, C]\n  y-axis 0 --> 10\n  bar [2, 4, 6]\n  line [5, 7, 8]";
    let renderer = Renderer::new();
    let render = |preset| {
        let options = SvgRequest::default()
            .with_native_theme(ThemeSelection::preset(preset, None))
            .with_background(Color::rgb(17, 34, 51));
        renderer
            .render_svg_data(RenderRequest::svg(source, OperationControl::new(), options))
            .unwrap()
            .unwrap()
    };
    let studio = render(BuiltinPreset::Studio);
    let document = roxmltree::Document::parse(&studio).unwrap();
    let group = |class| {
        document
            .descendants()
            .find(|node| node.attribute("class") == Some(class))
            .unwrap()
    };
    assert_eq!(group("background").attribute("fill"), Some("none"));
    assert_eq!(
        group("merman-theme-canvas")
            .descendants()
            .find(|node| node.has_tag_name("rect"))
            .unwrap()
            .attribute("fill"),
        Some("#112233")
    );
    assert!(
        group("gridlines")
            .children()
            .any(|node| node.has_tag_name("path"))
    );
    assert!(
        group("bar-plot-0")
            .children()
            .any(|node| node.has_tag_name("path"))
    );
    assert_eq!(
        group("line-markers-1")
            .children()
            .filter(|node| node.has_tag_name("circle"))
            .count(),
        3
    );

    let blueprint = render(BuiltinPreset::Blueprint);
    assert!(blueprint.contains("stroke-width=\"1.5\""));
    let monograph = render(BuiltinPreset::Monograph);
    assert!(monograph.contains("fill=\"url(#xychart-xy-series-0)\""));
}

#[cfg(all(feature = "svg", feature = "diagram-state"))]
#[test]
fn native_state_draws_depth_and_preserves_authored_node_paint() {
    use merman::theme::{BuiltinPreset, ThemeSelection};
    use merman::{OperationControl, RenderRequest, Renderer, SvgRequest};

    let source = "---\nconfig:\n  look: classic\n  htmlLabels: false\n---\nstateDiagram-v2\n  [*] --> Idle\n  Idle --> Active: continue\n  classDef picked fill:#123456,stroke:#234567\n  class Active picked";
    let renderer = Renderer::new();
    let render = |options| {
        renderer
            .render_svg_data(RenderRequest::svg(source, OperationControl::new(), options))
            .unwrap()
            .unwrap()
    };
    let classic = render(SvgRequest::default());
    let native = render(
        SvgRequest::default()
            .with_native_theme(ThemeSelection::preset(BuiltinPreset::PaperCut, None)),
    );
    assert!(!classic.contains("data-merman-theme-depth"));
    let document = roxmltree::Document::parse(&native).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("data-merman-theme-depth") == Some("node"))
            .count(),
        6
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("rect")
            && node.attribute("rx") == Some("8")
            && node
                .attribute("style")
                .is_some_and(|style| style.contains("fill:#123456 !important"))
    }));
}
