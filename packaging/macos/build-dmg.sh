#!/bin/sh
# Build a universal macOS .dmg from the arm64 and x86_64 binaries. Runs on macOS (CI).
# Usage: packaging/macos/build-dmg.sh <arm64-binary> <x86_64-binary> <out-dir>
set -eu

arm64=$1
x86_64=$2
out=$3
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)

stage="$(mktemp -d)/ccswitch"
mkdir -p "$stage" "$out"

lipo -create -output "$stage/ccswitch" "$arm64" "$x86_64"
chmod 755 "$stage/ccswitch"
# Ad-hoc signature: Apple Silicon refuses to run unsigned binaries.
codesign --force --sign - "$stage/ccswitch"

cp "packaging/macos/Install ccswitch.command" "$stage/"
chmod 755 "$stage/Install ccswitch.command"
cp README.md LICENSE "$stage/"

hdiutil create -volname "ccswitch $version" -srcfolder "$stage" -format UDZO -ov \
    "$out/ccswitch-$version-macos-universal.dmg"
