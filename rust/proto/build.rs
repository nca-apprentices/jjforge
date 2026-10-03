use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The contract lives in /shared/proto, shared with the server.
    let proto_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../shared/proto");

    let files = [
        proto_root.join("echo/v1/echo.proto"),
        proto_root.join("kernel/v1/kernel.proto"),
        proto_root.join("store/v1/store.proto"),
        proto_root.join("sync/v1/sync.proto"),
    ];

    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
    }

    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .compile_protos(&files, &[proto_root])?;

    Ok(())
}
