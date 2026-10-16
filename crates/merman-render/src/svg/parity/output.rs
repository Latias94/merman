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
/// remain literal in character data, and forbidden XML scalars remain visible to final validation.
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

impl<S: SvgOutput> fmt::Write for XmlTextOutput<'_, S> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        const MAX_PLAIN_FRAGMENT_BYTES: usize = 2_048;

        self.out.observe_operation().map_err(|_| fmt::Error)?;
        let mut search_start = 0;
        let mut plain_start = 0usize;
        while search_start < value.len() {
            let mut end = plain_start
                .saturating_add(MAX_PLAIN_FRAGMENT_BYTES)
                .min(value.len());
            while !value.is_char_boundary(end) {
                end += 1;
            }
            // Bound scanning as well as copying. UTF-8 completion adds at most three bytes to
            // the old observation interval; no whole-input pre-scan can postpone cancellation.
            if let Some(relative) =
                memchr::memchr3(b'&', b'<', b'>', &value.as_bytes()[search_start..end])
            {
                let offset = search_start + relative;
                self.retain_trailing_brackets(&value[search_start..offset]);
                let replacement = match value.as_bytes()[offset] {
                    b'&' => Some("&amp;"),
                    b'<' => Some("&lt;"),
                    b'>' if self.trailing_brackets == 2 => Some("&gt;"),
                    _ => None,
                };
                self.trailing_brackets = 0;
                if let Some(replacement) = replacement {
                    if plain_start < offset {
                        self.out.write_str(&value[plain_start..offset])?;
                    }
                    self.out.write_str(replacement)?;
                    plain_start = offset + 1;
                }
                search_start = offset + 1;
            } else {
                self.retain_trailing_brackets(&value[search_start..end]);
                self.out.write_str(&value[plain_start..end])?;
                search_start = end;
                plain_start = end;
            }
        }
        if plain_start < value.len() {
            self.out.write_str(&value[plain_start..])?;
        }
        self.out.checkpoint().map_err(|_| fmt::Error)
    }
}

impl<S> XmlTextOutput<'_, S> {
    fn retain_trailing_brackets(&mut self, plain: &str) {
        self.trailing_brackets = if plain.ends_with("]]") {
            2
        } else if plain == "]" {
            (self.trailing_brackets + 1).min(2)
        } else if plain.ends_with(']') {
            1
        } else if plain.is_empty() {
            self.trailing_brackets
        } else {
            0
        };
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
            None => Ok(self.out),
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
            if let Err(failure) = self.admit_growth(growth) {
                return Err(self.fail(failure));
            }
        }
        self.out.replace_range(range, replacement);
        Ok(())
    }

    fn admit_growth(&mut self, additional: usize) -> Result<(), BoundedSvgOutputFailure> {
        self.budget
            .observe_operation()
            .map_err(BoundedSvgOutputFailure::Operation)?;
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

    fn record_write(&mut self, value: &str) -> fmt::Result {
        #[cfg(test)]
        {
            self.write_attempts += 1;
        }
        if matches!(&self.state, BoundedSvgOutputState::Failed(_)) {
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
        self.record_write(value)
    }
}

impl SvgOutput for BoundedSvgOutput<'_> {
    fn push_str(&mut self, value: &str) {
        let _ = self.record_write(value);
    }

    fn push(&mut self, value: char) {
        let mut encoded = [0u8; 4];
        let _ = self.record_write(value.encode_utf8(&mut encoded));
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
    fn xml_text_output_keeps_forbidden_scalars_for_final_xml_validation() {
        for forbidden in ['\0', '\u{1f}', '\u{fffe}', '\u{ffff}'] {
            let mut out = String::from("<svg><style>");
            XmlTextOutput::new(&mut out).push(forbidden);
            out.push_str("</style></svg>");
            assert!(out.contains(forbidden));
            let error = crate::svg::pipeline::validate_well_formed_svg_with_controls(
                &out,
                &mut || Ok(()),
                &mut |_, _| Ok(()),
            )
            .expect_err(&format!(
                "XML scalar U+{:04X} must fail final validation",
                forbidden as u32
            ));
            assert!(
                matches!(error, crate::Error::SvgPostprocess { pass, .. } if pass == "validate-well-formed-svg")
            );
        }
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

    #[test]
    fn xml_text_output_matches_reference_output_errors_and_cancellation_boundaries() {
        use merman_core::OperationControl;

        let fragments = [
            String::new(),
            "a]".to_owned(),
            "]".to_owned(),
            "> > & <".to_owned(),
            format!("{}😀Ω]", "p".repeat(2_047)),
            "]> literal &amp; \0".to_owned(),
            format!("&{}]", ">\u{0301}字".repeat(1_025)),
            "]".to_owned(),
            String::new(),
            ">".to_owned(),
        ];
        let mut expected = String::new();
        let mut trailing_brackets = 0;
        for fragment in &fragments {
            reference_xml_write(&mut expected, &mut trailing_brackets, fragment).unwrap();
        }
        for maximum in [
            1,
            2,
            7,
            2_047,
            2_048,
            2_049,
            expected.len() - 1,
            expected.len(),
        ] {
            for checkpoints in [0, 1, 2, 3, 5, 9, usize::MAX] {
                let policy = RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxSvgBytes, maximum)
                    .unwrap();
                let new_control = OperationControl::new();
                let reference_control = OperationControl::new();
                if checkpoints != usize::MAX {
                    new_control.cancel_after_checkpoints(checkpoints);
                    reference_control.cancel_after_checkpoints(checkpoints);
                }
                let new_meter = OperationWorkMeter::new_with_control(policy, new_control);
                let reference_meter =
                    OperationWorkMeter::new_with_control(policy, reference_control);
                let mut new = BoundedSvgOutput::new(&new_meter);
                let mut reference = BoundedSvgOutput::new(&reference_meter);
                let mut reference_brackets = 0;
                {
                    let mut xml = XmlTextOutput::new(&mut new);
                    for fragment in &fragments {
                        let actual = fmt::Write::write_str(&mut xml, fragment);
                        let previous =
                            reference_xml_write(&mut reference, &mut reference_brackets, fragment);
                        assert_eq!(actual.is_ok(), previous.is_ok());
                    }
                }
                assert_eq!(new.as_str(), reference.as_str());
                assert_eq!(new_meter.projected_svg_bytes(), 0);
                assert_eq!(reference_meter.projected_svg_bytes(), 0);
                assert_eq!(
                    new.finish().map_err(|error| format!("{error:?}")),
                    reference.finish().map_err(|error| format!("{error:?}")),
                    "maximum={maximum}, checkpoints={checkpoints}",
                );
            }
        }
    }

    #[test]
    fn xml_text_output_observation_replaces_empty_sink_write() {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let mut new = BoundedSvgOutput::new(&meter);
        let mut reference = BoundedSvgOutput::new(&meter);
        let mut reference_brackets = 0;
        {
            let mut xml = XmlTextOutput::new(&mut new);
            for fragment in ["font-family:", "普通字體", "", ";"] {
                fmt::Write::write_str(&mut xml, fragment).unwrap();
                reference_xml_write(&mut reference, &mut reference_brackets, fragment).unwrap();
            }
        }
        assert_eq!(new.as_str(), reference.as_str());
        assert_eq!(new.write_attempts, 3);
        assert_eq!(reference.write_attempts, 7);
    }

    // The former scalar serializer remains a test oracle for exact write/error boundaries.
    fn reference_xml_write(
        out: &mut impl SvgOutput,
        trailing_brackets: &mut u8,
        value: &str,
    ) -> fmt::Result {
        out.write_str("")?;
        let mut start = 0;
        for (offset, byte) in value.bytes().enumerate() {
            let replacement = match byte {
                b'&' => Some("&amp;"),
                b'<' => Some("&lt;"),
                b'>' if *trailing_brackets == 2 => Some("&gt;"),
                _ => None,
            };
            *trailing_brackets = if byte == b']' {
                (*trailing_brackets + 1).min(2)
            } else {
                0
            };
            if let Some(replacement) = replacement {
                if start < offset {
                    out.write_str(&value[start..offset])?;
                }
                out.write_str(replacement)?;
                start = offset + 1;
            } else if offset + 1 - start >= 2_048 && value.is_char_boundary(offset + 1) {
                out.write_str(&value[start..offset + 1])?;
                start = offset + 1;
            }
        }
        if start < value.len() {
            out.write_str(&value[start..])?;
        }
        out.checkpoint().map_err(|_| fmt::Error)
    }

    fn resource_limit(error: crate::Error) -> crate::ResourceLimitExceeded {
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected bounded SVG output to preserve its resource-limit failure");
        };
        error
    }
}
