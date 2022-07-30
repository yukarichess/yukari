#!/bin/sh

LLVM_PROFDATA=~/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata

[ -e "$LLVM_PROFDATA" ] || rustup component add llvm-tools-preview

RUSTFLAGS="-Cprofile-generate=/tmp/pgo" cargo build --release --example bench
target/release/examples/bench
$LLVM_PROFDATA merge -o /tmp/pgo/merged.profdata /tmp/pgo
RUSTFLAGS="-Cprofile-use=/tmp/pgo/merged.profdata" cargo build --release --example bench
target/release/examples/bench
