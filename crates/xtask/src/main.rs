use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;
use roxmltree::Document;

#[derive(Debug, thiserror::Error)]
enum XtaskError {
    #[error("usage: xtask <command> ...")]
    Usage,
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("failed to read file {path}: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write file {path}: {source}")]
    WriteFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse YAML schema: {0}")]
    ParseYaml(#[from] serde_yaml::Error),
    #[error("failed to process JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid $ref: {0}")]
    InvalidRef(String),
    #[error("unresolved $ref: {0}")]
    UnresolvedRef(String),
    #[error("failed to parse dompurify dist file: {0}")]
    ParseDompurify(String),
    #[error("verification failed:\n{0}")]
    VerifyFailed(String),
    #[error("snapshot update failed: {0}")]
    SnapshotUpdateFailed(String),
    #[error("layout snapshot update failed: {0}")]
    LayoutSnapshotUpdateFailed(String),
    #[error("alignment check failed:\n{0}")]
    AlignmentCheckFailed(String),
    #[error("debug svg generation failed:\n{0}")]
    DebugSvgFailed(String),
    #[error("upstream svg generation failed:\n{0}")]
    UpstreamSvgFailed(String),
    #[error("svg compare failed:\n{0}")]
    SvgCompareFailed(String),
}

fn main() -> Result<(), XtaskError> {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else {
        return Err(XtaskError::Usage);
    };

    match cmd.as_str() {
        "gen-default-config" => gen_default_config(args.collect()),
        "gen-dompurify-defaults" => gen_dompurify_defaults(args.collect()),
        "verify-generated" => verify_generated(args.collect()),
        "update-snapshots" => update_snapshots(args.collect()),
        "update-layout-snapshots" | "gen-layout-goldens" => update_layout_snapshots(args.collect()),
        "check-alignment" => check_alignment(args.collect()),
        "gen-debug-svgs" => gen_debug_svgs(args.collect()),
        "gen-er-svgs" => gen_er_svgs(args.collect()),
        "gen-upstream-svgs" => gen_upstream_svgs(args.collect()),
        "compare-er-svgs" => compare_er_svgs(args.collect()),
        other => Err(XtaskError::UnknownCommand(other.to_string())),
    }
}

fn update_layout_snapshots(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut decimals: u32 = 3;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args
                    .get(i)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "all".to_string());
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--decimals" => {
                i += 1;
                decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    fn round_f64(v: f64, decimals: u32) -> f64 {
        let p = 10_f64.powi(decimals as i32);
        (v * p).round() / p
    }

    fn round_json_numbers(v: &mut JsonValue, decimals: u32) {
        match v {
            JsonValue::Number(n) => {
                let Some(f) = n.as_f64() else {
                    return;
                };
                let r = round_f64(f, decimals);
                if let Some(nn) = serde_json::Number::from_f64(r) {
                    *v = JsonValue::Number(nn);
                }
            }
            JsonValue::Array(arr) => {
                for item in arr {
                    round_json_numbers(item, decimals);
                }
            }
            JsonValue::Object(map) => {
                for (_k, val) in map.iter_mut() {
                    round_json_numbers(val, decimals);
                }
            }
            _ => {}
        }
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_root = workspace_root.join("fixtures");

    let mut mmd_files = Vec::new();
    let mut stack = vec![fixtures_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "upstream-svgs") {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "mmd") {
                if let Some(ref f) = filter {
                    if !path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.contains(f))
                    {
                        continue;
                    }
                }
                mmd_files.push(path);
            }
        }
    }
    mmd_files.sort();
    if mmd_files.is_empty() {
        return Err(XtaskError::LayoutSnapshotUpdateFailed(format!(
            "no .mmd fixtures found under {}",
            fixtures_root.display()
        )));
    }

    let engine = merman::Engine::new();
    let layout_opts = merman_render::LayoutOptions::default();
    let mut failures = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        if diagram != "all" {
            let dt = parsed.meta.diagram_type.as_str();
            let matches = dt == diagram
                || (diagram == "er" && matches!(dt, "er" | "erDiagram"))
                || (diagram == "flowchart" && dt == "flowchart-v2")
                || (diagram == "state" && dt == "stateDiagram")
                || (diagram == "class" && matches!(dt, "class" | "classDiagram"));
            if !matches {
                continue;
            }
        }

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(merman_render::Error::UnsupportedDiagram { .. }) => {
                // Layout snapshots are only defined for diagram types currently supported by
                // `merman-render::layout_parsed`. Skip unsupported diagrams so `--diagram all`
                // can be used for "all supported layout diagrams".
                continue;
            }
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let mut layout_json = match serde_json::to_value(&layouted.layout) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to serialize layout JSON for {}: {err}",
                    mmd_path.display()
                ));
                continue;
            }
        };
        round_json_numbers(&mut layout_json, decimals);

        let out = serde_json::json!({
            "diagramType": parsed.meta.diagram_type,
            "layout": layout_json,
        });

        let pretty = match serde_json::to_string_pretty(&out) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to pretty-print JSON for {}: {err}",
                    mmd_path.display()
                ));
                continue;
            }
        };

        let out_path = mmd_path.with_extension("layout.golden.json");
        if let Some(parent) = out_path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                failures.push(format!("failed to create dir {}: {err}", parent.display()));
                continue;
            }
        }
        if let Err(err) = fs::write(&out_path, format!("{pretty}\n")) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::LayoutSnapshotUpdateFailed(failures.join("\n")))
    }
}

fn compare_er_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_markers: bool = false;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "strict".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-markers" => check_markers = true,
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "strict".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("er");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("er");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("er_report.md")
    });

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !path.extension().is_some_and(|e| e == "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    let re_viewbox = Regex::new(r#"viewBox="([^"]+)""#).unwrap();
    let re_max_width = Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap();
    let re_marker_id = Regex::new(r#"<marker[^>]*\bid="([^"]+)""#).unwrap();
    let re_marker_ref = Regex::new(r#"marker-(?:start|end)="url\(#([^)]+)\)""#).unwrap();

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SvgDomNode {
        name: String,
        attrs: std::collections::BTreeMap<String, String>,
        text: Option<String>,
        children: Vec<SvgDomNode>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DomMode {
        Strict,
        Structure,
        Parity,
    }

    fn parse_dom_mode(s: &str) -> DomMode {
        match s {
            "structure" => DomMode::Structure,
            "parity" => DomMode::Parity,
            _ => DomMode::Strict,
        }
    }

    fn round_f64(v: f64, decimals: u32) -> f64 {
        let p = 10_f64.powi(decimals as i32);
        (v * p).round() / p
    }

    fn normalize_whitespace(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut last_was_ws = false;
        for ch in s.chars() {
            if ch.is_whitespace() {
                last_was_ws = true;
                continue;
            }
            if last_was_ws && !out.is_empty() {
                out.push(' ');
            }
            last_was_ws = false;
            out.push(ch);
        }
        out.trim().to_string()
    }

    fn normalize_class_list(s: &str) -> String {
        let mut parts: Vec<&str> = s.split_whitespace().collect();
        parts.sort_unstable();
        parts.dedup();
        parts.join(" ")
    }

    fn normalize_css_value(s: &str) -> String {
        // Keep semantics while reducing whitespace noise.
        normalize_whitespace(&s.replace('\n', " "))
    }

    fn re_num() -> &'static Regex {
        // -12, 12.34, .5, 1e-3, -2.0E+4
        static ONCE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        ONCE.get_or_init(|| Regex::new(r"-?(?:\d+\.\d+|\d+\.|\.\d+|\d+)(?:[eE][+-]?\d+)?").unwrap())
    }

    fn re_css_max_width() -> &'static Regex {
        static ONCE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        ONCE.get_or_init(|| Regex::new(r"(?i)max-width\s*:\s*[0-9.]+px").unwrap())
    }

    fn normalize_numeric_tokens(s: &str, decimals: u32) -> String {
        // Pragmatic DOM compare: ignore float formatting differences by rounding all numeric tokens.
        re_num()
            .replace_all(s, |caps: &regex::Captures<'_>| {
                let raw = caps.get(0).map(|m| m.as_str()).unwrap_or_default();
                let Ok(v) = raw.parse::<f64>() else {
                    return raw.to_string();
                };
                let r = round_f64(v, decimals);
                let r = if r == 0.0 { 0.0 } else { r };
                let mut out = format!("{r}");
                if out.contains('.') {
                    while out.ends_with('0') {
                        out.pop();
                    }
                    if out.ends_with('.') {
                        out.pop();
                    }
                }
                out
            })
            .to_string()
    }

    fn normalize_numeric_tokens_mode(s: &str, decimals: u32, mode: DomMode) -> String {
        match mode {
            DomMode::Strict => normalize_numeric_tokens(s, decimals),
            DomMode::Structure => re_num().replace_all(s, "<n>").to_string(),
            DomMode::Parity => normalize_numeric_tokens(s, decimals),
        }
    }

    fn strip_css_property(style: &str, key: &str) -> String {
        // Very small, pragmatic parser for `style="k: v; k2: v2;"` used on the root `<svg>`.
        let mut out: Vec<String> = Vec::new();
        for part in style.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let Some((k, v)) = part.split_once(':') else {
                continue;
            };
            if k.trim().eq_ignore_ascii_case(key) {
                continue;
            }
            out.push(format!("{}: {}", k.trim(), v.trim()));
        }
        if out.is_empty() {
            String::new()
        } else {
            format!("{};", out.join("; "))
        }
    }

    fn is_geometry_attr(name: &str) -> bool {
        matches!(
            name,
            "viewBox"
                | "transform"
                | "d"
                | "points"
                | "x"
                | "y"
                | "x1"
                | "y1"
                | "x2"
                | "y2"
                | "cx"
                | "cy"
                | "r"
                | "rx"
                | "ry"
                | "width"
                | "height"
        )
    }

    fn normalize_svg_attr(
        element_name: &str,
        name: &str,
        value: &str,
        decimals: u32,
        mode: DomMode,
    ) -> String {
        match name {
            "data-points" if mode == DomMode::Structure || mode == DomMode::Parity => {
                "<data-points>".to_string()
            }
            "class" => normalize_class_list(value),
            "style" => {
                let v = if mode == DomMode::Parity && element_name == "svg" {
                    strip_css_property(value, "max-width")
                } else {
                    value.to_string()
                };
                match mode {
                    DomMode::Parity => {
                        // Headless rendering does not currently match upstream max-width heuristics for
                        // HTML labels. Treat `max-width: <n>px` as geometry noise for parity checks.
                        let v = if element_name == "div" {
                            re_css_max_width()
                                .replace_all(&v, "max-width: <n>px")
                                .to_string()
                        } else {
                            v
                        };
                        normalize_css_value(&normalize_numeric_tokens(&v, decimals))
                    }
                    DomMode::Strict | DomMode::Structure => {
                        normalize_css_value(&normalize_numeric_tokens_mode(&v, decimals, mode))
                    }
                }
            }
            "viewBox" => {
                if mode == DomMode::Parity {
                    normalize_whitespace(&normalize_numeric_tokens_mode(
                        value,
                        decimals,
                        DomMode::Structure,
                    ))
                } else {
                    normalize_whitespace(&normalize_numeric_tokens_mode(value, decimals, mode))
                }
            }
            "transform" => {
                if mode == DomMode::Parity {
                    normalize_whitespace(&normalize_numeric_tokens_mode(
                        value,
                        decimals,
                        DomMode::Structure,
                    ))
                } else {
                    normalize_whitespace(&normalize_numeric_tokens_mode(value, decimals, mode))
                }
            }
            "d" => {
                let v = value.replace(',', " ");
                let m = if mode == DomMode::Parity {
                    DomMode::Structure
                } else {
                    mode
                };
                normalize_numeric_tokens_mode(&v, decimals, m)
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect()
            }
            "points" => {
                let v = value.replace(',', " ");
                let m = if mode == DomMode::Parity {
                    DomMode::Structure
                } else {
                    mode
                };
                normalize_numeric_tokens_mode(&v, decimals, m)
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect()
            }
            "x" | "y" | "x1" | "y1" | "x2" | "y2" | "cx" | "cy" | "r" | "rx" | "ry" | "width"
            | "height" | "stroke-width" | "font-size" | "opacity" => {
                if mode == DomMode::Parity && is_geometry_attr(name) {
                    normalize_whitespace(&normalize_numeric_tokens_mode(
                        value,
                        decimals,
                        DomMode::Structure,
                    ))
                } else {
                    normalize_whitespace(&normalize_numeric_tokens_mode(value, decimals, mode))
                }
            }
            _ => normalize_whitespace(value),
        }
    }

    fn dom_node_from_xml(
        node: roxmltree::Node<'_, '_>,
        decimals: u32,
        mode: DomMode,
    ) -> Option<SvgDomNode> {
        if !node.is_element() {
            return None;
        }
        let name = node.tag_name().name().to_string();

        let mut attrs: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();
        for a in node.attributes() {
            let key = a.name().to_string();
            let val = normalize_svg_attr(&name, &key, a.value(), decimals, mode);
            attrs.insert(key, val);
        }

        if (mode == DomMode::Structure || mode == DomMode::Parity) && name == "style" {
            return Some(SvgDomNode {
                name,
                attrs,
                text: None,
                children: Vec::new(),
            });
        }

        let mut text: Option<String> = None;
        for child in node.children() {
            if child.is_text() {
                if let Some(t) = child.text() {
                    let t = normalize_whitespace(t);
                    if !t.is_empty() {
                        text = Some(t);
                        break;
                    }
                }
            }
        }

        let mut children: Vec<SvgDomNode> = Vec::new();
        for child in node.children() {
            if let Some(c) = dom_node_from_xml(child, decimals, mode) {
                children.push(c);
            }
        }

        Some(SvgDomNode {
            name,
            attrs,
            text,
            children,
        })
    }

    fn svg_dom_signature(svg: &str, decimals: u32, mode: DomMode) -> Result<SvgDomNode, String> {
        let doc = Document::parse(svg).map_err(|e| format!("xml parse failed: {e}"))?;
        let root = doc.root_element();
        dom_node_from_xml(root, decimals, mode).ok_or_else(|| "missing root element".to_string())
    }

    fn truncate(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            return s.to_string();
        }
        let mut out = s
            .chars()
            .take(max_len.saturating_sub(1))
            .collect::<String>();
        out.push('…');
        out
    }

    fn dom_diff_path(
        upstream: &SvgDomNode,
        local: &SvgDomNode,
        path: &mut Vec<String>,
    ) -> Option<String> {
        if upstream.name != local.name {
            return Some(format!(
                "{}: element name mismatch upstream={} local={}",
                path.join("/"),
                upstream.name,
                local.name
            ));
        }

        if upstream.attrs != local.attrs {
            for (k, v_up) in &upstream.attrs {
                match local.attrs.get(k) {
                    None => return Some(format!("{}: missing attr `{k}`", path.join("/"))),
                    Some(v_lo) if v_lo != v_up => {
                        return Some(format!(
                            "{}: attr `{k}` mismatch upstream=`{}` local=`{}`",
                            path.join("/"),
                            truncate(v_up, 120),
                            truncate(v_lo, 120)
                        ));
                    }
                    _ => {}
                }
            }
            for k in local.attrs.keys() {
                if !upstream.attrs.contains_key(k) {
                    return Some(format!("{}: extra attr `{k}`", path.join("/")));
                }
            }
            return Some(format!("{}: attrs mismatch", path.join("/")));
        }

        if upstream.text != local.text {
            return Some(format!(
                "{}: text mismatch upstream=`{}` local=`{}`",
                path.join("/"),
                truncate(upstream.text.as_deref().unwrap_or(""), 120),
                truncate(local.text.as_deref().unwrap_or(""), 120)
            ));
        }

        if upstream.children.len() != local.children.len() {
            return Some(format!(
                "{}: children count mismatch upstream={} local={}",
                path.join("/"),
                upstream.children.len(),
                local.children.len()
            ));
        }

        for (idx, (cu, cl)) in upstream
            .children
            .iter()
            .zip(local.children.iter())
            .enumerate()
        {
            path.push(format!("{}[{idx}]", cu.name));
            let diff = dom_diff_path(cu, cl, path);
            path.pop();
            if diff.is_some() {
                return diff;
            }
        }

        None
    }

    #[derive(Default)]
    struct SvgSig {
        view_box: Option<String>,
        max_width_px: Option<String>,
        marker_ids: std::collections::BTreeSet<String>,
        marker_refs: std::collections::BTreeSet<String>,
    }

    fn sig_for_svg(
        svg: &str,
        re_viewbox: &Regex,
        re_max_width: &Regex,
        re_marker_id: &Regex,
        re_marker_ref: &Regex,
    ) -> SvgSig {
        let mut sig = SvgSig::default();
        sig.view_box = re_viewbox
            .captures(svg)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string());
        sig.max_width_px = re_max_width
            .captures(svg)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string());
        for cap in re_marker_id.captures_iter(svg) {
            if let Some(m) = cap.get(1) {
                sig.marker_ids.insert(m.as_str().to_string());
            }
        }
        for cap in re_marker_ref.captures_iter(svg) {
            if let Some(m) = cap.get(1) {
                sig.marker_refs.insert(m.as_str().to_string());
            }
        }
        sig
    }

    let engine = merman::Engine::new();
    let layout_opts = merman_render::LayoutOptions::default();

    let mut report = String::new();
    let _ = writeln!(&mut report, "# ER SVG Compare Report");
    let _ = writeln!(&mut report, "");
    let _ = writeln!(
        &mut report,
        "- Upstream: `fixtures/upstream-svgs/er/*.svg` (Mermaid CLI pinned to Mermaid 11.12.2)"
    );
    let _ = writeln!(&mut report, "- Local: `render_er_diagram_svg` (Stage B)");
    let _ = writeln!(&mut report, "");
    let _ = writeln!(
        &mut report,
        "| fixture | markers ok | dom ok | viewBox (upstream) | viewBox (local) | max-width (upstream) | max-width (local) |"
    );
    let _ = writeln!(&mut report, "|---|---:|---:|---|---|---:|---:|");

    let mut failures: Vec<String> = Vec::new();
    let mut dom_failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {}: {} ({err})",
                    mmd_path.display(),
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::ErDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_er_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let upstream_sig = sig_for_svg(
            &upstream_svg,
            &re_viewbox,
            &re_max_width,
            &re_marker_id,
            &re_marker_ref,
        );
        let local_sig = sig_for_svg(
            &local_svg,
            &re_viewbox,
            &re_max_width,
            &re_marker_id,
            &re_marker_ref,
        );

        let mut marker_ok = true;
        let mut missing: Vec<String> = Vec::new();
        let mut extra: Vec<String> = Vec::new();
        for m in &upstream_sig.marker_ids {
            if !local_sig.marker_ids.contains(m) {
                marker_ok = false;
                missing.push(m.clone());
            }
        }
        for m in &local_sig.marker_ids {
            if !upstream_sig.marker_ids.contains(m) {
                marker_ok = false;
                extra.push(m.clone());
            }
        }
        for r in &local_sig.marker_refs {
            if !local_sig.marker_ids.contains(r) {
                marker_ok = false;
                extra.push(format!("ref-missing-def:{r}"));
            }
        }

        if check_markers && !marker_ok {
            failures.push(format!(
                "marker mismatch for {stem}: missing={:?} extra={:?}",
                missing, extra
            ));
        }

        let mut dom_ok = true;
        let dom_ok_str = if check_dom {
            let dom_mode_parsed = parse_dom_mode(dom_mode.as_str());
            let upstream_dom = match svg_dom_signature(&upstream_svg, dom_decimals, dom_mode_parsed)
            {
                Ok(v) => v,
                Err(err) => {
                    dom_ok = false;
                    dom_failures.push(format!("dom parse failed (upstream) for {stem}: {err}"));
                    SvgDomNode {
                        name: "<parse-failed>".to_string(),
                        attrs: Default::default(),
                        text: None,
                        children: vec![],
                    }
                }
            };
            let local_dom = match svg_dom_signature(&local_svg, dom_decimals, dom_mode_parsed) {
                Ok(v) => v,
                Err(err) => {
                    dom_ok = false;
                    dom_failures.push(format!("dom parse failed (local) for {stem}: {err}"));
                    SvgDomNode {
                        name: "<parse-failed>".to_string(),
                        attrs: Default::default(),
                        text: None,
                        children: vec![],
                    }
                }
            };

            if dom_ok {
                let mut path = vec!["svg".to_string()];
                if let Some(diff) = dom_diff_path(&upstream_dom, &local_dom, &mut path) {
                    dom_ok = false;
                    dom_failures.push(format!("{stem}: {diff}"));
                }
            }

            if !dom_ok {
                failures.push(format!(
                    "dom mismatch for {stem} (mode={dom_mode}, decimals={dom_decimals})"
                ));
            }

            if dom_ok { "yes" } else { "no" }
        } else {
            "-"
        };

        let _ = writeln!(
            &mut report,
            "| `{}` | {} | {} | `{}` | `{}` | `{}` | `{}` |",
            stem,
            if marker_ok { "yes" } else { "no" },
            dom_ok_str,
            upstream_sig
                .view_box
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            local_sig
                .view_box
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            upstream_sig
                .max_width_px
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            local_sig
                .max_width_px
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        );
    }

    if check_dom && !dom_failures.is_empty() {
        let _ = writeln!(&mut report, "");
        let _ = writeln!(&mut report, "## DOM Mismatch Details");
        let _ = writeln!(&mut report, "");
        for f in &dom_failures {
            let _ = writeln!(&mut report, "- {f}");
        }
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::SvgCompareFailed(failures.join("\n")))
}

fn gen_upstream_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "er".to_string();
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut install: bool = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--install" => install = true,
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root =
        out_root.unwrap_or_else(|| workspace_root.join("fixtures").join("upstream-svgs"));

    let tools_root = workspace_root.join("tools").join("mermaid-cli");
    let node_modules = tools_root.join("node_modules");
    if install || !node_modules.exists() {
        let npm_cmd = if tools_root.join("package-lock.json").is_file() {
            "ci"
        } else {
            "install"
        };
        let status = Command::new("npm")
            .arg(npm_cmd)
            .current_dir(&tools_root)
            .status()
            .map_err(|err| {
                XtaskError::UpstreamSvgFailed(format!(
                    "failed to run `npm {npm_cmd}` in {}: {err}",
                    tools_root.display()
                ))
            })?;
        if !status.success() {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "npm {npm_cmd} failed in {}",
                tools_root.display()
            )));
        }
    }

    let mmdc = find_mmdc(&tools_root).ok_or_else(|| {
        XtaskError::UpstreamSvgFailed(format!(
            "mmdc not found under {} (run: npm install)",
            tools_root.display()
        ))
    })?;

    fn run_one(
        workspace_root: &Path,
        out_root: &Path,
        mmdc: &Path,
        diagram: &str,
        filter: Option<&str>,
    ) -> Result<(), XtaskError> {
        let fixtures_dir = workspace_root.join("fixtures").join(diagram);
        let out_dir = out_root.join(diagram);

        let mut mmd_files: Vec<PathBuf> = Vec::new();
        let Ok(entries) = fs::read_dir(&fixtures_dir) else {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "failed to list fixtures directory {}",
                fixtures_dir.display()
            )));
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if !path.extension().is_some_and(|e| e == "mmd") {
                continue;
            }
            if let Some(f) = filter {
                if !path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains(f))
                {
                    continue;
                }
            }
            mmd_files.push(path);
        }
        mmd_files.sort();

        if mmd_files.is_empty() {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "no .mmd fixtures matched under {}",
                fixtures_dir.display()
            )));
        }

        fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
            path: out_dir.display().to_string(),
            source,
        })?;

        let mut failures: Vec<String> = Vec::new();

        for mmd_path in mmd_files {
            let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
                failures.push(format!("invalid fixture filename {}", mmd_path.display()));
                continue;
            };
            let out_path = out_dir.join(format!("{stem}.svg"));

            let status = Command::new(mmdc)
                .arg("-i")
                .arg(&mmd_path)
                .arg("-o")
                .arg(&out_path)
                .arg("-t")
                .arg("default")
                .arg("--svgId")
                .arg(stem)
                .status();

            match status {
                Ok(s) if s.success() => {}
                Ok(s) => failures.push(format!(
                    "mmdc failed for {} (exit={})",
                    mmd_path.display(),
                    s.code().unwrap_or(-1)
                )),
                Err(err) => failures.push(format!("mmdc failed for {}: {err}", mmd_path.display())),
            }
        }

        if failures.is_empty() {
            return Ok(());
        }

        let failures_path = out_dir.join("_failures.txt");
        let _ = fs::write(&failures_path, failures.join("\n"));

        Err(XtaskError::UpstreamSvgFailed(failures.join("\n")))
    }

    let filter = filter.as_deref();
    match diagram.as_str() {
        "all" => {
            let mut failures: Vec<String> = Vec::new();
            for d in ["er", "flowchart", "state", "class"] {
                if let Err(err) = run_one(&workspace_root, &out_root, &mmdc, d, filter) {
                    failures.push(format!("{d}: {err}"));
                }
            }
            if failures.is_empty() {
                Ok(())
            } else {
                Err(XtaskError::UpstreamSvgFailed(failures.join("\n")))
            }
        }
        "er" | "flowchart" | "state" | "class" => {
            run_one(&workspace_root, &out_root, &mmdc, &diagram, filter)
        }
        other => Err(XtaskError::UpstreamSvgFailed(format!(
            "unsupported diagram for upstream svg export: {other} (supported: er, flowchart, state, class, all)"
        ))),
    }
}

fn find_mmdc(tools_root: &Path) -> Option<PathBuf> {
    let bin_root = tools_root.join("node_modules").join(".bin");
    for name in ["mmdc.cmd", "mmdc.ps1", "mmdc"] {
        let p = bin_root.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn gen_er_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root = out_root.unwrap_or_else(|| workspace_root.join("target").join("svgs"));

    let fixtures_dir = workspace_root.join("fixtures").join("er");
    let out_dir = out_root.join("er");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !path.extension().is_some_and(|e| e == "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new();
    let mut failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = merman_render::LayoutOptions::default();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::ErDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let svg = match merman_render::svg::render_er_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let out_path = out_dir.join(format!("{stem}.svg"));
        if let Err(err) = fs::write(&out_path, svg) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

fn gen_debug_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "class".to_string();
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root = out_root.unwrap_or_else(|| workspace_root.join("target").join("debug-svgs"));

    fn gen_one(
        workspace_root: &Path,
        out_root: &Path,
        diagram: &str,
        filter: Option<&str>,
    ) -> Result<(), XtaskError> {
        let (fixtures_dir, out_dir) = match diagram {
            "flowchart" | "flowchart-v2" | "flowchartV2" => (
                workspace_root.join("fixtures").join("flowchart"),
                out_root.join("flowchart"),
            ),
            "state" | "stateDiagram" | "stateDiagram-v2" | "stateDiagramV2" => (
                workspace_root.join("fixtures").join("state"),
                out_root.join("state"),
            ),
            "class" | "classDiagram" => (
                workspace_root.join("fixtures").join("class"),
                out_root.join("class"),
            ),
            "er" | "erDiagram" => (
                workspace_root.join("fixtures").join("er"),
                out_root.join("er"),
            ),
            other => {
                return Err(XtaskError::DebugSvgFailed(format!(
                    "unsupported diagram for debug svg export: {other} (supported: flowchart, state, class, er)"
                )));
            }
        };

        let mut mmd_files: Vec<PathBuf> = Vec::new();
        let Ok(entries) = fs::read_dir(&fixtures_dir) else {
            return Err(XtaskError::DebugSvgFailed(format!(
                "failed to list fixtures directory {}",
                fixtures_dir.display()
            )));
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if !path.extension().is_some_and(|e| e == "mmd") {
                continue;
            }
            if let Some(f) = filter {
                if !path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains(f))
                {
                    continue;
                }
            }
            mmd_files.push(path);
        }
        mmd_files.sort();

        if mmd_files.is_empty() {
            return Err(XtaskError::DebugSvgFailed(format!(
                "no .mmd fixtures matched under {}",
                fixtures_dir.display()
            )));
        }

        fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
            path: out_dir.display().to_string(),
            source,
        })?;

        let engine = merman::Engine::new();
        let mut failures: Vec<String> = Vec::new();

        for mmd_path in mmd_files {
            let text = match fs::read_to_string(&mmd_path) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                    continue;
                }
            };

            let parsed = match futures::executor::block_on(
                engine.parse_diagram(&text, merman::ParseOptions::default()),
            ) {
                Ok(Some(v)) => v,
                Ok(None) => {
                    failures.push(format!("no diagram detected in {}", mmd_path.display()));
                    continue;
                }
                Err(err) => {
                    failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                    continue;
                }
            };

            let layouted = match merman_render::layout_parsed(
                &parsed,
                &merman_render::LayoutOptions::default(),
            ) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                    continue;
                }
            };

            let svg = match &layouted.layout {
                merman_render::model::LayoutDiagram::FlowchartV2(layout) => {
                    merman_render::svg::render_flowchart_v2_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                }
                merman_render::model::LayoutDiagram::StateDiagramV2(layout) => {
                    merman_render::svg::render_state_diagram_v2_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                }
                merman_render::model::LayoutDiagram::ClassDiagramV2(layout) => {
                    merman_render::svg::render_class_diagram_v2_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                }
                merman_render::model::LayoutDiagram::ErDiagram(layout) => {
                    merman_render::svg::render_er_diagram_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                }
            };

            let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
                failures.push(format!("invalid fixture filename {}", mmd_path.display()));
                continue;
            };
            let out_path = out_dir.join(format!("{stem}.svg"));
            if let Err(err) = fs::write(&out_path, svg) {
                failures.push(format!("failed to write {}: {err}", out_path.display()));
                continue;
            }
        }

        if failures.is_empty() {
            return Ok(());
        }

        Err(XtaskError::DebugSvgFailed(failures.join("\n")))
    }

    let filter = filter.as_deref();
    let diagrams: Vec<&str> = match diagram.as_str() {
        "all" => vec!["flowchart", "state", "class", "er"],
        other => vec![other],
    };

    let mut failures: Vec<String> = Vec::new();
    for d in diagrams {
        if let Err(err) = gen_one(&workspace_root, &out_root, d, filter) {
            failures.push(format!("{d}: {err}"));
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

fn check_alignment(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() && !(args.len() == 1 && (args[0] == "--help" || args[0] == "-h")) {
        return Err(XtaskError::Usage);
    }
    if args.len() == 1 {
        return Err(XtaskError::Usage);
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let alignment_dir = workspace_root.join("docs").join("alignment");
    let fixtures_root = workspace_root.join("fixtures");

    let mut failures: Vec<String> = Vec::new();

    // 1) Every *_MINIMUM.md should have a *_UPSTREAM_TEST_COVERAGE.md sibling.
    let mut minimum_docs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&alignment_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.ends_with("_MINIMUM.md") {
                minimum_docs.push(path);
            }
        }
    }
    minimum_docs.sort();
    for min_path in &minimum_docs {
        let Some(stem) = min_path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix("_MINIMUM.md"))
        else {
            continue;
        };
        let cov = alignment_dir.join(format!("{stem}_UPSTREAM_TEST_COVERAGE.md"));
        if !cov.exists() {
            failures.push(format!(
                "missing upstream coverage doc for {stem}: expected {}",
                cov.display()
            ));
        }
    }

    fn strip_reference_suffix(s: &str) -> &str {
        // Normalize "path:line" and "path#Lline" forms to just "path" for existence checks.
        if let Some((left, right)) = s.rsplit_once(':') {
            if right.chars().all(|c| c.is_ascii_digit()) {
                return left;
            }
        }
        if let Some((left, right)) = s.rsplit_once("#L") {
            if right.chars().all(|c| c.is_ascii_digit()) {
                return left;
            }
        }
        s
    }

    fn is_probably_relative_path(s: &str) -> bool {
        s.starts_with("fixtures/")
            || s.starts_with("docs/")
            || s.starts_with("crates/")
            || s.starts_with("repo-ref/")
    }

    fn contains_glob(s: &str) -> bool {
        s.contains('*') || s.contains('?') || s.contains('[') || s.contains(']')
    }

    // 2) Every `fixtures/**/*.mmd` must have a sibling `.golden.json`.
    let mut mmd_files = Vec::new();
    let mut stack = vec![fixtures_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "mmd") {
                mmd_files.push(path);
            }
        }
    }
    mmd_files.sort();
    for mmd in &mmd_files {
        let golden = mmd.with_extension("golden.json");
        if !golden.exists() {
            failures.push(format!(
                "missing golden snapshot for fixture {} (expected {})",
                mmd.display(),
                golden.display()
            ));
        }
    }

    // 3) Coverage docs should not reference non-existent local files.
    let backtick_re = Regex::new(r"`([^`]+)`")
        .map_err(|e| XtaskError::AlignmentCheckFailed(format!("invalid regex: {e}")))?;

    let mut coverage_docs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&alignment_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.ends_with("_UPSTREAM_TEST_COVERAGE.md") {
                coverage_docs.push(path);
            }
        }
    }
    coverage_docs.sort();

    for cov_path in &coverage_docs {
        let text = read_text(cov_path)?;
        for caps in backtick_re.captures_iter(&text) {
            let raw = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let raw = strip_reference_suffix(raw.trim());
            if raw.is_empty() {
                continue;
            }
            if !is_probably_relative_path(raw) {
                continue;
            }
            if contains_glob(raw) {
                continue;
            }
            let path = workspace_root.join(raw);
            // `repo-ref/*` repositories are optional workspace checkouts (not committed).
            // We only require `fixtures/`, `docs/`, and `crates/` references to exist.
            if raw.starts_with("repo-ref/") && !path.exists() {
                continue;
            }
            if !path.exists() {
                failures.push(format!(
                    "broken reference in {}: `{}` does not exist",
                    cov_path.display(),
                    raw
                ));
                continue;
            }
            if raw.starts_with("fixtures/") && raw.ends_with(".mmd") {
                let golden = path.with_extension("golden.json");
                if !golden.exists() {
                    failures.push(format!(
                        "broken reference in {}: missing golden for `{}` (expected {})",
                        cov_path.display(),
                        raw,
                        golden.display()
                    ));
                }
            }
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::AlignmentCheckFailed(failures.join("\n")))
}

fn gen_default_config(args: Vec<String>) -> Result<(), XtaskError> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Err(XtaskError::Usage);
    }

    let mut schema_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--schema" => {
                i += 1;
                schema_path = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let schema_path = schema_path.unwrap_or_else(|| {
        PathBuf::from("repo-ref/mermaid/packages/mermaid/src/schemas/config.schema.yaml")
    });
    let out_path = out_path
        .unwrap_or_else(|| PathBuf::from("crates/merman-core/src/generated/default_config.json"));

    let schema_text = fs::read_to_string(&schema_path).map_err(|source| XtaskError::ReadFile {
        path: schema_path.display().to_string(),
        source,
    })?;
    let schema_yaml: YamlValue = serde_yaml::from_str(&schema_text)?;

    let Some(root_defaults) = extract_defaults(&schema_yaml, &schema_yaml) else {
        return Err(XtaskError::InvalidRef(
            "schema produced no defaults (unexpected)".to_string(),
        ));
    };

    let pretty = serde_json::to_string_pretty(&root_defaults)?;
    let out_dir = out_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    fs::write(&out_path, pretty).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

fn verify_generated(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() && !(args.len() == 1 && (args[0] == "--help" || args[0] == "-h")) {
        return Err(XtaskError::Usage);
    }
    if args.len() == 1 {
        return Err(XtaskError::Usage);
    }

    let tmp_dir = PathBuf::from("target/xtask");
    fs::create_dir_all(&tmp_dir).map_err(|source| XtaskError::WriteFile {
        path: tmp_dir.display().to_string(),
        source,
    })?;

    let mut failures = Vec::new();

    // Verify default config JSON.
    let expected_config = PathBuf::from("crates/merman-core/src/generated/default_config.json");
    let actual_config = tmp_dir.join("default_config.actual.json");
    gen_default_config(vec![
        "--schema".to_string(),
        "repo-ref/mermaid/packages/mermaid/src/schemas/config.schema.yaml".to_string(),
        "--out".to_string(),
        actual_config.display().to_string(),
    ])?;
    let expected_config_json: JsonValue = serde_json::from_str(&read_text(&expected_config)?)?;
    let actual_config_json: JsonValue = serde_json::from_str(&read_text(&actual_config)?)?;
    if expected_config_json != actual_config_json {
        failures.push(format!(
            "default config mismatch: regenerate with `cargo run -p xtask -- gen-default-config` ({})",
            expected_config.display()
        ));
    }

    // Verify DOMPurify allowlists.
    let expected_purify = PathBuf::from("crates/merman-core/src/generated/dompurify_defaults.rs");
    let actual_purify = tmp_dir.join("dompurify_defaults.actual.rs");
    gen_dompurify_defaults(vec![
        "--src".to_string(),
        "repo-ref/dompurify/dist/purify.cjs.js".to_string(),
        "--out".to_string(),
        actual_purify.display().to_string(),
    ])?;
    if read_text_normalized(&expected_purify)? != read_text_normalized(&actual_purify)? {
        failures.push(format!(
            "dompurify defaults mismatch: regenerate with `cargo run -p xtask -- gen-dompurify-defaults` ({})",
            expected_purify.display()
        ));
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::VerifyFailed(failures.join("\n")))
}

fn update_snapshots(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() && !(args.len() == 1 && (args[0] == "--help" || args[0] == "-h")) {
        return Err(XtaskError::Usage);
    }
    if args.len() == 1 {
        return Err(XtaskError::Usage);
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_root = workspace_root.join("fixtures");

    let mut mmd_files = Vec::new();
    let mut stack = vec![fixtures_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "mmd") {
                mmd_files.push(path);
            }
        }
    }
    mmd_files.sort();
    if mmd_files.is_empty() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "no .mmd fixtures found under {}",
            fixtures_root.display()
        )));
    }

    let engine = merman::Engine::new();
    let mut failures = Vec::new();

    fn ms_to_local_iso(ms: i64) -> Option<String> {
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)?;
        Some(
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%dT%H:%M:%S%.3f")
                .to_string(),
        )
    }

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let mut model = parsed.model;
        if let JsonValue::Object(obj) = &mut model {
            obj.remove("config");
            if parsed.meta.diagram_type == "mindmap" && obj.get("diagramId").is_some() {
                obj.insert(
                    "diagramId".to_string(),
                    JsonValue::String("<dynamic>".to_string()),
                );
            }

            if parsed.meta.diagram_type == "gantt" {
                let date_format = obj
                    .get("dateFormat")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("")
                    .trim();
                if !matches!(date_format, "x" | "X") {
                    if let Some(tasks) = obj.get_mut("tasks").and_then(JsonValue::as_array_mut) {
                        for task in tasks {
                            let JsonValue::Object(task_obj) = task else {
                                continue;
                            };
                            for key in ["startTime", "endTime", "renderEndTime"] {
                                let Some(v) = task_obj.get_mut(key) else {
                                    continue;
                                };
                                let Some(ms) = v
                                    .as_i64()
                                    .or_else(|| v.as_u64().and_then(|n| i64::try_from(n).ok()))
                                else {
                                    continue;
                                };
                                if let Some(s) = ms_to_local_iso(ms) {
                                    *v = JsonValue::String(s);
                                }
                            }
                        }
                    }
                }
            }
        }

        if parsed.meta.diagram_type == "gitGraph" {
            let re = Regex::new(r"\b(\d+)-[0-9a-f]{7}\b").map_err(|e| {
                XtaskError::SnapshotUpdateFailed(format!("invalid gitGraph id regex: {e}"))
            })?;

            fn walk(re: &Regex, v: &mut JsonValue) {
                match v {
                    JsonValue::String(s) => {
                        if re.is_match(s) {
                            *s = re.replace_all(s, "$1-<dynamic>").to_string();
                        }
                    }
                    JsonValue::Array(arr) => {
                        for item in arr {
                            walk(re, item);
                        }
                    }
                    JsonValue::Object(map) => {
                        for (_k, val) in map.iter_mut() {
                            walk(re, val);
                        }
                    }
                    _ => {}
                }
            }

            walk(&re, &mut model);
        }

        let out = serde_json::json!({
            "diagramType": parsed.meta.diagram_type,
            "model": model,
        });

        let pretty = match serde_json::to_string_pretty(&out) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to serialize JSON for {}: {err}",
                    mmd_path.display()
                ));
                continue;
            }
        };

        let out_path = mmd_path.with_extension("golden.json");
        if let Some(parent) = out_path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                failures.push(format!("failed to create dir {}: {err}", parent.display()));
                continue;
            }
        }
        if let Err(err) = fs::write(&out_path, format!("{pretty}\n")) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::SnapshotUpdateFailed(failures.join("\n")))
}

fn read_text(path: &Path) -> Result<String, XtaskError> {
    fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })
}

fn read_text_normalized(path: &Path) -> Result<String, XtaskError> {
    let text = read_text(path)?;
    let normalized_line_endings = text.replace("\r\n", "\n");
    Ok(normalized_line_endings.trim_end().to_string())
}

fn gen_dompurify_defaults(args: Vec<String>) -> Result<(), XtaskError> {
    let mut src_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--src" => {
                i += 1;
                src_path = args.get(i).map(PathBuf::from);
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

    let src_path =
        src_path.unwrap_or_else(|| PathBuf::from("repo-ref/dompurify/dist/purify.cjs.js"));
    let out_path = out_path
        .unwrap_or_else(|| PathBuf::from("crates/merman-core/src/generated/dompurify_defaults.rs"));

    let src_text = fs::read_to_string(&src_path).map_err(|source| XtaskError::ReadFile {
        path: src_path.display().to_string(),
        source,
    })?;

    let html_tags = extract_frozen_string_array(&src_text, "html$1")?;
    let svg_tags = extract_frozen_string_array(&src_text, "svg$1")?;
    let svg_filters = extract_frozen_string_array(&src_text, "svgFilters")?;
    let mathml_tags = extract_frozen_string_array(&src_text, "mathMl$1")?;

    let html_attrs = extract_frozen_string_array(&src_text, "html")?;
    let svg_attrs = extract_frozen_string_array(&src_text, "svg")?;
    let mathml_attrs = extract_frozen_string_array(&src_text, "mathMl")?;
    let xml_attrs = extract_frozen_string_array(&src_text, "xml")?;

    let default_data_uri_tags =
        extract_add_to_set_string_array(&src_text, "DEFAULT_DATA_URI_TAGS")?;
    let default_uri_safe_attrs =
        extract_add_to_set_string_array(&src_text, "DEFAULT_URI_SAFE_ATTRIBUTES")?;

    let allowed_tags = unique_sorted_lowercase(
        html_tags
            .into_iter()
            .chain(svg_tags)
            .chain(svg_filters)
            .chain(mathml_tags),
    );

    let allowed_attrs = unique_sorted_lowercase(
        html_attrs
            .into_iter()
            .chain(svg_attrs)
            .chain(mathml_attrs)
            .chain(xml_attrs),
    );

    let data_uri_tags = unique_sorted_lowercase(default_data_uri_tags);
    let uri_safe_attrs = unique_sorted_lowercase(default_uri_safe_attrs);

    let out_dir = out_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let rust = render_dompurify_defaults_rs(
        &allowed_tags,
        &allowed_attrs,
        &uri_safe_attrs,
        &data_uri_tags,
    );
    fs::write(&out_path, rust).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

fn render_dompurify_defaults_rs(
    allowed_tags: &[String],
    allowed_attrs: &[String],
    uri_safe_attrs: &[String],
    data_uri_tags: &[String],
) -> String {
    fn render_slice(name: &str, values: &[String]) -> String {
        let mut out = String::new();
        out.push_str(&format!("pub const {name}: &[&str] = &[\n"));
        for v in values {
            out.push_str(&format!("    {v:?},\n"));
        }
        out.push_str("];\n\n");
        out
    }

    let mut out = String::new();
    out.push_str("// This file is @generated by `cargo run -p xtask -- gen-dompurify-defaults`.\n");
    out.push_str("// Source: `repo-ref/dompurify/dist/purify.cjs.js` (DOMPurify 3.2.5)\n\n");
    out.push_str(&render_slice("DEFAULT_ALLOWED_TAGS", allowed_tags));
    out.push_str(&render_slice("DEFAULT_ALLOWED_ATTR", allowed_attrs));
    out.push_str(&render_slice("DEFAULT_URI_SAFE_ATTRIBUTES", uri_safe_attrs));
    out.push_str(&render_slice("DEFAULT_DATA_URI_TAGS", data_uri_tags));
    out
}

fn unique_sorted_lowercase<I>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut set = std::collections::BTreeSet::new();
    for v in values {
        set.insert(v.to_ascii_lowercase());
    }
    set.into_iter().collect()
}

fn extract_add_to_set_string_array(src: &str, ident: &str) -> Result<Vec<String>, XtaskError> {
    let needle = format!("const {ident} = addToSet({{}}, [");
    let start = src
        .find(&needle)
        .ok_or_else(|| XtaskError::ParseDompurify(format!("missing {ident} definition")))?;
    let bracket_start = start + needle.len() - 1; // points at '['
    extract_string_array_at(src, bracket_start)
}

fn extract_frozen_string_array(src: &str, ident: &str) -> Result<Vec<String>, XtaskError> {
    let needle = format!("const {ident} = freeze([");
    let start = src
        .find(&needle)
        .ok_or_else(|| XtaskError::ParseDompurify(format!("missing {ident} definition")))?;
    let bracket_start = start + needle.len() - 1; // points at '['
    extract_string_array_at(src, bracket_start)
}

fn extract_string_array_at(src: &str, bracket_start: usize) -> Result<Vec<String>, XtaskError> {
    let bytes = src.as_bytes();
    if *bytes.get(bracket_start).unwrap_or(&0) != b'[' {
        return Err(XtaskError::ParseDompurify("expected array '['".to_string()));
    }

    let mut out: Vec<String> = Vec::new();
    let mut i = bracket_start + 1;
    let mut in_string = false;
    let mut cur = String::new();

    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            match b {
                b'\\' => {
                    // Minimal escape handling: keep the escaped character verbatim.
                    if i + 1 >= bytes.len() {
                        return Err(XtaskError::ParseDompurify(
                            "unterminated escape".to_string(),
                        ));
                    }
                    let next = bytes[i + 1] as char;
                    cur.push(next);
                    i += 2;
                    continue;
                }
                b'\'' => {
                    out.push(cur.clone());
                    cur.clear();
                    in_string = false;
                    i += 1;
                    continue;
                }
                _ => {
                    cur.push(b as char);
                    i += 1;
                    continue;
                }
            }
        }

        match b {
            b'\'' => {
                in_string = true;
                i += 1;
            }
            b']' => return Ok(out),
            _ => i += 1,
        }
    }

    Err(XtaskError::ParseDompurify("unterminated array".to_string()))
}

fn extract_defaults(schema: &YamlValue, root: &YamlValue) -> Option<JsonValue> {
    let schema = expand_schema(schema, root);

    if let Some(default) = schema
        .as_mapping()
        .and_then(|m| m.get(&YamlValue::String("default".to_string())))
    {
        return yaml_to_json(default).ok();
    }

    if let Some(any_of) = schema
        .as_mapping()
        .and_then(|m| m.get(&YamlValue::String("anyOf".to_string())))
        .and_then(|v| v.as_sequence())
    {
        for s in any_of {
            if let Some(d) = extract_defaults(s, root) {
                return Some(d);
            }
        }
    }

    if let Some(one_of) = schema
        .as_mapping()
        .and_then(|m| m.get(&YamlValue::String("oneOf".to_string())))
        .and_then(|v| v.as_sequence())
    {
        for s in one_of {
            if let Some(d) = extract_defaults(s, root) {
                return Some(d);
            }
        }
    }

    let is_object_type = schema
        .as_mapping()
        .and_then(|m| m.get(&YamlValue::String("type".to_string())))
        .and_then(|v| v.as_str())
        == Some("object");

    let props = schema
        .as_mapping()
        .and_then(|m| m.get(&YamlValue::String("properties".to_string())))
        .and_then(|v| v.as_mapping());

    if is_object_type || props.is_some() {
        let mut out: BTreeMap<String, JsonValue> = BTreeMap::new();
        if let Some(props) = props {
            for (k, v) in props {
                let Some(k) = k.as_str() else { continue };
                if let Some(d) = extract_defaults(v, root) {
                    out.insert(k.to_string(), d);
                }
            }
        }
        if out.is_empty() {
            return None;
        }
        return Some(JsonValue::Object(out.into_iter().collect()));
    }

    None
}

fn expand_schema(schema: &YamlValue, root: &YamlValue) -> YamlValue {
    let mut schema = schema.clone();
    schema = resolve_ref(&schema, root).unwrap_or(schema);

    let all_of = schema
        .as_mapping()
        .and_then(|m| m.get(&YamlValue::String("allOf".to_string())))
        .and_then(|v| v.as_sequence())
        .cloned();

    if let Some(all_of) = all_of {
        let mut merged = schema.clone();
        if let Some(m) = merged.as_mapping_mut() {
            m.remove(&YamlValue::String("allOf".to_string()));
        }
        for s in all_of {
            let s = expand_schema(&s, root);
            merged = merge_yaml(merged, s);
        }
        merged
    } else {
        schema
    }
}

fn resolve_ref(schema: &YamlValue, root: &YamlValue) -> Result<YamlValue, XtaskError> {
    let Some(map) = schema.as_mapping() else {
        return Ok(schema.clone());
    };
    let Some(ref_str) = map
        .get(&YamlValue::String("$ref".to_string()))
        .and_then(|v| v.as_str())
    else {
        return Ok(schema.clone());
    };
    let target = resolve_ref_target(ref_str, root)?;
    let mut base = expand_schema(target, root);

    // Overlay other keys on top of the resolved target.
    let mut overlay = YamlValue::Mapping(map.clone());
    if let Some(m) = overlay.as_mapping_mut() {
        m.remove(&YamlValue::String("$ref".to_string()));
    }
    base = merge_yaml(base, overlay);
    Ok(base)
}

fn resolve_ref_target<'a>(r: &str, root: &'a YamlValue) -> Result<&'a YamlValue, XtaskError> {
    if !r.starts_with("#/") {
        return Err(XtaskError::InvalidRef(r.to_string()));
    }
    let mut cur = root;
    for seg in r.trim_start_matches("#/").split('/') {
        let Some(map) = cur.as_mapping() else {
            return Err(XtaskError::UnresolvedRef(r.to_string()));
        };
        let key = YamlValue::String(seg.to_string());
        cur = map
            .get(&key)
            .ok_or_else(|| XtaskError::UnresolvedRef(r.to_string()))?;
    }
    Ok(cur)
}

fn merge_yaml(mut base: YamlValue, overlay: YamlValue) -> YamlValue {
    match (&mut base, overlay) {
        (YamlValue::Mapping(dst), YamlValue::Mapping(src)) => {
            for (k, v) in src {
                match dst.get_mut(&k) {
                    Some(existing) => {
                        let merged = merge_yaml(existing.clone(), v);
                        *existing = merged;
                    }
                    None => {
                        dst.insert(k, v);
                    }
                }
            }
            base
        }
        (_, v) => v,
    }
}

fn yaml_to_json(v: &YamlValue) -> Result<JsonValue, serde_json::Error> {
    serde_json::to_value(v)
}
