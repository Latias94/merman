use merman::diagram_theme::{
    DiagramFamilyId, DiagramThemeCompiler, DiagramThemeSpecWireV1, SpecifiedWireV1,
    ThemeCanvasPaintWireV1, ThemeColorTokenV1, ThemeDefinitionBuilderV1,
    ThemeDefinitionCompileError, ThemeDefinitionV1, ThemeRuleBuilderV1, ThemeRuleFacetV1,
    ThemeRuleSetWireV1, ThemeStrokePatchWireV1, ThemeStylePatchWireV1, ThemeSupportOutputV1,
    ThemeSupportQueryV1, ThemeSupportStateV1, ThemeTarget, ThemeTokensV1, ThemeVariant,
    compile_theme_definition, compile_theme_definition_json, describe_theme_support,
    materialize_theme,
};
use merman::svg::{ThemeResourceLimitId, ThemeResourcePolicy};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer};

struct AuthoringRenderWitness {
    name: &'static str,
    diagram_id: &'static str,
    source: &'static str,
}

const AUTHORING_RENDER_WITNESSES: [AuthoringRenderWitness; 3] = [
    AuthoringRenderWitness {
        name: "Flowchart",
        diagram_id: "authoring-flowchart",
        source: "flowchart LR\nA[Alpha] --> B[Beta]\n",
    },
    AuthoringRenderWitness {
        name: "State",
        diagram_id: "authoring-state",
        source: "stateDiagram-v2\n[*] --> Active\nActive --> [*]\n",
    },
    AuthoringRenderWitness {
        name: "Sequence",
        diagram_id: "authoring-sequence",
        source: "sequenceDiagram\nAlice->>Bob: Hello\nBob-->>Alice: World\n",
    },
];

const TERMINAL_FLOWCHART_WITNESS: AuthoringRenderWitness = AuthoringRenderWitness {
    name: "Flowchart expansion rows",
    diagram_id: "authoring-expansion-flowchart",
    source: "flowchart LR\nsubgraph Group\nA[Alpha]\nend\nA --> B[Beta]\n",
};

const TERMINAL_SEQUENCE_WITNESS: AuthoringRenderWitness = AuthoringRenderWitness {
    name: "Sequence expansion rows",
    diagram_id: "authoring-expansion-sequence",
    source: "sequenceDiagram\nparticipant Alice\nparticipant Bob\nactivate Bob\nAlice->>Bob: Hello\nNote right of Bob: Sequence note\ndeactivate Bob\n",
};

const TERMINAL_STATE_WITNESS: AuthoringRenderWitness = AuthoringRenderWitness {
    name: "State expansion rows",
    diagram_id: "authoring-expansion-state",
    source: "---\ntitle: State theme witness\n---\nstateDiagram-v2\n[*] --> Idle: start\nstate Parent {\n  Idle\n}\nstate Decide <<choice>>\nIdle --> Decide: choose\nDecide --> [*]: finish\nnote right of Idle\n  State note\nend note\n",
};

const TERMINAL_ER_WITNESS: AuthoringRenderWitness = AuthoringRenderWitness {
    name: "ER expansion rows",
    diagram_id: "authoring-expansion-er",
    source: "erDiagram\nCUSTOMER {\n  int id\n}\nCUSTOMER ||--o{ ORDER : places\n",
};

const TERMINAL_PIE_WITNESS: AuthoringRenderWitness = AuthoringRenderWitness {
    name: "Pie expansion palette",
    diagram_id: "authoring-expansion-pie",
    source: "pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n",
};

const TERMINAL_SERIES: [&str; 2] = ["#12ab34", "#3456de"];
const TERMINAL_FIRST_SERIES_RGB: &str = "rgb(18, 171, 52)";

#[derive(Clone, Copy)]
struct GeneratedRuleTerminalWitness {
    target: ThemeTarget,
    variant: Option<ThemeVariant>,
    fill: Option<ThemeColorTokenV1>,
    stroke: Option<ThemeColorTokenV1>,
    diagram: &'static AuthoringRenderWitness,
}

const fn generated_rule_terminal_witness(
    target: ThemeTarget,
    variant: Option<ThemeVariant>,
    fill: Option<ThemeColorTokenV1>,
    stroke: Option<ThemeColorTokenV1>,
    diagram: &'static AuthoringRenderWitness,
) -> GeneratedRuleTerminalWitness {
    GeneratedRuleTerminalWitness {
        target,
        variant,
        fill,
        stroke,
        diagram,
    }
}

const GENERATED_RULE_TERMINAL_WITNESSES: [GeneratedRuleTerminalWitness; 23] = [
    generated_rule_terminal_witness(
        ThemeTarget::Text,
        None,
        Some(ThemeColorTokenV1::Text),
        None,
        &TERMINAL_STATE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::Title,
        None,
        Some(ThemeColorTokenV1::Text),
        None,
        &TERMINAL_STATE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::Node,
        None,
        None,
        Some(ThemeColorTokenV1::Border),
        &TERMINAL_FLOWCHART_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::Edge,
        None,
        None,
        Some(ThemeColorTokenV1::Line),
        &TERMINAL_FLOWCHART_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::Cluster,
        None,
        Some(ThemeColorTokenV1::SurfaceMuted),
        Some(ThemeColorTokenV1::Border),
        &TERMINAL_FLOWCHART_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::Actor,
        None,
        Some(ThemeColorTokenV1::Surface),
        Some(ThemeColorTokenV1::Border),
        &TERMINAL_SEQUENCE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::Lifeline,
        None,
        None,
        Some(ThemeColorTokenV1::Line),
        &TERMINAL_SEQUENCE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::Message,
        None,
        None,
        Some(ThemeColorTokenV1::Line),
        &TERMINAL_SEQUENCE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::State,
        None,
        Some(ThemeColorTokenV1::Surface),
        Some(ThemeColorTokenV1::Border),
        &TERMINAL_STATE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::StateLabel,
        None,
        Some(ThemeColorTokenV1::Text),
        None,
        &TERMINAL_STATE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::Transition,
        None,
        None,
        Some(ThemeColorTokenV1::Line),
        &TERMINAL_STATE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::TransitionMarker,
        None,
        Some(ThemeColorTokenV1::Line),
        Some(ThemeColorTokenV1::Line),
        &TERMINAL_STATE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::TransitionLabel,
        None,
        Some(ThemeColorTokenV1::Text),
        None,
        &TERMINAL_STATE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::TransitionLabelBackground,
        None,
        Some(ThemeColorTokenV1::Canvas),
        None,
        &TERMINAL_STATE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::Composite,
        None,
        Some(ThemeColorTokenV1::Canvas),
        Some(ThemeColorTokenV1::Border),
        &TERMINAL_STATE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::CompositeHeader,
        None,
        Some(ThemeColorTokenV1::SurfaceAlt),
        Some(ThemeColorTokenV1::Border),
        &TERMINAL_STATE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::CompositeLabel,
        None,
        Some(ThemeColorTokenV1::Text),
        None,
        &TERMINAL_STATE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::SpecialState,
        Some(ThemeVariant::Special),
        Some(ThemeColorTokenV1::Accent),
        Some(ThemeColorTokenV1::Accent),
        &TERMINAL_STATE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::SpecialStateInner,
        Some(ThemeVariant::End),
        Some(ThemeColorTokenV1::Canvas),
        Some(ThemeColorTokenV1::Canvas),
        &TERMINAL_STATE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::Note,
        None,
        Some(ThemeColorTokenV1::SurfaceAlt),
        Some(ThemeColorTokenV1::Border),
        &TERMINAL_SEQUENCE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::NoteLabel,
        None,
        Some(ThemeColorTokenV1::Text),
        None,
        &TERMINAL_STATE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::Activation,
        None,
        Some(ThemeColorTokenV1::SurfaceAlt),
        Some(ThemeColorTokenV1::Border),
        &TERMINAL_SEQUENCE_WITNESS,
    ),
    generated_rule_terminal_witness(
        ThemeTarget::Entity,
        None,
        Some(ThemeColorTokenV1::Surface),
        Some(ThemeColorTokenV1::Border),
        &TERMINAL_ER_WITNESS,
    ),
];

#[derive(Clone, Copy)]
struct GeneratedPaletteTerminalWitness {
    target: ThemeTarget,
    diagram: &'static AuthoringRenderWitness,
}

const GENERATED_PALETTE_TERMINAL_WITNESSES: [GeneratedPaletteTerminalWitness; 2] = [
    GeneratedPaletteTerminalWitness {
        target: ThemeTarget::Node,
        diagram: &TERMINAL_FLOWCHART_WITNESS,
    },
    GeneratedPaletteTerminalWitness {
        target: ThemeTarget::PieSlice,
        diagram: &TERMINAL_PIE_WITNESS,
    },
];

fn light_authoring_witness() -> ThemeDefinitionV1 {
    ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_canvas("#fffdf5")
            .with_surface("#fef3c7")
            .with_surface_alt("#fde68a")
            .with_surface_muted("#fef9c3")
            .with_text("#3f2d20")
            .with_border("#b45309")
            .with_line("#92400e")
            .with_accent("#c2410c")
            .with_series(vec![
                "#f59e0b".to_owned(),
                "#84cc16".to_owned(),
                "#06b6d4".to_owned(),
            ]),
    )
}

fn dark_authoring_witness() -> ThemeDefinitionV1 {
    ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_canvas("#07111f")
            .with_surface("#172554")
            .with_surface_alt("#1e3a8a")
            .with_surface_muted("#0f172a")
            .with_text("#e0f2fe")
            .with_border("#38bdf8")
            .with_line("#7dd3fc")
            .with_accent("#f472b6")
            .with_series(vec![
                "#22d3ee".to_owned(),
                "#a78bfa".to_owned(),
                "#fb7185".to_owned(),
            ]),
    )
}

fn terminal_color(token: ThemeColorTokenV1) -> &'static str {
    match token {
        ThemeColorTokenV1::Canvas => "#110011",
        ThemeColorTokenV1::Surface => "#220022",
        ThemeColorTokenV1::SurfaceAlt => "#330033",
        ThemeColorTokenV1::SurfaceMuted => "#440044",
        ThemeColorTokenV1::Text => "#550055",
        ThemeColorTokenV1::Border => "#660066",
        ThemeColorTokenV1::Line => "#770077",
        ThemeColorTokenV1::Accent => "#880088",
    }
}

fn terminal_witness_definition() -> ThemeDefinitionV1 {
    ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_canvas(terminal_color(ThemeColorTokenV1::Canvas))
            .with_surface(terminal_color(ThemeColorTokenV1::Surface))
            .with_surface_alt(terminal_color(ThemeColorTokenV1::SurfaceAlt))
            .with_surface_muted(terminal_color(ThemeColorTokenV1::SurfaceMuted))
            .with_text(terminal_color(ThemeColorTokenV1::Text))
            .with_border(terminal_color(ThemeColorTokenV1::Border))
            .with_line(terminal_color(ThemeColorTokenV1::Line))
            .with_accent(terminal_color(ThemeColorTokenV1::Accent))
            .with_series(TERMINAL_SERIES.map(str::to_owned).to_vec()),
    )
}

fn expected_generated_style(witness: GeneratedRuleTerminalWitness) -> ThemeStylePatchWireV1 {
    let mut style = ThemeStylePatchWireV1::default();
    if let Some(token) = witness.fill {
        style.fill = SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color(
            terminal_color(token).to_owned(),
        ));
    }
    if let Some(token) = witness.stroke {
        style.stroke = Some(ThemeStrokePatchWireV1 {
            paint: SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color(
                terminal_color(token).to_owned(),
            )),
            ..ThemeStrokePatchWireV1::default()
        });
    }
    style
}

fn materialize_pretty_json_round_trip(
    definition: &ThemeDefinitionV1,
    witness_name: &str,
) -> merman::diagram_theme::MaterializedThemeWireV1 {
    let pretty_json = serde_json::to_string_pretty(definition)
        .unwrap_or_else(|error| panic!("{witness_name} definition should serialize: {error}"));
    let decoded: ThemeDefinitionV1 = serde_json::from_str(&pretty_json)
        .unwrap_or_else(|error| panic!("{witness_name} definition should decode: {error}"));
    assert_eq!(
        definition
            .canonical_json_bytes()
            .expect("the typed definition should canonicalize"),
        decoded
            .canonical_json_bytes()
            .expect("the JSON definition should canonicalize"),
        "{witness_name} canonical definition bytes must survive pretty JSON",
    );

    let typed = materialize_theme(definition).unwrap_or_else(|error| {
        panic!("{witness_name} typed definition should materialize: {error}")
    });
    let imported = materialize_theme(&decoded).unwrap_or_else(|error| {
        panic!("{witness_name} JSON definition should materialize: {error}")
    });
    assert_eq!(
        typed.spec(),
        imported.spec(),
        "{witness_name} materialized spec must survive pretty JSON",
    );
    imported
}

fn witness_svg_request(diagram_id: &str) -> merman::SvgRequest {
    merman::SvgRequest {
        options: merman::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id.to_owned()),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn render_authoring_svg(
    renderer: &Renderer,
    witness: &AuthoringRenderWitness,
    theme: &merman::diagram_theme::DiagramTheme,
) -> String {
    render_authoring_svg_output(renderer, witness, theme)
        .svg()
        .to_owned()
}

fn render_authoring_svg_output(
    renderer: &Renderer,
    witness: &AuthoringRenderWitness,
    theme: &merman::diagram_theme::DiagramTheme,
) -> merman::SvgOutput {
    let output = renderer
        .render(
            RenderRequest::svg(
                witness.source,
                OperationControl::new(),
                witness_svg_request(witness.diagram_id),
            )
            .with_theme(theme.clone()),
        )
        .unwrap_or_else(|error| panic!("{} SVG should render: {error}", witness.name));
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("{} should produce an SVG", witness.name);
    };
    output
}

fn assert_generated_rule_terminal(
    renderer: &Renderer,
    witness: GeneratedRuleTerminalWitness,
    theme: &merman::diagram_theme::DiagramTheme,
) {
    let output = render_authoring_svg_output(renderer, witness.diagram, theme);
    let render_evidence = output.evidence();
    let evidence = render_evidence.theme_evidence();
    assert!(
        evidence.is_verified(),
        "{} must verify the isolated {} expansion row",
        witness.diagram.name,
        witness.target.id(),
    );
    #[cfg(feature = "internal-theme-acceptance")]
    {
        let family =
            merman::__theme_acceptance::theme_acceptance_evidence(render_evidence).family();
        assert_eq!(
            family.applied_count(),
            1,
            "{} must apply exactly one isolated {} expansion row",
            witness.diagram.name,
            witness.target.id(),
        );
        assert_eq!(
            family.not_applicable_count(),
            0,
            "{} must not classify the isolated {} expansion row as not applicable",
            witness.diagram.name,
            witness.target.id(),
        );
        assert!(
            family.is_verified(),
            "{} family evidence must verify the isolated {} expansion row",
            witness.diagram.name,
            witness.target.id(),
        );
    }

    let document = roxmltree::Document::parse(output.svg()).unwrap_or_else(|error| {
        panic!(
            "{} must emit valid terminal SVG: {error}",
            witness.diagram.name
        )
    });
    assert_generated_rule_terminal_surface(&document, witness);
}

fn inline_style_property<'a>(node: roxmltree::Node<'a, '_>, property: &str) -> Option<&'a str> {
    last_css_property(node.attribute("style")?, property)
}

fn last_css_property<'a>(declarations: &'a str, property: &str) -> Option<&'a str> {
    declarations.split(';').fold(None, |winner, declaration| {
        let Some((name, value)) = declaration.split_once(':') else {
            return winner;
        };
        (name.trim() == property).then(|| value.trim()).or(winner)
    })
}

fn normalized_css_value(value: &str) -> &str {
    value
        .trim()
        .strip_suffix("!important")
        .unwrap_or(value.trim())
        .trim()
}

fn element_property<'a>(node: roxmltree::Node<'a, '_>, property: &str) -> Option<&'a str> {
    inline_style_property(node, property)
        .or_else(|| node.attribute(property))
        .map(normalized_css_value)
}

fn assert_element_property(
    node: roxmltree::Node<'_, '_>,
    property: &str,
    expected: &str,
    context: &str,
) {
    assert_eq!(
        element_property(node, property),
        Some(expected),
        "{context} must own terminal {property}: {:?}",
        node.attribute("style"),
    );
}

fn assert_witness_fill_property(
    node: roxmltree::Node<'_, '_>,
    witness: GeneratedRuleTerminalWitness,
    property: &str,
    context: &str,
) {
    if let Some(token) = witness.fill {
        assert_element_property(node, property, terminal_color(token), context);
    }
}

fn assert_witness_stroke_property(
    node: roxmltree::Node<'_, '_>,
    witness: GeneratedRuleTerminalWitness,
    property: &str,
    context: &str,
) {
    if let Some(token) = witness.stroke {
        assert_element_property(node, property, terminal_color(token), context);
    }
}

fn assert_witness_fill_and_stroke(
    node: roxmltree::Node<'_, '_>,
    witness: GeneratedRuleTerminalWitness,
    fill_property: &str,
    stroke_property: &str,
    context: &str,
) {
    assert_witness_fill_property(node, witness, fill_property, context);
    assert_witness_stroke_property(node, witness, stroke_property, context);
}

fn element_has_text(node: roxmltree::Node<'_, '_>, expected: &str) -> bool {
    node.descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|descendant| descendant.text())
        .any(|text| text.trim() == expected)
}

fn find_element_with_class_and_text<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    tag: &str,
    class: &str,
    text: &str,
    context: &str,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.has_tag_name(tag)
                && element_has_class(*node, class)
                && element_has_text(*node, text)
        })
        .unwrap_or_else(|| panic!("missing {context}"))
}

fn assert_path_property(
    scope: roxmltree::Node<'_, '_>,
    property: &str,
    expected: &str,
    context: &str,
) {
    assert!(
        scope.descendants().any(|node| {
            node.has_tag_name("path") && element_property(node, property) == Some(expected)
        }),
        "{context} must contain a terminal path with {property}:{expected}",
    );
}

fn assert_witness_path_properties(
    scope: roxmltree::Node<'_, '_>,
    witness: GeneratedRuleTerminalWitness,
    context: &str,
) {
    if let Some(token) = witness.fill {
        assert_path_property(scope, "fill", terminal_color(token), context);
    }
    if let Some(token) = witness.stroke {
        assert_path_property(scope, "stroke", terminal_color(token), context);
    }
}

fn stylesheet_property<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    selector: &str,
    property: &str,
) -> Option<&'a str> {
    let prefix = format!("{selector}{{");
    let mut terminal_value = None;
    for css in document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
    {
        let mut remaining = css;
        while let Some(rule_start) = remaining.find(&prefix) {
            let declarations_start = rule_start + prefix.len();
            let Some(declarations_end) = remaining[declarations_start..].find('}') else {
                break;
            };
            let declarations =
                &remaining[declarations_start..declarations_start + declarations_end];
            if let Some(value) = last_css_property(declarations, property) {
                terminal_value = Some(normalized_css_value(value));
            }
            remaining = &remaining[declarations_start + declarations_end + 1..];
        }
    }
    terminal_value
}

fn assert_sequence_css_rule_for_live_surface(
    document: &roxmltree::Document<'_>,
    tag: &str,
    live_class: &str,
    selector_classes: &[&str],
    witness: GeneratedRuleTerminalWitness,
    context: &str,
) {
    let root_id = document
        .root_element()
        .attribute("id")
        .expect("terminal Sequence SVG root id");
    let selector = selector_classes
        .iter()
        .map(|class| format!("#{root_id} .{class}"))
        .collect::<Vec<_>>()
        .join(",");
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name(tag) && element_has_class(node, live_class)),
        "missing live {context} surface matched by {selector}",
    );
    for (property, token) in [("fill", witness.fill), ("stroke", witness.stroke)] {
        let Some(token) = token else {
            continue;
        };
        assert_eq!(
            stylesheet_property(document, &selector, property),
            Some(terminal_color(token)),
            "{context} CSS rule {selector} must own terminal {property}",
        );
    }
}

fn find_idle_to_decide_transition<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("data-edge") == Some("true")
                && node.attribute("data-id") == Some("edge1")
        })
        .expect("Idle-to-Decide transition path")
}

fn find_idle_to_decide_label<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> roxmltree::Node<'a, 'input> {
    find_element_with_class_and_text(
        document,
        "div",
        "labelBkg",
        "choose",
        "Idle-to-Decide transition label",
    )
}

fn assert_generated_rule_terminal_surface(
    document: &roxmltree::Document<'_>,
    witness: GeneratedRuleTerminalWitness,
) {
    match witness.target {
        ThemeTarget::Text | ThemeTarget::StateLabel => {
            let label = find_element_with_class_and_text(
                document,
                "g",
                "label",
                "Idle",
                "ordinary State label",
            );
            assert_witness_fill_property(label, witness, "color", witness.target.id());
        }
        ThemeTarget::Title => {
            let title = find_element_with_class_and_text(
                document,
                "text",
                "statediagramTitleText",
                "State theme witness",
                "terminal State title",
            );
            assert_witness_fill_property(title, witness, "fill", "State title");
        }
        ThemeTarget::Node => {
            let node = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("data-id") == Some("A")
                        && element_has_class(*node, "node")
                })
                .expect("Flowchart node A");
            let shape = node
                .descendants()
                .find(|node| element_has_class(*node, "label-container"))
                .expect("Flowchart node A shape");
            assert_witness_stroke_property(shape, witness, "stroke", "Flowchart node A shape");
        }
        ThemeTarget::Edge => {
            let edge = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("path")
                        && node.attribute("data-edge") == Some("true")
                        && node.attribute("data-id") == Some("L_A_B_0")
                })
                .expect("Flowchart A-to-B edge path");
            assert_witness_stroke_property(edge, witness, "stroke", "Flowchart A-to-B edge path");
        }
        ThemeTarget::Cluster => {
            let cluster = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("data-id") == Some("Group")
                        && element_has_class(*node, "cluster")
                })
                .expect("Flowchart Group cluster");
            let shape = cluster
                .children()
                .find(|node| node.has_tag_name("rect"))
                .expect("Flowchart Group cluster shape");
            assert_witness_fill_and_stroke(
                shape,
                witness,
                "fill",
                "stroke",
                "Flowchart Group cluster shape",
            );
        }
        ThemeTarget::Actor => {
            assert_sequence_css_rule_for_live_surface(
                document,
                "rect",
                "actor",
                &["actor"],
                witness,
                "Sequence actor rectangle",
            );
        }
        ThemeTarget::Lifeline => {
            assert_sequence_css_rule_for_live_surface(
                document,
                "line",
                "actor-line",
                &["actor-line"],
                witness,
                "Sequence actor lifeline",
            );
        }
        ThemeTarget::Message => {
            assert_sequence_css_rule_for_live_surface(
                document,
                "line",
                "messageLine0",
                &["messageLine0", "messageLine1"],
                witness,
                "Sequence message line",
            );
        }
        ThemeTarget::State => {
            let state = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && element_has_class(*node, "statediagram-state")
                        && element_has_text(*node, "Idle")
                })
                .expect("ordinary Idle state");
            let shape = state
                .descendants()
                .find(|node| element_has_class(*node, "label-container"))
                .expect("ordinary Idle state shape");
            assert_witness_fill_and_stroke(
                shape,
                witness,
                "fill",
                "stroke",
                "ordinary Idle state shape",
            );
        }
        ThemeTarget::Transition => {
            let transition = find_idle_to_decide_transition(document);
            assert_witness_stroke_property(
                transition,
                witness,
                "stroke",
                "Idle-to-Decide transition path",
            );
        }
        ThemeTarget::TransitionMarker => {
            let transition = find_idle_to_decide_transition(document);
            let marker_id = transition
                .attribute("marker-end")
                .and_then(|value| value.strip_prefix("url(#"))
                .and_then(|value| value.strip_suffix(')'))
                .expect("Idle-to-Decide transition marker reference");
            let marker_path = document
                .descendants()
                .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(marker_id))
                .and_then(|marker| marker.descendants().find(|node| node.has_tag_name("path")))
                .expect("referenced Idle-to-Decide marker path");
            assert_witness_fill_and_stroke(
                marker_path,
                witness,
                "fill",
                "stroke",
                "Idle-to-Decide marker path",
            );
        }
        ThemeTarget::TransitionLabel => {
            let label = find_idle_to_decide_label(document);
            assert_witness_fill_property(
                label,
                witness,
                "color",
                "Idle-to-Decide transition label",
            );
        }
        ThemeTarget::TransitionLabelBackground => {
            let label = find_idle_to_decide_label(document);
            assert_witness_fill_property(
                label,
                witness,
                "background-color",
                "Idle-to-Decide transition label background",
            );
        }
        ThemeTarget::Composite | ThemeTarget::CompositeHeader => {
            let cluster = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("data-id") == Some("Parent")
                        && element_has_class(*node, "statediagram-cluster")
                })
                .expect("Parent composite state");
            let class = if witness.target == ThemeTarget::Composite {
                "inner"
            } else {
                "outer"
            };
            let shape = cluster
                .descendants()
                .find(|node| node.has_tag_name("rect") && element_has_class(*node, class))
                .unwrap_or_else(|| panic!("Parent composite {class} shape"));
            assert_witness_fill_and_stroke(shape, witness, "fill", "stroke", witness.target.id());
        }
        ThemeTarget::CompositeLabel => {
            let label = find_element_with_class_and_text(
                document,
                "g",
                "cluster-label",
                "Parent",
                "Parent composite label",
            );
            assert_witness_fill_property(label, witness, "color", "Parent composite label");
        }
        ThemeTarget::SpecialState => {
            let special = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node
                            .attribute("id")
                            .is_some_and(|id| id.contains("-state-Decide-"))
                })
                .expect("Decide special state");
            assert_witness_path_properties(special, witness, "Decide special state");
        }
        ThemeTarget::SpecialStateInner => {
            let end = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node
                            .attribute("id")
                            .is_some_and(|id| id.contains("-state-root_end-"))
                })
                .expect("terminal end state");
            assert_witness_path_properties(end, witness, "terminal end-state inner surface");
        }
        ThemeTarget::Note => {
            assert_sequence_css_rule_for_live_surface(
                document,
                "rect",
                "note",
                &["note"],
                witness,
                "Sequence note rectangle",
            );
        }
        ThemeTarget::NoteLabel => {
            let label = find_element_with_class_and_text(
                document,
                "g",
                "noteLabel",
                "State note",
                "State note label",
            );
            assert_witness_fill_property(label, witness, "color", "State note label");
        }
        ThemeTarget::Activation => {
            assert_sequence_css_rule_for_live_surface(
                document,
                "rect",
                "activation0",
                &["activation0", "activation1", "activation2"],
                witness,
                "Sequence activation rectangle",
            );
        }
        ThemeTarget::Entity => {
            let entity = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node
                            .attribute("id")
                            .is_some_and(|id| id.ends_with("-entity-CUSTOMER-0"))
                })
                .expect("CUSTOMER entity group");
            let shell = entity
                .children()
                .find(|node| node.has_tag_name("g") && element_has_class(*node, "outer-path"))
                .expect("CUSTOMER entity outer shell");
            assert_witness_path_properties(shell, witness, "CUSTOMER entity outer shell");
        }
        _ => panic!(
            "missing terminal surface assertion for generated target {}",
            witness.target.id()
        ),
    }
}

fn element_has_class(node: roxmltree::Node<'_, '_>, expected: &str) -> bool {
    node.attribute("class").is_some_and(|classes| {
        classes
            .split_ascii_whitespace()
            .any(|class| class == expected)
    })
}

fn assert_generated_palette_terminal(
    renderer: &Renderer,
    witness: GeneratedPaletteTerminalWitness,
    theme: &merman::diagram_theme::DiagramTheme,
) {
    let output = render_authoring_svg_output(renderer, witness.diagram, theme);
    assert!(
        output.evidence().theme_evidence().is_verified(),
        "{} must verify the isolated {} palette",
        witness.diagram.name,
        witness.target.id(),
    );
    let document = roxmltree::Document::parse(output.svg()).unwrap_or_else(|error| {
        panic!(
            "{} must emit valid terminal SVG: {error}",
            witness.diagram.name
        )
    });

    match witness.target {
        ThemeTarget::Node => {
            let style = document
                .descendants()
                .find(|node| {
                    node.is_element()
                        && element_has_class(*node, "label-container")
                        && node
                            .ancestors()
                            .any(|ancestor| ancestor.attribute("data-id") == Some("A"))
                })
                .and_then(|node| node.attribute("style"))
                .expect("the first Flowchart node shape style");
            assert!(
                style.contains(&format!("fill:{} !important", TERMINAL_SERIES[0])),
                "the generated Node palette must own the first Flowchart node fill: {style}",
            );
        }
        ThemeTarget::PieSlice => {
            let first_slice_fill = document
                .descendants()
                .find(|node| node.has_tag_name("path") && element_has_class(*node, "pieCircle"))
                .and_then(|node| node.attribute("fill"))
                .expect("the first Pie slice fill");
            assert_eq!(first_slice_fill, TERMINAL_SERIES[0]);

            let first_legend_style = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect")
                        && node.parent().is_some_and(|parent| {
                            parent.has_tag_name("g") && element_has_class(parent, "legend")
                        })
                })
                .and_then(|node| node.attribute("style"))
                .expect("the first Pie legend swatch style");
            assert!(
                first_legend_style.contains(&format!("fill: {TERMINAL_FIRST_SERIES_RGB}")),
                "the generated PieSlice palette must own the first legend swatch: {first_legend_style}",
            );
        }
        _ => panic!(
            "unexpected generated palette target {}",
            witness.target.id()
        ),
    }
}

#[cfg(feature = "png")]
fn render_authoring_png(
    renderer: &Renderer,
    witness: &AuthoringRenderWitness,
    theme: &merman::diagram_theme::DiagramTheme,
) -> Vec<u8> {
    let output = renderer
        .render(
            RenderRequest::png(
                witness.source,
                OperationControl::new(),
                merman::PngRequest {
                    svg: witness_svg_request(witness.diagram_id),
                    options: merman::svg::export::RasterOptions::default(),
                },
            )
            .with_theme(theme.clone()),
        )
        .unwrap_or_else(|error| panic!("{} PNG should render: {error}", witness.name));
    let RenderOutput::Png(Some(output)) = output else {
        panic!("{} should produce a PNG", witness.name);
    };
    assert!(output.bytes().starts_with(b"\x89PNG\r\n\x1a\n"));
    output.into_bytes()
}

#[test]
fn prefreeze_tokens_only_themes_round_trip_and_render_without_state_leakage() {
    let light_definition = light_authoring_witness();
    let dark_definition = dark_authoring_witness();
    assert!(light_definition.styles().is_empty());
    assert!(dark_definition.styles().is_empty());
    let light_materialized = materialize_pretty_json_round_trip(&light_definition, "light");
    let dark_materialized = materialize_pretty_json_round_trip(&dark_definition, "dark");
    let compiler = DiagramThemeCompiler::new();
    let light_theme = compiler
        .compile_spec_wire(light_materialized.into_spec())
        .expect("the light materialized spec should compile once");
    let dark_theme = compiler
        .compile_spec_wire(dark_materialized.into_spec())
        .expect("the dark materialized spec should compile once");
    let first_renderer = Renderer::new();
    let second_renderer = Renderer::new();

    for witness in &AUTHORING_RENDER_WITNESSES {
        let first_light = render_authoring_svg(&first_renderer, witness, &light_theme);
        let first_dark = render_authoring_svg(&first_renderer, witness, &dark_theme);
        let repeated_light = render_authoring_svg(&first_renderer, witness, &light_theme);
        let independent_light = render_authoring_svg(&second_renderer, witness, &light_theme);
        let independent_dark = render_authoring_svg(&second_renderer, witness, &dark_theme);

        assert_eq!(
            first_light, repeated_light,
            "{} light -> dark -> light SVG rendering must not leak state",
            witness.name,
        );
        assert_eq!(
            first_light, independent_light,
            "{} light SVG must be stable across Renderer instances",
            witness.name,
        );
        assert_eq!(
            first_dark, independent_dark,
            "{} dark SVG must be stable across Renderer instances",
            witness.name,
        );
        assert_ne!(
            first_light, first_dark,
            "{} light and dark SVGs must differ",
            witness.name,
        );
    }

    #[cfg(feature = "png")]
    for witness in &AUTHORING_RENDER_WITNESSES {
        let first_light = render_authoring_png(&first_renderer, witness, &light_theme);
        let first_dark = render_authoring_png(&first_renderer, witness, &dark_theme);
        let repeated_light = render_authoring_png(&first_renderer, witness, &light_theme);
        let independent_light = render_authoring_png(&second_renderer, witness, &light_theme);
        let independent_dark = render_authoring_png(&second_renderer, witness, &dark_theme);

        assert_eq!(
            first_light, repeated_light,
            "{} light -> dark -> light PNG rendering must not leak state",
            witness.name,
        );
        assert_eq!(
            first_light, independent_light,
            "{} light PNG must be stable across Renderer instances",
            witness.name,
        );
        assert_eq!(
            first_dark, independent_dark,
            "{} dark PNG must be stable across Renderer instances",
            witness.name,
        );
        assert_ne!(
            first_light, first_dark,
            "{} light and dark PNGs must differ",
            witness.name,
        );
    }
}

#[test]
fn complete_spec_can_cold_start_without_authoring_materialization() {
    const COLD_START_COLOR: &str = "#dc2626";
    let style = merman::svg::ThemeStylePatch::default().with_fill(
        merman::svg::CanvasPaint::solid(COLD_START_COLOR)
            .expect("the cold-start fixture color should be valid"),
    );
    let spec = merman::svg::DiagramThemeSpec::new().with_styles(
        merman::svg::ThemeRuleSet::default().with_rule(
            merman::svg::ThemeRule::new(merman::svg::ThemeTarget::Node, style)
                .for_family(DiagramFamilyId::FLOWCHART),
        ),
    );
    let theme = DiagramThemeCompiler::new()
        .compile(spec)
        .expect("the complete spec should compile without authoring materialization");
    let svg = render_authoring_svg(&Renderer::new(), &AUTHORING_RENDER_WITNESSES[0], &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid cold-start Flowchart SVG");
    let node_style = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "label-container")
                })
        })
        .and_then(|node| node.attribute("style"))
        .expect("the cold-start Flowchart node style");
    assert!(
        node_style.contains(&format!("fill:{COLD_START_COLOR} !important")),
        "the complete-spec rule must own the terminal node fill: {node_style}",
    );
}

#[test]
fn typed_family_rule_matches_the_equivalent_shareable_json() {
    let typed = ThemeDefinitionBuilderV1::new(ThemeTokensV1::default())
        .with_rule(
            ThemeRuleBuilderV1::new(ThemeTarget::Node)
                .for_family(DiagramFamilyId::FLOWCHART)
                .with_variant(ThemeVariant::Primary)
                .with_ordinal_cycle(4, 1)
                .with_fill_color("#60a5fa")
                .with_stroke_color("#1e293b")
                .with_stroke_width(2.0)
                .with_radius(8.0)
                .clear(ThemeRuleFacetV1::FillOpacity),
        )
        .with_ordinal_palette(ThemeTarget::Node, ["#60a5fa", "#34d399"])
        .build();
    let decoded: ThemeDefinitionV1 = serde_json::from_str(
        r##"{
            "authoring_schema_version": 1,
            "expansion_version": 1,
            "tokens": {},
            "styles": [
                {
                    "kind": "rule",
                    "target": "node",
                    "family": "flowchart",
                    "variant": "primary",
                    "ordinal": {"cycle": {"period": 4, "offset": 1}},
                    "style": {
                        "fill": "#60a5fa",
                        "fill_opacity": null,
                        "stroke": {"paint": "#1e293b", "width": 2},
                        "radius": 8
                    }
                },
                {
                    "kind": "ordinal-palette",
                    "target": "node",
                    "colors": ["#60a5fa", "#34d399"]
                }
            ]
        }"##,
    )
    .expect("the equivalent shared JSON should decode");

    assert_eq!(
        typed
            .canonical_json_bytes()
            .expect("the typed definition should canonicalize"),
        decoded
            .canonical_json_bytes()
            .expect("the decoded definition should canonicalize"),
    );
}

#[test]
fn versioned_authoring_materializes_and_compiles_through_the_rust_facade() {
    let definition = ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_color(ThemeColorTokenV1::Canvas, "#0f172a")
            .with_color(ThemeColorTokenV1::Surface, "#111827")
            .with_color(ThemeColorTokenV1::Text, "#e5e7eb"),
    );
    let compiler = DiagramThemeCompiler::new();
    let materialized =
        materialize_theme(&definition).expect("the explicit materialization step should succeed");
    let expected = compiler
        .compile_spec_wire(materialized.into_spec())
        .expect("the explicit compilation step should succeed");
    let theme = compile_theme_definition(&compiler, &definition)
        .expect("the materialized complete spec should compile");

    assert_eq!(theme.recipe_fingerprint(), expected.recipe_fingerprint());
}

#[test]
fn generated_authoring_rows_and_palettes_reach_typed_terminal_witnesses() {
    let definition = terminal_witness_definition();
    let materialized = materialize_theme(&definition)
        .expect("the terminal-witness definition should pass admission and materialize");
    let materialized_spec = materialized.spec().clone();
    let styles = materialized_spec
        .styles
        .as_ref()
        .expect("the version one expansion must emit styles");
    assert_eq!(
        styles.len(),
        GENERATED_RULE_TERMINAL_WITNESSES.len() + GENERATED_PALETTE_TERMINAL_WITNESSES.len(),
        "adding or removing a generated row requires a terminal witness"
    );

    let compiler = DiagramThemeCompiler::new();
    let renderer = Renderer::new();
    for (index, witness) in GENERATED_RULE_TERMINAL_WITNESSES
        .iter()
        .copied()
        .enumerate()
    {
        let entry = &styles[index];
        let ThemeRuleSetWireV1::Rule {
            target,
            family,
            variant,
            ordinal,
            style,
        } = entry
        else {
            panic!("materialized row {index} must remain a generated rule")
        };
        assert_eq!(
            target,
            witness.target.id(),
            "materialized generated rule {index} changed target or order"
        );
        assert!(
            family.is_none(),
            "materialized generated rule {index} must remain cross-family"
        );
        assert_eq!(
            variant.as_deref(),
            witness.variant.map(ThemeVariant::id),
            "materialized generated rule {index} changed variant"
        );
        assert!(
            ordinal.is_none(),
            "materialized generated rule {index} must remain non-ordinal"
        );
        assert_eq!(
            style,
            &expected_generated_style(witness),
            "materialized generated rule {index} changed emitted facets"
        );

        // Isolate the indexed materialized entry so another generated row using the same token
        // cannot satisfy this row's terminal witness.
        let isolated_spec = DiagramThemeSpecWireV1 {
            styles: Some(vec![entry.clone()]),
            ..DiagramThemeSpecWireV1::default()
        };
        let theme = compiler
            .compile_spec_wire(isolated_spec)
            .unwrap_or_else(|error| panic!("generated rule {index} should compile: {error}"));
        assert_generated_rule_terminal(&renderer, witness, &theme);
    }

    let expected_series = TERMINAL_SERIES.map(str::to_owned).to_vec();
    for (palette_index, witness) in GENERATED_PALETTE_TERMINAL_WITNESSES
        .iter()
        .copied()
        .enumerate()
    {
        let materialized_index = GENERATED_RULE_TERMINAL_WITNESSES.len() + palette_index;
        let entry = &styles[materialized_index];
        let ThemeRuleSetWireV1::OrdinalPalette { target, colors } = entry else {
            panic!("materialized row {materialized_index} must remain a generated palette")
        };
        assert_eq!(
            target,
            witness.target.id(),
            "generated palette {palette_index} changed target or order"
        );
        assert_eq!(
            colors, &expected_series,
            "generated palette {palette_index} must retain the authored series"
        );

        let isolated_spec = DiagramThemeSpecWireV1 {
            styles: Some(vec![entry.clone()]),
            ..DiagramThemeSpecWireV1::default()
        };
        let theme = compiler
            .compile_spec_wire(isolated_spec)
            .unwrap_or_else(|error| {
                panic!("generated palette {palette_index} should compile: {error}")
            });
        assert_generated_palette_terminal(&renderer, witness, &theme);
    }
}

#[test]
fn rust_facade_exposes_the_versioned_support_discovery_operation() {
    let support = describe_theme_support(&ThemeSupportQueryV1::known(
        DiagramFamilyId::FLOWCHART.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Node.id(),
        ThemeRuleFacetV1::Radius,
    ));

    assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
    assert_eq!(support.query().family_id(), "flowchart");
}

#[test]
fn rust_facade_preserves_materialization_and_compilation_error_sources() {
    let materialization_error = compile_theme_definition(
        &DiagramThemeCompiler::new(),
        &ThemeDefinitionV1::new(ThemeTokensV1::default().with_series(Vec::new())),
    )
    .expect_err("an empty authored series should fail during materialization");
    let ThemeDefinitionCompileError::Materialization(error) = materialization_error else {
        panic!("empty series must retain the materialization error source")
    };
    assert_eq!(error.diagnostic().code(), "theme-authoring.empty-series");
    assert_eq!(error.diagnostic().path(), "/tokens/series");

    let compilation_error = compile_theme_definition(
        &DiagramThemeCompiler::new(),
        &ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![
            ThemeRuleSetWireV1::Rule {
                target: "future-node".to_owned(),
                family: None,
                variant: None,
                ordinal: None,
                style: ThemeStylePatchWireV1::default(),
            },
        ]),
    )
    .expect_err("an unknown semantic target should fail during compilation");
    assert!(matches!(
        compilation_error,
        ThemeDefinitionCompileError::Compilation(_)
    ));
}

#[test]
fn json_authoring_rejects_the_first_rule_beyond_the_v1_budget_before_typed_decode() {
    let rule = r##"{"kind":"rule","target":"node","style":{"fill":"#123456"}}"##;
    let admitted_styles = std::iter::repeat_n(rule, 489).collect::<Vec<_>>().join(",");

    let exact = format!(
        r##"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{}},"styles":[{admitted_styles}]}}"##
    );
    compile_theme_definition_json(&DiagramThemeCompiler::new(), exact.as_bytes())
        .expect("the exact V1 authored-rule budget should compile");

    let oversized = format!(
        r##"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{}},"styles":[{admitted_styles},{{"kind":"rule","target":{{"typed":"decode must not reach this value"}},"style":{{"fill":"#123456"}}}}]}}"##
    );
    let error = compile_theme_definition_json(&DiagramThemeCompiler::new(), oversized.as_bytes())
        .expect_err("the first rule beyond the V1 budget must fail during streaming admission");
    let ThemeDefinitionCompileError::Materialization(error) = error else {
        panic!("oversized authoring rules must fail during materialization")
    };
    let diagnostic = error.diagnostic();
    assert_eq!(diagnostic.code(), "theme-authoring.rule-budget-exceeded");
    assert_eq!(diagnostic.limit_id(), Some("max_authored_rules"));
    assert_eq!(diagnostic.actual(), Some(490));
    assert_eq!(diagnostic.max(), Some(489));
}

#[test]
fn json_authoring_collection_limits_accept_the_exact_boundary_and_reject_the_next_item() {
    let colors = std::iter::repeat_n("\"#123456\"", 256)
        .collect::<Vec<_>>()
        .join(",");
    let exact_series = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{"series":[{colors}]}}}}"#
    );
    compile_theme_definition_json(&DiagramThemeCompiler::new(), exact_series.as_bytes())
        .expect("the exact series-color budget should compile");
    let oversized_series = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{"series":[{colors},{{"typed":"decode must not reach this value"}}]}}}}"#
    );
    let error =
        compile_theme_definition_json(&DiagramThemeCompiler::new(), oversized_series.as_bytes())
            .expect_err("the first series color beyond the bound must fail during admission");
    assert_materialization_resource_limit(
        error,
        "/tokens/series",
        "max_theme_palette_colors",
        257,
        256,
    );

    let families = (0..32)
        .map(|index| format!("\"Authoring Font {index}\""))
        .collect::<Vec<_>>()
        .join(",");
    let exact_fonts = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{"typography":{{"font_stack":[{families}]}}}}}}"#
    );
    compile_theme_definition_json(&DiagramThemeCompiler::new(), exact_fonts.as_bytes())
        .expect("the exact font-stack budget should compile");
    let oversized_fonts = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{"typography":{{"font_stack":[{families},{{"typed":"decode must not reach this value"}}]}}}}}}"#
    );
    let error =
        compile_theme_definition_json(&DiagramThemeCompiler::new(), oversized_fonts.as_bytes())
            .expect_err("the first font family beyond the bound must fail during admission");
    assert_materialization_resource_limit(
        error,
        "/tokens/typography/font_stack",
        "max_theme_font_stack_entries",
        33,
        32,
    );

    let exact_palette = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{}},"styles":[{{"kind":"ordinal-palette","target":"node","colors":[{colors}]}}]}}"#
    );
    compile_theme_definition_json(&DiagramThemeCompiler::new(), exact_palette.as_bytes())
        .expect("the exact ordinal-palette color budget should compile");
    let oversized_palette = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{}},"styles":[{{"kind":"ordinal-palette","target":"node","colors":[{colors},{{"typed":"decode must not reach this value"}}]}}]}}"#
    );
    let error =
        compile_theme_definition_json(&DiagramThemeCompiler::new(), oversized_palette.as_bytes())
            .expect_err("the first ordinal color beyond the bound must fail during admission");
    assert_materialization_resource_limit(
        error,
        "/styles/ordinal-palette/colors",
        "max_theme_palette_colors",
        257,
        256,
    );
}

#[test]
fn typed_compact_paint_obeys_the_decoded_string_ceiling() {
    let definition = ThemeDefinitionBuilderV1::new(ThemeTokensV1::default())
        .with_rule(
            ThemeRuleBuilderV1::new(ThemeTarget::Node).with_fill_color("x".repeat(64 * 1024 + 1)),
        )
        .build();
    let error = compile_theme_definition(&DiagramThemeCompiler::new(), &definition)
        .expect_err("compact paint strings must use the shared decoded-string ceiling");

    assert_materialization_resource_limit(
        error,
        "/styles/rule/style/paint/color",
        "max_theme_definition_string_bytes",
        65_537,
        65_536,
    );
}

#[test]
fn typed_authoring_uses_the_caller_owned_encoded_input_policy_before_expansion() {
    let policy = ThemeResourcePolicy::default()
        .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
        .expect("one byte is a valid caller-owned encoded-theme ceiling");
    let compiler = DiagramThemeCompiler::new().with_resource_policy(policy);
    let error =
        compile_theme_definition(&compiler, &ThemeDefinitionV1::new(ThemeTokensV1::default()))
            .expect_err(
                "the typed facade must apply the compiler policy before materialization expands",
            );

    assert_materialization_resource_limit(error, "", "max_theme_encoded_bytes", 2, 1);
}

fn assert_materialization_resource_limit(
    error: ThemeDefinitionCompileError,
    path: &str,
    limit_id: &str,
    actual: u64,
    max: u64,
) {
    let ThemeDefinitionCompileError::Materialization(error) = error else {
        panic!("resource admission must retain the materialization error source")
    };
    let diagnostic = error.diagnostic();
    assert_eq!(diagnostic.code(), "theme-authoring.resource-limit-exceeded");
    assert_eq!(diagnostic.path(), path);
    assert_eq!(diagnostic.limit_id(), Some(limit_id));
    assert_eq!(diagnostic.actual(), Some(actual));
    assert_eq!(diagnostic.max(), Some(max));
}
