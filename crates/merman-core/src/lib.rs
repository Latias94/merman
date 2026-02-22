#![forbid(unsafe_code)]
// LALRPOP generates code that can contain an "empty line after outer attribute" in its output.
// We keep the generated sources as-is and suppress this lint at the crate level.
#![allow(clippy::empty_line_after_outer_attr)]

//! Mermaid parser + semantic model (headless).
//!
//! Design goals:
//! - 1:1 parity with upstream Mermaid (`mermaid@11.12.2`)
//! - deterministic, testable outputs (semantic snapshot goldens)
//! - runtime-agnostic async APIs (no specific executor required)

pub mod common;
pub mod common_db;
pub mod config;
pub mod detect;
pub mod diagram;
pub mod diagrams;
pub mod entities;
pub mod error;
pub mod generated;
pub mod geom;
pub mod models;
pub mod preprocess;
mod runtime;
pub mod sanitize;
mod theme;
pub mod utils;

pub use config::MermaidConfig;
pub use detect::{Detector, DetectorRegistry};
pub use diagram::{
    DiagramRegistry, DiagramSemanticParser, ParsedDiagram, ParsedDiagramRender, RenderSemanticModel,
};
pub use error::{Error, Result};
pub use preprocess::{PreprocessResult, preprocess_diagram, preprocess_diagram_with_known_type};

#[derive(Debug, Clone, Copy, Default)]
pub struct ParseOptions {
    pub suppress_errors: bool,
}

impl ParseOptions {
    /// Strict parsing (errors are returned).
    pub fn strict() -> Self {
        Self {
            suppress_errors: false,
        }
    }

    /// Lenient parsing: on parse failures, return an `error` diagram instead of returning an error.
    pub fn lenient() -> Self {
        Self {
            suppress_errors: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseMetadata {
    pub diagram_type: String,
    /// Parsed config overrides extracted from front-matter and directives.
    /// This mirrors Mermaid's `mermaidAPI.parse()` return shape.
    pub config: MermaidConfig,
    /// The effective config used for detection/parsing after applying site defaults.
    pub effective_config: MermaidConfig,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Engine {
    registry: DetectorRegistry,
    diagram_registry: DiagramRegistry,
    site_config: MermaidConfig,
    fixed_today_local: Option<chrono::NaiveDate>,
    fixed_local_offset_minutes: Option<i32>,
}

impl Default for Engine {
    fn default() -> Self {
        let site_config = generated::default_site_config();

        Self {
            registry: DetectorRegistry::default_mermaid_11_12_2(),
            diagram_registry: DiagramRegistry::default_mermaid_11_12_2(),
            site_config,
            fixed_today_local: None,
            fixed_local_offset_minutes: None,
        }
    }
}

impl Engine {
    fn parse_timing_enabled() -> bool {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ENABLED.get_or_init(|| match std::env::var("MERMAN_PARSE_TIMING").as_deref() {
            Ok("1") | Ok("true") => true,
            _ => false,
        })
    }

    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides the "today" value used by diagrams that depend on local time (e.g. Gantt).
    ///
    /// This exists primarily to make fixture snapshots deterministic. By default, Mermaid uses the
    /// current local date.
    pub fn with_fixed_today(mut self, today: Option<chrono::NaiveDate>) -> Self {
        self.fixed_today_local = today;
        self
    }

    /// Overrides the local timezone offset (in minutes) used by diagrams that depend on local time
    /// semantics (notably Gantt).
    ///
    /// This exists primarily to make fixture snapshots deterministic across CI runners. When
    /// `None`, the system local timezone is used.
    pub fn with_fixed_local_offset_minutes(mut self, offset_minutes: Option<i32>) -> Self {
        self.fixed_local_offset_minutes = offset_minutes;
        self
    }

    pub fn with_site_config(mut self, site_config: MermaidConfig) -> Self {
        // Merge overrides onto Mermaid schema defaults so detectors keep working.
        self.site_config.deep_merge(site_config.as_value());
        self
    }

    pub fn registry(&self) -> &DetectorRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut DetectorRegistry {
        &mut self.registry
    }

    pub fn diagram_registry(&self) -> &DiagramRegistry {
        &self.diagram_registry
    }

    pub fn diagram_registry_mut(&mut self) -> &mut DiagramRegistry {
        &mut self.diagram_registry
    }

    /// Synchronous variant of [`Engine::parse_metadata`].
    ///
    /// This is useful for UI render pipelines that are synchronous (e.g. immediate-mode UI),
    /// where introducing an async executor would be awkward. The parsing work is CPU-bound and
    /// does not perform I/O.
    pub fn parse_metadata_sync(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParseMetadata>> {
        let Some((_, meta)) = self.preprocess_and_detect(text, options)? else {
            return Ok(None);
        };
        Ok(Some(meta))
    }

    /// Parses metadata for an already-known diagram type (skips type detection).
    ///
    /// This is intended for integrations that already know the diagram type, e.g. Markdown fences
    /// like ````mermaid` / ` ```flowchart` / ` ```sequenceDiagram`.
    ///
    /// ## Example (Markdown fence)
    ///
    /// ```no_run
    /// use merman_core::{Engine, ParseOptions};
    ///
    /// let engine = Engine::new();
    ///
    /// // Your markdown parser provides the fence info string (e.g. "flowchart", "sequenceDiagram").
    /// let fence = "sequenceDiagram";
    /// let diagram = r#"sequenceDiagram
    ///   Alice->>Bob: Hello
    /// "#;
    ///
    /// // Map fence info strings to merman's internal diagram ids.
    /// let diagram_type = match fence {
    ///     "sequenceDiagram" => "sequence",
    ///     "flowchart" | "graph" => "flowchart-v2",
    ///     "stateDiagram" | "stateDiagram-v2" => "stateDiagram",
    ///     other => other,
    /// };
    ///
    /// let meta = engine
    ///     .parse_metadata_as_sync(diagram_type, diagram, ParseOptions::strict())?
    ///     .expect("diagram detected");
    /// # Ok::<(), merman_core::Error>(())
    /// ```
    pub fn parse_metadata_as_sync(
        &self,
        diagram_type: &str,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParseMetadata>> {
        let Some((_, meta)) = self.preprocess_and_assume_type(diagram_type, text, options)? else {
            return Ok(None);
        };
        Ok(Some(meta))
    }

    pub async fn parse_metadata(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParseMetadata>> {
        self.parse_metadata_sync(text, options)
    }

    pub async fn parse_metadata_as(
        &self,
        diagram_type: &str,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParseMetadata>> {
        self.parse_metadata_as_sync(diagram_type, text, options)
    }

    /// Synchronous variant of [`Engine::parse_diagram`].
    ///
    /// Note: callers that want “always returns a diagram” behavior can set
    /// [`ParseOptions::suppress_errors`] to `true` to get an `error` diagram on parse failures.
    pub fn parse_diagram_sync(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        let timing_enabled = Self::parse_timing_enabled();
        let total_start = timing_enabled.then(std::time::Instant::now);

        let preprocess_start = timing_enabled.then(std::time::Instant::now);
        let Some((code, meta)) = self.preprocess_and_detect(text, options)? else {
            return Ok(None);
        };
        let preprocess = preprocess_start.map(|s| s.elapsed());

        let parse_start = timing_enabled.then(std::time::Instant::now);
        let parse = crate::runtime::with_fixed_today_local(self.fixed_today_local, || {
            crate::runtime::with_fixed_local_offset_minutes(self.fixed_local_offset_minutes, || {
                diagram::parse_or_unsupported(
                    &self.diagram_registry,
                    &meta.diagram_type,
                    &code,
                    &meta,
                )
            })
        });

        let mut model = match parse {
            Ok(v) => v,
            Err(err) => {
                if !options.suppress_errors {
                    return Err(err);
                }

                let mut error_meta = meta.clone();
                error_meta.diagram_type = "error".to_string();
                let mut error_model = serde_json::json!({ "type": "error" });
                common_db::apply_common_db_sanitization(
                    &mut error_model,
                    &error_meta.effective_config,
                );
                if let Some(start) = total_start {
                    let parse = parse_start.map(|s| s.elapsed()).unwrap_or_default();
                    eprintln!(
                        "[parse-timing] diagram=error total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                        start.elapsed(),
                        preprocess.unwrap_or_default(),
                        parse,
                        std::time::Duration::default(),
                        text.len(),
                    );
                }
                return Ok(Some(ParsedDiagram {
                    meta: error_meta,
                    model: error_model,
                }));
            }
        };
        let parse = parse_start.map(|s| s.elapsed());

        let sanitize_start = timing_enabled.then(std::time::Instant::now);
        common_db::apply_common_db_sanitization(&mut model, &meta.effective_config);
        let sanitize = sanitize_start.map(|s| s.elapsed());

        if let Some(start) = total_start {
            eprintln!(
                "[parse-timing] diagram={} total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                meta.diagram_type,
                start.elapsed(),
                preprocess.unwrap_or_default(),
                parse.unwrap_or_default(),
                sanitize.unwrap_or_default(),
                text.len(),
            );
        }
        Ok(Some(ParsedDiagram { meta, model }))
    }

    pub async fn parse_diagram(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        self.parse_diagram_sync(text, options)
    }

    /// Parses a diagram for layout/render pipelines.
    ///
    /// Compared to [`Engine::parse_diagram_sync`], this may omit semantic-model keys that are not
    /// required by merman's layout/SVG renderers (e.g. embedding the full effective config into the
    /// returned model). This keeps the public `parse_diagram*` APIs stable while allowing render
    /// pipelines to avoid paying large JSON clone costs.
    pub fn parse_diagram_for_render_sync(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        let Some((code, meta)) = self.preprocess_and_detect(text, options)? else {
            return Ok(None);
        };

        let parse_res = match meta.diagram_type.as_str() {
            "mindmap" => crate::diagrams::mindmap::parse_mindmap_for_render(&code, &meta),
            "stateDiagram" | "state" => {
                crate::diagrams::state::parse_state_for_render(&code, &meta)
            }
            _ => diagram::parse_or_unsupported(
                &self.diagram_registry,
                &meta.diagram_type,
                &code,
                &meta,
            ),
        };

        let mut model = match parse_res {
            Ok(v) => v,
            Err(err) => {
                if !options.suppress_errors {
                    return Err(err);
                }

                let mut error_meta = meta.clone();
                error_meta.diagram_type = "error".to_string();
                let mut error_model = serde_json::json!({ "type": "error" });
                common_db::apply_common_db_sanitization(
                    &mut error_model,
                    &error_meta.effective_config,
                );
                return Ok(Some(ParsedDiagram {
                    meta: error_meta,
                    model: error_model,
                }));
            }
        };

        common_db::apply_common_db_sanitization(&mut model, &meta.effective_config);
        Ok(Some(ParsedDiagram { meta, model }))
    }

    pub async fn parse_diagram_for_render(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        self.parse_diagram_for_render_sync(text, options)
    }

    /// Parses a diagram into a typed semantic model optimized for headless layout + SVG rendering.
    ///
    /// Unlike [`Engine::parse_diagram_for_render_sync`], this avoids constructing large
    /// `serde_json::Value` object trees for some high-impact diagrams (currently `stateDiagram` and
    /// `mindmap`) and instead returns typed semantic structs that the renderer can consume
    /// directly.
    ///
    /// Callers that need the semantic JSON model should continue using [`Engine::parse_diagram_sync`]
    /// or [`Engine::parse_diagram_for_render_sync`].
    pub fn parse_diagram_for_render_model_sync(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagramRender>> {
        let timing_enabled = Self::parse_timing_enabled();
        let total_start = timing_enabled.then(std::time::Instant::now);

        let preprocess_start = timing_enabled.then(std::time::Instant::now);
        let Some((code, meta)) = self.preprocess_and_detect(text, options)? else {
            return Ok(None);
        };
        let preprocess = preprocess_start.map(|s| s.elapsed());

        let parse_start = timing_enabled.then(std::time::Instant::now);
        let parse_res: Result<RenderSemanticModel> = match meta.diagram_type.as_str() {
            "mindmap" => crate::diagrams::mindmap::parse_mindmap_model_for_render(&code, &meta)
                .map(RenderSemanticModel::Mindmap),
            "stateDiagram" | "state" => {
                crate::diagrams::state::parse_state_model_for_render(&code, &meta)
                    .map(RenderSemanticModel::State)
            }
            "flowchart-v2" | "flowchart" | "flowchart-elk" => {
                crate::diagrams::flowchart::parse_flowchart_model_for_render(&code, &meta)
                    .map(RenderSemanticModel::Flowchart)
            }
            "classDiagram" | "class" => crate::diagrams::class::parse_class_typed(&code, &meta)
                .map(RenderSemanticModel::Class),
            "architecture" => {
                crate::diagrams::architecture::parse_architecture_model_for_render(&code, &meta)
                    .map(RenderSemanticModel::Architecture)
            }
            _ => diagram::parse_or_unsupported(
                &self.diagram_registry,
                &meta.diagram_type,
                &code,
                &meta,
            )
            .map(RenderSemanticModel::Json),
        };
        let parse = parse_start.map(|s| s.elapsed());

        let mut model = match parse_res {
            Ok(v) => v,
            Err(err) => {
                if !options.suppress_errors {
                    return Err(err);
                }

                let mut error_meta = meta.clone();
                error_meta.diagram_type = "error".to_string();
                let mut error_model = serde_json::json!({ "type": "error" });
                common_db::apply_common_db_sanitization(
                    &mut error_model,
                    &error_meta.effective_config,
                );
                if let Some(start) = total_start {
                    eprintln!(
                        "[parse-render-timing] diagram=error model=json total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                        start.elapsed(),
                        preprocess.unwrap_or_default(),
                        parse.unwrap_or_default(),
                        std::time::Duration::default(),
                        text.len(),
                    );
                }
                return Ok(Some(ParsedDiagramRender {
                    meta: error_meta,
                    model: RenderSemanticModel::Json(error_model),
                }));
            }
        };

        let sanitize_start = timing_enabled.then(std::time::Instant::now);
        match &mut model {
            RenderSemanticModel::Json(v) => {
                common_db::apply_common_db_sanitization(v, &meta.effective_config);
            }
            RenderSemanticModel::State(v) => {
                if let Some(s) = v.acc_title.as_deref() {
                    v.acc_title = Some(common_db::sanitize_acc_title(s, &meta.effective_config));
                }
                if let Some(s) = v.acc_descr.as_deref() {
                    v.acc_descr = Some(common_db::sanitize_acc_descr(s, &meta.effective_config));
                }
            }
            RenderSemanticModel::Mindmap(_) => {}
            RenderSemanticModel::Flowchart(_) => {}
            RenderSemanticModel::Class(v) => {
                if let Some(s) = v.acc_title.as_deref() {
                    v.acc_title = Some(common_db::sanitize_acc_title(s, &meta.effective_config));
                }
                if let Some(s) = v.acc_descr.as_deref() {
                    v.acc_descr = Some(common_db::sanitize_acc_descr(s, &meta.effective_config));
                }
            }
            RenderSemanticModel::Architecture(v) => {
                if let Some(s) = v.title.as_deref() {
                    v.title = Some(crate::sanitize::sanitize_text(s, &meta.effective_config));
                }
                if let Some(s) = v.acc_title.as_deref() {
                    v.acc_title = Some(common_db::sanitize_acc_title(s, &meta.effective_config));
                }
                if let Some(s) = v.acc_descr.as_deref() {
                    v.acc_descr = Some(common_db::sanitize_acc_descr(s, &meta.effective_config));
                }
            }
        }
        let sanitize = sanitize_start.map(|s| s.elapsed());

        if let Some(start) = total_start {
            let model_kind = match &model {
                RenderSemanticModel::Json(_) => "json",
                RenderSemanticModel::State(_) => "state",
                RenderSemanticModel::Mindmap(_) => "mindmap",
                RenderSemanticModel::Flowchart(_) => "flowchart",
                RenderSemanticModel::Architecture(_) => "architecture",
                RenderSemanticModel::Class(_) => "class",
            };
            eprintln!(
                "[parse-render-timing] diagram={} model={} total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                meta.diagram_type,
                model_kind,
                start.elapsed(),
                preprocess.unwrap_or_default(),
                parse.unwrap_or_default(),
                sanitize.unwrap_or_default(),
                text.len(),
            );
        }

        Ok(Some(ParsedDiagramRender { meta, model }))
    }

    pub async fn parse_diagram_for_render_model(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagramRender>> {
        self.parse_diagram_for_render_model_sync(text, options)
    }

    /// Parses a diagram into a typed semantic render model when the diagram type is already known
    /// (skips type detection).
    ///
    /// This is the preferred entrypoint for Markdown renderers and editors that already know the
    /// diagram type from the code fence info string. It avoids the detection pass and can reduce a
    /// small fixed overhead in tight render loops.
    pub fn parse_diagram_for_render_model_as_sync(
        &self,
        diagram_type: &str,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagramRender>> {
        let timing_enabled = Self::parse_timing_enabled();
        let total_start = timing_enabled.then(std::time::Instant::now);

        let preprocess_start = timing_enabled.then(std::time::Instant::now);
        let Some((code, meta)) = self.preprocess_and_assume_type(diagram_type, text, options)?
        else {
            return Ok(None);
        };
        let preprocess = preprocess_start.map(|s| s.elapsed());

        let parse_start = timing_enabled.then(std::time::Instant::now);
        let parse_res: Result<RenderSemanticModel> = match meta.diagram_type.as_str() {
            "mindmap" => crate::diagrams::mindmap::parse_mindmap_model_for_render(&code, &meta)
                .map(RenderSemanticModel::Mindmap),
            "stateDiagram" | "state" => {
                crate::diagrams::state::parse_state_model_for_render(&code, &meta)
                    .map(RenderSemanticModel::State)
            }
            "flowchart-v2" | "flowchart" | "flowchart-elk" => {
                crate::diagrams::flowchart::parse_flowchart_model_for_render(&code, &meta)
                    .map(RenderSemanticModel::Flowchart)
            }
            "classDiagram" | "class" => crate::diagrams::class::parse_class_typed(&code, &meta)
                .map(RenderSemanticModel::Class),
            "architecture" => {
                crate::diagrams::architecture::parse_architecture_model_for_render(&code, &meta)
                    .map(RenderSemanticModel::Architecture)
            }
            _ => diagram::parse_or_unsupported(
                &self.diagram_registry,
                &meta.diagram_type,
                &code,
                &meta,
            )
            .map(RenderSemanticModel::Json),
        };
        let parse = parse_start.map(|s| s.elapsed());

        let mut model = match parse_res {
            Ok(v) => v,
            Err(err) => {
                if !options.suppress_errors {
                    return Err(err);
                }

                let mut error_meta = meta.clone();
                error_meta.diagram_type = "error".to_string();
                let mut error_model = serde_json::json!({ "type": "error" });
                common_db::apply_common_db_sanitization(
                    &mut error_model,
                    &error_meta.effective_config,
                );
                if let Some(start) = total_start {
                    eprintln!(
                        "[parse-render-timing] diagram=error model=json total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                        start.elapsed(),
                        preprocess.unwrap_or_default(),
                        parse.unwrap_or_default(),
                        std::time::Duration::default(),
                        text.len(),
                    );
                }
                return Ok(Some(ParsedDiagramRender {
                    meta: error_meta,
                    model: RenderSemanticModel::Json(error_model),
                }));
            }
        };

        let sanitize_start = timing_enabled.then(std::time::Instant::now);
        match &mut model {
            RenderSemanticModel::Json(v) => {
                common_db::apply_common_db_sanitization(v, &meta.effective_config);
            }
            RenderSemanticModel::State(v) => {
                if let Some(s) = v.acc_title.as_deref() {
                    v.acc_title = Some(common_db::sanitize_acc_title(s, &meta.effective_config));
                }
                if let Some(s) = v.acc_descr.as_deref() {
                    v.acc_descr = Some(common_db::sanitize_acc_descr(s, &meta.effective_config));
                }
            }
            RenderSemanticModel::Mindmap(_) => {}
            RenderSemanticModel::Flowchart(_) => {}
            RenderSemanticModel::Class(v) => {
                if let Some(s) = v.acc_title.as_deref() {
                    v.acc_title = Some(common_db::sanitize_acc_title(s, &meta.effective_config));
                }
                if let Some(s) = v.acc_descr.as_deref() {
                    v.acc_descr = Some(common_db::sanitize_acc_descr(s, &meta.effective_config));
                }
            }
            RenderSemanticModel::Architecture(v) => {
                if let Some(s) = v.title.as_deref() {
                    v.title = Some(crate::sanitize::sanitize_text(s, &meta.effective_config));
                }
                if let Some(s) = v.acc_title.as_deref() {
                    v.acc_title = Some(common_db::sanitize_acc_title(s, &meta.effective_config));
                }
                if let Some(s) = v.acc_descr.as_deref() {
                    v.acc_descr = Some(common_db::sanitize_acc_descr(s, &meta.effective_config));
                }
            }
        }
        let sanitize = sanitize_start.map(|s| s.elapsed());

        if let Some(start) = total_start {
            let model_kind = match &model {
                RenderSemanticModel::Json(_) => "json",
                RenderSemanticModel::State(_) => "state",
                RenderSemanticModel::Mindmap(_) => "mindmap",
                RenderSemanticModel::Flowchart(_) => "flowchart",
                RenderSemanticModel::Architecture(_) => "architecture",
                RenderSemanticModel::Class(_) => "class",
            };
            eprintln!(
                "[parse-render-timing] diagram={} model={} total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                meta.diagram_type,
                model_kind,
                start.elapsed(),
                preprocess.unwrap_or_default(),
                parse.unwrap_or_default(),
                sanitize.unwrap_or_default(),
                text.len(),
            );
        }

        Ok(Some(ParsedDiagramRender { meta, model }))
    }

    pub async fn parse_diagram_for_render_model_as(
        &self,
        diagram_type: &str,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagramRender>> {
        self.parse_diagram_for_render_model_as_sync(diagram_type, text, options)
    }

    /// Parses a diagram when the diagram type is already known (skips type detection).
    ///
    /// This is the preferred entrypoint for Markdown renderers and editors that already know the
    /// diagram type from the code fence info string. It avoids the detection pass and can reduce a
    /// small fixed overhead in tight render loops.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// use merman_core::{Engine, ParseOptions};
    ///
    /// let engine = Engine::new();
    /// let input = "flowchart TD; A-->B;";
    ///
    /// let parsed = engine
    ///     .parse_diagram_as_sync("flowchart-v2", input, ParseOptions::strict())?
    ///     .expect("diagram detected");
    ///
    /// assert_eq!(parsed.meta.diagram_type, "flowchart-v2");
    /// # Ok::<(), merman_core::Error>(())
    /// ```
    pub fn parse_diagram_as_sync(
        &self,
        diagram_type: &str,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        let Some((code, meta)) = self.preprocess_and_assume_type(diagram_type, text, options)?
        else {
            return Ok(None);
        };

        let parse = crate::runtime::with_fixed_today_local(self.fixed_today_local, || {
            crate::runtime::with_fixed_local_offset_minutes(self.fixed_local_offset_minutes, || {
                diagram::parse_or_unsupported(
                    &self.diagram_registry,
                    &meta.diagram_type,
                    &code,
                    &meta,
                )
            })
        });

        let mut model = match parse {
            Ok(v) => v,
            Err(err) => {
                if !options.suppress_errors {
                    return Err(err);
                }

                let mut error_meta = meta.clone();
                error_meta.diagram_type = "error".to_string();
                let mut error_model = serde_json::json!({ "type": "error" });
                common_db::apply_common_db_sanitization(
                    &mut error_model,
                    &error_meta.effective_config,
                );
                return Ok(Some(ParsedDiagram {
                    meta: error_meta,
                    model: error_model,
                }));
            }
        };
        common_db::apply_common_db_sanitization(&mut model, &meta.effective_config);
        Ok(Some(ParsedDiagram { meta, model }))
    }

    pub async fn parse_diagram_as(
        &self,
        diagram_type: &str,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        self.parse_diagram_as_sync(diagram_type, text, options)
    }

    pub async fn parse(&self, text: &str, options: ParseOptions) -> Result<Option<ParseMetadata>> {
        self.parse_metadata(text, options).await
    }

    fn preprocess_and_detect(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<(String, ParseMetadata)>> {
        let pre = preprocess_diagram(text, &self.registry)?;
        if pre.code.trim_start().starts_with("---") {
            return Err(Error::MalformedFrontMatter);
        }

        let mut effective_config = self.site_config.clone();
        effective_config.deep_merge(pre.config.as_value());

        let diagram_type = match self
            .registry
            .detect_type_precleaned(&pre.code, &mut effective_config)
        {
            Ok(t) => t.to_string(),
            Err(err) => {
                if options.suppress_errors {
                    return Ok(None);
                }
                return Err(err);
            }
        };
        theme::apply_theme_defaults(&mut effective_config);

        let title = pre
            .title
            .as_ref()
            .map(|t| crate::sanitize::sanitize_text(t, &effective_config))
            .filter(|t| !t.is_empty());

        Ok(Some((
            pre.code,
            ParseMetadata {
                diagram_type,
                config: pre.config,
                effective_config,
                title,
            },
        )))
    }

    fn preprocess_and_assume_type(
        &self,
        diagram_type: &str,
        text: &str,
        _options: ParseOptions,
    ) -> Result<Option<(String, ParseMetadata)>> {
        let pre = preprocess_diagram_with_known_type(text, &self.registry, Some(diagram_type))?;
        if pre.code.trim_start().starts_with("---") {
            return Err(Error::MalformedFrontMatter);
        }

        let mut effective_config = self.site_config.clone();
        effective_config.deep_merge(pre.config.as_value());
        apply_detector_side_effects_for_known_type(diagram_type, &mut effective_config);
        theme::apply_theme_defaults(&mut effective_config);

        let title = pre
            .title
            .as_ref()
            .map(|t| crate::sanitize::sanitize_text(t, &effective_config))
            .filter(|t| !t.is_empty());

        Ok(Some((
            pre.code,
            ParseMetadata {
                diagram_type: diagram_type.to_string(),
                config: pre.config,
                effective_config,
                title,
            },
        )))
    }
}

fn apply_detector_side_effects_for_known_type(
    diagram_type: &str,
    effective_config: &mut MermaidConfig,
) {
    // Some Mermaid detectors have side effects on config (e.g. selecting ELK layout).
    // When the diagram type is known ahead of time, we must preserve these side effects so the
    // downstream layout/render pipeline behaves like the auto-detect path.
    if diagram_type == "flowchart-elk" {
        effective_config.set_value("layout", serde_json::Value::String("elk".to_string()));
        return;
    }

    if matches!(diagram_type, "flowchart-v2" | "flowchart")
        && effective_config.get_str("flowchart.defaultRenderer") == Some("elk")
    {
        effective_config.set_value("layout", serde_json::Value::String("elk".to_string()));
    }
}

#[cfg(test)]
mod tests;
