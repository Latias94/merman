"""Python package shim for generated merman UniFFI bindings."""

try:
    from .merman_uniffi import MermanEngine, MermanError, MermanValidationResult
except ModuleNotFoundError as exc:
    if exc.name == f"{__name__}.merman_uniffi":
        raise ImportError(
            "Generated merman UniFFI bindings are missing. "
            "Run `cargo build -p merman-uniffi --features bindgen-smoke`, then "
            "`cargo run -p merman-uniffi --features bindgen-smoke --example "
            "generate_python_package -- --package-dir platforms/python/merman`."
        ) from exc
    raise

__all__ = ["MermanEngine", "MermanError", "MermanValidationResult"]
