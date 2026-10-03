#![cfg(feature = "diagram-packet")]

use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::PacketDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

const DESCENDING_PACKET: &str = include_str!("../../../fixtures/packet/bit_order_descending.mmd");

fn render_packet(source: &str) -> (PacketDiagramLayout, String) {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse packet")
        .expect("packet diagram");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact =
        family::prepare(parsed, &LayoutOptions::default(), session).expect("packet layout");
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
            let bits = after.end - after.start + 1;
            assert_eq!(after.x, 1.0 + (16 - after.start % 16 - bits) as f64 * 10.0);
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
