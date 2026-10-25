use std::cell::RefCell;

use crate::runtime::Runtime;
use crate::wire;

thread_local! {
    static RUNTIME: RefCell<Runtime> = RefCell::new(Runtime::default());
}

pub(crate) fn with_runtime<T>(busy: T, operation: impl FnOnce(&mut Runtime) -> T) -> T {
    RUNTIME.with(|runtime| match runtime.try_borrow_mut() {
        Ok(mut runtime) => operation(&mut runtime),
        Err(_) => busy,
    })
}

#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
#[allow(unsafe_code)]
mod exports {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    unsafe extern "C" {
        fn __wasm_call_ctors();
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn _initialize() {
        static INITIALIZED: AtomicBool = AtomicBool::new(false);
        if INITIALIZED.swap(true, Ordering::AcqRel) {
            std::process::abort();
        }
        // An explicit constructor call selects reactor linking instead of per-export ctors/dtors.
        // SAFETY: the host calls this once before any SDK operation, as required by WASI reactors.
        unsafe { __wasm_call_ctors() };
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn merman_wasi_abi_version() -> u32 {
        with_runtime(0, |_| crate::ABI_VERSION)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn merman_wasi_alloc(len: u32) -> u32 {
        with_runtime(0, |runtime| runtime.alloc(len))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn merman_wasi_free(offset: u32) -> i32 {
        with_runtime(wire::BUSY, |runtime| runtime.free(offset))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn merman_wasi_engine_new(
        config_off: u32,
        config_len: u32,
        fonts_off: u32,
        fonts_len: u32,
    ) -> u32 {
        with_runtime(0, |runtime| {
            runtime.engine_new(config_off, config_len, fonts_off, fonts_len)
        })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn merman_wasi_engine_close(engine: u32) -> i32 {
        with_runtime(wire::BUSY, |runtime| runtime.engine_close(engine))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn merman_wasi_execute(
        engine: u32,
        request_off: u32,
        request_len: u32,
        source_off: u32,
        source_len: u32,
    ) -> u32 {
        with_runtime(0, |runtime| {
            runtime.execute(engine, request_off, request_len, source_off, source_len)
        })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn merman_wasi_runtime_catalog() -> u32 {
        with_runtime(0, Runtime::runtime_catalog)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn merman_wasi_result_info(result: u32) -> u32 {
        with_runtime(0, |runtime| runtime.result_info(result))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn merman_wasi_result_free(result: u32) -> i32 {
        with_runtime(wire::BUSY, |runtime| runtime.result_free(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_instance_reentry_is_rejected_without_mutation() {
        with_runtime((), |runtime| {
            let before = runtime.counts();
            assert_eq!(with_runtime(0, |runtime| runtime.alloc(1)), 0);
            assert_eq!(
                with_runtime(wire::BUSY, |runtime| runtime.free(1)),
                wire::BUSY
            );
            assert_eq!(runtime.counts(), before);
        });
        let result = with_runtime(0, Runtime::runtime_catalog);
        assert_ne!(result, 0);
        assert_eq!(
            with_runtime(wire::BUSY, |runtime| runtime.result_free(result)),
            0
        );
    }
}
