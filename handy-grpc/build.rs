fn main() {
    tonic_build::configure()
        .compile_protos(&["proto/transferpb.proto"], &["proto"])
        .unwrap();
}
