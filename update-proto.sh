#!/bin/sh
set -ex

type wget >/dev/null 2>&1 || { echo >&2 "This requires the wget command to download the protobuf file"; exit 1; }

function vmproto() {
    proto_filename="vm.proto"
    remote_path="https://raw.githubusercontent.com/ava-labs/avalanchego/master/vms/rpcchainvm/vmproto/"
    local_path="./proto/"
    echo "Updating the VM protobuf definition from upstream Avalanche Repo: $remote_path"
    wget ${remote_path}${proto_filename} -O ${local_path}${proto_filename} 
}

function prometheusproto() {
    proto_filename="metrics.proto"
    remote_path="https://raw.githubusercontent.com/prometheus/client_model/master/io/prometheus/client/"
    local_path="./proto/"
    echo "Updating the metrics protobuf definition from upstream Prometheus Client Repo: $remote_path"
    wget ${remote_path}${proto_filename} -O ${local_path}${proto_filename} 
}

vmproto
prometheusproto
