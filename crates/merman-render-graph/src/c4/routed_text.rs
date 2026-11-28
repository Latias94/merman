use kurbo::{Rect, Vec2};
use merman_render_core::{
    Result,
    text::{
        TextMeasurer, TextStyle, WrapMode, line_break::split_mermaid_common_br_lines,
        svg_title_bbox_vertical_extents_px, try_measure_wrapped_text,
    },
};

use merman_theme_contract::v2::EdgeLabelBackground;

use super::{
    C4_TECHNOLOGY_GAP, C4BoundaryLayout, C4Measurement, C4NativeStyle, C4RelLayout,
    C4TextBlockLayout, C4TypographyThemePlan, c4_relation_offset, c4_text_row_offset,
    geometry::C4TextRenderPlan, typography::C4PreparedFont,
};

const RELATION_WRAP_WIDTH: f64 = 190.0;
pub const SHAFT_CLEARANCE: f64 = 2.0;
const PLATE_PAD_X: f64 = 6.0;
const PLATE_PAD_Y: f64 = 3.0;

impl C4TypographyThemePlan {
    /// Reserve the largest caption footprint across canvas backgrounds.
    pub fn routed_caption_footprint(&self, cutout: Rect) -> Rect {
        if self
            .native_style()
            .is_some_and(|theme| theme.edge_label_bg == EdgeLabelBackground::Auto)
        {
            cutout.inflate(PLATE_PAD_X - SHAFT_CLEARANCE, PLATE_PAD_Y - SHAFT_CLEARANCE)
        } else {
            cutout
        }
    }
}

impl C4NativeStyle {
    /// Maximum painted shaft width, including the transparent-canvas contrast outline.
    pub fn routed_shaft_outline_width(&self) -> f64 {
        let halo = (self.connector_width / 2.0).min(0.65);
        self.connector_width + 2.0 * halo
    }
}

pub fn measure_c4_relation_label(
    rel: &mut C4RelLayout,
    ctx: &C4Measurement<'_>,
) -> Result<Option<(Rect, Vec2)>> {
    let font = ctx.typography.message_font();
    let mut ink = relation_ink(&mut rel.label, font, false, ctx)?;
    if let Some(techn) = &mut rel.techn
        && !techn.text.trim().is_empty()
        && let Some(technology) = relation_ink(techn, font, true, ctx)?
    {
        // The blocks are centered on their own baselines. Use the last message row and
        // first technology row's measured extents to leave the same gap for every row count.
        techn.y = ink.map_or(0.0, |message| {
            message.y1 + C4_TECHNOLOGY_GAP - technology.y0
        });
        ink = union(ink, Some(technology + Vec2::new(0.0, techn.y)));
    }
    let offset = |value| if f64::is_finite(value) { value } else { 0.0 };
    Ok(ink.map(|ink| {
        (
            ink.inflate(SHAFT_CLEARANCE, SHAFT_CLEARANCE),
            Vec2::new(
                offset(c4_relation_offset(rel.offset_x.as_ref())),
                offset(c4_relation_offset(rel.offset_y.as_ref())),
            ),
        )
    }))
}

fn relation_ink(
    block: &mut C4TextBlockLayout,
    font: &C4PreparedFont,
    technology: bool,
    ctx: &C4Measurement<'_>,
) -> Result<Option<Rect>> {
    let size = font.css_font_size_px.expect("C4 relationship font size");
    let mut italic_style;
    let bracketed;
    let (text, style) = if technology {
        italic_style = font.measurement_style.clone();
        italic_style.font_style = Some("italic".into());
        ctx.work.charge(block.text.len())?;
        bracketed = format!("[{}]", block.text);
        (&bracketed, &italic_style)
    } else {
        (&block.text, &font.measurement_style)
    };
    let measure = |text: &str| {
        text_ink(text, 0.0, size, style, ctx.measurer, |units| {
            Ok(ctx.work.charge(units)?)
        })
    };
    let mut ink = measure(text)?;
    if ctx.conf.wrap && ink.is_some_and(|ink| ink.width() > RELATION_WRAP_WIDTH) {
        let mut all_lines = Vec::new();
        for segment in split_mermaid_common_br_lines(text) {
            ctx.work.charge(segment.len().saturating_add(1))?;
            let wrapped = try_measure_wrapped_text(
                segment,
                style,
                Some(RELATION_WRAP_WIDTH),
                WrapMode::SvgLikeSingleRun,
                size,
                |row, style| {
                    ctx.work.charge(row.len().saturating_add(1))?;
                    Ok::<_, merman_render_core::Error>(
                        ctx.measurer
                            .measure_svg_tspan_text_bbox_width_px(row, style),
                    )
                },
            )?;
            for row in wrapped.lines {
                ctx.work.charge(row.len().saturating_add("<br/>".len()))?;
                all_lines.push(row);
            }
        }
        block.line_count = all_lines.len().max(1);
        let wrapped = all_lines.join("<br/>");
        ink = measure(&wrapped)?;
        block.text = if technology {
            wrapped
                .strip_prefix('[')
                .and_then(|inner| inner.strip_suffix(']'))
                .unwrap_or(wrapped.as_str())
                .to_owned()
        } else {
            wrapped
        };
    }
    block.width = ink.map_or(0.0, |ink| ink.width());
    block.height = ink.map_or(0.0, |ink| ink.height());
    block.render_plan = ink.map(C4TextRenderPlan::RelationInk);
    Ok(ink)
}

pub(super) fn relation_label_plate(
    relation: &C4RelLayout,
    reference: Option<&C4TextBlockLayout>,
) -> Option<Rect> {
    [Some(&relation.label), relation.techn.as_ref(), reference]
        .into_iter()
        .flatten()
        .filter_map(|block| match block.render_plan.as_ref()? {
            C4TextRenderPlan::RelationInk(ink) => Some(*ink + Vec2::new(0.0, block.y)),
            C4TextRenderPlan::Unified(_) => None,
        })
        .reduce(|ink, block| ink.union(block))
        .map(|ink| ink.inflate(PLATE_PAD_X, PLATE_PAD_Y))
}

pub fn title_ink(boundary: &C4BoundaryLayout, ctx: &C4Measurement<'_>) -> Result<Option<Rect>> {
    boundary_title_footprint(boundary, ctx.typography, ctx.measurer, |units| {
        Ok(ctx.work.charge(units)?)
    })
}

pub(super) fn boundary_title_footprint(
    boundary: &C4BoundaryLayout,
    typography: &C4TypographyThemePlan,
    measurer: &dyn TextMeasurer,
    mut charge: impl FnMut(usize) -> Result<()>,
) -> Result<Option<Rect>> {
    let fonts = typography.boundary_fonts();
    let mut measure = |block: &C4TextBlockLayout, font: &C4PreparedFont| {
        text_ink(
            &block.text,
            block.y,
            font.css_font_size_px.expect("C4 boundary font size"),
            &font.measurement_style,
            measurer,
            &mut charge,
        )
    };
    let mut ink = measure(&boundary.label, &fonts.name)?;
    for (text, font) in [(&boundary.ty, &fonts.ty), (&boundary.descr, &fonts.descr)] {
        if let Some(block) = text {
            ink = union(ink, measure(block, font)?);
        }
    }
    Ok(ink.map(|ink| ink.inflate(PLATE_PAD_X, PLATE_PAD_Y)))
}

fn union(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(b)),
        (Some(a), None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

fn text_ink(
    text: &str,
    y: f64,
    font_size: f64,
    style: &TextStyle,
    measurer: &dyn TextMeasurer,
    mut charge: impl FnMut(usize) -> Result<()>,
) -> Result<Option<Rect>> {
    if style.font_size <= 0.0 {
        return Ok(None);
    }
    charge(text.len())?;
    let split;
    let rows = if text.contains('<') {
        split = split_mermaid_common_br_lines(text);
        split.as_slice()
    } else {
        std::slice::from_ref(&text)
    };
    let mut ink = None;
    for (row, content) in rows.iter().enumerate() {
        charge(content.len().saturating_add(1))?;
        if content.trim().is_empty() {
            continue;
        }
        let half_width = measurer.measure_svg_tspan_text_bbox_width_px(content, style) / 2.0;
        let height = measurer.measure_svg_tspan_text_bbox_height_px(content, style);
        let middle = measurer.measure_svg_raw_text_middle_bbox_y_offset_px(content, style);
        let (top, bottom) = middle.map_or_else(
            || {
                // When the profile has no middle-baseline metric, reserve both the
                // centered row and the usual ascent/descent around its baseline.
                let (ascent, descent) = svg_title_bbox_vertical_extents_px(style);
                (-ascent.max(height / 2.0), descent.max(height / 2.0))
            },
            |top| (top, top + height),
        );
        let at = y + c4_text_row_offset(row, rows.len(), font_size);
        let row = Rect::new(-half_width, at + top, half_width, at + bottom);
        if row.width() > 0.0 && row.height() > 0.0 {
            ink = union(ink, Some(row));
        }
    }
    Ok(ink)
}
