use merman_bindings_core::BindingError;

use crate::runtime::Limits;

/// Native tests use virtual offsets, never truncated native addresses.
#[derive(Default)]
pub(crate) struct Addresses {
    #[cfg(not(target_arch = "wasm32"))]
    next: u32,
}

impl Addresses {
    pub(crate) fn locate(&mut self, bytes: &[u8]) -> Option<u32> {
        #[cfg(target_arch = "wasm32")]
        {
            let offset = u32::try_from(bytes.as_ptr() as usize).ok()?;
            offset.checked_add(u32::try_from(bytes.len()).ok()?)?;
            (offset != 0).then_some(offset)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let start = self.next.max(8);
            let end = start.checked_add(u32::try_from(bytes.len()).ok()?)?;
            self.next = end.checked_add(7)? & !7;
            Some(start)
        }
    }
}

pub(crate) struct Input {
    pub(crate) offset: u32,
    pub(crate) bytes: Box<[u8]>,
}

#[derive(Default)]
pub(crate) struct Inputs {
    pub(crate) entries: Vec<Input>,
    pub(crate) bytes: usize,
}

impl Inputs {
    pub(crate) fn alloc(&mut self, len: u32, limits: Limits, addresses: &mut Addresses) -> u32 {
        let len = len as usize;
        if len == 0
            || len > limits.input_size
            || self.entries.len() >= limits.inputs
            || len > limits.input_bytes.saturating_sub(self.bytes)
            || self.entries.try_reserve(1).is_err()
        {
            return 0;
        }
        let mut bytes = Vec::new();
        if bytes.try_reserve_exact(len).is_err() {
            return 0;
        }
        bytes.resize(len, 0);
        let bytes = bytes.into_boxed_slice();
        let Some(offset) = addresses.locate(&bytes) else {
            return 0;
        };
        self.entries.push(Input { offset, bytes });
        self.bytes += len;
        offset
    }

    pub(crate) fn free(&mut self, offset: u32) -> bool {
        let Some(index) = self.entries.iter().position(|entry| entry.offset == offset) else {
            return false;
        };
        let entry = self.entries.swap_remove(index);
        self.bytes -= entry.bytes.len();
        true
    }

    pub(crate) fn borrow(&self, offset: u32, len: u32) -> Result<&[u8], BindingError> {
        if offset == 0 && len == 0 {
            return Ok(&[]);
        }
        offset
            .checked_add(len)
            .ok_or_else(|| BindingError::invalid_argument("input offset plus length overflows"))?;
        for entry in &self.entries {
            let Some(start) = offset.checked_sub(entry.offset) else {
                continue;
            };
            let start = start as usize;
            let Some(end) = start.checked_add(len as usize) else {
                continue;
            };
            if let Some(bytes) = entry.bytes.get(start..end) {
                return Ok(bytes);
            }
        }
        Err(BindingError::invalid_argument(
            "input range does not belong to a live module allocation",
        ))
    }
}

#[cfg(feature = "native-fonts")]
pub(crate) fn copy_font(bytes: &[u8]) -> Result<Vec<u8>, BindingError> {
    let mut owned = Vec::new();
    owned.try_reserve_exact(bytes.len()).map_err(|_| {
        crate::runtime::limit_error(
            "wasi-font-bytes",
            bytes.len(),
            bytes.len(),
            "font allocation failed",
        )
    })?;
    owned.extend_from_slice(bytes);
    Ok(owned)
}
