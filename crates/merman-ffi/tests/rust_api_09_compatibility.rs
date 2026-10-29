//! External Rust consumers of the published 0.9.2 native table and its additive font table.

use merman_ffi::{
    MERMAN_NATIVE_ABI_FULL_DESCRIPTOR_DIGEST, MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST,
    MERMAN_NATIVE_ABI_VERSION, MERMAN_NATIVE_API_ENGINE_NEW_WITH_FONTS_PREFIX_SIZE,
    MERMAN_NATIVE_API_EXECUTE_COLLECT_CONTROLLED_PREFIX_SIZE, MERMAN_NATIVE_STATUS_OK,
    MermanNativeApi, MermanNativeApiRequest, MermanNativeApiWithFonts, MermanNativeSlice,
    merman_get_native_api,
};
use std::mem::{offset_of, size_of};

fn slice(bytes: &[u8]) -> MermanNativeSlice {
    MermanNativeSlice {
        struct_size: size_of::<MermanNativeSlice>() as u32,
        data: bytes.as_ptr(),
        len: bytes.len(),
    }
}

fn discovery_request() -> MermanNativeApiRequest {
    MermanNativeApiRequest {
        struct_size: size_of::<MermanNativeApiRequest>() as u32,
        expected_abi_version: MERMAN_NATIVE_ABI_VERSION,
        expected_minimum_prefix_layout_digest: slice(
            MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes(),
        ),
    }
}

#[test]
fn published_struct_literal_and_discovery_preserve_the_old_prefix() {
    // No struct update syntax: adding a public field must break this downstream consumer.
    let api = MermanNativeApi {
        struct_size: size_of::<MermanNativeApi>() as u32,
        abi_version: 0,
        minimum_prefix_layout_digest: slice(b""),
        full_descriptor_digest: slice(b""),
        capability_catalog_digest: slice(b""),
        package_version: slice(b""),
        runtime_catalog: None,
        engine_new: None,
        engine_try_close: None,
        execute_collect: None,
        result_free: None,
        metadata_collect: None,
        engine_new_with_services: None,
        operation_control_new: None,
        operation_control_cancel: None,
        operation_control_release: None,
        execute_collect_controlled: None,
    };
    #[repr(C)]
    struct OldConsumerBuffer {
        api: MermanNativeApi,
        suffix: [u8; 32],
    }
    let mut buffer = OldConsumerBuffer {
        api,
        suffix: [0xa5; 32],
    };
    let request = discovery_request();

    // SAFETY: Both records are initialized and correctly aligned. The advertised output
    // capacity is exactly the old table; request slices borrow static bytes for the call.
    assert_eq!(
        unsafe { merman_get_native_api(&request, &mut buffer.api) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(buffer.api.struct_size, size_of::<MermanNativeApi>() as u32);
    assert_eq!(
        buffer.api.struct_size,
        MERMAN_NATIVE_API_EXECUTE_COLLECT_CONTROLLED_PREFIX_SIZE
    );
    assert_eq!(buffer.api.abi_version, MERMAN_NATIVE_ABI_VERSION);
    assert!(buffer.api.runtime_catalog.is_some());
    assert!(buffer.api.engine_new.is_some());
    assert!(buffer.api.engine_try_close.is_some());
    assert!(buffer.api.execute_collect.is_some());
    assert!(buffer.api.result_free.is_some());
    assert!(buffer.api.metadata_collect.is_some());
    assert!(buffer.api.engine_new_with_services.is_some());
    assert!(buffer.api.operation_control_new.is_some());
    assert!(buffer.api.operation_control_cancel.is_some());
    assert!(buffer.api.operation_control_release.is_some());
    assert!(buffer.api.execute_collect_controlled.is_some());
    assert_eq!(
        buffer.suffix, [0xa5; 32],
        "discovery must honor old capacity"
    );

    // SAFETY: Successful discovery supplies readable static digest slices.
    let prefix_digest = unsafe {
        std::slice::from_raw_parts(
            buffer.api.minimum_prefix_layout_digest.data,
            buffer.api.minimum_prefix_layout_digest.len,
        )
    };
    assert_eq!(
        prefix_digest,
        MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes()
    );
    // SAFETY: Successful discovery supplies readable static digest slices.
    let full_digest = unsafe {
        std::slice::from_raw_parts(
            buffer.api.full_descriptor_digest.data,
            buffer.api.full_descriptor_digest.len,
        )
    };
    assert_eq!(
        full_digest,
        MERMAN_NATIVE_ABI_FULL_DESCRIPTOR_DIGEST.as_bytes()
    );
}

#[test]
fn full_table_constructor_discovers_the_appended_font_function() {
    let mut api = MermanNativeApiWithFonts::for_discovery();
    let request = discovery_request();
    assert_eq!(
        api.struct_size,
        size_of::<MermanNativeApiWithFonts>() as u32
    );
    assert_eq!(
        offset_of!(MermanNativeApiWithFonts, runtime_catalog),
        offset_of!(MermanNativeApi, runtime_catalog)
    );
    assert_eq!(
        offset_of!(MermanNativeApiWithFonts, execute_collect_controlled),
        offset_of!(MermanNativeApi, execute_collect_controlled)
    );
    assert_eq!(
        offset_of!(MermanNativeApiWithFonts, engine_new_with_fonts),
        size_of::<MermanNativeApi>()
    );
    let out_api: *mut MermanNativeApi = api.as_mut_api_ptr();

    // SAFETY: The constructor advertises the capacity of the complete initialized table,
    // and the pointer refers to that live table for the duration of discovery.
    assert_eq!(
        unsafe { merman_get_native_api(&request, out_api) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(
        api.struct_size,
        size_of::<MermanNativeApiWithFonts>() as u32
    );
    assert_eq!(
        api.struct_size,
        MERMAN_NATIVE_API_ENGINE_NEW_WITH_FONTS_PREFIX_SIZE
    );
    assert_eq!(api.abi_version, MERMAN_NATIVE_ABI_VERSION);
    assert!(api.runtime_catalog.is_some());
    assert!(api.execute_collect_controlled.is_some());
    assert!(api.engine_new_with_fonts.is_some());
}
