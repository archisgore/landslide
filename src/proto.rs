// This is pulled from vm.proto
pub mod vm_proto {
    tonic::include_proto!("vmproto");
}

// This is pulled from metrics.proto
pub mod io {
    pub mod prometheus {
        pub mod client {
            tonic::include_proto!("io.prometheus.client");
        }
    }
}

pub mod appsender {
    tonic::include_proto!("appsenderproto");
}

pub mod galiasreader {
    tonic::include_proto!("galiasreaderproto");
}

pub mod ghttp {
    tonic::include_proto!("ghttpproto");
}

pub mod gkeystore {
    tonic::include_proto!("gkeystoreproto");
}

pub mod gsharedmemory {
    tonic::include_proto!("gsharedmemoryproto");
}

pub mod gsubnetlookup {
    tonic::include_proto!("gsubnetlookupproto");
}

pub mod messenger {
    tonic::include_proto!("messengerproto");
}

pub mod rpcdb {
    tonic::include_proto!("rpcdbproto");
}

// Copied from: https://github.com/ava-labs/avalanchego/blob/master/snow/engine/common/http_handler.go#L11
// To get a u32 representation of this, just pick any one variant 'as u32'. For example:
//     lock: Lock::WriteLock as u32
pub enum Lock {
    WriteLock,
    ReadLock,
    NoLock,
}
