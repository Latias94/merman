//! Sankey SVG grouping projected from public semantic scopes, never the family model.

use super::*;

impl DocumentSvgEncoder<'_> {
    pub(super) fn begin_sankey_semantic_group(&mut self, semantic_id: &str) -> Result<()> {
        // Labels stay in their shared source layer; individual logical label and document
        // scopes do not introduce DOM wrappers. Paint and compositing stay on drawing commands.
        let class = (!semantic_id.starts_with("sankey.label."))
            .then(|| self.semantic_extra_class(semantic_id).map(str::to_owned))
            .flatten();
        let emitted = class.is_some();
        if let Some(class) = class {
            write!(self.output, "<g class=\"{}\">", escaped_attr(&class))
                .map_err(|_| invalid("Sankey semantic group"))?;
        }
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: semantic_id.to_owned(),
            projected_transform: Transform::IDENTITY,
        });
        Ok(())
    }
}
