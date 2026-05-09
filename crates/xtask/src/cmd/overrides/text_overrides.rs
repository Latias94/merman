//! Text/bbox override generators derived from upstream SVG fixtures.

use crate::XtaskError;
use std::fmt::Write as _;
use std::path::PathBuf;

pub(crate) fn gen_er_text_overrides(args: Vec<String>) -> Result<(), XtaskError> {
    use std::collections::{BTreeMap, BTreeSet};

    let workspace_root = crate::cmd::workspace_root();

    let mut in_dir: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

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
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let in_dir = in_dir.unwrap_or_else(|| {
        workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("er")
    });
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("crates")
            .join("merman-render")
            .join("src")
            .join("generated")
            .join("er_text_overrides_11_12_2.rs")
    });

    fn font_size_key(font_size: f64) -> u16 {
        if !(font_size.is_finite() && font_size > 0.0) {
            return 0;
        }
        let k = (font_size * 100.0).round();
        if !(k.is_finite() && k >= 0.0 && k <= (u16::MAX as f64)) {
            return 0;
        }
        k as u16
    }

    fn node_has_class_token(node: roxmltree::Node<'_, '_>, token: &str) -> bool {
        node.attribute("class").is_some_and(|c| {
            c.split_whitespace()
                .any(|t| !t.is_empty() && t.trim() == token)
        })
    }

    fn has_ancestor_class_token(node: roxmltree::Node<'_, '_>, token: &str) -> bool {
        let mut cur = Some(node);
        while let Some(n) = cur {
            if n.is_element() && node_has_class_token(n, token) {
                return true;
            }
            cur = n.parent();
        }
        false
    }

    fn parse_max_width_px(style: &str) -> Option<i64> {
        // Keep it strict: we only want the integer `max-width: Npx` that Mermaid emits.
        let s = style;
        let key = "max-width:";
        let idx = s.find(key)?;
        let rest = s[idx + key.len()..].trim_start();
        let mut num = String::new();
        for ch in rest.chars() {
            if ch.is_ascii_digit() {
                num.push(ch);
            } else {
                break;
            }
        }
        if num.is_empty() {
            return None;
        }
        let rest = &rest[num.len()..];
        if !rest.trim_start().starts_with("px") {
            return None;
        }
        num.parse::<i64>().ok()
    }

    // `((font_size_key, text) -> width_px)` and `((font_size_key, text) -> calc_text_width_px)`.
    let mut html_widths: BTreeMap<(u16, String), f64> = BTreeMap::new();
    let mut calc_text_widths: BTreeMap<(u16, String), i64> = BTreeMap::new();

    let mut svg_paths: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&in_dir).map_err(|e| {
        XtaskError::SvgCompareFailed(format!("failed to read dir {}: {}", in_dir.display(), e))
    })? {
        let entry = entry.map_err(|e| {
            XtaskError::SvgCompareFailed(format!(
                "failed to read dir entry {}: {}",
                in_dir.display(),
                e
            ))
        })?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|e| e.to_string_lossy().to_ascii_lowercase() == "svg")
        {
            svg_paths.push(path);
        }
    }
    svg_paths.sort();

    let mut conflicts: BTreeSet<String> = BTreeSet::new();
    for path in svg_paths {
        let svg = std::fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        let doc = roxmltree::Document::parse(&svg).map_err(|e| {
            XtaskError::SvgCompareFailed(format!(
                "failed to parse upstream ER SVG {}: {}",
                path.display(),
                e
            ))
        })?;

        for fo in doc
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "foreignObject")
        {
            let Some(w_str) = fo.attribute("width") else {
                continue;
            };
            let Ok(width_px) = w_str.parse::<f64>() else {
                continue;
            };
            if !(width_px.is_finite() && width_px >= 0.0) {
                continue;
            }

            // Mermaid ER labels are single-line in the fixtures we care about, but the HTML
            // structure varies:
            // - Normal labels: `<span class="nodeLabel"><p>TEXT</p></span>`
            // - Generic labels: raw text nodes (e.g. `type&lt;T&gt;`) without nested tags
            //
            // Extract the user-visible string by concatenating text nodes under the inner `<div>`.
            let div = fo
                .descendants()
                .find(|n| n.is_element() && n.tag_name().name() == "div");
            let Some(div) = div else {
                continue;
            };
            let mut text_decoded = String::new();
            for t in div.descendants().filter(|n| n.is_text()) {
                if let Some(s) = t.text() {
                    text_decoded.push_str(s);
                }
            }
            let text_decoded = text_decoded.trim().to_string();
            if text_decoded.is_empty() {
                continue;
            }

            // Mermaid erBox.ts passes a pre-workaround string into `calculateTextWidth()`:
            // generics get replaced from `<`/`>` to `&lt;`/`&gt;` before the call.
            let text_calc_input = if text_decoded.contains('<') || text_decoded.contains('>') {
                text_decoded.replace('<', "&lt;").replace('>', "&gt;")
            } else {
                text_decoded.clone()
            };

            let font_size = if has_ancestor_class_token(fo, "edgeLabel") {
                14.0
            } else {
                16.0
            };
            let fs_key = font_size_key(font_size);
            if fs_key == 0 {
                continue;
            }

            let html_key = (fs_key, text_decoded.clone());
            if let Some(prev) = html_widths.get(&html_key).copied() {
                if (prev - width_px).abs() > 1e-9 {
                    conflicts.insert(format!(
                        "html width conflict for font_size={} text={:?}: {} vs {} (file {})",
                        font_size,
                        text_decoded,
                        prev,
                        width_px,
                        path.display()
                    ));
                }
            } else {
                html_widths.insert(html_key, width_px);
            }

            // Try to derive `calculateTextWidth()` from Mermaid's `createText(..., width=calc+100)`.
            // This shows up as `max-width: <n>px` in the inner div style.
            let max_width_px = div.attribute("style").and_then(parse_max_width_px);

            if let Some(mw) = max_width_px {
                // Edge labels use the flowchart wrapping width (200px) and are not driven by
                // `calculateTextWidth()+100`.
                if mw != 200 && mw >= 100 {
                    let calc_w = mw - 100;
                    let calc_key = (fs_key, text_calc_input);
                    if let Some(prev) = calc_text_widths.get(&calc_key).copied() {
                        if prev != calc_w {
                            conflicts.insert(format!(
                                "calcTextWidth conflict for font_size={} text={:?}: {} vs {} (file {})",
                                font_size,
                                calc_key.1,
                                prev,
                                calc_w,
                                path.display()
                            ));
                        }
                    } else {
                        calc_text_widths.insert(calc_key, calc_w);
                    }
                }
            }
        }
    }

    if !conflicts.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "conflicts while generating ER text overrides:\n{}",
            conflicts.into_iter().collect::<Vec<_>>().join("\n")
        )));
    }

    fn rust_f64(v: f64) -> String {
        // Preserve `1/64` widths exactly when possible (e.g. `78.984375`).
        let mut s = format!("{v}");
        if !s.contains('.') && !s.contains('e') && !s.contains('E') {
            s.push_str(".0");
        }
        s
    }

    let mut out = String::new();
    let _ = writeln!(
        &mut out,
        "// This file is generated by `xtask gen-er-text-overrides`.\n//\n// Mermaid baseline: 11.12.3\n// Source: fixtures/upstream-svgs/er/*.svg\n"
    );
    let _ = writeln!(&mut out, "fn font_size_key(font_size: f64) -> u16 {{");
    let _ = writeln!(
        &mut out,
        "    if !(font_size.is_finite() && font_size > 0.0) {{ return 0; }}"
    );
    let _ = writeln!(&mut out, "    let k = (font_size * 100.0).round();");
    let _ = writeln!(
        &mut out,
        "    if !(k.is_finite() && k >= 0.0 && k <= (u16::MAX as f64)) {{ return 0; }}"
    );
    let _ = writeln!(&mut out, "    k as u16");
    let _ = writeln!(&mut out, "}}");
    let _ = writeln!(&mut out);

    let html_entries: Vec<(u16, String, f64)> = html_widths
        .into_iter()
        .map(|((fs, t), w)| (fs, t, w))
        .collect();
    let calc_entries: Vec<(u16, String, i64)> = calc_text_widths
        .into_iter()
        .map(|((fs, t), w)| (fs, t, w))
        .collect();

    let _ = writeln!(
        &mut out,
        "static HTML_WIDTH_OVERRIDES_PX: &[(u16, &str, f64)] = &["
    );
    for (fs, t, w) in &html_entries {
        let _ = writeln!(&mut out, "    ({fs}, {:?}, {}),", t, rust_f64(*w));
    }
    let _ = writeln!(&mut out, "];\n");

    let _ = writeln!(
        &mut out,
        "static CALC_TEXT_WIDTH_OVERRIDES_PX: &[(u16, &str, i64)] = &["
    );
    for (fs, t, w) in &calc_entries {
        let _ = writeln!(&mut out, "    ({fs}, {:?}, {w}),", t);
    }
    let _ = writeln!(&mut out, "];\n");

    let _ = writeln!(
        &mut out,
        "pub fn lookup_html_width_px(font_size: f64, text: &str) -> Option<f64> {{"
    );
    let _ = writeln!(&mut out, "    let fs = font_size_key(font_size);");
    let _ = writeln!(
        &mut out,
        "    if fs == 0 || text.is_empty() {{ return None; }}"
    );
    let _ = writeln!(&mut out, "    let mut lo = 0usize;");
    let _ = writeln!(&mut out, "    let mut hi = HTML_WIDTH_OVERRIDES_PX.len();");
    let _ = writeln!(&mut out, "    while lo < hi {{");
    let _ = writeln!(&mut out, "        let mid = (lo + hi) / 2;");
    let _ = writeln!(
        &mut out,
        "        let (k_fs, k_text, w) = HTML_WIDTH_OVERRIDES_PX[mid];"
    );
    let _ = writeln!(&mut out, "        match k_fs.cmp(&fs) {{");
    let _ = writeln!(&mut out, "            std::cmp::Ordering::Equal => {{");
    let _ = writeln!(&mut out, "                match k_text.cmp(text) {{");
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Equal => return Some(w),"
    );
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Less => lo = mid + 1,"
    );
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Greater => hi = mid,"
    );
    let _ = writeln!(&mut out, "                }}");
    let _ = writeln!(&mut out, "            }}");
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Less => lo = mid + 1,"
    );
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Greater => hi = mid,"
    );
    let _ = writeln!(&mut out, "        }}");
    let _ = writeln!(&mut out, "    }}");
    let _ = writeln!(&mut out, "    None");
    let _ = writeln!(&mut out, "}}\n");

    let _ = writeln!(
        &mut out,
        "pub fn lookup_calc_text_width_px(font_size: f64, text: &str) -> Option<i64> {{"
    );
    let _ = writeln!(&mut out, "    let fs = font_size_key(font_size);");
    let _ = writeln!(
        &mut out,
        "    if fs == 0 || text.is_empty() {{ return None; }}"
    );
    let _ = writeln!(&mut out, "    let mut lo = 0usize;");
    let _ = writeln!(
        &mut out,
        "    let mut hi = CALC_TEXT_WIDTH_OVERRIDES_PX.len();"
    );
    let _ = writeln!(&mut out, "    while lo < hi {{");
    let _ = writeln!(&mut out, "        let mid = (lo + hi) / 2;");
    let _ = writeln!(
        &mut out,
        "        let (k_fs, k_text, w) = CALC_TEXT_WIDTH_OVERRIDES_PX[mid];"
    );
    let _ = writeln!(&mut out, "        match k_fs.cmp(&fs) {{");
    let _ = writeln!(&mut out, "            std::cmp::Ordering::Equal => {{");
    let _ = writeln!(&mut out, "                match k_text.cmp(text) {{");
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Equal => return Some(w),"
    );
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Less => lo = mid + 1,"
    );
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Greater => hi = mid,"
    );
    let _ = writeln!(&mut out, "                }}");
    let _ = writeln!(&mut out, "            }}");
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Less => lo = mid + 1,"
    );
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Greater => hi = mid,"
    );
    let _ = writeln!(&mut out, "        }}");
    let _ = writeln!(&mut out, "    }}");
    let _ = writeln!(&mut out, "    None");
    let _ = writeln!(&mut out, "}}");

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    std::fs::write(&out_path, out).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

pub(crate) fn gen_gantt_text_overrides(args: Vec<String>) -> Result<(), XtaskError> {
    use std::collections::{BTreeMap, BTreeSet};

    let workspace_root = crate::cmd::workspace_root();

    let mut in_dir: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

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
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let in_dir = in_dir.unwrap_or_else(|| {
        workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("gantt")
    });
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("crates")
            .join("merman-render")
            .join("src")
            .join("generated")
            .join("gantt_text_overrides_11_12_2.rs")
    });

    fn font_size_key(font_size: f64) -> u16 {
        if !(font_size.is_finite() && font_size > 0.0) {
            return 0;
        }
        let k = (font_size * 100.0).round();
        if !(k.is_finite() && k >= 0.0 && k <= (u16::MAX as f64)) {
            return 0;
        }
        k as u16
    }

    fn rust_f64(v: f64) -> String {
        let mut s = format!("{v}");
        if !s.contains('.') && !s.contains('e') && !s.contains('E') {
            s.push_str(".0");
        }
        s
    }

    let mut widths: BTreeMap<(u16, String), f64> = BTreeMap::new();
    let mut conflicts: BTreeSet<String> = BTreeSet::new();

    let mut svg_paths: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&in_dir).map_err(|e| {
        XtaskError::SvgCompareFailed(format!("failed to read dir {}: {}", in_dir.display(), e))
    })? {
        let entry = entry.map_err(|e| {
            XtaskError::SvgCompareFailed(format!(
                "failed to read dir entry {}: {}",
                in_dir.display(),
                e
            ))
        })?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|e| e.to_string_lossy().to_ascii_lowercase() == "svg")
        {
            svg_paths.push(path);
        }
    }
    svg_paths.sort();

    for path in svg_paths {
        let svg = std::fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        let doc = roxmltree::Document::parse(&svg).map_err(|e| {
            XtaskError::SvgCompareFailed(format!(
                "failed to parse upstream Gantt SVG {}: {}",
                path.display(),
                e
            ))
        })?;

        for node in doc.descendants().filter(|n| n.has_tag_name("text")) {
            let class = node.attribute("class").unwrap_or_default();
            if class.is_empty() {
                continue;
            }
            // Only capture the width hints that Mermaid emits on task labels:
            // `taskText ... width-<bboxWidth>`.
            if !class.split_whitespace().any(|t| t.starts_with("taskText")) {
                continue;
            }
            let Some(width_tok) = class.split_whitespace().find(|t| t.starts_with("width-")) else {
                continue;
            };
            let Some(width_str) = width_tok.strip_prefix("width-") else {
                continue;
            };
            let Ok(width_px) = width_str.parse::<f64>() else {
                continue;
            };
            if !(width_px.is_finite() && width_px >= 0.0) {
                continue;
            }

            let font_size = node
                .attribute("font-size")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0);
            let fs_key = font_size_key(font_size);
            if fs_key == 0 {
                continue;
            }

            let text = node.text().unwrap_or_default().trim_end().to_string();
            if text.is_empty() {
                continue;
            }

            let key = (fs_key, text);
            if let Some(prev) = widths.get(&key).copied() {
                if (prev - width_px).abs() > 1e-6 {
                    conflicts.insert(format!(
                        "gantt width conflict for font_size={} text={:?}: {} vs {} (file {})",
                        font_size,
                        key.1,
                        rust_f64(prev),
                        rust_f64(width_px),
                        path.display()
                    ));
                }
            } else {
                widths.insert(key, width_px);
            }
        }
    }

    if !conflicts.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "conflicts while generating Gantt text overrides:\n{}",
            conflicts.into_iter().collect::<Vec<_>>().join("\n")
        )));
    }

    let entries: Vec<(u16, String, f64)> =
        widths.into_iter().map(|((fs, t), w)| (fs, t, w)).collect();

    let mut out = String::new();
    let _ = writeln!(
        &mut out,
        "// This file is generated by `xtask gen-gantt-text-overrides`.\n//\n// Mermaid baseline: 11.12.3\n// Source: fixtures/upstream-svgs/gantt/*.svg\n"
    );
    let _ = writeln!(&mut out, "fn font_size_key(font_size: f64) -> u16 {{");
    let _ = writeln!(
        &mut out,
        "    if !(font_size.is_finite() && font_size > 0.0) {{ return 0; }}"
    );
    let _ = writeln!(&mut out, "    let k = (font_size * 100.0).round();");
    let _ = writeln!(
        &mut out,
        "    if !(k.is_finite() && k >= 0.0 && k <= (u16::MAX as f64)) {{ return 0; }}"
    );
    let _ = writeln!(&mut out, "    k as u16");
    let _ = writeln!(&mut out, "}}");
    let _ = writeln!(&mut out);

    let _ = writeln!(
        &mut out,
        "static TASK_TEXT_BBOX_WIDTH_OVERRIDES_PX: &[(u16, &str, f64)] = &["
    );
    for (fs, t, w) in &entries {
        let _ = writeln!(&mut out, "    ({fs}, {:?}, {}),", t, rust_f64(*w));
    }
    let _ = writeln!(&mut out, "];\n");

    let _ = writeln!(
        &mut out,
        "pub fn lookup_task_text_bbox_width_px(font_size: f64, text: &str) -> Option<f64> {{"
    );
    let _ = writeln!(&mut out, "    let fs = font_size_key(font_size);");
    let _ = writeln!(
        &mut out,
        "    if fs == 0 || text.is_empty() {{ return None; }}"
    );
    let _ = writeln!(&mut out, "    let mut lo = 0usize;");
    let _ = writeln!(
        &mut out,
        "    let mut hi = TASK_TEXT_BBOX_WIDTH_OVERRIDES_PX.len();"
    );
    let _ = writeln!(&mut out, "    while lo < hi {{");
    let _ = writeln!(&mut out, "        let mid = (lo + hi) / 2;");
    let _ = writeln!(
        &mut out,
        "        let (k_fs, k_text, w) = TASK_TEXT_BBOX_WIDTH_OVERRIDES_PX[mid];"
    );
    let _ = writeln!(&mut out, "        match k_fs.cmp(&fs) {{");
    let _ = writeln!(&mut out, "            std::cmp::Ordering::Equal => {{");
    let _ = writeln!(&mut out, "                match k_text.cmp(text) {{");
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Equal => return Some(w),"
    );
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Less => lo = mid + 1,"
    );
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Greater => hi = mid,"
    );
    let _ = writeln!(&mut out, "                }}");
    let _ = writeln!(&mut out, "            }}");
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Less => lo = mid + 1,"
    );
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Greater => hi = mid,"
    );
    let _ = writeln!(&mut out, "        }}");
    let _ = writeln!(&mut out, "    }}");
    let _ = writeln!(&mut out, "    None");
    let _ = writeln!(&mut out, "}}");

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    std::fs::write(&out_path, out).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}
