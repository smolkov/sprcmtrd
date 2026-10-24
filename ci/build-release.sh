#!/bin/sh
# Builds the release binary for TARGET on Debian and packs it with the README
# and licenses into dist/sprcmtrd-TAG-TARGET.tar.gz (Linux) or .zip (Windows);
# for Linux also a Debian package (cargo-deb, settings in Cargo.toml).
#
#   ci/build-release.sh x86_64-unknown-linux-gnu v0.2.0
#
# Used by .gitlab-ci.yml; cross-compiles aarch64 Linux and Windows (MinGW).
set -eu

target=$1
tag=$2
name="sprcmtrd-$tag-$target"

case "$target" in
    x86_64-unknown-linux-gnu) packages="" ;;
    aarch64-unknown-linux-gnu) packages="gcc-aarch64-linux-gnu libc6-dev-arm64-cross" ;;
    x86_64-pc-windows-gnu) packages="gcc-mingw-w64-x86-64 zip" ;;
    *) echo "unsupported target $target" >&2; exit 1 ;;
esac
if [ -n "$packages" ]; then
    apt-get update -qq
    # shellcheck disable=SC2086 # word splitting of the package list is intended
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends $packages > /dev/null
fi

rustup target add "$target"
cargo build --release --locked --target "$target"

rm -rf "dist/$name"
mkdir -p "dist/$name"
cp README.md LICENSE-MIT LICENSE-APACHE "dist/$name/"
cd dist
case "$target" in
    *windows*)
        cp "../target/$target/release/sprcmtrd.exe" "$name/"
        zip -qr "$name.zip" "$name"
        ;;
    *)
        cp "../target/$target/release/sprcmtrd" "$name/"
        tar czf "$name.tar.gz" "$name"
        ;;
esac
rm -rf "$name"
cd ..

case "$target" in
    *linux*)
        command -v cargo-deb > /dev/null || cargo install cargo-deb --locked --quiet
        cargo deb --no-build --no-strip --target "$target" --output dist/
        ;;
esac
ls -l dist
