use super::{ExportError, Result};
use merman_render::diagram_theme::{FontCatalog, FontSource};
use merman_render::svg::ResvgCompatibleSvg;
use std::collections::{BTreeSet, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

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
        Self::from_resources(
            svg.font_catalog(),
            svg.font_source_policy(),
            shared_system_fontdb,
        )
    }

    fn from_resources(
        catalog: &FontCatalog,
        policy: &merman_render::diagram_theme::FontSourcePolicy,
        system_fonts: impl FnOnce() -> Arc<usvg::fontdb::Database>,
    ) -> Result<Self> {
        if !policy.priority().any(|source| {
            source == FontSource::System
                && catalog
                    .available_sources()
                    .any(|available| available == source)
        }) {
            return Err(ExportError::FontSourceUnavailable);
        }

        let fontdb = system_fonts();
        let recorder = Arc::new(FontResolutionRecorder::default());
        let default_family = default_font_family(&fontdb).unwrap_or_else(|| "Arial".to_owned());
        let resolver = system_font_resolver(Arc::clone(&fontdb), Arc::clone(&recorder));

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

fn system_font_resolver(
    fontdb: Arc<usvg::fontdb::Database>,
    recorder: Arc<FontResolutionRecorder>,
) -> usvg::FontResolver<'static> {
    let select_fontdb = Arc::clone(&fontdb);
    let select_recorder = Arc::clone(&recorder);

    usvg::FontResolver {
        select_font: Box::new(move |font, combined_db| {
            if let Some(id) = super::select_requested_font_case_insensitively(font, &select_fontdb)
            {
                debug_assert!(combined_db.face(id).is_some());
                return Some(id);
            }
            if let Some(id) = query_default_serif(font, &select_fontdb)
                .or_else(|| query_browser_like_fallback_font(font, &select_fontdb))
                .or_else(|| select_fontdb.faces().next().map(|face| face.id))
            {
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
            let base_face = excluded.first().and_then(|id| combined_db.face(*id));
            let selected = fontdb.faces().find_map(|face| {
                if excluded.contains(&face.id) {
                    return None;
                }
                if let Some(base) = base_face
                    && base.style != face.style
                    && base.weight != face.weight
                    && base.stretch != face.stretch
                {
                    return None;
                }
                face_has_char(&fontdb, face.id, character).then_some(face.id)
            });
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

fn face_has_char(database: &usvg::fontdb::Database, id: usvg::fontdb::ID, character: char) -> bool {
    database
        .with_face_data(id, |data, face_index| {
            ttf_parser::Face::parse(data, face_index)
                .ok()
                .and_then(|face| face.glyph_index(character))
                .is_some()
        })
        .unwrap_or(false)
}

fn query_default_serif(
    font: &usvg::Font,
    fontdb: &usvg::fontdb::Database,
) -> Option<usvg::fontdb::ID> {
    fontdb.query(&usvg::fontdb::Query {
        families: &[usvg::fontdb::Family::Serif],
        weight: usvg::fontdb::Weight(font.weight()),
        stretch: font.stretch().into(),
        style: font.style().into(),
    })
}

fn query_browser_like_fallback_font(
    font: &usvg::Font,
    fontdb: &usvg::fontdb::Database,
) -> Option<usvg::fontdb::ID> {
    let families = if font_requests_monospace(font) {
        [
            usvg::fontdb::Family::Monospace,
            usvg::fontdb::Family::SansSerif,
            usvg::fontdb::Family::Serif,
        ]
    } else {
        [
            usvg::fontdb::Family::SansSerif,
            usvg::fontdb::Family::Serif,
            usvg::fontdb::Family::Monospace,
        ]
    };

    fontdb.query(&usvg::fontdb::Query {
        families: &families,
        weight: usvg::fontdb::Weight(font.weight()),
        stretch: font.stretch().into(),
        style: font.style().into(),
    })
}

fn font_requests_monospace(font: &usvg::Font) -> bool {
    font.families().iter().any(|family| match family {
        usvg::FontFamily::Monospace => true,
        usvg::FontFamily::Named(name) => {
            let name = name.to_ascii_lowercase();
            name.contains("mono")
                || name.contains("courier")
                || name.contains("consolas")
                || name.contains("menlo")
        }
        _ => false,
    })
}

pub(crate) fn shared_system_fontdb() -> Arc<usvg::fontdb::Database> {
    static FONTDB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    Arc::clone(FONTDB.get_or_init(|| {
        let mut fontdb = usvg::fontdb::Database::new();
        fontdb.load_system_fonts();
        configure_fontdb_generic_families(&mut fontdb);
        Arc::new(fontdb)
    }))
}

fn configure_fontdb_generic_families(fontdb: &mut usvg::fontdb::Database) {
    let sans = first_font_family(fontdb, |face| !face.monospaced)
        .or_else(|| first_font_family(fontdb, |_| true));
    let mono = first_font_family(fontdb, |face| face.monospaced).or_else(|| sans.clone());

    if query_normal_font_family(fontdb, usvg::fontdb::Family::SansSerif).is_none()
        && let Some(family) = sans.as_ref()
    {
        fontdb.set_sans_serif_family(family.clone());
    }
    if query_normal_font_family(fontdb, usvg::fontdb::Family::Serif).is_none()
        && let Some(family) = sans.as_ref()
    {
        fontdb.set_serif_family(family.clone());
    }
    if query_normal_font_family(fontdb, usvg::fontdb::Family::Monospace).is_none()
        && let Some(family) = mono.as_ref()
    {
        fontdb.set_monospace_family(family.clone());
    }
}

fn default_font_family(fontdb: &usvg::fontdb::Database) -> Option<String> {
    query_normal_font_family(fontdb, usvg::fontdb::Family::SansSerif)
        .or_else(|| query_normal_font_family(fontdb, usvg::fontdb::Family::Serif))
        .or_else(|| first_font_family(fontdb, |_| true))
}

fn query_normal_font_family(
    fontdb: &usvg::fontdb::Database,
    family: usvg::fontdb::Family<'_>,
) -> Option<String> {
    let families = [family];
    fontdb
        .query(&usvg::fontdb::Query {
            families: &families,
            weight: usvg::fontdb::Weight::NORMAL,
            stretch: usvg::fontdb::Stretch::Normal,
            style: usvg::fontdb::Style::Normal,
        })
        .and_then(|id| fontdb.face(id))
        .and_then(face_family_name)
}

fn first_font_family<F>(fontdb: &usvg::fontdb::Database, mut predicate: F) -> Option<String>
where
    F: FnMut(&usvg::fontdb::FaceInfo) -> bool,
{
    fontdb
        .faces()
        .find(|face| predicate(face))
        .and_then(face_family_name)
}

fn face_family_name(face: &usvg::fontdb::FaceInfo) -> Option<String> {
    face.families
        .iter()
        .find(|(_, lang)| *lang == usvg::fontdb::Language::English_UnitedStates)
        .or_else(|| face.families.first())
        .map(|(family, _)| family.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
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
                Arc::new(database)
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
                || Arc::clone(&database),
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
        let control = merman_core::OperationControl::new();
        let context = crate::NativeFontOptions::default()
            .with_font_file("tests/fonts/Cousine-Regular.ttf")
            .load(&control)
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
