use std::fmt;
use std::ops::Range;

use crate::resources::{OperationWorkMeter, SvgByteBudget};

/// Minimal SVG output surface shared by unbounded helpers and operation-bounded renderers.
///
/// Family writers intentionally depend on this narrow interface instead of `String`, allowing the
/// operation boundary to reject an append before the backing allocation grows.
pub(super) trait SvgOutput: fmt::Write {
    fn push_str(&mut self, value: &str);
    fn push(&mut self, value: char);
    fn len(&self) -> usize;
    fn as_str(&self) -> &str;
    fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()>;
    fn checkpoint(&mut self) -> crate::Result<()>;

    /// Appends an unchanged XML fragment whose bytes have already been observed while scanning.
    /// Retained-byte admission and sink failures still belong to this append.
    fn write_scanned_text(&mut self, value: &str) -> fmt::Result {
        self.write_str(value)
    }

    /// Accounts for scanned input, including input discarded by serialization.
    fn observe_scanned_work(&mut self, _bytes: usize) -> crate::Result<()> {
        self.observe_operation()
    }

    /// Observes operation cancellation without pretending to append an empty fragment.
    fn observe_operation(&mut self) -> crate::Result<()> {
        self.checkpoint()
    }
}

impl SvgOutput for String {
    fn push_str(&mut self, value: &str) {
        String::push_str(self, value);
    }

    fn push(&mut self, value: char) {
        String::push(self, value);
    }

    fn len(&self) -> usize {
        String::len(self)
    }

    fn as_str(&self) -> &str {
        String::as_str(self)
    }

    fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()> {
        String::replace_range(self, range, replacement);
        Ok(())
    }

    fn checkpoint(&mut self) -> crate::Result<()> {
        Ok(())
    }
}

/// Serializes logical stylesheet fragments as XML character data exactly once.
///
/// The document sink continues to own retained-byte admission and operation failures. Quotes
/// remain literal in character data, and forbidden XML scalars are removed during serialization.
/// Keeping the trailing brackets across fragments also prevents a split `]]>` terminator.
pub(in crate::svg::parity) struct XmlTextOutput<'a, S> {
    out: &'a mut S,
    trailing_brackets: u8,
}

impl<'a, S: SvgOutput> XmlTextOutput<'a, S> {
    pub(in crate::svg::parity) fn new(out: &'a mut S) -> Self {
        Self {
            out,
            trailing_brackets: 0,
        }
    }
}

impl<S: SvgOutput> XmlTextOutput<'_, S> {
    fn write_scalar_chunk(&mut self, value: &str, start: usize, end: usize) -> fmt::Result {
        let bytes = value.as_bytes();
        let mut plain_start = start;
        let mut offset = start;
        // A str already proves UTF-8 validity. XML 1.0 excludes only ASCII controls and
        // U+FFFE/U+FFFF from its scalar range, so other UTF-8 bytes need no decoding.
        while offset < end {
            let (replacement, consumed) = match bytes[offset] {
                0..=8 | 11 | 12 | 14..=31 => (Some(""), 1),
                0xef if bytes[offset..end].starts_with(&[0xef, 0xbf, 0xbe])
                    || bytes[offset..end].starts_with(&[0xef, 0xbf, 0xbf]) =>
                {
                    (Some(""), 3)
                }
                b']' => {
                    self.trailing_brackets = (self.trailing_brackets + 1).min(2);
                    (None, 1)
                }
                byte => {
                    let replacement = match byte {
                        b'&' => Some("&amp;"),
                        b'<' => Some("&lt;"),
                        b'>' if self.trailing_brackets == 2 => Some("&gt;"),
                        _ => None,
                    };
                    self.trailing_brackets = 0;
                    (replacement, 1)
                }
            };
            if let Some(replacement) = replacement {
                if plain_start < offset {
                    self.out.write_scanned_text(&value[plain_start..offset])?;
                }
                if !replacement.is_empty() {
                    self.out.write_str(replacement)?;
                }
                // Removed scalars do not break a retained `]]` sequence.
                plain_start = offset + consumed;
            }
            offset += consumed;
        }
        if plain_start < end {
            self.out.write_scanned_text(&value[plain_start..end])?;
        }

        Ok(())
    }
    fn write_searchable_chunk(&mut self, value: &str, start: usize, end: usize) -> fmt::Result {
        let bytes = value.as_bytes();
        let mut offset = start;
        let mut plain_start = start;
        while offset < end {
            if self.trailing_brackets == 2 && bytes[offset] == b'>' {
                if plain_start < offset {
                    self.out.write_scanned_text(&value[plain_start..offset])?;
                }
                self.out.write_str("&gt;")?;
                self.trailing_brackets = 0;
                offset += 1;
                plain_start = offset;
                continue;
            }
            let ordinary =
                memchr::memchr3(b'&', b'<', b']', &bytes[offset..end]).unwrap_or(end - offset);
            if ordinary != 0 {
                self.trailing_brackets = 0;
                offset += ordinary;
            }
            if offset == end {
                break;
            }
            match bytes[offset] {
                b']' => {
                    self.trailing_brackets = (self.trailing_brackets + 1).min(2);
                    offset += 1;
                }
                byte => {
                    if plain_start < offset {
                        self.out.write_scanned_text(&value[plain_start..offset])?;
                    }
                    self.out
                        .write_str(if byte == b'&' { "&amp;" } else { "&lt;" })?;
                    self.trailing_brackets = 0;
                    offset += 1;
                    plain_start = offset;
                }
            }
        }
        if plain_start < end {
            self.out.write_scanned_text(&value[plain_start..end])?;
        }
        Ok(())
    }
}

impl<S: SvgOutput> fmt::Write for XmlTextOutput<'_, S> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        const MAX_PLAIN_FRAGMENT_BYTES: usize = 2_048;

        let bytes = value.as_bytes();
        let mut start = 0;
        while start < bytes.len() {
            let mut end = start
                .saturating_add(MAX_PLAIN_FRAGMENT_BYTES)
                .min(bytes.len());
            while !value.is_char_boundary(end) {
                end += 1;
            }
            self.out
                .observe_scanned_work(end - start)
                .map_err(|_| fmt::Error)?;
            let first = bytes[start];
            if first < 0x20 || matches!(first, b'&' | b'<' | b']' | b'>' | 0xef) {
                self.write_scalar_chunk(value, start, end)?;
            } else {
                let flags = bytes[start..end].iter().fold(0u8, |flags, &byte| {
                    flags
                        | (u8::from((byte < 0x20) | (byte == 0xef)) << 1)
                        | u8::from(matches!(byte, b'&' | b'<' | b']'))
                });
                match flags {
                    0 => {
                        self.trailing_brackets = 0;
                        self.out.write_scanned_text(&value[start..end])?;
                    }
                    1 => self.write_searchable_chunk(value, start, end)?,
                    _ => self.write_scalar_chunk(value, start, end)?,
                }
            }
            start = end;
        }
        self.out.checkpoint().map_err(|_| fmt::Error)
    }
}

impl<S: SvgOutput> SvgOutput for XmlTextOutput<'_, S> {
    fn push_str(&mut self, value: &str) {
        let _ = fmt::Write::write_str(self, value);
    }

    fn push(&mut self, value: char) {
        let mut encoded = [0; 4];
        self.push_str(value.encode_utf8(&mut encoded));
    }

    fn len(&self) -> usize {
        self.out.len()
    }

    fn as_str(&self) -> &str {
        self.out.as_str()
    }

    fn replace_range(&mut self, _range: Range<usize>, _replacement: &str) -> crate::Result<()> {
        self.out.checkpoint()?;
        Err(crate::Error::InvalidModel {
            message: "XML character-data writer does not replace emitted text".to_owned(),
        })
    }

    fn checkpoint(&mut self) -> crate::Result<()> {
        self.out.checkpoint()
    }

    fn observe_scanned_work(&mut self, bytes: usize) -> crate::Result<()> {
        self.out.observe_scanned_work(bytes)
    }

    fn observe_operation(&mut self) -> crate::Result<()> {
        self.out.observe_operation()
    }
}

/// Operation-owned SVG buffer that checks every retained append before allocation.
///
/// Producer-owned projected-byte accounting remains separate: some emitters reserve or charge
/// their own contributions before serialization, so this document sink must not charge them a
/// second time.
pub(super) struct BoundedSvgOutput<'a> {
    out: String,
    budget: SvgByteBudget<'a>,
    admitted_end: usize,
    work_until_observation: usize,
    state: BoundedSvgOutputState,
    #[cfg(test)]
    capacity_admissions: usize,
    #[cfg(test)]
    write_attempts: usize,
}

enum BoundedSvgOutputState {
    Active,
    Failed(BoundedSvgOutputFailure),
}

#[derive(Clone)]
enum BoundedSvgOutputFailure {
    Operation(crate::resources::OperationWorkError),
    InvalidModel(&'static str),
}

impl BoundedSvgOutputState {
    fn record_failure(&mut self, failure: BoundedSvgOutputFailure) {
        if matches!(self, Self::Active) {
            *self = Self::Failed(failure);
        }
    }

    fn error(&self) -> Option<crate::Error> {
        match self {
            Self::Active => None,
            Self::Failed(failure) => Some(failure.to_error()),
        }
    }

    fn into_error(self) -> Option<crate::Error> {
        match self {
            Self::Active => None,
            Self::Failed(failure) => Some(failure.into_error()),
        }
    }
}

impl BoundedSvgOutputFailure {
    fn to_error(&self) -> crate::Error {
        self.clone().into_error()
    }

    fn into_error(self) -> crate::Error {
        match self {
            Self::Operation(error) => error.into(),
            Self::InvalidModel(message) => crate::Error::InvalidModel {
                message: message.to_string(),
            },
        }
    }
}

impl<'a> BoundedSvgOutput<'a> {
    pub(super) fn new(work_meter: &'a OperationWorkMeter) -> Self {
        Self {
            out: String::new(),
            budget: work_meter.svg_byte_budget(),
            admitted_end: 0,
            work_until_observation: 0,
            state: BoundedSvgOutputState::Active,
            #[cfg(test)]
            capacity_admissions: 0,
            #[cfg(test)]
            write_attempts: 0,
        }
    }

    pub(super) fn finish(self) -> crate::Result<String> {
        match self.state.into_error() {
            Some(error) => Err(error),
            None => {
                self.budget.observe_operation()?;
                Ok(self.out)
            }
        }
    }

    fn replace_retained_range(
        &mut self,
        range: Range<usize>,
        replacement: &str,
    ) -> crate::Result<()> {
        if let Some(error) = self.state.error() {
            return Err(error);
        }
        let removed = range.len();
        let Some(retained_without_range) = self.out.len().checked_sub(removed) else {
            return Err(self.fail(BoundedSvgOutputFailure::InvalidModel(
                "bounded SVG replacement range exceeds the current output",
            )));
        };
        let Some(final_len) = retained_without_range.checked_add(replacement.len()) else {
            return Err(self.fail(BoundedSvgOutputFailure::InvalidModel(
                "bounded SVG replacement length overflowed",
            )));
        };
        if final_len > self.out.len() {
            let growth = final_len - self.out.len();
            if let Err(error) = self.observe_work(growth) {
                return Err(self.fail(BoundedSvgOutputFailure::Operation(error)));
            }
            if let Err(failure) = self.admit_growth(growth) {
                return Err(self.fail(failure));
            }
        }
        self.out.replace_range(range, replacement);
        Ok(())
    }

    fn admit_growth(&mut self, additional: usize) -> Result<(), BoundedSvgOutputFailure> {
        // Credit is explicit: an allocator may return more capacity than requested. Fitting the
        // admitted end proves both non-overflow and the exact retained-byte ceiling; excess
        // allocator capacity never grants permission to retain more serialized bytes.
        if additional <= self.admitted_end - self.out.len() {
            return Ok(());
        }
        #[cfg(test)]
        {
            self.capacity_admissions += 1;
        }
        let target_capacity = self
            .budget
            .reservation_capacity(self.out.len(), self.admitted_end, additional)
            .map_err(BoundedSvgOutputFailure::Operation)?;
        let reserve = target_capacity.checked_sub(self.out.len()).ok_or(
            BoundedSvgOutputFailure::InvalidModel(
                "bounded SVG output capacity fell below its retained length",
            ),
        )?;
        self.out.try_reserve_exact(reserve).map_err(|_| {
            BoundedSvgOutputFailure::InvalidModel("failed to reserve bounded SVG output")
        })?;
        self.admitted_end = target_capacity;
        Ok(())
    }

    fn observe_work(&mut self, bytes: usize) -> Result<(), crate::resources::OperationWorkError> {
        const OBSERVATION_INTERVAL: usize = 2_048;
        if bytes >= self.work_until_observation {
            self.budget.observe_operation()?;
            self.work_until_observation = OBSERVATION_INTERVAL;
        }
        self.work_until_observation = self.work_until_observation.saturating_sub(bytes);
        Ok(())
    }

    fn record_write(&mut self, value: &str, scanned: bool) -> fmt::Result {
        #[cfg(test)]
        {
            self.write_attempts += 1;
        }
        if matches!(&self.state, BoundedSvgOutputState::Failed(_)) {
            return Err(fmt::Error);
        }
        if !scanned && let Err(error) = self.observe_work(value.len()) {
            self.state
                .record_failure(BoundedSvgOutputFailure::Operation(error));
            return Err(fmt::Error);
        }
        if let Err(failure) = self.admit_growth(value.len()) {
            self.state.record_failure(failure);
            return Err(fmt::Error);
        }
        self.out.push_str(value);
        Ok(())
    }

    fn fail(&mut self, failure: BoundedSvgOutputFailure) -> crate::Error {
        self.state.record_failure(failure);
        self.state
            .error()
            .expect("recording a bounded SVG failure must make the sink terminal")
    }

    #[cfg(test)]
    fn retained_capacity(&self) -> usize {
        self.out.capacity()
    }
}

impl fmt::Write for BoundedSvgOutput<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.record_write(value, false)
    }
}

impl SvgOutput for BoundedSvgOutput<'_> {
    fn push_str(&mut self, value: &str) {
        let _ = self.record_write(value, false);
    }

    fn push(&mut self, value: char) {
        let mut encoded = [0u8; 4];
        let _ = self.record_write(value.encode_utf8(&mut encoded), false);
    }

    fn len(&self) -> usize {
        self.out.len()
    }

    fn as_str(&self) -> &str {
        self.out.as_str()
    }

    fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()> {
        self.replace_retained_range(range, replacement)
    }

    fn checkpoint(&mut self) -> crate::Result<()> {
        match self.state.error() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn write_scanned_text(&mut self, value: &str) -> fmt::Result {
        self.record_write(value, true)
    }

    fn observe_scanned_work(&mut self, bytes: usize) -> crate::Result<()> {
        if let Some(error) = self.state.error() {
            return Err(error);
        }
        self.observe_work(bytes)
            .map_err(|error| self.fail(BoundedSvgOutputFailure::Operation(error)))
    }

    fn observe_operation(&mut self) -> crate::Result<()> {
        if let Some(error) = self.state.error() {
            return Err(error);
        }
        self.budget
            .observe_operation()
            .map_err(|error| self.fail(BoundedSvgOutputFailure::Operation(error)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

    #[test]
    fn xml_text_output_matches_scalar_normalization_across_unicode_and_fragments() {
        let mut logical = String::from("]]\0> &< ]\u{fffe}]\u{ffff}> ");
        logical.extend((0..=0x10ffff).filter_map(char::from_u32));
        logical.push_str("]]\u{1f}> &<");
        let expected = crate::xml::strip_forbidden_xml_1_0_chars(&logical)
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace("]]>", "]]&gt;");
        let mut serialized = String::new();
        {
            let mut out = XmlTextOutput::new(&mut serialized);
            let mut start = 0;
            while start < logical.len() {
                let mut end = (start + 509).min(logical.len());
                while !logical.is_char_boundary(end) {
                    end += 1;
                }
                fmt::Write::write_str(&mut out, &logical[start..end]).unwrap();
                start = end;
            }
        }
        assert_eq!(serialized, expected);
    }

    #[test]
    fn xml_text_output_preserves_logical_css_across_fragment_boundaries() {
        let logical = "'A&B' < Ω ]]> literal &amp; \"quotes\" >";
        let mut serialized = String::from("<svg><style>");
        {
            let mut out = XmlTextOutput::new(&mut serialized);
            for fragment in ["'A&B' < Ω ]", "]", "> literal &amp; \"quotes\" >"] {
                out.push_str(fragment);
            }
        }
        serialized.push_str("</style></svg>");
        assert!(serialized.contains("]]&gt;"));
        assert!(serialized.contains("&amp;amp;"));
        assert!(serialized.contains("\"quotes\" >"));
        let document = crate::xml_dom::Document::parse(&serialized).unwrap();
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(css, logical);
    }

    #[test]
    fn xml_text_output_charges_serialized_bytes_and_keeps_first_terminal_failure() {
        use merman_core::OperationControl;

        const EXPECTED: &str = "<svg><style>A&amp;B&lt;]]&gt;</style></svg>";
        for max in [EXPECTED.len(), EXPECTED.len() - 1] {
            let policy = RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, max)
                .unwrap();
            let control = OperationControl::new();
            let meter = OperationWorkMeter::new_with_control(policy, control.clone());
            let mut out = BoundedSvgOutput::new(&meter);
            out.push_str("<svg><style>");
            {
                let mut css_out = XmlTextOutput::new(&mut out);
                css_out.push_str("A&B<]");
                css_out.push_str("]>");
            }
            out.push_str("</style></svg>");
            if max == EXPECTED.len() {
                assert_eq!(out.finish().unwrap(), EXPECTED);
            } else {
                let first = resource_limit(out.checkpoint().unwrap_err());
                assert_eq!(first.actual, EXPECTED.len());
                assert_eq!(first.max, max);
                control.cancel();
                {
                    let mut css_out = XmlTextOutput::new(&mut out);
                    css_out.push_str("&");
                    assert_eq!(resource_limit(css_out.checkpoint().unwrap_err()), first);
                    assert_eq!(
                        resource_limit(css_out.replace_range(0..0, "").unwrap_err()),
                        first
                    );
                }
                assert_eq!(resource_limit(out.finish().unwrap_err()), first);
            }
            assert_eq!(meter.projected_svg_bytes(), 0);
        }
    }

    #[test]
    fn xml_text_output_preserves_the_first_over_limit_append_boundary() {
        for (fragments, maximum, retained, actual) in [
            (vec!["abc&de".to_string()], 4, "abc".to_string(), 8),
            (
                vec!["abc".to_string(), "def".to_string()],
                4,
                "abc".to_string(),
                6,
            ),
            (vec!["a".repeat(4096)], 2049, "a".repeat(2048), 4096),
        ] {
            let policy = RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, maximum)
                .unwrap();
            let meter = OperationWorkMeter::new(policy);
            let mut out = BoundedSvgOutput::new(&meter);
            {
                let mut text = XmlTextOutput::new(&mut out);
                for fragment in &fragments {
                    text.push_str(fragment);
                }
            }
            assert_eq!(out.as_str(), retained);
            let first = resource_limit(out.checkpoint().unwrap_err());
            assert_eq!(first.actual, actual);
            assert_eq!(first.max, maximum);
            assert_eq!(resource_limit(out.finish().unwrap_err()), first);
        }
    }

    #[test]
    fn xml_text_output_observes_cancellation_before_retaining_encoded_text() {
        use merman_core::OperationControl;

        let control = OperationControl::new();
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control.clone(),
        );
        let mut out = BoundedSvgOutput::new(&meter);
        control.cancel();
        {
            let mut css_out = XmlTextOutput::new(&mut out);
            css_out.push_str("&<]]>");
            assert!(matches!(
                css_out.checkpoint(),
                Err(crate::Error::Cancelled(_))
            ));
            assert!(css_out.as_str().is_empty());
        }
        assert!(matches!(out.finish(), Err(crate::Error::Cancelled(_))));
    }

    #[test]
    fn xml_text_output_long_plain_fragment_observes_cancellation_during_emission() {
        use merman_core::OperationControl;

        let control = OperationControl::new();
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control.clone(),
        );
        let mut out = BoundedSvgOutput::new(&meter);
        control.cancel_after_checkpoints(2);
        let logical = "ordinary-css-font-name".repeat(16_384);
        {
            let mut css_out = XmlTextOutput::new(&mut out);
            assert!(fmt::Write::write_str(&mut css_out, &logical).is_err());
            assert!(matches!(
                css_out.checkpoint(),
                Err(crate::Error::Cancelled(_))
            ));
            assert!(css_out.len() < logical.len());
        }
        assert!(matches!(out.finish(), Err(crate::Error::Cancelled(_))));
    }

    #[test]
    fn xml_text_output_stops_mid_unicode_fragment_and_preserves_the_first_failure() {
        use merman_core::OperationControl;

        let control = OperationControl::new();
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control.clone(),
        );
        let mut out = BoundedSvgOutput::new(&meter);
        control.cancel_after_checkpoints(3);
        let logical = "字體Ω😀 plain CSS ]".repeat(16_384);
        {
            let mut css_out = XmlTextOutput::new(&mut out);
            assert!(fmt::Write::write_str(&mut css_out, &logical).is_err());
            assert!(matches!(
                css_out.checkpoint(),
                Err(crate::Error::Cancelled(_))
            ));
        }
        assert!(!out.as_str().is_empty());
        assert!(out.len() < logical.len());
        assert!(logical.starts_with(out.as_str()));
        let retained = out.as_str().to_owned();
        {
            let mut css_out = XmlTextOutput::new(&mut out);
            assert!(fmt::Write::write_str(&mut css_out, "&<]]>").is_err());
            assert_eq!(css_out.as_str(), retained);
            assert!(matches!(
                css_out.checkpoint(),
                Err(crate::Error::Cancelled(_))
            ));
        }
        assert!(matches!(out.finish(), Err(crate::Error::Cancelled(_))));
    }

    #[test]
    fn xml_text_output_removes_forbidden_scalars_without_creating_cdata_terminators() {
        let mut out = String::from("<svg><style>");
        {
            let mut css = XmlTextOutput::new(&mut out);
            for fragment in ["A\0&\u{1f}", "<Ω]\u{fffe}", "]\u{ffff}>", "\t\n\rB"] {
                css.push_str(fragment);
            }
        }
        out.push_str("</style></svg>");
        assert_eq!(out, "<svg><style>A&amp;&lt;Ω]]&gt;\t\n\rB</style></svg>");
        crate::xml_dom::Document::parse(&out).expect("the producer must emit valid XML");
    }

    #[test]
    fn xml_text_output_observes_cancellation_while_removing_forbidden_scalars() {
        use merman_core::OperationControl;

        let control = OperationControl::new();
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control.clone(),
        );
        let mut out = BoundedSvgOutput::new(&meter);
        control.cancel_after_checkpoints(2);
        let logical = "\0".repeat(16_384);
        {
            let mut css = XmlTextOutput::new(&mut out);
            assert!(fmt::Write::write_str(&mut css, &logical).is_err());
        }
        assert!(out.as_str().is_empty());
        assert!(matches!(out.finish(), Err(crate::Error::Cancelled(_))));
    }

    #[test]
    fn xml_text_output_preserves_scalar_and_fragment_boundaries() {
        for padding in [0, 1, 15, 16, 17, 31, 32, 33, 2047, 2048, 2049] {
            for suffix in [
                "]]>尾",
                "]]\0>",
                "]]\u{fffe}>",
                "\u{ffff}&<😀",
                "\u{feff}合法>",
            ] {
                let logical = format!("{}{suffix}", "a".repeat(padding));
                let expected = crate::xml::strip_forbidden_xml_1_0_chars(&logical)
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace("]]>", "]]&gt;");
                for split in padding..=logical.len() {
                    if !logical.is_char_boundary(split) {
                        continue;
                    }
                    let mut serialized = String::new();
                    let mut out = XmlTextOutput::new(&mut serialized);
                    out.push_str(&logical[..split]);
                    out.push_str(&logical[split..]);
                    assert_eq!(serialized, expected, "padding={padding}, split={split}");
                }
            }
        }
    }

    #[test]
    fn xml_text_output_rejects_an_entity_before_retaining_or_reserving_it() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 8)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let mut out = BoundedSvgOutput::new(&meter);
        out.push_str("safe");
        let capacity = out.retained_capacity();
        XmlTextOutput::new(&mut out).push_str("&");
        assert_eq!(out.as_str(), "safe");
        assert_eq!(out.retained_capacity(), capacity);
        let error = resource_limit(out.finish().unwrap_err());
        assert_eq!(error.actual, 9);
        assert_eq!(error.max, 8);
        assert_eq!(meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn xml_text_output_observes_cancellation_during_dense_entity_expansion() {
        use merman_core::OperationControl;

        let control = OperationControl::new();
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control.clone(),
        );
        let mut out = BoundedSvgOutput::new(&meter);
        let logical = "&".repeat(16_384);
        control.cancel_after_checkpoints(3);
        XmlTextOutput::new(&mut out).push_str(&logical);
        assert!(!out.as_str().is_empty());
        assert!(out.len() < logical.len() * 5);
        assert!(out.as_str().split("&amp;").all(str::is_empty));
        let retained = out.as_str().to_owned();
        XmlTextOutput::new(&mut out).push_str("<");
        assert_eq!(out.as_str(), retained);
        assert!(matches!(out.finish(), Err(crate::Error::Cancelled(_))));
    }

    #[test]
    fn bounded_svg_output_rejects_before_retaining_an_over_limit_append() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 10)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let mut out = BoundedSvgOutput::new(&meter);

        out.push_str("<svg>");
        out.push_str("12345");
        assert_eq!(out.as_str(), "<svg>12345");
        assert_eq!(meter.projected_svg_bytes(), 0);
        let capacity_before_rejection = out.retained_capacity();

        out.push_str("x");
        assert_eq!(out.as_str(), "<svg>12345");
        assert_eq!(out.retained_capacity(), capacity_before_rejection);
        let error = out.finish().unwrap_err();
        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
    }

    #[test]
    fn bounded_svg_output_keeps_the_first_failure_after_every_checkpoint() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 10)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let mut out = BoundedSvgOutput::new(&meter);

        out.push_str("<svg>12345");
        out.push('x');

        let first = resource_limit(out.checkpoint().unwrap_err());
        let repeated = resource_limit(out.checkpoint().unwrap_err());
        assert_eq!(repeated, first);

        let replacement = out.replace_range(0..5, "").unwrap_err();
        assert_eq!(resource_limit(replacement), first);
        assert_eq!(out.as_str(), "<svg>12345");
        assert_eq!(meter.projected_svg_bytes(), 0);

        let finished = resource_limit(out.finish().unwrap_err());
        assert_eq!(finished, first);
        assert_eq!(meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn bounded_svg_output_observes_cancellation_before_returning_completed_document() {
        let control = merman_core::OperationControl::new();
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control.clone(),
        );
        let mut out = BoundedSvgOutput::new(&meter);
        out.push_str("<svg/>");
        control.cancel();
        assert!(matches!(out.finish(), Err(crate::Error::Cancelled(_))));
    }

    #[test]
    fn bounded_svg_output_geometrically_reserves_repeated_short_writes() {
        const MAX_SVG_BYTES: usize = 64;

        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, MAX_SVG_BYTES)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let mut out = BoundedSvgOutput::new(&meter);
        let mut capacity_changes = 0;
        let mut previous_capacity = out.retained_capacity();

        for _ in 0..MAX_SVG_BYTES {
            out.push('x');
            let capacity = out.retained_capacity();
            assert!(capacity <= MAX_SVG_BYTES);
            if capacity != previous_capacity {
                capacity_changes += 1;
                previous_capacity = capacity;
            }
        }

        assert!(capacity_changes <= 8);
        assert_eq!(out.write_attempts, MAX_SVG_BYTES);
        assert!(out.capacity_admissions <= 8);
        assert_eq!(meter.projected_svg_bytes(), 0);
        assert_eq!(out.finish().unwrap(), "x".repeat(MAX_SVG_BYTES));
        assert_eq!(meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn bounded_svg_output_allocator_capacity_never_grants_byte_credit() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 10)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let mut out = BoundedSvgOutput::new(&meter);
        // Model an allocator supplying more storage than requested by an exact reservation.
        out.out = String::with_capacity(64);
        out.push_str("0123456789");
        assert_eq!(out.admitted_end, 10);
        out.push('x');
        assert_eq!(out.as_str(), "0123456789");
        let error = resource_limit(out.finish().unwrap_err());
        assert_eq!(error.actual, 11);
        assert_eq!(error.max, 10);
    }

    #[test]
    fn bounded_svg_output_empty_start_and_replacements_preserve_byte_credit() {
        use merman_core::OperationControl;

        for maximum in [1, 10] {
            let policy = RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, maximum)
                .unwrap();
            let control = OperationControl::new();
            let meter = OperationWorkMeter::new_with_control(policy, control.clone());
            let mut out = BoundedSvgOutput::new(&meter);
            assert!(fmt::Write::write_str(&mut out, "").is_ok());
            assert_eq!(out.admitted_end, 0);
            assert_eq!(out.retained_capacity(), 0);
            if maximum == 10 {
                out.push_str("0123456789");
                out.replace_range(0..5, "").unwrap();
                out.replace_range(0..1, "012345").unwrap();
                assert_eq!(out.as_str(), "0123456789");
                assert_eq!(out.admitted_end, maximum);
            } else {
                out.push('x');
                assert_eq!(out.as_str(), "x");
                assert_eq!(out.admitted_end, 1);
            }
            out.push('x');
            let first = resource_limit(out.checkpoint().unwrap_err());
            assert_eq!(first.actual, maximum + 1);
            assert_eq!(first.max, maximum);
            control.cancel();
            assert_eq!(resource_limit(out.observe_operation().unwrap_err()), first);
            assert_eq!(resource_limit(out.finish().unwrap_err()), first);
            assert_eq!(meter.projected_svg_bytes(), 0);
        }
    }

    fn resource_limit(error: crate::Error) -> crate::ResourceLimitExceeded {
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected bounded SVG output to preserve its resource-limit failure");
        };
        error
    }
}
