//! Source-only Mermaid text resolution, before public text is measured or emitted.

use super::{DrawingListBuilder, allocation_failed, checked_increment};
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::entities::{DecodedHtmlFragment, visit_mermaid_text_content};
use merman_display_list::{DrawingCommand, DrawingListError, SemanticAnnotation};
use std::borrow::Cow;

#[derive(Clone, Copy)]
enum SourceTextPurpose {
    DrawText,
    Metadata,
    Normalization,
}

impl DrawingListBuilder<'_> {
    /// Names a text-only scope from its final public runs, without decoding source text again.
    pub(crate) fn resolved_text_name_since(&self, start: usize) -> Result<String> {
        let commands = self.commands.get(start..).ok_or_else(|| {
            super::contract_error("Semantic text range starts beyond the command stream")
        })?;
        let mut bytes = 0;
        for (index, command) in commands.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let DrawingCommand::DrawText { run } = command else {
                return Err(super::contract_error(
                    "Semantic text range contains a non-text command",
                ));
            };
            checked_increment(&mut bytes, run.text.len(), "semantic text bytes")?;
            if index != 0 {
                checked_increment(&mut bytes, 1, "semantic line separator")?;
            }
        }
        if let crate::drawing_list::DocumentBudget::DrawingList(limits) = self.budget
            && bytes > limits.max_serialized_bytes
        {
            return Err(Error::DrawingListContract(
                DrawingListError::ResourceLimit {
                    resource: "serialized_bytes",
                    actual: bytes,
                    maximum: limits.max_serialized_bytes,
                },
            ));
        }
        // Metadata copying is work but is not part of the final command text footprint.
        self.session
            .work_meter()
            .charge_at(bytes, OperationPhase::Emit)?;
        let mut name = String::new();
        name.try_reserve_exact(bytes)
            .map_err(|_| allocation_failed("semantic text name"))?;
        for (index, command) in commands.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let DrawingCommand::DrawText { run } = command else {
                unreachable!()
            };
            if index != 0 {
                name.push('\n');
            }
            name.push_str(&run.text);
        }
        Ok(name)
    }

    pub(crate) fn resolve_mermaid_text<'a>(&self, source: &'a str) -> Result<Cow<'a, str>> {
        self.resolve_source_text(source, SourceTextPurpose::DrawText)
    }

    /// The final normalized fragments, not this temporary source-sized buffer, own text quota.
    pub(crate) fn resolve_mermaid_layout_text<'a>(&self, source: &'a str) -> Result<Cow<'a, str>> {
        self.resolve_source_text(source, SourceTextPurpose::Normalization)
    }

    pub(crate) fn push_mermaid_semantic(&mut self, mut semantic: SemanticAnnotation) -> Result<()> {
        for field in [&mut semantic.title, &mut semantic.description] {
            if let Some(source) = field.as_deref()
                && let Cow::Owned(resolved) =
                    self.resolve_source_text(source, SourceTextPurpose::Metadata)?
            {
                *field = Some(resolved);
            }
        }
        self.push_semantic(semantic)
    }

    fn resolve_source_text<'a>(
        &self,
        source: &'a str,
        purpose: SourceTextPurpose,
    ) -> Result<Cow<'a, str>> {
        if !source.contains("ﬂ°") && !source.contains("¶ß") {
            return Ok(Cow::Borrowed(source));
        }
        let mut bytes = 0;
        visit_mermaid_text_content(
            source,
            || self.session.checkpoint(OperationPhase::Emit),
            |part| {
                let len = match part {
                    DecodedHtmlFragment::Borrowed(text) => text.len(),
                    DecodedHtmlFragment::Scalar(ch) => ch.len_utf8(),
                };
                checked_increment(&mut bytes, len, "resolved source text bytes")?;
                if matches!(purpose, SourceTextPurpose::DrawText) {
                    let mut projected = self.usage;
                    checked_increment(&mut projected.text_bytes, bytes, "text byte count")?;
                    self.preflight(projected)?;
                } else {
                    // A metadata field alone is a lower bound on its final JSON byte count.
                    if matches!(purpose, SourceTextPurpose::Metadata)
                        && let crate::drawing_list::DocumentBudget::DrawingList(limits) =
                            self.budget
                        && bytes > limits.max_serialized_bytes
                    {
                        return Err(Error::DrawingListContract(
                            DrawingListError::ResourceLimit {
                                resource: "serialized_bytes",
                                actual: bytes,
                                maximum: limits.max_serialized_bytes,
                            },
                        ));
                    }
                    // This temporary decoding work is not part of the final document's
                    // footprint. Charge each fragment once so reported work admits replay.
                    self.session
                        .work_meter()
                        .charge_at(len, OperationPhase::Emit)?;
                }
                Ok(())
            },
        )?;
        let mut output = String::new();
        output
            .try_reserve_exact(bytes)
            .map_err(|_| allocation_failed("resolved source text"))?;
        visit_mermaid_text_content(
            source,
            || self.session.checkpoint(OperationPhase::Emit),
            |part| {
                match part {
                    DecodedHtmlFragment::Borrowed(text) => output.push_str(text),
                    DecodedHtmlFragment::Scalar(ch) => output.push(ch),
                }
                Ok(())
            },
        )?;
        Ok(Cow::Owned(output))
    }
}
