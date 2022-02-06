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

function gkeystoreproto() {
    proto_filename="gkeystore.proto"
    remote_path="https://raw.githubusercontent.com/ava-labs/avalanchego/master/api/keystore/gkeystore/gkeystoreproto/"
    local_path="./proto/"
    echo "Updating the gkeystoreproto protobuf definition from upstream AvalancheGo Repo: $remote_path"
    wget ${remote_path}${proto_filename} -O ${local_path}${proto_filename} 
}

function gsharedmemoryproto() {
    proto_filename="gsharedmemory.proto"
    remote_path="https://raw.githubusercontent.com/ava-labs/avalanchego/master/chains/atomic/gsharedmemory/gsharedmemoryproto/"
    local_path="./proto/"
    echo "Updating the gsharedmemoryproto protobuf definition from upstream AvalancheGo Repo: $remote_path"
    wget ${remote_path}${proto_filename} -O ${local_path}${proto_filename} 
}

function rpcdbproto() {
    proto_filename="rpcdb.proto"
    remote_path="https://raw.githubusercontent.com/ava-labs/avalanchego/master/database/rpcdb/rpcdbproto/"
    local_path="./proto/"
    echo "Updating the rpcdbproto protobuf definition from upstream AvalancheGo Repo: $remote_path"
    wget ${remote_path}${proto_filename} -O ${local_path}${proto_filename} 
}

function galiasreaderproto() {
    proto_filename="galiasreader.proto"
    remote_path="https://raw.githubusercontent.com/ava-labs/avalanchego/master/ids/galiasreader/galiasreaderproto/"
    local_path="./proto/"
    echo "Updating the galiasreaderproto protobuf definition from upstream AvalancheGo Repo: $remote_path"
    wget ${remote_path}${proto_filename} -O ${local_path}${proto_filename} 
}

function appsenderproto() {
    proto_filename="appsender.proto"
    remote_path="https://raw.githubusercontent.com/ava-labs/avalanchego/master/snow/engine/common/appsender/appsenderproto/"
    local_path="./proto/"
    echo "Updating the appsenderproto protobuf definition from upstream AvalancheGo Repo: $remote_path"
    wget ${remote_path}${proto_filename} -O ${local_path}${proto_filename} 
}

function ghttpproto() {
    proto_filename="ghttp.proto"
    remote_path="https://raw.githubusercontent.com/ava-labs/avalanchego/master/vms/rpcchainvm/ghttp/ghttpproto/"
    local_path="./proto/"
    echo "Updating the ghttpproto protobuf definition from upstream AvalancheGo Repo: $remote_path"
    wget ${remote_path}${proto_filename} -O ${local_path}${proto_filename} 
}

function gsubnetlookupproto() {
    proto_filename="gsubnetlookup.proto"
    remote_path="https://raw.githubusercontent.com/ava-labs/avalanchego/master/vms/rpcchainvm/gsubnetlookup/gsubnetlookupproto/"
    local_path="./proto/"
    echo "Updating the gsubnetlookupproto protobuf definition from upstream AvalancheGo Repo: $remote_path"
    wget ${remote_path}${proto_filename} -O ${local_path}${proto_filename} 
}

function messengerproto() {
    proto_filename="messenger.proto"
    remote_path="https://raw.githubusercontent.com/ava-labs/avalanchego/master/vms/rpcchainvm/messenger/messengerproto/"
    local_path="./proto/"
    echo "Updating the messengerproto protobuf definition from upstream AvalancheGo Repo: $remote_path"
    wget ${remote_path}${proto_filename} -O ${local_path}${proto_filename} 
}

vmproto
prometheusproto
gkeystoreproto
gsharedmemoryproto
rpcdbproto
galiasreaderproto
appsenderproto
ghttpproto
gsubnetlookupproto
messengerproto
