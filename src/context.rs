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
