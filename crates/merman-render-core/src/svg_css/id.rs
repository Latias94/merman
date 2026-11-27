use std::cell::{Cell, RefCell};
use std::fmt::{self, Write as _};

use merman_model::OperationPhase;

use super::SvgDiagramIdValue;
use crate::{
    Error, Result,
    resources::{OperationWorkMeter, ResourceLimitPhase},
};

pub(super) const DROP_SHADOW_ID: &str = "drop-shadow";
pub(super) const DROP_SHADOW_URL: &str = "url(#drop-shadow)";

/// Cumulative operation admission for diagram ID occurrences.
pub struct SvgDiagramIdProjection<'a> {
    work_meter: &'a OperationWorkMeter,
    projected_bytes: Cell<usize>,
    error: RefCell<Option<Error>>,
}

impl<'a> SvgDiagramIdProjection<'a> {
    pub fn new(work_meter: &'a OperationWorkMeter) -> Self {
        Self {
            work_meter,
            projected_bytes: Cell::new(0),
            error: RefCell::new(None),
        }
    }

    /// Reports the original operation error after formatting has ended.
    pub fn finish(&self) -> Result<()> {
        match self.error.borrow_mut().take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn admit(&self, bytes: usize) -> bool {
        if self.error.borrow().is_some() {
            return false;
        }
        let Some(projected_bytes) = self.projected_bytes.get().checked_add(bytes) else {
            self.error.replace(Some(
                self.work_meter
                    .terminate_svg_byte_count_overflow(
                        ResourceLimitPhase::SvgOutput,
                        OperationPhase::Emit,
                    )
                    .into(),
            ));
            return false;
        };
        match self.work_meter.preflight_svg_byte_count(
            projected_bytes,
            ResourceLimitPhase::SvgOutput,
            OperationPhase::Emit,
        ) {
            Ok(()) => {
                self.projected_bytes.set(projected_bytes);
                true
            }
            Err(error) => {
                self.error.replace(Some(error.into()));
                false
            }
        }
    }
}

/// A diagram ID that can be written in XML attributes, CSS selectors, and CSS fragment URLs.
#[derive(Clone, Copy)]
pub struct SvgId<'a> {
    value: &'a str,
    projection: Option<&'a SvgDiagramIdProjection<'a>>,
}

impl<'a> SvgId<'a> {
    /// Checks an already normalized ID once before reuse across output contexts.
    pub const fn new(value: &'a str) -> Option<Self> {
        if is_normalized_svg_diagram_id(value) {
            Some(Self {
                value,
                projection: None,
            })
        } else {
            None
        }
    }

    /// Returns the ID for semantic comparisons. Formatting performs occurrence admission.
    pub const fn semantic_str(self) -> &'a str {
        self.value
    }

    /// Attaches a projection that records the operation error when admission fails.
    pub fn with_projection(mut self, projection: &'a SvgDiagramIdProjection<'a>) -> Self {
        self.projection = Some(projection);
        self
    }
}

impl fmt::Debug for SvgId<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SvgDiagramId")
            .field(&self.value)
            .finish()
    }
}

impl fmt::Display for SvgId<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(projection) = self.projection
            && !projection.admit(self.value.len())
        {
            return Ok(());
        }
        formatter.write_str(self.value)
    }
}

impl SvgDiagramIdValue for SvgId<'_> {
    fn semantic_value(&self) -> &str {
        self.value
    }

    fn validated_svg_id(&self) -> Option<SvgId<'_>> {
        Some(*self)
    }
}

/// Formats a diagram ID as a logical CSS selector identifier.
#[derive(Clone, Copy)]
pub struct CssSelectorDiagramId<I>(pub I);

impl<I> std::fmt::Display for CssSelectorDiagramId<I>
where
    I: SvgDiagramIdValue,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(id) = self.0.validated_svg_id() {
            return fmt::Display::fmt(&id, formatter);
        }
        formatter.write_str(&super::escape_css_identifier(self.0.semantic_value()))
    }
}

/// Formats an ID inside an unquoted CSS URL, before the output sink encodes XML text.
#[derive(Clone, Copy)]
pub struct FragmentDiagramId<I>(pub I);

impl<I> std::fmt::Display for FragmentDiagramId<I>
where
    I: SvgDiagramIdValue,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(id) = self.0.validated_svg_id() {
            return fmt::Display::fmt(&id, formatter);
        }
        let value = self.0.semantic_value();
        if is_normalized_svg_diagram_id(value) {
            return formatter.write_str(value);
        }
        // An unquoted CSS URL has a different token grammar from a selector identifier.
        // Keep punctuation such as `:` and `.` literal; XML metacharacters belong to the sink.
        for character in value.chars() {
            if character <= '\u{20}' || character == '\u{7f}' {
                write!(formatter, "\\{:x} ", character as u32)?;
            } else {
                if matches!(character, '"' | '\'' | '(' | ')' | '\\') {
                    formatter.write_str("\\")?;
                }
                formatter.write_char(character)?;
            }
        }
        Ok(())
    }
}

const fn is_normalized_svg_diagram_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_alphabetic() {
        return false;
    }
    let mut index = 1;
    while index < bytes.len() {
        let byte = bytes[index];
        if !byte.is_ascii_alphanumeric() && !matches!(byte, b'-' | b'_') {
            return false;
        }
        index += 1;
    }
    true
}

#[derive(Clone, Copy)]
pub struct ScopedSvgId<'a, I> {
    diagram_id: I,
    local_id: &'a str,
}

impl<I: std::fmt::Display> std::fmt::Display for ScopedSvgId<'_, I> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}-{}", self.diagram_id, self.local_id)
    }
}

/// Joins logical ID fragments. The caller chooses escaping for the destination context.
pub fn scoped_svg_id<I>(diagram_id: I, local_id: &str) -> ScopedSvgId<'_, I> {
    ScopedSvgId {
        diagram_id,
        local_id,
    }
}

#[cfg(feature = "css-style")]
/// Chooses a fresh diagram-local ID from a stem unique to the element being renamed.
pub fn available_local_svg_id(
    stem: String,
    occupied_count: usize,
    is_occupied: impl Fn(&str) -> bool,
    work_meter: &OperationWorkMeter,
) -> Result<String> {
    work_meter.charge(stem.len())?;
    if !is_occupied(&stem) {
        return Ok(stem);
    }
    for suffix in 0..=occupied_count {
        let candidate = format!("{stem}-{suffix}");
        work_meter.charge(candidate.len())?;
        if !is_occupied(&candidate) {
            return Ok(candidate);
        }
    }
    Err(Error::InvalidModel {
        message: "exhausted local SVG IDs".into(),
    })
}

#[derive(Clone, Copy)]
pub struct ScopedSvgUrl<'a, I> {
    diagram_id: I,
    local_id: &'a str,
}

impl<I: std::fmt::Display> std::fmt::Display for ScopedSvgUrl<'_, I> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "url(#{})",
            scoped_svg_id(&self.diagram_id, self.local_id)
        )
    }
}

/// Joins fragments already formatted for an unquoted CSS URL into a logical CSS value.
/// Callers embedding the result in XML must apply the destination's XML encoding.
pub fn scoped_svg_url<I>(diagram_id: I, local_id: &str) -> ScopedSvgUrl<'_, I> {
    ScopedSvgUrl {
        diagram_id,
        local_id,
    }
}

#[derive(Clone, Copy)]
pub struct ScopedDropShadow<'a, I> {
    diagram_id: I,
    source: &'a str,
}

impl<I> std::fmt::Display for ScopedDropShadow<'_, I>
where
    I: Copy + std::fmt::Display,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = self.source.split(DROP_SHADOW_URL);
        formatter.write_str(parts.next().unwrap_or_default())?;
        for suffix in parts {
            write!(
                formatter,
                "{}",
                scoped_svg_url(self.diagram_id, DROP_SHADOW_ID)
            )?;
            formatter.write_str(suffix)?;
        }
        Ok(())
    }
}

/// Substitutes a local fragment in a logical CSS value using a CSS URL-formatted ID.
pub fn scoped_drop_shadow<I>(diagram_id: I, source: &str) -> ScopedDropShadow<'_, I> {
    ScopedDropShadow { diagram_id, source }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_id_requires_a_shared_xml_and_css_spelling() {
        let id = SvgId::new("Pie-12_a").unwrap();
        assert_eq!(id.to_string(), "Pie-12_a");
        for value in ["", "12-pie", "Pie:12", "Pie&12", "Pie(12)", "Pie 12"] {
            assert!(SvgId::new(value).is_none(), "{value:?}");
        }
    }

    #[derive(Clone, Copy)]
    struct DivergentDisplay;

    impl fmt::Display for DivergentDisplay {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("other ID")
        }
    }

    impl SvgDiagramIdValue for DivergentDisplay {
        fn semantic_value(&self) -> &str {
            "pie"
        }
    }

    #[test]
    fn css_id_adapters_emit_the_validated_spelling() {
        assert_eq!(CssSelectorDiagramId(DivergentDisplay).to_string(), "pie");
        assert_eq!(FragmentDiagramId(DivergentDisplay).to_string(), "pie");
    }
}
