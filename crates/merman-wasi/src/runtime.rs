use merman_bindings_core::{BindingEngine, BindingError, BindingOperationRequest, MetadataKey};

use crate::fonts;
use crate::memory::{Addresses, Inputs};
use crate::wire::{self, OwnedResult, Payload, Request};

#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub(crate) inputs: usize,
    pub(crate) input_size: usize,
    pub(crate) input_bytes: usize,
    pub(crate) results: usize,
    pub(crate) result_size: usize,
    pub(crate) result_bytes: usize,
    pub(crate) engines: usize,
    pub(crate) engine_font_bytes: usize,
    pub(crate) font_bytes: usize,
    pub(crate) json_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            inputs: 64,
            input_size: 32 * 1024 * 1024,
            input_bytes: 64 * 1024 * 1024,
            results: 64,
            result_size: 32 * 1024 * 1024,
            result_bytes: 64 * 1024 * 1024,
            engines: 16,
            engine_font_bytes: 32 * 1024 * 1024,
            font_bytes: 64 * 1024 * 1024,
            json_bytes: 1024 * 1024,
        }
    }
}

struct EngineEntry {
    handle: u32,
    engine: BindingEngine,
    font_bytes: usize,
}

struct ResultEntry {
    handle: u32,
    result: OwnedResult,
}

#[derive(Default)]
pub(crate) struct Runtime {
    pub(crate) inputs: Inputs,
    addresses: Addresses,
    engines: Vec<EngineEntry>,
    results: Vec<ResultEntry>,
    result_bytes: usize,
    font_bytes: usize,
    last_handle: u32,
    pub(crate) limits: Limits,
}

impl Runtime {
    pub(crate) fn alloc(&mut self, len: u32) -> u32 {
        self.inputs.alloc(len, self.limits, &mut self.addresses)
    }

    pub(crate) fn free(&mut self, offset: u32) -> i32 {
        if self.inputs.free(offset) {
            wire::RELEASE_OK
        } else {
            wire::INVALID_ALLOCATION
        }
    }

    fn next_handle(&mut self) -> Result<u32, BindingError> {
        self.last_handle = self.last_handle.checked_add(1).ok_or_else(|| {
            BindingError::internal("WASI handle space exhausted; replace the instance")
        })?;
        Ok(self.last_handle)
    }

    fn publish(&mut self, payload: Payload) -> Result<u32, BindingError> {
        if self.results.len() >= self.limits.results {
            return Err(limit_error(
                "wasi-result-count",
                self.results.len() + 1,
                self.limits.results,
                "live result limit exhausted",
            ));
        }
        self.results
            .try_reserve(1)
            .map_err(|_| BindingError::internal("WASI result registry allocation failed"))?;
        let result = payload.into_owned(self.limits, self.result_bytes, &mut self.addresses)?;
        let handle = self.next_handle()?;
        self.result_bytes += result.bytes.len();
        self.results.push(ResultEntry { handle, result });
        Ok(handle)
    }

    fn finish(&mut self, payload: Result<Payload, BindingError>) -> u32 {
        let payload = payload.unwrap_or_else(|error| Payload::error(&error));
        match self.publish(payload) {
            Ok(handle) => handle,
            Err(error) => self.publish(Payload::error(&error)).unwrap_or(0),
        }
    }

    pub(crate) fn engine_new(
        &mut self,
        config_off: u32,
        config_len: u32,
        fonts_off: u32,
        fonts_len: u32,
    ) -> u32 {
        let prepared = (|| {
            let config = self.inputs.borrow(config_off, config_len)?;
            let fonts = self.inputs.borrow(fonts_off, fonts_len)?;
            if self.engines.len() >= self.limits.engines {
                return Err(limit_error(
                    "wasi-engine-count",
                    self.engines.len() + 1,
                    self.limits.engines,
                    "live engine limit exhausted",
                ));
            }
            fonts::create_engine(
                config,
                fonts,
                self.limits,
                self.limits.font_bytes.saturating_sub(self.font_bytes),
            )
        })();
        let (engine, font_bytes) = match prepared {
            Ok(engine) => engine,
            Err(error) => return self.finish(Err(error)),
        };
        if self.engines.try_reserve(1).is_err() {
            return self.finish(Err(BindingError::internal(
                "WASI engine registry allocation failed",
            )));
        }
        let engine_handle = match self.next_handle() {
            Ok(handle) => handle,
            Err(error) => return self.finish(Err(error)),
        };
        let payload = Payload::success(
            engine_handle,
            "application/json",
            Vec::new(),
            b"{\"version\":1,\"ok\":true}".to_vec(),
        );
        // Publish both identities or neither; reserve the engine slot before the result commit.
        match self.publish(payload) {
            Ok(result_handle) => {
                self.engines.push(EngineEntry {
                    handle: engine_handle,
                    engine,
                    font_bytes,
                });
                self.font_bytes += font_bytes;
                result_handle
            }
            Err(error) => self.finish(Err(error)),
        }
    }

    pub(crate) fn engine_close(&mut self, handle: u32) -> i32 {
        let Some(index) = self.engines.iter().position(|entry| entry.handle == handle) else {
            return wire::INVALID_HANDLE;
        };
        let entry = self.engines.swap_remove(index);
        self.font_bytes -= entry.font_bytes;
        wire::RELEASE_OK
    }

    pub(crate) fn execute(
        &mut self,
        engine: u32,
        request_off: u32,
        request_len: u32,
        source_off: u32,
        source_len: u32,
    ) -> u32 {
        let payload = (|| {
            let entry = self
                .engines
                .iter()
                .find(|entry| entry.handle == engine)
                .ok_or_else(|| {
                    BindingError::invalid_argument("unknown or closed WASI engine handle")
                })?;
            let request = self.inputs.borrow(request_off, request_len)?;
            let source = self.inputs.borrow(source_off, source_len)?;
            let request: Request<'_> = wire::parse(request, self.limits.json_bytes)?;
            wire::check_version(request.version)?;
            let options = wire::nonempty_options(request.options).unwrap_or(b"");
            if MetadataKey::from_id(&request.operation_id).is_some() {
                if !source.is_empty() || !options.is_empty() || request.uri.is_some() {
                    return Err(BindingError::invalid_argument(
                        "metadata requests accept no source, URI, or option overlay",
                    ));
                }
                let data = entry.engine.metadata_json(&request.operation_id)?;
                return Ok(Payload::success(
                    engine,
                    "application/json",
                    data,
                    Vec::new(),
                ));
            }
            let request = BindingOperationRequest::new(&request.operation_id, source)
                .with_options_json(options)
                .with_optional_uri(request.uri.as_deref().map(str::as_bytes));
            let (_, media, data, metadata) = entry.engine.execute(request)?.into_parts();
            Ok(Payload::success(
                engine,
                media,
                data,
                metadata.into_json_bytes(),
            ))
        })();
        self.finish(payload)
    }

    pub(crate) fn runtime_catalog(&mut self) -> u32 {
        let payload = crate::artifact_contract()
            .runtime_catalog_json(crate::ABI_VERSION)
            .map(|data| Payload::success(0, "application/json", data, Vec::new()));
        self.finish(payload)
    }

    pub(crate) fn result_info(&self, handle: u32) -> u32 {
        self.results
            .iter()
            .find(|entry| entry.handle == handle)
            .map_or(0, |entry| entry.result.offset)
    }

    pub(crate) fn result_free(&mut self, handle: u32) -> i32 {
        let Some(index) = self.results.iter().position(|entry| entry.handle == handle) else {
            return wire::INVALID_HANDLE;
        };
        let entry = self.results.swap_remove(index);
        self.result_bytes -= entry.result.bytes.len();
        wire::RELEASE_OK
    }

    #[cfg(test)]
    pub(crate) fn result(&self, handle: u32) -> Option<&OwnedResult> {
        self.results
            .iter()
            .find(|entry| entry.handle == handle)
            .map(|entry| &entry.result)
    }

    #[cfg(test)]
    pub(crate) fn counts(&self) -> (usize, usize, usize, usize) {
        (
            self.engines.len(),
            self.results.len(),
            self.font_bytes,
            self.result_bytes,
        )
    }

    #[cfg(test)]
    pub(crate) fn exhaust_handles(&mut self, last: u32) {
        self.last_handle = last;
    }
}

pub(crate) fn limit_error(
    id: &'static str,
    actual: usize,
    max: usize,
    message: &'static str,
) -> BindingError {
    BindingError::resource_limit(
        "admission",
        id,
        actual as u64,
        max as u64,
        "wasi-transport-v1",
        message,
    )
}
