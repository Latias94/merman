pub use merman_ascii::{
    AsciiCharset, AsciiError, AsciiRenderOptions, AsciiRenderer, render_flowchart, render_model,
    render_sequence,
};

#[derive(Debug, thiserror::Error)]
pub enum HeadlessAsciiError {
    #[error(transparent)]
    Parse(#[from] merman_core::Error),
    #[error(transparent)]
    Ascii(#[from] merman_ascii::AsciiError),
}

pub type Result<T> = std::result::Result<T, HeadlessAsciiError>;

/// Synchronous ASCII/Unicode render helper (executor-free).
///
/// The Mermaid source is parsed by `merman-core`; the typed render model is then rendered by
/// `merman-ascii`.
pub fn render_ascii_sync(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    ascii_options: &AsciiRenderOptions,
) -> Result<Option<String>> {
    let (ascii_options, text) = ascii_options.apply_mermaid_ascii_directives(text);
    let Some(parsed) = engine.parse_diagram_for_render_model_sync(text.as_ref(), parse_options)?
    else {
        return Ok(None);
    };

    Ok(Some(merman_ascii::render_model(
        &parsed.model,
        &ascii_options,
    )?))
}

pub async fn render_ascii(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    ascii_options: &AsciiRenderOptions,
) -> Result<Option<String>> {
    // This async API is runtime-agnostic: rendering is CPU-bound and does not perform I/O.
    // It executes synchronously and does not yield.
    render_ascii_sync(engine, text, parse_options, ascii_options)
}

/// Convenience wrapper that bundles an [`merman_core::Engine`] and ASCII render options.
///
/// This is intended for terminal, log, documentation, and chat-surface integrations that want
/// stable text output without wiring parsing and rendering parameters on every call.
#[derive(Clone)]
pub struct HeadlessAsciiRenderer {
    pub engine: merman_core::Engine,
    pub parse: merman_core::ParseOptions,
    pub ascii: AsciiRenderOptions,
}

impl Default for HeadlessAsciiRenderer {
    fn default() -> Self {
        Self {
            engine: merman_core::Engine::new(),
            parse: merman_core::ParseOptions::default(),
            ascii: AsciiRenderOptions::default(),
        }
    }
}

impl HeadlessAsciiRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_site_config(mut self, site_config: merman_core::MermaidConfig) -> Self {
        self.engine = self.engine.with_site_config(site_config);
        self
    }

    pub fn with_parse_options(mut self, parse: merman_core::ParseOptions) -> Self {
        self.parse = parse;
        self
    }

    pub fn with_strict_parsing(self) -> Self {
        self.with_parse_options(merman_core::ParseOptions::strict())
    }

    pub fn with_lenient_parsing(self) -> Self {
        self.with_parse_options(merman_core::ParseOptions::lenient())
    }

    pub fn with_ascii_options(mut self, ascii: AsciiRenderOptions) -> Self {
        self.ascii = ascii;
        self
    }

    pub fn with_charset(mut self, charset: AsciiCharset) -> Self {
        self.ascii.charset = charset;
        self
    }

    pub fn parse_metadata_sync(&self, text: &str) -> Result<Option<merman_core::ParseMetadata>> {
        Ok(self.engine.parse_metadata_sync(text, self.parse)?)
    }

    pub fn parse_diagram_sync(&self, text: &str) -> Result<Option<merman_core::ParsedDiagram>> {
        Ok(self.engine.parse_diagram_sync(text, self.parse)?)
    }

    pub fn render_model(
        &self,
        model: &merman_core::diagram::RenderSemanticModel,
    ) -> Result<String> {
        Ok(merman_ascii::render_model(model, &self.ascii)?)
    }

    pub fn render_ascii_sync(&self, text: &str) -> Result<Option<String>> {
        render_ascii_sync(&self.engine, text, self.parse, &self.ascii)
    }

    pub async fn render_ascii(&self, text: &str) -> Result<Option<String>> {
        render_ascii_sync(&self.engine, text, self.parse, &self.ascii)
    }
}
