//! Sankey SVG grouping projected from public semantic scopes, never the family model.

use super::*;

impl DocumentSvgEncoder<'_> {
    pub(super) fn write_sankey_referenced_gradient(&mut self, style: &PathStyle) -> Result<()> {
        for paint in style
            .fill
            .iter()
            .chain(style.stroke.iter().map(|stroke| &stroke.paint))
        {
            let Paint::Resource { id } = paint else {
                continue;
            };
            if !self.sankey_inline_gradients.contains(id.as_str())
                || !self.emitted_sankey_gradients.insert(id.as_str().to_owned())
            {
                continue;
            }
            self.session.checkpoint(OperationPhase::Emit)?;
            let Some(DrawingResource::LinearGradient(gradient)) =
                self.resources.get(id.as_str()).copied()
            else {
                return Err(invalid("Sankey inline gradient resource is not linear"));
            };
            let svg_id = self.svg_resource_id(id.as_str())?;
            self.write_linear_gradient(&svg_id, gradient)?;
        }
        Ok(())
    }

    pub(super) fn begin_sankey_semantic_group(&mut self, semantic_id: &str) -> Result<()> {
        // Labels stay in their shared source layer; individual logical label and document
        // scopes do not introduce DOM wrappers. Paint and compositing stay on drawing commands.
        let class = (!semantic_id.starts_with("sankey.label."))
            .then(|| self.semantic_extra_class(semantic_id).map(str::to_owned))
            .flatten();
        let emitted = class.is_some();
        let mut projected_transform = Transform::IDENTITY;
        if let Some(class) = class {
            write!(self.output, "<g class=\"{}\"", escaped_attr(&class))
                .map_err(|_| invalid("Sankey semantic group"))?;
            if let Some(index) = semantic_id
                .strip_prefix("sankey.node.")
                .and_then(|index| index.parse::<usize>().ok())
                .and_then(|index| index.checked_add(1))
            {
                let prefix = if self.options.diagram_id.is_some() {
                    format!("{}-", self.diagram_id)
                } else {
                    String::new()
                };
                write!(self.output, " id=\"{}node-{index}\"", escaped_attr(&prefix))
                    .map_err(|_| invalid("Sankey node ID"))?;
                let transform = self.state.transform;
                if transform.a == 1.0
                    && transform.b == 0.0
                    && transform.c == 0.0
                    && transform.d == 1.0
                {
                    write!(
                        self.output,
                        " transform=\"translate({},{})\" x=\"{}\" y=\"{}\"",
                        fmt(transform.e),
                        fmt(transform.f),
                        fmt(transform.e),
                        fmt(transform.f)
                    )
                    .map_err(|_| invalid("Sankey node translation"))?;
                } else {
                    write!(
                        self.output,
                        " transform=\"matrix({})\"",
                        matrix_attr(transform)
                    )
                    .map_err(|_| invalid("Sankey node transform"))?;
                }
                projected_transform = transform;
                self.state.transform = Transform::IDENTITY;
            }
            self.output.push('>');
        }
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: semantic_id.to_owned(),
            projected_transform,
        });
        Ok(())
    }

    pub(super) fn emit_sankey_node_rect(&mut self, bounds: Rect, style: &PathStyle) -> Result<()> {
        write!(
            self.output,
            "<rect height=\"{}\" width=\"{}\"",
            fmt(bounds.height),
            fmt(bounds.width)
        )
        .map_err(|_| invalid("Sankey node rectangle"))?;
        if bounds.x != 0.0 || bounds.y != 0.0 {
            write!(
                self.output,
                " x=\"{}\" y=\"{}\"",
                fmt(bounds.x),
                fmt(bounds.y)
            )
            .map_err(|_| invalid("Sankey edited rectangle origin"))?;
        }
        self.output.push_str(" shape-rendering=\"crispEdges\"");
        self.write_fill_stroke_style(style)?;
        self.write_state_attrs();
        self.output.push_str("/>");
        Ok(())
    }
}
