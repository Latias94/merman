#![cfg(feature = "diagram-packet")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::PacketDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{NativeTheme, ThemeCompiler};

#[test]
fn native_theme_v2_packet_styles_fields_and_bit_labels_without_relayout() {
    let source = r#"---
title: Packet header
config:
  packet:
    bitsPerRow: 16
    bitOrder: descending
  themeVariables:
    packet:
      labelColor: '#AABBCC'
      byteFontSize: '9px'
---
packet
0-7: "Header"
8-15: "Payload"
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version":2,"extends":"paper",
                "roles":{"surface.node":"#BEEFD0","stroke.border":"#405060",
                         "text.muted":"#345678","text.title":"#243648"},
                "typography":{"mono":["Packet Mono","monospace"],"base_size_px":21},
                "form":{"stroke_width":3}
            }"##,
            None,
        )
        .unwrap();
    let environment = RenderEnvironment::deterministic();
    let layout_options = LayoutOptions::default();
    let mermaid = family::prepare(
        parsed.presentation(),
        &layout_options,
        environment.begin_session().unwrap(),
    )
    .unwrap();
    let native = family::prepare(
        parsed.presentation(),
        &layout_options,
        environment
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        mermaid.layout_json().unwrap()["layout"],
        native.layout_json().unwrap()["layout"]
    );
    let rendered = native
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains(".packetBlock{stroke:#405060;stroke-width:3;fill:#BEEFD0;}"));
    assert!(css.contains(".packetByte.start{fill:#345678;}"));
    assert!(css.contains(".packetByte{font-size:9px;}"));
    assert!(css.contains(".packetByte{font-family:\"Packet Mono\", monospace;}"));
    assert!(css.contains(".packetLabel{fill:#AABBCC;font-size:18.000px;}"));
    assert!(css.contains(".packetTitle{fill:#243648;font-size:21.000px;}"));
}

fn independent_packet_presentation(
    packet: Vec<Vec<merman_model::families::packet::PacketBlock>>,
    config: serde_json::Value,
) -> merman_model::presentation::PresentationDiagram {
    use merman_model::families::packet::PacketDiagramModel;
    use merman_model::presentation::{
        AuthoredPresentationMetadata, PresentationContext, PresentationDiagram, RenderSemanticModel,
    };
    use merman_model::resources::InputResourcePolicy;
    use merman_model::{MermaidConfig, OperationControl};

    let config = MermaidConfig::from_value(config);
    PresentationDiagram::try_new(
        AuthoredPresentationMetadata {
            diagram_type: "packet".to_string(),
            config: config.clone(),
            effective_config: config,
            title: None,
        },
        RenderSemanticModel::Packet(PacketDiagramModel {
            packet,
            ..Default::default()
        }),
        PresentationContext::None,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .expect("independent Packet construction is active")
    .expect("small Packet model passes construction resource admission")
}

#[test]
fn packet_direct_input_rejects_impossible_block_ranges() {
    use merman_model::families::packet::PacketBlock;

    for (start, end) in [
        (-1.0, 0.0),
        (2.0, 0.0),
        (0.0, f64::MAX),
        (f64::NAN, 1.0),
        (0.0, f64::INFINITY),
        (0.0, -0.5),
    ] {
        for order in ["ascending", "descending"] {
            let parsed = independent_packet_presentation(
                vec![vec![PacketBlock {
                    start,
                    end,
                    bits: 1.0,
                    label: "Field".to_string(),
                }]],
                serde_json::json!({"packet": {"bitOrder": order}}),
            );
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let error = family::prepare(&parsed, &LayoutOptions::default(), session)
                .err()
                .expect("impossible Packet ranges must return an error instead of overflowing");
            assert!(matches!(error, merman_render::Error::InvalidModel { .. }));
        }
    }
}

#[test]
fn packet_direct_input_preserves_split_rows_and_single_bit_fields() {
    use merman_model::families::packet::PacketBlock;

    for order in ["ascending", "descending"] {
        let block = |start, end| PacketBlock {
            start,
            end,
            bits: end - start + 1.0,
            label: format!("{start}-{end}"),
        };
        let parsed = independent_packet_presentation(
            vec![
                vec![block(0.0, 7.0)],
                vec![block(8.0, 11.0), block(12.0, 12.0)],
            ],
            serde_json::json!({"packet": {"bitsPerRow": 8, "bitOrder": order}}),
        );
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
            .expect("source-shaped split rows remain valid in both bit orders");
        let projection = artifact.layout_json().unwrap();
        let layout: PacketDiagramLayout =
            serde_json::from_value(projection["layout"]["PacketDiagram"].clone()).unwrap();
        assert_eq!(layout.words.len(), 2);
        let blocks: Vec<_> = layout.words.iter().flat_map(|word| &word.blocks).collect();
        assert_eq!(
            blocks
                .iter()
                .map(|block| (block.start, block.end))
                .collect::<Vec<_>>(),
            [(0.0, 7.0), (8.0, 11.0), (12.0, 12.0)]
        );
        assert_eq!(
            blocks.iter().map(|block| block.width).collect::<Vec<_>>(),
            [251.0, 123.0, 27.0]
        );
        assert_eq!(
            blocks.iter().map(|block| block.x).collect::<Vec<_>>(),
            if order == "ascending" {
                [1.0, 1.0, 129.0]
            } else {
                [1.0, 129.0, 97.0]
            }
        );
    }
}

#[test]
fn packet_direct_input_validation_preserves_operation_error_priority() {
    use merman_model::OperationControl;
    use merman_model::families::packet::PacketBlock;
    use merman_render::resources::{RenderResourcePolicy, ResourceLimitId};

    let parsed = independent_packet_presentation(
        vec![vec![PacketBlock {
            start: 0.0,
            end: 9_223_372_036_854_775_808.0,
            bits: 1.0,
            label: "Field".to_string(),
        }]],
        serde_json::json!({}),
    );
    let control = OperationControl::new();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())
        .unwrap();
    control.cancel();
    assert!(matches!(
        family::prepare(&parsed, &LayoutOptions::default(), session),
        Err(merman_render::Error::Cancelled(_))
    ));

    let control = OperationControl::new();
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxModelItems, 1)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session_with_control(control.clone())
        .unwrap();
    let error = family::plan_render(&parsed, &session).err().unwrap();
    let merman_render::Error::ResourceLimitExceeded(first) = error else {
        panic!("expected model resource admission before Packet validation, got {error:?}");
    };
    assert_eq!(first.limit, ResourceLimitId::MaxModelItems.as_str());
    control.cancel();
    let error = family::prepare(&parsed, &LayoutOptions::default(), session)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        merman_render::Error::ResourceLimitExceeded(replayed) if replayed == first
    ));
}

fn packet_source() -> &'static str {
    r#"packet
title Packet Typography
0-7: "Header"
8-15: "Payload"
"#
}

fn packet_stylesheet<'input>(document: &'input roxmltree::Document<'input>) -> &'input str {
    document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Packet stylesheet")
}

#[test]
fn packet_default_and_dark_themes_reach_every_visible_css_role() {
    for (theme, text_color, block_fill) in
        [("default", "black", "#efefef"), ("dark", "#e0dfdf", "#333")]
    {
        for owner in ["source", "site"] {
            let config = serde_json::json!({"theme": theme});
            let (source, site_config) = if owner == "source" {
                (
                    format!("%%{{init: {config}}}%%\n{}", packet_source()),
                    serde_json::json!({"secure": []}),
                )
            } else {
                (packet_source().to_owned(), config)
            };
            let (_, svg) = render_packet_with_engine(
                &source,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site_config),
                ),
            );
            let document = roxmltree::Document::parse(&svg).expect("valid Packet theme SVG");
            let css = packet_stylesheet(&document);

            for (class, rule) in [
                (
                    "packetByte start",
                    format!(".packetByte.start{{fill:{text_color};}}"),
                ),
                (
                    "packetByte end",
                    format!(".packetByte.end{{fill:{text_color};}}"),
                ),
                (
                    "packetLabel",
                    format!(".packetLabel{{fill:{text_color};font-size:12px;}}"),
                ),
                (
                    "packetTitle",
                    format!(".packetTitle{{fill:{text_color};font-size:14px;}}"),
                ),
                (
                    "packetBlock",
                    format!(
                        ".packetBlock{{stroke:{text_color};stroke-width:1;fill:{block_fill};}}"
                    ),
                ),
            ] {
                assert!(
                    document
                        .descendants()
                        .any(|node| node.attribute("class") == Some(class)),
                    "{owner}/{theme}: missing visible {class} consumer"
                );
                assert!(
                    css.contains(&rule),
                    "{owner}/{theme}: expected {rule}: {css}"
                );
            }
            assert!(css.contains(".packetByte{font-size:10px;}"));
        }
    }
}

#[test]
fn packet_theme_variable_styles_preserve_root_layout_configuration() {
    for owner in ["source", "site"] {
        let config = serde_json::json!({
            "packet": {
                "showBits": false,
                "rowHeight": 40,
                "labelColor": "#010203",
                "blockFillColor": "#040506"
            },
            "themeVariables": { "packet": {
                "showBits": true,
                "rowHeight": 80,
                "byteFontSize": "11px",
                "startByteColor": "#111111",
                "endByteColor": "#222222",
                "labelColor": "#333333",
                "labelFontSize": "13px",
                "titleColor": "#444444",
                "titleFontSize": "15px",
                "blockStrokeColor": "#555555",
                "blockStrokeWidth": 2,
                "blockFillColor": "#666666"
            } }
        });
        let (source, site_config) = if owner == "source" {
            (
                format!("%%{{init: {config}}}%%\n{}", packet_source()),
                serde_json::json!({"secure": []}),
            )
        } else {
            (packet_source().to_owned(), config)
        };
        let (layout, svg) = render_packet_with_engine(
            &source,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(site_config),
            ),
        );
        assert!(
            !layout.show_bits,
            "{owner}: layout must still read root packet.showBits"
        );
        assert_eq!(layout.row_height, 40.0, "{owner}");
        let document = roxmltree::Document::parse(&svg).expect("valid configured Packet SVG");
        let css = packet_stylesheet(&document);
        // Init directives retain Mermaid's key allowlist; site config can supply every role.
        let (byte_size, start, end, block) = if owner == "source" {
            (
                "10px",
                "black",
                "black",
                "stroke:black;stroke-width:1;fill:#efefef;",
            )
        } else {
            (
                "11px",
                "#111111",
                "#222222",
                "stroke:#555555;stroke-width:2;fill:#666666;",
            )
        };
        for rule in [
            format!(".packetByte{{font-size:{byte_size};}}"),
            format!(".packetByte.start{{fill:{start};}}"),
            format!(".packetByte.end{{fill:{end};}}"),
            ".packetLabel{fill:#333333;font-size:13px;}".into(),
            ".packetTitle{fill:#444444;font-size:15px;}".into(),
            format!(".packetBlock{{{block}}}"),
        ] {
            assert!(css.contains(&rule), "{owner}: expected {rule}: {css}");
        }
        assert!(
            !document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|class| class.starts_with("packetByte "))
            }),
            "{owner}: root showBits=false must hide byte text"
        );
    }
}

const DESCENDING_PACKET: &str = include_str!("../../../fixtures/packet/bit_order_descending.mmd");

fn render_packet(source: &str) -> (PacketDiagramLayout, String) {
    render_packet_with_engine(source, MermaidEngine::new())
}

fn render_packet_with_engine(source: &str, engine: MermaidEngine) -> (PacketDiagramLayout, String) {
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse packet")
        .expect("packet diagram")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact =
        family::prepare(&parsed, &LayoutOptions::default(), session).expect("packet layout");
    let projection = artifact.layout_json().expect("layout projection");
    let layout = serde_json::from_value(projection["layout"]["PacketDiagram"].clone())
        .expect("packet layout");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("packet SVG")
        .svg()
        .to_owned();
    (layout, svg)
}

#[test]
fn packet_explicit_root_font_size_preserves_relative_roles_and_fixed_geometry() {
    let body = "packet\n0-7: \"Header\"\n";
    let (baseline, _) = render_packet(&format!(
        "---\nconfig:\n  packet:\n    showBits: false\n---\n{body}"
    ));
    for (root_size, expected_px) in [
        (serde_json::json!("32px"), "32"),
        (serde_json::json!("24pt"), "32"),
        (serde_json::json!("0px"), "0"),
        (serde_json::json!("0.5px"), "0.5"),
        (serde_json::json!("0.0000000001px"), "0.0000000001"),
    ] {
        let config = serde_json::json!({
            "packet": {"showBits": false},
            "themeVariables": {
                "fontFamily": "monospace",
                "fontSize": root_size,
                "packet": {"labelFontSize": "1em"}
            }
        });
        let (layout, svg) = render_packet(&format!("---\nconfig: {config}\n---\n{body}"));
        assert_eq!(
            serde_json::to_value(&layout).unwrap(),
            serde_json::to_value(&baseline).unwrap(),
            "root size {root_size} must not change Packet geometry",
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let css = packet_stylesheet(&document);
        for selector in ["#merman", "#merman svg"] {
            assert!(
                css.contains(&format!(
                    "{selector}{{font-family:monospace;font-size:{expected_px}px;"
                )),
                "{root_size}: {css}",
            );
        }
        assert!(css.contains(".packetLabel{fill:black;font-size:1em;}"));
        assert!(css.contains(".packetByte{font-size:10px;}"));
        assert!(css.contains(".packetTitle{fill:black;font-size:14px;}"));
    }
}

#[test]
fn packet_multiline_labels_reach_svg_as_literal_text() {
    let source = "packet-beta\n0: \"雪\n<node>&%%\"\n1: \"Tail\"\n";
    for source in [source.to_owned(), source.replace('\n', "\r\n")] {
        let (layout, svg) = render_packet(&source);
        assert_eq!(layout.words[0].blocks.len(), 2);
        let document = roxmltree::Document::parse(&svg).expect("valid literal Packet SVG");
        let labels = document
            .descendants()
            .filter(|node| node.attribute("class") == Some("packetLabel"))
            .map(|node| node.text().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(labels, ["雪\n<node>&%%", "Tail"]);
        assert!(!document.descendants().any(|node| node.has_tag_name("node")));
    }
}

#[test]
fn packet_intrinsic_svg_size_follows_effective_family_configuration() {
    use merman_core::presentation::SourcePresentationConfig;

    for (site_value, source_value, responsive) in [
        (None, None, true),
        (Some(false), None, false),
        (Some(true), Some(false), false),
        (Some(false), Some(true), true),
    ] {
        let engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
            serde_json::json!({"useMaxWidth": false, "packet": {"useMaxWidth": site_value}}),
        ));
        let prefix = source_value.map_or_else(String::new, |value| {
            format!("---\nconfig:\n  packet:\n    useMaxWidth: {value}\n---\n")
        });
        let (_, svg) = render_packet_with_engine(
            &format!("{prefix}packet-beta\n0-31: \"Header\"\n+8: \"Tail\"\n"),
            engine,
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let root = document.root_element();
        let view_box = root
            .attribute("viewBox")
            .unwrap()
            .split_ascii_whitespace()
            .map(|value| value.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        if responsive {
            assert_eq!(root.attribute("width"), Some("100%"));
            assert!(root.attribute("height").is_none());
            assert!(root.attribute("style").unwrap().contains("max-width:"));
        } else {
            assert_eq!(
                root.attribute("width").unwrap().parse::<f64>().unwrap(),
                view_box[2]
            );
            assert_eq!(
                root.attribute("height").unwrap().parse::<f64>().unwrap(),
                view_box[3]
            );
            assert!(root.attribute("style").is_none());
        }
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.attribute("class") == Some("packetBlock"))
                .count(),
            2
        );
    }
}

#[test]
fn packet_descending_mirrors_each_row_without_reversing_semantic_ranges() {
    let (ascending, _) = render_packet(&DESCENDING_PACKET.replace("descending", "ascending"));
    let (descending, svg) = render_packet(DESCENDING_PACKET);
    assert_eq!(descending.words.len(), 2);
    assert_eq!(
        (ascending.width, ascending.height),
        (descending.width, descending.height)
    );
    for (before, after) in ascending.words.iter().zip(&descending.words) {
        for (before, after) in before.blocks.iter().zip(&after.blocks) {
            assert_eq!(
                (before.start, before.end, &before.label),
                (after.start, after.end, &after.label)
            );
            assert_eq!(
                (before.width, before.height, before.y),
                (after.width, after.height, after.y)
            );
            let bits = after.end - after.start + 1.0;
            assert_eq!(after.x, 1.0 + (16.0 - after.start % 16.0 - bits) * 10.0);
        }
    }
    assert_eq!(
        descending.words[0]
            .blocks
            .iter()
            .map(|block| block.x)
            .collect::<Vec<_>>(),
        [81.0, 41.0, 31.0, 1.0]
    );
    assert_eq!(
        descending.words[1]
            .blocks
            .iter()
            .map(|block| block.x)
            .collect::<Vec<_>>(),
        [111.0, 81.0]
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let labels = |class| {
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some(class))
            .map(|node| node.text().unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        labels("packetByte start"),
        ["7", "11", "12", "15", "20", "23"]
    );
    assert_eq!(labels("packetByte end"), ["0", "8", "13", "16", "21"]);
    let single = document
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("packetByte start") && node.text() == Some("12")
        })
        .unwrap();
    assert_eq!(single.attribute("text-anchor"), Some("middle"));
    assert_eq!(single.attribute("x"), Some("35"));
    assert_eq!(
        labels("packetLabel"),
        ["DATA", "TYPE", "EN", "FLAGS", "FLAGS", "TAIL"]
    );
}

#[test]
fn packet_descending_applies_when_bit_numbers_are_hidden() {
    let (visible, _) = render_packet(DESCENDING_PACKET);
    let (hidden, svg) =
        render_packet(&DESCENDING_PACKET.replace("showBits: true", "showBits: false"));
    assert!(!hidden.show_bits);
    for (visible, hidden) in visible.words.iter().zip(&hidden.words) {
        for (visible, hidden) in visible.blocks.iter().zip(&hidden.blocks) {
            assert_eq!((visible.x, visible.width), (hidden.x, hidden.width));
        }
    }
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert!(!document.descendants().any(|node| {
        node.attribute("class")
            .is_some_and(|class| class.starts_with("packetByte "))
    }));
}

#[test]
fn packet_default_order_is_ascending_and_one_bit_rows_keep_absolute_numbers() {
    let (ascending, explicit) =
        render_packet(&DESCENDING_PACKET.replace("descending", "ascending"));
    let (default, implicit) =
        render_packet(&DESCENDING_PACKET.replace("    bitOrder: descending", ""));
    assert_eq!(
        serde_json::to_value(&ascending).unwrap(),
        serde_json::to_value(&default).unwrap()
    );
    assert_eq!(explicit, implicit);

    let (layout, svg) = render_packet(
        "---\nconfig:\n  packet:\n    bitsPerRow: 1\n    bitOrder: descending\n---\npacket\n+3: \"bits\"\n",
    );
    assert_eq!(layout.words.len(), 3);
    assert!(layout.words.iter().all(|word| word.blocks[0].x == 1.0));
    let document = roxmltree::Document::parse(&svg).unwrap();
    let leading: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("packetByte start"))
        .map(|node| node.text().unwrap())
        .collect();
    assert_eq!(leading, ["0", "1", "2"]);
    assert!(
        !document
            .descendants()
            .any(|node| node.attribute("class") == Some("packetByte end"))
    );
}

#[allow(dead_code)]
mod common;
