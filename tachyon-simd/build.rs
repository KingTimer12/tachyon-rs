// Compiles the C++ SIMD code with platform-appropriate flags.
// If simdjson single-header files are present in cpp/vendor/, links them too.

use std::path::PathBuf;

fn main() {
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // Check if simdjson files exist in cpp/vendor/
    let simdjson_dir = PathBuf::from("cpp/vendor");
    let simdjson_h = simdjson_dir.join("simdjson.h");
    let simdjson_cpp = simdjson_dir.join("simdjson.cpp");
    let has_simdjson = simdjson_h.exists() && simdjson_cpp.exists();

    if !has_simdjson {
        println!("cargo:warning=simdjson not found in cpp/vendor/. JSON parsing will use fallback.");
        println!("cargo:warning=To enable: place simdjson.h + simdjson.cpp in crates/tachyon-simd/cpp/vendor/");
    }

    // --- Build the cxx bridge ---
    let mut build = cxx_build::bridge("src/lib.rs");

    build.file("cpp/simd_scan.cpp");
    build.file("cpp/rio.cpp");
    build.include("cpp");

    if has_simdjson {
        build.file(&simdjson_cpp);
        build.include(&simdjson_dir);
        build.define("TACHYON_HAS_SIMDJSON", None);
        build.define("SIMDJSON_EXCEPTIONS", "0");
    }

    build.std("c++17");

    let is_msvc = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default() == "msvc";

    match target_arch.as_str() {
        "x86_64" | "x86" => {
            if is_msvc {
                // MSVC: SSE4.2 is on by default for x64; enable AVX2 explicitly
                build.flag_if_supported("/arch:AVX2");
            } else {
                build.flag_if_supported("-msse4.2");
                build.flag_if_supported("-mavx2");
            }
        }
        "aarch64" => {}
        _ => {
            println!("cargo:warning=No SIMD for arch '{}', scalar fallback", target_arch);
        }
    }

    build.opt_level(3);

    match target_os.as_str() {
        "linux" => { build.define("_GNU_SOURCE", None); }
        "windows" => {
            println!("cargo:rustc-link-lib=ws2_32");
            println!("cargo:rustc-link-lib=mswsock");
        }
        _ => {}
    }

    if has_simdjson {
        build.flag_if_supported("-Wno-unused-parameter");
        build.flag_if_supported("-Wno-missing-field-initializers");
        build.flag_if_supported("-Wno-sign-compare");
    }

    build.compile("tachyon_simd_cpp");

    println!("cargo:rerun-if-changed=cpp/simd_scan.h");
    println!("cargo:rerun-if-changed=cpp/simd_scan.cpp");
    println!("cargo:rerun-if-changed=cpp/rio.h");
    println!("cargo:rerun-if-changed=cpp/rio.cpp");
    println!("cargo:rerun-if-changed=src/lib.rs");
    if has_simdjson {
        println!("cargo:rerun-if-changed=cpp/vendor/simdjson.h");
        println!("cargo:rerun-if-changed=cpp/vendor/simdjson.cpp");
    }
}