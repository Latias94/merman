/// Flutter and Dart FFI bindings for the `merman` headless Mermaid engine.
///
/// Import this library in Flutter apps that need to render Mermaid source to
/// SVG or ASCII text, inspect parsed diagram JSON, or query binding metadata.
library;

export 'src/merman_ffi.dart'
    show
        Merman,
        MermanException,
        MermanStatus,
        MermanValidationResult,
        mermanAbiVersion,
        openMermanLibrary,
        openMermanLibraryFromPath;
