use merman_ascii::{AsciiColorMode, AsciiRenderOptions, TerminalWidthProfile, render_model};
use merman_core::diagram::RenderSemanticModel;
use merman_core::diagrams::git_graph::{GitGraphBranchRenderModel, GitGraphRenderModel};
use merman_core::{Engine, ParseOptions};
use unicode_width::UnicodeWidthStr;

fn parse_model(source: &str) -> RenderSemanticModel {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("diagram should parse")
        .expect("diagram should be detected");
    parsed.into_parts().1
}

fn render(source: &str, options: &AsciiRenderOptions) -> String {
    render_model(&parse_model(source), options).expect("diagram should render")
}

fn git_graph_with_authored_text(text: &str) -> RenderSemanticModel {
    RenderSemanticModel::GitGraph(GitGraphRenderModel {
        diagram_type: "gitGraph".to_string(),
        commits: Vec::new(),
        branches: vec![GitGraphBranchRenderModel {
            name: text.to_string(),
        }],
        current_branch: text.to_string(),
        direction: "LR".to_string(),
        title: Some(text.to_string()),
        acc_title: None,
        acc_descr: None,
        warning_facts: Vec::new(),
    })
}

const CURRENT_ASCII_FAMILY_SOURCES: [(&str, &str); 14] = [
    ("class", "classDiagram\nclass A"),
    ("er", "erDiagram\nA {\n  string name\n}"),
    ("flowchart", "flowchart LR\nA[Basic]"),
    (
        "gantt",
        "gantt\ntitle Basic\ndateFormat YYYY-MM-DD\nsection Work\nTask :task, 2024-01-01, 1d",
    ),
    ("gitGraph", "gitGraph\ncommit"),
    (
        "journey",
        "journey\ntitle Basic\nsection Work\nTask: 5: Alice",
    ),
    ("kanban", "kanban\n  todo[Basic]"),
    ("mindmap", "mindmap\n  root((Basic))"),
    ("packet", "packet\ntitle Basic\n0-7: \"field\""),
    ("sequence", "sequenceDiagram\ntitle Basic\nparticipant A"),
    ("state", "stateDiagram-v2\nstate \"Basic\" as A"),
    ("timeline", "timeline\ntitle Basic\n2024 : Event"),
    (
        "xychart",
        "xychart\ntitle \"Basic\"\nx-axis [A]\ny-axis 0 --> 1\nbar [1]",
    ),
    ("treeView", "treeView-beta\nroot/\n  file.txt"),
];

fn model_with_visible_text(family: &str, source: &str, text: &str) -> RenderSemanticModel {
    let mut model = parse_model(source);
    match &mut model {
        RenderSemanticModel::Class(model) => {
            let class = model
                .classes
                .values_mut()
                .next()
                .expect("class fixture node");
            class.label = text.to_string();
            class.text = text.to_string();
        }
        RenderSemanticModel::Er(model) => {
            let entity = model
                .entities
                .values_mut()
                .next()
                .expect("ER fixture entity");
            entity.label = text.to_string();
            entity.alias.clear();
        }
        RenderSemanticModel::Flowchart(model) => {
            model
                .nodes
                .first_mut()
                .expect("flowchart fixture node")
                .label = Some(text.to_string());
        }
        RenderSemanticModel::Gantt(model) => model.title = Some(text.to_string()),
        RenderSemanticModel::GitGraph(model) => model.title = Some(text.to_string()),
        RenderSemanticModel::Journey(model) => model.title = Some(text.to_string()),
        RenderSemanticModel::Kanban(model) => {
            model.nodes.first_mut().expect("Kanban fixture node").label = text.to_string();
        }
        RenderSemanticModel::Mindmap(model) => {
            model.nodes.first_mut().expect("mindmap fixture node").label = text.to_string();
        }
        RenderSemanticModel::Packet(model) => model.title = Some(text.to_string()),
        RenderSemanticModel::Sequence(model) => model.title = Some(text.to_string()),
        RenderSemanticModel::State(model) => {
            let node = model.nodes.first_mut().expect("state fixture node");
            node.label = None;
            node.description = Some(vec![text.to_string()]);
        }
        RenderSemanticModel::Timeline(model) => model.title = Some(text.to_string()),
        RenderSemanticModel::XyChart(model) => model.title = Some(text.to_string()),
        RenderSemanticModel::TreeView(model) => model.title = Some(text.to_string()),
        other => panic!("{family} fixture parsed as unexpected model {other:?}"),
    }
    model
}

fn assert_only_renderer_owned_sgr(output: &str, family: &str, mode: AsciiColorMode) {
    let bytes = output.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0x1b {
            index += 1;
            continue;
        }

        assert_ne!(
            mode,
            AsciiColorMode::Plain,
            "raw ESC in {family}: {output:?}"
        );
        assert_ne!(
            mode,
            AsciiColorMode::Html,
            "raw ESC in {family}: {output:?}"
        );
        assert_eq!(
            bytes.get(index + 1),
            Some(&b'['),
            "non-SGR ESC in {family}: {output:?}"
        );
        index += 2;
        while bytes
            .get(index)
            .is_some_and(|byte| byte.is_ascii_digit() || *byte == b';')
        {
            index += 1;
        }
        assert_eq!(
            bytes.get(index),
            Some(&b'm'),
            "unterminated SGR in {family}: {output:?}"
        );
        index += 1;
    }
}

#[test]
fn flowchart_preserves_complete_authored_grapheme_clusters() {
    let source = "flowchart LR\nA[\"Cafe\u{301} \u{1f469}\u{200d}\u{1f4bb} \u{1f1fa}\u{1f1f8}\"]";

    let rendered = render(source, &AsciiRenderOptions::unicode());

    assert!(
        rendered.contains("Cafe\u{301} \u{1f469}\u{200d}\u{1f4bb} \u{1f1fa}\u{1f1f8}"),
        "{rendered}"
    );
}

#[test]
fn flowchart_layout_uses_the_selected_ambiguous_width_profile() {
    let source = "flowchart LR\nA[\"A·B\"]";
    let unicode = render(
        source,
        &AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Unicode),
    );
    let cjk = render(
        source,
        &AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk),
    );

    let unicode_border = unicode
        .lines()
        .find(|line| line.contains('┌'))
        .expect("Unicode output should contain a node border");
    let cjk_border = cjk
        .lines()
        .find(|line| line.contains('+'))
        .expect("CJK output should use a single-cell ASCII node border");

    assert_eq!(
        UnicodeWidthStr::width_cjk(cjk_border),
        UnicodeWidthStr::width(unicode_border) + 1,
        "Unicode:\n{unicode}\nCJK:\n{cjk}"
    );
}

#[test]
fn structured_text_visibly_escapes_terminal_and_bidi_controls() {
    let authored = "before\u{1b}]8;;https://example.invalid\u{7}link\u{9b}after\u{202e}";
    let model = git_graph_with_authored_text(authored);

    let rendered = render_model(&model, &AsciiRenderOptions::ascii()).expect("git graph renders");

    for control in ['\u{1b}', '\u{7}', '\u{9b}', '\u{202e}'] {
        assert!(
            !rendered.contains(control),
            "raw control {control:?} leaked: {rendered:?}"
        );
    }
    assert!(rendered.contains(r"\u{1B}"), "{rendered:?}");
    assert!(rendered.contains(r"\u{7}"), "{rendered:?}");
    assert!(rendered.contains(r"\u{9B}"), "{rendered:?}");
    assert!(rendered.contains(r"\u{202E}"), "{rendered:?}");
}

#[test]
fn structured_text_html_escapes_authored_markup_after_normalization() {
    let model = git_graph_with_authored_text("<node & title>\u{1b}");
    let options = AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::Html);

    let rendered = render_model(&model, &options).expect("git graph renders");

    assert!(!rendered.contains("<node & title>"), "{rendered:?}");
    assert!(
        rendered.contains("&lt;node &amp; title&gt;"),
        "{rendered:?}"
    );
    assert!(rendered.contains(r"\u{1B}"), "{rendered:?}");
    assert!(!rendered.contains('\u{1b}'), "{rendered:?}");
}

#[test]
fn every_current_family_preserves_complete_graphemes_in_both_width_profiles() {
    let authored = "Cafe\u{301} 👩‍💻 🇺🇸";

    for (family, source) in CURRENT_ASCII_FAMILY_SOURCES {
        let model = model_with_visible_text(family, source, authored);
        for charset_options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
                let options = charset_options.with_terminal_width_profile(profile);
                let rendered = render_model(&model, &options)
                    .unwrap_or_else(|error| panic!("{family}/{profile:?} failed: {error}"));

                assert!(
                    rendered.contains(authored),
                    "{family}/{profile:?} split or dropped a grapheme:\n{rendered}"
                );
            }
        }
    }
}

#[test]
fn every_current_family_normalizes_authored_controls_in_every_encoder() {
    let authored = "before\u{1b}]8;;https://example.invalid\u{7}link\u{9b}after\u{202e}";
    let modes = [
        AsciiColorMode::Plain,
        AsciiColorMode::Ansi16,
        AsciiColorMode::Ansi256,
        AsciiColorMode::TrueColor,
        AsciiColorMode::Html,
    ];

    for (family, source) in CURRENT_ASCII_FAMILY_SOURCES {
        let model = model_with_visible_text(family, source, authored);
        for charset_options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for mode in modes {
                let options = charset_options.with_color_mode(mode);
                let rendered = render_model(&model, &options)
                    .unwrap_or_else(|error| panic!("{family}/{mode:?} failed: {error}"));

                assert_only_renderer_owned_sgr(&rendered, family, mode);
                for control in ['\u{7}', '\u{9b}', '\u{202e}'] {
                    assert!(
                        !rendered.contains(control),
                        "raw control {control:?} leaked from {family}/{mode:?}: {rendered:?}"
                    );
                }
                for visible in [r"\u{1B}", r"\u{7}", r"\u{9B}", r"\u{202E}"] {
                    assert!(
                        rendered.contains(visible),
                        "missing {visible} in {family}/{mode:?}: {rendered:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn graph_and_relation_edge_labels_normalize_before_empty_checks() {
    let authored = "\tendpoint\r";
    let cases = [
        ("flowchart", "flowchart LR\nA -->|label| B"),
        ("state", "stateDiagram-v2\nA --> B: label"),
        ("class", "classDiagram\nA \"1\" --> \"many\" B : label"),
    ];

    for (family, source) in cases {
        let mut model = parse_model(source);
        match &mut model {
            RenderSemanticModel::Flowchart(model) => {
                model.edges.first_mut().expect("flowchart edge").label = Some(authored.to_string());
            }
            RenderSemanticModel::State(model) => {
                model.edges.first_mut().expect("state edge").label = authored.to_string();
            }
            RenderSemanticModel::Class(model) => {
                model
                    .relations
                    .first_mut()
                    .expect("class relation")
                    .relation_title_1 = authored.to_string();
            }
            other => panic!("{family} fixture parsed as unexpected model {other:?}"),
        }

        let rendered = render_model(&model, &AsciiRenderOptions::unicode())
            .unwrap_or_else(|error| panic!("{family} failed: {error}"));
        assert!(
            rendered.contains(r"\u{9}endpoint\u{D}"),
            "{family} normalized after trimming authored controls:\n{rendered}"
        );
    }
}
