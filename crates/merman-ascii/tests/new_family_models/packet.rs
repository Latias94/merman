use super::*;

#[test]
fn packet_render_model_renders_rows_and_ranges() {
    let model = PacketDiagramModel {
        title: Some("Packet".to_string()),
        acc_title: Some("Packet title".to_string()),
        acc_descr: Some("Packet description".to_string()),
        packet: vec![
            vec![
                PacketBlock {
                    start: 0.0,
                    end: 7.0,
                    bits: 8.0,
                    label: "header".to_string(),
                },
                PacketBlock {
                    start: 8.0,
                    end: 15.0,
                    bits: 8.0,
                    label: "payload".to_string(),
                },
            ],
            vec![PacketBlock {
                start: 16.0,
                end: 31.0,
                bits: 16.0,
                label: "footer".to_string(),
            }],
        ],
    };

    let rendered = render(RenderSemanticModel::Packet(model));

    assert_eq!(
        rendered,
        concat!(
            "title(bytes=6)=\"Packet\"\n",
            "accTitle(bytes=12)=\"Packet title\"\n",
            "accDescr(bytes=18)=\"Packet description\"\n",
            "row 1:\n",
            "  - range=[0..7] bits=8 label(bytes=6)=\"header\"\n",
            "  - range=[8..15] bits=8 label(bytes=7)=\"payload\"\n",
            "row 2:\n",
            "  - range=[16..31] bits=16 label(bytes=6)=\"footer\"",
        )
    );
}
#[test]
fn packet_parser_split_blocks_render_the_database_bit_counts() {
    let rendered = render_parsed(
        r#"packet
0-10: "test"
11-90: "multiple"
"#,
    );

    assert_eq!(
        rendered,
        concat!(
            "row 1:\n",
            "  - range=[0..10] bits=11 label(bytes=4)=\"test\"\n",
            "  - range=[11..31] bits=20 label(bytes=8)=\"multiple\"\n",
            "row 2:\n",
            "  - range=[32..63] bits=31 label(bytes=8)=\"multiple\"\n",
            "row 3:\n",
            "  - range=[64..90] bits=26 label(bytes=8)=\"multiple\"",
        )
    );
}

#[test]
fn packet_labels_cannot_forge_following_block_boundaries() {
    let forged = PacketDiagramModel {
        packet: vec![vec![PacketBlock {
            start: 0.0,
            end: 7.0,
            bits: 8.0,
            label: "header (8 bits) | [8..15] payload".to_string(),
        }]],
        ..Default::default()
    };
    let split = PacketDiagramModel {
        packet: vec![vec![
            PacketBlock {
                start: 0.0,
                end: 7.0,
                bits: 8.0,
                label: "header".to_string(),
            },
            PacketBlock {
                start: 8.0,
                end: 15.0,
                bits: 8.0,
                label: "payload".to_string(),
            },
        ]],
        ..Default::default()
    };

    assert_ne!(
        render(RenderSemanticModel::Packet(forged)),
        render(RenderSemanticModel::Packet(split)),
        "length-framed packet labels must distinguish one authored block from two blocks"
    );

    let leading = PacketDiagramModel {
        packet: vec![vec![PacketBlock {
            start: 0.0,
            end: 7.0,
            bits: 8.0,
            label: " label".to_string(),
        }]],
        ..Default::default()
    };
    let trailing = PacketDiagramModel {
        packet: vec![vec![PacketBlock {
            start: 0.0,
            end: 7.0,
            bits: 8.0,
            label: "label ".to_string(),
        }]],
        ..Default::default()
    };
    assert_ne!(
        render(RenderSemanticModel::Packet(leading)),
        render(RenderSemanticModel::Packet(trailing)),
        "equal-length whitespace variants must remain distinguishable after wrapping"
    );
}
