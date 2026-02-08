use super::*;

// Treemap diagram SVG renderer implementation (split from legacy.rs).

pub(super) fn render_treemap_diagram_svg(
    layout: &crate::model::TreemapDiagramLayout,
    _semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    #[derive(Default)]
    struct OrdinalScale {
        range: Vec<String>,
        domain: std::collections::HashMap<String, usize>,
    }

    impl OrdinalScale {
        fn get(&mut self, key: &str) -> String {
            let idx = if let Some(idx) = self.domain.get(key).copied() {
                idx
            } else {
                let idx = self.domain.len();
                self.domain.insert(key.to_string(), idx);
                idx
            };
            if self.range.is_empty() {
                return String::new();
            }
            self.range[idx % self.range.len()].clone()
        }
    }

    fn replace_first(haystack: &str, needle: &str, replacement: &str) -> String {
        if needle.is_empty() {
            return haystack.to_string();
        }
        let Some(idx) = haystack.find(needle) else {
            return haystack.to_string();
        };
        let mut out = String::with_capacity(haystack.len() - needle.len() + replacement.len());
        out.push_str(&haystack[..idx]);
        out.push_str(replacement);
        out.push_str(&haystack[idx + needle.len()..]);
        out
    }

    #[derive(Default)]
    struct OrderedMap {
        order: Vec<(String, String)>,
        idx: std::collections::HashMap<String, usize>,
    }

    impl OrderedMap {
        fn set(&mut self, k: &str, v: &str) {
            if k.is_empty() {
                return;
            }
            if let Some(&i) = self.idx.get(k) {
                self.order[i].1 = v.to_string();
                return;
            }
            self.idx.insert(k.to_string(), self.order.len());
            self.order.push((k.to_string(), v.to_string()));
        }
    }

    fn treemap_is_label_style(key: &str) -> bool {
        matches!(
            key.trim(),
            "color"
                | "font-size"
                | "font-family"
                | "font-weight"
                | "font-style"
                | "text-decoration"
                | "text-align"
                | "text-transform"
                | "line-height"
                | "letter-spacing"
                | "word-spacing"
                | "text-shadow"
                | "text-overflow"
                | "white-space"
                | "word-wrap"
                | "word-break"
                | "overflow-wrap"
                | "hyphens"
        )
    }

    #[derive(Default)]
    struct TreemapCompiledStyles {
        label_styles: String,
        node_styles: String,
        border_styles: Vec<String>,
    }

    fn treemap_styles2_string(css_compiled_styles: &[String]) -> TreemapCompiledStyles {
        // Ported from Mermaid `handDrawnShapeStyles.compileStyles()` / `styles2String()`:
        // - preserve insertion order of the first occurrence of a key
        // - later occurrences override values, without changing order
        // - tolerate tokens without `:` (JS `split(':')` yields `value = undefined`)
        let mut m = OrderedMap::default();

        for entry in css_compiled_styles {
            for raw in entry.split(';') {
                let s = raw.trim();
                if s.is_empty() {
                    continue;
                }
                let (k, v) = if let Some((k, v)) = s.split_once(':') {
                    (k.trim(), v.trim())
                } else {
                    (s.trim(), "")
                };
                m.set(k, v);
            }
        }

        let mut label_styles: Vec<String> = Vec::new();
        let mut node_styles: Vec<String> = Vec::new();
        let mut border_styles: Vec<String> = Vec::new();

        for (k, v) in &m.order {
            let decl = if v.is_empty() {
                format!("{k}:")
            } else {
                format!("{k}:{v}")
            };
            let decl_imp = format!("{decl} !important");
            if treemap_is_label_style(k) {
                label_styles.push(decl_imp);
            } else {
                node_styles.push(decl_imp.clone());
                if k.contains("stroke") {
                    border_styles.push(decl_imp);
                }
            }
        }

        TreemapCompiledStyles {
            label_styles: label_styles.join(";"),
            node_styles: node_styles.join(";"),
            border_styles,
        }
    }

    fn parse_css_rgb(color: &str) -> Option<(u8, u8, u8)> {
        let c = color.trim();
        if c.eq_ignore_ascii_case("black") {
            return Some((0, 0, 0));
        }
        if c.eq_ignore_ascii_case("white") {
            return Some((255, 255, 255));
        }
        if let Some(hex) = c.strip_prefix('#') {
            let h = hex.trim();
            if h.len() == 3 {
                let r = u8::from_str_radix(&h[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&h[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&h[2..3].repeat(2), 16).ok()?;
                return Some((r, g, b));
            }
            if h.len() == 6 {
                let r = u8::from_str_radix(&h[0..2], 16).ok()?;
                let g = u8::from_str_radix(&h[2..4], 16).ok()?;
                let b = u8::from_str_radix(&h[4..6], 16).ok()?;
                return Some((r, g, b));
            }
        }
        let lower = c.to_ascii_lowercase();
        if let Some(args) = lower.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
            let parts = args
                .split(',')
                .map(|p| p.trim())
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>();
            if parts.len() >= 3 {
                let r = parts[0].parse::<u16>().ok()?;
                let g = parts[1].parse::<u16>().ok()?;
                let b = parts[2].parse::<u16>().ok()?;
                if r <= 255 && g <= 255 && b <= 255 {
                    return Some((r as u8, g as u8, b as u8));
                }
            }
        }
        None
    }

    fn invert_css_color_to_hex(color: &str) -> Option<String> {
        let (r, g, b) = parse_css_rgb(color)?;
        let ir = 255u8.saturating_sub(r);
        let ig = 255u8.saturating_sub(g);
        let ib = 255u8.saturating_sub(b);
        Some(format!("#{:02x}{:02x}{:02x}", ir, ig, ib))
    }

    fn normalize_dom_style_color(color: &str) -> String {
        // jsdom serialization tends to normalize hex colors to `rgb(r, g, b)` when the style
        // attribute has been mutated (e.g. via `.style(...)` in upstream Mermaid).
        let c = color.trim();
        if c.starts_with('#') {
            if let Some((r, g, b)) = parse_css_rgb(c) {
                return format!("rgb({r}, {g}, {b})");
            }
        }
        c.to_string()
    }

    fn default_c_scale(i: usize) -> &'static str {
        match i {
            0 => "hsl(240, 100%, 76.2745098039%)",
            1 => "hsl(60, 100%, 73.5294117647%)",
            2 => "hsl(80, 100%, 76.2745098039%)",
            3 => "hsl(270, 100%, 76.2745098039%)",
            4 => "hsl(300, 100%, 76.2745098039%)",
            5 => "hsl(330, 100%, 76.2745098039%)",
            6 => "hsl(0, 100%, 76.2745098039%)",
            7 => "hsl(30, 100%, 76.2745098039%)",
            8 => "hsl(90, 100%, 76.2745098039%)",
            9 => "hsl(150, 100%, 76.2745098039%)",
            10 => "hsl(180, 100%, 76.2745098039%)",
            _ => "hsl(210, 100%, 76.2745098039%)",
        }
    }

    fn default_c_scale_peer(i: usize) -> &'static str {
        match i {
            0 => "hsl(240, 100%, 61.2745098039%)",
            1 => "hsl(60, 100%, 48.5294117647%)",
            2 => "hsl(80, 100%, 56.2745098039%)",
            3 => "hsl(270, 100%, 61.2745098039%)",
            4 => "hsl(300, 100%, 61.2745098039%)",
            5 => "hsl(330, 100%, 61.2745098039%)",
            6 => "hsl(0, 100%, 61.2745098039%)",
            7 => "hsl(30, 100%, 61.2745098039%)",
            8 => "hsl(90, 100%, 61.2745098039%)",
            9 => "hsl(150, 100%, 61.2745098039%)",
            10 => "hsl(180, 100%, 61.2745098039%)",
            _ => "hsl(210, 100%, 61.2745098039%)",
        }
    }

    fn format_int_with_commas(n: i64) -> String {
        let mut s = n.abs().to_string();
        let mut out = String::new();
        while s.len() > 3 {
            let split_at = s.len() - 3;
            let tail = &s[split_at..];
            if out.is_empty() {
                out = tail.to_string();
            } else {
                out = format!("{tail},{out}");
            }
            s.truncate(split_at);
        }
        if out.is_empty() {
            out = s;
        } else {
            out = format!("{s},{out}");
        }
        if n < 0 { format!("-{out}") } else { out }
    }

    fn format_value(value: f64, format_str: &str) -> String {
        let format_str = format_str.trim();
        let uses_commas = format_str.is_empty() || format_str == ",";
        if uses_commas {
            if (value - value.round()).abs() < 1e-9 {
                return format_int_with_commas(value.round() as i64);
            }
            let raw = format!("{value}");
            let Some((head, tail)) = raw.split_once('.') else {
                return raw;
            };
            let int_part = head
                .parse::<i64>()
                .ok()
                .map(format_int_with_commas)
                .unwrap_or_else(|| head.to_string());
            if tail.is_empty() {
                return int_part;
            }
            format!("{int_part}.{tail}")
        } else if format_str == "$0,0" {
            let v = value.round() as i64;
            format!("${}", format_int_with_commas(v))
        } else if format_str.starts_with('$') {
            let v = format_value(value, ",");
            format!("${v}")
        } else {
            // Fallback: approximate D3 `format()` behavior.
            format_value(value, ",")
        }
    }

    let diagram_id = options.diagram_id.as_deref().unwrap_or("treemap");
    let diagram_id_esc = escape_xml(diagram_id);

    let mut color_scale = OrdinalScale::default();
    color_scale.range.push("transparent".to_string());
    for i in 0..12 {
        let key = format!("cScale{i}");
        let v = theme_color(effective_config, &key, default_c_scale(i));
        color_scale.range.push(v);
    }
    let mut color_scale_peer = OrdinalScale::default();
    color_scale_peer.range.push("transparent".to_string());
    for i in 0..12 {
        let key = format!("cScalePeer{i}");
        let v = theme_color(effective_config, &key, default_c_scale_peer(i));
        color_scale_peer.range.push(v);
    }

    // Mermaid treemap uses `cScaleLabel*` theme variables (see `renderer.ts`), but not all of our
    // effective configs include the derived fields. Mirror theme-default's defaults as a
    // fallback so strict SVG baselines match:
    // - `cScaleLabel0` and `cScaleLabel3`: `invert(labelTextColor)`
    // - the rest: `labelTextColor`
    let label_text_color = theme_color(effective_config, "labelTextColor", "black");
    let label_text_is_calculated = label_text_color.trim() == "calculated";
    let scale_label_color = theme_color(effective_config, "scaleLabelColor", &label_text_color);
    let mut color_scale_label = OrdinalScale::default();
    for i in 0..12 {
        let key = format!("cScaleLabel{i}");
        let v = config_string(effective_config, &["themeVariables", key.as_str()]).unwrap_or_else(
            || {
                if label_text_is_calculated {
                    scale_label_color.clone()
                } else if i == 0 || i == 3 {
                    invert_css_color_to_hex(&label_text_color)
                        .unwrap_or_else(|| label_text_color.clone())
                } else {
                    label_text_color.clone()
                }
            },
        );
        color_scale_label.range.push(v);
    }

    let has_acc_title = layout
        .acc_title
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());
    let has_acc_descr = layout
        .acc_descr
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    fn add_rect_bounds(
        min_x: &mut f64,
        min_y: &mut f64,
        max_x: &mut f64,
        max_y: &mut f64,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    ) {
        let w = x1 - x0;
        let h = y1 - y0;
        if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
            return;
        }
        *min_x = (*min_x).min(x0);
        *min_y = (*min_y).min(y0);
        *max_x = (*max_x).max(x1);
        *max_y = (*max_y).max(y1);
    }

    for s in &layout.sections {
        if s.depth == 0 {
            continue;
        }
        add_rect_bounds(
            &mut min_x, &mut min_y, &mut max_x, &mut max_y, s.x0, s.y0, s.x1, s.y1,
        );
    }
    for l in &layout.leaves {
        add_rect_bounds(
            &mut min_x, &mut min_y, &mut max_x, &mut max_y, l.x0, l.y0, l.x1, l.y1,
        );
    }

    let vb_x;
    let vb_y;
    let vb_w;
    let vb_h;
    if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
        vb_x = min_x - layout.diagram_padding;
        vb_y = min_y - layout.diagram_padding;
        vb_w = (max_x - min_x) + layout.diagram_padding * 2.0;
        vb_h = (max_y - min_y) + layout.diagram_padding * 2.0;
    } else {
        vb_x = -layout.diagram_padding;
        vb_y = -layout.diagram_padding;
        vb_w = layout.diagram_padding * 2.0;
        vb_h = layout.diagram_padding * 2.0;
    }

    let css = treemap_css(diagram_id);

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="{min_x} {min_y} {w} {h}" style="max-width: {max_w}px; background-color: white;" class="flowchart" role="graphics-document document" aria-roledescription="treemap""#,
        min_x = fmt(vb_x),
        min_y = fmt(vb_y),
        w = fmt(vb_w.max(1.0)),
        h = fmt(vb_h.max(1.0)),
        max_w = fmt(vb_w.max(1.0)),
    );

    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#" aria-describedby="chart-desc-{diagram_id_esc}""#
        );
    }
    if has_acc_title {
        let _ = write!(
            &mut out,
            r#" aria-labelledby="chart-title-{diagram_id_esc}""#
        );
    }
    out.push('>');

    if let (Some(title), true) = (layout.acc_title.as_deref(), has_acc_title) {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{diagram_id_esc}">{}</title>"#,
            escape_xml(title)
        );
    }
    if let (Some(descr), true) = (layout.acc_descr.as_deref(), has_acc_descr) {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{diagram_id_esc}">{}</desc>"#,
            escape_xml(descr.trim_end_matches('\n'))
        );
    }

    let _ = write!(&mut out, "<style>{}</style>", css);
    out.push_str("<g/>");

    if let Some(title) = layout.title.as_deref().filter(|t| !t.trim().is_empty()) {
        let _ = write!(
            &mut out,
            r#"<text x="{x}" y="{y}" class="treemapTitle" text-anchor="middle" dominant-baseline="middle">{text}</text>"#,
            x = fmt(layout.width / 2.0),
            y = fmt(layout.title_height / 2.0),
            text = escape_xml(title)
        );
    }

    let _ = write!(
        &mut out,
        r#"<g transform="translate(0, {ty})" class="treemapContainer">"#,
        ty = fmt(layout.title_height)
    );

    let measurer = crate::text::VendoredFontMetricsTextMeasurer::default();
    let font_family = r#""trebuchet ms",verdana,arial,sans-serif"#.to_string();

    for (i, section) in layout.sections.iter().enumerate() {
        let w = section.x1 - section.x0;
        let h = section.y1 - section.y0;
        let _ = write!(
            &mut out,
            r#"<g class="treemapSection" transform="translate({x},{y})">"#,
            x = fmt(section.x0),
            y = fmt(section.y0)
        );

        let header_style = if section.depth == 0 {
            "display: none;"
        } else {
            ""
        };
        let _ = write!(
            &mut out,
            r#"<rect width="{w}" height="{hh}" class="treemapSectionHeader" fill="none" fill-opacity="0.6" stroke-width="0.6" style="{style}"/>"#,
            w = fmt(w),
            hh = fmt(25.0),
            style = header_style
        );

        let _ = write!(
            &mut out,
            r#"<clipPath id="clip-section-{id}-{i}"><rect width="{w}" height="{h}"/></clipPath>"#,
            id = escape_attr(diagram_id),
            i = i,
            w = fmt((w - 12.0).max(0.0)),
            h = fmt(25.0)
        );

        let fill = color_scale.get(&section.name);
        let stroke = color_scale_peer.get(&section.name);
        let section_css: &[String] = section.css_compiled_styles.as_deref().unwrap_or(&[]);
        let compiled = treemap_styles2_string(section_css);
        let section_style = if section.depth == 0 {
            "display: none;".to_string()
        } else {
            format!(
                "{};{}",
                compiled.node_styles,
                compiled.border_styles.join(";")
            )
        };
        let _ = write!(
            &mut out,
            r#"<rect width="{w}" height="{h}" class="treemapSection section{i}" fill="{fill}" fill-opacity="0.6" stroke="{stroke}" stroke-width="2" stroke-opacity="0.4" style="{style}"/>"#,
            w = fmt(w),
            h = fmt(h),
            i = i,
            fill = escape_attr(&fill),
            stroke = escape_attr(&stroke),
            style = escape_attr(&section_style)
        );

        let mut label_text = if section.depth == 0 {
            String::new()
        } else {
            section.name.clone()
        };

        let label_fill = if section.depth == 0 {
            String::new()
        } else {
            color_scale_label.get(&section.name)
        };
        let label_styles_suffix = replace_first(&compiled.label_styles, "color:", "fill:");

        if label_text.is_empty() {
            let _ = write!(
                &mut out,
                r#"<text class="treemapSectionLabel" x="6" y="12.5" dominant-baseline="middle" font-weight="bold" style="display: none;"/>"#
            );
        } else {
            // Mirror Mermaid's truncation loop in `renderer.ts` (uses `getComputedTextLength()`).
            let total_header_width = w;
            let label_x_position = 6.0;
            let mut space_for_text_content = total_header_width - label_x_position - 6.0;
            if layout.show_values && section.value != 0.0 {
                let value_ends_at_x_relative = total_header_width - 10.0;
                let estimated_value_text_actual_width = 30.0;
                let gap_between_label_and_value = 10.0;
                let label_must_end_before_x = value_ends_at_x_relative
                    - estimated_value_text_actual_width
                    - gap_between_label_and_value;
                space_for_text_content = label_must_end_before_x - label_x_position;
            }
            let minimum_width_to_display: f64 = 15.0;
            let actual_available_width = minimum_width_to_display.max(space_for_text_content);

            let style = crate::text::TextStyle {
                font_family: Some(font_family.clone()),
                font_size: 12.0,
                font_weight: Some("bold".to_string()),
            };

            if measurer.measure(&label_text, &style).width > actual_available_width {
                let ellipsis = "...";
                let original = label_text.clone();
                let mut current = original.clone();
                while !current.is_empty() {
                    current.pop();
                    if current.is_empty() {
                        if measurer.measure(ellipsis, &style).width > actual_available_width {
                            label_text.clear();
                        } else {
                            label_text = ellipsis.to_string();
                        }
                        break;
                    }
                    let candidate = format!("{current}{ellipsis}");
                    if measurer.measure(&candidate, &style).width <= actual_available_width {
                        label_text = candidate;
                        break;
                    }
                }
            }

            let section_label_style = format!(
                "dominant-baseline: middle; font-size: 12px; fill:{fill}; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;{suffix}",
                fill = escape_attr(&label_fill),
                suffix = label_styles_suffix
            );
            let _ = write!(
                &mut out,
                r#"<text class="treemapSectionLabel" x="6" y="12.5" dominant-baseline="middle" font-weight="bold" style="{style}">{text}</text>"#,
                style = escape_attr(&section_label_style),
                text = escape_xml(&label_text)
            );
        }

        if layout.show_values {
            let value_text = if section.value != 0.0 {
                format_value(section.value, &layout.value_format)
            } else {
                String::new()
            };
            let section_value_style = if section.depth == 0 {
                "display: none;".to_string()
            } else {
                format!(
                    "text-anchor: end; dominant-baseline: middle; font-size: 10px; fill:{fill}; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;{suffix}",
                    fill = escape_attr(&label_fill),
                    suffix = label_styles_suffix
                )
            };
            if value_text.is_empty() {
                let _ = write!(
                    &mut out,
                    r#"<text class="treemapSectionValue" x="{x}" y="12.5" text-anchor="end" dominant-baseline="middle" font-style="italic" style="{style}"/>"#,
                    x = fmt(w - 10.0),
                    style = escape_attr(&section_value_style)
                );
            } else {
                let _ = write!(
                    &mut out,
                    r#"<text class="treemapSectionValue" x="{x}" y="12.5" text-anchor="end" dominant-baseline="middle" font-style="italic" style="{style}">{text}</text>"#,
                    x = fmt(w - 10.0),
                    style = escape_attr(&section_value_style),
                    text = escape_xml(&value_text)
                );
            }
        }

        out.push_str("</g>");
    }

    for (i, leaf) in layout.leaves.iter().enumerate() {
        let w = leaf.x1 - leaf.x0;
        let h = leaf.y1 - leaf.y0;

        let group_class = if let Some(cls) = leaf
            .class_selector
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            format!("treemapNode treemapLeafGroup leaf{i} {cls}x")
        } else {
            format!("treemapNode treemapLeafGroup leaf{i}x")
        };

        let fill_key = leaf
            .parent_name
            .as_deref()
            .unwrap_or_else(|| leaf.name.as_str());
        let fill = color_scale.get(fill_key);

        let leaf_css: &[String] = leaf.css_compiled_styles.as_deref().unwrap_or(&[]);
        let compiled = treemap_styles2_string(leaf_css);
        let leaf_rect_style = compiled.node_styles.clone();
        let label_styles_suffix = replace_first(&compiled.label_styles, "color:", "fill:");
        let leaf_label_fill = color_scale_label.get(&leaf.name);

        let _ = write!(
            &mut out,
            r#"<g class="{class}" transform="translate({x},{y})">"#,
            class = escape_attr(&group_class),
            x = fmt(leaf.x0),
            y = fmt(leaf.y0)
        );

        let _ = write!(
            &mut out,
            r#"<rect width="{w}" height="{h}" class="treemapLeaf" fill="{fill}" style="{style}" fill-opacity="0.3" stroke="{fill}" stroke-width="3"/>"#,
            w = fmt(w),
            h = fmt(h),
            fill = escape_attr(&fill),
            style = escape_attr(&leaf_rect_style)
        );

        let _ = write!(
            &mut out,
            r#"<clipPath id="clip-{id}-{i}"><rect width="{w}" height="{h}"/></clipPath>"#,
            id = escape_attr(diagram_id),
            i = i,
            w = fmt((w - 4.0).max(0.0)),
            h = fmt((h - 4.0).max(0.0))
        );

        let padding = 4.0;
        let available_w = w - 2.0 * padding;
        let available_h = h - 2.0 * padding;

        let mut label_font_size = 38.0;
        let min_label_font_size = 8.0;
        let original_value_rel_font_size = 28.0;
        let value_scale_factor = 0.6;
        let min_value_font_size = 6.0;
        let spacing_between_label_and_value = 2.0;

        let mut label_hidden = false;
        if available_w < 10.0 || available_h < 10.0 {
            label_hidden = true;
        } else {
            let mut style = crate::text::TextStyle {
                font_family: Some(font_family.clone()),
                font_size: label_font_size,
                font_weight: None,
            };

            while measurer.measure(&leaf.name, &style).width > available_w
                && label_font_size > min_label_font_size
            {
                label_font_size -= 1.0;
                style.font_size = label_font_size;
            }

            let mut prospective_value_font_size = (label_font_size * value_scale_factor)
                .round()
                .min(original_value_rel_font_size)
                .max(min_value_font_size);
            let mut combined_h =
                label_font_size + spacing_between_label_and_value + prospective_value_font_size;

            while combined_h > available_h && label_font_size > min_label_font_size {
                label_font_size -= 1.0;
                style.font_size = label_font_size;
                prospective_value_font_size = (label_font_size * value_scale_factor)
                    .round()
                    .min(original_value_rel_font_size)
                    .max(min_value_font_size);
                combined_h =
                    label_font_size + spacing_between_label_and_value + prospective_value_font_size;
            }

            style.font_size = label_font_size;
            if measurer.measure(&leaf.name, &style).width > available_w
                || label_font_size < min_label_font_size
                || available_h < label_font_size
            {
                label_hidden = true;
            }
        }

        let label_style = if !label_hidden && (label_font_size - 38.0).abs() < 1e-9 {
            // Preserve Mermaid's "raw attr('style', ...)" formatting when the label isn't
            // modified by the `.each()` loop.
            format!(
                "text-anchor: middle; dominant-baseline: middle; font-size: 38px;fill:{fill};{suffix}",
                fill = escape_attr(&leaf_label_fill),
                suffix = label_styles_suffix
            )
        } else {
            let fill = normalize_dom_style_color(&leaf_label_fill);
            let mut s = format!(
                "text-anchor: middle; dominant-baseline: middle; font-size: {fs}px; fill: {fill};",
                fs = fmt(label_font_size),
                fill = escape_attr(&fill),
            );
            if label_hidden {
                s.push_str(" display: none;");
            }
            if !label_styles_suffix.is_empty() {
                s.push_str(&label_styles_suffix);
            }
            s
        };

        let _ = write!(
            &mut out,
            r#"<text class="treemapLabel" x="{x}" y="{y}" style="{style}" clip-path="url(#clip-{id}-{i})">{text}</text>"#,
            x = fmt(w / 2.0),
            y = fmt(h / 2.0),
            style = escape_attr(&label_style),
            id = escape_attr(diagram_id),
            i = i,
            text = escape_xml(&leaf.name)
        );

        if layout.show_values {
            let value_text = if leaf.value != 0.0 {
                format_value(leaf.value, &layout.value_format)
            } else {
                String::new()
            };
            let mut value_font_size = 28.0;
            let mut value_y = h / 2.0; // placeholder (overwritten when label is visible)
            let mut value_hidden = true;

            if !label_hidden {
                let actual_value_font_size = (label_font_size * value_scale_factor)
                    .round()
                    .min(original_value_rel_font_size)
                    .max(min_value_font_size);
                value_font_size = actual_value_font_size;

                let label_center_y = h / 2.0;
                value_y =
                    label_center_y + (label_font_size / 2.0) + spacing_between_label_and_value;

                let cell_bottom_padding = 4.0;
                let max_value_bottom_y = h - cell_bottom_padding;
                let available_w_for_value = w - 2.0 * 4.0;

                let style = crate::text::TextStyle {
                    font_family: Some(font_family.clone()),
                    font_size: value_font_size,
                    font_weight: None,
                };
                let value_w_px = measurer.measure(&value_text, &style).width;
                if value_w_px <= available_w_for_value
                    && value_y + value_font_size <= max_value_bottom_y
                    && value_font_size >= min_value_font_size
                {
                    value_hidden = false;
                }
            }

            let fill = normalize_dom_style_color(&leaf_label_fill);
            let mut value_style = format!(
                "text-anchor: middle; dominant-baseline: hanging; font-size: {fs}px; fill: {fill};",
                fs = fmt(value_font_size),
                fill = escape_attr(&fill)
            );
            if value_hidden {
                value_style.push_str(" display: none;");
            }
            if !label_styles_suffix.is_empty() {
                value_style.push_str(&label_styles_suffix);
            }

            if value_text.is_empty() {
                let _ = write!(
                    &mut out,
                    r#"<text class="treemapValue" x="{x}" y="{y}" style="{style}" clip-path="url(#clip-{id}-{i})"/>"#,
                    x = fmt(w / 2.0),
                    y = fmt(value_y),
                    style = escape_attr(&value_style),
                    id = escape_attr(diagram_id),
                    i = i,
                );
            } else {
                let _ = write!(
                    &mut out,
                    r#"<text class="treemapValue" x="{x}" y="{y}" style="{style}" clip-path="url(#clip-{id}-{i})">{text}</text>"#,
                    x = fmt(w / 2.0),
                    y = fmt(value_y),
                    style = escape_attr(&value_style),
                    id = escape_attr(diagram_id),
                    i = i,
                    text = escape_xml(&value_text)
                );
            }
        }

        out.push_str("</g>");
    }

    out.push_str("</g></svg>\n");
    Ok(out)
}
