#!/bin/sh
set -ex

type wget >/dev/null 2>&1 || { echo >&2 "This requires the wget command to download the protobuf file"; exit 1; }

proto_filename="vm.proto"
remote_path="https://github.com/ava-labs/avalanchego/blob/master/vms/rpcchainvm/vmproto/"
local_path="./proto/"
echo "Updating the VM protobuf definition from upstream Avalanche Repo: $remote_path"
wget ${remote_path}${proto_filename} -O ${local_path}${proto_filename} 
