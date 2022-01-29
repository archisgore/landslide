
// Build the VM's protobuf into a Rust server
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/vm.proto")?;
    Ok(())
}