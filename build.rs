fn main() {
    tonic_build::configure()
        // Where to place generated code inside your src/ tree:
        .out_dir("src/plugins/proto_gen")
        // Path to your proto + an include path (both point at "src/plugins")
        .compile(
            &["src/plugins/plugin_service.proto"],
            &["src/plugins"],
        )
        .unwrap();
}