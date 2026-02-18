//! Generators for SVG root viewport overrides.

use crate::XtaskError;
use crate::util::*;
use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn gen_svg_overrides(args: Vec<String>) -> Result<(), XtaskError> {
    let mut in_dir: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut base_font_size_px: f64 = 16.0;
    let mut mode: String = "sequence".to_string();
    let mut browser_exe: Option<PathBuf> = None;
    let mut text_anchor: String = "start".to_string();
    let mut preserve_spaces: bool = false;

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
            "--mode" => {
                i += 1;
                mode = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "sequence".to_string());
            }
            "--browser-exe" => {
                i += 1;
                browser_exe = args.get(i).map(PathBuf::from);
            }
            "--text-anchor" => {
                i += 1;
                text_anchor = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "start".to_string());
            }
            "--preserve-spaces" => preserve_spaces = true,
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

    fn parse_style_font_size_px(style: &str) -> Option<f64> {
        // Very small parser for `font-size: 16px;` patterns.
        let s = style.to_ascii_lowercase();
        let idx = s.find("font-size")?;
        let rest = &s[idx + "font-size".len()..];
        let rest = rest.trim_start_matches(|c: char| c == ':' || c.is_whitespace());
        let mut num = String::new();
        for ch in rest.chars() {
            if ch.is_ascii_digit() || ch == '.' {
                num.push(ch);
            } else {
                break;
            }
        }
        if num.is_empty() {
            return None;
        }
        num.parse::<f64>().ok()
    }

    fn node_is_inside_defs(n: roxmltree::Node<'_, '_>) -> bool {
        n.ancestors()
            .filter(|a| a.is_element())
            .any(|a| a.has_tag_name("defs"))
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone)]
    struct SampleKey {
        font_key: String,
        font_family_raw: String,
        size_key: usize,
    }

    let Ok(entries) = fs::read_dir(&in_dir) else {
        return Err(XtaskError::ReadFile {
            path: in_dir.display().to_string(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        });
    };

    // font_key + size_key => strings
    let mut strings_by_key: BTreeMap<(String, usize), Vec<String>> = BTreeMap::new();
    let mut family_by_font_key: BTreeMap<String, String> = BTreeMap::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_file_with_extension(&path, "svg") {
            continue;
        }
        let svg = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;

        let base_family_raw = extract_base_font_family(&svg);
        let font_key = normalize_font_key(&base_family_raw);
        if font_key.is_empty() {
            continue;
        }
        family_by_font_key
            .entry(font_key.clone())
            .or_insert_with(|| base_family_raw.clone());

        let Ok(doc) = roxmltree::Document::parse(&svg) else {
            continue;
        };

        for text_node in doc.descendants().filter(|n| n.has_tag_name("text")) {
            if node_is_inside_defs(text_node) {
                continue;
            }
            let class = text_node.attribute("class").unwrap_or_default();
            let tokens: Vec<&str> = class.split_whitespace().collect();

            let include = match mode.as_str() {
                "all" => true,
                // For strict SVG XML parity, sequence layout is extremely sensitive to message
                // text width (it drives `actor.margin` and thus all x coordinates). We start by
                // generating overrides from Mermaid's own text measurement. In practice, actor
                // box sizing is also driven by `calculateTextDimensions(...)`, so include actor
                // labels as well to avoid drift on long participant ids.
                "sequence" => tokens.iter().any(|t| matches!(*t, "messageText" | "actor")),
                _ => false,
            };
            if !include {
                continue;
            }

            let size_px = text_node
                .attribute("font-size")
                .and_then(|v| v.parse::<f64>().ok())
                .or_else(|| {
                    text_node
                        .attribute("style")
                        .and_then(parse_style_font_size_px)
                })
                .unwrap_or(base_font_size_px)
                .max(1.0);
            let size_key = (size_px * 1000.0).round().max(1.0) as usize;

            let mut pushed = false;
            for tspan in text_node.children().filter(|n| n.has_tag_name("tspan")) {
                if node_is_inside_defs(tspan) {
                    continue;
                }
                let raw = tspan.text().unwrap_or_default().to_string();
                if raw.trim().is_empty() {
                    continue;
                }
                pushed = true;
                strings_by_key
                    .entry((font_key.clone(), size_key))
                    .or_default()
                    .push(raw);
            }
            if pushed {
                continue;
            }
            let raw = text_node.text().unwrap_or_default().to_string();
            if raw.trim().is_empty() {
                continue;
            }
            strings_by_key
                .entry((font_key.clone(), size_key))
                .or_default()
                .push(raw);
        }
    }

    // For Mermaid `sequenceDiagram`, text widths are computed from the *encoded* Mermaid source
    // (after `encodeEntities(...)`), not from the final decoded SVG glyphs. To match upstream,
    // include raw strings extracted from our pinned fixture corpus as additional override seeds.
    if mode == "sequence" {
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        let fixtures_dir = workspace_root.join("fixtures").join("sequence");

        let engine = merman::Engine::new();
        let parse_opts = merman::ParseOptions {
            suppress_errors: true,
        };

        let mut extra: Vec<String> = Vec::new();
        if let Ok(entries) = fs::read_dir(&fixtures_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !is_file_with_extension(&path, "mmd") {
                    continue;
                }
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                let parsed =
                    match futures::executor::block_on(engine.parse_diagram(&text, parse_opts)) {
                        Ok(Some(v)) => v,
                        _ => continue,
                    };

                let m = &parsed.model;
                if let Some(actors) = m.get("actors").and_then(|v| v.as_object()) {
                    for a in actors.values() {
                        if let Some(s) = a.get("description").and_then(|v| v.as_str()) {
                            extra.push(s.to_string());
                        }
                    }
                }
                if let Some(msgs) = m.get("messages").and_then(|v| v.as_array()) {
                    for msg in msgs {
                        if let Some(s) = msg.get("message").and_then(|v| v.as_str()) {
                            extra.push(s.to_string());
                        }
                    }
                }
                if let Some(notes) = m.get("notes").and_then(|v| v.as_array()) {
                    for note in notes {
                        if let Some(s) = note.get("message").and_then(|v| v.as_str()) {
                            extra.push(s.to_string());
                        }
                    }
                }
                if let Some(boxes) = m.get("boxes").and_then(|v| v.as_array()) {
                    for b in boxes {
                        if let Some(s) = b.get("name").and_then(|v| v.as_str()) {
                            extra.push(s.to_string());
                        }
                    }
                }
                if let Some(title) = m.get("title").and_then(|v| v.as_str()) {
                    extra.push(title.to_string());
                }
            }
        }

        if !extra.is_empty() {
            for v in strings_by_key.values_mut() {
                v.extend(extra.iter().cloned());
            }
        }
    }

    if strings_by_key.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no svg text samples found under {}",
            in_dir.display()
        )));
    }

    #[derive(Debug, Clone, Copy, serde::Deserialize)]
    struct SvgTextBBoxMetrics {
        bbox_x: f64,
        bbox_w: f64,
    }

    #[derive(Debug, Clone, serde::Deserialize)]
    struct SequenceMessageWidth {
        // `utils.calculateTextDimensions(...).width` (NOT including wrapPadding).
        width_px: Option<f64>,
        #[serde(default)]
        center_diff: Option<f64>,
        #[serde(default)]
        margin_px: Option<f64>,
        #[serde(default)]
        debug_line_ids: Option<Vec<String>>,
        #[serde(default)]
        debug_svg_start: Option<String>,
        #[serde(default)]
        debug_actor_x1: Option<Vec<f64>>,
        #[serde(default)]
        debug_actor_rect_w: Option<Vec<f64>>,
        #[serde(default)]
        debug_cfg_message_font_family: Option<String>,
        #[serde(default)]
        debug_cfg_actor_margin: Option<f64>,
        #[serde(default)]
        debug_cfg_wrap_padding: Option<f64>,
        #[serde(default)]
        debug_cfg_width: Option<f64>,
    }

    fn measure_svg_text_bbox_metrics_via_browser(
        node_cwd: &Path,
        browser_exe: &Path,
        font_family: &str,
        font_size_px: f64,
        text_anchor: &str,
        preserve_spaces: bool,
        strings: &[String],
    ) -> Result<Vec<SvgTextBBoxMetrics>, XtaskError> {
        use std::process::Stdio;
        if strings.is_empty() {
            return Ok(Vec::new());
        }
        // Mermaid's default config ships `fontFamily` with a trailing `;` (see `getConfig()`),
        // and `sequenceRenderer.setConf(...)` copies that verbatim into `messageFontFamily`.
        //
        // When applying font families via CSSOM (as `calculateTextDimensions()` does), that
        // trailing `;` can change fallback font selection under Puppeteer headless shell. Our
        // upstream SVG baselines are generated via `mmdc` (headless shell), so preserve that
        // behavior by measuring with a trailing `;` here.
        let font_family = {
            // IMPORTANT: `calculateTextDimensions()` applies `fontFamily` via CSSOM:
            // `selection.style('font-family', fontFamily)`, i.e. `CSSStyleDeclaration::setProperty`.
            //
            // Mermaid's default `fontFamily` string includes a trailing `;` (see Mermaid config).
            // In Chromium (esp. Puppeteer headless shell), passing that exact value to CSSOM can
            // cause the declaration to be rejected and the UA fallback font to be used instead.
            //
            // Our upstream SVG baselines are generated via `mmdc` (headless shell), so we must
            // preserve this behavior here (do not strip quotes; only ensure a trailing `;`).
            let trimmed = font_family.trim_end();
            if trimmed.ends_with(';') {
                trimmed.to_string()
            } else {
                format!("{trimmed};")
            }
        };
        let input_json = serde_json::json!({
            "browser_exe": browser_exe.display().to_string(),
            "font_family": font_family,
            "font_size_px": font_size_px,
            "text_anchor": text_anchor,
            "preserve_spaces": preserve_spaces,
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
const textAnchor = input.text_anchor;
const preserveSpaces = !!input.preserve_spaces;
const strings = input.strings;

(async () => {
  const browser = await puppeteer.launch({
    headless: 'shell',
    executablePath: browserExe,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  const page = await browser.newPage();
  await page.setContent(`<!doctype html><html><head><style>body{margin:0;padding:0;}</style></head><body></body></html>`);

  const out = await page.evaluate(({ strings, fontFamily, fontSizePx, textAnchor, preserveSpaces }) => {
    const SVG_NS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('width', '2000');
    svg.setAttribute('height', '200');
    document.body.appendChild(svg);

    // `mermaid/utils.calculateTextDimensions()` measures both `'sans-serif'` and the supplied
    // font-family, then selects a result based on a heuristic (to handle missing user fonts).
    // For strict parity with `mmdc` baselines (which run under Puppeteer headless shell), we
    // replicate that logic here and store the chosen width as our override.
    const ff = String(fontFamily || '');
    const res = [];
    for (const s of strings) {
      const raw = String(s);
      const normalized = raw
        .replace(/<br\s*\/?\s*>/gi, ' ')
        .replace(/[\r\n]+/g, ' ');

      function measureWithFont(fontFamily) {
        const t = document.createElementNS(SVG_NS, 'text');
        t.setAttribute('x', '0');
        t.setAttribute('y', '0');
        const tspan = document.createElementNS(SVG_NS, 'tspan');
        tspan.setAttribute('x', '0');
        t.appendChild(tspan);

        // Mirror Mermaid `drawSimpleText(...).style(...)` behavior: apply presentation attributes
        // via CSSOM (not by string-building a `style="..."` attribute), because `fontFamily`
        // can contain a trailing `;` which must be parsed the same way as upstream baselines.
        t.style.setProperty('text-anchor', String(textAnchor || 'start'));
        t.style.setProperty('font-size', `${fontSizePx}px`);
        t.style.setProperty('font-weight', '400');
        t.style.setProperty('font-family', String(fontFamily || ''));
        if (preserveSpaces) {
          t.setAttribute('xml:space', 'preserve');
          t.style.setProperty('white-space', 'pre');
        }

        tspan.textContent = normalized || '\u200b';
        svg.appendChild(t);
        const bb = t.getBBox();
        svg.removeChild(t);
        const w = Math.round(bb.width);
        const h = Math.round(bb.height);
        return { w, h, lineHeight: h };
      }

      const dims0 = measureWithFont('sans-serif');
      const dims1 = measureWithFont(ff);
      const use0 = Number.isNaN(dims1.h) ||
        Number.isNaN(dims1.w) ||
        Number.isNaN(dims1.lineHeight) ||
        (dims0.h > dims1.h && dims0.w > dims1.w && dims0.lineHeight > dims1.lineHeight);
      const chosen = use0 ? dims0 : dims1;

      res.push({ bbox_x: 0, bbox_w: chosen.w });
    }
    return res;
  }, { strings, fontFamily, fontSizePx, textAnchor, preserveSpaces });

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
        Ok(raw)
    }

    fn infer_sequence_message_dimensions_width_px_via_mermaid_layout(
        node_cwd: &Path,
        browser_exe: Option<&Path>,
        strings: &[String],
    ) -> Result<Vec<SequenceMessageWidth>, XtaskError> {
        use std::process::Stdio;
        if strings.is_empty() {
            return Ok(Vec::new());
        }

        let debug = std::env::var_os("MERMAN_XTASK_DEBUG_SEQUENCE").is_some();
        let input_json = serde_json::json!({
            "browser_exe": browser_exe.map(|p| p.display().to_string()),
            "strings": strings,
            "debug": debug,
        })
        .to_string();

        // IMPORTANT: we infer Mermaid's internal `calculateTextDimensions(...).width` by
        // rendering a minimal 2-actor sequence diagram and inverting Mermaid's margin formula.
        //
        // Mermaid computes an actor-to-next margin using:
        //
        //   actor.margin = max(conf.actorMargin, messageWidth + conf.actorMargin - actor.width/2 - next.width/2)
        //
        // where:
        //
        //   messageWidth = calculateTextDimensions.width + 2*conf.wrapPadding
        //
        // If the margin saturates to `conf.actorMargin`, the exact width can't be recovered from
        // layout. To avoid that, we intentionally render with a very small `sequence.width`,
        // making actor widths small enough that typical message labels are in the non-saturated
        // regime.
        const JS: &str = r#"
const fs = require('fs');
const path = require('path');
const url = require('url');
const { createRequire } = require('module');
const requireFromCwd = createRequire(path.join(process.cwd(), 'package.json'));
const puppeteer = requireFromCwd('puppeteer');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const browserExe = input.browser_exe || null;
 const strings = input.strings || [];
 const debug = !!input.debug;

const cliRoot = process.cwd();
const mermaidHtmlPath = path.join(cliRoot, 'node_modules', '@mermaid-js', 'mermaid-cli', 'dist', 'index.html');
const mermaidIifePath = path.join(cliRoot, 'node_modules', 'mermaid', 'dist', 'mermaid.js');
const zenumlIifePath = path.join(cliRoot, 'node_modules', '@mermaid-js', 'mermaid-zenuml', 'dist', 'mermaid-zenuml.js');

(async () => {
  const launchOpts = { headless: 'shell', args: ['--no-sandbox', '--disable-setuid-sandbox'] };
  // NOTE: mmdc does NOT set `executablePath`, letting Puppeteer pick the best
  // headless-shell binary. Only use an explicit path if provided.
  if (browserExe) {
    launchOpts.executablePath = browserExe;
  }
  const browser = await puppeteer.launch(launchOpts);

  const page = await browser.newPage();
  await page.goto(url.pathToFileURL(mermaidHtmlPath).href);
  await page.addScriptTag({ path: mermaidIifePath });

   const out = await page.evaluate(async ({ strings, debug }) => {
    const mermaid = globalThis.mermaid;
    if (!mermaid) {
      throw new Error('mermaid global not found');
    }
    // Match upstream fixture generation: deterministic handDrawn seed, default theme, and
    // explicit sequence defaults to avoid any drift from build-time or environment defaults.
     mermaid.initialize({
       startOnLoad: false,
       theme: 'default',
       handDrawnSeed: 1,
       sequence: {
         actorMargin: 50,
         // Use a tiny min actor width to avoid margin saturation at `actorMargin`, so we can
         // invert from actor center distance to the internal text width deterministically.
         width: 1,
         wrapPadding: 10,
         messageFontSize: 16,
         messageFontFamily: '\"trebuchet ms\", verdana, arial, sans-serif',
       },
     });
     const cfg = mermaid.mermaidAPI && mermaid.mermaidAPI.getConfig ? mermaid.mermaidAPI.getConfig() : null;
     const cfgSeq = cfg && cfg.sequence ? cfg.sequence : {};

     const results = [];
     const container = document.getElementById('container') || document.body;
     const ACTOR_MARGIN = 50; // conf.actorMargin default
     const WRAP_PADDING = 10; // conf.wrapPadding default

    for (let i = 0; i < strings.length; i++) {
      const raw = String(strings[i] ?? '');
      // Keep the label as-is; Mermaid will normalize `<br/>` for width calculations internally.
      const def = [
        'sequenceDiagram',
        'participant A',
        'participant B',
        `A->>B: ${raw}`,
       ].join('\n');

      // Use a stable SVG id to mirror `mmdc` defaults (unless the user passes `--svgId`).
      // This reduces the risk of accidental id-scoped CSS differences affecting measurement.
      container.innerHTML = '';
      const { svg } = await mermaid.render('my-svg', def, container);

       const doc = new DOMParser().parseFromString(svg, 'image/svg+xml');
       const parseNumber = (v) => {
         const n = Number(v);
         return Number.isFinite(n) ? n : null;
       };

       // Mermaid increments actor line ids across renders (`actor0/actor1`, then `actor2/actor3`,
       // ...). Use the `actor{N}` id pattern and infer left/right ordering by x coordinate.
       const actorLines = Array.from(doc.querySelectorAll('line'))
         .filter((n) => /^actor\d+$/.test(String(n.getAttribute('id') || '')));
       if (actorLines.length < 2) {
         const lineIds = Array.from(doc.querySelectorAll('line'))
           .map((n) => n.getAttribute('id'))
           .filter((s) => !!s)
           .slice(0, 8);
         results.push({
           width_px: null,
           center_diff: null,
           margin_px: null,
           debug_line_ids: lineIds,
           debug_svg_start: svg.slice(0, 160),
         });
         continue;
       }
       const xs = actorLines
         .map((n) => parseNumber(n.getAttribute('x1')))
         .filter((n) => n !== null)
         .sort((a, b) => a - b);
       if (xs.length < 2) {
         const lineIds = Array.from(doc.querySelectorAll('line'))
           .map((n) => n.getAttribute('id'))
           .filter((s) => !!s)
           .slice(0, 8);
         results.push({
           width_px: null,
           center_diff: null,
           margin_px: null,
           debug_line_ids: lineIds,
           debug_svg_start: svg.slice(0, 160),
         });
         continue;
       }
       const centerDiff = xs[xs.length - 1] - xs[0];
       const rectWs = Array.from(doc.querySelectorAll('rect'))
         .filter((n) => String(n.getAttribute('class') || '').split(/\\s+/g).includes('actor-top'))
         .map((n) => parseNumber(n.getAttribute('width')))
         .filter((n) => n !== null)
         .slice(0, 4);
       const w0 = rectWs.length >= 1 ? rectWs[0] : null;
       const w1 = rectWs.length >= 2 ? rectWs[1] : null;
       const margin = (w0 !== null && w1 !== null) ? (centerDiff - (w0 / 2) - (w1 / 2)) : null;

       // With non-saturated margins (ensured by `sequence.width: 1`), we have:
       //   centerDiff = messageWidth + ACTOR_MARGIN
       //   messageWidth = calculateTextDimensions.width + 2*WRAP_PADDING
       const inferredWidthPx = Math.round(centerDiff - ACTOR_MARGIN - 2 * WRAP_PADDING);
       const meta = {
         width_px: Number.isFinite(inferredWidthPx) ? inferredWidthPx : null,
         center_diff: centerDiff,
         margin_px: margin,
       };
       if (debug) {
         meta.debug_actor_x1 = xs;
         meta.debug_actor_rect_w = rectWs;
         if (i === 0 && cfgSeq) {
           meta.debug_cfg_message_font_family = String(cfgSeq.messageFontFamily ?? '');
           meta.debug_cfg_actor_margin = Number(cfgSeq.actorMargin ?? NaN);
           meta.debug_cfg_wrap_padding = Number(cfgSeq.wrapPadding ?? NaN);
           meta.debug_cfg_width = Number(cfgSeq.width ?? NaN);
         }
       }
       results.push(meta);
     }
     return results;
   }, { strings, debug });

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
                "sequence layout inference failed".to_string(),
            ));
        }
        let raw: Vec<SequenceMessageWidth> =
            serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
        Ok(raw)
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

    let browser_exe = if let Some(p) = browser_exe.as_deref() {
        p.to_path_buf()
    } else if cfg!(windows) {
        detect_windows_browser_exe().ok_or_else(|| {
            XtaskError::SvgCompareFailed("no supported browser found for svg measurement".into())
        })?
    } else {
        return Err(XtaskError::SvgCompareFailed(
            "browser measurement requires --browser-exe on this platform".into(),
        ));
    };

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let node_cwd = workspace_root.join("tools").join("mermaid-cli");

    // font_key => (text => (size_key, left_em, right_em))
    let mut best_by_font: BTreeMap<String, BTreeMap<String, (usize, f64, f64)>> = BTreeMap::new();
    let base_size_key = (base_font_size_px * 1000.0).round().max(1.0) as usize;

    for ((font_key, size_key), mut strings) in strings_by_key {
        strings.sort();
        strings.dedup();
        if strings.is_empty() {
            continue;
        }
        let Some(font_family_raw) = family_by_font_key.get(&font_key).cloned() else {
            continue;
        };
        let font_size_px = (size_key as f64) / 1000.0;
        let denom = font_size_px.max(1.0);
        let by_text = best_by_font.entry(font_key.clone()).or_default();

        if mode == "sequence" {
            // For sequence message text, infer widths from Mermaid layout itself (see helper).
            let debug = std::env::var_os("MERMAN_XTASK_DEBUG_SEQUENCE").is_some();
            if debug {
                eprintln!(
                    "[gen-svg-overrides] sequence: font_key={font_key} size_px={font_size_px} unique_strings={}",
                    strings.len()
                );
                for s in strings.iter().take(8) {
                    eprintln!("  sample: {:?}", s);
                }
            }
            let raw = infer_sequence_message_dimensions_width_px_via_mermaid_layout(
                &node_cwd, None, &strings,
            )?;
            let widths = raw.iter().map(|m| m.width_px).collect::<Vec<_>>();
            if debug {
                let inferred = widths.iter().filter(|w| w.is_some()).count();
                eprintln!("  inferred_widths={inferred}");
                for ((s, w), meta) in strings.iter().zip(widths.iter()).zip(raw.iter()).take(8) {
                    eprintln!(
                        "  out: {:?} => width={:?} (center_diff={:?}, margin_px={:?}, debug_actor_x1={:?}, debug_actor_rect_w={:?}, cfg={:?}/{:?}/{:?}/{:?}, debug_line_ids={:?})",
                        s,
                        w,
                        meta.center_diff,
                        meta.margin_px,
                        meta.debug_actor_x1,
                        meta.debug_actor_rect_w,
                        meta.debug_cfg_message_font_family,
                        meta.debug_cfg_actor_margin,
                        meta.debug_cfg_wrap_padding,
                        meta.debug_cfg_width,
                        meta.debug_line_ids
                    );
                    if meta.center_diff.is_none() {
                        if let Some(s) = meta.debug_svg_start.as_deref() {
                            eprintln!("    debug_svg_start: {}", s);
                        }
                    }
                }
            }
            for (text, w_px_opt) in strings.into_iter().zip(widths.into_iter()) {
                let Some(w_px) = w_px_opt else {
                    continue;
                };
                if !w_px.is_finite() || w_px <= 0.0 {
                    continue;
                }
                let left_em = 0.0;
                let right_em = w_px / denom;
                match by_text.get(&text) {
                    None => {
                        by_text.insert(text, (size_key, left_em, right_em));
                    }
                    Some((existing_size, _, _)) if *existing_size == base_size_key => {}
                    Some((existing_size, _, _)) if size_key == base_size_key => {
                        by_text.insert(text, (size_key, left_em, right_em));
                    }
                    Some(_) => {}
                }
            }
            continue;
        }

        let metrics = measure_svg_text_bbox_metrics_via_browser(
            &node_cwd,
            &browser_exe,
            &font_family_raw,
            font_size_px,
            &text_anchor,
            preserve_spaces,
            &strings,
        )?;

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
            if !(left_em.is_finite() && right_em.is_finite() && (left_em + right_em) > 0.0) {
                continue;
            }

            match by_text.get(&text) {
                None => {
                    by_text.insert(text, (size_key, left_em, right_em));
                }
                Some((existing_size, _, _)) if *existing_size == base_size_key => {}
                Some((existing_size, _, _)) if size_key == base_size_key => {
                    by_text.insert(text, (size_key, left_em, right_em));
                }
                Some(_) => {}
            }
        }
    }

    // Render as a deterministic Rust module.
    let mut out = String::new();
    fn rust_f64(v: f64) -> String {
        let mut s = format!("{v}");
        if !s.contains('.') && !s.contains('e') && !s.contains('E') {
            s.push_str(".0");
        }
        s
    }
    let _ = writeln!(&mut out, "// This file is generated by `xtask`.\n");
    let _ = writeln!(
        &mut out,
        "pub fn lookup_svg_override_em(font_key: &str, text: &str) -> Option<(f64, f64)> {{"
    );
    let _ = writeln!(&mut out, "    match font_key {{");
    for font_key in best_by_font.keys() {
        let _ = writeln!(
            &mut out,
            "        {:?} => lookup_in_{}(),",
            font_key,
            font_key.replace(['-', ','], "_")
        );
    }
    let _ = writeln!(&mut out, "        _ => None,");
    let _ = writeln!(&mut out, "    }}");
    let _ = writeln!(&mut out, "    .and_then(|tbl| lookup_in(tbl, text))");
    let _ = writeln!(&mut out, "}}\n");

    let _ = writeln!(
        &mut out,
        "fn lookup_in(tbl: &'static [(&'static str, f64, f64)], text: &str) -> Option<(f64, f64)> {{"
    );
    let _ = writeln!(&mut out, "    let mut lo = 0usize;");
    let _ = writeln!(&mut out, "    let mut hi = tbl.len();");
    let _ = writeln!(&mut out, "    while lo < hi {{");
    let _ = writeln!(&mut out, "        let mid = (lo + hi) / 2;");
    let _ = writeln!(&mut out, "        let (k, l, r) = tbl[mid];");
    let _ = writeln!(&mut out, "        match k.cmp(text) {{");
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Equal => return Some((l, r)),"
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
    let _ = writeln!(&mut out, "}}\n");

    for (font_key, by_text) in &best_by_font {
        let mut list: Vec<(&str, f64, f64)> = by_text
            .iter()
            .map(|(k, (_size, l, r))| (k.as_str(), *l, *r))
            .collect();
        list.sort_by(|a, b| a.0.cmp(b.0));

        let fn_name = format!("lookup_in_{}", font_key.replace(['-', ','], "_"));
        let _ = writeln!(
            &mut out,
            "fn {fn_name}() -> Option<&'static [(&'static str, f64, f64)]> {{ Some(SVG_OVERRIDES_{key}) }}",
            fn_name = fn_name,
            key = font_key.replace(['-', ','], "_").to_ascii_uppercase()
        );
        let _ = writeln!(
            &mut out,
            "static SVG_OVERRIDES_{key}: &[(&str, f64, f64)] = &[",
            key = font_key.replace(['-', ','], "_").to_ascii_uppercase()
        );
        for (text, l, r) in &list {
            let _ = writeln!(
                &mut out,
                "    ({:?}, {}, {}),",
                text,
                rust_f64(*l),
                rust_f64(*r)
            );
        }
        let _ = writeln!(&mut out, "];\n");
    }

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
