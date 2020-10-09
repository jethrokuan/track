#!/bin/sh
cargo install cargo-bloat
cargo build --release
cargo bloat --release
strip target/release/track
cp target/release/track fluminurs.track
