use crate::{DrawingListError, DrawingListLimits};
use std::marker::PhantomData;

/// An admission or cancellation checkpoint during document validation.
///
/// Counts are cumulative within their resource category, except that nested resources may also
/// report a smaller local count. They are observations, not incremental work charges. Pixel
/// counts remain `u64` until admitted so a 32-bit host can reject large images before conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationEvent {
    Checkpoint,
    ResourceCount { resource: &'static str, actual: u64 },
}

impl DrawingListLimits {
    /// Applies the protocol quantity policy to one validation event.
    ///
    /// This does not enforce serialized bytes, which are measured by the JSON reader/writer.
    pub fn admit_validation_event(&self, event: ValidationEvent) -> Result<(), DrawingListError> {
        let ValidationEvent::ResourceCount { resource, actual } = event else {
            return Ok(());
        };
        let maximum = match resource {
            "commands" => self.max_commands,
            "resources" => self.max_resources,
            "fallbacks" => self.max_fallbacks,
            "path_segments" => self.max_path_segments,
            "stroke_dash_entries" => self.max_stroke_dash_entries,
            "image_bytes" => self.max_image_bytes,
            "image_pixels" => self.max_image_pixels,
            "fallback_pixels" => self.max_fallback_pixels,
            "font_bytes" => self.max_font_bytes,
            "nesting_depth" => self.max_nesting_depth,
            "text_bytes" => self.max_text_bytes,
            "glyphs" => self.max_glyphs,
            _ => {
                return Err(DrawingListError::invalid(
                    "unknown validation resource category",
                ));
            }
        };
        if actual > maximum as u64 {
            return Err(DrawingListError::ResourceLimit {
                resource,
                actual: usize::try_from(actual).unwrap_or(usize::MAX),
                maximum,
            });
        }
        Ok(())
    }
}

pub(crate) struct ValidationControl<E, F> {
    callback: F,
    error: PhantomData<fn() -> E>,
}

impl<E, F> ValidationControl<E, F>
where
    E: From<DrawingListError>,
    F: FnMut(ValidationEvent) -> Result<(), E>,
{
    pub(crate) fn new(callback: F) -> Self {
        Self {
            callback,
            error: PhantomData,
        }
    }

    pub(crate) fn checkpoint(&mut self) -> Result<(), E> {
        (self.callback)(ValidationEvent::Checkpoint)
    }

    pub(crate) fn count(&mut self, resource: &'static str, actual: u64) -> Result<(), E> {
        (self.callback)(ValidationEvent::ResourceCount { resource, actual })
    }
}
