use std::env;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    if target_os == "windows" {
        build_windows(&root);
    } else if target_os == "linux" {
        build_linux(&root);
    } else {
        panic!("Unsupported OS: {}", target_os);
    }
}

fn build_windows(root: &PathBuf) {
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

fn build_linux(root: &PathBuf) {
    // Check if we should use the stub (for testing without VR dependencies)
    if env::var("MAOWBOT_USE_VR_STUB").is_ok() {
        println!("cargo:rerun-if-changed=src/openvr_wrapper_stub.cpp");
        
        cc::Build::new()
            .file("src/openvr_wrapper_stub.cpp")
            .cpp(true)
            .flag_if_supported("-std=c++17")
            .include("vendor/imgui")
            .file("vendor/imgui/imgui.cpp")
            .file("vendor/imgui/imgui_draw.cpp")
            .file("vendor/imgui/imgui_tables.cpp")
            .file("vendor/imgui/imgui_widgets.cpp")
            .compile("openvr_imgui_wrapper");
        
        return;
    }

    let lib_dir = root.join("vendor/openvr/lib/linux64");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rerun-if-changed=src/openvr_wrapper_gl.cpp");
    println!("cargo:rustc-link-lib=dylib=openvr_api");
    println!("cargo:rustc-link-lib=dylib=GL");
    println!("cargo:rustc-link-lib=dylib=GLEW");

    // Use pkg-config to find system libraries
    #[cfg(unix)]
    pkg_config::find_library("gl").ok();
    #[cfg(unix)]
    pkg_config::find_library("glew").ok();

    cc::Build::new()
        .file("src/openvr_wrapper_gl.cpp")
        .cpp(true)
        .flag_if_supported("-std=c++17")
        .include("vendor/openvr/headers")
        .include("vendor/imgui")
        .file("vendor/imgui/imgui.cpp")
        .file("vendor/imgui/imgui_draw.cpp")
        .file("vendor/imgui/imgui_tables.cpp")
        .file("vendor/imgui/imgui_widgets.cpp")
        .file("vendor/imgui/backends/imgui_impl_opengl3.cpp")
        .compile("openvr_imgui_wrapper");
}