use std::{borrow::Cow, ops::Range, sync::Arc};

use merman_markup::theme_color::ThemeColor;
use merman_model::{
    OperationPhase,
    families::sequence::{
        SequenceCentralDecoration, SequenceMessage, SequenceMessageKind, SequenceMessageMarker,
        SequenceMessageStroke,
    },
    presentation::PresentationConfig,
};
use merman_render_core::{
    paint_scene::{
        self, PaintScene, SceneBox, SceneColor, SceneFontFamily, SceneGroup, SceneItem,
        SceneMarker, SceneMarkerOrient, SceneMarkerRef, SceneMarkerUnits, SceneMetadata,
        ScenePaint, ScenePath, ScenePathCommand, SceneRect, SceneRole, SceneStroke, SceneText,
        SceneTextAnchor, SceneTextBaseline, SceneTextStyle, SceneViewport,
    },
    resources::{OperationWorkError, PreparedTextRetainedReservation},
    svg_css::SvgId,
    text::{FontStyle, ParsedCssFontStack, parse_css_font_stack},
};
use merman_render_sequence::rect_paint::include_rect_stroke_bounds;
use merman_theme_contract::v2::{Elevation, MarkerStyle, Paint, Texture};
use rustc_hash::FxHashMap;

use super::{
    SequenceEmitCheckpoints, actor_effect,
    arrowhead::{ARROWHEAD_REFERENCE, ARROWHEAD_SIZE, sequence_arrowhead},
    css,
    geometry::{
        actor_label_dy, lifeline_endpoints, message_label_x, node_left_top, note_label_position,
    },
    messages::SequenceMessagePaintPlan,
    model::SequenceSvgModel,
    root::sequence_root_bounds,
    settings::SequenceRenderSettings,
};
use crate::{
    DiagramFamilyId, Error,
    model::{LayoutEdge, LayoutNode},
    resources::OperationWorkMeter,
    sequence::{
        SequenceDrawnTextNode, SequencePreparedArtifact, SequenceTerminalTextStyle,
        SequenceTextSurface, measure_sequence_drawn_line_height, prepare_sequence_text_line,
        sequence_drawn_text_style, sequence_drawn_text_y,
    },
    svg::parity::{SvgExecution, root_svg::RootViewportSpec},
    text::split_mermaid_common_br_lines,
};

const MAX_OPAQUE_TOKEN_BYTES: usize = 8 * 1024;

#[derive(Debug)]
pub(in crate::svg::parity) enum SequenceCompactError {
    Unsupported(&'static str),
    Render(Error),
}

impl From<Error> for SequenceCompactError {
    fn from(error: Error) -> Self {
        Self::Render(error)
    }
}

impl From<OperationWorkError> for SequenceCompactError {
    fn from(error: OperationWorkError) -> Self {
        Self::Render(error.into())
    }
}

type Result<T> = std::result::Result<T, SequenceCompactError>;

fn invalid(message: &'static str) -> SequenceCompactError {
    Error::InvalidModel {
        message: message.to_owned(),
    }
    .into()
}

fn unsupported(meter: &OperationWorkMeter, reason: &'static str) -> SequenceCompactError {
    meter
        .checkpoint(OperationPhase::Emit)
        .err()
        .map_or(SequenceCompactError::Unsupported(reason), Into::into)
}

fn admit_token(raw: &str, meter: &OperationWorkMeter) -> Result<()> {
    if raw.len() > MAX_OPAQUE_TOKEN_BYTES {
        return Err(unsupported(meter, "projection-token-bytes"));
    }
    meter.charge_emit_work(1 + raw.len().div_ceil(32))?;
    Ok(())
}

fn has_authored_css(raw: &str, meter: &OperationWorkMeter) -> Result<bool> {
    meter.charge_emit_work(raw.len().div_ceil(64))?;
    let mut start = None;
    let mut end = 0;
    let mut next_check = 64;
    for (index, character) in raw.char_indices() {
        if index >= next_check {
            meter.checkpoint(OperationPhase::Emit)?;
            next_check = index.saturating_add(64);
        }
        if !character.is_whitespace() {
            start.get_or_insert(index);
            end = index + character.len_utf8();
        }
    }
    meter.checkpoint(OperationPhase::Emit)?;
    Ok(start.is_some_and(|start| &raw[start..end] != "{ /* ERROR: Unbalanced CSS */ }"))
}

#[derive(Clone, Copy)]
enum TextRole {
    Actor = 0,
    Message = 1,
    Note = 2,
}

struct TerminalFont {
    stack: ParsedCssFontStack,
    fill: SceneColor,
    size: f64,
    weight: u16,
    style: FontStyle,
}

impl TerminalFont {
    fn resolve(
        terminal: SequenceTerminalTextStyle<'_>,
        meter: &OperationWorkMeter,
    ) -> Result<Self> {
        let style = terminal.text_style;
        let family = style
            .font_family
            .as_deref()
            .ok_or_else(|| unsupported(meter, "font-family"))?;
        admit_token(family, meter)?;
        let stack =
            parse_css_font_stack(family).ok_or_else(|| unsupported(meter, "font-family"))?;
        meter.checkpoint(OperationPhase::Emit)?;
        for value in [style.font_weight.as_deref(), style.font_style.as_deref()]
            .into_iter()
            .flatten()
        {
            admit_token(value, meter)?;
        }
        let weight = match style.font_weight.as_deref().map(str::trim) {
            None | Some("normal") => 400,
            Some("bold") => 700,
            Some(raw) => raw
                .parse::<u16>()
                .ok()
                .filter(|weight| (1..=1000).contains(weight))
                .ok_or_else(|| unsupported(meter, "font-weight"))?,
        };
        let font_style = match style.font_style.as_deref().map(str::trim) {
            None | Some("normal") => FontStyle::Normal,
            Some("italic") => FontStyle::Italic,
            Some("oblique") => FontStyle::Oblique,
            _ => return Err(unsupported(meter, "font-style")),
        };
        Ok(Self {
            stack,
            fill: resolved_color(terminal.foreground, meter)?,
            size: style.font_size,
            weight,
            style: font_style,
        })
    }

    fn families(&self) -> Vec<SceneFontFamily<'_>> {
        self.stack
            .resolved_families()
            .map(|(name, generic)| {
                generic.map_or(SceneFontFamily::Named(name), SceneFontFamily::Generic)
            })
            .collect()
    }

    fn scene_style<'a>(&self, families: &'a [SceneFontFamily<'a>]) -> SceneTextStyle<'a> {
        SceneTextStyle {
            families,
            fill: self.fill,
            size: self.size,
            weight: Some(self.weight),
            style: self.style,
        }
    }
}

struct Label<'a> {
    text: Cow<'a, str>,
    _retained: Option<PreparedTextRetainedReservation>,
}

impl<'a> Label<'a> {
    fn decoded(raw: &'a str, meter: &Arc<OperationWorkMeter>) -> Result<Self> {
        Self::prepare(raw, meter, prepare_sequence_text_line)
    }

    fn prepare(
        raw: &'a str,
        meter: &Arc<OperationWorkMeter>,
        decode: impl FnOnce(&'a str) -> Cow<'a, str>,
    ) -> Result<Self> {
        admit_token(raw, meter)?;
        let text = decode(raw);
        meter.checkpoint(OperationPhase::Emit)?;
        let retained = match &text {
            Cow::Borrowed(_) => None,
            Cow::Owned(text) => {
                let reservation = meter
                    .reserve_prepared_text_retained_bytes(text.capacity())
                    .map_err(|error| {
                        meter.terminate_absolute_resource_error(error, OperationPhase::Emit)
                    })?;
                Some(reservation)
            }
        };
        Ok(Self {
            text,
            _retained: retained,
        })
    }
}

enum Item<'a> {
    Rect(SceneRect),
    Line {
        points: [ScenePathCommand; 2],
        stroke: SceneStroke,
        start: Option<SceneMarkerRef>,
        end: Option<SceneMarkerRef>,
    },
    Text {
        label: Label<'a>,
        role: TextRole,
        position: [f64; 2],
        dy: f64,
        anchor: SceneTextAnchor,
        baseline: SceneTextBaseline,
    },
}

impl Item<'_> {
    fn view<'a>(&'a self, styles: &[SceneTextStyle<'a>; 3]) -> SceneItem<'a> {
        match self {
            Self::Rect(rect) => SceneItem::Rect(*rect),
            Self::Line {
                points,
                stroke,
                start,
                end,
            } => SceneItem::Path(ScenePath {
                commands: points,
                paint: ScenePaint {
                    stroke: Some(*stroke),
                    fill: None,
                },
                start_marker: *start,
                end_marker: *end,
            }),
            Self::Text {
                label,
                role,
                position,
                dy,
                anchor,
                baseline,
            } => SceneItem::Text(SceneText {
                text: &label.text,
                position: *position,
                dy: *dy,
                anchor: *anchor,
                baseline: *baseline,
                style: styles[*role as usize],
            }),
        }
    }
}

struct Projected<'a> {
    meter: &'a Arc<OperationWorkMeter>,
    items: Vec<Item<'a>>,
    groups: Vec<(SceneRole, Range<usize>)>,
    uses_arrow: bool,
}

impl<'a> Projected<'a> {
    fn item(&mut self, item: Item<'a>) -> Result<()> {
        self.meter.charge_emit_work(1)?;
        self.items
            .try_reserve(1)
            .map_err(|_| invalid("unable to allocate Sequence compact items"))?;
        self.items.push(item);
        Ok(())
    }

    fn group(&mut self, role: SceneRole, start: usize) -> Result<()> {
        if start < self.items.len() {
            self.meter.charge_emit_work(1)?;
            self.groups
                .try_reserve(1)
                .map_err(|_| invalid("unable to allocate Sequence compact groups"))?;
            self.groups.push((role, start..self.items.len()));
        }
        Ok(())
    }

    fn line(
        &mut self,
        from: [f64; 2],
        to: [f64; 2],
        stroke: SceneStroke,
        [start, end]: [Option<SceneMarkerRef>; 2],
    ) -> Result<()> {
        self.uses_arrow |= start.is_some() || end.is_some();
        self.item(Item::Line {
            points: [ScenePathCommand::MoveTo(from), ScenePathCommand::LineTo(to)],
            stroke,
            start,
            end,
        })
    }

    fn views<'view>(
        &'view self,
        styles: &[SceneTextStyle<'view>; 3],
    ) -> Result<Vec<SceneItem<'view>>> {
        self.meter.charge_emit_work(self.items.len())?;
        let mut views = Vec::new();
        views
            .try_reserve_exact(self.items.len())
            .map_err(|_| invalid("unable to allocate Sequence compact item views"))?;
        let checks = SequenceEmitCheckpoints::for_emit(self.meter);
        for (index, item) in self.items.iter().enumerate() {
            checks.checkpoint_loop(index)?;
            views.push(item.view(styles));
        }
        Ok(views)
    }

    fn groups<'view>(&self, items: &'view [SceneItem<'view>]) -> Result<Vec<SceneGroup<'view>>> {
        self.meter.charge_emit_work(self.groups.len())?;
        let mut groups = Vec::new();
        groups
            .try_reserve_exact(self.groups.len())
            .map_err(|_| invalid("unable to allocate Sequence compact group views"))?;
        let checks = SequenceEmitCheckpoints::for_emit(self.meter);
        for (index, (role, range)) in self.groups.iter().enumerate() {
            checks.checkpoint_loop(index)?;
            groups.push(SceneGroup {
                id: None,
                role: *role,
                items: &items[range.clone()],
            });
        }
        Ok(groups)
    }
}

fn resolved_color(raw: &str, meter: &OperationWorkMeter) -> Result<SceneColor> {
    admit_token(raw, meter)?;
    let color = ThemeColor::parse(raw.trim())
        .map_err(|_| unsupported(meter, "unresolved-paint"))?
        .rgba_channels();
    meter.checkpoint(OperationPhase::Emit)?;
    Ok(color)
}

fn solid(color: crate::theme::Color) -> SceneColor {
    let [red, green, blue, alpha] = color.channels().map(f64::from);
    SceneColor {
        red,
        green,
        blue,
        alpha: alpha / 255.0,
    }
}

fn compact_canvas(options: &SvgExecution<'_>) -> Result<Option<SceneColor>> {
    let session = options.family.session();
    let background = session.canvas_background().map(solid);
    let Some(theme) = session.native_theme() else {
        return Ok(background);
    };
    if theme
        .resolved()
        .canvas()
        .textures
        .iter()
        .any(|texture| !matches!(texture, Texture::None))
    {
        return Err(unsupported(options.work_meter(), "canvas-texture"));
    }
    if background.is_some() {
        return Ok(background);
    }
    match &theme.resolved().family("sequence").roles.canvas {
        Paint::Solid(color) => Ok(Some(solid(*color))),
        _ => Err(unsupported(options.work_meter(), "canvas-paint")),
    }
}

fn bounds(node: &LayoutNode) -> SceneBox {
    let (x, y) = node_left_top(node);
    SceneBox {
        x,
        y,
        width: node.width,
        height: node.height,
    }
}

fn stroke(color: SceneColor, width: f64) -> SceneStroke {
    SceneStroke {
        color,
        width,
        dash: None,
    }
}

fn surface(fill: &str, border: &str, width: f64, meter: &OperationWorkMeter) -> Result<ScenePaint> {
    Ok(ScenePaint {
        fill: Some(resolved_color(fill, meter)?),
        stroke: Some(stroke(resolved_color(border, meter)?, width)),
    })
}

fn admit_text(raw: &str, wrapped: bool, meter: &OperationWorkMeter) -> Result<()> {
    admit_token(raw, meter)?;
    if wrapped {
        return Err(unsupported(meter, "soft-wrap"));
    }
    if raw.contains("$$") {
        return Err(unsupported(meter, "math-label"));
    }
    Ok(())
}

fn hard_breaks<'text>(raw: &'text str, meter: &OperationWorkMeter) -> Result<Vec<&'text str>> {
    admit_token(raw, meter)?;
    let lines = split_mermaid_common_br_lines(raw);
    meter.checkpoint(OperationPhase::Emit)?;
    Ok(lines)
}

fn actor<'a>(
    projected: &mut Projected<'a>,
    node: &LayoutNode,
    description: &'a str,
    radius: f64,
    paint: ScenePaint,
    size: f64,
) -> Result<()> {
    projected.item(Item::Rect(SceneRect {
        bounds: bounds(node),
        radius,
        paint,
    }))?;
    let lines = hard_breaks(description, projected.meter)?;
    let line_count = lines.len();
    for (index, raw) in lines.into_iter().enumerate() {
        let label = Label::decoded(raw, projected.meter)?;
        if !label.text.is_empty() {
            projected.item(Item::Text {
                label,
                role: TextRole::Actor,
                position: [node.x, node.y],
                dy: actor_label_dy(index, line_count, size),
                anchor: SceneTextAnchor::Middle,
                baseline: SceneTextBaseline::Central,
            })?;
        }
    }
    Ok(())
}

struct DrawnRows {
    role: TextRole,
    origin: [f64; 2],
    anchor: SceneTextAnchor,
    margin: f64,
}

fn drawn_rows<'a>(
    projected: &mut Projected<'a>,
    raw: &'a str,
    rows: DrawnRows,
    prepared: &SequencePreparedArtifact,
    config: &PresentationConfig,
    options: &SvgExecution<'_>,
) -> Result<()> {
    let typography = prepared.typography();
    let (role, node) = match rows.role {
        TextRole::Note => (typography.note(), SequenceDrawnTextNode::Tspan),
        _ => (typography.message(), SequenceDrawnTextNode::Direct),
    };
    let style = role.measurement_style();
    let drawn = sequence_drawn_text_style(style, config);
    let checkpoints = SequenceEmitCheckpoints::for_emit(options.work_meter());
    let mut preceding_height = 0.0;
    for raw_line in hard_breaks(raw, projected.meter)? {
        let mut label = Label::decoded(raw_line, projected.meter)?;
        if label.text.is_empty() {
            label.text = Cow::Borrowed("\u{200b}");
        }
        let y = sequence_drawn_text_y(rows.origin[1], rows.margin, preceding_height);
        if rows.margin > 0.0 {
            preceding_height += measure_sequence_drawn_line_height(
                options.text_measurer(),
                &label.text,
                &drawn,
                node,
                checkpoints.text(),
            )?;
        }
        projected.item(Item::Text {
            label,
            role: rows.role,
            position: [rows.origin[0], y],
            dy: role.terminal_text_style().font_size,
            anchor: rows.anchor,
            baseline: SceneTextBaseline::Middle,
        })?;
    }
    Ok(())
}

fn allow_message(message: &SequenceMessage, meter: &OperationWorkMeter) -> Result<()> {
    match message.semantic_kind() {
        SequenceMessageKind::Signal => {
            if message.from == message.to {
                return Err(unsupported(meter, "self-message"));
            }
            if message.central_decoration() != Some(SequenceCentralDecoration::None) {
                return Err(unsupported(meter, "central-decoration"));
            }
            let semantics = message
                .signal_semantics()
                .ok_or_else(|| invalid("Sequence compact signal lacks semantics"))?;
            if [semantics.source_marker, semantics.target_marker]
                .into_iter()
                .any(|marker| {
                    !matches!(
                        marker,
                        SequenceMessageMarker::None | SequenceMessageMarker::Filled
                    )
                })
            {
                return Err(unsupported(meter, "signal-marker"));
            }
            admit_text(message.message_text(), message.wrap, meter)
        }
        SequenceMessageKind::Note => admit_text(message.message_text(), message.wrap, meter),
        SequenceMessageKind::ActivationStart | SequenceMessageKind::ActivationEnd => Ok(()),
        SequenceMessageKind::Autonumber => Err(unsupported(meter, "sequence-numbers")),
        _ => Err(unsupported(meter, "sequence-control")),
    }
}

pub(in crate::svg::parity) fn render_sequence_compact(
    prepared: &SequencePreparedArtifact,
    model: &SequenceSvgModel,
    config: &PresentationConfig,
    options: &SvgExecution<'_>,
) -> Result<String> {
    let sidecar = prepare_sequence_compact(prepared, model, config, options)?;
    sidecar.write_svg(options.diagram_id_or("merman"))
}

struct SequenceCompactPrepared<'a> {
    viewport: SceneViewport,
    title: Option<Label<'a>>,
    description: Option<Label<'a>>,
    marker: SceneMarker<'static>,
    fonts: [TerminalFont; 3],
    projected: Projected<'a>,
}

impl SequenceCompactPrepared<'_> {
    fn write_svg(&self, diagram_id: SvgId<'_>) -> Result<String> {
        let meter = self.projected.meter;
        meter.checkpoint(OperationPhase::Emit)?;
        let families = self.fonts.each_ref().map(TerminalFont::families);
        let [actor_families, message_families, note_families] = &families;
        let [actor_font, message_font, note_font] = &self.fonts;
        let styles = [
            actor_font.scene_style(actor_families),
            message_font.scene_style(message_families),
            note_font.scene_style(note_families),
        ];
        let items = self.projected.views(&styles)?;
        let groups = self.projected.groups(&items)?;
        let scene = PaintScene {
            viewport: self.viewport,
            metadata: SceneMetadata {
                title: self.title.as_ref().map(|label| label.text.as_ref()),
                description: self.description.as_ref().map(|label| label.text.as_ref()),
            },
            markers: if self.projected.uses_arrow {
                std::slice::from_ref(&self.marker)
            } else {
                &[]
            },
            groups: &groups,
        };
        Ok(paint_scene::write_svg(&scene, diagram_id, meter)?)
    }
}

fn prepare_sequence_compact<'a>(
    prepared: &'a SequencePreparedArtifact,
    model: &'a SequenceSvgModel,
    config: &'a PresentationConfig,
    options: &'a SvgExecution<'_>,
) -> Result<SequenceCompactPrepared<'a>> {
    let meter = options.family.session().work_meter();
    let checks = SequenceEmitCheckpoints::for_emit(meter);
    checks.checkpoint()?;
    let model = prepared.model_for_render(model);
    let layout = prepared.layout();
    let compat = prepared.typography().compat_binding();
    let native = prepared.native();
    if let Some(css) = config.get_str("themeCSS")
        && has_authored_css(css, meter)?
    {
        if !options.family.session().allows_trusted_theme_css() {
            return Err(Error::UntrustedThemeCss.into());
        }
        return Err(unsupported(meter, "authored-css"));
    }
    if compat.actor_palette.enabled()
        || crate::config::config_diagram_look(config.as_value()).as_str() != "classic"
    {
        return Err(unsupported(meter, "sequence-look-or-palette"));
    }
    if native.is_some_and(|theme| !matches!(theme.elevation, Elevation::Flat)) {
        return Err(unsupported(meter, "sequence-elevation"));
    }
    if config.get_bool("sequence.showSequenceNumbers") == Some(true) {
        return Err(unsupported(meter, "sequence-numbers"));
    }
    if !model.boxes.is_empty() || !layout.frames.is_empty() || layout.title.is_some() {
        return Err(unsupported(meter, "box-frame-or-title"));
    }
    for (index, actor_id) in model.actor_order.iter().enumerate() {
        checks.checkpoint_loop(index)?;
        admit_token(actor_id, meter)?;
        let actor = model
            .actors
            .get(actor_id)
            .ok_or_else(|| invalid("Sequence compact actor lookup failed"))?;
        if actor.actor_type != "participant"
            || !actor.properties.is_empty()
            || !actor.links.is_empty()
            || model.actor_lifecycle_at(index).is_some()
        {
            return Err(unsupported(meter, "actor-kind-or-interaction"));
        }
        admit_text(&actor.description, actor.wrap, meter)?;
    }
    for (index, message) in model.messages.iter().enumerate() {
        checks.checkpoint_loop(index)?;
        admit_token(&message.id, meter)?;
        allow_message(message, meter)?;
    }

    let canvas = compact_canvas(options)?;
    let has_root_canvas =
        native.is_some() || options.family.session().canvas_background().is_some();
    let settings =
        SequenceRenderSettings::from_resolved_typography(config.as_value(), prepared.typography());
    let native_width = native.map(|theme| theme.form.stroke_width as f32);
    let native_radius = native.map(|theme| theme.form.radius as f32);
    let connector_width = native.map(|theme| theme.form.connector_width as f32);
    let header_radius = native_radius.map_or(3.0, f64::from);
    let overlay_radius = native_radius.map_or(0.0, f64::from);
    let overlay_width = native_width.map_or(1.0, f64::from);
    let header_width = native_width
        .map(f64::from)
        .or(compat.native_stroke_width)
        .ok_or_else(|| unsupported(meter, "actor-stroke-width"))?;
    let actor_paint = surface(
        &compat.actor_fill,
        &compat.actor_border,
        header_width,
        meter,
    )?;
    let note_paint = surface(&compat.note_fill, &compat.note_border, overlay_width, meter)?;
    let activation_paint = surface(
        &compat.activation_fill,
        &compat.activation_border,
        overlay_width,
        meter,
    )?;
    let signal_color = resolved_color(&compat.signal_color, meter)?;
    let lifeline = stroke(
        resolved_color(&compat.actor_line, meter)?,
        css::lifeline_effective_stroke_width(native_width),
    );
    let surfaces = [
        SequenceTextSurface::ParticipantLabel,
        SequenceTextSurface::MessageLabel,
        SequenceTextSurface::NoteLabel,
    ];
    let [actor_font, message_font, note_font] = surfaces;
    let fonts = [
        TerminalFont::resolve(prepared.typography().terminal_text_style(actor_font), meter)?,
        TerminalFont::resolve(
            prepared.typography().terminal_text_style(message_font),
            meter,
        )?,
        TerminalFont::resolve(prepared.typography().terminal_text_style(note_font), meter)?,
    ];

    let mut nodes: FxHashMap<&str, &LayoutNode> = FxHashMap::default();
    meter.charge_emit_work(layout.nodes.len())?;
    nodes
        .try_reserve(layout.nodes.len())
        .map_err(|_| invalid("unable to allocate Sequence compact node lookup"))?;
    for (index, node) in layout.nodes.iter().enumerate() {
        checks.checkpoint_loop(index)?;
        admit_token(&node.id, meter)?;
        nodes.insert(&node.id, node);
    }
    let mut edges: FxHashMap<&str, &LayoutEdge> = FxHashMap::default();
    meter.charge_emit_work(layout.edges.len())?;
    edges
        .try_reserve(layout.edges.len())
        .map_err(|_| invalid("unable to allocate Sequence compact edge lookup"))?;
    for (index, edge) in layout.edges.iter().enumerate() {
        checks.checkpoint_loop(index)?;
        admit_token(&edge.id, meter)?;
        edges.insert(&edge.id, edge);
    }

    let mut projected = Projected {
        meter,
        items: Vec::new(),
        groups: Vec::new(),
        uses_arrow: false,
    };
    let mut lifeline_bounds = None;
    for placement in if settings.mirror_actors {
        &["bottom", "top"][..]
    } else {
        &["top"][..]
    } {
        for (index, id) in model.actor_order.iter().rev().enumerate() {
            checks.checkpoint_loop(index)?;
            let participant = &model.actors[id];
            let node = nodes
                .get(format!("actor-{placement}-{id}").as_str())
                .ok_or_else(|| invalid("Sequence compact actor is missing prepared placement"))?;
            let start = projected.items.len();
            if *placement == "top" {
                let bottom = nodes
                    .get(format!("actor-bottom-{id}").as_str())
                    .ok_or_else(|| invalid("Sequence compact actor is missing its lifeline end"))?;
                let (y1, y2) = lifeline_endpoints(
                    node,
                    bottom,
                    edges.get(format!("lifeline-{id}").as_str()).copied(),
                );
                if has_root_canvas {
                    actor_effect::include_lifeline_paint_bounds(
                        &mut lifeline_bounds,
                        node.x,
                        y1,
                        y2,
                        lifeline.width,
                    );
                }
                projected.line([node.x, y1], [node.x, y2], lifeline, [None, None])?;
            }
            actor(
                &mut projected,
                node,
                &participant.description,
                header_radius,
                actor_paint,
                settings.actor_text_style.font_size,
            )?;
            projected.group(SceneRole::Node, start)?;
        }
    }

    let geometry = prepared.activation_geometry();
    let mut activation_bounds = None;
    let mut note_bounds = None;
    for (index, message) in model.messages.iter().enumerate() {
        checks.checkpoint_loop(index)?;
        if let Some(group) = geometry.group_for_start(&message.id).flatten() {
            let activation = layout
                .activations
                .get(group.layout_index)
                .ok_or_else(|| invalid("Sequence compact activation has a missing rectangle"))?;
            if let Some(width) = native_width {
                include_rect_stroke_bounds(
                    &mut activation_bounds,
                    [
                        activation.x,
                        activation.y,
                        activation.width,
                        activation.height,
                    ],
                    f64::from(width),
                );
            }
            let start = projected.items.len();
            projected.item(Item::Rect(SceneRect {
                bounds: SceneBox {
                    x: activation.x,
                    y: activation.y,
                    width: activation.width,
                    height: activation.height,
                },
                radius: overlay_radius,
                paint: activation_paint,
            }))?;
            projected.group(SceneRole::Activation, start)?;
        }
        if message.semantic_kind() == SequenceMessageKind::Note {
            let note = nodes
                .get(format!("note-{}", message.id).as_str())
                .ok_or_else(|| invalid("Sequence compact note has a missing rectangle"))?;
            let rect = bounds(note);
            let margin = settings.note_margin;
            if let Some(width) = native_width {
                include_rect_stroke_bounds(
                    &mut note_bounds,
                    [rect.x, rect.y, rect.width, rect.height],
                    f64::from(width),
                );
            }
            let (origin, anchor) = note_label_position(
                rect.x,
                rect.y,
                rect.width,
                margin,
                config.get_str("sequence.noteAlign"),
            );
            let start = projected.items.len();
            projected.item(Item::Rect(SceneRect {
                bounds: rect,
                radius: overlay_radius,
                paint: note_paint,
            }))?;
            drawn_rows(
                &mut projected,
                message.message_text(),
                DrawnRows {
                    role: TextRole::Note,
                    origin,
                    anchor: scene_anchor(anchor),
                    margin,
                },
                prepared,
                config,
                options,
            )?;
            projected.group(SceneRole::Note, start)?;
        }
    }

    for (index, message) in model.messages.iter().enumerate() {
        checks.checkpoint_loop(index)?;
        let Some(semantics) = message.signal_semantics() else {
            continue;
        };
        let edge = edges
            .get(format!("msg-{}", message.id).as_str())
            .ok_or_else(|| invalid("Sequence compact signal has a missing route"))?;
        let [from, to, ..] = edge.points.as_slice() else {
            return Err(invalid("Sequence compact signal lacks directed endpoints"));
        };
        if !message.message_text().is_empty() {
            let label = edge
                .label
                .as_ref()
                .ok_or_else(|| invalid("Sequence compact signal lacks its prepared label"))?;
            let margin = settings.wrap_padding;
            let (x, anchor) = message_label_x(
                label.x,
                from.x,
                to.x,
                (from.x - to.x).abs().max(0.0),
                margin,
                &settings.message_align,
            );
            let start = projected.items.len();
            drawn_rows(
                &mut projected,
                message.message_text(),
                DrawnRows {
                    role: TextRole::Message,
                    origin: [x.round(), label.y],
                    anchor: scene_anchor(anchor),
                    margin,
                },
                prepared,
                config,
                options,
            )?;
            projected.group(SceneRole::EdgeLabel, start)?;
        }
        let markers = [semantics.source_marker, semantics.target_marker]
            .map(|marker| (marker == SequenceMessageMarker::Filled).then_some(SceneMarkerRef(0)));
        let start = projected.items.len();
        projected.line(
            [from.x, from.y],
            [to.x, to.y],
            SceneStroke {
                dash: (semantics.stroke == SequenceMessageStroke::Dotted).then_some([3.0, 3.0]),
                ..stroke(signal_color, connector_width.map_or(1.5, f64::from))
            },
            markers,
        )?;
        projected.group(SceneRole::Edge, start)?;
    }

    let message_bounds = SequenceMessagePaintPlan::prepare(
        model,
        &nodes,
        &edges,
        connector_width,
        settings.right_angles,
        settings.actor_height,
        checks,
    )?
    .bounds;
    let root_bounds = sequence_root_bounds(
        layout,
        native_width.map(f64::from).unwrap_or(0.0) / 2.0,
        &[
            message_bounds.as_ref(),
            note_bounds.as_ref(),
            activation_bounds.as_ref(),
            lifeline_bounds.as_ref(),
        ],
    );
    let diagram_id = options.diagram_id_or("merman");
    let view_box = options
        .root_viewport(DiagramFamilyId::SEQUENCE, diagram_id)
        .plan(RootViewportSpec::responsive(root_bounds))?
        .view_box()
        .ok_or_else(|| invalid("Sequence compact viewport has no viewBox"))?;

    let arrowhead =
        sequence_arrowhead(native.map_or(MarkerStyle::Classic, |native| native.form.marker_style));
    let marker = SceneMarker {
        view_box: None,
        reference: ARROWHEAD_REFERENCE,
        width: ARROWHEAD_SIZE[0],
        height: ARROWHEAD_SIZE[1],
        units: SceneMarkerUnits::UserSpaceOnUse,
        orient: SceneMarkerOrient::AutoStartReverse,
        commands: arrowhead.commands,
        paint: ScenePaint {
            fill: arrowhead.has_fill.then_some(signal_color),
            stroke: Some(stroke(signal_color, 1.0)),
        },
    };
    let metadata_label = |label| {
        Label::prepare(
            label,
            meter,
            crate::entities::decode_mermaid_entities_for_render_text,
        )
    };
    let title = model.acc_title.as_deref().map(metadata_label).transpose()?;
    let description = model.acc_descr.as_deref().map(metadata_label).transpose()?;
    checks.checkpoint()?;
    Ok(SequenceCompactPrepared {
        viewport: SceneViewport {
            view_box: SceneBox {
                x: view_box.min_x,
                y: view_box.min_y,
                width: view_box.width,
                height: view_box.height,
            },
            width: view_box.width,
            height: view_box.height,
            canvas: canvas.filter(|color| color.alpha != 0.0),
        },
        title,
        description,
        marker,
        fonts,
        projected,
    })
}

fn scene_anchor(value: &str) -> SceneTextAnchor {
    match value {
        "start" => SceneTextAnchor::Start,
        "end" => SceneTextAnchor::End,
        _ => SceneTextAnchor::Middle,
    }
}

#[cfg(test)]
mod tests;
