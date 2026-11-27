use std::borrow::Cow;

use super::SvgValidation;
use crate::svg_scan::checkpoint_loop;
use crate::{Error, Result};

impl SvgValidation<'_> {
    /// Removes Mermaid's redundant background ID from renderer-certified Mindmap SVG.
    ///
    /// The caller supplies the trusted family identity before embedding admission.
    pub fn normalize_mindmap_ids_for_embedding<'a>(
        self,
        svg: Cow<'a, str>,
    ) -> Result<Cow<'a, str>> {
        self.checkpoint()?;
        let document = roxmltree::Document::parse(&svg).map_err(|error| {
            Error::svg_postprocess("normalize-embedding-ids", error.to_string())
        })?;
        let mut removed = Vec::new();
        for (index, node) in document.descendants().enumerate() {
            checkpoint_loop(index, &mut || self.checkpoint())?;
            if !node.has_tag_name(("http://www.w3.org/2000/svg", "path"))
                || !node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| class == "node-bkg")
                })
            {
                continue;
            }
            let Some(parent) = node.parent_element() else {
                continue;
            };
            if !parent.has_tag_name(("http://www.w3.org/2000/svg", "g")) {
                continue;
            }
            let classes = parent.attribute("class").unwrap_or_default();
            if !classes
                .split_ascii_whitespace()
                .any(|class| class == "node")
                || !classes
                    .split_ascii_whitespace()
                    .any(|class| class == "mindmap-node")
            {
                continue;
            }
            let Some(attribute) = node
                .attributes()
                .find(|attribute| attribute.namespace().is_none() && attribute.name() == "id")
            else {
                continue;
            };
            if parent.attribute("id") == Some(attribute.value()) {
                removed.push(attribute.range());
            }
        }
        if removed.is_empty() {
            return Ok(svg);
        }
        self.checkpoint()?;
        let mut output = String::with_capacity(svg.len());
        let mut cursor = 0;
        for (index, range) in removed.into_iter().enumerate() {
            checkpoint_loop(index, &mut || self.checkpoint())?;
            output.push_str(&svg[cursor..range.start]);
            cursor = range.end;
        }
        output.push_str(&svg[cursor..]);
        self.checkpoint()?;
        Ok(Cow::Owned(output))
    }
}
