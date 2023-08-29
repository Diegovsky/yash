#!/usr/bin/env bash
set -e
cargo build --release 
exe="target/release/$(basename $(pwd))"
strip $exe
upx --best --lzma $exe
