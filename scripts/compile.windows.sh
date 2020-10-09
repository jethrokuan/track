cargo install cargo-bloat
cargo build --release
cargo bloat --release
strip -s target/release/track.exe
cp target/release/track.exe track.windows.exe
