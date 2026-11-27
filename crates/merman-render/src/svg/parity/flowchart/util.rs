//! Flowchart-specific rendering helpers.

pub(in crate::svg::parity) use super::super::util::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR;
pub(super) use merman_render_flow::svg_label::OptionalStyleAttr;

pub(super) struct OptionalStyleXmlAttr<'a>(pub(super) &'a str);

impl std::fmt::Display for OptionalStyleXmlAttr<'_> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.0.trim();
        if s.is_empty() {
            return Ok(());
        }
        write!(
            f,
            r#" style="{}""#,
            super::super::util::escape_xml_display(s)
        )
    }
}
