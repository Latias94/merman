#[cfg(any(all(feature = "system-fonts", not(target_family = "wasm")), test))]
use super::font_selection::FontSelectionPolicy;
use super::{ExportError, Result};
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

    /// Returns whether usvg could not select any face for at least one text run.
    pub const fn unresolved_font_request(self) -> bool {
        self.unresolved_font_request
    }

    /// Returns whether usvg could not find a fallback face for at least one character.
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

    pub(crate) fn finish_with_tree(&self, tree: &usvg::Tree) -> ExportFontPlan {
        #[cfg(feature = "native-fonts")]
        let evidence = self.native_export.as_ref().map_or_else(
            || FinalTreeFontEvidence::collect(tree, &self.system_fonts),
            |observer| FinalTreeFontEvidence::collect_native(tree, observer),
        );
        #[cfg(not(feature = "native-fonts"))]
        let evidence = FinalTreeFontEvidence::collect(tree, &self.system_fonts);
        let unresolved_glyph_fallback = self
            .recorder
            .unresolved_glyph_fallback
            .load(Ordering::Relaxed)
            || evidence.notdef_glyph_count != 0;
        #[cfg(feature = "native-fonts")]
        let native_facts = self.native_export.as_ref().map(|observer| {
            (
                observer.family_fallback_used(),
                observer.glyph_fallback_used(),
                observer.unresolved_font_request(),
                observer.unresolved_glyph_fallback(),
            )
        });
        #[cfg(not(feature = "native-fonts"))]
        let native_facts: Option<(bool, bool, bool, bool)> = None;

        ExportFontPlan {
            source_mode: self.source_mode,
            // Native contexts derive this from visible final glyphs and the exact resolver's
            // operation-local admission record, not from an initial fontdb snapshot.
            used_system_fonts: evidence.used_system_fonts,
            family_fallback_used: self.recorder.family_fallback_used.load(Ordering::Relaxed)
                || native_facts.is_some_and(|facts| facts.0),
            glyph_fallback_used: self.recorder.glyph_fallback_used.load(Ordering::Relaxed)
                || native_facts.is_some_and(|facts| facts.1),
            unresolved_font_request: self
                .recorder
                .unresolved_font_request
                .load(Ordering::Relaxed)
                || native_facts.is_some_and(|facts| facts.2),
            unresolved_glyph_fallback: unresolved_glyph_fallback
                || native_facts.is_some_and(|facts| facts.3),
            unclassified_face_count: evidence.unclassified_face_count,
            notdef_glyph_count: evidence.notdef_glyph_count,
        }
    }
}

#[derive(Debug, Default)]
struct FinalTreeFontEvidence {
    used_system_fonts: bool,
    unclassified_face_count: usize,
    notdef_glyph_count: usize,
}

impl FinalTreeFontEvidence {
    fn collect(tree: &usvg::Tree, system_fonts: &usvg::fontdb::Database) -> Self {
        Self::collect_admitted(
            tree,
            system_fonts.faces().map(|face| face.id).collect(),
            BTreeSet::new(),
        )
    }

    #[cfg(feature = "native-fonts")]
    fn collect_native(
        tree: &usvg::Tree,
        observer: &crate::fonts::NativeExportFontObserver,
    ) -> Self {
        let mut system_faces = BTreeSet::new();
        let mut caller_faces = BTreeSet::new();
        for (id, source) in observer.admitted_faces() {
            match source {
                crate::fonts::NativeExportFontSource::System => {
                    system_faces.insert(id);
                }
                crate::fonts::NativeExportFontSource::Caller => {
                    caller_faces.insert(id);
                }
            }
        }
        Self::collect_admitted(tree, system_faces, caller_faces)
    }

    fn collect_admitted(
        tree: &usvg::Tree,
        system_faces: BTreeSet<usvg::fontdb::ID>,
        caller_faces: BTreeSet<usvg::fontdb::ID>,
    ) -> Self {
        let mut evidence = Self::default();
        let mut unclassified_faces = HashSet::new();
        let mut visited_groups = HashSet::new();
        evidence.visit_group(
            tree.root(),
            &system_faces,
            &caller_faces,
            &mut unclassified_faces,
            &mut visited_groups,
        );
        evidence.unclassified_face_count = unclassified_faces.len();
        evidence
    }

    fn visit_group(
        &mut self,
        group: &usvg::Group,
        system_faces: &BTreeSet<usvg::fontdb::ID>,
        caller_faces: &BTreeSet<usvg::fontdb::ID>,
        unclassified_faces: &mut HashSet<usvg::fontdb::ID>,
        visited_groups: &mut HashSet<*const usvg::Group>,
    ) {
        if !visited_groups.insert(std::ptr::from_ref(group)) {
            return;
        }
        for node in group.children() {
            match node {
                usvg::Node::Group(group) => self.visit_group(
                    group,
                    system_faces,
                    caller_faces,
                    unclassified_faces,
                    visited_groups,
                ),
                usvg::Node::Text(text) => {
                    self.visit_text(text, system_faces, caller_faces, unclassified_faces)
                }
                usvg::Node::Path(_) | usvg::Node::Image(_) => {}
            }
            node.subroots(|subroot| {
                self.visit_group(
                    subroot,
                    system_faces,
                    caller_faces,
                    unclassified_faces,
                    visited_groups,
                );
            });
        }
    }

    fn visit_text(
        &mut self,
        text: &usvg::Text,
        system_faces: &BTreeSet<usvg::fontdb::ID>,
        caller_faces: &BTreeSet<usvg::fontdb::ID>,
        unclassified_faces: &mut HashSet<usvg::fontdb::ID>,
    ) {
        for span in text.layouted().iter().filter(|span| span.visible) {
            for glyph in &span.positioned_glyphs {
                if glyph.id.0 == 0 {
                    self.notdef_glyph_count = self.notdef_glyph_count.saturating_add(1);
                }
                if system_faces.contains(&glyph.font) {
                    self.used_system_fonts = true;
                } else if !caller_faces.contains(&glyph.font) {
                    unclassified_faces.insert(glyph.font);
                }
            }
        }
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
                let selection = Arc::new(FontSelectionPolicy::from_database(&database));
                let database = Arc::new(database);
                let recorder = Arc::new(FontResolutionRecorder::default());
                let options = usvg::Options {
                    fontdb: Arc::clone(&database),
                    font_resolver: system_font_resolver(database, selection, Arc::clone(&recorder)),
                    ..Default::default()
                };
                let tree = usvg::Tree::from_str(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="80"><text id="probe" x="0" y="40" font-family="Noto Sans">Aam</text></svg>"#,
                    &options,
                ).unwrap();
                let Some(usvg::Node::Text(text)) = tree.node_by_id("probe") else {
                    panic!("partial text must still be painted");
                };
                let expected = if include_text_fallback {
                    "Cousine-Regular"
                } else {
                    "NotoSans-Regular"
                };
                let glyphs: Vec<_> = text
                    .layouted()
                    .iter()
                    .flat_map(|span| &span.positioned_glyphs)
                    .collect();
                assert!(!glyphs.is_empty());
                assert!(glyphs.iter().all(|glyph| {
                    tree.fontdb().face(glyph.font).unwrap().post_script_name == expected
                }));
                assert_eq!(
                    recorder.glyph_fallback_used.load(Ordering::Relaxed),
                    include_text_fallback
                );
                assert_eq!(
                    recorder.unresolved_glyph_fallback.load(Ordering::Relaxed),
                    !include_text_fallback
                );
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
                    .finish_with_tree(&tree)
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
        let plan = environment.plan.finish_with_tree(&tree);
        assert!(plan.used_system_fonts());
        assert!(plan.is_host_dependent());
        assert_eq!(plan.unclassified_face_count(), 0);
        assert_eq!(plan.notdef_glyph_count(), 0);
        assert!(!plan.unresolved_glyph_fallback());

        let unadmitted = FinalTreeFontEvidence::collect(&tree, &usvg::fontdb::Database::new());
        assert!(!unadmitted.used_system_fonts);
        assert_eq!(unadmitted.unclassified_face_count, 2);

        let missing_source = source.replace("Request Volume", "Request Volume&#x10ffff;");
        let missing_tree = usvg::Tree::from_str(&missing_source, &options).unwrap();
        let missing = environment.plan.finish_with_tree(&missing_tree);
        assert!(missing.notdef_glyph_count() > 0);
        assert!(missing.unresolved_glyph_fallback());
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
            environment.plan.finish_with_tree(&tree)
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
        let plan = seed.finish_with_tree(&tree);

        assert_eq!(plan.source_mode(), ExportFontMode::NativeContext);
        assert!(!plan.used_system_fonts());
        assert_eq!(plan.unclassified_face_count(), 0);
        assert!(plan.is_host_dependent());
    }
}
