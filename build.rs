fn main() {
    tonic_build::configure()
        .compile(
            &["proto/plugin.proto"],
            &["proto"],
        )
        .unwrap();
}
