fn main() {
    #[cfg(feature = "transport-napi")]
    napi_build::setup();
}
