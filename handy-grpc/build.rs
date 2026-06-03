fn main() {
    tonic_prost_build::configure()
        .compile_protos(&["proto/transferpb.proto"], &["proto"])
        .unwrap();
}
