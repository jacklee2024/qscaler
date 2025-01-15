#/bin/bash
# Build the release version of the project
# This script is used to build the release version of the project
# It is used to build the project for the production environment
# The project is built using the musl target to create a static binary
rustup target add x86_64-unknown-linux-musl
sudo apt-get install musl-tools
cargo build --release --target x86_64-unknown-linux-musl
