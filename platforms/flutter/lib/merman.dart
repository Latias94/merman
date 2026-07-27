/// Flutter and Dart FFI bindings for the `merman` headless Mermaid engine.
///
/// This public facade owns Dart-friendly lifecycle, error, resource, and text
/// measurement APIs. Its C declarations stay private and are generated from
/// the ABI 3 header by `ffigen`.
library;

export 'src/generated/resource_options.dart'
    show
        MermanResourceLimitId,
        MermanResourceOptions,
        MermanResourceOptionsBuilder,
        MermanResourceProfile;
export 'src/generated/text_measurement_protocol.dart'
    show
        MermanTextDirection,
        MermanTextMeasurementOperation,
        MermanTextMeasurementPhase,
        MermanTextMeasurementResultKind,
        MermanTextWhiteSpace,
        MermanTextWrapMode;
export 'src/merman_ffi.dart'
    show
        Merman,
        MermanErrorKind,
        MermanException,
        MermanMissingCapabilityException,
        MermanOperation,
        MermanOperationResult,
        MermanReusableEngine,
        MermanRuntimeCatalog,
        MermanTextMeasureRequest,
        MermanTextMeasureResult,
        MermanTextMeasurer,
        MermanUnknownOperationException,
        MermanUnsupportedOperationException,
        MermanValidationResult,
        openMermanLibrary,
        openMermanLibraryFromPath;
