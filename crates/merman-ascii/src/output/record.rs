//! Private, bounded reading records shared by the family-owned semantic documents.

use super::{RawValue, Result, SemanticFallbackWriter, display_width_with_profile};
use crate::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use std::collections::BTreeMap;
use unicode_segmentation::UnicodeSegmentation;

pub(super) enum RecordPart<'a> {
    Plain(&'a str),
    #[cfg_attr(
        not(feature = "all-diagrams"),
        allow(
            dead_code,
            reason = "Some families write dynamic quoted fields through RecordWriter"
        )
    )]
    Quoted(&'a str),
}

pub(super) fn write_record(
    parts: &[RecordPart<'_>],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    let mut record = RecordWriter::new(lines)?;
    for part in parts {
        let (text, quoted) = match part {
            RecordPart::Plain(text) => (*text, false),
            RecordPart::Quoted(text) => (*text, true),
        };
        record.text(text, quoted)?;
    }
    record.finish()
}

pub(super) fn write_plain_record(text: &str, lines: &mut SemanticFallbackWriter<'_>) -> Result<()> {
    write_record(&[RecordPart::Plain(text)], lines)
}

pub(super) struct RecordWriter<'a, 'operation> {
    lines: &'a mut SemanticFallbackWriter<'operation>,
    width: usize,
    has_content: bool,
}

impl<'a, 'operation> RecordWriter<'a, 'operation> {
    pub(super) fn new(lines: &'a mut SemanticFallbackWriter<'operation>) -> Result<Self> {
        let (width, has_content) = lines.begin_line()?;
        Ok(Self {
            lines,
            width,
            has_content,
        })
    }

    pub(super) fn finish(self) -> Result<()> {
        self.lines.finish_line()
    }

    pub(super) fn text(&mut self, text: &str, quoted: bool) -> Result<()> {
        let resources = self.lines.emit_resources.clone();
        let mut start = 0;
        let mut trailing_space = false;
        for (index, grapheme) in text.grapheme_indices(true) {
            resources.charge_layout_work(1)?;
            let end = index + grapheme.len();
            trailing_space = grapheme == " ";
            if trailing_space && end < text.len() {
                self.word(&text[start..end], quoted, start == 0, false, true)?;
                start = end;
            }
        }
        self.word(&text[start..], quoted, start == 0, true, trailing_space)
    }

    fn word(
        &mut self,
        text: &str,
        quoted: bool,
        first: bool,
        last: bool,
        trailing_space: bool,
    ) -> Result<()> {
        let mut resources = self.lines.emit_resources.clone();
        let mut word_width = usize::from(quoted && first) + usize::from(quoted && last);
        visit_record_text(
            &text[..text.len() - usize::from(trailing_space)],
            quoted,
            &mut resources,
            self.lines.profile,
            |_, width, _| {
                word_width = self
                    .lines
                    .candidate_resources
                    .checked_grid_add(word_width, width)?;
                Ok(())
            },
        )?;
        if self.has_content
            && word_width <= self.lines.max_width
            && self
                .lines
                .candidate_resources
                .checked_grid_add(self.width, word_width)?
                > self.lines.max_width
        {
            self.continue_line(word_width)?;
        }
        if quoted && first {
            self.fragment("\"", 1, 1)?;
        }
        visit_record_text(
            text,
            quoted,
            &mut resources,
            self.lines.profile,
            |fragment, width, bytes| self.fragment(fragment, width, bytes),
        )?;
        if quoted && last {
            self.fragment("\"", 1, 1)?;
        }
        Ok(())
    }

    fn continue_line(&mut self, next_width: usize) -> Result<()> {
        self.lines.push_fragment("\n", 1)?;
        self.width = 0;
        self.has_content = false;
        // Indentation is presentation only. Authored spaces are emitted independently.
        if next_width <= self.lines.max_width.saturating_sub(2) && self.lines.max_width > 2 {
            self.lines
                .push_wrapped_fragment("  ", 2, 1, &mut self.width, &mut self.has_content)?;
        }
        Ok(())
    }

    fn fragment(&mut self, fragment: &str, width: usize, bytes: usize) -> Result<()> {
        if self.has_content
            && self
                .lines
                .candidate_resources
                .checked_grid_add(self.width, width)?
                > self.lines.max_width
        {
            self.continue_line(width)?;
        }
        self.lines.push_wrapped_fragment(
            fragment,
            width,
            bytes,
            &mut self.width,
            &mut self.has_content,
        )
    }
}

fn visit_record_text(
    text: &str,
    quoted: bool,
    resources: &mut ResourceContext,
    profile: TerminalWidthProfile,
    mut visit: impl FnMut(&str, usize, usize) -> Result<()>,
) -> Result<()> {
    if !quoted {
        return crate::safe_text::visit_safe_line_fragments(resources, text, profile, visit);
    }
    crate::safe_text::visit_quoted_terminal_text(text, resources, |fragment| {
        if fragment == "\"" {
            return Ok(());
        }
        let mut grapheme_count = 0usize;
        let mut grapheme_bytes = 0usize;
        for grapheme in fragment.graphemes(true) {
            grapheme_count = grapheme_count.saturating_add(1);
            grapheme_bytes = grapheme_bytes.max(grapheme.len());
        }
        resources.charge_layout_work(grapheme_count)?;
        visit(
            fragment,
            display_width_with_profile(fragment, profile),
            grapheme_bytes,
        )
    })
}

pub(super) fn details(
    scope: &str,
    fields: &BTreeMap<String, &RawValue>,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    if fields.is_empty() {
        return Ok(());
    }
    let overflow = || {
        lines
            .semantic_resources
            .overflow(AsciiResourceLimitId::MaxOutputBytes)
    };
    let mut length = scope.len().checked_add(12).ok_or_else(overflow)?;
    for (index, (key, raw)) in fields.iter().enumerate() {
        // Field names are model-owned identifiers. Opaque values stay complete borrowed JSON.
        length = length
            .checked_add(key.len())
            .and_then(|length| length.checked_add(raw.get().len()))
            .and_then(|length| length.checked_add(if index == 0 { 1 } else { 2 }))
            .ok_or_else(overflow)?;
    }
    lines
        .semantic_resources
        .check(AsciiResourceLimitId::MaxOutputBytes, length)?;
    let mut record = RecordWriter::new(lines)?;
    record.text(scope, false)?;
    record.text(" details: {", false)?;
    for (index, (key, raw)) in fields.iter().enumerate() {
        if index > 0 {
            record.text(",", false)?;
        }
        record.text(key, false)?;
        record.text(":", false)?;
        record.text(raw.get(), false)?;
    }
    record.text("}", false)?;
    record.finish()
}
