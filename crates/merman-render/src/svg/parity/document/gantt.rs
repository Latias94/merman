//! Gantt SVG chrome; geometry and paint are owned by the public command stream.

use super::*;

impl DocumentSvgEncoder<'_> {
    // Fuse only an exact semantic-owned layer. An edited sequence with more commands or scopes
    // retains the ordinary serializer rather than losing those commands during DOM projection.
    fn gantt_tick_layer_at(&self, index: usize) -> bool {
        if !matches!(self.svg_body, SvgStructureBody::Gantt(_)) {
            return false;
        }
        let Some(
            [
                DrawingCommand::BeginSemanticGroup { semantic_id },
                DrawingCommand::BeginLayer { .. },
                DrawingCommand::DrawPath { .. },
                DrawingCommand::DrawText { .. },
                DrawingCommand::EndLayer,
                DrawingCommand::EndSemanticGroup,
                ..,
            ],
        ) = self.document.commands.get(index..)
        else {
            return false;
        };
        semantic_id.starts_with("gantt.axis.")
            && semantic_id.contains(".tick.")
            && self.semantics.get(semantic_id).is_some_and(|semantic| {
                semantic.link.is_none()
                    && semantic.description.is_none()
                    && self.debug_visibility(semantic.role)
            })
    }

    pub(super) fn begin_gantt_tick_layer(
        &mut self,
        opacity: f64,
        blend: BlendMode,
    ) -> Result<bool> {
        let Some(index) = self.command_index.checked_sub(1) else {
            return Ok(false);
        };
        if !self.gantt_tick_layer_at(index) {
            return Ok(false);
        }
        write!(
            self.output,
            "<g class=\"tick\" opacity=\"{}\"",
            fmt(opacity)
        )?;
        self.write_gantt_group_transform()?;
        write_blend_style(&mut self.output, blend)?;
        self.output.push('>')?;
        self.groups.push(GroupKind::Layer {
            projected_transform: self.state.transform,
        });
        self.state.transform = Transform::IDENTITY;
        Ok(true)
    }

    fn write_gantt_group_transform(&mut self) -> Result<()> {
        let transform = self.state.transform;
        if transform.a == 1.0 && transform.b == 0.0 && transform.c == 0.0 && transform.d == 1.0 {
            write!(
                self.output,
                " transform=\"translate({},{})\"",
                fmt(transform.e),
                fmt(transform.f)
            )?;
        } else {
            write!(
                self.output,
                " transform=\"matrix({})\"",
                matrix_attr(transform)
            )?;
        }
        Ok(())
    }

    pub(super) fn begin_gantt_collection(&mut self, semantic_id: &str) -> Result<bool> {
        if !matches!(self.svg_body, SvgStructureBody::Gantt(_)) {
            return Ok(false);
        }
        let axis = matches!(semantic_id, "gantt.axis.bottom" | "gantt.axis.top");
        if axis
            && self.semantics.get(semantic_id).is_some_and(|semantic| {
                semantic.link.is_some()
                    || semantic.description.is_some()
                    || !self.debug_visibility(semantic.role)
            })
        {
            return Ok(false);
        }
        let emitted = match semantic_id {
            "gantt.document" | "gantt.title" => false,
            "gantt.excludes" | "gantt.rows" | "gantt.tasks" | "gantt.sections" => true,
            "gantt.axis.bottom" | "gantt.axis.top" => true,
            _ if self.gantt_tick_layer_at(self.command_index) => false,
            _ => return Ok(false),
        };
        // These public scopes define paint order; Mermaid exposes only the collection wrappers.
        // The visible title and accessibility references already have their own SVG anchors.
        let projected_transform = if axis {
            self.output.push_str("<g class=\"grid\"")?;
            self.write_gantt_group_transform()?;
            self.output.push('>')?;
            let transform = self.state.transform;
            self.state.transform = Transform::IDENTITY;
            transform
        } else if emitted {
            self.output.push_str("<g>")?;
            Transform::IDENTITY
        } else {
            Transform::IDENTITY
        };
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: semantic_id.to_owned(),
            projected_transform,
        });
        Ok(true)
    }

    pub(super) fn write_gantt_style(&mut self) -> Result<()> {
        // Retain source cursor/rasterization hints only. Legacy paint, font, opacity and
        // milestone-transform CSS would reinterpret or double-apply resolved drawing commands.
        let id = sanitize_svg_id(self.diagram_id.as_str());
        write!(
            self.output,
            "<style>#{id} .grid .tick{{shape-rendering:crispEdges;}}#{id} .clickable{{cursor:pointer;}}"
        )?;
        for index in 0..4 {
            if index != 0 {
                self.output.push(',')?;
            }
            write!(self.output, "#{id} .doneCrit{index}")?;
        }
        self.output
            .push_str("{cursor:pointer;shape-rendering:crispEdges;}</style><g/>")?;
        Ok(())
    }
}
