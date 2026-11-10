#![cfg(feature = "diagram-packet")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::PacketDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn source(width: &str, order: &str, body: &str) -> String {
    format!(
        "---\nconfig:\n  packet:\n    bitsPerRow: {width}\n    bitOrder: {order}\n    bitWidth: 1\n    paddingX: 0\n---\npacket\n{body}\n"
    )
}

fn render(source: &str) -> (PacketDiagramLayout, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let projection = artifact.layout_json().unwrap();
    let layout = serde_json::from_value(projection["layout"]["PacketDiagram"].clone()).unwrap();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned();
    (layout, svg)
}

#[test]
fn packet_fractional_row_splits_keep_remainder_drift_and_positive_short_extents() {
    let (layout, svg) = render(&source("1.2", "ascending", "0-5: \"x\""));
    let fourth = &layout.words[3].blocks[0];
    assert_eq!(fourth.start, 3.5999999999999996);
    assert_eq!(fourth.end, 3.8);
    assert_eq!(fourth.x, 2.1999999999999997);
    assert_eq!(fourth.width, 1.2000000000000002);
    assert!(svg.contains("x=\"2.1999999999999997\""));
    assert!(svg.contains("width=\"1.2000000000000002\""));

    let (layout, svg) = render(&source("1.5", "ascending", "0: \"a\"\n1-5: \"b\""));
    let short = &layout.words[0].blocks[1];
    assert_eq!(
        (short.start, short.end, short.x, short.width),
        (1.0, 0.5, 2.0, 0.5)
    );
    assert!(svg.contains("width=\"0.5\""));
}

#[test]
fn packet_svg_uses_rounded_ranges_for_geometry_and_bit_labels_in_both_orders() {
    for order in ["ascending", "descending"] {
        let (layout, svg) = render(&source(
            "9007199254740994",
            order,
            "0-9007199254740993: \"x\"",
        ));
        assert_eq!(layout.bits_per_row, 9_007_199_254_740_994.0);
        assert_eq!(layout.words.len(), 1);
        assert_eq!(layout.words[0].blocks.len(), 1);
        let block = &layout.words[0].blocks[0];
        assert_eq!(block.end, 9_007_199_254_740_992.0);
        assert_eq!(block.width, 9_007_199_254_740_992.0);
        assert_eq!(block.x, if order == "ascending" { 1.0 } else { 3.0 });

        let document = roxmltree::Document::parse(&svg).unwrap();
        let labels: Vec<_> = document
            .descendants()
            .filter(|node| {
                node.attribute("class")
                    .is_some_and(|class| class.starts_with("packetByte "))
            })
            .filter_map(|node| node.text())
            .collect();
        assert_eq!(
            labels,
            if order == "ascending" {
                ["0", "9007199254740992"]
            } else {
                ["9007199254740992", "0"]
            }
        );
    }
}

#[test]
fn packet_svg_formats_large_bit_numbers_as_javascript_text() {
    for (width, bits, label) in [
        (
            "36893488147419103232",
            "18446744073709551616",
            "18446744073709552000",
        ),
        ("2000000000000000000000", "1000000000000000000000", "1e+21"),
    ] {
        let (_, svg) = render(&source(width, "ascending", &format!("+{bits}: \"x\"")));
        let document = roxmltree::Document::parse(&svg).unwrap();
        let end = document
            .descendants()
            .find(|node| node.attribute("class") == Some("packetByte end"))
            .unwrap();
        assert_eq!(end.text(), Some(label));
    }
}

#[test]
fn packet_svg_geometry_uses_the_inclusive_range_without_rewriting_db_width() {
    let (layout, _) = render(&source(
        "9007199254740996",
        "descending",
        "+9007199254740994: \"x\"",
    ));
    let block = &layout.words[0].blocks[0];
    assert_eq!(block.width, 9_007_199_254_740_992.0);
    assert_eq!(block.x, 5.0);

    let (layout, _) = render(&source("2.5", "descending", "0-3: \"x\""));
    assert_eq!(layout.words.len(), 2);
    assert_eq!(layout.words[0].blocks[0].width, 2.5);
    assert_eq!(layout.words[0].blocks[0].x, 1.0);
    assert_eq!(layout.words[1].blocks[0].width, 1.5);
    assert_eq!(layout.words[1].blocks[0].x, 2.0);
}

#[test]
fn packet_nonfinite_source_is_accepted_before_svg_geometry_admission() {
    let source = source(
        "1e308",
        "ascending",
        &format!("0-{}: \"x\"", "9".repeat(400)),
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let error = family::prepare(&parsed, &LayoutOptions::default(), session)
        .err()
        .unwrap();
    assert!(
        matches!(error, merman_render::Error::InvalidModel { ref message }
        if message.contains("Packet geometry requires finite"))
    );
}
