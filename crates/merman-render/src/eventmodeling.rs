use crate::model::{
    Bounds, EventModelingBoxLayout, EventModelingDiagramLayout, EventModelingRelationLayout,
    EventModelingSwimlaneLayout,
};
use crate::resources::{OperationWorkMeter, PreparedTextRetainedReservation};
use crate::text::{
    TextMeasurer, TextStyle, split_html_br_lines, wrap_label_like_mermaid_lines_with_checkpoint,
};
use crate::{Error, Result};
use merman_core::presentation::PresentationConfig;
use merman_model::families::event_modeling::{
    EventModelingDataEntity, EventModelingDiagramModel, EventModelingFrame,
};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

mod config;
mod css_binding;
mod theme;

pub(crate) use css_binding::EventModelingCssBinding;
pub(crate) use theme::EventModelingTextThemePlan;

const SWIMLANE_MIN_HEIGHT: f64 = 70.0;
const SWIMLANE_PADDING: f64 = 15.0;
const SWIMLANE_GAP: f64 = 10.0;
const BOX_PADDING: f64 = 10.0;
const BOX_OVERLAP: f64 = 90.0;
const BOX_MIN_WIDTH: f64 = 80.0;
const BOX_MAX_WIDTH: f64 = 450.0;
const BOX_MIN_HEIGHT: f64 = 80.0;
const BOX_MAX_HEIGHT: f64 = 750.0;
const CONTENT_START_X: f64 = 250.0;
const TEXT_MAX_WIDTH: f64 = 430.0;
const BOX_TEXT_PADDING: f64 = 10.0;
const TEXT_FONT_SIZE: f64 = 16.0;
const HTML_LABEL_BBOX_LINE_HEIGHT: f64 = 19.0;

/// Final XHTML terminals from the pinned `calculateTextProps` producer, indexed by box index.
#[derive(Debug, Default)]
pub(crate) struct EventModelingPreparedLabels {
    boxes: Vec<EventModelingPreparedHtmlLabel>,
}

#[derive(Debug)]
struct EventModelingPreparedHtmlLabel {
    xhtml: String,
    dimension: TextDimension,
    _retained_reservation: PreparedTextRetainedReservation,
}

impl EventModelingPreparedLabels {
    pub(crate) fn box_xhtml(&self, index: usize) -> Result<&str> {
        self.boxes
            .get(index)
            .map(|label| label.xhtml.as_str())
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing prepared Event Modeling XHTML label for box `{index}`"),
            })
    }
}

pub(crate) fn prepare_eventmodeling_diagram_typed_with_binding(
    model: &EventModelingDiagramModel,
    theme: &EventModelingCssBinding,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    work_meter: &Arc<OperationWorkMeter>,
) -> Result<(EventModelingDiagramLayout, EventModelingPreparedLabels)> {
    work_meter.checkpoint(merman_core::OperationPhase::Layout)?;
    let cfg = theme.layout_settings;
    let mut label_config = effective_config.clone();
    if label_config.get_str("securityLevel").is_none() {
        label_config.set_value("securityLevel", serde_json::json!("strict"));
    }
    // Author text is sanitized before wrapping, as in pinned db.ts::calculateTextProps.
    // The second pass serializes generated markup; it must not escape it as author text.
    let mut fragment_config = label_config.clone();
    fragment_config.set_value("securityLevel", serde_json::json!("loose"));
    let mut prepared_labels = EventModelingPreparedLabels::default();
    let data_entities: HashMap<&str, &EventModelingDataEntity> = model
        .data_entities
        .iter()
        .map(|entity| (entity.name.as_str(), entity))
        .collect();

    let mut swimlanes: BTreeMap<i64, SwimlaneState> = BTreeMap::new();
    let mut boxes = Vec::new();
    let mut frame_to_box = HashMap::new();
    let mut relation_specs = Vec::new();
    let mut previous_swimlane_index = None;
    let mut max_r: f64 = 0.0;

    for (index, frame) in model.frames.iter().enumerate() {
        let text = frame_text(frame, &data_entities);
        let label = EventModelingPreparedHtmlLabel::prepare(
            frame,
            &data_entities,
            &label_config,
            &fragment_config,
            measurer,
            work_meter,
        )?;
        let text_dimension = label.dimension;
        work_meter.checkpoint(merman_core::OperationPhase::Layout)?;
        prepared_labels.boxes.push(label);
        let event_width = text_dimension.width + 2.0 * BOX_TEXT_PADDING;
        let event_height = text_dimension.height + 2.0 * BOX_TEXT_PADDING;
        let width = event_width.clamp(BOX_MIN_WIDTH, BOX_MAX_WIDTH) + 2.0 * BOX_PADDING;
        let height = event_height.clamp(BOX_MIN_HEIGHT, BOX_MAX_HEIGHT) + 2.0 * BOX_PADDING;
        let swimlane_props = calculate_swimlane_props(frame, &swimlanes);
        let swimlane = swimlanes
            .entry(swimlane_props.index)
            .or_insert_with(|| SwimlaneState {
                index: swimlane_props.index,
                label: swimlane_props.label.clone(),
                namespace: swimlane_props.namespace.clone(),
                r: 0.0,
                y: 0.0,
                height: SWIMLANE_MIN_HEIGHT,
                max_height: SWIMLANE_MIN_HEIGHT,
            });

        let x = calculate_x(swimlane, previous_swimlane_index, boxes.last());
        let r = x + width + BOX_PADDING;
        swimlane.r = x + width;
        swimlane.max_height = swimlane.max_height.max(height);
        swimlane.height = swimlane.max_height.max(SWIMLANE_MIN_HEIGHT) + 2.0 * SWIMLANE_PADDING;
        max_r = max_r.max(swimlane.r).max(r);

        let visual = entity_visual_props(theme, &frame.model_entity_type);
        let box_state = BoxState {
            index,
            frame_name: frame.name.clone(),
            frame_kind: frame.frame_kind.clone(),
            model_entity_type: frame.model_entity_type.clone(),
            entity_identifier: frame.entity_identifier.clone(),
            text,
            x,
            width,
            height,
            fill: visual.fill,
            stroke: visual.stroke,
            swimlane_index: swimlane.index,
            r,
        };
        let target_box_idx = boxes.len();
        frame_to_box.insert(frame.name.clone(), target_box_idx);
        boxes.push(box_state);

        if !(index == 0 && frame.source_frames.is_empty()) {
            if frame.source_frames.is_empty() && frame.frame_kind != "resetframe" {
                if let Some(source_idx) =
                    find_previous_cross_swimlane_box(&boxes, swimlane_props.index, index)
                {
                    relation_specs.push((source_idx, target_box_idx));
                }
            } else {
                for source_name in &frame.source_frames {
                    if let Some(source_idx) = frame_to_box.get(source_name).copied() {
                        relation_specs.push((source_idx, target_box_idx));
                    }
                }
            }
        }

        previous_swimlane_index = Some(swimlane_props.index);
        recalculate_swimlane_y(&mut swimlanes);
    }

    recalculate_swimlane_y(&mut swimlanes);
    let swimlane_width = max_r + SWIMLANE_PADDING;
    let swimlane_layouts: Vec<_> = swimlanes
        .values()
        .map(|swimlane| EventModelingSwimlaneLayout {
            index: swimlane.index,
            label: swimlane.label.clone(),
            namespace: swimlane.namespace.clone(),
            x: 0.0,
            y: swimlane.y,
            width: swimlane_width.max(1.0),
            height: swimlane.height,
        })
        .collect();

    let box_layouts: Vec<_> = boxes
        .iter()
        .map(|box_state| {
            let swimlane = &swimlanes[&box_state.swimlane_index];
            EventModelingBoxLayout {
                index: box_state.index,
                frame_name: box_state.frame_name.clone(),
                frame_kind: box_state.frame_kind.clone(),
                model_entity_type: box_state.model_entity_type.clone(),
                entity_identifier: box_state.entity_identifier.clone(),
                text: box_state.text.clone(),
                x: box_state.x,
                y: swimlane.y + SWIMLANE_PADDING,
                width: box_state.width,
                height: box_state.height,
                fill: box_state.fill.clone(),
                stroke: box_state.stroke.clone(),
                swimlane_index: box_state.swimlane_index,
            }
        })
        .collect();

    let relation_stroke = theme.relation_stroke.clone();
    let relation_layouts: Vec<_> = relation_specs
        .into_iter()
        .map(|(source_idx, target_idx)| {
            let source = &box_layouts[source_idx];
            let target = &box_layouts[target_idx];
            let upwards = source.y > target.y;
            let x1 = source.x + (source.width * 2.0) / 3.0;
            let x2 = target.x + target.width / 3.0;
            let (y1, y2) = if upwards {
                (source.y, target.y + target.height)
            } else {
                (source.y + source.height, target.y)
            };
            EventModelingRelationLayout {
                source_frame: source.frame_name.clone(),
                target_frame: target.frame_name.clone(),
                x1,
                y1,
                x2,
                y2,
                stroke: relation_stroke.clone(),
            }
        })
        .collect();

    let mut bounds = BoundsAcc::new();
    for swimlane in &swimlane_layouts {
        bounds.include_rect(swimlane.x, swimlane.y, swimlane.width, swimlane.height);
    }
    for box_layout in &box_layouts {
        bounds.include_rect(
            box_layout.x,
            box_layout.y,
            box_layout.width,
            box_layout.height,
        );
    }
    for relation in &relation_layouts {
        bounds.include_point(relation.x1, relation.y1);
        bounds.include_point(relation.x2, relation.y2);
    }

    let padded = bounds.finish(cfg.padding).unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 1.0,
        max_y: 1.0,
    });
    let total_width = (padded.max_x - padded.min_x).max(1.0);
    let total_height = (padded.max_y - padded.min_y).max(1.0);
    let viewbox_x = padded.min_x;
    let viewbox_y = padded.min_y;

    Ok((
        EventModelingDiagramLayout {
            bounds: Some(padded),
            total_width,
            total_height,
            viewbox_x,
            viewbox_y,
            padding: cfg.padding,
            use_max_width: cfg.use_max_width,
            swimlanes: swimlane_layouts,
            boxes: box_layouts,
            relations: relation_layouts,
        },
        prepared_labels,
    ))
}

#[derive(Debug, Clone)]
struct SwimlaneProps {
    index: i64,
    label: String,
    namespace: Option<String>,
}

#[derive(Debug, Clone)]
struct SwimlaneState {
    index: i64,
    label: String,
    namespace: Option<String>,
    r: f64,
    y: f64,
    height: f64,
    max_height: f64,
}

#[derive(Debug, Clone)]
struct BoxState {
    index: usize,
    frame_name: String,
    frame_kind: String,
    model_entity_type: String,
    entity_identifier: String,
    text: String,
    x: f64,
    width: f64,
    height: f64,
    fill: String,
    stroke: String,
    swimlane_index: i64,
    r: f64,
}

#[derive(Debug, Clone)]
struct VisualProps {
    fill: String,
    stroke: String,
}

#[derive(Debug, Clone, Copy)]
struct TextDimension {
    width: f64,
    height: f64,
}

fn calculate_swimlane_props(
    frame: &EventModelingFrame,
    swimlanes: &BTreeMap<i64, SwimlaneState>,
) -> SwimlaneProps {
    let namespace = extract_namespace(&frame.entity_identifier);
    match frame.model_entity_type.as_str() {
        "ui" | "pcr" | "processor" => {
            namespaced_or_default_swimlane(swimlanes, namespace, 0, 100, "UI/Automation", "UI/A: ")
        }
        "rmo" | "readmodel" | "cmd" | "command" => namespaced_or_default_swimlane(
            swimlanes,
            namespace,
            100,
            200,
            "Command/Read Model",
            "C/RM: ",
        ),
        "evt" | "event" => {
            namespaced_or_default_swimlane(swimlanes, namespace, 200, 300, "Events", "Stream: ")
        }
        _ => namespaced_or_default_swimlane(swimlanes, namespace, 200, 300, "Events", "Stream: "),
    }
}

fn namespaced_or_default_swimlane(
    swimlanes: &BTreeMap<i64, SwimlaneState>,
    namespace: Option<String>,
    boundary_min: i64,
    boundary_max: i64,
    default_label: &str,
    prefix: &str,
) -> SwimlaneProps {
    if let Some(namespace) = namespace {
        if let Some(swimlane) =
            find_swimlane_by_namespace(swimlanes, &namespace, boundary_min, boundary_max)
        {
            return SwimlaneProps {
                index: swimlane.index,
                label: swimlane.label.clone(),
                namespace: Some(namespace),
            };
        }

        SwimlaneProps {
            index: find_next_available_index(swimlanes, boundary_min, boundary_max),
            label: format!("{prefix}{namespace}"),
            namespace: Some(namespace),
        }
    } else {
        SwimlaneProps {
            index: boundary_min,
            label: default_label.to_string(),
            namespace: None,
        }
    }
}

fn extract_namespace(entity_identifier: &str) -> Option<String> {
    let mut parts = entity_identifier.split('.');
    let namespace = parts.next()?;
    let name = parts.next()?;
    parts
        .next()
        .is_none()
        .then(|| (!namespace.is_empty() && !name.is_empty()).then(|| namespace.to_string()))
        .flatten()
}

fn extract_name(entity_identifier: &str) -> &str {
    let mut parts = entity_identifier.split('.');
    let Some(first) = parts.next() else {
        return entity_identifier;
    };
    let Some(second) = parts.next() else {
        return entity_identifier;
    };
    if parts.next().is_none() {
        second
    } else {
        first
    }
}

fn find_swimlane_by_namespace<'a>(
    swimlanes: &'a BTreeMap<i64, SwimlaneState>,
    namespace: &str,
    boundary_min: i64,
    boundary_max: i64,
) -> Option<&'a SwimlaneState> {
    swimlanes.values().find(|swimlane| {
        swimlane.index > boundary_min
            && swimlane.index < boundary_max
            && swimlane.namespace.as_deref() == Some(namespace)
    })
}

fn find_next_available_index(
    swimlanes: &BTreeMap<i64, SwimlaneState>,
    boundary_min: i64,
    boundary_max: i64,
) -> i64 {
    swimlanes
        .keys()
        .copied()
        .filter(|index| *index > boundary_min && *index < boundary_max)
        .fold(boundary_min, i64::max)
        + 1
}

fn calculate_x(
    swimlane: &SwimlaneState,
    previous_swimlane_index: Option<i64>,
    last_box: Option<&BoxState>,
) -> f64 {
    if previous_swimlane_index.is_none() {
        return CONTENT_START_X;
    }
    if previous_swimlane_index == Some(swimlane.index) && swimlane.r > 0.0 {
        return swimlane.r + BOX_PADDING;
    }
    if let Some(last_box) = last_box {
        return last_box.r - BOX_OVERLAP + BOX_PADDING;
    }
    CONTENT_START_X
}

fn find_previous_cross_swimlane_box(
    boxes: &[BoxState],
    target_swimlane_index: i64,
    line_index: usize,
) -> Option<usize> {
    if line_index == 0 {
        return None;
    }
    (0..line_index)
        .rev()
        .find(|idx| boxes[*idx].swimlane_index != target_swimlane_index)
}

fn recalculate_swimlane_y(swimlanes: &mut BTreeMap<i64, SwimlaneState>) {
    let mut next_y = 0.0;
    for swimlane in swimlanes.values_mut() {
        swimlane.y = next_y;
        next_y += swimlane.height + SWIMLANE_GAP;
    }
}

fn frame_text(
    frame: &EventModelingFrame,
    data_entities: &HashMap<&str, &EventModelingDataEntity>,
) -> String {
    let mut text = extract_name(&frame.entity_identifier).to_string();
    if let Some(data) = frame.data_inline_value.as_deref() {
        text.push('\n');
        text.push_str(data);
    } else if let Some(reference) = frame.data_reference.as_deref()
        && let Some(entity) = data_entities.get(reference)
    {
        text.push('\n');
        text.push_str(&entity.data_block_value);
    }
    text
}

fn eventmodeling_label_style() -> TextStyle {
    TextStyle {
        font_size: TEXT_FONT_SIZE,
        font_weight: Some("700".to_string()),
        ..Default::default()
    }
}

fn measure_eventmodeling_label_html(
    html: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
) -> TextDimension {
    let sans_style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: style.font_size,
        font_weight: style.font_weight.clone(),
        font_style: None,
    };
    let default_font_style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: style.font_size,
        font_weight: style.font_weight.clone(),
        font_style: None,
    };
    let sans = measure_eventmodeling_label_html_with_style(html, measurer, &sans_style);
    let default_font =
        measure_eventmodeling_label_html_with_style(html, measurer, &default_font_style);

    if sans.height > default_font.height
        && sans.width > default_font.width
        && sans.line_height > default_font.line_height
    {
        sans.into_dimension()
    } else {
        default_font.into_dimension()
    }
}

#[derive(Debug, Clone, Copy)]
struct EventModelingHtmlTextDimension {
    width: f64,
    height: f64,
    line_height: f64,
}

impl EventModelingHtmlTextDimension {
    fn into_dimension(self) -> TextDimension {
        TextDimension {
            width: self.width,
            height: self.height,
        }
    }
}

fn measure_eventmodeling_label_html_with_style(
    html: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
) -> EventModelingHtmlTextDimension {
    let mut width: f64 = 0.0;
    let mut height: f64 = 0.0;
    let mut line_height: f64 = 0.0;
    for line in split_html_br_lines(html) {
        let line = line.replace(['\r', '\n'], " ");
        width = width.max(
            measurer
                .measure_svg_simple_text_bbox_width_px(&line, style)
                .round(),
        );
        let current_height = measurer
            .measure_svg_simple_text_bbox_height_px(&line, style)
            .round()
            .max(HTML_LABEL_BBOX_LINE_HEIGHT);
        height += current_height;
        line_height = line_height.max(current_height);
    }
    EventModelingHtmlTextDimension {
        width,
        height,
        line_height,
    }
}

impl EventModelingPreparedHtmlLabel {
    fn prepare(
        frame: &EventModelingFrame,
        data_entities: &HashMap<&str, &EventModelingDataEntity>,
        label_config: &PresentationConfig,
        fragment_config: &PresentationConfig,
        measurer: &dyn TextMeasurer,
        work_meter: &Arc<OperationWorkMeter>,
    ) -> Result<Self> {
        use crate::html::HtmlSanitizeSink;

        // Include the vector owner slot, consistently with Block's prepared-label accounting.
        const OWNER_SLOT_BYTES: usize = 192;
        let owner_bytes = std::mem::size_of::<Self>().saturating_add(OWNER_SLOT_BYTES);
        work_meter.preflight_prepared_text_retained_bytes(owner_bytes)?;
        let parse_work = std::cell::Cell::new(0);
        let max = work_meter
            .remaining_prepared_text_retained_bytes()
            .map_or(usize::MAX, |remaining| {
                remaining.saturating_sub(owner_bytes)
            });
        let sink = HtmlSanitizeSink::new(
            max,
            work_meter,
            &parse_work,
            merman_core::OperationPhase::Layout,
        );
        // Author sanitization is transient and may shrink during wrapLabel whitespace folding.
        // Its parser/storage work is metered separately from final retained XHTML bytes.
        let author_sink = HtmlSanitizeSink::new(
            usize::MAX,
            work_meter,
            &parse_work,
            merman_core::OperationPhase::Layout,
        );
        let sanitize_author = |raw: &str| {
            eventmodeling_sanitize_result(
                merman_markup::sanitize::sanitize_text_with_sink(
                    raw,
                    label_config.as_config(),
                    &author_sink,
                ),
                owner_bytes,
                work_meter,
            )
        };
        let style = eventmodeling_label_style();
        let wrap = |text: &str| {
            wrap_label_like_mermaid_lines_with_checkpoint(
                text,
                measurer,
                &style,
                TEXT_MAX_WIDTH,
                || Ok(work_meter.checkpoint(merman_core::OperationPhase::Layout)?),
            )
            .map(|lines| lines.join("<br/>"))
        };
        let title = wrap(&sanitize_author(extract_name(&frame.entity_identifier))?)?;
        let mut html = format!("<b>{title}</b>");
        let data = frame.data_inline_value.as_deref().or_else(|| {
            frame
                .data_reference
                .as_deref()
                .and_then(|reference| data_entities.get(reference))
                .map(|entity| entity.data_block_value.as_str())
        });
        if let Some(data) = data {
            let data = sanitize_author(&normalize_eventmodeling_data(data))?;
            let wrapped_data = wrap(&data)?.replace(' ', "&nbsp;");
            html.push_str(
                r#"<br/><br/><code style="text-align: left; display: block;max-width:430px">"#,
            );
            html.push_str(&wrapped_data);
            if frame.data_reference.is_some() {
                html.push_str("<br/>");
            }
            html.push_str("</code>");
        }
        // Pinned calculateTextDimensions probes generated HTML source with SVG text APIs.
        // Preserve entity spellings here; XHTML serialization is a separate terminal projection.
        let mut dimension = measure_eventmodeling_label_html(&html, measurer, &style);
        if data.is_some() {
            dimension.width /= 3.0;
        }
        dimension.width = dimension.width.min(TEXT_MAX_WIDTH);
        // Keep entity-authored angle brackets encoded until this HTML parse consumes them.
        // Only this sanitized, XML-serialized terminal may be written beneath the XHTML span.
        let xhtml = eventmodeling_sanitize_result(
            merman_markup::sanitize::sanitize_xhtml_with_sink(
                &html,
                fragment_config.as_config(),
                &sink,
            ),
            owner_bytes,
            work_meter,
        )?;
        let work_control =
            crate::layout_work::OperationLayoutWorkControl::new(Arc::clone(work_meter));
        let reservation = work_control
            .reserve_prepared_text_retained_bytes(owner_bytes.saturating_add(xhtml.capacity()))?;
        Ok(Self {
            xhtml,
            dimension,
            _retained_reservation: reservation,
        })
    }
}

fn eventmodeling_sanitize_result(
    result: std::result::Result<
        String,
        merman_markup::sanitize::SanitizeFailure<crate::html::HtmlOutputError>,
    >,
    owner_bytes: usize,
    work_meter: &OperationWorkMeter,
) -> Result<String> {
    use crate::html::HtmlOutputError;
    use merman_markup::sanitize::SanitizeFailure;

    match result {
        Ok(text) => Ok(text),
        Err(SanitizeFailure::Output(HtmlOutputError::Operation(error))) => Err(error.into()),
        Err(SanitizeFailure::Output(HtmlOutputError::Limit { attempted })) => {
            work_meter
                .preflight_prepared_text_retained_bytes(owner_bytes.saturating_add(attempted))?;
            Ok(String::new())
        }
        // Preserve the sanitizer's fail-closed behavior for malformed input/allocation.
        Err(_) => Ok(String::new()),
    }
}

fn normalize_eventmodeling_data(raw: &str) -> String {
    let trimmed = raw.trim();
    let without_outer_braces = trimmed
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or(trimmed);
    without_outer_braces.trim().to_string()
}

fn entity_visual_props(theme: &EventModelingCssBinding, entity_type: &str) -> VisualProps {
    match entity_type {
        "ui" => VisualProps {
            fill: theme.ui_fill.clone(),
            stroke: theme.ui_stroke.clone(),
        },
        "pcr" | "processor" => VisualProps {
            fill: theme.processor_fill.clone(),
            stroke: theme.processor_stroke.clone(),
        },
        "rmo" | "readmodel" => VisualProps {
            fill: theme.read_model_fill.clone(),
            stroke: theme.read_model_stroke.clone(),
        },
        "cmd" | "command" => VisualProps {
            fill: theme.command_fill.clone(),
            stroke: theme.command_stroke.clone(),
        },
        "evt" | "event" => VisualProps {
            fill: theme.event_fill.clone(),
            stroke: theme.event_stroke.clone(),
        },
        _ => VisualProps {
            fill: "red".to_string(),
            stroke: "black".to_string(),
        },
    }
}

#[derive(Debug, Clone, Copy)]
struct BoundsAcc {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    has_value: bool,
}

impl BoundsAcc {
    fn new() -> Self {
        Self {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 0.0,
            max_y: 0.0,
            has_value: false,
        }
    }

    fn include_point(&mut self, x: f64, y: f64) {
        if !self.has_value {
            self.min_x = x;
            self.max_x = x;
            self.min_y = y;
            self.max_y = y;
            self.has_value = true;
            return;
        }
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
        self.min_y = self.min_y.min(y);
        self.max_y = self.max_y.max(y);
    }

    fn include_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.include_point(x, y);
        self.include_point(x + width, y + height);
    }

    fn finish(self, padding: f64) -> Option<Bounds> {
        self.has_value.then_some(Bounds {
            min_x: self.min_x - padding,
            min_y: self.min_y - padding,
            max_x: self.max_x + padding,
            max_y: self.max_y + padding,
        })
    }
}
