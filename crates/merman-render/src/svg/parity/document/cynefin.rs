//! Cynefin DOM scopes projected from canonical semantic groups and transforms.

use super::*;
use crate::render_geometry::cynefin::{MARKER_SIZE, REF_X, REF_Y, VIEW_BOX_SIZE, marker_transform};

impl DocumentSvgEncoder<'_> {
    pub(super) fn emit_cynefin_marked_edge(&mut self, index: usize) -> Result<bool> {
        // Element opacity/blending composites the edge and its SVG marker together, whereas the
        // public commands paint them separately. Keep ordinary paths when that distinction matters.
        if self.state.opacity != 1.0 || self.state.blend_mode != BlendMode::Normal {
            return Ok(false);
        }
        let Some(
            [
                DrawingCommand::DrawPath {
                    path: line_id,
                    style: line_style,
                },
                DrawingCommand::Save,
                DrawingCommand::ConcatTransform { transform },
                DrawingCommand::DrawPath {
                    path: arrow_id,
                    style: arrow_style,
                },
                DrawingCommand::Restore,
            ],
        ) = self.document.commands.get(index..index.saturating_add(5))
        else {
            return Ok(false);
        };
        let Some(prefix) = line_id.as_str().strip_suffix(".line") else {
            return Ok(false);
        };
        if !prefix.starts_with("cynefin.transition.")
            || arrow_id.as_str().strip_suffix(".arrowhead") != Some(prefix)
        {
            return Ok(false);
        }
        let Some(stroke) = line_style.stroke.as_ref() else {
            return Ok(false);
        };
        let [
            PathSegment::MoveTo { to: start },
            PathSegment::QuadTo { control, to: end },
        ] = self.path_resource(line_id)?.segments.as_slice()
        else {
            return Ok(false);
        };
        if marker_transform(*start, *control, *end, stroke.width) != Some(*transform) {
            // An edited command stream is still rendered faithfully as ordinary paths. Only
            // the exact source marker placement can be projected into marker-end syntax.
            return Ok(false);
        }
        let Some(Paint::Solid { color }) = arrow_style.fill.as_ref() else {
            return Ok(false);
        };
        if arrow_style.stroke.is_some() {
            return Ok(false);
        }
        let arrow = self.path_resource(arrow_id)?;
        if !arrow.segments.iter().all(|segment| match segment {
            PathSegment::MoveTo { to } | PathSegment::LineTo { to } => {
                (0.0..=VIEW_BOX_SIZE).contains(&to.x) && (0.0..=VIEW_BOX_SIZE).contains(&to.y)
            }
            PathSegment::Close => true,
            _ => false,
        }) {
            // Do not clip arbitrary edited paths to the source marker's viewBox.
            return Ok(false);
        }
        let marker_body = format!(
            "<path d=\"{}\" class=\"cynefinArrowHead\" fill=\"{}\" fill-opacity=\"{}\" fill-rule=\"{}\" stroke=\"none\"/>",
            escaped_attr(&path_d(&arrow.segments)),
            color_css(*color),
            fmt(f64::from(color.alpha) / 255.0),
            fill_rule_name(arrow_style.fill_rule),
        );
        let count = self.cynefin_marker_definitions.len();
        let diagram_id = &self.diagram_id;
        let marker_id = self
            .cynefin_marker_definitions
            .entry(marker_body)
            .or_insert_with(|| {
                if count == 0 {
                    format!("cynefin-arrow-{diagram_id}")
                } else {
                    format!("cynefin-arrow-{diagram_id}-{count}")
                }
            })
            .clone();
        let path_data = path_d(&self.path_resource(line_id)?.segments);
        write!(
            self.output,
            "<path class=\"cynefinArrowLine\" d=\"{}\" marker-end=\"url(#{})\"",
            escaped_attr(&path_data),
            escaped_attr(&marker_id)
        )
        .map_err(|_| invalid("Cynefin marked edge"))?;
        self.write_path_style(line_style)?;
        self.write_state_attrs();
        self.output.push_str("/>");
        Ok(true)
    }

    pub(super) fn write_cynefin_marker_defs(&mut self) -> Result<()> {
        if self.cynefin_marker_definitions.is_empty() {
            return Ok(());
        }
        self.output.push_str("<defs>");
        for (body, id) in &self.cynefin_marker_definitions {
            self.session.checkpoint(OperationPhase::Emit)?;
            write!(self.output,
                "<marker id=\"{}\" viewBox=\"0 0 {} {}\" refX=\"{}\" refY=\"{}\" markerWidth=\"{}\" markerHeight=\"{}\" orient=\"auto-start-reverse\">{body}</marker>",
                escaped_attr(id), fmt(VIEW_BOX_SIZE), fmt(VIEW_BOX_SIZE), fmt(REF_X), fmt(REF_Y), fmt(MARKER_SIZE), fmt(MARKER_SIZE))
                .map_err(|_| invalid("Cynefin marker definition"))?;
        }
        self.output.push_str("</defs>");
        self.session.checkpoint(OperationPhase::Emit)?;
        Ok(())
    }

    pub(super) fn begin_cynefin_semantic_group(
        &mut self,
        semantic_id: &str,
        role: SemanticRole,
    ) -> Result<()> {
        // Labels and edges have public semantic ownership but no corresponding SVG wrapper.
        // Containers and item nodes retain Mermaid's exact group classes and translations.
        let emitted = matches!(role, SemanticRole::Group | SemanticRole::Node);
        let projected_transform = if emitted {
            self.state.transform
        } else {
            Transform::IDENTITY
        };
        if emitted {
            self.output.push_str("<g");
            if role == SemanticRole::Group
                && let Some(class) = self.semantic_extra_class(semantic_id).map(str::to_owned)
                && !class.is_empty()
            {
                write!(self.output, " class=\"{}\"", escaped_attr(&class))
                    .map_err(|_| invalid("Cynefin group class"))?;
            }
            if projected_transform != Transform::IDENTITY {
                let t = projected_transform;
                if t.a == 1.0 && t.b == 0.0 && t.c == 0.0 && t.d == 1.0 {
                    write!(
                        self.output,
                        " transform=\"translate({}, {})\"",
                        fmt(t.e),
                        fmt(t.f)
                    )
                    .map_err(|_| invalid("Cynefin group translation"))?;
                } else {
                    write!(self.output, " transform=\"matrix({})\"", matrix_attr(t))
                        .map_err(|_| invalid("Cynefin group transform"))?;
                }
                self.state.transform = Transform::IDENTITY;
            }
            self.output.push('>');
        }
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: semantic_id.to_string(),
            projected_transform,
        });
        Ok(())
    }
}
