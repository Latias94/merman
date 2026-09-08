//! Gantt SVG chrome; geometry and paint are owned by the public command stream.

use super::*;

impl DocumentSvgEncoder<'_> {
    pub(super) fn begin_gantt_collection(&mut self, semantic_id: &str) -> Result<bool> {
        if !matches!(self.svg_body, SvgStructureBody::Gantt(_)) {
            return Ok(false);
        }
        let emitted = match semantic_id {
            "gantt.document" | "gantt.title" => false,
            "gantt.excludes" | "gantt.rows" | "gantt.tasks" | "gantt.sections" => true,
            _ => return Ok(false),
        };
        // These public scopes define paint order; Mermaid exposes only the collection wrappers.
        // The visible title and accessibility references already have their own SVG anchors.
        if emitted {
            self.output.push_str("<g>")?;
        }
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: semantic_id.to_owned(),
            projected_transform: Transform::IDENTITY,
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
