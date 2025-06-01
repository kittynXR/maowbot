fn main() {
    tonic_build::configure()
        .compile_protos(&["proto/plugin.proto"], &["proto"])
        .unwrap();
}