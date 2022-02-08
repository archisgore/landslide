echo "Building landslide."

echo "Running cargo build - that's really it for the most part..."
cargo build --release

echo
echo
echo
echo
echo "To run Avalanche on this host with Landslide as your Custom VM:"
echo "  1. Export the following environment:"
echo "        export LANDSLIDE_GIT_ROOT=\$PWD"
echo "        export LANDSLIDE_BIN_PATH=\$PWD/target/release/landslide"
echo 
echo "        # Optionally if you want to configure the landslide logger"
echo "        # in the log4rs configuration format"
echo "        # export LANDSLIDE_LOG_CONFIG_FILE=<path/to/log_config_file>"
echo
echo "        # an example file resides under the landslide git root"
echo "        export LANDSLIDE_LOG_CONFIG_FILE=\$PWD/log_config.yml"
echo
echo "  2. In a directory of your choosing, clone ava-sim (the avalanache simulator):"
echo "        git clone https://github.com/ava-labs/ava-sim.git"
echo
echo "  3. Go into the ava-sim directory:"
echo "        cd ava-sim"
echo
echo "  4. Build and run ava-sim with the custom VM"
echo "        ./scripts/build.sh && ./scripts/run.sh \$LANDSLIDE_BIN_PATH \"\$LANDSLIDE_GIT_ROOT/genesis.data\""
echo ""