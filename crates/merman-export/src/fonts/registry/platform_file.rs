//! Bounded table reads for a registered SFNT face before acquiring its full file.

use super::*;
use std::io::{Seek, SeekFrom};
use ttf_parser::{Face, RawFaceTables, cmap, fvar, maxp};
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

    pub(super) fn probe(
        &mut self,
        expected: &platform::PlatformFace,
        index: u32,
        request: &CandidateRequest<'_>,
        limits: &NativeFontLimits,
        scan_bytes: &mut usize,
    ) -> Result<Option<Vec<FontVariation>>, NativeFontError> {
        let mut probe = Probe {
            source: self,
            limits,
            scan_bytes,
        };
        let Some(directory) = probe.directory(index)? else {
            return Ok(None);
        };
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
        if Face::from_raw_tables(RawFaceTables {
            head: &head,
            hhea: &hhea,
            maxp: &maxp,
            ..Default::default()
        })
        .is_err()
        {
            return Ok(None);
        }
        // A named bitmap or unsupported format can fall through to the next family.
        // Actual bounds errors after discovery are reported separately as source errors.
        let Some(cmap) = probe.table(&directory, b"cmap", limits.max_system_cmap_table_bytes)?
        else {
            return Ok(None);
        };
        checked_cmap_bytes(&cmap, probe.source.control, 0, limits)?;
        let os2 = probe.prefix(&directory, b"OS/2", 34)?;
        if request.implicit
            && !crate::font_selection::allows_font_tables(
                &head,
                os2.as_deref(),
                &cmap,
                &maxp,
                request.fallback.map(|fallback| fallback.text),
            )
        {
            return Ok(None);
        }
        if let Some(fallback) = request.fallback {
            let Some(glyph_count) =
                maxp::Table::parse(&maxp).map(|table| table.number_of_glyphs.get())
            else {
                return Ok(None);
            };
            if !cmap::Table::parse(&cmap).is_some_and(|cmap| {
                platform::cluster::cmap_may_cover(cmap, glyph_count, fallback.text)
            }) {
                return Ok(None);
            }
            if fallback.scalar().is_none() {
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
                    }) && !platform::cluster::may_shape(face, fallback)
                    {
                        return Ok(None);
                    }
                }
            }
        }
        let fvar = if expected.has_variations() {
            probe.table(&directory, b"fvar", 1024 * 1024)?
        } else {
            None
        };
        axes(expected, fvar.as_deref(), request.instances)
            .map_err(|reason| source_error(probe.source.path, reason))
    }
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
    fn read(&mut self, offset: u64, data: &mut [u8]) -> Result<(), NativeFontError> {
        self.source
            .control
            .terminal_checkpoint_at(OperationPhase::Admission)?;
        let len = data.len();
        if self.source.len.saturating_sub(offset) < len as u64 {
            return Err(source_error(
                self.source.path,
                "font table extends beyond the source",
            ));
        }
        let total = self.scan_bytes.saturating_add(len);
        check_limit(
            "system-discovery-bytes",
            total,
            self.limits.max_system_scan_bytes,
        )?;
        *self.scan_bytes = total;
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
        let mut records = vec![0; entries * 16];
        self.read(offset + 12, &mut records)?;
        Ok(Some(Directory(records)))
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
        let mut data = vec![0; size.min(len)];
        self.read(offset, &mut data)?;
        Ok(Some(data))
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
        let mut data = vec![0; len];
        self.read(offset, &mut data)?;
        Ok(Some(data))
    }
}

pub(super) fn axes(
    expected: &platform::PlatformFace,
    table: Option<&[u8]>,
    instances: bool,
) -> Result<Option<Vec<FontVariation>>, &'static str> {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    let named = expected.named_instance;
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    let named = None;
    if named.is_none() && expected.variations.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let Some((data, table)) = table.and_then(|data| Some((data, fvar::Table::parse(data)?))) else {
        return if named.is_some() {
            Err("font has no valid named instance")
        } else {
            Ok(Some(Vec::new()))
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
            if !instances && value != axis.def_value {
                return Ok(None);
            }
            if instances {
                axes.push(FontVariation::new(
                    tag,
                    if value == 0.0 { 0.0 } else { value },
                ));
            }
        }
    }
    axes.sort_by_key(|axis| axis.tag);
    Ok(Some(axes))
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
