use merman_render_core::{
    Result,
    number_format::canonical_number as fmt,
    svg_css::{CssSelectorDiagramId, FragmentDiagramId},
    svg_output::{SvgOutput, XmlTextOutput},
    theme_svg::write_color,
    xml::{escape_attr, escape_xml},
};

use super::{InfoClassicStyle, InfoDiagramLayout, InfoNativeStyle};

impl InfoClassicStyle {
    /// Writes the Info stylesheet and version text inside an open Mermaid SVG.
    ///
    /// `diagram_id` is the semantic ID shared by the root, CSS selector and fragment URL.
    pub fn write_svg_contents(
        &self,
        out: &mut impl SvgOutput,
        layout: &InfoDiagramLayout,
        diagram_id: &str,
    ) -> Result<()> {
        out.push_str("<style>");
        let selector = CssSelectorDiagramId(diagram_id);
        let fragment = FragmentDiagramId(diagram_id);
        {
            let mut css = XmlTextOutput::new(out);
            self.css.write_prefix(&mut css, selector)?;
            self.css
                .write_root(&mut css, selector, fragment, self.css.font_family())?;
        }
        out.push_str("</style>");
        out.checkpoint()?;
        out.push_str("<g/>");
        out.checkpoint()?;
        let version = &self.version;
        let _ = write!(
            out,
            r#"<g><text x="{}" y="{}" class="version" font-size="{}" style="text-anchor: middle;">{}</text></g>"#,
            fmt(version.x),
            fmt(version.y),
            fmt(version.font_size_px),
            escape_xml(&layout.version)
        );
        out.checkpoint()
    }
}

impl InfoNativeStyle {
    /// Writes the native version text and the closing tag of an open Info SVG.
    ///
    /// The caller finishes the operation-owned sink and reports its error.
    pub fn write_svg_tail(&self, out: &mut impl SvgOutput, layout: &InfoDiagramLayout) {
        let version = &self.version;
        let _ = write!(
            out,
            r#"<g><text x="{}" y="{}" class="version" font-size="{}" font-family="{}" "#,
            fmt(version.x),
            fmt(version.y),
            fmt(version.font_size_px),
            escape_attr(&self.font_family),
        );
        let _ = write_color(out, "fill", self.text_color, 1.0);
        // Keep these closing tags in one write for the existing output admission boundary.
        let _ = write!(
            out,
            r#" text-anchor="middle">{}</text></g></svg>"#,
            escape_xml(&layout.version)
        );
    }
}
