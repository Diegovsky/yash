
run:
    cargo run

build-release:
    cargo build --release
    strip target/release/yash
    upx target/release/yash

install-release: build-release
    sudo install target/release/yash /usr/local/bin/yash

