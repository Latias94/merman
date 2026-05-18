//! Font metrics generator used by deterministic text measurement.

use crate::XtaskError;
use crate::util::*;
use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn solve_ridge(at_a: &mut [Vec<f64>], at_b: &mut [f64]) -> Vec<f64> {
    let n = at_b.len();
    let mut i = 0;
    while i < n {
        // Pivot.
        let mut pivot = i;
        let mut best = at_a[i][i].abs();
        let mut r = i + 1;
        while r < n {
            let v = at_a[r][i].abs();
            if v > best {
                best = v;
                pivot = r;
            }
            r += 1;
        }
        if pivot != i {
            at_a.swap(i, pivot);
            at_b.swap(i, pivot);
        }

        let diag = at_a[i][i];
        if diag.abs() < 1e-12 {
            i += 1;
            continue;
        }
        let inv = 1.0 / diag;
        let mut c = i;
        while c < n {
            at_a[i][c] *= inv;
            c += 1;
        }
        at_b[i] *= inv;

        let mut r = 0;
        while r < n {
            if r == i {
                r += 1;
                continue;
            }
            let factor = at_a[r][i];
            if factor.abs() < 1e-12 {
                r += 1;
                continue;
            }
            let mut c = i;
            while c < n {
                at_a[r][c] -= factor * at_a[i][c];
                c += 1;
            }
            at_b[r] -= factor * at_b[i];
            r += 1;
        }
        i += 1;
    }
    at_b.to_vec()
}

#[derive(Debug, Clone)]
struct Sample {
    font_key: String,
    text: String,
    width_px: f64,
    font_size_px: f64,
    plain_html_label: bool,
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let mid = v.len() / 2;
    if v.len() % 2 == 1 {
        Some(v[mid])
    } else {
        Some((v[mid - 1] + v[mid]) / 2.0)
    }
}

fn rust_f64(v: f64) -> String {
    let mut s = format!("{v}");
    if !s.contains('.') && !s.contains('e') && !s.contains('E') {
        s.push_str(".0");
    }
    s
}

fn has_class_ancestor(node: roxmltree::Node<'_, '_>, class_names: &[&str]) -> bool {
    node.ancestors().filter(|n| n.is_element()).any(|n| {
        n.attribute("class")
            .unwrap_or_default()
            .split_whitespace()
            .any(|t| class_names.contains(&t))
    })
}

fn foreignobject_content_width_px(fo: roxmltree::Node<'_, '_>, width_px: f64) -> f64 {
    let has_icon_or_image_shape_ancestor = has_class_ancestor(fo, &["icon-shape", "image-shape"]);
    let has_paragraph = fo.descendants().any(|n| n.has_tag_name("p"));

    // Mermaid icon/image HTML labels inherit `.icon-shape p, .image-shape p { padding: 2px; }`.
    // The exported `<foreignObject width>` therefore includes 2px on each side, while the
    // layout-time text measurer is expected to return the content width and the renderer adds
    // the label background padding separately.
    if has_icon_or_image_shape_ancestor && has_paragraph {
        (width_px - 4.0).max(0.0)
    } else {
        width_px
    }
}

fn is_plain_html_label_sample(fo: roxmltree::Node<'_, '_>) -> bool {
    if has_class_ancestor(fo, &["icon-shape", "image-shape"]) {
        return false;
    }

    let mut paragraphs = 0usize;
    for n in fo.descendants().filter(|n| n.is_element()) {
        let name = n.tag_name().name();
        if name == "p" {
            paragraphs += 1;
            continue;
        }

        if !matches!(name, "foreignObject" | "div" | "span") {
            return false;
        }

        let class_attr = n.attribute("class").unwrap_or_default();
        if class_attr
            .split_whitespace()
            .any(|t| matches!(t, "label-icon" | "fa" | "fas" | "far" | "fab"))
        {
            return false;
        }
    }

    paragraphs == 1
}

fn build_html_overrides_by_font(
    samples_by_font: &BTreeMap<String, Vec<Sample>>,
    plain_html_only: bool,
) -> BTreeMap<String, Vec<(String, f64)>> {
    let mut out = BTreeMap::new();
    for (font_key, ss) in samples_by_font {
        let mut by_text: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        let mut mixed_texts: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for s in ss {
            if plain_html_only && !s.plain_html_label {
                mixed_texts.insert(s.text.clone());
                continue;
            }
            if !(s.width_px.is_finite() && s.width_px > 0.0) {
                continue;
            }
            let em = s.width_px / s.font_size_px.max(1.0);
            if em.is_finite() && em > 0.0 {
                by_text.entry(s.text.clone()).or_default().push(em);
            }
        }

        let mut html_overrides = Vec::new();
        for (text, mut ems) in by_text {
            if plain_html_only && mixed_texts.contains(&text) {
                continue;
            }
            let Some(m) = median(&mut ems) else {
                continue;
            };
            if m.is_finite() && m > 0.0 {
                html_overrides.push((text, m));
            }
        }
        html_overrides.sort_by(|a, b| a.0.cmp(&b.0));
        out.insert(font_key.clone(), html_overrides);
    }
    out
}

fn replace_html_overrides_for_font(
    source: &str,
    font_key: &str,
    html_overrides: &[(String, f64)],
) -> Result<String, XtaskError> {
    let font_key_line = format!("        font_key: {font_key:?},");
    let Some(font_key_start) = source.find(&font_key_line) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "preserved font metrics missing font_key {font_key:?}"
        )));
    };

    let Some(rel_html_start) = source[font_key_start..].find("        html_overrides: &[") else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "preserved font metrics missing html_overrides for font_key {font_key:?}"
        )));
    };
    let html_start = font_key_start + rel_html_start;
    let Some(rel_svg_start) = source[html_start..].find("        svg_overrides: &[") else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "preserved font metrics missing svg_overrides after font_key {font_key:?}"
        )));
    };
    let svg_start = html_start + rel_svg_start;

    let header = "        html_overrides: &[\n";
    let entries_start = html_start + header.len();
    let Some(rel_entries_end) = source[html_start..svg_start].rfind("        ],\n") else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "preserved font metrics has malformed html_overrides for font_key {font_key:?}"
        )));
    };
    let entries_end = html_start + rel_entries_end;
    let mut entries = source[entries_start..entries_end].to_string();

    for (text, em) in html_overrides {
        let prefix = format!("            ({text:?}, ");
        let Some(start) = entries.find(&prefix) else {
            continue;
        };
        let value_start = start + prefix.len();
        let Some(rel_line_end) = entries[value_start..].find("),\n") else {
            continue;
        };
        let value_end = value_start + rel_line_end;
        entries.replace_range(value_start..value_end, &rust_f64(*em));
    }

    let mut out = String::with_capacity(source.len());
    out.push_str(&source[..entries_start]);
    out.push_str(&entries);
    out.push_str(&source[entries_end..]);
    Ok(out)
}

pub(crate) fn gen_font_metrics(args: Vec<String>) -> Result<(), XtaskError> {
    let mut in_dir: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut base_font_size_px: f64 = 16.0;
    let mut debug_text: Option<String> = None;
    let mut debug_dump: usize = 0;
    let mut backend: String = "browser".to_string();
    let mut browser_exe: Option<PathBuf> = None;
    let mut svg_sample_mode: String = "flowchart".to_string();
    let mut preserve_layout_from: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--in" => {
                i += 1;
                in_dir = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--font-size" => {
                i += 1;
                base_font_size_px = args
                    .get(i)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(16.0);
            }
            "--debug-text" => {
                i += 1;
                debug_text = args.get(i).map(|s| s.to_string());
            }
            "--debug-dump" => {
                i += 1;
                debug_dump = args
                    .get(i)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);
            }
            "--backend" => {
                i += 1;
                backend = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "browser".to_string());
            }
            "--svg-sample-mode" => {
                i += 1;
                svg_sample_mode = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "flowchart".to_string());
            }
            "--preserve-layout-from" => {
                i += 1;
                preserve_layout_from = args.get(i).map(PathBuf::from);
            }
            "--browser-exe" => {
                i += 1;
                browser_exe = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let in_dir = in_dir.ok_or(XtaskError::Usage)?;
    let out_path = out_path.ok_or(XtaskError::Usage)?;

    fn normalize_font_key(s: &str) -> String {
        s.chars()
            .filter_map(|ch| {
                if ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ';' {
                    None
                } else {
                    Some(ch.to_ascii_lowercase())
                }
            })
            .collect()
    }

    fn extract_base_font_family(svg: &str) -> String {
        let Ok(doc) = roxmltree::Document::parse(svg) else {
            return String::new();
        };
        let Some(root) = doc.descendants().find(|n| n.has_tag_name("svg")) else {
            return String::new();
        };
        let id = root.attribute("id").unwrap_or_default();
        let Some(style_node) = doc.descendants().find(|n| n.has_tag_name("style")) else {
            return String::new();
        };
        let style_text = style_node.text().unwrap_or_default();
        if id.is_empty() || style_text.is_empty() {
            return String::new();
        }
        let pat = format!(
            r#"#{id}\{{[^}}]*font-family:([^;}}]+)"#,
            id = regex::escape(id)
        );
        let Ok(re) = Regex::new(&pat) else {
            return String::new();
        };
        let Some(caps) = re.captures(style_text) else {
            return String::new();
        };
        caps.get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    }

    fn foreignobject_text_lines(fo: roxmltree::Node<'_, '_>) -> Vec<String> {
        let mut raw = String::new();
        for n in fo.descendants() {
            if n.is_element() {
                match n.tag_name().name() {
                    "br" => raw.push('\n'),
                    "p" => {
                        if !raw.is_empty() && !raw.ends_with('\n') {
                            raw.push('\n');
                        }
                    }
                    _ => {}
                }
            }
            if n.is_text() {
                if let Some(t) = n.text() {
                    raw.push_str(t);
                }
            }
        }

        raw.split('\n')
            .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
    }

    fn extract_flowchart_title_font_size_px(svg: &str, diagram_id: &str) -> Option<f64> {
        if diagram_id.is_empty() {
            return None;
        }
        let Ok(doc) = roxmltree::Document::parse(svg) else {
            return None;
        };
        let style_node = doc.descendants().find(|n| n.has_tag_name("style"))?;
        let style_text = style_node.text().unwrap_or_default();
        if style_text.is_empty() {
            return None;
        }
        let pat = format!(
            r#"#{id}\s+\.flowchartTitleText\{{[^}}]*font-size:([0-9.]+)px"#,
            id = regex::escape(diagram_id)
        );
        let Ok(re) = Regex::new(&pat) else {
            return None;
        };
        let caps = re.captures(style_text)?;
        caps.get(1)?.as_str().parse::<f64>().ok()
    }

    fn extract_base_font_size_px(svg: &str, diagram_id: &str) -> Option<f64> {
        if diagram_id.is_empty() {
            return None;
        }
        let Ok(doc) = roxmltree::Document::parse(svg) else {
            return None;
        };
        let style_node = doc.descendants().find(|n| n.has_tag_name("style"))?;
        let style_text = style_node.text().unwrap_or_default();
        if style_text.is_empty() {
            return None;
        }
        let pat = format!(
            r#"#{id}\{{[^}}]*font-size:([0-9.]+)px"#,
            id = regex::escape(diagram_id)
        );
        let Ok(re) = Regex::new(&pat) else {
            return None;
        };
        let caps = re.captures(style_text)?;
        caps.get(1)?.as_str().parse::<f64>().ok()
    }

    let mut html_samples: Vec<Sample> = Vec::new();
    let mut html_seed_samples: Vec<Sample> = Vec::new();
    let mut svg_samples: Vec<Sample> = Vec::new();
    let mut font_family_by_key: BTreeMap<String, String> = BTreeMap::new();
    let Ok(entries) = fs::read_dir(&in_dir) else {
        return Err(XtaskError::ReadFile {
            path: in_dir.display().to_string(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        });
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_file_with_extension(&path, "svg") {
            continue;
        }
        let svg = match fs::read_to_string(&path) {
            Ok(v) => v,
            Err(err) => {
                return Err(XtaskError::ReadFile {
                    path: path.display().to_string(),
                    source: err,
                });
            }
        };

        let base_family_raw = extract_base_font_family(&svg);
        let font_key = normalize_font_key(&base_family_raw);
        if font_key.is_empty() {
            continue;
        }
        // Mermaid's `calculateTextDimensions` probes both `sans-serif` and the configured
        // `fontFamily`. Generate a dedicated `sans-serif` table so headless `calculateTextWidth`
        // call sites can follow upstream behavior.
        let sans_key = "sans-serif".to_string();
        font_family_by_key
            .entry(sans_key.clone())
            .or_insert_with(|| "sans-serif".to_string());
        font_family_by_key
            .entry(font_key.clone())
            .or_insert_with(|| base_family_raw.clone());

        let Ok(doc) = roxmltree::Document::parse(&svg) else {
            continue;
        };

        let Some(root_svg) = doc.descendants().find(|n| n.has_tag_name("svg")) else {
            continue;
        };
        let diagram_id = root_svg.attribute("id").unwrap_or_default();
        let diagram_font_size_px = extract_base_font_size_px(&svg, diagram_id)
            .unwrap_or(base_font_size_px)
            .max(1.0);

        for fo in doc
            .descendants()
            .filter(|n| n.has_tag_name("foreignObject"))
        {
            let lines = foreignobject_text_lines(fo);
            for text in &lines {
                if text.is_empty() {
                    continue;
                }
                // Seed samples are used to build the per-font character set (including unicode
                // characters from long labels). Width is intentionally zero so these do not affect
                // `html_overrides` regression.
                html_seed_samples.push(Sample {
                    font_key: font_key.clone(),
                    text: text.clone(),
                    width_px: 0.0,
                    font_size_px: diagram_font_size_px,
                    plain_html_label: false,
                });
                html_seed_samples.push(Sample {
                    font_key: sans_key.clone(),
                    text: text.clone(),
                    width_px: 0.0,
                    font_size_px: diagram_font_size_px,
                    plain_html_label: false,
                });
            }

            let width_px = fo
                .attribute("width")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0);
            if !(width_px.is_finite() && width_px > 0.0) {
                continue;
            }
            // Mermaid HTML labels are rendered with `max-width: 200px`. When a label hits that
            // constraint, the measured width is no longer a linear function of per-character
            // advances. Filter those samples out to keep the regression stable.
            if width_px >= 190.0 {
                continue;
            }
            if lines.len() != 1 {
                continue;
            }
            let text = lines[0].clone();
            if text.is_empty() {
                continue;
            }
            html_samples.push(Sample {
                font_key: font_key.clone(),
                text,
                width_px: foreignobject_content_width_px(fo, width_px),
                font_size_px: diagram_font_size_px,
                plain_html_label: is_plain_html_label_sample(fo),
            });
        }

        // Collect SVG `<text>` samples to later derive a `svg_scale` factor that approximates
        // Mermaid's SVG text advance measurement behavior (`getComputedTextLength()` in practice).

        // Prefer collecting the inner tspans used by Mermaid's `createText(...)` output. These
        // correspond to individual wrapped lines and are abundant across fixtures, which makes the
        // derived scale significantly more stable than the older "title-dominant viewBox" heuristic.
        for tspan in doc.descendants().filter(|n| n.has_tag_name("tspan")) {
            let class = tspan.attribute("class").unwrap_or_default();
            if !class.split_whitespace().any(|t| t == "text-inner-tspan") {
                continue;
            }
            let line = tspan.text().unwrap_or_default().trim().to_string();
            if line.is_empty() {
                continue;
            }
            svg_samples.push(Sample {
                font_key: font_key.clone(),
                text: line.clone(),
                width_px: 0.0,
                font_size_px: diagram_font_size_px,
                plain_html_label: false,
            });
            svg_samples.push(Sample {
                font_key: sans_key.clone(),
                text: line,
                width_px: 0.0,
                font_size_px: diagram_font_size_px,
                plain_html_label: false,
            });
        }

        // Keep flowchart diagram title samples as a fallback (they are usually unwrapped).
        if let Some(title_node) = doc.descendants().find(|n| {
            n.has_tag_name("text")
                && n.attribute("class")
                    .unwrap_or_default()
                    .split_whitespace()
                    .any(|t| t == "flowchartTitleText")
        }) {
            let title_text = title_node.text().unwrap_or_default().trim().to_string();
            if !title_text.is_empty() {
                let title_font_size_px = extract_flowchart_title_font_size_px(&svg, diagram_id)
                    .unwrap_or(diagram_font_size_px)
                    .max(1.0);
                svg_samples.push(Sample {
                    font_key: font_key.clone(),
                    text: title_text.clone(),
                    width_px: 0.0,
                    font_size_px: title_font_size_px,
                    plain_html_label: false,
                });
                svg_samples.push(Sample {
                    font_key: sans_key.clone(),
                    text: title_text,
                    width_px: 0.0,
                    font_size_px: title_font_size_px,
                    plain_html_label: false,
                });
            }
        }

        // Mermaid sequence diagrams render many labels as plain SVG `<text>` (or single `<tspan>`
        // runs) without the `text-inner-tspan` helper class. When generating metrics for those
        // diagrams, include the relevant label strings so we can derive stable `svg_overrides`
        // from upstream fixtures.
        if svg_sample_mode == "sequence" {
            for text_node in doc.descendants().filter(|n| n.has_tag_name("text")) {
                let class = text_node.attribute("class").unwrap_or_default();
                let tokens: Vec<&str> = class.split_whitespace().collect();
                if tokens.is_empty() {
                    continue;
                }
                let is_sequence_label = tokens.iter().any(|t| {
                    matches!(
                        *t,
                        "messageText"
                            | "noteText"
                            | "labelText"
                            | "loopText"
                            | "actor"
                            | "actor-man"
                    )
                });
                if !is_sequence_label {
                    continue;
                }

                // Prefer per-line `<tspan>` runs when present.
                let mut pushed_any = false;
                for tspan in text_node.children().filter(|n| n.has_tag_name("tspan")) {
                    let line = tspan.text().unwrap_or_default().trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    pushed_any = true;
                    svg_samples.push(Sample {
                        font_key: font_key.clone(),
                        text: line.clone(),
                        width_px: 0.0,
                        font_size_px: diagram_font_size_px,
                        plain_html_label: false,
                    });
                    svg_samples.push(Sample {
                        font_key: sans_key.clone(),
                        text: line,
                        width_px: 0.0,
                        font_size_px: diagram_font_size_px,
                        plain_html_label: false,
                    });
                }
                if pushed_any {
                    continue;
                }

                let line = text_node.text().unwrap_or_default().trim().to_string();
                if line.is_empty() {
                    continue;
                }
                svg_samples.push(Sample {
                    font_key: font_key.clone(),
                    text: line.clone(),
                    width_px: 0.0,
                    font_size_px: diagram_font_size_px,
                    plain_html_label: false,
                });
                svg_samples.push(Sample {
                    font_key: sans_key.clone(),
                    text: line,
                    width_px: 0.0,
                    font_size_px: diagram_font_size_px,
                    plain_html_label: false,
                });
            }
        }
    }

    // Add a small set of extra seed strings that are known to appear across non-flowchart
    // diagrams (notably ER) and that are sensitive to uppercase kerning/hinting in Chromium.
    // These samples improve `calculateTextWidth` parity without requiring per-diagram tables.
    const EXTRA_SEED_TEXTS: &[&str] = &["DRIVER", "PERSON"];
    for font_key in font_family_by_key.keys().cloned().collect::<Vec<_>>() {
        for &text in EXTRA_SEED_TEXTS {
            html_seed_samples.push(Sample {
                font_key: font_key.clone(),
                text: text.to_string(),
                width_px: 0.0,
                font_size_px: base_font_size_px.max(1.0),
                plain_html_label: false,
            });
            svg_samples.push(Sample {
                font_key: font_key.clone(),
                text: text.to_string(),
                width_px: 0.0,
                font_size_px: base_font_size_px.max(1.0),
                plain_html_label: false,
            });
        }
    }

    if html_samples.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no foreignObject samples found under {}",
            in_dir.display()
        )));
    }

    if let Some(preserve_path) = preserve_layout_from.as_deref() {
        let mut html_samples_by_font: BTreeMap<String, Vec<Sample>> = BTreeMap::new();
        for s in html_samples {
            html_samples_by_font
                .entry(s.font_key.clone())
                .or_default()
                .push(s);
        }

        let html_overrides_by_font = build_html_overrides_by_font(&html_samples_by_font, true);
        if html_overrides_by_font.is_empty() {
            return Err(XtaskError::SvgCompareFailed(
                "no html_overrides collected for preserve-layout mode".to_string(),
            ));
        }

        let mut preserved =
            fs::read_to_string(preserve_path).map_err(|source| XtaskError::ReadFile {
                path: preserve_path.display().to_string(),
                source,
            })?;
        let mut replaced_fonts = 0usize;
        for (font_key, html_overrides) in &html_overrides_by_font {
            let font_key_line = format!("        font_key: {font_key:?},");
            if !preserved.contains(&font_key_line) {
                continue;
            }
            preserved = replace_html_overrides_for_font(&preserved, font_key, html_overrides)?;
            replaced_fonts += 1;
        }
        if replaced_fonts == 0 {
            return Err(XtaskError::SvgCompareFailed(
                "preserve-layout mode did not match any generated font table".to_string(),
            ));
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
                path: parent.display().to_string(),
                source,
            })?;
        }
        fs::write(&out_path, preserved).map_err(|source| XtaskError::WriteFile {
            path: out_path.display().to_string(),
            source,
        })?;

        return Ok(());
    }

    if matches!(backend.as_str(), "browser" | "puppeteer") && !svg_samples.is_empty() {
        let browser_exe = if let Some(p) = browser_exe.as_deref() {
            p.to_path_buf()
        } else if cfg!(windows) {
            detect_windows_browser_exe().ok_or_else(|| {
                XtaskError::SvgCompareFailed(
                    "no supported browser found for font measurement".into(),
                )
            })?
        } else {
            return Err(XtaskError::SvgCompareFailed(
                "browser measurement requires --browser-exe on this platform".into(),
            ));
        };

        let node_cwd = crate::cmd::mermaid_cli_root();

        // Group by `(font_key, font_size_px)` to minimize browser round-trips.
        let mut groups: BTreeMap<(String, i64), Vec<usize>> = BTreeMap::new();
        for (idx, s) in svg_samples.iter().enumerate() {
            let size_key = (s.font_size_px * 1000.0).round() as i64;
            groups
                .entry((s.font_key.clone(), size_key))
                .or_default()
                .push(idx);
        }

        for ((font_key, size_key), idxs) in groups {
            let Some(font_family) = font_family_by_key.get(&font_key) else {
                continue;
            };
            let font_size_px = (size_key as f64) / 1000.0;
            let strings = idxs
                .iter()
                .map(|&i| svg_samples[i].text.clone())
                .collect::<Vec<_>>();
            let widths_px = measure_svg_text_bbox_widths_via_browser(
                &node_cwd,
                &browser_exe,
                font_family,
                font_size_px,
                &strings,
            )?;
            for (&i, w) in idxs.iter().zip(widths_px.into_iter()) {
                svg_samples[i].width_px = w;
            }
        }

        svg_samples.retain(|s| s.width_px.is_finite() && s.width_px > 0.0);
    }

    if let Some(dt) = debug_text.as_deref() {
        eprintln!("debug-text={dt:?}");
        for (label, list) in [("html", &html_samples), ("svg", &svg_samples)] {
            let mut by_font: BTreeMap<String, Vec<f64>> = BTreeMap::new();
            for s in list.iter() {
                if s.text == dt {
                    by_font
                        .entry(s.font_key.clone())
                        .or_default()
                        .push(s.width_px / s.font_size_px.max(1.0));
                }
            }
            if by_font.is_empty() {
                continue;
            }
            eprintln!("  source={label}");
            for (k, mut ws) in by_font {
                ws.sort_by(|a, b| a.total_cmp(b));
                let min = ws.first().copied().unwrap_or(0.0);
                let max = ws.last().copied().unwrap_or(0.0);
                let mean = if ws.is_empty() {
                    0.0
                } else {
                    ws.iter().sum::<f64>() / ws.len() as f64
                };
                eprintln!(
                    "    font_key={:?} samples={} em[min/mean/max]=[{:.4}/{:.4}/{:.4}]",
                    k,
                    ws.len(),
                    min,
                    mean,
                    max
                );
            }
        }
    }

    if debug_dump > 0 {
        let mut by_font: BTreeMap<String, Vec<&Sample>> = BTreeMap::new();
        for s in &html_samples {
            by_font.entry(s.font_key.clone()).or_default().push(s);
        }
        eprintln!("debug-dump: showing up to {debug_dump} samples per font_key");
        for (k, mut ss) in by_font {
            ss.sort_by(|a, b| {
                a.text
                    .cmp(&b.text)
                    .then_with(|| a.width_px.total_cmp(&b.width_px))
            });
            eprintln!("  font_key={k:?} total={}", ss.len());
            for s in ss.into_iter().take(debug_dump) {
                eprintln!("    text={:?} width_px={}", s.text, s.width_px);
            }
        }
    }

    // Group by font key and fit widths in `em`, separately for:
    // - HTML `<foreignObject>` labels (used when `htmlLabels=true`), and
    // - SVG `<text>` titles (used for the flowchart title).
    let mut html_samples_by_font: BTreeMap<String, Vec<Sample>> = BTreeMap::new();
    for s in html_samples {
        html_samples_by_font
            .entry(s.font_key.clone())
            .or_default()
            .push(s);
    }
    let mut html_seed_samples_by_font: BTreeMap<String, Vec<Sample>> = BTreeMap::new();
    for s in html_seed_samples {
        html_seed_samples_by_font
            .entry(s.font_key.clone())
            .or_default()
            .push(s);
    }
    let mut svg_samples_by_font: BTreeMap<String, Vec<Sample>> = BTreeMap::new();
    for s in svg_samples {
        svg_samples_by_font
            .entry(s.font_key.clone())
            .or_default()
            .push(s);
    }

    fn heuristic_char_width_em(ch: char) -> f64 {
        if ch == ' ' {
            return 0.33;
        }
        if ch == '\t' {
            return 0.66;
        }
        if ch == '_' || ch == '-' {
            return 0.33;
        }
        if matches!(ch, '.' | ',' | ':' | ';') {
            return 0.28;
        }
        if matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '/') {
            return 0.33;
        }
        if matches!(ch, '+' | '*' | '=' | '\\' | '^' | '|' | '~') {
            return 0.45;
        }
        if ch.is_ascii_digit() {
            return 0.56;
        }
        if ch.is_ascii_uppercase() {
            return match ch {
                'I' => 0.30,
                'W' => 0.85,
                _ => 0.60,
            };
        }
        if ch.is_ascii_lowercase() {
            return match ch {
                'i' | 'l' => 0.28,
                'm' | 'w' => 0.78,
                'k' | 'y' => 0.55,
                _ => 0.43,
            };
        }
        0.60
    }

    #[derive(Debug, Clone)]
    struct FontTable {
        font_key: String,
        default_em: f64,
        entries: Vec<(char, f64)>,
        kern_pairs: Vec<(u32, u32, f64)>,
        /// Extra width adjustment (in `em`) for the pattern `a + ' ' + b`.
        ///
        /// In Chromium layout, the width contributed by a normal space can depend on surrounding
        /// glyphs (GPOS kerning around spaces, etc.). Measuring 2-char strings like `"e "` / `" T"`
        /// is unreliable because HTML collapses leading/trailing spaces. Instead, we capture the
        /// combined adjustment for internal spaces via these trigrams.
        space_trigrams: Vec<(u32, u32, f64)>,
        /// Extra width adjustment (in `em`) for the trigram pattern `a + b + c` (with no
        /// whitespace).
        ///
        /// In Chromium layout, text advances are not perfectly decomposable into
        /// `single-char widths + pair kerning`: subpixel positioning and hinting can make glyph
        /// contributions depend on immediate neighbors. We learn the residual for 3-char samples
        /// and apply it as a local correction while measuring longer strings.
        trigrams: Vec<(u32, u32, u32, f64)>,
        /// Exact (already-quantized) widths for single-line HTML `<foreignObject>` labels, stored
        /// in `em` units (width_px / font_size_px).
        ///
        /// This is used as an override for DOM parity: Chromium's layout uses fixed-point
        /// arithmetic and hinting that can make widths non-additive even with kerning pairs and
        /// local trigram residuals.
        html_overrides: Vec<(String, f64)>,
        /// Exact SVG `<text>` extents (in `em`) for `text-anchor: middle`, stored as `(text, left_em, right_em)`.
        ///
        /// SVG `getBBox()` and `getComputedTextLength()` do not match HTML layout advances, and
        /// approximations (scale factors / per-glyph overhang) can drift for long titles. These
        /// overrides are measured via a real browser and used to match upstream viewBox parity.
        svg_overrides: Vec<(String, f64, f64)>,
    }

    fn fit_tables(
        samples_by_font: BTreeMap<String, Vec<Sample>>,
        prior_by_font: Option<&BTreeMap<String, BTreeMap<char, f64>>>,
    ) -> BTreeMap<String, FontTable> {
        let mut out: BTreeMap<String, FontTable> = BTreeMap::new();

        for (font_key, mut ss) in samples_by_font {
            ss.sort_by(|a, b| {
                a.text
                    .cmp(&b.text)
                    .then_with(|| a.width_px.total_cmp(&b.width_px))
            });

            // Stage 1: lock in direct per-character widths from single-character samples (if any).
            let mut direct: BTreeMap<char, Vec<f64>> = BTreeMap::new();
            for s in &ss {
                let mut it = s.text.chars();
                let Some(ch) = it.next() else {
                    continue;
                };
                if it.next().is_some() {
                    continue;
                }
                let w_em = s.width_px / s.font_size_px.max(1.0);
                if !(w_em.is_finite() && w_em > 0.0) {
                    continue;
                }
                direct.entry(ch).or_default().push(w_em);
            }

            let mut fixed: BTreeMap<char, f64> = BTreeMap::new();
            for (ch, mut ws) in direct {
                if let Some(m) = median(&mut ws) {
                    fixed.insert(ch, m);
                }
            }

            // Stage 2: fit remaining characters via ridge regression around priors.
            let mut unknown_chars: Vec<char> = ss
                .iter()
                .flat_map(|s| s.text.chars())
                .filter(|ch| !fixed.contains_key(ch))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            unknown_chars.sort_by_key(|a| *a as u32);

            let mut unknown_index: BTreeMap<char, usize> = BTreeMap::new();
            for (idx, ch) in unknown_chars.iter().enumerate() {
                unknown_index.insert(*ch, idx);
            }

            let n_vars = unknown_chars.len();
            let mut sol: Vec<f64> = vec![0.0; n_vars];
            if n_vars > 0 {
                let mut at_a = vec![vec![0.0_f64; n_vars]; n_vars];
                let mut at_b = vec![0.0_f64; n_vars];
                let mut prior = vec![0.0_f64; n_vars];

                let priors_for_font = prior_by_font.and_then(|m| m.get(&font_key));
                for (idx, ch) in unknown_chars.iter().enumerate() {
                    prior[idx] = priors_for_font
                        .and_then(|m| m.get(ch))
                        .copied()
                        .unwrap_or_else(|| heuristic_char_width_em(*ch));
                }

                for s in &ss {
                    let mut counts = vec![0.0_f64; n_vars];
                    let mut fixed_sum_em: f64 = 0.0;
                    for ch in s.text.chars() {
                        if let Some(w) = fixed.get(&ch) {
                            fixed_sum_em += *w;
                            continue;
                        }
                        let Some(&idx) = unknown_index.get(&ch) else {
                            continue;
                        };
                        counts[idx] += 1.0;
                    }

                    let mut b_em = s.width_px / s.font_size_px.max(1.0) - fixed_sum_em;
                    if !b_em.is_finite() {
                        continue;
                    }
                    // For samples dominated by rounding noise, skip to avoid destabilizing the fit.
                    if b_em.abs() < 1e-6 {
                        continue;
                    }
                    // Clamp residuals to avoid pathological negative values caused by kerning or
                    // DOM rounding on very short strings.
                    if b_em < 0.0 {
                        b_em = 0.0;
                    }

                    for i in 0..n_vars {
                        let ci = counts[i];
                        if ci == 0.0 {
                            continue;
                        }
                        at_b[i] += ci * b_em;
                        for (j, count) in counts.iter().enumerate().take(n_vars) {
                            at_a[i][j] += ci * *count;
                        }
                    }
                }

                let lambda = 0.05;
                for i in 0..n_vars {
                    at_a[i][i] += lambda;
                    at_b[i] += lambda * prior[i];
                }

                let mut rhs = at_b;
                let mut mat = at_a;
                sol = solve_ridge(&mut mat, &mut rhs)
                    .into_iter()
                    .map(|v| v.max(0.0))
                    .collect();
            }

            let mut entries: Vec<(char, f64)> = Vec::new();
            for (ch, w) in fixed {
                entries.push((ch, w));
            }
            for (idx, ch) in unknown_chars.iter().enumerate() {
                entries.push((*ch, sol[idx]));
            }
            entries.sort_by(|a, b| (a.0 as u32).cmp(&(b.0 as u32)));

            let avg_em = if entries.is_empty() {
                0.6
            } else {
                entries.iter().map(|(_, v)| *v).sum::<f64>() / entries.len() as f64
            };

            out.insert(
                font_key.clone(),
                FontTable {
                    font_key,
                    default_em: avg_em.max(0.1),
                    entries,
                    kern_pairs: Vec::new(),
                    space_trigrams: Vec::new(),
                    trigrams: Vec::new(),
                    html_overrides: Vec::new(),
                    svg_overrides: Vec::new(),
                },
            );
        }

        out
    }

    fn detect_windows_browser_exe() -> Option<PathBuf> {
        let candidates = [
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ];
        for p in candidates {
            let path = PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    fn measure_text_widths_via_browser(
        node_cwd: &Path,
        browser_exe: &Path,
        font_family: &str,
        font_size_px: f64,
        strings: &[String],
    ) -> Result<Vec<f64>, XtaskError> {
        use std::process::Stdio;

        if strings.is_empty() {
            return Ok(Vec::new());
        }

        let input_json = serde_json::json!({
            "browser_exe": browser_exe.display().to_string(),
            "font_family": font_family,
            "font_size_px": font_size_px,
            "strings": strings,
        })
        .to_string();

        const JS: &str = r#"
const fs = require('fs');
const puppeteer = require('puppeteer-core');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const browserExe = input.browser_exe;
const fontFamily = input.font_family;
const fontSizePx = input.font_size_px;
const strings = input.strings;

 (async () => {
   const browser = await puppeteer.launch({
     headless: 'shell',
     executablePath: browserExe,
     args: [
       '--no-sandbox',
       '--disable-setuid-sandbox',
       // Match Mermaid CLI (Chromium) layout units more deterministically.
       '--force-device-scale-factor=1',
     ],
   });
 
   const page = await browser.newPage();
   await page.setViewport({ width: 800, height: 600, deviceScaleFactor: 1 });
   await page.setContent(`<!doctype html><html><head><style>body{margin:0;padding:0;} p{margin:0;}</style></head><body></body></html>`);
 
   const widths = await page.evaluate(({ strings, fontFamily, fontSizePx }) => {
     const ff = String(fontFamily || '').replace(/;\s*$/, '');
 
     // Mimic Mermaid's flowchart foreignObject label container (single-line).
     const div = document.createElement('div');
     div.style.display = 'table-cell';
     div.style.whiteSpace = 'nowrap';
     div.style.lineHeight = '1.5';
     div.style.maxWidth = '200px';
     div.style.textAlign = 'center';
     div.style.fontFamily = ff;
     div.style.fontSize = `${fontSizePx}px`;
 
     const span = document.createElement('span');
     span.className = 'nodeLabel';
     const p = document.createElement('p');
     span.appendChild(p);
     div.appendChild(span);
     document.body.appendChild(div);
 
     const out = [];
     for (const s of strings) {
       const ss = String(s);
       // A lone U+0020 would collapse away in HTML and measure as 0px. Use NBSP for that one
       // special case so we can still derive correct space advances for in-line spaces.
       p.textContent = ss === ' ' ? '\u00A0' : ss;
       out.push(div.getBoundingClientRect().width);
     }
     return out;
   }, { strings, fontFamily, fontSizePx });

  console.log(JSON.stringify(widths));
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
"#;

        let mut cmd = Command::new("node");
        cmd.current_dir(node_cwd)
            .arg("-e")
            .arg(JS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to spawn node: {source}"))
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, input_json.as_bytes()).map_err(|source| {
                XtaskError::SvgCompareFailed(format!("failed to write node stdin: {source}"))
            })?;
        }
        let output = child.wait_with_output().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to run node: {source}"))
        })?;
        if !output.status.success() {
            return Err(XtaskError::SvgCompareFailed(
                "browser measurement failed".to_string(),
            ));
        }

        let widths_px: Vec<f64> =
            serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
        let mut out = Vec::with_capacity(widths_px.len());
        for w in widths_px {
            if w.is_finite() && w >= 0.0 {
                out.push(w);
            } else {
                out.push(0.0);
            }
        }
        Ok(out)
    }

    fn measure_svg_text_bbox_widths_via_browser(
        node_cwd: &Path,
        browser_exe: &Path,
        font_family: &str,
        font_size_px: f64,
        strings: &[String],
    ) -> Result<Vec<f64>, XtaskError> {
        use std::process::Stdio;
        if strings.is_empty() {
            return Ok(Vec::new());
        }
        let input_json = serde_json::json!({
            "browser_exe": browser_exe.display().to_string(),
            "font_family": font_family,
            "font_size_px": font_size_px,
            "strings": strings,
        })
        .to_string();
        const JS: &str = r#"
const fs = require('fs');
const puppeteer = require('puppeteer-core');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const browserExe = input.browser_exe;
const fontFamily = input.font_family;
const fontSizePx = input.font_size_px;
const strings = input.strings;

(async () => {
  const browser = await puppeteer.launch({
    headless: 'shell',
    executablePath: browserExe,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  const page = await browser.newPage();
  await page.setContent(`<!doctype html><html><head><style>body{margin:0;padding:0;}</style></head><body></body></html>`);

  const widths = await page.evaluate(({ strings, fontFamily, fontSizePx }) => {
    const out = [];
    const SVG_NS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('width', '1000');
    svg.setAttribute('height', '200');
    document.body.appendChild(svg);

    const ff = String(fontFamily || '').replace(/;\\s*$/, '');
    for (const s of strings) {
      const t = document.createElementNS(SVG_NS, 'text');
      t.setAttribute('x', '0');
      t.setAttribute('y', '0');
      // Preserve spaces so `getComputedTextLength()` matches Mermaid's layout inputs.
      t.setAttribute('xml:space', 'preserve');
      t.setAttribute('style', `font-family:${ff};font-size:${fontSizePx}px;white-space:pre;`);
      t.textContent = String(s);
      svg.appendChild(t);
      out.push(t.getComputedTextLength());
      svg.removeChild(t);
    }
    return out;
  }, { strings, fontFamily, fontSizePx });

  console.log(JSON.stringify(widths));
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
"#;
        let mut cmd = Command::new("node");
        cmd.current_dir(node_cwd)
            .arg("-e")
            .arg(JS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to spawn node: {source}"))
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, input_json.as_bytes()).map_err(|source| {
                XtaskError::SvgCompareFailed(format!("failed to write node stdin: {source}"))
            })?;
        }
        let output = child.wait_with_output().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to run node: {source}"))
        })?;
        if !output.status.success() {
            return Err(XtaskError::SvgCompareFailed(
                "browser svg measurement failed".to_string(),
            ));
        }
        let widths_px: Vec<f64> =
            serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
        let mut out = Vec::with_capacity(widths_px.len());
        for w in widths_px {
            if w.is_finite() && w >= 0.0 {
                out.push(w);
            } else {
                out.push(0.0);
            }
        }
        Ok(out)
    }

    #[derive(Debug, Clone, Copy, serde::Deserialize)]
    struct SvgTextBBoxMetrics {
        adv_px: f64,
        bbox_x: f64,
        bbox_w: f64,
    }

    fn measure_svg_text_bbox_metrics_via_browser(
        node_cwd: &Path,
        browser_exe: &Path,
        font_family: &str,
        font_size_px: f64,
        strings: &[String],
    ) -> Result<Vec<SvgTextBBoxMetrics>, XtaskError> {
        use std::process::Stdio;
        if strings.is_empty() {
            return Ok(Vec::new());
        }
        let input_json = serde_json::json!({
            "browser_exe": browser_exe.display().to_string(),
            "font_family": font_family,
            "font_size_px": font_size_px,
            "strings": strings,
        })
        .to_string();
        const JS: &str = r#"
const fs = require('fs');
const puppeteer = require('puppeteer-core');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const browserExe = input.browser_exe;
const fontFamily = input.font_family;
const fontSizePx = input.font_size_px;
const strings = input.strings;

(async () => {
  const browser = await puppeteer.launch({
    headless: 'shell',
    executablePath: browserExe,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  const page = await browser.newPage();
  await page.setContent(`<!doctype html><html><head><style>body{margin:0;padding:0;}</style></head><body></body></html>`);

  const out = await page.evaluate(({ strings, fontFamily, fontSizePx }) => {
    const SVG_NS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('width', '1000');
    svg.setAttribute('height', '200');
    document.body.appendChild(svg);

    const ff = String(fontFamily || '').replace(/;\\s*$/, '');
    const res = [];
    for (const s of strings) {
      const t = document.createElementNS(SVG_NS, 'text');
      t.setAttribute('x', '0');
      t.setAttribute('y', '0');
      t.setAttribute('text-anchor', 'middle');
      // Preserve spaces so bbox/advance measurements match Mermaid's `<text>` output.
      t.setAttribute('xml:space', 'preserve');
      t.setAttribute('style', `font-family:${ff};font-size:${fontSizePx}px;white-space:pre;`);
      t.textContent = String(s);
      svg.appendChild(t);

      const adv = t.getComputedTextLength();
      const bb = t.getBBox();
      res.push({ adv_px: adv, bbox_x: bb.x, bbox_w: bb.width });
      svg.removeChild(t);
    }
    return res;
  }, { strings, fontFamily, fontSizePx });

  console.log(JSON.stringify(out));
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
"#;

        let mut cmd = Command::new("node");
        cmd.current_dir(node_cwd)
            .arg("-e")
            .arg(JS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to spawn node: {source}"))
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, input_json.as_bytes()).map_err(|source| {
                XtaskError::SvgCompareFailed(format!("failed to write node stdin: {source}"))
            })?;
        }
        let output = child.wait_with_output().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to run node: {source}"))
        })?;
        if !output.status.success() {
            return Err(XtaskError::SvgCompareFailed(
                "browser svg measurement failed".to_string(),
            ));
        }
        let raw: Vec<SvgTextBBoxMetrics> =
            serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
        let mut out = Vec::with_capacity(raw.len());
        for m in raw {
            if m.adv_px.is_finite()
                && m.adv_px >= 0.0
                && m.bbox_x.is_finite()
                && m.bbox_w.is_finite()
            {
                out.push(m);
            } else {
                out.push(SvgTextBBoxMetrics {
                    adv_px: 0.0,
                    bbox_x: 0.0,
                    bbox_w: 0.0,
                });
            }
        }
        Ok(out)
    }

    fn build_tables_via_browser(
        samples_by_font: BTreeMap<String, Vec<Sample>>,
        font_family_by_key: &BTreeMap<String, String>,
        base_font_size_px: f64,
        browser_exe: Option<&Path>,
    ) -> Result<BTreeMap<String, FontTable>, XtaskError> {
        let browser_exe = if let Some(p) = browser_exe {
            p.to_path_buf()
        } else if cfg!(windows) {
            detect_windows_browser_exe().ok_or_else(|| {
                XtaskError::SvgCompareFailed(
                    "no supported browser found for font measurement".into(),
                )
            })?
        } else {
            return Err(XtaskError::SvgCompareFailed(
                "browser measurement requires --browser-exe on this platform".into(),
            ));
        };

        let node_cwd = crate::cmd::mermaid_cli_root();

        let mut out: BTreeMap<String, FontTable> = BTreeMap::new();
        for (font_key, ss) in samples_by_font {
            let Some(font_family) = font_family_by_key.get(&font_key) else {
                continue;
            };

            let mut charset: std::collections::BTreeSet<char> = std::collections::BTreeSet::new();
            let mut pairset: std::collections::BTreeSet<(char, char)> =
                std::collections::BTreeSet::new();
            for s in &ss {
                let mut prev: Option<char> = None;
                for ch in s.text.chars() {
                    charset.insert(ch);
                    if let Some(p) = prev {
                        // Avoid pairs involving whitespace. HTML collapses leading/trailing spaces,
                        // which makes 2-char samples like `"e "` / `" T"` produce bogus negative
                        // "kerning" that effectively cancels the space width. Real Mermaid labels
                        // do not apply kerning to spaces, so skipping them keeps the model stable.
                        if !p.is_whitespace() && !ch.is_whitespace() {
                            pairset.insert((p, ch));
                        }
                    }
                    prev = Some(ch);
                }
            }
            if charset.is_empty() {
                continue;
            }
            let chars = charset.into_iter().collect::<Vec<_>>();
            let char_strings = chars.iter().map(|ch| ch.to_string()).collect::<Vec<_>>();
            let widths_px = measure_text_widths_via_browser(
                &node_cwd,
                &browser_exe,
                font_family,
                base_font_size_px,
                &char_strings,
            )?;
            let mut measured: BTreeMap<char, f64> = BTreeMap::new();
            for (ch, w_px) in chars.iter().copied().zip(widths_px.into_iter()) {
                let em = w_px / base_font_size_px.max(1.0);
                if em.is_finite() && em >= 0.0 {
                    measured.insert(ch, em);
                }
            }

            let char_em: BTreeMap<char, f64> = measured.clone();
            let mut entries = measured.into_iter().collect::<Vec<_>>();
            entries.sort_by(|a, b| (a.0 as u32).cmp(&(b.0 as u32)));

            let mut for_default = entries
                .iter()
                .filter(|(ch, _)| !ch.is_whitespace())
                .map(|(_, v)| *v)
                .collect::<Vec<_>>();
            let default_em = median(&mut for_default).unwrap_or_else(|| {
                if entries.is_empty() {
                    0.6
                } else {
                    entries.iter().map(|(_, v)| *v).sum::<f64>() / entries.len() as f64
                }
            });

            let mut kern_pairs: Vec<(u32, u32, f64)> = Vec::new();
            if !pairset.is_empty() {
                let pairs = pairset.into_iter().collect::<Vec<_>>();
                let pair_strings = pairs
                    .iter()
                    .map(|(a, b)| format!("{a}{b}"))
                    .collect::<Vec<_>>();
                let widths_px = measure_text_widths_via_browser(
                    &node_cwd,
                    &browser_exe,
                    font_family,
                    base_font_size_px,
                    &pair_strings,
                )?;
                for ((a, b), w_px) in pairs.into_iter().zip(widths_px.into_iter()) {
                    let a_em = char_em.get(&a).copied().unwrap_or(default_em);
                    let b_em = char_em.get(&b).copied().unwrap_or(default_em);
                    let pair_em = w_px / base_font_size_px.max(1.0);
                    if !(pair_em.is_finite() && a_em.is_finite() && b_em.is_finite()) {
                        continue;
                    }
                    let adj = pair_em - a_em - b_em;
                    if adj.abs() > 1e-9 && adj.is_finite() {
                        kern_pairs.push((a as u32, b as u32, adj));
                    }
                }
                kern_pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            }

            // Measure internal-space adjustments for `a + ' ' + b`.
            //
            // In Chromium, normal spaces can have context-dependent spacing due to kerning around
            // spaces and because U+0020 and U+00A0 are not guaranteed to share the same advance.
            // We cannot learn this from 2-char strings like `"e "` / `" T"` because HTML collapses
            // leading/trailing spaces, so we measure 3-char strings with the space in the middle.
            let mut space_trigrams: Vec<(u32, u32, f64)> = Vec::new();
            {
                let mut trigram_set: std::collections::BTreeSet<(char, char)> =
                    std::collections::BTreeSet::new();
                for s in &ss {
                    let chars = s.text.chars().collect::<Vec<_>>();
                    if chars.len() < 3 {
                        continue;
                    }
                    for i in 1..(chars.len() - 1) {
                        if chars[i] != ' ' {
                            continue;
                        }
                        let a = chars[i - 1];
                        let b = chars[i + 1];
                        if a.is_whitespace() || b.is_whitespace() {
                            continue;
                        }
                        trigram_set.insert((a, b));
                    }
                }
                if !trigram_set.is_empty() {
                    let trigrams = trigram_set.into_iter().collect::<Vec<_>>();
                    let trigram_strings = trigrams
                        .iter()
                        .map(|(a, b)| format!("{a} {b}"))
                        .collect::<Vec<_>>();
                    let widths_px = measure_text_widths_via_browser(
                        &node_cwd,
                        &browser_exe,
                        font_family,
                        base_font_size_px,
                        &trigram_strings,
                    )?;
                    let space_em = char_em.get(&' ').copied().unwrap_or(default_em);
                    for ((a, b), w_px) in trigrams.into_iter().zip(widths_px.into_iter()) {
                        let a_em = char_em.get(&a).copied().unwrap_or(default_em);
                        let b_em = char_em.get(&b).copied().unwrap_or(default_em);
                        let trigram_em = w_px / base_font_size_px.max(1.0);
                        if !(trigram_em.is_finite()
                            && a_em.is_finite()
                            && space_em.is_finite()
                            && b_em.is_finite())
                        {
                            continue;
                        }
                        let adj = trigram_em - a_em - space_em - b_em;
                        if adj.abs() > 1e-9 && adj.is_finite() {
                            space_trigrams.push((a as u32, b as u32, adj));
                        }
                    }
                    space_trigrams.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
                }
            }

            // Measure residuals for 3-char (non-whitespace) trigrams `a + b + c`.
            //
            // Even after learning `kern_pairs`, Chromium's DOM width is not perfectly additive due
            // to subpixel positioning/hinting. Capturing the 3-char residual and applying it as a
            // local correction greatly improves parity for longer words.
            let mut trigrams: Vec<(u32, u32, u32, f64)> = Vec::new();
            {
                let mut trigram_set: std::collections::BTreeSet<(char, char, char)> =
                    std::collections::BTreeSet::new();
                for s in &ss {
                    let chars = s.text.chars().collect::<Vec<_>>();
                    if chars.len() < 3 {
                        continue;
                    }
                    for i in 1..(chars.len() - 1) {
                        let a = chars[i - 1];
                        let b = chars[i];
                        let c = chars[i + 1];
                        if a.is_whitespace() || b.is_whitespace() || c.is_whitespace() {
                            continue;
                        }
                        trigram_set.insert((a, b, c));
                    }
                }

                if !trigram_set.is_empty() {
                    let trigrams_keys = trigram_set.into_iter().collect::<Vec<_>>();
                    let trigram_strings = trigrams_keys
                        .iter()
                        .map(|(a, b, c)| format!("{a}{b}{c}"))
                        .collect::<Vec<_>>();
                    let widths_px = measure_text_widths_via_browser(
                        &node_cwd,
                        &browser_exe,
                        font_family,
                        base_font_size_px,
                        &trigram_strings,
                    )?;

                    let mut kern_map: std::collections::BTreeMap<(u32, u32), f64> =
                        std::collections::BTreeMap::new();
                    for (a, b, adj) in &kern_pairs {
                        kern_map.insert((*a, *b), *adj);
                    }

                    for ((a, b, c), w_px) in trigrams_keys.into_iter().zip(widths_px.into_iter()) {
                        let a_em = char_em.get(&a).copied().unwrap_or(default_em);
                        let b_em = char_em.get(&b).copied().unwrap_or(default_em);
                        let c_em = char_em.get(&c).copied().unwrap_or(default_em);
                        let trigram_em = w_px / base_font_size_px.max(1.0);
                        if !(trigram_em.is_finite()
                            && a_em.is_finite()
                            && b_em.is_finite()
                            && c_em.is_finite())
                        {
                            continue;
                        }
                        let ab_adj = kern_map.get(&(a as u32, b as u32)).copied().unwrap_or(0.0);
                        let bc_adj = kern_map.get(&(b as u32, c as u32)).copied().unwrap_or(0.0);

                        let adj = trigram_em - a_em - b_em - c_em - ab_adj - bc_adj;
                        if adj.abs() > 1e-9 && adj.is_finite() {
                            trigrams.push((a as u32, b as u32, c as u32, adj));
                        }
                    }
                    trigrams.sort_by(|a, b| {
                        a.0.cmp(&b.0)
                            .then_with(|| a.1.cmp(&b.1))
                            .then_with(|| a.2.cmp(&b.2))
                    });
                }
            }

            let mut html_overrides: Vec<(String, f64)> = Vec::new();
            {
                let mut by_text: BTreeMap<String, Vec<f64>> = BTreeMap::new();
                for s in &ss {
                    if !(s.width_px.is_finite() && s.width_px > 0.0) {
                        continue;
                    }
                    let em = s.width_px / s.font_size_px.max(1.0);
                    if em.is_finite() && em > 0.0 {
                        by_text.entry(s.text.clone()).or_default().push(em);
                    }
                }
                for (text, mut ems) in by_text {
                    let Some(m) = median(&mut ems) else {
                        continue;
                    };
                    if m.is_finite() && m > 0.0 {
                        html_overrides.push((text, m));
                    }
                }
                html_overrides.sort_by(|a, b| a.0.cmp(&b.0));
            }

            out.insert(
                font_key.clone(),
                FontTable {
                    font_key,
                    default_em: default_em.max(0.1),
                    entries,
                    kern_pairs,
                    space_trigrams,
                    trigrams,
                    html_overrides,
                    svg_overrides: Vec::new(),
                },
            );
        }
        Ok(out)
    }

    let html_tables = if matches!(backend.as_str(), "browser" | "puppeteer") {
        // Use both HTML and SVG title samples to build the kerning pair set. Titles dominate the
        // flowchart viewport width in many upstream fixtures, so missing title-specific kerning
        // pairs can skew `viewBox`/`max-width` parity.
        let mut canvas_samples_by_font = html_samples_by_font.clone();
        for (k, mut ss) in html_seed_samples_by_font.clone() {
            canvas_samples_by_font.entry(k).or_default().append(&mut ss);
        }
        for (k, mut ss) in svg_samples_by_font.clone() {
            canvas_samples_by_font.entry(k).or_default().append(&mut ss);
        }
        build_tables_via_browser(
            canvas_samples_by_font,
            &font_family_by_key,
            base_font_size_px,
            browser_exe.as_deref(),
        )?
    } else {
        fit_tables(html_samples_by_font, None)
    };

    fn lookup_char_em(entries: &[(char, f64)], default_em: f64, ch: char) -> f64 {
        let mut lo = 0usize;
        let mut hi = entries.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            match entries[mid].0.cmp(&ch) {
                std::cmp::Ordering::Equal => return entries[mid].1,
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        default_em
    }

    fn lookup_kern_em(kern_pairs: &[(u32, u32, f64)], a: char, b: char) -> f64 {
        let key_a = a as u32;
        let key_b = b as u32;
        let mut lo = 0usize;
        let mut hi = kern_pairs.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            let (ma, mb, v) = kern_pairs[mid];
            match (ma.cmp(&key_a), mb.cmp(&key_b)) {
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => return v,
                (std::cmp::Ordering::Less, _) => lo = mid + 1,
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Less) => lo = mid + 1,
                _ => hi = mid,
            }
        }
        0.0
    }

    fn lookup_space_trigram_em(space_trigrams: &[(u32, u32, f64)], a: char, b: char) -> f64 {
        let key_a = a as u32;
        let key_b = b as u32;
        let mut lo = 0usize;
        let mut hi = space_trigrams.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            let (ma, mb, v) = space_trigrams[mid];
            match (ma.cmp(&key_a), mb.cmp(&key_b)) {
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => return v,
                (std::cmp::Ordering::Less, _) => lo = mid + 1,
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Less) => lo = mid + 1,
                _ => hi = mid,
            }
        }
        0.0
    }

    fn line_width_px(
        entries: &[(char, f64)],
        default_em: f64,
        kern_pairs: &[(u32, u32, f64)],
        space_trigrams: &[(u32, u32, f64)],
        text: &str,
        font_size: f64,
    ) -> f64 {
        let mut em = 0.0;
        let mut prev: Option<char> = None;
        let mut it = text.chars().peekable();
        while let Some(ch) = it.next() {
            em += lookup_char_em(entries, default_em, ch);
            if let Some(p) = prev {
                em += lookup_kern_em(kern_pairs, p, ch);
            }
            if ch == ' ' {
                if let (Some(a), Some(&b)) = (prev, it.peek()) {
                    em += lookup_space_trigram_em(space_trigrams, a, b);
                }
            }
            prev = Some(ch);
        }
        em * font_size
    }

    // Derive a simple SVG text scaling factor from SVG text samples:
    // `svg_scale ≈ computedTextLength(svg_text) / width(canvas_measureText_model)`.
    //
    // Mermaid uses SVG text measurement heavily (wrapping, label layout). We keep this as a single
    // scale factor (instead of per-glyph corrections) to remain deterministic while still
    // converging on upstream DOM output.
    let mut svg_scales_by_font: BTreeMap<String, f64> = BTreeMap::new();
    for (font_key, ss) in &svg_samples_by_font {
        let Some(html) = html_tables.get(font_key) else {
            continue;
        };
        let mut scales: Vec<f64> = Vec::new();
        for s in ss {
            let pred_px = line_width_px(
                &html.entries,
                html.default_em.max(0.1),
                &html.kern_pairs,
                &html.space_trigrams,
                &s.text,
                s.font_size_px.max(1.0),
            );
            if !(pred_px.is_finite() && pred_px > 0.0) {
                continue;
            }
            let scale = s.width_px / pred_px;
            if scale.is_finite() && (0.5..=2.0).contains(&scale) {
                scales.push(scale);
            }
        }
        if let Some(m) = median(&mut scales) {
            svg_scales_by_font.insert(font_key.clone(), m.clamp(0.5, 2.0));
        }
    }

    // Derive first/last-character bbox overhangs (relative to the `text-anchor=middle` position)
    // from browser SVG metrics. This models the fact that SVG `getBBox()` can be asymmetric due to
    // glyph overhangs. Overhangs are stored in `em` and applied on top of scaled advances.
    type SvgBBoxOverhangs = (f64, f64, Vec<(char, f64)>, Vec<(char, f64)>);
    let mut svg_bbox_overhangs_by_font: BTreeMap<String, SvgBBoxOverhangs> = BTreeMap::new();
    let mut svg_overrides_by_font: BTreeMap<String, Vec<(String, f64, f64)>> = BTreeMap::new();
    if matches!(backend.as_str(), "browser" | "puppeteer") {
        let browser_exe = if let Some(p) = browser_exe.as_deref() {
            p.to_path_buf()
        } else if cfg!(windows) {
            detect_windows_browser_exe().ok_or_else(|| {
                XtaskError::SvgCompareFailed(
                    "no supported browser found for font measurement".into(),
                )
            })?
        } else {
            return Err(XtaskError::SvgCompareFailed(
                "browser measurement requires --browser-exe on this platform".into(),
            ));
        };
        let node_cwd = crate::cmd::mermaid_cli_root();

        for (font_key, html) in &html_tables {
            let Some(font_family) = font_family_by_key.get(font_key) else {
                continue;
            };

            let mut chars = html.entries.iter().map(|(ch, _)| *ch).collect::<Vec<_>>();
            chars.sort_by_key(|ch| *ch as u32);
            chars.dedup();

            let strings = chars.iter().map(|ch| ch.to_string()).collect::<Vec<_>>();
            let metrics = measure_svg_text_bbox_metrics_via_browser(
                &node_cwd,
                &browser_exe,
                font_family,
                base_font_size_px.max(1.0),
                &strings,
            )?;

            let mut left_all: Vec<f64> = Vec::new();
            let mut right_all: Vec<f64> = Vec::new();
            let mut left_by_char: BTreeMap<char, f64> = BTreeMap::new();
            let mut right_by_char: BTreeMap<char, f64> = BTreeMap::new();
            for (ch, m) in chars.iter().copied().zip(metrics.into_iter()) {
                let adv_px = m.adv_px;
                let bbox_x = m.bbox_x;
                let bbox_w = m.bbox_w;
                if !(adv_px.is_finite()
                    && adv_px >= 0.0
                    && bbox_x.is_finite()
                    && bbox_w.is_finite())
                {
                    continue;
                }
                let left_extent = (-bbox_x).max(0.0);
                let right_extent = (bbox_x + bbox_w).max(0.0);
                let half = (adv_px / 2.0).max(0.0);
                let denom = base_font_size_px.max(1.0);
                let left_em = ((left_extent - half) / denom).clamp(-0.2, 0.2);
                let right_em = ((right_extent - half) / denom).clamp(-0.2, 0.2);
                left_all.push(left_em);
                right_all.push(right_em);
                left_by_char.insert(ch, left_em);
                right_by_char.insert(ch, right_em);
            }

            let default_left = median(&mut left_all).unwrap_or(0.0).clamp(-0.2, 0.2);
            let default_right = median(&mut right_all).unwrap_or(0.0).clamp(-0.2, 0.2);

            let mut left_entries: Vec<(char, f64)> = Vec::new();
            let mut right_entries: Vec<(char, f64)> = Vec::new();
            for (ch, v) in left_by_char {
                if (v - default_left).abs() > 1e-6 {
                    left_entries.push((ch, v));
                }
            }
            for (ch, v) in right_by_char {
                if (v - default_right).abs() > 1e-6 {
                    right_entries.push((ch, v));
                }
            }
            left_entries.sort_by_key(|(ch, _)| *ch as u32);
            right_entries.sort_by_key(|(ch, _)| *ch as u32);

            svg_bbox_overhangs_by_font.insert(
                font_key.clone(),
                (default_left, default_right, left_entries, right_entries),
            );
        }

        for (font_key, ss) in &svg_samples_by_font {
            let Some(font_family) = font_family_by_key.get(font_key) else {
                continue;
            };

            // Titles use a different font size (18px by default). SVG `getBBox()` can be
            // non-linear due to hinting, so measure overrides at the actual observed font size
            // and store them in `em` relative to that size.
            let base_size_key = (base_font_size_px.max(1.0) * 1000.0).round() as i64;
            let mut groups: BTreeMap<i64, Vec<String>> = BTreeMap::new();
            for s in ss {
                let size_key = (s.font_size_px.max(1.0) * 1000.0).round() as i64;
                groups.entry(size_key).or_default().push(s.text.clone());
            }

            let mut best_by_text: BTreeMap<String, (i64, f64, f64)> = BTreeMap::new();
            for (size_key, mut strings) in groups {
                strings.sort();
                strings.dedup();
                if strings.is_empty() {
                    continue;
                }

                let font_size_px = (size_key as f64) / 1000.0;
                let metrics = measure_svg_text_bbox_metrics_via_browser(
                    &node_cwd,
                    &browser_exe,
                    font_family,
                    font_size_px,
                    &strings,
                )?;
                let denom = font_size_px.max(1.0);

                for (text, m) in strings.into_iter().zip(metrics.into_iter()) {
                    let bbox_x = m.bbox_x;
                    let bbox_w = m.bbox_w;
                    if !(bbox_x.is_finite() && bbox_w.is_finite()) {
                        continue;
                    }
                    let left_px = (-bbox_x).max(0.0);
                    let right_px = (bbox_x + bbox_w).max(0.0);
                    let left_em = left_px / denom;
                    let right_em = right_px / denom;
                    if !(left_em.is_finite() && right_em.is_finite() && (left_em + right_em) > 0.0)
                    {
                        continue;
                    }

                    // If the same string appears at multiple sizes, prefer base size (16px)
                    // measurements since most SVG text in Mermaid flowcharts is at the diagram
                    // font size.
                    match best_by_text.get(&text) {
                        None => {
                            best_by_text.insert(text, (size_key, left_em, right_em));
                        }
                        Some((existing_size, _, _)) if *existing_size == base_size_key => {}
                        Some((_existing_size, _, _)) if size_key == base_size_key => {
                            best_by_text.insert(text, (size_key, left_em, right_em));
                        }
                        Some(_) => {}
                    }
                }
            }

            let overrides = best_by_text
                .into_iter()
                .map(|(text, (_size, left_em, right_em))| (text, left_em, right_em))
                .collect::<Vec<_>>();
            svg_overrides_by_font.insert(font_key.clone(), overrides);
        }
    }

    type FontTableWithScaleAndOverhangs = (FontTable, f64, SvgBBoxOverhangs);
    let mut tables: Vec<FontTableWithScaleAndOverhangs> = Vec::new();
    for (font_key, mut t) in html_tables {
        if let Some(ov) = svg_overrides_by_font.get(&font_key).cloned() {
            t.svg_overrides = ov;
        }
        let scale = svg_scales_by_font.get(&font_key).copied().unwrap_or(1.0);
        let overhangs = svg_bbox_overhangs_by_font
            .get(&font_key)
            .cloned()
            .unwrap_or((0.0, 0.0, Vec::new(), Vec::new()));
        tables.push((t, scale, overhangs));
    }

    // Render as a deterministic Rust module.
    let mut out = String::new();
    let _ = writeln!(&mut out, "// This file is generated by `xtask`.\n");
    let _ = writeln!(&mut out, "#[derive(Debug, Clone, Copy)]");
    let _ = writeln!(&mut out, "pub struct FontMetricsTables {{");
    let _ = writeln!(&mut out, "    pub font_key: &'static str,");
    let _ = writeln!(&mut out, "    pub base_font_size_px: f64,");
    let _ = writeln!(&mut out, "    pub default_em: f64,");
    let _ = writeln!(&mut out, "    pub entries: &'static [(char, f64)],");
    let _ = writeln!(&mut out, "    pub kern_pairs: &'static [(u32, u32, f64)],");
    let _ = writeln!(
        &mut out,
        "    pub space_trigrams: &'static [(u32, u32, f64)],"
    );
    let _ = writeln!(
        &mut out,
        "    pub trigrams: &'static [(u32, u32, u32, f64)],"
    );
    let _ = writeln!(
        &mut out,
        "    pub html_overrides: &'static [(&'static str, f64)],"
    );
    let _ = writeln!(
        &mut out,
        "    pub svg_overrides: &'static [(&'static str, f64, f64)],"
    );
    let _ = writeln!(&mut out, "    pub svg_scale: f64,");
    let _ = writeln!(&mut out, "    pub svg_bbox_overhang_left_default_em: f64,");
    let _ = writeln!(&mut out, "    pub svg_bbox_overhang_right_default_em: f64,");
    let _ = writeln!(
        &mut out,
        "    pub svg_bbox_overhang_left: &'static [(char, f64)],"
    );
    let _ = writeln!(
        &mut out,
        "    pub svg_bbox_overhang_right: &'static [(char, f64)],"
    );
    let _ = writeln!(&mut out, "}}\n");

    let _ = writeln!(
        &mut out,
        "pub const FONT_METRICS_TABLES: &[FontMetricsTables] = &["
    );
    for (t, svg_scale, (left_default, right_default, left_oh, right_oh)) in &tables {
        let _ = writeln!(
            &mut out,
            "    FontMetricsTables {{ font_key: {:?}, base_font_size_px: {}, default_em: {}, entries: &[",
            t.font_key,
            rust_f64(base_font_size_px),
            rust_f64(t.default_em)
        );
        for (ch, w) in &t.entries {
            let _ = writeln!(&mut out, "        ({:?}, {}),", ch, rust_f64(*w));
        }
        let _ = writeln!(&mut out, "    ], kern_pairs: &[");
        for (a, b, adj) in &t.kern_pairs {
            let _ = writeln!(&mut out, "        ({}, {}, {}),", a, b, rust_f64(*adj));
        }
        let _ = writeln!(&mut out, "    ], space_trigrams: &[");
        for (a, b, adj) in &t.space_trigrams {
            let _ = writeln!(&mut out, "        ({}, {}, {}),", a, b, rust_f64(*adj));
        }
        let _ = writeln!(&mut out, "    ], trigrams: &[");
        for (a, b, c, adj) in &t.trigrams {
            let _ = writeln!(
                &mut out,
                "        ({}, {}, {}, {}),",
                a,
                b,
                c,
                rust_f64(*adj)
            );
        }
        let _ = writeln!(&mut out, "    ], html_overrides: &[");
        for (text, em) in &t.html_overrides {
            let _ = writeln!(&mut out, "        ({:?}, {}),", text, rust_f64(*em));
        }
        let _ = writeln!(&mut out, "    ], svg_overrides: &[");
        for (text, left_em, right_em) in &t.svg_overrides {
            let _ = writeln!(
                &mut out,
                "        ({:?}, {}, {}),",
                text,
                rust_f64(*left_em),
                rust_f64(*right_em)
            );
        }
        let _ = writeln!(
            &mut out,
            "    ], svg_scale: {}, svg_bbox_overhang_left_default_em: {}, svg_bbox_overhang_right_default_em: {}, svg_bbox_overhang_left: &{:?}, svg_bbox_overhang_right: &{:?} }},\n",
            rust_f64(*svg_scale),
            rust_f64(*left_default),
            rust_f64(*right_default),
            left_oh,
            right_oh
        );
    }
    let _ = writeln!(&mut out, "];\n");

    let _ = writeln!(
        &mut out,
        "pub fn lookup_font_metrics(font_key: &str) -> Option<&'static FontMetricsTables> {{"
    );
    let _ = writeln!(&mut out, "    FONT_METRICS_TABLES");
    let _ = writeln!(&mut out, "        .iter()");
    let _ = writeln!(&mut out, "        .find(|t| t.font_key == font_key)");
    let _ = writeln!(&mut out, "}}\n");

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, out).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        Sample, build_html_overrides_by_font, foreignobject_content_width_px,
        replace_html_overrides_for_font, solve_ridge,
    };
    use std::collections::BTreeMap;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn solve_ridge_solves_small_dense_system() {
        let mut at_a = vec![vec![2.0, 1.0], vec![1.0, 3.0]];
        let mut at_b = vec![1.0, 2.0];

        let solution = solve_ridge(&mut at_a, &mut at_b);

        assert_close(solution[0], 0.2);
        assert_close(solution[1], 0.6);
    }

    #[test]
    fn solve_ridge_pivots_away_from_zero_diagonal() {
        let mut at_a = vec![vec![0.0, 2.0], vec![1.0, 1.0]];
        let mut at_b = vec![4.0, 3.0];

        let solution = solve_ridge(&mut at_a, &mut at_b);

        assert_close(solution[0], 1.0);
        assert_close(solution[1], 2.0);
    }

    #[test]
    fn icon_shape_foreignobject_width_excludes_paragraph_padding() {
        let svg = r#"<svg><g class="icon-shape"><foreignObject width="88.6875"><div><span><p>CloudWatch</p></span></div></foreignObject></g></svg>"#;
        let doc = roxmltree::Document::parse(svg).unwrap();
        let fo = doc
            .descendants()
            .find(|n| n.has_tag_name("foreignObject"))
            .unwrap();

        assert_close(foreignobject_content_width_px(fo, 88.6875), 84.6875);
    }

    #[test]
    fn normal_foreignobject_width_keeps_full_label_width() {
        let svg = r#"<svg><g class="node"><foreignObject width="111.109375"><div><span><p>This ❤ Unicode</p></span></div></foreignObject></g></svg>"#;
        let doc = roxmltree::Document::parse(svg).unwrap();
        let fo = doc
            .descendants()
            .find(|n| n.has_tag_name("foreignObject"))
            .unwrap();

        assert_close(foreignobject_content_width_px(fo, 111.109375), 111.109375);
    }

    #[test]
    fn preserve_layout_updates_existing_html_overrides_only() {
        let source = r#"pub const FONT_METRICS_TABLES: &[FontMetricsTables] = &[
    FontMetricsTables {
        font_key: "trebuchetms,verdana,arial,sans-serif",
        base_font_size_px: 16.0,
        default_em: 0.5,
        entries: &[
            ('A', 0.5),
        ],
        kern_pairs: &[
            (65, 66, -0.1),
        ],
        space_trigrams: &[],
        trigrams: &[],
        html_overrides: &[
            ("keep", 1.0),
            ("new", 1.0),
        ],
        svg_overrides: &[
            ("old", 0.5, 0.5),
        ],
        svg_scale: 1.0, svg_bbox_overhang_left_default_em: 0.0, svg_bbox_overhang_right_default_em: 0.0, svg_bbox_overhang_left: &[], svg_bbox_overhang_right: &[] },
];
"#;
        let mut samples_by_font = BTreeMap::new();
        samples_by_font.insert(
            "trebuchetms,verdana,arial,sans-serif".to_string(),
            vec![
                Sample {
                    font_key: "trebuchetms,verdana,arial,sans-serif".to_string(),
                    text: "new".to_string(),
                    width_px: 32.0,
                    font_size_px: 16.0,
                    plain_html_label: true,
                },
                Sample {
                    font_key: "trebuchetms,verdana,arial,sans-serif".to_string(),
                    text: "insert".to_string(),
                    width_px: 48.0,
                    font_size_px: 16.0,
                    plain_html_label: false,
                },
                Sample {
                    font_key: "trebuchetms,verdana,arial,sans-serif".to_string(),
                    text: "mixed".to_string(),
                    width_px: 16.0,
                    font_size_px: 16.0,
                    plain_html_label: true,
                },
                Sample {
                    font_key: "trebuchetms,verdana,arial,sans-serif".to_string(),
                    text: "mixed".to_string(),
                    width_px: 48.0,
                    font_size_px: 16.0,
                    plain_html_label: false,
                },
            ],
        );
        let by_font = build_html_overrides_by_font(&samples_by_font, true);
        let updated = replace_html_overrides_for_font(
            source,
            "trebuchetms,verdana,arial,sans-serif",
            by_font.get("trebuchetms,verdana,arial,sans-serif").unwrap(),
        )
        .unwrap();

        assert!(updated.contains("            ('A', 0.5),"));
        assert!(updated.contains("            (65, 66, -0.1),"));
        assert!(updated.contains("            (\"keep\", 1.0),"));
        assert!(updated.contains("            (\"new\", 2.0),"));
        assert!(!updated.contains("insert"));
        assert!(!updated.contains("mixed"));
        assert!(
            updated.contains(
                "        svg_overrides: &[\n            (\"old\", 0.5, 0.5),\n        ],"
            )
        );
    }
}
