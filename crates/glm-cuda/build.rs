fn main() {
    println!("cargo:rerun-if-env-changed=GLMAXX_KERNEL_LIB_DIR");
    if std::env::var_os("CARGO_FEATURE_CUDA_FFI").is_some() {
        let directory = std::env::var("GLMAXX_KERNEL_LIB_DIR")
            .expect("GLMAXX_KERNEL_LIB_DIR is required with cuda-ffi");
        println!("cargo:rustc-link-search=native={directory}");
        println!("cargo:rustc-link-lib=dylib=glmaxx_sm120");
    }
}
