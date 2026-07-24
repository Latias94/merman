fn main() {
    if std::env::var_os("CARGO_FEATURE_TRANSPORT_NAPI").is_some() {
        napi_build::setup();
    }
}
