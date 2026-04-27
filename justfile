# Default target for cross-compilation
cross-build TARGET="x86_64-unknown-linux-musl":
    cross build --release --target {{TARGET}}
    @echo "Binary available at: target/{{TARGET}}/release/fireplace"

# Run tests via cross
cross-test TARGET="x86_64-unknown-linux-musl":
    cross test --target {{TARGET}}

# Build release binaries for all targets
build-releases:
    cross build --release --target x86_64-unknown-linux-musl
    cross build --release --target aarch64-unknown-linux-musl
    cross build --release --target x86_64-pc-windows-gnu
    cross build --release --target aarch64-pc-windows-gnu
    cross build --release --target aarch64-apple-darwin
    cross build --release --target wasm32-wasip1
    cross build --release --target x86_64-unknown-freebsd
    @echo "All release builds complete"

# Build with cargo directly (local)
build:
    cargo build --release

# Run tests locally
test:
    cargo test

# Clean build artifacts
clean:
    cargo clean
