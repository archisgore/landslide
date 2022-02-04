// Build the VM's protobuf into a Rust server
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The servers we'll expose
    tonic_build::configure()
        .build_client(false)
        .format(true)
        .compile(
            &[
                "proto/vm.proto",
                "proto/metrics.proto",
                "proto/appsender.proto",
                "proto/galiasreader.proto",
                "proto/ghttp.proto",
                "proto/gkeystore.proto",
                "proto/gsharedmemory.proto",
                "proto/gsubnetlookup.proto",
                "proto/messenger.proto",
                "proto/rpcdb.proto",
            ],
            &["proto"],
        )?;

    // the clients we'll consume
    tonic_build::configure()
        .build_server(false)
        .format(true)
        .compile(
            &[
                "proto/appsender.proto",
                "proto/galiasreader.proto",
                "proto/ghttp.proto",
                "proto/gkeystore.proto",
                "proto/gsharedmemory.proto",
                "proto/gsubnetlookup.proto",
                "proto/messenger.proto",
                "proto/rpcdb.proto",
            ],
            &["proto"],
        )?;

    Ok(())
}
