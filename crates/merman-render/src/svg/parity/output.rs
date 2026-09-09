//! Fallible SVG buffers shared by canonical emission and root construction.

use crate::environment::RenderSession;
use crate::resources::ResourceLimitPhase;
use crate::{Error, Result};
use merman_core::OperationPhase;
use std::fmt;

pub(super) trait SvgBuffer {
    fn append(&mut self, text: &str) -> Result<()>;
    fn as_str(&self) -> &str;

    fn checkpoint(&self) -> Result<()> {
        Ok(())
    }

    fn len(&self) -> usize {
        self.as_str().len()
    }

    fn append_char(&mut self, character: char) -> Result<()> {
        self.append(character.encode_utf8(&mut [0; 4]))
    }

    fn append_fmt(&mut self, arguments: fmt::Arguments<'_>) -> Result<()> {
        struct Adapter<'a, T: ?Sized> {
            buffer: &'a mut T,
            error: Option<Error>,
        }
        impl<T: SvgBuffer + ?Sized> fmt::Write for Adapter<'_, T> {
            fn write_str(&mut self, text: &str) -> fmt::Result {
                if self.error.is_some() {
                    return Err(fmt::Error);
                }
                self.buffer.append(text).map_err(|error| {
                    self.error = Some(error);
                    fmt::Error
                })
            }
        }
        let mut adapter = Adapter {
            buffer: self,
            error: None,
        };
        let result = fmt::write(&mut adapter, arguments);
        if let Some(error) = adapter.error {
            return Err(error);
        }
        self.checkpoint()?;
        result.map_err(|_| Error::InvalidModel {
            message: "SVG value formatting failed".into(),
        })
    }
}

impl SvgBuffer for String {
    fn append(&mut self, text: &str) -> Result<()> {
        self.push_str(text);
        Ok(())
    }

    fn as_str(&self) -> &str {
        self.as_str()
    }
}

pub(super) struct SvgOutput<'a> {
    text: String,
    session: &'a RenderSession,
}

impl<'a> SvgOutput<'a> {
    pub(super) fn new(session: &'a RenderSession) -> Self {
        Self {
            text: String::new(),
            session,
        }
    }

    pub(super) fn push_str(&mut self, text: &str) -> Result<()> {
        self.append(text)
    }

    pub(super) fn push(&mut self, character: char) -> Result<()> {
        self.append_char(character)
    }

    pub(super) fn pop(&mut self) -> Option<char> {
        self.text.pop()
    }

    pub(super) fn write_fmt(&mut self, arguments: fmt::Arguments<'_>) -> Result<()> {
        self.append_fmt(arguments)
    }

    pub(super) fn finish(self) -> Result<String> {
        self.session.checkpoint(OperationPhase::Emit)?;
        Ok(self.text)
    }
}

impl SvgBuffer for SvgOutput<'_> {
    fn checkpoint(&self) -> Result<()> {
        self.session.checkpoint(OperationPhase::Emit)
    }

    fn append(&mut self, text: &str) -> Result<()> {
        let meter = self.session.work_meter();
        let length = self.text.len().checked_add(text.len()).ok_or_else(|| {
            Error::from(meter.terminate_svg_byte_count_overflow(
                ResourceLimitPhase::SvgOutput,
                OperationPhase::Emit,
            ))
        })?;
        meter.preflight_svg_byte_count(
            length,
            ResourceLimitPhase::SvgOutput,
            OperationPhase::Emit,
        )?;
        self.text
            .try_reserve(text.len())
            .map_err(|error| Error::InvalidModel {
                message: format!("failed to allocate SVG output: {error}"),
            })?;
        let mut remaining = text;
        while !remaining.is_empty() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let mut end = remaining.len().min(16 * 1024);
            while !remaining.is_char_boundary(end) {
                end -= 1;
            }
            self.text.push_str(&remaining[..end]);
            remaining = &remaining[end..];
        }
        Ok(())
    }

    fn as_str(&self) -> &str {
        &self.text
    }
}

pub(super) fn escape_attr(out: &mut impl SvgBuffer, text: impl fmt::Display) -> Result<()> {
    out.append_fmt(format_args!("{}", super::util::escape_attr_display(text)))
}

/// Writes already resolved document text. Source entity decoding belongs to its producer.
pub(super) fn escape_xml(out: &mut impl SvgBuffer, text: &str) -> Result<()> {
    out.append_fmt(format_args!(
        "{}",
        super::util::escape_xml_raw_display(text)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::RenderEnvironment;
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    use merman_core::OperationControl;

    #[test]
    fn output_escapes_public_text_without_decoding_entity_spellings() {
        let environment = RenderEnvironment::deterministic();
        let session = environment.begin_session().unwrap();
        for text in [
            "#; #quot; #35;quot; ﬂ°quot¶ß",
            "&nbsp; &#160; &amp; <literal> A]]>B",
            "中文 é 😀",
        ] {
            let mut output = SvgOutput::new(&session);
            output.push_str("<text>").unwrap();
            escape_xml(&mut output, text).unwrap();
            output.push_str("</text>").unwrap();
            let svg = output.finish().unwrap();
            let xml = roxmltree::Document::parse(&svg).unwrap();
            assert_eq!(xml.root_element().text(), Some(text));
        }
    }

    #[test]
    fn output_admits_exact_bytes_without_double_charging_icon_reservations() {
        let environment = RenderEnvironment::deterministic().with_resource_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, 9)
                .unwrap(),
        );
        let session = environment
            .begin_session_with_control(OperationControl::new())
            .unwrap();
        session.work_meter().charge_svg_bytes(5).unwrap();
        let mut output = SvgOutput::new(&session);
        output.push_str("12345").unwrap();
        escape_attr(&mut output, "<").unwrap();
        assert_eq!(output.finish().unwrap(), "12345&lt;");
    }

    #[test]
    fn output_preserves_resource_errors_and_does_not_append_rejected_chunks() {
        let environment = RenderEnvironment::deterministic().with_resource_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, 8)
                .unwrap(),
        );
        let control = OperationControl::new();
        let session = environment
            .begin_session_with_control(control.clone())
            .unwrap();
        let mut output = SvgOutput::new(&session);
        output.push_str("12345").unwrap();
        let error = escape_attr(&mut output, "<").unwrap_err();
        assert!(matches!(error, Error::ResourceLimitExceeded(_)));
        assert_eq!(output.as_str(), "12345");
        control.cancel();
        assert!(matches!(
            output.finish(),
            Err(Error::ResourceLimitExceeded(_))
        ));
    }

    #[test]
    fn output_cancellation_stops_a_large_write_and_prevents_finish() {
        let environment = RenderEnvironment::deterministic();
        let control = OperationControl::new();
        let session = environment
            .begin_session_with_control(control.clone())
            .unwrap();
        let mut output = SvgOutput::new(&session);
        control.cancel_after_checkpoints(3);
        assert!(matches!(
            output.push_str(&"é".repeat(64 * 1024)),
            Err(Error::Cancelled(_))
        ));
        assert!(output.len() < 128 * 1024);
        assert!(matches!(output.finish(), Err(Error::Cancelled(_))));
    }
}
