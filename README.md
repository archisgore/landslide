# A rust-based Custom VM for Avalanche Subnets

Curious about how to run Rust-based smart contracts, or just custom VMs for [Avalanche blockchain](https://www.avax.network/)? You're in the right place.


## Usage

### Build

Run standard `cargo build`.

### Update protobuf definitions from upstream

```.bash
./scripts/update-proto.sh
```

### Test with [ava-sim](https://github.com/ava-labs/ava-sim)

1. Export path to landslide git repository root, in the environment variable LANDSLIDE_GIT_ROOT
2. Export path to landslide executable in the environment variable LANDSLIDE_BIN_PATH
2. Clone https://github.com/ava-labs/ava-sim and go in the directory.
3. In ava-sim root, run:
```.bash
./scripts/run.sh $LANDSLIDE_BIN_PATH "$LANDSLIDE_GIT_ROOT/genesis"
```