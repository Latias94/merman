//! Cynefin DOM scopes projected from canonical semantic groups and transforms.

use super::*;

impl DocumentSvgEncoder<'_> {
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
