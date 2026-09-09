//! Equivalent projection of the public Journey activity line and expanded marker.

use super::*;

pub(super) struct MarkerProjection<'a> {
    line_index: usize,
    line_id: &'a ResourceId,
    arrow_id: &'a ResourceId,
    start: Point,
    end: Point,
    line_style: &'a PathStyle,
    arrow_style: &'a PathStyle,
}

impl<'a> MarkerProjection<'a> {
    pub(super) fn new(
        document: &'a DrawingListDocument,
        session: &RenderSession,
    ) -> Result<Option<Self>> {
        let mut candidate = None;
        let mut occurrences = 0;
        for (index, command) in document.commands.iter().enumerate() {
            session.checkpoint(OperationPhase::Emit)?;
            if !matches!(command, DrawingCommand::BeginSemanticGroup { semantic_id }
                if semantic_id == "journey.activity")
            {
                continue;
            }
            occurrences += 1;
            if occurrences > 1 {
                return Ok(None);
            }
            let Some(
                [
                    DrawingCommand::DrawPath {
                        path: line_id,
                        style: line_style,
                    },
                    DrawingCommand::DrawPath {
                        path: arrow_id,
                        style: arrow_style,
                    },
                    DrawingCommand::EndSemanticGroup,
                    ..,
                ],
            ) = document.commands.get(index + 1..)
            else {
                continue;
            };
            if line_id.as_str() != "journey.activity.line"
                || arrow_id.as_str() != "journey.activity.arrowhead"
                || line_style.fill.is_some()
                || arrow_style.stroke.is_some()
                || !matches!(arrow_style.fill, Some(Paint::Solid { .. }))
            {
                continue;
            }
            let Some(stroke) = line_style.stroke.as_ref() else {
                continue;
            };
            if stroke.width <= 0.0 || !matches!(stroke.paint, Paint::Solid { .. }) {
                continue;
            }
            let mut line = None;
            let mut arrow = None;
            for resource in &document.resources {
                session.checkpoint(OperationPhase::Emit)?;
                if let DrawingResource::Path(path) = resource {
                    if path.id == *line_id {
                        line = Some(path);
                    } else if path.id == *arrow_id {
                        arrow = Some(path);
                    }
                }
            }
            let (Some(line), Some(arrow)) = (line, arrow) else {
                continue;
            };
            let Some((start, end)) = line_from_path(line) else {
                continue;
            };
            if !source_marker_matches(arrow, start, end, stroke.width) {
                continue;
            }
            candidate = Some(Self {
                line_index: index + 1,
                line_id,
                arrow_id,
                start,
                end,
                line_style,
                arrow_style,
            });
        }
        Ok(if occurrences == 1 { candidate } else { None })
    }
}

/// Source marker units use strokeWidth, ref=(5,2), and path M0,0 V4 L6,2 Z.
/// Compare the existing expanded triangle, never replace an edited arrow with source geometry.
fn source_marker_matches(path: &PathResource, start: Point, end: Point, width: f64) -> bool {
    let [
        PathSegment::MoveTo { to: p0 },
        PathSegment::LineTo { to: p1 },
        PathSegment::LineTo { to: p2 },
        PathSegment::Close,
    ] = path.segments.as_slice()
    else {
        return false;
    };
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = dx.hypot(dy);
    if !length.is_finite() || length <= f64::EPSILON {
        return false;
    }
    let ux = dx / length;
    let uy = dy / length;
    let point = |x: f64, y: f64| {
        Point::new(
            end.x + ux * x * width - uy * y * width,
            end.y + uy * x * width + ux * y * width,
        )
    };
    let expected = [point(-5.0, -2.0), point(-5.0, 2.0), point(1.0, 0.0)];
    let actual = [*p0, *p1, *p2];
    let same = |a: Point, b: Point| (a.x - b.x).abs() <= 1e-9 && (a.y - b.y).abs() <= 1e-9;
    // Cyclic starts and reversed winding describe the same filled, unstroked triangle.
    (0..3).any(|offset| (0..3).all(|i| same(actual[i], expected[(offset + i) % 3])))
        || (0..3).any(|offset| (0..3).all(|i| same(actual[i], expected[(offset + 3 - i) % 3])))
}

impl DocumentSvgEncoder<'_> {
    /// Emit the source marker first in the shared root defs, alongside public clip resources.
    pub(super) fn write_journey_marker(&mut self) -> Result<()> {
        let Some(marker) = self.journey_marker.as_ref() else {
            return Ok(());
        };
        let (style, arrow_id) = (marker.arrow_style, marker.arrow_id);
        write!(
            self.output,
            "<marker id=\"{}-arrowhead\" refX=\"5\" refY=\"2\" markerWidth=\"6\" markerHeight=\"4\" orient=\"auto\"><path d=\"M 0,0 V 4 L6,2 Z\"",
            escaped_attr(&self.diagram_id)
        )?;
        self.write_fill_stroke_style(style)?;
        write_resource_metadata(&mut self.output, self.debug, arrow_id.as_str())?;
        self.output.push_str("/></marker>")?;
        Ok(())
    }

    /// True consumes exactly the current line command and its following expanded arrow command.
    pub(super) fn emit_journey_marked_activity(&mut self, index: usize) -> Result<bool> {
        // A marker composites with its line. Separate public paints cannot be fused when their
        // element opacity or blend makes overlap observable. Keep the generic commands instead.
        if self.state.opacity != 1.0 || self.state.blend_mode != BlendMode::Normal {
            return Ok(false);
        }
        let Some(marker) = self.journey_marker.as_ref() else {
            return Ok(false);
        };
        if marker.line_index != index {
            return Ok(false);
        }
        let (start, end, style, line_id) =
            (marker.start, marker.end, marker.line_style, marker.line_id);
        write!(
            self.output,
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" marker-end=\"url(#{}-arrowhead)\"",
            fmt(start.x),
            fmt(start.y),
            fmt(end.x),
            fmt(end.y),
            escaped_attr(&self.diagram_id)
        )?;
        if !self.write_journey_line_presentation(line_id, style)? {
            self.write_fill_stroke_style(style)?;
            self.write_state_attrs()?;
        }
        write_resource_metadata(&mut self.output, self.debug, line_id.as_str())?;
        self.output.push_str("/>")?;
        Ok(true)
    }
}
