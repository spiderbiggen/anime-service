fn main() {
    tonic_build::configure()
        .compile(
            &["api.proto"], // Files in the path
            &["protos"],    // The path to search
        )
        .unwrap();
}
