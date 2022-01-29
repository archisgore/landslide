// Build the VM's protobuf into a Rust server
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_client(false)
        .format(true)
        .compile(&["proto/vm.proto"], &["proto"])?;

    Ok(())
}
