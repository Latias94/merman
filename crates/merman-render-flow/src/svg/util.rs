//! Flowchart-specific rendering helpers.

pub(in crate::svg) use merman_render_core::html::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR;

pub(super) struct OptionalStyleAttr<'a>(pub(super) &'a str);

impl std::fmt::Display for OptionalStyleAttr<'_> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.0.trim();
        if s.is_empty() {
            return Ok(());
        }
        write!(f, r#" style="{}""#, escape_attr_display(s))
    }
}

pub(super) use merman_render_core::xml::escape_attr_display;

pub(in crate::svg) use merman_render_core::number_format::canonical_number as fmt;
pub(in crate::svg) use merman_render_core::number_format::canonical_number as fmt_display;

pub(in crate::svg) fn fmt_into(out: &mut impl super::SvgOutput, value: f64) {
    let _ = write!(out, "{}", fmt_display(value));
}
