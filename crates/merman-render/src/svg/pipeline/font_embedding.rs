use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

use base64::Engine as _;
use cssparser::{BasicParseErrorKind, Delimiter, Parser, ParserInput, Token};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::name::{NamespaceResolver, ResolveResult};
use quick_xml::reader::NsReader;

use crate::diagram_theme::{
    FontAssetFingerprint, FontCatalog, FontCatalogFingerprint, FontContainer,
    FontEmbeddingRequirement, FontSource, FontStyle,
};
use crate::text::{
    PreparedTextLabelId, PreparedTextLabelLedgerEntry, PreparedTextLabelProvenance,
    parse_css_font_stack,
};
use crate::{Error, Result};

use super::builtin::util::{SvgTagScanner, start_tag_name};

pub(super) const TYPED_FONT_STYLE_ATTRIBUTE: &str = "data-merman-typed-fonts";
pub(super) const TYPED_FONT_STYLE_VERSION: &str = "v1";
const SVG_NAMESPACE: &[u8] = b"http://www.w3.org/2000/svg";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SvgFontSeal {
    text_element_count: usize,
    prepared_label_count: usize,
    prepared_text_element_count: usize,
    embedded_face_count: usize,
    catalog_fingerprint: Option<FontCatalogFingerprint>,
}

impl SvgFontSeal {
    pub(super) const fn unsealed(text_element_count: usize) -> Self {
        Self {
            text_element_count,
            prepared_label_count: 0,
            prepared_text_element_count: 0,
            embedded_face_count: 0,
            catalog_fingerprint: None,
        }
    }

    pub(super) const fn not_required() -> Self {
        Self::unsealed(0)
    }

    fn embedded(plan: &SvgFontEmbeddingPlan, text_element_count: usize) -> Result<Self> {
        if text_element_count != plan.prepared_text_element_count {
            return Err(font_embedding_error(format!(
                "typed font plan covers {} prepared text elements but terminal SVG contains {text_element_count} text elements",
                plan.prepared_text_element_count
            )));
        }
        Ok(Self {
            text_element_count,
            prepared_label_count: plan.prepared_label_count,
            prepared_text_element_count: plan.prepared_text_element_count,
            embedded_face_count: plan.faces.len(),
            catalog_fingerprint: Some(plan.catalog_fingerprint),
        })
    }

    pub(crate) const fn is_complete(&self) -> bool {
        self.text_element_count == 0
            || (self.catalog_fingerprint.is_some()
                && self.prepared_text_element_count == self.text_element_count)
    }

    pub(crate) const fn text_element_count(&self) -> usize {
        self.text_element_count
    }

    pub(crate) const fn embedded_face_count(&self) -> usize {
        self.embedded_face_count
    }

    pub(crate) const fn catalog_fingerprint(&self) -> Option<FontCatalogFingerprint> {
        self.catalog_fingerprint
    }
}

#[derive(Debug, Clone)]
pub(super) struct SvgFontEmbeddingPlan {
    catalog_fingerprint: FontCatalogFingerprint,
    prepared_label_count: usize,
    prepared_text_element_count: usize,
    faces: Box<[EmbeddedFontFace]>,
    css: String,
}

#[derive(Debug, Clone)]
struct EmbeddedFontFace {
    asset_fingerprint: FontAssetFingerprint,
    face_index: u32,
    family_name: String,
    container: FontContainer,
    canonical_bytes: std::sync::Arc<[u8]>,
    style: FontStyle,
    weight: u16,
    width: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct UsedFontFace {
    asset_fingerprint: FontAssetFingerprint,
    face_index: u32,
}

#[derive(Debug, Clone)]
struct PreparedLabelFont {
    family_name: String,
    style: FontStyle,
    weight: u16,
    width: u16,
    line_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EmittedFontDescriptor {
    family_name: String,
    style: FontStyle,
    weight: u16,
    width: u16,
}

#[derive(Debug, Default)]
enum PreparedLabelCoverage {
    #[default]
    Unseen,
    Base,
    Lines(BTreeSet<u32>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreparedTextTokenScope {
    Base,
    Line(u32),
}

/// Tracks SVG `<text>` elements that contain character content.
///
/// Mermaid may retain empty text/tspan nodes for DOM parity. Those nodes do not select or paint a
/// font face, so they must not participate in prepared-text coverage or the terminal font seal.
/// Whitespace still counts because `xml:space="preserve"` can make it rendering-significant.
#[derive(Debug, Default)]
pub(super) struct SvgTextContentTracker {
    mode: SvgTextTrackingMode,
    // One bit per open XML element identifies text end events without retaining its attributes.
    element_stack: Vec<bool>,
    // Only currently open text elements retain prepared evidence.
    open_text_stack: Vec<OpenSvgTextElement>,
    text_element_count: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum SvgTextTrackingMode {
    #[default]
    CountOnly,
    PreparedCoverage,
}

#[derive(Debug)]
struct OpenSvgTextElement {
    prepared: Option<PreparedSvgTextElement>,
    has_character_content: bool,
}

#[derive(Debug)]
struct PreparedSvgTextElement {
    label_token: Option<String>,
    style: Option<String>,
}

impl SvgTextContentTracker {
    fn for_prepared_coverage() -> Self {
        Self {
            mode: SvgTextTrackingMode::PreparedCoverage,
            ..Self::default()
        }
    }

    pub(super) fn observe_start(
        &mut self,
        element: &BytesStart<'_>,
        resolver: &NamespaceResolver,
    ) -> Option<()> {
        let (namespace, local_name) = resolver.resolve_element(element.name());
        let is_svg_element = match namespace {
            ResolveResult::Unknown(_) => return None,
            ResolveResult::Unbound => true,
            ResolveResult::Bound(namespace) => namespace.as_ref() == SVG_NAMESPACE,
        };
        let is_text = is_svg_element && local_name.as_ref().eq_ignore_ascii_case(b"text");
        let open_text = if is_text {
            Some(OpenSvgTextElement {
                prepared: match self.mode {
                    SvgTextTrackingMode::CountOnly => None,
                    SvgTextTrackingMode::PreparedCoverage => {
                        Some(parse_prepared_text_element(element)?)
                    }
                },
                has_character_content: false,
            })
        } else {
            None
        };
        self.element_stack.push(open_text.is_some());
        if let Some(open_text) = open_text {
            self.open_text_stack.push(open_text);
        }
        Some(())
    }

    pub(super) fn observe_end(&mut self) -> Option<()> {
        self.close_element()?;
        Some(())
    }

    fn observe_prepared_end(&mut self) -> Option<Option<PreparedSvgTextElement>> {
        (self.mode == SvgTextTrackingMode::PreparedCoverage).then_some(())?;
        self.close_element()
    }

    fn close_element(&mut self) -> Option<Option<PreparedSvgTextElement>> {
        let closes_text = self.element_stack.pop()?;
        if !closes_text {
            return Some(None);
        }

        let open_text = self.open_text_stack.pop()?;
        if !open_text.has_character_content {
            return Some(None);
        }

        self.text_element_count = self.text_element_count.checked_add(1)?;
        // Content belongs to the innermost text while it is open, then makes its parent
        // font-bearing when nested text closes.
        if let Some(parent) = self.open_text_stack.last_mut() {
            parent.has_character_content = true;
        }
        Some(open_text.prepared)
    }

    pub(super) fn observe_content(&mut self, content: &str) {
        if !content.is_empty() {
            if let Some(text_element) = self.open_text_stack.last_mut() {
                text_element.has_character_content = true;
            }
        }
    }

    pub(super) fn observe_character(&mut self, _character: char) {
        if let Some(text_element) = self.open_text_stack.last_mut() {
            text_element.has_character_content = true;
        }
    }

    pub(super) fn text_element_count(&self) -> usize {
        self.text_element_count
    }

    fn finish_prepared(self) -> Option<()> {
        (self.mode == SvgTextTrackingMode::PreparedCoverage
            && self.element_stack.is_empty()
            && self.open_text_stack.is_empty())
        .then_some(())
    }
}

fn parse_prepared_text_element(element: &BytesStart<'_>) -> Option<PreparedSvgTextElement> {
    let mut label_token = None;
    let mut style = None;
    for attribute in element.attributes() {
        let attribute = attribute.ok()?;
        let destination = match attribute.key.as_ref() {
            b"id" => &mut label_token,
            b"style" => &mut style,
            _ => continue,
        };
        if destination.is_some() {
            return None;
        }
        *destination = Some(
            attribute
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, element.decoder())
                .ok()?
                .into_owned(),
        );
    }
    Some(PreparedSvgTextElement { label_token, style })
}

impl SvgFontEmbeddingPlan {
    pub(super) fn for_prepared_text(
        catalog: &FontCatalog,
        ledger: &[PreparedTextLabelLedgerEntry],
        svg: &str,
    ) -> Option<Self> {
        if catalog.embedding_requirement() != FontEmbeddingRequirement::FullFont
            || ledger.is_empty()
        {
            return None;
        }

        let catalog_fingerprint = catalog.fingerprint();
        let assets_by_id = catalog
            .assets()
            .iter()
            .map(|asset| (asset.id(), asset.fingerprint()))
            .collect::<BTreeMap<_, _>>();
        let face_descriptors = catalog
            .faces()
            .iter()
            .filter_map(|face| {
                assets_by_id
                    .get(face.asset_id())
                    .copied()
                    .map(|fingerprint| {
                        (
                            (fingerprint, face.face_index()),
                            EmittedFontDescriptor {
                                family_name: face.family_name().to_string(),
                                style: face.style(),
                                weight: face.weight(),
                                width: face.width(),
                            },
                        )
                    })
            })
            .collect::<BTreeMap<_, _>>();
        let mut used_faces = BTreeSet::new();
        let mut label_fonts = HashMap::with_capacity(ledger.len());
        for entry in ledger {
            if entry.catalog_fingerprint() != catalog_fingerprint
                || entry.provenance() != PreparedTextLabelProvenance::Native
                || entry.evidence().is_empty()
            {
                return None;
            }
            let mut entry_face = None;
            for run in entry.evidence() {
                if run.font_source() != FontSource::Embedded {
                    return None;
                }
                let face = run.face_key();
                let used_face = UsedFontFace {
                    asset_fingerprint: face.asset_fingerprint(),
                    face_index: face.face_index(),
                };
                if entry_face
                    .replace(used_face)
                    .is_some_and(|prior| prior != used_face)
                {
                    return None;
                }
                used_faces.insert(used_face);
            }
            let used_face = entry_face?;
            let descriptor = face_descriptors
                .get(&(used_face.asset_fingerprint, used_face.face_index))?
                .clone();
            let line_count = entry.line_count();
            if line_count == 0 || u32::try_from(line_count).is_err() {
                return None;
            }
            if label_fonts
                .insert(
                    entry.id(),
                    PreparedLabelFont {
                        family_name: descriptor.family_name,
                        style: descriptor.style,
                        weight: descriptor.weight,
                        width: descriptor.width,
                        line_count,
                    },
                )
                .is_some()
            {
                return None;
            }
        }

        let prepared_text_element_count = scan_prepared_text_coverage(svg, &label_fonts)?;

        Self::from_used_faces(
            catalog,
            ledger.len(),
            prepared_text_element_count,
            used_faces,
        )
    }

    fn from_used_faces(
        catalog: &FontCatalog,
        prepared_label_count: usize,
        prepared_text_element_count: usize,
        used_faces: impl IntoIterator<Item = UsedFontFace>,
    ) -> Option<Self> {
        if catalog.embedding_requirement() != FontEmbeddingRequirement::FullFont
            || prepared_label_count == 0
            || prepared_text_element_count == 0
        {
            return None;
        }

        let assets_by_fingerprint = catalog
            .assets()
            .iter()
            .map(|asset| (asset.fingerprint(), asset))
            .collect::<BTreeMap<_, _>>();
        let asset_fingerprints_by_id = catalog
            .assets()
            .iter()
            .map(|asset| (asset.id(), asset.fingerprint()))
            .collect::<BTreeMap<_, _>>();
        let faces_by_key = catalog
            .faces()
            .iter()
            .filter_map(|face| {
                asset_fingerprints_by_id
                    .get(face.asset_id())
                    .copied()
                    .map(|fingerprint| ((fingerprint, face.face_index()), face))
            })
            .collect::<BTreeMap<_, _>>();

        let mut faces = Vec::new();
        let mut css = String::new();
        let mut descriptor_identities = BTreeSet::new();
        for used in used_faces {
            let asset = assets_by_fingerprint
                .get(&used.asset_fingerprint)
                .copied()?;
            let face = faces_by_key
                .get(&(used.asset_fingerprint, used.face_index))
                .copied()?;
            if !matches!(
                asset.canonical_container(),
                FontContainer::TrueType | FontContainer::OpenType
            ) {
                return None;
            }

            let embedded_face = EmbeddedFontFace {
                asset_fingerprint: used.asset_fingerprint,
                face_index: used.face_index,
                family_name: face.family_name().to_string(),
                container: asset.canonical_container(),
                canonical_bytes: asset.canonical_data(),
                style: face.style(),
                weight: face.weight(),
                width: face.width(),
            };
            if !descriptor_identities.insert(font_face_descriptor_identity(&embedded_face)) {
                return None;
            }
            append_font_face_rule(&mut css, &embedded_face);
            faces.push(embedded_face);
        }
        if faces.is_empty() {
            return None;
        }

        Some(Self {
            catalog_fingerprint: catalog.fingerprint(),
            prepared_label_count,
            prepared_text_element_count,
            faces: faces.into_boxed_slice(),
            css,
        })
    }

    pub(super) fn inject(&self, svg: &str) -> Result<String> {
        let mut scanner = SvgTagScanner::new(svg);
        while let Some(tag) = scanner.next() {
            if start_tag_name(tag.raw()) != Some("svg") {
                continue;
            }
            if tag.is_self_closing() {
                return Err(font_embedding_error(
                    "typed fonts cannot be attached to a self-closing SVG root",
                ));
            }
            let insertion = scanner.cursor();
            let mut out =
                String::with_capacity(svg.len().saturating_add(self.css.len()).saturating_add(96));
            out.push_str(&svg[..insertion]);
            write!(
                out,
                "<style {TYPED_FONT_STYLE_ATTRIBUTE}=\"{TYPED_FONT_STYLE_VERSION}\">{}</style>",
                self.css
            )
            .expect("writing to String cannot fail");
            out.push_str(&svg[insertion..]);
            return Ok(out);
        }
        Err(font_embedding_error(
            "typed fonts require a terminal SVG root element",
        ))
    }

    pub(super) fn validate_typed_style(&self, css: &str) -> Result<()> {
        if !font_faces_have_unique_descriptor_identities(&self.faces) {
            return Err(font_embedding_error(
                "typed font plan contains competing CSS face descriptors",
            ));
        }
        let mut expected = String::new();
        let mut seen = BTreeSet::new();
        for face in &self.faces {
            if !seen.insert((face.asset_fingerprint, face.face_index)) {
                return Err(font_embedding_error(
                    "typed font plan contains a duplicate canonical face",
                ));
            }
            append_font_face_rule(&mut expected, face);
        }
        if css != expected || expected != self.css {
            return Err(font_embedding_error(
                "typed font stylesheet does not match the renderer-owned embedding plan",
            ));
        }
        Ok(())
    }

    pub(super) fn seal(&self, text_element_count: usize) -> Result<SvgFontSeal> {
        SvgFontSeal::embedded(self, text_element_count)
    }
}

fn scan_prepared_text_coverage(
    svg: &str,
    label_fonts: &HashMap<PreparedTextLabelId, PreparedLabelFont>,
) -> Option<usize> {
    let mut coverage = label_fonts
        .keys()
        .copied()
        .map(|id| (id, PreparedLabelCoverage::Unseen))
        .collect::<HashMap<_, _>>();
    let mut reader = NsReader::from_str(svg);
    reader.config_mut().enable_all_checks(true);
    let mut tracker = SvgTextContentTracker::for_prepared_coverage();

    let text_element_count = loop {
        match reader.read_event().ok()? {
            Event::Start(element) => tracker.observe_start(&element, reader.resolver())?,
            Event::Empty(_) => {}
            Event::End(_) => {
                if let Some(text_element) = tracker.observe_prepared_end()? {
                    validate_prepared_text_element(text_element, label_fonts, &mut coverage)?;
                }
            }
            Event::Text(text) => tracker.observe_content(&text.xml10_content().ok()?),
            Event::CData(text) => tracker.observe_content(&text.xml10_content().ok()?),
            Event::GeneralRef(reference) => {
                tracker.observe_character(resolve_xml_reference(&reference)?)
            }
            Event::PI(_) | Event::DocType(_) => return None,
            Event::Decl(_) | Event::Comment(_) => {}
            Event::Eof => {
                let text_element_count = tracker.text_element_count();
                tracker.finish_prepared()?;
                break text_element_count;
            }
        }
    };

    if text_element_count == 0 || coverage.len() != label_fonts.len() {
        return None;
    }
    for (label_id, expected) in label_fonts {
        match coverage.get(label_id)? {
            PreparedLabelCoverage::Base => {}
            PreparedLabelCoverage::Lines(lines) if lines.len() == expected.line_count => {}
            PreparedLabelCoverage::Unseen | PreparedLabelCoverage::Lines(_) => return None,
        }
    }
    Some(text_element_count)
}

fn validate_prepared_text_element(
    text_element: PreparedSvgTextElement,
    label_fonts: &HashMap<PreparedTextLabelId, PreparedLabelFont>,
    coverage: &mut HashMap<PreparedTextLabelId, PreparedLabelCoverage>,
) -> Option<()> {
    let label_token = text_element.label_token.as_deref()?;
    let label_id = PreparedTextLabelId::from_svg_id(label_token)?;
    let expected = label_fonts.get(&label_id)?;
    let emitted = emitted_font_descriptor(text_element.style.as_deref()?)?;
    if !emitted
        .family_name
        .eq_ignore_ascii_case(&expected.family_name)
        || emitted.style != expected.style
        || emitted.weight != expected.weight
        || emitted.width != expected.width
    {
        return None;
    }
    let scope = prepared_text_token_scope(label_token, label_id)?;
    let label_coverage = coverage.get_mut(&label_id)?;
    match scope {
        PreparedTextTokenScope::Base => match label_coverage {
            PreparedLabelCoverage::Unseen => *label_coverage = PreparedLabelCoverage::Base,
            PreparedLabelCoverage::Base | PreparedLabelCoverage::Lines(_) => return None,
        },
        PreparedTextTokenScope::Line(line) => {
            let line_count = u32::try_from(expected.line_count).ok()?;
            if line >= line_count {
                return None;
            }
            match label_coverage {
                PreparedLabelCoverage::Unseen => {
                    *label_coverage = PreparedLabelCoverage::Lines(BTreeSet::from([line]));
                }
                PreparedLabelCoverage::Base => return None,
                PreparedLabelCoverage::Lines(lines) => {
                    if !lines.insert(line) {
                        return None;
                    }
                }
            }
        }
    }
    Some(())
}

fn resolve_xml_reference(reference: &BytesRef<'_>) -> Option<char> {
    if let Some(value) = reference.resolve_char_ref().ok()? {
        return crate::xml::is_xml_1_0_char(value).then_some(value);
    }
    match reference.decode().ok()?.as_ref() {
        "amp" => Some('&'),
        "apos" => Some('\''),
        "gt" => Some('>'),
        "lt" => Some('<'),
        "quot" => Some('"'),
        _ => None,
    }
}

fn emitted_font_descriptor(style: &str) -> Option<EmittedFontDescriptor> {
    let mut input = ParserInput::new(style);
    let mut parser = Parser::new(&mut input);
    let mut family = None;
    let mut weight = None;
    let mut font_style = None;
    let mut width = None;

    while !parser.is_exhausted() {
        parser.skip_whitespace();
        if parser.is_exhausted() {
            break;
        }
        let declaration_start = parser.position();
        parser
            .parse_until_after(Delimiter::Semicolon, |declaration| {
                consume_css_component_values(declaration, 0)
            })
            .ok()?;
        let declaration_end = parser.position();
        let raw = parser.slice(declaration_start..declaration_end);
        let parsed = crate::mermaid_style::parse_style_declaration(raw)?;
        match parsed.property() {
            "font-family" => {
                if family.is_some() {
                    return None;
                }
                let stack = parse_css_font_stack(parsed.value())?;
                if stack.families().len() != 1 {
                    return None;
                }
                family = Some(stack.families()[0].clone());
            }
            "font-weight" => {
                if weight
                    .replace(parse_emitted_font_weight(parsed.value())?)
                    .is_some()
                {
                    return None;
                }
            }
            "font-style" => {
                if font_style
                    .replace(parse_emitted_font_style(parsed.value())?)
                    .is_some()
                {
                    return None;
                }
            }
            "font-stretch" => {
                if width
                    .replace(parse_emitted_font_width(parsed.value())?)
                    .is_some()
                {
                    return None;
                }
            }
            _ => {}
        }
    }

    Some(EmittedFontDescriptor {
        family_name: family?,
        style: font_style?,
        weight: weight?,
        width: width.unwrap_or(5),
    })
}

fn parse_emitted_font_weight(value: &str) -> Option<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(400),
        "bold" => Some(700),
        "bolder" | "lighter" => None,
        value => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=1000).contains(weight)),
    }
}

fn parse_emitted_font_style(value: &str) -> Option<FontStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(FontStyle::Normal),
        "italic" => Some(FontStyle::Italic),
        "oblique" => Some(FontStyle::Oblique),
        _ => None,
    }
}

fn parse_emitted_font_width(value: &str) -> Option<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "ultra-condensed" => Some(1),
        "extra-condensed" => Some(2),
        "condensed" => Some(3),
        "semi-condensed" => Some(4),
        "normal" => Some(5),
        "semi-expanded" => Some(6),
        "expanded" => Some(7),
        "extra-expanded" => Some(8),
        "ultra-expanded" => Some(9),
        _ => None,
    }
}

fn prepared_text_token_scope(
    token: &str,
    label_id: PreparedTextLabelId,
) -> Option<PreparedTextTokenScope> {
    let base = label_id.as_svg_id();
    if token == base {
        return Some(PreparedTextTokenScope::Base);
    }
    let suffix = token.strip_prefix(&base)?.strip_prefix("-line-")?;
    let line = parse_canonical_u32(suffix)?;
    Some(PreparedTextTokenScope::Line(line))
}

fn parse_canonical_u32(value: &str) -> Option<u32> {
    if value.is_empty() || (value.len() > 1 && value.starts_with('0')) {
        return None;
    }
    value.parse().ok()
}

fn consume_css_component_values<'i, 't>(
    input: &mut Parser<'i, 't>,
    depth: u8,
) -> std::result::Result<(), cssparser::ParseError<'i, ()>> {
    if depth >= 64 {
        return Err(input.new_custom_error(()));
    }
    loop {
        let token = match input.next_including_whitespace() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        if matches!(
            token,
            Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock
        ) {
            input.parse_nested_block(|nested| {
                consume_css_component_values(nested, depth.saturating_add(1))
            })?;
        }
    }
}

pub(super) fn stylesheet_contains_font_face(css: &str) -> bool {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    css_contains_font_face(&mut parser, 0).unwrap_or(false)
}

fn css_contains_font_face<'i, 't>(
    input: &mut Parser<'i, 't>,
    depth: u8,
) -> std::result::Result<bool, cssparser::ParseError<'i, ()>> {
    if depth >= 64 {
        return Err(input.new_custom_error(()));
    }
    loop {
        let token = match input.next_including_whitespace() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => {
                return Ok(false);
            }
            Err(error) => return Err(error.into()),
        };
        match token {
            Token::AtKeyword(name) if name.eq_ignore_ascii_case("font-face") => return Ok(true),
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                if input.parse_nested_block(|nested| {
                    css_contains_font_face(nested, depth.saturating_add(1))
                })? {
                    return Ok(true);
                }
            }
            _ => {}
        }
    }
}

fn append_font_face_rule(css: &mut String, face: &EmbeddedFontFace) {
    let (mime, format) = match face.container {
        FontContainer::TrueType => ("font/ttf", "truetype"),
        FontContainer::OpenType => ("font/otf", "opentype"),
        FontContainer::Collection | FontContainer::Woff2 => {
            unreachable!("unsupported canonical containers are rejected before CSS emission")
        }
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(face.canonical_bytes.as_ref());

    css.push_str("@font-face{font-family:\"");
    append_css_string_content(css, &face.family_name);
    write!(
        css,
        "\";src:url(\"data:{mime};base64,{encoded}\") format(\"{format}\");font-style:{};font-weight:{};font-stretch:{}}}",
        face.style.id(),
        face.weight,
        font_width_keyword(face.width),
    )
    .expect("writing to String cannot fail");
}

fn font_faces_have_unique_descriptor_identities(faces: &[EmbeddedFontFace]) -> bool {
    let mut identities = BTreeSet::new();
    faces
        .iter()
        .all(|face| identities.insert(font_face_descriptor_identity(face)))
}

fn font_face_descriptor_identity(face: &EmbeddedFontFace) -> (String, FontStyle, u16, u16) {
    (
        face.family_name.to_lowercase(),
        face.style,
        face.weight,
        face.width,
    )
}

fn append_css_string_content(out: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '<' | '>' | '&' | '\u{0}'..='\u{1f}' | '\u{7f}' => {
                write!(out, "\\{:x} ", character as u32).expect("writing to String cannot fail");
            }
            _ => out.push(character),
        }
    }
}

fn font_width_keyword(width: u16) -> &'static str {
    match width {
        1 => "ultra-condensed",
        2 => "extra-condensed",
        3 => "condensed",
        4 => "semi-condensed",
        6 => "semi-expanded",
        7 => "expanded",
        8 => "extra-expanded",
        9 => "ultra-expanded",
        _ => "normal",
    }
}

fn font_embedding_error(message: impl Into<String>) -> Error {
    Error::svg_postprocess("typed-font-embedding", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{FontAssetSpec, FontCatalogSpec, ThemeResourcePolicy};
    use crate::resources::RenderResourcePolicy;
    use crate::text::PreparedTextLabelFamily;

    const EXCALIFONT_WOFF2: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));

    fn embedded_catalog(requirement: FontEmbeddingRequirement) -> FontCatalog {
        FontCatalogSpec::new([FontAssetSpec::new("excalifont", EXCALIFONT_WOFF2)])
            .with_available_sources([FontSource::Embedded])
            .with_embedding_requirement(requirement)
            .compile(&ThemeResourcePolicy::interactive())
            .expect("embedded font fixture should compile")
    }

    fn first_used_face(catalog: &FontCatalog) -> UsedFontFace {
        let face = &catalog.faces()[0];
        let asset = catalog
            .assets()
            .iter()
            .find(|asset| asset.id() == face.asset_id())
            .expect("fixture face belongs to a retained asset");
        UsedFontFace {
            asset_fingerprint: asset.fingerprint(),
            face_index: face.face_index(),
        }
    }

    fn prepared_label_font(catalog: &FontCatalog, line_count: usize) -> PreparedLabelFont {
        let face = &catalog.faces()[0];
        PreparedLabelFont {
            family_name: face.family_name().to_string(),
            style: face.style(),
            weight: face.weight(),
            width: face.width(),
            line_count,
        }
    }

    #[test]
    fn full_font_plan_uses_canonical_sfnt_bytes_and_emits_a_complete_seal() {
        let catalog = embedded_catalog(FontEmbeddingRequirement::FullFont);
        let plan =
            SvgFontEmbeddingPlan::from_used_faces(&catalog, 1, 1, [first_used_face(&catalog)])
                .expect("full embedded face should produce a plan");

        assert!(plan.css.contains("@font-face{"));
        assert!(plan.css.contains("data:font/ttf;base64,"));
        assert!(plan.css.contains("format(\"truetype\")"));
        assert!(!plan.css.contains("data:font/woff2"));
        let seal = plan
            .seal(1)
            .expect("one planned label seals one text element");
        assert!(seal.is_complete());
        assert_eq!(seal.text_element_count(), 1);
        assert_eq!(seal.embedded_face_count(), 1);
        assert_eq!(seal.catalog_fingerprint(), Some(catalog.fingerprint()));
    }

    #[test]
    fn non_full_font_catalog_cannot_produce_a_typed_embedding_plan() {
        for requirement in [
            FontEmbeddingRequirement::NoEmbedding,
            FontEmbeddingRequirement::Subset,
        ] {
            let catalog = embedded_catalog(requirement);
            assert!(
                SvgFontEmbeddingPlan::from_used_faces(&catalog, 1, 1, [first_used_face(&catalog)],)
                    .is_none(),
                "{requirement:?} must not enter the full-font lane"
            );
        }
    }

    #[test]
    fn typed_style_validation_is_exact_and_detects_payload_tampering() {
        let catalog = embedded_catalog(FontEmbeddingRequirement::FullFont);
        let plan =
            SvgFontEmbeddingPlan::from_used_faces(&catalog, 1, 1, [first_used_face(&catalog)])
                .expect("full embedded face should produce a plan");

        plan.validate_typed_style(&plan.css)
            .expect("renderer-owned CSS should match its plan");
        let tampered = plan.css.replacen("base64,", "base64,A", 1);
        assert!(plan.validate_typed_style(&tampered).is_err());

        let mut competing_face = plan.faces[0].clone();
        competing_face.face_index = competing_face.face_index.saturating_add(1);
        let mut competing_css = String::new();
        append_font_face_rule(&mut competing_css, &plan.faces[0]);
        append_font_face_rule(&mut competing_css, &competing_face);
        let competing_plan = SvgFontEmbeddingPlan {
            catalog_fingerprint: plan.catalog_fingerprint,
            prepared_label_count: plan.prepared_label_count,
            prepared_text_element_count: plan.prepared_text_element_count,
            faces: vec![plan.faces[0].clone(), competing_face].into_boxed_slice(),
            css: competing_css,
        };
        assert!(
            competing_plan
                .validate_typed_style(&competing_plan.css)
                .is_err(),
            "distinct faces with the same CSS descriptor identity must compete fail-closed"
        );
    }

    #[test]
    fn wrapped_label_lines_share_one_ledger_owner_without_leaving_unsealed_text() {
        let catalog = embedded_catalog(FontEmbeddingRequirement::FullFont);
        let family_name = catalog.faces()[0].family_name().to_string();
        let label_id = PreparedTextLabelId::new(PreparedTextLabelFamily::State, 7);
        let label_fonts = HashMap::from([(label_id, prepared_label_font(&catalog, 2))]);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="{}" style="font-family:&quot;{}&quot; !important;font-weight:400 !important;font-style:normal !important">first</text><text id="{}" style="font-family:&quot;{}&quot; !important;font-weight:400 !important;font-style:normal !important">second</text></svg>"#,
            label_id.as_svg_line_id(0),
            family_name,
            label_id.as_svg_line_id(1),
            family_name,
        );

        assert_eq!(scan_prepared_text_coverage(&svg, &label_fonts), Some(2));

        let with_empty_parity_nodes = svg.replace("</svg>", "<text/><text><tspan/></text></svg>");
        assert_eq!(
            scan_prepared_text_coverage(&with_empty_parity_nodes, &label_fonts),
            Some(2),
            "empty Mermaid parity text must not require prepared-font coverage"
        );

        let with_preserved_whitespace = svg.replace(
            "</svg>",
            "<text xml:space=\"preserve\"><tspan> </tspan></text></svg>",
        );
        assert_eq!(
            scan_prepared_text_coverage(&with_preserved_whitespace, &label_fonts),
            None,
            "whitespace content must remain fail-closed because it can be preserved"
        );

        let with_unprepared_text = svg.replace("</svg>", "<text>unprepared</text></svg>");
        assert_eq!(
            scan_prepared_text_coverage(&with_unprepared_text, &label_fonts),
            None
        );

        let missing_line = svg.replace(
            &format!(
                r#"<text id="{}" style="font-family:&quot;{}&quot; !important;font-weight:400 !important;font-style:normal !important">second</text>"#,
                label_id.as_svg_line_id(1),
                family_name,
            ),
            "",
        );
        assert_eq!(
            scan_prepared_text_coverage(&missing_line, &label_fonts),
            None,
            "line-scoped emission must cover every prepared line"
        );

        let out_of_range = svg.replace(&label_id.as_svg_line_id(1), &label_id.as_svg_line_id(2));
        assert_eq!(
            scan_prepared_text_coverage(&out_of_range, &label_fonts),
            None,
            "line-scoped emission must reject indices beyond the ledger"
        );

        let duplicate_line = svg.replace(&label_id.as_svg_line_id(1), &label_id.as_svg_line_id(0));
        assert_eq!(
            scan_prepared_text_coverage(&duplicate_line, &label_fonts),
            None,
            "line-scoped emission must reject duplicate line tokens"
        );

        let mixed_scope = svg.replace(&label_id.as_svg_line_id(1), &label_id.as_svg_id());
        assert_eq!(
            scan_prepared_text_coverage(&mixed_scope, &label_fonts),
            None,
            "one label cannot mix a base token with line-scoped tokens"
        );

        let base_svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="{}" style="font-family:&quot;{}&quot; !important;font-weight:400 !important;font-style:normal !important"><tspan>first</tspan><tspan>second</tspan></text></svg>"#,
            label_id.as_svg_id(),
            family_name,
        );
        assert_eq!(
            scan_prepared_text_coverage(&base_svg, &label_fonts),
            Some(1),
            "one base text element may own every prepared line as tspans"
        );
    }

    #[test]
    fn prepared_text_tracking_propagates_nested_and_deep_content() {
        let catalog = embedded_catalog(FontEmbeddingRequirement::FullFont);
        let family_name = catalog.faces()[0].family_name().to_string();
        let outer_label = PreparedTextLabelId::new(PreparedTextLabelFamily::State, 70);
        let inner_label = PreparedTextLabelId::new(PreparedTextLabelFamily::State, 71);
        let label_fonts = HashMap::from([
            (outer_label, prepared_label_font(&catalog, 1)),
            (inner_label, prepared_label_font(&catalog, 1)),
        ]);
        let nested_svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="{}" style="font-family:&quot;{}&quot;;font-weight:400;font-style:normal"><g><text id="{}" style="font-family:&quot;{}&quot;;font-weight:400;font-style:normal"><tspan>&#xA0;</tspan></text></g></text></svg>"#,
            outer_label.as_svg_id(),
            family_name,
            inner_label.as_svg_id(),
            family_name,
        );
        assert_eq!(
            scan_prepared_text_coverage(&nested_svg, &label_fonts),
            Some(2),
            "content in a nested text element must also make its active parent font-bearing"
        );

        let deep_label = PreparedTextLabelId::new(PreparedTextLabelFamily::State, 72);
        let deep_fonts = HashMap::from([(deep_label, prepared_label_font(&catalog, 1))]);
        let mut deep_svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="{}" style="font-family:&quot;{}&quot;;font-weight:400;font-style:normal">"#,
            deep_label.as_svg_id(),
            family_name,
        );
        for _ in 0..1_024 {
            deep_svg.push_str("<g>");
        }
        deep_svg.push_str("Alpha");
        for _ in 0..1_024 {
            deep_svg.push_str("</g>");
        }
        deep_svg.push_str("</text></svg>");
        assert_eq!(
            scan_prepared_text_coverage(&deep_svg, &deep_fonts),
            Some(1),
            "deep non-text descendants must retain the active text owner"
        );
    }

    #[test]
    fn emitted_fallback_stack_cannot_claim_a_single_face_font_seal() {
        assert_eq!(
            emitted_font_descriptor(
                r#"font-family:"Arial", "Excalifont" !important;font-weight:400;font-style:normal;font-size:16px"#,
            ),
            None
        );
        assert_eq!(
            emitted_font_descriptor(
                r#"font-family:"Excalifont" !important;font-weight:400;font-style:normal;font-size:16px"#,
            ),
            Some(EmittedFontDescriptor {
                family_name: "Excalifont".to_string(),
                style: FontStyle::Normal,
                weight: 400,
                width: 5,
            })
        );

        let catalog = embedded_catalog(FontEmbeddingRequirement::FullFont);
        let face = &catalog.faces()[0];
        let label_id = PreparedTextLabelId::new(PreparedTextLabelFamily::State, 8);
        let label_fonts = HashMap::from([(label_id, prepared_label_font(&catalog, 1))]);
        let wrong_weight = if face.weight() == 700 { 400 } else { 700 };
        let wrong_weight = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="{}" style="font-family:&quot;{}&quot;;font-weight:{};font-style:{}">Alpha</text></svg>"#,
            label_id.as_svg_id(),
            face.family_name(),
            wrong_weight,
            face.style().id(),
        );
        assert_eq!(
            scan_prepared_text_coverage(&wrong_weight, &label_fonts),
            None,
            "emitted weight must select the exact face retained by the ledger"
        );

        let wrong_style = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="{}" style="font-family:&quot;{}&quot;;font-weight:{};font-style:italic">Alpha</text></svg>"#,
            label_id.as_svg_id(),
            face.family_name(),
            face.weight(),
        );
        assert_eq!(
            scan_prepared_text_coverage(&wrong_style, &label_fonts),
            None,
            "emitted style must select the exact face retained by the ledger"
        );

        let mut condensed = prepared_label_font(&catalog, 1);
        condensed.width = 3;
        let condensed_fonts = HashMap::from([(label_id, condensed)]);
        let missing_stretch = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="{}" style="font-family:&quot;{}&quot;;font-weight:{};font-style:{}">Alpha</text></svg>"#,
            label_id.as_svg_id(),
            face.family_name(),
            face.weight(),
            face.style().id(),
        );
        assert_eq!(
            scan_prepared_text_coverage(&missing_stretch, &condensed_fonts),
            None,
            "an omitted stretch descriptor can only prove a normal-width face"
        );
        let matching_stretch = missing_stretch.replace(
            &format!("font-style:{}", face.style().id()),
            &format!("font-style:{};font-stretch:condensed", face.style().id()),
        );
        assert_eq!(
            scan_prepared_text_coverage(&matching_stretch, &condensed_fonts),
            Some(1),
            "an explicit stretch descriptor may prove the matching face width"
        );
    }

    #[test]
    fn only_terminal_validation_bound_to_the_exact_plan_can_issue_a_font_seal() {
        let catalog = embedded_catalog(FontEmbeddingRequirement::FullFont);
        let plan =
            SvgFontEmbeddingPlan::from_used_faces(&catalog, 1, 1, [first_used_face(&catalog)])
                .expect("full embedded face should produce a plan");
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><text>Alpha</text><text/><text><tspan/></text></svg>"#;
        let embedded = plan
            .inject(svg)
            .expect("typed style should attach to SVG root");
        let limits = RenderResourcePolicy::unbounded_for_trusted_input();

        let forged =
            super::super::final_validation::validate_resvg_compatible_svg(&embedded, limits)
                .expect_err("ordinary terminal validation cannot trust a typed marker");
        assert!(forged.to_string().contains("no active embedding plan"));

        let terminal =
            super::super::final_validation::validate_resvg_compatible_svg_with_font_plan(
                &embedded, limits, &plan,
            )
            .expect("the exact renderer-owned plan should validate");
        assert_eq!(terminal.text_elements, 1);
        assert!(terminal.font_seal.is_complete());
        assert_eq!(terminal.font_seal.text_element_count(), 1);
        assert_eq!(terminal.font_seal.embedded_face_count(), 1);
        assert_eq!(terminal.resource_closure.inline_data_resource_count(), 1);

        for extra_text in [
            "<text>unprepared</text>",
            "<text xml:space=\"preserve\"> </text>",
            "<text>&#160;</text>",
        ] {
            let unprepared = embedded.replace("</svg>", &format!("{extra_text}</svg>"));
            let unprepared_error =
                super::super::final_validation::validate_resvg_compatible_svg_with_font_plan(
                    &unprepared,
                    limits,
                    &plan,
                )
                .expect_err(
                    "character-bearing unprepared text must keep the font seal fail-closed",
                );
            assert!(
                unprepared_error
                    .to_string()
                    .contains("terminal SVG contains 2 text elements"),
                "{unprepared_error}"
            );
        }

        let nested =
            embedded
                .replacen("<style", "<g><style", 1)
                .replacen("</style>", "</style></g>", 1);
        let nested_error =
            super::super::final_validation::validate_resvg_compatible_svg_with_font_plan(
                &nested, limits, &plan,
            )
            .expect_err("typed font styles must remain root children");
        assert!(nested_error.to_string().contains("direct child"));

        let inactive_media = embedded.replacen("<style ", r#"<style media="not all" "#, 1);
        let media_error =
            super::super::final_validation::validate_resvg_compatible_svg_with_font_plan(
                &inactive_media,
                limits,
                &plan,
            )
            .expect_err("typed font styles must reject attributes that alter applicability");
        assert!(media_error.to_string().contains("typed font stylesheet"));

        let namespaced = embedded.replacen("<style ", r#"<style xmlns:x="urn:test" "#, 1);
        super::super::final_validation::validate_resvg_compatible_svg_with_font_plan(
            &namespaced,
            limits,
            &plan,
        )
        .expect("namespace declarations do not change typed stylesheet applicability");

        let extra_font_face = embedded.replacen(
            "</style>",
            r#"</style><style>@font-face{font-family:"Other";src:local("Other")}</style>"#,
            1,
        );
        let extra_error =
            super::super::final_validation::validate_resvg_compatible_svg_with_font_plan(
                &extra_font_face,
                limits,
                &plan,
            )
            .expect_err("active plans must reject unrelated font-face rules");
        assert!(extra_error.to_string().contains("unplanned @font-face"));

        let escaped_font_face = embedded.replacen(
            "</style>",
            r#"</style><style>@\66 ont-face{font-family:"Other";src:local("Other")}</style>"#,
            1,
        );
        let escaped_error =
            super::super::final_validation::validate_resvg_compatible_svg_with_font_plan(
                &escaped_font_face,
                limits,
                &plan,
            )
            .expect_err("escaped at-rule names must not bypass font-face competition checks");
        assert!(escaped_error.to_string().contains("unplanned @font-face"));

        let ordinary_css = embedded.replace(
            &format!(" {TYPED_FONT_STYLE_ATTRIBUTE}=\"{TYPED_FONT_STYLE_VERSION}\""),
            "",
        );
        let ordinary =
            super::super::final_validation::validate_resvg_compatible_svg(&ordinary_css, limits)
                .expect_err("an ordinary stylesheet cannot introduce data:font resources");
        assert!(ordinary.to_string().contains("unsafe CSS URL"));
    }
}
