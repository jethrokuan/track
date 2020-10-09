cargo install cargo-bloat
cargo build --release
cargo bloat --release
strip -s target/release/track.exe
cp target/release/fluminurs track.windows.exe
