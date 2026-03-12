//! Optional math rendering hooks.
//!
//! Upstream Mermaid renders `$$...$$` fragments via KaTeX and measures the resulting HTML in a
//! browser DOM. merman is headless and pure-Rust by default, so math rendering is modeled as an
//! optional, pluggable backend.
//!
//! The default implementation is a no-op. For parity work, an optional Node.js-backed KaTeX
//! renderer is also provided.

use crate::text::{TextMetrics, TextStyle, WrapMode};
use merman_core::MermaidConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

/// Optional math renderer used to transform label HTML and (optionally) provide measurements.
///
/// Implementations should be:
/// - deterministic (stable output across runs),
/// - side-effect free (no global mutations),
/// - non-panicking (return `None` to decline handling).
pub trait MathRenderer: std::fmt::Debug {
    /// Attempts to render math fragments within an HTML label string.
    ///
    /// If the renderer declines to handle the input, it should return `None`.
    ///
    /// The returned string is treated as raw HTML and will still be sanitized by merman before
    /// emitting into an SVG `<foreignObject>`.
    fn render_html_label(&self, text: &str, config: &MermaidConfig) -> Option<String>;

    /// Optionally measures the rendered HTML label in pixels.
    ///
    /// This is intended to mirror upstream Mermaid's DOM measurement behavior for math labels.
    /// The default implementation returns `None`.
    fn measure_html_label(
        &self,
        _text: &str,
        _config: &MermaidConfig,
        _style: &TextStyle,
        _max_width_px: Option<f64>,
        _wrap_mode: WrapMode,
    ) -> Option<TextMetrics> {
        None
    }
}

/// Default math renderer: does nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopMathRenderer;

impl MathRenderer for NoopMathRenderer {
    fn render_html_label(&self, _text: &str, _config: &MermaidConfig) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RenderCacheKey {
    text: String,
    legacy_mathml: bool,
    force_legacy_mathml: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ProbeCacheKey {
    render: RenderCacheKey,
    font_family: Option<String>,
    font_size_bits: u64,
    font_weight: Option<String>,
    max_width_bits: u64,
}

#[derive(Debug, Clone)]
struct ProbeCacheValue {
    html: String,
    width: f64,
    height: f64,
    line_count: usize,
}

#[derive(Debug, Serialize)]
struct NodeRenderRequest {
    text: String,
    config: NodeMathConfig,
}

#[derive(Debug, Serialize)]
struct NodeProbeRequest {
    text: String,
    config: NodeMathConfig,
    #[serde(rename = "styleCss")]
    style_css: String,
    #[serde(rename = "maxWidthPx")]
    max_width_px: f64,
}

#[derive(Debug, Serialize)]
struct NodeMathConfig {
    #[serde(rename = "legacyMathML")]
    legacy_mathml: bool,
    #[serde(rename = "forceLegacyMathML")]
    force_legacy_mathml: bool,
}

#[derive(Debug, Deserialize)]
struct NodeRenderResponse {
    html: String,
}

#[derive(Debug, Deserialize)]
struct NodeProbeResponse {
    html: String,
    width: f64,
    height: f64,
}

/// Optional KaTeX backend that shells out to a local Node.js toolchain.
///
/// This backend is intended for parity work where a real browser DOM is available. It mirrors
/// Mermaid's flowchart HTML-label KaTeX path closely by:
/// - rendering KaTeX through the local `katex` npm package, and
/// - measuring the wrapped `<foreignObject>` HTML through local `puppeteer`.
///
/// The backend is completely opt-in; if the configured Node.js environment is unavailable or the
/// probe fails, it simply returns `None` and lets callers fall back to the default text path.
#[derive(Debug)]
pub struct NodeKatexMathRenderer {
    node_cwd: PathBuf,
    node_command: PathBuf,
    render_cache: Mutex<HashMap<RenderCacheKey, Option<String>>>,
    probe_cache: Mutex<HashMap<ProbeCacheKey, Option<ProbeCacheValue>>>,
}

impl NodeKatexMathRenderer {
    pub fn new(node_cwd: impl Into<PathBuf>) -> Self {
        Self {
            node_cwd: node_cwd.into(),
            node_command: PathBuf::from("node"),
            render_cache: Mutex::new(HashMap::new()),
            probe_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_node_command(mut self, node_command: impl Into<PathBuf>) -> Self {
        self.node_command = node_command.into();
        self
    }

    fn script_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("katex_flowchart_probe.cjs")
    }

    fn normalized_text(text: &str) -> String {
        text.replace("\\\\", "\\")
    }

    fn math_config(config: &MermaidConfig) -> NodeMathConfig {
        let config_value = config.as_value();
        let legacy_mathml = config_value
            .get("legacyMathML")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let force_legacy_mathml = config_value
            .get("forceLegacyMathML")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        NodeMathConfig {
            legacy_mathml,
            force_legacy_mathml,
        }
    }

    fn render_key(text: &str, config: &MermaidConfig) -> RenderCacheKey {
        let config = Self::math_config(config);
        RenderCacheKey {
            text: Self::normalized_text(text),
            legacy_mathml: config.legacy_mathml,
            force_legacy_mathml: config.force_legacy_mathml,
        }
    }

    fn style_css(style: &TextStyle) -> String {
        let mut out = String::new();
        let font_family = style
            .font_family
            .as_deref()
            .unwrap_or("\"trebuchet ms\",verdana,arial,sans-serif");
        let _ = write!(&mut out, "font-size: {}px;", style.font_size);
        let _ = write!(&mut out, "font-family: {};", font_family);
        if let Some(font_weight) = style.font_weight.as_deref() {
            if !font_weight.trim().is_empty() {
                let _ = write!(&mut out, "font-weight: {};", font_weight.trim());
            }
        }
        out
    }

    fn run_node_request<T, R>(&self, mode: &str, payload: &T) -> Option<R>
    where
        T: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        if !self.node_cwd.join("package.json").is_file() {
            return None;
        }

        let mut child = Command::new(&self.node_command)
            .arg(Self::script_path())
            .arg(mode)
            .current_dir(&self.node_cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .ok()?;

        if let Some(mut stdin) = child.stdin.take() {
            if serde_json::to_writer(&mut stdin, payload).is_err() {
                return None;
            }
            let _ = stdin.flush();
        }

        let output = child.wait_with_output().ok()?;
        if !output.status.success() {
            return None;
        }

        serde_json::from_slice(&output.stdout).ok()
    }

    fn render_cached(&self, text: &str, config: &MermaidConfig) -> Option<String> {
        let key = Self::render_key(text, config);
        if let Some(cached) = self
            .render_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&key).cloned())
        {
            return cached;
        }

        let response: Option<NodeRenderResponse> = self.run_node_request(
            "render",
            &NodeRenderRequest {
                text: key.text.clone(),
                config: NodeMathConfig {
                    legacy_mathml: key.legacy_mathml,
                    force_legacy_mathml: key.force_legacy_mathml,
                },
            },
        );
        let html = response.map(|value| value.html);

        if let Ok(mut cache) = self.render_cache.lock() {
            cache.insert(key, html.clone());
        }

        html
    }

    fn probe_cached(
        &self,
        text: &str,
        config: &MermaidConfig,
        style: &TextStyle,
        max_width_px: Option<f64>,
        _wrap_mode: WrapMode,
    ) -> Option<ProbeCacheValue> {
        let render = Self::render_key(text, config);
        let max_width = max_width_px.unwrap_or(200.0).max(1.0);
        let key = ProbeCacheKey {
            render: render.clone(),
            font_family: style.font_family.clone(),
            font_size_bits: style.font_size.to_bits(),
            font_weight: style.font_weight.clone(),
            max_width_bits: max_width.to_bits(),
        };
        if let Some(cached) = self
            .probe_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&key).cloned())
        {
            return cached;
        }

        let style_css = Self::style_css(style);
        let response: Option<NodeProbeResponse> = self.run_node_request(
            "probe",
            &NodeProbeRequest {
                text: render.text.clone(),
                config: NodeMathConfig {
                    legacy_mathml: render.legacy_mathml,
                    force_legacy_mathml: render.force_legacy_mathml,
                },
                style_css,
                max_width_px: max_width,
            },
        );
        let probed = response.and_then(|value| {
            if !value.width.is_finite() || !value.height.is_finite() {
                return None;
            }
            let line_count = value.html.match_indices("<div").count().max(1);
            Some(ProbeCacheValue {
                html: value.html,
                width: value.width.max(0.0),
                height: value.height.max(0.0),
                line_count,
            })
        });

        if let Some(probed_value) = probed.clone() {
            if let Ok(mut render_cache) = self.render_cache.lock() {
                render_cache
                    .entry(render)
                    .or_insert_with(|| Some(probed_value.html.clone()));
            }
        }
        if let Ok(mut cache) = self.probe_cache.lock() {
            cache.insert(key, probed.clone());
        }

        probed
    }
}

impl MathRenderer for NodeKatexMathRenderer {
    fn render_html_label(&self, text: &str, config: &MermaidConfig) -> Option<String> {
        if !text.contains("$$") {
            return None;
        }
        self.render_cached(text, config)
    }

    fn measure_html_label(
        &self,
        text: &str,
        config: &MermaidConfig,
        style: &TextStyle,
        max_width_px: Option<f64>,
        wrap_mode: WrapMode,
    ) -> Option<TextMetrics> {
        if wrap_mode != WrapMode::HtmlLike || !text.contains("$$") {
            return None;
        }
        let probed = self.probe_cached(text, config, style, max_width_px, wrap_mode)?;
        Some(TextMetrics {
            width: crate::text::round_to_1_64_px(probed.width),
            height: crate::text::round_to_1_64_px(probed.height),
            line_count: probed.line_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_katex_math_renderer_smoke() {
        let node_cwd = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tools")
            .join("mermaid-cli");
        if !node_cwd.join("package.json").is_file() || !node_cwd.join("node_modules").is_dir() {
            return;
        }

        let renderer = NodeKatexMathRenderer::new(node_cwd);
        let config = MermaidConfig::default();
        let style = TextStyle::default();

        let html = renderer
            .render_html_label("$$x^2$$", &config)
            .expect("node KaTeX renderer should produce HTML");
        assert!(html.contains("katex"), "unexpected HTML: {html}");

        let metrics = renderer
            .measure_html_label("$$x^2$$", &config, &style, Some(200.0), WrapMode::HtmlLike)
            .expect("node KaTeX renderer should produce metrics");
        assert!(metrics.width.is_finite() && metrics.width > 0.0);
        assert!(metrics.height.is_finite() && metrics.height > 0.0);
    }

    #[test]
    fn node_katex_math_renderer_matches_flowchart_browser_shell_metrics() {
        let node_cwd = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tools")
            .join("mermaid-cli");
        if !node_cwd.join("package.json").is_file() || !node_cwd.join("node_modules").is_dir() {
            return;
        }

        let renderer = NodeKatexMathRenderer::new(node_cwd);
        let config = MermaidConfig::default();
        let style = TextStyle::default();

        let node_metrics = renderer
            .measure_html_label(
                "$$f(\\relax{x}) = \\int_{-\\infty}^\\infty \\hat{f}(\\xi)\\,e^{2 \\pi i \\xi x}\\,d\\xi$$",
                &config,
                &style,
                Some(200.0),
                WrapMode::HtmlLike,
            )
            .expect("node label metrics");
        assert!(
            (node_metrics.width - 195.140625).abs() < 1e-9,
            "node width = {}",
            node_metrics.width
        );
        assert!(
            (node_metrics.height - 27.53125).abs() < 1e-9,
            "node height = {}",
            node_metrics.height
        );

        let edge_metrics = renderer
            .measure_html_label(
                "$$\\Bigg(\\bigg(\\Big(\\big((\\frac{-b\\pm\\sqrt{b^2-4ac}}{2a})\\big)\\Big)\\bigg)\\Bigg)$$",
                &config,
                &style,
                Some(200.0),
                WrapMode::HtmlLike,
            )
            .expect("edge label metrics");
        assert!(
            (edge_metrics.width - 184.78125).abs() < 1e-9,
            "edge width = {}",
            edge_metrics.width
        );
        assert!(
            (edge_metrics.height - 41.53125).abs() < 1e-9,
            "edge height = {}",
            edge_metrics.height
        );
    }
}
