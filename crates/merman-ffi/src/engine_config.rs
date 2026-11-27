//! Native constructor records, storage preflight, and service admission.

#[cfg(feature = "native-fonts")]
use merman::OperationControl;
#[cfg(feature = "native-fonts")]
use merman::svg::export::{NativeFontError, NativeFontLimits};
use merman_bindings_core::BindingEngineServices;
#[cfg(feature = "svg")]
use merman_bindings_core::{
    BindingError, IconPack, IconRegistryResourceLimitId, build_icon_registry,
};
use std::mem::size_of;
use std::ptr;

#[cfg(feature = "native-fonts")]
use super::records::native_struct_size;
#[cfg(feature = "svg")]
use super::records::validate_record_array_range;
use super::records::{
    native_slice_bytes, read_engine_config, read_engine_services_config, read_record_struct_size,
    validate_declared_record_array_disjoint, validate_disjoint_storage,
    validate_native_slice_shape, validate_pointer_alignment, validate_struct_size,
};
use super::result::{result_is_writable, result_status_boundary, write_native_failure};
use super::{
    MERMAN_NATIVE_OPERATION_NONE, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT, MermanNativeEngineConfig,
    MermanNativeEngineServicesConfig, MermanNativeEngineToken, MermanNativeFontEngineConfig,
    MermanNativeResult, MermanNativeStatus, NativeFailure, create_native_engine_state,
    publish_native_engine_result,
};
#[cfg(feature = "native-fonts")]
use super::{MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED, MermanNativeSlice};
#[cfg(feature = "svg")]
use super::{MermanNativeIconPack, native_failure_from_binding};

#[cfg(feature = "svg")]
pub(super) const NATIVE_ICON_PACK_RECORD_LIMIT: usize = 16;

#[cfg(feature = "svg")]
const _: () = assert!(
    NATIVE_ICON_PACK_RECORD_LIMIT == IconRegistryResourceLimitId::MaxPacks.fixed_value() as usize
);

struct DeferredNativeFailure {
    failure: NativeFailure,
    #[cfg(feature = "svg")]
    result_write_is_safe: bool,
}

fn defer_first_failure(slot: &mut Option<DeferredNativeFailure>, failure: NativeFailure) {
    if slot.is_none() {
        *slot = Some(DeferredNativeFailure {
            failure,
            #[cfg(feature = "svg")]
            result_write_is_safe: true,
        });
    }
}

#[cfg(feature = "svg")]
fn defer_first_status_only_failure(
    slot: &mut Option<DeferredNativeFailure>,
    failure: NativeFailure,
) {
    if let Some(deferred) = slot {
        deferred.result_write_is_safe = false;
    } else {
        *slot = Some(DeferredNativeFailure {
            failure,
            result_write_is_safe: false,
        });
    }
}

pub(super) unsafe extern "C" fn native_engine_new(
    config: *const MermanNativeEngineConfig,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    unsafe {
        result_status_boundary(out_result, MERMAN_NATIVE_OPERATION_NONE, || {
            engine_new_impl(config, out_engine, out_result)
        })
    }
}

unsafe fn engine_new_impl(
    config: *const MermanNativeEngineConfig,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    if let Err(failure) = unsafe { result_is_writable(out_result) } {
        return failure.status;
    }
    let config_ptr = config;
    let out_engine_storage_len = if out_engine.is_null() {
        0
    } else {
        size_of::<MermanNativeEngineToken>()
    };
    let mut early_output_failure = out_engine.is_null().then(|| {
        NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_engine must not be null",
        )
    });
    let fixed_storage_validation = (|| {
        validate_disjoint_storage(
            out_engine.cast::<u8>(),
            out_engine_storage_len,
            "out_engine",
            out_result.cast::<u8>(),
            size_of::<MermanNativeResult>(),
            "out_result",
        )?;
        validate_disjoint_storage(
            config_ptr.cast::<u8>(),
            size_of::<MermanNativeEngineConfig>(),
            "config",
            out_engine.cast::<u8>(),
            out_engine_storage_len,
            "out_engine",
        )?;
        validate_disjoint_storage(
            config_ptr.cast::<u8>(),
            size_of::<MermanNativeEngineConfig>(),
            "config",
            out_result.cast::<u8>(),
            size_of::<MermanNativeResult>(),
            "out_result",
        )
    })();
    if let Err(failure) = fixed_storage_validation {
        return failure.status;
    }
    if early_output_failure.is_none()
        && let Err(failure) = validate_pointer_alignment(out_engine, "out_engine")
    {
        early_output_failure = Some(failure);
    }
    let config = match unsafe { read_engine_config(config_ptr) } {
        Ok(config) => config,
        Err(failure) => {
            if early_output_failure.is_some() {
                return failure.status;
            }
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
            };
        }
    };
    let storage_validation = (|| {
        validate_disjoint_storage(
            config_ptr.cast::<u8>(),
            size_of::<MermanNativeEngineConfig>(),
            "config",
            config.options_json.data,
            config.options_json.len,
            "config.options_json",
        )?;
        validate_disjoint_storage(
            config.options_json.data,
            config.options_json.len,
            "config.options_json",
            out_engine.cast::<u8>(),
            out_engine_storage_len,
            "out_engine",
        )?;
        validate_disjoint_storage(
            config.options_json.data,
            config.options_json.len,
            "config.options_json",
            out_result.cast::<u8>(),
            size_of::<MermanNativeResult>(),
            "out_result",
        )
    })();
    if let Err(failure) = storage_validation {
        return failure.status;
    }
    if let Some(failure) = early_output_failure.take() {
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    if let Err(failure) = validate_native_slice_shape(config.options_json, "config.options_json") {
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    if unsafe { ptr::read(out_engine) } != 0 {
        let failure = NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_engine must be initialized to zero",
        );
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    let outcome = (|| {
        let options_json =
            unsafe { native_slice_bytes(config.options_json, "config.options_json") }?;
        let state = create_native_engine_state(config, options_json, BindingEngineServices::new())?;
        unsafe { publish_native_engine_result(state, out_engine, out_result, "engine-new") }
    })();

    match outcome {
        Ok(status) => status,
        Err(failure) => unsafe {
            write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
        },
    }
}

pub(super) unsafe extern "C" fn native_engine_new_with_services(
    config: *const MermanNativeEngineServicesConfig,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    unsafe {
        result_status_boundary(out_result, MERMAN_NATIVE_OPERATION_NONE, || {
            engine_new_with_services_impl(config, out_engine, out_result)
        })
    }
}

pub(super) unsafe extern "C" fn native_engine_new_with_fonts(
    config: *const MermanNativeFontEngineConfig,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    unsafe {
        result_status_boundary(out_result, MERMAN_NATIVE_OPERATION_NONE, || {
            engine_new_with_fonts_impl(config, out_engine, out_result)
        })
    }
}

unsafe fn engine_new_with_fonts_impl(
    config_ptr: *const MermanNativeFontEngineConfig,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    if let Err(failure) = unsafe { result_is_writable(out_result) } {
        return failure.status;
    }
    let out_engine_len = if out_engine.is_null() {
        0
    } else {
        size_of::<MermanNativeEngineToken>()
    };
    let outputs = [
        (
            out_engine.cast::<u8>().cast_const(),
            out_engine_len,
            "out_engine",
        ),
        (
            out_result.cast::<u8>().cast_const(),
            size_of::<MermanNativeResult>(),
            "out_result",
        ),
    ];
    let fixed_storage_validation = (|| {
        validate_disjoint_storage(
            outputs[0].0,
            outputs[0].1,
            outputs[0].2,
            outputs[1].0,
            outputs[1].1,
            outputs[1].2,
        )?;
        for (output, len, name) in outputs {
            validate_disjoint_storage(
                config_ptr.cast(),
                size_of::<MermanNativeFontEngineConfig>(),
                "config",
                output,
                len,
                name,
            )?;
        }
        if config_ptr.is_null() {
            return Err(NativeFailure::new(
                MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
                "config must not be null",
            ));
        }
        validate_pointer_alignment(config_ptr, "config")?;
        validate_struct_size::<MermanNativeFontEngineConfig>(
            unsafe { read_record_struct_size(config_ptr) },
            "config",
        )
    })();
    if let Err(failure) = fixed_storage_validation {
        return failure.status;
    }
    let config = unsafe { ptr::read(config_ptr) };
    let input_storage_validation = (|| {
        validate_disjoint_storage(
            config_ptr.cast(),
            size_of::<MermanNativeFontEngineConfig>(),
            "config",
            config.options_json.data,
            config.options_json.len,
            "config.options_json",
        )?;
        for (output, len, name) in outputs {
            validate_disjoint_storage(
                config.options_json.data,
                config.options_json.len,
                "config.options_json",
                output,
                len,
                name,
            )?;
            validate_declared_record_array_disjoint(
                config.fonts,
                config.font_count,
                "config.fonts",
                output,
                len,
                name,
            )?;
        }
        validate_declared_record_array_disjoint(
            config.fonts,
            config.font_count,
            "config.fonts",
            config_ptr.cast(),
            size_of::<MermanNativeFontEngineConfig>(),
            "config",
        )?;
        validate_declared_record_array_disjoint(
            config.fonts,
            config.font_count,
            "config.fonts",
            config.options_json.data,
            config.options_json.len,
            "config.options_json",
        )
    })();
    if let Err(failure) = input_storage_validation {
        return failure.status;
    }

    let mut deferred_failure = None;
    if let Err(failure) = validate_native_slice_shape(config.options_json, "config.options_json") {
        defer_first_failure(&mut deferred_failure, failure);
    }
    if config.use_bundled_fonts > 1 {
        defer_first_failure(
            &mut deferred_failure,
            NativeFailure::new(
                MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
                "config.use_bundled_fonts must be zero or one",
            ),
        );
    }

    #[cfg(feature = "native-fonts")]
    let fonts = {
        let limits = NativeFontLimits::default();
        let mut fonts = Vec::new();
        if config.font_count > limits.max_files {
            defer_first_failure(
                &mut deferred_failure,
                native_failure_from_font(NativeFontError::Limit {
                    resource: "files",
                    actual: config.font_count,
                    limit: limits.max_files,
                }),
            );
        } else if config.font_count != 0 && config.fonts.is_null() {
            defer_first_failure(
                &mut deferred_failure,
                NativeFailure::new(
                    MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
                    "config.fonts must not be null when font_count is non-zero",
                ),
            );
        } else {
            let array_len = match validate_record_array_range(
                config.fonts,
                config.font_count,
                "config.fonts",
            ) {
                Ok(len) => len,
                Err(failure) => return failure.status,
            };
            if config.font_count != 0
                && let Err(failure) = validate_pointer_alignment(config.fonts, "config.fonts")
            {
                return failure.status;
            }
            for index in 0..config.font_count {
                let font_ptr = unsafe { config.fonts.add(index) };
                if let Err(failure) = validate_struct_size::<MermanNativeSlice>(
                    unsafe { read_record_struct_size(font_ptr) },
                    "config.fonts[]",
                ) {
                    defer_first_status_only_failure(&mut deferred_failure, failure);
                    continue;
                }
                let font = unsafe { ptr::read(font_ptr) };
                let storage_validation: Result<(), NativeFailure> = (|| {
                    validate_disjoint_storage(
                        config_ptr.cast(),
                        size_of::<MermanNativeFontEngineConfig>(),
                        "config",
                        font.data,
                        font.len,
                        "config.fonts[].data",
                    )?;
                    validate_disjoint_storage(
                        config.fonts.cast(),
                        array_len,
                        "config.fonts",
                        font.data,
                        font.len,
                        "config.fonts[].data",
                    )?;
                    for (output, len, name) in outputs {
                        validate_disjoint_storage(
                            font.data,
                            font.len,
                            "config.fonts[].data",
                            output,
                            len,
                            name,
                        )?;
                    }
                    Ok(())
                })();
                if let Err(failure) = storage_validation {
                    return failure.status;
                }
                if let Err(failure) = validate_native_slice_shape(font, "config.fonts[]") {
                    defer_first_failure(&mut deferred_failure, failure);
                }
                fonts.push(font);
            }
        }
        if let Some(failure) = deferred_failure.as_ref()
            && !failure.result_write_is_safe
        {
            return failure.failure.status;
        }
        fonts
    };

    let output_validation = if out_engine.is_null() {
        Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_engine must not be null",
        ))
    } else {
        validate_pointer_alignment(out_engine, "out_engine").and_then(|()| {
            if unsafe { ptr::read(out_engine) } == 0 {
                Ok(())
            } else {
                Err(NativeFailure::new(
                    MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
                    "out_engine must be initialized to zero",
                ))
            }
        })
    };
    if let Err(failure) = output_validation {
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    if let Some(failure) = deferred_failure {
        return unsafe {
            write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure.failure)
        };
    }

    #[cfg(not(feature = "native-fonts"))]
    {
        let failure = NativeFailure::missing_capability(
            "native-fonts",
            "memory-font construction requires an artifact with the native-fonts capability",
        );
        unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) }
    }
    #[cfg(feature = "native-fonts")]
    {
        let outcome = (|| {
            let mut bundled = Vec::new();
            if config.use_bundled_fonts != 0 {
                #[cfg(feature = "bundled-fonts")]
                bundled.extend(merman::svg::export::bundled_font_data());
                #[cfg(not(feature = "bundled-fonts"))]
                return Err(NativeFailure::missing_capability(
                    "native-fonts",
                    "bundled fallback requires an artifact built with bundled-fonts",
                ));
            }
            let limits = NativeFontLimits::default();
            let count = fonts.len() + bundled.len();
            if count > limits.max_files {
                return Err(native_failure_from_font(NativeFontError::Limit {
                    resource: "files",
                    actual: count,
                    limit: limits.max_files,
                }));
            }
            let mut bytes = 0usize;
            for len in fonts
                .iter()
                .map(|font| font.len)
                .chain(bundled.iter().map(Vec::len))
            {
                bytes = bytes.saturating_add(len);
                if bytes > limits.max_bytes {
                    return Err(native_failure_from_font(NativeFontError::Limit {
                        resource: "font-bytes",
                        actual: bytes,
                        limit: limits.max_bytes,
                    }));
                }
            }
            let mut owned_fonts = Vec::with_capacity(count);
            for font in fonts {
                owned_fonts.push(unsafe { native_slice_bytes(font, "config.fonts[]") }?.to_vec());
            }
            owned_fonts.append(&mut bundled);
            let control = OperationControl::new();
            let services =
                BindingEngineServices::from_strict_native_font_bytes(owned_fonts, limits, &control)
                    .map_err(native_failure_from_font)?;
            let options_json =
                unsafe { native_slice_bytes(config.options_json, "config.options_json") }?;
            let engine_config = MermanNativeEngineConfig {
                struct_size: native_struct_size::<MermanNativeEngineConfig>(),
                options_json: config.options_json,
                text_measure: None,
                text_measure_user_data: ptr::null_mut(),
            };
            let state = create_native_engine_state(engine_config, options_json, services)?;
            unsafe {
                publish_native_engine_result(state, out_engine, out_result, "engine-new-with-fonts")
            }
        })();
        match outcome {
            Ok(status) => status,
            Err(failure) => unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
            },
        }
    }
}

#[cfg(feature = "native-fonts")]
fn native_failure_from_font(error: NativeFontError) -> NativeFailure {
    let status = if matches!(error, NativeFontError::Limit { .. }) {
        MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED
    } else {
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    };
    NativeFailure::new(status, error.to_string())
}

unsafe fn engine_new_with_services_impl(
    config: *const MermanNativeEngineServicesConfig,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    let config_ptr = config;
    if let Err(failure) = unsafe { result_is_writable(out_result) } {
        return failure.status;
    }
    let out_engine_storage_len = if out_engine.is_null() {
        0
    } else {
        size_of::<MermanNativeEngineToken>()
    };
    let mut early_output_failure = out_engine.is_null().then(|| {
        NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_engine must not be null",
        )
    });

    let fixed_storage_validation = (|| {
        validate_disjoint_storage(
            out_engine.cast::<u8>(),
            out_engine_storage_len,
            "out_engine",
            out_result.cast::<u8>(),
            size_of::<MermanNativeResult>(),
            "out_result",
        )?;
        validate_disjoint_storage(
            config.cast::<u8>(),
            size_of::<MermanNativeEngineServicesConfig>(),
            "config",
            out_engine.cast::<u8>(),
            out_engine_storage_len,
            "out_engine",
        )?;
        validate_disjoint_storage(
            config.cast::<u8>(),
            size_of::<MermanNativeEngineServicesConfig>(),
            "config",
            out_result.cast::<u8>(),
            size_of::<MermanNativeResult>(),
            "out_result",
        )
    })();
    if let Err(failure) = fixed_storage_validation {
        return failure.status;
    }
    if early_output_failure.is_none()
        && let Err(failure) = validate_pointer_alignment(out_engine, "out_engine")
    {
        early_output_failure = Some(failure);
    }

    let config = match unsafe { read_engine_services_config(config_ptr) } {
        Ok(config) => config,
        Err(failure) => return failure.status,
    };
    let engine_config = config.engine_config;

    let options_storage_validation = (|| {
        validate_disjoint_storage(
            config_ptr.cast::<u8>(),
            size_of::<MermanNativeEngineServicesConfig>(),
            "config",
            engine_config.options_json.data,
            engine_config.options_json.len,
            "config.engine_config.options_json",
        )?;
        validate_disjoint_storage(
            engine_config.options_json.data,
            engine_config.options_json.len,
            "config.engine_config.options_json",
            out_engine.cast::<u8>(),
            out_engine_storage_len,
            "out_engine",
        )?;
        validate_disjoint_storage(
            engine_config.options_json.data,
            engine_config.options_json.len,
            "config.engine_config.options_json",
            out_result.cast::<u8>(),
            size_of::<MermanNativeResult>(),
            "out_result",
        )
    })();
    if let Err(failure) = options_storage_validation {
        return failure.status;
    }
    let mut deferred_failure = None;
    if let Err(failure) = validate_struct_size::<MermanNativeEngineConfig>(
        engine_config.struct_size,
        "config.engine_config",
    ) {
        defer_first_failure(&mut deferred_failure, failure);
    }
    if let Err(failure) = validate_native_slice_shape(
        engine_config.options_json,
        "config.engine_config.options_json",
    ) {
        defer_first_failure(&mut deferred_failure, failure);
    }

    let declared_array_storage_validation = (|| {
        validate_declared_record_array_disjoint(
            config.icon_packs,
            config.icon_pack_count,
            "config.icon_packs",
            config_ptr.cast::<u8>(),
            size_of::<MermanNativeEngineServicesConfig>(),
            "config",
        )?;
        validate_declared_record_array_disjoint(
            config.icon_packs,
            config.icon_pack_count,
            "config.icon_packs",
            engine_config.options_json.data,
            engine_config.options_json.len,
            "config.engine_config.options_json",
        )?;
        validate_declared_record_array_disjoint(
            config.icon_packs,
            config.icon_pack_count,
            "config.icon_packs",
            out_engine.cast::<u8>(),
            out_engine_storage_len,
            "out_engine",
        )?;
        validate_declared_record_array_disjoint(
            config.icon_packs,
            config.icon_pack_count,
            "config.icon_packs",
            out_result.cast::<u8>(),
            size_of::<MermanNativeResult>(),
            "out_result",
        )
    })();
    if let Err(failure) = declared_array_storage_validation {
        return failure.status;
    }

    #[cfg(not(feature = "svg"))]
    {
        if let Some(failure) = early_output_failure.take() {
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
            };
        }
        if let Some(failure) = deferred_failure.take() {
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure.failure)
            };
        }
        if config.icon_pack_count != 0 {
            let failure = NativeFailure::missing_capability(
                "svg",
                "icon registry construction requires an artifact with the svg capability",
            );
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
            };
        }
    }

    #[cfg(feature = "svg")]
    let icon_packs = {
        if config.icon_pack_count > NATIVE_ICON_PACK_RECORD_LIMIT {
            defer_first_failure(
                &mut deferred_failure,
                native_icon_pack_count_limit_failure(config.icon_pack_count),
            );
        }

        let mut native_packs = Vec::new();
        if config.icon_pack_count <= NATIVE_ICON_PACK_RECORD_LIMIT {
            let icon_pack_byte_len = match validate_record_array_range(
                config.icon_packs,
                config.icon_pack_count,
                "config.icon_packs",
            ) {
                Ok(byte_len) => byte_len,
                Err(failure) => return failure.status,
            };
            let icon_pack_array = config.icon_packs.cast::<u8>();
            if config.icon_pack_count != 0
                && let Err(failure) =
                    validate_pointer_alignment(config.icon_packs, "config.icon_packs")
            {
                return failure.status;
            }

            native_packs.reserve(config.icon_pack_count);
            for index in 0..config.icon_pack_count {
                let pack_ptr = unsafe { config.icon_packs.add(index) };
                match validate_struct_size::<MermanNativeIconPack>(
                    unsafe { read_record_struct_size(pack_ptr) },
                    &format!("config.icon_packs[{index}]"),
                ) {
                    Ok(()) => native_packs.push(Some(unsafe { ptr::read(pack_ptr) })),
                    Err(failure) => {
                        defer_first_status_only_failure(&mut deferred_failure, failure);
                        native_packs.push(None);
                    }
                }
            }

            for (index, pack) in native_packs.iter().enumerate() {
                let Some(pack) = pack else {
                    continue;
                };
                for (slice, name) in [
                    (pack.json, format!("config.icon_packs[{index}].json")),
                    (
                        pack.registration_name,
                        format!("config.icon_packs[{index}].registration_name"),
                    ),
                ] {
                    let slice_storage_validation = (|| {
                        validate_disjoint_storage(
                            config_ptr.cast::<u8>(),
                            size_of::<MermanNativeEngineServicesConfig>(),
                            "config",
                            slice.data,
                            slice.len,
                            &name,
                        )?;
                        validate_disjoint_storage(
                            icon_pack_array,
                            icon_pack_byte_len,
                            "config.icon_packs",
                            slice.data,
                            slice.len,
                            &name,
                        )?;
                        validate_disjoint_storage(
                            slice.data,
                            slice.len,
                            &name,
                            out_engine.cast::<u8>(),
                            out_engine_storage_len,
                            "out_engine",
                        )?;
                        validate_disjoint_storage(
                            slice.data,
                            slice.len,
                            &name,
                            out_result.cast::<u8>(),
                            size_of::<MermanNativeResult>(),
                            "out_result",
                        )
                    })();
                    if let Err(failure) = slice_storage_validation {
                        return failure.status;
                    }
                    if let Err(failure) = validate_native_slice_shape(slice, &name) {
                        defer_first_failure(&mut deferred_failure, failure);
                    }
                }
            }
        }

        if let Some(failure) = deferred_failure.as_ref()
            && !failure.result_write_is_safe
        {
            return failure.failure.status;
        }
        if let Some(failure) = early_output_failure.take() {
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
            };
        }
        if let Some(failure) = deferred_failure.take() {
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure.failure)
            };
        }

        // Keep the safety checks above authoritative, then complete the whole length-only resource
        // preflight before constructing borrowed byte slices or decoding registration-name UTF-8.
        if let Err(failure) = preflight_native_icon_pack_resource_limits(&native_packs) {
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
            };
        }

        let mut packs = Vec::with_capacity(native_packs.len());
        for (index, pack) in native_packs.into_iter().enumerate() {
            let pack = pack.expect("validated native icon-pack records are complete");
            let json = match unsafe {
                native_slice_bytes(pack.json, &format!("config.icon_packs[{index}].json"))
            } {
                Ok(json) => json,
                Err(failure) => {
                    return unsafe {
                        write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
                    };
                }
            };
            let registration_name_bytes = match unsafe {
                native_slice_bytes(
                    pack.registration_name,
                    &format!("config.icon_packs[{index}].registration_name"),
                )
            } {
                Ok(bytes) => bytes,
                Err(failure) => {
                    return unsafe {
                        write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
                    };
                }
            };
            let registration_name = if registration_name_bytes.is_empty() {
                None
            } else {
                match std::str::from_utf8(registration_name_bytes) {
                    Ok(name) => Some(name),
                    Err(error) => {
                        let failure = native_failure_from_binding(
                            BindingError::icon_registry_invalid_utf8(
                                index,
                                format!(
                                    "config.icon_packs[{index}].registration_name must be valid UTF-8: {error}"
                                ),
                            ),
                        );
                        return unsafe {
                            write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
                        };
                    }
                }
            };
            let pack = match registration_name {
                Some(name) => IconPack::new(json).with_registration_name(name),
                None => IconPack::new(json),
            };
            packs.push(pack);
        }
        packs
    };

    if unsafe { ptr::read(out_engine) } != 0 {
        let failure = NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_engine must be initialized to zero",
        );
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    if engine_config.text_measure.is_none() && !engine_config.text_measure_user_data.is_null() {
        let failure = NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "config.engine_config.text_measure_user_data must be null when text_measure is null",
        );
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }

    let outcome = (|| {
        let options_json = unsafe {
            native_slice_bytes(
                engine_config.options_json,
                "config.engine_config.options_json",
            )
        }?;
        let services = BindingEngineServices::new();
        #[cfg(feature = "svg")]
        let services = if icon_packs.is_empty() {
            services
        } else {
            let icon_registry =
                build_icon_registry(icon_packs).map_err(native_failure_from_binding)?;
            services.with_icon_registry(icon_registry)
        };
        let state = create_native_engine_state(engine_config, options_json, services)?;
        unsafe {
            publish_native_engine_result(state, out_engine, out_result, "engine-new-with-services")
        }
    })();

    match outcome {
        Ok(status) => status,
        Err(failure) => unsafe {
            write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
        },
    }
}

#[cfg(feature = "svg")]
pub(super) fn preflight_native_icon_pack_resource_limits(
    native_packs: &[Option<MermanNativeIconPack>],
) -> Result<(), NativeFailure> {
    use IconRegistryResourceLimitId::{MaxInputBytes, MaxPackBytes, MaxPrefixBytes};

    let mut input_bytes = 0usize;
    for (index, pack) in native_packs.iter().enumerate() {
        let pack = pack
            .as_ref()
            .expect("validated native icon-pack records are complete");

        let pack_bytes = u64::try_from(pack.json.len).unwrap_or(u64::MAX);
        if pack_bytes > MaxPackBytes.fixed_value() {
            return Err(native_failure_from_binding(
                BindingError::icon_registry_resource_limit(
                    MaxPackBytes,
                    pack_bytes,
                    Some(index),
                    "icon pack bytes exceed the fixed per-pack ceiling",
                ),
            ));
        }

        let next_input_bytes = input_bytes.checked_add(pack.json.len).ok_or_else(|| {
            native_failure_from_binding(BindingError::icon_registry_resource_limit(
                MaxInputBytes,
                u64::MAX,
                Some(index),
                "aggregate icon pack byte accounting overflowed",
            ))
        })?;
        let next_input_bytes_u64 = u64::try_from(next_input_bytes).unwrap_or(u64::MAX);
        if next_input_bytes_u64 > MaxInputBytes.fixed_value() {
            return Err(native_failure_from_binding(
                BindingError::icon_registry_resource_limit(
                    MaxInputBytes,
                    next_input_bytes_u64,
                    Some(index),
                    "aggregate icon pack bytes exceed the fixed registry ceiling",
                ),
            ));
        }

        let prefix_bytes = u64::try_from(pack.registration_name.len).unwrap_or(u64::MAX);
        if prefix_bytes > MaxPrefixBytes.fixed_value() {
            return Err(native_failure_from_binding(
                BindingError::icon_registry_resource_limit(
                    MaxPrefixBytes,
                    prefix_bytes,
                    Some(index),
                    "icon registry registration name exceeds the fixed byte ceiling",
                ),
            ));
        }

        input_bytes = next_input_bytes;
    }

    Ok(())
}

#[cfg(feature = "svg")]
fn native_icon_pack_count_limit_failure(actual: usize) -> NativeFailure {
    let limit = IconRegistryResourceLimitId::MaxPacks;
    native_failure_from_binding(BindingError::icon_registry_resource_limit(
        limit,
        u64::try_from(actual).unwrap_or(u64::MAX),
        None,
        format!(
            "icon registry pack count {actual} exceeds the fixed limit {}",
            limit.fixed_value()
        ),
    ))
}
