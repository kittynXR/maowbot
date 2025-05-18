use std::env;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let lib_dir = root.join("vendor/openvr/lib/win64");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rerun-if-changed=src/openvr_wrapper.cpp");
    println!("cargo:rustc-link-lib=dylib=openvr_api");  // import lib
    println!("cargo:rustc-link-lib=dylib=d3d11");
    println!("cargo:rustc-link-lib=dylib=dxgi");

    cc::Build::new()
        .file("src/openvr_wrapper.cpp")
        .cpp(true)
        .flag_if_supported("-std=c++17")
        .include("vendor/openvr/headers")
        .compile("openvr_wrapper");
}
