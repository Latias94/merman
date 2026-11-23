#[cfg(any(all(feature = "system-fonts", not(target_family = "wasm")), test))]
use super::font_selection::FontSelectionPolicy;
use super::{ExportError, OperationControl, Result, export_checkpoint};
#[cfg(any(all(feature = "system-fonts", not(target_family = "wasm")), test))]
use merman_render::diagram_theme::{FontCatalog, FontSource};
use merman_render::svg::ResvgCompatibleSvg;
use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;
#[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Effective font-source order admitted for one native export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExportFontMode {
    /// Resolve only from fonts discovered on the host system.
    SystemOnly,
    /// A caller-provided native context supplied both measurement and painting fonts.
    NativeContext,
}

impl ExportFontMode {
    /// Returns the stable external identifier for this source order.
    pub const fn id(self) -> &'static str {
        match self {
            Self::SystemOnly => "system-only",
            Self::NativeContext => "native-context",
        }
    }
}

/// Frozen font-resolution evidence for one concrete native export target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportFontPlan {
    source_mode: ExportFontMode,
    used_system_fonts: bool,
    family_fallback_used: bool,
    glyph_fallback_used: bool,
    unresolved_font_request: bool,
    unresolved_glyph_fallback: bool,
    unclassified_face_count: usize,
    notdef_glyph_count: usize,
}

impl ExportFontPlan {
    /// Returns the admitted source mode for the native operation.
    pub const fn source_mode(self) -> ExportFontMode {
        self.source_mode
    }

    /// Returns whether the final usvg tree contains a visible glyph from a host system face.
    pub const fn used_system_fonts(self) -> bool {
        self.used_system_fonts
    }

    /// Returns whether a requested font-family list needed a generic or last-resort face.
    pub const fn family_fallback_used(self) -> bool {
        self.family_fallback_used
    }

    /// Returns whether a character needed a different face from the selected family face.
    pub const fn glyph_fallback_used(self) -> bool {
        self.glyph_fallback_used
    }

    /// Returns whether a visible text span lacked a usable primary face.
    /// Registry usvg 0.47 conservatively reports any failed primary lookup.
    pub const fn unresolved_font_request(self) -> bool {
        self.unresolved_font_request
    }

    /// Returns whether final visible text has unresolved glyph coverage.
    /// Registry usvg 0.47 cannot distinguish fully omitted glyphs from speculative lookup
    /// misses, so it conservatively includes both final `.notdef` glyphs and lookup misses.
    pub const fn unresolved_glyph_fallback(self) -> bool {
        self.unresolved_glyph_fallback
    }

    /// Returns the number of final font faces absent from the operation's admitted sources.
    pub const fn unclassified_face_count(self) -> usize {
        self.unclassified_face_count
    }

    /// Returns the number of final positioned glyphs that resolved to `.notdef`.
    pub const fn notdef_glyph_count(self) -> usize {
        self.notdef_glyph_count
    }

    /// Returns whether the concrete parsed output depends on caller-owned or system fonts.
    pub const fn is_host_dependent(self) -> bool {
        matches!(self.source_mode, ExportFontMode::NativeContext) || self.used_system_fonts
    }
}

#[derive(Default)]
struct FontResolutionRecorder {
    family_fallback_used: AtomicBool,
    glyph_fallback_used: AtomicBool,
    unresolved_font_request: AtomicBool,
    unresolved_glyph_fallback: AtomicBool,
}

pub(crate) struct ExportFontPlanSeed {
    source_mode: ExportFontMode,
    system_fonts: Arc<usvg::fontdb::Database>,
    recorder: Arc<FontResolutionRecorder>,
    #[cfg(feature = "native-fonts")]
    native_export: Option<crate::fonts::NativeExportFontObserver>,
}

impl ExportFontPlanSeed {
    #[cfg(feature = "native-fonts")]
    pub(crate) fn from_native_context(
        context: &crate::NativeFontContext,
        observer: crate::fonts::NativeExportFontObserver,
    ) -> Self {
        Self {
            source_mode: ExportFontMode::NativeContext,
            // Native contexts classify faces through the operation-local observer, including
            // COW-installed discovery faces that are absent from this initial database snapshot.
            system_fonts: context.fontdb_for_export(),
            recorder: Arc::new(FontResolutionRecorder::default()),
            native_export: Some(observer),
        }
    }

    pub(crate) fn finish_with_tree(
        &self,
        tree: &usvg::Tree,
        control: &OperationControl,
    ) -> Result<ExportFontPlan> {
        export_checkpoint(control)?;
        #[cfg(feature = "native-fonts")]
        let evidence = self.native_export.as_ref().map_or_else(
            || FinalTreeFontEvidence::collect(tree, &self.system_fonts, control),
            |observer| FinalTreeFontEvidence::collect_native(tree, observer, control),
        )?;
        #[cfg(not(feature = "native-fonts"))]
        let evidence = FinalTreeFontEvidence::collect(tree, &self.system_fonts, control)?;
        #[cfg(feature = "native-fonts")]
        let native_facts = self.native_export.as_ref().map(|observer| {
            (
                observer.family_fallback_used(),
                observer.glyph_fallback_used(),
            )
        });
        #[cfg(not(feature = "native-fonts"))]
        let native_facts: Option<(bool, bool)> = None;

        let mut unresolved_font_request = evidence.no_primary_face;
        let mut unresolved_glyph_fallback =
            evidence.dropped_notdef || evidence.notdef_glyph_count != 0;
        if tree.root().text_font_omissions().is_none() {
            // Registry usvg prunes some missing glyphs before exposing the tree.
            // Keep lookup misses as conservative coverage evidence on this backend.
            unresolved_font_request |= self
                .recorder
                .unresolved_font_request
                .load(Ordering::Relaxed);
            unresolved_glyph_fallback |= self
                .recorder
                .unresolved_glyph_fallback
                .load(Ordering::Relaxed);
            #[cfg(feature = "native-fonts")]
            if let Some(observer) = &self.native_export {
                unresolved_font_request |= observer.unresolved_font_request();
                unresolved_glyph_fallback |= observer.unresolved_glyph_fallback();
            }
        }

        export_checkpoint(control)?;
        Ok(ExportFontPlan {
            source_mode: self.source_mode,
            // Native contexts derive this from visible final glyphs and the exact resolver's
            // operation-local admission record, not from an initial fontdb snapshot.
            used_system_fonts: evidence.used_system_fonts,
            family_fallback_used: self.recorder.family_fallback_used.load(Ordering::Relaxed)
                || native_facts.is_some_and(|facts| facts.0),
            glyph_fallback_used: self.recorder.glyph_fallback_used.load(Ordering::Relaxed)
                || native_facts.is_some_and(|facts| facts.1),
            unresolved_font_request,
            unresolved_glyph_fallback,
            unclassified_face_count: evidence.unclassified_face_count,
            notdef_glyph_count: evidence.notdef_glyph_count,
        })
    }
}

// The workspace usvg provides an inherent method. Published Merman consumers using the
// registry usvg 0.47 API can inspect final positioned glyphs but cannot inspect omissions.
#[allow(dead_code, reason = "used when the consumer resolves unpatched usvg")]
trait RegistryUsvgGroup {
    fn text_font_omissions(&self) -> Option<(bool, bool)> {
        None
    }
}

impl RegistryUsvgGroup for usvg::Group {}

#[derive(Debug, Default)]
struct FinalTreeFontEvidence {
    used_system_fonts: bool,
    unclassified_face_count: usize,
    notdef_glyph_count: usize,
    no_primary_face: bool,
    dropped_notdef: bool,
}

impl FinalTreeFontEvidence {
    fn collect(
        tree: &usvg::Tree,
        system_fonts: &usvg::fontdb::Database,
        control: &OperationControl,
    ) -> Result<Self> {
        let mut system_faces = BTreeSet::new();
        for face in system_fonts.faces() {
            export_checkpoint(control)?;
            system_faces.insert(face.id);
        }
        Self::collect_admitted(tree, system_faces, BTreeSet::new(), control)
    }

    #[cfg(feature = "native-fonts")]
    fn collect_native(
        tree: &usvg::Tree,
        observer: &crate::fonts::NativeExportFontObserver,
        control: &OperationControl,
    ) -> Result<Self> {
        export_checkpoint(control)?;
        let mut system_faces = BTreeSet::new();
        let mut caller_faces = BTreeSet::new();
        if let Some(faces) = observer.admitted_faces() {
            for (&id, source) in faces.iter() {
                export_checkpoint(control)?;
                match source {
                    crate::fonts::NativeExportFontSource::System => {
                        system_faces.insert(id);
                    }
                    crate::fonts::NativeExportFontSource::Caller => {
                        caller_faces.insert(id);
                    }
                }
            }
        }
        Self::collect_admitted(tree, system_faces, caller_faces, control)
    }

    fn collect_admitted(
        tree: &usvg::Tree,
        system_faces: BTreeSet<usvg::fontdb::ID>,
        caller_faces: BTreeSet<usvg::fontdb::ID>,
        control: &OperationControl,
    ) -> Result<Self> {
        let mut walk = FinalTreeFontWalk {
            evidence: Self::default(),
            system_faces,
            caller_faces,
            unclassified_faces: HashSet::new(),
            visited_resources: HashSet::new(),
            control,
        };
        walk.visit_group(tree.root())?;
        walk.evidence.unclassified_face_count = walk.unclassified_faces.len();
        Ok(walk.evidence)
    }
}

struct FinalTreeFontWalk<'a> {
    evidence: FinalTreeFontEvidence,
    system_faces: BTreeSet<usvg::fontdb::ID>,
    caller_faces: BTreeSet<usvg::fontdb::ID>,
    unclassified_faces: HashSet<usvg::fontdb::ID>,
    visited_resources: HashSet<*const usvg::Group>,
    control: &'a OperationControl,
}

impl FinalTreeFontWalk<'_> {
    fn visit_group(&mut self, group: &usvg::Group) -> Result<bool> {
        export_checkpoint(self.control)?;
        if group.opacity().get() == 0.0 {
            return Ok(false);
        }
        let mut may_paint = !group.filters().is_empty();
        if let Some((no_primary_face, dropped_notdef)) = group.text_font_omissions() {
            self.evidence.no_primary_face |= no_primary_face;
            self.evidence.dropped_notdef |= dropped_notdef;
            may_paint |= no_primary_face || dropped_notdef;
        }
        for node in group.children() {
            export_checkpoint(self.control)?;
            match node {
                usvg::Node::Group(group) => may_paint |= self.visit_group(group)?,
                usvg::Node::Text(text) => {
                    self.visit_text(text)?;
                    may_paint |= self.visit_group(text.flattened())?;
                }
                usvg::Node::Path(path) if path.is_visible() => {
                    for paint in [
                        path.fill()
                            .filter(|fill| fill.opacity().get() != 0.0)
                            .map(usvg::Fill::paint),
                        path.stroke()
                            .filter(|stroke| stroke.opacity().get() != 0.0)
                            .map(usvg::Stroke::paint),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        may_paint = true;
                        if let usvg::Paint::Pattern(pattern) = paint {
                            self.visit_resource(pattern.root())?;
                        }
                    }
                }
                usvg::Node::Image(image) if image.is_visible() => {
                    if let usvg::ImageKind::SVG(tree) = image.kind() {
                        may_paint |= self.visit_group(tree.root())?;
                    } else {
                        may_paint = true;
                    }
                }
                usvg::Node::Path(_) | usvg::Node::Image(_) => {}
            }
        }
        if !may_paint {
            return Ok(false);
        }
        // Inspect linked roots here, under the consumer's visibility check. Node::subroots
        // exposes only the first two clip and mask links.
        let clips = std::iter::successors(group.clip_path(), |clip| clip.clip_path());
        self.visit_linked_resources(clips.map(usvg::ClipPath::root))?;
        let masks = std::iter::successors(group.mask(), |mask| mask.mask());
        self.visit_linked_resources(masks.map(usvg::Mask::root))?;
        for filter in group.filters() {
            self.visit_filter_images(filter)?;
        }
        Ok(true)
    }

    fn visit_filter_images<'filter>(
        &mut self,
        filter: &'filter usvg::filter::Filter,
    ) -> Result<()> {
        use usvg::filter::{Input, Kind};

        // usvg resolves omitted inputs to the previous result or SourceGraphic. Walk
        // back from the final output so each name binds to the nearest preceding result,
        // and only inspect images that can contribute to that output.
        let mut needed_results = HashSet::new();
        for (offset, primitive) in filter.primitives().iter().rev().enumerate() {
            export_checkpoint(self.control)?;
            if offset != 0 && !needed_results.remove(primitive.result()) {
                continue;
            }
            let mut require_input = |input: &'filter Input| {
                if let Input::Reference(name) = input {
                    needed_results.insert(name.as_str());
                }
            };
            match primitive.kind() {
                Kind::Image(image) => {
                    self.visit_resource(image.root())?;
                }
                Kind::Blend(fe) => {
                    require_input(fe.input1());
                    require_input(fe.input2());
                }
                Kind::Composite(fe) => {
                    require_input(fe.input1());
                    require_input(fe.input2());
                }
                Kind::DisplacementMap(fe) => {
                    require_input(fe.input1());
                    require_input(fe.input2());
                }
                Kind::Merge(fe) => {
                    for input in fe.inputs() {
                        export_checkpoint(self.control)?;
                        require_input(input);
                    }
                }
                Kind::ColorMatrix(fe) => require_input(fe.input()),
                Kind::ComponentTransfer(fe) => require_input(fe.input()),
                Kind::ConvolveMatrix(fe) => require_input(fe.input()),
                Kind::DiffuseLighting(fe) => require_input(fe.input()),
                Kind::DropShadow(fe) => require_input(fe.input()),
                Kind::GaussianBlur(fe) => require_input(fe.input()),
                Kind::Morphology(fe) => require_input(fe.input()),
                Kind::Offset(fe) => require_input(fe.input()),
                Kind::SpecularLighting(fe) => require_input(fe.input()),
                Kind::Tile(fe) => require_input(fe.input()),
                Kind::Flood(_) | Kind::Turbulence(_) => {}
            }
            if needed_results.is_empty() {
                break;
            }
        }
        Ok(())
    }

    fn visit_resource(&mut self, root: &usvg::Group) -> Result<bool> {
        if self.visited_resources.insert(std::ptr::from_ref(root)) {
            self.visit_group(root)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn visit_linked_resources<'tree>(
        &mut self,
        roots: impl Iterator<Item = &'tree usvg::Group>,
    ) -> Result<()> {
        for root in roots {
            // A visited clip or mask owns the same remaining links on every use.
            if !self.visit_resource(root)? {
                break;
            }
        }
        Ok(())
    }

    fn visit_text(&mut self, text: &usvg::Text) -> Result<()> {
        for span in text.layouted() {
            export_checkpoint(self.control)?;
            if !span.visible {
                continue;
            }
            let has_paint = span
                .fill
                .as_ref()
                .is_some_and(|fill| fill.opacity().get() != 0.0)
                || span
                    .stroke
                    .as_ref()
                    .is_some_and(|stroke| stroke.opacity().get() != 0.0)
                || [&span.underline, &span.overline, &span.line_through]
                    .into_iter()
                    .flatten()
                    .any(|path| {
                        path.is_visible()
                            && (path.fill().is_some_and(|fill| fill.opacity().get() != 0.0)
                                || path
                                    .stroke()
                                    .is_some_and(|stroke| stroke.opacity().get() != 0.0))
                    });
            for glyph in &span.positioned_glyphs {
                export_checkpoint(self.control)?;
                if glyph.id.0 == 0 && has_paint {
                    self.evidence.notdef_glyph_count =
                        self.evidence.notdef_glyph_count.saturating_add(1);
                }
                if self.system_faces.contains(&glyph.font) {
                    self.evidence.used_system_fonts = true;
                } else if !self.caller_faces.contains(&glyph.font) {
                    self.unclassified_faces.insert(glyph.font);
                }
            }
        }
        Ok(())
    }
}

pub(crate) struct ExportFontEnvironment {
    pub(crate) fontdb: Arc<usvg::fontdb::Database>,
    pub(crate) default_family: String,
    pub(crate) resolver: usvg::FontResolver<'static>,
    pub(crate) plan: ExportFontPlanSeed,
}

impl ExportFontEnvironment {
    pub(crate) fn from_svg(svg: &ResvgCompatibleSvg) -> Result<Self> {
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        {
            Self::from_resources(svg.font_catalog(), svg.font_source_policy(), || {
                shared_system_fonts().clone()
            })
        }
        #[cfg(not(all(feature = "system-fonts", not(target_family = "wasm"))))]
        {
            let _ = svg;
            Err(ExportError::FontSourceUnavailable)
        }
    }

    #[cfg(any(all(feature = "system-fonts", not(target_family = "wasm")), test))]
    fn from_resources(
        catalog: &FontCatalog,
        policy: &merman_render::diagram_theme::FontSourcePolicy,
        system_fonts: impl FnOnce() -> (Arc<usvg::fontdb::Database>, Arc<FontSelectionPolicy>),
    ) -> Result<Self> {
        if !policy.priority().any(|source| {
            source == FontSource::System
                && catalog
                    .available_sources()
                    .any(|available| available == source)
        }) {
            return Err(ExportError::FontSourceUnavailable);
        }

        let (fontdb, selection) = system_fonts();
        if fontdb.faces().next().is_none() {
            return Err(ExportError::FontSourceUnavailable);
        }
        let recorder = Arc::new(FontResolutionRecorder::default());
        let default_family = selection.default_family(&fontdb).unwrap_or_default();
        let resolver = system_font_resolver(Arc::clone(&fontdb), selection, Arc::clone(&recorder));

        Ok(Self {
            fontdb: Arc::clone(&fontdb),
            default_family,
            resolver,
            plan: ExportFontPlanSeed {
                source_mode: ExportFontMode::SystemOnly,
                system_fonts: fontdb,
                recorder,
                #[cfg(feature = "native-fonts")]
                native_export: None,
            },
        })
    }
}

#[cfg(any(all(feature = "system-fonts", not(target_family = "wasm")), test))]
fn system_font_resolver(
    fontdb: Arc<usvg::fontdb::Database>,
    selection: Arc<FontSelectionPolicy>,
    recorder: Arc<FontResolutionRecorder>,
) -> usvg::FontResolver<'static> {
    let select_fontdb = Arc::clone(&fontdb);
    let select_recorder = Arc::clone(&recorder);
    let fallback_selection = Arc::clone(&selection);

    usvg::FontResolver {
        select_font: Box::new(move |font, combined_db| {
            if let Some(id) = selection.requested(font, &select_fontdb) {
                debug_assert!(combined_db.face(id).is_some());
                return Some(id);
            }
            if let Some(id) = selection.fallback(font, &select_fontdb) {
                debug_assert!(combined_db.face(id).is_some());
                select_recorder
                    .family_fallback_used
                    .store(true, Ordering::Relaxed);
                return Some(id);
            }
            select_recorder
                .unresolved_font_request
                .store(true, Ordering::Relaxed);
            None
        }),
        select_fallback: Box::new(move |character, excluded, combined_db| {
            let selected = fallback_selection.glyph_fallback(character, excluded, combined_db);
            if selected.is_some() {
                recorder.glyph_fallback_used.store(true, Ordering::Relaxed);
            } else {
                recorder
                    .unresolved_glyph_fallback
                    .store(true, Ordering::Relaxed);
            }
            selected
        }),
    }
}

#[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
fn shared_system_fonts() -> &'static (Arc<usvg::fontdb::Database>, Arc<FontSelectionPolicy>) {
    static FONTS: OnceLock<(Arc<usvg::fontdb::Database>, Arc<FontSelectionPolicy>)> =
        OnceLock::new();
    FONTS.get_or_init(|| {
        let mut database = usvg::fontdb::Database::new();
        database.load_system_fonts();
        let selection = FontSelectionPolicy::from_database(&database);
        selection.configure_generics(&mut database);
        (Arc::new(database), Arc::new(selection))
    })
}

#[cfg(all(test, feature = "system-fonts", not(target_family = "wasm")))]
pub(crate) fn shared_system_fontdb() -> Arc<usvg::fontdb::Database> {
    Arc::clone(&shared_system_fonts().0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font_selection::face_family_name;
    use merman_render::diagram_theme::FontSourcePolicy;
    use merman_render::environment::RenderEnvironment;
    use merman_render::svg::finalize_resvg_svg;

    fn sealed_svg(catalog: FontCatalog, policy: FontSourcePolicy) -> ResvgCompatibleSvg {
        let session = RenderEnvironment::deterministic()
            .with_font_catalog(catalog)
            .with_font_source_policy(policy)
            .begin_session()
            .unwrap();
        finalize_resvg_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text font-family="Sketch">Alpha</text></svg>"#,
            &session,
        )
        .unwrap()
    }

    #[test]
    #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
    fn system_font_catalog_reuses_the_shared_system_database() {
        for policy in [
            FontSourcePolicy::system_only(),
            FontSourcePolicy::embedded_then_system(),
            FontSourcePolicy::system_then_embedded(),
        ] {
            let svg = sealed_svg(FontCatalog::system_fonts(), policy);
            let environment = ExportFontEnvironment::from_svg(&svg).unwrap();

            assert_eq!(environment.plan.source_mode, ExportFontMode::SystemOnly);
            assert!(Arc::ptr_eq(&environment.fontdb, &shared_system_fontdb()));
        }
    }

    #[test]
    fn embedded_only_policy_rejects_before_system_font_discovery() {
        let result = ExportFontEnvironment::from_resources(
            &FontCatalog::system_fonts(),
            &FontSourcePolicy::embedded_only(),
            || panic!("unavailable font sources must be rejected before discovering system fonts"),
        );

        assert!(matches!(result, Err(ExportError::FontSourceUnavailable)));
    }

    #[test]
    fn system_export_rejects_symbolic_generic_and_preserves_named_opt_in() {
        for fixture in [
            include_bytes!("../tests/fonts/MermanSymbolicTest.ttf").as_slice(),
            include_bytes!("../tests/fonts/MermanPictorialTest.ttf").as_slice(),
        ] {
            let mut database = usvg::fontdb::Database::new();
            database.load_font_data(fixture.to_vec());
            let symbol = face_family_name(database.faces().next().unwrap()).unwrap();
            let symbol_id = database.faces().next().unwrap().id;
            database.load_font_data(
                include_bytes!(
                    "../../merman-render/tests/fixtures/fonts/DejaVuSerif-NativeFilter.ttf"
                )
                .to_vec(),
            );
            let serif_id = database
                .faces()
                .find(|face| face.post_script_name == "DejaVuSerif")
                .unwrap()
                .id;
            database.set_serif_family(symbol.clone());
            database.set_sans_serif_family(symbol.clone());
            let selection = FontSelectionPolicy::from_database(&database);
            selection.configure_generics(&mut database);
            let database = Arc::new(database);
            let options = usvg::Options {
                fontdb: Arc::clone(&database),
                font_resolver: system_font_resolver(
                    database,
                    Arc::new(selection),
                    Arc::new(FontResolutionRecorder::default()),
                ),
                ..Default::default()
            };
            for (family, expected) in [
                ("serif", serif_id),
                ("sans-serif", serif_id),
                (symbol.as_str(), symbol_id),
            ] {
                let source = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="80"><text x="0" y="40" font-family="{family}">am</text></svg>"#
                );
                let tree = usvg::Tree::from_str(&source, &options).unwrap();
                let usvg::Node::Text(text) = &tree.root().children()[0] else {
                    panic!("text must be painted");
                };
                let glyphs: Vec<_> = text
                    .layouted()
                    .iter()
                    .flat_map(|span| &span.positioned_glyphs)
                    .collect();
                assert!(!glyphs.is_empty(), "family {family} must paint glyphs");
                assert!(
                    glyphs.iter().all(|glyph| glyph.font == expected),
                    "family {family}"
                );
            }
        }
    }

    #[test]
    fn system_export_glyph_fallback_filters_symbols_and_reports_missing_coverage() {
        for fixture in [
            include_bytes!("../tests/fonts/MermanSymbolicTest.ttf").as_slice(),
            include_bytes!("../tests/fonts/MermanPictorialTest.ttf").as_slice(),
        ] {
            for include_text_fallback in [false, true] {
                let mut database = usvg::fontdb::Database::new();
                database.load_font_data(
                    include_bytes!("../tests/fonts/NotoSans-Latin-subset.ttf").to_vec(),
                );
                database.load_font_data(fixture.to_vec());
                if include_text_fallback {
                    database.load_font_data(
                        include_bytes!("../tests/fonts/Cousine-Regular.ttf").to_vec(),
                    );
                }
                let environment = ExportFontEnvironment::from_resources(
                    &FontCatalog::system_fonts(),
                    &FontSourcePolicy::system_only(),
                    || {
                        let selection = FontSelectionPolicy::from_database(&database);
                        (Arc::new(database), Arc::new(selection))
                    },
                )
                .unwrap();
                let options = usvg::Options {
                    fontdb: environment.fontdb,
                    font_resolver: environment.resolver,
                    ..Default::default()
                };
                let tree = usvg::Tree::from_str(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="80"><text id="probe" x="0" y="40" font-family="Noto Sans">Aam</text></svg>"#,
                    &options,
                ).unwrap();
                let Some(usvg::Node::Text(text)) = tree.node_by_id("probe") else {
                    panic!("partial text must still be painted");
                };
                let mut expected = std::collections::BTreeSet::from(["NotoSans-Regular"]);
                if include_text_fallback {
                    expected.insert("Cousine-Regular");
                }
                let glyphs: Vec<_> = text
                    .layouted()
                    .iter()
                    .flat_map(|span| &span.positioned_glyphs)
                    .collect();
                let selected: std::collections::BTreeSet<_> = glyphs
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
                let plan = environment
                    .plan
                    .finish_with_tree(&tree, &OperationControl::new())
                    .unwrap();
                assert_eq!(plan.glyph_fallback_used(), include_text_fallback);
                assert_eq!(plan.unresolved_glyph_fallback(), !include_text_fallback);
            }
        }
    }

    #[cfg(all(target_os = "linux", feature = "system-fonts"))]
    #[test]
    fn linux_fontconfig_symbol_alias_preserves_serif_and_zenuml_default_stack() {
        const CHILD: &str = "MERMAN_SYMBOL_FONTCONFIG_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let environment = ExportFontEnvironment::from_resources(
                &FontCatalog::system_fonts(),
                &FontSourcePolicy::system_only(),
                || shared_system_fonts().clone(),
            )
            .unwrap();
            let options = usvg::Options {
                fontdb: environment.fontdb,
                font_resolver: environment.resolver,
                ..Default::default()
            };
            for family in ["serif", "Helvetica,Verdana,serif"] {
                let source = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="80"><text id="probe" x="0" y="40" font-family="{family}">am</text></svg>"#
                );
                let tree = usvg::Tree::from_str(&source, &options).unwrap();
                let Some(usvg::Node::Text(text)) = tree.node_by_id("probe") else {
                    panic!("text must paint for {family}");
                };
                let selected: BTreeSet<_> = text
                    .layouted()
                    .iter()
                    .flat_map(|span| &span.positioned_glyphs)
                    .map(|glyph| {
                        tree.fontdb()
                            .face(glyph.font)
                            .unwrap()
                            .post_script_name
                            .as_str()
                    })
                    .collect();
                assert_eq!(selected, BTreeSet::from(["DejaVuSerif"]), "{family}");
            }
            return;
        }
        let root =
            std::env::temp_dir().join(format!("merman-symbol-fontconfig-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        for (name, bytes) in [
            (
                "symbol.ttf",
                include_bytes!("../tests/fonts/MermanSymbolicTest.ttf").as_slice(),
            ),
            (
                "serif.ttf",
                include_bytes!(
                    "../../merman-render/tests/fixtures/fonts/DejaVuSerif-NativeFilter.ttf"
                )
                .as_slice(),
            ),
        ] {
            std::fs::write(root.join(name), bytes).unwrap();
        }
        let config = root.join("fonts.conf");
        std::fs::write(&config, format!(r#"<fontconfig><dir>{}</dir><alias><family>serif</family><prefer><family>Merman Symbolic Test</family></prefer></alias></fontconfig>"#, root.display())).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "font_environment::tests::linux_fontconfig_symbol_alias_preserves_serif_and_zenuml_default_stack", "--nocapture"])
            .env(CHILD, "1").env("FONTCONFIG_FILE", config).output().unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn fixture_font_environment() -> ExportFontEnvironment {
        ExportFontEnvironment::from_resources(
            &FontCatalog::system_fonts(),
            &FontSourcePolicy::system_only(),
            || {
                let mut database = usvg::fontdb::Database::new();
                database.load_font_data(
                    include_bytes!(
                        "../../merman-render/tests/fixtures/fonts/DejaVuSerif-NativeFilter.ttf"
                    )
                    .to_vec(),
                );
                database.load_font_data(
                    include_bytes!(
                        "../../merman-render/tests/fixtures/fonts/DejaVuSansBold-NativeFilter.ttf"
                    )
                    .to_vec(),
                );
                let selection = FontSelectionPolicy::from_database(&database);
                (Arc::new(database), Arc::new(selection))
            },
        )
        .unwrap()
    }

    #[test]
    fn system_export_matches_unicode_family_case_before_generic_fallback() {
        let mut database = fixture_font_environment().fontdb.as_ref().clone();
        let mut target = database
            .faces()
            .find(|face| face.weight == usvg::fontdb::Weight::BOLD)
            .unwrap()
            .clone();
        target.families = vec![(
            "Maße Test Sans".to_owned(),
            usvg::fontdb::Language::English_UnitedStates,
        )];
        let target_id = database.push_face_info(target);
        database.set_serif_family("DejaVu Serif");
        let database = Arc::new(database);
        let mut boxes = Vec::new();
        // Quoted family names avoid svgtypes' unquoted identifier restriction for U+00DF.
        for family in [
            "'Maße Test Sans'",
            "'maße test sans'",
            "'__missing__', 'MASSE TEST SANS'",
        ] {
            assert!(
                svgtypes::parse_font_families(family).is_ok(),
                "the fixture must reach font selection"
            );
            let environment = ExportFontEnvironment::from_resources(
                &FontCatalog::system_fonts(),
                &FontSourcePolicy::system_only(),
                || {
                    (
                        Arc::clone(&database),
                        Arc::new(FontSelectionPolicy::from_database(&database)),
                    )
                },
            )
            .unwrap();
            let options = usvg::Options {
                fontdb: Arc::clone(&database),
                font_resolver: environment.resolver,
                ..usvg::Options::default()
            };
            let source = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><text x="0" y="40" font-family="{family}" font-weight="700">Browse Products</text></svg>"#
            );
            let tree = usvg::Tree::from_str(&source, &options).unwrap();
            let text = tree
                .root()
                .children()
                .iter()
                .find_map(|node| {
                    if let usvg::Node::Text(text) = node {
                        Some(text)
                    } else {
                        None
                    }
                })
                .unwrap();
            let glyphs = text
                .layouted()
                .iter()
                .flat_map(|span| &span.positioned_glyphs)
                .collect::<Vec<_>>();
            assert!(!glyphs.is_empty());
            assert!(
                glyphs.iter().all(|glyph| glyph.font == target_id),
                "requested stack must select its named face: {family}; expected {target_id:?}, actual {:?}",
                glyphs.iter().map(|glyph| glyph.font).collect::<Vec<_>>()
            );
            boxes.push(text.bounding_box());
            assert!(
                !environment
                    .plan
                    .finish_with_tree(&tree, &OperationControl::new())
                    .unwrap()
                    .family_fallback_used()
            );
        }
        assert!(
            boxes.windows(2).all(|pair| pair[0] == pair[1]),
            "equivalent case-folded requests must retain shaped metrics"
        );
    }

    #[test]
    fn native_tree_classifies_system_and_unadmitted_faces_and_missing_glyphs() {
        let environment = fixture_font_environment();
        let options = usvg::Options {
            fontdb: Arc::clone(&environment.fontdb),
            font_resolver: environment.resolver,
            ..usvg::Options::default()
        };
        let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="500" height="100">
            <text x="0" y="20" font-family="DejaVu Serif">Request Volume</text>
            <text x="0" y="50" font-family="DejaVu Sans" font-weight="700">Browse Products</text>
            <text x="0" y="80" font-family="DejaVu Sans" font-weight="700">Browse Products</text>
        </svg>"#;
        let tree = usvg::Tree::from_str(source, &options).unwrap();
        let plan = environment
            .plan
            .finish_with_tree(&tree, &OperationControl::new())
            .unwrap();
        assert!(plan.used_system_fonts());
        assert!(plan.is_host_dependent());
        assert_eq!(plan.unclassified_face_count(), 0);
        assert_eq!(plan.notdef_glyph_count(), 0);
        assert!(!plan.unresolved_glyph_fallback());

        let unadmitted = FinalTreeFontEvidence::collect(
            &tree,
            &usvg::fontdb::Database::new(),
            &OperationControl::new(),
        )
        .unwrap();
        assert!(!unadmitted.used_system_fonts);
        assert_eq!(unadmitted.unclassified_face_count, 2);

        let missing_source = source.replace("Request Volume", "Request Volume&#x10ffff;");
        let missing_tree = usvg::Tree::from_str(&missing_source, &options).unwrap();
        let missing = environment
            .plan
            .finish_with_tree(&missing_tree, &OperationControl::new())
            .unwrap();
        assert!(missing.notdef_glyph_count() > 0);
        assert!(missing.unresolved_glyph_fallback());
    }

    #[cfg(feature = "native-fonts")]
    #[test]
    fn native_export_coverage_with_and_without_omission_evidence() {
        let control = OperationControl::new();
        let context = |fonts: &[&[u8]]| {
            crate::NativeFontContext::from_font_bytes(
                fonts.iter().map(|bytes| bytes.to_vec()),
                crate::NativeFontLimits::default(),
                &control,
            )
            .unwrap()
            .with_strict_glyph_coverage(true)
        };
        let symbolic_bytes = include_bytes!("../tests/fonts/MermanSymbolicTest.ttf");
        let noto_bytes = include_bytes!("../tests/fonts/NotoSansSC-subset.ttf");
        let symbolic = context(&[symbolic_bytes]);
        let noto = context(&[noto_bytes]);
        let mixed = context(&[noto_bytes, symbolic_bytes]);
        for (context, body, vendored, registry) in [
            (
                &mixed,
                r#"<text id="probe" y="20" font-family="Noto Sans SC">中<tspan font-family="Merman Symbolic Test">A</tspan></text>"#,
                (false, false),
                (false, true),
            ),
            (
                &symbolic,
                r#"<text id="probe" y="20" font-family="sans-serif">A</text>"#,
                (true, false),
                (true, false),
            ),
            (
                &symbolic,
                r#"<text id="probe" y="20" font-family="sans-serif">A<tspan font-family="Merman Symbolic Test">A</tspan></text>"#,
                (true, false),
                (true, false),
            ),
            (
                &symbolic,
                r#"<text id="probe" y="20" visibility="hidden" font-family="sans-serif">A</text>"#,
                (false, false),
                (true, false),
            ),
            (
                &symbolic,
                r#"<text id="probe" y="20" display="none" font-family="sans-serif">A</text>"#,
                (false, false),
                (false, false),
            ),
            (
                &noto,
                r#"<g transform="translate(8 0)"><text id="probe" y="20" font-family="Noto Sans SC">🚀</text></g>"#,
                (false, true),
                (false, true),
            ),
            (
                &noto,
                r#"<text id="probe" y="20" visibility="hidden" font-family="Noto Sans SC">🚀</text>"#,
                (false, false),
                (false, true),
            ),
            (
                &noto,
                r#"<text id="probe" y="20" font-family="Noto Sans SC">中<tspan visibility="hidden">🚀</tspan></text>"#,
                (false, false),
                (false, true),
            ),
            (
                &noto,
                r#"<g opacity="0"><text id="probe" y="20" font-family="Noto Sans SC">🚀</text></g>"#,
                (false, false),
                (false, true),
            ),
        ] {
            let source = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="60">{body}</svg>"#
            );
            let mut options = usvg::Options::default();
            let observer = context.apply_to_usvg_options_for_export(&mut options, &control);
            let seed = ExportFontPlanSeed::from_native_context(context, observer);
            let tree = usvg::Tree::from_str(&source, &options).unwrap();
            let plan = seed.finish_with_tree(&tree, &control).unwrap();
            assert_eq!(plan.notdef_glyph_count(), 0, "{body}");
            let expected = if tree.root().text_font_omissions().is_some() {
                vendored
            } else {
                registry
            };
            assert_eq!(
                (
                    plan.unresolved_font_request(),
                    plan.unresolved_glyph_fallback()
                ),
                expected,
                "{body}"
            );
            let result = crate::check_export_glyph_coverage(context, plan);
            if expected.0 || expected.1 {
                assert!(matches!(
                    result,
                    Err(ExportError::NativeFont(
                        crate::NativeFontError::MissingGlyphCoverage
                    ))
                ));
            } else {
                result.unwrap();
            }
        }
    }

    #[test]
    fn final_glyph_evidence_observes_cancellation_inside_a_text_run() {
        let environment = fixture_font_environment();
        let options = usvg::Options {
            fontdb: Arc::clone(&environment.fontdb),
            font_resolver: environment.resolver,
            ..usvg::Options::default()
        };
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="5000" height="100"><text x="0" y="30" font-family="DejaVu Serif">{}</text></svg>"#,
            "Visible glyphs ".repeat(20),
        );
        let tree = usvg::Tree::from_str(&source, &options).unwrap();
        let glyphs = tree
            .root()
            .children()
            .iter()
            .filter_map(|node| match node {
                usvg::Node::Text(text) => Some(text),
                _ => None,
            })
            .flat_map(|text| text.layouted())
            .flat_map(|span| &span.positioned_glyphs)
            .count();
        assert!(glyphs > 200);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(32);
        let error = environment
            .plan
            .finish_with_tree(&tree, &control)
            .unwrap_err();
        assert!(matches!(error, ExportError::Cancelled(cancelled)
            if cancelled.phase == merman_model::OperationPhase::Export));
        assert!(
            environment
                .plan
                .finish_with_tree(&tree, &OperationControl::new())
                .is_ok()
        );
    }

    #[test]
    fn final_glyph_evidence_replays_an_existing_resource_terminal() {
        let environment = fixture_font_environment();
        let tree = usvg::Tree::from_str(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#,
            &usvg::Options::default(),
        )
        .unwrap();
        let control = OperationControl::new();
        let terminal = control.terminate_resource_overflow(
            "max_svg_bytes",
            merman_model::OperationPhase::Postprocess,
            "svg_postprocess",
            u64::MAX,
            17,
            merman_model::OperationResourceProvenance::new(
                merman_model::OperationResourceDomain::Render,
                None,
                [],
            ),
        );
        control.cancel();
        let error = environment
            .plan
            .finish_with_tree(&tree, &control)
            .unwrap_err();
        assert!(
            matches!(error, ExportError::OperationResourceTerminal(actual) if actual == terminal)
        );
    }

    #[test]
    fn author_ids_do_not_change_native_font_classification() {
        let environment = fixture_font_environment();
        let options = usvg::Options {
            fontdb: Arc::clone(&environment.fontdb),
            font_resolver: environment.resolver,
            ..usvg::Options::default()
        };
        let plan = |id| {
            let source = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="500" height="100"><text id="{id}" x="0" y="20" font-family="DejaVu Serif">Request Volume</text></svg>"#,
            );
            let tree = usvg::Tree::from_str(&source, &options).unwrap();
            environment
                .plan
                .finish_with_tree(&tree, &OperationControl::new())
                .unwrap()
        };
        let ordinary = plan("ordinary-label");
        assert!(ordinary.used_system_fonts());
        assert!(ordinary.is_host_dependent());
        for id in [
            "merman-prepared-flowchart-7",
            "merman-prepared-flowchart-7-line-invalid",
            "merman&#x2d;prepared-flowchart-7",
        ] {
            assert_eq!(plan(id), ordinary, "author ID {id}");
        }
    }

    #[cfg(feature = "native-fonts")]
    #[test]
    fn explicit_native_context_is_host_owned_without_claiming_a_system_face() {
        let control = merman_model::OperationControl::new();
        let context = crate::NativeFontContext::from_font_bytes(
            [include_bytes!("../tests/fonts/Cousine-Regular.ttf").to_vec()],
            crate::NativeFontLimits::default(),
            &control,
        )
        .expect("explicit fixture font loads");
        let mut options = usvg::Options::default();
        let observer = context.apply_to_usvg_options_for_export(&mut options, &control);
        let seed = ExportFontPlanSeed::from_native_context(&context, observer);
        let tree = usvg::Tree::from_str(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text x="0" y="20" font-family="Cousine">native context</text></svg>"#,
            &options,
        )
        .expect("explicit native context parses text");
        let glyph_count = tree
            .root()
            .children()
            .iter()
            .filter_map(|node| match node {
                usvg::Node::Text(text) => Some(text),
                _ => None,
            })
            .flat_map(|text| text.layouted().iter())
            .flat_map(|span| span.positioned_glyphs.iter())
            .count();
        assert!(
            glyph_count > 0,
            "fixture must exercise an actual native glyph"
        );
        let plan = seed
            .finish_with_tree(&tree, &OperationControl::new())
            .unwrap();

        assert_eq!(plan.source_mode(), ExportFontMode::NativeContext);
        assert!(!plan.used_system_fonts());
        assert_eq!(plan.unclassified_face_count(), 0);
        assert!(plan.is_host_dependent());
    }
}
