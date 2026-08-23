#![forbid(unsafe_code)]

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
//! | Parse Mermaid or produce semantic JSON | no facade default features | [`Engine`] and [`ParseOptions`] |
//! | Analyze diagnostics or Markdown fences | `analysis` | [`analysis::Analyzer`] |
//! | Build parser-backed editor snapshots | `editor` | [`editor::analyze_document_snapshot_with_shared_text`] |
//! | Render Mermaid-like SVG | `svg` | [`Renderer`] and [`RenderRequest::svg`] |
//! | Render terminal-friendly text | `ascii` | [`Renderer`] and [`RenderRequest::ascii`] |
//! | Render PNG from Rust | `png` | [`Renderer`] and [`RenderRequest::png`] |
//! | Render JPEG from Rust | `jpeg` | [`Renderer`] and [`RenderRequest::jpeg`] |
//! | Render a vector PDF from Rust | `pdf` | [`Renderer`] and [`RenderRequest::pdf`] |
//!
//! If you already know the diagram type, use the `*_with_type_sync` methods on
//! [`Engine`] to skip detection. If you need lower-level layout or SVG pipeline
//! control, use the re-exported types under `merman::svg` or depend on
//! `merman-render` directly.
//!
//! # Features
//!
//! - `analysis`: render-free diagnostics, source mapping, and Markdown analysis through
//!   `merman::analysis`.
//! - `editor`: parser-backed editor snapshots and queries through `merman::editor`; this implies
//!   `analysis`.
//! - `svg`: layout plus SVG rendering through `merman::svg`.
//! - `ascii`: ASCII/Unicode text rendering through `merman::ascii`.
//! - `png`, `jpeg`, and `pdf`: bounded binary export through `merman::svg::export`; each
//!   implies `svg` but does not imply either of the other binary formats.
//! - `math`: pure-Rust math label rendering for the SVG path; this implies
//!   `svg`.
//!
//! The default feature set is [`complete-svg`](#features): it supports complete deterministic SVG
//! rendering, both optional layout engines, and math labels without compiling ambient system
//! adapters. Use `default-features = false` with the direct capability leaves when you need a
//! measured artifact closure.
//!
//! Parser-only applications should depend on `merman-core` directly. If they need this facade's
//! re-exports instead, they must set `default-features = false`; an ordinary `merman` dependency
//! intentionally compiles the complete SVG workflow.
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
//! For semantic inspection, use [`Renderer::prepare_semantic`] or a
//! [`RenderTarget::Semantic`] request. For terminal output, use [`RenderTarget::Ascii`]. The
//! target adapters never create a replacement operation or silently replace the caller's
//! cancellation handle.

pub use merman_core::*;

#[path = "operation.rs"]
mod operation_runner;
pub mod render;
#[cfg(feature = "ascii")]
pub use render::AsciiRequest;
#[cfg(any(feature = "png", feature = "jpeg"))]
pub use render::RasterOutput;
#[cfg(feature = "svg")]
pub use render::{
    DocumentPortabilityReport, RenderArtifactKind, RenderEvidence, RenderedDocument,
    SvgEnvironment, SvgLayoutOutput, SvgOutput, SvgRequest, TargetAdmissionError,
    TargetAdmissionReason, TargetAdmissionReceipt, TargetAdmissionStatus, TargetFontSource,
    ThemeEvidenceStatus, ThemeEvidenceSummary,
};
#[cfg(feature = "jpeg")]
pub use render::{JpegRequest, PreparedJpegExport};
#[cfg(feature = "pdf")]
pub use render::{PdfOutput, PdfRequest, PreparedPdfExport};
#[cfg(feature = "png")]
pub use render::{PngRequest, PreparedPngExport};
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
/// These helpers are deliberately feature-gated and excluded from the ordinary facade. They may
/// change or disappear without a compatibility promise.
#[cfg(feature = "internal-theme-acceptance")]
#[doc(hidden)]
pub mod __theme_acceptance {
    /// Borrowed production-owned artifact and its inseparable target admission receipt.
    ///
    /// The acceptance harness may inspect the exact bytes, but it cannot pair arbitrary bytes with
    /// a receipt or reconstruct the production target seal.
    #[derive(Debug, Clone, Copy)]
    pub struct TargetArtifactView<'a> {
        bytes: &'a [u8],
        receipt: &'a crate::TargetAdmissionReceipt,
    }

    impl<'a> TargetArtifactView<'a> {
        pub fn from_rendered_document(document: &'a crate::RenderedDocument) -> Self {
            Self {
                bytes: document.svg().as_bytes(),
                receipt: document.standalone_svg_admission(),
            }
        }

        #[cfg(any(feature = "png", feature = "jpeg"))]
        pub fn from_raster_output(output: &'a crate::RasterOutput) -> Self {
            Self {
                bytes: output.bytes(),
                receipt: output.admission(),
            }
        }

        #[cfg(feature = "pdf")]
        pub fn from_pdf_output(output: &'a crate::PdfOutput) -> Self {
            Self {
                bytes: output.bytes(),
                receipt: output.admission(),
            }
        }

        pub const fn bytes(self) -> &'a [u8] {
            self.bytes
        }

        pub const fn receipt(self) -> &'a crate::TargetAdmissionReceipt {
            self.receipt
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ThemeAcceptanceScopeEvidence {
        projection: crate::render::ThemeEvidenceScopeProjection,
    }

    impl ThemeAcceptanceScopeEvidence {
        pub const fn status(&self) -> crate::ThemeEvidenceStatus {
            self.projection.status
        }

        pub const fn required_count(&self) -> usize {
            self.projection.required_count
        }

        pub const fn accounted_count(&self) -> usize {
            self.projection.accounted_count
        }

        pub const fn applied_count(&self) -> usize {
            self.projection.applied_count
        }

        pub const fn not_applicable_count(&self) -> usize {
            self.projection.not_applicable_count
        }

        pub const fn incomplete_count(&self) -> usize {
            self.projection.incomplete_count()
        }

        pub const fn residual_count(&self) -> usize {
            self.projection.residual_count
        }

        pub const fn output_mutated(&self) -> bool {
            self.projection.output_mutated
        }

        pub const fn is_verified(&self) -> bool {
            matches!(self.projection.status, crate::ThemeEvidenceStatus::Verified)
                && self.projection.is_satisfied()
        }

        pub const fn is_satisfied(&self) -> bool {
            self.projection.is_satisfied()
        }
    }

    pub struct ThemeAcceptanceEvidence<'a> {
        projection: crate::render::ThemeAcceptanceEvidenceProjection<'a>,
    }

    impl<'a> ThemeAcceptanceEvidence<'a> {
        pub const fn root(&self) -> ThemeAcceptanceScopeEvidence {
            ThemeAcceptanceScopeEvidence {
                projection: self.projection.root,
            }
        }

        pub const fn family(&self) -> ThemeAcceptanceScopeEvidence {
            ThemeAcceptanceScopeEvidence {
                projection: self.projection.family,
            }
        }

        pub const fn source_residual_count(&self) -> usize {
            self.projection.source_residual_count
        }

        pub const fn compatibility_residual_count(&self) -> usize {
            self.projection.compatibility_residual_count
        }

        pub const fn mermaid_compatibility_residual_count(&self) -> usize {
            self.projection.mermaid_compatibility_residual_count
        }

        pub const fn recipe_report(
            &self,
        ) -> Option<&'a merman_render::diagram_theme::ThemeRecipeReport> {
            self.projection.recipe_report
        }

        pub const fn host_admission_report(
            &self,
        ) -> Option<&'a merman_render::diagram_theme::ThemeHostAdmissionReport> {
            self.projection.host_admission_report
        }

        pub const fn portability_requirement(
            &self,
        ) -> Option<merman_render::diagram_theme::ThemePortabilityRequirement> {
            match self.projection.host_admission_report {
                Some(report) => Some(report.portability_requirement()),
                None => None,
            }
        }

        pub const fn prepared_text_layout(
            &self,
        ) -> Option<&'a merman_render::text::PreparedTextLayoutReport> {
            self.projection.prepared_text_layout
        }

        pub const fn text_layout_failure(&self) -> Option<merman_render::text::TextLayoutFailure> {
            self.projection.text_layout_failure
        }

        pub const fn effective_theme_resource_policy(
            &self,
        ) -> &'a merman_render::diagram_theme::ThemeResourcePolicy {
            self.projection.effective_theme_resource_policy
        }
    }

    pub fn theme_acceptance_evidence(
        evidence: &crate::RenderEvidence,
    ) -> ThemeAcceptanceEvidence<'_> {
        ThemeAcceptanceEvidence {
            projection: evidence.theme_acceptance_evidence(),
        }
    }

    pub fn native_filter_receipt(
        evidence: &crate::RenderEvidence,
    ) -> Option<merman_render::__private::NativeSvgFilterReceipt> {
        evidence.native_filter_receipt()
    }

    #[cfg(feature = "layout-cytoscape")]
    pub fn architecture_text_cutover_receipt(
        evidence: &crate::RenderEvidence,
    ) -> Option<&merman_render::__private::ArchitectureTextCutoverReceipt> {
        evidence.architecture_text_cutover_receipt()
    }

    /// Returns the opaque renderer-owned route receipts for one finalized document.
    pub fn theme_route_cutover_receipts(
        document: &crate::RenderedDocument,
    ) -> &[merman_render::__private::ThemeRouteCutoverReceipt] {
        document.theme_route_cutover_receipts()
    }
}

/// ASCII target-local types and model-level backend interface.
#[cfg(feature = "ascii")]
pub mod ascii;
