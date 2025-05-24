use std::env;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let lib_dir = root.join("vendor/openvr/lib/win64");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rerun-if-changed=src/openvr_wrapper.cpp");
    println!("cargo:rustc-link-lib=dylib=openvr_api");
    println!("cargo:rustc-link-lib=dylib=d3d11");
    println!("cargo:rustc-link-lib=dylib=dxgi");
    println!("cargo:rustc-link-lib=dylib=d3dcompiler");

    cc::Build::new()
        .file("src/openvr_wrapper.cpp")
        .cpp(true)
        .flag_if_supported("-std=c++17")
        .include("vendor/openvr/headers")
        .include("vendor/imgui")
        .file("vendor/imgui/imgui.cpp")
        .file("vendor/imgui/imgui_draw.cpp")
        .file("vendor/imgui/imgui_tables.cpp")
        .file("vendor/imgui/imgui_widgets.cpp")
        .file("vendor/imgui/backends/imgui_impl_dx11.cpp")
        .compile("openvr_imgui_wrapper");
}