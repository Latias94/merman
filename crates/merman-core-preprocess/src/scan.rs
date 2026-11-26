//! Controlled token and line-ending primitives for Mermaid lexers.

use merman_model::{OperationControl, OperationControlResult};

#[doc(hidden)]
pub mod controlled;
#[doc(hidden)]
pub mod langium;
mod lines;

#[doc(hidden)]
pub use lines::{LineCursor, strip_line_ending};

#[cfg(feature = "mermaid-unicode")]
mod jison_unicode;
#[cfg(feature = "mermaid-unicode")]
pub use jison_unicode::is_mermaid_unicode_text;

pub fn is_ecmascript_inline_whitespace(ch: char) -> bool {
    merman_model::text::is_ecmascript_whitespace(ch) && !matches!(ch, '\r' | '\n')
}

/// One rule dispatch. A skipped rule must consume input or leave its lexical state.
/// Empty tokens are valid when they drain pending output or complete a state transition.
pub enum ScanStep<T> {
    Emit(T),
    Skip,
    End,
}

pub trait TokenScanner {
    type Event;

    fn scan_step(&mut self) -> ScanStep<Self::Event>;

    fn next_token(&mut self) -> Option<Self::Event> {
        loop {
            match self.scan_step() {
                ScanStep::Emit(event) => return Some(event),
                ScanStep::Skip => {}
                ScanStep::End => return None,
            }
        }
    }
}

/// A forward cursor over spanned lexical events, with one buffered lookahead.
pub struct TokenStream<'a, I: Iterator> {
    events: std::iter::Peekable<I>,
    control: &'a OperationControl,
    fetched: usize,
    checked_lookahead: bool,
    previous_end: usize,
}

impl<'a, I, T, E> TokenStream<'a, I>
where
    I: Iterator<Item = Result<(usize, T, usize), E>>,
{
    pub fn new(events: I, control: &'a OperationControl) -> Self {
        Self {
            events: events.peekable(),
            control,
            fetched: 0,
            checked_lookahead: false,
            previous_end: 0,
        }
    }

    pub fn peek(&mut self) -> OperationControlResult<Option<&I::Item>> {
        let event = self.events.peek();
        if !self.checked_lookahead && event.is_some() {
            if self.fetched.is_multiple_of(128) {
                self.control.checkpoint()?;
            }
            self.checked_lookahead = true;
            self.fetched = self.fetched.saturating_add(1);
        }
        Ok(event)
    }

    pub fn bump(&mut self) -> OperationControlResult<Option<I::Item>> {
        if self.peek()?.is_none() {
            return Ok(None);
        }
        let event = self.events.next();
        self.checked_lookahead = false;
        if let Some(Ok((_, _, end))) = &event {
            self.previous_end = *end;
        }
        Ok(event)
    }

    /// The end of the last consumed token, excluding skipped input.
    pub const fn previous_end(&self) -> usize {
        self.previous_end
    }
}

pub fn collect_lexical_events<S: TokenScanner>(
    scanner: &mut S,
    control: &OperationControl,
) -> OperationControlResult<Vec<S::Event>> {
    let mut events = Vec::new();
    let mut steps = 0usize;
    loop {
        if steps.is_multiple_of(128) {
            control.checkpoint()?;
        }
        match scanner.scan_step() {
            ScanStep::Emit(event) => events.push(event),
            ScanStep::Skip => {}
            ScanStep::End => break,
        }
        steps = steps.wrapping_add(1);
    }
    control.checkpoint()?;
    Ok(events)
}

pub fn consume_line_ending(source: &str, offset: usize) -> Option<usize> {
    match source.as_bytes().get(offset).copied()? {
        b'\r' if source.as_bytes().get(offset + 1) == Some(&b'\n') => Some(offset + 2),
        b'\r' | b'\n' => Some(offset + 1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::consume_line_ending;

    #[test]
    fn consume_line_ending_accepts_cr_lf_and_crlf() {
        assert_eq!(consume_line_ending("\r", 0), Some(1));
        assert_eq!(consume_line_ending("\n", 0), Some(1));
        assert_eq!(consume_line_ending("\r\n", 0), Some(2));
        assert_eq!(consume_line_ending("x", 0), None);
    }
}
