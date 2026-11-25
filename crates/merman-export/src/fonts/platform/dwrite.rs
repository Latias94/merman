//! DirectWrite family matching and cluster fallback over local physical font faces.

use super::{
    AcceptFallback, DwriteFileIdentity, FamilyFace, FamilyQuery, GlyphFallback,
    MAX_FVAR_TABLE_BYTES, NativeFontError, OperationControl, OperationPhase, PlatformFace, cluster,
    css_width, ranked_axis_faces,
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    ffi::{OsString, c_void},
    ops::Range,
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    ptr,
    sync::OnceLock,
};
use ttf_parser::{cmap, fvar, maxp, name, name_id};
use usvg::FontVariation;
use usvg::fontdb::{FaceInfo, ID, Stretch, Style, Weight};
use windows::Win32::Foundation::{
    DWRITE_E_FILEFORMAT, DWRITE_E_NOFONT, DWRITE_E_UNSUPPORTEDOPERATION, E_NOINTERFACE, E_NOTIMPL,
    E_POINTER,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_E_REMOTEFONT, DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_AXIS_TAG_ITALIC,
    DWRITE_FONT_AXIS_TAG_SLANT, DWRITE_FONT_AXIS_TAG_WEIGHT, DWRITE_FONT_AXIS_TAG_WIDTH,
    DWRITE_FONT_AXIS_VALUE, DWRITE_FONT_FACE_TYPE_CFF, DWRITE_FONT_FACE_TYPE_OPENTYPE_COLLECTION,
    DWRITE_FONT_FACE_TYPE_TRUETYPE, DWRITE_FONT_FAMILY_MODEL_WEIGHT_STRETCH_STYLE,
    DWRITE_FONT_STRETCH, DWRITE_FONT_STYLE, DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_STYLE_OBLIQUE, DWRITE_FONT_WEIGHT, DWRITE_LOCALITY_LOCAL, DWRITE_READING_DIRECTION,
    DWRITE_READING_DIRECTION_LEFT_TO_RIGHT, DWRITE_READING_DIRECTION_RIGHT_TO_LEFT,
    DWriteCreateFactory, IDWriteFactory, IDWriteFactory2, IDWriteFactory3, IDWriteFactory6,
    IDWriteFont, IDWriteFont3, IDWriteFontCollection, IDWriteFontFace, IDWriteFontFace2,
    IDWriteFontFace5, IDWriteFontFaceReference1, IDWriteFontFallback, IDWriteFontFallback1,
    IDWriteFontFamily1, IDWriteFontFamily2, IDWriteLocalFontFileLoader, IDWriteNumberSubstitution,
    IDWriteTextAnalysisSource, IDWriteTextAnalysisSource_Impl,
};
use windows_core::{Error, Interface, OutRef, PCWSTR, Result as WinResult, implement};

pub(super) struct Catalog {
    factory: Option<IDWriteFactory>,
    collection: OnceLock<Result<IDWriteFontCollection, NativeFontError>>,
    fallback: IDWriteFontFallback,
    modern_fallback: Option<IDWriteFontFallback1>,
}

struct MappedRun {
    len: u32,
    face: Option<MatchedFace>,
    scale: f32,
}

enum MatchedFace {
    Classic(IDWriteFontFace),
    AxisMatched(IDWriteFontFace5),
}

impl MatchedFace {
    fn native(&self) -> &IDWriteFontFace {
        match self {
            Self::Classic(face) => face,
            Self::AxisMatched(face) => face,
        }
    }
}

impl Catalog {
    pub(super) fn new() -> Result<Self, NativeFontError> {
        let unavailable = || NativeFontError::PlatformDiscoveryUnavailable {
            service: "DirectWrite",
        };
        // SAFETY: The shared factory returns owned interfaces and receives writable out slots.
        unsafe {
            let factory = DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED)
                .map_err(|error| failed("opening the shared factory", error))?;
            let fallback_factory = factory.cast::<IDWriteFactory2>().map_err(|error| {
                if error.code() == E_NOINTERFACE {
                    unavailable()
                } else {
                    failed("querying the fallback interface", error)
                }
            })?;
            let collection = system_collection(&factory)?;
            let fallback = fallback_factory.GetSystemFontFallback().map_err(|error| {
                if optional_api_unavailable(&error) {
                    unavailable()
                } else {
                    failed("opening system fallback", error)
                }
            })?;
            Ok(Self::from_interfaces(Some(factory), collection, fallback))
        }
    }

    fn from_interfaces(
        factory: Option<IDWriteFactory>,
        collection: IDWriteFontCollection,
        fallback: IDWriteFontFallback,
    ) -> Self {
        Self {
            factory,
            collection: OnceLock::from(Ok(collection)),
            modern_fallback: fallback.cast().ok(),
            fallback,
        }
    }

    pub(super) fn for_operation(&self) -> Self {
        Self {
            factory: self.factory.clone(),
            collection: if self.factory.is_some() {
                OnceLock::new()
            } else {
                self.collection.clone()
            },
            fallback: self.fallback.clone(),
            modern_fallback: self.modern_fallback.clone(),
        }
    }

    fn collection(&self) -> Result<&IDWriteFontCollection, NativeFontError> {
        self.collection
            .get_or_init(|| system_collection(self.factory.as_ref().ok_or_else(no_collection)?))
            .as_ref()
            .map_err(Clone::clone)
    }

    pub(super) fn family_candidates(
        &self,
        query: &FamilyQuery,
        control: &OperationControl,
        remaining_coverage: &mut usize,
    ) -> Result<Vec<FamilyFace>, NativeFontError> {
        let mut seen = HashSet::new();
        let mut candidates = Vec::new();
        self.family_faces(
            &query.name,
            FaceStyle {
                weight: query.weight,
                stretch: query.stretch,
                style: query.style,
            },
            None,
            control,
            |native, path, face| {
                if seen.insert((path.clone(), face.index, face.variations.clone())) {
                    candidates.push(FamilyFace {
                        candidate: face.into_file(path),
                        coverage: FamilyCoverage::read(native, remaining_coverage),
                    });
                }
                Ok(None)
            },
        )?;
        Ok(candidates)
    }

    fn family_faces(
        &self,
        name: &str,
        style: FaceStyle,
        request: Option<GlyphFallback<'_>>,
        control: &OperationControl,
        mut visit: impl FnMut(
            &IDWriteFontFace,
            PathBuf,
            PlatformFace,
        ) -> Result<Option<ID>, NativeFontError>,
    ) -> Result<Option<ID>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        if name.is_empty() || name.contains('\0') {
            return Ok(None);
        }
        let name: Vec<_> = name.encode_utf16().chain([0]).collect();
        let collection = self.collection()?;
        let mut index = 0;
        let mut exists = Default::default();
        let mut candidates = Vec::new();
        let mut add = |matched: MatchedFace| {
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            if let Some((path, face)) = file_face(&matched, 1.0, request) {
                let (weight, width, slant) = rendered_style(matched.native(), &face.variations);
                candidates.push(((matched, path, face), weight, width, slant));
            }
            Ok::<_, NativeFontError>(())
        };
        // SAFETY: The name is NUL terminated, output slots are writable, and the family/list
        // interfaces stay live while iterating indices within their reported counts.
        unsafe {
            collection
                .FindFamilyName(PCWSTR::from_raw(name.as_ptr()), &mut index, &mut exists)
                .map_err(|error| failed("finding a font family", error))?;
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            if !exists.as_bool() {
                return Ok(None);
            }
            let family = collection
                .GetFontFamily(index)
                .map_err(|error| failed("opening a font family", error))?;
            let modern = match family.cast::<IDWriteFontFamily2>() {
                Ok(family) => match family.GetMatchingFonts(&style.axes()) {
                    Ok(list) => Some(list),
                    Err(error) if optional_api_unavailable(&error) => None,
                    Err(error) => return Err(failed("matching family axes", error)),
                },
                Err(_) => None,
            };
            if let Some(list) = modern {
                // Keep the precise instances and use their native order to break CSS ties.
                for index in 0..list.GetFontCount() {
                    control.terminal_checkpoint_at(OperationPhase::Layout)?;
                    if list.GetFontLocality(index) != DWRITE_LOCALITY_LOCAL {
                        continue;
                    }
                    let Some(reference) = candidate_result(
                        list.GetFontFaceReference(index),
                        "reading a matched font reference",
                    )?
                    else {
                        continue;
                    };
                    let face = if let Ok(reference) = reference.cast::<IDWriteFontFaceReference1>()
                    {
                        reference.CreateFontFace().map(MatchedFace::AxisMatched)
                    } else {
                        reference
                            .CreateFontFace()
                            .map(|face| MatchedFace::Classic(face.into()))
                    };
                    if let Some(face) = candidate_result(face, "opening a matched font instance")? {
                        add(face)?;
                    }
                }
            } else {
                let locality = family.cast::<IDWriteFontFamily1>().ok();
                for index in 0..family.GetFontCount() {
                    control.terminal_checkpoint_at(OperationPhase::Layout)?;
                    if locality.as_ref().is_some_and(|family| {
                        family.GetFontLocality(index) != DWRITE_LOCALITY_LOCAL
                    }) {
                        continue;
                    }
                    let Some(font) =
                        candidate_result(family.GetFont(index), "reading a family member")?
                    else {
                        continue;
                    };
                    if let Some(face) = local_font_face(&font)? {
                        add(MatchedFace::Classic(face))?;
                    }
                }
            }
        }
        for (matched, path, face) in ranked_axis_faces(
            candidates,
            f32::from(style.weight.0),
            css_width(style.stretch),
            style.style,
        ) {
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            if let Some(id) = visit(matched.native(), path, face)? {
                return Ok(Some(id));
            }
        }
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        Ok(None)
    }

    pub(super) fn fallback(
        &self,
        request: GlyphFallback<'_>,
        base: &FaceInfo,
        control: &OperationControl,
        accept: &mut AcceptFallback<'_>,
    ) -> Result<Option<ID>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let text: Vec<_> = request.text.encode_utf16().collect();
        let Ok(len @ 1..) = u32::try_from(text.len()) else {
            return Ok(None);
        };
        let locale: Vec<_> = request
            .language
            .unwrap_or_default()
            .encode_utf16()
            .chain([0])
            .collect();
        let source: IDWriteTextAnalysisSource = TextSource {
            text,
            locale,
            rtl: request.rtl,
        }
        .into();
        let family = crate::font_selection::face_family_name(base)
            .filter(|name| !name.is_empty() && !name.contains('\0'))
            .map(|name| name.encode_utf16().chain([0]).collect::<Vec<_>>());
        let base_family = family.as_ref().map(|name| PCWSTR::from_raw(name.as_ptr()));
        let (weight, stretch, style) = request.style(base);
        let style = FaceStyle {
            weight,
            stretch,
            style,
        };
        let emoji = emoji_presentation(request.text);
        let mut seen = HashSet::new();
        let mut prepare = |native_face: &IDWriteFontFace, path: PathBuf, face: PlatformFace| {
            let emoji_family = face.family.to_ascii_lowercase().contains("emoji");
            let key = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            if !seen.insert((
                key,
                face.index,
                face.variations.clone(),
                face.fallback_scale.to_bits(),
            )) {
                return Ok(None);
            }
            let presents_emoji = emoji_family
                || (emoji
                    && native_face.cast::<IDWriteFontFace2>().is_ok_and(|face| {
                        // SAFETY: The color-font query only borrows the live face.
                        unsafe { face.IsColorFont().as_bool() }
                    }));
            Ok::<_, NativeFontError>(Some((face.into_file(path), presents_emoji)))
        };
        let mut deferred = Vec::new();
        let mut probe = |family, text: Range<u32>| {
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            let mapped = self.map(&source, text, family, style)?;
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            // A partial mapping still proposes a face. Preflight and acceptance use the
            // original cluster, including its surrounding shaping context in the resolver.
            if let Some(native) = mapped.face
                && let Some((path, face)) = file_face(&native, mapped.scale, Some(request))
                && let Some((candidate, presents_emoji)) = prepare(native.native(), path, face)?
            {
                if emoji != presents_emoji {
                    // Keep the native map eligible after the preferred presentation has been tried.
                    deferred.push(candidate);
                } else {
                    return Ok((mapped.len, accept(candidate)?));
                }
            }
            Ok::<_, NativeFontError>((mapped.len, None))
        };

        let no_family = PCWSTR::null();
        let mut probes = Vec::with_capacity(4);
        for family in base_family.into_iter().chain([no_family]) {
            let (mapped, id) = probe(family, 0..len)?;
            if id.is_some() {
                return Ok(id);
            }
            if let Some(suffix) = unmapped_suffix(request.text, mapped, len)
                && !probes.contains(&suffix)
            {
                probes.push(suffix);
            }
        }
        let mut scalars = request
            .text
            .chars()
            .scan(0, |position, ch| {
                let start = *position;
                *position += ch.len_utf16() as u32;
                Some((ch, start..*position))
            })
            .filter(|(ch, range)| {
                !cluster::is_default_ignorable(*ch) && range.end - range.start < len
            });
        if let Some((_, first)) = scalars.next()
            && !probes.contains(&first)
        {
            probes.push(first);
        }
        if let Some((_, last)) = scalars.last()
            && !probes.contains(&last)
        {
            probes.push(last);
        }
        for range in probes {
            if let (_, Some(id)) = probe(no_family, range)? {
                return Ok(Some(id));
            }
        }

        let mut supplemental = |family, accept: &mut AcceptFallback<'_>| {
            self.family_faces(
                family,
                style,
                Some(request),
                control,
                |native, path, face| match prepare(native, path, face)? {
                    Some((candidate, _)) => accept(candidate),
                    None => Ok(None),
                },
            )
        };
        if emoji && let Some(id) = supplemental("Segoe UI Emoji", accept)? {
            return Ok(Some(id));
        }
        let mut supplemental_text = |accept: &mut AcceptFallback<'_>| {
            for family in ["Segoe UI Symbol", "Segoe UI", "Arial", "Cambria Math"] {
                if let Some(id) = supplemental(family, accept)? {
                    return Ok(Some(id));
                }
            }
            Ok(None)
        };
        if !emoji && let Some(id) = supplemental_text(accept)? {
            return Ok(Some(id));
        }
        for candidate in deferred {
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            if let Some(id) = accept(candidate)? {
                return Ok(Some(id));
            }
        }
        if emoji {
            return supplemental_text(accept);
        }
        Ok(None)
    }

    fn map(
        &self,
        source: &IDWriteTextAnalysisSource,
        range: Range<u32>,
        family: PCWSTR,
        style: FaceStyle,
    ) -> Result<MappedRun, NativeFontError> {
        let collection = self.collection()?;
        let len = range.end - range.start;
        let mut mapped_len = 0;
        let mut scale = 1.0;
        // SAFETY: The range is on scalar boundaries in the live analysis source. The optional
        // family is NUL terminated and all output slots are writable through the call.
        unsafe {
            if let Some(fallback) = &self.modern_fallback {
                let mut face = None;
                match fallback.MapCharacters(
                    source,
                    range.start,
                    len,
                    collection,
                    family,
                    &style.axes(),
                    &mut mapped_len,
                    &mut scale,
                    &mut face,
                ) {
                    Ok(()) => {
                        let face = face.filter(|face| {
                            face.GetFontFaceReference().is_ok_and(|reference| {
                                reference.GetLocality() == DWRITE_LOCALITY_LOCAL
                            })
                        });
                        return Ok(MappedRun {
                            len: mapped_len,
                            face: face.map(MatchedFace::AxisMatched),
                            scale,
                        });
                    }
                    Err(error) if optional_api_unavailable(&error) => {}
                    Err(error) => {
                        candidate_result::<()>(Err(error), "mapping cluster axes")?;
                        return Ok(MappedRun {
                            len: mapped_len,
                            face: None,
                            scale,
                        });
                    }
                }
            }
            let mut font = None;
            mapped_len = 0;
            scale = 1.0;
            if candidate_result(
                self.fallback.MapCharacters(
                    source,
                    range.start,
                    len,
                    collection,
                    family,
                    style.classic_weight(),
                    style.classic_style(),
                    DWRITE_FONT_STRETCH(i32::from(style.stretch.to_number())),
                    &mut mapped_len,
                    &mut font,
                    &mut scale,
                ),
                "mapping a cluster",
            )?
            .is_none()
            {
                return Ok(MappedRun {
                    len: mapped_len,
                    face: None,
                    scale,
                });
            }
            let face = match font {
                Some(font) => local_font_face(&font)?.map(MatchedFace::Classic),
                None => None,
            };
            Ok(MappedRun {
                len: mapped_len,
                face,
                scale,
            })
        }
    }
}

#[derive(Clone, Copy)]
struct FaceStyle {
    weight: Weight,
    stretch: Stretch,
    style: Style,
}

fn rendered_style(face: &IDWriteFontFace, variations: &[FontVariation]) -> (f32, f32, Style) {
    // fontdb derives style from OS/2 and post. Simulated attributes do not
    // describe the outlines usvg loads from the underlying file.
    let os2 = FontTable::read(face, b"OS/2");
    let os2 = os2
        .as_ref()
        .and_then(|table| ttf_parser::os2::Table::parse(table.bytes()));
    let mut style = match os2.map(|table| table.style()) {
        Some(ttf_parser::Style::Italic) => Style::Italic,
        Some(ttf_parser::Style::Oblique) => Style::Oblique,
        _ if FontTable::read(face, b"post")
            .is_some_and(|table| table.bytes().get(4..8).is_some_and(|angle| angle != [0; 4])) =>
        {
            Style::Italic
        }
        _ => Style::Normal,
    };
    let coordinate = |tag| {
        variations
            .iter()
            .find(|axis| axis.tag == tag)
            .map(|axis| axis.value)
    };
    if let Some(italic) = coordinate(*b"ital") {
        style = if italic >= 0.5 {
            Style::Italic
        } else {
            Style::Normal
        };
    }
    if style != Style::Italic
        && let Some(slant) = coordinate(*b"slnt")
    {
        style = if slant == 0.0 {
            Style::Normal
        } else {
            Style::Oblique
        };
    }
    let weight = coordinate(*b"wght")
        .unwrap_or_else(|| os2.map_or(400.0, |table| f32::from(table.weight().to_number())));
    let width = coordinate(*b"wdth").unwrap_or_else(|| {
        super::css_width_class(os2.map_or(5, |table| table.width().to_number()))
    });
    (weight, width, style)
}

impl FaceStyle {
    fn classic_weight(self) -> DWRITE_FONT_WEIGHT {
        DWRITE_FONT_WEIGHT(i32::from(self.weight.0.clamp(1, 999)))
    }

    fn classic_style(self) -> DWRITE_FONT_STYLE {
        match self.style {
            Style::Normal => DWRITE_FONT_STYLE_NORMAL,
            Style::Italic => DWRITE_FONT_STYLE_ITALIC,
            Style::Oblique => DWRITE_FONT_STYLE_OBLIQUE,
        }
    }

    fn axes(self) -> [DWRITE_FONT_AXIS_VALUE; 4] {
        // OpenType wdth uses percentages and slnt uses counterclockwise degrees.
        [
            DWRITE_FONT_AXIS_VALUE {
                axisTag: DWRITE_FONT_AXIS_TAG_WEIGHT,
                value: self.weight.0.clamp(1, 1000).into(),
            },
            DWRITE_FONT_AXIS_VALUE {
                axisTag: DWRITE_FONT_AXIS_TAG_WIDTH,
                value: css_width(self.stretch),
            },
            DWRITE_FONT_AXIS_VALUE {
                axisTag: DWRITE_FONT_AXIS_TAG_ITALIC,
                value: if self.style == Style::Italic {
                    1.0
                } else {
                    0.0
                },
            },
            DWRITE_FONT_AXIS_VALUE {
                axisTag: DWRITE_FONT_AXIS_TAG_SLANT,
                value: if self.style == Style::Oblique {
                    -12.0
                } else {
                    0.0
                },
            },
        ]
    }
}

fn system_collection(factory: &IDWriteFactory) -> Result<IDWriteFontCollection, NativeFontError> {
    // SAFETY: The factory is live and the legacy APIs receive writable out slots.
    unsafe {
        if let Ok(factory) = factory.cast::<IDWriteFactory6>() {
            return factory
                .GetSystemFontCollection(false, DWRITE_FONT_FAMILY_MODEL_WEIGHT_STRETCH_STYLE)
                .map(Into::into)
                .map_err(|error| failed("opening the local system collection", error));
        }
        if let Ok(factory) = factory.cast::<IDWriteFactory3>() {
            let mut collection = None;
            factory
                .GetSystemFontCollection(false, &mut collection, true)
                .map_err(|error| failed("opening the local system collection", error))?;
            collection.map(Into::into).ok_or_else(no_collection)
        } else {
            let mut collection = None;
            factory
                .GetSystemFontCollection(&mut collection, true)
                .map_err(|error| failed("opening the system collection", error))?;
            collection.ok_or_else(no_collection)
        }
    }
}

fn no_collection() -> NativeFontError {
    NativeFontError::Measurement("DirectWrite returned no system font collection".to_owned())
}

fn failed(action: &str, error: Error) -> NativeFontError {
    NativeFontError::Measurement(format!("DirectWrite failed while {action}: {error}"))
}

fn optional_api_unavailable(error: &Error) -> bool {
    matches!(error.code(), E_NOTIMPL | DWRITE_E_UNSUPPORTEDOPERATION)
}

fn candidate_result<T>(result: WinResult<T>, action: &str) -> Result<Option<T>, NativeFontError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error)
            if matches!(
                error.code(),
                DWRITE_E_NOFONT | DWRITE_E_REMOTEFONT | DWRITE_E_FILEFORMAT
            ) || optional_api_unavailable(&error) =>
        {
            Ok(None)
        }
        Err(error) => Err(failed(action, error)),
    }
}

fn local_font_face(font: &IDWriteFont) -> Result<Option<IDWriteFontFace>, NativeFontError> {
    // SAFETY: The font is live and the newer locality getter has no additional inputs.
    unsafe {
        if let Ok(font3) = font.cast::<IDWriteFont3>()
            && font3.GetLocality() != DWRITE_LOCALITY_LOCAL
        {
            return Ok(None);
        }
        candidate_result(font.CreateFontFace(), "opening a local font face")
    }
}

fn unmapped_suffix(text: &str, position: u32, len: u32) -> Option<Range<u32>> {
    if !(1..len).contains(&position) {
        return None;
    }
    let mut offset = 0;
    for ch in text.chars() {
        offset += ch.len_utf16() as u32;
        if offset >= position {
            return (offset == position).then_some(position..len);
        }
    }
    None
}

fn file_face(
    matched: &MatchedFace,
    fallback_scale: f32,
    request: Option<GlyphFallback<'_>>,
) -> Option<(PathBuf, PlatformFace)> {
    if !fallback_scale.is_finite() || fallback_scale <= 0.0 {
        return None;
    }
    let face = matched.native();
    let (path, last_write_time) = local_file_reference(face)?;
    if request.is_some_and(|request| !covers_cluster(face, request)) {
        return None;
    }
    // A simulated match still references a real file member. Names and variation axes
    // come from that physical face. The renderer never receives synthetic style flags.
    let names = FontTable::read(face, b"name")?;
    let names = name::Table::parse(names.bytes())?;
    let family = physical_name(names, name_id::TYPOGRAPHIC_FAMILY)
        .or_else(|| physical_name(names, name_id::FAMILY))?;
    // SAFETY: The face is live and GetIndex identifies its member of the local file.
    let index = unsafe { face.GetIndex() };
    let hmtx = FontTable::read(face, b"hmtx")?;
    let hmtx = hmtx.bytes();
    // There are at most u16::MAX glyphs, each with at most four bytes of hmtx data.
    if hmtx.is_empty() || hmtx.len() > 4 * usize::from(u16::MAX) {
        return None;
    }
    let hvar = FontTable::try_read(face, b"HVAR").ok()?;
    let hvar = hvar.as_ref().map(FontTable::bytes);
    let dwrite_identity = Some(DwriteFileIdentity {
        last_write_time,
        hmtx_len: hmtx.len(),
        hvar_len: hvar.map(<[u8]>::len),
        metrics_digest: Sha256::new()
            .chain_update(hmtx)
            .chain_update(hvar.unwrap_or_default())
            .finalize()
            .into(),
    });
    let (variations, fvar_digest) = variations(matched)?;
    Some((
        path,
        PlatformFace {
            index,
            family,
            post_script_name: physical_name(names, name_id::POST_SCRIPT_NAME),
            other_families: Vec::new(),
            variations,
            fallback_scale,
            dwrite_identity,
            fvar_digest,
        },
    ))
}

fn physical_name(table: name::Table<'_>, id: u16) -> Option<String> {
    let find = |english: bool| {
        table
            .names
            .into_iter()
            .filter(|entry| {
                entry.name_id == id
                    && (!english || entry.language() == ttf_parser::Language::English_UnitedStates)
            })
            .find_map(|entry| {
                let text = entry.to_string().or_else(|| {
                    if entry.platform_id == ttf_parser::PlatformId::Macintosh
                        && entry.encoding_id == 0
                        && entry.name.is_ascii()
                    {
                        return std::str::from_utf8(entry.name).ok().map(str::to_owned);
                    }
                    None
                })?;
                (!text.is_empty()).then_some(text)
            })
    };
    find(true).or_else(|| find(false))
}

fn local_file_reference(face: &IDWriteFontFace) -> Option<(PathBuf, u64)> {
    // SAFETY: The face owns the output file, the file owns the reference key, and the
    // path buffer includes a NUL slot.
    unsafe {
        if !matches!(
            face.GetType(),
            DWRITE_FONT_FACE_TYPE_TRUETYPE
                | DWRITE_FONT_FACE_TYPE_CFF
                | DWRITE_FONT_FACE_TYPE_OPENTYPE_COLLECTION
        ) {
            return None;
        }
        let mut count = 0;
        face.GetFiles(&mut count, None).ok()?;
        if count != 1 {
            return None;
        }
        let mut file = None;
        face.GetFiles(&mut count, Some(&mut file)).ok()?;
        let file = file?;
        let loader = file
            .GetLoader()
            .ok()?
            .cast::<IDWriteLocalFontFileLoader>()
            .ok()?;
        let mut key = ptr::null_mut();
        let mut key_len = 0;
        file.GetReferenceKey(&mut key, &mut key_len).ok()?;
        if key.is_null() || key_len == 0 {
            return None;
        }
        let len = loader.GetFilePathLengthFromKey(key, key_len).ok()? as usize;
        let mut path = vec![0; len.checked_add(1)?];
        loader.GetFilePathFromKey(key, key_len, &mut path).ok()?;
        let path = PathBuf::from(OsString::from_wide(&path[..len]));
        if !path.is_absolute() {
            return None;
        }
        // This timestamp belongs to the selected reference, even if the path is replaced.
        let time = loader.GetLastWriteTimeFromKey(key, key_len).ok()?;
        let last_write_time =
            (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
        Some((path, last_write_time))
    }
}

fn variations(matched: &MatchedFace) -> Option<(Vec<FontVariation>, Option<[u8; 32]>)> {
    let classic;
    let (instance, axis_matched) = match matched {
        MatchedFace::AxisMatched(face) => (Some(face), true),
        MatchedFace::Classic(face) => {
            classic = face.cast::<IDWriteFontFace5>().ok();
            (classic.as_ref(), false)
        }
    };
    let table = FontTable::read(matched.native(), b"fvar");
    let Some((data, table)) = table
        .as_ref()
        .map(FontTable::bytes)
        .and_then(|data| Some((data, fvar::Table::parse(data)?)))
    else {
        return instance
            // SAFETY: The query only borrows the live face.
            .is_none_or(|instance| !unsafe { instance.HasVariations().as_bool() })
            .then(|| (Vec::new(), None));
    };
    if data.len() > MAX_FVAR_TABLE_BYTES {
        return None;
    }
    let mut values = if let Some(instance) = instance {
        // SAFETY: The face reports the required size before receiving the writable slice.
        let count = unsafe { instance.GetFontAxisValueCount() } as usize;
        let mut values = Vec::new();
        values.try_reserve_exact(count).ok()?;
        values.resize(count, DWRITE_FONT_AXIS_VALUE::default());
        unsafe { instance.GetFontAxisValues(&mut values) }.ok()?;
        Some(values)
    } else {
        None
    };
    let coordinates = physical_variations(table, values.as_deref_mut(), axis_matched)?;
    // A classic selection can leave coordinates to CSS while still binding the
    // admitted file to the variable resource selected by DirectWrite.
    Some((coordinates, Some(Sha256::digest(data).into())))
}

fn physical_variations(
    table: fvar::Table<'_>,
    values: Option<&mut [DWRITE_FONT_AXIS_VALUE]>,
    axis_matched: bool,
) -> Option<Vec<FontVariation>> {
    let mut axes: Vec<_> = table.axes.into_iter().collect();
    axes.sort_unstable_by_key(|axis| axis.tag);
    if axes.windows(2).any(|pair| pair[0].tag == pair[1].tag) {
        return None;
    }
    let Some(values) = values else {
        // Older DirectWrite can open a variable font's default without exposing its coordinates.
        return Some(Vec::new());
    };
    let mut instantiated = axis_matched;
    let mut variations = Vec::with_capacity(axes.len());
    values.sort_unstable_by_key(|value| value.axisTag.0);
    // DirectWrite also reports synthetic CSS axes. Only the physical fvar axes can
    // be instantiated by the renderer.
    for axis in axes {
        let tag = axis.tag.to_bytes();
        let value =
            match values.binary_search_by_key(&u32::from_le_bytes(tag), |value| value.axisTag.0) {
                Ok(index) => values[index].value,
                Err(_) if axis.min_value == axis.max_value => axis.def_value,
                Err(_) => return None,
            };
        if !value.is_finite() {
            return None;
        }
        let value = value.clamp(axis.min_value, axis.max_value);
        instantiated |= value != axis.def_value;
        variations.push(FontVariation::new(tag, value + 0.0));
    }
    // The classic default lets the renderer derive weight from CSS. A matched instance
    // may deliberately choose the same design value, so keep its explicit coordinate.
    if !instantiated {
        variations.clear();
    }
    Some(variations)
}

fn emoji_presentation(text: &str) -> bool {
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match chars.peek() {
            Some('\u{fe0e}') => {
                chars.next();
            }
            Some('\u{fe0f}') => return true,
            _ if ch == '\u{20e3}' || default_emoji_presentation(ch) => return true,
            _ => {}
        }
    }
    false
}

fn default_emoji_presentation(ch: char) -> bool {
    // Emoji_Presentation from Unicode emoji-data.txt, excluding text-default symbols.
    // https://www.unicode.org/Public/18.0.0/ucd/emoji/emoji-data.txt
    matches!(ch,
        '\u{231a}'..='\u{231b}'
        | '\u{23e9}'..='\u{23ec}'
        | '\u{23f0}'
        | '\u{23f3}'
        | '\u{25fd}'..='\u{25fe}'
        | '\u{2614}'..='\u{2615}'
        | '\u{2648}'..='\u{2653}'
        | '\u{267f}'
        | '\u{2693}'
        | '\u{26a1}'
        | '\u{26aa}'..='\u{26ab}'
        | '\u{26bd}'..='\u{26be}'
        | '\u{26c4}'..='\u{26c5}'
        | '\u{26ce}'
        | '\u{26d4}'
        | '\u{26ea}'
        | '\u{26f2}'..='\u{26f3}'
        | '\u{26f5}'
        | '\u{26fa}'
        | '\u{26fd}'
        | '\u{2705}'
        | '\u{270a}'..='\u{270b}'
        | '\u{2728}'
        | '\u{274c}'
        | '\u{274e}'
        | '\u{2753}'..='\u{2755}'
        | '\u{2757}'
        | '\u{2795}'..='\u{2797}'
        | '\u{27b0}'
        | '\u{27bf}'
        | '\u{2b1b}'..='\u{2b1c}'
        | '\u{2b50}'
        | '\u{2b55}'
        | '\u{1f004}'
        | '\u{1f0cf}'
        | '\u{1f18e}'
        | '\u{1f191}'..='\u{1f19a}'
        | '\u{1f1e6}'..='\u{1f1ff}'
        | '\u{1f201}'
        | '\u{1f21a}'
        | '\u{1f22f}'
        | '\u{1f232}'..='\u{1f236}'
        | '\u{1f238}'..='\u{1f23a}'
        | '\u{1f250}'..='\u{1f251}'
        | '\u{1f300}'..='\u{1f320}'
        | '\u{1f32d}'..='\u{1f335}'
        | '\u{1f337}'..='\u{1f37c}'
        | '\u{1f37e}'..='\u{1f393}'
        | '\u{1f3a0}'..='\u{1f3ca}'
        | '\u{1f3cf}'..='\u{1f3d3}'
        | '\u{1f3e0}'..='\u{1f3f0}'
        | '\u{1f3f4}'
        | '\u{1f3f8}'..='\u{1f43e}'
        | '\u{1f440}'
        | '\u{1f442}'..='\u{1f4fc}'
        | '\u{1f4ff}'..='\u{1f53d}'
        | '\u{1f54b}'..='\u{1f54e}'
        | '\u{1f550}'..='\u{1f567}'
        | '\u{1f57a}'
        | '\u{1f595}'..='\u{1f596}'
        | '\u{1f5a4}'
        | '\u{1f5fb}'..='\u{1f64f}'
        | '\u{1f680}'..='\u{1f6c5}'
        | '\u{1f6cc}'
        | '\u{1f6d0}'..='\u{1f6d2}'
        | '\u{1f6d5}'..='\u{1f6d9}'
        | '\u{1f6dc}'..='\u{1f6df}'
        | '\u{1f6eb}'..='\u{1f6ec}'
        | '\u{1f6f4}'..='\u{1f6fc}'
        | '\u{1f7e0}'..='\u{1f7eb}'
        | '\u{1f7f0}'
        | '\u{1f90c}'..='\u{1f93a}'
        | '\u{1f93c}'..='\u{1f945}'
        | '\u{1f947}'..='\u{1f9ff}'
        | '\u{1fa70}'..='\u{1fa7c}'
        | '\u{1fa80}'..='\u{1fac6}'
        | '\u{1fac8}'
        | '\u{1facc}'..='\u{1fadd}'
        | '\u{1fadf}'..='\u{1faeb}'
        | '\u{1faef}'..='\u{1fafa}')
}

pub(super) struct FamilyCoverage {
    cmap: Box<[u8]>,
    glyph_count: u16,
}

impl FamilyCoverage {
    fn read(face: &IDWriteFontFace, remaining: &mut usize) -> Option<Self> {
        if *remaining == 0 {
            return None;
        }
        let cmap = FontTable::read(face, b"cmap")?;
        let bytes = cmap.bytes();
        let left = remaining.checked_sub(bytes.len())?;
        cmap::Table::parse(bytes)?;
        let maxp = FontTable::read(face, b"maxp")?;
        let glyph_count = maxp::Table::parse(maxp.bytes())?.number_of_glyphs.get();
        *remaining = left;
        // A small owned table avoids retaining DirectWrite face handles or re-reading the file.
        Some(Self {
            cmap: bytes.into(),
            glyph_count,
        })
    }

    pub(super) fn may_cover(&self, text: &str, rtl: bool) -> bool {
        cluster::cmap_may_cover(&self.cmap, self.glyph_count, text, rtl)
    }
}

fn covers_cluster(face: &IDWriteFontFace, request: GlyphFallback<'_>) -> bool {
    let [Some(head), Some(cmap), Some(maxp), os2] =
        [b"head", b"cmap", b"maxp", b"OS/2"].map(|tag| FontTable::read(face, tag))
    else {
        return false;
    };
    let os2 = os2.as_ref().map(FontTable::bytes);
    if !crate::font_selection::allows_font_tables(
        head.bytes(),
        os2,
        cmap.bytes(),
        maxp.bytes(),
        Some((request.text, request.rtl)),
    ) {
        return false;
    }
    maxp::Table::parse(maxp.bytes()).is_some_and(|maxp| {
        cluster::cmap_may_cover_with_os2(
            cmap.bytes(),
            os2,
            maxp.number_of_glyphs.get(),
            request.text,
            request.rtl,
        )
    })
}

struct FontTable<'a> {
    face: &'a IDWriteFontFace,
    context: *mut c_void,
    data: *const u8,
    size: usize,
}

impl Drop for FontTable<'_> {
    fn drop(&mut self) {
        // SAFETY: This face returned the context for an existing table, possibly NULL.
        unsafe { self.face.ReleaseFontTable(self.context) };
    }
}

impl<'a> FontTable<'a> {
    fn read(face: &'a IDWriteFontFace, tag: &[u8; 4]) -> Option<Self> {
        Self::try_read(face, tag).ok().flatten()
    }

    fn try_read(face: &'a IDWriteFontFace, tag: &[u8; 4]) -> WinResult<Option<Self>> {
        let mut data = ptr::null_mut();
        let mut size = 0;
        let mut context = ptr::null_mut();
        let mut exists = Default::default();
        // SAFETY: The output slots are writable and the resulting lease retains the face.
        unsafe {
            face.TryGetFontTable(
                u32::from_le_bytes(*tag),
                &mut data,
                &mut size,
                &mut context,
                &mut exists,
            )?;
        }
        if !exists.as_bool() {
            return Ok(None);
        }
        let table = Self {
            face,
            context,
            data: data.cast(),
            size: size as usize,
        };
        if table.data.is_null() || table.size > isize::MAX as usize {
            return Err(Error::from_hresult(DWRITE_E_FILEFORMAT));
        }
        Ok(Some(table))
    }

    fn bytes(&self) -> &[u8] {
        // SAFETY: DirectWrite keeps the bounds-checked table live until the lease is dropped.
        unsafe { std::slice::from_raw_parts(self.data, self.size) }
    }
}

#[implement(IDWriteTextAnalysisSource)]
struct TextSource {
    text: Vec<u16>,
    locale: Vec<u16>,
    rtl: bool,
}

impl IDWriteTextAnalysisSource_Impl for TextSource_Impl {
    fn GetTextAtPosition(
        &self,
        position: u32,
        text: *mut *mut u16,
        len: *mut u32,
    ) -> WinResult<()> {
        report_text(
            self.text.get(position as usize..).unwrap_or_default(),
            text,
            len,
        )
    }

    fn GetTextBeforePosition(
        &self,
        position: u32,
        text: *mut *mut u16,
        len: *mut u32,
    ) -> WinResult<()> {
        report_text(
            self.text.get(..position as usize).unwrap_or_default(),
            text,
            len,
        )
    }

    fn GetParagraphReadingDirection(&self) -> DWRITE_READING_DIRECTION {
        if self.rtl {
            DWRITE_READING_DIRECTION_RIGHT_TO_LEFT
        } else {
            DWRITE_READING_DIRECTION_LEFT_TO_RIGHT
        }
    }

    fn GetLocaleName(&self, position: u32, len: *mut u32, locale: *mut *mut u16) -> WinResult<()> {
        if len.is_null() || locale.is_null() {
            return Err(Error::from_hresult(E_POINTER));
        }
        // SAFETY: COM supplies writable output pointers. The source owns the NUL-terminated locale.
        unsafe {
            len.write((self.text.len() as u32).saturating_sub(position));
            locale.write(self.locale.as_ptr().cast_mut());
        }
        Ok(())
    }

    fn GetNumberSubstitution(
        &self,
        position: u32,
        len: *mut u32,
        substitution: OutRef<'_, IDWriteNumberSubstitution>,
    ) -> WinResult<()> {
        if len.is_null() {
            return Err(Error::from_hresult(E_POINTER));
        }
        substitution.write(None)?;
        // SAFETY: COM supplies a writable output pointer.
        unsafe { len.write((self.text.len() as u32).saturating_sub(position)) };
        Ok(())
    }
}

fn report_text(run: &[u16], text: *mut *mut u16, len: *mut u32) -> WinResult<()> {
    if text.is_null() || len.is_null() {
        return Err(Error::from_hresult(E_POINTER));
    }
    let start = if run.is_empty() {
        ptr::null_mut()
    } else {
        run.as_ptr().cast_mut()
    };
    // SAFETY: COM supplies writable output pointers. DirectWrite reads the run while
    // the analysis source is live and never writes through the returned text pointer.
    unsafe {
        text.write(start);
        len.write(run.len() as u32);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{ExpectedFace, FontCandidate};
    use super::*;
    use std::path::Path;
    use usvg::fontdb::Database;
    use windows::Win32::Graphics::DirectWrite::{
        DWRITE_FONT_SIMULATIONS_BOLD, DWRITE_UNICODE_RANGE, IDWriteFactory5,
    };
    use windows_core::{HSTRING, w};

    fn fixture(name: impl AsRef<Path>) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fonts")
            .join(name)
    }

    fn fixture_catalog(files: &[impl AsRef<Path>], mappings: &[(char, PCWSTR, f32)]) -> Catalog {
        // SAFETY: The fixture paths are NUL terminated and the range/name arrays are live for
        // each call. The collection and fallback retain their COM font references.
        unsafe {
            let factory =
                DWriteCreateFactory::<IDWriteFactory5>(DWRITE_FACTORY_TYPE_SHARED).unwrap();
            let fonts = factory.CreateFontSetBuilder().unwrap();
            for name in files {
                let path = HSTRING::from(fixture(name).as_path());
                let file = factory.CreateFontFileReference(&path, None).unwrap();
                fonts.AddFontFile(&file).unwrap();
            }
            let fonts = fonts.CreateFontSet().unwrap();
            let collection = if let Ok(factory) = factory.cast::<IDWriteFactory6>() {
                factory
                    .CreateFontCollectionFromFontSet(
                        &fonts,
                        DWRITE_FONT_FAMILY_MODEL_WEIGHT_STRETCH_STYLE,
                    )
                    .unwrap()
                    .into()
            } else {
                factory
                    .CreateFontCollectionFromFontSet(&fonts)
                    .unwrap()
                    .into()
            };
            let builder = factory.CreateFontFallbackBuilder().unwrap();
            for &(character, name, scale) in mappings {
                let range = DWRITE_UNICODE_RANGE {
                    first: character as u32,
                    last: character as u32,
                };
                builder
                    .AddMapping(&[range], &[name.as_ptr()], &collection, None, None, scale)
                    .unwrap();
            }
            Catalog::from_interfaces(None, collection, builder.CreateFontFallback().unwrap())
        }
    }

    fn base_face() -> FaceInfo {
        let mut database = Database::new();
        database
            .load_font_data(include_bytes!("../../../tests/fonts/MermanProbeBase.ttf").to_vec());
        database.faces().next().unwrap().clone()
    }

    fn physical_face(candidate: FontCandidate) -> (PathBuf, PlatformFace, FaceInfo) {
        let FontCandidate::File {
            path,
            index,
            expected: ExpectedFace::Platform(expected),
        } = candidate
        else {
            panic!("DirectWrite should select a platform file");
        };
        let mut database = Database::new();
        database.load_font_data(std::fs::read(&path).unwrap());
        let face = database.faces().find(|face| face.index == index).unwrap();
        expected.validate(face, &path).unwrap();
        (path, expected, face.clone())
    }

    #[test]
    fn directwrite_keeps_only_finite_physical_axis_values() {
        let bytes = include_bytes!("../../../tests/fonts/MermanVariableWeight.ttf");
        let table = ttf_parser::Face::parse(bytes, 0)
            .unwrap()
            .tables()
            .fvar
            .unwrap();
        let width = DWRITE_FONT_AXIS_VALUE {
            axisTag: DWRITE_FONT_AXIS_TAG_WIDTH,
            value: 100.0,
        };
        let weight = |value| DWRITE_FONT_AXIS_VALUE {
            axisTag: DWRITE_FONT_AXIS_TAG_WEIGHT,
            value,
        };
        assert_eq!(
            physical_variations(table, Some(&mut [width, weight(700.0)]), true).unwrap(),
            [FontVariation::new(*b"wght", 700.0)]
        );
        assert_eq!(
            physical_variations(table, Some(&mut [weight(1000.0)]), true).unwrap(),
            [FontVariation::new(*b"wght", 900.0)]
        );
        assert!(physical_variations(table, Some(&mut [width]), true).is_none());
        assert!(physical_variations(table, Some(&mut [weight(f32::NAN)]), true).is_none());
        assert_eq!(
            physical_variations(table, Some(&mut [weight(100.0)]), true).unwrap(),
            [FontVariation::new(*b"wght", 100.0)]
        );
        assert!(
            physical_variations(table, Some(&mut [weight(100.0)]), false)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            physical_variations(table, Some(&mut [weight(700.0)]), false).unwrap(),
            [FontVariation::new(*b"wght", 700.0)]
        );
        assert!(physical_variations(table, None, false).unwrap().is_empty());
    }

    #[test]
    fn directwrite_fills_missing_fixed_axes_and_rejects_duplicate_physical_tags() {
        let raw = ttf_parser::RawFace::parse(
            include_bytes!("../../../tests/fonts/MermanVariableWeight.ttf"),
            0,
        )
        .unwrap();
        let source = raw.table(ttf_parser::Tag::from_bytes(b"fvar")).unwrap();
        let mut fvar = source.to_vec();
        let axis_start = usize::from(u16::from_be_bytes(source[4..6].try_into().unwrap()));
        let axis_size = usize::from(u16::from_be_bytes(source[10..12].try_into().unwrap()));
        let second = axis_start + axis_size;
        fvar.truncate(second);
        fvar.extend_from_within(axis_start..second);
        fvar[8..10].copy_from_slice(&2_u16.to_be_bytes());
        let weight = DWRITE_FONT_AXIS_VALUE {
            axisTag: DWRITE_FONT_AXIS_TAG_WEIGHT,
            value: 625.0,
        };
        let duplicate = fvar::Table::parse(&fvar).unwrap();
        assert!(physical_variations(duplicate, Some(&mut [weight]), true).is_none());
        assert!(physical_variations(duplicate, None, false).is_none());

        fvar[second..second + 4].copy_from_slice(b"wdth");
        let fixed_value = (100_i32 << 16).to_be_bytes();
        for bytes in fvar[second + 4..second + 16].chunks_exact_mut(4) {
            bytes.copy_from_slice(&fixed_value);
        }
        let fixed_width = fvar::Table::parse(&fvar).unwrap();
        assert_eq!(
            physical_variations(fixed_width, Some(&mut [weight]), true).unwrap(),
            [
                FontVariation::new(*b"wdth", 100.0),
                FontVariation::new(*b"wght", 625.0)
            ]
        );
    }

    #[test]
    fn directwrite_maps_fallback_to_the_selected_collection_member() {
        let name = "Cousine-test-collection.ttc";
        let mut catalog = fixture_catalog(&[name], &[('A', w!("Cousine"), 1.5)]);
        let mut base = base_face();
        base.style = Style::Italic;
        let control = OperationControl::new();
        let mut accept = |candidate| {
            let (path, expected, mut physical) = physical_face(candidate);
            assert_eq!(expected.fallback_scale, 1.5);
            assert_eq!(
                path.canonicalize().unwrap(),
                fixture(name).canonicalize().unwrap()
            );
            assert_eq!(physical.index, 1);
            assert_eq!(physical.post_script_name, "Cousine-Italic");
            physical.weight = Weight::BOLD;
            expected.validate(&physical, &path).unwrap();
            physical.index = 0;
            assert!(expected.validate(&physical, &path).is_err());
            Ok(Some(physical.id))
        };
        let request = GlyphFallback::from("AA");
        assert!(
            catalog
                .fallback(request, &base, &control, &mut accept)
                .unwrap()
                .is_some()
        );
        catalog.modern_fallback = None;
        assert!(
            catalog
                .fallback(request, &base, &control, &mut accept)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn directwrite_ranks_family_members_with_bounded_coverage() {
        let catalog = fixture_catalog(&["Cousine-test-collection.ttc"], &[]);
        let control = OperationControl::new();
        let query = FamilyQuery::new(
            "cOuSiNe",
            Weight::NORMAL,
            Stretch::Normal,
            Style::Italic,
            None,
        );
        let budget = 128 * 1024;
        let mut available = budget;
        let candidates = catalog
            .family_candidates(&query, &control, &mut available)
            .unwrap();
        let mut indices = Vec::new();
        let mut cmap_bytes = 0;
        for face in candidates {
            let (_, _, physical) = physical_face(face.candidate);
            indices.push(physical.index);
            let coverage = face.coverage.unwrap();
            assert!(coverage.may_cover("A", false));
            assert!(!coverage.may_cover("中", false));
            cmap_bytes += coverage.cmap.len();
        }
        assert_eq!(indices, [1, 0]);
        assert_eq!(available, budget - cmap_bytes);
        let mut no_coverage = 0;
        let candidates = catalog
            .family_candidates(&query, &control, &mut no_coverage)
            .unwrap();
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().all(|face| face.coverage.is_none()));

        let absent = FamilyQuery::new(
            "Arial",
            Weight::NORMAL,
            Stretch::Normal,
            Style::Normal,
            None,
        );
        assert!(
            catalog
                .family_candidates(&absent, &control, &mut available)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn directwrite_ranks_the_rendered_css_faces_instead_of_simulated_attributes() {
        let catalog = fixture_catalog(
            &[
                "Cousine-Regular.ttf",
                "Cousine-Italic.ttf",
                "Cousine-Bold.ttf",
            ],
            &[],
        );
        let control = OperationControl::new();
        for (weight, style, first) in [
            (400, Style::Normal, "Cousine-Regular"),
            (510, Style::Normal, "Cousine-Bold"),
            (625, Style::Italic, "Cousine-Italic"),
        ] {
            let query = FamilyQuery::new("Cousine", Weight(weight), Stretch::Normal, style, None);
            let candidates = catalog.family_candidates(&query, &control, &mut 0).unwrap();
            assert_eq!(candidates.len(), 3, "one candidate per physical face");
            let (_, _, selected) = physical_face(candidates.into_iter().next().unwrap().candidate);
            assert_eq!(selected.post_script_name, first, "{weight} {style:?}");
        }
    }

    #[test]
    fn directwrite_rechecks_the_original_cluster_after_a_partial_mapping() {
        let original: GlyphFallback<'_> = "✓中".into();
        let base = base_face();
        let control = OperationControl::new();
        for (file, family, can_shape) in [
            ("NotoSansSC-subset.ttf", w!("Noto Sans SC"), false),
            ("MermanProbeFull.ttf", w!("MermanProbeFull"), true),
        ] {
            let catalog = fixture_catalog(
                &["MermanProbePartial.ttf", file],
                &[('✓', w!("MermanProbePartial"), 1.0), ('中', family, 1.25)],
            );
            let mut tried = Vec::new();
            let found = catalog
                .fallback(original, &base, &control, &mut |candidate| {
                    let (path, expected, physical) = physical_face(candidate);
                    let bytes = std::fs::read(path).unwrap();
                    let parsed = ttf_parser::Face::parse(&bytes, physical.index).unwrap();
                    tried.push(expected.fallback_scale);
                    Ok(cluster::may_shape(parsed, original).then_some(physical.id))
                })
                .unwrap();
            assert_eq!(tried, [1.0, 1.25], "{file}");
            assert_eq!(found.is_some(), can_shape);
        }
    }

    #[test]
    fn directwrite_fallback_uses_the_physical_simulated_member() {
        let mut catalog = fixture_catalog(&["Cousine-Regular.ttf"], &[('V', w!("Cousine"), 1.25)]);
        let mut base = base_face();
        base.families.clear();
        base.style = Style::Italic;
        base.stretch = Stretch::Condensed;
        let control = OperationControl::new();
        for modern in [true, false] {
            if !modern {
                catalog.modern_fallback = None;
            }
            for weight in [1, 999, 1000] {
                base.weight = Weight(weight);
                let chosen = catalog
                    .fallback("V".into(), &base, &control, &mut |candidate| {
                        let (_, expected, physical) = physical_face(candidate);
                        assert_eq!(expected.fallback_scale, 1.25);
                        assert_eq!(physical.post_script_name, "Cousine-Regular");
                        assert_eq!(
                            (physical.weight, physical.stretch, physical.style),
                            (Weight::NORMAL, Stretch::Normal, Style::Normal)
                        );
                        Ok(Some(physical.id))
                    })
                    .unwrap();
                assert!(chosen.is_some(), "weight {weight}, modern {modern}");
            }
        }
        // SAFETY: The path and font file are live through creation of the simulated face.
        let simulated = unsafe {
            let factory =
                DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED).unwrap();
            let path = HSTRING::from(fixture("Cousine-Regular.ttf").as_path());
            let file = factory.CreateFontFileReference(&path, None).unwrap();
            factory
                .CreateFontFace(
                    DWRITE_FONT_FACE_TYPE_TRUETYPE,
                    &[Some(file)],
                    0,
                    DWRITE_FONT_SIMULATIONS_BOLD,
                )
                .unwrap()
        };
        let (path, face) =
            file_face(&MatchedFace::Classic(simulated), 1.25, Some("V".into())).unwrap();
        let (_, expected, physical) = physical_face(face.into_file(path));
        assert_eq!(expected.fallback_scale, 1.25);
        assert_eq!(physical.weight, Weight::NORMAL);
    }

    #[test]
    fn directwrite_carries_modern_variable_family_and_fallback_axes() {
        let mut catalog = fixture_catalog(
            &["MermanVariableWeight.ttf"],
            &[('A', w!("Merman Variable Weight"), 1.125)],
        );
        // SAFETY: The shared factory request only negotiates the available interface.
        if unsafe { DWriteCreateFactory::<IDWriteFactory6>(DWRITE_FACTORY_TYPE_SHARED) }.is_err() {
            return;
        }
        assert!(catalog.modern_fallback.is_some());
        let mut base = base_face();
        let control = OperationControl::new();
        let check = |scale, weight, candidate| {
            let (_, expected, physical) = physical_face(candidate);
            assert_eq!(expected.fallback_scale, scale);
            assert_eq!(expected.variations, [FontVariation::new(*b"wght", weight)]);
            assert_eq!(physical.weight, Weight(100));
            Ok(Some(physical.id))
        };
        for (weight, slant) in [
            (100, Style::Normal),
            (510, Style::Normal),
            (700, Style::Oblique),
        ] {
            base.weight = Weight(weight);
            base.style = slant;
            let query = FamilyQuery::new(
                "Merman Variable Weight",
                base.weight,
                base.stretch,
                base.style,
                None,
            );
            let candidates = catalog.family_candidates(&query, &control, &mut 0).unwrap();
            check(
                1.0,
                f32::from(weight),
                candidates.into_iter().next().unwrap().candidate,
            )
            .unwrap();
            assert!(
                catalog
                    .fallback("A".into(), &base, &control, &mut |candidate| {
                        check(1.125, f32::from(weight), candidate)
                    })
                    .unwrap()
                    .is_some()
            );
        }

        catalog.modern_fallback = None;
        base.weight = Weight(100);
        base.style = Style::Normal;
        assert!(
            catalog
                .fallback("A".into(), &base, &control, &mut |candidate| {
                    let (_, expected, physical) = physical_face(candidate);
                    assert_eq!(physical.weight, Weight(100));
                    assert!(expected.variations.is_empty());
                    Ok(Some(physical.id))
                })
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn directwrite_classic_fallback_rejects_a_same_name_static_replacement() {
        use crate::fonts::{NativeFontLimits, registry::FontSourceRegistry};
        use std::{fs, sync::Arc};

        let variable = include_bytes!("../../../tests/fonts/MermanVariableWeight.ttf");
        let mut replacement = variable.to_vec();
        let count = usize::from(u16::from_be_bytes(replacement[4..6].try_into().unwrap()));
        let fvar = replacement[12..12 + count * 16]
            .chunks_exact_mut(16)
            .find(|record| &record[..4] == b"fvar")
            .unwrap();
        // Keep the sorted directory and physical names, but remove the variation axis table.
        fvar[..4].copy_from_slice(b"fvzz");
        let replaced = ttf_parser::Face::parse(&replacement, 0).unwrap();
        assert!(!replaced.is_variable());

        let path =
            std::env::temp_dir().join(format!("merman-dwrite-fvar-{}.ttf", std::process::id()));
        let original = path.with_extension("variable.ttf");
        fs::write(&path, variable).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let control = OperationControl::new();
        let request = GlyphFallback::from("A");
        let mut base = base_face();
        base.weight = Weight(100);
        let candidate = {
            let mut catalog =
                fixture_catalog(&[&path], &[('A', w!("Merman Variable Weight"), 1.0)]);
            catalog.modern_fallback = None;
            let mut selected = None;
            catalog
                .fallback(request, &base, &control, &mut |candidate| {
                    selected = Some(candidate);
                    Ok(Some(ID::dummy()))
                })
                .unwrap();
            selected.unwrap()
        };
        let (_, expected, physical) = physical_face(candidate.clone());
        assert_eq!(physical.weight, Weight(100));
        assert!(expected.variations.is_empty());
        assert!(expected.fvar_digest.is_some());
        let mut static_db = Database::new();
        static_db.load_font_data(replacement.clone());
        expected
            .validate(static_db.faces().next().unwrap(), &path)
            .unwrap();

        let registry = FontSourceRegistry::new(Default::default(), Default::default(), 0, 0);
        let mut database = Arc::new(Database::new());
        let limits = NativeFontLimits::default();
        fs::rename(&path, &original).unwrap();
        fs::write(&path, replacement).unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        let result = registry.materialize(candidate, Some('A'), &mut database, &limits, &control);
        fs::remove_file(&path).unwrap();
        fs::remove_file(&original).unwrap();
        assert!(
            matches!(&result, Err(NativeFontError::Source { message, .. }) if message.contains("variation axes selected by the platform")),
            "{result:?}"
        );
        assert_eq!(registry.admitted_faces(), 0);
        assert_eq!(registry.resident_bytes(), 0);
        assert!(database.is_empty());
    }

    #[test]
    fn directwrite_rejects_changed_horizontal_metrics_with_the_same_path_and_timestamp() {
        use crate::fonts::{NativeFontLimits, registry::FontSourceRegistry};
        use std::{collections::BTreeMap, fs, io::Write, sync::Arc};

        let variable = include_bytes!("../../../tests/fonts/MermanVariableWeight.ttf");
        let mut parsed = ttf_parser::Face::parse(variable, 0).unwrap();
        let glyph = parsed.glyph_index('A').unwrap();
        let raw = parsed.raw_face();
        let hmtx = raw.table(ttf_parser::Tag::from_bytes(b"hmtx")).unwrap();
        let hvar = raw.table(ttf_parser::Tag::from_bytes(b"HVAR")).unwrap();
        let hmtx_offset =
            hmtx.as_ptr() as usize - variable.as_ptr() as usize + usize::from(glyph.0) * 4;
        // The fixture's single HVAR region has a short delta per glyph, with no index map.
        // A and V are the last two glyphs, both with a delta of 240 at maximum weight.
        let hvar_offset = hvar.as_ptr() as usize - variable.as_ptr() as usize + hvar.len() - 4;
        assert_eq!(
            &variable[hvar_offset..hvar_offset + 2],
            &240_i16.to_be_bytes()
        );
        let default_advance = parsed.glyph_hor_advance(glyph).unwrap();
        parsed
            .set_variation(ttf_parser::Tag::from_bytes(b"wght"), 510.0)
            .unwrap();
        let advance = parsed.glyph_hor_advance(glyph).unwrap();

        let path =
            std::env::temp_dir().join(format!("merman-dwrite-metrics-{}.ttf", std::process::id()));
        let original = path.with_extension("variable.ttf");
        fs::write(&path, variable).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let control = OperationControl::new();
        let candidates = {
            let name = "Merman Variable Weight";
            let mut catalog =
                fixture_catalog(&[&path], &[('A', w!("Merman Variable Weight"), 1.125)]);
            let modern = catalog.modern_fallback.is_some();
            let coordinates = vec![FontVariation::new(*b"wght", 510.0)];
            let capture_fallback = |catalog: &Catalog, weight| {
                let mut base = base_face();
                base.weight = Weight(weight);
                let mut selected = None;
                let found = catalog.fallback("A".into(), &base, &control, &mut |candidate| {
                    selected = Some(candidate);
                    Ok(Some(ID::dummy()))
                });
                assert!(found.unwrap().is_some());
                selected.unwrap()
            };
            let query = FamilyQuery::new(name, Weight(510), Stretch::Normal, Style::Normal, None);
            let named = catalog.family_candidates(&query, &control, &mut 0).unwrap();
            let named = named.into_iter().next().unwrap().candidate;
            let named_axes = modern.then(|| coordinates.clone());
            let mut candidates = vec![("named", named, 1.0, named_axes)];
            if modern {
                candidates.push((
                    "modern fallback",
                    capture_fallback(&catalog, 510),
                    1.125,
                    Some(coordinates),
                ));
            }
            catalog.modern_fallback = None;
            candidates.push((
                "classic fallback",
                capture_fallback(&catalog, 100),
                1.125,
                Some(Vec::new()),
            ));
            candidates
        };

        let admit = |candidate, fallback, limits: &NativeFontLimits, initial| {
            let registry = FontSourceRegistry::new(initial, Default::default(), 0, 0);
            let initial_bytes = registry.resident_bytes();
            let mut database = Arc::new(Database::new());
            let result = registry.materialize(candidate, fallback, &mut database, limits, &control);
            (
                result,
                registry.admitted_faces(),
                registry.resident_bytes() - initial_bytes,
                database.is_empty(),
            )
        };
        for (kind, candidate, scale, coordinates) in &candidates {
            let (_, expected, _) = physical_face(candidate.clone());
            assert_eq!(expected.fallback_scale, *scale, "{kind}");
            if let Some(coordinates) = coordinates {
                assert_eq!(&expected.variations, coordinates, "{kind}");
            }
            let (result, faces, bytes, _) = admit(
                candidate.clone(),
                (*kind != "named").then_some('A'),
                &NativeFontLimits::default(),
                BTreeMap::new(),
            );
            assert!(result.unwrap().is_some(), "{kind}");
            assert_eq!((faces, bytes), (1, variable.len()), "{kind}");
        }

        fs::rename(&path, &original).unwrap();
        // A replaced table should fail the bounded probe before any full-file budget check.
        let limits = NativeFontLimits {
            max_files: 0,
            max_bytes: 0,
            ..Default::default()
        };
        let mut rejected = Vec::new();
        for (tag, offset, value) in [
            (b"hmtx", hmtx_offset, default_advance + 1),
            (b"HVAR", hvar_offset, 360),
        ] {
            let mut replacement = variable.to_vec();
            replacement[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
            let mut replaced = ttf_parser::Face::parse(&replacement, 0).unwrap();
            let raw = replaced.raw_face();
            for record in parsed.raw_face().table_records {
                if record.tag.to_bytes() != *tag {
                    assert_eq!(raw.table(record.tag), parsed.raw_face().table(record.tag));
                }
            }
            replaced
                .set_variation(ttf_parser::Tag::from_bytes(b"wght"), 510.0)
                .unwrap();
            assert!(replaced.glyph_hor_advance(glyph).unwrap() > advance);

            let mut file = fs::File::create(&path).unwrap();
            file.write_all(&replacement).unwrap();
            file.set_times(fs::FileTimes::new().set_modified(modified))
                .unwrap();
            drop(file);
            let metadata = fs::metadata(&path).unwrap();
            assert_eq!(metadata.modified().unwrap(), modified);
            assert_eq!(metadata.len(), variable.len() as u64);
            let snapshot =
                BTreeMap::from([(fs::canonicalize(&path).unwrap(), Arc::new(replacement))]);
            for (source, initial) in [("file", BTreeMap::new()), ("snapshot", snapshot)] {
                rejected.extend(candidates.iter().map(|(kind, candidate, ..)| {
                    let fallback = (*kind != "named").then_some('A');
                    (
                        tag,
                        kind,
                        source,
                        admit(candidate.clone(), fallback, &limits, initial.clone()),
                    )
                }));
            }
            fs::remove_file(&path).unwrap();
        }
        fs::remove_file(original).unwrap();
        for (tag, kind, source, (result, faces, bytes, database_empty)) in rejected {
            let tag = std::str::from_utf8(tag).unwrap();
            assert!(
                matches!(&result, Err(NativeFontError::Source { message, .. }) if message.contains("horizontal metrics changed")),
                "{kind}, {tag}, {source}: {result:?}"
            );
            assert_eq!(
                (faces, bytes, database_empty),
                (0, 0, true),
                "{kind}, {tag}, {source}"
            );
        }
    }

    #[test]
    #[ignore = "requires installed Arial and Bahnschrift faces"]
    fn directwrite_system_renders_arial_510_and_bahnschrift_700_oblique() {
        use crate::fonts::NativeFontOptions;

        let control = OperationControl::new();
        let context = NativeFontOptions::default()
            .with_system_fonts(true)
            .load_platform(&control)
            .unwrap();
        let mut options = usvg::Options::default();
        context.apply_to_usvg_options(&mut options);
        let mut bahnschrift = Vec::new();
        for (family, weight, style, expected) in [
            ("Arial", 510, "normal", "Arial-BoldMT"),
            ("Bahnschrift", 400, "normal", "Bahnschrift"),
            ("Bahnschrift", 700, "normal", "Bahnschrift"),
            ("Bahnschrift", 700, "oblique", "Bahnschrift"),
        ] {
            let source = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80"><text id="sample" x="4" y="45" font-size="26" font-family="{family}" font-weight="{weight}" font-style="{style}">AV</text></svg>"#
            );
            let tree = usvg::Tree::from_str(&source, &options).unwrap();
            let Some(usvg::Node::Text(text)) = tree.node_by_id("sample") else {
                panic!("expected positioned {family} text");
            };
            let selected: Vec<_> = text
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
            assert_eq!(selected, [expected, expected]);
            if family == "Bahnschrift" {
                bahnschrift.push(tree.to_string(&usvg::WriteOptions::default()));
            }
        }
        let [normal, bold, oblique] = bahnschrift.as_slice() else {
            panic!("expected the three Bahnschrift samples");
        };
        assert_ne!(normal, bold, "the variable weight changes the outlines");
        assert_eq!(
            bold, oblique,
            "the rendered fallback keeps the upright outline"
        );
        context.check().unwrap();
    }

    #[test]
    fn directwrite_uses_scalar_boundaries_and_emoji_presentation() {
        assert_eq!(unmapped_suffix("😀中", 1, 3), None);
        assert_eq!(unmapped_suffix("😀中", 2, 3), Some(2..3));
        assert_eq!(unmapped_suffix("😀中", 3, 3), None);
        for text in ["✓", "∑", "♥", "♥\u{fe0e}", "⌚\u{fe0e}", "e\u{301}", "𠮷"] {
            assert!(!emoji_presentation(text), "{text:?}");
        }
        for text in ["♥\u{fe0f}", "😀", "1\u{20e3}", "🇨🇳", "👩\u{200d}🔬"] {
            assert!(emoji_presentation(text), "{text:?}");
        }
    }

    #[test]
    #[ignore = "requires Windows Segoe UI Emoji and Segoe UI Symbol in isolated collections"]
    fn directwrite_defers_an_emoji_mapping_until_text_faces_are_exhausted() {
        let fonts = PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("Fonts");
        let emoji_path = fonts.join("seguiemj.ttf");
        let symbol_path = fonts.join("seguisym.ttf");
        let mapping = [
            ('♥', w!("Segoe UI Emoji"), 1.0),
            ('🛡', w!("Segoe UI Emoji"), 1.0),
        ];
        let emoji_only = fixture_catalog(&[&emoji_path], &mapping);
        let with_symbol = fixture_catalog(&[&emoji_path, &symbol_path], &mapping);
        let base = base_face();
        let control = OperationControl::new();
        let attempted = |catalog: &Catalog, text: &str| {
            let mut families = Vec::new();
            catalog
                .fallback(text.into(), &base, &control, &mut |candidate| {
                    let FontCandidate::File {
                        expected: ExpectedFace::Platform(face),
                        ..
                    } = candidate
                    else {
                        panic!("expected a physical fallback face");
                    };
                    families.push(face.family);
                    Ok(None)
                })
                .unwrap();
            families
        };
        for text in ["♥", "🛡", "♥\u{fe0e}"] {
            assert_eq!(attempted(&emoji_only, text), ["Segoe UI Emoji"], "{text:?}");
            assert_eq!(
                attempted(&with_symbol, text),
                ["Segoe UI Symbol", "Segoe UI Emoji"],
                "{text:?}"
            );
        }
        assert_eq!(
            attempted(&with_symbol, "♥\u{fe0f}"),
            ["Segoe UI Emoji", "Segoe UI Symbol"]
        );
    }

    #[test]
    #[ignore = "requires the standard Windows text and emoji fonts"]
    fn directwrite_system_fallback_respects_text_and_emoji_presentation() {
        let catalog = Catalog::new().unwrap();
        let base = base_face();
        let control = OperationControl::new();
        let select = |text| {
            let mut selected_family = None;
            let id = catalog
                .fallback(text, &base, &control, &mut |candidate| {
                    let FontCandidate::File {
                        expected: ExpectedFace::Platform(face),
                        ..
                    } = candidate
                    else {
                        panic!("expected a physical fallback face");
                    };
                    selected_family = Some(face.family);
                    Ok(Some(ID::dummy()))
                })
                .unwrap();
            assert!(id.is_some(), "{:?}", text.text);
            selected_family.unwrap()
        };
        for text in ["✓", "∑", "♥\u{fe0e}", "e\u{301}"] {
            assert_ne!(select(text.into()), "Segoe UI Emoji", "{text:?}");
        }
        for text in ["♥\u{fe0f}", "😀"] {
            assert_eq!(select(text.into()), "Segoe UI Emoji", "{text:?}");
        }
    }
}
