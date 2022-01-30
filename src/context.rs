use super::vm::InitializeRequest;

// Context copied from:
// https://github.com/ava-labs/avalanchego/blob/master/snow/context.go
#[derive(Debug, Default)]
pub struct Context {
    pub network_id: u32,
    pub subnet_id: Vec<u8>,
    pub chain_id: Vec<u8>,
    pub node_id: Vec<u8>,

    pub x_chain_id: Vec<u8>,
    pub avax_asset_id: Vec<u8>,
}

impl Context {
    pub fn from(ir: InitializeRequest) -> Context {
        Context {
            network_id: ir.network_id,
            subnet_id: ir.subnet_id,
            chain_id: ir.chain_id,
            node_id: ir.node_id,

            x_chain_id: ir.x_chain_id,
            avax_asset_id: ir.avax_asset_id,
        }
    }
}
