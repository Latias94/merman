use merman_core::MermaidConfig;
use merman_core::theme_color::{ColorChannel, ThemeColor};
use serde_json::{Map, Value};

use crate::render_family::RenderFamilyKind;

use super::canvas::CanvasPaint;
use super::resolved::resolve_style;
use super::{DiagramThemeSpec, ThemeTarget, ThemeVariant};

pub(super) fn compile(spec: &DiagramThemeSpec) -> MermaidConfig {
    let roles = CompatibilityThemeRoles::new(spec);
    let palette = chart_palette(spec);
    let dark = spec.mermaid().dark_mode().unwrap_or(false);
    let mut root = Map::new();
    let mut variables = Map::new();

    root.insert("theme".to_string(), Value::String("base".to_string()));
    if let Some(dark_mode) = spec.mermaid().dark_mode() {
        root.insert("darkMode".to_string(), Value::Bool(dark_mode));
        variables.insert("darkMode".to_string(), Value::Bool(dark_mode));
    }

    let (font_family, font_size) = spec.typography().default_style().to_mermaid_values();
    root.insert("fontFamily".to_string(), Value::String(font_family.clone()));
    put_str(&mut variables, "fontFamily", &font_family);
    put_str(&mut variables, "fontSize", &font_size);

    put_theme_roles(&mut variables, &roles);
    put_series_palette(&mut variables, &palette, roles.canvas.as_deref(), dark);
    put_diagram_config(&mut root, &mut variables, &roles, &palette);
    root.insert("themeVariables".to_string(), Value::Object(variables));

    let mut config = MermaidConfig::from_value(Value::Object(root));
    config.deep_merge(spec.mermaid().to_mermaid_config().as_value());
    config
}

#[derive(Debug, Clone)]
struct CompatibilityThemeRoles {
    canvas: Option<String>,
    surface: Option<String>,
    surface_alt: Option<String>,
    surface_muted: Option<String>,
    text: Option<String>,
    title: Option<String>,
    subtle_text: Option<String>,
    border: Option<String>,
    line: Option<String>,
    arrowhead: Option<String>,
    edge_label_background: Option<String>,
    edge_label_text: Option<String>,
    commit_label_background: Option<String>,
    cluster_background: Option<String>,
    swimlane_background_odd: Option<String>,
    cluster_border: Option<String>,
    note_background: Option<String>,
    note_border: Option<String>,
    note_text: Option<String>,
    actor_background: Option<String>,
    actor_border: Option<String>,
    actor_text: Option<String>,
    lifeline: Option<String>,
    signal: Option<String>,
    signal_text: Option<String>,
    loop_background: Option<String>,
    loop_border: Option<String>,
    loop_text: Option<String>,
    activation_background: Option<String>,
    activation_border: Option<String>,
    state_background: Option<String>,
    state_border: Option<String>,
    state_text: Option<String>,
    transition: Option<String>,
    transition_text: Option<String>,
    special_state: Option<String>,
    composite_background: Option<String>,
    error: Option<String>,
    warning: Option<String>,
    success: Option<String>,
}

impl CompatibilityThemeRoles {
    fn new(spec: &DiagramThemeSpec) -> Self {
        let canvas = solid_paint(spec.canvas().base());
        let surface = fill(spec, RenderFamilyKind::Flowchart, ThemeTarget::Node);
        let surface_alt = fill(spec, RenderFamilyKind::Sequence, ThemeTarget::Loop)
            .or_else(|| fill(spec, RenderFamilyKind::Flowchart, ThemeTarget::Cluster))
            .or_else(|| surface.clone());
        let activation_background = fill(spec, RenderFamilyKind::Sequence, ThemeTarget::Activation);
        let surface_muted = activation_background
            .clone()
            .or_else(|| surface_alt.clone());
        let text = fill(spec, RenderFamilyKind::Flowchart, ThemeTarget::Text)
            .or_else(|| fill(spec, RenderFamilyKind::Flowchart, ThemeTarget::NodeLabel));
        let title =
            fill(spec, RenderFamilyKind::State, ThemeTarget::Title).or_else(|| text.clone());
        let subtle_text = fill(spec, RenderFamilyKind::Flowchart, ThemeTarget::ClusterLabel)
            .or_else(|| text.clone());
        let border = stroke(spec, RenderFamilyKind::Flowchart, ThemeTarget::Node);
        let line = stroke(spec, RenderFamilyKind::Flowchart, ThemeTarget::Edge)
            .or_else(|| fill(spec, RenderFamilyKind::Flowchart, ThemeTarget::Edge))
            .or_else(|| border.clone());
        let edge_label_background = fill(
            spec,
            RenderFamilyKind::Flowchart,
            ThemeTarget::EdgeLabelBackground,
        )
        .or_else(|| canvas.clone());
        let edge_label_text = fill(spec, RenderFamilyKind::Flowchart, ThemeTarget::EdgeLabel)
            .or_else(|| text.clone());
        let cluster_background = fill(spec, RenderFamilyKind::Flowchart, ThemeTarget::Cluster)
            .or_else(|| surface_alt.clone());
        let cluster_border = stroke(spec, RenderFamilyKind::Flowchart, ThemeTarget::Cluster)
            .or_else(|| border.clone());
        let note_background = fill(spec, RenderFamilyKind::Sequence, ThemeTarget::Note)
            .or_else(|| surface_alt.clone());
        let note_border =
            stroke(spec, RenderFamilyKind::Sequence, ThemeTarget::Note).or_else(|| border.clone());
        let note_text =
            fill(spec, RenderFamilyKind::Sequence, ThemeTarget::NoteLabel).or_else(|| text.clone());
        let actor_background = fill(spec, RenderFamilyKind::Sequence, ThemeTarget::Actor)
            .or_else(|| surface_alt.clone());
        let actor_border =
            stroke(spec, RenderFamilyKind::Sequence, ThemeTarget::Actor).or_else(|| border.clone());
        let actor_text = fill(spec, RenderFamilyKind::Sequence, ThemeTarget::ActorLabel)
            .or_else(|| text.clone());
        let lifeline = stroke(spec, RenderFamilyKind::Sequence, ThemeTarget::Lifeline)
            .or_else(|| actor_border.clone());
        let signal = stroke(spec, RenderFamilyKind::Sequence, ThemeTarget::Message)
            .or_else(|| fill(spec, RenderFamilyKind::Sequence, ThemeTarget::Message))
            .or_else(|| line.clone());
        let signal_text = fill(spec, RenderFamilyKind::Sequence, ThemeTarget::MessageLabel)
            .or_else(|| text.clone());
        let loop_background = fill(spec, RenderFamilyKind::Sequence, ThemeTarget::Loop)
            .or_else(|| actor_background.clone());
        let loop_border = stroke(spec, RenderFamilyKind::Sequence, ThemeTarget::Loop)
            .or_else(|| actor_border.clone());
        let loop_text = fill(spec, RenderFamilyKind::Sequence, ThemeTarget::LoopLabel)
            .or_else(|| actor_text.clone());
        let activation_border = stroke(spec, RenderFamilyKind::Sequence, ThemeTarget::Activation)
            .or_else(|| border.clone());
        let state_background =
            fill(spec, RenderFamilyKind::State, ThemeTarget::State).or_else(|| surface.clone());
        let state_border =
            stroke(spec, RenderFamilyKind::State, ThemeTarget::State).or_else(|| border.clone());
        let state_text =
            fill(spec, RenderFamilyKind::State, ThemeTarget::StateLabel).or_else(|| text.clone());
        let transition = stroke(spec, RenderFamilyKind::State, ThemeTarget::Transition)
            .or_else(|| fill(spec, RenderFamilyKind::State, ThemeTarget::Transition))
            .or_else(|| line.clone());
        let transition_text = fill(spec, RenderFamilyKind::State, ThemeTarget::TransitionLabel)
            .or_else(|| text.clone());
        let special_state = stroke_variant(
            spec,
            RenderFamilyKind::State,
            ThemeTarget::SpecialState,
            ThemeVariant::Special,
        )
        .or_else(|| {
            fill_variant(
                spec,
                RenderFamilyKind::State,
                ThemeTarget::SpecialState,
                ThemeVariant::Special,
            )
        })
        .or_else(|| line.clone());
        let composite_background = fill(spec, RenderFamilyKind::State, ThemeTarget::Composite)
            .or_else(|| canvas.clone())
            .or_else(|| surface.clone());
        let arrowhead = stroke(spec, RenderFamilyKind::Flowchart, ThemeTarget::Marker)
            .or_else(|| fill(spec, RenderFamilyKind::Flowchart, ThemeTarget::Marker))
            .or_else(|| line.clone());
        let error = stroke_variant(
            spec,
            RenderFamilyKind::Gantt,
            ThemeTarget::Task,
            ThemeVariant::Error,
        );
        let warning = stroke_variant(
            spec,
            RenderFamilyKind::Gantt,
            ThemeTarget::Task,
            ThemeVariant::Warning,
        );
        let success = stroke_variant(
            spec,
            RenderFamilyKind::Gantt,
            ThemeTarget::Task,
            ThemeVariant::Success,
        );

        Self {
            canvas: canvas.clone(),
            surface: surface.clone(),
            surface_alt: surface_alt.clone(),
            surface_muted: surface_muted.clone(),
            text,
            title,
            subtle_text,
            border,
            line,
            arrowhead,
            edge_label_background: edge_label_background.clone(),
            edge_label_text,
            commit_label_background: edge_label_background.or_else(|| surface.clone()),
            cluster_background: cluster_background.clone(),
            swimlane_background_odd: cluster_background.or(surface_muted),
            cluster_border,
            note_background,
            note_border,
            note_text,
            actor_background,
            actor_border,
            actor_text,
            lifeline,
            signal,
            signal_text,
            loop_background,
            loop_border,
            loop_text,
            activation_background,
            activation_border,
            state_background,
            state_border,
            state_text,
            transition,
            transition_text,
            special_state,
            composite_background,
            error,
            warning,
            success,
        }
    }
}

fn put_theme_roles(theme_variables: &mut Map<String, Value>, roles: &CompatibilityThemeRoles) {
    put_opt(theme_variables, "background", roles.canvas.as_deref());
    put_opt(theme_variables, "primaryColor", roles.surface.as_deref());
    put_opt(theme_variables, "mainBkg", roles.surface.as_deref());
    put_opt(
        theme_variables,
        "secondaryColor",
        roles.surface_alt.as_deref(),
    );
    put_opt(
        theme_variables,
        "tertiaryColor",
        roles.surface_muted.as_deref(),
    );
    put_opt(theme_variables, "primaryTextColor", roles.text.as_deref());
    put_opt(theme_variables, "nodeTextColor", roles.text.as_deref());
    put_opt(theme_variables, "textColor", roles.text.as_deref());
    put_opt(theme_variables, "titleColor", roles.title.as_deref());
    put_opt(
        theme_variables,
        "secondaryTextColor",
        roles.subtle_text.as_deref(),
    );
    put_opt(
        theme_variables,
        "tertiaryTextColor",
        roles.subtle_text.as_deref(),
    );
    put_opt(
        theme_variables,
        "primaryBorderColor",
        roles.border.as_deref(),
    );
    put_opt(theme_variables, "nodeBorder", roles.border.as_deref());
    put_opt(theme_variables, "lineColor", roles.line.as_deref());
    put_opt(
        theme_variables,
        "arrowheadColor",
        roles.arrowhead.as_deref(),
    );
    put_opt(
        theme_variables,
        "edgeLabelBackground",
        roles.edge_label_background.as_deref(),
    );
    put_opt(
        theme_variables,
        "clusterBkg",
        roles.cluster_background.as_deref(),
    );
    put_opt(
        theme_variables,
        "clusterBorder",
        roles.cluster_border.as_deref(),
    );
    put_opt(
        theme_variables,
        "noteBkgColor",
        roles.note_background.as_deref(),
    );
    put_opt(
        theme_variables,
        "noteBorderColor",
        roles.note_border.as_deref(),
    );
    put_opt(theme_variables, "noteTextColor", roles.note_text.as_deref());
    put_opt(
        theme_variables,
        "actorBkg",
        roles.actor_background.as_deref(),
    );
    put_opt(
        theme_variables,
        "actorBorder",
        roles.actor_border.as_deref(),
    );
    put_opt(
        theme_variables,
        "actorTextColor",
        roles.actor_text.as_deref(),
    );
    put_opt(theme_variables, "actorLineColor", roles.lifeline.as_deref());
    put_opt(theme_variables, "signalColor", roles.signal.as_deref());
    put_opt(
        theme_variables,
        "signalTextColor",
        roles.signal_text.as_deref(),
    );
    put_opt(
        theme_variables,
        "labelTextColor",
        roles.loop_text.as_deref(),
    );
    put_opt(theme_variables, "loopTextColor", roles.loop_text.as_deref());
    put_opt(
        theme_variables,
        "labelBoxBkgColor",
        roles.loop_background.as_deref(),
    );
    put_opt(
        theme_variables,
        "labelBoxBorderColor",
        roles.loop_border.as_deref(),
    );
    put_opt(
        theme_variables,
        "activationBkgColor",
        roles.activation_background.as_deref(),
    );
    put_opt(
        theme_variables,
        "activationBorderColor",
        roles.activation_border.as_deref(),
    );
    put_opt(theme_variables, "classText", roles.text.as_deref());
    put_opt(theme_variables, "labelColor", roles.text.as_deref());
    put_opt(
        theme_variables,
        "transitionColor",
        roles.transition.as_deref(),
    );
    put_opt(
        theme_variables,
        "transitionLabelColor",
        roles.transition_text.as_deref(),
    );
    put_opt(
        theme_variables,
        "stateLabelColor",
        roles.state_text.as_deref(),
    );
    put_opt(
        theme_variables,
        "stateBkg",
        roles.state_background.as_deref(),
    );
    put_opt(
        theme_variables,
        "stateBorder",
        roles.state_border.as_deref(),
    );
    put_opt(
        theme_variables,
        "specialStateColor",
        roles.special_state.as_deref(),
    );
    put_opt(
        theme_variables,
        "compositeBackground",
        roles.composite_background.as_deref(),
    );
    put_opt(
        theme_variables,
        "compositeTitleBackground",
        roles.composite_background.as_deref(),
    );
    put_opt(
        theme_variables,
        "altBackground",
        roles.surface_alt.as_deref(),
    );
    put_opt(
        theme_variables,
        "labelBackground",
        roles.edge_label_background.as_deref(),
    );
    put_opt(
        theme_variables,
        "attributeBackgroundColorOdd",
        roles.surface.as_deref(),
    );
    put_opt(
        theme_variables,
        "attributeBackgroundColorEven",
        roles.surface_alt.as_deref(),
    );
    put_opt(theme_variables, "rowOdd", roles.surface.as_deref());
    put_opt(theme_variables, "rowEven", roles.surface_alt.as_deref());
    put_opt(
        theme_variables,
        "requirementBackground",
        roles.surface.as_deref(),
    );
    put_opt(
        theme_variables,
        "requirementBorderColor",
        roles.border.as_deref(),
    );
    put_opt(
        theme_variables,
        "requirementTextColor",
        roles.text.as_deref(),
    );
    put_opt(theme_variables, "relationColor", roles.line.as_deref());
    put_opt(
        theme_variables,
        "relationLabelBackground",
        roles.edge_label_background.as_deref(),
    );
    put_opt(
        theme_variables,
        "relationLabelColor",
        roles.edge_label_text.as_deref(),
    );
    put_opt(
        theme_variables,
        "requirementEdgeLabelBackground",
        roles.edge_label_background.as_deref(),
    );
    put_opt(theme_variables, "pieTitleTextColor", roles.text.as_deref());
    put_opt(
        theme_variables,
        "pieSectionTextColor",
        roles.text.as_deref(),
    );
    put_opt(
        theme_variables,
        "pieLegendTextColor",
        roles.subtle_text.as_deref(),
    );
    put_opt(theme_variables, "pieStrokeColor", roles.border.as_deref());
    put_opt(
        theme_variables,
        "pieOuterStrokeColor",
        roles.border.as_deref(),
    );
    put_opt(theme_variables, "commitLabelColor", roles.text.as_deref());
    put_opt(
        theme_variables,
        "commitLabelBackground",
        roles.commit_label_background.as_deref(),
    );
    put_opt(theme_variables, "commitLineColor", roles.line.as_deref());
    put_opt(theme_variables, "tagLabelColor", roles.text.as_deref());
    put_opt(
        theme_variables,
        "tagLabelBackground",
        roles.surface.as_deref(),
    );
    put_opt(theme_variables, "tagLabelBorder", roles.border.as_deref());
    put_opt(theme_variables, "quadrant1Fill", roles.surface.as_deref());
    put_opt(
        theme_variables,
        "quadrant2Fill",
        roles.surface_alt.as_deref(),
    );
    put_opt(
        theme_variables,
        "quadrant3Fill",
        roles.canvas.as_deref().or(roles.surface.as_deref()),
    );
    put_opt(
        theme_variables,
        "quadrant4Fill",
        roles.surface_muted.as_deref(),
    );
    for key in [
        "quadrant1TextFill",
        "quadrant2TextFill",
        "quadrant3TextFill",
        "quadrant4TextFill",
        "quadrantPointTextFill",
        "quadrantTitleFill",
    ] {
        put_opt(theme_variables, key, roles.text.as_deref());
    }
    put_opt(theme_variables, "quadrantPointFill", roles.line.as_deref());
    put_opt(
        theme_variables,
        "quadrantXAxisTextFill",
        roles.subtle_text.as_deref(),
    );
    put_opt(
        theme_variables,
        "quadrantYAxisTextFill",
        roles.subtle_text.as_deref(),
    );
    put_opt(
        theme_variables,
        "quadrantExternalBorderStrokeFill",
        roles.border.as_deref(),
    );
    put_opt(
        theme_variables,
        "quadrantInternalBorderStrokeFill",
        roles.border.as_deref(),
    );
    put_opt(theme_variables, "archEdgeColor", roles.line.as_deref());
    put_opt(
        theme_variables,
        "archEdgeArrowColor",
        roles.arrowhead.as_deref(),
    );
    put_opt(
        theme_variables,
        "archGroupBorderColor",
        roles.cluster_border.as_deref(),
    );
    put_opt(theme_variables, "emUiFill", roles.surface.as_deref());
    put_opt(theme_variables, "emUiStroke", roles.border.as_deref());
    put_opt(theme_variables, "emRelationStroke", roles.line.as_deref());
    put_opt(theme_variables, "emArrowhead", roles.arrowhead.as_deref());
    put_opt(
        theme_variables,
        "emSwimlaneBackgroundOdd",
        roles.swimlane_background_odd.as_deref(),
    );
    put_opt(
        theme_variables,
        "emSwimlaneBackgroundStroke",
        roles.cluster_border.as_deref(),
    );
    put_opt(theme_variables, "taskTextDarkColor", roles.text.as_deref());
    put_opt(
        theme_variables,
        "taskTextClickableColor",
        roles.line.as_deref(),
    );
    put_opt(theme_variables, "taskTextColor", roles.text.as_deref());
    put_opt(
        theme_variables,
        "taskTextOutsideColor",
        roles.subtle_text.as_deref(),
    );
    put_opt(theme_variables, "taskBkgColor", roles.surface.as_deref());
    put_opt(theme_variables, "taskBorderColor", roles.border.as_deref());
    put_opt(
        theme_variables,
        "activeTaskBkgColor",
        roles.surface_muted.as_deref(),
    );
    put_opt(
        theme_variables,
        "activeTaskBorderColor",
        roles.line.as_deref(),
    );
    put_opt(
        theme_variables,
        "doneTaskBkgColor",
        roles.surface_alt.as_deref(),
    );
    put_opt(
        theme_variables,
        "doneTaskBorderColor",
        roles.success.as_deref().or(roles.border.as_deref()),
    );
    put_opt(
        theme_variables,
        "critBkgColor",
        roles.surface_alt.as_deref(),
    );
    put_opt(
        theme_variables,
        "critBorderColor",
        roles.error.as_deref().or(roles.border.as_deref()),
    );
    put_opt(
        theme_variables,
        "excludeBkgColor",
        roles.surface_alt.as_deref(),
    );
    put_opt(theme_variables, "gridColor", roles.border.as_deref());
    put_opt(
        theme_variables,
        "todayLineColor",
        roles
            .warning
            .as_deref()
            .or(roles.error.as_deref())
            .or(roles.line.as_deref()),
    );
    put_opt(
        theme_variables,
        "vertLineColor",
        roles.warning.as_deref().or(roles.line.as_deref()),
    );
    put_opt(
        theme_variables,
        "sectionBkgColor",
        roles
            .cluster_background
            .as_deref()
            .or(roles.surface_alt.as_deref()),
    );
    put_opt(
        theme_variables,
        "sectionBkgColor2",
        roles.surface_muted.as_deref(),
    );
    put_opt(
        theme_variables,
        "altSectionBkgColor",
        roles.canvas.as_deref(),
    );
    put_opt(theme_variables, "errorBkgColor", roles.error.as_deref());
    put_opt(theme_variables, "errorTextColor", roles.text.as_deref());
    put_opt(theme_variables, "faceColor", roles.surface.as_deref());
    put_opt(theme_variables, "border2", roles.cluster_border.as_deref());
}

fn put_series_palette(
    theme_variables: &mut Map<String, Value>,
    palette: &[String],
    canvas: Option<&str>,
    dark: bool,
) {
    if palette.is_empty() {
        return;
    }

    let mut xy = Map::new();
    xy.insert(
        "plotColorPalette".to_string(),
        Value::String(palette.join(",")),
    );
    xy.insert("accentColor".to_string(), Value::String(palette[0].clone()));
    theme_variables.insert("xyChart".to_string(), Value::Object(xy));

    for (index, color) in palette.iter().enumerate() {
        let label = readable_text_color(color, canvas, dark);
        put_str(theme_variables, &format!("cScale{index}"), color);
        put_str(theme_variables, &format!("cScalePeer{index}"), color);
        put_str(theme_variables, &format!("cScaleLabel{index}"), label);
        put_str(theme_variables, &format!("cScaleInv{index}"), label);
        put_str(theme_variables, &format!("git{index}"), color);
        put_str(theme_variables, &format!("gitBranchLabel{index}"), label);
        put_str(theme_variables, &format!("pie{}", index + 1), color);
        put_str(theme_variables, &format!("venn{}", index + 1), color);
        put_str(theme_variables, &format!("fillType{index}"), color);
        put_str(theme_variables, &format!("actor{index}"), color);
    }
}

fn put_diagram_config(
    root: &mut Map<String, Value>,
    theme_variables: &mut Map<String, Value>,
    roles: &CompatibilityThemeRoles,
    palette: &[String],
) {
    let mut packet = Map::new();
    put_opt(&mut packet, "startByteColor", roles.line.as_deref());
    put_opt(
        &mut packet,
        "endByteColor",
        roles.border.as_deref().or(roles.line.as_deref()),
    );
    put_opt(&mut packet, "labelColor", roles.text.as_deref());
    put_opt(&mut packet, "titleColor", roles.text.as_deref());
    put_opt(&mut packet, "blockStrokeColor", roles.border.as_deref());
    put_opt(&mut packet, "blockFillColor", roles.surface.as_deref());
    put_nonempty_object(root, "packet", packet);

    let mut treemap = Map::new();
    put_opt(&mut treemap, "titleColor", roles.text.as_deref());
    put_opt(&mut treemap, "labelColor", roles.text.as_deref());
    put_opt(&mut treemap, "valueColor", roles.subtle_text.as_deref());
    put_opt(&mut treemap, "sectionStrokeColor", roles.border.as_deref());
    put_opt(
        &mut treemap,
        "sectionFillColor",
        roles.surface_alt.as_deref(),
    );
    put_opt(&mut treemap, "leafStrokeColor", roles.border.as_deref());
    put_opt(&mut treemap, "leafFillColor", roles.surface.as_deref());
    put_nonempty_object(root, "treemap", treemap);

    let mut tree_view = Map::new();
    put_opt(&mut tree_view, "labelColor", roles.text.as_deref());
    put_opt(&mut tree_view, "lineColor", roles.line.as_deref());
    if !tree_view.is_empty() {
        let mut merged = theme_variables
            .get("treeView")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        merge_object(&mut merged, &tree_view);
        theme_variables.insert("treeView".to_string(), Value::Object(merged));
    }

    let mut radar = Map::new();
    put_opt(&mut radar, "axisColor", roles.line.as_deref());
    put_opt(&mut radar, "graticuleColor", roles.border.as_deref());
    put_nonempty_object(root, "radar", radar);

    let mut eventmodeling = Map::new();
    put_opt(
        &mut eventmodeling,
        "emProcessorFill",
        palette
            .get(3)
            .map(String::as_str)
            .or(roles.surface_alt.as_deref()),
    );
    put_opt(
        &mut eventmodeling,
        "emProcessorStroke",
        roles.border.as_deref(),
    );
    put_opt(
        &mut eventmodeling,
        "emReadModelFill",
        palette
            .get(1)
            .map(String::as_str)
            .or(roles.success.as_deref())
            .or(roles.surface_alt.as_deref()),
    );
    put_opt(
        &mut eventmodeling,
        "emReadModelStroke",
        roles.success.as_deref().or(roles.border.as_deref()),
    );
    put_opt(
        &mut eventmodeling,
        "emCommandFill",
        palette
            .first()
            .map(String::as_str)
            .or(roles.surface_alt.as_deref()),
    );
    put_opt(
        &mut eventmodeling,
        "emCommandStroke",
        roles.line.as_deref().or(roles.border.as_deref()),
    );
    put_opt(
        &mut eventmodeling,
        "emEventFill",
        palette
            .get(2)
            .map(String::as_str)
            .or(roles.warning.as_deref())
            .or(roles.surface_alt.as_deref()),
    );
    put_opt(
        &mut eventmodeling,
        "emEventStroke",
        roles.warning.as_deref().or(roles.border.as_deref()),
    );
    for (key, value) in eventmodeling {
        theme_variables.insert(key, value);
    }

    let mut c4 = Map::new();
    for prefix in [
        "person",
        "system",
        "system_db",
        "system_queue",
        "container",
        "container_db",
        "container_queue",
        "component",
        "component_db",
        "component_queue",
        "external_person",
        "external_system",
        "external_system_db",
        "external_system_queue",
        "external_container",
        "external_container_db",
        "external_container_queue",
        "external_component",
        "external_component_db",
        "external_component_queue",
    ] {
        put_opt(
            &mut c4,
            &format!("{prefix}_bg_color"),
            roles.surface.as_deref(),
        );
        put_opt(
            &mut c4,
            &format!("{prefix}_border_color"),
            roles.border.as_deref(),
        );
    }
    put_nonempty_object(root, "c4", c4);
}

fn chart_palette(spec: &DiagramThemeSpec) -> Vec<String> {
    spec.styles()
        .ordinal_palettes()
        .iter()
        .find(|(target, _)| *target == ThemeTarget::ChartSeries)
        .map(|(_, palette)| {
            palette
                .colors()
                .iter()
                .map(|color| color.as_css())
                .collect()
        })
        .unwrap_or_default()
}

fn fill(spec: &DiagramThemeSpec, family: RenderFamilyKind, target: ThemeTarget) -> Option<String> {
    fill_variant(spec, family, target, ThemeVariant::Default)
}

fn stroke(
    spec: &DiagramThemeSpec,
    family: RenderFamilyKind,
    target: ThemeTarget,
) -> Option<String> {
    stroke_variant(spec, family, target, ThemeVariant::Default)
}

fn fill_variant(
    spec: &DiagramThemeSpec,
    family: RenderFamilyKind,
    target: ThemeTarget,
    variant: ThemeVariant,
) -> Option<String> {
    resolve_style(spec, family, target, variant, None)
        .fill()
        .and_then(solid_paint)
}

fn stroke_variant(
    spec: &DiagramThemeSpec,
    family: RenderFamilyKind,
    target: ThemeTarget,
    variant: ThemeVariant,
) -> Option<String> {
    resolve_style(spec, family, target, variant, None)
        .stroke()
        .and_then(solid_paint)
}

fn solid_paint(paint: &CanvasPaint) -> Option<String> {
    match paint {
        CanvasPaint::Solid(color) => Some(color.as_css()),
        CanvasPaint::Transparent => Some("transparent".to_string()),
        CanvasPaint::LinearGradient(_)
        | CanvasPaint::RadialGradient(_)
        | CanvasPaint::Pattern(_) => None,
    }
}

fn put_nonempty_object(root: &mut Map<String, Value>, key: &str, object: Map<String, Value>) {
    if !object.is_empty() {
        root.insert(key.to_string(), Value::Object(object));
    }
}

fn put_opt(map: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        put_str(map, key, value);
    }
}

fn put_str(map: &mut Map<String, Value>, key: &str, value: &str) {
    map.insert(key.to_string(), Value::String(value.trim().to_string()));
}

fn merge_object(target: &mut Map<String, Value>, source: &Map<String, Value>) {
    for (key, value) in source {
        match (target.get_mut(key), value) {
            (Some(Value::Object(target)), Value::Object(source)) => merge_object(target, source),
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

fn readable_text_color(color: &str, canvas: Option<&str>, dark: bool) -> &'static str {
    let Ok(color) = ThemeColor::parse(color.trim()) else {
        return "#ffffff";
    };
    let fallback = if dark { [0.0; 3] } else { [1.0; 3] };
    let canvas = canvas
        .and_then(|canvas| ThemeColor::parse(canvas.trim()).ok())
        .map_or(fallback, |canvas| composite_over(&canvas, fallback));
    let [red, green, blue] = composite_over(&color, canvas);
    let luminance = relative_luminance(red, green, blue);
    let black_contrast = (luminance + 0.05) / 0.05;
    let white_contrast = 1.05 / (luminance + 0.05);
    if black_contrast >= white_contrast {
        "#000000"
    } else {
        "#ffffff"
    }
}

fn composite_over(color: &ThemeColor, background: [f64; 3]) -> [f64; 3] {
    let alpha = color.channel(ColorChannel::Alpha);
    let foreground = [
        color.channel(ColorChannel::Red) / 255.0,
        color.channel(ColorChannel::Green) / 255.0,
        color.channel(ColorChannel::Blue) / 255.0,
    ];
    std::array::from_fn(|index| foreground[index] * alpha + background[index] * (1.0 - alpha))
}

fn relative_luminance(r: f64, g: f64, b: f64) -> f64 {
    fn linear(channel: f64) -> f64 {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{DiagramThemeCompiler, ThemePreset};

    #[test]
    fn palette_readability_composites_transparency_against_the_canvas() {
        assert_eq!(
            readable_text_color("rgb(255 255 255 / .2)", Some("#000000"), true),
            "#ffffff"
        );
        assert_eq!(
            readable_text_color("rgb(0 0 0 / .2)", Some("#ffffff"), false),
            "#000000"
        );
    }

    #[test]
    fn dark_preset_projects_sequence_and_state_text_roles() {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(ThemePreset::EditorDark)
            .expect("editor dark theme");
        let variables = &theme.mermaid_config().as_value()["themeVariables"];

        assert_eq!(variables["actorTextColor"], "#e5e7eb");
        assert_eq!(variables["signalTextColor"], "#e5e7eb");
        assert_eq!(variables["noteTextColor"], "#fef3c7");
        assert_eq!(variables["stateLabelColor"], "#e5e7eb");
        assert_eq!(variables["transitionLabelColor"], "#e5e7eb");
    }
}
