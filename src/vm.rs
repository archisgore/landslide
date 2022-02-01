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

use crate::vm::vm_proto::vm_server::{Vm, VmServer};
use futures::future::BoxFuture;
use grr_plugin::{Plugin, PluginServer};
use std::clone::Clone;
use std::sync::Arc;
use std::task::{Context, Poll};
use tonic::body::BoxBody;
use tonic::codegen::Never;
use tonic::transport::{Body, NamedService};
use tower::Service;

// Copied from: https://github.com/ava-labs/avalanchego/blob/master/snow/engine/common/http_handler.go#L11
// To get a u32 representation of this, just pick any one variant 'as u32'. For example:
//     lock: Lock::WriteLock as u32
pub enum Lock {
    WriteLock,
    ReadLock,
    NoLock,
}

pub struct PluginVmServer<V: Vm>(Arc<VmServer<V>>);

impl<V: Vm> PluginVmServer<V> {
    pub fn new(vm: V) -> Self {
        PluginVmServer(Arc::new(VmServer::new(vm)))
    }
}

impl<V: Vm> Service<http::Request<Body>> for PluginVmServer<V> {
    type Response = http::Response<BoxBody>;
    type Error = Never;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let inner = std::sync::Arc::<VmServer<V>>::get_mut(&mut self.0).expect("In PluginVmServer, the inner server did not exist in the Arc. This is unprecedented and impossible! Unable to continue.");
        <VmServer<V> as Service<http::Request<Body>>>::poll_ready(inner, cx)
    }
    fn call(&mut self, req: http::Request<Body>) -> Self::Future {
        let inner = std::sync::Arc::<VmServer<V>>::get_mut(&mut self.0).expect("In PluginVmServer, the inner server did not exist in the Arc. This is unprecedented and impossible! Unable to continue.");
        <VmServer<V> as Service<http::Request<Body>>>::call(inner, req)
    }
}

impl<V: Vm> NamedService for PluginVmServer<V> {
    const NAME: &'static str = VmServer::<V>::NAME;
}

impl<V: Vm> Clone for PluginVmServer<V> {
    fn clone(&self) -> Self {
        PluginVmServer(self.0.clone())
    }
}

impl<V: Vm> Plugin for PluginVmServer<V> {}

impl<V: Vm> PluginServer for PluginVmServer<V> {}
