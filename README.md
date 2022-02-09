[![Build Status](https://github.com/archisgore/landslide/actions/workflows/build.yml/badge.svg)](https://github.com/archisgore/landslide/actions/workflows/build.yml)

# A rust-based Custom VM for Avalanche Subnets

Curious about how to run Rust-based smart contracts, or just custom VMs for [Avalanche blockchain](https://www.avax.network/)? You're in the right place.


## Usage

### Build

`cargo build`

### Build and run

In the landslide git root directory:

```.bash
./scripts/build.sh
```

The script will provide instructions on how to run landslide in avalanche.

### Update protobuf definitions from upstream

```.bash
./scripts/update-proto.sh
```

### Test with [ava-sim](https://github.com/ava-labs/ava-sim)

1. Export path to landslide genesis data file, in the environment variable LANDSLIDE_GENESIS_PATH
2. Export path to landslide executable in the environment variable LANDSLIDE_BIN_PATH
2. Clone https://github.com/ava-labs/ava-sim and go in the directory.
3. In ava-sim root, run:
```.bash
./scripts/run.sh $LANDSLIDE_BIN_PATH "$LANDSLIDE_GENESIS_PATH"
```

### Interact with it

Once the VM is launched, all Avalanche's TimestampVM instructions work completely drop-in:
https://docs.avax.network/build/tutorials/platform/subnets/create-a-virtual-machine-vm

You might also want to read how to create a custom blockchain:
https://docs.avax.network/build/tutorials/platform/subnets/create-custom-blockchain


