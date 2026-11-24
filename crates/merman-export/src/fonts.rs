//! Closed, bounded font acquisition and shared native text measurement.

use super::font_selection::FontSelectionPolicy;
use merman_model::{
    OperationControl, OperationLedgerError, OperationPhase, OperationResourceDomain,
    OperationResourceLimitExceeded, OperationResourceOverride, OperationResourceProvenance,
};
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementError,
    HostTextMeasurementRequest, HostTextMeasurer, TextMeasurementOperation,
};
use merman_render::text::{
    TextStyle, WrapMode, split_html_br_lines, try_measure_wrapped_text_with_line_metrics,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
#[cfg(not(target_family = "wasm"))]
use std::fs;
#[cfg(not(target_family = "wasm"))]
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

#[cfg(all(
    feature = "experimental-system-font-query",
    not(target_family = "wasm")
))]
mod platform;
#[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
mod registry;
#[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
mod resolver;
#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
mod sfnt;
#[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
mod system;

/// Explicit sources are processed in order; directory contents use stable path order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontSource {
    File(PathBuf),
    /// Resolves root and ancestor aliases before bounded traversal. Discovered symlinks are skipped.
    Directory(PathBuf),
}

/// Independent ceilings for font acquisition, residency, and shaping probes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFontLimits {
    /// Maximum input sources, including duplicate files or memory buffers.
    pub max_files: usize,
    /// Font data budget (default: 256 MiB). Explicit inputs are checked before deduplication.
    /// Selected system data shares the resident budget with the retained explicit sources.
    pub max_bytes: usize,
    pub max_faces: usize,
    pub max_directory_entries: usize,
    pub max_directory_depth: usize,
    pub max_label_bytes: usize,
    /// Maximum uncached, nonempty shaping probes within one operation (default: 16384).
    /// Cached metrics and missing-glyph outcomes do not consume this budget. This provider limit
    /// is independent of render resource profiles; set it with `NativeFontOptions::with_limits`
    /// before loading fonts.
    pub max_measurements: usize,
    pub max_cache_entries: usize,
    pub max_cache_bytes: usize,
    /// Successful native font selections pinned for one operation, including exports.
    /// Unlike the optional measurement cache, exhausting these limits is terminal.
    pub max_selection_entries: usize,
    pub max_selection_bytes: usize,
    /// Metadata discovery is separate from resident font bytes.
    pub max_system_files: usize,
    /// Cumulative temporary discovery reads; selected snapshots use `max_bytes` instead.
    pub max_system_scan_bytes: usize,
    pub max_system_faces: usize,
    pub max_system_cmap_bytes: usize,
    pub max_system_cmap_table_bytes: usize,
}

impl Default for NativeFontLimits {
    fn default() -> Self {
        Self {
            max_files: 256,
            max_bytes: 256 * 1024 * 1024,
            max_faces: 1024,
            max_directory_entries: 16_384,
            max_directory_depth: 16,
            max_label_bytes: 64 * 1024,
            max_measurements: 16_384,
            max_cache_entries: 1024,
            max_cache_bytes: 4 * 1024 * 1024,
            max_selection_entries: 16_384,
            max_selection_bytes: 4 * 1024 * 1024,
            max_system_files: 4096,
            max_system_scan_bytes: 2 * 1024 * 1024 * 1024,
            max_system_faces: 16_384,
            max_system_cmap_bytes: 16 * 1024 * 1024,
            max_system_cmap_table_bytes: 1024 * 1024,
        }
    }
}

/// No system discovery occurs unless requested and compiled with `system-fonts`.
#[derive(Debug, Clone, Default)]
pub struct NativeFontOptions {
    pub sources: Vec<FontSource>,
    pub system_fonts: bool,
    pub limits: NativeFontLimits,
}

impl NativeFontOptions {
    pub fn with_font_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.sources.push(FontSource::File(path.into()));
        self
    }

    pub fn with_font_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.sources.push(FontSource::Directory(path.into()));
        self
    }

    pub fn with_system_fonts(mut self, enabled: bool) -> Self {
        self.system_fonts = enabled;
        self
    }

    pub fn with_limits(mut self, limits: NativeFontLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn load(self, control: &OperationControl) -> Result<NativeFontContext, NativeFontError> {
        self.load_inner(control, false)
    }

    /// Authorizes platform-managed discovery when system fonts are requested.
    /// Platform metadata access is outside Merman's acquisition limits; selected
    /// font bytes remain bounded and shared by measurement and painting.
    #[cfg(all(
        feature = "experimental-system-font-query",
        not(target_family = "wasm")
    ))]
    pub fn load_platform(
        self,
        control: &OperationControl,
    ) -> Result<NativeFontContext, NativeFontError> {
        self.load_inner(control, true)
    }

    fn load_inner(
        self,
        control: &OperationControl,
        platform_requested: bool,
    ) -> Result<NativeFontContext, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Admission)?;
        #[cfg(not(all(feature = "system-fonts", not(target_family = "wasm"))))]
        if self.system_fonts {
            return Err(NativeFontError::SystemDiscoveryUnavailable);
        }
        #[cfg(all(
            feature = "experimental-system-font-query",
            not(target_family = "wasm")
        ))]
        let platform = (self.system_fonts && platform_requested)
            .then(|| platform::PlatformCatalog::new(control).map(Arc::new))
            .transpose()?;
        #[cfg(not(all(
            feature = "experimental-system-font-query",
            not(target_family = "wasm")
        )))]
        let _ = platform_requested;
        let mut loader = FontLoader::new(self.limits, control.clone());
        for source in self.sources {
            #[cfg(not(target_family = "wasm"))]
            match source {
                FontSource::File(path) => loader.file(&path)?,
                FontSource::Directory(path) => {
                    loader.checkpoint()?;
                    let root =
                        fs::canonicalize(&path).map_err(|error| source_error(&path, error))?;
                    loader.directory(&root, 0)?;
                }
            }
            #[cfg(target_family = "wasm")]
            match source {
                FontSource::File(path) | FontSource::Directory(path) => {
                    return Err(source_error(
                        &path,
                        "filesystem font acquisition is unavailable on WebAssembly; provide font bytes",
                    ));
                }
            }
        }
        loader.finish(
            self.system_fonts,
            control,
            #[cfg(all(
                feature = "experimental-system-font-query",
                not(target_family = "wasm")
            ))]
            platform,
        )
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum NativeFontError {
    #[error(
        "bounded system font discovery is unavailable; provide font bytes or supported explicit files"
    )]
    SystemDiscoveryUnavailable,
    #[error(
        "platform system font service {service} is unavailable; select bounded discovery or provide explicit fonts"
    )]
    PlatformDiscoveryUnavailable { service: &'static str },
    #[error("selected font sources contain no usable font faces")]
    EmptyFontSet,
    #[error("font source {path}: {message}")]
    Source { path: PathBuf, message: String },
    #[error("native font limit {resource} exceeded: {actual} > {limit}")]
    Limit {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error(transparent)]
    Operation(#[from] OperationLedgerError),
    #[error("native font measurement: {0}")]
    Measurement(String),
    #[error("the selected fonts cannot shape this label")]
    MissingGlyphCoverage,
}

impl NativeFontError {
    /// Stable resource ID, first rejected total, and configured ceiling for a quota failure.
    pub fn resource_limit(&self) -> Option<(&'static str, u64, u64)> {
        let Self::Limit {
            resource,
            actual,
            limit,
        } = self
        else {
            return None;
        };
        let id = match *resource {
            "font-bytes" | "system-font-bytes" => "native_font_bytes",
            "label-bytes" => "native_font_label_bytes",
            "measurements" => "native_font_measurements",
            _ => "native_font_resources",
        };
        Some((id, *actual as u64, *limit as u64))
    }
}

/// Shared font bytes and selection policy for text measurement and native export.
/// Fonts are not embedded in SVG output. Missing glyph coverage is declined by default;
/// [`Self::with_strict_glyph_coverage`] makes it an operation error.
#[derive(Clone)]
pub struct NativeFontContext(Arc<FontContextInner>);

/// Operation-local evidence emitted by the exact resolver installed into usvg.
///
/// This tracks the admission provenance for the exact font database used by one export parse.
/// Initial context faces are caller-owned; the discovery resolver records later admissions.
#[derive(Clone)]
pub(crate) struct NativeExportFontObserver {
    admitted_faces: Arc<Mutex<BTreeMap<usvg::fontdb::ID, NativeExportFontSource>>>,
    family_fallback: Arc<AtomicBool>,
    glyph_fallback: Arc<AtomicBool>,
    unresolved_font: Arc<AtomicBool>,
    unresolved_glyph: Arc<AtomicBool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeExportFontSource {
    Caller,
    System,
}

impl NativeExportFontObserver {
    fn new(database: &usvg::fontdb::Database) -> Self {
        Self {
            admitted_faces: Arc::new(Mutex::new(
                database
                    .faces()
                    .map(|face| (face.id, NativeExportFontSource::Caller))
                    .collect(),
            )),
            family_fallback: Arc::new(AtomicBool::new(false)),
            glyph_fallback: Arc::new(AtomicBool::new(false)),
            unresolved_font: Arc::new(AtomicBool::new(false)),
            unresolved_glyph: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn admit(&self, id: usvg::fontdb::ID, source: NativeExportFontSource) {
        if let Ok(mut faces) = self.admitted_faces.lock() {
            // A system catalog can point at an already-explicit face. Its caller ownership wins.
            faces.entry(id).or_insert(source);
        }
    }

    pub(crate) fn admitted_faces(
        &self,
    ) -> Option<std::sync::MutexGuard<'_, BTreeMap<usvg::fontdb::ID, NativeExportFontSource>>> {
        self.admitted_faces.lock().ok()
    }

    pub(crate) fn mark_family_fallback(&self) {
        self.family_fallback.store(true, Ordering::Relaxed);
    }

    pub(crate) fn family_fallback_used(&self) -> bool {
        self.family_fallback.load(Ordering::Relaxed)
    }

    pub(crate) fn mark_glyph_fallback(&self) {
        self.glyph_fallback.store(true, Ordering::Relaxed);
    }

    pub(crate) fn glyph_fallback_used(&self) -> bool {
        self.glyph_fallback.load(Ordering::Relaxed)
    }

    pub(crate) fn mark_unresolved_font(&self) {
        self.unresolved_font.store(true, Ordering::Relaxed);
    }

    pub(crate) fn unresolved_font_request(&self) -> bool {
        self.unresolved_font.load(Ordering::Relaxed)
    }

    pub(crate) fn mark_unresolved_glyph(&self) {
        self.unresolved_glyph.store(true, Ordering::Relaxed);
    }

    // Lookup misses are only coverage evidence on parsers without final omission reporting.
    pub(crate) fn unresolved_glyph_fallback(&self) -> bool {
        self.unresolved_glyph.load(Ordering::Relaxed)
    }
}

impl PartialEq for NativeFontContext {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for NativeFontContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeFontContext")
            .field("identity", &self.identity())
            .finish()
    }
}

struct FontContextInner {
    database: Arc<usvg::fontdb::Database>,
    selection: Arc<FontSelectionPolicy>,
    #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
    discovery: Option<resolver::DiscoveryProvider>,
    #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
    sources: registry::FontSourceRegistry,
    default_family: String,
    language: Option<String>,
    identity: String,
    limits: NativeFontLimits,
    control: OperationControl,
    state: Mutex<MeasurementState>,
    fatal: OnceLock<NativeFontError>,
}

#[derive(Default)]
struct MeasurementState {
    strict_glyph_coverage: bool,
    // None records proven missing glyph coverage for the same complete probe and font context.
    cache: BTreeMap<String, Option<NativeMetrics>>,
    cache_bytes: usize,
    measurements: usize,
    selected_faces: BTreeSet<String>,
}

#[derive(Clone, Copy, Default)]
struct NativeMetrics {
    advance: f64,
    width: f64,
    height: f64,
    x: f64,
    ink_x: f64,
    ink_right: f64,
    raw_y: f64,
}

struct ProbeSemantics {
    preserve: bool,
    tspan_preserve: bool,
    baseline: Option<TextMeasurementOperation>,
}

impl NativeFontContext {
    /// Sets the default BCP 47 language for native measurements and untagged SVG text.
    /// An empty string restores the platform preference. SVG `xml:lang` overrides
    /// this default when the renderer supports language-aware shaping.
    #[must_use]
    pub fn with_language(self, language: impl Into<String>) -> Self {
        let mut inner = self.operation_inner(&self.0.control);
        let language = language.into();
        inner.language = (!language.is_empty()).then_some(language);
        inner.update_policy_identity(self.strict_glyph_coverage());
        Self(Arc::new(inner))
    }

    /// Admits owned OpenType buffers in caller order without filesystem or system discovery.
    ///
    /// Files and buffers share byte validation, deduplication, face ceilings, and identity
    /// ordering. `max_files` counts buffers and `max_bytes` charges all input bytes before
    /// deduplication. Earlier equal family/style descriptors win. Malformed buffers report
    /// [`NativeFontError::Source`] with a synthetic `<memory-font-N>` diagnostic path.
    pub fn from_font_bytes(
        fonts: impl IntoIterator<Item = Vec<u8>>,
        limits: NativeFontLimits,
        control: &OperationControl,
    ) -> Result<Self, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Admission)?;
        let mut loader = FontLoader::new(limits, control.clone());
        let mut fonts = fonts.into_iter();
        loop {
            loader.checkpoint()?;
            let Some(bytes) = fonts.next() else {
                break;
            };
            loader.begin_source()?;
            let path = PathBuf::from(format!("<memory-font-{}>", loader.files));
            let admitted = loader.admit_bytes(bytes, &path)?;
            #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
            if let Some(bytes) = admitted {
                loader.initial_sources.insert(path, bytes);
            }
            #[cfg(not(all(feature = "system-fonts", not(target_family = "wasm"))))]
            let _ = admitted;
        }
        loader.finish(
            false,
            control,
            #[cfg(all(
                feature = "experimental-system-font-query",
                not(target_family = "wasm")
            ))]
            None,
        )
    }

    /// Starts an independent operation scope with the selected glyph coverage policy.
    ///
    /// Strict contexts return an error instead of declining unsupported shaping requests.
    /// Native export with registry usvg 0.47 conservatively rejects resolver misses because
    /// that parser cannot distinguish fully omitted glyphs from speculative lookups.
    /// Font bytes and selection order are shared; caches, counters, and failures are reset.
    /// Both [`Self::for_operation`] and [`Self::for_export`] preserve this policy.
    #[must_use]
    pub fn with_strict_glyph_coverage(self, enabled: bool) -> Self {
        let mut inner = self.operation_inner(&self.0.control);
        inner.state = Mutex::new(MeasurementState {
            strict_glyph_coverage: enabled,
            ..Default::default()
        });
        inner.update_policy_identity(enabled);
        Self(Arc::new(inner))
    }

    pub(crate) fn strict_glyph_coverage(&self) -> bool {
        self.0
            .state
            .lock()
            .map(|state| state.strict_glyph_coverage)
            .unwrap_or(true)
    }

    /// Names and indices within admitted font data selected by successful shaping probes.
    /// A standalone representation of a system collection face has data index zero.
    /// This does not describe deterministic fallback or other painting operations.
    pub fn selected_faces(&self) -> Vec<String> {
        self.0
            .state
            .lock()
            .map(|state| state.selected_faces.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Compatibility alias for the formerly title-only backend.
    pub fn selected_title_faces(&self) -> Vec<String> {
        self.selected_faces()
    }

    /// Context policy and initially admitted bytes. Platform discovery can also
    /// contribute the default family name. Later snapshots aren't fingerprinted here.
    pub fn identity(&self) -> &str {
        &self.0.identity
    }

    pub fn face_count(&self) -> usize {
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        if let Some(discovery) = &self.0.discovery {
            return discovery
                .catalog_face_count()
                .unwrap_or_else(|| self.0.sources.admitted_faces());
        }
        self.0.database.faces().count()
    }

    pub fn apply_to_usvg_options(&self, options: &mut usvg::Options<'_>) {
        self.apply_to_usvg_options_controlled(options, &self.0.control);
    }

    pub(crate) fn apply_to_usvg_options_controlled(
        &self,
        options: &mut usvg::Options<'_>,
        control: &OperationControl,
    ) {
        options.fontdb = Arc::clone(&self.0.database);
        options.font_family.clone_from(&self.0.default_family);
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        if self.0.discovery.is_some() {
            resolver::install_resolver(options, self.clone(), control.clone(), None);
            return;
        }
        options.font_resolver = super::browser_like_font_resolver(Arc::clone(&self.0.selection));
        options.set_text_fallback(self.0.language.clone(), None);
        #[cfg(not(all(feature = "system-fonts", not(target_family = "wasm"))))]
        let _ = control;
    }

    /// Installs this context once for export and returns evidence from that same resolver.
    pub(crate) fn apply_to_usvg_options_for_export(
        &self,
        options: &mut usvg::Options<'_>,
        control: &OperationControl,
    ) -> NativeExportFontObserver {
        let observer = NativeExportFontObserver::new(&self.0.database);
        options.fontdb = Arc::clone(&self.0.database);
        options.font_family.clone_from(&self.0.default_family);
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        if self.0.discovery.is_some() {
            resolver::install_resolver(
                options,
                self.clone(),
                control.clone(),
                Some(observer.clone()),
            );
            return observer;
        }
        options.font_resolver =
            native_export_font_resolver(observer.clone(), Arc::clone(&self.0.selection));
        options.set_text_fallback(self.0.language.clone(), None);
        #[cfg(not(all(feature = "system-fonts", not(target_family = "wasm"))))]
        let _ = control;
        observer
    }

    /// Shares the exact database installed into an export's usvg options with its receipt.
    /// The caller must still install the matching resolver through
    /// [`Self::apply_to_usvg_options_controlled`]; this accessor exists only so the receipt can
    /// classify faces from that same parse, never from a second font database.
    pub(crate) fn fontdb_for_export(&self) -> Arc<usvg::fontdb::Database> {
        Arc::clone(&self.0.database)
    }

    /// Reuses font metadata without inheriting operation failures or measurement counts.
    pub fn for_operation(&self, control: &OperationControl) -> Self {
        Self(Arc::new(self.operation_inner(control)))
    }

    fn operation_inner(&self, control: &OperationControl) -> FontContextInner {
        FontContextInner {
            database: Arc::clone(&self.0.database),
            selection: Arc::clone(&self.0.selection),
            #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
            discovery: self
                .0
                .discovery
                .as_ref()
                .map(resolver::DiscoveryProvider::for_operation),
            #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
            sources: self.0.sources.for_operation(),
            default_family: self.0.default_family.clone(),
            language: self.0.language.clone(),
            identity: self.0.identity.clone(),
            limits: self.0.limits.clone(),
            control: control.clone(),
            state: Mutex::new(MeasurementState {
                strict_glyph_coverage: self.strict_glyph_coverage(),
                ..Default::default()
            }),
            fatal: OnceLock::new(),
        }
    }

    /// Starts an independently controlled export while retaining already admitted font bytes.
    /// Source snapshots and their acquisition charges are preserved; later export failures do
    /// not poison the source context. Existing fatal font errors are replayed before forking.
    pub fn for_export(&self, control: &OperationControl) -> Result<Self, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Export)?;
        if let Some(error) = self.0.fatal.get() {
            return Err(error.clone());
        }
        let context = Self(Arc::new(FontContextInner {
            database: Arc::clone(&self.0.database),
            selection: Arc::clone(&self.0.selection),
            #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
            discovery: self.0.discovery.clone(),
            #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
            sources: self.0.sources.for_export()?,
            default_family: self.0.default_family.clone(),
            language: self.0.language.clone(),
            identity: self.0.identity.clone(),
            limits: self.0.limits.clone(),
            control: control.clone(),
            state: Mutex::new(MeasurementState {
                strict_glyph_coverage: self.strict_glyph_coverage(),
                ..Default::default()
            }),
            fatal: OnceLock::new(),
        }));
        control.terminal_checkpoint_at(OperationPhase::Export)?;
        Ok(context)
    }

    /// Whether the explicitly authorized font set includes host-system discovery.
    pub fn uses_system_fonts(&self) -> bool {
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        {
            self.0.discovery.is_some()
        }
        #[cfg(not(all(feature = "system-fonts", not(target_family = "wasm"))))]
        {
            false
        }
    }

    /// Font bytes retained by the context, excluding temporary discovery buffers.
    pub fn resident_font_bytes(&self) -> usize {
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        {
            self.0.sources.resident_bytes()
        }
        #[cfg(not(all(feature = "system-fonts", not(target_family = "wasm"))))]
        {
            binary_bytes(self.0.database.as_ref())
        }
    }

    /// Replay this context's fatal outcome at every fallible rendering/export boundary.
    pub fn check(&self) -> Result<(), NativeFontError> {
        if let Some(error) = self.0.fatal.get() {
            return Err(error.clone());
        }
        self.0
            .control
            .terminal_checkpoint_at(OperationPhase::Layout)?;
        Ok(())
    }
}

impl Eq for NativeFontContext {}

impl FontContextInner {
    fn update_policy_identity(&mut self, strict: bool) {
        // Acquisition produces a hex digest. Mode-specific policy is appended after it.
        if let Some(end) = self.identity.find('/') {
            self.identity.truncate(end);
        }
        if let Some(language) = &self.language {
            use std::fmt::Write;
            self.identity.push_str("/font-language-v1=");
            for byte in Sha256::digest(language.as_bytes()) {
                let _ = write!(self.identity, "{byte:02x}");
            }
        }
        if strict {
            self.identity.push_str("/strict-glyph-coverage-v1");
        }
    }
}

type ClusterFallbackFn<'a> = Box<
    dyn Fn(
            &usvg::Font,
            &str,
            Option<&str>,
            bool,
            &[usvg::fontdb::ID],
            &mut Arc<usvg::fontdb::Database>,
        ) -> Option<(usvg::fontdb::ID, f32)>
        + Send
        + Sync
        + 'a,
>;

// Registry consumers use the public scalar resolver. In workspace builds the
// inherent usvg method takes precedence and installs the contextual callback.
#[allow(dead_code)]
trait TextFallbackOptions<'a> {
    fn set_text_fallback(
        &mut self,
        _language: Option<String>,
        _select: Option<ClusterFallbackFn<'a>>,
    ) {
    }
}

impl<'a> TextFallbackOptions<'a> for usvg::Options<'a> {}

#[cfg(all(
    feature = "experimental-system-font-query",
    not(target_family = "wasm")
))]
type FontInstance = (usvg::fontdb::ID, f32, Vec<usvg::FontVariation>);

#[cfg(all(
    feature = "experimental-system-font-query",
    not(target_family = "wasm")
))]
type InstanceSelection<'a> = Box<
    dyn Fn(&usvg::Font, &mut Arc<usvg::fontdb::Database>) -> Option<FontInstance>
        + Send
        + Sync
        + 'a,
>;

#[cfg(all(
    feature = "experimental-system-font-query",
    not(target_family = "wasm")
))]
type InstanceFallbackSelection<'a> = Box<
    dyn Fn(
            &usvg::Font,
            &str,
            Option<&str>,
            bool,
            &[FontInstance],
            &mut Arc<usvg::fontdb::Database>,
        ) -> Option<FontInstance>
        + Send
        + Sync
        + 'a,
>;

#[cfg(all(
    feature = "experimental-system-font-query",
    not(target_family = "wasm")
))]
#[allow(dead_code)]
trait FontInstanceOptions<'a> {
    fn supports_font_instances(&self) -> bool {
        false
    }

    fn set_font_instances(
        &mut self,
        _select: Option<InstanceSelection<'a>>,
        _fallback: Option<InstanceFallbackSelection<'a>>,
    ) {
    }
}

#[cfg(all(
    feature = "experimental-system-font-query",
    not(target_family = "wasm")
))]
impl<'a> FontInstanceOptions<'a> for usvg::Options<'a> {}

// Published usvg lacks the workspace hook for language-aware primary selection.
#[cfg(feature = "experimental-system-font-query")]
#[allow(dead_code)]
trait FontSelectionLanguage {
    fn selection_language(&self) -> Option<&str> {
        None
    }
}

#[cfg(feature = "experimental-system-font-query")]
impl FontSelectionLanguage for usvg::Font {}

fn native_export_font_resolver(
    observer: NativeExportFontObserver,
    selection: Arc<FontSelectionPolicy>,
) -> usvg::FontResolver<'static> {
    let select_observer = observer.clone();
    let fallback_observer = observer.clone();
    let fallback_selection = Arc::clone(&selection);
    usvg::FontResolver {
        select_font: Box::new(move |font, fontdb| {
            if let Some(id) = selection.requested(font, fontdb.as_ref()) {
                return Some(id);
            }
            let selected = selection.fallback(font, fontdb.as_ref());
            if selected.is_some() {
                select_observer.mark_family_fallback();
            } else {
                select_observer.mark_unresolved_font();
            }
            selected
        }),
        select_fallback: Box::new(move |character, excluded, fontdb| {
            let selected = fallback_selection.glyph_fallback(character, excluded, fontdb);
            if selected.is_some() {
                fallback_observer.mark_glyph_fallback();
            } else {
                fallback_observer.mark_unresolved_glyph();
            }
            selected
        }),
    }
}

struct FontLoader {
    database: usvg::fontdb::Database,
    selection: FontSelectionPolicy,
    limits: NativeFontLimits,
    control: OperationControl,
    files: usize,
    #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
    initial_sources: BTreeMap<PathBuf, Arc<Vec<u8>>>,
    #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
    source_aliases: BTreeMap<PathBuf, PathBuf>,
    bytes: usize,
    #[cfg(not(target_family = "wasm"))]
    entries: usize,
    // Sources own their bytes. A digest also remembers duplicates with no surviving face.
    data_by_digest: BTreeMap<Vec<u8>, Weak<Vec<u8>>>,
    descriptors: BTreeSet<String>,
    hash: Sha256,
}

impl FontLoader {
    fn finish(
        self,
        system_fonts: bool,
        control: &OperationControl,
        #[cfg(all(
            feature = "experimental-system-font-query",
            not(target_family = "wasm")
        ))]
        platform: Option<Arc<platform::PlatformCatalog>>,
    ) -> Result<NativeFontContext, NativeFontError> {
        let mut loader = self;
        #[cfg(not(all(feature = "system-fonts", not(target_family = "wasm"))))]
        let _ = system_fonts;
        #[cfg(all(
            feature = "experimental-system-font-query",
            not(target_family = "wasm")
        ))]
        let platform_requested = platform.is_some();
        #[cfg(all(
            feature = "system-fonts",
            not(target_family = "wasm"),
            not(feature = "experimental-system-font-query")
        ))]
        let platform_requested = false;
        loader.selection.configure_generics(&mut loader.database);
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        let system = if system_fonts && !platform_requested {
            Some(system::SystemCatalog::discover(&loader, control)?)
        } else {
            None
        };
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        let metadata = system
            .as_ref()
            .map_or(&loader.database, |catalog| catalog.database.as_ref());
        #[cfg(not(all(feature = "system-fonts", not(target_family = "wasm"))))]
        let metadata = &loader.database;
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        let selection = system
            .as_ref()
            .map_or(&loader.selection, |catalog| &catalog.selection);
        #[cfg(not(all(feature = "system-fonts", not(target_family = "wasm"))))]
        let selection = &loader.selection;
        // A symbol-only context remains usable through an explicit family name.
        let default_family = metadata
            .faces()
            .next()
            .map(|_| selection.default_family(metadata).unwrap_or_default());
        #[cfg(all(
            feature = "experimental-system-font-query",
            not(target_family = "wasm")
        ))]
        let default_family = match (default_family, platform.as_ref()) {
            (None, Some(catalog)) => {
                let family = catalog
                    .default_family(control)?
                    .ok_or(NativeFontError::EmptyFontSet)?;
                loader.hash.update(b"platform-default-family\0");
                loader.hash.update(family.as_bytes());
                // usvg passes an empty default as a Named("") request. Let the platform
                // resolver apply the generic-family policy for that implicit request.
                Some(String::new())
            }
            (name, _) => name,
        };
        let default_family = default_family.ok_or(NativeFontError::EmptyFontSet)?;
        loader
            .hash
            .update(b"merman-native-text-v3/usvg-0.47/resolver-v3");
        #[cfg(all(
            feature = "experimental-system-font-query",
            not(target_family = "wasm")
        ))]
        if platform.is_some() {
            loader.hash.update(b"\0platform-query/native/v7");
        }
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        if let Some(catalog) = &system {
            loader.hash.update(&catalog.identity);
        }
        let identity = loader
            .hash
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        control.terminal_checkpoint_at(OperationPhase::Admission)?;
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        let discovery =
            system.map(|catalog| resolver::DiscoveryProvider::Bounded(Arc::new(catalog)));
        let database = Arc::new(loader.database);
        #[cfg(all(
            feature = "experimental-system-font-query",
            not(target_family = "wasm")
        ))]
        let discovery = platform
            .map(|catalog| resolver::DiscoveryProvider::Platform {
                catalog,
                explicit: Arc::clone(&database),
            })
            .or(discovery);
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        let sources = registry::FontSourceRegistry::new(
            loader.initial_sources,
            loader.source_aliases,
            loader.files,
            database.faces().count(),
        );
        Ok(NativeFontContext(Arc::new(FontContextInner {
            database,
            selection: Arc::new(loader.selection),
            #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
            discovery,
            #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
            sources,
            default_family,
            language: None,
            identity,
            limits: loader.limits,
            control: control.clone(),
            state: Mutex::new(MeasurementState::default()),
            fatal: OnceLock::new(),
        })))
    }

    fn new(limits: NativeFontLimits, control: OperationControl) -> Self {
        Self {
            database: usvg::fontdb::Database::new(),
            selection: FontSelectionPolicy::default(),
            limits,
            control,
            files: 0,
            #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
            initial_sources: BTreeMap::new(),
            #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
            source_aliases: BTreeMap::new(),
            bytes: 0,
            #[cfg(not(target_family = "wasm"))]
            entries: 0,
            data_by_digest: BTreeMap::new(),
            descriptors: BTreeSet::new(),
            hash: Sha256::new(),
        }
    }

    fn checkpoint(&self) -> Result<(), NativeFontError> {
        self.control
            .terminal_checkpoint_at(OperationPhase::Admission)?;
        Ok(())
    }

    fn begin_source(&mut self) -> Result<(), NativeFontError> {
        self.checkpoint()?;
        self.files = self.files.saturating_add(1);
        check_limit("files", self.files, self.limits.max_files)
    }

    #[cfg(not(target_family = "wasm"))]
    fn directory(&mut self, path: &Path, depth: usize) -> Result<(), NativeFontError> {
        self.checkpoint()?;
        check_limit("directory-depth", depth, self.limits.max_directory_depth)?;
        let metadata = fs::symlink_metadata(path).map_err(|e| source_error(path, e))?;
        if metadata.is_symlink() || !metadata.is_dir() {
            return Err(source_error(path, "expected a directory, not a symlink"));
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(path).map_err(|e| source_error(path, e))? {
            self.checkpoint()?;
            self.entries = self.entries.saturating_add(1);
            check_limit(
                "directory-entries",
                self.entries,
                self.limits.max_directory_entries,
            )?;
            entries.push(entry.map_err(|e| source_error(path, e))?);
        }
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            self.checkpoint()?;
            let child = entry.path();
            let kind = entry.file_type().map_err(|e| source_error(&child, e))?;
            // Never follow discovered symlinks, including cycles outside the supplied root.
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                self.directory(&child, depth.saturating_add(1))?;
            } else if kind.is_file() && is_font_path(&child) {
                self.file(&child)?;
            }
        }
        Ok(())
    }

    #[cfg(not(target_family = "wasm"))]
    fn file(&mut self, path: &Path) -> Result<(), NativeFontError> {
        self.checkpoint()?;
        let metadata = fs::symlink_metadata(path).map_err(|e| source_error(path, e))?;
        if metadata.is_symlink() || !metadata.is_file() {
            return Err(source_error(path, "expected a regular file, not a symlink"));
        }
        self.begin_source()?;
        let budget = FontByteBudget::new("font-bytes", self.bytes, self.limits.max_bytes);
        budget.check(metadata.len())?;
        let mut file = fs::File::open(path).map_err(|e| source_error(path, e))?;
        let mut bytes = Vec::new();
        let mut chunk = [0; 16 * 1024];
        loop {
            self.checkpoint()?;
            let count = file.read(&mut chunk).map_err(|e| source_error(path, e))?;
            if count == 0 {
                break;
            }
            budget.check((bytes.len() as u64).saturating_add(count as u64))?;
            bytes.extend_from_slice(&chunk[..count]);
        }
        let admitted = self.admit_bytes(bytes, path)?;
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        if let Some(bytes) = admitted {
            let key = fs::canonicalize(path).map_err(|error| source_error(path, error))?;
            if self
                .source_aliases
                .get(path)
                .is_some_and(|first| first != &key)
                || self
                    .initial_sources
                    .get(&key)
                    .is_some_and(|first| !Arc::ptr_eq(first, &bytes))
            {
                return Err(source_error(
                    path,
                    "explicit font changed between reads; retry the operation",
                ));
            }
            self.source_aliases.insert(path.to_owned(), key.clone());
            self.initial_sources.insert(key, bytes);
        }
        #[cfg(not(feature = "system-fonts"))]
        let _ = admitted;
        Ok(())
    }

    fn admit_bytes(
        &mut self,
        bytes: Vec<u8>,
        path: &Path,
    ) -> Result<Option<Arc<Vec<u8>>>, NativeFontError> {
        self.checkpoint()?;
        FontByteBudget::new("font-bytes", self.bytes, self.limits.max_bytes)
            .check(bytes.len() as u64)?;
        self.bytes += bytes.len();
        let digest = Sha256::digest(&bytes).to_vec();
        self.checkpoint()?;
        if let Some(previous) = self.data_by_digest.get(&digest) {
            return Ok(previous.upgrade());
        }
        // Check collection cardinality before fontdb allocates or parses all its faces.
        let faces = rustybuzz::ttf_parser::fonts_in_collection(&bytes).unwrap_or(1) as usize;
        check_limit(
            "font-faces",
            self.database.faces().count().saturating_add(faces),
            self.limits.max_faces,
        )?;
        if bytes.starts_with(b"wOFF") || bytes.starts_with(b"wOF2") {
            return Err(source_error(
                path,
                "WOFF/WOFF2 web font detected. Supply a TTF or OTF version of the same font for native rendering.",
            ));
        }
        // fontdb admits metadata-only faces; shaping also requires valid metrics tables.
        for index in 0..faces {
            self.checkpoint()?;
            rustybuzz::ttf_parser::Face::parse(&bytes, index as u32).map_err(|error| {
                source_error(path, format!("invalid OpenType face {index}: {error:?}"))
            })?;
        }
        let mut candidate = usvg::fontdb::Database::new();
        let bytes = Arc::new(bytes);
        candidate.load_font_source(usvg::fontdb::Source::Binary(bytes.clone()));
        self.checkpoint()?;
        if candidate.faces().next().is_none() {
            return Err(source_error(path, "no usable OpenType font faces"));
        }
        for face in candidate.faces() {
            self.checkpoint()?;
            // Earlier explicitly supplied faces win equal family/style descriptors.
            if self.descriptors.insert(face_descriptor(face)) {
                self.hash.update(&digest);
                self.hash.update(face.index.to_be_bytes());
                let id = self.database.push_face_info(face.clone());
                self.selection.admit(id, &candidate, face.id);
            }
        }
        self.data_by_digest.insert(digest, Arc::downgrade(&bytes));
        Ok(Some(bytes))
    }
}

fn face_descriptor(face: &usvg::fontdb::FaceInfo) -> String {
    let mut families: Vec<_> = face
        .families
        .iter()
        .map(|(name, _)| name.to_lowercase())
        .collect();
    families.sort();
    families.dedup();
    format!(
        "{families:?}/{:?}/{:?}/{}",
        face.style, face.stretch, face.weight.0
    )
}

#[cfg(not(target_family = "wasm"))]
fn is_font_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["ttf", "otf", "ttc", "otc"]
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}

#[cfg(any(not(all(feature = "system-fonts", not(target_family = "wasm"))), test))]
fn binary_bytes(database: &usvg::fontdb::Database) -> usize {
    let mut sources = BTreeSet::new();
    database
        .faces()
        .filter_map(|face| match &face.source {
            usvg::fontdb::Source::Binary(data) => {
                let key = Arc::as_ptr(data) as *const () as usize;
                sources.insert(key).then(|| data.as_ref().as_ref().len())
            }
            #[allow(unreachable_patterns)]
            _ => None,
        })
        .sum()
}

fn source_error(path: &Path, error: impl fmt::Display) -> NativeFontError {
    NativeFontError::Source {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

fn limit_error(resource: &'static str, actual: usize, limit: usize) -> NativeFontError {
    NativeFontError::Limit {
        resource,
        actual,
        limit,
    }
}

/// Checks a growing source against the remaining allowance, reporting operation-wide totals.
#[derive(Clone, Copy)]
struct FontByteBudget {
    resource: &'static str,
    used: usize,
    maximum: usize,
}

impl FontByteBudget {
    fn new(resource: &'static str, used: usize, maximum: usize) -> Self {
        Self {
            resource,
            used,
            maximum,
        }
    }

    fn remaining(self) -> usize {
        self.maximum.saturating_sub(self.used)
    }

    fn check(self, source_bytes: u64) -> Result<(), NativeFontError> {
        if source_bytes > self.remaining() as u64 {
            let actual = self
                .used
                .saturating_add(usize::try_from(source_bytes).unwrap_or(usize::MAX));
            return Err(limit_error(self.resource, actual, self.maximum));
        }
        Ok(())
    }
}

fn check_limit(resource: &'static str, actual: usize, limit: usize) -> Result<(), NativeFontError> {
    if actual > limit {
        Err(limit_error(resource, actual, limit))
    } else {
        Ok(())
    }
}

impl HostTextMeasurer for NativeFontContext {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        self.measure_with_control(request, &self.0.control)
    }

    fn measure_with_control(
        &self,
        request: HostTextMeasurementRequest<'_>,
        control: &OperationControl,
    ) -> HostMeasurementResult {
        match self.measure_operation(request, control) {
            Ok(result) => Ok(Some(result)),
            Err(NativeFontError::MissingGlyphCoverage) if !self.strict_glyph_coverage() => Ok(None),
            Err(error) => {
                let error = self.record_failure(control, false, error);
                Err(HostTextMeasurementError::new(error.to_string()))
            }
        }
    }
}

impl NativeFontContext {
    fn record_failure(
        &self,
        control: &OperationControl,
        exporting: bool,
        error: NativeFontError,
    ) -> &NativeFontError {
        self.0.fatal.get_or_init(|| {
            let Some((id, actual, maximum)) = error.resource_limit() else {
                return error;
            };
            let (phase, resource_phase, provenance) = if exporting {
                (
                    OperationPhase::Export,
                    "native-font-export",
                    OperationResourceProvenance::new(OperationResourceDomain::Export, None, []),
                )
            } else {
                (
                    OperationPhase::Layout,
                    "native-font-measurement",
                    OperationResourceProvenance::new(
                        OperationResourceDomain::Render,
                        None,
                        [OperationResourceOverride { id, value: maximum }],
                    ),
                )
            };
            let event = OperationResourceLimitExceeded {
                id,
                phase,
                resource_phase,
                limit: maximum,
                consumed: 0,
                requested: actual,
                provenance,
            };
            let terminal = control.terminate_resource_limit(event.clone());
            if terminal == OperationLedgerError::ResourceLimitExceeded(event) {
                error
            } else {
                NativeFontError::Operation(terminal)
            }
        })
    }

    fn measure_operation(
        &self,
        request: HostTextMeasurementRequest<'_>,
        control: &OperationControl,
    ) -> Result<HostTextMeasurement, NativeFontError> {
        use TextMeasurementOperation as Op;
        self.checkpoint(control)?;
        let style = request.style;
        self.check_label(request.text, style)?;
        if !(style.font_size.is_finite() && style.font_size >= 0.0) {
            return Err(NativeFontError::MissingGlyphCoverage);
        }
        let result = match request.operation {
            Op::Measure | Op::Wrapped | Op::WrappedWithRawWidth => {
                let (mut layout, line_metrics) = try_measure_wrapped_text_with_line_metrics(
                    request.text,
                    style,
                    request.max_width,
                    request.wrap_mode,
                    |line, style| {
                        let metrics = self.shape_with_space(
                            line,
                            style,
                            control,
                            request.wrap_mode == WrapMode::HtmlLike,
                        )?;
                        Ok::<_, NativeFontError>((metrics.advance, metrics))
                    },
                )?;
                layout.metrics.height = match request.wrap_mode {
                    WrapMode::HtmlLike => layout.metrics.line_count as f64 * style.font_size * 1.5,
                    WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => {
                        // Mermaid emits one outer tspan per row. Use the emitted rows' actual
                        // font bbox height; subsequent row baselines advance by dy=1.1em.
                        let mut top = f64::INFINITY;
                        let mut bottom = f64::NEG_INFINITY;
                        for (index, metrics) in line_metrics.iter().enumerate() {
                            if metrics.height > 0.0 {
                                let baseline = index as f64 * style.font_size * 1.1;
                                top = top.min(baseline + metrics.raw_y);
                                bottom = bottom.max(baseline + metrics.raw_y + metrics.height);
                            }
                        }
                        if top.is_finite() {
                            (bottom - top).max(0.0)
                        } else {
                            0.0
                        }
                    }
                };
                if request.operation == Op::WrappedWithRawWidth {
                    HostTextMeasurement::WrappedWithRawWidth {
                        metrics: layout.metrics,
                        raw_width: layout.raw_width,
                    }
                } else {
                    HostTextMeasurement::Metrics(layout.metrics)
                }
            }
            operation => {
                // drawSimpleText replaces explicit HTML breaks before creating its tspan.
                let simple_text;
                let text = if matches!(
                    operation,
                    Op::SimpleBBoxWidth | Op::SimpleBBoxHeight | Op::MermaidCalculateTextDimensions
                ) {
                    simple_text = split_html_br_lines(request.text).join(" ");
                    simple_text.as_str()
                } else {
                    request.text
                };
                let baseline = matches!(
                    operation,
                    Op::CreateTextBBoxYOffset
                        | Op::CreateTextMiddleBBoxYOffset
                        | Op::RawMiddleBBoxYOffset
                        | Op::RawHangingBBoxYOffset
                )
                .then_some(operation);
                let metrics = self.shape_probe(
                    text,
                    style,
                    control,
                    operation == Op::CanvasMeasureTextWidth
                        || request.wrap_mode == WrapMode::HtmlLike,
                    matches!(operation, Op::TspanBBoxWidth | Op::TspanBBoxHeight)
                        && request.wrap_mode == WrapMode::HtmlLike,
                    baseline,
                )?;
                match operation {
                    Op::ComputedLength | Op::CanvasMeasureTextWidth => {
                        HostTextMeasurement::Length(metrics.advance)
                    }
                    Op::BBoxX | Op::BBoxXWithAsciiOverhang | Op::TitleBBoxX => {
                        let half = metrics.advance / 2.0;
                        let (x, right) = if operation == Op::BBoxX {
                            (metrics.x, metrics.x + metrics.width)
                        } else {
                            (
                                metrics.x.min(metrics.ink_x),
                                (metrics.x + metrics.width).max(metrics.ink_right),
                            )
                        };
                        HostTextMeasurement::HorizontalExtents {
                            left: (half - x).max(0.0),
                            right: (right - half).max(0.0),
                        }
                    }
                    Op::SimpleBBoxWidth
                    | Op::RawBBoxWidth
                    | Op::TspanBBoxWidth
                    | Op::WrapProbeBBoxWidth
                    | Op::BoundingClientRectWidth => HostTextMeasurement::Length(metrics.width),
                    Op::HtmlLineHeight => {
                        // Empty or collapsed whitespace does not create an HTML inline line box.
                        let empty = request
                            .text
                            .bytes()
                            .all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0c));
                        HostTextMeasurement::Length(if empty { 0.0 } else { style.font_size * 1.5 })
                    }
                    Op::SimpleBBoxHeight | Op::RawBBoxHeight | Op::TspanBBoxHeight => {
                        HostTextMeasurement::Length(metrics.height)
                    }
                    Op::CreateTextBBoxYOffset
                    | Op::CreateTextMiddleBBoxYOffset
                    | Op::RawBBoxYOffset
                    | Op::RawMiddleBBoxYOffset
                    | Op::RawHangingBBoxYOffset => HostTextMeasurement::Length(metrics.raw_y),
                    Op::MermaidCalculateTextDimensions => {
                        HostTextMeasurement::Metrics(merman_render::text::TextMetrics {
                            width: metrics.width,
                            height: metrics.height,
                            line_count: 1,
                        })
                    }
                    Op::Measure | Op::Wrapped | Op::WrappedWithRawWidth => unreachable!(),
                }
            }
        };
        self.checkpoint(control)?;
        Ok(result)
    }

    fn checkpoint(&self, control: &OperationControl) -> Result<(), NativeFontError> {
        self.check()?;
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        Ok(())
    }

    #[cfg(test)]
    fn shape_line(
        &self,
        text: &str,
        style: &TextStyle,
        control: &OperationControl,
    ) -> Result<NativeMetrics, NativeFontError> {
        self.shape_with_space(text, style, control, true)
    }

    fn shape_with_space(
        &self,
        text: &str,
        style: &TextStyle,
        control: &OperationControl,
        preserve: bool,
    ) -> Result<NativeMetrics, NativeFontError> {
        self.shape_probe(text, style, control, preserve, false, None)
    }

    fn shape_probe(
        &self,
        text: &str,
        style: &TextStyle,
        control: &OperationControl,
        preserve: bool,
        tspan_preserve: bool,
        baseline: Option<TextMeasurementOperation>,
    ) -> Result<NativeMetrics, NativeFontError> {
        self.checkpoint(control)?;
        self.check_label(text, style)?;
        // A valid zero font paints no glyphs. Do not resolve faces or report missing coverage;
        // the HTML caller still owns its parent strut and any visible sibling line boxes.
        if style.font_size == 0.0
            || text.is_empty()
            || (!preserve && text.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')))
        {
            return Ok(NativeMetrics::default());
        }
        let key = format!(
            "{:?}",
            (
                text,
                preserve,
                tspan_preserve,
                baseline,
                &style.font_family,
                style.font_size.to_bits(),
                &style.font_weight,
                &style.font_style,
                &self.0.language,
            )
        );
        let mut state = self.0.state.lock().map_err(|_| {
            NativeFontError::Measurement("measurement state is poisoned".to_owned())
        })?;
        self.checkpoint(control)?;
        if let Some(&metrics) = state.cache.get(&key) {
            return metrics.ok_or(NativeFontError::MissingGlyphCoverage);
        }
        state.measurements = state.measurements.saturating_add(1);
        check_limit(
            "measurements",
            state.measurements,
            self.0.limits.max_measurements,
        )?;
        let result = self.shape_uncached(
            text,
            style,
            control,
            &mut state,
            ProbeSemantics {
                preserve,
                tspan_preserve,
                baseline,
            },
        );
        let cacheable = match &result {
            Ok(metrics) => Some(Some(*metrics)),
            Err(NativeFontError::MissingGlyphCoverage) => Some(None),
            Err(_) => None,
        };
        if let Some(metrics) = cacheable {
            let bytes = key
                .len()
                .saturating_add(std::mem::size_of::<Option<NativeMetrics>>());
            if state.cache.len() < self.0.limits.max_cache_entries
                && bytes
                    <= self
                        .0
                        .limits
                        .max_cache_bytes
                        .saturating_sub(state.cache_bytes)
            {
                state.cache_bytes += bytes;
                state.cache.insert(key, metrics);
            }
        }
        result
    }

    fn check_label(&self, text: &str, style: &TextStyle) -> Result<(), NativeFontError> {
        let label_bytes = text
            .len()
            .saturating_add(style.font_family.as_deref().map_or(0, str::len))
            .saturating_add(style.font_weight.as_deref().map_or(0, str::len))
            .saturating_add(style.font_style.as_deref().map_or(0, str::len))
            .saturating_add(self.0.language.as_deref().map_or(0, str::len));
        check_limit("label-bytes", label_bytes, self.0.limits.max_label_bytes)?;
        Ok(())
    }

    fn shape_uncached(
        &self,
        text: &str,
        style: &TextStyle,
        control: &OperationControl,
        state: &mut MeasurementState,
        semantics: ProbeSemantics,
    ) -> Result<NativeMetrics, NativeFontError> {
        let ProbeSemantics {
            preserve,
            tspan_preserve,
            baseline,
        } = semantics;
        let whitespace_only = text.chars().all(char::is_whitespace);
        // usvg prunes text with neither outlines nor decorations. Retain whitespace probes
        // with a decoration; their metric bbox and shaped advance are unaffected.
        let decoration = if whitespace_only { "underline" } else { "none" };
        let space = if preserve && !tspan_preserve {
            "preserve"
        } else {
            "default"
        };
        let escaped = escape_xml(text);
        let payload = if tspan_preserve {
            format!(r#"<tspan xml:space="preserve">{escaped}</tspan>"#)
        } else {
            escaped
        };
        use TextMeasurementOperation as Op;
        let nodes = match baseline {
            None => format!(r#"<text id="start" x="0" y="0" text-anchor="start">{payload}</text>"#),
            Some(Op::CreateTextBBoxYOffset) => format!(
                r#"<text id="start" x="0" y="-10.1"><tspan y="-0.1em" dy="1.1em">{payload}</tspan></text>"#
            ),
            Some(Op::CreateTextMiddleBBoxYOffset) => format!(
                r#"<text id="start" x="0" y="-10.1" dominant-baseline="middle"><tspan y="-0.1em" dy="1.1em">{payload}</tspan></text>"#
            ),
            Some(Op::RawMiddleBBoxYOffset) => format!(
                r#"<text id="start" x="0" y="0" dominant-baseline="middle">{payload}</text>"#
            ),
            Some(Op::RawHangingBBoxYOffset) => format!(
                r#"<text id="start" x="0" y="0" dominant-baseline="hanging">{payload}</text>"#
            ),
            Some(_) => {
                return Err(NativeFontError::Measurement(
                    "invalid baseline probe operation".to_owned(),
                ));
            }
        };
        let weight = style
            .font_weight
            .as_deref()
            .map_or_else(String::new, |weight| {
                format!(r#" font-weight="{}""#, escape_xml(weight))
            });
        let slant = style
            .font_style
            .as_deref()
            .map_or_else(String::new, |style| {
                format!(r#" font-style="{}""#, escape_xml(style))
            });
        let svg = |nodes: &str| {
            format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><g font-family="{}" font-size="{}"{weight}{slant} xml:space="{space}" text-decoration="{decoration}">{nodes}</g></svg>"#,
                escape_xml(
                    style
                        .font_family
                        .as_deref()
                        .unwrap_or(&self.0.default_family)
                ),
                style.font_size,
            )
        };
        let mut options = usvg::Options::default();
        self.apply_to_usvg_options_controlled(&mut options, control);
        let (tree, end_x) = parse_text_probe(
            svg,
            &nodes,
            baseline.is_none().then_some(payload.as_str()),
            &options,
        )
        .map_err(|error| NativeFontError::Measurement(error.to_string()))?;
        if let Some(error) = self.0.fatal.get() {
            return Err(error.clone());
        }
        self.checkpoint(control)?;
        let Some(usvg::Node::Text(start)) = tree.node_by_id("start") else {
            return Err(NativeFontError::MissingGlyphCoverage);
        };
        if start
            .layouted()
            .iter()
            .flat_map(|span| &span.positioned_glyphs)
            .any(|glyph| glyph.id.0 == 0)
        {
            // Native glyph coverage is incomplete; the report records this operation's fallback.
            return Err(NativeFontError::MissingGlyphCoverage);
        }
        for glyph in start
            .layouted()
            .iter()
            .flat_map(|span| &span.positioned_glyphs)
        {
            if let Some(face) = tree.fontdb().face(glyph.font) {
                state
                    .selected_faces
                    .insert(format!("{}#{}", face.post_script_name, face.index));
            }
        }
        let metric = start.bounding_box();
        let ink = start.flattened().bounding_box();
        let advance = if baseline.is_none() {
            let end_x = end_x.ok_or_else(|| {
                NativeFontError::Measurement("missing end-anchor probe".to_owned())
            })?;
            f64::from(metric.x() - end_x).abs()
        } else {
            0.0
        };
        // usvg applies text anchors to the shaped cluster advance, including kerning and fallback.
        let metrics = NativeMetrics {
            advance,
            width: f64::from(metric.width()),
            height: f64::from(metric.height()),
            x: f64::from(metric.x()),
            ink_x: if whitespace_only {
                f64::from(metric.x())
            } else {
                f64::from(ink.x())
            },
            ink_right: if whitespace_only {
                f64::from(metric.right())
            } else {
                f64::from(ink.right())
            },
            raw_y: f64::from(metric.y()),
        };
        if ![
            metrics.advance,
            metrics.width,
            metrics.height,
            metrics.x,
            metrics.ink_x,
            metrics.ink_right,
            metrics.raw_y,
        ]
        .iter()
        .all(|v| v.is_finite())
        {
            return Err(NativeFontError::Measurement(
                "non-finite native text metrics".to_owned(),
            ));
        }
        self.checkpoint(control)?;
        Ok(metrics)
    }
}

type AnchorProbe = (usvg::Tree, Option<f32>);

// The workspace usvg provides an inherent method. A published Merman consumer can use the
// same code with the public usvg 0.47 API and its two original SVG anchors.
#[allow(dead_code, reason = "used when the consumer resolves unpatched usvg")]
trait RegistryUsvg {
    fn try_from_str_with_text_anchor_probe(
        _source: impl FnOnce() -> String,
        _options: &usvg::Options<'_>,
        _id: &str,
    ) -> Result<Option<AnchorProbe>, usvg::Error> {
        Ok(None)
    }
}

impl RegistryUsvg for usvg::Tree {}

fn parse_text_probe(
    svg: impl Fn(&str) -> String,
    start: &str,
    anchor_payload: Option<&str>,
    options: &usvg::Options<'_>,
) -> Result<AnchorProbe, usvg::Error> {
    let Some(payload) = anchor_payload else {
        return usvg::Tree::from_str(&svg(start), options).map(|tree| (tree, None));
    };
    if let Some(probe) =
        usvg::Tree::try_from_str_with_text_anchor_probe(|| svg(start), options, "start")?
    {
        return Ok(probe);
    }

    let nodes = format!(r#"{start}<text id="end" x="0" y="0" text-anchor="end">{payload}</text>"#);
    let tree = usvg::Tree::from_str(&svg(&nodes), options)?;
    let end_x = match tree.node_by_id("end") {
        Some(usvg::Node::Text(end)) => Some(end.bounding_box().x()),
        _ => None,
    };
    Ok((tree, end_x))
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use merman_render::environment::TextMeasurementPhase;
    use merman_render::text::{TextStyle, WrapMode};

    #[test]
    fn acquisition_requires_an_explicit_nonempty_font_set() {
        assert!(matches!(
            NativeFontOptions::default().load(&OperationControl::new()),
            Err(NativeFontError::EmptyFontSet)
        ));
    }

    #[test]
    fn one_shape_preserves_two_anchor_rounding_and_bounds() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
        let control = OperationControl::new();
        let context = NativeFontOptions::default()
            .with_font_file(root.join("NotoSans-Latin-subset.ttf"))
            .with_font_file(root.join("Cousine-Regular.ttf"))
            .with_font_file(root.join("NotoSansSC-subset.ttf"))
            .load(&control)
            .unwrap()
            .with_strict_glyph_coverage(true);
        let mut options = usvg::Options::default();
        context.apply_to_usvg_options(&mut options);
        for (family, text, preserve, tspan) in [
            ("Noto Sans", "AVA fi", false, false),
            ("Noto Sans", "  AV\u{a0}  ", true, false),
            ("Noto Sans", " \u{a0} ", true, true),
            ("Cousine", "אבגד", false, false),
            ("Cousine", "A中V", false, false),
        ] {
            let style = TextStyle {
                font_family: Some(family.to_owned()),
                font_size: 31.125,
                ..Default::default()
            };
            let actual = context
                .shape_probe(text, &style, &control, preserve, tspan, None)
                .unwrap();
            let whitespace = text.chars().all(char::is_whitespace);
            let space = if preserve && !tspan {
                "preserve"
            } else {
                "default"
            };
            let decoration = if whitespace { "underline" } else { "none" };
            let payload = if tspan {
                format!(r#"<tspan xml:space="preserve">{text}</tspan>"#)
            } else {
                text.to_owned()
            };
            let source = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><g font-family="{family}" font-size="31.125" font-weight="400" font-style="normal" xml:space="{space}" text-decoration="{decoration}"><text id="start" x="0" y="0" text-anchor="start">{payload}</text><text id="end" x="0" y="0" text-anchor="end">{payload}</text></g></svg>"#,
            );
            let tree = usvg::Tree::from_str(&source, &options).unwrap();
            let (Some(usvg::Node::Text(start)), Some(usvg::Node::Text(end))) =
                (tree.node_by_id("start"), tree.node_by_id("end"))
            else {
                panic!("expected both reference anchors for {text:?}");
            };
            let metric = start.bounding_box();
            let ink = if whitespace {
                metric
            } else {
                start.flattened().bounding_box()
            };
            assert_eq!(
                [
                    actual.advance,
                    actual.width,
                    actual.height,
                    actual.x,
                    actual.ink_x,
                    actual.ink_right,
                    actual.raw_y
                ]
                .map(f64::to_bits),
                [
                    (metric.x() - end.bounding_box().x()).abs(),
                    metric.width(),
                    metric.height(),
                    metric.x(),
                    ink.x(),
                    ink.right(),
                    metric.y(),
                ]
                .map(|value| f64::from(value).to_bits()),
                "{text:?}"
            );
        }
    }

    #[test]
    fn partial_fallback_with_different_ligatures_resolves_matching_clusters() {
        let base = include_bytes!("../tests/fonts/MermanProbeBase.ttf");
        let partial = include_bytes!("../tests/fonts/MermanProbePartial.ttf");
        let full = include_bytes!("../tests/fonts/MermanProbeFull.ttf");
        for (text, expected) in [
            (
                "fi✓\u{e000}",
                ["MermanProbeBase", "MermanProbePartial", "MermanProbeBase"],
            ),
            (
                "fi✓中",
                ["MermanProbeBase", "MermanProbePartial", "MermanProbeFull"],
            ),
        ] {
            let control = OperationControl::new();
            let context = NativeFontContext::from_font_bytes(
                [base.to_vec(), partial.to_vec(), full.to_vec()],
                NativeFontLimits::default(),
                &control,
            )
            .unwrap()
            .with_strict_glyph_coverage(true);
            let style = TextStyle {
                font_family: Some("MermanProbeBase".to_owned()),
                font_size: 28.0,
                ..Default::default()
            };
            let measured = context.measure(HostTextMeasurementRequest {
                operation: TextMeasurementOperation::CanvasMeasureTextWidth,
                phase: TextMeasurementPhase::ComputedLength,
                text,
                style: &style,
                max_width: None,
                wrap_mode: WrapMode::HtmlLike,
            });
            let Some(HostTextMeasurement::Length(width)) = measured.unwrap() else {
                panic!("expected complete native measurement");
            };
            assert_eq!(width, 42.0);

            let mut options = usvg::Options::default();
            let observer = context.apply_to_usvg_options_for_export(&mut options, &control);
            let source = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="50"><text id="probe" x="0" y="40" font-family="MermanProbeBase" font-size="28">{text}</text></svg>"#
            );
            let tree = usvg::Tree::from_str(&source, &options).unwrap();
            let Some(usvg::Node::Text(text)) = tree.node_by_id("probe") else {
                panic!("expected a laid-out text node");
            };
            let faces: Vec<_> = text
                .layouted()
                .iter()
                .flat_map(|span| &span.positioned_glyphs)
                .map(|glyph| {
                    assert_ne!(glyph.id.0, 0);
                    tree.fontdb()
                        .face(glyph.font)
                        .unwrap()
                        .post_script_name
                        .as_str()
                })
                .collect();
            assert_eq!(faces, expected);
            assert!(!observer.unresolved_glyph_fallback());
            context.check().unwrap();
        }
    }

    #[test]
    fn native_export_glyph_fallback_filters_symbols_and_records_the_outcome() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
        for fixture in ["MermanSymbolicTest.ttf", "MermanPictorialTest.ttf"] {
            for include_text_fallback in [false, true] {
                let mut fonts = NativeFontOptions::default()
                    .with_font_file(root.join("NotoSans-Latin-subset.ttf"))
                    .with_font_file(root.join(fixture));
                if include_text_fallback {
                    fonts = fonts.with_font_file(root.join("Cousine-Regular.ttf"));
                }
                let control = OperationControl::new();
                let context = fonts.load(&control).unwrap();
                let mut options = usvg::Options::default();
                let observer = context.apply_to_usvg_options_for_export(&mut options, &control);
                let tree = usvg::Tree::from_str(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="80"><text id="probe" x="0" y="40" font-family="Noto Sans">Aam</text></svg>"#,
                    &options,
                ).unwrap();
                let Some(usvg::Node::Text(text)) = tree.node_by_id("probe") else {
                    panic!("partial text must still be painted");
                };
                let mut expected = BTreeSet::from(["NotoSans-Regular"]);
                if include_text_fallback {
                    expected.insert("Cousine-Regular");
                }
                let glyphs: Vec<_> = text
                    .layouted()
                    .iter()
                    .flat_map(|span| &span.positioned_glyphs)
                    .collect();
                let selected: BTreeSet<_> = glyphs
                    .iter()
                    .map(|glyph| {
                        tree.fontdb()
                            .face(glyph.font)
                            .unwrap()
                            .post_script_name
                            .as_str()
                    })
                    .collect();
                assert_eq!(selected, expected);
                assert_eq!(observer.glyph_fallback_used(), include_text_fallback);
                assert_eq!(observer.unresolved_glyph_fallback(), !include_text_fallback);
                context.check().unwrap();
            }
        }
    }

    #[test]
    #[cfg(not(all(feature = "system-fonts", not(target_family = "wasm"))))]
    fn system_discovery_is_explicitly_unavailable() {
        assert!(matches!(
            NativeFontOptions::default()
                .with_system_fonts(true)
                .load(&OperationControl::new()),
            Err(NativeFontError::SystemDiscoveryUnavailable)
        ));
    }

    #[test]
    fn acquisition_replays_cancellation_before_io() {
        let control = OperationControl::new();
        control.cancel();
        assert!(matches!(
            NativeFontOptions::default()
                .with_font_file("missing.ttf")
                .load(&control),
            Err(NativeFontError::Operation(_))
        ));
    }

    #[test]
    fn directory_filter_is_case_insensitive_and_excludes_web_fonts() {
        assert!(is_font_path(Path::new("font.TTC")));
        assert!(!is_font_path(Path::new("font.woff2")));
    }

    #[test]
    fn measurement_budget_failure_is_sticky_at_the_fallible_boundary() {
        let limits = NativeFontLimits {
            max_label_bytes: 1,
            ..Default::default()
        };
        let context = NativeFontContext(Arc::new(FontContextInner {
            database: Arc::new(usvg::fontdb::Database::new()),
            selection: Arc::new(FontSelectionPolicy::default()),
            #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
            discovery: None,
            #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
            sources: registry::FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0),
            default_family: "sans-serif".to_owned(),
            language: None,
            identity: "test".to_owned(),
            limits,
            control: OperationControl::new(),
            state: Mutex::new(MeasurementState::default()),
            fatal: OnceLock::new(),
        }));
        let style = TextStyle::default();
        let result = context.measure(HostTextMeasurementRequest {
            operation: TextMeasurementOperation::TitleBBoxX,
            phase: TextMeasurementPhase::SvgBBox,
            text: "long",
            style: &style,
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        });
        assert!(result.is_err());
        assert!(matches!(
            context.check(),
            Err(NativeFontError::Limit {
                resource: "label-bytes",
                ..
            })
        ));
        assert!(matches!(
            context.for_export(&OperationControl::new()),
            Err(NativeFontError::Limit {
                resource: "label-bytes",
                ..
            })
        ));
    }

    #[test]
    fn whitespace_advance_is_not_mistaken_for_missing_glyph_coverage() {
        let context = NativeFontOptions::default()
            .with_font_file(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
            )
            .load(&OperationControl::new())
            .unwrap();
        let style = TextStyle {
            font_family: Some("Cousine".to_owned()),
            font_size: 16.0,
            ..TextStyle::default()
        };
        let result = context
            .measure(HostTextMeasurementRequest {
                operation: TextMeasurementOperation::CanvasMeasureTextWidth,
                phase: TextMeasurementPhase::ComputedLength,
                text: "   ",
                style: &style,
                max_width: None,
                wrap_mode: WrapMode::SvgLike,
            })
            .unwrap();
        assert!(matches!(result, Some(HostTextMeasurement::Length(width)) if width > 0.0));
    }

    #[test]
    fn repeated_missing_glyph_probes_share_the_bounded_measurement_cache() {
        let control = OperationControl::new();
        let context = NativeFontOptions::default()
            .with_font_file(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
            )
            .with_limits(NativeFontLimits {
                max_measurements: 1,
                ..Default::default()
            })
            .load(&control)
            .unwrap();
        let style = TextStyle {
            font_family: Some("Cousine".to_owned()),
            ..TextStyle::default()
        };
        let request = HostTextMeasurementRequest {
            operation: TextMeasurementOperation::TitleBBoxX,
            phase: TextMeasurementPhase::SvgBBox,
            text: "\u{10ffff}",
            style: &style,
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        };
        for _ in 0..3 {
            assert!(context.measure(request).unwrap().is_none());
        }
        let state = context.0.state.lock().unwrap();
        assert_eq!(state.measurements, 1);
        assert_eq!(state.cache.len(), 1);
        assert!(state.cache.values().all(Option::is_none));
        assert!(state.cache_bytes > request.text.len());
        assert!(state.cache_bytes <= context.0.limits.max_cache_bytes);
        drop(state);
        context.check().unwrap();

        let fresh = context.for_operation(&OperationControl::new());
        assert!(fresh.0.state.lock().unwrap().cache.is_empty());
        assert!(fresh.measure(request).unwrap().is_none());
        assert_eq!(fresh.0.state.lock().unwrap().measurements, 1);

        control.cancel();
        assert!(context.measure(request).is_err());
        assert_eq!(context.0.state.lock().unwrap().measurements, 1);
        assert!(matches!(
            context.check(),
            Err(NativeFontError::Operation(_))
        ));
        fresh.check().unwrap();
    }

    #[test]
    fn missing_glyph_cache_respects_entry_and_byte_limits() {
        for (max_cache_entries, max_cache_bytes, warm_success) in
            [(0, 4096, false), (4, 0, false), (1, 4096, true)]
        {
            let control = OperationControl::new();
            let context = NativeFontOptions::default()
                .with_font_file(
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
                )
                .with_limits(NativeFontLimits {
                    max_cache_entries,
                    max_cache_bytes,
                    max_measurements: 1 + usize::from(warm_success),
                    ..Default::default()
                })
                .load(&control)
                .unwrap();
            let style = TextStyle {
                font_family: Some("Cousine".to_owned()),
                ..TextStyle::default()
            };
            let request = HostTextMeasurementRequest {
                operation: TextMeasurementOperation::TitleBBoxX,
                phase: TextMeasurementPhase::SvgBBox,
                text: "\u{10ffff}",
                style: &style,
                max_width: None,
                wrap_mode: WrapMode::SvgLike,
            };
            if warm_success {
                assert!(
                    context
                        .measure(HostTextMeasurementRequest {
                            text: "A",
                            ..request
                        })
                        .unwrap()
                        .is_some()
                );
            }
            assert!(context.measure(request).unwrap().is_none());
            assert!(context.measure(request).is_err());
            assert!(matches!(
                context.check(),
                Err(NativeFontError::Limit {
                    resource: "measurements",
                    ..
                })
            ));
            let state = context.0.state.lock().unwrap();
            assert_eq!(state.cache.len(), usize::from(warm_success));
            assert!(state.cache_bytes <= max_cache_bytes);
        }
    }

    #[test]
    fn missing_glyph_cache_keys_keep_probe_text_and_style_distinct() {
        let control = OperationControl::new();
        let context = NativeFontOptions::default()
            .with_font_file(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
            )
            .load(&control)
            .unwrap();
        let style = TextStyle::default();
        let larger = TextStyle {
            font_size: style.font_size + 1.0,
            ..style.clone()
        };
        for (text, style) in [
            ("\u{10ffff}", &style),
            ("A\u{10ffff}", &style),
            ("\u{10ffff}", &larger),
        ] {
            let request = HostTextMeasurementRequest {
                operation: TextMeasurementOperation::TitleBBoxX,
                phase: TextMeasurementPhase::SvgBBox,
                text,
                style,
                max_width: None,
                wrap_mode: WrapMode::SvgLike,
            };
            for _ in 0..2 {
                assert!(context.measure(request).unwrap().is_none());
            }
        }
        let state = context.0.state.lock().unwrap();
        assert_eq!(state.measurements, 3);
        assert_eq!(state.cache.len(), 3);
    }

    #[test]
    fn svg_wrap_height_uses_native_bbox_and_em_row_offsets() {
        let control = OperationControl::new();
        let context = NativeFontOptions::default()
            .with_font_file(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
            )
            .load(&control)
            .unwrap();
        let style = TextStyle {
            font_size: 20.0,
            ..TextStyle::default()
        };
        let row = context.shape_line("text", &style, &control).unwrap();
        for mode in [WrapMode::SvgLike, WrapMode::SvgLikeSingleRun] {
            let result = context
                .measure(HostTextMeasurementRequest {
                    operation: TextMeasurementOperation::Wrapped,
                    phase: TextMeasurementPhase::Wrap,
                    text: "text<br>text<br>text",
                    style: &style,
                    max_width: None,
                    wrap_mode: mode,
                })
                .unwrap()
                .unwrap();
            let HostTextMeasurement::Metrics(metrics) = result else {
                panic!("expected metrics")
            };
            assert_eq!(metrics.line_count, 3);
            assert!((metrics.height - (row.height + 2.0 * 20.0 * 1.1)).abs() < 1e-6);
        }
    }

    #[test]
    fn uncached_svg_final_rows_supply_width_and_height_from_one_probe_each() {
        let context = NativeFontOptions::default()
            .with_font_file(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
            )
            .with_limits(NativeFontLimits {
                max_cache_entries: 0,
                max_cache_bytes: 0,
                max_measurements: 2,
                ..Default::default()
            })
            .load(&OperationControl::new())
            .unwrap();
        let style = TextStyle {
            font_size: 20.0,
            ..Default::default()
        };
        let result = context
            .measure(HostTextMeasurementRequest {
                operation: TextMeasurementOperation::Wrapped,
                phase: TextMeasurementPhase::Wrap,
                text: "first<br>second",
                style: &style,
                max_width: None,
                wrap_mode: WrapMode::SvgLike,
            })
            .unwrap()
            .unwrap();
        let HostTextMeasurement::Metrics(metrics) = result else {
            panic!("expected metrics")
        };
        assert_eq!(metrics.line_count, 2);
        assert!(metrics.width > 0.0 && metrics.height > 20.0);
        let state = context.0.state.lock().unwrap();
        assert_eq!(state.measurements, 2);
        assert!(state.cache.is_empty());
        drop(state);
        context.check().unwrap();
    }

    #[test]
    fn default_svg_whitespace_matches_independent_text_probes() {
        let control = OperationControl::new();
        let context = NativeFontOptions::default()
            .with_font_file(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
            )
            .load(&control)
            .unwrap();
        let style = TextStyle {
            font_family: Some("Cousine".to_owned()),
            font_size: 20.0,
            ..TextStyle::default()
        };
        for label in ["  A  B  ", "A\tB", "A\nB", "A\r\nB", "\u{a0}A\u{a0}B\u{a0}"] {
            let source = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="100"><text id="independent" x="0" y="0" font-family="Cousine" font-size="20">{}</text></svg>"#,
                escape_xml(label)
            );
            let mut options = usvg::Options::default();
            context.apply_to_usvg_options(&mut options);
            let tree = usvg::Tree::from_str(&source, &options).unwrap();
            let Some(usvg::Node::Text(text)) = tree.node_by_id("independent") else {
                panic!("probe text missing");
            };
            let expected = f64::from(text.bounding_box().width());
            for operation in [
                TextMeasurementOperation::MermaidCalculateTextDimensions,
                TextMeasurementOperation::SimpleBBoxWidth,
                TextMeasurementOperation::RawBBoxWidth,
                TextMeasurementOperation::TspanBBoxWidth,
            ] {
                let result = context
                    .measure(HostTextMeasurementRequest {
                        operation,
                        phase: TextMeasurementPhase::SvgBBox,
                        text: label,
                        style: &style,
                        max_width: None,
                        wrap_mode: WrapMode::SvgLike,
                    })
                    .unwrap()
                    .unwrap();
                let width = match result {
                    HostTextMeasurement::Length(width) => width,
                    HostTextMeasurement::Metrics(metrics) => metrics.width,
                    _ => panic!("unexpected measurement kind"),
                };
                assert!(
                    (width - expected).abs() < 1e-6,
                    "{operation:?} {label:?}: {width} != {expected}"
                );
            }
        }
    }

    #[test]
    fn preserved_tspan_requests_use_distinct_native_cache_entries() {
        let context = NativeFontOptions::default()
            .with_font_file(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
            )
            .load(&OperationControl::new())
            .unwrap();
        let style = TextStyle {
            font_family: Some("Cousine".to_owned()),
            font_size: 20.0,
            ..TextStyle::default()
        };
        let label = "  A  B  ";
        let width = |mode| {
            let result = context
                .measure(HostTextMeasurementRequest {
                    operation: TextMeasurementOperation::TspanBBoxWidth,
                    phase: TextMeasurementPhase::SvgBBox,
                    text: label,
                    style: &style,
                    max_width: None,
                    wrap_mode: mode,
                })
                .unwrap()
                .unwrap();
            let HostTextMeasurement::Length(width) = result else {
                panic!("expected width")
            };
            width
        };
        let collapsed = width(WrapMode::SvgLike);
        let preserved = width(WrapMode::HtmlLike);
        assert!(preserved > collapsed);
        for mode in [WrapMode::SvgLike, WrapMode::HtmlLike, WrapMode::SvgLike] {
            let space = if mode == WrapMode::HtmlLike {
                "preserve"
            } else {
                "default"
            };
            let source = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="100"><text id="probe" x="0" y="0" font-family="Cousine" font-size="20"><tspan xml:space="{space}">{label}</tspan></text></svg>"#
            );
            let mut options = usvg::Options::default();
            context.apply_to_usvg_options(&mut options);
            let tree = usvg::Tree::from_str(&source, &options).unwrap();
            let Some(usvg::Node::Text(text)) = tree.node_by_id("probe") else {
                panic!("missing probe")
            };
            assert!(
                (width(mode) - f64::from(text.bounding_box().width())).abs() < 1e-6,
                "{mode:?}: native={}, probe={}",
                width(mode),
                text.bounding_box().width()
            );
        }
    }

    #[test]
    fn baseline_probes_are_lazy_and_respect_the_shared_cache_limit() {
        for max_cache_entries in [0, 16] {
            let context = NativeFontOptions::default()
                .with_font_file(
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
                )
                .with_limits(NativeFontLimits {
                    max_cache_entries,
                    ..Default::default()
                })
                .load(&OperationControl::new())
                .unwrap();
            let style = TextStyle {
                font_size: 20.0,
                ..TextStyle::default()
            };
            let request = |operation| HostTextMeasurementRequest {
                operation,
                phase: TextMeasurementPhase::SvgBBox,
                text: "baseline",
                style: &style,
                max_width: None,
                wrap_mode: WrapMode::SvgLike,
            };
            context
                .measure(request(TextMeasurementOperation::RawBBoxWidth))
                .unwrap();
            assert_eq!(context.0.state.lock().unwrap().measurements, 1);
            let first = context
                .measure(request(TextMeasurementOperation::CreateTextBBoxYOffset))
                .unwrap();
            let second = context
                .measure(request(TextMeasurementOperation::CreateTextBBoxYOffset))
                .unwrap();
            assert!(matches!((first, second),
                (Some(HostTextMeasurement::Length(a)), Some(HostTextMeasurement::Length(b)))
                if a == b));
            let state = context.0.state.lock().unwrap();
            if max_cache_entries == 0 {
                assert_eq!(state.measurements, 3);
                assert_eq!(state.cache.len(), 0);
                assert_eq!(state.cache_bytes, 0);
            } else {
                assert_eq!(state.measurements, 2);
                assert_eq!(state.cache.len(), 2);
            }
        }
    }
}
