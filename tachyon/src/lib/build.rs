fn main() {
    #[cfg(feature = "enable-napi")]
    napi_build::setup();
}
