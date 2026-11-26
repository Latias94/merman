use merman_render_core::{
    Result,
    number_format::canonical_number as fmt,
    svg_output::SvgOutput,
    xml::{escape_attr, escape_attr_display},
};

/// Already selected paint for a leaf rectangle.
pub struct TreemapLeafRectPaint<'a> {
    pub fill: &'a str,
    pub stroke: &'a str,
    pub style: &'a str,
    pub native_radius: Option<f64>,
}

/// Opens a leaf group at its layout origin.
pub fn write_treemap_leaf_group_open(
    out: &mut impl SvgOutput,
    group_class: &str,
    x: f64,
    y: f64,
) -> Result<()> {
    let _ = write!(
        out,
        r#"<g class="{class}" transform="translate({x},{y})">"#,
        class = escape_attr(group_class),
        x = fmt(x),
        y = fmt(y)
    );
    out.checkpoint()
}

/// Writes the leaf rectangle and label clip before the caller writes fitted text.
///
/// `clip_id` is the caller's already projected ID, reused in the caller's text
/// references. This writer XML-escapes it without performing another projection.
pub fn write_treemap_leaf_rect_and_clip(
    out: &mut impl SvgOutput,
    width: f64,
    height: f64,
    clip_id: &str,
    paint: TreemapLeafRectPaint<'_>,
) -> Result<()> {
    let (opacity, radius) = match paint.native_radius {
        Some(radius) => (
            "1",
            format!(
                r#" rx="{}""#,
                fmt(radius.min(width.min(height) / 2.0).max(0.0))
            ),
        ),
        None => ("0.3", String::new()),
    };
    let _ = write!(
        out,
        r#"<rect width="{w}" height="{h}" class="treemapLeaf" fill="{fill}" style="{style}" fill-opacity="{opacity}" stroke="{stroke}" stroke-width="3"{radius}/>"#,
        w = fmt(width),
        h = fmt(height),
        fill = escape_attr(paint.fill),
        stroke = escape_attr(paint.stroke),
        style = escape_attr(paint.style)
    );
    out.checkpoint()?;

    let _ = write!(
        out,
        r#"<clipPath id="{id}"><rect width="{w}" height="{h}"/></clipPath>"#,
        id = escape_attr_display(clip_id),
        w = fmt((width - 4.0).max(0.0)),
        h = fmt((height - 4.0).max(0.0))
    );
    out.checkpoint()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treemap_leaf_clip_id_is_escaped_as_xml_attribute_text() {
        let mut out = String::new();
        write_treemap_leaf_rect_and_clip(
            &mut out,
            12.0,
            24.0,
            r#"clip-"&<"#,
            TreemapLeafRectPaint {
                fill: "none",
                stroke: "none",
                style: "",
                native_radius: None,
            },
        )
        .expect("write leaf rectangle and clip");

        assert!(out.contains(r#"<clipPath id="clip-&quot;&amp;&lt;">"#));
    }
}
