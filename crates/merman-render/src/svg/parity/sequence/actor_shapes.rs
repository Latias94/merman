use super::super::*;
use super::SequenceEmitCheckpoints;
use super::geometry::node_left_top;
use super::math_label::{record_sequence_katex_terminal_emission, sequence_katex_label};
use crate::math::PREPARED_MATH_TERMINAL_SWITCH_ATTRIBUTE;
use crate::sequence::{SEQUENCE_GLYPH_BAND_HEIGHT, SequenceActorBands, SequenceMathHeightMode};
use merman_core::diagrams::sequence::SequenceActor;

pub(super) struct ActorLifelineIdentity<'a> {
    pub(super) actor_id: &'a str,
    pub(super) actor_type: &'a str,
}

#[derive(Clone, Copy)]
pub(super) struct ActorLabelContext<'a> {
    pub(super) config: &'a merman_core::MermaidConfig,
    pub(super) diagram_id: SvgDiagramId<'a>,
    pub(super) typed_fill: Option<&'a str>,
    pub(super) typed_stroke: Option<&'a str>,
    pub(super) translate_y: f64,
    pub(super) shadow: &'a super::text_effect::SequenceTextShadow<'a>,
    pub(super) shadow_evidence: &'a crate::diagram_theme::SvgShadowEvidenceRecorder,
    pub(super) wrap_width_px: f64,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) style: &'a TextStyle,
    pub(super) typography: &'a crate::sequence::SequenceResolvedTypography,
    pub(super) typography_receipt: &'a crate::sequence::SequenceTypographyThemeReceipt,
    pub(super) math_sidecar: &'a crate::sequence::SequenceMathSidecar,
    pub(super) actor_index: Option<usize>,
    pub(super) checkpoints: SequenceEmitCheckpoints<'a>,
}

impl<'a> ActorLabelContext<'a> {
    pub(super) fn for_actor(&self, actor_index: usize) -> Self {
        Self {
            actor_index: Some(actor_index),
            ..*self
        }
    }

    pub(super) fn write_shadow(
        &self,
        out: &mut impl SvgOutput,
        text: &str,
        x: f64,
        y: f64,
        legacy_size: f64,
        baseline: super::text_effect::TextShadowBaseline,
    ) -> Result<Option<super::text_effect::SequenceTextShadowApplication>> {
        if !self.shadow.needs_bounds() || self.shadow.is_paintless(text) {
            return Ok(None);
        }
        let mut style = self.typography.terminal_text_style().clone();
        if !self.typography.requires_resolved_emission() {
            style.font_size = legacy_size;
        }
        self.shadow
            .write_definition(out, text, x, y, baseline, &style, self.measurer)
    }

    pub(super) fn record_shadow(
        &self,
        application: Option<&super::text_effect::SequenceTextShadowApplication>,
        text: &str,
        surface: crate::sequence::SequenceTextSurface,
        translate_y: f64,
    ) {
        self.shadow.record_translated_terminal(
            application,
            self.shadow.is_paintless(text),
            self.shadow_evidence,
            self.typography_receipt,
            surface,
            translate_y + self.translate_y,
        );
    }

    fn write_actor(
        &self,
        out: &mut impl SvgOutput,
        cx: f64,
        cy: f64,
        actor: &SequenceActor,
    ) -> Result<()> {
        write_actor_label(
            out,
            cx,
            cy,
            &actor.description,
            actor.wrap,
            "actor-box",
            self,
        )
    }

    pub(super) fn write_actor_man(
        &self,
        out: &mut impl SvgOutput,
        cx: f64,
        cy: f64,
        actor: &SequenceActor,
    ) -> Result<()> {
        write_actor_label(
            out,
            cx,
            cy,
            &actor.description,
            actor.wrap,
            "actor-man",
            self,
        )
    }

    pub(super) fn is_neo(&self) -> bool {
        crate::config::config_diagram_look(self.config.as_value()).is_neo()
    }

    pub(super) fn bands(&self, node: &LayoutNode, footer: bool) -> Option<SequenceActorBands> {
        self.is_neo().then(|| {
            let (_, top) = node_left_top(node);
            // Neo row height is the shared glyph band plus two 6px gaps and the label block.
            // Derive the label block from that datum so host bbox differences cannot move glyphs.
            let text_height =
                (node.height - crate::sequence::SEQUENCE_GLYPH_BAND_HEIGHT - 12.0).max(1.0);
            SequenceActorBands::new(top, node.height, text_height, footer)
        })
    }

    pub(super) fn write_shadow_attr(&self, out: &mut impl SvgOutput) {
        if self.is_neo() {
            let _ = write!(
                out,
                r#" filter="url(#{}-drop-shadow)""#,
                escape_attr_display(self.diagram_id)
            );
        }
    }

    pub(super) fn write_style_attr(
        &self,
        out: &mut impl SvgOutput,
        actor_type: &str,
        actor_index: usize,
    ) {
        self.write_style_attr_with_width(out, actor_type, actor_index, None);
    }

    fn write_style_attr_with_width(
        &self,
        out: &mut impl SvgOutput,
        actor_type: &str,
        actor_index: usize,
        width: Option<f32>,
    ) {
        let config = self.config.as_value();
        let mut style = String::new();
        let mut declaration = |property: &str, color: &str| {
            if (property == "fill" && self.typed_fill.is_some())
                || (property == "stroke" && self.typed_stroke.is_some())
            {
                return;
            }
            if !style.is_empty() {
                style.push(' ');
            }
            let color = super::super::util::cssom_color_value(color);
            let _ = write!(style, "{property}: {color};");
        };
        if matches!(
            config.get("theme").and_then(serde_json::Value::as_str),
            Some("redux-color" | "redux-dark-color")
        ) {
            for (property, key) in [("stroke", "borderColorArray"), ("fill", "bkgColorArray")] {
                if let Some(palette) = config
                    .get("themeVariables")
                    .and_then(|theme| theme.get(key))
                    .and_then(serde_json::Value::as_array)
                    && !palette.is_empty()
                    && let Some(color) = palette[actor_index % palette.len()].as_str()
                {
                    declaration(property, color);
                }
            }
        } else {
            let theme = MermaidThemeAdapter::new(config).sequence_diagram();
            if matches!(actor_type, "actor" | "boundary" | "control" | "database") {
                declaration("stroke", theme.actor_border.as_str());
            }
            if actor_type == "control" {
                declaration("fill", theme.actor_fill.as_str());
            }
        }
        if let Some(width) = width {
            let _ = write!(style, " stroke-width:{}px;", fmt(f64::from(width)));
        }
        if !style.is_empty() {
            let _ = write!(out, r#" style="{}""#, escape_attr(&style));
        }
    }
}

pub(super) fn is_actor_man_variant(actor_type: &str) -> bool {
    matches!(actor_type, "actor" | "boundary" | "control" | "entity")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ActorFillCoverage {
    TypedCss,
    Unhandled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ActorStrokeCoverage {
    TypedCss,
    Unhandled,
}

pub(super) const LIFELINE_STROKE_WIDTH_PX: f64 = 0.5;

/// Classifies which actor shapes are actually covered by the `.actor` CSS consumer.
///
/// The static Actor theme route may only claim evidence for shapes that receive the generated
/// CSS. Actor-man glyphs have child `circle`/`line` nodes targeted directly by the generated
/// selectors. Control remains conservative because its marker path still inherits the wrapper's
/// inline fill; a participant custom class likewise owns its rectangle fill.
pub(super) fn actor_fill_coverage(actor: &SequenceActor) -> ActorFillCoverage {
    match actor.actor_type.as_str() {
        // These actor-man glyphs use only circles/lines covered by the generated selectors.
        "actor" | "boundary" => ActorFillCoverage::TypedCss,
        "entity" | "collections" | "queue" | "database" => ActorFillCoverage::TypedCss,
        // The control arrow marker path still inherits the wrapper's inline fill. Do not sign the
        // whole Actor fill route until the marker writer participates in the typed receipt.
        "control" => ActorFillCoverage::Unhandled,
        // A participant with a custom class deliberately omits `.actor`; its source-owned fill
        // must not be counted as typed theme consumption. Treat it as uncovered so a strict
        // portable request cannot claim the typed route for a mixed or custom-only diagram.
        _ if actor_custom_class(actor).is_some() => ActorFillCoverage::Unhandled,
        _ => ActorFillCoverage::TypedCss,
    }
}

/// Classifies which actor shapes receive the generated actor stroke without an inline override.
pub(super) fn actor_stroke_coverage(actor: &SequenceActor) -> ActorStrokeCoverage {
    match actor.actor_type.as_str() {
        // Actor-man children, regular rectangles, collections, and queue paths all receive the
        // generated `.actor`/`.actor-man` stroke selectors directly or through an unstyled parent.
        "actor" | "boundary" | "entity" | "collections" | "queue" => ActorStrokeCoverage::TypedCss,
        // Mermaid's control and database glyphs put inline stroke on their wrappers. The control
        // marker path still inherits that stroke even though its circle/line children are covered.
        // Do not claim a typed stroke until those writers have dedicated terminal receipts.
        "control" | "database" => ActorStrokeCoverage::Unhandled,
        _ if actor_custom_class(actor).is_some() => ActorStrokeCoverage::Unhandled,
        _ => ActorStrokeCoverage::TypedCss,
    }
}

pub(super) fn write_actor_man_lifeline(
    out: &mut impl SvgOutput,
    idx: usize,
    cx: f64,
    y1: f64,
    y2: f64,
    actor_id: &str,
    filter: &str,
) {
    let _ = write!(
        out,
        r##"<g><line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="{stroke_width}px" stroke="#999" name="{name}" data-et="life-line" data-id="{data_id}"{filter}/></g>"##,
        idx = idx,
        cx = fmt(cx),
        y1 = fmt(y1),
        y2 = fmt(y2),
        stroke_width = fmt(LIFELINE_STROKE_WIDTH_PX),
        name = escape_xml(actor_id),
        data_id = escape_attr(actor_id),
    );
}

impl ActorLabelContext<'_> {
    pub(super) fn write_lifeline_root_open(
        &self,
        out: &mut impl SvgOutput,
        idx: usize,
        cx: f64,
        y1: f64,
        y2: f64,
        identity: ActorLifelineIdentity<'_>,
        filter: &str,
    ) {
        let ActorLifelineIdentity {
            actor_id,
            actor_type,
        } = identity;
        out.push_str("<g>");
        let root_class = if actor_type == "queue" {
            r#" class="actor actor-top""#
        } else {
            ""
        };
        let _ = write!(
            out,
            r##"<line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="{stroke_width}px" stroke="#999" name="{name}" data-et="life-line" data-id="{data_id}"{filter}/><g id="root-{idx}"{root_class} data-et="participant" data-type="{actor_type}" data-id="{data_id}"{look_attr}"##,
            idx = idx,
            cx = fmt(cx),
            y1 = fmt(y1),
            y2 = fmt(y2),
            name = escape_xml(actor_id),
            data_id = escape_attr(actor_id),
            root_class = root_class,
            actor_type = escape_attr(actor_type),
            stroke_width = fmt(LIFELINE_STROKE_WIDTH_PX),
            look_attr = if self.is_neo() {
                r#" data-look="neo""#
            } else {
                ""
            },
        );
        if actor_type == "collections" {
            self.write_shadow_attr(out);
        }
        out.push('>');
    }
}

pub(super) fn write_collection_actor_shape(
    out: &mut impl SvgOutput,
    n: &LayoutNode,
    actor_id: &str,
    actor: &SequenceActor,
    placement_class: &str,
    actor_index: usize,
    label_ctx: &ActorLabelContext<'_>,
) -> Result<()> {
    const OFFSET: f64 = 6.0;
    let (x, y) = node_left_top(n);
    let height = n.height - if label_ctx.is_neo() { OFFSET } else { 0.0 };
    for (rect_x, rect_y, class) in [
        (x, y, format!("actor {placement_class}")),
        (x - OFFSET, y + OFFSET, "actor".to_string()),
    ] {
        let _ = write!(
            out,
            r##"<rect x="{x}" y="{y}" fill="#eaeaea" stroke="#666" width="{w}" height="{h}" name="{name}" class="{class}""##,
            x = fmt(rect_x),
            y = fmt(rect_y),
            w = fmt(n.width),
            h = fmt(height),
            name = escape_attr(actor_id)
        );
        if label_ctx.is_neo() {
            out.push_str(r#" data-look="neo""#);
        }
        label_ctx.write_style_attr(out, "collections", actor_index);
        out.push_str("/>");
    }
    label_ctx.write_actor(out, n.x - OFFSET, y + OFFSET + height / 2.0, actor)
}

pub(super) fn write_queue_actor_shape(
    out: &mut impl SvgOutput,
    n: &LayoutNode,
    actor: &SequenceActor,
    actor_index: usize,
    label_ctx: &ActorLabelContext<'_>,
) -> Result<()> {
    let (x, y) = node_left_top(n);
    let ry = n.height / 2.0;
    let rx = ry / (2.5 + n.height / 50.0);
    let y_mid = y + ry;
    for (index, tx) in [rx, n.width - rx].into_iter().enumerate() {
        let _ = write!(
            out,
            r#"<g transform="translate({}, {})""#,
            fmt(tx),
            fmt(-n.height / 2.0)
        );
        if index == 0 {
            label_ctx.write_shadow_attr(out);
        }
        label_ctx.write_style_attr(out, "queue", actor_index);
        let _ = write!(
            out,
            r#"><path d="M {x},{y_mid} a {rx},{ry} 0 0 0 0,{h}"#,
            x = fmt(x),
            y_mid = fmt(y_mid),
            rx = fmt(rx),
            ry = fmt(ry),
            h = fmt(n.height)
        );
        if index == 0 {
            let _ = write!(
                out,
                " h {} a {},{} 0 0 0 0,-{} Z",
                fmt(n.width - 2.0 * rx),
                fmt(rx),
                fmt(ry),
                fmt(n.height)
            );
        }
        out.push_str("\"/></g>");
    }
    label_ctx.write_actor(out, n.x, y_mid, actor)
}

pub(super) fn write_database_actor_shape(
    out: &mut impl SvgOutput,
    n: &LayoutNode,
    actor: &SequenceActor,
    actor_index: usize,
    placement_class: &str,
    legacy_actor_height: f64,
    label_ctx: &ActorLabelContext<'_>,
) -> Result<()> {
    let (x, y) = node_left_top(n);
    let w = n.width / 3.0;
    let bands = label_ctx.bands(n, placement_class == "actor-bottom");
    let h = if bands.is_some() {
        SEQUENCE_GLYPH_BAND_HEIGHT
    } else {
        w
    };
    let rx = w / 2.0;
    let ry = rx / (2.5 + w / 50.0);
    let cylinder_y = bands
        .as_ref()
        .map_or(y, |band| band.glyph_bottom_y - h - ry);
    let _ = write!(
        out,
        r#"<g class="actor {placement_class}" transform="translate({}, {})""#,
        fmt(w),
        fmt(ry)
    );
    label_ctx.write_shadow_attr(out);
    label_ctx.write_style_attr(out, "database", actor_index);
    let _ = write!(
        out,
        r#"><path d="M {x},{y} a {rx},{ry} 0 0 0 {w},0 a {rx},{ry} 0 0 0 -{w},0 l 0,{h2} a {rx},{ry} 0 0 0 {w},0 l 0,-{h2}"/></g>"#,
        x = fmt(x),
        y = fmt(cylinder_y + ry),
        rx = fmt(rx),
        ry = fmt(ry),
        w = fmt(w),
        h2 = fmt(h - 2.0 * ry)
    );
    let y_text = bands
        .as_ref()
        .map_or(y + 35.0 + legacy_actor_height / 2.0, |band| {
            band.label_center_y
        });
    label_ctx.write_actor(out, n.x, y_text, actor)
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SequenceActorRectStyle {
    pub(super) stroke_width: Option<f32>,
    pub(super) radius: Option<f32>,
}

pub(super) fn actor_rect_geometry_supported(actor: &SequenceActor) -> bool {
    !is_actor_man_variant(&actor.actor_type)
        && !matches!(
            actor.actor_type.as_str(),
            "collections" | "queue" | "database"
        )
        && actor_custom_class(actor).is_none()
}

// Keep the geometry and text receipts with their respective terminal owners.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_rect_actor_shape(
    out: &mut impl SvgOutput,
    n: &LayoutNode,
    actor_id: &str,
    actor: &SequenceActor,
    placement_class: &str,
    actor_index: usize,
    label_ctx: &ActorLabelContext<'_>,
    rect_style: SequenceActorRectStyle,
    receipt: &crate::sequence::SequenceActorThemeReceipt,
    shadow: Option<super::actor_effect::SequenceActorShadow<'_>>,
) -> Result<()> {
    let filter = shadow.as_ref().map(|shadow| shadow.write_definition(out));
    let (x, y) = node_left_top(n);
    let custom_class = actor_custom_class(actor);
    let fill = if custom_class.is_some() {
        "#EDF2AE"
    } else {
        "#eaeaea"
    };
    let class = custom_class
        .map(|c| format!("{c} {placement_class}"))
        .unwrap_or_else(|| format!("actor {placement_class}"));
    let supported = actor_rect_geometry_supported(actor);
    let is_neo = label_ctx.is_neo();
    let legacy_radius = if is_neo { 6.0 } else { 3.0 };
    let radius = if supported {
        rect_style.radius.unwrap_or(legacy_radius)
    } else {
        legacy_radius
    };
    let _ = write!(
        out,
        r##"<rect x="{x}" y="{y}" fill="{fill}" stroke="#666" width="{w}" height="{h}" name="{name}" rx="{radius}" ry="{radius}" class="{class}""##,
        x = fmt(x),
        y = fmt(y),
        w = fmt(n.width),
        h = fmt(n.height),
        name = escape_xml(actor_id),
        fill = escape_xml_display(fill),
        class = escape_attr(&class),
        radius = fmt(f64::from(radius)),
    );
    if is_neo {
        out.push_str(r#" data-look="neo""#);
        if filter.is_none() && !receipt.effect_cleared {
            label_ctx.write_shadow_attr(out);
        }
    }
    label_ctx.write_style_attr_with_width(
        out,
        "participant",
        actor_index,
        supported.then_some(rect_style.stroke_width).flatten(),
    );
    if let Some(filter) = &filter {
        let _ = write!(out, r#" filter="{}""#, escape_attr(filter));
    }
    out.push_str("/>");
    out.checkpoint()?;
    if let Some(shadow) = shadow {
        shadow.record_emission();
        receipt.record_effect_rect();
    } else if supported && receipt.effect_cleared {
        receipt.record_effect_rect();
    }
    if supported {
        receipt.record_geometry_rect();
    }
    label_ctx.write_actor(out, n.x, n.y, actor)
}

fn actor_custom_class(actor: &SequenceActor) -> Option<&str> {
    actor
        .properties
        .get("class")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn write_actor_label(
    out: &mut impl SvgOutput,
    cx: f64,
    cy: f64,
    label: &str,
    wrap: bool,
    label_class: &str,
    ctx: &ActorLabelContext<'_>,
) -> Result<()> {
    ctx.checkpoints.checkpoint()?;
    let wrapped_label = if wrap {
        Some(
            crate::sequence::wrap_sequence_label_like_mermaid_lines(
                label,
                ctx.measurer,
                ctx.style,
                ctx.wrap_width_px,
                ctx.checkpoints.text(),
            )?
            .join("<br>"),
        )
    } else {
        None
    };
    let rendered_label = wrapped_label.as_deref().unwrap_or(label);

    let prepared_math = ctx.actor_index.and_then(|actor_index| {
        ctx.math_sidecar.terminal(
            &crate::sequence::SequenceMathOccurrence::Actor(actor_index),
            rendered_label,
        )
    });
    if let Some(katex) =
        sequence_katex_label(prepared_math, ctx.style, SequenceMathHeightMode::Actor)
    {
        let x = cx - katex.width / 2.0;
        let y = cy - katex.height / 2.0;
        out.push_str("<switch ");
        out.push_str(PREPARED_MATH_TERMINAL_SWITCH_ATTRIBUTE);
        out.push('>');
        let _ = write!(
            out,
            r#"<foreignObject x="{x}" y="{y}" width="{w}" height="{h}"><div class="actor {label_class}" xmlns="http://www.w3.org/1999/xhtml" style="height: 100%; width: 100%;"><div style="text-align: center; vertical-align: middle;">{html}</div></div></foreignObject>"#,
            x = fmt(x),
            y = fmt(y),
            w = fmt(katex.width),
            h = fmt(katex.height),
            html = katex.html,
        );
        let raw_lines = crate::text::split_html_br_lines(rendered_label);
        let line_count = raw_lines.len();
        write_actor_label_lines(out, cx, cy, raw_lines, line_count, label_class, ctx, false)?;
        out.push_str("</switch>");
        record_sequence_katex_terminal_emission(
            ctx.typography_receipt,
            crate::sequence::SequenceTextSurface::ParticipantLabel,
            &katex,
        );
        ctx.record_shadow(
            None,
            rendered_label,
            crate::sequence::SequenceTextSurface::ParticipantLabel,
            0.0,
        );
        return ctx.checkpoints.checkpoint();
    }

    // Split/wrap before decoding Mermaid entities so escaped `<br>` (`#lt;br#gt;`) remains
    // literal text rather than being treated as an actual `<br>` break.
    if let Some(wrapped_label) = wrapped_label {
        let raw_lines = crate::text::split_html_br_lines(&wrapped_label);
        write_actor_label_lines(
            out,
            cx,
            cy,
            raw_lines.iter().copied(),
            raw_lines.len(),
            label_class,
            ctx,
            true,
        )?;
    } else {
        let raw_lines = crate::text::split_html_br_lines(label);
        let line_count = raw_lines.len();
        write_actor_label_lines(out, cx, cy, raw_lines, line_count, label_class, ctx, true)?;
    }
    ctx.checkpoints.checkpoint()
}

fn write_actor_label_lines<'a>(
    out: &mut impl SvgOutput,
    cx: f64,
    cy: f64,
    raw_lines: impl IntoIterator<Item = &'a str>,
    line_count: usize,
    label_class: &str,
    ctx: &ActorLabelContext<'_>,
    record_receipt: bool,
) -> Result<()> {
    let n = line_count.max(1) as f64;
    for (i, raw) in raw_lines.into_iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(i)?;
        let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(raw);
        let dy = if n <= 1.0 {
            0.0
        } else {
            (i as f64 - (n - 1.0) / 2.0) * ctx.style.font_size
        };
        let application = if record_receipt {
            ctx.write_shadow(
                out,
                decoded.as_ref(),
                cx,
                cy + dy,
                ctx.style.font_size,
                super::text_effect::TextShadowBaseline::Middle,
            )?
        } else {
            None
        };
        let filter = application
            .as_ref()
            .map(|a| format!(" filter=\"{}\"", escape_attr(&a.filter)))
            .unwrap_or_default();
        let legacy_style = format!(
            "text-anchor: middle; {}",
            super::settings::sequence_text_style_attribute(ctx.style)
        );
        let inline_style = ctx
            .typography
            .terminal_style("text-anchor: middle", legacy_style);
        let _ = write!(
            out,
            r#"<text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="actor {label_class}" style="{style}"{filter}><tspan x="{x}" dy="{dy}">{text}</tspan></text>"#,
            x = fmt(cx),
            y = fmt(cy),
            style = escape_attr_display(&inline_style),
            dy = fmt(dy),
            text = escape_xml_display(decoded.as_ref())
        );
        out.checkpoint()?;
        if record_receipt {
            ctx.record_shadow(
                application.as_ref(),
                decoded.as_ref(),
                crate::sequence::SequenceTextSurface::ParticipantLabel,
                0.0,
            );
            ctx.typography_receipt
                .record_terminal_text(crate::sequence::SequenceTextSurface::ParticipantLabel);
        }
    }
    ctx.checkpoints.checkpoint()
}
