use super::*;

#[test]
fn packet_render_model_renders_rows_and_ranges() {
    let mut model = PacketDiagramRenderModel::default();
    model.title = Some("Packet".to_string());
    model.acc_title = Some("Packet title".to_string());
    model.acc_descr = Some("Packet description".to_string());
    model.packet = vec![
        vec![
            PacketRenderBlock {
                start: 0,
                end: 7,
                bits: 8,
                label: "header".to_string(),
            },
            PacketRenderBlock {
                start: 8,
                end: 15,
                bits: 8,
                label: "payload".to_string(),
            },
        ],
        vec![PacketRenderBlock {
            start: 16,
            end: 31,
            bits: 16,
            label: "footer".to_string(),
        }],
    ];

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
fn packet_parser_split_blocks_render_inclusive_bit_counts() {
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
            "  - range=[11..31] bits=21 label(bytes=8)=\"multiple\"\n",
            "row 2:\n",
            "  - range=[32..63] bits=32 label(bytes=8)=\"multiple\"\n",
            "row 3:\n",
            "  - range=[64..90] bits=27 label(bytes=8)=\"multiple\"",
        )
    );
}

#[test]
fn packet_render_model_rejects_noninclusive_bit_counts() {
    let mut model = PacketDiagramRenderModel::default();
    model.packet = vec![vec![PacketRenderBlock {
        start: 11,
        end: 31,
        bits: 20,
        label: "multiple".to_string(),
    }]];

    let error = render_model(
        &RenderSemanticModel::Packet(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("packet range width must be validated before rendering");
    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "packet",
            feature: "packet block bit count does not match inclusive range",
        }
    );
}

#[test]
fn packet_labels_cannot_forge_following_block_boundaries() {
    let mut forged = PacketDiagramRenderModel::default();
    forged.packet = vec![vec![PacketRenderBlock {
        start: 0,
        end: 7,
        bits: 8,
        label: "header (8 bits) | [8..15] payload".to_string(),
    }]];
    let mut split = PacketDiagramRenderModel::default();
    split.packet = vec![vec![
        PacketRenderBlock {
            start: 0,
            end: 7,
            bits: 8,
            label: "header".to_string(),
        },
        PacketRenderBlock {
            start: 8,
            end: 15,
            bits: 8,
            label: "payload".to_string(),
        },
    ]];

    assert_ne!(
        render(RenderSemanticModel::Packet(forged)),
        render(RenderSemanticModel::Packet(split)),
        "length-framed packet labels must distinguish one authored block from two blocks"
    );

    let mut leading = PacketDiagramRenderModel::default();
    leading.packet = vec![vec![PacketRenderBlock {
        start: 0,
        end: 7,
        bits: 8,
        label: " label".to_string(),
    }]];
    let mut trailing = PacketDiagramRenderModel::default();
    trailing.packet = vec![vec![PacketRenderBlock {
        start: 0,
        end: 7,
        bits: 8,
        label: "label ".to_string(),
    }]];
    assert_ne!(
        render(RenderSemanticModel::Packet(leading)),
        render(RenderSemanticModel::Packet(trailing)),
        "equal-length whitespace variants must remain distinguishable after wrapping"
    );
}
