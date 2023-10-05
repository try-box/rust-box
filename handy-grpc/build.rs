fn main() {
    tonic_build::configure()
        .compile(&["proto/transferpb.proto"], &["proto"])
        .unwrap();
}
