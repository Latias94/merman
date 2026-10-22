#![forbid(unsafe_code)]
#![allow(
    clippy::result_large_err,
    reason = "RenderError preserves rich terminal diagnostics at the public facade boundary."
)]

//! Mermaid parsing and rendering through one operation-scoped facade.
//!
//! [`Renderer`] owns long-lived engine defaults. Each [`RenderRequest`] describes one source,
//! target, resource policy, and [`OperationControl`]. The request is executed synchronously by
//! [`Renderer::render`]; SVG, ASCII, and binary output remain target-local adapters behind the
//! same operation boundary.
//!
//! The facade deliberately does not expose source-to-SVG or source-to-ASCII convenience
//! functions. Keeping the target and its policy in a typed request makes cancellation,
//! deadlines, resource budgets, and output ownership explicit at every call site.
//!
//! # Choosing an API
//!
//! | Goal | Feature | Start with |
//! | --- | --- | --- |
//! | Inspect canonical Mermaid meaning | `diagram-*` selectors, without defaults | [`SemanticParser`] or [`Renderer::parse_semantic`] |
//! | Produce Mermaid compatibility JSON | `compat-json` and family selectors | `MermaidEngine` and [`ParseOptions`] |
//! | Analyze diagnostics or Markdown fences | `analysis` | `analysis::Analyzer` |
//! | Build parser-backed editor snapshots | `editor` | `editor::analyze_document_snapshot_with_shared_text` |
//! | Render Mermaid-like SVG | `svg` | [`Renderer`] and `RenderRequest::svg` |
//! | Render terminal-friendly text | `ascii` | [`Renderer`] and `RenderRequest::ascii` |
//! | Render PNG from Rust | `png` | [`Renderer`] and `RenderRequest::png` |
//! | Render JPEG from Rust | `jpeg` | [`Renderer`] and `RenderRequest::jpeg` |
//! | Render a vector PDF from Rust | `pdf` | [`Renderer`] and `RenderRequest::pdf` |
//!
//! If you already know the diagram type, use the `*_with_type_sync` methods on
//! [`SemanticParser`] to skip detection. If you need lower-level layout or SVG pipeline
//! control, use the re-exported types under `merman::svg` or depend on
//! `merman-render` directly.
//!
//! # Features
//!
//! - `diagram-*`: compile the selected logical families and their typed models.
//!   `all-diagrams` selects every built-in family; these selectors do not enable outputs or engines.
//!
//! - `analysis`: render-free diagnostics, source mapping, and Markdown analysis through
//!   `merman::analysis`.
//! - `presentation`: full Mermaid configuration and paired presentation artifacts.
//! - `compat-json`: Mermaid-compatible JSON projections; implies `presentation`.
//! - `editor-facts`: optional parser-backed source facts without presentation.
//! - `editor`: parser-backed editor snapshots and queries through `merman::editor`; this implies
//!   `analysis`.
//! - `svg`: layout plus SVG rendering through `merman::svg`.
//! - `ascii`: ASCII/Unicode text rendering through `merman::ascii`.
//! - `png`, `jpeg`, and `pdf`: bounded binary export through `merman::svg::export`; each
//!   implies `svg` but does not imply either of the other binary formats.
//! - `math`: opt-in pure-Rust math label rendering for the SVG path; this implies
//!   `svg` and is not included in defaults.
//!
//! The default feature set is `all-diagrams`, `svg`, `layout-cytoscape`, and `layout-elk`: it supports
//! all built-in parsers, Mermaid 12's default ELK SVG path, deterministic rendering, and the Cytoscape
//! layout engine without ambient system adapters. Enable `math` explicitly to render math labels.
//! Distributed artifacts must retain the ELK implementation's EPL-2.0 notices and source provenance.
//! Use `default-features = false` with explicit family selectors, `svg`, and `layout-cytoscape` to omit
//! ELK, or choose individual capability leaves when you need a measured artifact closure.
//!
//! Parser-only applications should depend on `merman-core` directly. If they need this facade's
//! re-exports instead, set `default-features = false` and select the required `diagram-*` features
//! (or `all-diagrams` to retain the previous parser surface). An ordinary `merman` dependency
//! intentionally compiles SVG rendering with both layout engines.
//!
//! # Quick start
//!
//! ```no_run
//! # #[cfg(feature = "svg")]
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};
//!
//! let output = Renderer::new().render(RenderRequest::svg(
//!     "flowchart TD\nA[Start] --> B[Done]",
//!     OperationControl::new(),
//!     SvgRequest::default(),
//! ))?;
//! let RenderOutput::Svg(Some(svg)) = output else {
//!     return Err("source did not contain a Mermaid diagram".into());
//! };
//! println!("{}", svg.svg());
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "svg"))]
//! # fn main() {}
//! ```
//!
//! For semantic inspection, use [`Renderer::parse_semantic`] or a
//! [`RenderTarget::Semantic`] request. For terminal output, use `RenderTarget::Ascii`. The
//! target adapters never create a replacement operation or silently replace the caller's
//! cancellation handle.

#![doc = include_str!(
    "../docs/generated/merman-rustdoc/crate-overview.md"
)]

pub use merman_core::*;

pub mod diagnostic;
#[path = "operation.rs"]
mod operation_runner;
#[doc = include_str!(
    "../docs/generated/merman-rustdoc/render-module.md"
)]
pub mod render;
pub use diagnostic::{
    TerminalDiagnostic, TerminalDiagnosticClass, TerminalDiagnosticDetails,
    TerminalRuntimePolicyError, normalize_terminal_diagnostic, normalize_terminal_text,
};
#[cfg(feature = "ascii")]
pub use render::AsciiRequest;
#[cfg(any(feature = "png", feature = "jpeg"))]
pub use render::RasterOutput;
#[cfg(feature = "svg")]
pub use render::{
    DocumentPortabilityReport, OperationExecutionPath, RenderArtifactKind, RenderEvidence,
    RenderedDocument, SvgEnvironment, SvgLayoutOutput, SvgOutput, SvgRequest, TargetAdmissionError,
    TargetAdmissionReason, TargetAdmissionReceipt, TargetAdmissionStatus, TargetFontSource,
    ThemeDiagnostic, ThemeEvidenceStatus, ThemeEvidenceSummary,
};
#[cfg(feature = "jpeg")]
pub use render::{JpegRequest, PreparedJpegExport};
#[cfg(feature = "pdf")]
pub use render::{PdfOutput, PdfRequest, PreparedPdfExport};
#[cfg(feature = "png")]
pub use render::{PngRequest, PreparedPngExport};
#[cfg(feature = "presentation")]
pub use render::{PrepareSourceRequest, PreparedSource, PresentationArtifact};
pub use render::{
    RenderError, RenderOutput, RenderRequest, RenderTarget, Renderer, ResourceLimitCause,
    ResourceLimitExceeded, SemanticArtifact,
};

/// Diagnostics and source-mapping APIs for lint and analysis workflows.
#[cfg(feature = "analysis")]
pub use merman_analysis as analysis;

/// Parser-backed editor intelligence APIs.
#[cfg(feature = "editor")]
pub use merman_editor_core as editor;

/// SVG target-local types and backend capabilities.
#[cfg(feature = "svg")]
pub mod svg;

/// Versioned visual diagram-theme authoring and compilation.
#[cfg(feature = "svg")]
pub mod diagram_theme;

/// Workspace-only evidence seams used by the non-published theme acceptance harness.
///
/// These helpers require the workspace acceptance cfg and are excluded from published packages.
/// They may change or disappear without a compatibility promise.
#[cfg(all(feature = "svg", merman_internal_theme_acceptance))]
#[doc(hidden)]
#[path = "theme_acceptance.rs"]
pub mod __theme_acceptance;

/// ASCII target-local types and model-level backend interface.
#[cfg(feature = "ascii")]
pub mod ascii;
