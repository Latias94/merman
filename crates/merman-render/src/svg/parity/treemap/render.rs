use super::super::*;
use crate::treemap::{TREEMAP_SECTION_HEADER_HEIGHT_PX, treemap_presentation};

// Treemap diagram SVG renderer implementation (split from parity.rs).

fn normalize_dom_style_color(color: &str) -> String {
    // Upstream mutates this style through D3 after setting the attribute, so preserve the
    // browser CSSOM serialization boundary while sharing the color parser.
    super::super::util::cssom_color_value(color)
}

pub(crate) fn render_treemap_diagram_svg(
    layout: &crate::model::TreemapDiagramLayout,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("treemap");
    let theme = PresentationTheme::new(effective_config).treemap()?;
    let font_family = crate::config::config_font_family_css(effective_config);
    let computed_length_measurer = options.text_measurer_for(TextMeasurementPhase::ComputedLength);
    let presentation = treemap_presentation(
        layout,
        &theme,
        &font_family,
        options.text_measurer(),
        &computed_length_measurer,
    );
    let has_acc_title = layout
        .acc_title
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());
    let has_acc_descr = layout
        .acc_descr
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());
    let css = treemap_css(diagram_id, effective_config)?;

    let mut out = String::new();
    let aria_labelledby = has_acc_title.then(|| format!("chart-title-{diagram_id}"));
    let aria_describedby = has_acc_descr.then(|| format!("chart-desc-{diagram_id}"));
    let extra_attrs: [(&str, &str); 1] = [("class", "flowchart")];
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "treemap");
    root_chrome.extra_attrs = &extra_attrs;
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom = root_svg::RootDomProfile {
        style_viewbox_order: root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle,
        trailing_newline: false,
        ..root_svg::RootDomProfile::default()
    };
    let root_document =
        root_svg::RootViewportContext::new(crate::family::RenderFamilyKind::Treemap, diagram_id)
            .write_open(
                &mut out,
                root_svg::RootViewportSpec::responsive(root_svg::DiagramBounds::from_view_box(
                    presentation.viewport.x,
                    presentation.viewport.y,
                    presentation.viewport.width,
                    presentation.viewport.height,
                )),
                root_chrome,
            )?;
    options.checkpoint_emit()?;

    if let (Some(title), true) = (layout.acc_title.as_deref(), has_acc_title) {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{diagram_id}">{}</title>"#,
            escape_xml(title)
        );
    }
    if let (Some(descr), true) = (layout.acc_descr.as_deref(), has_acc_descr) {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{diagram_id}">{}</desc>"#,
            escape_xml(descr.trim_end_matches('\n'))
        );
    }

    let _ = write!(&mut out, "<style>{}</style>", css);
    out.push_str("<g/>");
    options.checkpoint_emit()?;

    if let Some(title) = &presentation.title {
        let _ = write!(
            &mut out,
            r#"<text x="{x}" y="{y}" class="treemapTitle" text-anchor="middle" dominant-baseline="middle">{text}</text>"#,
            x = fmt(title.x),
            y = fmt(title.y),
            text = escape_xml(&title.text)
        );
    }

    let _ = write!(
        &mut out,
        r#"<g transform="translate(0, {ty})" class="treemapContainer">"#,
        ty = fmt(layout.title_height)
    );

    for (i, section) in presentation.sections.iter().enumerate() {
        let section_clip_id = format!("clip-section-{diagram_id}-{i}");
        options.checkpoint_emit()?;
        let _ = write!(
            &mut out,
            r#"<g class="treemapSection" transform="translate({x},{y})">"#,
            x = fmt(section.x),
            y = fmt(section.y)
        );

        let header_style = if section.hidden { "display: none;" } else { "" };
        let _ = write!(
            &mut out,
            r#"<rect width="{w}" height="{hh}" class="treemapSectionHeader" fill="none" fill-opacity="0.6" stroke-width="0.6" style="{style}"/>"#,
            w = fmt(section.width),
            hh = fmt(TREEMAP_SECTION_HEADER_HEIGHT_PX),
            style = header_style
        );

        let _ = write!(
            &mut out,
            r#"<clipPath id="{id}"><rect width="{w}" height="{h}"/></clipPath>"#,
            id = section_clip_id.as_str(),
            w = fmt(section.clip_width),
            h = fmt(TREEMAP_SECTION_HEADER_HEIGHT_PX)
        );

        let section_style = if section.hidden {
            "display: none;".to_string()
        } else {
            format!(
                "{};{}",
                section.compiled.node_styles,
                section.compiled.border_styles.join(";")
            )
        };
        let _ = write!(
            &mut out,
            r#"<rect width="{w}" height="{h}" class="treemapSection section{i}" fill="{fill}" fill-opacity="0.6" stroke="{stroke}" stroke-width="2" stroke-opacity="0.4" style="{style}"/>"#,
            w = fmt(section.width),
            h = fmt(section.height),
            i = i,
            fill = escape_attr(&section.fill),
            stroke = escape_attr(&section.stroke),
            style = escape_attr(&section_style)
        );

        let label_styles_suffix = section.compiled.label_styles_as_fill();

        if section.label_text.is_empty() {
            let _ = write!(
                &mut out,
                r#"<text class="treemapSectionLabel" x="{x}" y="{y}" dominant-baseline="middle" font-weight="bold" clip-path="url(#{id})" style="display: none;"/>"#,
                x = fmt(section.label_x),
                y = fmt(section.label_y),
                id = section_clip_id.as_str(),
            );
        } else {
            let section_label_style = format!(
                "dominant-baseline: middle; font-size: {}px; fill:{fill}; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;{suffix}",
                fmt(section.label_font_size),
                fill = escape_attr(&section.label_fill),
                suffix = label_styles_suffix
            );
            let _ = write!(
                &mut out,
                r#"<text class="treemapSectionLabel" x="{x}" y="{y}" dominant-baseline="middle" font-weight="bold" clip-path="url(#{id})" style="{style}">{text}</text>"#,
                x = fmt(section.label_x),
                y = fmt(section.label_y),
                id = section_clip_id.as_str(),
                style = escape_attr(&section_label_style),
                text = escape_xml(&section.label_text)
            );
        }

        if let Some(value_text) = &section.value_text {
            let section_value_style = if section.hidden {
                "display: none;".to_string()
            } else {
                format!(
                    "text-anchor: end; dominant-baseline: middle; font-size: {}px; fill:{fill}; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;{suffix}",
                    fmt(section.value_font_size),
                    fill = escape_attr(&section.label_fill),
                    suffix = label_styles_suffix
                )
            };
            if value_text.is_empty() {
                let _ = write!(
                    &mut out,
                    r#"<text class="treemapSectionValue" x="{x}" y="{y}" text-anchor="end" dominant-baseline="middle" font-style="italic" style="{style}"/>"#,
                    x = fmt(section.value_x),
                    y = fmt(section.value_y),
                    style = escape_attr(&section_value_style)
                );
            } else {
                let _ = write!(
                    &mut out,
                    r#"<text class="treemapSectionValue" x="{x}" y="{y}" text-anchor="end" dominant-baseline="middle" font-style="italic" style="{style}">{text}</text>"#,
                    x = fmt(section.value_x),
                    y = fmt(section.value_y),
                    style = escape_attr(&section_value_style),
                    text = escape_xml(value_text)
                );
            }
        }

        out.push_str("</g>");
    }

    let is_complex_treemap = layout.leaves.len() > 20;
    let base_label_font_size = if is_complex_treemap { 16.0 } else { 38.0 };

    for (i, (leaf_layout, leaf)) in layout.leaves.iter().zip(&presentation.leaves).enumerate() {
        let leaf_clip_id = format!("clip-{diagram_id}-{i}");
        options.checkpoint_emit()?;

        let group_class = if let Some(cls) = leaf_layout
            .class_selector
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            format!("treemapNode treemapLeafGroup leaf{i} {cls}x")
        } else {
            format!("treemapNode treemapLeafGroup leaf{i}x")
        };

        let label_styles_suffix = leaf.compiled.label_styles_as_fill();

        let _ = write!(
            &mut out,
            r#"<g class="{class}" transform="translate({x},{y})">"#,
            class = escape_attr(&group_class),
            x = fmt(leaf.x),
            y = fmt(leaf.y)
        );

        let _ = write!(
            &mut out,
            r#"<rect width="{w}" height="{h}" class="treemapLeaf" fill="{fill}" style="{style}" fill-opacity="0.3" stroke="{fill}" stroke-width="3"/>"#,
            w = fmt(leaf.width),
            h = fmt(leaf.height),
            fill = escape_attr(&leaf.fill),
            style = escape_attr(&leaf.compiled.node_styles)
        );

        let _ = write!(
            &mut out,
            r#"<clipPath id="{id}"><rect width="{w}" height="{h}"/></clipPath>"#,
            id = leaf_clip_id.as_str(),
            w = fmt(leaf.clip_width),
            h = fmt(leaf.clip_height)
        );

        let label_style = if !leaf.label_hidden
            && (leaf.label_font_size - base_label_font_size).abs() < 1e-9
        {
            // Preserve Mermaid's "raw attr('style', ...)" formatting when the label isn't
            // modified by the `.each()` loop.
            format!(
                "text-anchor: middle; dominant-baseline: middle; font-size: {font_size}px;fill:{fill};{suffix}",
                font_size = fmt(base_label_font_size),
                fill = escape_attr(&leaf.label_fill),
                suffix = label_styles_suffix
            )
        } else {
            let fill = normalize_dom_style_color(&leaf.label_fill);
            let mut s = format!(
                "text-anchor: middle; dominant-baseline: middle; font-size: {fs}px; fill: {fill};",
                fs = fmt(leaf.label_font_size),
                fill = escape_attr(&fill),
            );
            if leaf.label_hidden {
                s.push_str(" display: none;");
            }
            if !label_styles_suffix.is_empty() {
                s.push_str(&label_styles_suffix);
            }
            s
        };

        let _ = write!(
            &mut out,
            r#"<text class="treemapLabel" x="{x}" y="{y}" style="{style}" clip-path="url(#{id})">{text}</text>"#,
            x = fmt(leaf.label_x),
            y = fmt(leaf.label_y),
            style = escape_attr(&label_style),
            id = leaf_clip_id.as_str(),
            text = escape_xml(&leaf_layout.name)
        );

        if let Some(value_text) = &leaf.value_text {
            let fill = normalize_dom_style_color(&leaf.label_fill);
            let mut value_style = format!(
                "text-anchor: middle; dominant-baseline: hanging; font-size: {fs}px; fill: {fill};",
                fs = fmt(leaf.value_font_size),
                fill = escape_attr(&fill)
            );
            if leaf.value_hidden {
                value_style.push_str(" display: none;");
            }
            if !label_styles_suffix.is_empty() {
                value_style.push_str(&label_styles_suffix);
            }

            if value_text.is_empty() {
                let _ = write!(
                    &mut out,
                    r#"<text class="treemapValue" x="{x}" y="{y}" style="{style}" clip-path="url(#{id})"/>"#,
                    x = fmt(leaf.value_x),
                    y = fmt(leaf.value_y),
                    style = escape_attr(&value_style),
                    id = leaf_clip_id.as_str(),
                );
            } else {
                let _ = write!(
                    &mut out,
                    r#"<text class="treemapValue" x="{x}" y="{y}" style="{style}" clip-path="url(#{id})">{text}</text>"#,
                    x = fmt(leaf.value_x),
                    y = fmt(leaf.value_y),
                    style = escape_attr(&value_style),
                    id = leaf_clip_id.as_str(),
                    text = escape_xml(value_text)
                );
            }
        }

        out.push_str("</g>");
    }

    out.push_str("</g></svg>\n");
    options.checkpoint_emit()?;
    root_document.complete(out)
}

/// Returns the source-backed Treemap stylesheet for the canonical document serializer.
pub(crate) fn canonical_treemap_css(
    diagram_id: &str,
    effective_config: &serde_json::Value,
) -> Result<String> {
    treemap_css(diagram_id, effective_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{TreemapDiagramLayout, TreemapLeafLayout, TreemapSectionLayout};

    fn leaf(name: impl Into<String>, value: f64, x0: f64, x1: f64, y1: f64) -> TreemapLeafLayout {
        TreemapLeafLayout {
            name: name.into(),
            value,
            parent_name: None,
            x0,
            y0: 0.0,
            x1,
            y1,
            class_selector: None,
            css_compiled_styles: None,
        }
    }

    fn leaf_group(svg: &str, index: usize) -> &str {
        let class = format!(r#"class="treemapNode treemapLeafGroup leaf{index}x""#);
        let class_start = svg.find(&class).expect("leaf group class");
        let start = svg[..class_start].rfind("<g").expect("leaf group start");
        let end = start + svg[start..].find("</g>").expect("leaf group end") + 4;
        &svg[start..end]
    }

    fn opening_tag_by_class<'a>(fragment: &'a str, class_name: &str) -> &'a str {
        let needle = format!(r#"<text class="{class_name}""#);
        let start = fragment.find(&needle).expect("text tag by class");
        let end = start + fragment[start..].find('>').expect("text tag end") + 1;
        &fragment[start..end]
    }

    fn attr_f64(tag: &str, name: &str) -> f64 {
        let prefix = format!(r#"{name}=""#);
        let start = tag.find(&prefix).expect("attribute") + prefix.len();
        let end = start + tag[start..].find('"').expect("attribute end");
        tag[start..end].parse().expect("numeric attribute")
    }

    fn font_size_px(tag: &str) -> f64 {
        let (_, suffix) = tag.split_once("font-size:").expect("font-size style");
        let value = suffix.trim_start();
        let end = value.find("px").expect("font-size px suffix");
        value[..end].trim().parse().expect("font-size number")
    }

    #[test]
    fn treemap_complex_leaf_text_and_section_clipping_match_mermaid_11_16() {
        let mut leaves = vec![
            leaf("Wide", 100.0, 0.0, 200.0, 100.0),
            leaf("A label much wider than its cell", 1.0, 210.0, 222.0, 40.0),
            leaf("Tiny", 1.0, 230.0, 236.0, 40.0),
        ];
        for index in 3..21 {
            leaves.push(leaf(format!("Leaf {index}"), 1.0, 0.0, 100.0, 60.0));
        }
        let layout = TreemapDiagramLayout {
            title_height: 0.0,
            width: 500.0,
            height: 200.0,
            use_max_width: true,
            diagram_padding: 8.0,
            show_values: true,
            value_format: ",".to_string(),
            acc_title: None,
            acc_descr: None,
            title: None,
            sections: vec![TreemapSectionLayout {
                name: "Section label wider than its header".to_string(),
                depth: 1,
                value: 102.0,
                x0: 0.0,
                y0: 0.0,
                x1: 40.0,
                y1: 100.0,
                class_selector: None,
                css_compiled_styles: None,
            }],
            leaves,
        };

        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let request = SvgRenderOptions::default();
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::new(&request, &debug, &session).expect("SVG execution");
        let svg = render_treemap_diagram_svg(&layout, &serde_json::json!({}), &execution).unwrap();

        let section_label = opening_tag_by_class(&svg, "treemapSectionLabel");
        assert!(
            section_label.contains(r#"clip-path="url(#clip-section-treemap-0)""#),
            "section label must use its emitted clipping path: {section_label}"
        );

        let wide = leaf_group(&svg, 0);
        let wide_label = opening_tag_by_class(wide, "treemapLabel");
        let wide_value = opening_tag_by_class(wide, "treemapValue");
        assert!(!wide_label.contains("display: none"), "{wide_label}");
        assert!(!wide_value.contains("display: none"), "{wide_value}");
        assert!(font_size_px(wide_label) > font_size_px(wide_value));
        assert!(attr_f64(wide_value, "y") > attr_f64(wide_label, "y"));

        let narrow = leaf_group(&svg, 1);
        let narrow_label = opening_tag_by_class(narrow, "treemapLabel");
        let narrow_value = opening_tag_by_class(narrow, "treemapValue");
        assert!(
            narrow.contains(">A label much wider than its cell</text>"),
            "{narrow}"
        );
        assert!(font_size_px(narrow_label) <= font_size_px(wide_label));
        assert!(font_size_px(narrow_label) > 0.0);
        assert!(!narrow_label.contains("display: none"), "{narrow_label}");
        assert!(
            narrow_label.contains(r#"clip-path="url(#clip-treemap-1)""#),
            "narrow complex labels remain present and rely on clipping: {narrow_label}"
        );
        assert!(font_size_px(narrow_value) <= font_size_px(narrow_label));
        assert!(attr_f64(narrow_value, "y") > attr_f64(narrow_label, "y"));
        assert!(!narrow_value.contains("display: none"), "{narrow_value}");

        let tiny = leaf_group(&svg, 2);
        let tiny_label = opening_tag_by_class(tiny, "treemapLabel");
        let tiny_value = opening_tag_by_class(tiny, "treemapValue");
        assert!(tiny_label.contains("display: none"), "{tiny_label}");
        assert!(tiny_value.contains("display: none"), "{tiny_value}");
    }
}
