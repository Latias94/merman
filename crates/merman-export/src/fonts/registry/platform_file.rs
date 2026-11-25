//! Bounded table reads for a registered SFNT face before acquiring its full file.

use super::super::resolver::GlyphFallback;
use super::*;
use std::io::{Seek, SeekFrom};
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
use ttf_parser::Tag;
use ttf_parser::{Face, RawFaceTables, fvar};
use usvg::FontVariation;

pub(super) struct OpenPlatformFont<'a> {
    pub(super) file: fs::File,
    pub(super) len: u64,
    path: &'a Path,
    control: &'a OperationControl,
}

impl<'a> OpenPlatformFont<'a> {
    pub(super) fn new(
        path: &'a Path,
        control: &'a OperationControl,
    ) -> Result<Self, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Admission)?;
        if !fs::metadata(path)
            .map_err(|error| source_error(path, error))?
            .is_file()
        {
            return Err(source_error(path, "expected a regular font file"));
        }
        let file = fs::File::open(path).map_err(|error| source_error(path, error))?;
        let metadata = file.metadata().map_err(|error| source_error(path, error))?;
        if !metadata.is_file() {
            return Err(source_error(path, "expected a regular font file"));
        }
        Ok(Self {
            file,
            len: metadata.len(),
            path,
            control,
        })
    }

    pub(super) fn rewind(&mut self) -> Result<(), NativeFontError> {
        self.file
            .rewind()
            .map_err(|error| source_error(self.path, error))
    }

    #[cfg(windows)]
    pub(super) fn validate_dwrite_file_identity(
        &self,
        expected: &platform::PlatformFace,
    ) -> Result<(), NativeFontError> {
        if let Some(identity) = expected.dwrite_identity {
            self.control
                .terminal_checkpoint_at(OperationPhase::Admission)?;
            let metadata = self
                .file
                .metadata()
                .map_err(|error| source_error(self.path, error))?;
            if metadata.last_write_time() != identity.last_write_time || metadata.len() != self.len
            {
                return Err(source_error(
                    self.path,
                    "font file metadata changed after DirectWrite lookup; retry the operation",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn probe(
        &mut self,
        expected: &platform::PlatformFace,
        index: u32,
        request: &CandidateRequest<'_>,
        limits: &NativeFontLimits,
        scan_bytes: &mut usize,
    ) -> Result<Option<Vec<FontVariation>>, NativeFontError> {
        #[cfg(windows)]
        self.validate_dwrite_file_identity(expected)?;
        let mut probe = Probe {
            source: self,
            limits,
            scan_bytes,
        };
        let Some(directory) = probe.directory(index)? else {
            return Ok(None);
        };
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        if let Some(required) = &expected.required_uvs
            && !probe.has_variation_subtable(&directory, request.fallback, required)?
        {
            return Ok(None);
        }
        let Some(head) = probe.prefix(&directory, b"head", 54)? else {
            return Ok(None);
        };
        if request.implicit && crate::font_selection::is_last_resort_head(&head) {
            return Ok(None);
        }
        let Some(hhea) = probe.prefix(&directory, b"hhea", 36)? else {
            return Ok(None);
        };
        let Some(maxp) = probe.prefix(&directory, b"maxp", 6)? else {
            return Ok(None);
        };
        let Ok(headers) = Face::from_raw_tables(RawFaceTables {
            head: &head,
            hhea: &hhea,
            maxp: &maxp,
            ..Default::default()
        }) else {
            return Ok(None);
        };
        // A named bitmap or unsupported format can fall through to the next family.
        // Actual bounds errors after discovery are reported separately as source errors.
        let Some(cmap) = probe.table(&directory, b"cmap", limits.max_system_cmap_table_bytes)?
        else {
            return Ok(None);
        };
        checked_cmap_bytes(&cmap, probe.source.control, 0, limits)?;
        let glyph_count = headers.number_of_glyphs();
        if !required_variation_matches(expected, Some(&cmap), glyph_count, request.fallback) {
            return Ok(None);
        }
        let os2 = if request.fallback.is_some() || request.implicit {
            // Include the full version-zero prefix used by legacy symbol fonts.
            probe.prefix(&directory, b"OS/2", 78)?
        } else {
            None
        };
        if request.implicit {
            if !crate::font_selection::allows_font_tables(
                &head,
                os2.as_deref(),
                &cmap,
                &maxp,
                request
                    .fallback
                    .map(|fallback| (fallback.text, fallback.rtl)),
            ) {
                return Ok(None);
            }
        }
        if let Some(fallback) = request.fallback {
            if !platform::cluster::cmap_may_cover_with_os2(
                &cmap,
                os2.as_deref(),
                glyph_count,
                fallback.text,
                fallback.rtl,
            ) {
                return Ok(None);
            }
            if fallback.scalar().is_none()
                && !platform::cluster::has_variation_sequence(fallback.text)
            {
                // Only small substitution tables participate in the speculative shape.
                // Defer expensive or variation-dependent shaping to the admitted face.
                const MAX_PROBE_LAYOUT_TABLE: usize = 256 * 1024;
                let tags = [b"GSUB", b"GPOS", b"GDEF", b"morx", b"feat"];
                if directory.entry(b"fvar").is_none()
                    && tags.iter().all(|tag| {
                        directory
                            .entry(tag)
                            .is_none_or(|(_, len)| len <= MAX_PROBE_LAYOUT_TABLE)
                    })
                {
                    let gsub = probe.table(&directory, b"GSUB", MAX_PROBE_LAYOUT_TABLE)?;
                    let gpos = probe.table(&directory, b"GPOS", MAX_PROBE_LAYOUT_TABLE)?;
                    let gdef = probe.table(&directory, b"GDEF", MAX_PROBE_LAYOUT_TABLE)?;
                    let morx = probe.table(&directory, b"morx", MAX_PROBE_LAYOUT_TABLE)?;
                    let feat = probe.table(&directory, b"feat", MAX_PROBE_LAYOUT_TABLE)?;
                    if let Ok(face) = Face::from_raw_tables(RawFaceTables {
                        head: &head,
                        hhea: &hhea,
                        maxp: &maxp,
                        cmap: Some(&cmap),
                        gsub: gsub.as_deref(),
                        gpos: gpos.as_deref(),
                        gdef: gdef.as_deref(),
                        morx: morx.as_deref(),
                        feat: feat.as_deref(),
                        ..Default::default()
                    }) && !platform::cluster::may_shape_after_cmap(face, fallback)
                    {
                        return Ok(None);
                    }
                }
            }
        }
        #[cfg(windows)]
        if let Some(identity) = expected.dwrite_identity {
            let hmtx = if directory
                .entry(b"hmtx")
                .is_some_and(|(_, len)| len == identity.hmtx_len)
            {
                probe.table(&directory, b"hmtx", identity.hmtx_len)?
            } else {
                None
            };
            validate_dwrite_metrics(identity, hmtx.as_deref(), probe.source.path)?;
        }
        let fvar = if expected.has_variations() {
            probe.table(&directory, b"fvar", platform::MAX_FVAR_TABLE_BYTES)?
        } else {
            None
        };
        axes(expected, fvar.as_deref())
            .map(Some)
            .map_err(|reason| source_error(probe.source.path, reason))
    }
}

#[cfg(windows)]
pub(super) fn validate_dwrite_metrics(
    identity: platform::DwriteFileIdentity,
    hmtx: Option<&[u8]>,
    path: &Path,
) -> Result<(), NativeFontError> {
    if !hmtx.is_some_and(|hmtx| {
        hmtx.len() == identity.hmtx_len
            && <[u8; 32]>::from(Sha256::digest(hmtx)) == identity.hmtx_digest
    }) {
        return Err(source_error(
            path,
            "font horizontal metrics changed after DirectWrite lookup; retry the operation",
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(super) fn has_required_variation(
    expected: &platform::PlatformFace,
    face: &Face<'_>,
    request: &CandidateRequest<'_>,
) -> bool {
    required_variation_matches(
        expected,
        face.raw_face().table(Tag::from_bytes(b"cmap")),
        face.number_of_glyphs(),
        request.fallback,
    )
}

fn required_variation_matches(
    expected: &platform::PlatformFace,
    mapping: Option<&[u8]>,
    glyph_count: u16,
    fallback: Option<GlyphFallback<'_>>,
) -> bool {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    if let Some(required) = &expected.required_uvs {
        return mapping.zip(fallback).is_some_and(|(cmap, request)| {
            crate::font_selection::PlatformCmap::parse(cmap, glyph_count).is_some_and(|cmap| {
                if required.is_empty() {
                    platform::cluster::variation_sequences(request.text)
                        .any(cmap.maps_variation(request.rtl, true))
                } else {
                    required
                        .iter()
                        .copied()
                        .all(cmap.maps_variation(request.rtl, false))
                }
            })
        });
    }
    let _ = (expected, mapping, glyph_count, fallback);
    true
}

struct Probe<'a, 'file> {
    source: &'a mut OpenPlatformFont<'file>,
    limits: &'a NativeFontLimits,
    scan_bytes: &'a mut usize,
}

struct Directory(Vec<u8>);

impl Directory {
    fn entry(&self, tag: &[u8; 4]) -> Option<(u64, usize)> {
        self.0
            .as_chunks::<16>()
            .0
            .iter()
            .find(|entry| &entry[..4] == tag)
            .map(|entry| {
                (
                    u64::from(be_u32(&entry[8..12])),
                    be_u32(&entry[12..16]) as usize,
                )
            })
    }
}

impl Probe<'_, '_> {
    fn contains(&self, offset: u64, len: usize) -> bool {
        self.source
            .len
            .checked_sub(offset)
            .is_some_and(|remaining| remaining >= len as u64)
    }

    fn check_range(&self, offset: u64, len: usize) -> Result<(), NativeFontError> {
        if !self.contains(offset, len) {
            return Err(source_error(
                self.source.path,
                "font table extends beyond the source",
            ));
        }
        Ok(())
    }

    fn reserve_read(&mut self, offset: u64, len: usize) -> Result<(), NativeFontError> {
        self.source
            .control
            .terminal_checkpoint_at(OperationPhase::Admission)?;
        self.check_range(offset, len)?;
        let total = self.scan_bytes.saturating_add(len);
        check_limit(
            "system-discovery-bytes",
            total,
            self.limits.max_system_scan_bytes,
        )?;
        *self.scan_bytes = total;
        Ok(())
    }

    fn read(&mut self, offset: u64, data: &mut [u8]) -> Result<(), NativeFontError> {
        self.reserve_read(offset, data.len())?;
        self.read_reserved(offset, data)
    }

    fn read_vec(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, NativeFontError> {
        self.reserve_read(offset, len)?;
        let mut data = vec![0; len];
        self.read_reserved(offset, &mut data)?;
        Ok(data)
    }

    fn read_reserved(&mut self, offset: u64, data: &mut [u8]) -> Result<(), NativeFontError> {
        self.source
            .file
            .seek(SeekFrom::Start(offset))
            .and_then(|_| self.source.file.read_exact(data))
            .map_err(|error| source_error(self.source.path, error))
    }

    fn directory(&mut self, index: u32) -> Result<Option<Directory>, NativeFontError> {
        if self.source.len < 12 {
            return Ok(None);
        }
        let mut header = [0; 12];
        self.read(0, &mut header)?;
        let offset = if &header[..4] == b"ttcf" {
            let count = be_u32(&header[8..]);
            if index >= count {
                return Err(source_error(
                    self.source.path,
                    "selected collection face is missing",
                ));
            }
            let mut offset = [0; 4];
            self.read(12 + 4 * u64::from(index), &mut offset)?;
            let offset = u64::from(be_u32(&offset));
            self.read(offset, &mut header)?;
            offset
        } else if index == 0 {
            0
        } else {
            return Err(source_error(
                self.source.path,
                "selected collection face is missing",
            ));
        };
        if !matches!(&header[..4], b"\0\x01\0\0" | b"OTTO" | b"true") {
            return Ok(None);
        }
        let entries = usize::from(be_u16(&header[4..6]));
        self.read_vec(offset + 12, entries * 16)
            .map(Directory)
            .map(Some)
    }

    fn prefix(
        &mut self,
        directory: &Directory,
        tag: &[u8; 4],
        size: usize,
    ) -> Result<Option<Vec<u8>>, NativeFontError> {
        let Some((offset, len)) = directory.entry(tag) else {
            return Ok(None);
        };
        self.check_range(offset, len)?;
        self.read_vec(offset, size.min(len)).map(Some)
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn has_variation_subtable(
        &mut self,
        directory: &Directory,
        fallback: Option<GlyphFallback<'_>>,
        required: &[(char, char)],
    ) -> Result<bool, NativeFontError> {
        let Some(request) = fallback else {
            return Ok(false);
        };
        let Some((offset, len)) = directory.entry(b"cmap") else {
            return Ok(false);
        };
        // An invalid cmap cannot supply a UVS. Ordinary admission still checks
        // the source range and reports the error for a selected candidate.
        if len < 4 || !self.contains(offset, len) {
            return Ok(false);
        }
        let mut header = [0; 4];
        self.read(offset, &mut header)?;
        let count = usize::from(be_u16(&header[2..]));
        if header[..2] != [0, 0] || 4 + count * 8 > len {
            return Ok(false);
        }
        check_limit("system-cmap-subtables", count, 32)?;
        let mut records = [0; 32 * 8];
        let records = &mut records[..count * 8];
        self.read(offset + 4, records)?;
        let mut selected = None;
        for record in records.as_chunks::<8>().0.iter().rev() {
            if record[..4] != [0, 0, 0, 5] {
                continue;
            }
            let start = be_u32(&record[4..]) as usize;
            let start_offset = offset + start as u64;
            let remaining = len.saturating_sub(start);
            if remaining < 2 || !self.contains(start_offset, 2) {
                continue;
            }
            if remaining < 10 {
                continue;
            }
            let mut header = [0; 10];
            self.read(start_offset, &mut header)?;
            let glyph_width = match be_u16(&header[..2]) {
                14 => 5,
                15 => 6,
                _ => continue,
            };
            let length = be_u32(&header[2..6]) as usize;
            let count = be_u32(&header[6..]) as usize;
            let table = UvsTable {
                offset: start_offset,
                length,
                count,
                glyph_width,
            };
            // Use the same reverse search and format 15 precedence as read-fonts.
            let table = (table, remaining);
            if glyph_width == 6 {
                selected = Some(table);
                break;
            }
            selected.get_or_insert(table);
        }
        let Some((table, remaining)) = selected else {
            return Ok(false);
        };
        if !(10..=remaining).contains(&table.length) || table.count > (table.length - 10) / 11 {
            return Ok(false);
        }

        let allow_default = required.is_empty();
        let variants = platform::cluster::variation_sequences(request.text)
            .take_while(|_| allow_default)
            .chain(required.iter().copied());
        for (base, selector) in variants {
            let mut found = self.has_uvs_pair(&table, base, selector, allow_default)?;
            // The effective base depends on the preferred nominal cmap. Keep
            // either possibility here and resolve the direction after parsing it.
            if !found
                && request.rtl
                && let Some(mirror) = unicode_bidi_mirroring::get_mirrored(base)
            {
                found = self.has_uvs_pair(&table, mirror, selector, allow_default)?;
            }
            if found == allow_default {
                return Ok(found);
            }
        }
        Ok(!allow_default)
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn has_uvs_pair(
        &mut self,
        table: &UvsTable,
        base: char,
        selector: char,
        allow_default: bool,
    ) -> Result<bool, NativeFontError> {
        let mut left = 0;
        let mut right = table.count;
        while left < right {
            let mid = left + (right - left) / 2;
            let mut record = [0; 11];
            self.read(table.offset + 10 + 11 * mid as u64, &mut record)?;
            match be_u24(&record).cmp(&u32::from(selector)) {
                std::cmp::Ordering::Less => left = mid + 1,
                std::cmp::Ordering::Greater => right = mid,
                std::cmp::Ordering::Equal => {
                    let non_default = be_u32(&record[7..]) as usize;
                    let default = be_u32(&record[3..7]) as usize;
                    return Ok(
                        (allow_default && self.has_uvs_mapping(table, default, base, 4)?)
                            || self.has_uvs_mapping(table, non_default, base, table.glyph_width)?,
                    );
                }
            }
        }
        Ok(false)
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn has_uvs_mapping(
        &mut self,
        table: &UvsTable,
        start: usize,
        base: char,
        width: usize,
    ) -> Result<bool, NativeFontError> {
        let Some(remaining) = table.length.checked_sub(start) else {
            return Ok(false);
        };
        if start == 0 || remaining < 4 {
            return Ok(false);
        }
        let offset = table.offset + start as u64;
        let mut count = [0; 4];
        self.read(offset, &mut count)?;
        let mut right = be_u32(&count) as usize;
        if right > (remaining - 4) / width {
            return Ok(false);
        }
        let base = u32::from(base);
        let mut left = 0;
        while left < right {
            let mid = left + (right - left) / 2;
            let mut record = [0; 6];
            self.read(offset + 4 + (width * mid) as u64, &mut record[..width])?;
            let first = be_u24(&record);
            let last = first + if width == 4 { u32::from(record[3]) } else { 0 };
            if base < first {
                right = mid;
            } else if base > last {
                left = mid + 1;
            } else {
                return Ok(width == 4 || record[3..width].iter().any(|&byte| byte != 0));
            }
        }
        Ok(false)
    }

    fn table(
        &mut self,
        directory: &Directory,
        tag: &[u8; 4],
        max: usize,
    ) -> Result<Option<Vec<u8>>, NativeFontError> {
        let Some((offset, len)) = directory.entry(tag) else {
            return Ok(None);
        };
        let resource = if tag == b"cmap" {
            "system-cmap-table-bytes"
        } else {
            "system-font-table-bytes"
        };
        check_limit(resource, len, max)?;
        if tag == b"cmap" {
            check_limit("system-cmap-bytes", len, self.limits.max_system_cmap_bytes)?;
        }
        self.read_vec(offset, len).map(Some)
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
struct UvsTable {
    offset: u64,
    length: usize,
    count: usize,
    glyph_width: usize,
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn be_u24(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]])
}

pub(super) fn axes(
    expected: &platform::PlatformFace,
    table: Option<&[u8]>,
) -> Result<Vec<FontVariation>, &'static str> {
    #[cfg(windows)]
    if let Some(digest) = expected.fvar_digest
        && !table.is_some_and(|data| <[u8; 32]>::from(Sha256::digest(data)) == digest)
    {
        return Err("font no longer has the variation axes selected by the platform");
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    let named = expected.named_instance;
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    let named = None;
    if named.is_none() && expected.variations.is_empty() {
        return Ok(Vec::new());
    }
    let Some((data, table)) = table.and_then(|data| Some((data, fvar::Table::parse(data)?))) else {
        return if named.is_some() {
            Err("font has no valid named instance")
        } else if cfg!(windows) {
            Err("font no longer has the variation axes selected by the platform")
        } else {
            // Fontconfig render rules can supply axis hints for a static face.
            Ok(Vec::new())
        };
    };
    let coordinates = match named {
        Some(index) => Some(
            instance_coordinates(data, index, table.axes.len())
                .ok_or("selected named instance is missing")?,
        ),
        None => None,
    };
    let mut axes = Vec::new();
    for (index, axis) in table.axes.into_iter().enumerate() {
        let tag = axis.tag.to_bytes();
        let value = expected
            .variations
            .iter()
            .rev()
            .find(|value| value.tag == tag && value.value.is_finite())
            .map(|value| value.value)
            .or_else(|| coordinates.map(|data| fixed(&data[index * 4..][..4])));
        if let Some(value) = value {
            let value = value.clamp(axis.min_value, axis.max_value);
            axes.push(FontVariation::new(tag, value + 0.0));
        }
    }
    axes.sort_by_key(|axis| axis.tag);
    if cfg!(windows) && axes != expected.variations {
        return Err("font no longer has the variation axes selected by the platform");
    }
    Ok(axes)
}

// fvar instance indices in Fontconfig are one-based. Coordinates start after
// subfamilyNameID and flags in each fixed-size instance record.
fn instance_coordinates(data: &[u8], index: u16, axis_count: u16) -> Option<&[u8]> {
    let header = data.get(..16)?;
    let axis_offset = usize::from(be_u16(&header[4..6]));
    let axis_size = usize::from(be_u16(&header[10..12]));
    let instance_count = be_u16(&header[12..14]);
    let instance_size = usize::from(be_u16(&header[14..16]));
    let index = usize::from(index.checked_sub(1)?);
    let coords_size = usize::from(axis_count) * 4;
    if axis_size != 20 || index >= usize::from(instance_count) || instance_size < 4 + coords_size {
        return None;
    }
    let offset = axis_offset + usize::from(axis_count) * axis_size;
    let offset = offset
        .checked_add(index.checked_mul(instance_size)?)?
        .checked_add(4)?;
    data.get(offset..offset.checked_add(coords_size)?)
}

fn be_u16(bytes: &[u8]) -> u16 {
    u16::from_be_bytes(bytes.try_into().expect("two-byte SFNT field"))
}

fn be_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().expect("four-byte SFNT field"))
}

fn fixed(bytes: &[u8]) -> f32 {
    i32::from_be_bytes(bytes.try_into().expect("four-byte SFNT coordinate")) as f32 / 65536.0
}
