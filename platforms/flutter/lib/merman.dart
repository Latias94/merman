/// Flutter and Dart FFI bindings for the `merman` headless Mermaid engine.
///
/// Import this library in Flutter apps that need to render Mermaid source to
/// SVG or ASCII text, inspect parsed diagram JSON, or query binding metadata.
library;

export 'src/generated/text_measurement_abi.dart'
    show
        MermanTextMeasurementOperation,
        MermanTextMeasurementResultKind,
        mermanAbiVersion;
export 'src/generated/resource_options.dart'
    show
        MermanResourceLimitId,
        MermanResourceOptions,
        MermanResourceOptionsBuilder,
        MermanResourceProfile;
export 'src/merman_ffi.dart'
    show
        Merman,
        MermanAsciiCapability,
        MermanAsciiCapabilityEvidence,
        MermanDiagramFamilyCapability,
        MermanException,
        MermanLintRuleCatalogEntry,
        MermanReusableEngine,
        MermanStatus,
        MermanTextDirection,
        MermanTextMeasureRequest,
        MermanTextMeasureResult,
        MermanTextMeasurementPhase,
        MermanTextMeasurer,
        MermanTextWhiteSpace,
        MermanTextWrapMode,
        MermanValidationResult,
        openMermanLibrary,
        openMermanLibraryFromPath;
