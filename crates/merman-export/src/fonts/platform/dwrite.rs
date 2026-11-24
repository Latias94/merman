//! DirectWrite family matching and cluster fallback over local physical font faces.

use super::{
    AcceptFallback, GlyphFallback, NativeFontError, OperationControl, OperationPhase, PlatformFace,
    cluster, ranked_faces,
};
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
    DWRITE_FONT_SIMULATIONS_NONE, DWRITE_FONT_STRETCH, DWRITE_FONT_STYLE, DWRITE_FONT_STYLE_ITALIC,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STYLE_OBLIQUE, DWRITE_FONT_WEIGHT, DWRITE_LOCALITY_LOCAL,
    DWRITE_READING_DIRECTION, DWRITE_READING_DIRECTION_LEFT_TO_RIGHT,
    DWRITE_READING_DIRECTION_RIGHT_TO_LEFT, DWriteCreateFactory, IDWriteFactory, IDWriteFactory2,
    IDWriteFactory3, IDWriteFactory6, IDWriteFont, IDWriteFont3, IDWriteFontCollection,
    IDWriteFontFace, IDWriteFontFace2, IDWriteFontFace5, IDWriteFontFaceReference1,
    IDWriteFontFallback, IDWriteFontFallback1, IDWriteFontFamily1, IDWriteFontFamily2,
    IDWriteFontList, IDWriteLocalFontFileLoader, IDWriteNumberSubstitution,
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
    face: Option<IDWriteFontFace>,
    scale: f32,
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

    pub(super) fn candidates(
        &self,
        family: &str,
        weight: Weight,
        stretch: Stretch,
        style: Style,
        control: &OperationControl,
        accept: &mut AcceptFallback<'_>,
    ) -> Result<Option<ID>, NativeFontError> {
        let mut seen = HashSet::new();
        self.family_faces(
            family,
            FaceStyle {
                weight,
                stretch,
                style,
            },
            control,
            |face| {
                let candidate = file_face(&face, 1.0, None);
                control.terminal_checkpoint_at(OperationPhase::Layout)?;
                if let Some((path, face)) = candidate
                    && seen.insert((path.clone(), face.index, face.variations.clone()))
                {
                    return accept(face.into_file(path));
                }
                Ok(None)
            },
        )
    }

    fn family_faces(
        &self,
        name: &str,
        style: FaceStyle,
        control: &OperationControl,
        mut visit: impl FnMut(IDWriteFontFace) -> Result<Option<ID>, NativeFontError>,
    ) -> Result<Option<ID>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        if name.is_empty() || name.contains('\0') {
            return Ok(None);
        }
        let name: Vec<_> = name.encode_utf16().chain([0]).collect();
        let collection = self.collection()?;
        let mut index = 0;
        let mut exists = Default::default();
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
            let locality = family.cast::<IDWriteFontFamily1>().ok();
            let list: &IDWriteFontList = modern.as_ref().map_or(&family, |list| list);
            let mut candidates = Vec::new();
            for index in 0..list.GetFontCount() {
                control.terminal_checkpoint_at(OperationPhase::Layout)?;
                let locality = match &modern {
                    Some(list) => Some(list.GetFontLocality(index)),
                    None => locality
                        .as_ref()
                        .map(|family| family.GetFontLocality(index)),
                };
                if locality.is_some_and(|value| value != DWRITE_LOCALITY_LOCAL) {
                    continue;
                }
                let Some(font) = candidate_result(list.GetFont(index), "reading a family member")?
                else {
                    continue;
                };
                // The physical style is listed separately from synthetic family members.
                if font.GetSimulations() != DWRITE_FONT_SIMULATIONS_NONE {
                    continue;
                }
                let attributes = FaceStyle::from_font(&font);
                candidates.push((
                    (index, font),
                    attributes.weight,
                    attributes.stretch,
                    attributes.style,
                ));
            }
            for (index, font) in ranked_faces(candidates, style.weight, style.stretch, style.style)
            {
                control.terminal_checkpoint_at(OperationPhase::Layout)?;
                let face = match &modern {
                    // The old GetFont/CreateFontFace pair cannot carry the matched axis values.
                    Some(list) => candidate_result(
                        list.GetFontFaceReference(index).and_then(|reference| {
                            if let Ok(reference) = reference.cast::<IDWriteFontFaceReference1>() {
                                reference.CreateFontFace().map(Into::into)
                            } else {
                                reference.CreateFontFace().map(Into::into)
                            }
                        }),
                        "opening a matched font instance",
                    )?,
                    None => local_font_face(&font)?,
                };
                control.terminal_checkpoint_at(OperationPhase::Layout)?;
                if let Some(face) = face
                    && let Some(id) = visit(face)?
                {
                    return Ok(Some(id));
                }
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
        let mut prepare = |native_face: IDWriteFontFace, scale| {
            let candidate = file_face(&native_face, scale, Some(request));
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            let Some((path, face)) = candidate else {
                return Ok(None);
            };
            let emoji_family = face.family.to_ascii_lowercase().contains("emoji");
            if !emoji && emoji_family {
                return Ok(None);
            }
            let key = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            if !seen.insert((key, face.index, face.variations.clone(), scale.to_bits())) {
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
            if let Some(face) = mapped.face
                && let Some((candidate, presents_emoji)) = prepare(face, mapped.scale)?
            {
                if emoji && !presents_emoji {
                    // DirectWrite can map the base character of an emoji variation sequence
                    // to a plain text face. Keep that map as a fallback for the emoji search.
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
            self.family_faces(family, style, control, |face| match prepare(face, 1.0)? {
                Some((candidate, _)) => accept(candidate),
                None => Ok(None),
            })
        };
        if emoji && let Some(id) = supplemental("Segoe UI Emoji", accept)? {
            return Ok(Some(id));
        }
        for candidate in deferred {
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            if let Some(id) = accept(candidate)? {
                return Ok(Some(id));
            }
        }
        for family in ["Segoe UI Symbol", "Segoe UI", "Arial", "Cambria Math"] {
            if let Some(id) = supplemental(family, accept)? {
                return Ok(Some(id));
            }
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
                            face: face.map(Into::into),
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
                Some(font) => local_font_face(&font)?,
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

impl FaceStyle {
    fn from_font(font: &IDWriteFont) -> Self {
        // SAFETY: These property getters only borrow the live font interface.
        unsafe {
            Self {
                weight: Weight(font.GetWeight().0.clamp(1, 1000) as u16),
                stretch: match font.GetStretch().0 {
                    1 => Stretch::UltraCondensed,
                    2 => Stretch::ExtraCondensed,
                    3 => Stretch::Condensed,
                    4 => Stretch::SemiCondensed,
                    6 => Stretch::SemiExpanded,
                    7 => Stretch::Expanded,
                    8 => Stretch::ExtraExpanded,
                    9 => Stretch::UltraExpanded,
                    _ => Stretch::Normal,
                },
                style: match font.GetStyle() {
                    DWRITE_FONT_STYLE_ITALIC => Style::Italic,
                    DWRITE_FONT_STYLE_OBLIQUE => Style::Oblique,
                    _ => Style::Normal,
                },
            }
        }
    }

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
        let width = [50., 62.5, 75., 87.5, 100., 112.5, 125., 150., 200.];
        [
            DWRITE_FONT_AXIS_VALUE {
                axisTag: DWRITE_FONT_AXIS_TAG_WEIGHT,
                value: self.weight.0.clamp(1, 1000).into(),
            },
            DWRITE_FONT_AXIS_VALUE {
                axisTag: DWRITE_FONT_AXIS_TAG_WIDTH,
                value: width[usize::from(self.stretch.to_number() - 1)],
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
    face: &IDWriteFontFace,
    fallback_scale: f32,
    request: Option<GlyphFallback<'_>>,
) -> Option<(PathBuf, PlatformFace)> {
    if !fallback_scale.is_finite() || fallback_scale <= 0.0 {
        return None;
    }
    let path = file_path(face)?;
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
    Some((
        path,
        PlatformFace {
            index,
            family,
            post_script_name: physical_name(names, name_id::POST_SCRIPT_NAME),
            other_families: Vec::new(),
            variations: variations(face)?,
            fallback_scale,
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

fn file_path(face: &IDWriteFontFace) -> Option<PathBuf> {
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
        path.is_absolute().then_some(path)
    }
}

fn variations(face: &IDWriteFontFace) -> Option<Vec<FontVariation>> {
    let Ok(instance) = face.cast::<IDWriteFontFace5>() else {
        return Some(Vec::new());
    };
    // SAFETY: The live face reports the required size before the writable slice is passed in.
    let mut values = unsafe {
        if !instance.HasVariations().as_bool() {
            return Some(Vec::new());
        }
        let count = usize::try_from(instance.GetFontAxisValueCount()).ok()?;
        let mut values = Vec::new();
        values.try_reserve_exact(count).ok()?;
        values.resize(count, DWRITE_FONT_AXIS_VALUE::default());
        instance.GetFontAxisValues(&mut values).ok()?;
        values
    };
    let table = FontTable::read(face, b"fvar")?;
    physical_variations(fvar::Table::parse(table.bytes())?, &mut values)
}

fn physical_variations(
    table: fvar::Table<'_>,
    values: &mut [DWRITE_FONT_AXIS_VALUE],
) -> Option<Vec<FontVariation>> {
    let mut variations = Vec::new();
    variations
        .try_reserve_exact(usize::from(table.axes.len()))
        .ok()?;
    values.sort_unstable_by_key(|value| value.axisTag.0);
    // DirectWrite also reports synthetic CSS axes. Only the physical fvar axes can
    // be instantiated by the renderer.
    for axis in table.axes {
        let tag = axis.tag.to_bytes();
        let index = values
            .binary_search_by_key(&u32::from_le_bytes(tag), |value| value.axisTag.0)
            .ok()?;
        let value = values[index].value;
        if !value.is_finite() {
            return None;
        }
        let value = value.clamp(axis.min_value, axis.max_value);
        variations.push(FontVariation::new(
            tag,
            if value == 0.0 { 0.0 } else { value },
        ));
    }
    variations.sort_unstable_by_key(|axis| axis.tag);
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

fn covers_cluster(face: &IDWriteFontFace, request: GlyphFallback<'_>) -> bool {
    let [Some(head), Some(cmap), Some(maxp), os2] =
        [b"head", b"cmap", b"maxp", b"OS/2"].map(|tag| FontTable::read(face, tag))
    else {
        return false;
    };
    if !crate::font_selection::allows_font_tables(
        head.bytes(),
        os2.as_ref().map(FontTable::bytes),
        cmap.bytes(),
        maxp.bytes(),
        Some(request.text),
    ) {
        return false;
    }
    cmap::Table::parse(cmap.bytes())
        .zip(maxp::Table::parse(maxp.bytes()))
        .is_some_and(|(cmap, maxp)| {
            cluster::cmap_may_cover(cmap, maxp.number_of_glyphs.get(), request.text)
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
            )
            .ok()?;
        }
        if !exists.as_bool() {
            return None;
        }
        let table = Self {
            face,
            context,
            data: data.cast(),
            size: size as usize,
        };
        (!table.data.is_null() && table.size <= isize::MAX as usize).then_some(table)
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

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fonts")
            .join(name)
    }

    fn fixture_catalog(files: &[&str], mappings: &[(char, PCWSTR, f32)]) -> Catalog {
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
            physical_variations(table, &mut [width, weight(700.0)]).unwrap(),
            [FontVariation::new(*b"wght", 700.0)]
        );
        assert_eq!(
            physical_variations(table, &mut [weight(1000.0)]).unwrap(),
            [FontVariation::new(*b"wght", 900.0)]
        );
        assert!(physical_variations(table, &mut [width]).is_none());
        assert!(physical_variations(table, &mut [weight(f32::NAN)]).is_none());
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
    fn directwrite_continues_through_ranked_family_members() {
        let catalog = fixture_catalog(&["Cousine-test-collection.ttc"], &[]);
        let control = OperationControl::new();
        let mut tried = Vec::new();
        let mut accept = |candidate| {
            let (_, _, physical) = physical_face(candidate);
            tried.push(physical.index);
            Ok((physical.index == 0).then_some(physical.id))
        };
        assert!(
            catalog
                .candidates(
                    "cOuSiNe",
                    Weight::NORMAL,
                    Stretch::Normal,
                    Style::Italic,
                    &control,
                    &mut accept
                )
                .unwrap()
                .is_some()
        );
        assert!(
            catalog
                .candidates(
                    "Arial",
                    Weight::NORMAL,
                    Stretch::Normal,
                    Style::Normal,
                    &control,
                    &mut accept
                )
                .unwrap()
                .is_none()
        );
        assert_eq!(tried, [1, 0]);
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
        let (path, face) = file_face(&simulated, 1.25, Some("V".into())).unwrap();
        let (_, expected, physical) = physical_face(face.into_file(path));
        assert_eq!(expected.fallback_scale, 1.25);
        assert_eq!(physical.weight, Weight::NORMAL);
    }

    #[test]
    fn directwrite_carries_modern_variable_family_and_fallback_axes() {
        let catalog = fixture_catalog(
            &["MermanVariableWeight.ttf"],
            &[('A', w!("Merman Variable Weight"), 1.125)],
        );
        // SAFETY: The shared factory request only negotiates the available interface.
        if unsafe { DWriteCreateFactory::<IDWriteFactory6>(DWRITE_FACTORY_TYPE_SHARED) }.is_err() {
            return;
        }
        assert!(catalog.modern_fallback.is_some());
        let mut base = base_face();
        base.weight = Weight(700);
        let control = OperationControl::new();
        let check = |scale, candidate| {
            let (_, expected, physical) = physical_face(candidate);
            assert_eq!(expected.fallback_scale, scale);
            assert_eq!(expected.variations, [FontVariation::new(*b"wght", 700.0)]);
            assert_eq!(physical.weight, Weight(100));
            Ok(Some(physical.id))
        };
        assert!(
            catalog
                .candidates(
                    "Merman Variable Weight",
                    base.weight,
                    base.stretch,
                    base.style,
                    &control,
                    &mut |candidate| check(1.0, candidate)
                )
                .unwrap()
                .is_some()
        );
        assert!(
            catalog
                .fallback("A".into(), &base, &control, &mut |candidate| check(
                    1.125, candidate
                ))
                .unwrap()
                .is_some()
        );
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
