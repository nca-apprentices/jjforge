use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The contract lives in /shared/proto, shared with the server.
    let proto_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../shared/proto");

    let files = [
        proto_root.join("echo/v1/echo.proto"),
        proto_root.join("source/v1/diff.proto"),
        proto_root.join("source/v1/file.proto"),
        proto_root.join("source/v1/history.proto"),
        proto_root.join("source/v1/repo.proto"),
        proto_root.join("source/v1/session.proto"),
        proto_root.join("store/v1/store.proto"),
        proto_root.join("sync/v1/sync.proto"),
    ];

    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
    }

    let mut config = tonic_prost_build::Config::new();
    config.protoc_executable(protoc_bin_vendored::protoc_bin_path()?);

    // An object's ID is the hash of its bytes, so its maps encode in key
    // order, never in a HashMap's random one.
    tonic_prost_build::configure()
        .btree_map(".store.v1")
        .build_client(true)
        .build_server(true)
        .compile_with_config(config, &files, &[proto_root])?;

    Ok(())
}
