# Installation

Each [GitHub release](https://github.com/smolkov/sprcmtrd/releases) has
prebuilt binaries for Linux (x86_64, aarch64), macOS (Intel, Apple Silicon) and
Windows (x86_64), Debian packages for Linux, and a `SHA256SUMS` file to verify
them (`sha256sum --check --ignore-missing SHA256SUMS`).

**Debian / Ubuntu:** the package installs the binary, the documentation and
tab completion for bash, zsh and fish:

```sh
sudo apt install ./sprcmtrd_0.6.0-1_amd64.deb     # arm64 for aarch64
```

**With Cargo** (Rust 1.88 or newer), from the repository or a checkout:

```sh
cargo install --git https://github.com/smolkov/sprcmtrd --locked
cargo install --path . --locked
```

or build and run from the repository:

```sh
cargo build --release
./target/release/sprcmtrd
```

## Shell completion

Tab completion offers subcommands, options and the COMTRADE recordings in the
typed directory and its subdirectories. The Debian package installs it; for
other installations see [Shell completion](commands/completions.md).
