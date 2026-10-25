use std::borrow::Cow;

use merman_bindings_core::{BindingError, BindingStatus, binding_error_payload_json_bytes};
use serde::Deserialize;
use serde_json::value::RawValue;

use crate::memory::Addresses;
use crate::runtime::{Limits, limit_error};
use crate::{ABI_VERSION, RESULT_DESCRIPTOR_SIZE};

pub(crate) const STATUS_OK: u32 = 0;
pub(crate) const STATUS_ERROR: u32 = 1;
pub(crate) const RELEASE_OK: i32 = 0;
pub(crate) const INVALID_ALLOCATION: i32 = -1;
pub(crate) const INVALID_HANDLE: i32 = -2;
pub(crate) const BUSY: i32 = -3;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(not(feature = "native-fonts"), allow(dead_code))]
pub(crate) struct Config<'a> {
    pub(crate) version: u32,
    #[serde(borrow)]
    pub(crate) options: Option<&'a RawValue>,
    #[serde(default)]
    pub(crate) fonts: Vec<FontRange>,
    #[serde(default = "bundled_by_default")]
    pub(crate) bundled_fonts: bool,
}

fn bundled_by_default() -> bool {
    cfg!(feature = "bundled-fonts")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(not(feature = "native-fonts"), allow(dead_code))]
pub(crate) struct FontRange {
    pub(crate) offset: u32,
    pub(crate) length: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request<'a> {
    pub(crate) version: u32,
    #[serde(borrow)]
    pub(crate) operation_id: Cow<'a, str>,
    #[serde(borrow)]
    pub(crate) options: Option<&'a RawValue>,
    #[serde(borrow)]
    pub(crate) uri: Option<Cow<'a, str>>,
}

pub(crate) fn parse<'a, T: Deserialize<'a>>(
    bytes: &'a [u8],
    max: usize,
) -> Result<T, BindingError> {
    if bytes.len() > max {
        return Err(limit_error(
            "wasi-json-bytes",
            bytes.len(),
            max,
            "wire JSON is too large",
        ));
    }
    serde_json::from_slice(bytes).map_err(|error| {
        BindingError::invalid_argument(format!("invalid WASI v1 envelope: {error}"))
    })
}

pub(crate) fn check_version(version: u32) -> Result<(), BindingError> {
    if version != ABI_VERSION {
        return Err(BindingError::invalid_argument(
            "unsupported WASI envelope version",
        ));
    }
    Ok(())
}

pub(crate) fn nonempty_options(raw: Option<&RawValue>) -> Option<&[u8]> {
    let raw = raw?.get();
    let inner = raw
        .trim()
        .strip_prefix('{')
        .and_then(|raw| raw.strip_suffix('}'));
    if inner.is_some_and(|inner| inner.trim().is_empty()) {
        None
    } else {
        Some(raw.as_bytes())
    }
}

pub(crate) struct Payload {
    pub(crate) status: u32,
    pub(crate) engine: u32,
    pub(crate) media: &'static str,
    pub(crate) data: Vec<u8>,
    pub(crate) json: Vec<u8>,
}

impl Payload {
    pub(crate) fn error(error: &BindingError) -> Self {
        Self {
            status: STATUS_ERROR,
            engine: 0,
            media: "application/json",
            data: Vec::new(),
            json: binding_error_payload_json_bytes(error),
        }
    }

    pub(crate) fn success(engine: u32, media: &'static str, data: Vec<u8>, json: Vec<u8>) -> Self {
        Self {
            status: STATUS_OK,
            engine,
            media,
            data,
            json,
        }
    }

    pub(crate) fn into_owned(
        self,
        limits: Limits,
        live_bytes: usize,
        addresses: &mut Addresses,
    ) -> Result<OwnedResult, BindingError> {
        let size = [self.media.len(), self.data.len(), self.json.len()]
            .into_iter()
            .try_fold(RESULT_DESCRIPTOR_SIZE, usize::checked_add)
            .ok_or_else(|| BindingError::internal("WASI result length overflow"))?;
        let max = limits
            .result_size
            .min(limits.result_bytes.saturating_sub(live_bytes));
        if size > max {
            return Err(limit_error(
                "wasi-result-bytes",
                size,
                max,
                "result byte budget exhausted",
            ));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .map_err(|_| limit_error("wasi-result-bytes", size, max, "result allocation failed"))?;
        bytes.resize(RESULT_DESCRIPTOR_SIZE, 0);
        bytes.extend_from_slice(self.media.as_bytes());
        bytes.extend_from_slice(&self.data);
        bytes.extend_from_slice(&self.json);
        let mut bytes = bytes.into_boxed_slice();
        let offset = addresses.locate(&bytes).ok_or_else(|| {
            BindingError::new(
                BindingStatus::ResourceLimitExceeded,
                "WASI address space exhausted",
            )
        })?;
        let media_start = RESULT_DESCRIPTOR_SIZE;
        let data_start = media_start + self.media.len();
        let json_start = data_start + self.data.len();
        let segment = |start: usize, len: usize| -> Result<[u32; 2], BindingError> {
            if len == 0 {
                return Ok([0, 0]);
            }
            let len =
                u32::try_from(len).map_err(|_| BindingError::internal("WASI length overflow"))?;
            let start = u32::try_from(start)
                .ok()
                .and_then(|start| offset.checked_add(start))
                .ok_or_else(|| BindingError::internal("WASI offset overflow"))?;
            Ok([start, len])
        };
        let media = segment(media_start, self.media.len())?;
        let data = segment(data_start, self.data.len())?;
        let json = segment(json_start, self.json.len())?;
        let fields = [
            ABI_VERSION,
            RESULT_DESCRIPTOR_SIZE as u32,
            self.status,
            self.engine,
            media[0],
            media[1],
            data[0],
            data[1],
            json[0],
            json[1],
        ];
        for (chunk, value) in bytes[..RESULT_DESCRIPTOR_SIZE]
            .chunks_exact_mut(4)
            .zip(fields)
        {
            chunk.copy_from_slice(&value.to_le_bytes());
        }
        Ok(OwnedResult { offset, bytes })
    }
}

pub(crate) struct OwnedResult {
    pub(crate) offset: u32,
    pub(crate) bytes: Box<[u8]>,
}
