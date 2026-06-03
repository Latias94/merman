//! C4-specific text width overrides derived from browser SVG measurement.

use crate::XtaskError;
use crate::util::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct C4TextStyleKey {
    font_key: String,
    font_size_key: usize,
    font_weight: String,
}

#[derive(Debug, Clone)]
struct C4TextSample {
    style: C4TextStyleKey,
    font_family_raw: String,
    text: String,
}

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

fn parse_style_prop(style: &str, key: &str) -> Option<String> {
    style
        .split(';')
        .filter_map(|decl| decl.split_once(':'))
        .find_map(|(k, v)| {
            if k.trim().eq_ignore_ascii_case(key) {
                let value = v.trim();
                if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                }
            } else {
                None
            }
        })
}

fn node_is_inside_defs(node: roxmltree::Node<'_, '_>) -> bool {
    node.ancestors()
        .filter(|n| n.is_element())
        .any(|n| n.has_tag_name("defs"))
}

fn collect_c4_text_samples(svg: &str) -> Vec<C4TextSample> {
    let Ok(doc) = roxmltree::Document::parse(svg) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for text_node in doc.descendants().filter(|n| n.has_tag_name("text")) {
        if node_is_inside_defs(text_node) {
            continue;
        }
        if text_node.attribute("textLength").is_some()
            || text_node.attribute("lengthAdjust").is_some()
        {
            // C4 type lines use fixed `textLength` pins rather than generic browser measurement.
            continue;
        }

        let style_attr = text_node.attribute("style").unwrap_or_default();
        let font_family = parse_style_prop(style_attr, "font-family")
            .or_else(|| text_node.attribute("font-family").map(str::to_string));
        let Some(font_family_raw) = font_family else {
            continue;
        };

        let font_size = parse_style_prop(style_attr, "font-size")
            .or_else(|| text_node.attribute("font-size").map(str::to_string))
            .and_then(|v| v.trim_end_matches("px").parse::<f64>().ok());
        let Some(font_size_px) = font_size.filter(|v| v.is_finite() && *v > 0.0) else {
            continue;
        };

        let font_weight = parse_style_prop(style_attr, "font-weight")
            .or_else(|| text_node.attribute("font-weight").map(str::to_string))
            .unwrap_or_else(|| "normal".to_string())
            .trim()
            .to_ascii_lowercase();

        let text = text_node
            .descendants()
            .filter(|n| n.is_text())
            .filter_map(|n| n.text())
            .collect::<String>()
            .trim()
            .to_string();
        if text.is_empty() {
            continue;
        }

        out.push(C4TextSample {
            style: C4TextStyleKey {
                font_key: normalize_font_key(&font_family_raw),
                font_size_key: (font_size_px * 1000.0).round().max(1.0) as usize,
                font_weight,
            },
            font_family_raw,
            text,
        });
    }

    out
}

fn measure_svg_text_bbox_widths_via_browser(
    node_cwd: &Path,
    browser_exe: &Path,
    font_family: &str,
    font_size_px: f64,
    font_weight: &str,
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
        "font_weight": font_weight,
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
const fontWeight = input.font_weight;
const strings = input.strings;

(async () => {
  const browser = await puppeteer.launch({
    headless: 'shell',
    executablePath: browserExe,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  const page = await browser.newPage();
  await page.setViewport({ width: 1200, height: 800, deviceScaleFactor: 1 });
  await page.setContent(`<!doctype html><html><head><style>body{margin:0;padding:0;}</style></head><body></body></html>`);

  const widths = await page.evaluate(({ strings, fontFamily, fontSizePx, fontWeight }) => {
    const SVG_NS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('width', '4000');
    svg.setAttribute('height', '400');
    document.body.appendChild(svg);

    function measureWithFont(text, family) {
      const node = document.createElementNS(SVG_NS, 'text');
      node.setAttribute('x', '0');
      node.setAttribute('y', '0');
      node.style.setProperty('font-size', `${fontSizePx}px`);
      node.style.setProperty('font-weight', String(fontWeight || 'normal'));
      node.style.setProperty('font-family', String(family || ''));
      node.textContent = String(text || '');
      svg.appendChild(node);
      const bbox = node.getBBox();
      svg.removeChild(node);
      return bbox.width;
    }

    return strings.map((text) => {
      const sans = measureWithFont(text, 'sans-serif');
      const requested = measureWithFont(text, fontFamily);
      return Math.max(sans, requested);
    });
  }, { strings, fontFamily, fontSizePx, fontWeight });

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
    let output = child
        .wait_with_output()
        .map_err(|source| XtaskError::SvgCompareFailed(format!("failed to run node: {source}")))?;
    if !output.status.success() {
        return Err(XtaskError::SvgCompareFailed(
            "browser c4 measurement failed".to_string(),
        ));
    }

    let widths_px: Vec<f64> = serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
    Ok(widths_px
        .into_iter()
        .map(|w| if w.is_finite() && w >= 0.0 { w } else { 0.0 })
        .collect())
}

pub(crate) fn gen_c4_text_overrides(args: Vec<String>) -> Result<(), XtaskError> {
    let mut in_dir: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut browser_exe: Option<PathBuf> = None;

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
    let browser_exe = browser_exe.ok_or(XtaskError::Usage)?;

    let mut samples = Vec::new();
    let entries = fs::read_dir(&in_dir).map_err(|source| XtaskError::ReadFile {
        path: in_dir.display().to_string(),
        source,
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_file_with_extension(&path, "svg") {
            continue;
        }
        let svg = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        samples.extend(collect_c4_text_samples(&svg));
    }

    if samples.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no C4 text samples found under {}",
            in_dir.display()
        )));
    }

    let node_cwd = crate::cmd::mermaid_cli_root();
    let mut widths_by_style: BTreeMap<C4TextStyleKey, BTreeMap<String, f64>> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut grouped: BTreeMap<C4TextStyleKey, (String, Vec<String>)> = BTreeMap::new();
    for sample in samples {
        let dedupe_key = (
            sample.style.clone(),
            sample.font_family_raw.clone(),
            sample.text.clone(),
        );
        if !seen.insert(dedupe_key) {
            continue;
        }
        let entry = grouped
            .entry(sample.style.clone())
            .or_insert_with(|| (sample.font_family_raw.clone(), Vec::new()));
        entry.1.push(sample.text);
    }

    for (style, (font_family_raw, mut strings)) in grouped {
        strings.sort();
        strings.dedup();
        let widths = measure_svg_text_bbox_widths_via_browser(
            &node_cwd,
            &browser_exe,
            &font_family_raw,
            (style.font_size_key as f64) / 1000.0,
            &style.font_weight,
            &strings,
        )?;
        let by_text = widths_by_style.entry(style).or_default();
        for (text, width) in strings.into_iter().zip(widths.into_iter()) {
            by_text.insert(text, width);
        }
    }

    let mut out = String::new();
    fn rust_f64(v: f64) -> String {
        let mut s = format!("{v}");
        if !s.contains('.') && !s.contains('e') && !s.contains('E') {
            s.push_str(".0");
        }
        s
    }

    let _ = writeln!(
        &mut out,
        "// This file is generated by `xtask gen-c4-text-overrides`.\n"
    );
    let _ = writeln!(
        &mut out,
        "pub fn lookup_c4_text_width_px(font_key: &str, font_size_key: usize, font_weight: &str, text: &str) -> Option<f64> {{"
    );
    let _ = writeln!(
        &mut out,
        "    match (font_key, font_size_key, font_weight) {{"
    );
    for (style, entries) in &widths_by_style {
        let _ = writeln!(
            &mut out,
            "        ({:?}, {}, {:?}) => {{",
            style.font_key, style.font_size_key, style.font_weight
        );
        let _ = writeln!(&mut out, "            let tbl: &[(&str, f64)] = &[");
        for (text, width) in entries {
            let _ = writeln!(
                &mut out,
                "                ({text:?}, {}),",
                rust_f64(*width)
            );
        }
        let _ = writeln!(&mut out, "            ];");
        let _ = writeln!(&mut out, "            lookup_in(tbl, text)");
        let _ = writeln!(&mut out, "        }},");
    }
    let _ = writeln!(&mut out, "        _ => None,");
    let _ = writeln!(&mut out, "    }}");
    let _ = writeln!(&mut out, "}}\n");
    let _ = writeln!(
        &mut out,
        "fn lookup_in(tbl: &'static [(&'static str, f64)], text: &str) -> Option<f64> {{"
    );
    let _ = writeln!(&mut out, "    let mut lo = 0usize;");
    let _ = writeln!(&mut out, "    let mut hi = tbl.len();");
    let _ = writeln!(&mut out, "    while lo < hi {{");
    let _ = writeln!(&mut out, "        let mid = (lo + hi) / 2;");
    let _ = writeln!(&mut out, "        let (k, v) = tbl[mid];");
    let _ = writeln!(&mut out, "        match k.cmp(text) {{");
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Equal => return Some(v),"
    );
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
