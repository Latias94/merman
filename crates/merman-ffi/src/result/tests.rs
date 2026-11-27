use super::{
    NativeResultRegistry, encode_result_token, initialized_native_result as native_result,
    write_native_result_with_registry,
};
use crate::tests::test_engine_state;
use crate::{
    MERMAN_NATIVE_ENGINE_TOKEN_DOMAIN_TAG, MERMAN_NATIVE_OPERATION_NONE,
    MERMAN_NATIVE_RESULT_TOKEN_DOMAIN_TAG, MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
    MERMAN_NATIVE_STATUS_OK, MERMAN_NATIVE_TOKEN_COUNTER_MAX, MERMAN_NATIVE_TOKEN_COUNTER_SHIFT,
    MERMAN_NATIVE_TOKEN_DOMAIN_MASK, MermanNativeEngineToken, MermanNativeResult,
    NativeEngineEntry, NativeEngineRegistry, PendingNativeEngine, encode_engine_token,
    native_struct_size,
};
use std::sync::Mutex;

#[test]
fn allocation_token_exhaustion_is_checked_without_reuse() {
    let registry = Mutex::new(NativeResultRegistry::new(1));
    let mut result = native_result();
    assert_eq!(
        unsafe {
            write_native_result_with_registry(
                &registry,
                &mut result,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_NONE,
                None,
                Vec::new(),
                Vec::new(),
            )
        },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(result.allocation_token, encode_result_token(1));
    let retired = registry.lock().unwrap().remove(1);
    assert!(retired.is_some());
    drop(retired);

    result = native_result();
    let status = unsafe {
        write_native_result_with_registry(
            &registry,
            &mut result,
            MERMAN_NATIVE_STATUS_OK,
            MERMAN_NATIVE_OPERATION_NONE,
            None,
            b"rejected data".to_vec(),
            b"rejected metadata".to_vec(),
        )
    };

    assert_eq!(status, MERMAN_NATIVE_STATUS_INTERNAL_ERROR);
    let registry = registry.lock().unwrap();
    assert!(registry.was_issued(1));
    assert!(registry.get(1).is_none());
    assert!(!registry.was_issued(2));
    assert_eq!(
        result.struct_size,
        native_struct_size::<MermanNativeResult>()
    );
    assert_eq!(result.allocation_token, 0);
    assert_eq!(result.status, 0);
    assert_eq!(result.operation, 0);
    assert_eq!(result.media_type.struct_size, 0);
    assert!(result.media_type.data.is_null());
    assert_eq!(result.media_type.len, 0);
    assert_eq!(result.data.struct_size, 0);
    assert!(result.data.data.is_null());
    assert_eq!(result.data.len, 0);
    assert_eq!(result.metadata_or_error_json.struct_size, 0);
    assert!(result.metadata_or_error_json.data.is_null());
    assert_eq!(result.metadata_or_error_json.len, 0);
}

fn publish_test_engine(registry: &Mutex<NativeEngineRegistry>) -> MermanNativeEngineToken {
    let Ok(pending) = PendingNativeEngine::reserve(registry, test_engine_state()) else {
        panic!("test engine reservation failed");
    };
    let mut token = 0;
    unsafe { pending.commit(&mut token) };
    token
}

#[test]
fn engine_and_result_tokens_are_disjoint_positive_signed_64_domains() {
    let engines = Mutex::new(NativeEngineRegistry::new(1));
    let results = Mutex::new(NativeResultRegistry::new(1));

    let engine = publish_test_engine(&engines);
    let mut record = native_result();
    assert_eq!(
        unsafe {
            write_native_result_with_registry(
                &results,
                &mut record,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_NONE,
                None,
                Vec::new(),
                Vec::new(),
            )
        },
        MERMAN_NATIVE_STATUS_OK
    );
    let result = record.allocation_token;
    assert_ne!(engine, result);
    assert_eq!(
        engine >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT,
        result >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT,
        "the same counter value must still produce distinct token domains"
    );
    assert_eq!(
        engine & MERMAN_NATIVE_TOKEN_DOMAIN_MASK,
        MERMAN_NATIVE_ENGINE_TOKEN_DOMAIN_TAG
    );
    assert_eq!(
        result & MERMAN_NATIVE_TOKEN_DOMAIN_MASK,
        MERMAN_NATIVE_RESULT_TOKEN_DOMAIN_TAG
    );
    assert!(i64::try_from(engine).is_ok());
    assert!(i64::try_from(result).is_ok());

    let signed_max = i64::MAX as u64;
    assert_eq!(
        MERMAN_NATIVE_TOKEN_COUNTER_MAX,
        signed_max >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT
    );
    let last_counter_bits = signed_max & !MERMAN_NATIVE_TOKEN_DOMAIN_MASK;
    let final_engine = encode_engine_token(MERMAN_NATIVE_TOKEN_COUNTER_MAX);
    assert_eq!(
        final_engine,
        last_counter_bits | MERMAN_NATIVE_ENGINE_TOKEN_DOMAIN_TAG
    );
    let final_result = encode_result_token(MERMAN_NATIVE_TOKEN_COUNTER_MAX);
    assert_eq!(
        final_result,
        last_counter_bits | MERMAN_NATIVE_RESULT_TOKEN_DOMAIN_TAG
    );
}

#[test]
fn result_token_exhaustion_does_not_retire_or_poison_an_engine() {
    let mut engines = Mutex::new(NativeEngineRegistry::new(1));
    let token = publish_test_engine(&engines);
    let counter = token >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT;
    let engines = engines.get_mut().unwrap();

    let results = Mutex::new(NativeResultRegistry::new(0));
    let mut result = native_result();
    assert_eq!(
        unsafe {
            write_native_result_with_registry(
                &results,
                &mut result,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_NONE,
                None,
                Vec::new(),
                Vec::new(),
            )
        },
        MERMAN_NATIVE_STATUS_INTERNAL_ERROR
    );
    assert!(!results.lock().unwrap().was_issued(1));
    assert_eq!(result.allocation_token, 0);

    let acquired = engines
        .get(counter)
        .and_then(NativeEngineEntry::published_state)
        .expect("engine remains published");
    let operation = acquired
        .admission
        .enter_operation()
        .expect("engine remains usable");
    drop(operation);
    assert!(engines.remove(counter).is_some());
}
