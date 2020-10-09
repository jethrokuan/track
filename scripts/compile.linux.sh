#!/bin/sh
cargo install cargo-bloat
cargo build --release
cargo bloat --release
strip -s target/release/track
cp target/release/track fluminurs.track
